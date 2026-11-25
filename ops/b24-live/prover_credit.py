#!/usr/bin/env python3
"""Report before the rented proving hosts run out of Vast credit.

Every 15 minutes (prover-credit-alert.timer): read the account's remaining
credit (`vastai show user --raw`) and the hourly cost of its instances
(`vastai show instances --raw`: `dph_total` of running instances plus
`storage_total_cost` of stopped ones), and report through the B24 reading's
hook (reading_alert.notify, B24_READING_NOTIFY) when the hours left fall under
--warn-hours (24) and again, louder, under --urgent-hours (6). Each crossing is
reported once it is delivered; a credit that cannot be read is reported once,
and again every 6 hours while it stays unreadable. The Vast CLI reads its own key file; this script
reads that file only to strip the key from anything it prints or sends.
Standard library only, so it runs with `python3 -I` like the failure hook.
"""
import argparse,importlib.util,json,math,os,re,subprocess,sys,time
from pathlib import Path
# Loaded by path: this directory also holds operator.py, which must not shadow the standard library.
spec=importlib.util.spec_from_file_location('reading_alert',Path(__file__).resolve().with_name('reading_alert.py'))
reading_alert=importlib.util.module_from_spec(spec);spec.loader.exec_module(reading_alert)
notify,utc_now,write=reading_alert.notify,reading_alert.utc_now,reading_alert.write

VASTAI=Path.home()/'.local/bin/vastai'
STATE=Path.home()/'.local/state/prover-credit-alert/state.json'
LEVELS=('ok','warn','urgent')
RUNNING={'running'}
REPEAT_SECONDS=6*3600  # an unreadable credit is reported again after this long
# Each Vast CLI call; two calls and the hook (reading_alert.notify, at most 60 s) stay far inside the unit's TimeoutStartSec.
VASTAI_TIMEOUT_SECONDS=60

def key_files():
    config=Path(os.environ.get('XDG_CONFIG_HOME') or Path.home()/'.config')
    return [config/'vastai/vast_api_key',Path.home()/'.vast_api_key']

def secrets():
    found=[]
    for path in key_files():
        try:value=path.read_text().strip()
        except OSError:continue
        if value:found.append(value)
    if os.environ.get('VAST_API_KEY','').strip():found.append(os.environ['VAST_API_KEY'].strip())
    return found

def redact(text,values=None):
    """Remove each key, then any start or end of it of 8 or more characters, longest first:
    a text cut elsewhere may hold only part of a key."""
    for value in (secrets() if values is None else values):
        for size in range(len(value),min(8,len(value))-1,-1):text=text.replace(value[:size],'[redacted]').replace(value[-size:],'[redacted]')
    text=re.sub(r'(api_key=)[^&\s"\']+',r'\1[redacted]',text)
    return re.sub(r'(Bearer\s+)\S+',r'\1[redacted]',text)

def vastai(*args):
    """Run one read-only CLI call; the error never carries the key or raw output."""
    try:result=subprocess.run([str(VASTAI),*args,'--raw'],stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=VASTAI_TIMEOUT_SECONDS)
    except Exception as error:raise RuntimeError(f'vastai {args[-1]}: {type(error).__name__}') from None
    if result.returncode:raise RuntimeError(f'vastai {args[-1]} exited {result.returncode}: '+redact(' '.join(result.stderr.split()))[-200:])
    try:return json.loads(result.stdout)
    except ValueError:raise RuntimeError(f'vastai {args[-1]}: output is not JSON') from None

def number(value):return isinstance(value,(int,float)) and not isinstance(value,bool) and math.isfinite(value)

def read_credit(run=vastai):
    """Return (credit in USD, spend in USD per hour, running count, stopped count billed for storage).
    A running instance costs its `dph_total` (storage included); any other instance still costs its
    `storage_total_cost` (USD per hour) until it is destroyed.
    A reply of any other shape raises, so it is reported as not usable, never as endless hours."""
    user=run('show','user')
    # `credit` only: nothing in the Vast CLI's documentation says `balance` means the same.
    credit=user.get('credit') if isinstance(user,dict) else None
    if not number(credit):raise RuntimeError('vastai show user: no credit field')
    instances=run('show','instances')
    if not isinstance(instances,list) or not all(isinstance(i,dict) for i in instances):raise RuntimeError('vastai show instances: not a list of instances')
    spend,count,stored=0.0,0,0
    for i in instances:
        if RUNNING&{i.get('actual_status'),i.get('intended_status'),i.get('cur_state')}:
            if not number(i.get('dph_total')) or i['dph_total']<=0:raise RuntimeError(f"vastai show instances: running instance {i.get('id','?')} has no positive dph_total")
            spend,count=spend+i['dph_total'],count+1
        else:
            if not number(i.get('storage_total_cost')) or i['storage_total_cost']<0:raise RuntimeError(f"vastai show instances: stopped instance {i.get('id','?')} has no storage_total_cost")
            spend,stored=spend+i['storage_total_cost'],stored+(i['storage_total_cost']>0)
    return float(credit),float(spend),count,stored

def level(hours,warn,urgent):
    return 'urgent' if hours<urgent else 'warn' if hours<warn else 'ok'

def message(kind,credit=0,spend=0,count=0,hours=0,warn=24,urgent=6,error='',stored=0):
    if kind=='unreadable':return f'Proving hosts: Vast credit could not be read ({error}); the reading is not usable and the low-credit alert is blind until it can ({utc_now()})'
    head='URGENT: proving hosts stop' if kind=='urgent' else 'Proving hosts stop'
    limit=urgent if kind=='urgent' else warn
    return f'{head} in about {hours:.1f} h: Vast credit {credit:.2f} USD at {spend:.2f} USD/h for {count} running instance(s){f' and {stored} stopped instance(s) billed for storage' if stored else ''}, under the {limit:g} h limit. Top up the Vast account ({utc_now()})'

def load(path):
    try:state=json.loads(path.read_text())
    except (OSError,ValueError):state={}
    kept=dict(level=state.get('level','ok') if state.get('level') in LEVELS else 'ok',unreadable=bool(state.get('unreadable')))
    if number(state.get('reported_at')):kept['reported_at']=float(state['reported_at'])
    return kept

def delivered(result):
    """True only when the hook took the report. An unset address sends nothing, so it is a failure too."""
    if result=='unset':print('the alert address is not set: nothing was sent',file=sys.stderr);return False
    return not result.startswith(('hook failed','hook exited'))

def handled(state):
    """Name the systemd run that made its own report, so its OnFailure= unit does not repeat it."""
    if os.environ.get('INVOCATION_ID'):state['handled_invocation']=os.environ['INVOCATION_ID']

def check(warn=24.0,urgent=6.0,state_path=STATE,run=vastai,send=notify):
    """One run: returns the state written. A report counts as made only when delivered;
    an undelivered report stays pending for the next run and the run fails (ok=False)."""
    state=load(state_path)
    try:credit,spend,count,stored=read_credit(run)
    except Exception as error:
        line=message('unreadable',error=redact(str(error))[:300])
        if not state['unreadable'] or time.time()-state.get('reported_at',0)>REPEAT_SECONDS:
            result=send(line)
            # A delivered report is recorded at once, before anything else can fail or time out.
            if delivered(result):state.update(unreadable=True,reported_at=time.time());write(state_path,state)
            print(line+f'; notification {result}',file=sys.stderr)
        else:print(line+'; already reported',file=sys.stderr)
        handled(state);write(state_path,state);return dict(state,ok=False)
    state['unreadable']=False;state.pop('reported_at',None)
    hours=0.0 if credit<=0 else float('inf') if spend<=0 else credit/spend
    now=level(hours,warn,urgent);summary=f'credit {credit:.2f} USD, spend {spend:.2f} USD/h, {count} running, {stored} stopped billed for storage, {hours:.1f} h left, level {now}'
    ok=True
    if LEVELS.index(now)>LEVELS.index(state['level']):
        result=send(message(now,credit,spend,count,hours,warn,urgent,stored=stored))
        if delivered(result):state['level']=now;write(state_path,state)
        else:now,ok=state['level'],False
        summary+=f'; notification {result}'
    print(summary)
    state['level']=now;handled(state);write(state_path,state);return dict(state,ok=ok,hours=hours)

def service_failed(unit,state_path=STATE,send=notify):
    """OnFailure= entry: report a failed run that ended without its own report (a crash, a timeout)."""
    invocation=os.environ.get('MONITOR_INVOCATION_ID')
    if not invocation:
        try:invocation=reading_alert.unit_state(unit).get('InvocationID')
        except Exception as error:print(f'systemctl show failed: {type(error).__name__}',file=sys.stderr)
    try:state=json.loads(state_path.read_text())
    except (OSError,ValueError):state={}
    if invocation and isinstance(state,dict) and state.get('handled_invocation')==invocation:
        print(f'{unit} failed after its own report; nothing more to send');return True
    result=os.environ.get('MONITOR_SERVICE_RESULT','unknown')
    line=redact(f'{unit} failed (result {result}) without its own report: the Vast credit was not checked. See journalctl --user -u {unit} ({utc_now()})')
    sent=send(line);print(line+f'; notification {sent}',file=sys.stderr)
    return delivered(sent)

def main():
    ap=argparse.ArgumentParser(description=__doc__,formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--service-failed',metavar='UNIT',help='OnFailure= entry: report a failed run of UNIT')
    ap.add_argument('--warn-hours',type=float,default=float(os.environ.get('PROVER_CREDIT_WARN_HOURS',24)))
    ap.add_argument('--urgent-hours',type=float,default=float(os.environ.get('PROVER_CREDIT_URGENT_HOURS',6)))
    a=ap.parse_args()
    if a.service_failed:sys.exit(0 if service_failed(a.service_failed) else 1)
    if not 0<a.urgent_hours<a.warn_hours:ap.error('need 0 < --urgent-hours < --warn-hours')
    sys.exit(0 if check(a.warn_hours,a.urgent_hours)['ok'] else 1)
if __name__=='__main__':main()
