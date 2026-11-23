"""Hardware-only synthetic fixture entrypoint, excluded from the collector EIF.

Uses the production NSM, verifier, TLS, SigV4, CMS and archive code. Its distinct
PCRs must never be presented as measurements of the production collector.
No credential plaintext, data key or enclave private key is returned.
"""
import base64
from datetime import datetime, timezone
import json
from pathlib import Path
import ssl

from cryptography import x509
from cryptography.hazmat.primitives import serialization

from pftl_schwab_tee.canonical import canonical_json_bytes, sha256_bytes
from pftl_schwab_tee.nitro_archive_transport import _read_frame, _write_frame
from pftl_schwab_tee.nitro_attestation import NitroNativeAttestationVerifier
from pftl_schwab_tee.nitro_entrypoint import vsock
from pftl_schwab_tee.nitro_kms import (NitroKmsEnvelopeEncryptor,
    decrypt_nitro_ciphertext_bundle, request_attested_secret_ciphertext)
from pftl_schwab_tee.nitro_kms_transport import AwsTemporaryCredentials, NitroKmsJsonClient
from pftl_schwab_tee.nitro_runtime import NitroSecureModule, NitroStatementKeys, decrypt_recipient_ciphertext
from pftl_schwab_tee.nitro_transport import NitroVsockTlsSocketFactory, NitroHttpsJsonTransport


BOUND = 64 * 1024
SYNTHETIC = b"YOLO AWS Nitro qualification synthetic secret v1"


def main():
    control = vsock(9010, 600)
    def send(value):
        _write_frame(control, canonical_json_bytes(value), BOUND)
    boot = json.loads(_read_frame(control, BOUND))
    binding = boot['bindingDigest']
    keys = NitroStatementKeys.generate()
    nsm = NitroSecureModule('/usr/local/lib/libnsm.so')
    root = x509.load_der_x509_certificate(Path('/app/aws-nitro-root.der').read_bytes())
    verifier = NitroNativeAttestationVerifier(root_certificate_pem=root.public_bytes(serialization.Encoding.PEM),
        max_age_ms=300_000, max_future_skew_ms=5000)
    document = keys.attest_statement(nsm, attestation_binding_digest=binding)
    try:
        parsed = verifier.parse_and_verify(document, attestation_binding_digest=binding)
    except ValueError as exc:
        send({'schema': 'postfiat.yolo.hardware_attestation.v1', 'bindingDigest': binding,
            'documentBase64': base64.b64encode(document).decode(), 'statementKey': keys.statement_key,
            'pcrs': {}, 'nativeSignatureAndChainVerified': False, 'validationError': str(exc)})
        return
    assert parsed.statement_key == keys.statement_key
    assert parsed.public_key == keys.recipient_public_key_der
    send({'schema': 'postfiat.yolo.hardware_attestation.v1', 'bindingDigest': binding,
        'documentBase64': base64.b64encode(document).decode(), 'statementKey': keys.statement_key,
        'pcrs': {str(i): v.hex() for i, v in parsed.pcrs.items()},
        'nativeSignatureAndChainVerified': True})
    for _ in range(32):
        cmd = json.loads(_read_frame(control, BOUND))
        action = cmd['action']
        if action == 'stop':
            send({'action': action, 'status': 'OK'})
            return
        aws = cmd['awsCredentials']
        credentials = AwsTemporaryCredentials(aws['AccessKeyId'], aws['SecretAccessKey'], aws['Token'])
        region = 'us-east-1'
        host = f'kms.{region}.amazonaws.com'
        factory = NitroVsockTlsSocketFactory(parent_cid=3, proxy_port=9002,
            server_hostname=host, ssl_context=ssl.create_default_context(cafile='/etc/ssl/certs/ca-certificates.crt'),
            timeout_seconds=20)
        transport = NitroHttpsJsonTransport(socket_factory=factory, host=host, max_response_bytes=BOUND)
        # Capture only the AWS error identifier from the authenticated response.
        # Request headers, tokens and response messages are never exported.
        codes = []
        original = transport.request
        def request(*args, **kwargs):
            response = original(*args, **kwargs)
            if response.status_code != 200:
                body = json.loads(response.content)
                code = body.get('__type', '').split('#')[-1]
                if code and code.isascii() and code.isalnum(): codes.append(code)
            return response
        transport.request = request
        kms = NitroKmsJsonClient(transport, region=region, credentials=lambda: credentials,
            clock=lambda: datetime.now(timezone.utc))
        used_document = document
        if action == 'altered-document':
            used_document = document[:-1] + bytes([document[-1] ^ 1])
        try:
            if action in ('decrypt', 'altered-document'):
                result = request_attested_secret_ciphertext(kms, key_id=cmd['keyArn'],
                    encrypted_secret=base64.b64decode(cmd['ciphertextBase64']),
                    attestation_document=used_document, encryption_context=cmd['encryptionContext'])
                plain = decrypt_recipient_ciphertext(keys.recipient_private_key, result.ciphertext_for_recipient)
                assert plain == SYNTHETIC
                send({'action': action, 'status': 'OK', 'syntheticPlaintextMatches': True,
                    'recipientCiphertextBytes': len(result.ciphertext_for_recipient), 'plaintextExported': False})
            elif action == 'archive-roundtrip':
                encryptor = NitroKmsEnvelopeEncryptor(kms_client=kms, key_id=cmd['keyArn'],
                    attestation_document=document, recipient_private_key=keys.recipient_private_key,
                    encryption_context=cmd['encryptionContext'])
                aad = canonical_json_bytes({'schema': 'postfiat.yolo.hardware_archive_aad.v1', 'bindingDigest': binding})
                bundle = encryptor.encrypt(SYNTHETIC, associated_data=aad)
                plain = decrypt_nitro_ciphertext_bundle(bundle, associated_data=aad, kms_client=kms,
                    attestation_document=document, recipient_private_key=keys.recipient_private_key)
                assert plain == SYNTHETIC
                send({'action': action, 'status': 'OK', 'syntheticPlaintextMatches': True,
                    'ciphertextBundleSha256': sha256_bytes(bundle), 'plaintextExported': False,
                    'ciphertextBundleBase64': base64.b64encode(bundle).decode(),
                    'associatedDataBase64': base64.b64encode(aad).decode()})
            elif action == 'archive-replay':
                bundle = base64.b64decode(cmd['ciphertextBundleBase64'], validate=True)
                aad = base64.b64decode(cmd['associatedDataBase64'], validate=True)
                plain = decrypt_nitro_ciphertext_bundle(bundle, associated_data=aad, kms_client=kms,
                    attestation_document=document, recipient_private_key=keys.recipient_private_key)
                assert plain == SYNTHETIC
                send({'action': action, 'status': 'OK', 'syntheticPlaintextMatches': True,
                    'ciphertextBundleSha256': sha256_bytes(bundle), 'plaintextExported': False})
            else:
                raise ValueError('unsupported hardware fixture operation')
        except Exception as exc:
            send({'action': action, 'status': 'REJECTED', 'exceptionType': type(exc).__name__,
                'awsErrorCodes': codes})


if __name__ == '__main__':
    main()
