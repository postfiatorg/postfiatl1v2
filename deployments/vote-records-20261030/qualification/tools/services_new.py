"""Six loopback services of the new build on the six 2026-09-28 copies (height 1021, post-rotation), both start
orders, startup and restart (services.py rollback mode with the candidate). The 1020 originals that the governed
rotation needs are deleted, so the rotation itself cannot be rerun. Local only."""
from runner import *
import argparse, runpy, socket
sys.path.insert(0, str(REPO/'python'))
from postfiat_rpc.client import PostFiatRpcClient
def terminate(signum, frame): raise RuntimeError('qualification resource/time guard stopped local services')
signal.signal(signal.SIGTERM, terminate)
gate = runpy.run_path(str(REPO/'deployments/signing-fix-qualification-20260909/run_local_service_gate.py'))
args = argparse.Namespace(binary=ROOT/'binaries/candidate-1', clone_root=ROOT/'working', key_root=ROOT/'isolated-signers', topology=ROOT/'topology.json', work_root=ROOT/'new-build-services')
args.work_root.mkdir(exist_ok=True)
topology = json.loads(args.topology.read_text())
assert len(topology['peers']) == 6 and all(p['host'] == '127.0.0.1' for p in topology['peers'])
for p in topology['peers']:
    for field in ('rpc_port', 'p2p_port'):
        with socket.socket() as s: s.bind(('127.0.0.1', p[field]))
expected = json.loads(pathlib.Path('/home/postfiatchad/.cache/release-repair-20260928/v2-service-receipt.json').read_text())['finality_statuses'][0]
services = gate['Services'](args, topology)
clients = [PostFiatRpcClient(f"127.0.0.1:{p['rpc_port']}", timeout_seconds=180) for p in topology['peers']]
receipt = dict(scope='disposable_loopback_clones', live_mutations=False, finality_exercised=False, binary_sha256=sha(args.binary),
               expected_identity_source='~/.cache/release-repair-20260928/v2-service-receipt.json finality_statuses[0] (2026-09-28 rotation at 1021)',
               expected=dict(block_height=expected['block_height'], block_tip_hash=expected['block_tip_hash'], state_root=expected['state_root']))
result = 1
try:
    gate['stage_local_keys'](args)
    for cycle in ('startup', 'restart'):
        services.start_all(); rows = gate['statuses'](clients)
        assert len(rows) == 6 and all(all(r[f] == expected[f] for f in ('block_height', 'block_tip_hash', 'state_root')) and r['mempool_pending'] == 0 for r in rows)
        receipt[cycle] = rows; receipt['start_orders'] = services.orders
        print(f'{cycle}: all six new-build services agree at 1021', flush=True)
        services.stop_all()
    receipt['result'] = 'PASS_STARTUP_AND_RESTART_ONLY'; result = 0
except Exception as error:
    receipt.update(result='FAIL', error=str(error)); print(f'FAIL: {error}', file=sys.stderr)
finally:
    services.stop_all(); write(ROOT/'new-build-service-receipt.json', receipt)
sys.exit(result)
