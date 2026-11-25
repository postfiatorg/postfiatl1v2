"""Read-only timing of the payout steps for an already-paid burn."""
import sys, time, json
from pathlib import Path
from exit_payout import *
from exit_tag8 import setup_cluster
T=time.time()
def lap(name):
    global T; n=time.time(); print(f"{n-T:7.1f}s  {name}",flush=True); T=n
c=setup_cluster("stage"); lap("cluster")
reds=rpc(0,"vault_bridge_status",asset_id=ASSET)["redemptions"]; lap("vault_bridge_status")
red=reds[-1] if len(sys.argv)<2 else next(r for r in reds if r["burn_tx_id"].startswith(sys.argv[1]))
burn=red["burn_tx_id"]; owner=validate_burn(red,burn); h=red["created_at_height"]; print("burn",burn[:12],"height",h)
signature(c,burn,None); lap("signature lookup")
latest,prior=light(c); lap("light")
fetch=BlockReads(); FORCE=len(sys.argv)>3
if FORCE or latest>=h:
  prior=prior_before(c,h,fetch); lap("prior_before")
work=Path(sys.argv[2] if len(sys.argv)>2 else "work"); work.mkdir(exist_ok=True)
w=egress_witness(c,red,prior,h,work,fetch); lap("egress_witness")
(work/"witness.json").write_text(json.dumps(w,separators=(",",":"))); print("witness bytes",(work/"witness.json").stat().st_size)
