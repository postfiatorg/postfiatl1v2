#!/usr/bin/env python3
"""Pool launch, native leg: reserve + subscribe + export 5,000 B24 paid from Alice's S-series PFUSDC,
then capture the public archive and build the receipt witness. copy-only never writes live."""
import argparse,os
from phase6_export import *
from qualify import S
L=COORD/'evidence/launch'
AMOUNT=5000000000
LABEL='launch-5000-B24-20261116'

def check(start,d):
    s=load(start);ops=operations(s,AMOUNT,LABEL,S);due=ops[0]['operation']['max_settlement_value_atoms']
    before=balance(s['ledger'],ALICE,S);assert before>=due,'Alice S balance %d below due %d'%(before,due)
    c=LiveCopy(start,d/'copy')
    for row in ops:c.apply(row['label'],row['operation'],ALICE)
    after=c.ledger();r=item(after['pftl_uniswap_routes'],'route_id',ROUTE);old=item(s['ledger']['pftl_uniswap_routes'],'route_id',ROUTE)
    assert invariant(r) and r['outstanding_bridge_claims_atoms']==old['outstanding_bridge_claims_atoms']+AMOUNT
    assert r['authorized_valid_supply_atoms']==old['authorized_valid_supply_atoms']+AMOUNT and r['pftl_spendable_supply_atoms']==old['pftl_spendable_supply_atoms']
    paid=before-balance(after,ALICE,S);assert paid==due,(paid,due)
    save(d/'operations.json',ops);save(d/'copy-result.json',dict(passed=True,amount_atoms=AMOUNT,due_atoms=due,paid_S_atoms=paid,nav_per_unit=item(s['ledger']['nav_assets'],'asset_id',B24)['nav_per_unit'],height=s['tip']['height'],live_transactions=0))
    print(json.dumps(load(d/'copy-result.json')),flush=True);return ops

def main():
    ap=argparse.ArgumentParser();ap.add_argument('action',choices=['copy-only','execute']);a=ap.parse_args();os.umask(0o077)
    from phase6_reads import read_six
    live.read_six=read_six;archive.read_six=read_six
    if a.action=='copy-only':
        d=L/('export-copy-'+datetime.now(timezone.utc).strftime('%H%M%S'));d.mkdir(mode=0o700,parents=True);check_start=d/'before.json';export(check_start);check(check_start,d);return
    d=L/'export';d.mkdir(mode=0o700,exist_ok=False)
    with live.LOCK.open('a+') as lock:
        live.acquire_tee_lock(lock)
        try:
            start=d/'before.json';export(start);ops=check(start,d)
            last,receipts=live.run_operations(ops,start,d/'live')
            packet=ops[-1]['operation']['packet_hash'];save(d/'native-result.json',dict(passed=True,packet_hash=packet,amount_atoms=AMOUNT,receipts=receipts,final_snapshot=str(last)))
            archive.receipt(d/'archive',packet)
            save(d/'result.json',dict(passed=True,packet_hash=packet,amount_atoms=AMOUNT,receipts=receipts,final_snapshot=str(last),real_archive=True))
        finally:
            lock.seek(0)
            if lock.read().strip()=='TEE':lock.seek(0);lock.truncate();lock.flush()
if __name__=='__main__':main()
