"""Parent side of the opt-in real Nitro test fixture; only synthetic inputs.

Commands/results live in a private local directory. IMDSv2 temporary role
credentials are injected in memory; neither documents nor commands contain
any long-term AWS credential or enclave private key.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import socket
import struct
import subprocess
import threading
import time
import urllib.request


BOUND = 64 * 1024


def read(stream):
    def exact(n):
        result = bytearray()
        while len(result) < n:
            chunk = stream.recv(n - len(result))
            if not chunk: raise EOFError('enclave closed control connection')
            result.extend(chunk)
        return bytes(result)
    length = struct.unpack('>I', exact(4))[0]
    if not 0 < length <= BOUND: raise ValueError('invalid frame')
    return json.loads(exact(length))


def write(stream, value):
    data = json.dumps(value, sort_keys=True, separators=(',', ':')).encode()
    if not 0 < len(data) <= BOUND: raise ValueError('invalid frame')
    stream.sendall(struct.pack('>I', len(data)) + data)


def role_credentials():
    base = 'http://169.254.169.254/latest/'
    token = urllib.request.urlopen(urllib.request.Request(base + 'api/token', method='PUT',
        headers={'X-aws-ec2-metadata-token-ttl-seconds': '60'}), timeout=5).read().decode()
    req = urllib.request.Request(base + 'meta-data/iam/security-credentials/yolo-nitro-parent-v1',
        headers={'X-aws-ec2-metadata-token': token})
    result = json.loads(urllib.request.urlopen(req, timeout=5).read())
    return {key: result[key] for key in ('AccessKeyId', 'SecretAccessKey', 'Token')}


def proxy(tamper=False):
    listener = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
    listener.bind((socket.VMADDR_CID_ANY, 9002))
    listener.listen(8)
    def relay(left):
        # A deliberately redirected relay must fail the enclave's fixed KMS TLS.
        host = 's3.amazonaws.com' if tamper else 'kms.us-east-1.amazonaws.com'
        right = socket.create_connection((host, 443), timeout=20)
        def copy(source, target):
            try:
                while data := source.recv(65536): target.sendall(data)
            except OSError: pass
            finally:
                try: target.shutdown(socket.SHUT_WR)
                except OSError: pass
        reverse = threading.Thread(target=copy, args=(right, left), daemon=True)
        reverse.start()
        copy(left, right)
        reverse.join()
        left.close(); right.close()
    while True:
        stream, _ = listener.accept()
        threading.Thread(target=relay, args=(stream,), daemon=True).start()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--eif', required=True)
    parser.add_argument('--directory', required=True, type=Path)
    parser.add_argument('--debug', action='store_true')
    parser.add_argument('--collector-boot', type=Path)
    parser.add_argument('--kms-recipient-check', action='store_true')
    parser.add_argument('--tamper-kms-upstream', action='store_true')
    args = parser.parse_args()
    args.directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    threading.Thread(target=proxy, args=(args.tamper_kms_upstream,), daemon=True).start()
    listener = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
    listener.bind((socket.VMADDR_CID_ANY, 9010))
    listener.listen(1); listener.settimeout(60)
    command = ['nitro-cli', 'run-enclave', '--eif-path', args.eif, '--cpu-count', '2', '--memory', '2048', '--enclave-cid', '16']
    if args.debug: command.append('--debug-mode')
    launch = subprocess.run(command, check=True, stdout=subprocess.PIPE, text=True)
    (args.directory / 'launch.json').write_text(launch.stdout)
    if args.debug:
        with (args.directory / 'debug-console.log').open('w') as console:
            subprocess.Popen(['nitro-cli', 'console', '--enclave-id', json.loads(launch.stdout)['EnclaveID']],
                stdout=console, stderr=subprocess.STDOUT)
        time.sleep(0.5)
    control, _ = listener.accept(); control.settimeout(600)
    if args.collector_boot:
        config = json.loads(args.collector_boot.read_text())
        credentials = role_credentials()
        config['awsCredentials'] = {'accessKeyId': credentials['AccessKeyId'],
            'secretAccessKey': credentials['SecretAccessKey'], 'sessionToken': credentials['Token']}
        write(control, config)
        event = read(control)
        assert event['schema'] == 'postfiat.yolo.enclave_source_request.v1'
        assert event['kind'] == 'prior-state'
        (args.directory / 'collector-source-request.json').write_text(json.dumps(event, indent=2) + '\n')
        (args.directory / 'attestation.cbor').write_bytes(base64.b64decode(event['recipientAttestationDocumentBase64']))
        if args.kms_recipient_check:
            request = {'KeyId': config['kmsKeyArn'], 'CiphertextBlob': config['encryptedSchwabCredentialsBase64'],
                'EncryptionContext': config['encryptionContext'], 'Recipient': {
                    'KeyEncryptionAlgorithm': 'RSAES_OAEP_SHA_256',
                    'AttestationDocument': event['recipientAttestationDocumentBase64']}}
            path = args.directory / 'kms-recipient-request.json'
            path.write_text(json.dumps(request)); path.chmod(0o600)
            process = subprocess.run(['aws', 'kms', 'decrypt', '--region', 'us-east-1',
                '--cli-input-json', 'file://' + str(path)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            if process.returncode:
                result = {'status': 'REJECTED', 'awsErrorCode': process.stderr.partition('(')[2].partition(')')[0]}
            else:
                response = json.loads(process.stdout)
                assert not response.get('Plaintext')
                cipher = base64.b64decode(response['CiphertextForRecipient'])
                result = {'status': 'OK', 'plaintextReturnedToParent': False,
                    'recipientCiphertextBytes': len(cipher), 'recipientCiphertextSha256': hashlib.sha256(cipher).hexdigest(),
                    'scope': 'KMS authorizes actual collector PCR; parent receives only recipient ciphertext'}
            (args.directory / 'kms-recipient-result.json').write_text(json.dumps(result, indent=2) + '\n')
            print(json.dumps(result), flush=True)
        print(json.dumps({'collectorSourceRequest': True, 'collectionAttempted': False}), flush=True)
        control.close()  # Deliberate stop before any private source or OAuth input.
        return
    binding = hashlib.sha256(('synthetic-hardware:' + str(time.time_ns())).encode()).hexdigest()
    write(control, {'bindingDigest': binding})
    attestation = read(control)
    (args.directory / 'attestation.json').write_text(json.dumps(attestation, indent=2) + '\n')
    (args.directory / 'attestation.cbor').write_bytes(base64.b64decode(attestation['documentBase64']))
    print(json.dumps({'attestationReceived': True, 'pcrs': attestation['pcrs']}), flush=True)
    for i in range(32):
        path = args.directory / f'command-{i:02d}.json'
        start = time.monotonic()
        while not path.exists():
            if time.monotonic() - start > 570: raise TimeoutError('no qualification command')
            time.sleep(0.25)
        command = json.loads(path.read_text())
        command['awsCredentials'] = role_credentials()
        write(control, command)
        result = read(control)
        (args.directory / f'result-{i:02d}.json').write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps({'index': i, **result}), flush=True)
        if command['action'] == 'stop': break
    control.close()


if __name__ == '__main__':
    main()
