#!/usr/bin/env python3
"""Build the actual operator's unsigned deployment and execute it only on a local fork."""
import argparse,socket,time,math
from eth_abi import encode
from sign import *
OUT=COORD/'evidence/tee/phase5'
PUBLIC_RPC='https://ethereum-rpc.publicnode.com'
SP1=Web3.to_checksum_address('0x397a5f7f3dbd538f23de225b51f532c34448da9b')
USDC=Web3.to_checksum_address('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48')
MANAGER=Web3.to_checksum_address('0x000000000004444c5dc75cb358380d2e3dE08A90')
ART=Path(__file__).resolve().parents[1]/'b24-replacement/contracts/out'
def artifact(name):
    paths=list(ART.glob('*/'+name+'.json'));assert len(paths)==1;return load(paths[0])
def guard(w):
    assert str(w.provider.endpoint_uri).startswith('http://127.0.0.1:') and 'anvil' in w.client_version.lower()
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--local-fork-only',action='store_true',required=True);ap.parse_args()
    op=load(OUT/'operator.json');pins=load(OUT/'checkpoint-pins.json');assert pins['passed'] and pins['real_live_finality']
    pub=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':30}));assert pub.eth.chain_id==1
    block=pub.eth.block_number;runtime=pub.eth.get_code(SP1,block_identifier=block);assert runtime
    gas_price=pub.eth.gas_price;priority=pub.eth.max_priority_fee;base=pub.eth.get_block(block)['baseFeePerGas'];max_fee=2*base+priority
    with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
    with (OUT/'deployment-fork.log').open('w') as log:
        proc=subprocess.Popen(['anvil','--silent','--host','127.0.0.1','--port',str(port),'--chain-id','1','--fork-url',PUBLIC_RPC,'--fork-block-number',str(block)],stdout=log,stderr=log)
        try:
            w=Web3(Web3.HTTPProvider(f'http://127.0.0.1:{port}',request_kwargs={'timeout':60}))
            for _ in range(120):
                if w.is_connected():break
                time.sleep(.25)
            guard(w);owner=Web3.to_checksum_address(op['address']);assert w.eth.get_transaction_count(owner)==0
            for key in ('token','verifier','controller'):assert not w.eth.get_code(op[key])
            w.provider.make_request('anvil_setBalance',[owner,hex(10**20)]);w.provider.make_request('anvil_impersonateAccount',[owner])
            genesis=bytes.fromhex(load(OUT/'before-state.json')['tip']['genesis_hash']);poolkey=(min(op['token'],USDC,key=lambda x:int(x,16)),max(op['token'],USDC,key=lambda x:int(x,16)),500,10,'0x'+'00'*20)
            poolid=Web3.keccak(encode(['address','address','uint24','int24','address'],poolkey));rows=[];receipts=[]
            def send(label,data,to=None):
                guard(w);tx=dict(chainId=1,nonce=len(rows),**{'from':owner},data=data,value=0)
                if to:tx['to']=to
                estimate=w.eth.estimate_gas(tx);tx['gas']=math.ceil(estimate*1.25);tx['maxFeePerGas']=max_fee;tx['maxPriorityFeePerGas']=priority
                receipt=w.eth.wait_for_transaction_receipt(w.eth.send_transaction(tx));assert receipt.status==1
                rows.append(dict(label=label,transaction=tx));receipts.append(dict(label=label,gas_used=receipt.gasUsed,estimated_gas=estimate,address=receipt.contractAddress,tx_hash=receipt.transactionHash.hex()))
                return receipt
            def deploy(name,args):
                a=artifact(name);data=w.eth.contract(abi=a['abi'],bytecode=a['bytecode']['object']).constructor(*args).data_in_transaction
                return send('deploy-'+name,data)
            r=deploy('WrappedVenueNAVCoin',['Wrapped Schwab NAVCoin B24','wB24',6,owner]);assert r.contractAddress==op['token']
            tokenhash=Web3.keccak(w.eth.get_code(op['token']))
            config=(SP1,Web3.keccak(runtime),bytes.fromhex('4388a21c'),bytes.fromhex('0060d83567657466eaac782e0be174b57277df6ac349f42288852a93d72f0fe1'),Web3.keccak(text='postfiat-wan-devnet-2'),Web3.keccak(genesis),1,Web3.keccak(text=ROUTE),Web3.keccak(bytes.fromhex(B24)),Web3.keccak(bytes.fromhex(PFUSDC)),1,op['controller'],op['token'],tokenhash,356,1120,bytes.fromhex(pins['initial_checkpoint_commitment']),bytes.fromhex(pins['initial_committee_root_commitment']),pins['initial_height'])
            r=deploy('PFTLReceiptFinalityVerifierV2',[config]);assert r.contractAddress==op['verifier']
            r=deploy('PFTLUniswapPrimaryMarketV3',[op['token'],op['verifier'],(1,bytes.fromhex(PFUSDC),bytes.fromhex(B24),poolid,2_000_000_000_000,250_000_000_000,owner),('postfiat-wan-devnet-2',genesis,1,ROUTE)]);assert r.contractAddress==op['controller']
            token=w.eth.contract(address=op['token'],abi=artifact('WrappedVenueNAVCoin')['abi']);controller=w.eth.contract(address=op['controller'],abi=artifact('PFTLUniswapPrimaryMarketV3')['abi'])
            send('setController',token.encode_abi('setController',args=[op['controller']]),op['token']);send('lockController',token.encode_abi('lockController',args=[]),op['token'])
            assert token.functions.controller_locked().call() and token.functions.totalSupply().call()==0 and token.functions.owner().call()==owner
            assert controller.functions.mintPaused().call() and controller.functions.returnPaused().call() and controller.functions.receiptVerifier().call()==op['verifier']
            pins_out=dict(passed=True,fixture_checkpoint_and_roles=False,mainnet_deployed=False,mint_paused=True,return_paused=True,token=op['token'],verifier=op['verifier'],controller=op['controller'],token_runtime_code_hash='0x'+tokenhash.hex(),controller_runtime_code_hash='0x'+Web3.keccak(w.eth.get_code(op['controller'])).hex(),verifier_runtime_code_hash='0x'+Web3.keccak(w.eth.get_code(op['verifier'])).hex(),sp1_runtime_code_hash='0x'+Web3.keccak(runtime).hex(),checkpoint=pins,pool_key=poolkey,pool_id='0x'+poolid.hex())
            save(OUT/'deployment-pins.json',pins_out)
            # Pool initialization is qualified separately; not part of paused-contract deployment.
            pooldata='0x'+(Web3.keccak(text='initialize((address,address,uint24,int24,address),uint160)')[:4]+encode(['(address,address,uint24,int24,address)','uint160'],[poolkey,2**96])).hex()
            send('initialize-pool-at-NAV-1',pooldata,MANAGER)
            budget_gas=sum(x['transaction']['gas'] for x in rows)+math.ceil((500_000+245_296+878_664+150_000)*1.30)
            requested=((budget_gas*max_fee+10**14-1)//10**14)*10**14
            save(OUT/'ethereum-unsigned.json',dict(status='QUALIFIED_UNSIGNED_MAINNET_DEPLOYMENT',operator=owner,chain_id=1,contracts=rows[:5],pool_initialization=rows[5],runtime_pins=pins_out,mainnet_transactions=0,local_fork_balance_fixture_wei=10**20,local_receipts=receipts))
            save(OUT/'gas-funding.json',dict(block=block,gas_price_wei=gas_price,base_fee_wei=base,priority_fee_wei=priority,max_fee_per_gas_wei=max_fee,qualified_deployment_gas=sum(r['gas_used'] for r in receipts[:5]),pool_initialization_gas=receipts[5]['gas_used'],budget_gas=budget_gas,remaining_actions='500k unmeasured actual receipt-proof acceptance allowance, 245296 consume, 878664 LP, 150k approval/unpause allowance; 30 percent gas margin on those allowances',exact_ETH_to_send_wei=requested,exact_ETH_to_send=str(Decimal(requested)/Decimal(10**18)),pool_USDC_atoms=2_000_000_000,creation_USDC_atoms=5_025_000_000,creation_funding_note='Send to the existing USDC vault with route-bound recipient Alice; direct operator funding is not a vault deposit.',operator=owner,mainnet_deployed=False))
            print(json.dumps({'local_fork_passed':True,'operator':owner,'ETH_to_send':str(Decimal(requested)/Decimal(10**18)),'mainnet_transactions':0}))
        finally:proc.terminate();proc.wait(timeout=10)
if __name__=='__main__':
    from decimal import Decimal
    main()
