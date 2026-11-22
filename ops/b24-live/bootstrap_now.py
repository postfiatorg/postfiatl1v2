#!/usr/bin/env python3
"""First reading on the on-demand B24 proof setting, then switch B24 to it.

One AWS run measures the new enclave image, takes a real Schwab reading and proves it. Before any
live write, the whole sequence (register setting, switch B24, submit, finalize, route pricing) is
applied to a copy of the current ledger. Only if that passes are the same five transactions sent live.
If the live reading fails after the switch, B24 is put back on its previous setting.
Usage: bootstrap_now.py --execute
"""
import sys
from daily import *
OLD_SEED=COORD/'evidence/tee/phase5/capture'
def take_lock(lock):
    deadline=__import__('time').monotonic()+1800
    while True:
        fcntl.flock(lock,fcntl.LOCK_EX);lock.seek(0);holder=lock.read().strip()
        if not holder:lock.seek(0);lock.truncate();lock.write('NOW\n');lock.flush();return
        fcntl.flock(lock,fcntl.LOCK_UN)
        if __import__('time').monotonic()>deadline:raise TimeoutError('chain lock held for 30 minutes')
        __import__('time').sleep(5)
def main():
    assert sys.argv[1:]==['--execute'];os.umask(0o077);now=datetime.now(timezone.utc)
    for p in (DEPLOYMENT/'profile-template.json',DEPLOYMENT/'capture-plan.json',NATIVE,GPU_NATIVE,helpers.ELF):assert p.is_file(),p
    assert not SEED.exists(),'bootstrap capture already exists'
    active=subprocess.check_output(['systemctl','--user','show','b24-tee-nav-daily.service','--property=ActiveState','--value'],text=True).strip();assert active not in ('active','activating','deactivating')
    d=COORD/'evidence/now'/('bootstrap-'+now.strftime('%Y%m%dt%H%M%S'));d.mkdir(mode=0o700)
    with (DAILY/'run.lock').open('a') as running:
        fcntl.flock(running,fcntl.LOCK_EX|fcntl.LOCK_NB)
        try:
            state=export(d/'before-capture.json');nav=item(state['ledger']['nav_assets'],'asset_id',B24);old_profile=nav['proof_profile'];assert not nav['halted']
            manifest=helpers.reading_manifest(load(OLD_SEED/'manifest.json'),int(now.timestamp()*1000)+1800000,state['tip']['height'],nav['finalized_epoch'],previous_cutoff(state))
            runner=helpers.runner_module();plan=load(DEPLOYMENT/'capture-plan.json');plan['runId']='b23-tee-b24-now-'+now.strftime('%Y%m%dt%H%M%S');plan=runner.reviewed_plan(plan);runner.check_inputs(plan,manifest,execution=True)
            aws=runner.infra.session(runner.AWS_CREDENTIALS);assert aws.client('sts').get_caller_identity()['Account']==plan['accountId']
            ec2=runner.infra.client(aws,'ec2',plan);rows=ec2.describe_instances(Filters=[{'Name':'tag:Project','Values':['yolo-nitro-qualification']},{'Name':'instance-state-name','Values':['pending','running','stopping','stopped','shutting-down']}]);assert not any(r['Instances'] for r in rows['Reservations'])
            cost=runner.cost_preflight(aws,plan);budget=reserve_lane(plan['runId'],cost['resource_time_bound_usd'],now.strftime('%Y-%m'));save(d/'budget.json',budget);helpers.oauth_baseline(d/'oauth-baseline.json')
            capture=SEED;runner.execute(plan,manifest,capture,aws,profile_operation=load(DEPLOYMENT/'profile-template.json'),profile_cli=NATIVE,on_demand=True);assert load(capture/'final-inventory.json')['noOwnedBillableComputeRemaining']
            manifest=load(capture/'manifest.json');profile=load(capture/'profile.json');new_profile=manifest['proof_profile_id'];assert new_profile==profile['profile_id']!=old_profile
            print(json.dumps(dict(stage='captured',new_profile=new_profile,epoch=manifest['observation_epoch'])),flush=True)
            helpers.command([sys.executable,NAV/'containers/yolo-nitro/aws/verify_nav_archive.py','--capture',capture,'--recovery-key',helpers.RECOVERY,'--oauth-baseline',d/'oauth-baseline.json','--output',d/'archive-verification.json'])
            helpers.command(['systemctl','--user','disable','--now',plan['cleanupTimer']])
            helpers.command([sys.executable,TOOLS/'qualify_reading.py','--capture',capture,'--collector-repo',NAV,'--native-cli',NATIVE,'--root-der',COORD/'evidence/tee/aws-nitro-root.der','--output',d/'negative-reading'])
            helpers.command([sys.executable,TOOLS/'prove_remote.py','--capture',capture,'--elf',helpers.ELF,'--native-cli',NATIVE,'--prover-cli',GPU_NATIVE,'--output',d/'proof'])
            print(json.dumps(dict(stage='proved')),flush=True)
            register=dict(load(capture/'profile-operation.json'),operation='nav_profile_register')
            rebind=lambda p:dict(operation='nav_asset_register',issuer=ISSUER,asset_id=B24,reserve_operator=nav['reserve_operator'],proof_profile=p,valuation_unit=nav['valuation_unit'],redemption_account=nav['redemption_account'])
            switch=[dict(label='register-on-demand-profile',source=ISSUER,kind='asset',operation=register),dict(label='switch-B24',source=ISSUER,kind='asset',operation=rebind(new_profile))]
            with LOCK.open('a+') as lock:
                take_lock(lock)
                try:
                    sys.path.insert(0,str(TOOLS));import submit_live
                    from concurrent.futures import ThreadPoolExecutor
                    with ThreadPoolExecutor(max_workers=6) as pool:versions=dict(pool.map(submit_live.runtime,submit_live.pftl.PEERS))
                    save(d/'runtime-hashes.json',versions);assert all(v['matches'] for v in versions.values())
                    current=d/'before-switch.json';export(current)
                    # Whole sequence on a copy first. Nothing live has happened yet.
                    c=Copy(current,d/'copy-qualify')
                    for row in switch:c.apply(row['label'],row['operation'])
                    assert item(c.ledger()['nav_assets'],'asset_id',B24)['proof_profile']==new_profile
                    prepare(c.current,capture,d/'proof',d/'copy-reading');print(json.dumps(dict(stage='copy_qualified')),flush=True)
                    switched,r1=run_operations(switch,current,d/'live-switch',lock_owner='NOW')
                    try:
                        ops=prepare(switched,capture,d/'proof',d/'canary');final,r2=run_operations(ops,switched,d/'live',lock_owner='NOW')
                    except BaseException as error:
                        save(d/'reading-failure.json',dict(error=str(error)[:500]))
                        latest=d/'before-rollback.json';s=export(latest)
                        if item(s['ledger']['nav_assets'],'asset_id',B24)['finalized_epoch']==nav['finalized_epoch']:
                            run_operations([dict(label='rollback-B24',source=ISSUER,kind='asset',operation=rebind(old_profile))],latest,d/'live-rollback',lock_owner='NOW');save(d/'rolled-back.json',dict(profile=old_profile))
                        raise
                    n=item(load(final)['ledger']['nav_assets'],'asset_id',B24)
                    save(d/'complete.json',dict(epoch=n['finalized_epoch'],nav_per_unit=n['nav_per_unit'],profile=new_profile,previous_profile=old_profile,budget=budget,receipts=r1+r2,six_validator_verified=True))
                    print(json.dumps(dict(stage='complete',epoch=n['finalized_epoch'],nav_per_unit=n['nav_per_unit'],profile=new_profile)),flush=True)
                finally:
                    lock.seek(0)
                    if lock.read().strip()=='NOW':lock.seek(0);lock.truncate();lock.flush()
        except BaseException as error:save(d/'failure.json',dict(error_type=type(error).__name__,message=str(error)[:800]));raise
if __name__=='__main__':main()
