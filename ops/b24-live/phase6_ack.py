#!/usr/bin/env python3
"""Acknowledge the real mint using six independently observed Ethereum votes."""
import fcntl,os,shlex,time
from concurrent.futures import ThreadPoolExecutor
from phase6_deploy import *
import live
from snapshot import export
PEERS=[('validator-0','64.176.220.75'),('validator-1','95.179.184.122'),('validator-2','66.42.48.39'),('validator-3','149.28.63.106'),('validator-4','95.179.179.206'),('validator-5','45.32.110.170')]
REMOTE_NODE='/opt/postfiat/releases/b23-release4-20261113/postfiat-node'
RELEASE_SHA='3039c21af67420a9842b5644eb580b2809081a97419e3e8e1f849d49a487cb6f'
def main():
    os.umask(0o077);mint=load(OUT/'mainnet-small-mint/result.json');assert mint['passed'];tx=next(r for r in mint['receipts'] if r['label']=='consumeMintOnly-one-B24');d=OUT/'destination-ack';d.mkdir(mode=0o700,exist_ok=False);remote='/tmp/b24-phase6-ack-'+tx['tx_hash'][2:18]
    w=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));receipt=w.eth.get_transaction_receipt(tx['tx_hash']);assert receipt.status==1 and receipt.blockNumber==tx['block'];o=load(OUT/'operator.json');logs=[(i,x) for i,x in enumerate(receipt.logs) if x['address'].lower()==o['controller'].lower()];assert len(logs)==1;index,event=logs[0];assert hx(event['topics'][0])==hx(Web3.keccak(text='PacketConsumed(bytes32,bytes32,bytes32,address,uint256)'))
    save(d/'mint-receipt.json',json.loads(w.to_json(receipt)));assert w.eth.block_number>=receipt.blockNumber+12
    def ssh(peer,args):
        n,h=peer;return subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=20','root@'+h,shlex.join(args)],capture_output=True,text=True)
    def node(peer,command,args,label):
        attempts=[]
        for attempt in range(12):
            r=ssh(peer,[REMOTE_NODE,command,'--data-dir','/var/lib/postfiat/'+peer[0],*args]);attempts.append(dict(attempt=attempt,returncode=r.returncode,stderr=r.stderr[-1200:]));save(d/(label+'-attempts.json'),attempts)
            if r.returncode==0:save(d/(label+'-report.json'),json.loads(r.stdout));return
            if not any(x in r.stderr for x in ('Database already open. Cannot acquire lock.','Resource temporarily unavailable')):r.check_returncode()
            time.sleep(2)
        raise RuntimeError(label+' remained busy; no chain submission')
    def down(peer,name,target):subprocess.run(['scp','-q','root@'+peer[1]+':'+remote+'/'+name,str(target)],check=True)
    def up(peer,source,name):subprocess.run(['scp','-q',str(source),'root@'+peer[1]+':'+remote+'/'+name],check=True)
    for peer in PEERS:
        r=ssh(peer,['sha256sum',REMOTE_NODE]);r.check_returncode();assert r.stdout.split()[0]==RELEASE_SHA
        r=ssh(peer,['mkdir','-m','700',remote]);r.check_returncode()
    # Observing and constructing the MPT proof are read-only; voting only updates
    # each validator's guarded checkpoint signing journal. No key leaves its host.
    node(PEERS[0],'ethereum-checkpoint-observe',['--route-id',ROUTE,'--ethereum-rpc','http://127.0.0.1:28701','--block-number',str(tx['block']),'--checkpoint-file',remote+'/checkpoint.json'],'observe');down(PEERS[0],'checkpoint.json',d/'checkpoint.json');checkpoint=load(d/'checkpoint.json');assert checkpoint['block_number']==tx['block'] and checkpoint['route_id']==ROUTE and checkpoint['observed_head_number']>=tx['block']+12
    node(PEERS[0],'ethereum-receipt-proof-build',['--route-id',ROUTE,'--ethereum-rpc','http://127.0.0.1:28701','--transaction-hash',tx['tx_hash'],'--proof-file',remote+'/receipt-proof.json'],'receipt-proof');down(PEERS[0],'receipt-proof.json',d/'receipt-proof.json')
    def vote(peer):
        if peer!=PEERS[0]:up(peer,d/'checkpoint.json','checkpoint.json')
        n=peer[0];node(peer,'ethereum-checkpoint-vote-sign',['--checkpoint-file',remote+'/checkpoint.json','--ethereum-rpc','http://127.0.0.1:28701','--validator',n,'--validator-key-file','/var/lib/postfiat/'+n+'/validator_keys.json','--vote-file',remote+'/'+n+'.vote.json'],n);down(peer,n+'.vote.json',d/(n+'.vote.json'));return n
    with ThreadPoolExecutor(max_workers=6) as executor:validators=list(executor.map(vote,PEERS))
    assert len(validators)==6
    for peer in PEERS[1:]:up(PEERS[0],d/(peer[0]+'.vote.json'),peer[0]+'.vote.json')
    node(PEERS[0],'ethereum-checkpoint-certificate-assemble',['--checkpoint-file',remote+'/checkpoint.json','--vote-files',','.join(remote+'/'+n+'.vote.json' for n in validators),'--certificate-file',remote+'/certificate.json'],'certificate');down(PEERS[0],'certificate.json',d/'certificate.json')
    proof=load(d/'receipt-proof.json');assert proof['transaction_hash'].lower()==tx['tx_hash'].lower() and proof['block_number']==tx['block'];op=dict(operation='pftl_uniswap_destination_consume',operator=ISSUER,route_id=ROUTE,packet_hash=load(OUT/'real-export/result.json')['packet_hash'],ethereum_consume_tx_hash=tx['tx_hash'][2:],consumed_height=tx['block'],finalized_height=checkpoint['observed_head_number'],external_event_proof=dict(checkpoint_certificate=load(d/'certificate.json'),receipt_proof=proof['proof'],log_index=index));save(d/'operation.json',op)
    with live.LOCK.open('a+') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB);lock.seek(0);assert not lock.read().strip();lock.seek(0);lock.truncate();lock.write('TEE\n');lock.flush()
        try:
            before=d/'before.json';s=export(before);r0=item(s['ledger']['pftl_uniswap_routes'],'route_id',ROUTE);assert r0['outstanding_bridge_claims_atoms']==1000000
            last,receipts=live.run_operations([dict(label='ack-real-mainnet-mint',source=ISSUER,kind='asset',operation=op)],before,d/'live');r=item(load(last)['ledger']['pftl_uniswap_routes'],'route_id',ROUTE);assert invariant(r) and r['outstanding_bridge_claims_atoms']==0 and r['ethereum_spendable_supply_atoms']==1000000 and r['authorized_valid_supply_atoms']==r0['authorized_valid_supply_atoms']
            save(d/'result.json',dict(passed=True,six_independent_Ethereum_votes=True,real_receipt_MPT_proof=True,receipts=receipts,final_snapshot=str(last),outstanding_bridge_claims_atoms=0,ethereum_spendable_supply_atoms=1000000,invariant=True))
        finally:
            lock.seek(0)
            if lock.read().strip()=='TEE':lock.seek(0);lock.truncate();lock.flush()
if __name__=='__main__':main()
