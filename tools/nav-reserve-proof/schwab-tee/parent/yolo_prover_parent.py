"""Opt-in Nitro qualification CLI: encrypted replay in, public proof out.

Requires the committed parent transport modules on PYTHONPATH. Boot JSON holds
public run settings only; temporary role credentials are fetched from IMDSv2.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import socket
import signal
import subprocess
import threading
import time
import urllib.request

from pftl_schwab_tee.canonical import canonical_json_bytes
from pftl_schwab_tee.nitro_archive_transport import receive_ciphertext_archive
from pftl_schwab_tee.nitro_prover_parent import exchange
from pftl_schwab_tee.nitro_transport import relay_bidirectional
from pftl_schwab_tee.private_archive import CiphertextArchive


def credentials():
    base = 'http://169.254.169.254/latest/'
    token = urllib.request.urlopen(urllib.request.Request(base + 'api/token', method='PUT',
        headers={'X-aws-ec2-metadata-token-ttl-seconds': '60'}), timeout=5).read().decode()
    request = urllib.request.Request(base + 'meta-data/iam/security-credentials/yolo-nitro-parent-v1',
        headers={'X-aws-ec2-metadata-token': token})
    value = json.loads(urllib.request.urlopen(request, timeout=5).read())
    return {'accessKeyId': value['AccessKeyId'], 'secretAccessKey': value['SecretAccessKey'],
            'sessionToken': value['Token']}


def listener(port):
    stream = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
    stream.bind((socket.VMADDR_CID_ANY, port))
    stream.listen(8)
    return stream


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('eif', 'boot', 'pins', 'circuits', 'ciphertext', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--cpu-count', type=int, required=True)
    parser.add_argument('--memory-mib', type=int, required=True)
    args = parser.parse_args()
    def stop(signum, frame):
        raise SystemExit(128 + signum)
    signal.signal(signal.SIGTERM, stop)
    os.umask(0o077)
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    config = json.loads(args.boot.read_bytes())
    if 'awsCredentials' in config:
        raise ValueError('boot file must not contain AWS credentials')
    pins = json.loads(args.pins.read_bytes())
    control_listener = listener(9010)
    kms_listener = listener(config['kmsProxyPort'])
    archive_listener = listener(config['archivePort'])
    archive = CiphertextArchive(args.output / 'archive')
    def kms_relay():
        while True:
            left, _ = kms_listener.accept()
            try:
                with left, socket.create_connection(('kms.us-east-1.amazonaws.com', 443), timeout=60) as right:
                    relay_bidirectional(left, right)
            except OSError as error:
                print(json.dumps({'relayErrorType': type(error).__name__}), flush=True)
    def archive_relay():
        while True:
            stream, _ = archive_listener.accept()
            with stream:
                receive_ciphertext_archive(stream, archive)
    threading.Thread(target=kms_relay, daemon=True).start()
    threading.Thread(target=archive_relay, daemon=True).start()
    started, enclave_id = time.monotonic(), None
    try:
        command = ['nitro-cli', 'run-enclave', '--eif-path', str(args.eif), '--cpu-count',
            str(args.cpu_count), '--memory', str(args.memory_mib), '--enclave-cid', '16']
        with (args.output / 'launch.stderr.log').open('x') as log:
            launch = subprocess.run(command, stdout=subprocess.PIPE, stderr=log, check=True)
        metadata = json.loads(launch.stdout)
        enclave_id = metadata['EnclaveID']
        (args.output / 'launch.json').write_bytes(launch.stdout)
        control_listener.settimeout(90)
        control, _ = control_listener.accept()
        control.settimeout(7500)
        config['awsCredentials'] = credentials()
        def record(value):
            schema = value['schema']
            with (args.output / 'events.jsonl').open('ab') as events:
                events.write(canonical_json_bytes(value) + b'\n')
            if schema == 'postfiat.yolo.prover_recipient.v1':
                (args.output / 'recipient.json').write_bytes(canonical_json_bytes(value))
            print(json.dumps({'event': schema, 'status': value.get('status', 'OK')}), flush=True)
        with control:
            result = exchange(control, config=config, circuits=args.circuits, pins=pins['circuits'],
                ciphertext=args.ciphertext.read_bytes(), record=record)
        (args.output / 'result.json').write_bytes(canonical_json_bytes(result))
        if result['status'] == 'VERIFIED':
            for field, name in [('proofBase64', 'proof.bin'), ('proofCalldataBase64', 'proof-calldata.bin'),
                                ('publicValuesBase64', 'public-values.bin')]:
                (args.output / name).write_bytes(base64.b64decode(result[field], validate=True))
        (args.output / 'timing.json').write_text(json.dumps({'elapsedSeconds': time.monotonic() - started,
            'status': result['status'], 'syntheticQualification': True}) + '\n')
        return 0 if result['status'] == 'VERIFIED' else 1
    finally:
        if enclave_id:
            with (args.output / 'terminate.log').open('x') as log:
                subprocess.run(['nitro-cli', 'terminate-enclave', '--enclave-id', enclave_id],
                    stdout=log, stderr=subprocess.STDOUT, check=True)


if __name__ == '__main__':
    raise SystemExit(main())
