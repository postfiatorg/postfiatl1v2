#!/usr/bin/env python3
"""Phase 6: qualify exact funded mainnet deployment, then send once with 12 confirmations."""
import argparse,math,socket,stat,time
from datetime import datetime,timezone
from eth_abi import encode,decode
from eth_account import Account
from sign import *
from deployment import artifact,PUBLIC_RPC,SP1,USDC,MANAGER,guard
OUT=COORD/'evidence/tee/phase6'
OLD=COORD/'evidence/tee/phase5c'
VIEW=Web3.to_checksum_address('0x7ffe42c4a5deea5b0fec41c94c136cf115597227')
KEY=Path('/home/postfiat/.config/b24/eth-operator.key')
ZERO='0x'+'00'*20
CONFIRMATIONS=12
def hx(x):return '0x'+bytes(x).hex()
def norm(x):
    if isinstance(x,bytes):return hx(x)
    if isinstance(x,str) and x.startswith('0x'):return x.lower()
    return x
def contract(w,name,address):return w.eth.contract(address=address,abi=artifact(name)['abi'])
def pool(w,pid):
    def read(sig,types):return list(decode(types,w.eth.call({'to':VIEW,'data':Web3.keccak(text=sig)[:4]+encode(['bytes32'],[bytes.fromhex(pid[2:])])})))
    return dict(slot0=read('getSlot0(bytes32)',['uint160','int24','uint24','uint24']),liquidity=read('getLiquidity(bytes32)',['uint128'])[0])
def expected():
    op=load(OUT/'operator.json');s=load(OUT/'before-state.json');r=item(s['ledger']['pftl_uniswap_routes'],'route_id',ROUTE);p=load(OUT/'checkpoint-pins.json');old=load(OLD/'deployment-pins.json')
    assert p['passed'] and p['real_live_finality'] and p['initial_height']==s['tip']['height']
    assert p['guest_elf_sha256']=='803d64902ec32aa5371119c01525a026a3955a7378ffdf729e5473d1bb569208'
    assert r['ethereum_chain_id']==1 and r['wrapped_navcoin_token']==op['token'].lower() and r['handoff_controller']==op['controller'].lower() and r['settlement_adapter']==op['controller'].lower()
    policy=r['ethereum_verification_policy'];assert hx(policy['wrapped_navcoin_code_hash'])==old['token_runtime_code_hash'] and hx(policy['handoff_controller_code_hash'])==old['controller_runtime_code_hash']
    assert r['native_nav_asset_id']==B24 and r['settlement_asset_id']==PFUSDC and r['v2']['outbound_verification_class']=='TRUSTLESS_FINALITY'
    assert r['v2']['primary_market_policy']['issue_multiplier_bps']==10050 and r['v2']['primary_market_policy']['redeem_multiplier_bps']==9995 and not r.get('spread_recipient') and not r.get('funding_sequence')
    owner=op['address'];genesis=bytes.fromhex(s['tip']['genesis_hash']);key=(min(op['token'],USDC,key=lambda x:int(x,16)),max(op['token'],USDC,key=lambda x:int(x,16)),500,10,ZERO);pid=hx(Web3.keccak(encode(['address','address','uint24','int24','address'],key)))
    n=item(s['ledger']['nav_assets'],'asset_id',B24);nav=n['nav_per_unit'];assert not n['halted'] and r.get('live_value_enabled') and not r['paused']
    # Both tokens use six decimals. Use exact integer sqrt and token ordering.
    numerator,denominator=(nav,1000000) if key[0]==op['token'] else (1000000,nav);sqrt=math.isqrt((numerator<<192)//denominator)
    verifier=dict(sp1Verifier=SP1,sp1VerifierRuntimeCodeHash=old['sp1_runtime_code_hash'],sp1ProofSelector='0x4388a21c',programVKey='0x0060d83567657466eaac782e0be174b57277df6ac349f42288852a93d72f0fe1',pftlChainIdHash=hx(Web3.keccak(text=s['tip']['chain_id'])),pftlGenesisHashCommitment=hx(Web3.keccak(genesis)),pftlProtocolVersion=1,routeIdCommitment=hx(Web3.keccak(text=ROUTE)),nativeNavAssetIdCommitment=hx(Web3.keccak(bytes.fromhex(B24))),settlementAssetIdCommitment=hx(Web3.keccak(bytes.fromhex(PFUSDC))),destinationChainId=1,controller=op['controller'],wrappedToken=op['token'],wrappedTokenRuntimeCodeHash=old['token_runtime_code_hash'],maxProofBytes=356,maxPublicValuesBytes=1120,latestCheckpointCommitment='0x'+p['initial_checkpoint_commitment'],latestFinalizedHeight=p['initial_height'])
    controller=dict(wrappedToken=op['token'],receiptVerifier=op['verifier'],destinationChainId=1,uniswapPoolId=pid,routeSupplyCapAtoms=r['route_supply_cap_atoms'],packetNotionalCapAtoms=r['packet_notional_cap_atoms'],governance=owner,settlementAssetId='0x'+PFUSDC,nativeNavAssetId='0x'+B24,returnRouteBinding=hx(Web3.keccak(encode(['string','string','bytes','uint32','string'],['postfiat.pftl_uniswap.return_route.v3',s['tip']['chain_id'],genesis,1,ROUTE]))),totalMintedAtoms=0,totalReturnBurnedAtoms=0,outstandingMintedAtoms=0,mintPaused=True,returnPaused=True)
    return dict(operator=op,tip=s['tip'],checkpoint=p,runtime=old,pool_key=key,pool_id=pid,NAV_atoms=nav,nav_epoch=n['finalized_epoch'],sqrt_price_x96=sqrt,verifier=verifier,controller=controller,route_binding_note='Native route directly pins token/controller and code hashes; controller runtime immutable binds the verifier address.')
def verify(w,e,stage):
    o=e['operator'];rows={};names=('token','verifier','controller');types=('WrappedVenueNAVCoin','PFTLReceiptFinalityVerifierV2','PFTLUniswapPrimaryMarketV3')
    for i,(name,typename) in enumerate(zip(names,types)):
        if i>stage:continue
        address=o[name];code=w.eth.get_code(address);actual=hx(Web3.keccak(code));assert actual==e['runtime'][name+'_runtime_code_hash'],name+' code pin differs'
        c=contract(w,typename,address)
        fields=dict(name='Wrapped Schwab NAVCoin B24',symbol='wB24',decimals=6,owner=o['address'],controller=o['controller'] if stage>=3 else ZERO,controller_locked=stage>=4,totalSupply=0) if name=='token' else e[name]
        values={}
        for field,wanted in fields.items():
            value=getattr(c.functions,field)().call();values[field]=norm(value);assert norm(value)==norm(wanted),name+'.'+field+' pin differs'
        if name=='verifier':
            cp=bytes.fromhex(e['checkpoint']['initial_checkpoint_commitment']);assert c.functions.acceptedCheckpointCommitment(cp).call()
            assert c.functions.checkpointFinalizedHeight(cp).call()==e['checkpoint']['initial_height']
            assert hx(c.functions.checkpointCommitteeRootCommitment(cp).call())=='0x'+e['checkpoint']['initial_committee_root_commitment']
            values['checkpointCommitteeRootCommitment']='0x'+e['checkpoint']['initial_committee_root_commitment']
        rows[name]=dict(address=address,runtime_code_hash=actual,values=values)
    if stage>=5:
        rows['pool']=pool(w,e['pool_id']);assert rows['pool']['slot0'][0]==e['sqrt_price_x96'] and rows['pool']['slot0'][3]==500 and rows['pool']['liquidity']==0
    return rows
def qualify():
    d=OUT/'deployment-qualification';d.mkdir(mode=0o700,exist_ok=False);e=expected();o=e['operator'];old=load(OLD/'ethereum-unsigned.json');p=e['checkpoint'];v=e['verifier'];w0=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));assert w0.eth.chain_id==1
    block=w0.eth.block_number;base=w0.eth.get_block(block)['baseFeePerGas'];priority=w0.eth.max_priority_fee;fee=max(2*base+priority,2000000000)
    assert w0.eth.get_transaction_count(o['address'],'pending')==0 and all(not w0.eth.get_code(o[k]) for k in ('token','verifier','controller'))
    assert hx(Web3.keccak(w0.eth.get_code(SP1)))==e['runtime']['sp1_runtime_code_hash']
    rows=[]
    def add(label,data,to=None):
        tx=dict(chainId=1,nonce=len(rows),**{'from':o['address']},data=data,value=0,maxFeePerGas=fee,maxPriorityFeePerGas=priority)
        if to:tx['to']=to
        rows.append(dict(label=label,transaction=tx))
    add('deploy-token',w0.eth.contract(abi=artifact('WrappedVenueNAVCoin')['abi'],bytecode=artifact('WrappedVenueNAVCoin')['bytecode']['object']).constructor('Wrapped Schwab NAVCoin B24','wB24',6,o['address']).data_in_transaction)
    config=[v[k] for k in list(v)[:16]]+[v['latestCheckpointCommitment'],'0x'+p['initial_committee_root_commitment'],p['initial_height']]
    add('deploy-verifier',w0.eth.contract(abi=artifact('PFTLReceiptFinalityVerifierV2')['abi'],bytecode=artifact('PFTLReceiptFinalityVerifierV2')['bytecode']['object']).constructor(config).data_in_transaction)
    add('deploy-controller',w0.eth.contract(abi=artifact('PFTLUniswapPrimaryMarketV3')['abi'],bytecode=artifact('PFTLUniswapPrimaryMarketV3')['bytecode']['object']).constructor(o['token'],o['verifier'],(1,bytes.fromhex(PFUSDC),bytes.fromhex(B24),bytes.fromhex(e['pool_id'][2:]),2000000000000,250000000000,o['address']),(e['tip']['chain_id'],bytes.fromhex(e['tip']['genesis_hash']),1,ROUTE)).data_in_transaction)
    token=contract(w0,'WrappedVenueNAVCoin',o['token']);add('setController',token.encode_abi('setController',args=[o['controller']]),o['token']);add('lockController',token.encode_abi('lockController',args=[]),o['token'])
    for i in (0,2,3,4):assert rows[i]['transaction']['data']==old['contracts'][i]['transaction']['data']
    oldpins=load(COORD/'evidence/tee/phase5/checkpoint-pins.json');oldconfig=config[:16]+['0x'+oldpins['initial_checkpoint_commitment'],'0x'+oldpins['initial_committee_root_commitment'],oldpins['initial_height']]
    assert w0.eth.contract(abi=artifact('PFTLReceiptFinalityVerifierV2')['abi'],bytecode=artifact('PFTLReceiptFinalityVerifierV2')['bytecode']['object']).constructor(oldconfig).data_in_transaction==old['contracts'][1]['transaction']['data']
    add('initialize-hookless-pool','0x'+(Web3.keccak(text='initialize((address,address,uint24,int24,address),uint160)')[:4]+encode(['(address,address,uint24,int24,address)','uint160'],[e['pool_key'],e['sqrt_price_x96']])).hex(),MANAGER)
    e['checkpoint_refresh']=dict(prior_height=oldpins['initial_height'],new_height=p['initial_height'],same_committee=oldpins['initial_committee_root']==p['initial_committee_root'],only_constructor_initial_checkpoint_storage_changed=True)
    e['infrastructure_code_hashes']={name:hx(Web3.keccak(w0.eth.get_code(address,block_identifier=block))) for name,address in [('manager',MANAGER),('state_view',VIEW),('SP1',SP1)]}
    with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
    with (d/'anvil.log').open('w') as log:
        process=subprocess.Popen(['anvil','--silent','--host','127.0.0.1','--port',str(port),'--chain-id','1','--fork-url',PUBLIC_RPC,'--fork-block-number',str(block)],stdout=log,stderr=log)
        try:
            w=Web3(Web3.HTTPProvider(f'http://127.0.0.1:{port}',request_kwargs={'timeout':60}))
            for _ in range(120):
                if w.is_connected():break
                time.sleep(.25)
            guard(w);assert w.eth.get_balance(o['address'])==20000000000000000;assert pool(w,e['pool_id'])['slot0'][0]==0
            w.provider.make_request('anvil_impersonateAccount',[o['address']]);receipts=[]
            for i,row in enumerate(rows):
                tx=row['transaction'];estimate=w.eth.estimate_gas(tx);tx['gas']=math.ceil(estimate*1.25)
                assert w.eth.get_balance(o['address'])>=tx['gas']*fee;receipt=w.eth.wait_for_transaction_receipt(w.eth.send_transaction(tx));assert receipt.status==1
                if i<3:assert receipt.contractAddress==o[('token','verifier','controller')[i]]
                checks=verify(w,e,i);save(d/f'{i}-checks.json',checks);receipts.append(dict(label=row['label'],hash=receipt.transactionHash.hex(),gas_used=receipt.gasUsed,estimated_gas=estimate,address=receipt.contractAddress))
            save(OUT/'deployment-expected.json',e);save(OUT/'ethereum-unsigned.json',dict(chain_id=1,operator=o['address'],transactions=rows,expected_sha256=hashlib.sha256((OUT/'deployment-expected.json').read_bytes()).hexdigest()))
            maximum=sum(r['transaction']['gas'] for r in rows)*fee
            save(d/'result.json',dict(passed=True,block=block,base_fee_wei=base,max_fee_wei=fee,priority_fee_wei=priority,mainnet_transactions=0,funded_balance_used_without_override=True,initial_checkpoint=p['initial_height'],receipts=receipts,maximum_deployment_cost_wei=maximum,remaining_at_max_fee_wei=20000000000000000-maximum,pool_safe_before_liquidity=True,pool_reason='Same qualified hookless pool, supply zero, no liquidity; price and NAV rechecked immediately before initialization.',unsigned_sha256=hashlib.sha256((OUT/'ethereum-unsigned.json').read_bytes()).hexdigest()))
            print(json.dumps(load(d/'result.json')),flush=True)
        finally:process.terminate();process.wait(timeout=10)
def confirmed(w,h,receipt):
    deadline=time.monotonic()+900
    while w.eth.block_number-receipt.blockNumber+1<CONFIRMATIONS:
        assert time.monotonic()<deadline,'Confirmation wait timed out; do not resend';time.sleep(12)
    assert w.eth.get_block(receipt.blockNumber)['hash']==receipt.blockHash and w.eth.get_transaction_receipt(h).status==1
def deploy():
    q=load(OUT/'deployment-qualification/result.json');assert q['passed'];u=OUT/'ethereum-unsigned.json';assert hashlib.sha256(u.read_bytes()).hexdigest()==q['unsigned_sha256'];data=load(u);e=load(OUT/'deployment-expected.json');assert hashlib.sha256((OUT/'deployment-expected.json').read_bytes()).hexdigest()==data['expected_sha256']
    w=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));assert w.eth.chain_id==1;o=e['operator'];assert w.eth.get_transaction_count(o['address'],'pending')==0
    assert not KEY.is_symlink() and not KEY.parent.is_symlink() and stat.S_IMODE(KEY.stat().st_mode)==0o600 and stat.S_IMODE(KEY.parent.stat().st_mode)==0o700
    account=Account.from_key(KEY.read_text().strip());assert account.address==o['address'];d=OUT/'mainnet-deployment';d.mkdir(mode=0o700,exist_ok=False);results=[]
    for i,row in enumerate(data['transactions']):
        assert w.eth.get_transaction_count(o['address'],'pending')==i
        for name,address in [('manager',MANAGER),('state_view',VIEW),('SP1',SP1)]:assert hx(Web3.keccak(w.eth.get_code(address)))==e['infrastructure_code_hashes'][name]
        tx=dict(row['transaction']);assert w.eth.get_block('latest')['baseFeePerGas']+tx['maxPriorityFeePerGas']<=tx['maxFeePerGas'],'Fee cap exceeded; no send'
        assert w.eth.get_balance(o['address'])>=sum(z['transaction']['gas']*z['transaction']['maxFeePerGas'] for z in data['transactions'][i:])
        if i<3:assert not w.eth.get_code(o[('token','verifier','controller')[i]])
        if i==5:
            from rpc import call
            nav=[call('nav_reserve_proof_status',{'asset_id':B24},node=n) for n in range(6)];save(d/'NAV-before-pool.json',nav)
            finalized=[max((p for p in report['packets'] if p['state']=='finalized'),key=lambda p:p['epoch']) for report in nav]
            assert all(p==finalized[0] for p in finalized),'Finalized NAV differs across validators'
            if finalized[0]['nav_per_unit']!=e['NAV_atoms']:
                save(d/'pool-deferred.json',dict(reason='Daily NAV changed since exact pool simulation; initialize at freshly qualified price during seeding.',current_NAV_atoms=finalized[0]['nav_per_unit']));break
            assert pool(w,e['pool_id'])['slot0'][0]==0
        tx.pop('from');signed=account.sign_transaction(tx);save(d/f'{i}-submission.json',dict(label=row['label'],expected_hash=hx(signed.hash),transaction=tx))
        h=w.eth.send_raw_transaction(signed.raw_transaction);assert h==signed.hash
        receipt=w.eth.wait_for_transaction_receipt(h,timeout=600);save(d/f'{i}-receipt.json',json.loads(w.to_json(receipt)));assert receipt.status==1
        confirmed(w,h,receipt)
        if i<3:assert receipt.contractAddress==o[('token','verifier','controller')[i]],'Address differs; no continuation/redeployment'
        checks=verify(w,e,i);save(d/f'{i}-checks.json',checks)
        results.append(dict(label=row['label'],tx_hash=hx(h),block=receipt.blockNumber,confirmations=w.eth.block_number-receipt.blockNumber+1,gas_used=receipt.gasUsed,effective_gas_price=receipt.effectiveGasPrice,cost_wei=receipt.gasUsed*receipt.effectiveGasPrice,address=receipt.contractAddress));save(d/'receipts-index.json',results);print(json.dumps(results[-1]),flush=True)
    save(d/'result.json',dict(passed=True,completed_at=datetime.now(timezone.utc).isoformat(),receipts=results,gas_used=sum(r['gas_used'] for r in results),gas_spent_wei=sum(r['cost_wei'] for r in results),ETH_left_wei=w.eth.get_balance(o['address']),mint_paused=True,return_paused=True,supply=0,ownership_retained=True,pool_initialized=len(results)==6,pool_liquidity=0))
if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('action',choices=['qualify','deploy']);a=ap.parse_args();qualify() if a.action=='qualify' else deploy()
