"""Advance the Solana light checkpoint to a chosen L1 height using the setup payer."""
import sys, time, traceback
from pathlib import Path
from exit_payout import *
from exit_segments import select_target
from exit_tag8 import setup_cluster
end=int(sys.argv[1]); work=Path(sys.argv[2])
c=setup_cluster("stage")
latest,prior=light(c); print("light before",latest,flush=True)
if latest>=end: print("already at/after target"); sys.exit(0)
block=select_target(latest,rpc,end=end); print("target",block["height"],flush=True)
members=[dict(row,active=True) for row in rpc(0,"validators")["validators"]]
w=public_checkpoint(rpc,latest,prior,block,members,read(CONFIG)); print("witness built",flush=True)
verify_endpoints(c,w,prior,block); print("endpoints verified",flush=True)
work.mkdir(parents=True,exist_ok=True)
t=time.time(); submit(c,"checkpoint",w,work); print("submitted in",round(time.time()-t),"s",flush=True)
latest2,prior2=light(c); print("light after",latest2, (latest2,prior2)==(block["height"],block["block_hash"]),flush=True)
