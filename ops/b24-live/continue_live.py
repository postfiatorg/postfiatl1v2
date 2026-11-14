#!/usr/bin/env python3
"""Complete the qualified Phase 5c sequence, with explicitly authorized U4 repair."""
import argparse,fcntl,time
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime,timezone
from sign import *
import live,repair_u4
from snapshot import export
OUT=COORD/'evidence/tee/phase5c'
OLD=COORD/'evidence/tee/phase5'
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--execute',action='store_true',required=True);ap.add_argument('--resume-after-create',action='store_true');args=ap.parse_args()
    gate=load(OUT/'continue-canary/result.json');plan=OUT/'continue-canary/operations.json'
    assert gate['passed'] and gate['live_transactions']==0 and gate['release4_freshness_accepted'] and gate['same_burn_signature']
    assert hashlib.sha256(plan.read_bytes()).hexdigest()==gate['operations_sha256'];ops=load(plan);assert len(ops)==14
    assert load(OUT/'initial-repair-result.json')['passed'] and load(OLD/'real-proof/qualification.json')['passed']
    assert load(OLD/'capture/final-inventory.json')['noOwnedBillableComputeRemaining']
    assert not (COORD/'status/b24-cutover-done').exists()
    sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'tools/b23-nav-canary'));import submit_live
    with ThreadPoolExecutor(max_workers=6) as pool:versions=dict(pool.map(submit_live.runtime,submit_live.pftl.PEERS))
    save(OUT/'runtime-before.json',versions);assert all(v['matches'] for v in versions.values())
    live.OUT=OUT;prior=[];parent=OUT/'before-state.json';output=OUT/'live';base=OUT/'live-before.json'
    if args.resume_after_create:
        prior=load(OUT/'live/receipts-index.json');assert len(prior)==2 and [r['height'] for r in prior]==[1318,1319]
        assert all(r['accepted'] and r['six_validators'] and r['full_ledger_match'] for r in prior)
        assert not (OUT/'live/03-profile/transaction-signed.json').exists(),'Profile was already signed; cannot use this preparation-only resume'
        for n,row in enumerate(prior):
            assert row['label']==f'{n+1:02d}-'+ops[n]['label']
        parent=OUT/'live/02-create/actual-state.json';output=OUT/'live-resume-after-create';base=OUT/'resume-before.json';ops=ops[2:]
    with live.LOCK.open('a+') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX);lock.seek(0);assert not lock.read().strip(),'Another lane holds chain.lock'
        lock.seek(0);lock.truncate();lock.write('TEE\n');lock.flush()
        try:
            repair_u4.enable_watchdogs();actual=export(base);expected=load(parent)
            assert actual['tip']==expected['tip'] and actual['ledger']==expected['ledger'],'Parent changed; qualify again'
            for row in prior:
                six=live.read_six(row['tx_id']);assert len(live.points(six))==1
                assert all(any(x['tx_id']==row['tx_id'] and x['accepted'] for x in live.receipt_rows(r['receipts'])) for r in six)
                save(OUT/('resume-confirm-'+str(row['height'])+'.json'),six)
            current,receipts=live.run_operations(ops,base,output,repair_callback=repair_u4.callback);receipts=prior+receipts
            for attempt in range(10):
                repair_u4.callback('final-drain-'+str(attempt),current)
                if not any(repair_u4.fleet_scan('final-check-'+str(attempt))):break
                time.sleep(3)
            else:raise RuntimeError('Outboxes did not drain after final transaction')
            repair_u4.enable_watchdogs();six=live.read_six();save(OUT/'final-six.json',six);s=load(current)
            assert len(live.points(six))==1 and six[0]['server']['ledger']['state_root']==s['tip']['state_root'] and all(x['server']['mempool']['pending']==0 for x in six)
            n=item(s['ledger']['nav_assets'],'asset_id',B24);r=item(s['ledger']['pftl_uniswap_routes'],'route_id',ROUTE)
            old=item(s['ledger']['nav_assets'],'asset_id',B23);oldr=item(s['ledger']['pftl_uniswap_routes'],'route_id',OLD_ROUTE)
            assert n['finalized_epoch']==1 and not n['halted'] and r.get('live_value_enabled') and not r['paused'] and invariant(r)
            assert r['authorized_valid_supply_atoms']==420322380000 and r['settlement_reserve_atoms']==20000000 and r['v2']['non_nav_spread_atoms']==100000
            assert not r['v2'].get('active_reservations') and not r['v2'].get('export_entitlements')
            assert old['halted'] and oldr['paused'] and not oldr.get('live_value_enabled') and oldr['settlement_reserve_atoms']==1002487
            assert balance(s['ledger'],FOUNDER,B23)==1000000 and all(balance(s['ledger'],a,B23)==0 for a in (ALICE,BOB,TEST))
            offers=[o for o in s['ledger']['offers'] if o['state']=='open' and o['owner']==ALICE and o['taker_gets_asset_id']==B24]
            assert len(offers)==1 and offers[0]['taker_gets_amount_remaining']==100000000
            result=dict(passed=True,completed_at=datetime.now(timezone.utc).isoformat(),phase='5c',height=s['tip']['height'],state_root=s['tip']['state_root'],asset_id=B24,profile=n['proof_profile'],nav_per_unit=n['nav_per_unit'],opening_supply_atoms=420302380000,supply_atoms=r['authorized_valid_supply_atoms'],reserve_atoms=r['settlement_reserve_atoms'],spread_atoms=r['v2']['non_nav_spread_atoms'],offer=offers[0],founder_B24_held_atoms=1000000,founder_native_delivery_pending_trustline=True,founder_B23_reserve_atoms=1002487,receipts=receipts,final_snapshot=str(current),all_six_outboxes_empty=True,all_six_watchdogs_running=True)
            save(OUT/'cutover-result.json',result);save(COORD/'status/b24-cutover-done',result);print(json.dumps(result),flush=True)
        except BaseException as error:
            save(OUT/'cutover-failure.json',dict(error_type=type(error).__name__,message=str(error)[:1500],automatic_retry=False));raise
        finally:
            lock.seek(0)
            if lock.read().strip()=='TEE':lock.seek(0);lock.truncate();lock.flush()
if __name__=='__main__':main()
