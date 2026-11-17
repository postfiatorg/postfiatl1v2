#!/bin/bash
# G2 phase A/B (read-only on PFTL; reads Ethereum mainnet beacon/execution):
# build the NEW Ethereum route profile (epoch EPOCH, new vkey) and the beacon anchor
# from a finalized checkpoint agreed by two independent beacon providers.
# Usage: ops/b23/g2_prepare.sh EPOCH ACTIVATION_HEIGHT OUTDIR
set -euo pipefail
EPOCH=$1; ACT=$2; O=$3; mkdir -p $O
T=/home/postfiat/repos/postfiatl1v2-b23-ingress/target/debug/eth-l1-mainnet-fast-lane-p0
VKEY=0x00d8cbf06d5640adde0fef863b3c689c4ab3442b0dc91d28ac4cc5f9c9b2ccbb
B1=https://ethereum-beacon-api.publicnode.com
B2=https://lodestar-mainnet.chainsafe.io
EL=https://ethereum-rpc.publicnode.com
VAULT=0xe7a432a28b20621a70e845c7f202004cf98e7a2c; USDC=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48

echo "== vault/token runtime code hashes at finalized execution block"
FIN=$(curl -s -m 20 -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"eth_getBlockByNumber","params":["finalized",false]}' $EL | python3 -c "import sys,json;print(json.load(sys.stdin)['result']['number'])")
VH=$(~/.foundry/bin/cast keccak $(~/.foundry/bin/cast code $VAULT --block $FIN --rpc-url $EL))
TH=$(~/.foundry/bin/cast keccak $(~/.foundry/bin/cast code $USDC --block $FIN --rpc-url $EL))
echo "finalized_el_block=$FIN vault=$VH token=$TH" | tee $O/code-hashes.txt
[ "$VH" = 0xc6dbb722c23bfc841624bb909fcb54d84a65a2ea6ece96e2a28bf61d5dea6d05 ] || { echo "VAULT CODE HASH CHANGED"; exit 1; }
[ "$TH" = 0xd80d4b7c890cb9d6a4893e6b52bc34b56b25335cb13716e0d1d31383e6b41505 ] || { echo "TOKEN CODE HASH CHANGED"; exit 1; }

echo "== route profile epoch $EPOCH activation $ACT"
$T route-profile --asset-id 02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b \
  --vault-address $VAULT --vault-runtime-code-hash $VH --token-runtime-code-hash $TH \
  --ingress-program-vkey $VKEY --verifier-policy-hash 44a402a3df72ee8da6f23737becef04fd0ce55e683706e929606a6aea1f05b25 \
  --route-epoch $EPOCH --activation-height $ACT --expires-at-height 1000000 --output $O/route-profile-tool.json > $O/route-profile.stdout
python3 - "$O" <<'EOF'
import json,sys
O=sys.argv[1]; d=json.load(open(f"{O}/route-profile-tool.json"))
prof=d.get("profile") or d.get("route_profile"); json.dump(prof,open(f"{O}/route-profile.json","w"),indent=1)
print("P", d.get("route_profile_hash"), "binding", d.get("route_binding"))
open(f"{O}/P.txt","w").write(d["route_profile_hash"]); open(f"{O}/binding.txt","w").write(str(d.get("route_binding")))
EOF

echo "== finalized checkpoint from two providers"
c1=$(curl -s -m 20 $B1/eth/v1/beacon/states/finalized/finality_checkpoints | python3 -c "import sys,json;d=json.load(sys.stdin)['data']['finalized'];print(d['epoch'],d['root'])")
c2=$(curl -s -m 20 $B2/eth/v1/beacon/states/finalized/finality_checkpoints | python3 -c "import sys,json;d=json.load(sys.stdin)['data']['finalized'];print(d['epoch'],d['root'])")
echo "publicnode=$c1"; echo "chainsafe=$c2"; echo "publicnode=$c1 chainsafe=$c2" > $O/checkpoint.txt
[ "$c1" = "$c2" ] || { echo "CHECKPOINT MISMATCH"; exit 1; }
ROOT=$(echo $c1 | awk '{print $2}')
echo "== inspect-bootstrap via both providers"
$T inspect-bootstrap --route-profile $O/route-profile.json --beacon-root $ROOT --beacon-rpc $B1 --output $O/ANCHOR.publicnode.json > $O/inspect-publicnode.out 2>&1
$T inspect-bootstrap --route-profile $O/route-profile.json --beacon-root $ROOT --beacon-rpc $B2 --output $O/ANCHOR.chainsafe.json > $O/inspect-chainsafe.out 2>&1
cmp $O/ANCHOR.publicnode.json $O/ANCHOR.chainsafe.json && echo "ANCHORS IDENTICAL" || { echo "ANCHOR MISMATCH"; exit 1; }
cp $O/ANCHOR.publicnode.json $O/ANCHOR.json
python3 -c "import json;a=json.load(open('$O/ANCHOR.json'));print(json.dumps({k:a[k] for k in a if k!='schema'}))"
echo "PREPARE OK"
