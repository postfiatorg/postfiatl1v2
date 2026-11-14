"""Reuse the verified read-only exporter; never open a live NodeStore for writing."""
import uuid,shlex,time
from sign import *
from rpc import call
def export(output):
    digest=hashlib.sha256(CANARY.read_bytes()).hexdigest();remote='/tmp/b23-tee-nav-canary-'+digest[:16]
    actual=subprocess.check_output(['ssh','-o','BatchMode=yes','root@64.176.220.75','sha256sum '+shlex.quote(remote)],text=True).split()[0]
    assert actual==digest
    target='/tmp/b24-readonly-'+uuid.uuid4().hex+'.json';before=call('server_info')
    attempts=[]
    for attempt in range(20):
        result=subprocess.run(['ssh','-o','BatchMode=yes','root@64.176.220.75',shlex.join([remote,'export','--data-dir','/var/lib/postfiat/validator-0','--output',target])],capture_output=True,text=True)
        attempts.append(dict(attempt=attempt,returncode=result.returncode,stderr=result.stderr[-2000:]))
        save(Path(str(output)+'.export-attempts.json'),attempts)
        if result.returncode==0:break
        if 'Database already open. Cannot acquire lock.' not in result.stderr:result.check_returncode()
        time.sleep(2)
    else:raise RuntimeError('Read-only export database remained busy; no services changed')
    subprocess.run(['scp','-q','root@64.176.220.75:'+target,str(output)],check=True)
    after=call('server_info');s=load(output)
    assert before['ledger']['height']==after['ledger']['height']==s['tip']['height'] and before['ledger']['state_root']==after['ledger']['state_root']==s['tip']['state_root']
    return s
