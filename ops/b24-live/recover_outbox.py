#!/usr/bin/env python3
"""Phase 5b: normal redelivery, identical burn replay, then bounded service restarts.

Never edits/deletes outbox records, deploys a binary, or changes consensus.
If normal redelivery still fails after the affected services restart, stop.
"""
import argparse,fcntl,shlex,time
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime,timezone
from live import read_six,points,receipt_rows,require_lock,LOCK
from sign import *
from rpc import call,envelope
from snapshot import export
OUT=COORD/'evidence/tee/phase5b'
OLD=COORD/'evidence/tee/phase5'
HOSTS={4:'95.179.179.206',5:'45.32.110.170'}
RELEASE='/opt/postfiat/releases/b23-release4-20261113/postfiat-node'
SHA='3039c21af67420a9842b5644eb580b2809081a97419e3e8e1f849d49a487cb6f'
def ssh(node,args,label,timeout=300):
    require_lock();r=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=10','root@'+HOSTS[node],shlex.join(args)],capture_output=True,text=True,timeout=timeout)
    (OUT/(label+'.stdout')).write_text(r.stdout);(OUT/(label+'.stderr')).write_text(r.stderr)
    save(OUT/(label+'.result.json'),dict(returncode=r.returncode,at=datetime.now(timezone.utc).isoformat()))
    return r
def six(label):
    rows=read_six();save(OUT/(label+'-six.json'),rows);assert len(points(rows))==1 and all(r['server']['mempool']['pending']==0 for r in rows)
    base=load(OLD/'stopped-live-state.json')['tip'];assert all(r['server']['ledger']['height']==base['height'] and r['server']['ledger']['state_root']==base['state_root'] for r in rows)
    return rows
def pending(label):
    code="import json,pathlib; p=pathlib.Path('/var/lib/postfiat/validator-4/certified-send-outbox'); print(json.dumps([dict(path=str(f),uid=f.stat().st_uid,job=json.loads(f.read_text())) for f in p.glob('*/job.json') if not json.loads(f.read_text()).get('completed')]))"
    r=ssh(4,['python3','-c',code],label);assert r.returncode==0;rows=json.loads(r.stdout);save(OUT/(label+'.json'),rows);return rows
def resume(label):
    r=ssh(4,['runuser','-u','postfiat','--',RELEASE,'transport-certified-send-outbox-resume','--data-dir','/var/lib/postfiat/validator-4','--topology','/etc/postfiat/releases/b23-release4-20261113/topology.json','--max-jobs','1'],label)
    report=json.loads(r.stdout);save(OUT/(label+'.json'),report)
    assert report['quarantined']==0
    print(json.dumps(dict(action=label,exit=r.returncode,pending=report['pending'],all_completed=report['all_completed'])),flush=True)
    six(label);return report['all_completed'] and r.returncode==0
def restart(node):
    before=ssh(node,['systemctl','cat',f'postfiat-validator-{node}.service',f'postfiat-validator-{node}-rpc.service'],f'restart-{node}-units-before');assert before.returncode==0
    for suffix in ('','-rpc'):
        r=ssh(node,['systemctl','restart',f'postfiat-validator-{node}{suffix}.service'],f'restart-{node}{suffix}');assert r.returncode==0
        time.sleep(5)
    for suffix in ('','-rpc'):
        unit=f'postfiat-validator-{node}{suffix}.service'
        code="import subprocess,hashlib; p=subprocess.check_output(['systemctl','show','--value','-p','MainPID',"+repr(unit)+"]).decode().strip(); print(hashlib.sha256(open('/proc/'+p+'/exe','rb').read()).hexdigest())"
        r=ssh(node,['python3','-c',code],f'restart-{node}{suffix}-runtime');assert r.returncode==0 and r.stdout.strip()==SHA
    six(f'restart-{node}');print(json.dumps(dict(restarted_only_node=node,six_converged=True,binary_unchanged=True)),flush=True)
def replay_burn():
    d=OLD/'live/08-alice-voluntary-burn';signed=load(d/'transaction-signed.json');base=load(OLD/'stopped-live-state.json');expected=load(d/'expected-state.json');tx=expected['canary_receipts'][0]['receipt']['tx_id']
    six('same-burn-before');params=dict(signed_asset_transaction_json=json.dumps(signed,separators=(',',':')),proxy_required_current_height=base['tip']['height'],proxy_required_parent_hash=base['tip']['block_hash'],proxy_required_state_root=base['tip']['state_root'])
    require_lock();r=envelope('mempool_submit_signed_asset_transaction_finality',params,4,180);save(OUT/'same-burn-response.json',r)
    save(OUT/'same-burn-identity.json',dict(transaction_id=tx,signed_file=str(d/'transaction-signed.json'),signed_file_sha256=hashlib.sha256((d/'transaction-signed.json').read_bytes()).hexdigest(),new_signature_generated=False))
    if not r.get('ok'):
        e=r.get('error') or {};assert e.get('code')=='rpc_finality_submit_failed' and 'outbox still has 1 pending job' in e.get('message',''),'Unexpected burn replay response'
        six('same-burn-refused');print(json.dumps(dict(same_burn_replayed=True,committed=False,error=e)),flush=True);return False
    f=r['result']['finality'];assert f['confirmed'] and f['receipt']['accepted'] and f['receipt']['tx_id']==tx and f['block']['header']['height']==1318
    rows=read_six(tx);save(OUT/'same-burn-after-six.json',rows);assert len(points(rows))==1 and rows[0]['server']['ledger']['height']==1318
    assert all(any(v['tx_id']==tx and v['accepted'] for v in receipt_rows(row['receipts'])) for row in rows)
    actual=export(OUT/'same-burn-actual-state.json');assert actual['ledger']==expected['ledger'];save(OUT/'same-burn-verified.json',dict(passed=True,height=1318,tx_id=tx,six_validators=True,full_ledger_match=True));return True
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--execute',action='store_true',required=True);ap.parse_args()
    assert not (OUT/'recovery-result.json').exists()
    with LOCK.open('a+') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX);lock.seek(0);assert not lock.read().strip(),'chain lock occupied';lock.seek(0);lock.truncate();lock.write('TEE\n');lock.flush()
        try:
            six('recovery-start');rows=pending('outbox-initial');assert len(rows)==1 and rows[0]['job']['target']=='validator-5' and rows[0]['job']['block_height']==1312
            for node in (4,5):
                r=ssh(node,['sha256sum',RELEASE],f'binary-{node}');assert r.returncode==0 and r.stdout.split()[0]==SHA
            if resume('normal-redelivery-1'):
                save(OUT/'recovery-result.json',dict(cleared=True,method='normal-redelivery',burn_committed=False));return
            if replay_burn():
                save(OUT/'recovery-result.json',dict(cleared=True,method='identical-burn-replay',burn_committed=True));return
            for node in (4,5):
                restart(node)
                if resume(f'normal-redelivery-after-restart-{node}'):
                    save(OUT/'recovery-result.json',dict(cleared=True,method=f'normal-redelivery-after-restart-{node}',burn_committed=False,trivial_transaction_required=True));return
            rows=pending('outbox-after-allowed-recovery');assert len(rows)==1
            save(OUT/'recovery-result.json',dict(cleared=False,stopped=True,reason='Persistent duplicate-record rejection survives normal redelivery, identical burn replay, and sequential affected-service restarts. No permitted data/binary repair remains.',pending_job=rows[0],burn_committed=False,manual_outbox_edits=False,data_deletion=False,binary_changes=False,restarted_nodes=[4,5]))
        except BaseException as e:save(OUT/'recovery-failure.json',dict(error_type=type(e).__name__,message=str(e)[:1500]));raise
        finally:
            lock.seek(0)
            if lock.read().strip()=='TEE':lock.seek(0);lock.truncate();lock.flush()
if __name__=='__main__':main()
