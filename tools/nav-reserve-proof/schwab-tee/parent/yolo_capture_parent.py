"""Real-data capture parent: fixed encrypted relays and public attestations only."""

import argparse
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import threading
import time

from pftl_schwab_tee.canonical import canonical_json_bytes
from pftl_schwab_tee.nitro_archive_transport import _read_frame, _write_frame, receive_ciphertext_archive
from pftl_schwab_tee.nitro_transport import relay_bidirectional
from pftl_schwab_tee.private_archive import CiphertextArchive
from yolo_prover_parent import credentials


def listener(port):
    server = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
    server.bind((socket.VMADDR_CID_ANY, port)); server.listen(8)
    return server


def publish_json(path, value):
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_bytes(canonical_json_bytes(value))
    temporary.replace(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("eif", "boot", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--nav", action="store_true", help="Use the B23 account-value protocol")
    args = parser.parse_args()
    os.umask(0o077)
    args.output.mkdir(parents=True, exist_ok=False, mode=0o700)
    def stop(signum, frame): raise SystemExit(128 + signum)
    signal.signal(signal.SIGTERM, stop)
    config = json.loads(args.boot.read_bytes())
    if "awsCredentials" in config: raise ValueError("boot file contains AWS credentials")
    config["awsCredentials"] = credentials()
    control_listener = listener(9010)
    def proxy(server, host):
        while True:
            left, _ = server.accept()
            def relay(left):
                try:
                    with left, socket.create_connection((host, 443), timeout=30) as right:
                        relay_bidirectional(left, right)
                except Exception as error:
                    print(json.dumps({"relayErrorType": type(error).__name__}), flush=True)
            threading.Thread(target=relay, args=(left,), daemon=True).start()
    for port, host in ((9001, "api.schwabapi.com"), (9002, "kms.us-east-1.amazonaws.com")):
        server = listener(port)
        threading.Thread(target=proxy, args=(server, host), daemon=True).start()
    archive_server = listener(9003); archive = CiphertextArchive(args.output / "archive")
    def receive():
        while True:
            stream, _ = archive_server.accept()
            try:
                with stream: receive_ciphertext_archive(stream, archive)
            except Exception as error:
                print(json.dumps({"archiveErrorType": type(error).__name__}), flush=True)
    threading.Thread(target=receive, daemon=True).start()
    enclave = None
    try:
        with (args.output / "launch.stderr").open("x") as log:
            launch = subprocess.run(["nitro-cli", "run-enclave", "--eif-path", str(args.eif),
                "--cpu-count", "2", "--memory", "4096", "--enclave-cid", "16"],
                stdout=subprocess.PIPE, stderr=log, check=True)
        metadata = json.loads(launch.stdout); enclave = metadata["EnclaveID"]
        (args.output / "launch.json").write_bytes(launch.stdout)
        control_listener.settimeout(90)
        control, _ = control_listener.accept(); control.settimeout(600)
        with control:
            _write_frame(control, canonical_json_bytes(config), 64 * 1024)
            recipient = json.loads(_read_frame(control, 64 * 1024))
            expected_recipient = "postfiat.b23.nav_recipient.v1" if args.nav else "postfiat.yolo.capture_recipient.v1"
            if recipient.get("schema") != expected_recipient:
                raise ValueError("unexpected capture recipient")
            publish_json(args.output / "recipient.json", recipient)
            print(json.dumps({"status": "RECIPIENT_READY"}), flush=True)
            accepted = args.output / "recipient-accepted.json"
            started = time.monotonic()
            while not accepted.exists():
                if time.monotonic() - started > 300: raise TimeoutError("recipient approval not received")
                time.sleep(0.25)
            _write_frame(control, accepted.read_bytes(), 64 * 1024)
            for index in range(20):
                event = json.loads(_read_frame(control, 64 * 1024))
                publish_json(args.output / ("event-" + str(index) + ".json"), event)
                if args.nav:
                    if event.get("schema") != "postfiat.b23.attested_nav_reading.v1":
                        raise ValueError("unexpected NAV event")
                    publish_json(args.output / "result.json", event)
                    print(json.dumps({"status": "NAV_CAPTURED", "verificationPending": True}), flush=True)
                    return 0
                if event.get("schema") == "postfiat.yolo.attested_capture_result.v1":
                    publish_json(args.output / "result.json", event)
                    print(json.dumps({"status": event["body"]["status"]}), flush=True)
                    return 0 if event["body"]["status"] == "CAPTURED" else 1
                if event.get("schema") != "postfiat.yolo.capture_archive.v1":
                    raise ValueError("unexpected capture event")
                print(json.dumps({"status": "ARCHIVED", "artifact": event["artifactId"]}), flush=True)
            raise ValueError("capture exceeded public event bound")
    finally:
        if enclave:
            with (args.output / "terminate.log").open("x") as log:
                subprocess.run(["nitro-cli", "terminate-enclave", "--enclave-id", enclave],
                    stdout=log, stderr=subprocess.STDOUT, check=True)


if __name__ == "__main__":
    raise SystemExit(main())
