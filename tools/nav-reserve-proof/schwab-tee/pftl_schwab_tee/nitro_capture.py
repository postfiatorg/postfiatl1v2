"""Attested on-demand option-chain capture; no portfolio calculation or orders."""

import base64
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import socket
import ssl
import sys

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding, rsa
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

from .canonical import canonical_json_bytes, domain_sha256, sha256_bytes
from .nitro_archive_transport import NitroCiphertextArchiveClient, _read_frame, _write_frame
from .nitro_attestation import NitroNativeAttestationVerifier
from .nitro_entrypoint import vsock, MAX_BOOT_BYTES
from .nitro_kms import request_attested_secret_ciphertext, request_attested_data_key_ciphertext
from .nitro_kms_transport import AwsTemporaryCredentials, NitroKmsJsonClient
from .nitro_oauth import NitroSchwabTokenProvider
from .nitro_run import strict, digest, private_json
from .nitro_runtime import NitroStatementKeys, NitroSecureModule, decrypt_recipient_ciphertext
from .nitro_transport import NitroHttpsJsonTransport, NitroVsockTlsSocketFactory, NitroSchwabApiClient
from .private_archive import encrypt_snapshot


MANIFEST_SCHEMA = "postfiat.yolo.on_demand_capture.v1"
ENVELOPE_SCHEMA = "postfiat.yolo.operator_recoverable_capture.v1"
RESULT_SCHEMA = "postfiat.yolo.on_demand_capture_result.v1"


def manifest_identity(manifest):
    strict(manifest, {"schema", "runId", "symbols", "accountApplicationIdentitySha256",
                      "operatorPublicKeyDerBase64"}, "capture manifest")
    if (manifest["schema"] != MANIFEST_SCHEMA or not isinstance(manifest["runId"], str)
            or not 0 < len(manifest["runId"].encode()) <= 128 or "\0" in manifest["runId"]):
        raise ValueError("invalid capture manifest identity")
    symbols = manifest["symbols"]
    if (not isinstance(symbols, list) or not 1 <= len(symbols) <= 8
            or any(not isinstance(s, str) or not 0 < len(s) <= 32
                   or any(c not in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-" for c in s) for s in symbols)
            or len(set(symbols)) != len(symbols)):
        raise ValueError("capture requires an explicit ordered list of unique symbols")
    digest(manifest["accountApplicationIdentitySha256"])
    encoded = manifest["operatorPublicKeyDerBase64"]
    if not isinstance(encoded, str) or len(encoded) > 2048:
        raise ValueError("operator public key exceeds its bound")
    raw = base64.b64decode(encoded, validate=True)
    if base64.b64encode(raw).decode() != encoded:
        raise ValueError("operator key encoding differs")
    key = serialization.load_der_public_key(raw)
    if not isinstance(key, rsa.RSAPublicKey) or key.key_size not in (2048, 3072, 4096):
        raise ValueError("operator recovery requires a bounded RSA public key")
    return domain_sha256(MANIFEST_SCHEMA, manifest), key


class CaptureEncryptor:
    """A KMS-generated AES key with an additional owner-held recovery wrapper."""

    provider_id = "aws-nitro-kms-aes256-gcm-operator-recovery-v1"

    def __init__(self, *, kms, key_id, context, recipient_keys, document, operator_key):
        self.kms, self.key_id, self.context = kms, key_id, context
        self.recipient_keys, self.document, self.operator_key = recipient_keys, document, operator_key

    def encrypt(self, plaintext, *, associated_data):
        released = request_attested_data_key_ciphertext(self.kms, key_id=self.key_id,
            attestation_document=self.document(), encryption_context=self.context)
        key = decrypt_recipient_ciphertext(self.recipient_keys.recipient_private_key,
            released.ciphertext_for_recipient)
        if len(key) != 32:
            raise ValueError("capture data key must be AES-256")
        nonce = os.urandom(12)
        wrapped = self.operator_key.encrypt(key, padding.OAEP(mgf=padding.MGF1(hashes.SHA256()),
            algorithm=hashes.SHA256(), label=associated_data))
        return canonical_json_bytes({"schema": ENVELOPE_SCHEMA, "kmsKeyId": self.key_id,
            "encryptionContext": self.context,
            "encryptedDataKeyBase64": base64.b64encode(released.encrypted_data_key).decode(),
            "operatorEncryptedDataKeyBase64": base64.b64encode(wrapped).decode(),
            "nonceBase64": base64.b64encode(nonce).decode(),
            "ciphertextBase64": base64.b64encode(AESGCM(key).encrypt(nonce, plaintext, associated_data)).decode()})


def response_summary(payload, response, symbol):
    if response.status_code != 200:
        raise RuntimeError("option-chain HTTP " + str(response.status_code))
    if not isinstance(payload, dict) or payload.get("symbol") != symbol:
        raise ValueError("option-chain response symbol differs")
    counts, timestamps = {}, []
    for side, field in (("CALL", "callExpDateMap"), ("PUT", "putExpDateMap")):
        expirations = payload.get(field)
        if not isinstance(expirations, dict):
            raise ValueError("complete call and put maps are required")
        count = 0
        for strikes in expirations.values():
            if not isinstance(strikes, dict): raise ValueError("invalid strike map")
            for rows in strikes.values():
                if not isinstance(rows, list): raise ValueError("invalid contract rows")
                for row in rows:
                    if not isinstance(row, dict): raise ValueError("invalid contract object")
                    count += 1
                    stamp = row.get("quoteTimeInLong")
                    if type(stamp) is int and stamp > 0: timestamps.append(stamp)
        counts[side] = count
    return {"contractCounts": counts, "quoteTimestampCount": len(timestamps),
        "oldestQuoteTimestampMs": min(timestamps) if timestamps else None,
        "newestQuoteTimestampMs": max(timestamps) if timestamps else None}


def capture_chains(manifest, client, archive, clock):
    """Archive each exact response before proceeding to the next selected symbol."""
    results = []
    for sequence, symbol in enumerate(manifest["symbols"]):
        # Partition by contract type to stay below the provider gateway body limit.
        # CALL plus PUT retains the whole chain without strike/expiration filters.
        for side in ("CALL", "PUT"):
            params = {"symbol": symbol, "contractType": side, "strategy": "SINGLE",
                      "includeUnderlyingQuote": "true"}
            started = clock().isoformat()
            payload, response = client.request("GET", "/marketdata/v1/chains", params=params)
            captured = clock().isoformat()
            # Retain provider error bodies privately too; never send them in public errors.
            private = canonical_json_bytes({"schema": "postfiat.yolo.private_on_demand_response.v1",
                "symbol": symbol, "sequence": sequence, "contractType": side, "request": params,
                "requestStartedAtUtc": started, "capturedAtUtc": captured,
                "statusCode": response.status_code,
                "responseHeaders": {k:v for k,v in response.headers.items()
                    if k.lower() in {"content-type", "date", "x-request-id", "x-correlation-id"}},
                "rawResponseBase64": base64.b64encode(response.content).decode()})
            receipt = archive("chain-" + str(sequence) + "-" + side.lower(), private)
            summary = response_summary(payload, response, symbol)
            results.append({"symbol": symbol, "sequence": sequence, "contractType": side, "capturedAtUtc": captured,
                "httpStatus": response.status_code, "rawResponseSha256": sha256_bytes(response.content),
                "privateResponseSha256": sha256_bytes(private), "archive": receipt, **summary})
    return results


def execute(control, *, manifest, root_der, nsm, socket_opener=vsock,
            tls_factory=NitroVsockTlsSocketFactory, clock=lambda: datetime.now(timezone.utc)):
    manifest_hash, operator_key = manifest_identity(manifest)
    config = private_json(_read_frame(control, MAX_BOOT_BYTES))
    strict(config, {"schema", "awsCredentials", "kmsKeyArn", "encryptionContext",
                   "encryptedSchwabCredentialsBase64"}, "capture boot")
    if config["schema"] != "postfiat.yolo.capture_boot.v1":
        raise ValueError("capture boot schema differs")
    credentials = strict(config["awsCredentials"], {"accessKeyId", "secretAccessKey", "sessionToken"}, "AWS credentials")
    aws = AwsTemporaryCredentials(credentials["accessKeyId"], credentials["secretAccessKey"], credentials["sessionToken"])
    def transport(host, port, bound):
        factory = tls_factory(parent_cid=socket.VMADDR_CID_HOST, proxy_port=port,
            server_hostname=host, ssl_context=ssl.create_default_context(cafile="/etc/ssl/certs/ca-certificates.crt"),
            timeout_seconds=30)
        return NitroHttpsJsonTransport(socket_factory=factory, host=host, max_response_bytes=bound)
    kms = NitroKmsJsonClient(transport("kms.us-east-1.amazonaws.com", 9002, MAX_BOOT_BYTES),
        region="us-east-1", credentials=lambda: aws, clock=clock)
    keys = NitroStatementKeys.generate()
    root = x509.load_der_x509_certificate(root_der)
    verifier = NitroNativeAttestationVerifier(root_certificate_pem=root.public_bytes(serialization.Encoding.PEM),
        max_age_ms=300_000, max_future_skew_ms=5000, clock=clock)
    binding = domain_sha256("postfiat.yolo.capture_recipient.v1",
        {"captureManifestSha256": manifest_hash, "statementKey": keys.statement_key})
    def document(bound=binding):
        value = keys.attest_statement(nsm, attestation_binding_digest=bound)
        parsed = verifier.parse_and_verify(value, attestation_binding_digest=bound)
        if (parsed.statement_key != keys.statement_key or parsed.public_key != keys.recipient_public_key_der
                or any(parsed.pcrs.get(i, b"\0" * 48) == b"\0" * 48 for i in (0, 1, 2))):
            raise ValueError("capture recipient or non-debug measurements differ")
        return value
    def event(value): _write_frame(control, canonical_json_bytes(value), MAX_BOOT_BYTES)
    event({"schema": "postfiat.yolo.capture_recipient.v1", "captureManifestSha256": manifest_hash,
        "bindingDigest": binding, "statementKey": keys.statement_key,
        "documentBase64": base64.b64encode(document()).decode()})
    # The operator verifies this fresh document before enabling credential use.
    acknowledgement = private_json(_read_frame(control, MAX_BOOT_BYTES))
    if acknowledgement != {"schema": "postfiat.yolo.capture_recipient_accepted.v1", "bindingDigest": binding}:
        raise ValueError("recipient acceptance differs")
    archive_client = NitroCiphertextArchiveClient(lambda: socket_opener(9003, 30))
    encryptor = CaptureEncryptor(kms=kms, key_id=config["kmsKeyArn"], context=config["encryptionContext"],
        recipient_keys=keys, document=document, operator_key=operator_key)
    receipts = []
    def archive(name, value):
        aad = {"schema": "postfiat.yolo.capture_archive_aad.v1", "captureManifestSha256": manifest_hash,
               "artifactId": name}
        sealed = encrypt_snapshot(value, associated_data=canonical_json_bytes(aad), encryptor=encryptor)
        receipt = archive_client.store(run_id=manifest["runId"], artifact_id=name, envelope=sealed)
        record = {"artifactId": name, "associatedData": aad, "receipt": receipt}
        receipts.append(record)
        event({"schema": "postfiat.yolo.capture_archive.v1", **record})
        return receipt
    def load_credentials():
        encoded = config["encryptedSchwabCredentialsBase64"]
        if not isinstance(encoded, str) or len(encoded) > 8192: raise ValueError("credential bound")
        release = request_attested_secret_ciphertext(kms, key_id=config["kmsKeyArn"],
            encrypted_secret=base64.b64decode(encoded, validate=True), attestation_document=document(),
            encryption_context=config["encryptionContext"])
        value = private_json(decrypt_recipient_ciphertext(keys.recipient_private_key, release.ciphertext_for_recipient))
        strict(value, {"captureManifestSha256", "credentials"}, "capture credential binding")
        if value["captureManifestSha256"] != manifest_hash: raise ValueError("credential run binding differs")
        return value["credentials"]
    token = NitroSchwabTokenProvider(transport("api.schwabapi.com", 9001, MAX_BOOT_BYTES),
        load_credentials=load_credentials, persist_credentials=lambda x: archive("credentials-1", canonical_json_bytes(x)),
        account_identity=manifest["accountApplicationIdentitySha256"])
    try:
        access_token = token()
        client = NitroSchwabApiClient(transport("api.schwabapi.com", 9001, 32 * 1024 * 1024),
            access_token=lambda: access_token)
        chains = capture_chains(manifest, client, archive, clock)
        body = {"schema": RESULT_SCHEMA, "runId": manifest["runId"], "captureManifestSha256": manifest_hash,
            "status": "CAPTURED", "mode": "ON_DEMAND_NO_PORTFOLIO_CALCULATION",
            "chains": chains, "archives": receipts, "statementKey": keys.statement_key}
    except Exception as error:
        archive("diagnostic", canonical_json_bytes({"exceptionType": type(error).__name__, "detail": str(error)}))
        body = {"schema": RESULT_SCHEMA, "runId": manifest["runId"], "captureManifestSha256": manifest_hash,
            "status": "CAPTURE_FAILED", "exceptionType": type(error).__name__,
            "archives": receipts, "statementKey": keys.statement_key}
    result_binding = domain_sha256(RESULT_SCHEMA, body)
    event({"body": body, "bindingDigest": result_binding, "signature": keys.sign_digest(result_binding),
           "documentBase64": base64.b64encode(document(result_binding)).decode(),
           "schema": "postfiat.yolo.attested_capture_result.v1"})
    return body


def main():
    os.umask(0o077)
    control = vsock(9010, 60)
    try:
        manifest = json.loads(Path("/app/capture-manifest.json").read_bytes())
        result = execute(control, manifest=manifest, root_der=Path("/app/aws-nitro-root.der").read_bytes(),
            nsm=NitroSecureModule("/usr/local/lib/libnsm.so"))
        return 0 if result["status"] == "CAPTURED" else 1
    except Exception:
        print('{"status":"CAPTURE_PROTOCOL_FAILED"}', file=sys.stderr, flush=True)
        return 1
    finally:
        control.close()


if __name__ == "__main__":
    raise SystemExit(main())
