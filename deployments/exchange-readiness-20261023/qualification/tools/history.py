"""History, rotation and rollback checks with the new build (store-handle-20261021 tools/history.py). Local only."""
from runner import *
while not (ROOT/'node-builds.json').exists(): time.sleep(5)
assert json.loads((ROOT/'node-builds.json').read_text())['result'] == 'PASS', 'builds did not reproduce'
import shutil, threading
CAND = ROOT/'binaries/candidate-1'; OLD = ROOT/'binaries/rollback'
PUB = pathlib.Path('/home/postfiatchad/.postfiat/deployments/cobalt-activation-8694b99d/snapshot-keys/snapshot-publisher.public.json')
CANARIES = {  # newest signed archive first: 2026-10-21 (1131), 2026-10-15, 2026-10-09, 2026-09-28
    1131: pathlib.Path('/home/postfiatchad/.postfiat/deployments/store-handle-20261021/evidence'),
    1123: pathlib.Path('/home/postfiatchad/.postfiat/deployments/fastlane-cap-20261014/evidence'),
    1115: pathlib.Path('/home/postfiatchad/.postfiat/deployments/signer-rotation-20261007/evidence'),
    1050: pathlib.Path('/home/postfiatchad/.postfiat/deployments/combined-fastpay-20260928/evidence'),
}
ROTATED = json.loads(pathlib.Path('/home/postfiatchad/.cache/release-repair-20260928/v2-service-receipt.json').read_text())['finality_statuses'][0]
checks = []; mutex = threading.Lock(); slots = threading.BoundedSemaphore(6)


def save():
    write(ROOT/'history-run.json', dict(source_commit=SOURCE, binary_sha256=sha(CAND), rollback_binary_sha256=sha(OLD), checks=checks, live_mutations=False, updated_at=now()))


def add(rec):
    with mutex: checks.append(rec); save()
    return rec


def verify(name, binary, data, expected, checkpoint=False):
    with slots:
        rec = run(name, [binary, 'verify-finalized-checkpoint' if checkpoint else 'verify-state', '--data-dir', data], timeout=1200)
    if rec['status'] == 'PASS':
        try:
            result = json.loads(pathlib.Path(rec['stdout']).read_text())
            block = result if checkpoint else result['block_log']
            observed = dict(block_height=block.get('checkpoint_height' if checkpoint else 'block_count'), block_tip_hash=block.get('checkpoint_block_hash' if checkpoint else 'tip_hash'), state_root=block.get('checkpoint_state_root' if checkpoint else 'state_root'))
            passed = result.get('verified') is True and block.get('verified') is True and all(observed[k] == expected[k] for k in observed)
            rec.update(status='PASS' if passed else 'FAIL', verified=result.get('verified'), observed=observed, expected={k: expected[k] for k in observed})
            if not passed: rec['error'] = 'verification or certified identity mismatch'
        except Exception as e: rec.update(status='FAIL', error=str(e))
    write(ROOT/'reports'/f'{name}.json', rec)
    return add(rec)


def each(jobs):
    ts = [threading.Thread(target=verify, args=j) for j in jobs]
    for t in ts: t.start()
    for t in ts: t.join()


def canary(height, tamper):
    E = CANARIES[height]; C = ROOT/f'canary-{height}'; C.mkdir(exist_ok=True)
    backup = json.loads((E/'rollout-state.json').read_text())['backup']
    assert backup.get('verified') is True and backup['height'] == height
    exp = dict(block_height=backup['height'], block_tip_hash=backup['tip'], state_root=backup['state_root'])
    signed = E/'pre-rollout-backup/backup-signed'
    assert sha(signed/'snapshot.signed-manifest.json') == backup['signed_manifest_sha256']
    imp = add(run(f'canary-{height}-import-signed', [CAND, 'snapshot-import-signed-finalized-checkpoint', '--data-dir', C/'signed-import', '--snapshot-dir', signed, '--trusted-publisher-key-file', PUB, '--node-id', backup.get('source_validator', 'validator-1')], timeout=600))
    if tamper:
        tampered = C/'tampered-signed'; shutil.copytree(signed, tampered)
        m = json.loads((tampered/'snapshot.signed-manifest.json').read_text())
        def flip(o):
            for k, v in o.items():
                if 'signature' in k and isinstance(v, str) and len(v) > 8:
                    o[k] = v[:-1] + ('0' if v[-1] != '0' else '1'); return True
                if isinstance(v, dict) and flip(v): return True
            return False
        assert flip(m)
        (tampered/'snapshot.signed-manifest.json').write_text(json.dumps(m, indent=2))
        bad = run(f'canary-{height}-import-tampered-signature', [CAND, 'snapshot-import-signed-finalized-checkpoint', '--data-dir', C/'tampered-import', '--snapshot-dir', tampered, '--trusted-publisher-key-file', PUB, '--node-id', 'validator-1'], timeout=300)
        bad.update(expected='nonzero exit (rejection)', status='PASS' if bad.get('exit_code') not in (0, None) else 'FAIL'); add(bad)
        shutil.rmtree(tampered); shutil.rmtree(C/'tampered-import', ignore_errors=True)
    if imp['status'] != 'PASS': return
    jobs = [(f'canary-{height}-checkpoint-new', CAND, C/'signed-import', exp, True), (f'canary-{height}-full-new', CAND, C/'signed-import', exp)]
    if tamper:  # rollback: the deployed executable reads the data the new build imported
        jobs += [(f'canary-{height}-checkpoint-rollback', OLD, C/'signed-import', exp, True), (f'canary-{height}-full-rollback', OLD, C/'signed-import', exp)]
    each(jobs)


def lane_a():
    W = ROOT/'working'
    if not W.exists():
        add(run('copy-2026-09-28-working', ['cp', '-a', '/home/postfiatchad/.cache/release-repair-20260928/working', str(W)], cwd=ROOT, timeout=900))
    each([(f'rotated-1021-full-new-validator-{i}', CAND, W/f'validator-{i}', ROTATED) for i in range(6)])
    add(run('new-build-services', ['python3', ROOT/'tools/services_new.py'], cwd=ROOT/'tools', timeout=1200))
    add(run('rollback-new-build-services', ['python3', ROOT/'tools/rollback_new.py'], cwd=ROOT/'tools', timeout=1500))


def lane_b():
    canary(1131, True)
    canary(1123, False)
    canary(1115, False)
    canary(1050, False)


threads = [threading.Thread(target=f) for f in (lane_a, lane_b)]
for t in threads: t.start()
for t in threads: t.join()
write(ROOT/'operational-complete.json', dict(completed_at=now(), source_commit=SOURCE))
print(f'{now()} operational sequence complete', flush=True)
