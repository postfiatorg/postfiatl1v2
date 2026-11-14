#!/usr/bin/env python3
"""Capture authentic public history read-only and run V2 witness export offline."""
import argparse,os,shlex,time,uuid
from sign import *
from live import read_six,points
from checkpoint import ELF,PROVER
OUT=COORD/'evidence/tee/phase6'
ARCHIVE=CANARY.parent/'b24-readonly-archive'
OFFLINE_NODE=Path('/mnt/HC_Volume_106212907/stakehub-hyperliquid/l1-security-target-20261031/release/postfiat-node')
HOST='root@64.176.220.75'
def capture(directory):
    directory.mkdir(mode=0o700,parents=True,exist_ok=False);before=read_six();save(directory/'before-six.json',before);assert len(points(before))==1
    digest=hashlib.sha256(ARCHIVE.read_bytes()).hexdigest();remote='/tmp/b24-readonly-archive-'+digest[:16]
    check=subprocess.run(['ssh','-o','BatchMode=yes',HOST,'sha256sum '+shlex.quote(remote)],capture_output=True,text=True)
    if check.returncode:
        subprocess.run(['scp','-q',str(ARCHIVE),HOST+':'+remote],check=True)
        check=subprocess.run(['ssh','-o','BatchMode=yes',HOST,'sha256sum '+shlex.quote(remote)],capture_output=True,text=True,check=True)
    assert check.stdout.split()[0]==digest
    target='/tmp/b24-public-archive-'+uuid.uuid4().hex+'.json';attempts=[]
    for n in range(20):
        result=subprocess.run(['ssh','-o','BatchMode=yes',HOST,shlex.join([remote,'export','--data-dir','/var/lib/postfiat/validator-0','--output',target])],capture_output=True,text=True)
        attempts.append(dict(attempt=n,returncode=result.returncode,stderr=result.stderr[-2000:]));save(directory/'export-attempts.json',attempts)
        if result.returncode==0:break
        if 'Database already open. Cannot acquire lock.' not in result.stderr:result.check_returncode()
        time.sleep(2)
    else:raise RuntimeError('Read-only archive exporter remained busy')
    bundle=directory/'public-archive.json';subprocess.run(['scp','-q',HOST+':'+target,str(bundle)],check=True)
    remote_hash=subprocess.check_output(['ssh','-o','BatchMode=yes',HOST,'sha256sum '+shlex.quote(target)],text=True).split()[0];assert remote_hash==hashlib.sha256(bundle.read_bytes()).hexdigest()
    after=read_six();save(directory/'after-six.json',after);v=load(bundle);assert points(after)==points(before) and v['tip']['height']==after[0]['server']['ledger']['height'] and v['tip']['state_root']==after[0]['server']['ledger']['state_root']
    offline=directory/'offline-store';env=dict(os.environ,B24_OFFLINE_ARCHIVE_IMPORT='1')
    r=subprocess.run([str(ARCHIVE),'import','--bundle',str(bundle),'--output',str(offline)],capture_output=True,text=True,check=True,env=env);save(directory/'offline-import.json',json.loads(r.stdout))
    save(directory/'provenance.json',dict(public_bundle_sha256=remote_hash,exporter_sha256=digest,offline_node_sha256=hashlib.sha256(OFFLINE_NODE.read_bytes()).hexdigest(),source_node='validator-0',live_read_only=True,six_validators_match=True,tip=v['tip'],validator_private_keys_copied=False,source_integrity_key_copied=False,offline_integrity_key_generated_locally=True))
    return offline,v
def checkpoint_check(directory):
    offline,v=capture(directory);tip=v['tip'];prior=next(b for b in v['blocks']['blocks'] if b['header']['height']==tip['height']-1)['header']['block_hash'];w=directory/'checkpoint-witness-v2.json'
    with w.open('w') as out:subprocess.run([str(OFFLINE_NODE),'pftl-uniswap-checkpoint-witness-v2','--data-dir',str(offline),'--target-block',tip['block_hash'],'--prior-checkpoint',prior],stdout=out,check=True)
    with (directory/'checkpoint-execute.log').open('w') as log:subprocess.run([str(PROVER),'checkpoint','--witness',str(w),'--elf',str(ELF),'--proof-program-version','2','--output-dir',str(directory/'checkpoint-execute')],stdout=log,stderr=subprocess.STDOUT,check=True)
    values=(directory/'checkpoint-execute/public-values.bin').read_bytes();assert values==(OUT/'checkpoint-execute/public-values.bin').read_bytes()
    save(directory/'result.json',dict(passed=True,real_archive_V2_checkpoint_export=True,public_values_match_authenticated_live_checkpoint=True,height=tip['height'],real_receipt_export_not_yet_executed=True,live_transactions=0,GPU_work=0));print(json.dumps(load(directory/'result.json')),flush=True)
def receipt(directory,packet_hash):
    offline,v=capture(directory);pins=load(OUT/'checkpoint-pins.json');assert v['tip']['height']-pins['initial_height']<=64
    witness=directory/'receipt-witness-v2.json'
    with witness.open('w') as out:subprocess.run([str(OFFLINE_NODE),'pftl-uniswap-receipt-witness-v2','--data-dir',str(offline),'--packet-hash',packet_hash,'--prior-checkpoint',pins['initial_checkpoint_block_id']],stdout=out,check=True)
    with (directory/'receipt-execute.log').open('w') as log:subprocess.run([str(PROVER),'receipt','--witness',str(witness),'--elf',str(ELF),'--proof-program-version','2','--output-dir',str(directory/'receipt-execute')],stdout=log,stderr=subprocess.STDOUT,check=True)
    save(directory/'result.json',dict(passed=True,real_archive_V2_receipt_export=True,real_live_finality=True,synthetic_committee=False,packet_hash=packet_hash,height=v['tip']['height'],public_values_sha256=hashlib.sha256((directory/'receipt-execute/public-values.bin').read_bytes()).hexdigest(),groth16_proof_generated=False));print(json.dumps(load(directory/'result.json')),flush=True)
if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('--checkpoint-check',action='store_true');ap.add_argument('--packet-hash');ap.add_argument('--output',type=Path,required=True);a=ap.parse_args();os.umask(0o077)
    if a.checkpoint_check:assert not a.packet_hash;checkpoint_check(a.output)
    else:assert a.packet_hash;receipt(a.output,a.packet_hash)
