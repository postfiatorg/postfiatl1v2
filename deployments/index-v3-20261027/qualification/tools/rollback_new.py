"""Rollback build (exchange-readiness-20261023) on the six copies the new build's services ran on: checkpoint x6 and
full replay of validator-0 (history.py verify), then rollback_core.sh. Local only."""
from runner import *
import threading
OLD = ROOT/'binaries/rollback'
expected = json.loads((ROOT/'new-build-service-receipt.json').read_text())['expected']
checks = []; lock_ = threading.Lock()
def verify(name, data, checkpoint):
    rec = run(name, [OLD, 'verify-finalized-checkpoint' if checkpoint else 'verify-state', '--data-dir', data], timeout=900)
    if rec['status'] == 'PASS':
        result = json.loads(pathlib.Path(rec['stdout']).read_text())
        block = result if checkpoint else result['block_log']
        observed = dict(block_height=block.get('checkpoint_height' if checkpoint else 'block_count'), block_tip_hash=block.get('checkpoint_block_hash' if checkpoint else 'tip_hash'), state_root=block.get('checkpoint_state_root' if checkpoint else 'state_root'))
        ok = result.get('verified') is True and block.get('verified') is True and observed == expected
        rec.update(status='PASS' if ok else 'FAIL', observed=observed, expected=expected)
    write(ROOT/'reports'/f'{name}.json', rec)
    with lock_: checks.append(rec)
ts = [threading.Thread(target=verify, args=(f'rollback-checkpoint-new-build-services-validator-{i}', ROOT/'working'/f'validator-{i}', True)) for i in range(6)]
ts.append(threading.Thread(target=verify, args=('rollback-full-new-build-services-validator-0', ROOT/'working/validator-0', False)))
for t in ts: t.start()
for t in ts: t.join()
core = run('rollback-core', ['bash', ROOT/'tools/rollback_core.sh'], cwd=ROOT/'tools', timeout=600, separate=False)
checks.append(core)
write(ROOT/'rollback-run.json', dict(rollback_binary_sha256=sha(OLD), expected=expected, checks=checks, live_mutations=False))
print(json.dumps({c['name']: c['status'] for c in checks}, indent=1))
