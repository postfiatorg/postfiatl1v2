#!/usr/bin/env python3
"""B24 18:15 UTC update: $9/run, $250/month, and remaining $100 cumulative lane cap.

No retry/backfill. --check is local only; --execute uses real time, the pinned
collector, shared GPU lock, strict copy verification and three live operations.
"""
import argparse,copy,fcntl,importlib.util,os,signal
from datetime import datetime,timezone
from decimal import Decimal,ROUND_CEILING
from sign import *
from snapshot import export
from live import run_operations,LOCK
ROOT=Path(__file__).resolve().parents[2]
NAV=Path('/home/postfiat/repos/navstrategies-b23-tee-nav')
SEED=COORD/'evidence/tee/phase5/capture'
BASE=COORD/'evidence/tee/phase5'
DAILY=COORD/'evidence/tee/b24-daily'
TOOLS=ROOT/'tools/b23-nav-canary'
spec=importlib.util.spec_from_file_location('b23_daily_helpers',ROOT/'ops/b23-tee-nav/daily.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
def reserve_lane(run_id,cost,month):
    with (BASE/'aws-budget.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        ledger=load(BASE/'aws-budget.json');fixed=sum((Decimal(str(x['reserved_upper_usd'])) for x in ledger['prior_phase2']['runs']),Decimal(0)).quantize(Decimal('.01'),rounding=ROUND_CEILING)+sum((Decimal(x['reserved_upper_usd']) for x in ledger['runs']),Decimal(0))
        used=Decimal(0)
        for path in (COORD/'evidence/tee/daily/monthly-budget.json',DAILY/'monthly-budget.json'):
            if path.exists():used+=sum((Decimal(x['reserved_usd']) for rows in load(path)['months'].values() for x in rows),Decimal(0))
        reservation=Decimal(str(cost)).quantize(Decimal('.01'),rounding=ROUND_CEILING)
        if fixed+used+reservation>Decimal('100'):raise ValueError('Cumulative lane cap $100 exhausted; no new AWS run')
        b=helpers.reserve_budget(DAILY/'monthly-budget.json',month,run_id,reservation)
        b['cumulative_lane_reserved_usd']=str(fixed+used+reservation);b['lane_cap_usd']='100.00';return b
def prepare(state_path,capture,proof,out):
    out.mkdir(mode=0o700);s=load(state_path);m=load(capture/'manifest.json');n=item(s['ledger']['nav_assets'],'asset_id',B24);r=item(s['ledger']['pftl_uniswap_routes'],'route_id',ROUTE)
    assert n['proof_profile']==m['proof_profile_id'] and n['finalized_epoch']+1==m['observation_epoch'] and not n['halted'] and r.get('live_value_enabled')
    context=out/'packet-context.json';subprocess.run([str(CANARY),'packet-context','--snapshot',str(state_path),'--public-values',str(proof/'public-values.bin'),'--output',str(context)],check=True)
    x=load(context);values=json.loads(subprocess.check_output([str(NATIVE),'decode','--public-values',str(proof/'public-values.bin')]))
    packet=sha48('b24.daily.packet',dict(public=hashlib.sha256((proof/'public-values.bin').read_bytes()).hexdigest(),context=x))
    submit=dict(operation='nav_reserve_submit',issuer=ISSUER,submitter=ISSUER,asset_id=B24,epoch=m['observation_epoch'],nav_per_unit=x['nav_per_unit'],circulating_supply=x['supply_atoms'],verified_net_assets=x['verified_net_assets'],proof_profile=n['proof_profile'],source_root=x['source_root'],attestor_root=values['valuation_trust_root'],reserve_packet_hash=packet,reserve_accounts=[],sp1_proof_bytes=list((proof/'proof-calldata.bin').read_bytes()),sp1_public_values=list((proof/'public-values.bin').read_bytes()))
    p=copy.deepcopy(r['v2']['primary_market_policy']);p.update(policy_epoch=p['policy_epoch']+1,pricing_nav_epoch=m['observation_epoch'],pricing_reserve_packet_hash=packet);p['policy_hash']=policy_hash(p)
    ops=[dict(label='daily-submit',source=ISSUER,kind='asset',operation=submit),dict(label='daily-finalize',source=ISSUER,kind='asset',operation=dict(operation='nav_epoch_finalize',issuer=ISSUER,asset_id=B24,epoch=m['observation_epoch'],reserve_packet_hash=packet)),dict(label='daily-route-pricing',source=ISSUER,kind='asset',operation=dict(operation='pftl_uniswap_route_epoch_advance',operator=ISSUER,route_id=ROUTE,prior_route_epoch=r['v2']['route_epoch'],next_route_epoch=r['v2']['route_epoch']+1,next_route_config_digest=sha48('b24.daily.route',p),live_value_enabled=True,next_primary_market_policy=p))]
    c=Copy(state_path,out/'copy');bad=copy.deepcopy(submit);bad['sp1_proof_bytes'][-1]^=1;c.apply('reject-bad-proof',bad,accepted=False,code='sp1_proof_invalid')
    for row in ops:c.apply(row['label'],row['operation'])
    after=c.ledger();route=item(after['pftl_uniswap_routes'],'route_id',ROUTE);nav=item(after['nav_assets'],'asset_id',B24);assert invariant(route) and nav['finalized_epoch']==m['observation_epoch'] and nav['circulating_supply']==n['circulating_supply']
    save(out/'operations.json',ops);save(out/'result.json',dict(passed=True,epoch=m['observation_epoch'],nav_per_unit=x['nav_per_unit'],attested_value=x['attested_value'],consensus_overlay=x['overlay_value'],no_live_transactions=True));return ops
def execute(now):
    DAILY.mkdir(mode=0o700,parents=True,exist_ok=True)
    with (DAILY/'run.lock').open('a') as running:
        fcntl.flock(running,fcntl.LOCK_EX|fcntl.LOCK_NB);d=DAILY/now.strftime('%Y%m%d');d.mkdir(mode=0o700,exist_ok=False)
        try:
            before=d/'before-capture.json';state=export(before);seed=load(SEED/'manifest.json');manifest=helpers.daily_manifest(seed,now.date(),state['tip']['height']);nav=item(state['ledger']['nav_assets'],'asset_id',B24)
            assert nav['proof_profile']==seed['proof_profile_id'] and nav['finalized_epoch']+1==manifest['observation_epoch'] and not nav['halted']
            runner=helpers.runner_module();plan=load(BASE/'capture-plan.json');assert plan['sourceRevision']==helpers.COLLECTOR_REVISION
            plan['runId']='b23-tee-b24-daily-'+now.strftime('%Y%m%d');plan=runner.reviewed_plan(plan);runner.check_inputs(plan,manifest,execution=True)
            original=runner.bind_measured_profile
            def bind(*args,**kwargs):
                actual,policy=original(*args,**kwargs)
                if actual['proof_profile_id']!=seed['proof_profile_id']:raise ValueError('Measured daily profile differs; no secret release')
                return actual,policy
            runner.bind_measured_profile=bind;aws=runner.infra.session(runner.AWS_CREDENTIALS);assert aws.client('sts').get_caller_identity()['Account']==plan['accountId']
            ec2=runner.infra.client(aws,'ec2',plan);rows=ec2.describe_instances(Filters=[{'Name':'tag:Project','Values':['yolo-nitro-qualification']},{'Name':'instance-state-name','Values':['pending','running','stopping','stopped','shutting-down']}]);assert not any(r['Instances'] for r in rows['Reservations'])
            cost=runner.cost_preflight(aws,plan);budget=reserve_lane(plan['runId'],cost['resource_time_bound_usd'],now.strftime('%Y-%m'));save(d/'budget.json',budget);helpers.oauth_baseline(d/'oauth-baseline.json')
            capture=d/'capture';runner.execute(plan,manifest,capture,aws,profile_operation=load(BASE/'profile-template.json'),profile_cli=NATIVE);assert load(capture/'final-inventory.json')['noOwnedBillableComputeRemaining']
            helpers.command([NAV/'.venv/bin/python',NAV/'containers/yolo-nitro/aws/verify_nav_archive.py','--capture',capture,'--recovery-key',helpers.RECOVERY,'--oauth-baseline',d/'oauth-baseline.json','--output',d/'archive-verification.json'])
            helpers.command(['systemctl','--user','disable','--now',plan['cleanupTimer']])
            helpers.command([NAV/'.venv/bin/python',TOOLS/'qualify_reading.py','--capture',capture,'--collector-repo',NAV,'--native-cli',NATIVE,'--root-der',COORD/'evidence/tee/aws-nitro-root.der','--output',d/'negative-reading'])
            helpers.command([sys.executable,TOOLS/'prove_remote.py','--capture',capture,'--elf',helpers.ELF,'--native-cli',NATIVE,'--output',d/'proof'])
            with LOCK.open('a+') as lock:
                deadline=__import__('time').monotonic()+3600
                while True:
                    fcntl.flock(lock,fcntl.LOCK_EX);lock.seek(0);holder=lock.read().strip()
                    if not holder:lock.seek(0);lock.truncate();lock.write('TEE\n');lock.flush();break
                    fcntl.flock(lock,fcntl.LOCK_UN)
                    if __import__('time').monotonic()>deadline:raise TimeoutError('chain lock held for 60 minutes')
                    __import__('time').sleep(5)
                try:
                    sys.path.insert(0,str(TOOLS));import submit_live
                    from concurrent.futures import ThreadPoolExecutor
                    with ThreadPoolExecutor(max_workers=6) as pool:versions=dict(pool.map(submit_live.runtime,submit_live.pftl.PEERS))
                    save(d/'runtime-hashes.json',versions);assert all(v['matches'] for v in versions.values()),'Daily execution requires the qualified release-4 runtimes'
                    current=d/'before-submit.json';export(current);ops=prepare(current,capture,d/'proof',d/'canary');final,receipts=run_operations(ops,current,d/'live')
                    save(d/'complete.json',dict(epoch=manifest['observation_epoch'],six_validator_verified=True,budget=budget,final_snapshot=str(final),receipts=receipts))
                    nav=item(load(final)['ledger']['nav_assets'],'asset_id',B24);status=COORD/'status/tee.md';lines=status.read_text().splitlines();lines[0]=f"B24 PROFILE ON CHAIN: sp1-nav-reserve-v1 {nav['proof_profile']} epoch {nav['finalized_epoch']} finalized; NAV {nav['nav_per_unit']/1e6:.6f}; six validators verified. B23 retired/halted; founder legacy reserve retained.";status.write_text('\n'.join(lines)+'\n')
                finally:
                    lock.seek(0)
                    if lock.read().strip()=='TEE':lock.seek(0);lock.truncate();lock.flush()
        except BaseException as error:save(d/'failure.json',dict(error_type=type(error).__name__,message=str(error)[:500],automatic_retry=False));raise
def main():
    ap=argparse.ArgumentParser(description=__doc__);g=ap.add_mutually_exclusive_group(required=True);g.add_argument('--check',action='store_true');g.add_argument('--execute',action='store_true');a=ap.parse_args();now=datetime.now(timezone.utc)
    if a.check:
        m=helpers.daily_manifest(load(SEED/'manifest.json'),now.date(),1310)
        print(json.dumps(dict(local_check=True,asset_id=m['nav_asset_id'],profile=m['proof_profile_id'],epoch=m['observation_epoch'],cutoff_unix_ms=m['cutoff_unix_ms'],run_cap_usd='9',month_cap_usd='250',cumulative_lane_cap_usd='100',aws_calls=0,chain_writes=0)));return
    os.umask(0o077)
    def stop(signum,frame):raise SystemExit(128+signum)
    signal.signal(signal.SIGTERM,stop);signal.signal(signal.SIGINT,stop);execute(now)
if __name__=='__main__':main()
