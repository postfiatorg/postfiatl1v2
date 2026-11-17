#!/usr/bin/env python3
"""Phase 5c's exact, backed-up U4 repair after independent finalized-block checks."""
import argparse,fcntl,shlex,time
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime,timezone
from live import read_six,points,require_lock,LOCK
from sign import *
from rpc import call
OUT=COORD/'evidence/tee/phase5c'
HOSTS=['64.176.220.75','95.179.184.122','66.42.48.39','149.28.63.106','95.179.179.206','45.32.110.170']
RELEASE='/opt/postfiat/releases/b23-release4-20261113/postfiat-node'
SCRIPTS=Path(__file__).resolve().parents[1]/'devnet-2'
INITIAL='92fe306019243c222ba67a58e03769304ce01bc9ac5500a58114482eb33345da5ebd23209c2c7bea709ff6cdcf2507fe'
START=1794665805 # Phase 5c directive began 2026-11-14 14:16:45 UTC.
def ssh(i,args,check=True):
    r=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=10','root@'+HOSTS[i],shlex.join(args)],capture_output=True,text=True,timeout=300)
    if check and r.returncode:raise RuntimeError(f'validator-{i}: '+r.stderr[-1500:])
    return r
def scan(i):
    code="""import json,pathlib
root=pathlib.Path('/var/lib/postfiat/validator-N/certified-send-outbox');rows=[]
for f in root.glob('*/job.json'):
 j=json.loads(f.read_text())
 if not j.get('completed'):rows.append(dict(path=str(f.parent),archived=False,job=j))
for d in pathlib.Path('/root/outbox-watchdog-cleared').glob('cleared-*'):
 parts=d.name.split('-',2)
 if len(parts)<3 or int(parts[1])<START:continue
 f=d/'job.json'
 if not f.exists():continue
 j=json.loads(f.read_text());jid=j['job_id']
 if (root/jid).exists() or (root/'completed'/jid).exists():continue
 rows.append(dict(path=str(d),archived=True,job=j))
print(json.dumps(rows))
""".replace('validator-N','validator-'+str(i)).replace('START',str(START))
    return json.loads(ssh(i,['python3','-c',code]).stdout)
def fleet_scan(label):
    with ThreadPoolExecutor(max_workers=6) as pool:rows=list(pool.map(scan,range(6)))
    save(OUT/(label+'-outboxes.json'),rows);return rows
def repair(i,row,label):
    require_lock();j=row['job'];jid=j['job_id'];height=j['block_height'];target=int(j['target'].split('-')[1]);assert 0<=target<6 and j['source']=='validator-'+str(i)
    assert jid==INITIAL or height>=1318,'Only initial job or subsequent cutover blocks are authorized'
    assert 'storage_duplicate_record' in j.get('last_error','') and 'already exists' in j['last_error']
    directory=OUT/'repairs'/(label+'-'+jid[:16]);directory.mkdir(parents=True,exist_ok=False,mode=0o700);save(directory/'job-before.json',row)
    before=read_six();save(directory/'before-six.json',before);assert len(points(before))==1 and before[0]['server']['ledger']['height']>=height
    with ThreadPoolExecutor(max_workers=6) as pool:blocks=list(pool.map(lambda n:call('blocks',{'from_height':height,'limit':1},n),range(6)))
    save(directory/'verified-finalized-blocks.json',blocks)
    assert all(len(b)==1 and b[0]['header']['height']==height and b[0]['header']['block_hash']==j['block_hash'] and b[0]['header']['state_root']==j['expected_state_root'] for b in blocks)
    assert all(b==blocks[0] for b in blocks)
    active=ssh(i,['systemctl','is-active','postfiat-outbox-watchdog.service'],False).stdout.strip()=='active'
    if active:ssh(i,['systemctl','stop','postfiat-outbox-watchdog.service'])
    try:
        root=f'/var/lib/postfiat/validator-{i}/certified-send-outbox';backup=f'/var/backups/postfiat/b24-phase5c/{label}-{jid[:16]}.tar.gz'
        code="import pathlib,tarfile,hashlib,json; p=pathlib.Path("+repr(backup)+"); p.parent.mkdir(mode=0o700,parents=True,exist_ok=True); assert not p.exists(); t=tarfile.open(p,'w:gz'); t.add("+repr(root)+",arcname='certified-send-outbox'); t.close(); print(json.dumps(dict(path=str(p),bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest())))"
        save(directory/'backup.json',json.loads(ssh(i,['python3','-c',code]).stdout))
        if row['archived']:
            # The authorized legacy watchdog preserves copies but moves the live job.
            # Restore this independently verified job only; never delete its backup.
            code="import pathlib,shutil,pwd,os; src=pathlib.Path("+repr(row['path'])+"); dst=pathlib.Path("+repr(root+'/'+jid)+"); assert not dst.exists(); shutil.copytree(src,dst); u=pwd.getpwnam('postfiat'); [os.chown(p,u.pw_uid,u.pw_gid) for p in [dst,*dst.rglob('*')]]"
            ssh(i,['python3','-c',code]);save(directory/'restored-watchdog-copy.json',dict(from_path=row['path'],to_path=root+'/'+jid,source_backup_preserved=True))
        remote='/tmp/b24-phase5c-u4';ssh(i,['mkdir','-p',remote])
        for name in ('complete_certified_send_job.py','rebuild_certified_send_tombstone.py'):
            source=SCRIPTS/name;subprocess.run(['scp','-q',str(source),'root@'+HOSTS[i]+':'+remote+'/'+name],check=True)
            assert ssh(i,['sha256sum',remote+'/'+name]).stdout.split()[0]==hashlib.sha256(source.read_bytes()).hexdigest()
        # The historical helper finds a sibling by height. Independently bind it
        # to the same finalized hash/root/batch/certificate before allowing it.
        code="""import importlib.util,json,pathlib
spec=importlib.util.spec_from_file_location('u4',SCRIPT);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
d=pathlib.Path(JOB);j=json.loads((d/'job.json').read_text());present=(d/(j['target']+'.send.json')).exists()
assert 'storage_duplicate_record' in j.get('last_error','') and 'already exists' in j['last_error']
if not present:
 s,r=m.find_sibling(d.parent,j['block_height'],j['target']);assert s and r
 assert all(s[k]==j[k] for k in ['block_height','block_hash','expected_state_root','batch_hash','certificate_hash'])
print(json.dumps(dict(existing_tombstone=present,sibling_verified=not present)))
""".replace('SCRIPT',repr(remote+'/rebuild_certified_send_tombstone.py')).replace('JOB',repr(root+'/'+jid))
        save(directory/'sibling-check.json',json.loads(ssh(i,['python3','-c',code]).stdout))
        for name,args in [('rebuild_certified_send_tombstone.py',[root+'/'+jid,f'/etc/postfiat/releases/b23-release4-20261113/topology.json']),('complete_certified_send_job.py',[root+'/'+jid+'/job.json'])]:
            r=ssh(i,['runuser','-u','postfiat','--','python3',remote+'/'+name,*args]);(directory/(name+'.log')).write_text(r.stdout+r.stderr)
        result=ssh(i,['runuser','-u','postfiat','--',RELEASE,'transport-certified-send-outbox-resume','--data-dir',f'/var/lib/postfiat/validator-{i}','--topology','/etc/postfiat/releases/b23-release4-20261113/topology.json','--max-jobs','1'],False)
        (directory/'normal-resume.stdout').write_text(result.stdout);(directory/'normal-resume.stderr').write_text(result.stderr)
        report=json.loads(result.stdout);save(directory/'normal-resume.json',report);assert report['all_completed'] and report['pending']==0 and result.returncode==0
        after=read_six();save(directory/'after-six.json',after);assert points(after)==points(before)
        save(directory/'result.json',dict(passed=True,at=datetime.now(timezone.utc).isoformat(),job_id=jid,source=j['source'],target=j['target'],height=height,backup_verified=True,six_matching_finalized_blocks=True,pending_after=0,ledger_unchanged=True,documented_scripts_unmodified=True))
        print(json.dumps(load(directory/'result.json')),flush=True)
    finally:
        if active:ssh(i,['systemctl','start','postfiat-outbox-watchdog.service'])
def callback(label,current=None):
    rows=fleet_scan(label);repaired=[]
    for i,jobs in enumerate(rows):
        for n,row in enumerate(jobs):
            error=row['job'].get('last_error') or ''
            if 'storage_duplicate_record' in error and 'already exists' in error:
                repair(i,row,label+'-'+str(i)+'-'+str(n));repaired.append(row['job']['job_id'])
            elif error:raise RuntimeError('Non-U4 outbox failure; not authorized for this repair: '+error[:500])
    return repaired
def enable_watchdogs():
    def one(i):
        # Run exactly one installed watchdog per node; node 2 has a duplicate unit.
        if i==2:ssh(i,['systemctl','disable','--now','postfiat-outbox2-watchdog.service'])
        r=ssh(i,['systemctl','enable','--now','postfiat-outbox-watchdog.service'])
        active=ssh(i,['systemctl','show','postfiat-outbox-watchdog.service','-p','ActiveState','-p','SubState','-p','UnitFileState','-p','MainPID']).stdout
        assert 'ActiveState=active' in active and 'SubState=running' in active and 'UnitFileState=enabled' in active;return dict(node=i,state=active)
    with ThreadPoolExecutor(max_workers=6) as pool:r=list(pool.map(one,range(6)))
    save(OUT/'watchdogs-enabled.json',r);return r
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--initial-repair',action='store_true',required=True);ap.parse_args()
    with LOCK.open('a+') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX);lock.seek(0);assert not lock.read().strip();lock.seek(0);lock.truncate();lock.write('TEE\n');lock.flush()
        try:
            rows=scan(4);assert len(rows)==1 and rows[0]['job']['job_id']==INITIAL;repair(4,rows[0],'initial');enable_watchdogs()
            rows=fleet_scan('after-initial');assert not any(rows);save(OUT/'initial-repair-result.json',dict(passed=True,all_six_outboxes_empty=True,all_six_watchdogs_running=True))
        finally:
            lock.seek(0)
            if lock.read().strip()=='TEE':lock.seek(0);lock.truncate();lock.flush()
if __name__=='__main__':main()
