#!/usr/bin/env python3
"""Rehearse the approved controlled-holder retirement, retaining founder reserve."""
import argparse
from sign import *
from retirement import prepare
OUT=COORD/'evidence/tee/phase5'

def plan(snapshot):
    p=prepare(snapshot)
    p['operations']=[x for x in p['operations'] if x['source']!=FOUNDER]
    founder=next(x for x in p['holder_terms'] if x['owner']==FOUNDER)
    p.update(founder_signatures=0,founder_B23_retained_atoms=founder['old_B23_atoms'],
        founder_reserve_retained_atoms=founder['gross_PFUSDC_atoms'],
        founder_payout_currently_claimable_after_halt=False,
        founder_reason='Release4 cash-in rejects halted NAV. Keep his gross payout reserved; never spend it for Alice or B24. One B24 is separately allocated from opening inventory until he adds a trustline.',
        old_spendable_supply_after_atoms=founder['old_B23_atoms'],
        old_historical_nav_and_route_counter_after_atoms=p['old_historical_nav_and_route_counter_after_atoms']+founder['old_B23_atoms'])
    return p

def main():
    a=argparse.ArgumentParser();a.add_argument('--copy-only',action='store_true',required=True);a.add_argument('--snapshot',type=Path,required=True);a.add_argument('--output',type=Path,required=True);args=a.parse_args()
    p=plan(args.snapshot);c=Copy(args.snapshot,args.output,signer=copy_signer)
    for row in p['operations']:c.apply(row['label'],row['operation'],row['source'],kind=row['kind'])
    s=c.ledger();r=item(s['pftl_uniswap_routes'],'route_id',OLD_ROUTE);nav=item(s['nav_assets'],'asset_id',B23)
    assert r['paused'] and not r.get('live_value_enabled',False) and nav['halted']
    assert r['settlement_reserve_atoms']==p['founder_reserve_retained_atoms'] and balance(s,FOUNDER,B23)==1_000_000
    for owner in (ALICE,BOB,TEST):assert balance(s,owner,B23)==0
    assert r['authorized_valid_supply_atoms']==p['old_historical_nav_and_route_counter_after_atoms']
    op=dict(next(x['operation'] for x in p['operations'] if x['label']=='cash-out-'+ALICE),redemption_nonce=hashlib.sha256(b'phase5-halt-negative').hexdigest())
    c.apply('halt-prevents-old-cash-in',op,ALICE,False,'pftl_uniswap_nav_asset_halted')
    save(args.output/'plan.json',p);save(args.output/'result.json',dict(passed=True,founder_B23_retained_atoms=1_000_000,founder_reserve_retained_atoms=r['settlement_reserve_atoms'],founder_claimable_via_halted_route=False,controlled_holder_B23_balances_zero=True,old_route_counter_atoms=r['authorized_valid_supply_atoms'],live_transactions=0,final_snapshot=str(c.current)))
    print(json.dumps(load(args.output/'result.json')))

if __name__=='__main__':main()
