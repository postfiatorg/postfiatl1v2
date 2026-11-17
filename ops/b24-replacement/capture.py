#!/usr/bin/env python3
"""Bounded Phase-4 real reading; no chain submission code. Explicit --execute required."""
import argparse
from datetime import datetime, timezone
from decimal import Decimal, ROUND_CEILING
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

COORD = Path('/home/postfiat/repos/b23-coord')
NAV = Path('/home/postfiat/repos/navstrategies-b23-tee-nav')
OUT = COORD / 'evidence/tee/phase4'
NATIVE = Path('/mnt/HC_Volume_106212907/sec-fundamentals-proxy/b23-tee-nav-target/release/postfiat-b23-nav')
CAP = Decimal('100.00')

def load(p): return json.loads(p.read_bytes())
def save(p, value):
    temporary = p.with_suffix('.tmp')
    with temporary.open('w') as f:
        json.dump(value, f, indent=2); f.write('\n'); f.flush(); os.fsync(f.fileno())
    temporary.replace(p)

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--execute', action='store_true')
    args = ap.parse_args()
    os.umask(0o077)
    sys.path[:0] = [str(NAV), str(NAV/'containers/yolo-nitro/aws')]
    spec = importlib.util.spec_from_file_location('nav_capture', NAV/'containers/yolo-nitro/aws/nav_reading_run.py')
    runner = importlib.util.module_from_spec(spec); spec.loader.exec_module(runner)
    plan = runner.reviewed_plan(load(OUT/'capture-plan.json'))
    manifest = load(OUT/'capture-manifest.json')
    review = runner.check_inputs(plan, manifest, execution=args.execute)
    save(OUT/'capture-input-review.json', review)
    if not args.execute:
        print(json.dumps({'inputs_valid':True,'aws_calls':0,'chain_writes':0,'lane_cap_usd':str(CAP)})); return
    def stop(signum, frame): raise SystemExit(128+signum)
    signal.signal(signal.SIGTERM, stop); signal.signal(signal.SIGINT, stop)
    # Keep the old ledger intact; carry all of its reservations into this lane ledger.
    with (OUT/'aws-budget.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX|fcntl.LOCK_NB)
        path=OUT/'aws-budget.json'
        ledger=load(path) if path.exists() else {'cap_usd':str(CAP),'prior_phase2':load(COORD/'evidence/tee/phase2/aws-budget.json'),'runs':[]}
        if Decimal(ledger['cap_usd'])!=CAP: raise ValueError('lane cap changed')
        if any(x['run_id']==plan['runId'] for x in ledger['runs']): raise ValueError('attempt already reserved; no automatic retry')
        aws=runner.infra.session(runner.AWS_CREDENTIALS)
        if aws.client('sts').get_caller_identity()['Account']!=plan['accountId']: raise ValueError('AWS account differs')
        cost=runner.cost_preflight(aws,plan)
        reservation=Decimal(str(cost['resource_time_bound_usd'])).quantize(Decimal('.01'),rounding=ROUND_CEILING)
        prior=sum((Decimal(str(r['reserved_upper_usd'])) for r in ledger['prior_phase2']['runs']),Decimal(0)).quantize(Decimal('.01'),rounding=ROUND_CEILING)
        used=sum((Decimal(x['reserved_upper_usd']) for x in ledger['runs']),Decimal(0))
        daily_path=COORD/'evidence/tee/daily/monthly-budget.json'
        daily=sum((Decimal(r['reserved_usd']) for rows in load(daily_path)['months'].values() for r in rows),Decimal(0)) if daily_path.exists() else Decimal(0)
        total=prior+used+daily+reservation
        if reservation>Decimal('9') or total>CAP: raise ValueError('per-attempt or cumulative lane cap exceeded')
        ec2=runner.infra.client(aws,'ec2',plan)
        rows=ec2.describe_instances(Filters=[{'Name':'tag:Project','Values':['yolo-nitro-qualification']},
            {'Name':'instance-state-name','Values':['pending','running','stopping','stopped','shutting-down']}])
        if any(r['Instances'] for r in rows['Reservations']): raise ValueError('another qualification instance exists')
        ledger['runs'].append({'run_id':plan['runId'],'reserved_upper_usd':str(reservation),'reserved_utc':datetime.now(timezone.utc).isoformat(),'pricing':cost})
        ledger['daily_reserved_usd']=str(daily); ledger['cumulative_reserved_upper_usd']=str(total)
        save(path,ledger); save(OUT/'cost-preflight.json',cost)
        # Existing helper exports only a token commitment; secrets remain in their stores.
        spec=importlib.util.spec_from_file_location('daily_helpers',Path(__file__).resolve().parents[1]/'b23-tee-nav/daily.py')
        helpers=importlib.util.module_from_spec(spec);spec.loader.exec_module(helpers)
        helpers.oauth_baseline(OUT/'oauth-baseline.json')
        runner.execute(plan,manifest,OUT/'capture',aws,
            profile_operation=load(OUT/'profile-template.json'),profile_cli=NATIVE)
        assert load(OUT/'capture/final-inventory.json')['noOwnedBillableComputeRemaining']
        subprocess.run([str(NAV/'.venv/bin/python'),str(NAV/'containers/yolo-nitro/aws/verify_nav_archive.py'),
            '--capture',str(OUT/'capture'),'--recovery-key','/home/postfiat/.local/state/b23-tee-nav/recovery-20261113.pem',
            '--oauth-baseline',str(OUT/'oauth-baseline.json'),'--output',str(OUT/'archive-verification.json')],check=True)
        # This guard belongs only to this completed capture. Do not touch B23's daily timer.
        subprocess.run(['systemctl','--user','disable','--now',plan['cleanupTimer']],check=True)
        print(json.dumps({'capture_complete':True,'chain_writes':0,'cumulative_reserved_usd':str(total)}))

if __name__=='__main__': main()
