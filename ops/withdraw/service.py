#!/usr/bin/env python3
"""PFUSDC -> USDC withdrawal job service (Ethereum mainnet, vault 0xE7A4...).

A holder burns PFUSDC on the network from their own wallet (vault_bridge_burn_to_redeem).
This service then does the rest with no further signature from the holder:
  read the finalized burn -> catch the Ethereum checker up if it is more than 64 blocks behind ->
  build the withdrawal witness on the vault's fixed exit setting -> prove it on the GPU ->
  send withdrawWithProof from the operator wallet (gas only; the proof fixes amount and recipient).
It cannot pay anyone anything the proof does not bind. Operator key is used by path, never printed.
Listens on 127.0.0.1 only; the Task Node server is the only caller.
"""
import json,os,re,shlex,subprocess,sys,threading,time,uuid,hashlib
from web3 import Web3
from datetime import datetime,timezone
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from pathlib import Path
HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE.parent/'b24-live'));sys.path.insert(0,str(HERE))
from sign import load,save
from rpc import call
from live import read_six,points
import archive as arch
import exit_profile
from eth_journal import durable_json, ensure_directory, intent_path, read_eth_result, ReconciliationRequired
from eth_send import RPC, VERIFIER, VAULT, OPERATOR, KEY
PY=sys.executable
COORD=Path('/home/postfiat/repos/b23-coord');E=COORD/'evidence/withdraw';JOBS=E/'jobs'
HOST_BIN=E/'bin/proof-host';ELF=E/'historical/egress.elf'
FAMILY='02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b'
SERIES='541e451e6ae96b6680988655571a0feec59c7f1d073c9b5d29547254cab38a3be540f8767446d25baebc7081ce97aef7'
VAST=['-p','20628','root@ssh8.vast.ai'];REMOTE='/workspace/b23-withdraw-20261116'
PORT=18084;MAX_STEP=64
queue_event=threading.Event();lock=threading.RLock()
worker_started=threading.Event()
ACTIVE_STAGES = {'queued', 'reading', 'catching_up', 'proving', 'paying'}


def now():return datetime.now(timezone.utc).isoformat(timespec='seconds')
def job_path(burn):return JOBS/burn/'job.json'
def read_job(burn):
    p=job_path(burn);return load(p) if p.exists() else None
def write_job(job):
    with lock:
        p=job_path(job['burn_tx_id']);ensure_directory(p.parent);durable_json(p,job)
def stage(job,name,detail=''):
    job.update(stage=name,detail=detail,updated_at=now());job.setdefault('history',[]).append([now(),name,detail]);write_job(job);print(now(),job['burn_tx_id'][:12],name,detail,flush=True)

def run(args,**kw):return subprocess.run([str(a) for a in args],check=True,capture_output=True,text=True,**kw)

def bridge_status():
    return call('vault_bridge_status',{'asset_id':FAMILY})
def payout_preflight():
    if not worker_started.is_set():
        return 'Withdrawal worker is stopped.'
    if not os.access(HOST_BIN, os.X_OK) or not ELF.is_file():
        return 'Local proving artifacts are unavailable.'
    if not os.access(KEY, os.R_OK):
        return 'Ethereum operator signing key is unavailable.'
    abi = [{'type':'function', 'name':name, 'stateMutability':'view', 'inputs':[],
            'outputs':[{'name':'', 'type':kind}]} for name, kind in
           (('paused', 'bool'), ('finalityVerifier', 'address'))]
    try:
        client = Web3(Web3.HTTPProvider(RPC, request_kwargs={'timeout':5}))
        vault = client.eth.contract(address=VAULT, abi=abi)
        block = client.eth.block_number
        if (client.eth.chain_id != 1 or vault.functions.paused().call(block_identifier=block) or
                vault.functions.finalityVerifier().call(block_identifier=block).lower() != VERIFIER.lower()):
            return 'Ethereum vault is paused or its network/verifier binding is unexpected.'
        if not client.eth.get_balance(OPERATOR, block_identifier=block):
            return 'Ethereum operator has no gas balance.'
    except Exception as error:
        return 'Ethereum route preflight failed: '+str(error)[:160]
    return None


def readiness():
    s=bridge_status();route=call('vault_bridge_route',{'asset_id':FAMILY})
    profile_hash=route['beacon_finality_anchor']['route_profile_hash']
    buckets=[b for b in s['buckets'] if b['policy_hash']==profile_hash and b.get('status','active')=='active']
    jobs = [load(path) for path in JOBS.glob('*/job.json')]
    blocked = [j['burn_tx_id'] for j in jobs if j['stage'] in ('failed', 'reconciliation_required')]
    payments = [j['updated_at'] for j in jobs if j['stage'] == 'paid']
    stalled = [j['burn_tx_id'] for j in jobs if j['stage'] in ACTIVE_STAGES and
               (datetime.now(timezone.utc) - datetime.fromisoformat(j['updated_at'])).total_seconds() > 3600]
    preflight_error = payout_preflight()
    payout_ready = not (blocked or stalled or preflight_error)
    health = dict(ok=True, bucket_ready=len(buckets)==1, payout_ready=payout_ready,
                  payout_preflight_error=preflight_error, last_payment_recorded_at=max(payments, default=None),
                  blocked_jobs=blocked, stalled_jobs=stalled)
    if len(buckets)!=1:return dict(**health,ready=False,message='No single active deposit bucket for the current bridge setting.')
    return dict(**health,ready=payout_ready,asset_id=FAMILY,series_asset_id=SERIES,bucket_id=buckets[0]['bucket_id'],issuer=s['issuer'],epoch=s['finalized_epoch'],
        reserve_packet_hash=s['finalized_reserve_packet_hash'],bucket_outstanding_atoms=str(buckets[0]['outstanding_vault_bridge_atoms']),
        destination_prefix='evm-erc20:1:',vault_address='0xe7a432a28b20621a70e845c7f202004cf98e7a2c',minutes_estimate=12,current_height=route['current_height'])

def export_bundle(directory):
    """Authentic public finalized history, read-only, same exporter the launch used."""
    before=read_six();assert len(points(before))==1
    digest=hashlib.sha256(arch.ARCHIVE.read_bytes()).hexdigest();remote='/tmp/b24-readonly-archive-'+digest[:16]
    check=subprocess.run(['ssh','-o','BatchMode=yes',arch.HOST,'sha256sum '+shlex.quote(remote)],capture_output=True,text=True)
    if check.returncode or check.stdout.split()[0]!=digest:
        run(['scp','-q',arch.ARCHIVE,arch.HOST+':'+remote])
    target='/tmp/b24-public-archive-'+uuid.uuid4().hex+'.json'
    for n in range(30):
        r=subprocess.run(['ssh','-o','BatchMode=yes',arch.HOST,shlex.join([remote,'export','--data-dir','/var/lib/postfiat/validator-0','--output',target])],capture_output=True,text=True)
        if r.returncode==0:break
        if 'Database already open. Cannot acquire lock.' not in r.stderr:r.check_returncode()
        time.sleep(2)
    else:raise RuntimeError('network history export stayed busy')
    bundle=directory/'public-archive.json';run(['scp','-q',arch.HOST+':'+target,bundle]);subprocess.run(['ssh','-o','BatchMode=yes',arch.HOST,'rm -f '+shlex.quote(target)])
    after=read_six();v=load(bundle);assert points(after)==points(before) and v['tip']['height']==after[0]['server']['ledger']['height'] and v['tip']['state_root']==after[0]['server']['ledger']['state_root']
    return bundle,v

def audit(directory):
    run([PY,HERE/'audit.py','--output',directory],env=dict(os.environ,PYTHONDONTWRITEBYTECODE='1'));return load(directory/'mainnet.json')

def prover_window():
    """The daily Schwab reading proves between about 18:10 and 18:25 UTC. Stay out of its way."""
    while 1803<=int(datetime.now(timezone.utc).strftime('%H%M'))<1827:time.sleep(20)
def prove(kind,witness,directory,name):
    prover_window();directory.mkdir(parents=True,exist_ok=True)
    run([HOST_BIN,kind,ELF,witness,directory/'native','native'])
    run(['ssh','-o','BatchMode=yes']+VAST+[f'mkdir -p {REMOTE}/jobs'])
    run(['scp','-q','-P','20628',witness,f'root@ssh8.vast.ai:{REMOTE}/jobs/{name}.json'])
    cmd=f"cd {REMOTE} && rm -rf jobs/{name}-proof && (exec 9>/workspace/b23-gpu/prover.lock; flock -w 2400 9; SP1_PROVER=cuda RAYON_NUM_THREADS=4 timeout 1500 ./proof-host {kind} egress.elf jobs/{name}.json jobs/{name}-proof prove > jobs/{name}.log 2>&1); test -s jobs/{name}-proof/proof-report.json"
    r=subprocess.run(['ssh','-o','BatchMode=yes','-o','ServerAliveInterval=30']+VAST+[cmd],capture_output=True,text=True)
    if r.returncode:raise RuntimeError('GPU proof failed: '+(r.stderr or r.stdout)[-300:])
    for f in ('proof-report.json','public-values.bin','proof-calldata.bin'):run(['scp','-q','-P','20628',f'root@ssh8.vast.ai:{REMOTE}/jobs/{name}-proof/{f}',directory/f])
    assert (directory/'public-values.bin').read_bytes()==(directory/'native/native-public-values.bin').read_bytes()
    return directory
def eth_send(mode, proof_dir, out):
    result = subprocess.run(
        [PY, str(HERE / 'eth_send.py'), mode, str(proof_dir), str(out)],
        capture_output=True, text=True, env=dict(os.environ, PYTHONDONTWRITEBYTECODE='1'),
    )
    if result.returncode:
        detail = (result.stderr.strip().splitlines() or ['unknown'])[-1][-300:]
        if out.exists():
            return read_eth_result(mode, out)
        if intent_path(out).exists():
            try:
                tx = load(intent_path(out))['tx']
            except (KeyError, OSError, ValueError, TypeError):
                tx = 'unreadable transaction id'
            raise ReconciliationRequired(f'Reconcile signed Ethereum {mode} {tx}: {detail}')
        raise RuntimeError('Ethereum transaction failed: ' + detail)
    return read_eth_result(mode, out)


def historical_checkpoint(blocks, height, audit_record):
    verifier = audit_record['contracts']['PFTLFinalityVerifierV1']
    if verifier['address'].lower() != VERIFIER.lower():
        raise ReconciliationRequired('Unexpected Ethereum verifier; reconcile the payout route.')
    client = Web3(Web3.HTTPProvider(RPC, request_kwargs={'timeout':30}))
    if client.eth.chain_id != 1:
        raise ReconciliationRequired('Unexpected Ethereum network; reconcile the payout route.')
    abi = [{'type':'function', 'name':'acceptedCheckpointCommitment', 'stateMutability':'view',
            'inputs':[{'name':'commitment', 'type':'bytes32'}], 'outputs':[{'name':'', 'type':'bool'}]}]
    contract = client.eth.contract(address=VERIFIER, abi=abi)
    for candidate in range(height-1, max(-1, height-MAX_STEP-2), -1):
        if candidate not in blocks:
            continue
        commitment = Web3.keccak(bytes.fromhex(blocks[candidate]['header']['block_hash']))
        if contract.functions.acceptedCheckpointCommitment(commitment).call(block_identifier=audit_record['block']):
            return candidate
    raise ReconciliationRequired('No accepted Ethereum ancestor in the bounded proof window; reconcile the burn.')


def native_witness(blocks, prior, target, withdrawal_id=None):
    """Page the native exporter while preserving governance transitions at each boundary."""
    prior_hash = blocks[prior]['header']['block_hash']
    ancestry = []
    start = prior
    span = 8  # Typical committee certificates take about 200 KB per block; RPC frames are 4 MB.
    while True:
        end = min(start + span, target)
        final = end == target
        params = {'prior_checkpoint': blocks[start]['header']['block_hash']}
        if final and withdrawal_id:
            method = 'pfusdc_egress_witness'
            params['withdrawal_id'] = withdrawal_id
        else:
            method = 'pfusdc_checkpoint_witness'
            params['target_block'] = blocks[end]['header']['block_hash']
        try:
            page = call(method, params)
        except (RuntimeError, json.JSONDecodeError) as error:
            # Older validators close the stream when their response is too large.
            oversized = isinstance(error, json.JSONDecodeError) and not error.doc.strip()
            if span <= 2 or not (oversized or 'rpc_response_too_large' in str(error)):
                raise
            span = max(2, span // 2)
            continue
        steps = page.get('finality_ancestry', [])
        if (page['prior_checkpoint_block_id'] != params['prior_checkpoint'] or
                page['block']['header']['block_hash'] != blocks[end]['header']['block_hash'] or
                [step['block']['header']['block_hash'] for step in steps] !=
                [blocks[h]['header']['block_hash'] for h in range(start+1, end)]):
            raise ValueError('Native witness page differs from the six-validator finalized archive.')
        ancestry.extend(steps)
        if final:
            return dict(page, finality_ancestry=ancestry, prior_checkpoint_block_id=prior_hash)
        # Keep the boundary block as an ancestry step on the next page. The exporter
        # includes its governance transition only when it can see the following block.
        start = end - 1


def recover_payment(job, directory):
    result = directory / 'eth-withdrawal.json'
    if result.exists() or intent_path(result).exists():
        try:
            done = read_eth_result('withdraw', result) if result.exists() else eth_send('withdraw', directory/'proof', result)
        except ReconciliationRequired as error:
            stage(job, 'reconciliation_required', str(error))
            raise
        job.update(ethereum_tx='0x'+done['tx'].removeprefix('0x'), ethereum_block=done['block'], error=None)
        stage(job, 'paid', 'USDC paid on Ethereum.')
        return True
    return False


def process(job):
    burn=job['burn_tx_id'];d=JOBS/burn
    if recover_payment(job, d):
        return
    history = {entry[1] for entry in job.get('history', [])} | {job.get('stage')}
    if job.get('journal_version') != 1:
        legacy_checkpoint = any((folder/'proof-report.json').exists() and
                                not (folder/'eth-result.json').exists()
                                for folder in d.glob('checkpoint-*'))
        if 'paying' in history or legacy_checkpoint:
            raise ReconciliationRequired('Legacy send stage has no complete transaction journal; reconcile against Ethereum.')
        job['journal_version'] = 1
        write_job(job)
    # An independently advanced verifier tip must never hide a failed or unresolved checkpoint.
    for folder in sorted(d.glob('checkpoint-*')):
        result = folder / 'eth-result.json'
        if result.exists() or intent_path(result).exists():
            stage(job, 'catching_up', 'Checking saved Ethereum checkpoint transaction.')
            if result.exists():
                read_eth_result('checkpoint', result)
            else:
                eth_send('checkpoint', folder, result)
    stage(job,'reading','Reading the finalized burn from all six validators.')
    bundle,A=export_bundle(d);blocks={b['header']['height']:b for b in A['blocks']['blocks']}
    red=[r for r in A['ledger']['vault_bridge_redemptions'] if r['withdrawal_packet']['burn_tx_id']==burn]
    if len(red)!=1:raise RuntimeError('This burn is not finalized on the network.')
    red=red[0];p=red['withdrawal_packet'];h=red['created_at_height']
    if p['source_chain_id']!=1 or p['vault_address'].lower()!='0xe7a432a28b20621a70e845c7f202004cf98e7a2c':raise RuntimeError('This burn is not for the Ethereum mainnet USDC vault.')
    job.update(amount_atoms=str(p['amount_atoms']),recipient=p['recipient'],burn_height=h,withdrawal_id=p['withdrawal_id']);write_job(job)
    six=read_six(burn);rs={json.dumps(x['receipts'],sort_keys=True) for x in six};assert len(six)==6 and len(rs)==1,'validators disagree on the burn receipt'
    receipt=[r for r in six[0]['receipts'] if r['tx_id']==burn];assert len(receipt)==1 and receipt[0]['accepted'],'burn receipt is not accepted'
    a=audit(d/'audit');v=a['contracts']['PFTLFinalityVerifierV1']['reads'];L=v['latestFinalizedHeight']
    if L >= h:
        L = historical_checkpoint(blocks, h, a)
    settle(job, d, a, A, blocks, L, h, receipt[0], p)
    bundle.unlink(missing_ok=True)


def settle(job, d, a, archive, blocks, L, h, receipt, packet):
    burn = job['burn_tx_id']
    n=0;total=max(0,-(-(h-L-MAX_STEP-1)//65))
    while h-L>MAX_STEP+1:
        n+=1;end=min(L+65,h-1);stage(job,'catching_up',f'Bringing the Ethereum checker up to date, step {n} of {total}.')
        w=native_witness(blocks, L, end)
        cd=d/f'checkpoint-{L}-{end}';ensure_directory(cd);save(cd/'witness.json',w)
        checkpoint_result = cd / 'eth-result.json'
        if checkpoint_result.exists():
            read_eth_result('checkpoint', checkpoint_result)
        else:
            prove('checkpoint',cd/'witness.json',cd,f'{burn[:16]}-cp-{end}')
            eth_send('checkpoint', cd, checkpoint_result)
        L=end
    stage(job,'proving','Building the withdrawal proof on the GPU.')
    w=native_witness(blocks, L, h, packet['withdrawal_id'])
    if (w['receipt'] != receipt or w['withdrawal_packet'] != packet or
            w['block']['header']['block_hash'] != blocks[h]['header']['block_hash']):
        raise ValueError('Native witness differs from the finalized withdrawal.')
    if w['route_profile']['profile']['route_epoch']!=a['contracts']['PFTLFinalityVerifierV1']['reads']['routeEpoch']:
        w=exit_profile.select(w,archive,a)
    save(d/'exit-witness.json',w)
    prove('withdrawal',d/'exit-witness.json',d/'proof',f'{burn[:16]}-wd')
    stage(job,'paying','Sending the payout transaction on Ethereum.')
    eth_send('withdraw',d/'proof',d/'eth-withdrawal.json')
    recover_payment(job,d)


def reconciliation_due(job, directory, startup):
    if job.get('stage') != 'reconciliation_required':
        return False
    elapsed = (datetime.now(timezone.utc) - datetime.fromisoformat(job['updated_at'])).total_seconds()
    if not startup and elapsed < 300:
        return False
    results = [directory/'eth-withdrawal.json',
               *(folder/'eth-result.json' for folder in directory.glob('checkpoint-*'))]
    return any(intent_path(result).exists() and not result.exists() for result in results)


def run_jobs(startup=False):
    for path in sorted(JOBS.glob('*/job.json'),key=lambda p:p.stat().st_mtime):
        try:
            job=load(path)
            if job.get('stage') not in ACTIVE_STAGES and not reconciliation_due(job, path.parent, startup):
                continue
            try:
                process(job)
            except Exception as error:
                detail = str(error)[:400]
                job['error'] = detail
                new_stage = 'reconciliation_required' if isinstance(error, ReconciliationRequired) else 'failed'
                if job.get('stage') != new_stage or job.get('detail') != detail:
                    stage(job, new_stage, detail)
        except (ValueError, OSError) as error:
            print(now(), path, error, file=sys.stderr, flush=True)


def worker():
    worker_started.set()
    try:
        startup = True
        while True:
            run_jobs(startup)
            startup = False
            queue_event.wait(15)
            queue_event.clear()
    finally:
        worker_started.clear()


class Handler(BaseHTTPRequestHandler):
    def log_message(self,*a):pass
    def reply(self,code,value):
        body=json.dumps(value).encode();self.send_response(code);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.send_header('Cache-Control','no-store');self.end_headers();self.wfile.write(body)
    def public(self,job):return {k:job.get(k) for k in ('burn_tx_id','stage','detail','amount_atoms','recipient','ethereum_tx','error','created_at','updated_at')}
    def do_GET(self):
        try:
            if self.path=='/api/bridge/withdrawals/readiness':return self.reply(200,readiness())
            m=re.fullmatch(r'/api/bridge/withdrawals/([0-9a-f]{96})',self.path)
            if m:
                job=read_job(m.group(1));return self.reply(200,dict(ok=True,**self.public(job))) if job else self.reply(404,dict(ok=False,code='withdrawal_not_found',message='No withdrawal job for this burn.'))
            self.reply(404,dict(ok=False,code='not_found'))
        except Exception as error:self.reply(503,dict(ok=False,ready=False,code='withdrawal_service_error',message=str(error)[:200]))
    def do_POST(self):
        try:
            if self.path!='/api/bridge/withdrawals':return self.reply(404,dict(ok=False,code='not_found'))
            body=json.loads(self.rfile.read(min(int(self.headers.get('Content-Length','0')),4096)) or b'{}');burn=body.get('burn_tx_id','')
            if not isinstance(burn,str) or not re.fullmatch(r'[0-9a-f]{96}',burn):return self.reply(400,dict(ok=False,code='invalid_withdrawal_request',message='Invalid burn transaction id.'))
            with lock:
                job=read_job(burn)
                if job is None or job.get('stage') in ('failed','reconciliation_required'):
                    job=dict(job or {},burn_tx_id=burn,created_at=(job or {}).get('created_at',now()),error=None)
                    stage(job,'queued','Waiting for the withdrawal worker.')
            queue_event.set();self.reply(200,dict(ok=True,**self.public(job)))
        except Exception as error:self.reply(503,dict(ok=False,code='withdrawal_service_error',message=str(error)[:200]))

if __name__=='__main__':
    ensure_directory(JOBS)
    threading.Thread(target=worker,daemon=True).start()
    print(now(),'withdrawal service on 127.0.0.1:%d'%PORT,flush=True);ThreadingHTTPServer(('127.0.0.1',PORT),Handler).serve_forever()
