"""Complete local framing + real crypto with synthetic NSM, KMS and Schwab HTTP."""

import base64
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import io
import json
import socket

import pytest
from cryptography.hazmat.primitives import serialization

from test_nav_reading import CUTOFF, PCRS, Transport, manifest
from test_yolo_nitro_attestation import _certificates, _document
from test_yolo_nitro_cms import recipient_envelope
from pftl_schwab_tee import nav_reading_entrypoint as entry
from pftl_schwab_tee.canonical import canonical_json_bytes
from pftl_schwab_tee.nav_reading import validate_manifest, verify_reading
from pftl_schwab_tee.nitro_archive_transport import _read_frame, _write_frame, receive_ciphertext_archive
from pftl_schwab_tee.nitro_attestation import _decode_cbor
from pftl_schwab_tee.private_archive import CiphertextArchive


def test_complete_enclave_protocol_and_ciphertext_only_archive(tmp_path, monkeypatch, manifest):
    import test_yolo_nitro_attestation as certificates_module
    now=datetime.fromtimestamp((CUTOFF+3000)/1000,timezone.utc)
    monkeypatch.setattr(certificates_module,"NOW",now)
    certificates=_certificates()
    arn="arn:aws:kms:us-east-1:000000000000:key/00000000-0000-0000-0000-000000000000"
    secret={"manifest_sha256":validate_manifest(manifest),"credentials":{
        "schema":"postfiat.yolo.schwab_credentials.v1","clientId":"SYNTHETIC-ID",
        "clientSecret":"SYNTHETIC-SECRET","refreshToken":"SYNTHETIC-REFRESH",
        "accountApplicationIdentitySha256":manifest["account_commitment_sha256"]}}
    config={"schema":"postfiat.b23.nav_boot.v1","manifest":manifest,"kmsKeyArn":arn,
        "encryptionContext":{"run":"synthetic"},"encryptedSchwabCredentialsBase64":base64.b64encode(b"sealed").decode(),
        "awsCredentials":{"accessKeyId":"ASIASYNTHETICEXAMPLE","secretAccessKey":"SYNTHETIC-AWS","sessionToken":"SYNTHETIC-SESSION"}}
    calls=[]; archived=[]; bodies=Transport()
    class Nsm:
        def attest(self,*,user_data,nonce,public_key):
            user=json.loads(user_data)
            return _document(timestamp=now,certificates=certificates,nonce=nonce,
                statement_digest=user["statementBindingDigest"],statement_key=user["statementKey"],
                payload_overrides={"public_key":public_key})[0]
    class HttpSocket:
        def __init__(self,host): self.host=host; self.request=b""
        def sendall(self,value): self.request+=value
        def makefile(self,mode):
            headers,body=self.request.split(b"\r\n\r\n",1)
            first=headers.split(b"\r\n")[0]; calls.append((self.host,first))
            if self.host.startswith("kms."):
                request=json.loads(body)
                outer=_decode_cbor(base64.b64decode(request["Recipient"]["AttestationDocument"]))
                payload=_decode_cbor(outer.value[2])
                key=serialization.load_der_public_key(payload["public_key"])
                generate=b"TrentService.GenerateDataKey" in headers
                plaintext=b"d"*32 if generate else canonical_json_bytes(secret)
                result={"KeyId":arn,"CiphertextForRecipient":base64.b64encode(recipient_envelope(key,plaintext)).decode()}
                if generate: result["CiphertextBlob"]=base64.b64encode(b"wrapped").decode()
                else: result["EncryptionAlgorithm"]="SYMMETRIC_DEFAULT"
            elif first==b"POST /v1/oauth/token HTTP/1.1":
                result={"access_token":"SYNTHETIC-ACCESS","refresh_token":"SYNTHETIC-ROTATED","token_type":"Bearer","expires_in":1800}
            elif first==b"GET /trader/v1/accounts/accountNumbers HTTP/1.1": result=bodies.numbers
            elif first==b"GET /trader/v1/accounts/synthetic-account-hash?fields=positions HTTP/1.1": result=bodies.detail
            else: raise AssertionError("unapproved endpoint")
            raw=json.dumps(result,separators=(",",":")).encode()
            return io.BytesIO(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: "+str(len(raw)).encode()+b"\r\n\r\n"+raw)
        def close(self): pass
    def tls_factory(**kwargs):
        assert kwargs["ssl_context"].check_hostname
        assert kwargs["server_hostname"] in {"api.schwabapi.com","kms.us-east-1.amazonaws.com"}
        return lambda:HttpSocket(kwargs["server_hostname"])
    def archive(stream):
        with stream: return receive_ciphertext_archive(stream,CiphertextArchive(tmp_path/"archive"))
    with ThreadPoolExecutor(max_workers=3) as pool:
        def opener(port,timeout):
            assert port==9003
            left,right=socket.socketpair(); left.settimeout(5); right.settimeout(5)
            archived.append(pool.submit(archive,right)); return left
        left,right=socket.socketpair(); left.settimeout(15); right.settimeout(15)
        with left,right:
            result=pool.submit(entry.execute,left,policy=entry.measured_policy(manifest),
                root_der=certificates[1].public_bytes(serialization.Encoding.DER),nsm=Nsm(),
                socket_opener=opener,tls_factory=tls_factory,clock=lambda:now)
            _write_frame(right,canonical_json_bytes(config),entry.MAX_BOOT_BYTES)
            recipient=json.loads(_read_frame(right,entry.MAX_BOOT_BYTES))
            _write_frame(right,canonical_json_bytes({"schema":"postfiat.b23.nav_recipient_accepted.v1",
                "binding_digest":recipient["binding_digest"]}),entry.MAX_BOOT_BYTES)
            envelope=json.loads(_read_frame(right,entry.MAX_BOOT_BYTES))
            assert result.result(timeout=15)==envelope
        paths=[future.result(timeout=5) for future in archived]
    values=verify_reading(envelope,manifest=manifest,expected_pcrs=PCRS,
        root_pem=certificates[1].public_bytes(serialization.Encoding.PEM),now_ms=CUTOFF+4000)
    assert values["attested_value"]==419234350000
    assert len(paths)==3  # OAuth rotation plus both exact account bodies, all encrypted.
    for path in paths:
        contents=path.read_bytes()
        for sensitive in (b"SYNTHETIC-REFRESH",b"SYNTHETIC-SECRET",b"SYNTHETIC-ACCOUNT-ONLY",b"PRIVATE-ONLY"):
            assert sensitive not in contents
    assert [first for host,first in calls if host=="api.schwabapi.com"]==[
        b"POST /v1/oauth/token HTTP/1.1",b"GET /trader/v1/accounts/accountNumbers HTTP/1.1",
        b"GET /trader/v1/accounts/synthetic-account-hash?fields=positions HTTP/1.1"]


def test_boot_cannot_replace_measured_account_policy(manifest):
    policy=entry.measured_policy(manifest)
    manifest={**manifest,"account_commitment_sha256":"bb"*32}
    config={"schema":"postfiat.b23.nav_boot.v1","manifest":manifest,
        "awsCredentials":{},"kmsKeyArn":"synthetic","encryptionContext":{},"encryptedSchwabCredentialsBase64":""}
    left,right=socket.socketpair()
    with left,right:
        _write_frame(right,canonical_json_bytes(config),entry.MAX_BOOT_BYTES)
        with pytest.raises(ValueError,match="measured policy"):
            entry.execute(left,policy=policy,root_der=b"",nsm=None)
