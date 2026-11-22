#!/usr/bin/env python3
"""Real new-asset reserve proof through strict release-4 COPY execution only.

No RPC submission or Ethereum transaction path. The 5000-unit branch explicitly
models newly cleared PFUSDC funding; the unmodified branch uses actual balances.
"""
import argparse
import copy
import hashlib
import json
import subprocess
from common import *

def subscribe(c,amount,label):
    route=item(c.ledger()['pftl_uniswap_routes'],'route_id',ROUTE);p=route['v2']['primary_market_policy']
    reservation=sha48('b24.reservation',label);due=(amount*10050+9999)//10000
    reserve=dict(operation='pftl_uniswap_order_reserve',subscriber=ALICE,route_id=ROUTE,reservation_id=reservation,
        ethereum_recipient=predicted_addresses()['recipient'],route_epoch=route['v2']['route_epoch'],
        policy_epoch=p['policy_epoch'],policy_hash=p['policy_hash'],mint_amount_atoms=amount,
        max_settlement_value_atoms=due,expires_at_height=c.height+100)
    c.apply(label+'-reserve',reserve,ALICE)
    op=dict(operation='pftl_uniswap_primary_subscribe_v2',subscriber=ALICE,route_id=ROUTE,reservation_id=reservation,
        subscription_nonce=hashlib.sha256(label.encode()).hexdigest(),settlement_asset_id=PFUSDC,settlement_value_atoms=due,
        pricing_nav_epoch=p['pricing_nav_epoch'],pricing_reserve_packet_hash=p['pricing_reserve_packet_hash'])
    c.apply(label+'-subscribe',op,ALICE)
    route=item(c.ledger()['pftl_uniswap_routes'],'route_id',ROUTE)
    c.apply(label+'-export',export_operation(route,amount,label,reservation),ALICE)
    final=c.ledger();route=item(final['pftl_uniswap_routes'],'route_id',ROUTE)
    assert invariant(route) and route['outstanding_bridge_claims_atoms']==amount
    assert route['settlement_reserve_atoms']==amount and route['v2']['non_nav_spread_atoms']==due-amount
    assert not route['v2'].get('active_reservations',{}) and not route['v2'].get('export_entitlements',{})
    assert 'spread_recipient' not in route['v2']['primary_market_policy'] and route['v2'].get('funding_sequence') is None
    assert route['pftl_spendable_supply_atoms']==balance(final,ALICE,B24)
    replay=export_operation(route,amount,label,reservation)
    c.apply(label+'-reject-replay',replay,ALICE,False,'missing_pftl_uniswap_export_entitlement')
    save(c.output/'result.json',dict(passed=True,export_atoms=amount,settlement_paid_atoms=due,
        reserve_principal_atoms=amount,non_nav_spread_atoms=due-amount,route_invariant=True,
        native_supply_atoms=route['pftl_spendable_supply_atoms'],total_supply_atoms=route['authorized_valid_supply_atoms'],
        pending_export_atoms=amount,ethereum_minted_atoms=0,live_finality=False))
    return route

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--copy-only',action='store_true',required=True)
    for name in ('snapshot','capture','proof','output'):ap.add_argument('--'+name,type=Path,required=True)
    a=ap.parse_args();m=load(a.capture/'manifest.json');profile=load(a.capture/'profile.json')
    assert m['nav_asset_id']==B24 and m['observation_epoch']==1 and load(a.proof/'qualification.json')['passed']
    values=json.loads(subprocess.check_output([str(NATIVE),'decode','--public-values',str(a.proof/'public-values.bin')]))
    public=(a.proof/'public-values.bin').read_bytes();proof=(a.proof/'proof-calldata.bin').read_bytes()
    assert public==bytes.fromhex(load(a.capture/'result.json')['body']['public_values_hex'])
    amount=values['verified_net_assets'];assert values['attested_value']==amount and values['source_count']==1
    c=Copy(a.snapshot,a.output/'opening');h=c.height;addresses=predicted_addresses();save(a.output/'predicted-addresses.json',addresses)
    c.apply('create',dict(operation='asset_create',issuer=ISSUER,code='B24',version=1,precision=6,display_name='Schwab NAVCoin B24',max_supply=None,requires_authorization=False,freeze_enabled=False,clawback_enabled=False))
    assert item(c.ledger()['asset_definitions'],'code','B24')['asset_id']==B24
    c.apply('profile',dict(load(a.capture/'profile-operation.json'),operation='nav_profile_register'))
    c.apply('nav-register',dict(operation='nav_asset_register',issuer=ISSUER,asset_id=B24,reserve_operator=ISSUER,redemption_account=ISSUER,valuation_unit='USDC',proof_profile=profile['profile_id']))
    c.apply('opening-holder-trustline',dict(operation='trust_set',account=ALICE,issuer=ISSUER,asset_id=B24,limit=2_000_000_000_000,reserve_paid=10,authorized=False,frozen=False),ALICE)
    packet=sha48('b24.nav.packet',dict(public_sha256=hashlib.sha256(public).hexdigest(),proof_sha256=hashlib.sha256(proof).hexdigest(),supply_atoms=amount,nav_per_unit=1_000_000))
    submit=dict(operation='nav_reserve_submit',issuer=ISSUER,submitter=ISSUER,asset_id=B24,epoch=1,
        nav_per_unit=1_000_000,circulating_supply=amount,verified_net_assets=amount,proof_profile=profile['profile_id'],
        source_root=values['source_observation_root'],attestor_root=values['valuation_trust_root'],reserve_packet_hash=packet,
        reserve_accounts=[],sp1_proof_bytes=list(proof),sp1_public_values=list(public))
    bad=copy.deepcopy(submit);bad['sp1_proof_bytes'][-1]^=1
    c.apply('reject-bad-proof',bad,accepted=False,code='sp1_proof_invalid')
    c.apply('reject-stale-reading',submit,accepted=False,code='nav_reserve_observation_stale',height=m['observation_height']+profile['max_snapshot_age_blocks']+1)
    c.apply('reserve-submit',submit)
    c.apply('finalize',dict(operation='nav_epoch_finalize',issuer=ISSUER,asset_id=B24,epoch=1,reserve_packet_hash=packet))
    c.apply('mint-whole-account-inventory',dict(operation='nav_mint_at_nav',issuer=ISSUER,to=ALICE,asset_id=B24,epoch=1,reserve_packet_hash=packet,amount=amount))
    old=item(c.base['ledger']['pftl_uniswap_routes'],'route_id',OLD_ROUTE)
    policy=dict(policy_epoch=1,issue_multiplier_bps=10050,redeem_multiplier_bps=9995,
        issue_capacity_atoms=250_000_000_000,redeem_capacity_atoms=250_000_000_000,max_order_atoms=250_000_000_000,
        min_order_atoms=1_000_000,valid_from_height=h,expires_at_height=h+100_000,max_nav_age_blocks=1000,
        pricing_nav_epoch=1,pricing_reserve_packet_hash=packet)
    policy['policy_hash']=policy_hash(policy)
    deployment=load(EVIDENCE/'deployment-pins.json')
    assert deployment['passed'] and deployment['return_paused'] and not deployment['mainnet_deployed']
    ethereum_policy=copy.deepcopy(old['ethereum_verification_policy'])
    ethereum_policy['handoff_controller_code_hash']=list(bytes.fromhex(deployment['controller_runtime_code_hash'][2:]))
    ethereum_policy['wrapped_navcoin_code_hash']=list(bytes.fromhex(deployment['token_runtime_code_hash'][2:]))
    init=dict(operation='pftl_uniswap_route_init_v2',operator=ISSUER,route_id=ROUTE,
        route_config_digest=sha48('b24.route',{'addresses':addresses,'policy':policy}),native_nav_asset_id=B24,
        settlement_asset_id=PFUSDC,opening_inventory_atoms=amount,opening_inventory_holder=ALICE,
        handoff_controller=addresses['controller'],settlement_adapter=addresses['controller'],wrapped_navcoin_token=addresses['token'],
        ethereum_chain_id=1,route_supply_cap_atoms=2_000_000_000_000,packet_notional_cap_atoms=250_000_000_000,
        latest_finalized_nav_epoch=1,return_finality_blocks=old['return_finality_blocks'],route_epoch=1,
        outbound_verification_class='TRUSTLESS_FINALITY',return_verification_class='BFT_CHECKPOINT',live_value_enabled=False,
        ethereum_verification_policy=ethereum_policy,primary_market_policy=policy)
    c.apply('route-init-disabled',init)
    policy=dict(policy,policy_epoch=2);policy['policy_hash']=policy_hash(policy)
    c.apply('route-enable',dict(operation='pftl_uniswap_route_epoch_advance',operator=ISSUER,route_id=ROUTE,
        prior_route_epoch=1,next_route_epoch=2,next_route_config_digest=sha48('b24.route.epoch2',{'addresses':addresses,'policy':policy}),
        live_value_enabled=True,next_primary_market_policy=policy))
    route=item(c.ledger()['pftl_uniswap_routes'],'route_id',ROUTE)
    assert invariant(route) and route['authorized_valid_supply_atoms']==amount
    c.apply('reject-unreserved-5000',export_operation(route,5_000_000_000,'unreserved'),ALICE,False,'pftl_uniswap_export_entitlement_required')
    c.apply('reject-whole-opening-over-packet-cap',export_operation(route,amount,'whole-opening'),ALICE,False,'pftl_uniswap_packet_cap_exceeded')
    reserve=dict(operation='pftl_uniswap_order_reserve',subscriber=ALICE,route_id=ROUTE,reservation_id=sha48('b24.reservation','unfunded'),
        ethereum_recipient=addresses['recipient'],route_epoch=2,policy_epoch=2,policy_hash=policy['policy_hash'],mint_amount_atoms=5_000_000_000,
        max_settlement_value_atoms=5_025_000_000,expires_at_height=c.height+100)
    c.apply('reject-unfunded-5000-reservation',reserve,ALICE,False,'insufficient_issued_balance')
    save(a.output/'opening-result.json',dict(passed=True,nav_per_unit=1_000_000,supply_atoms=amount,attested_value=amount,
        asset_id=B24,profile=profile['profile_id'],source_count=1,cryptographically_verified_value=0,route_invariant=True,
        original_B23_unchanged=True,scope='ISOLATED COPY ONLY; B24 does not exist live'))
    original=c.base['ledger'];now=c.ledger()
    for field in ('nav_assets','trustlines','offers','pftl_uniswap_routes'):
        assert [x for x in now[field] if x.get('asset_id')==B23 or x.get('native_nav_asset_id')==B23 or x.get('taker_gets_asset_id')==B23]==[x for x in original[field] if x.get('asset_id')==B23 or x.get('native_nav_asset_id')==B23 or x.get('taker_gets_asset_id')==B23]
    real=Copy(c.current,a.output/'actual-balance-20');subscribe(real,20_000_000,'actual-20')
    def nav_context(branch,expected_overlay):
        path=branch.output/'next-packet-context.json'
        subprocess.run([str(CANARY),'packet-context','--snapshot',str(branch.current),'--public-values',str(a.proof/'public-values.bin'),'--output',str(path)],check=True)
        context=load(path)
        assert context['overlay_value']==expected_overlay and context['nav_per_unit']==1_000_000
        assert context['attested_value']==amount and context['verified_net_assets']==amount+expected_overlay
        return context
    nav_context(real,20_000_000)
    # Explicit conditional-funding fixture, separate from the unmodified live export.
    funded=load(c.current);line=item([x for x in funded['ledger']['trustlines'] if x['account']==ALICE],'asset_id',PFUSDC)
    addition=5_025_000_000;line['balance']+=addition;line['limit']=max(line['limit'],line['balance'])
    item(funded['ledger']['nav_assets'],'asset_id',PFUSDC)['circulating_supply']+=addition
    bucket=next(b for b in funded['ledger']['vault_bridge_bucket_states'] if b['asset_id']==PFUSDC and b['status']=='active' and b['source_domain'].startswith('erc20_bridge_vault:1:'))
    for field in ('gross_receipt_atoms','counted_value_atoms','outstanding_vault_bridge_atoms'):bucket[field]+=addition
    fixture=a.output/'conditional-funded-snapshot.json';save(fixture,funded)
    save(a.output/'funding-fixture-disclosure.json',dict(synthetic=True,cleared_deposit_proved=False,
        scope='Conditional 5000-unit subscription mechanics only; not evidence of available funds or deposit backing',
        changes=[{'field':'Alice pooled PFUSDC trustline balance/limit','added_atoms':addition},{'field':'PFUSDC NAV circulating supply','added_atoms':addition},
            {'field':'active PFUSDC bucket gross_receipt_atoms/counted_value_atoms/outstanding_vault_bridge_atoms','bucket_id':bucket['bucket_id'],'added_atoms':addition}],
        original_PFUSDC_balance_atoms=balance(c.ledger(),ALICE,PFUSDC),required_new_settlement_atoms=addition,
        reserve_proof_for_B24_is_genuine=True,finality_is_synthetic=False,finality_created=False))
    conditional=Copy(fixture,a.output/'conditional-funded-5000');subscribe(conditional,5_000_000_000,'conditional-5000')
    nav_context(conditional,5_000_000_000)
    save(a.output/'result.json',dict(passed=True,real_reserve_proof=True,opening_inventory_atoms=amount,nav_per_unit=1_000_000,
        actual_balance_export_atoms=20_000_000,conditional_funding_export_atoms=5_000_000_000,
        conditional_funding_required_atoms=5_025_000_000,live_transactions=0,ethereum_transactions=0,real_finality_created=False,
        mainnet_ready=False,return_enabled=False))
    print(json.dumps(load(a.output/'result.json')))

if __name__=='__main__':main()
