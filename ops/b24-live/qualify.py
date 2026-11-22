#!/usr/bin/env python3
"""Qualify exact operator pins, source-series funding and consumer limits on copies."""
import argparse,copy
from sign import *
import common
import rehearse
OUT=COORD/'evidence/tee/phase5'
S='541e451e6ae96b6680988655571a0feec59c7f1d073c9b5d29547254cab38a3be540f8767446d25baebc7081ce97aef7'
def addresses():
    p=load(OUT/'operator.json');return {k:(v.lower() if isinstance(v,str) and v.startswith('0x') else v) for k,v in p.items() if k in ('deployer','token','verifier','controller','recipient','nonce','production_roles_appointed','mainnet_deployed')}
class LiveCopy(Copy):
    def __init__(self,snapshot,output):
        super().__init__(snapshot,output,signer=copy_signer);self.height=self.base.get('copy_execution_height',self.height)
    def apply(self,label,op,*args,**kwargs):
        if label=='route-enable':op=dict(op,settlement_source_asset_ids=[S])
        result=super().apply(label,op,*args,**kwargs)
        state=load(self.current);state['copy_execution_height']=self.height;save(self.current,state)
        return result
def offer(owner=ALICE,amount=100_000_000):
    return dict(operation='offer_create',owner=owner,taker_gets_asset_id=B24,taker_gets_amount=amount,taker_pays_asset_id=S,taker_pays_amount=amount,expiration_height=0)
def source_subscribe(c,amount,label,owner=ALICE,export=False):
    r=item(c.ledger()['pftl_uniswap_routes'],'route_id',ROUTE);p=r['v2']['primary_market_policy'];reservation=sha48('b24.source.reservation',label);due=(amount*10050+9999)//10000
    c.apply(label+'-reserve',dict(operation='pftl_uniswap_order_reserve',subscriber=owner,route_id=ROUTE,reservation_id=reservation,ethereum_recipient=addresses()['recipient'],route_epoch=r['v2']['route_epoch'],policy_epoch=p['policy_epoch'],policy_hash=p['policy_hash'],mint_amount_atoms=amount,max_settlement_value_atoms=due,expires_at_height=c.height+100,settlement_source_asset_id=S),owner)
    c.apply(label+'-subscribe',dict(operation='pftl_uniswap_primary_subscribe_v2',subscriber=owner,route_id=ROUTE,reservation_id=reservation,subscription_nonce=hashlib.sha256(label.encode()).hexdigest(),settlement_asset_id=PFUSDC,settlement_value_atoms=due,pricing_nav_epoch=p['pricing_nav_epoch'],pricing_reserve_packet_hash=p['pricing_reserve_packet_hash']),owner)
    if export:
        assert owner==ALICE;r=item(c.ledger()['pftl_uniswap_routes'],'route_id',ROUTE);c.apply(label+'-export',export_operation(r,amount,label,reservation),owner)
    r=item(c.ledger()['pftl_uniswap_routes'],'route_id',ROUTE);custody=next(x for x in c.ledger()['pftl_uniswap_source_custody'] if x['route_id']==ROUTE and x['asset_id']==S)
    assert custody['principal_atoms']==amount and custody['spread_atoms']==due-amount and invariant(r)
    return p,r
def redeem_op(c,owner,amount,source=None,label='redeem'):
    r=item(c.ledger()['pftl_uniswap_routes'],'route_id',ROUTE);p=r['v2']['primary_market_policy']
    op=dict(operation='pftl_uniswap_primary_redeem',owner=owner,settlement_recipient=owner,route_id=ROUTE,redemption_nonce=hashlib.sha256(label.encode()).hexdigest(),nav_amount_atoms=amount,min_settlement_value_atoms=amount*9995//10000,route_epoch=r['v2']['route_epoch'],policy_epoch=p['policy_epoch'],policy_hash=p['policy_hash'],pricing_nav_epoch=p['pricing_nav_epoch'],pricing_reserve_packet_hash=p['pricing_reserve_packet_hash'],expires_at_height=c.height+100)
    if source:op['settlement_source_asset_id']=source
    return op
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--copy-only',action='store_true',required=True)
    for n in ('capture','proof','output'):ap.add_argument('--'+n,type=Path,required=True)
    a=ap.parse_args();a.output.mkdir(parents=True,exist_ok=False)
    retired=load(OUT/'retirement-qualified/result.json');state=load(retired['final_snapshot']);state['copy_execution_height']=max(x['height'] for x in load(OUT/'retirement-qualified/receipts.json') if x['receipt']['accepted'])
    snapshot=a.output/'retired-copy-state.json';save(snapshot,state)
    common.predicted_addresses=addresses;rehearse.predicted_addresses=addresses;rehearse.Copy=LiveCopy;rehearse.EVIDENCE=OUT
    sys.argv=['rehearse.py','--copy-only','--snapshot',str(snapshot),'--capture',str(a.capture),'--proof',str(a.proof),'--output',str(a.output/'reserve-rehearsal')];rehearse.main()
    opening=a.output/'reserve-rehearsal/opening/route-enable-state.json'
    # Genuine available source-series balance: create and redeem one native unit.
    source=LiveCopy(opening,a.output/'actual-source-1');before=balance(source.ledger(),ALICE,S);source_subscribe(source,1_000_000,'source-actual-1')
    source.apply('source-cash-in',redeem_op(source,ALICE,1_000_000,S,'source-cash-in'),ALICE)
    assert balance(source.ledger(),ALICE,S)==before-5500
    save(source.output/'result.json',dict(passed=True,source_asset_id=S,actual_source_funds=True,issue_debit_atoms=1_005_000,redeem_credit_atoms=999_500,conversion_needed=False))
    # Conditional 5025-USDC bridge claim fixture, explicitly not a real deposit.
    funded=load(opening);addition=5_025_000_000;line=item([x for x in funded['ledger']['trustlines'] if x['account']==ALICE],'asset_id',S);line['balance']+=addition;line['limit']=max(line['limit'],line['balance'])
    definition=item(funded['ledger']['asset_definitions'],'asset_id',S);bucket=item(funded['ledger']['vault_bridge_bucket_states'],'bucket_id',definition['source_bucket_id'])
    for field in ('gross_receipt_atoms','counted_value_atoms','outstanding_vault_bridge_atoms'):bucket[field]+=addition
    item(funded['ledger']['nav_assets'],'asset_id',PFUSDC)['circulating_supply']+=addition
    fixture=a.output/'conditional-source-snapshot.json';save(fixture,funded)
    c=LiveCopy(fixture,a.output/'conditional-source-5000');source_subscribe(c,5_000_000_000,'source-conditional-5000',export=True)
    r=item(c.ledger()['pftl_uniswap_routes'],'route_id',ROUTE);assert r['outstanding_bridge_claims_atoms']==5_000_000_000
    context=c.output/'packet-context.json';subprocess.run([str(CANARY),'packet-context','--snapshot',str(c.current),'--public-values',str(a.proof/'public-values.bin'),'--output',str(context)],check=True)
    assert load(context)['overlay_value']==5_000_000_000 and load(context)['nav_per_unit']==1_000_000
    save(c.output/'result.json',dict(passed=True,conditional=True,actual_bridge_deposit=False,source_asset_id=S,subscription_family_field=PFUSDC,source_selector_on_reservation=True,paid_source_atoms=addition,export_atoms=5_000_000_000,overlay_atoms=5_000_000_000,spread_atoms=25_000_000,conversion_needed=False,fixture_changes=['Alice S balance/limit +5025','S backing bucket gross/count/outstanding +5025','PFUSDC NAV supply +5025'],real_finality=False))
    # Public order-book inventory works; route inventory does not follow legacy fills.
    market=LiveCopy(opening,a.output/'consumer-market');market.apply('bob-B24-trustline',dict(operation='trust_set',account=BOB,issuer=ISSUER,asset_id=B24,limit=1_000_000_000,reserve_paid=10,authorized=False,frozen=False),BOB)
    market.apply('fund-bob-from-existing-Alice-S',dict(operation='issued_payment',issuer=item(market.ledger()['asset_definitions'],'asset_id',S)['issuer'],asset_id=S,**{'from':ALICE,'to':BOB},amount=900_000),ALICE)
    market.apply('alice-resting-100',offer(),ALICE,kind='offer');market.apply('bob-buy-1',dict(operation='offer_create',owner=BOB,taker_gets_asset_id=S,taker_gets_amount=1_000_000,taker_pays_asset_id=B24,taker_pays_amount=1_000_000,expiration_height=0),BOB,kind='offer',code='filled')
    assert balance(market.ledger(),BOB,B24)==1_000_000
    market.apply('cash-in-empty-reserve',redeem_op(market,BOB,1_000_000,label='empty'),BOB,False,'pftl_uniswap_redemption_unavailable')
    source_subscribe(market,1_000_000,'market-alice-source-fund')
    market.apply('cash-in-book-buyer-untracked',redeem_op(market,BOB,1_000_000,S,'untracked'),BOB,False,'insufficient_pftl_uniswap_wallet_balance')
    market.apply('alice-source-redeem',redeem_op(market,ALICE,1_000_000,S,'alice-source-redeem'),ALICE)
    ask=next(o for o in market.ledger()['offers'] if o['owner']==ALICE and o['state']=='open' and o['taker_gets_asset_id']==B24)
    market.apply('cancel-Alice-ask-before-buyback',dict(operation='offer_cancel',owner=ALICE,offer_id=ask['offer_id']),ALICE,kind='offer')
    market.apply('bob-sell-1',offer(BOB,1_000_000),BOB,kind='offer')
    market.apply('alice-buy-back-1',dict(operation='offer_create',owner=ALICE,taker_gets_asset_id=S,taker_gets_amount=1_000_000,taker_pays_asset_id=B24,taker_pays_amount=1_000_000,expiration_height=0),ALICE,kind='offer',code='filled')
    assert balance(market.ledger(),BOB,B24)==0
    save(market.output/'result.json',dict(passed=True,market_buy_sell=True,book_buyer_primary_cash_in=False,book_buyer_cash_in_rejection='insufficient_pftl_uniswap_wallet_balance',opening_cash_in_reserve_atoms=0,tracked_subscriber_cash_in=True,source_series_cash_in=True,no_live_transactions=True))
    liveops=load(OUT/'retirement-qualified/operations.json')+load(a.output/'reserve-rehearsal/opening/operations.json')+[dict(label='alice-resting-100-B24',source=ALICE,kind='offer',operation=offer())]
    save(a.output/'live-operations.json',liveops);save(a.output/'result.json',dict(passed=True,fresh_capture=str(a.capture),proof=str(a.proof),actual_operator_addresses=addresses(),native_operations=len(liveops),live_operations_sha256=hashlib.sha256((a.output/'live-operations.json').read_bytes()).hexdigest(),source_S_supported_without_conversion=True,market_buy_sell=True,book_buyer_cash_in_unsupported=True,founder_allocation_atoms=1_000_000,founder_trustline_required=True,live_transactions=0))
    print(json.dumps(load(a.output/'result.json')))
if __name__=='__main__':main()
