#!/usr/bin/env python3
"""Daily measured account reading and existing-profile NAV update; disabled by default.

--check is local-only. --execute uses real UTC time, one bounded collector,
the existing authorized GPU, an isolated canary, and three NAV transactions.
"""
import argparse
import copy
from datetime import datetime, timezone
from decimal import Decimal, ROUND_CEILING
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import secrets
import shlex
import signal
import subprocess
import sys

REPO=Path(__file__).resolve().parents[2]
COORD=Path('/home/postfiat/repos/b23-coord')
NAV=Path('/home/postfiat/repos/navstrategies-b23-tee-nav')
SEED=COORD/'evidence/tee/phase2/opening3-run'
EVIDENCE=COORD/'evidence/tee/daily'
RECOVERY=Path('/home/postfiat/.local/state/b23-tee-nav/recovery-20261113.pem')
TOOLS=REPO/'tools/b23-nav-canary'
NATIVE=Path('/mnt/HC_Volume_106212907/sec-fundamentals-proxy/b23-tee-nav-target/release/postfiat-b23-nav')
CANARY=Path('/mnt/HC_Volume_106212907/sec-fundamentals-proxy/b23-tee-nav-canary-target/release/postfiat-b23-nav-canary')
ELF=COORD/'evidence/tee/phase2/b23-nav.elf'
NODE=COORD/'artifacts/release4/postfiat-node'
COLLECTOR_REVISION='f6d6be7f14cbb46452cc44d886f68a2e829cbfb0'
RUN_CAP=Decimal('9.00')
MONTH_CAP=Decimal('250.00')

def load(path):return json.loads(path.read_bytes())
def save(path,value):
    temporary=path.with_suffix(path.suffix+'.tmp')
    with temporary.open('w') as stream:
        json.dump(value,stream,indent=2);stream.write('\n');stream.flush();os.fsync(stream.fileno())
    temporary.replace(path)

def daily_manifest(seed,day,height):
    m=copy.deepcopy(seed);schedule=m['schedule']
    cutoff=int(datetime(day.year,day.month,day.day,18,15,tzinfo=timezone.utc).timestamp()*1000)
    delta=cutoff-schedule['first_daily_unix_ms']
    if delta<0 or delta%86400000:raise ValueError('date precedes the registered daily schedule')
    m.update(cutoff_unix_ms=cutoff,observation_epoch=schedule['first_daily_epoch']+delta//86400000,
        observation_height=height,challenge_sha256=secrets.token_hex(32))
    return m

def reserve_budget(path,month,run_id,reservation,run_cap=RUN_CAP,month_cap=MONTH_CAP):
    """Caller holds the exclusive run lock. Reservations never get refunded."""
    reservation=Decimal(str(reservation)).quantize(Decimal('.01'),rounding=ROUND_CEILING)
    if not Decimal(0)<run_cap<=RUN_CAP or not Decimal(0)<month_cap<=MONTH_CAP:
        raise ValueError('caps exceed the committed daily limits')
    if reservation<=0 or reservation>run_cap:raise ValueError('per-run AWS cap exceeded')
    ledger=load(path) if path.exists() else {'months':{}}
    entries=ledger['months'].setdefault(month,[])
    if any(row['run_id']==run_id for rows in ledger['months'].values() for row in rows):
        raise ValueError('this daily run already reserved; no automatic retry')
    total=sum((Decimal(row['reserved_usd']) for row in entries),Decimal(0))+reservation
    if total>month_cap:raise ValueError('monthly AWS cap exceeded')
    entries.append({'run_id':run_id,'reserved_usd':str(reservation),'reserved_at':datetime.now(timezone.utc).isoformat()})
    save(path,ledger)
    return {'run_reserved_usd':str(reservation),'month_reserved_usd':str(total),'run_cap_usd':str(run_cap),'month_cap_usd':str(month_cap)}

def runner_module():
    sys.path.insert(0,str(NAV/'containers/yolo-nitro/aws'));sys.path.insert(0,str(NAV))
    spec=importlib.util.spec_from_file_location('daily_capture_runner',NAV/'containers/yolo-nitro/aws/nav_reading_run.py')
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module

def command(args):subprocess.run([str(x) for x in args],check=True)

def oauth_baseline(path):
    # Only a commitment leaves the existing remote credential store.
    script="import hashlib,json;from pathlib import Path;p=Path('/home/pfrpc/.config/navstrategies/schwab_tokens.json');v=json.loads(p.read_bytes());print(json.dumps({'refresh_token_sha256':hashlib.sha256(v['refresh_token'].encode()).hexdigest()}))"
    value=subprocess.check_output(['ssh','-o','BatchMode=yes','pfrpc@productionrpc','python3 -c '+shlex.quote(script)])
    save(path,json.loads(value))

def execute(now,run_cap,month_cap):
    os.umask(0o077);EVIDENCE.mkdir(parents=True,exist_ok=True,mode=0o700)
    with (EVIDENCE/'run.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        run_id='b23-tee-daily-'+now.strftime('%Y%m%d');directory=EVIDENCE/now.strftime('%Y%m%d')
        directory.mkdir(mode=0o700,exist_ok=False)
        try:
            sys.path.insert(0,str(TOOLS));from live_snapshot import export_snapshot
            export_snapshot(CANARY,directory/'before-capture-state.json')
            state=load(directory/'before-capture-state.json');seed=load(SEED/'manifest.json')
            manifest=daily_manifest(seed,now.date(),state['tip']['height'])
            nav=next(x for x in state['ledger']['nav_assets'] if x['asset_id']==manifest['nav_asset_id'])
            if nav['proof_profile']!=seed['proof_profile_id'] or nav['finalized_epoch']+1!=manifest['observation_epoch']:
                raise ValueError('daily epoch/profile is not the next registered reading; no backfill or rebind')
            runner=runner_module();plan=load(COORD/'evidence/tee/phase2/opening3-plan.json')
            if plan['sourceRevision']!=COLLECTOR_REVISION:raise ValueError('collector source pin changed')
            plan['runId']=run_id;plan=runner.reviewed_plan(plan)
            runner.check_inputs(plan,manifest,execution=True)
            # Require exactly the registered measurements/profile before secret release.
            original_bind=runner.bind_measured_profile
            def pinned_profile(*args,**kwargs):
                actual,policy=original_bind(*args,**kwargs)
                if actual['proof_profile_id']!=seed['proof_profile_id']:raise ValueError('rebuilt collector profile differs from registered profile')
                return actual,policy
            runner.bind_measured_profile=pinned_profile
            aws=runner.infra.session(runner.AWS_CREDENTIALS)
            if aws.client('sts').get_caller_identity()['Account']!=plan['accountId']:raise ValueError('AWS account differs')
            cost=runner.cost_preflight(aws,plan)
            ec2=runner.infra.client(aws,'ec2',plan)
            instances=ec2.describe_instances(Filters=[{'Name':'tag:Project','Values':['yolo-nitro-qualification']},
                {'Name':'instance-state-name','Values':['pending','running','stopping','stopped','shutting-down']}])
            if any(row['Instances'] for row in instances['Reservations']):raise ValueError('another qualification instance exists')
            budget=reserve_budget(EVIDENCE/'monthly-budget.json',now.strftime('%Y-%m'),run_id,cost['resource_time_bound_usd'],run_cap,month_cap)
            save(directory/'budget.json',budget);save(directory/'cost-preflight.json',cost)
            oauth_baseline(directory/'oauth-baseline.json')
            capture=directory/'capture'
            runner.execute(plan,manifest,capture,aws,profile_operation=load(COORD/'evidence/tee/phase2/profile-template.json'),profile_cli=NATIVE)
            if not load(capture/'final-inventory.json')['noOwnedBillableComputeRemaining']:raise ValueError('collector inventory not clear')
            command([NAV/'.venv/bin/python',NAV/'containers/yolo-nitro/aws/verify_nav_archive.py','--capture',capture,
                '--recovery-key',RECOVERY,'--oauth-baseline',directory/'oauth-baseline.json','--output',directory/'archive-verification.json'])
            command([NAV/'.venv/bin/python',TOOLS/'qualify_reading.py','--capture',capture,'--collector-repo',NAV,
                '--native-cli',NATIVE,'--root-der',COORD/'evidence/tee/aws-nitro-root.der','--output',directory/'negative-reading'])
            command([sys.executable,TOOLS/'prove_remote.py','--capture',capture,'--elf',ELF,'--native-cli',NATIVE,'--output',directory/'proof'])
            export_snapshot(CANARY,directory/'before-submit-state.json')
            command([sys.executable,TOOLS/'run_canary.py','--daily','--snapshot',directory/'before-submit-state.json',
                '--capture',capture,'--proof',directory/'proof','--native-cli',NATIVE,'--canary-bin',CANARY,'--node',NODE,'--output',directory/'canary'])
            command([sys.executable,TOOLS/'submit_live.py','--daily','--canary',directory/'canary','--canary-bin',CANARY,'--output',directory/'live','--execute'])
            save(directory/'complete.json',{'epoch':manifest['observation_epoch'],'six_validator_verified':True,'budget':budget})
        except BaseException as error:
            save(directory/'failure.json',{'error_type':type(error).__name__,'automatic_retry':False})
            raise

def main():
    ap=argparse.ArgumentParser(description=__doc__);mode=ap.add_mutually_exclusive_group(required=True)
    mode.add_argument('--check',action='store_true');mode.add_argument('--execute',action='store_true')
    ap.add_argument('--at',help='UTC ISO timestamp for local checks only; prohibited with --execute')
    args=ap.parse_args()
    if args.execute and args.at:raise ValueError('execute always uses real UTC time')
    now=datetime.fromisoformat(args.at.replace('Z','+00:00')) if args.at else datetime.now(timezone.utc)
    if now.tzinfo is None:raise ValueError('UTC-aware time required')
    now=now.astimezone(timezone.utc)
    run_cap=Decimal(os.environ.get('B23_DAILY_RUN_CAP_USD',str(RUN_CAP)))
    month_cap=Decimal(os.environ.get('B23_DAILY_MONTH_CAP_USD',str(MONTH_CAP)))
    if args.check:
        manifest=daily_manifest(load(SEED/'manifest.json'),now.date(),1296)
        print(json.dumps({'mode':'local_check','epoch':manifest['observation_epoch'],'cutoff_unix_ms':manifest['cutoff_unix_ms'],
            'per_run_cap_usd':str(run_cap),'monthly_cap_usd':str(month_cap),'aws_calls':0,'chain_writes':0}));return
    def stop(signum,frame):raise SystemExit(128+signum)
    signal.signal(signal.SIGTERM,stop);signal.signal(signal.SIGINT,stop)
    execute(now,run_cap,month_cap)

if __name__=='__main__':
    try:main()
    except Exception as error:
        print(json.dumps({'daily_failed':True,'error_type':type(error).__name__,'automatic_retry':False}),file=sys.stderr)
        raise SystemExit(1)
