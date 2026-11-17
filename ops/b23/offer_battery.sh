#!/bin/bash
# Release-2 battery step 4: Alice posts 1 B23 for 1 PFUSDC and cancels it via
# mempool_submit_signed_offer_transaction_finality. Six-node checks before/after each.
set -uo pipefail
cd /home/postfiat/repos/postfiatl1v2-b23-ops
E=${OFFER_EVID:-/home/postfiat/repos/b23-coord/evidence/ops/release2/offer-battery}; mkdir -p $E
I=/home/postfiat/repos/b23-coord/evidence/ops/ops-inputs
A=pfab9b9228942e5c529633a13aa271d5297bec6353
K=/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/holder-key.json
export PYTHONDONTWRITEBYTECODE=1
T=$(date +%s)
echo "=== state before"; python3 -u ops/b23/offer_state.py $E/state-0-before.json || exit 2
echo "=== offer_create"
python3 -u ops/b23/offer_op.py r2-offer-create-$T $A $K $I/r2-offer-create.json || { echo "CREATE FAILED"; exit 3; }
H=$(python3 -c "import json;print(json.load(open('/home/postfiat/repos/b23-coord/evidence/ops/submissions/r2-offer-create-$T/finality.json'))['height'])")
python3 -u ops/b23/check_block.py $E/create-check.json $H $A || exit 4
echo "=== state with open offer"; python3 -u ops/b23/offer_state.py $E/state-1-open.json || exit 5
OID=$(python3 -c "import json;d=json.load(open('$E/state-1-open.json'));o=[x for x in d['validator-0']['alice_offers'] if x.get('state')=='open'];print(o[0]['offer_id'] if len(o)==1 else '')")
[ -n "$OID" ] || { echo "EXPECTED EXACTLY ONE OPEN OFFER"; exit 6; }
echo "offer_id=$OID"
python3 -c "import json;json.dump({'operation':'offer_cancel','owner':'$A','offer_id':'$OID'},open('$I/r2-offer-cancel-$T.json','w'))"
echo "=== offer_cancel"
python3 -u ops/b23/offer_op.py r2-offer-cancel-$T $A $K $I/r2-offer-cancel-$T.json || { echo "CANCEL FAILED - OPEN OFFER $OID REMAINS"; exit 7; }
H=$(python3 -c "import json;print(json.load(open('/home/postfiat/repos/b23-coord/evidence/ops/submissions/r2-offer-cancel-$T/finality.json'))['height'])")
python3 -u ops/b23/check_block.py $E/cancel-check.json $H $A || exit 8
echo "=== state after"; python3 -u ops/b23/offer_state.py $E/state-2-after.json || exit 9
echo "OFFER BATTERY DONE"
