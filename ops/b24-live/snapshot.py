"""Reuse the verified read-only exporter; never open a live NodeStore for writing."""
import uuid,shlex,time
from sign import *
from rpc import read
VALIDATOR0='root@64.176.220.75'
def export(output):
    digest=hashlib.sha256(CANARY.read_bytes()).hexdigest();remote='/tmp/b23-tee-nav-canary-'+digest[:16]
    actual=subprocess.check_output(['ssh','-o','BatchMode=yes',VALIDATOR0,'sha256sum '+shlex.quote(remote)],text=True).split()[0]
    assert actual==digest
    target='/tmp/b24-readonly-'+uuid.uuid4().hex+'.json';before=read('status')
    attempts=[]
    for attempt in range(20):
        result=subprocess.run(['ssh','-o','BatchMode=yes',VALIDATOR0,shlex.join([remote,'export','--data-dir','/var/lib/postfiat/validator-0','--output',target])],capture_output=True,text=True)
        attempts.append(dict(attempt=attempt,returncode=result.returncode,stderr=result.stderr[-2000:]))
        save(Path(str(output)+'.export-attempts.json'),attempts)
        if result.returncode==0:break
        if 'Database already open. Cannot acquire lock.' not in result.stderr:result.check_returncode()
        time.sleep(2)
    else:raise RuntimeError('Read-only export database remained busy; no services changed')
    subprocess.run(['scp','-q',VALIDATOR0+':'+target,str(output)],check=True)
    after=read('status');s=load(output)
    assert before['block_height']==after['block_height']==s['tip']['height'] and before['state_root']==after['state_root']==s['tip']['state_root']
    return s
