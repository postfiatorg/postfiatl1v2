#!/usr/bin/env python3
"""Verify the already-accepted daily finalize, then submit only its unsent pricing tail."""
import argparse,fcntl,os,time
from datetime import datetime,timezone
from sign import *
import live
from phase6_reads import read_six
from snapshot import export
OUT=COORD/'evidence/tee/phase6';DAILY=COORD/'evidence/tee/b24-daily/20261114'
def verify_accepted_tail():
    """Finish only evidence checks after the pricing transaction already committed."""
    d=OUT/'daily-recovery';tail_dir=d/'live-tail-resume3';step=tail_dir/'01-daily-route-pricing'
    assert not (DAILY/'complete.json').exists() and not (d/'result.json').exists()
    unit=subprocess.check_output(['systemctl','--user','show','b24-tee-nav-daily.service','--property=MainPID'],text=True);assert 'MainPID=0' in unit
    with live.LOCK.open('a+') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB);lock.seek(0);assert not lock.read().strip();lock.seek(0);lock.truncate();lock.write('TEE\n');lock.flush()
        try:
            actual=load(step/'actual-state.json');expected=load(step/'expected-state.json');assert actual['ledger']==expected['ledger'] and actual['tip']['height']==1355
            old_six=load(step/'after-six.json');assert len(live.points(old_six))==1 and old_six[0]['server']['ledger']['state_root']==actual['tip']['state_root']
            receipts=load(DAILY/'live/receipts-index.json');assert len(receipts)==1
            for label,height,source in [('02-daily-finalize',1353,DAILY/'live/02-daily-finalize'),('01-daily-route-pricing',1355,step)]:
                final=load(source/'finality.json');assert final['finality']['confirmed'] and final['finality']['receipt']['accepted'];tx=final['tx_id'];rows=read_six(tx);save(d/(label+'-recovered-six.json'),rows)
                assert len(live.points(rows))==1
                for row in rows:
                    found=[r for r in live.receipt_rows(row['receipts']) if r['tx_id']==tx];assert found and all(r['accepted'] and r['code']=='accepted' for r in found)
                receipts.append(dict(label=label,height=height,tx_id=tx,accepted=True,receipt_code='accepted',six_validators=True,full_ledger_match=True,confirmation_recovered_without_resubmission=True))
            current=step/'actual-state.json'
            if rows[0]['server']['ledger']['height']!=1355:
                current=d/'after-pricing-current.json';fresh=export(current)
                for key,field,value in [('nav_assets','asset_id',B24),('pftl_uniswap_routes','route_id',ROUTE)]:assert item(fresh['ledger'][key],field,value)==item(actual['ledger'][key],field,value)
            else:assert rows[0]['server']['ledger']['state_root']==actual['tip']['state_root']
            state=load(current);nav=item(state['ledger']['nav_assets'],'asset_id',B24);route=item(state['ledger']['pftl_uniswap_routes'],'route_id',ROUTE);assert nav['finalized_epoch']==route['v2']['primary_market_policy']['pricing_nav_epoch']==2 and invariant(route)
            save(step/'verification.json',dict(passed=True,six_validator_receipts=True,height=1355,tx_id=receipts[-1]['tx_id'],accepted=True,full_ledger_matches_exact_signed_canary=True,verification_only_recovery=True));save(tail_dir/'receipts-index.json',[receipts[-1]])
            result=dict(passed=True,completed_at=datetime.now(timezone.utc).isoformat(),epoch=2,six_validator_verified=True,budget=load(DAILY/'budget.json'),final_snapshot=str(current),receipts=receipts,completion_mode='verified_native_tail_resume_after_service_exit',original_failure_preserved=True,accepted_operations_resubmitted=0,new_AWS_starts=0,new_proofs=0,service_restarted=False,pricing_post_read_recovered_without_resubmission=True);save(d/'result.json',result);save(DAILY/'complete.json',result)
            status=COORD/'status/tee.md';lines=status.read_text().splitlines();lines[0]=f"B24 PROFILE ON CHAIN: sp1-nav-reserve-v1 {nav['proof_profile']} epoch 2 finalized; NAV {nav['nav_per_unit']/1e6:.6f}; six validators verified. Daily native tail recovered after read contention; B23 retired/halted.";status.write_text('\n'.join(lines)+'\n');print(json.dumps(result),flush=True)
        finally:
            lock.seek(0)
            if lock.read().strip()=='TEE':lock.seek(0);lock.truncate();lock.flush()

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--resume-unsent-tail',action='store_true');ap.add_argument('--verify-accepted-tail',action='store_true');args=ap.parse_args()
    if args.verify_accepted_tail:
        assert not args.resume_unsent_tail;verify_accepted_tail();return
    os.umask(0o077);unit=subprocess.check_output(['systemctl','--user','show','b24-tee-nav-daily.service','--property=ActiveState,Result,MainPID'],text=True);assert 'MainPID=0' in unit and 'ActiveState=failed' in unit
    failure=load(DAILY/'failure.json');assert 'storage_writer_busy' in failure['message'] and not (DAILY/'complete.json').exists();assert load(DAILY/'proof/qualification.json')['passed'] and load(DAILY/'canary/result.json')['passed']
    assert not (DAILY/'live/03-daily-route-pricing').exists();d=OUT/'daily-recovery'
    if args.resume_unsent_tail:
        assert d.exists() and not (d/'result.json').exists() and not list(d.glob('live-tail*/**/submit-validator-*.json')) 
        attempt=len(list(d.glob('verified-after-finalize-resume*.json')))+1;tail_dir=d/('live-tail-resume'+str(attempt));assert not tail_dir.exists();save(d/'read-only-resume.json',dict(previous_stopped_before_submission=True,at=datetime.now(timezone.utc).isoformat()))
    else:
        d.mkdir(mode=0o700,exist_ok=False);tail_dir=d/'live-tail';save(d/'original-failure.json',failure);save(d/'service-exited.json',dict(unit=unit,at=datetime.now(timezone.utc).isoformat()))
    live.read_six=read_six
    with live.LOCK.open('a+') as lock:
        deadline=time.monotonic()+3600
        while True:
            fcntl.flock(lock,fcntl.LOCK_EX);lock.seek(0);holder=lock.read().strip()
            if not holder:lock.seek(0);lock.truncate();lock.write('TEE\n');lock.flush();break
            fcntl.flock(lock,fcntl.LOCK_UN);assert time.monotonic()<deadline,'Other lane retained lock';time.sleep(5)
        try:
            current=d/('verified-after-finalize-resume'+str(attempt)+'.json' if args.resume_unsent_tail else 'verified-after-finalize.json');s=export(current);expected=load(DAILY/'live/02-daily-finalize/expected-state.json');assert s['tip']['height']>=1353
            if s['tip']['height']==1353:assert s['ledger']==expected['ledger']
            else:
                assert item(s['ledger']['nav_assets'],'asset_id',B24)==item(expected['ledger']['nav_assets'],'asset_id',B24)
                assert item(s['ledger']['pftl_uniswap_routes'],'route_id',ROUTE)==item(expected['ledger']['pftl_uniswap_routes'],'route_id',ROUTE)
                save(d/'intervening-parent.json',dict(height=s['tip']['height'],daily_finalized_asset_and_route_unchanged=True,full_ledger_canary_uses_current_parent=True))
            receipts=load(DAILY/'live/receipts-index.json');assert len(receipts)==1
            final=load(DAILY/'live/02-daily-finalize/finality.json');assert final['finality']['confirmed'] and final['finality']['receipt']['accepted'];tx=final['tx_id'];rows=read_six(tx);save(d/'finalize-six.json',rows);assert len(live.points(rows))==1 and rows[0]['server']['ledger']['state_root']==s['tip']['state_root']
            for row in rows:
                found=[r for r in live.receipt_rows(row['receipts']) if r['tx_id']==tx];assert found and all(r['accepted'] and r['code']=='accepted' for r in found)
            receipts.append(dict(label='02-daily-finalize',height=1353,tx_id=tx,accepted=True,receipt_code='accepted',six_validators=True,full_ledger_match=True,confirmation_recovered_without_resubmission=True))
            ops=load(DAILY/'canary/operations.json');assert len(ops)==3 and ops[-1]['label']=='daily-route-pricing';last,tail=live.run_operations([ops[-1]],current,tail_dir);receipts+=tail;state=load(last);nav=item(state['ledger']['nav_assets'],'asset_id',B24);route=item(state['ledger']['pftl_uniswap_routes'],'route_id',ROUTE);assert nav['finalized_epoch']==route['v2']['primary_market_policy']['pricing_nav_epoch']==2 and invariant(route)
            result=dict(passed=True,completed_at=datetime.now(timezone.utc).isoformat(),epoch=2,six_validator_verified=True,budget=load(DAILY/'budget.json'),final_snapshot=str(last),receipts=receipts,completion_mode='verified_native_tail_resume_after_service_exit',original_failure_preserved=True,accepted_operations_resubmitted=0,new_AWS_starts=0,new_proofs=0,service_restarted=False);save(d/'result.json',result);save(DAILY/'complete.json',result)
            status=COORD/'status/tee.md';lines=status.read_text().splitlines();lines[0]=f"B24 PROFILE ON CHAIN: sp1-nav-reserve-v1 {nav['proof_profile']} epoch 2 finalized; NAV {nav['nav_per_unit']/1e6:.6f}; six validators verified. Daily native tail recovered after read contention; B23 retired/halted.";status.write_text('\n'.join(lines)+'\n');print(json.dumps(result),flush=True)
        finally:
            lock.seek(0)
            if lock.read().strip()=='TEE':lock.seek(0);lock.truncate();lock.flush()
if __name__=='__main__':main()
