"""Pay one finalized burn using a witness produced by the validator CLI (no RPC reply limit)."""
import sys, time, json, subprocess
from pathlib import Path
from exit_payout import *
from exit_tag8 import setup_cluster
burn=sys.argv[1]; work=Path(sys.argv[2]); work.mkdir(parents=True,exist_ok=True)
c=setup_cluster("stage")
red=next(r for r in rpc(0,"vault_bridge_status",asset_id=ASSET)["redemptions"] if r["burn_tx_id"]==burn)
owner=validate_burn(red,burn); h=red["created_at_height"]
require(signature(c,burn,None) is None,"Already paid.")
latest,prior=light(c)
if latest>=h:
    start=max(1,h-65); headers=[]
    for first in range(start,h+1,8):
        headers.extend(b["header"] for b in rpc(0,"blocks",from_height=first,limit=min(8,h-first+1)))
    states=c.call("getMultipleAccounts",[str(checkpoint_pda(v["block_hash"])) for v in headers[:-1]],dict(c.level,encoding="base64"))["value"]
    prior=compatible_prior(headers,states)
wf=work/"witness.json"
if not wf.exists():
    out=subprocess.check_output(["ssh","-oBatchMode=yes","root@64.176.220.75","runuser -u postfiat -- /opt/postfiat/releases/sol-exit-r1/postfiat-node pfusdc-egress-witness --data-dir /var/lib/postfiat/validator-0 --withdrawal-id "+red["redemption_id"]+" --prior-checkpoint "+prior])
    wf.write_bytes(out)
w=json.loads(wf.read_text()); print("witness bytes",wf.stat().st_size,"compact",len(json.dumps(w)),flush=True)
require(w["receipt"]["tx_id"]==burn and w["receipt"]["accepted"] and w["route_profile"]["profile_hash"]==EXIT and w["withdrawal_packet"]["source_bucket_id"]==BUCKET,"Witness does not match this burn.")
target=rpc(0,"blocks",from_height=h,limit=1)[0]["header"]
verify_endpoints(c,w,prior,target); print("endpoints verified; owner",owner,"amount",red["amount_atoms"],flush=True)
def journal(sig,height):
    (work/"attempt.json").write_text(json.dumps({"signature":str(sig),"last_valid_block_height":height})); print("attempt",sig,height,flush=True)
t=time.time(); sent=submit(c,"withdrawal",w,work,owner,before_send=journal); print("submitted",sent,round(time.time()-t),"s",flush=True)
for _ in range(40):
    sig=signature(c,burn,sent)
    if sig: break
    time.sleep(5)
print("paid marker",sig,flush=True)
