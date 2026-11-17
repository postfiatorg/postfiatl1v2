#!/usr/bin/env python3
"""Live PFUSDC burn-to-redeem by the controlled holder (Alice), canary-checked first.
Usage: live_burn.py <amount_atoms> <0xrecipient> <output_dir>"""
import sys,fcntl,json
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'b24-live'))
from sign import *
from snapshot import export
from live import run_operations,LOCK,acquire_tee_lock
FAMILY='02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b'
BUCKET='a1dbe54059cd948e25b6695a9ea32f95d8236b1616a6cff47c2ca20992c04b991bb03d3b6a805990e0dd5cd67b2af090'
def main():
    OWNER=dict(alice=ALICE,test=TEST,bob=BOB)[sys.argv[4]] if len(sys.argv)>4 else ALICE
    amount=int(sys.argv[1]);recipient=sys.argv[2].lower();out=Path(sys.argv[3]);out.mkdir(parents=True,exist_ok=False)
    assert recipient.startswith('0x') and len(recipient)==42 and 0<amount<=2_000_000
    with LOCK.open('a+') as lock:
        acquire_tee_lock(lock,timeout=120)
        try:
            state=export(out/'before.json');nav=item(state['ledger']['nav_assets'],'asset_id',FAMILY)
            bucket=item(state['ledger']['vault_bridge_bucket_states'],'bucket_id',BUCKET);assert bucket['outstanding_vault_bridge_atoms']>=amount
            op=dict(operation='vault_bridge_burn_to_redeem',owner=OWNER,issuer=nav['issuer'],asset_id=FAMILY,bucket_id=BUCKET,amount_atoms=amount,epoch=nav['finalized_epoch'],reserve_packet_hash=nav['finalized_reserve_packet_hash'],destination_ref='evm-erc20:1:'+recipient)
            final,receipts=run_operations([dict(label='burn-to-redeem',source=OWNER,kind='asset',operation=op)],out/'before.json',out/'live')
            save(out/'complete.json',dict(receipts=receipts,final=str(final)));print(json.dumps(receipts))
        finally:
            lock.seek(0)
            if lock.read().strip()=='TEE':lock.seek(0);lock.truncate();lock.flush()
if __name__=='__main__':main()
