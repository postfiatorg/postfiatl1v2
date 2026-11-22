#!/usr/bin/env python3
"""Failure records and the optional notification hook for the B24 reading.

Standard library only, so the OnFailure= unit can run it with `python3 -I`
even when the job's own environment is broken. B24_READING_NOTIFY holds either
an http(s) webhook URL, which receives the one-line summary as a text/plain
POST, or a command, which receives it as its last argument. Unset: nothing is
sent. Keep the value in the optional EnvironmentFile, never in the repository.
"""
import argparse,json,os,shlex,subprocess,sys,urllib.request
from datetime import datetime,timezone
from pathlib import Path
COIN='B24'
HOOK='B24_READING_NOTIFY'
# Fixed location for records of runs that fail before their run directory exists.
STATE=Path('/home/postfiat/.local/state/b24-tee-nav-daily')

def utc_now():return datetime.now(timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
def write(path,value):
    path.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
    temporary=path.with_suffix(path.suffix+'.tmp');temporary.write_text(json.dumps(value,indent=2)+'\n');temporary.replace(path)

def describe(error):
    if isinstance(error,SystemExit):return f'stopped with exit code {error.code}'
    return ' '.join(f'{type(error).__name__}: {error}'.split())[:300]

def notify(line):
    """Call the configured hook once. The result never contains the hook value."""
    hook=os.environ.get(HOOK,'').strip()
    if not hook:return 'unset'
    try:
        if hook.startswith(('https://','http://')):
            request=urllib.request.Request(hook,data=line.encode(),headers={'Content-Type':'text/plain; charset=utf-8'},method='POST')
            with urllib.request.urlopen(request,timeout=30) as response:return f'sent (HTTP {response.status})'
        result=subprocess.run(shlex.split(hook)+[line],stdin=subprocess.DEVNULL,timeout=60)
        return 'sent' if result.returncode==0 else f'hook exited {result.returncode}'
    except Exception as error:return f"hook failed: {type(error).__name__} {getattr(error,'code','')}".strip()

def record_failure(stage,error,directory=None,mode='daily',invocation=None):
    """Write the failure record, call the hook once and return the record.

    Never raises: the caller re-raises the original error.
    """
    at=utc_now();invocation=invocation or os.environ.get('INVOCATION_ID')
    record=dict(coin=COIN,stage=stage,error_type=type(error).__name__,message=str(error)[:500],time=at,mode=mode,automatic_retry=False,invocation_id=invocation)
    targets=([Path(directory)/'failure.json'] if directory else [])+[STATE/'failures'/f"{at.replace(':','')}-{mode}-{stage}.json"]
    path=None
    for target in targets:
        try:write(target,record);path=target;break
        except Exception as problem:print(f'failure record not written to {target}: {describe(problem)}',file=sys.stderr)
    line=f'{COIN} price reading failed at stage {stage}: {describe(error)} ({at})'+(f'; record {path}' if path else '')
    record['notification']=notify(line)
    print(line+f"; notification {record['notification']}",file=sys.stderr)
    try:
        if path:write(path,record)
        if invocation:write(STATE/'notified'/f'{invocation}.json',dict(record=str(path),notification=record['notification'],time=at))
    except Exception as problem:print(f'failure record not updated: {describe(problem)}',file=sys.stderr)
    return record

def unit_state(unit):
    out=subprocess.run(['systemctl','--user','show',unit,'--property=InvocationID,Result,ExecMainCode,ExecMainStatus'],capture_output=True,text=True,timeout=30).stdout
    return dict(line.split('=',1) for line in out.splitlines() if '=' in line)

def service_failed(unit):
    """OnFailure= entry: report a service death that the job did not report itself."""
    state={}
    try:state=unit_state(unit)
    except Exception as error:print(f'systemctl show failed: {describe(error)}',file=sys.stderr)
    invocation=os.environ.get('MONITOR_INVOCATION_ID') or state.get('InvocationID') or None
    if invocation and (STATE/'notified'/f'{invocation}.json').exists():print('failure already reported by the job');return None
    result=os.environ.get('MONITOR_SERVICE_RESULT') or state.get('Result','unknown')
    status=os.environ.get('MONITOR_EXIT_STATUS') or state.get('ExecMainStatus','unknown')
    return record_failure('service',RuntimeError(f'{unit} ended with result {result}, status {status}'),mode='service',invocation=invocation)

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--service-failed',metavar='UNIT',required=True);a=ap.parse_args()
    service_failed(a.service_failed)
if __name__=='__main__':main()
