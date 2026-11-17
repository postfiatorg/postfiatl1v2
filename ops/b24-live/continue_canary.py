#!/usr/bin/env python3
"""Qualify the exact remaining Phase 5c sequence and test-funded redemption reserve."""
import argparse,copy
from sign import *
from qualify import LiveCopy,redeem_op
OUT=COORD/'evidence/tee/phase5c';OLD=COORD/'evidence/tee/phase5'
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--copy-only',action='store_true',required=True);ap.parse_args()
    d=OUT/'continue-canary';d.mkdir(mode=0o700,exist_ok=False);snapshot=OUT/'before-state.json';s=load(snapshot);assert s['tip']['height']==1317
    ops=load(OLD/'final-copy/live-operations.json')[7:];assert len(ops)==11 and ops[0]['label']=='alice-voluntary-burn'
    signed_path=OLD/'live/08-alice-voluntary-burn/transaction-signed.json';signed=load(signed_path)
    save(d/'burn-steps.json',[dict(label='same-signed-burn',height=1318,transaction=signed,kind='asset',expect_accepted=True)])
    burn=d/'burn-state.json';subprocess.run([str(CANARY),'apply','--snapshot',str(snapshot),'--steps',str(d/'burn-steps.json'),'--output',str(burn)],check=True,capture_output=True)
    b=load(burn);b['copy_execution_height']=1318;save(burn,b);ops[0]['signed_transaction_file']=str(signed_path)
    c=LiveCopy(burn,d/'opening');m=load(OLD/'capture/manifest.json');profile=load(OLD/'capture/profile.json')
    for row in ops[1:]:
        if row['label']=='reserve-submit':
            bad=copy.deepcopy(row['operation']);bad['sp1_proof_bytes'][-1]^=1;c.apply('reject-bad-proof',bad,accepted=False,code='sp1_proof_invalid')
            c.apply('reject-height-stale-reading',row['operation'],accepted=False,code='nav_reserve_observation_stale',height=m['observation_height']+profile['max_snapshot_age_blocks']+1)
        c.apply(row['label'],row['operation'],row['source'],kind=row['kind'])
    opening=c.current;before_F=balance(c.ledger(),ALICE,PFUSDC);assert before_F>=20_100_000
    reserve=sha48('b24.phase5c.test-reserve','20-F');r=item(c.ledger()['pftl_uniswap_routes'],'route_id',ROUTE);p=r['v2']['primary_market_policy'];address=load(OLD/'operator.json')['address'].lower()
    additions=[dict(label='fund-small-reserve-reserve',source=ALICE,kind='asset',operation=dict(operation='pftl_uniswap_order_reserve',subscriber=ALICE,route_id=ROUTE,reservation_id=reserve,ethereum_recipient=address,route_epoch=r['v2']['route_epoch'],policy_epoch=p['policy_epoch'],policy_hash=p['policy_hash'],mint_amount_atoms=20_000_000,max_settlement_value_atoms=20_100_000,expires_at_height=c.height+100)),dict(label='fund-small-reserve-subscribe',source=ALICE,kind='asset',operation=dict(operation='pftl_uniswap_primary_subscribe_v2',subscriber=ALICE,route_id=ROUTE,reservation_id=reserve,subscription_nonce=hashlib.sha256(b'b24.phase5c.test-reserve.20').hexdigest(),settlement_asset_id=PFUSDC,settlement_value_atoms=20_100_000,pricing_nav_epoch=p['pricing_nav_epoch'],pricing_reserve_packet_hash=p['pricing_reserve_packet_hash'])),dict(label='release-test-reserve-export-capacity',source=ALICE,kind='asset',operation=dict(operation='pftl_uniswap_order_release',releaser=ALICE,route_id=ROUTE,reservation_id=reserve))]
    for row in additions:c.apply(row['label'],row['operation'],row['source'])
    ops+=additions;final=c.current;r=item(c.ledger()['pftl_uniswap_routes'],'route_id',ROUTE)
    assert r['settlement_reserve_atoms']==20_000_000 and r['v2']['non_nav_spread_atoms']==100_000 and invariant(r)
    assert balance(c.ledger(),ALICE,PFUSDC)==before_F-20_100_000 and not r['v2'].get('active_reservations') and not r['v2'].get('export_entitlements')
    context=d/'packet-context.json';subprocess.run([str(CANARY),'packet-context','--snapshot',str(final),'--public-values',str(OLD/'real-proof/public-values.bin'),'--output',str(context)],check=True,capture_output=True);x=load(context);assert x['overlay_value']==20_000_000 and x['nav_per_unit']==1_000_000
    # A paid native subscriber can actually redeem; this test is copy-only.
    cash=LiveCopy(final,d/'cash-in-check');cash.apply('redeem-one-test-unit',redeem_op(cash,ALICE,1_000_000,label='phase5c-native-reserve-check'),ALICE)
    assert balance(cash.ledger(),ALICE,PFUSDC)==balance(c.ledger(),ALICE,PFUSDC)+999_500
    # Releasing the unused export entitlement avoids blocking the daily epoch advance.
    advance=LiveCopy(final,d/'daily-policy-check');nextp=copy.deepcopy(p);nextp['policy_epoch']+=1;nextp['policy_hash']=policy_hash(nextp)
    advance.apply('epoch-advance-after-entitlement-release',dict(operation='pftl_uniswap_route_epoch_advance',operator=ISSUER,route_id=ROUTE,prior_route_epoch=r['v2']['route_epoch'],next_route_epoch=r['v2']['route_epoch']+1,next_route_config_digest=sha48('b24.phase5c.copy.daily',nextp),live_value_enabled=True,next_primary_market_policy=nextp))
    save(d/'operations.json',ops);save(d/'result.json',dict(passed=True,reading_reused_UTC='2026-11-14T13:30:11.439Z',release4_freshness_accepted=True,observation_height=m['observation_height'],planned_reserve_submit_height=next(z['height'] for z in c.receipts if z['label']=='reserve-submit'),stale_height_negative_rejected=True,bad_proof_negative_rejected=True,same_burn_signature=True,live_transactions=0,remaining_operations=len(ops),operations_sha256=hashlib.sha256((d/'operations.json').read_bytes()).hexdigest(),final_snapshot=str(final),opening_supply_atoms=420302380000,final_supply_atoms=r['authorized_valid_supply_atoms'],reserve_atoms=20_000_000,spread_atoms=100_000,existing_test_F_debit_atoms=20_100_000,founder_funds_used=False,copy_cash_in_credit_atoms=999_500,export_entitlement_released=True,daily_policy_advance_passed=True))
    print(json.dumps(load(d/'result.json')))
if __name__=='__main__':main()
