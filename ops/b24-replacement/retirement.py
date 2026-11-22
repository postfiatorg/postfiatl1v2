#!/usr/bin/env python3
"""Prepare exact unsigned B23 retirement operations; never opens holder keys or RPC.

Requires --dry-run. Proposed economics require each holder's authorization:
small holders cash out at the pinned B23 NAV/20bps; Alice receives the remaining
PFUSDC cash-out, voluntarily burns the remainder, and receives the whole-account
B24 opening inventory. No issuer clawback or founder signature is assumed.
"""
import argparse
import copy
from common import *

def prepare(snapshot):
    s=load(snapshot);ledger=s['ledger'];r=item(ledger['pftl_uniswap_routes'],'route_id',OLD_ROUTE)
    nav=item(ledger['nav_assets'],'asset_id',B23);definition=item(ledger['asset_definitions'],'asset_id',B23)
    assert not definition['clawback_enabled'] and not definition['freeze_enabled']
    assert r['outstanding_bridge_claims_atoms']==0 and r['ethereum_spendable_supply_atoms']==0
    assert not r['v2'].get('active_reservations',{}) and not r['v2'].get('export_entitlements',{})
    assert not any(x['asset_id']==B23 and x['state']=='open' for x in ledger.get('escrows',[]))
    p=copy.deepcopy(r['v2']['primary_market_policy']);assert p['redeem_multiplier_bps']==9980
    p.update(policy_epoch=p['policy_epoch']+1,min_order_atoms=1,valid_from_height=s['tip']['height']+1)
    p['policy_hash']=policy_hash(p);epoch=r['v2']['route_epoch']+1
    steps=[]
    def add(label,source,op,kind='asset'):steps.append(dict(label=label,source=source,kind=kind,operation=op))
    offers=[x for x in ledger['offers'] if x['state']=='open' and B23 in (x['taker_gets_asset_id'],x['taker_pays_asset_id'])]
    for i,o in enumerate(offers):add(f'cancel-offer-{i}',o['owner'],dict(operation='offer_cancel',owner=o['owner'],offer_id=o['offer_id']),'offer')
    add('pause-old-route',ISSUER,dict(operation='pftl_uniswap_route_pause',operator=ISSUER,route_id=OLD_ROUTE,paused=True))
    add('retirement-policy-minimum-one-atom',ISSUER,dict(operation='pftl_uniswap_route_epoch_advance',operator=ISSUER,route_id=OLD_ROUTE,
        prior_route_epoch=r['v2']['route_epoch'],next_route_epoch=epoch,next_route_config_digest=sha48('b23.retirement.policy',p),
        live_value_enabled=False,next_primary_market_policy=p))
    # Do not halt NAV until cash-outs finish: pricing rejects a halted NAV asset.
    available=r['settlement_reserve_atoms'];holders=[]
    positions=r['native_spendable_balances_atoms'];assert sum(positions.values())==r['pftl_spendable_supply_atoms']
    def redeem(owner,amount):
        nonlocal available
        gross=(amount*nav['nav_per_unit']+999999)//1_000_000;payout=gross*9980//10000
        assert gross<=available;available-=gross
        if not any(x['account']==owner and x['asset_id']==PFUSDC for x in ledger['trustlines']):
            settlement=item(ledger['asset_definitions'],'asset_id',PFUSDC)
            add('cash-out-trustline-'+owner,owner,dict(operation='trust_set',account=owner,issuer=settlement['issuer'],
                asset_id=PFUSDC,limit=max(payout,1_000_000_000),reserve_paid=10,authorized=False,frozen=False))
        op=dict(operation='pftl_uniswap_primary_redeem',owner=owner,settlement_recipient=owner,route_id=OLD_ROUTE,
            redemption_nonce=hashlib.sha256(('b24-retirement-'+owner).encode()).hexdigest(),nav_amount_atoms=amount,
            min_settlement_value_atoms=payout,route_epoch=epoch,policy_epoch=p['policy_epoch'],policy_hash=p['policy_hash'],
            pricing_nav_epoch=nav['finalized_epoch'],pricing_reserve_packet_hash=nav['finalized_reserve_packet_hash'],
            expires_at_height=s['tip']['height']+100)
        add('cash-out-'+owner,owner,op)
        return dict(owner=owner,old_B23_atoms=positions[owner],cash_in_B23_atoms=amount,gross_PFUSDC_atoms=gross,
            net_PFUSDC_atoms=payout,management_fee_atoms=gross-payout,owner_signature_required=True)
    for owner,amount in sorted(positions.items()):
        if owner!=ALICE:holders.append(redeem(owner,amount))
    alice_redeem=min(positions[ALICE],available*1_000_000//nav['nav_per_unit'])
    alice=redeem(ALICE,alice_redeem);burn=positions[ALICE]-alice_redeem
    alice.update(voluntary_burn_B23_atoms=burn,new_inventory='all B24 opening atoms equal to the newly attested account value; not a 1-for-1 denomination conversion')
    holders.append(alice)
    add('halt-old-B23',ISSUER,dict(operation='nav_halt',issuer=ISSUER,asset_id=B23,halted=True,reason='Retired by holder-approved B24 replacement; never reactivate'))
    add('alice-voluntary-burn',ALICE,dict(operation='asset_burn',owner=ALICE,issuer=ISSUER,asset_id=B23,amount=burn))
    return dict(schema='b24-retirement-dry-run-v1',production_authorized=False,snapshot_tip=s['tip'],
        holder_terms=holders,original_reserve_atoms=r['settlement_reserve_atoms'],remaining_reserve_dust_atoms=available,
        old_supply_before_atoms=r['authorized_valid_supply_atoms'],old_spendable_supply_after_atoms=0,
        old_historical_nav_and_route_counter_after_atoms=burn,
        warning='Generic asset_burn does not lower NAV/route counters. Zero holder supply plus paused/disabled route, halted NAV, zero claims and stopped old timer is operational retirement, not deletion or an immutable issuer revocation. Founder must approve this shape or require a migration upgrade.',
        preconditions=['Founder approves replacement, these per-holder cash-out/burn terms, new denomination, funding and roles.',
            'Each non-issuer holder signs its own operation; no founder/custody key is accessed by this preparation.',
            'Stop B23 daily timer and wait for any service completion only under the future go; current timer must remain untouched.',
            'Take chain.lock=TEE only for future live writes, export/reconcile all holders/offers/reserves again, and regenerate quotes/nonces.',
            'B24 NAV may not become live until all B23 holder balances/open custody/claims are zero and old route/NAV are disabled.'],
        operations=steps)

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--dry-run',action='store_true',required=True)
    ap.add_argument('--snapshot',type=Path,required=True);ap.add_argument('--output',type=Path,required=True);a=ap.parse_args()
    plan=prepare(a.snapshot);a.output.mkdir(parents=True,exist_ok=False)
    for n,step in enumerate(plan['operations'],1):save(a.output/f'{n:02d}-{step["label"]}.json',step)
    save(a.output/'plan.json',plan);print(json.dumps({'dry_run':True,'operations':len(plan['operations']),'holder_terms':plan['holder_terms'],'remaining_reserve_atoms':plan['remaining_reserve_dust_atoms']}))

if __name__=='__main__':main()
