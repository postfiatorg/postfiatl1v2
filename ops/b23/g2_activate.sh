#!/bin/bash
# G2 activation (chain writes; chain.lock must say ops). Run AFTER g2_prepare.sh.
#  D. PFUSDC issuer nav_profile_register (route policy hash = P, new vkey) -> accepted
#  E. PFUSDC issuer nav_asset_register (family -> new profile Q)            -> accepted
#  F. governance: amendment on v0, 6 authorizations (each on its own host, slot = tip+1),
#     assemble, certified governance round on the deterministic proposer   -> accepted
#  G. read back vault_bridge_route (epoch, P, anchor) on all six
# All host commands run as the postfiat user with the LIVE release binary.
# Usage: ops/b23/g2_activate.sh PREP_DIR
set -uo pipefail
O=$1; E=$O; cd /home/postfiat/repos/postfiatl1v2-b23-ops; export PYTHONDONTWRITEBYTECODE=1
[ "$(cat /home/postfiat/repos/b23-coord/chain.lock)" = ops ] || { echo "chain.lock is not ops"; exit 1; }
REL=${REL:-b23-release3-20261112}
NODE=/opt/postfiat/releases/$REL/postfiat-node; TOPO=/etc/postfiat/releases/$REL/topology.json
ISSUER=pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8
IKEY=/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/pfusdc-issuer-key.json
VKEY=0x00d8cbf06d5640adde0fef863b3c689c4ab3442b0dc91d28ac4cc5f9c9b2ccbb
P=$(cat $O/P.txt)
HOSTS=(64.176.220.75 95.179.184.122 66.42.48.39 149.28.63.106 95.179.179.206 45.32.110.170)
pf() { ssh -o BatchMode=yes -o ConnectTimeout=10 root@$1 "runuser -u postfiat -- bash -c '$2'"; }
tip() { python3 -c "import sys;sys.path.insert(0,'/home/postfiat/repos/postfiatl1v2-live/.scratch-deposit');import pftl;print(pftl.rpc('server_info',{},'t')['ledger']['height'])"; }

echo "== D. nav_profile_register (route policy $P)"
python3 - "$O" "$P" "$VKEY" <<'PY'
import json,sys
O,P,V=sys.argv[1:4]
t=json.load(open('/home/postfiat/var/b23-g2/h1006-nav_profile_register.json'))
keep=["operation","registrant","verifier_kind","source_class","max_snapshot_age_blocks","challenge_window_blocks","max_epoch_gap_blocks","settle_deadline_blocks","min_challenge_bond","min_attestations","tolerance_bp","valuation_policy_hash","vault_bridge_route_policy_hash","sp1_program_vkey","sp1_proof_encoding","max_proof_bytes","max_public_values_bytes"]
op={k:t[k] for k in keep}; op["vault_bridge_route_policy_hash"]=P; op["sp1_program_vkey"]=V
json.dump(op,open(f"{O}/nav_profile_register.json","w"),indent=1)
sys.path.insert(0,'/home/postfiat/repos/postfiatl1v2-b23-ops/ops/b23'); import nav_profile_id
Q=nav_profile_id.profile_id(op); open(f"{O}/Q.txt","w").write(Q); print("Q",Q)
a=json.load(open('/home/postfiat/var/b23-g2/h1007-nav_asset_register.json'))
akeep=["operation","issuer","asset_id","reserve_operator","proof_profile","valuation_unit","redemption_account"]
aop={k:a[k] for k in akeep}; aop["proof_profile"]=Q
json.dump(aop,open(f"{O}/nav_asset_register.json","w"),indent=1)
PY
python3 -u ops/b23/submit_asset_op.py g2-nav-profile-register-$(date +%s) $ISSUER $IKEY $O/nav_profile_register.json || { echo "D FAILED (nothing switched yet)"; exit 2; }
echo "== E. nav_asset_register -> $(cat $O/Q.txt)"
python3 -u ops/b23/submit_asset_op.py g2-nav-asset-register-$(date +%s) $ISSUER $IKEY $O/nav_asset_register.json || { echo "E FAILED: PFUSDC still on old profile; route unaffected"; exit 3; }
echo "   (from here until F is accepted, the epoch-10 route fails its own profile check; ingress was already fail-closed)"

echo "== F. governance"
T0=$(tip); H=$((T0+1)); PROP=$((H % 6)); echo "tip=$T0 H=$H proposer=validator-$PROP"
R=/var/lib/postfiat/validator-0/b23-g2
pf ${HOSTS[0]} "mkdir -p $R"
scp -q $O/route-profile.json $O/ANCHOR.json root@${HOSTS[0]}:$R/ && ssh -o BatchMode=yes root@${HOSTS[0]} "chown postfiat:postfiat $R/*"
pf ${HOSTS[0]} "$NODE vault-bridge-route-profile-governance --data-dir /var/lib/postfiat/validator-0 --validators validator-0,validator-1,validator-2,validator-3,validator-4,validator-5 --profile-file $R/route-profile.json --tier4-finality-bootstrap-file $R/ANCHOR.json --amendment-file $R/amendment.json --batch-file $R/batch-unsigned.json > $R/governance.stdout" || { echo "F amendment creation FAILED"; exit 4; }
scp -q root@${HOSTS[0]}:$R/amendment.json $O/amendment.json
for i in 0 1 2 3 4 5; do
  h=${HOSTS[$i]}; RR=/var/lib/postfiat/validator-$i/b23-g2
  pf $h "mkdir -p $RR"; scp -q $O/amendment.json root@$h:$RR/amendment.json; ssh -o BatchMode=yes root@$h "chown postfiat:postfiat $RR/amendment.json"
  pf $h "$NODE governance-authorization-sign --data-dir /var/lib/postfiat/validator-$i --amendment-file $RR/amendment.json --validator validator-$i --validator-key-file /var/lib/postfiat/validator-$i/validator_keys.json --proposal-slot $H --expires-at-height $((H+30)) --authorization-file $RR/auth-$i.json > /dev/null" || { echo "F sign FAILED at validator-$i"; exit 5; }
  scp -q root@$h:$RR/auth-$i.json $O/auth-$i.json; echo "signed validator-$i"
done
for i in 0 1 2 3 4 5; do scp -q $O/auth-$i.json root@${HOSTS[0]}:$R/auth-$i.json; done; ssh -o BatchMode=yes root@${HOSTS[0]} "chown postfiat:postfiat $R/*"
pf ${HOSTS[0]} "$NODE governance-amendment-assemble --data-dir /var/lib/postfiat/validator-0 --amendment-file $R/amendment.json --authorization-files $R/auth-0.json,$R/auth-1.json,$R/auth-2.json,$R/auth-3.json,$R/auth-4.json,$R/auth-5.json --proposal-slot $H --output $R/amendment-signed.json > /dev/null" || { echo "F amendment assemble FAILED"; exit 6; }
pf ${HOSTS[0]} "$NODE vault-bridge-route-profile-governance-assemble --data-dir /var/lib/postfiat/validator-0 --profile-file $R/route-profile.json --tier4-finality-bootstrap-file $R/ANCHOR.json --signed-amendment-file $R/amendment-signed.json --proposal-slot $H --batch-file $R/batch-signed.json > /dev/null" || { echo "F batch assemble FAILED"; exit 7; }
scp -q root@${HOSTS[0]}:$R/batch-signed.json root@${HOSTS[0]}:$R/amendment-signed.json $O/ 2>/dev/null || scp -q root@${HOSTS[0]}:$R/batch-signed.json $O/
scp -q root@${HOSTS[0]}:$R/amendment-signed.json $O/
[ "$(tip)" = "$T0" ] || { echo "TIP MOVED during signing; slot $H invalid, re-run F"; exit 8; }
PH=${HOSTS[$PROP]}; PR=/var/lib/postfiat/validator-$PROP/b23-g2
pf $PH "mkdir -p $PR"; scp -q $O/batch-signed.json root@$PH:$PR/batch-signed.json; ssh -o BatchMode=yes root@$PH "chown postfiat:postfiat $PR/batch-signed.json"
pf $PH "$NODE transport-peer-certified-batch-round --data-dir /var/lib/postfiat/validator-$PROP --topology $TOPO --batch-kind governance --batch-file $PR/batch-signed.json --key-file /var/lib/postfiat/validator-$PROP/validator_keys.json --proposal-key-file /var/lib/postfiat/validator-$PROP/validator_keys.json --require-local-proposer --quorum-early-full-propagation --local-apply-before-certified-send --artifact-dir $PR/round-$H --height $H --view 0 --timeout-ms 60000 --send-retries 16 --retry-backoff-ms 250" > $O/round-$H.stdout 2> $O/round-$H.stderr; echo "round rc=$?"; tail -c 600 $O/round-$H.stderr
echo "== G. receipt + route read-back"
python3 - "$O" "$H" <<'PY'
import json,sys
sys.path.insert(0,'/home/postfiat/repos/postfiatl1v2-live/.scratch-deposit'); import pftl
O,H=sys.argv[1],int(sys.argv[2])
amend=json.load(open(f"{O}/amendment-signed.json")); aid=amend.get("amendment_id") or amend.get("amendment",{}).get("amendment_id")
out={}; keys=set()
for p in pftl.PEERS:
    b=pftl.rpc("blocks",{"from_height":H,"limit":1},"g",p)
    hdr=b[0]["header"] if b else {}
    rc=pftl.rpc("tx",{"tx_id":aid},"t",p).get("receipt",{}) if b else {}
    r=pftl.rpc("vault_bridge_route",{"asset_id":"02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b"},"r",p)
    row={"height":hdr.get("height"),"batch_kind":hdr.get("batch_kind"),"root":hdr.get("state_root"),"receipt":rc,
         "route_epoch":r["profile"]["route_epoch"],"profile_hash":r["profile_hash"],"route_binding":r.get("route_binding"),
         "anchor":r.get("beacon_finality_anchor")}
    out[p[0]]=row; keys.add(json.dumps(row,sort_keys=True))
    print(p[0],row["height"],row["batch_kind"],str(row["root"])[:16],"receipt",rc.get("accepted"),rc.get("code"),str(rc.get("message"))[:90],"| route epoch",row["route_epoch"],row["profile_hash"][:16],"anchor",bool(row["anchor"]))
json.dump(out,open(f"{O}/activation-readback.json","w"),indent=1)
print("AGREE" if len(keys)==1 else "DISAGREE")
v=out["validator-0"]
ok = v["receipt"].get("accepted") is True and v["route_epoch"]==json.load(open(f"{O}/route-profile.json"))["route_epoch"] and v["profile_hash"]==open(f"{O}/P.txt").read().strip() and v["anchor"]
print("ACTIVATION", "ACCEPTED" if ok and len(keys)==1 else "NOT ACCEPTED")
PY
