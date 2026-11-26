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
from datetime import datetime,timedelta,timezone
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
from eth_finality import AwaitingFinality, TransactionUnknown, confirm_action
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


# A route failure (prover, SSH, Ethereum send) is retried after 5, 10 and 20 minutes.
# After the last attempt the job stays failed but no longer gates other burns.
ROUTE_RETRY_ATTEMPTS = 4
ROUTE_RETRY_BASE_SECONDS = 300
# Messages that IneligibleBurn carries, and carried before burn failures were scoped.
BURN_FAILURE_MESSAGES = ('This burn is not finalized on the network.',
                         'This burn is not for the Ethereum mainnet USDC vault.')


class IneligibleBurn(RuntimeError):
    """This request cannot proceed, independently of payout infrastructure health."""



def utcnow():return datetime.now(timezone.utc)
def now():return datetime.now(timezone.utc).isoformat(timespec='seconds')
def job_path(burn):return JOBS/burn/'job.json'
def classify_failure(job):
    """Give a failed row written before scoped retries the scope and retry its error implies."""
    if job.get('stage') == 'failed':
        if 'error_scope' not in job:
            job['error_scope'] = 'burn' if job.get('error') in BURN_FAILURE_MESSAGES else 'route'
        if job['error_scope'] == 'route' and not job.get('retry_exhausted') and not job.get('retry_at'):
            job.setdefault('route_attempts', 1)
            job['retry_at'] = job['updated_at']
    return job
def load_job(path):return classify_failure(load(path))
def read_job(burn):
    p=job_path(burn);return load_job(p) if p.exists() else None
def route_retry_pending(job):
    return job.get('stage') == 'failed' and job.get('error_scope') == 'route' and not job.get('retry_exhausted')
def route_retry_due(job):
    return route_retry_pending(job) and utcnow() >= datetime.fromisoformat(job['retry_at'])
def record_route_failure(job):
    attempts = job.get('route_attempts', 0) + 1
    job['route_attempts'] = attempts
    if attempts >= ROUTE_RETRY_ATTEMPTS:
        job.update(retry_exhausted=True, retry_at=None)
    else:
        delay = ROUTE_RETRY_BASE_SECONDS * 2 ** (attempts - 1)
        job.update(retry_exhausted=False, retry_at=(utcnow()+timedelta(seconds=delay)).isoformat(timespec='seconds'))
def write_job(job):
    with lock:
        p=job_path(job['burn_tx_id']);ensure_directory(p.parent);durable_json(p,job)
def stage(job,name,detail=''):
    job.update(stage=name,detail=detail,updated_at=now());job.setdefault('history',[]).append([now(),name,detail]);write_job(job);print(now(),job['burn_tx_id'][:12],name,detail,flush=True)
def restage(job,name,detail=''):
    # A repeated check of the same state refreshes its clock without growing the history.
    if job.get('stage')==name and job.get('detail')==detail:
        job['updated_at']=now();write_job(job)
    else:stage(job,name,detail)

def run(args,**kw):return subprocess.run([str(a) for a in args],check=True,capture_output=True,text=True,**kw)

def bridge_status():
    return call('vault_bridge_status',{'asset_id':FAMILY})
# An optional proving window. WITHDRAW_PROVING_WINDOW_FILE names the file the window's launcher
# writes while the window is open (`window.mode`, holding `deposits` or `none`; removed at the close).
# WITHDRAW_PROVING_WINDOW_HOURS gives its hours in UTC, `17:00-19:00`, for the next opening time and,
# without the file, for the window itself. Neither set: no window, as before.
WINDOW_MODES = ('deposits', 'none')
PROBE_SECONDS = 60
_probe = {}


def window_hours():
    hours = os.environ.get('WITHDRAW_PROVING_WINDOW_HOURS')
    m = re.fullmatch(r'([01]\d|2[0-3]):([0-5]\d)-([01]\d|2[0-3]):([0-5]\d)', hours or '')
    return (int(m[1]) * 60 + int(m[2]), int(m[3]) * 60 + int(m[4])) if m else None


def proving_window():
    """None without a configured window; else whether it is open and, when known, its next opening."""
    path, hours = os.environ.get('WITHDRAW_PROVING_WINDOW_FILE'), os.environ.get('WITHDRAW_PROVING_WINDOW_HOURS')
    if not path and not hours:
        return None
    now, span = utcnow(), window_hours()
    if path:
        try:
            is_open = Path(path).read_text().strip() in WINDOW_MODES
        except OSError:
            is_open = False
    else:
        minute = now.hour * 60 + now.minute
        is_open = span is not None and (span[0] <= minute < span[1] if span[0] <= span[1]
                                        else minute >= span[0] or minute < span[1])
    opens = None
    if not is_open and span is not None:
        start = now.replace(hour=span[0] // 60, minute=span[0] % 60, second=0, microsecond=0)
        opens = (start if start > now else start + timedelta(days=1)).strftime('%Y-%m-%dT%H:%M:%SZ')
    return dict(open=is_open, opens_at=opens)


def prover_answers():
    """A cheap probe of the proving machine (its proof host is there), at most once a minute."""
    if _probe.get('at', 0) + PROBE_SECONDS > time.time():
        return _probe['ok']
    try:
        ok = subprocess.run(['ssh', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=8'] + VAST
                            + [f'test -x {REMOTE}/proof-host'], capture_output=True, timeout=20).returncode == 0
    except (OSError, subprocess.SubprocessError):
        ok = False
    _probe.update(at=time.time(), ok=ok)
    return ok


def window_refusal(window):
    """Why payouts cannot be proved now, or None. Only with a configured window."""
    if window is None:
        return None
    if not window['open']:
        return 'Ethereum withdrawals are closed: proving window closed' + (
            f"; it next opens at {window['opens_at']}." if window['opens_at'] else '.')
    if not prover_answers():
        return 'Ethereum withdrawals are closed: the proving window is open but the prover does not answer.'
    return None


class WindowClosed(RuntimeError):
    """New proof work or a new Ethereum action was reached while the proving window is closed."""


def require_window(what):
    """The gate before new proof work and before a new Ethereum action (not a recovery)."""
    if window_closed():
        raise WindowClosed(f'Waiting for the next proving window before {what}.')


def window_closed():
    window = proving_window()
    return window is not None and not window['open']


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
    jobs = [load_job(path) for path in JOBS.glob('*/job.json')]
    blocked = [j['burn_tx_id'] for j in jobs if (j['stage'] == 'reconciliation_required' and j.get('error_scope') != 'burn')
               or route_retry_pending(j)]
    exhausted = [j['burn_tx_id'] for j in jobs if j['stage'] == 'failed' and j.get('retry_exhausted')]
    payments = [j['updated_at'] for j in jobs if j['stage'] == 'paid' and proven_paid(j)]
    awaiting = [j['burn_tx_id'] for j in jobs if j['stage'] == 'awaiting_finality' or
                (j['stage'] == 'paid' and not proven_paid(j))]
    stalled = [j['burn_tx_id'] for j in jobs if j['stage'] in ACTIVE_STAGES and
               (datetime.now(timezone.utc) - datetime.fromisoformat(j['updated_at'])).total_seconds() > 3600]
    window = proving_window()
    waiting = [j['burn_tx_id'] for j in jobs if j['stage'] == 'waiting_for_window']
    preflight_error = window_refusal(window) or payout_preflight()
    payout_ready = not (blocked or stalled or preflight_error)
    health = dict(ok=True, bucket_ready=len(buckets)==1, payout_ready=payout_ready,
                  payout_preflight_error=preflight_error, last_payment_recorded_at=max(payments, default=None),
                  blocked_jobs=blocked, stalled_jobs=stalled, awaiting_finality_jobs=awaiting, exhausted_jobs=exhausted)
    if window is not None:
        health.update(proving_window_open=window['open'], next_window_opens_at=window['opens_at'],
                      waiting_for_window_jobs=waiting)
    if len(buckets)!=1:return dict(**health,ready=False,message='No single active deposit bucket for the current bridge setting.')
    if not payout_ready:
        health['message'] = preflight_error or 'The withdrawal worker has blocked or stalled jobs.'
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
    require_window(f'the {kind} proof')
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
    # The sender authorizes a new transaction only when neither its receipt nor its journal exists;
    # that new action waits for the window. Recovering a journaled one always goes on.
    if not out.exists() and not intent_path(out).exists():
        require_window(f'a new Ethereum {mode} transaction')
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


def rebroadcast(mode, proof_dir, out):
    """Ask eth_send.py to resend the journaled bytes of a recorded transaction the chain no longer knows."""
    result = subprocess.run(
        [PY, str(HERE / 'eth_send.py'), mode, str(proof_dir), str(out)],
        capture_output=True, text=True, env=dict(os.environ, PYTHONDONTWRITEBYTECODE='1'),
    )
    if result.returncode:
        detail = (result.stderr.strip().splitlines() or ['unknown'])[-1][-300:]
        raise ReconciliationRequired(f'Reconcile dropped Ethereum {mode}: {detail}')


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


def operator_checkpoint(prior, target):
    # The checkpoint method is local-only on the validator. Use the same operator
    # SSH route as archive export and the binary currently running its RPC unit.
    command = """set -eu
pid=$(systemctl show -p MainPID --value postfiat-validator-0-rpc.service)
exe=$(readlink -e "/proc/$pid/exe")
case "$exe" in /opt/postfiat/releases/*/postfiat-node) ;; *) echo 'Unexpected validator executable' >&2; exit 1;; esac
exec runuser -u postfiat -- "$exe" pfusdc-checkpoint-witness --data-dir /var/lib/postfiat/validator-0 """
    command += shlex.join(['--prior-checkpoint', prior, '--target-block', target])
    return json.loads(run(['ssh', '-o','BatchMode=yes',arch.HOST,command], timeout=180).stdout)


def native_witness(blocks, prior, target, withdrawal_id=None):
    """Get a bounded burn over the public reader and an unframed native checkpoint over operator SSH."""
    prior_hash = blocks[prior]['header']['block_hash']
    header = blocks[target]['header']
    if withdrawal_id:
        parent = header['parent_hash']
        burn = call('pfusdc_egress_witness', {'prior_checkpoint':parent,'withdrawal_id':withdrawal_id})
        if (burn['prior_checkpoint_block_id'] != parent or burn.get('finality_ancestry')
                or burn['block']['header'] != header or burn['withdrawal_packet']['withdrawal_id'] != withdrawal_id):
            raise ValueError('Native burn witness differs from the finalized archive.')
        if parent == prior_hash:
            return burn
    segment = operator_checkpoint(prior_hash, header['block_hash'])
    steps = segment.get('finality_ancestry', [])
    if (segment['prior_checkpoint_block_id'] != prior_hash or segment['block']['header'] != header
            or [step['block']['header']['block_hash'] for step in steps] !=
               [blocks[h]['header']['block_hash'] for h in range(prior+1, target)]):
        raise ValueError('Native checkpoint witness differs from the finalized archive.')
    return dict(burn, prior_checkpoint_block_id=prior_hash, finality_ancestry=steps) if withdrawal_id else segment


def ethereum_reader():
    return Web3(Web3.HTTPProvider(RPC, request_kwargs={'timeout':20})).eth


def finalized_result(mode, out, expected, known_tx=None):
    cached = read_eth_result(mode, out) if out.exists() else {}
    tx = cached.get('tx') or known_tx
    if not tx or (known_tx and tx.removeprefix('0x').lower() != known_tx.removeprefix('0x').lower()):
        raise ReconciliationRequired('Missing or inconsistent Ethereum transaction id; reconcile the job.')
    try:
        finalized = confirm_action(ethereum_reader(), mode, tx, expected)
    except ReconciliationRequired:
        raise
    except Exception as error:
        raise AwaitingFinality('Cannot verify Ethereum finality for '+tx+': '+str(error)[:160]) from error
    result = dict(cached, mode=mode, status=1, tx=finalized['tx'], block=finalized['block'],
                  block_hash=finalized['block_hash'], finality=finalized)
    durable_json(out, result)
    return result


def withdrawal_terms(job, directory):
    fields = ('withdrawal_id', 'burn_tx_id', 'recipient', 'amount_atoms')
    terms = {key:job.get(key) for key in (*fields, 'packet_digest')}
    witness_path = directory/'exit-witness.json'
    if witness_path.exists():
        witness = load(witness_path)
        packet = witness['withdrawal_packet']
        for key in fields:
            if terms[key] is not None and str(terms[key]).lower() != str(packet[key]).lower():
                raise ReconciliationRequired('Job differs from retained withdrawal packet; reconcile '+key)
            terms[key] = packet[key]
        digest = witness.get('withdrawal_packet_evm_digest')
        if digest and terms['packet_digest'] and digest != terms['packet_digest']:
            raise ReconciliationRequired('Job differs from retained withdrawal packet digest; reconcile the burn.')
        terms['packet_digest'] = digest or terms['packet_digest']
    if any(value is None for value in terms.values()):
        raise ReconciliationRequired('Missing withdrawal packet terms; reconcile the retained burn evidence.')
    return terms


def finalized_checkpoint(directory):
    out = directory/'eth-result.json'
    read_eth_result('checkpoint', out)
    try:
        witness = load(directory/'witness.json')
        header = witness['block']['header']
        expected = dict(prior=witness['prior_checkpoint_block_id'], target=header['block_hash'], height=header['height'])
    except (OSError, ValueError, TypeError, KeyError) as error:
        raise ReconciliationRequired('Missing checkpoint witness; reconcile the retained Ethereum receipt.') from error
    return finalized_result('checkpoint', out, expected)


def was_paid(job):
    return job.get('reconcile_paid_only') or job.get('stage') == 'paid' or bool(job.get('ethereum_tx')) or any(
        entry[1] == 'paid' for entry in job.get('history', []))


def recover_payment(job, directory):
    result = directory / 'eth-withdrawal.json'
    historical = was_paid(job)
    if not (result.exists() or intent_path(result).exists() or historical):
        return False
    if historical and not job.get('reconcile_paid_only'):
        job['reconcile_paid_only'] = True
        write_job(job)
    try:
        known_tx = job.get('ethereum_tx')
        if result.exists():
            read_eth_result('withdraw', result)
        elif historical and intent_path(result).exists():
            intent = load(intent_path(result))
            if intent['mode'] != 'withdraw' or (known_tx and
                    known_tx.removeprefix('0x').lower() != intent['tx'].removeprefix('0x').lower()):
                raise ReconciliationRequired('Historical paid row differs from the signed withdrawal; reconcile the job.')
            known_tx = intent['tx']
        elif not historical:
            eth_send('withdraw', directory/'proof', result)
        # A legacy paid row can be recovered with just its original tx id and packet.
        # This is always read-only on Ethereum, even when the old receipt file is absent.
        terms = withdrawal_terms(job, directory)
        try:
            done = finalized_result('withdraw', result, terms, known_tx)
        except TransactionUnknown as unknown:
            if not historical and result.exists() and intent_path(result).exists():
                # A lagging RPC can also answer "unknown"; a refused resend keeps the job waiting.
                try:
                    rebroadcast('withdraw', directory/'proof', result)
                except ReconciliationRequired as refused:
                    raise AwaitingFinality(f'{unknown}; resend refused: {refused}'[:400]) from refused
            raise
    except (ReconciliationRequired, OSError, ValueError, TypeError, KeyError) as error:
        if not isinstance(error, ReconciliationRequired):
            error = ReconciliationRequired('Unusable withdrawal evidence; reconcile '+str(error)[:160])
        restage(job, 'awaiting_finality' if isinstance(error, AwaitingFinality) else 'reconciliation_required', str(error))
        raise error
    job.update(ethereum_tx=done['tx'], ethereum_block=done['block'], ethereum_finality=done['finality'], error=None)
    stage(job, 'paid', 'USDC payout finalized on Ethereum.')
    return True


def proven_paid(job):
    evidence = job.get('ethereum_finality') or {}
    return (evidence.get('chain_id') == 1 and evidence.get('block_hash')
            and evidence.get('expected', {}).get('burn_tx_id') == job['burn_tx_id'])


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
            if not result.exists():
                stage(job, 'catching_up', 'Recovering the saved Ethereum checkpoint transaction.')
                eth_send('checkpoint', folder, result)
            # A saved receipt is only checked: an unchanged wait refreshes the job, not its history.
            try:
                finalized_checkpoint(folder)
            except AwaitingFinality as error:
                restage(job, 'awaiting_finality', str(error)[:400])
                raise
    stage(job,'reading','Reading the finalized burn from all six validators.')
    bundle,A=export_bundle(d);blocks={b['header']['height']:b for b in A['blocks']['blocks']}
    red=[r for r in A['ledger']['vault_bridge_redemptions'] if r['withdrawal_packet']['burn_tx_id']==burn]
    if len(red)!=1:raise IneligibleBurn('This burn is not finalized on the network.')
    red=red[0];p=red['withdrawal_packet'];h=red['created_at_height']
    if p['source_chain_id']!=1 or p['vault_address'].lower()!='0xe7a432a28b20621a70e845c7f202004cf98e7a2c':raise IneligibleBurn('This burn is not for the Ethereum mainnet USDC vault.')
    job.update(amount_atoms=str(p['amount_atoms']),recipient=p['recipient'],burn_height=h,withdrawal_id=p['withdrawal_id'],packet_digest=red.get('withdrawal_packet_evm_digest'));write_job(job)
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
        if not checkpoint_result.exists():
            prove('checkpoint',cd/'witness.json',cd,f'{burn[:16]}-cp-{end}')
            eth_send('checkpoint', cd, checkpoint_result)
        finalized_checkpoint(cd)
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
    if job.get('stage') == 'paid':
        return not proven_paid(job)
    if job.get('stage') not in ('awaiting_finality', 'reconciliation_required'):
        return False
    elapsed = (datetime.now(timezone.utc) - datetime.fromisoformat(job['updated_at'])).total_seconds()
    if not startup and elapsed < 300:
        return False
    if job.get('stage') == 'awaiting_finality':
        return True
    results = [directory/'eth-withdrawal.json',
               *(folder/'eth-result.json' for folder in directory.glob('checkpoint-*'))]
    return any(intent_path(result).exists() and not result.exists() for result in results)


def has_ethereum_action(job, directory):
    """Whether this job has, or may have, an Ethereum action: a journal or result of a checkpoint
    or payout, a recorded transaction identity, a paid history, or a send history from before the
    journal (no journal_version). Such a job is never parked by the proving window."""
    if any(directory.glob('**/*.intent.json')) or any(directory.glob('**/eth-*.json')):
        return True
    if job.get('ethereum_tx') or job.get('reconcile_paid_only') or job.get('ethereum_finality'):
        return True
    stages = {entry[1] for entry in job.get('history', [])} | {job.get('stage')}
    if stages & {'paid', 'awaiting_finality', 'reconciliation_required'}:
        return True
    return 'paying' in stages and job.get('journal_version') != 1


def since_update(job):
    return (datetime.now(timezone.utc) - datetime.fromisoformat(job['updated_at'])).total_seconds()


def run_jobs(startup=False):
    for path in sorted(JOBS.glob('*/job.json'),key=lambda p:p.stat().st_mtime):
        try:
            stored=load(path);job=classify_failure(dict(stored))
            closed = window_closed()
            acting = closed and has_ethereum_action(job, path.parent)
            if job.get('stage') == 'waiting_for_window':
                # A parked job with an Ethereum action keeps its recovery, at the reconciliation pace.
                if closed and (not acting or (not startup and since_update(job) < 300)):
                    continue
            elif closed and not acting and (job.get('stage') in ACTIVE_STAGES or route_retry_due(job)):
                # Nothing of it was sent: unsent work waits for the next proving window, from any stage.
                restage(job, 'waiting_for_window', 'Waiting for the next proving window.')
                continue
            elif (job.get('stage') not in ACTIVE_STAGES and not reconciliation_due(job, path.parent, startup)
                    and not route_retry_due(job)):
                if job != stored:
                    write_job(job)
                continue
            try:
                process(job)
            except WindowClosed as error:
                job['error'] = None
                restage(job, 'waiting_for_window', str(error)[:400])
                continue
            except Exception as error:
                detail = str(error)[:400]
                job['error'] = detail
                job['error_scope'] = 'burn' if isinstance(error, IneligibleBurn) else 'route'
                new_stage = ('awaiting_finality' if isinstance(error, AwaitingFinality) else
                             'reconciliation_required' if isinstance(error, ReconciliationRequired) else 'failed')
                if (new_stage == 'failed' and job['error_scope'] == 'route' and window_closed()
                        and not has_ethereum_action(job, path.parent)):
                    # Cut by the window's close, or no prover outside the window: a wait, not a failure.
                    job['error'] = None
                    restage(job, 'waiting_for_window', 'Waiting for the next proving window: ' + detail)
                    continue
                if new_stage == 'failed' and job['error_scope'] == 'route':
                    record_route_failure(job)
                if job.get('stage') != new_stage or job.get('detail') != detail or new_stage == 'failed':
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
    def public(self,job):
        value = {k:job.get(k) for k in ('burn_tx_id','stage','detail','amount_atoms','recipient',
                 'ethereum_tx','ethereum_finality','error','created_at','updated_at')}
        if job.get('stage') == 'paid' and not proven_paid(job):
            value.update(stage='awaiting_finality', detail='Checking the historical payout against finalized Ethereum.')
        return value
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
                    for key in ('error_scope','route_attempts','retry_at','retry_exhausted'):job.pop(key,None)
                    stage(job,'queued','Waiting for the withdrawal worker.')
            queue_event.set();self.reply(200,dict(ok=True,**self.public(job)))
        except Exception as error:self.reply(503,dict(ok=False,code='withdrawal_service_error',message=str(error)[:200]))

if __name__=='__main__':
    ensure_directory(JOBS)
    threading.Thread(target=worker,daemon=True).start()
    print(now(),'withdrawal service on 127.0.0.1:%d'%PORT,flush=True);ThreadingHTTPServer(('127.0.0.1',PORT),Handler).serve_forever()
