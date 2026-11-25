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
PY=sys.executable
COORD=Path('/home/postfiat/repos/b23-coord');E=COORD/'evidence/withdraw';JOBS=E/'jobs'
HOST_BIN=E/'bin/proof-host';ELF=E/'historical/egress.elf'
FAMILY='02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b'
SERIES='541e451e6ae96b6680988655571a0feec59c7f1d073c9b5d29547254cab38a3be540f8767446d25baebc7081ce97aef7'
VAST=['-p','20628','root@ssh8.vast.ai'];REMOTE='/workspace/b23-withdraw-20261116'
PORT=18084;MAX_STEP=64
queue_event=threading.Event();lock=threading.Lock()

def now():return datetime.now(timezone.utc).isoformat(timespec='seconds')
def job_path(burn):return JOBS/burn/'job.json'
def read_job(burn):
    p=job_path(burn);return load(p) if p.exists() else None
def write_job(job):
    with lock:
        p=job_path(job['burn_tx_id']);p.parent.mkdir(parents=True,exist_ok=True);tmp=p.with_suffix('.tmp');tmp.write_text(json.dumps(job,indent=1));tmp.replace(p)
def stage(job,name,detail=''):
    job.update(stage=name,detail=detail,updated_at=now());job.setdefault('history',[]).append([now(),name,detail]);write_job(job);print(now(),job['burn_tx_id'][:12],name,detail,flush=True)

def run(args,**kw):return subprocess.run([str(a) for a in args],check=True,capture_output=True,text=True,**kw)

def bridge_status():
    return call('vault_bridge_status',{'asset_id':FAMILY})
def readiness():
    s=bridge_status();route=call('vault_bridge_route',{'asset_id':FAMILY})
    profile_hash=route['beacon_finality_anchor']['route_profile_hash']
    buckets=[b for b in s['buckets'] if b['policy_hash']==profile_hash and b.get('status','active')=='active']
    if len(buckets)!=1:return dict(ok=True,ready=False,message='No single active deposit bucket for the current bridge setting.')
    return dict(ok=True,ready=True,asset_id=FAMILY,series_asset_id=SERIES,bucket_id=buckets[0]['bucket_id'],issuer=s['issuer'],epoch=s['finalized_epoch'],
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
def read_eth_result(mode, out):
    """Only a mined, successful receipt for the requested action confirms the job."""
    try:
        receipt = load(out)
    except (OSError, ValueError) as error:
        raise RuntimeError(f'Cannot read Ethereum {mode} receipt in {out.name}; reconcile before retry.') from error
    if (not isinstance(receipt, dict) or receipt.get('mode') != mode
            or type(receipt.get('status')) is not int or receipt['status'] != 1
            or not isinstance(receipt.get('tx'), str)
            or not re.fullmatch(r'(?:0x)?[a-fA-F0-9]{64}', receipt['tx'])
            or type(receipt.get('block')) is not int or receipt['block'] < 0):
        raise RuntimeError(f'No successful Ethereum {mode} receipt in {out.name}; reconcile before retry.')
    return receipt


def eth_send(mode, proof_dir, out):
    result = subprocess.run(
        [PY, str(HERE / 'eth_send.py'), mode, str(proof_dir), str(out)],
        capture_output=True, text=True, env=dict(os.environ, PYTHONDONTWRITEBYTECODE='1'),
    )
    if result.returncode:
        detail = (result.stderr.strip().splitlines() or ['unknown'])[-1][-300:]
        raise RuntimeError('Ethereum transaction failed: ' + detail)
    return read_eth_result(mode, out)


def process(job):
    burn=job['burn_tx_id'];d=JOBS/burn
    stage(job,'reading','Reading the finalized burn from all six validators.')
    bundle,A=export_bundle(d);blocks={b['header']['height']:b for b in A['blocks']['blocks']}
    red=[r for r in A['ledger']['vault_bridge_redemptions'] if r['withdrawal_packet']['burn_tx_id']==burn]
    if len(red)!=1:raise RuntimeError('This burn is not finalized on the network.')
    red=red[0];p=red['withdrawal_packet'];h=red['created_at_height']
    if p['source_chain_id']!=1 or p['vault_address'].lower()!='0xe7a432a28b20621a70e845c7f202004cf98e7a2c':raise RuntimeError('This burn is not for the Ethereum mainnet USDC vault.')
    job.update(amount_atoms=str(p['amount_atoms']),recipient=p['recipient'],burn_height=h,withdrawal_id=p['withdrawal_id']);write_job(job)
    six=read_six(burn);rs={json.dumps(x['receipts'],sort_keys=True) for x in six};assert len(six)==6 and len(rs)==1,'validators disagree on the burn receipt'
    receipt=[r for r in six[0]['receipts'] if r['tx_id']==burn];assert len(receipt)==1 and receipt[0]['accepted'],'burn receipt is not accepted'
    if len(blocks[h]['receipt_ids'])!=1:raise RuntimeError('Burn shares its block with other transactions; this service handles single-burn blocks only for now.')
    a=audit(d/'audit');v=a['contracts']['PFTLFinalityVerifierV1']['reads'];L=v['latestFinalizedHeight']
    withdrawal_result = d / 'eth-withdrawal.json'
    if withdrawal_result.exists():
        done = read_eth_result('withdraw', withdrawal_result)
        job.update(ethereum_tx='0x' + done['tx'].removeprefix('0x'), ethereum_block=done['block'])
        stage(job, 'paid', 'USDC paid on Ethereum.')
        return
    c=[dict(r,active=r.get('active',True)) for r in A['validator_registry']['validators']]
    step=lambda b:dict(block=b,committee_epoch=b['header']['consensus_v2_commit']['proposal']['domain']['committee_epoch'],committee=c)
    if L>=h:raise RuntimeError('Ethereum checker is already past this burn; it needs manual handling.')
    n=0;total=max(0,-(-(h-L-MAX_STEP)//65))
    while h-L>MAX_STEP:
        n+=1;end=min(L+65,h-1);stage(job,'catching_up',f'Bringing the Ethereum checker up to date, step {n} of {total}.')
        w=dict(schema='postfiat.pfusdc.checkpoint_proof_witness.v1',chain_id=A['tip']['chain_id'],genesis_hash=A['tip']['genesis_hash'],protocol_version=A['tip']['protocol_version'],prior_checkpoint_block_id=blocks[L]['header']['block_hash'],finality_ancestry=[step(blocks[x]) for x in range(L+1,end)],**step(blocks[end]))
        cd=d/f'checkpoint-{L}-{end}';cd.mkdir(exist_ok=True);save(cd/'witness.json',w)
        checkpoint_result = cd / 'eth-result.json'
        if checkpoint_result.exists():
            read_eth_result('checkpoint', checkpoint_result)
        else:
            prove('checkpoint',cd/'witness.json',cd,f'{burn[:16]}-cp-{end}')
            eth_send('checkpoint', cd, checkpoint_result)
        L=end
    stage(job,'proving','Building the withdrawal proof on the GPU.')
    bucket=[b for b in A['ledger']['vault_bridge_bucket_states'] if b['bucket_id']==red['bucket_id']][0]
    prof=[r for r in A['governance']['vault_bridge_route_profiles'] if r['profile_hash']==bucket['policy_hash']];assert len(prof)==1
    leaf=dict(schema='postfiat.bridge_exit_leaf.v1',accepted_receipt_code='accepted',accepted_receipt_id=burn,amount_atoms=p['amount_atoms'],asset_id=p['vault_bridge_asset_id'],burn_tx_id=burn,destination_hash=p['destination_hash'],evidence_root=p['evidence_root'],finalized_height=p['finalized_height'],recipient=p['recipient'],route_epoch=prof[0]['profile']['route_epoch'],source_bucket_id=p['source_bucket_id'],withdrawal_id=p['withdrawal_id'],withdrawal_packet_evm_digest=red['withdrawal_packet_evm_digest'],withdrawal_packet_hash=red['withdrawal_packet_hash'])
    w=dict(schema='postfiat.pfusdc.egress_proof_witness.v1',chain_id=A['tip']['chain_id'],genesis_hash=A['tip']['genesis_hash'],protocol_version=A['tip']['protocol_version'],bridge_exit_root_activation_height=1,prior_checkpoint_block_id=blocks[L]['header']['block_hash'],finality_ancestry=[step(blocks[x]) for x in range(L+1,h)],route_profile=prof[0],receipt=receipt[0],merkle_proof=dict(leaf=leaf,leaf_count=1,leaf_index=0,siblings=[]),withdrawal_packet=p,withdrawal_packet_evm_digest=red['withdrawal_packet_evm_digest'],withdrawal_packet_hash=red['withdrawal_packet_hash'],**step(blocks[h]))
    a=audit(d/'audit')
    if prof[0]['profile']['route_epoch']!=a['contracts']['PFTLFinalityVerifierV1']['reads']['routeEpoch']:w=exit_profile.select(w,A,a)
    save(d/'exit-witness.json',w);bundle.unlink(missing_ok=True)
    prove('withdrawal',d/'exit-witness.json',d/'proof',f'{burn[:16]}-wd')
    stage(job,'paying','Sending the payout transaction on Ethereum.')
    done=eth_send('withdraw',d/'proof',withdrawal_result)
    job.update(ethereum_tx='0x'+done['tx'].removeprefix('0x'),ethereum_block=done['block']);stage(job,'paid','USDC paid on Ethereum.')

def worker():
    while True:
        queue_event.wait(15);queue_event.clear()
        for path in sorted(JOBS.glob('*/job.json'),key=lambda p:p.stat().st_mtime):
            job=load(path)
            if job.get('stage') not in ('queued',):continue
            try:process(job)
            except Exception as error:
                job['error']=str(error)[:400];stage(job,'failed',str(error)[:400])

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
            job=read_job(burn)
            if job is None or job.get('stage')=='failed':
                job=dict(job or {},burn_tx_id=burn,created_at=(job or {}).get('created_at',now()),error=None);stage(job,'queued','Waiting for the withdrawal worker.')
            queue_event.set();self.reply(200,dict(ok=True,**self.public(job)))
        except Exception as error:self.reply(503,dict(ok=False,code='withdrawal_service_error',message=str(error)[:200]))

if __name__=='__main__':
    JOBS.mkdir(parents=True,exist_ok=True)
    threading.Thread(target=worker,daemon=True).start()
    print(now(),'withdrawal service on 127.0.0.1:%d'%PORT,flush=True);ThreadingHTTPServer(('127.0.0.1',PORT),Handler).serve_forever()
