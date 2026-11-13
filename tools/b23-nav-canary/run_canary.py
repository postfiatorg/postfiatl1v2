#!/usr/bin/env python3
"""Actual release-4 execution on an exported live ledger; never submits an RPC write.

The only RPC is asset_fee_quote. Signing uses the released node binary. Every
state transition executes postfiat-execution from the release-4 source tree.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys

HELPERS=Path('/home/postfiat/repos/postfiatl1v2-navcoin-proof-b2.2/.scratch-deposit')
sys.path.insert(0,str(HELPERS))
import pftl
import facility as f

def canonical(value):return json.dumps(value,sort_keys=True,separators=(',',':')).encode()
def save(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def load(path):return json.loads(path.read_bytes())
def item(rows,key,value):return next(x for x in rows if x[key]==value)
def outstanding(ledger,asset):
    # Exact issued_asset_stats basis used by the released node's asset_info.
    return (sum(x['balance'] for x in ledger.get('trustlines',[]) if x['asset_id']==asset)
        +sum(x['amount'] for x in ledger.get('escrows',[]) if x['asset_id']==asset and x['state']=='open')
        +sum(x['taker_gets_amount_remaining'] for x in ledger.get('offers',[]) if x['taker_gets_asset_id']==asset and x['state']=='open'))

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    for name in ('snapshot','capture','proof','native_cli','canary_bin','node','output'):
        ap.add_argument('--'+name.replace('_','-'),type=Path,required=True)
    args=ap.parse_args();args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    base=load(args.snapshot);m=load(args.capture/'manifest.json');profile=load(args.capture/'profile.json')
    public=bytes.fromhex(load(args.capture/'result.json')['body']['public_values_hex'])
    assert public==(args.proof/'public-values.bin').read_bytes()
    values=json.loads(subprocess.check_output([str(args.native_cli),'decode','--public-values',str(args.proof/'public-values.bin')]))
    proof=(args.proof/'proof-calldata.bin').read_bytes()
    asset=m['nav_asset_id'];issuer=f.ISSUER
    nav=item(base['ledger']['nav_assets'],'asset_id',asset)
    supply=outstanding(base['ledger'],asset)
    assert supply==nav['circulating_supply'] and supply>0
    assert values['observation_epoch']==nav['finalized_epoch']+1
    amount=values['verified_net_assets'];price=amount*1_000_000//supply
    packet_hash=hashlib.sha3_384(b'postfiat.b23.tee_nav_packet.v1\0'+canonical({
        'public_values_sha256':hashlib.sha256(public).hexdigest(),
        'proof_sha256':hashlib.sha256(proof).hexdigest(),'circulating_supply':supply,'nav_per_unit':price})).hexdigest()
    register=dict(load(args.capture/'profile-operation.json'),operation='nav_profile_register')
    rebind={k:nav[k] for k in ('issuer','asset_id','reserve_operator','valuation_unit','redemption_account')}
    rebind.update(operation='nav_asset_register',proof_profile=profile['profile_id'])
    submit=dict(operation='nav_reserve_submit',issuer=issuer,submitter=issuer,asset_id=asset,
        epoch=m['observation_epoch'],nav_per_unit=price,circulating_supply=supply,verified_net_assets=amount,
        proof_profile=profile['profile_id'],source_root=values['source_observation_root'],
        attestor_root=values['valuation_trust_root'],reserve_packet_hash=packet_hash,reserve_accounts=[],
        sp1_proof_bytes=list(proof),sp1_public_values=list(public))
    finalize=dict(operation='nav_epoch_finalize',issuer=issuer,asset_id=asset,epoch=m['observation_epoch'],reserve_packet_hash=packet_hash)
    route=item(base['ledger']['pftl_uniswap_routes'],'route_id',f.ROUTE_ID)
    policy=copy.deepcopy(route['v2']['primary_market_policy'])
    assert f.policy_hash(policy)==policy['policy_hash']
    policy.update(policy_epoch=policy['policy_epoch']+1,pricing_nav_epoch=m['observation_epoch'],
                  pricing_reserve_packet_hash=packet_hash,valid_from_height=base['tip']['height']+1)
    policy['policy_hash']=f.policy_hash(policy)
    advance=dict(operation='pftl_uniswap_route_epoch_advance',operator=issuer,route_id=f.ROUTE_ID,
        prior_route_epoch=route['v2']['route_epoch'],next_route_epoch=route['v2']['route_epoch']+1,
        next_route_config_digest=f.sha48(b'b23.route.policy',policy['policy_hash'].encode()),
        live_value_enabled=route['live_value_enabled'],next_primary_market_policy=policy)
    operations=[('register-profile',register),('rebind',rebind),('submit',submit),('finalize',finalize),('route-pricing',advance)]
    save(args.output/'authorized-live-operations.json',[{'label':label,'source':issuer,'operation':op} for label,op in operations])
    save(args.output/'expected.json',dict(supply_atoms=supply,verified_net_assets=amount,nav_per_unit=price,
        conservative_rounding_numerator=amount*1_000_000-supply*price,profile=profile['profile_id'],
        epoch=m['observation_epoch'],reserve_packet_hash=packet_hash,attested_value=amount,
        source_count=1,cryptographically_verified_value=0,route_policy=policy))
    current=args.snapshot;report=[]
    def apply(label,op,height,*,snapshot=None,accepted=True,code=None,source=issuer,key=f.ISSUER_KEY):
        source_snapshot=snapshot or current;state=load(source_snapshot)
        account=item(state['ledger']['accounts'],'address',source)
        quote=pftl.rpc('asset_fee_quote',dict(source=source,sequence=account['sequence']+1,
            operation_json=json.dumps(op,separators=(',',':'))),'tee-canary-'+label)
        q=args.output/(label+'-quote.json');save(q,quote)
        signed=json.loads(subprocess.check_output([str(args.node),'wallet-sign-asset-transaction',
            '--key-file',str(key),'--quote-file',str(q)],stderr=subprocess.PIPE))
        steps=args.output/(label+'-steps.json');save(steps,[dict(label=label,height=height,
            transaction=signed,expect_accepted=accepted,expect_code=code)])
        target=args.output/(label+'-state.json')
        result=subprocess.run([str(args.canary_bin),'apply','--snapshot',str(source_snapshot),
            '--steps',str(steps),'--output',str(target)],capture_output=True)
        if target.exists():
            observed=load(target)['canary_receipts'];report.extend(observed);save(args.output/'receipts.json',report)
        if result.returncode:raise RuntimeError('CANARY FAILED: '+label+'; inspect literal receipts')
        return target
    h=base['tip']['height']
    for offset,(label,op) in enumerate(operations,1):
        if label=='submit':
            bad=copy.deepcopy(submit);bad['sp1_proof_bytes'][-1]^=1
            apply('reject-bad-proof',bad,h+offset,accepted=False,code='sp1_proof_invalid')
            # Same valid proof, evaluated beyond the pinned block-age limit.
            apply('reject-stale-proof',submit,m['observation_height']+profile['max_snapshot_age_blocks']+1,accepted=False,code='nav_reserve_observation_stale')
            # Account and PCR policy are committed in source_manifest_hash. Neither
            # can be replaced while retaining the valid proof's public values.
            for label_bad,position in [('reject-wrong-account-policy',220),('reject-wrong-pcr-policy',221)]:
                bad=copy.deepcopy(submit);bad['sp1_public_values'][position]^=1
                apply(label_bad,bad,h+offset,accepted=False,code='sp1_proof_invalid')
        current=apply(label,op,h+offset)
        state=load(current)['ledger']
        assert outstanding(state,asset)==supply
        assert item(state['nav_assets'],'asset_id',asset)['circulating_supply']==supply
        assert state['offers']==base['ledger']['offers']
    before=load(current)['ledger'];newnav=item(before['nav_assets'],'asset_id',asset)
    assert newnav['nav_per_unit']==price and not newnav['halted']
    newroute=item(before['pftl_uniswap_routes'],'route_id',f.ROUTE_ID)
    assert newroute['v2']['primary_market_policy']['pricing_nav_epoch']==m['observation_epoch']
    assert newroute['authorized_valid_supply_atoms']==supply
    # Existing product cash-in is B23 redemption into pooled family PFUSDC.
    # One B23 is burned ONLY inside this isolated canary ledger.
    redeemed=1_000_000;gross=(redeemed*price+999999)//1_000_000
    payout=gross*policy['redeem_multiplier_bps']//10000
    redeem=dict(operation='pftl_uniswap_primary_redeem',settlement_source_asset_id=None,
        owner=f.HOLDER,settlement_recipient=f.HOLDER,route_id=f.ROUTE_ID,
        redemption_nonce=hashlib.sha256(b'b23-tee-nav-canary-cash-in'+packet_hash.encode()).hexdigest(),
        nav_amount_atoms=redeemed,min_settlement_value_atoms=payout,
        route_epoch=advance['next_route_epoch'],policy_epoch=policy['policy_epoch'],policy_hash=policy['policy_hash'],
        pricing_nav_epoch=m['observation_epoch'],pricing_reserve_packet_hash=packet_hash,expires_at_height=h+20)
    current=apply('cash-in',redeem,h+6,source=f.HOLDER,key=f.HOLDER_KEY)
    after=load(current)['ledger'];afterroute=item(after['pftl_uniswap_routes'],'route_id',f.ROUTE_ID)
    def balance(ledger,account,asset):return next((x['balance'] for x in ledger['trustlines'] if x['account']==account and x['asset_id']==asset),0)
    checks=dict(profile_switch_supply_unchanged=True,finalized_epoch=newnav['finalized_epoch']==m['observation_epoch'],
        holder_pfusdc_delta=balance(after,f.HOLDER,f.PFUSDC)-balance(before,f.HOLDER,f.PFUSDC)==payout,
        holder_b23_delta=balance(after,f.HOLDER,asset)-balance(before,f.HOLDER,asset)==-redeemed,
        management_pfusdc_delta=balance(after,policy['spread_recipient'],f.PFUSDC)-balance(before,policy['spread_recipient'],f.PFUSDC)==gross-payout,
        reserve_delta=afterroute['settlement_reserve_atoms']-newroute['settlement_reserve_atoms']==-gross,
        supply_delta=outstanding(after,asset)-supply==-redeemed,
        route_supply_delta=afterroute['authorized_valid_supply_atoms']-supply==-redeemed)
    save(args.output/'result.json',dict(passed=all(checks.values()),checks=checks,base_tip=base['tip'],
        authorized_operations_sha256=hashlib.sha256((args.output/'authorized-live-operations.json').read_bytes()).hexdigest(),
        cash_in=dict(b23_atoms=redeemed,gross_pfusdc_atoms=gross,payout_pfusdc_atoms=payout),
        no_live_transactions=True))
    assert all(checks.values()),'CANARY FAILED: cash-in deltas'
    print(json.dumps({'canary_passed':True,'nav_per_unit':price,'supply_atoms':supply,'cash_in_payout_atoms':payout}))

if __name__=='__main__':main()
