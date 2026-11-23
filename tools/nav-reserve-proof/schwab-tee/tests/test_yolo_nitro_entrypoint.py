"""Real framed process protocol with synthetic NSM/KMS and HTTP responses."""
import base64
from concurrent.futures import ThreadPoolExecutor
import io
import json
import os
import socket

import pytest
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

from test_yolo_nitro_attestation import _document
from test_yolo_nitro_cms import recipient_envelope
from test_yolo_nitro_run import Harness, PROGRAM
from pftl_schwab_tee import nitro_entrypoint as entry
from pftl_schwab_tee.canonical import canonical_json_bytes
from pftl_schwab_tee.nitro_archive_transport import _read_frame, _write_frame, receive_ciphertext_archive
from pftl_schwab_tee.nitro_attestation import _decode_cbor
from pftl_schwab_tee.nitro_kms import NITRO_CIPHERTEXT_BUNDLE_SCHEMA
from pftl_schwab_tee.private_archive import CiphertextArchive


def test_complete_entrypoint_framing_recipient_crypto_oauth_and_durable_archive(tmp_path):
    fixture = Harness()
    key = 'arn:aws:kms:us-east-1:111122223333:key/12345678-1234-1234-1234-123456789012'
    context = {'runId': fixture.manifest['runId']}
    secret = {'schema': 'postfiat.yolo.schwab_credentials.v1', 'clientId': 'synthetic-client',
        'clientSecret': 'synthetic-secret', 'refreshToken': 'synthetic-refresh',
        'accountApplicationIdentitySha256': 'bb' * 32}
    config = {'schema': 'postfiat.yolo.enclave_boot.v1', 'programSha256': PROGRAM,
        'manifest': fixture.manifest, 'parameters': fixture.run.parameters.to_dict(),
        'sources': fixture.run.sources, 'awsRegion': 'us-east-1',
        'awsCredentials': {'accessKeyId': 'ASIASYNTHETICEXAMPLE', 'secretAccessKey': 'synthetic-secret', 'sessionToken': 'synthetic-session'},
        'kmsKeyArn': key, 'encryptionContext': context,
        'encryptedSchwabCredentialsBase64': base64.b64encode(b'credential-blob').decode(),
        'schwabProxyPort': 9001, 'kmsProxyPort': 9002, 'archivePort': 9003, 'networkTimeoutSeconds': 15}
    dek = b'd' * 32
    wire_requests, futures = [], []

    class Nsm:
        def attest(self, *, user_data, nonce, public_key):
            body = json.loads(user_data)
            return _document(timestamp=fixture.clock_value, certificates=fixture.certificates,
                recipient_key=fixture.run.keys.recipient_private_key, nonce=nonce,
                statement_key=body['statementKey'], statement_digest=body['statementBindingDigest'],
                payload_overrides={'public_key': public_key})[0]

    class HttpSocket:
        def __init__(self, host): self.host, self.request = host, b''
        def sendall(self, value): self.request += value
        def makefile(self, mode):
            headers, body = self.request.split(b'\r\n\r\n', 1)
            wire_requests.append((self.host, headers.split(b'\r\n')[0]))
            if self.host.startswith('kms.'):
                value = json.loads(body)
                cose = _decode_cbor(base64.b64decode(value['Recipient']['AttestationDocument']))
                payload = _decode_cbor(cose.value[2])
                public = serialization.load_der_public_key(payload['public_key'])
                generate = b'TrentService.GenerateDataKey' in headers
                plaintext = canonical_json_bytes(secret) if value.get('CiphertextBlob') == base64.b64encode(b'credential-blob').decode() else dek
                result = {'KeyId': key, 'CiphertextForRecipient': base64.b64encode(recipient_envelope(public, plaintext)).decode()}
                if generate: result['CiphertextBlob'] = base64.b64encode(b'wrapped-key').decode()
                else: result['EncryptionAlgorithm'] = 'SYMMETRIC_DEFAULT'
            elif b'POST /v1/oauth/token ' in headers:
                result = {'access_token': 'synthetic-access', 'refresh_token': 'synthetic-rotated', 'token_type': 'Bearer', 'expires_in': 1800}
            else:
                result, _ = fixture.request('GET', '/marketdata/v1/chains')
            encoded = canonical_json_bytes(result)
            return io.BytesIO(b'HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ' + str(len(encoded)).encode() + b'\r\n\r\n' + encoded)
        def close(self): pass

    def tls_factory(**kwargs):
        assert kwargs['ssl_context'].check_hostname
        return lambda: HttpSocket(kwargs['server_hostname'])

    def store(stream):
        with stream: return receive_ciphertext_archive(stream, CiphertextArchive(tmp_path / 'archive'))

    with ThreadPoolExecutor(max_workers=3) as pool:
        def opener(port, timeout):
            assert port == 9003
            client, server = socket.socketpair()
            client.settimeout(timeout); server.settimeout(timeout)
            futures.append(pool.submit(store, server))
            return client
        client, server = socket.socketpair()
        client.settimeout(15); server.settimeout(15)
        with client, server:
            task = pool.submit(entry.execute, client, root_der=fixture.run.root_der, nsm=Nsm(),
                socket_opener=opener, tls_factory=tls_factory, clock=lambda: fixture.clock_value, scheduler=fixture.wait)
            _write_frame(server, canonical_json_bytes(config), entry.MAX_BOOT_BYTES)
            events = []
            while True:
                try: event = json.loads(_read_frame(server, entry.MAX_BOOT_BYTES))
                except TimeoutError:
                    task.result(timeout=1)
                    raise
                events.append(event)
                if event['schema'] == 'postfiat.yolo.enclave_source_request.v1':
                    assert base64.b64decode(event['recipientAttestationDocumentBase64'], validate=True)
                    kind, sequence, snapshot_hash = event['kind'], event['sequence'], event['privateSnapshotSha256']
                    if kind == 'normalization':
                        fixture.archived[f'snapshot-{sequence}'] = True
                        fixture.events.append({'status': 'SNAPSHOT_ARCHIVED'})
                    packet = fixture.packet(kind, sequence, snapshot_hash, b'')
                    aad = canonical_json_bytes({'schema': 'postfiat.yolo.source_packet_aad.v1',
                        'collectionManifestSha256': fixture.run.manifest_sha256, 'kind': kind,
                        'sequence': sequence, 'privateSnapshotSha256': snapshot_hash})
                    nonce = os.urandom(12)
                    bundle = {'schema': NITRO_CIPHERTEXT_BUNDLE_SCHEMA, 'kmsKeyId': key, 'encryptionContext': context,
                        'encryptedDataKeyBase64': base64.b64encode(b'wrapped-key').decode(),
                        'nonceBase64': base64.b64encode(nonce).decode(),
                        'ciphertextBase64': base64.b64encode(AESGCM(dek).encrypt(nonce, packet, aad)).decode()}
                    _write_frame(server, canonical_json_bytes(bundle), entry.MAX_PRIVATE_BYTES)
                if event.get('status') in ('SEALED', 'COLLECTION_INCOMPLETE'): break
            result = task.result(timeout=5)
        archives = [future.result(timeout=5) for future in futures]
    assert result['status'] == 'SEALED' and result['statementCount'] == 6
    assert len(archives) == 11 and len(set(archives)) == 11
    assert len(fixture.calls) == 5
    assert sum(b'POST /v1/oauth/token ' in line for _, line in wire_requests) == 5
    public = canonical_json_bytes(events)
    for private in (b'synthetic-secret', b'synthetic-access', b'synthetic-refresh', b'callExpDateMap', b'settledCashMicrodollars'):
        assert private not in public
    for path in archives: assert path.stat().st_mode & 0o777 == 0o600


def test_process_failure_never_prints_private_exception(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr(entry, 'ROOT_PATH', tmp_path / 'private-token-that-must-not-print')
    assert entry.main([]) == 1
    output = capsys.readouterr()
    assert output.out == ''
    assert json.loads(output.err) == {'schema': 'postfiat.yolo.enclave_failure.v1', 'status': 'RUN_FAILED'}


def test_process_capabilities_are_fixed_and_public(capsys):
    assert entry.main(['--describe']) == 0
    value = json.loads(capsys.readouterr().out)
    assert value['schwabRoutes'] == ['GET /marketdata/v1/chains', 'POST /v1/oauth/token']
    assert value['targetProgramSha256'] == PROGRAM
    assert value['controlPort'] == 9010


def test_control_channel_cannot_collide_with_nitro_startup(monkeypatch):
    def forbidden(*args, **kwargs):
        raise AssertionError('reserved port reached socket allocation')
    monkeypatch.setattr(entry.socket, 'socket', forbidden)
    assert entry.CONTROL_PORT not in entry.NITRO_RESERVED_PORTS
    with pytest.raises(ValueError, match='vsock port'):
        entry.vsock(9000, 1)


@pytest.mark.parametrize('route', ['schwabProxyPort', 'kmsProxyPort', 'archivePort'])
@pytest.mark.parametrize('port', [9000, 9010])
def test_boot_rejects_routes_that_collide_with_nitro_or_control(route, port):
    config = dict.fromkeys(['manifest', 'parameters', 'sources', 'awsRegion', 'awsCredentials',
        'kmsKeyArn', 'encryptionContext', 'encryptedSchwabCredentialsBase64'])
    config.update(schema='postfiat.yolo.enclave_boot.v1', programSha256=PROGRAM,
        schwabProxyPort=9001, kmsProxyPort=9002, archivePort=9003, networkTimeoutSeconds=1)
    config[route] = port
    client, server = socket.socketpair()
    with client, server:
        _write_frame(server, canonical_json_bytes(config), entry.MAX_BOOT_BYTES)
        with pytest.raises(ValueError, match='vsock routes'):
            entry.execute(client, root_der=b'', nsm=None)


@pytest.mark.parametrize('hostname,accepted', [('api.schwabapi.com', True), ('kms.us-east-1.amazonaws.com', False)])
def test_enclave_tls_handshake_authenticates_fixed_upstream(tmp_path, monkeypatch, hostname, accepted):
    from datetime import datetime, timedelta, timezone
    import ssl
    import types
    from cryptography import x509
    from cryptography.hazmat.primitives import hashes
    from cryptography.hazmat.primitives.asymmetric import rsa
    from cryptography.x509.oid import NameOID
    from pftl_schwab_tee import nitro_transport
    private = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, 'synthetic TLS root')])
    now = datetime.now(timezone.utc)
    cert = (x509.CertificateBuilder().subject_name(name).issuer_name(name).public_key(private.public_key())
        .serial_number(1).not_valid_before(now - timedelta(minutes=1)).not_valid_after(now + timedelta(minutes=5))
        .add_extension(x509.BasicConstraints(ca=True, path_length=None), critical=True)
        .add_extension(x509.SubjectAlternativeName([x509.DNSName('api.schwabapi.com')]), critical=False)
        .sign(private, hashes.SHA256()))
    cert_path, key_path = tmp_path / 'cert.pem', tmp_path / 'key.pem'
    cert_path.write_bytes(cert.public_bytes(serialization.Encoding.PEM))
    key_path.write_bytes(private.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))
    server_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    server_context.load_cert_chain(cert_path, key_path)
    client_context = ssl.create_default_context(cafile=str(cert_path))
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0)); listener.listen(); listener.settimeout(5)
        destination = listener.getsockname()
        class RedirectedSocket(socket.socket):
            def connect(self, address):
                assert address == (3, 9001)
                return super().connect(destination)
        monkeypatch.setattr(nitro_transport, 'socket', types.SimpleNamespace(
            socket=lambda *args: RedirectedSocket(), AF_VSOCK=socket.AF_VSOCK, SOCK_STREAM=socket.SOCK_STREAM))
        def serve():
            raw, _ = listener.accept()
            with raw:
                try:
                    with server_context.wrap_socket(raw, server_side=True) as secure:
                        assert secure.recv(4) == b'ping'; secure.sendall(b'pong')
                except ssl.SSLError:
                    assert not accepted
        with ThreadPoolExecutor(max_workers=1) as pool:
            task = pool.submit(serve)
            factory = nitro_transport.NitroVsockTlsSocketFactory(parent_cid=3, proxy_port=9001,
                server_hostname=hostname, ssl_context=client_context, timeout_seconds=5)
            if accepted:
                with factory() as secure:
                    secure.sendall(b'ping'); assert secure.recv(4) == b'pong'
            else:
                with pytest.raises(ssl.SSLCertVerificationError): factory()
            task.result(timeout=5)
