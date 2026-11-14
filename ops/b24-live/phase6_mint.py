#!/usr/bin/env python3
"""Fork-qualify then relay exactly the real one-B24 proof and re-pause minting."""
import argparse,copy,math,os,socket,stat,time
from datetime import datetime,timezone
from phase6_deploy import *

def inputs():
    q=load(OUT/'real-receipt-proof/qualification.json');assert q['passed'] and q['real_live_export']
    witness=load(OUT/'real-export/archive/receipt-witness-v2.json')['witness'];p=witness['mint_packet'];e=load(OUT/'deployment-expected.json')
    values=(OUT/'real-receipt-proof/public-values.bin').read_bytes();proof=(OUT/'real-receipt-proof/proof-calldata.bin').read_bytes()
    assert len(values)==1120 and len(proof)==356 and proof[:4].hex()=='4388a21c'
    assert p['mint_amount_atoms']==1000000 and p['source_packet_hash']==load(OUT/'real-export/result.json')['packet_hash'] and p['ethereum_recipient'].lower()==e['operator']['address'].lower()
    assert p['destination_controller'].lower()==e['operator']['controller'].lower() and p['wrapped_token'].lower()==e['operator']['token'].lower()
    component=next(x for x in artifact('PFTLUniswapPrimaryMarketV3')['abi'] if x.get('name')=='consumeMintOnly')['inputs'][0]['components']
    import re
    packet=[]
    for x in component:
        key=re.sub(r'(?<!^)(?=[A-Z])','_',x['name']).lower();v=p['deadline_seconds' if key=='deadline' else key]
        packet.append(bytes.fromhex(v) if x['type'].startswith('bytes') else Web3.to_checksum_address(v) if x['type']=='address' else v)
    return e,p,packet,values,proof

def checks(w,e,p,packet,values,stage):
    o=e['operator'];t=contract(w,'WrappedVenueNAVCoin',o['token']);v=contract(w,'PFTLReceiptFinalityVerifierV2',o['verifier']);c=contract(w,'PFTLUniswapPrimaryMarketV3',o['controller']);amount=1000000 if stage>=2 else 0
    expected=copy.deepcopy(e);expected['controller'].update(totalMintedAtoms=amount,outstandingMintedAtoms=amount,mintPaused=stage not in (1,2))
    expected['verifier'].update(latestCheckpointCommitment=hx(values[32*32:33*32]),latestFinalizedHeight=int.from_bytes(values[33*32:34*32],'big'))
    for name,con in [('token',t),('verifier',v),('controller',c)]:
        assert hx(Web3.keccak(w.eth.get_code(con.address)))==e['runtime'][name+'_runtime_code_hash']
        fields=dict(owner=o['address'],controller=o['controller'],controller_locked=True,totalSupply=amount,decimals=6) if name=='token' else expected[name]
        for field,wanted in fields.items():assert norm(getattr(con.functions,field)().call())==norm(wanted),name+'.'+field
    digest=c.functions.packetDigest(packet).call();accepted=v.functions.isReceiptAccepted(bytes.fromhex(p['source_receipt_root']),bytes.fromhex(p['source_receipt_hash']),bytes.fromhex(p['route_config_digest']),Web3.keccak(text='TRUSTLESS_FINALITY'),digest).call()
    assert accepted and t.functions.balanceOf(o['address']).call()==amount and c.functions.consumedPacket(digest).call()==(stage>=2)
    assert v.functions.acceptedCheckpointCommitment(values[32*32:33*32]).call() and v.functions.consumedProofNullifier(values[34*32:35*32]).call()
    assert c.functions.returnPaused().call() and c.functions.totalReturnBurnedAtoms().call()==0
    return dict(passed=True,supply=amount,operator_wrapper_balance=amount,mint_paused=c.functions.mintPaused().call(),return_paused=True,receipt_accepted=accepted,packet_consumed=stage>=2,packet_digest=hx(digest),latest_finalized_height=v.functions.latestFinalizedHeight().call(),ownership_retained=True)

def functions(w,e,packet,values,proof):
    v=contract(w,'PFTLReceiptFinalityVerifierV2',e['operator']['verifier']);c=contract(w,'PFTLUniswapPrimaryMarketV3',e['operator']['controller'])
    return [('verifyAndAccept-real-export',v.functions.verifyAndAccept(values,proof)),('unpause-test-mint',c.functions.setMintPaused(False)),('consumeMintOnly-one-B24',c.functions.consumeMintOnly(packet)),('restore-mint-paused',c.functions.setMintPaused(True))]

def qualify():
    e,p,packet,values,proof=inputs();o=e['operator'];d=OUT/'mint-qualification';d.mkdir(mode=0o700,exist_ok=False);w0=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));block=w0.eth.block_number;nonce=w0.eth.get_transaction_count(o['address'],'pending');priority=w0.eth.max_priority_fee;fee=max(2*w0.eth.get_block(block)['baseFeePerGas']+priority,2000000000)
    assert int(time.time())+3600<p['deadline_seconds'];verify(w0,e,5)
    with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
    with (d/'anvil.log').open('w') as log:
        proc=subprocess.Popen(['anvil','--silent','--host','127.0.0.1','--port',str(port),'--chain-id','1','--fork-url',PUBLIC_RPC,'--fork-block-number',str(block)],stdout=log,stderr=log)
        try:
            w=Web3(Web3.HTTPProvider(f'http://127.0.0.1:{port}',request_kwargs={'timeout':60}))
            for _ in range(120):
                if w.is_connected():break
                time.sleep(.25)
            guard(w);w.provider.make_request('anvil_impersonateAccount',[o['address']]);verify(w,e,5);rows=[];negative=[]
            v=contract(w,'PFTLReceiptFinalityVerifierV2',o['verifier']);c=contract(w,'PFTLUniswapPrimaryMarketV3',o['controller'])
            def reject(fn,label):
                try:fn.call({'from':o['address']})
                except Exception as error:
                    assert type(error).__name__ in ('ContractLogicError','ContractCustomError','ContractPanicError'),'Transport error is not rejection';negative.append(label);return
                raise AssertionError('Expected rejection '+label)
            bad=bytearray(proof);bad[-1]^=1;reject(v.functions.verifyAndAccept(values,bytes(bad)),'bad-proof');reject(c.functions.consumeMintOnly(packet),'paused')
            for i,(label,fn) in enumerate(functions(w,e,packet,values,proof)):
                tx=fn.build_transaction({'from':o['address'],'chainId':1,'nonce':nonce+i,'value':0,'maxFeePerGas':fee,'maxPriorityFeePerGas':priority});tx['gas']=math.ceil(w.eth.estimate_gas(tx)*1.3)
                assert w.eth.get_balance(o['address'])>=tx['gas']*fee;receipt=w.eth.wait_for_transaction_receipt(w.eth.send_transaction(tx));assert receipt.status==1;save(d/f'{i}-checks.json',checks(w,e,p,packet,values,i));rows.append(dict(label=label,transaction=tx,gas_used=receipt.gasUsed))
                if i==0:reject(v.functions.verifyAndAccept(values,proof),'proof-replay')
                if i==1:
                    altered=list(packet);altered[-2]+=1;reject(c.functions.consumeMintOnly(altered),'altered-amount')
                if i==2:reject(c.functions.consumeMintOnly(packet),'mint-replay')
            remaining_allowance=3000000;maximum=(sum(x['transaction']['gas'] for x in rows)+remaining_allowance)*fee;assert w0.eth.get_balance(o['address'])>=maximum,'ETH must cover test plus future launch allowance'
            save(OUT/'mint-unsigned.json',dict(block=block,nonce=nonce,transactions=rows,real_proof_sha256=hashlib.sha256(proof).hexdigest(),public_values_sha256=hashlib.sha256(values).hexdigest()))
            save(d/'result.json',dict(passed=True,fork_block=block,real_proof_verified=True,mainnet_transactions=0,negative_checks=negative,test_maximum_cost_wei=sum(x['transaction']['gas'] for x in rows)*fee,future_allowance_gas=remaining_allowance,max_fee_wei=fee,combined_maximum_wei=maximum,unsigned_sha256=hashlib.sha256((OUT/'mint-unsigned.json').read_bytes()).hexdigest(),final=checks(w,e,p,packet,values,3)))
        finally:proc.terminate();proc.wait(timeout=10)

def execute():
    e,p,packet,values,proof=inputs();q=load(OUT/'mint-qualification/result.json');assert q['passed'];assert hashlib.sha256((OUT/'mint-unsigned.json').read_bytes()).hexdigest()==q['unsigned_sha256'];data=load(OUT/'mint-unsigned.json');assert hashlib.sha256(proof).hexdigest()==data['real_proof_sha256'] and hashlib.sha256(values).hexdigest()==data['public_values_sha256'];d=OUT/'mainnet-small-mint';d.mkdir(mode=0o700,exist_ok=False)
    w=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));assert w.eth.chain_id==1;verify(w,e,5)
    assert not KEY.is_symlink() and stat.S_IMODE(KEY.stat().st_mode)==0o600 and stat.S_IMODE(KEY.parent.stat().st_mode)==0o700;account=Account.from_key(KEY.read_text().strip());assert account.address==e['operator']['address'];rows=[]
    for i,row in enumerate(data['transactions']):
        tx=dict(row['transaction']);assert w.eth.get_transaction_count(account.address,'pending')==tx['nonce'];assert int(time.time())+1800<p['deadline_seconds'];assert w.eth.get_block('latest')['baseFeePerGas']+tx['maxPriorityFeePerGas']<=tx['maxFeePerGas']
        assert w.eth.get_balance(account.address)>=sum(x['transaction']['gas']*x['transaction']['maxFeePerGas'] for x in data['transactions'][i:])+3000000*tx['maxFeePerGas']
        tx.pop('from');signed=account.sign_transaction(tx);save(d/f'{i}-submission.json',dict(label=row['label'],expected_hash=hx(signed.hash),transaction=tx));h=w.eth.send_raw_transaction(signed.raw_transaction);assert h==signed.hash;receipt=w.eth.wait_for_transaction_receipt(h,timeout=600);save(d/f'{i}-receipt.json',json.loads(w.to_json(receipt)));assert receipt.status==1;confirmed(w,h,receipt);save(d/f'{i}-checks.json',checks(w,e,p,packet,values,i))
        rows.append(dict(label=row['label'],tx_hash=hx(h),block=receipt.blockNumber,gas_used=receipt.gasUsed,effective_gas_price=receipt.effectiveGasPrice,cost_wei=receipt.gasUsed*receipt.effectiveGasPrice));save(d/'receipts-index.json',rows);print(json.dumps(rows[-1]),flush=True)
    save(d/'result.json',dict(passed=True,completed_at=datetime.now(timezone.utc).isoformat(),receipts=rows,gas_used=sum(x['gas_used'] for x in rows),gas_spent_wei=sum(x['cost_wei'] for x in rows),ETH_left_wei=w.eth.get_balance(account.address),final=checks(w,e,p,packet,values,3)))

if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('action',choices=['qualify','execute']);a=ap.parse_args();os.umask(0o077);qualify() if a.action=='qualify' else execute()
