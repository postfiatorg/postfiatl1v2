#!/bin/bash
# G2 step 4: one real 1 USDC deposit, driven end to end through the bridge service API.
# Usage: ops/b23/g2_deposit.sh OUTDIR
set -uo pipefail
O=$1; mkdir -p $O; cd /home/postfiat/repos/postfiatl1v2-b23-ops; export PYTHONDONTWRITEBYTECODE=1
BOB=pf5c266a1bdb18320e140961411a8c4d8383eaafb2
S=541e451e6ae96b6680988655571a0feec59c7f1d073c9b5d29547254cab38a3be540f8767446d25baebc7081ce97aef7
BIND=0x95f0dcbf204825b0789384769dddfa12e4be09aa1af8e841fe4a56bec423927d
bal() { python3 - "$1" <<'PY'
import sys,json; sys.path.insert(0,'/home/postfiat/repos/postfiatl1v2-navcoin-proof-b2.2/.scratch-deposit'); import pftl
S='541e451e6ae96b6680988655571a0feec59c7f1d073c9b5d29547254cab38a3be540f8767446d25baebc7081ce97aef7'
out={}
for p in pftl.PEERS:
    ls=pftl.rpc('account_lines',{'account':'pf5c266a1bdb18320e140961411a8c4d8383eaafb2','limit':60},'b',p).get('lines') or []
    si=pftl.rpc('server_info',{},'s',p)['ledger']
    out[p[0]]={'height':si['height'],'root':si['state_root'],'S':next((l['balance'] for l in ls if l['asset_id']==S),0),
               'lines':sorted((l['code'],l['asset_id'][:8],l['balance']) for l in ls)}
    print(p[0], out[p[0]]['height'], out[p[0]]['root'][:16], 'bob S balance', out[p[0]]['S'])
json.dump(out,open(sys.argv[1],'w'),indent=1)
PY
}
echo "== Bob balances before"; bal $O/bob-before.json
echo "== readiness"; python3 ops/b23/bridge_job.py readiness $O/readiness.json
python3 -c "import json,sys;d=json.load(open('$O/readiness.json'));sys.exit(0 if d.get('ready') is True else 1)" || { echo "NOT READY - stopping before any deposit"; exit 2; }
echo "== Ethereum deposit (1 USDC, test EOA)"
/home/postfiat/repos/StakeHub/.venv/bin/python -u ops/b23/eth_deposit.py --amount-atoms 1000000 --recipient $BOB --route-binding $BIND --readiness-json $O/readiness.json --credited-asset $S --out $O/eth || { echo "ETH DEPOSIT FAILED"; exit 3; }
echo "== bridge job"
python3 -u ops/b23/bridge_job.py submit $O/eth/deposit-record.json $O/job || { echo "JOB FAILED"; exit 4; }
echo "== Bob balances after"; bal $O/bob-after.json
echo "DEPOSIT FLOW END"
