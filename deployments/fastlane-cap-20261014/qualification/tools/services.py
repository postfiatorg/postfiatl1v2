from runner import *
import argparse, runpy, socket
sys.path.insert(0,str(REPO/'python'))
from postfiat_rpc.client import PostFiatRpcClient
def terminate(signum, frame): raise RuntimeError('qualification resource/time guard stopped local services')
signal.signal(signal.SIGTERM,terminate)
mode=sys.argv[1]
if mode=='governance':
    sys.argv=[str(REPO/'deployments/release-repair-20260916/run_local_governance_gate.py'),
      '--binary',str(ROOT/'binaries/candidate-1'),'--source-commit',SOURCE,
      '--clone-root',str(ROOT/'working'),'--key-root',str(ROOT/'isolated-signers'),
      '--topology',str(ROOT/'topology.json'),'--batch-file',str(ROOT/'v2-governance/signed-batch.json'),
      '--work-root',str(ROOT/'v2-services'),'--receipt',str(ROOT/'v2-service-receipt.json')]
    runpy.run_path(sys.argv[0],run_name='__main__')
elif mode=='rollback':
    gate=runpy.run_path(str(REPO/'deployments/signing-fix-qualification-20260909/run_local_service_gate.py'))
    args=argparse.Namespace(binary=ROOT/'binaries/rollback',clone_root=ROOT/'rollback',key_root=ROOT/'isolated-signers',topology=ROOT/'topology.json',work_root=ROOT/'old-binary-services')
    topology=json.loads(args.topology.read_text())
    assert len(topology['peers'])==6 and all(p['host']=='127.0.0.1' for p in topology['peers'])
    for p in topology['peers']:
        for field in ('rpc_port','p2p_port'):
            with socket.socket() as s:s.bind(('127.0.0.1',p[field]))
    expected=json.loads((REPO/'deployments/release-repair-20260916/old-binary-service-receipt.json').read_text())['startup'][0]
    services=gate['Services'](args,topology)
    clients=[PostFiatRpcClient(f"127.0.0.1:{p['rpc_port']}",timeout_seconds=180) for p in topology['peers']]
    receipt=dict(scope='disposable_loopback_clones',live_mutations=False,finality_exercised=False,binary_sha256=sha(args.binary))
    result=1
    try:
        gate['stage_local_keys'](args)
        for cycle in ('startup','restart'):
            services.start_all(); rows=gate['statuses'](clients)
            assert len(rows)==6 and all(all(r[f]==expected[f] for f in ('block_height','block_tip_hash','state_root')) and r['mempool_pending']==0 for r in rows)
            receipt[cycle]=rows;receipt['start_orders']=services.orders
            print(f'{cycle}: all six services agree at captured height 1020',flush=True)
            services.stop_all()
        receipt['result']='PASS_STARTUP_AND_RESTART_ONLY';result=0
    except Exception as error:
        receipt.update(result='FAIL',error=str(error))
        print(f'FAIL: {error}',file=sys.stderr)
    finally:
        services.stop_all();write(ROOT/'old-binary-service-receipt.json',receipt)
    sys.exit(result)
