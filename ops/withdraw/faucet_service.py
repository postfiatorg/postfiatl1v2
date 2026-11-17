#!/usr/bin/env python3
"""Starter PFT for new Task Node wallets. Loopback only; the Task Node relay is the only caller.
POST /api/bridge/faucet {"address":"pf…","account":"<task node account id>"} is idempotent per address:
the first call queues one small grant, later calls report its state. Limits: one grant per address,
MAX_PER_ACCOUNT addresses per Task Node account, MAX_PER_DAY grants per UTC day. The grant itself is
the existing `pft faucet` command (sealed faucet key, six-validator check); this file holds no key."""
import fcntl,json,os,re,subprocess,sys,threading,time
from datetime import datetime,timezone
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from pathlib import Path
PORT=int(os.environ.get('FAUCET_PORT','18085'))
AMOUNT=int(os.environ.get('FAUCET_AMOUNT_ATOMS','500000'))
MAX_PER_ACCOUNT=int(os.environ.get('FAUCET_MAX_PER_ACCOUNT','3'))
MAX_PER_DAY=int(os.environ.get('FAUCET_MAX_PER_DAY','20'))
PFT=os.environ.get('FAUCET_PFT_BIN','/home/postfiat/.local/bin/pft')
COORD=Path('/home/postfiat/repos/b23-coord');E=COORD/'evidence/faucet';E.mkdir(parents=True,exist_ok=True)
STATE=E/'grants.json';LOCK=COORD/'chain.lock'
mutex=threading.Lock();wake=threading.Event()
now=lambda:datetime.now(timezone.utc).isoformat(timespec='seconds')
def load():
    try:return json.loads(STATE.read_text())
    except FileNotFoundError:return {}
def save(grants):
    tmp=STATE.with_suffix('.tmp');tmp.write_text(json.dumps(grants,indent=1));tmp.replace(STATE)
def log(*a):print(now(),*a,flush=True)

def acquire(lock,timeout=900):
    """Same rule as the live writers: never write while another lane holds chain.lock."""
    deadline=time.monotonic()+timeout
    while time.monotonic()<deadline:
        try:fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        except BlockingIOError:time.sleep(2);continue
        lock.seek(0)
        if not lock.read().strip():
            lock.seek(0);lock.truncate();lock.write('FAUCET\n');lock.flush();return
        fcntl.flock(lock,fcntl.LOCK_UN);time.sleep(2)
    raise RuntimeError('chain busy')
def release(lock):
    lock.seek(0)
    if lock.read().strip()=='FAUCET':lock.seek(0);lock.truncate();lock.flush()

def grant(address):
    with LOCK.open('a+') as lock:
        acquire(lock)
        try:
            r=subprocess.run([PFT,'faucet',address,'--asset','PFT','--amount',str(AMOUNT)],capture_output=True,text=True,timeout=300)
        finally:release(lock)
    out=r.stdout+r.stderr
    fields=dict(re.findall(r'^(\w+): (.*)$',out,re.M))
    if r.returncode!=0 or fields.get('ok')!='True' or fields.get('verified_6_of_6')!='True' or fields.get('to')!=address:
        raise RuntimeError('grant not confirmed: '+out.strip()[-300:])
    return dict(tx_id=fields.get('tx_id'),height=int(fields.get('height','0')))

def worker():
    while True:
        wake.wait(20);wake.clear()
        with mutex:pending=[a for a,g in load().items() if g['stage']=='queued']
        for address in pending:
            try:result=dict(grant(address),stage='sent',error=None)
            except Exception as error:result=dict(stage='failed',error=str(error)[:300])
            with mutex:
                grants=load();grants[address].update(result,updated_at=now());save(grants)
            log(address,result['stage'],result.get('tx_id') or result.get('error'))

class Handler(BaseHTTPRequestHandler):
    def log_message(self,*a):pass
    def reply(self,code,value):
        body=json.dumps(value).encode();self.send_response(code);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.send_header('Cache-Control','no-store');self.end_headers();self.wfile.write(body)
    def do_POST(self):
        try:
            if self.path!='/api/bridge/faucet':return self.reply(404,dict(ok=False,code='not_found'))
            body=json.loads(self.rfile.read(min(int(self.headers.get('Content-Length','0')),4096)) or b'{}')
            address,account=body.get('address',''),body.get('account','')
            if not isinstance(address,str) or not re.fullmatch(r'pf[0-9a-f]{40}',address) or not isinstance(account,str) or not re.fullmatch(r'[A-Za-z0-9:_@.\-]{1,128}',account):
                return self.reply(400,dict(ok=False,code='invalid_faucet_request',message='Invalid starter PFT request.'))
            with mutex:
                grants=load();g=grants.get(address)
                if g is None or (g['stage']=='failed' and g.get('retries',0)<3):
                    today=now()[:10]
                    mine={a for a,x in grants.items() if x['account']==account}|{address}
                    if len(mine)>MAX_PER_ACCOUNT:return self.reply(429,dict(ok=False,code='faucet_account_limit',message='This account has already received starter PFT for its wallets.'))
                    if g is None and sum(1 for x in grants.values() if x['created_at'][:10]==today)>=MAX_PER_DAY:return self.reply(429,dict(ok=False,code='faucet_daily_limit',message='Starter PFT is used up for today. Try again tomorrow.'))
                    g=dict(account=account,amount_atoms=AMOUNT,stage='queued',error=None,tx_id=None,created_at=(g or {}).get('created_at',now()),updated_at=now(),retries=(g or {}).get('retries',-1)+1)
                    grants[address]=g;save(grants);log(address,'queued',account[:12])
            wake.set();self.reply(200,dict(ok=True,address=address,stage=g['stage'],amount_atoms=str(g['amount_atoms']),tx_id=g.get('tx_id')))
        except Exception as error:self.reply(503,dict(ok=False,code='faucet_service_error',message=str(error)[:200]))

if __name__=='__main__':
    threading.Thread(target=worker,daemon=True).start()
    log('faucet service on 127.0.0.1:%d'%PORT);ThreadingHTTPServer(('127.0.0.1',PORT),Handler).serve_forever()
