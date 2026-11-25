"""Measured NAV collector using September's NSM, TLS, KMS and encrypted archive."""

import base64
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import socket
import ssl
import sys

from cryptography import x509
from cryptography.hazmat.primitives import serialization

from .canonical import canonical_json_bytes, domain_sha256
from .nav_reading import (MAX_RESPONSE, SchwabAccountClient, attest_reading,
                         collect_account, exact, validate_manifest)
from .nitro_archive_transport import NitroCiphertextArchiveClient, _read_frame, _write_frame
from .nitro_attestation import NitroNativeAttestationVerifier
from .nitro_capture import CaptureEncryptor
from .nitro_entrypoint import vsock, MAX_BOOT_BYTES
from .nitro_kms import request_attested_secret_ciphertext
from .nitro_kms_transport import AwsTemporaryCredentials, NitroKmsJsonClient
from .nitro_oauth import NitroSchwabTokenProvider
from .nitro_run import private_json
from .nitro_runtime import NitroStatementKeys, NitroSecureModule, decrypt_recipient_ciphertext
from .nitro_transport import NitroHttpsJsonTransport, NitroVsockTlsSocketFactory
from .private_archive import encrypt_snapshot

DYNAMIC_FIELDS = {"observation_epoch", "observation_height", "proof_profile_id",
                  "cutoff_unix_ms", "challenge_sha256", "source_manifest_hash", "valuation_policy_hash"}


def measured_policy(manifest):
    validate_manifest(manifest)
    return {key: value for key, value in manifest.items() if key not in DYNAMIC_FIELDS}


def execute(control, *, policy, root_der, nsm, socket_opener=vsock,
            tls_factory=NitroVsockTlsSocketFactory, clock=lambda: datetime.now(timezone.utc)):
    config = private_json(_read_frame(control, MAX_BOOT_BYTES))
    exact(config, {"schema", "manifest", "awsCredentials", "kmsKeyArn", "encryptionContext",
                   "encryptedSchwabCredentialsBase64"}, "NAV boot")
    if config["schema"] != "postfiat.b23.nav_boot.v1":
        raise ValueError("NAV boot schema differs")
    manifest = config["manifest"]
    manifest_hash = validate_manifest(manifest)
    if measured_policy(manifest) != policy:
        raise ValueError("boot manifest differs from measured policy")
    creds = exact(config["awsCredentials"], {"accessKeyId", "secretAccessKey", "sessionToken"}, "role credentials")
    aws = AwsTemporaryCredentials(creds["accessKeyId"], creds["secretAccessKey"], creds["sessionToken"])
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
    binding = domain_sha256("postfiat.b23.nav_recipient.v1",
        {"manifest_sha256": manifest_hash, "statement_key": keys.statement_key})
    def document():
        value = keys.attest_statement(nsm, attestation_binding_digest=binding)
        parsed = verifier.parse_and_verify(value, attestation_binding_digest=binding)
        if (parsed.statement_key != keys.statement_key or parsed.public_key != keys.recipient_public_key_der
                or any(parsed.pcrs.get(i, bytes(48)) == bytes(48) for i in (0, 1, 2))):
            raise ValueError("NAV recipient or non-debug measurements differ")
        return value, parsed
    def event(value):
        _write_frame(control, canonical_json_bytes(value), MAX_BOOT_BYTES)
    recipient, _ = document()
    event({"schema": "postfiat.b23.nav_recipient.v1", "manifest_sha256": manifest_hash,
           "binding_digest": binding, "statement_key": keys.statement_key,
           "document_base64": base64.b64encode(recipient).decode()})
    # Operator verifies pinned root, PCRs, manifest and ephemeral key before ACK.
    if private_json(_read_frame(control, MAX_BOOT_BYTES)) != {
            "schema": "postfiat.b23.nav_recipient_accepted.v1", "binding_digest": binding}:
        raise ValueError("NAV recipient acceptance differs")
    archive_client = NitroCiphertextArchiveClient(lambda: socket_opener(9003, 30))
    operator_key = serialization.load_der_public_key(base64.b64decode(manifest["operator_public_key_der_base64"], validate=True))
    encryptor = CaptureEncryptor(kms=kms, key_id=config["kmsKeyArn"], context=config["encryptionContext"],
        recipient_keys=keys, document=lambda: document()[0], operator_key=operator_key)
    def archive(name, plaintext):
        aad = {"schema": "postfiat.b23.nav_archive.v1", "manifest_sha256": manifest_hash, "artifact_id": name}
        sealed = encrypt_snapshot(plaintext, associated_data=canonical_json_bytes(aad), encryptor=encryptor)
        return archive_client.store(run_id=manifest_hash, artifact_id=name, envelope=sealed)
    def credentials():
        encoded = config["encryptedSchwabCredentialsBase64"]
        if not isinstance(encoded, str) or len(encoded) > 8192:
            raise ValueError("encrypted credential bound")
        released = request_attested_secret_ciphertext(kms, key_id=config["kmsKeyArn"],
            encrypted_secret=base64.b64decode(encoded, validate=True), attestation_document=document()[0],
            encryption_context=config["encryptionContext"])
        value = private_json(decrypt_recipient_ciphertext(keys.recipient_private_key, released.ciphertext_for_recipient))
        exact(value, {"manifest_sha256", "credentials"}, "credential binding")
        if value["manifest_sha256"] != manifest_hash:
            raise ValueError("wrong credential manifest")
        return value["credentials"]
    token = NitroSchwabTokenProvider(transport("api.schwabapi.com", 9001, 64 * 1024),
        load_credentials=credentials, persist_credentials=lambda v: archive("oauth-rotation", canonical_json_bytes(v)),
        account_identity=manifest["account_commitment_sha256"])
    access = token()
    client = SchwabAccountClient(transport("api.schwabapi.com", 9001, MAX_RESPONSE), lambda: access)
    # No host wall clock is used to date the account reading: obtain signed NSM time.
    reading = collect_account(manifest, client, clock_ms=lambda: document()[1].timestamp_ms, archive=archive)
    result = attest_reading(manifest, reading, keys, nsm)
    event(result)
    return result


def main():
    os.umask(0o077)
    control = vsock(9010, 60)
    try:
        execute(control, policy=json.loads(Path("/app/nav-policy.json").read_bytes()),
            root_der=Path("/app/aws-nitro-root.der").read_bytes(),
            nsm=NitroSecureModule("/usr/local/lib/libnsm.so"))
        return 0
    except Exception as error:
        # Provider/JSON errors may carry account data. Emit only a type, never text.
        print(json.dumps({"status": "NAV_CAPTURE_FAILED", "error_type": type(error).__name__}), file=sys.stderr)
        return 1
    finally:
        control.close()


if __name__ == "__main__":
    raise SystemExit(main())
