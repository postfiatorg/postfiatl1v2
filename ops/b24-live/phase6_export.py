#!/usr/bin/env python3
"""One real paid B24 export, gated on completion of the scheduled daily run."""
import argparse,fcntl,os,time
from datetime import datetime,timezone
from sign import *
import live
from snapshot import export
from qualify import LiveCopy
import archive
OUT=COORD/'evidence/tee/phase6'
DAILY=COORD/'evidence/tee/b24-daily/20261114'

def operations(state,amount,label,source_asset=None):
    r=item(state['ledger']['pftl_uniswap_routes'],'route_id',ROUTE);p=r['v2']['primary_market_policy'];n=item(state['ledger']['nav_assets'],'asset_id',B24)
    assert n['finalized_epoch']==p['pricing_nav_epoch'] and n['finalized_reserve_packet_hash']==p['pricing_reserve_packet_hash']
    assert invariant(r) and not r['paused'] and r['live_value_enabled'] and not n['halted']
    # Six-decimal B24 and PFUSDC, both valued in micro-USD. Round each stage up.
    assert all(item(state['ledger']['asset_definitions'],'asset_id',a)['precision']==6 for a in (B24,PFUSDC))
    base=(amount*n['nav_per_unit']+999999)//1000000;due=(base*p['issue_multiplier_bps']+9999)//10000
    recipient=load(OUT/'operator.json')['address'].lower();reservation=sha48('b24.phase6.reservation',label);packet=sha48('b24.phase6.export',label);nonce=hashlib.sha256(label.encode()).hexdigest();deadline=int(time.time())+86400
    reserve=dict(operation='pftl_uniswap_order_reserve',subscriber=ALICE,route_id=ROUTE,reservation_id=reservation,ethereum_recipient=recipient,route_epoch=r['v2']['route_epoch'],policy_epoch=p['policy_epoch'],policy_hash=p['policy_hash'],mint_amount_atoms=amount,max_settlement_value_atoms=due,expires_at_height=state['tip']['height']+100)
    if source_asset:reserve['settlement_source_asset_id']=source_asset
    subscribe=dict(operation='pftl_uniswap_primary_subscribe_v2',subscriber=ALICE,route_id=ROUTE,reservation_id=reservation,subscription_nonce=hashlib.sha256((label+'.subscribe').encode()).hexdigest(),settlement_asset_id=PFUSDC,settlement_value_atoms=due,pricing_nav_epoch=p['pricing_nav_epoch'],pricing_reserve_packet_hash=p['pricing_reserve_packet_hash'])
    identifiers=[r['route_config_digest'],packet,reservation,PFUSDC,B24,p['pricing_reserve_packet_hash']]
    packed=Web3.keccak(text='postfiat.pftl_uniswap.mint_packet.v2')+b''.join(Web3.keccak(bytes.fromhex(x)) for x in identifiers)+Web3.keccak(bytes.fromhex(p['policy_hash']))
    packed+=b''.join(x.to_bytes(8,'big') for x in (r['v2']['route_epoch'],p['pricing_nav_epoch'],deadline))+bytes.fromhex(nonce)+(1).to_bytes(32,'big')
    packed+=b''.join(bytes.fromhex(x[2:]) for x in (r['handoff_controller'],r['wrapped_navcoin_token'],recipient))+amount.to_bytes(32,'big')+due.to_bytes(32,'big')
    debit=dict(operation='pftl_uniswap_export_debit',owner=ALICE,route_id=ROUTE,packet_hash=packet,export_nonce=nonce,ethereum_recipient=recipient,amount_atoms=amount,destination_deadline_seconds=deadline,refund_delay_blocks=100,settlement_value_atoms=due,ethereum_packet_digest=Web3.keccak(packed).hex(),ethereum_packet_schema_version=2,reservation_id=reservation)
    return [dict(label=label+'-'+k,source=ALICE,kind='asset',operation=v) for k,v in [('reserve',reserve),('subscribe',subscribe),('export',debit)]]

def qualify(start,d):
    s=load(start);ops=operations(s,1000000,'real-one-B24');c=LiveCopy(start,d/'copy')
    before=balance(s['ledger'],ALICE,PFUSDC);assert before>=ops[0]['operation']['max_settlement_value_atoms']
    for row in ops:c.apply(row['label'],row['operation'],ALICE)
    after=c.ledger();r=item(after['pftl_uniswap_routes'],'route_id',ROUTE);old=item(s['ledger']['pftl_uniswap_routes'],'route_id',ROUTE)
    assert invariant(r) and r['outstanding_bridge_claims_atoms']==old['outstanding_bridge_claims_atoms']+1000000
    assert r['pftl_spendable_supply_atoms']==old['pftl_spendable_supply_atoms'] and r['authorized_valid_supply_atoms']==old['authorized_valid_supply_atoms']+1000000
    assert before-balance(after,ALICE,PFUSDC)==ops[0]['operation']['max_settlement_value_atoms']
    save(d/'operations.json',ops);save(d/'qualification.json',dict(passed=True,amount_atoms=1000000,paid_F_atoms=before-balance(after,ALICE,PFUSDC),actual_existing_funds=True,founder_funds=False,live_transactions=0))
    return ops

def execute():
    assert (DAILY/'complete.json').exists(),'Daily run must finish successfully first'
    assert not (DAILY/'failure.json').exists()
    daily=load(DAILY/'complete.json');assert daily['epoch']==2 and daily['six_validator_verified']
    unit=subprocess.check_output(['systemctl','--user','show','b24-tee-nav-daily.service','--property=ActiveState,SubState,Result,MainPID'],text=True)
    assert 'ActiveState=inactive' in unit and 'MainPID=0' in unit and 'Result=success' in unit
    assert load(OUT/'mainnet-deployment/result.json')['passed']
    from phase6_deploy import PUBLIC_RPC,verify
    w=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));e=load(OUT/'deployment-expected.json');verify(w,e,5)
    # Conservative allowance for this proof/mint/re-pause AND later ingress/proof/LP.
    fee=max(2*w.eth.get_block('latest')['baseFeePerGas']+w.eth.max_priority_fee,2000000000);allowance=4500000*fee
    assert w.eth.get_balance(e['operator']['address'])>=allowance,'ETH insufficient for test plus future launch allowance'
    d=OUT/'real-export';d.mkdir(mode=0o700,exist_ok=False);save(d/'gate.json',dict(daily=daily,unit=unit,fee_cap_wei=fee,combined_future_gas_allowance=4500000,required_wei=allowance,checked_at=datetime.now(timezone.utc).isoformat()))
    live.OUT=OUT
    with live.LOCK.open('a+') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB);lock.seek(0);assert not lock.read().strip(),'Another lane owns chain.lock'
        lock.seek(0);lock.truncate();lock.write('TEE\n');lock.flush()
        try:
            start=d/'before.json';s=export(start);assert item(s['ledger']['nav_assets'],'asset_id',B24)['finalized_epoch']==2
            ops=qualify(start,d);last,receipts=live.run_operations(ops,start,d/'live')
            packet=ops[-1]['operation']['packet_hash'];archive.receipt(d/'archive',packet)
            save(d/'result.json',dict(passed=True,packet_hash=packet,amount_atoms=1000000,receipts=receipts,final_snapshot=str(last),real_archive=True,mainnet_mint_executed=False))
        finally:
            lock.seek(0)
            if lock.read().strip()=='TEE':lock.seek(0);lock.truncate();lock.flush()

if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('action',choices=['copy-only','execute']);a=ap.parse_args();os.umask(0o077)
    if a.action=='execute':execute()
    else:
        d=OUT/'export-preflight';d.mkdir(mode=0o700,exist_ok=False);qualify(OUT/'before-state.json',d)
