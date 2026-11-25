#!/usr/bin/env python3
"""B24 scheduled or --now update: $9/run, $250/month, $250 cumulative lane cap.

No retry/backfill. --check is local only; --execute uses real time, the pinned
collector, shared GPU lock, strict copy verification and three live operations.
Every failed run writes failure.json (run directory, else reading_alert.STATE),
calls the optional B24_READING_NOTIFY hook once, and a reading counts only once
its epoch is read back finalized from the chain.
"""
import argparse,copy,fcntl,importlib.util,os,signal,time
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime,timezone
from decimal import Decimal,ROUND_CEILING
from sign import *
from snapshot import export
from live import run_operations,LOCK,read_six,points,receipt_rows
from reading_alert import record_failure
ROOT=Path(__file__).resolve().parents[2]
NAV=Path('/home/postfiat/repos/navstrategies-now')
DEPLOYMENT=COORD/'evidence/now/deployment'
SEED=DEPLOYMENT/'capture'
BASE=COORD/'evidence/tee/phase5'
DAILY=COORD/'evidence/tee/b24-daily'
TOOLS=ROOT/'tools/b23-nav-canary'
sys.path[:0] = [str(TOOLS), str(ROOT / '.scratch-deposit')]
from pftl import PEERS
from runtime_hashes import runtime
STATUS=COORD/'status/now.md'
LOCK_WAIT=3600;LOCK_POLL=5
LANDING_WAIT=600;LANDING_POLL=30
spec=importlib.util.spec_from_file_location('b23_daily_helpers',ROOT/'ops/b23-tee-nav/daily.py');helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
helpers.NAV=NAV
NATIVE=DEPLOYMENT/'postfiat-b24-nav'
GPU_NATIVE=DEPLOYMENT/'postfiat-b24-nav-cuda'
helpers.NATIVE=NATIVE
helpers.ELF=DEPLOYMENT/'b24-nav.elf'

# b24-release5-20261117, ac91a1f42. Both processes on every validator must match.
DAILY_NODE_SHA = 'd46376ff131e941cd271632d79eb5cfe4a22269d31588bd24caddda962d3e39e'  # sol-exit-r1 (71dbc9b3, from main incl. B24 line)

def require_runtime():
    paths=[SEED/'manifest.json',SEED/'profile.json',DEPLOYMENT/'profile-template.json',DEPLOYMENT/'capture-plan.json',NATIVE,GPU_NATIVE,helpers.ELF]
    missing=[str(p) for p in paths if not p.is_file()]
    if missing:raise ValueError('On-demand deployment is not qualified; missing: '+', '.join(missing))

def previous_cutoff(state):
    return helpers.finalized_cutoff(state,B24,[COORD/'evidence/tee/phase5/capture',DAILY,COORD/'evidence/now'])

def next_manifest(seed,now,state,on_demand=False):
    nav=item(state['ledger']['nav_assets'],'asset_id',B24)
    previous=previous_cutoff(state)
    if on_demand:
        return helpers.reading_manifest(seed,int(now.timestamp()*1000)+120000,state['tip']['height'],nav['finalized_epoch'],previous)
    return helpers.daily_manifest(seed,now.date(),state['tip']['height'],nav['finalized_epoch'],previous)

def validate_next_reading(state,manifest):
    nav=item(state['ledger']['nav_assets'],'asset_id',B24)
    if manifest['observation_epoch']!=nav['finalized_epoch']+1:raise ValueError('epoch reuse or skipped finalized epoch')
    if manifest['cutoff_unix_ms']<=previous_cutoff(state):raise ValueError('cutoff replay or backdated reading')
def wait_lock(lock,deadline,name):
    """Take an OS lock, giving up at the deadline instead of blocking forever."""
    while True:
        try:fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB);return
        except BlockingIOError:
            if time.monotonic()>=deadline:raise TimeoutError(name+' held for 60 minutes') from None
            time.sleep(LOCK_POLL)
def reserve_lane(run_id,cost,month):
    with (BASE/'aws-budget.lock').open('a') as lock:
        wait_lock(lock,time.monotonic()+LOCK_WAIT,'budget lock')
        ledger=load(BASE/'aws-budget.json');fixed=sum((Decimal(str(x['reserved_upper_usd'])) for x in ledger['prior_phase2']['runs']),Decimal(0)).quantize(Decimal('.01'),rounding=ROUND_CEILING)+sum((Decimal(x['reserved_upper_usd']) for x in ledger['runs']),Decimal(0))
        used=Decimal(0)
        for path in (COORD/'evidence/tee/daily/monthly-budget.json',DAILY/'monthly-budget.json'):
            if path.exists():used+=sum((Decimal(x['reserved_usd']) for rows in load(path)['months'].values() for x in rows),Decimal(0))
        reservation=Decimal(str(cost)).quantize(Decimal('.01'),rounding=ROUND_CEILING)
        if fixed+used+reservation>Decimal('250'):raise ValueError('Cumulative lane cap $250 exhausted; no new AWS run')
        b=helpers.reserve_budget(DAILY/'monthly-budget.json',month,run_id,reservation)
        b['cumulative_lane_reserved_usd']=str(fixed+used+reservation);b['lane_cap_usd']='250.00';return b
def prepare(state_path,capture,proof,out):
    out.mkdir(mode=0o700);s=load(state_path);m=load(capture/'manifest.json');n=item(s['ledger']['nav_assets'],'asset_id',B24);r=item(s['ledger']['pftl_uniswap_routes'],'route_id',ROUTE)
    assert n['proof_profile']==m['proof_profile_id'] and n['finalized_epoch']+1==m['observation_epoch'] and not n['halted'] and r.get('live_value_enabled')
    validate_next_reading(s,m)
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
class Progress:
    """Where a run is, for its failure record."""
    def __init__(self):self.stage='start';self.directory=None;self.mode='daily'
def run(now,on_demand=False):
    """Every failed run leaves a record and calls the notification hook once."""
    progress=Progress();progress.mode='now' if on_demand else 'daily'
    try:execute(now,on_demand,progress)
    except BaseException as error:
        if not (isinstance(error,SystemExit) and error.code in (0,None)):record_failure(progress.stage,error,progress.directory,progress.mode)
        raise
def execute(now,on_demand=False,progress=None):
    p=progress or Progress()
    p.stage='inputs';require_runtime()
    p.stage='state-directory';DAILY.mkdir(mode=0o700,parents=True,exist_ok=True)
    p.stage='run-lock'
    with (DAILY/'run.lock').open('a') as running:
        fcntl.flock(running,fcntl.LOCK_EX|fcntl.LOCK_NB)
        if on_demand:
            p.stage='scheduled-service-check';active=subprocess.check_output(['systemctl','--user','show','b24-tee-nav-daily.service','--property=ActiveState','--value'],text=True).strip()
            if active in ('active','activating','deactivating'):raise ValueError('Scheduled B24 reading is active; do not run concurrently')
        stamp=now.strftime('%Y%m%dt%H%M%S%f');mode='now' if on_demand else 'daily'
        p.stage='run-directory';d=DAILY/(mode+'-'+stamp);d.mkdir(mode=0o700,exist_ok=False);p.directory=d
        p.stage='pre-capture-runtime-check';runtime_check(d, 'pre-capture-runtimes.json')
        manifest,capture,budget=collect(now,on_demand,d,mode,stamp,p)
        submit(d,capture,manifest,budget,p)
def collect(now,on_demand,d,mode,stamp,p):
    p.stage='chain-read';before=d/'before-capture.json';state=export(before);seed=load(SEED/'manifest.json');manifest=next_manifest(seed,now,state,on_demand);nav=item(state['ledger']['nav_assets'],'asset_id',B24)
    assert nav['proof_profile']==seed['proof_profile_id'] and nav['finalized_epoch']+1==manifest['observation_epoch'] and not nav['halted']
    p.stage='collector-plan';runner=helpers.runner_module();plan=load(DEPLOYMENT/'capture-plan.json')
    plan['runId']='b23-tee-b24-'+mode+'-'+stamp;plan=runner.reviewed_plan(plan);runner.check_inputs(plan,manifest,execution=True)
    original=runner.bind_measured_profile
    def bind(*args,**kwargs):
        actual,policy=original(*args,**kwargs)
        if actual['proof_profile_id']!=seed['proof_profile_id']:raise ValueError('Measured daily profile differs; no secret release')
        return actual,policy
    p.stage='aws-preflight';runner.bind_measured_profile=bind;aws=runner.infra.session(runner.AWS_CREDENTIALS);assert aws.client('sts').get_caller_identity()['Account']==plan['accountId']
    ec2=runner.infra.client(aws,'ec2',plan);rows=ec2.describe_instances(Filters=[{'Name':'tag:Project','Values':['yolo-nitro-qualification']},{'Name':'instance-state-name','Values':['pending','running','stopping','stopped','shutting-down']}]);assert not any(r['Instances'] for r in rows['Reservations'])
    cost=runner.cost_preflight(aws,plan);p.stage='budget';budget=reserve_lane(plan['runId'],cost['resource_time_bound_usd'],now.strftime('%Y-%m'));save(d/'budget.json',budget);p.stage='oauth-baseline';helpers.oauth_baseline(d/'oauth-baseline.json')
    p.stage='capture';capture=d/'capture';runner.execute(plan,manifest,capture,aws,profile_operation=load(DEPLOYMENT/'profile-template.json'),profile_cli=NATIVE,on_demand=on_demand);assert load(capture/'final-inventory.json')['noOwnedBillableComputeRemaining']
    manifest=load(capture/'manifest.json')
    p.stage='archive-verification';helpers.command([sys.executable,NAV/'containers/yolo-nitro/aws/verify_nav_archive.py','--capture',capture,'--recovery-key',helpers.RECOVERY,'--oauth-baseline',d/'oauth-baseline.json','--output',d/'archive-verification.json'])
    p.stage='cleanup-timer';helpers.command(['systemctl','--user','disable','--now',plan['cleanupTimer']])
    p.stage='reading-qualification';helpers.command([sys.executable,TOOLS/'qualify_reading.py','--capture',capture,'--collector-repo',NAV,'--native-cli',NATIVE,'--root-der',COORD/'evidence/tee/aws-nitro-root.der','--output',d/'negative-reading'])
    p.stage='proof';helpers.command([sys.executable,TOOLS/'prove_remote.py','--capture',capture,'--elf',helpers.ELF,'--native-cli',NATIVE,'--prover-cli',GPU_NATIVE,'--output',d/'proof'])
    return manifest,capture,budget
def runtime_check(d, filename='runtime-hashes.json'):
    with ThreadPoolExecutor(max_workers=6) as pool:
        versions=dict(pool.map(lambda peer: runtime(peer, DAILY_NODE_SHA),PEERS))
    save(d/filename,versions)
    if set(versions) != {f'validator-{i}' for i in range(6)} or not all(v['matches'] for v in versions.values()):
        raise RuntimeError('B24 reading requires release 5 RPC and transport on all six validators')
def confirm_landed(d,epoch,packet):
    """Read the chain again until the new epoch is finalized with this packet, within LANDING_WAIT."""
    deadline=time.monotonic()+LANDING_WAIT
    while True:
        try:
            nav=item(export(d/'landed-state.json')['ledger']['nav_assets'],'asset_id',B24)
            if nav['finalized_epoch']==epoch and nav.get('finalized_reserve_packet_hash')==packet:save(d/'landed.json',dict(epoch=epoch,reserve_packet_hash=packet,confirmed_at=datetime.now(timezone.utc).isoformat()));return nav
            last=f"chain shows finalized epoch {nav['finalized_epoch']}"
        except Exception as error:last=f'{type(error).__name__}: {error}'
        if time.monotonic()>=deadline:raise TimeoutError(f'epoch {epoch} not finalized on chain within {LANDING_WAIT} s; last read: {last}'[:500])
        time.sleep(LANDING_POLL)
def submit(d,capture,manifest,budget,p):
    p.stage='chain-lock'
    with LOCK.open('a+') as lock:
        deadline=time.monotonic()+LOCK_WAIT
        while True:
            wait_lock(lock,deadline,'chain lock');lock.seek(0);holder=lock.read().strip()
            if not holder:lock.seek(0);lock.truncate();lock.write('NOW\n');lock.flush();break
            fcntl.flock(lock,fcntl.LOCK_UN)
            if time.monotonic()>=deadline:raise TimeoutError('chain lock held for 60 minutes')
            time.sleep(LOCK_POLL)
        try:
            p.stage='runtime-check';runtime_check(d)
            p.stage='rehearsal';current=d/'before-submit.json';export(current);ops=prepare(current,capture,d/'proof',d/'canary')
            p.stage='submission';final,receipts=run_operations(ops,current,d/'live',lock_owner='NOW')
            p.stage='landing-check';nav=confirm_landed(d,manifest['observation_epoch'],ops[0]['operation']['reserve_packet_hash'])
            save(d/'complete.json',dict(epoch=manifest['observation_epoch'],six_validator_verified=True,landed_on_chain=True,budget=budget,final_snapshot=str(final),receipts=receipts))
            p.stage='status'
            with STATUS.open('a') as status:status.write(f"\nREADING COMPLETE {datetime.now(timezone.utc).isoformat()}: epoch {nav['finalized_epoch']}, NAV {nav['nav_per_unit']/1e6:.6f}; six validators and full ledger verified. Evidence: {d}\n")
        finally:
            lock.seek(0)
            if lock.read().strip()=='NOW':lock.seek(0);lock.truncate();lock.flush()
def continue_after_submit(d,p):
    """The submit landed (accepted on all six) but the run stopped before finalize and route pricing; run only those."""
    p.stage='continue-inputs';capture=d/'capture';manifest=load(capture/'manifest.json');budget=load(d/'budget.json')
    ops=load(d/'canary/operations.json');assert [o['label'] for o in ops]==['daily-submit','daily-finalize','daily-route-pricing']
    txid=load(d/'live/01-daily-submit/expected-state.json')['canary_receipts'][0]['receipt']['tx_id']
    six=read_six(txid);save(d/'continue-submit-six.json',six);assert len(points(six))==1
    assert all(any(x['tx_id']==txid and x['accepted'] for x in receipt_rows(r['receipts'])) for r in six),'submit not accepted on all six'
    p.stage='chain-lock'
    with LOCK.open('a+') as lock:
        wait_lock(lock,time.monotonic()+LOCK_WAIT,'chain lock');lock.seek(0)
        if lock.read().strip():raise RuntimeError('chain lock held')
        lock.seek(0);lock.truncate();lock.write('NOW\n');lock.flush()
        try:
            p.stage='runtime-check';runtime_check(d,'continue-runtimes.json')
            p.stage='continue-submission';current=d/'continue-before.json';export(current)
            final,receipts=run_operations(ops[1:],current,d/'live-continue',lock_owner='NOW')
            p.stage='landing-check';nav=confirm_landed(d,manifest['observation_epoch'],ops[0]['operation']['reserve_packet_hash'])
            save(d/'complete.json',dict(epoch=manifest['observation_epoch'],six_validator_verified=True,landed_on_chain=True,budget=budget,final_snapshot=str(final),submit_tx_id=txid,receipts=receipts))
            with STATUS.open('a') as status:status.write(f"\nREADING COMPLETE {datetime.now(timezone.utc).isoformat()}: epoch {nav['finalized_epoch']}, NAV {nav['nav_per_unit']/1e6:.6f}; six validators and full ledger verified. Evidence: {d}\n")
        finally:
            lock.seek(0)
            if lock.read().strip()=='NOW':lock.seek(0);lock.truncate();lock.flush()
def resume(d,progress=None):
    """Submit an already captured and proven reading; no AWS. Same lock, runtime check, rehearsal and landing check."""
    p=progress or Progress();p.mode='resume';p.directory=d
    p.stage='resume-inputs';capture=d/'capture'
    for path in (capture/'manifest.json',capture/'final-inventory.json',d/'budget.json',d/'archive-verification.json',d/'negative-reading',d/'proof/public-values.bin',d/'proof/proof-calldata.bin'):
        if not path.exists():raise FileNotFoundError(str(path))
    if (d/'complete.json').exists():raise ValueError('reading already landed')
    if (d/'live/01-daily-submit').exists():return continue_after_submit(d,p)
    assert load(capture/'final-inventory.json')['noOwnedBillableComputeRemaining']
    p.stage='pre-submit-runtime-check';runtime_check(d,'resume-runtimes.json')
    if (d/'canary').exists():(d/'canary').rename(d/('canary-failed-'+datetime.now(timezone.utc).strftime('%Y%m%dt%H%M%S')))
    submit(d,capture,load(capture/'manifest.json'),load(d/'budget.json'),p)
def main():
    ap=argparse.ArgumentParser(description=__doc__);g=ap.add_mutually_exclusive_group(required=True);g.add_argument('--check',action='store_true');g.add_argument('--execute',action='store_true');g.add_argument('--now',action='store_true');g.add_argument('--resume',type=Path,help='Run directory with a captured, proven reading');a=ap.parse_args();now=datetime.now(timezone.utc)
    if a.check:
        require_runtime()
        seed=load(SEED/'manifest.json');m=helpers.reading_manifest(seed,max(int(now.timestamp()*1000)+120000,seed['cutoff_unix_ms']+1),seed['observation_height'],seed['observation_epoch'],seed['cutoff_unix_ms'])
        plan=load(DEPLOYMENT/'capture-plan.json');helpers.runner_module().check_inputs(helpers.runner_module().reviewed_plan(plan),m)
        identity=json.loads(subprocess.check_output([str(NATIVE),'identity','--elf',str(helpers.ELF)]));assert identity['program_vkey']==load(SEED/'profile.json')['sp1_program_vkey']
        print(json.dumps(dict(local_check=True,asset_id=m['nav_asset_id'],profile=m['proof_profile_id'],epoch=m['observation_epoch'],cutoff_unix_ms=m['cutoff_unix_ms'],run_cap_usd='9',month_cap_usd='250',cumulative_lane_cap_usd='250',aws_calls=0,chain_writes=0)));return
    os.umask(0o077)
    def stop(signum,frame):raise SystemExit(128+signum)
    signal.signal(signal.SIGTERM,stop);signal.signal(signal.SIGINT,stop)
    if a.resume:
        progress=Progress()
        try:resume(a.resume.resolve(),progress)
        except BaseException as error:
            if not (isinstance(error,SystemExit) and error.code in (0,None)):record_failure(progress.stage,error,progress.directory,progress.mode)
            raise
        return
    run(now,a.now)
if __name__=='__main__':main()
