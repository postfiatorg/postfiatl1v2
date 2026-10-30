"""5b, local only: the new executable's rpc-serve WITHOUT --allow-private-action-admission on a scratch copy of the
newest signed import (1153, 2026-10-27). Loopback only; the copy is deleted afterwards."""
import json, pathlib, shutil, socket, subprocess, sys, time
ROOT = pathlib.Path('/home/postfiatchad/.cache/release-vote-records-20261030')
NEW = ROOT/'binaries/candidate-1'; SRC = ROOT/'canary-1153/signed-import'; X = ROOT/'rpc-1153'
sys.path.insert(0, str(ROOT/'source-1/python'))
from postfiat_rpc.client import PostFiatRpcClient
if X.exists(): shutil.rmtree(X)
shutil.copytree(SRC, X, symlinks=True)
with socket.socket() as s: s.bind(('127.0.0.1', 0)); port = s.getsockname()[1]
(X/'runtime').mkdir(exist_ok=True); (X/'readiness').mkdir(exist_ok=True)
cmd = [str(NEW), 'rpc-serve', '--unsafe-devnet-json-storage', '--data-dir', str(X), '--spool-dir', str(X/'runtime/rpc-spool'),
       '--ready-file', str(X/'readiness/rpc.ready.json'), '--bind-host', '127.0.0.1', '--port', str(port), '--max-requests', '100000',
       '--timeout-ms', '30000', '--child-timeout-ms', '30000', '--keep-alive']
log = open(ROOT/'private-off-rpc-serve.log', 'w')
p = subprocess.Popen(cmd, stdout=log, stderr=subprocess.STDOUT)
out = dict(scope='scratch copy of the signed 1153 import, new executable, rpc-serve without the admission switch', command=cmd[:1] + ['rpc-serve', '...'] + [c for c in cmd if c.startswith('--allow')], calls={})
try:
    end = time.time() + 60
    while not (X/'readiness/rpc.ready.json').exists() and time.time() < end: time.sleep(0.2)
    c = PostFiatRpcClient(f'127.0.0.1:{port}', timeout_seconds=120)
    def call(name, method, params):
        try: r = c._send(c._request(method, params))
        except Exception as e: r = dict(exception=str(e))
        out['calls'][name] = dict(method=method, params=params, response=r)
        return r
    call('status', 'status', {})
    call('private_action_submit', 'private_action_submit', {'action_file': '/nonexistent.json'})
    call('private_action_receipt', 'private_action_receipt', {'action_id': '00' * 32})
    call('private_readiness', 'private_readiness', {'pool_id': 'asset-orchard-v1'})
    call('private_pool_snapshot', 'private_pool_snapshot', {'pool_id': 'asset-orchard-v1'})
finally:
    p.terminate(); p.wait(timeout=20)
st = subprocess.run([sys.executable, str(ROOT/'prev/deployments/index-v3-20261027/read-import-state.py'), str(X)], capture_output=True, text=True)
out['activation_heights'] = json.loads(st.stdout) if st.returncode == 0 else st.stderr
gov = json.JSONDecoder().raw_decode((X/'governance.json').read_text())[0]
out['w1_activation_amendments'] = [a for a in gov['amendments'] if a['kind'] in ('asset_orchard_pricing_enforcement_activation_height', 'asset_orchard_egress_policy_activation_height')]
out['amendment_kinds'] = sorted({a['kind'] for a in gov['amendments']})
(ROOT/'private-off-report.json').write_text(json.dumps(out, indent=2) + '\n')
shutil.rmtree(X)
for k, v in out['calls'].items():
    r = v['response']; print(k, json.dumps(r.get('error') or r.get('result'))[:700])
print('activation', json.dumps(out['activation_heights'])); print('w1 amendments', out['w1_activation_amendments'])
