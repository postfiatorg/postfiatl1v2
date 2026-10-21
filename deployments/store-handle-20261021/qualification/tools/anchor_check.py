"""Item 5: the real five-vote AnchorCheckpoint (70,944 B) and an ActivateCommittee-sized input through the RPC
request validation path, new build vs the deployed signer-rotation-20261007 build. Local only: rpc-serve runs
read-only (no --allow-mempool-submit*, no finality topology) on a disposable copy, bound to 127.0.0.1."""
from runner import *
import shutil, socket
OUT = ROOT/'anchor-check'; OUT.mkdir(exist_ok=True)
ANCHOR = pathlib.Path('/home/postfiatchad/repos/postfiatl1v2-rel20261007/deployments/signer-rotation-20261007/observed/signer-activation/final-checkpoint-epoch1.fastlane-primary.json')
EPOCH2_ROOT = '72836827f9f3ecd3042261a77abff72235f0238b6a81f673630a62199415021a1be0cbd5451878edca0e110434c1da37'
SDK = ROOT/'binaries/postfiat-rpc-sdk'; NEW = ROOT/'binaries/candidate-1'; OLD = ROOT/'binaries/rollback'
FIXTURE = pathlib.Path('/home/postfiatchad/.cache/release-repair-20260928/canary-backup/signed-import')  # disposable 1050 import
METHOD = 'mempool_submit_fastlane_primary_finality'
anchor = ANCHOR.read_text().rstrip('\n')
assert len(anchor.encode()) == 70944, len(anchor.encode())


def pattern(n, seed): return [((i * 197 + seed * 89) % 256) for i in range(n)]


# ActivateCommittee shaped like crates/rpc_sdk/src/protocol_request_tests.rs synthetic_activate_committee_json:
# six 1952-byte ML-DSA-65-sized keys, the real five-vote final checkpoint, five 3309-byte control votes.
# Keys and vote signatures are synthetic; committee_root carries the real epoch-2 root (not recomputed over them).
tx = json.loads(anchor); cert = tx['operation']['certificate']
current = cert['votes'][0]['checkpoint']['committee']
nxt = dict(current, committee_epoch=current['committee_epoch'] + 1, validator_count=6, committee_root=list(bytes.fromhex(EPOCH2_ROOT)))
activate = json.dumps({'operation': {'kind': 'control', 'certificate': {
    'action': {'kind': 'activate_committee',
               'committee': {'domain': nxt, 'validators': [{'validator_id': f'validator-{i}', 'public_key': pattern(1952, i)} for i in range(6)]},
               'final_checkpoint': cert},
    'votes': [{'committee': current, 'action_digest': [7] * 48, 'validator_id': f'validator-{i}', 'signature': pattern(3309, i + 10)} for i in range(5)]}}},
    separators=(',', ':'))
CAP = 256 * 1024
inputs = {'anchor-70944': anchor, f'activate-{len(activate)}': activate, f'over-cap-{CAP + 1}': 'x' * (CAP + 1)}
expect_accept = {'anchor-70944': True, f'activate-{len(activate)}': True, f'over-cap-{CAP + 1}': False}
results = dict(anchor_file=str(ANCHOR), anchor_sha256=sha(ANCHOR), anchor_bytes=70944, activate_bytes=len(activate),
               activate_shape='synthetic keys and vote signatures; real final checkpoint; committee_root = epoch-2 root', cap_bytes=CAP,
               validate_request=[], rpc_serve=[], live_mutations=False, host_contact=False)
PLAIN = 'mempool_submit_fastlane_primary'  # the kind validate-request maps; the size rule is per param key, the same for both methods
for name, body in inputs.items():
    for method, kind in ((PLAIN, PLAIN), (METHOD, None)):
        req = OUT/f'{name}.{method}.request.json'
        req.write_text(json.dumps({'version': 'postfiat-local-rpc-v1', 'id': name, 'method': method, 'params': {'fastlane_primary_json': body}}))
        cmd = [SDK, 'validate-request', '--input', req, '--expect-id', name] + (['--expect-kind', kind] if kind else [])
        p = subprocess.run(cmd, capture_output=True, text=True)
        (OUT/f'{name}.{method}.validate-request.stdout').write_text(p.stdout); (OUT/f'{name}.{method}.validate-request.stderr').write_text(p.stderr)
        ok = p.returncode == 0
        results['validate_request'].append(dict(input=name, method=method, expect_kind=kind, bytes=len(body), exit_code=p.returncode, accepted=ok,
                                                stdout=p.stdout.strip()[:300], stderr=p.stderr.strip()[:300],
                                                status=('PASS' if ok == expect_accept[name] else 'FAIL') if kind else 'REFERENCE'))
for name in inputs: (OUT/f'{name}.request.json').write_text((OUT/f'{name}.{METHOD}.request.json').read_text())


def serve(label, binary):
    data = OUT/f'fixture-{label}'
    if not data.exists(): subprocess.run(['cp', '-a', str(FIXTURE), str(data)], check=True)
    with socket.socket() as s: s.bind(('127.0.0.1', 0)); port = s.getsockname()[1]
    ready = OUT/f'rpc-{label}.ready.json'; ready.unlink(missing_ok=True); (OUT/f'rpc-{label}.events.ndjson').unlink(missing_ok=True)
    cmd = [binary, 'rpc-serve', '--unsafe-devnet-json-storage', '--data-dir', data, '--spool-dir', OUT/f'spool-{label}', '--ready-file', ready,
           '--bind-host', '127.0.0.1', '--port', str(port), '--max-requests', '100', '--timeout-ms', '30000', '--child-timeout-ms', '30000',
           '--event-log', OUT/f'rpc-{label}.events.ndjson']
    log = open(OUT/f'rpc-{label}.stderr', 'wb')
    proc = subprocess.Popen([str(c) for c in cmd], stdout=log, stderr=log, start_new_session=True)
    try:
        for _ in range(120):
            if ready.exists(): break
            time.sleep(0.5)
        for name, body in inputs.items():
            line = (OUT/f'{name}.request.json').read_text()
            with socket.create_connection(('127.0.0.1', port), timeout=60) as sock:
                f = sock.makefile('rwb'); f.write(line.encode() + b'\n'); f.flush()
                reply = json.loads(f.readline(8 * 1024 * 1024) or b'{}')
            err = reply.get('error') or {}
            # Passing the parameter check = anything but rpc_protocol_error; the read-only server then refuses the method.
            passed = err.get('code') != 'rpc_protocol_error' and reply.get('id') == name
            results['rpc_serve'].append(dict(binary=label, binary_sha256=sha(binary), input=name, bytes=len(body), ok=reply.get('ok'),
                                             error_code=err.get('code'), error_message=(err.get('message') or '')[:200], parameter_check_passed=passed,
                                             status=('PASS' if passed == expect_accept[name] else 'FAIL') if label == 'new' else 'REFERENCE'))
    finally:
        os.killpg(proc.pid, signal.SIGTERM)
        try: proc.wait(10)
        except subprocess.TimeoutExpired: os.killpg(proc.pid, signal.SIGKILL)


serve('new', NEW)
serve('old', OLD)
for label in ('new', 'old'): shutil.rmtree(OUT/f'fixture-{label}', ignore_errors=True); shutil.rmtree(OUT/f'spool-{label}', ignore_errors=True)
new_rows = [r for r in results['rpc_serve'] if r['binary'] == 'new']
results['result'] = 'PASS' if all(r['status'] in ('PASS', 'REFERENCE') for r in results['validate_request']) and all(r['status'] == 'PASS' for r in new_rows) else 'FAIL'
write(ROOT/'anchor-check.json', results)
print(json.dumps(dict(result=results['result'], validate=[(r['input'], r['accepted']) for r in results['validate_request']],
                      serve=[(r['binary'], r['input'], r['error_code']) for r in results['rpc_serve']])), flush=True)
