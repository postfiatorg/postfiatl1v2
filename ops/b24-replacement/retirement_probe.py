#!/usr/bin/env python3
"""Execute only issuer/Alice retirement steps on a copy; never sign as other holders."""
import argparse
from common import *
from retirement import prepare

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--copy-only',action='store_true',required=True)
    ap.add_argument('--snapshot',type=Path,required=True);ap.add_argument('--output',type=Path,required=True);a=ap.parse_args()
    plan=prepare(a.snapshot);c=Copy(a.snapshot,a.output);deferred=[]
    for step in plan['operations']:
        if step['source'] not in (ISSUER,ALICE):deferred.append(step['label']);continue
        c.apply(step['label'],step['operation'],step['source'],kind=step['kind'])
    state=c.ledger();old=item(state['pftl_uniswap_routes'],'route_id',OLD_ROUTE);nav=item(state['nav_assets'],'asset_id',B23)
    alice=next(x for x in plan['holder_terms'] if x['owner']==ALICE)
    other_gross=sum(x['gross_PFUSDC_atoms'] for x in plan['holder_terms'] if x['owner']!=ALICE)
    assert old['settlement_reserve_atoms']==other_gross+plan['remaining_reserve_dust_atoms']
    assert balance(state,ALICE,B23)==0 and nav['halted'] and old['paused'] and not old.get('live_value_enabled',False)
    for holder in plan['holder_terms']:
        if holder['owner']!=ALICE:assert balance(state,holder['owner'],B23)==holder['old_B23_atoms']
    assert balance(state,ALICE,PFUSDC)-balance(c.base['ledger'],ALICE,PFUSDC)==alice['net_PFUSDC_atoms']
    assert old['authorized_valid_supply_atoms']==plan['old_supply_before_atoms']-alice['cash_in_B23_atoms']
    result=dict(passed=True,scope='COPY: issuer and Alice only; other holders untouched, their signatures NOT fabricated',
        deferred_holder_operations=deferred,alice_B23_remaining_atoms=0,other_holder_B23_remaining_atoms=sum(x['old_B23_atoms'] for x in plan['holder_terms'] if x['owner']!=ALICE),
        reserve_remaining_for_other_holders_atoms=old['settlement_reserve_atoms'],alice_payout_atoms=alice['net_PFUSDC_atoms'],
        route_counter_after_burn_atoms=old['authorized_valid_supply_atoms'],counter_reconciliation_required_for_zero_counter_retirement=True,
        complete_retirement_executed=False,founder_key_accessed=False,live_transactions=0)
    save(a.output/'result.json',result);print(json.dumps(result))

if __name__=='__main__':main()
