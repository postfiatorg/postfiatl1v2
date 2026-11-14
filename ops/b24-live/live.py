#!/usr/bin/env python3
"""Execute only the hashed, fresh-proof-qualified Phase5 plan under chain.lock=TEE.

Each exact signed transaction first executes on a read-only copy. After its sole
submission, require a literal accepted receipt and six-validator convergence,
then compare a fresh full live ledger to the expected copy. Never retry a write
except a node's explicit wrong-proposer redirect with the identical signature.
"""
import argparse,fcntl,time
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime,timezone
from sign import *
from rpc import call,envelope
from snapshot import export
OUT=COORD/'evidence/tee/phase5'
LOCK=COORD/'chain.lock'
def require_lock():assert LOCK.read_text().strip()=='TEE','TEE lock lost'
def read_six(tx=None):
    def one(i):
        r=dict(server=call('server_info',node=i),status=call('status',node=i))
        if tx:r['receipts']=call('receipts',{'tx_id':tx,'limit':10},i)
        return r
    with ThreadPoolExecutor(max_workers=6) as pool:return list(pool.map(one,range(6)))
def points(rows):return {(r['server']['ledger']['height'],r['server']['ledger']['state_root'],r['server']['ledger']['hash']) for r in rows}
def receipt_rows(value):
    if isinstance(value,dict):
        if 'accepted' in value and 'tx_id' in value:yield value
        for v in value.values():yield from receipt_rows(v)
    elif isinstance(value,list):
        for v in value:yield from receipt_rows(v)
def run_operations(operations,start,output):
    output.mkdir(mode=0o700,parents=True,exist_ok=False);current=start;summary=[]
    for n,row in enumerate(operations,1):
        require_lock();label=f'{n:02d}-'+row['label'];d=output/label;d.mkdir(mode=0o700)
        state=load(current);before=read_six();save(d/'before-six.json',before)
        assert len(points(before))==1 and before[0]['server']['ledger']['height']==state['tip']['height'] and before[0]['server']['ledger']['state_root']==state['tip']['state_root']
        assert all(r['server']['mempool']['pending']==0 for r in before),'Pending transactions before cutover step'
        if row['label']=='create':
            old=item(state['ledger']['nav_assets'],'asset_id',B23);route=item(state['ledger']['pftl_uniswap_routes'],'route_id',OLD_ROUTE)
            assert old['halted'] and route['paused'] and not route.get('live_value_enabled',False)
            assert all(balance(state['ledger'],a,B23)==0 for a in (ALICE,BOB,TEST)) and balance(state['ledger'],FOUNDER,B23)==1_000_000 and route['settlement_reserve_atoms']==1_002_487
            subprocess.run(['systemctl','--user','disable','--now','b23-tee-nav-daily.timer'],check=True)
            subprocess.run(['systemctl','--user','stop','b23-tee-nav-daily.service'],check=True)
            save(OUT/'old-daily-disabled.json',dict(at=datetime.now(timezone.utc).isoformat(),retirement_verified=True))
        source=row['source'];assert source in (ISSUER,ALICE,BOB,TEST);kind=row['kind'];op=row['operation'];save(d/'operation.json',op)
        quote=call(kind+'_fee_quote',{'source':source,'operation_json':json.dumps(op,separators=(',',':'))});save(d/'quote.json',quote)
        assert quote['source']==source and quote['chain_id']==state['tip']['chain_id'] and quote['genesis_hash']==state['tip']['genesis_hash']
        assert quote['sequence']==item(state['ledger']['accounts'],'address',source)['sequence']+1
        assert all(quote['operation'].get(k)==v for k,v in op.items()),'quote operation differs'
        f=dict(chain_id=quote['chain_id'],genesis_hash=quote['genesis_hash'],protocol_version=quote['protocol_version'],source=source,fee=quote['minimum_fee'],sequence=quote['sequence'],operation=quote['operation'])
        signed=sign_fields(f,kind,d/'transaction');height=state['tip']['height']+1
        save(d/'copy-steps.json',[dict(label=label,height=height,transaction=signed,kind=kind,expect_accepted=True)])
        projected=d/'expected-state.json';subprocess.run([str(CANARY),'apply','--snapshot',str(current),'--steps',str(d/'copy-steps.json'),'--output',str(projected)],check=True,capture_output=True)
        expected=load(projected);expected_receipt=expected['canary_receipts'][0]['receipt'];assert expected_receipt['accepted']
        require_lock();params={'signed_'+kind+'_transaction_json':json.dumps(signed,separators=(',',':')),'proxy_required_current_height':state['tip']['height'],'proxy_required_parent_hash':state['tip']['block_hash'],'proxy_required_state_root':state['tip']['state_root']}
        method='mempool_submit_signed_'+kind+'_transaction_finality';result=None
        for i in range(6):
            response=envelope(method,params,i,150);save(d/f'submit-validator-{i}.json',response)
            if response.get('ok'):result=response['result'];break
            if (response.get('error') or {}).get('code')!='rpc_finality_wrong_proposer':raise RuntimeError('Unexpected submission response; no retry')
        assert result is not None,'No designated proposer accepted';save(d/'finality.json',result)
        final=result.get('finality') or {};receipt=final.get('receipt') or {};assert final.get('confirmed') is True and receipt.get('accepted') is True and receipt.get('tx_id')==expected_receipt['tx_id']
        assert result.get('tx_id')==receipt['tx_id']
        assert receipt['code']==expected_receipt['code'] and final['block']['header']['height']==height
        deadline=time.monotonic()+180
        while True:
            after=read_six(receipt['tx_id'])
            if len(points(after))==1 and after[0]['server']['ledger']['height']==height:break
            assert time.monotonic()<deadline,'Fleet convergence timeout';time.sleep(2)
        save(d/'after-six.json',after)
        for r in after:
            matching=[x for x in receipt_rows(r['receipts']) if x['tx_id']==receipt['tx_id']];assert matching and all(x['accepted'] and x['code']==expected_receipt['code'] for x in matching)
        actual_path=d/'actual-state.json';actual=export(actual_path)
        same=actual['ledger']==expected['ledger'];save(d/'verification.json',dict(passed=same,six_validator_receipts=True,height=height,tx_id=receipt['tx_id'],accepted=True,full_ledger_matches_exact_signed_canary=same))
        assert same,'Unexpected full-ledger delta; stopped before next write'
        current=actual_path;summary.append(dict(label=label,height=height,tx_id=receipt['tx_id'],accepted=True,receipt_code=receipt['code'],six_validators=True,full_ledger_match=True));save(output/'receipts-index.json',summary)
        print(json.dumps(summary[-1]),flush=True)
    return current,summary
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--execute',action='store_true',required=True);ap.add_argument('--canary',type=Path,required=True);a=ap.parse_args()
    gate=load(a.canary/'result.json');plan=a.canary/'live-operations.json';assert gate['passed'] and gate['live_transactions']==0 and gate['fresh_capture']==str(OUT/'capture') and gate['proof']==str(OUT/'real-proof')
    assert gate['live_operations_sha256']==hashlib.sha256(plan.read_bytes()).hexdigest();operations=load(plan);assert len(operations)==18
    assert load(OUT/'real-proof/qualification.json')['passed'] and load(OUT/'capture/final-inventory.json')['noOwnedBillableComputeRemaining']
    assert subprocess.run(['systemctl','--user','is-active','b23-tee-nav-daily.service'],capture_output=True,text=True).stdout.strip()=='inactive'
    sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'tools/b23-nav-canary'))
    import submit_live
    with ThreadPoolExecutor(max_workers=6) as pool:versions=dict(pool.map(submit_live.runtime,submit_live.pftl.PEERS))
    save(OUT/'runtime-hashes.json',versions);assert all(v['matches'] for v in versions.values())
    with LOCK.open('a+') as lock:
        deadline=time.monotonic()+3600
        while True:
            fcntl.flock(lock,fcntl.LOCK_EX);lock.seek(0);holder=lock.read().strip()
            if not holder:lock.seek(0);lock.truncate();lock.write('TEE\n');lock.flush();break
            fcntl.flock(lock,fcntl.LOCK_UN);assert time.monotonic()<deadline,'Chain lock wait exceeded';time.sleep(5)
        try:
            require_lock();base=OUT/'live-before.json';actual=export(base);before=load(OUT/'before-state.json');assert actual['tip']==before['tip'] and actual['ledger']==before['ledger'],'Live parent changed; refresh complete canary'
            current,summary=run_operations(operations,base,OUT/'live')
            s=load(current);new=item(s['ledger']['nav_assets'],'asset_id',B24);r=item(s['ledger']['pftl_uniswap_routes'],'route_id',ROUTE)
            assert new['finalized_epoch']==1 and not new['halted'] and r.get('live_value_enabled') and not r['paused'] and invariant(r)
            offers=[o for o in s['ledger']['offers'] if o['state']=='open' and o['owner']==ALICE and o['taker_gets_asset_id']==B24];assert len(offers)==1 and offers[0]['taker_gets_amount_remaining']==100_000_000
            save(OUT/'cutover-result.json',dict(passed=True,completed_at=datetime.now(timezone.utc).isoformat(),height=s['tip']['height'],state_root=s['tip']['state_root'],asset_id=B24,profile=new['proof_profile'],nav_per_unit=new['nav_per_unit'],supply_atoms=r['authorized_valid_supply_atoms'],reserve_atoms=r['settlement_reserve_atoms'],offer=offers[0],founder_B24_held_atoms=1_000_000,founder_native_delivery_pending_trustline=True,founder_B23_reserve_atoms=1_002_487,receipts=summary,final_snapshot=str(current)))
            save(COORD/'status/b24-cutover-done',load(OUT/'cutover-result.json'))
        except BaseException as error:
            save(OUT/'cutover-failure.json',dict(error_type=type(error).__name__,message=str(error)[:1500],automatic_retry=False));raise
        finally:
            lock.seek(0);holder=lock.read().strip()
            if holder=='TEE':lock.seek(0);lock.truncate();lock.flush()
if __name__=='__main__':main()
