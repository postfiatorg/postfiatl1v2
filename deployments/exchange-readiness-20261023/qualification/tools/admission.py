"""Admission check of the new build on a local fixture (postfiat-local-admission, one validator). Local only:
a loopback rpc-serve on a fresh init data dir; no devnet key, no live endpoint."""
import json, os, pathlib, socket, subprocess, sys, time
sys.path.insert(0, '/home/postfiatchad/repos/postfiatl1v2/python')
from postfiat_rpc.client import PostFiatRpcClient
F = pathlib.Path(__file__).parent; D = F/'data2'
B = '/home/postfiatchad/.cache/release-exchange-readiness-20261023/binaries/candidate-1'
status = json.loads(subprocess.check_output([B, 'status', '--data-dir', D]))
faucet = json.loads((D/'faucet_key.json').read_text())['address']
seq = json.loads(subprocess.check_output([B, 'account', '--data-dir', D, '--address', faucet]))['sequence'] + 1
def sign(to):
    out = subprocess.run([B, 'wallet-sign-transfer', '--key-file', D/'faucet_key.json', '--chain-id', status['chain_id'], '--genesis-hash', status['genesis_hash'],
                          '--protocol-version', '1', '--to', to, '--amount', '50000000', '--fee', '100000', '--sequence', str(seq)], capture_output=True, text=True)
    return out
VALID = 'pf6395ef7f43ffaa74ce178d320b9bbbfd45532b38'  # the devnet `testing` wallet (review table: pass)
signed_valid = sign(VALID)
assert signed_valid.returncode == 0, signed_valid.stderr
valid_json = json.loads(signed_valid.stdout)
valid_json = valid_json.get('signed_transfer', valid_json)
cases = {'canonical (testing wallet)': VALID,
         'one hex character short (41)': VALID[:-1],
         'uppercase prefix': 'PF' + VALID[2:],
         'valid mixed-case checksum': None,
         'named account': 'pf-exchange'}
port = 47391
(F/'ready').unlink(missing_ok=True)
srv = subprocess.Popen([B, 'rpc-serve', '--unsafe-devnet-json-storage', '--data-dir', D, '--port', str(port), '--bind-host', '127.0.0.1',
                        '--allow-mempool-submit', '--ready-file', F/'ready', '--max-requests', '100'], stdout=open(F/'rpc-serve.stdout', 'w'), stderr=open(F/'rpc-serve.stderr', 'w'))
results = []
try:
    for _ in range(100):
        if (F/'ready').exists(): break
        time.sleep(0.1)
    client = PostFiatRpcClient(f'127.0.0.1:{port}', timeout_seconds=60)
    import postfiat_rpc.address as addr
    cases['valid mixed-case checksum'] = addr.checksum_address(VALID)
    for label, to in cases.items():
        tx = json.loads(json.dumps(valid_json)); tx['unsigned']['to'] = to  # only the canonical case keeps a valid signature
        req = client._request('mempool_submit_signed_transfer', {'signed_transfer_json': json.dumps(tx, separators=(',', ':'))})
        resp = client._send(req)
        err = resp.get('error')
        results.append(dict(case=label, to=to, ok=resp.get('ok', err is None), error_code=(err or {}).get('code'), error_message=((err or {}).get('message') or '')[:160],
                            result_code=(resp.get('result') or {}).get('code') if isinstance(resp.get('result'), dict) else None))
    mempool = json.loads(subprocess.check_output([B, 'verify-mempool', '--data-dir', D])) if False else None
finally:
    srv.terminate(); srv.wait(10)
pending, _ = json.JSONDecoder().raw_decode((D/'mempool.json').read_text())
out = dict(binary=B, build_git_revision=status.get('build_git_revision'), chain_id=status['chain_id'], results=results,
           mempool_pending_transfers=[t['transfer']['unsigned']['to'] for t in pending.get('pending', [])])
(F/'admission-result.json').write_text(json.dumps(out, indent=2) + '\n')
print(json.dumps(out, indent=1))
