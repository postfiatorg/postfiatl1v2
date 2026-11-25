"""Fixed-purpose Nitro process entrypoint. No secret files or environment inputs."""

import argparse
import base64
from datetime import datetime, timezone
import json
from pathlib import Path
import socket
import ssl
import sys
import time

from .canonical import canonical_json_bytes, sha256_bytes
from .nitro_archive_transport import NitroCiphertextArchiveClient, _read_frame, _write_frame
from .nitro_kms import NitroKmsEnvelopeEncryptor, decrypt_nitro_ciphertext_bundle, request_attested_secret_ciphertext
from .nitro_kms_transport import AwsTemporaryCredentials, NitroKmsJsonClient
from .nitro_oauth import NitroSchwabTokenProvider
from .nitro_run import NitroTargetRun, private_json, strict, MAX_PRIVATE_BYTES
from .nitro_runtime import NitroSecureModule, decrypt_recipient_ciphertext
from .nitro_transport import NitroHttpsJsonTransport, NitroSchwabApiClient, NitroVsockTlsSocketFactory


# Nitro CLI reserves parent vsock port 9000 for its enclave-ready handshake.
NITRO_RESERVED_PORTS = frozenset({9000})
CONTROL_PORT = 9010
PROGRAM_SHA256 = "9aa40d86cc478a01fd7c6b2a63308188fb7e671079bd3321b405e7ac1a275cff"
ROOT_PATH = Path("/app/aws-nitro-root.der")
ROOT_SHA_PATH = Path("/app/aws-nitro-root.sha256")
MAX_BOOT_BYTES = 64 * 1024


def vsock(port, timeout):
    if type(port) is not int or not 1 <= port <= 65_535 or port in NITRO_RESERVED_PORTS:
        raise ValueError("invalid fixed vsock port")
    if isinstance(timeout, bool) or not isinstance(timeout, (int, float)) or not 0 < timeout <= 3600:
        raise ValueError("invalid transport timeout")
    stream = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
    stream.settimeout(timeout)
    try:
        stream.connect((socket.VMADDR_CID_HOST, port))
        return stream
    except BaseException:
        stream.close()
        raise


def wait_until(scheduled):
    while True:
        remaining = (scheduled - datetime.now(timezone.utc)).total_seconds()
        if remaining <= 0: return
        time.sleep(min(remaining, 1))


def execute(control, *, root_der, nsm, socket_opener=vsock,
            tls_factory=NitroVsockTlsSocketFactory, clock=lambda: datetime.now(timezone.utc),
            scheduler=wait_until):
    """Execute one strictly framed run. Injection points are for local tests only."""
    config = private_json(_read_frame(control, MAX_BOOT_BYTES))
    strict(config, {"schema", "programSha256", "manifest", "parameters", "sources",
        "awsRegion", "awsCredentials", "kmsKeyArn", "encryptionContext",
        "encryptedSchwabCredentialsBase64", "schwabProxyPort", "kmsProxyPort",
        "archivePort", "networkTimeoutSeconds"}, "enclave boot")
    if config["schema"] != "postfiat.yolo.enclave_boot.v1" or config["programSha256"] != PROGRAM_SHA256:
        raise ValueError("boot schema or pinned target program differs")
    ports = [config[name] for name in ("schwabProxyPort", "kmsProxyPort", "archivePort")]
    if (any(type(port) is not int or not 1 <= port <= 65_535 or port in NITRO_RESERVED_PORTS for port in ports)
            or len(set([CONTROL_PORT, *ports])) != 4):
        raise ValueError("fixed-purpose vsock routes must be distinct")
    timeout = config["networkTimeoutSeconds"]
    if isinstance(timeout, bool) or not isinstance(timeout, (int, float)) or not 0 < timeout <= 3600:
        raise ValueError("network timeout must be explicit and bounded")
    control.settimeout(timeout)
    context = config["encryptionContext"]
    if not isinstance(context, dict) or not context or any(
            not isinstance(k, str) or not k or not isinstance(v, str) or not v for k, v in context.items()):
        raise ValueError("explicit KMS encryption context is required")
    credentials = strict(config["awsCredentials"], {"accessKeyId", "secretAccessKey", "sessionToken"}, "AWS credentials")
    aws = AwsTemporaryCredentials(credentials["accessKeyId"], credentials["secretAccessKey"], credentials["sessionToken"])
    ssl_context = ssl.create_default_context(cafile="/etc/ssl/certs/ca-certificates.crt")
    def transport(host, port, bound):
        factory = tls_factory(parent_cid=socket.VMADDR_CID_HOST, proxy_port=port,
            server_hostname=host, ssl_context=ssl_context, timeout_seconds=timeout)
        return NitroHttpsJsonTransport(socket_factory=factory, host=host, max_response_bytes=bound)
    kms = NitroKmsJsonClient(transport(f"kms.{config['awsRegion']}.amazonaws.com", ports[1], 64 * 1024),
                             region=config["awsRegion"], credentials=lambda: aws, clock=clock)
    archive = NitroCiphertextArchiveClient(lambda: socket_opener(ports[2], timeout))
    holder = {}
    def encryptor(document):
        return NitroKmsEnvelopeEncryptor(kms_client=kms, key_id=config["kmsKeyArn"],
            attestation_document=document, recipient_private_key=holder["run"].keys.recipient_private_key,
            encryption_context=context)
    def event(value):
        _write_frame(control, canonical_json_bytes(value), MAX_BOOT_BYTES)
    def packet(kind, sequence, snapshot_hash, document):
        # The signed NSM document contains only public keys and commitments.
        # Export it for independent hardware verification before source release.
        event({"schema": "postfiat.yolo.enclave_source_request.v1", "runId": config["manifest"]["runId"],
               "kind": kind, "sequence": sequence, "privateSnapshotSha256": snapshot_hash,
               "recipientAttestationDocumentBase64": base64.b64encode(document).decode("ascii")})
        bundle = _read_frame(control, MAX_PRIVATE_BYTES)
        aad = canonical_json_bytes({"schema": "postfiat.yolo.source_packet_aad.v1",
            "collectionManifestSha256": holder["run"].manifest_sha256,
            "kind": kind, "sequence": sequence, "privateSnapshotSha256": snapshot_hash})
        return decrypt_nitro_ciphertext_bundle(bundle, associated_data=aad, kms_client=kms,
            attestation_document=document, recipient_private_key=holder["run"].keys.recipient_private_key)
    def load_credentials():
        encoded = config["encryptedSchwabCredentialsBase64"]
        if not isinstance(encoded, str) or len(encoded) > 8192:
            raise ValueError("encrypted credential size outside bound")
        secret = base64.b64decode(encoded, validate=True)
        if not secret or len(secret) > 6144 or base64.b64encode(secret).decode() != encoded:
            raise ValueError("invalid encrypted credential encoding")
        document = holder["run"].recipient_document()
        result = request_attested_secret_ciphertext(kms, key_id=config["kmsKeyArn"],
            encrypted_secret=secret, attestation_document=document, encryption_context=context)
        return private_json(decrypt_recipient_ciphertext(holder["run"].keys.recipient_private_key,
                                                        result.ciphertext_for_recipient))
    rotation = [0]
    def persist_credentials(value):
        rotation[0] += 1
        run = holder["run"]
        aad = canonical_json_bytes({"schema": "postfiat.yolo.credential_rotation_aad.v1",
            "collectionManifestSha256": run.manifest_sha256, "rotation": rotation[0]})
        receipt = run._archive(f"credentials-{rotation[0]}", canonical_json_bytes(value), aad, run.recipient_document())
        event({"schema": "postfiat.yolo.enclave_credential_receipt.v1", "runId": run.manifest["runId"],
               "rotation": rotation[0], "associatedData": json.loads(aad), "receipt": receipt})
    oauth = NitroSchwabTokenProvider(transport("api.schwabapi.com", ports[0], 64 * 1024),
        load_credentials=load_credentials, persist_credentials=persist_credentials,
        account_identity=config["manifest"]["epoch"]["accountApplicationIdentitySha256"])
    schwab = NitroSchwabApiClient(transport("api.schwabapi.com", ports[0], 32 * 1024 * 1024), access_token=oauth)
    run = NitroTargetRun(manifest=config["manifest"], parameters=config["parameters"], sources=config["sources"],
        program_sha256=PROGRAM_SHA256, root_der=root_der, nsm=nsm, encryptor=encryptor,
        archive=archive, source_packet=packet, client=schwab, clock=clock, wait_until=scheduler, event=event)
    holder["run"] = run
    result = run.run()
    event(result)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description="YOLO Nitro target collector; fixed-purpose vsock, encrypted outputs.")
    parser.add_argument("--describe", action="store_true", help="Print public protocol capabilities and exit.")
    args = parser.parse_args(argv)
    if args.describe:
        print(json.dumps({"schema": "postfiat.yolo.enclave_capabilities.v1", "controlPort": CONTROL_PORT,
            "targetProgramSha256": PROGRAM_SHA256, "operations": ["collect-target"],
            "schwabRoutes": ["GET /marketdata/v1/chains", "POST /v1/oauth/token"],
            "kmsOperations": ["DecryptWithRecipient", "GenerateDataKeyWithRecipient"],
            "output": "ciphertext-and-commitments"}, sort_keys=True))
        return 0
    control = None
    try:
        root_der = ROOT_PATH.read_bytes()
        if sha256_bytes(root_der) != ROOT_SHA_PATH.read_text().strip():
            raise ValueError("packaged root checksum differs")
        nsm = NitroSecureModule("/usr/local/lib/libnsm.so")
        control = vsock(CONTROL_PORT, 60)
        execute(control, root_der=root_der, nsm=nsm)
        return 0
    except Exception:
        # Never print traceback/exception text from private-input paths.
        message = {"schema": "postfiat.yolo.enclave_failure.v1", "status": "RUN_FAILED"}
        print(json.dumps(message, sort_keys=True), file=sys.stderr, flush=True)
        return 1
    finally:
        if control is not None: control.close()


if __name__ == "__main__":
    raise SystemExit(main())
