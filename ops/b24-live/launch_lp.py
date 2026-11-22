#!/usr/bin/env python3
"""Pool launch, last leg: seed the B24/USDC Uniswap v4 pool from the operator's real balances.
qualify = mainnet fork with the real balances (no fixtures); execute = same steps signed on mainnet."""
import argparse,os
from decimal import Decimal,getcontext,ROUND_FLOOR,ROUND_CEILING
from phase6_deploy import *
getcontext().prec=90
L=COORD/'evidence/launch'
POS=Web3.to_checksum_address('0xbd216513d74c8cf14cf4747e6aaa6420ff64ee9e')
PERMIT=Web3.to_checksum_address('0x000000000022d473030f116ddee9f6b43ac78ba3')
WRAPPED=5000000000;QUOTE=2000000000
def run(w,e,d,sender,fork):
    o=e['operator'];rows=[];budget={o['token']:WRAPPED,USDC:QUOTE}
    def read(addr,sig,types,args,outputs):return decode(outputs,w.eth.call({'to':addr,'data':Web3.keccak(text=sig)[:4]+encode(types,args)}))
    def balance(addr):return read(addr,'balanceOf(address)',['address'],[o['address']],['uint256'])[0]
    def send(label,addr,sig,types,args):
        tx={'from':o['address'],'to':addr,'data':hx(Web3.keccak(text=sig)[:4]+encode(types,args)),'value':0};tx['gas']=math.ceil(w.eth.estimate_gas(tx)*1.3)
        receipt=sender(tx,label);assert receipt.status==1;rows.append(dict(label=label,tx_hash=hx(receipt.transactionHash),block=receipt.blockNumber,gas_used=receipt.gasUsed));save(d/'receipts-index.json',rows);print(json.dumps(rows[-1]),flush=True);return receipt
    start=pool(w,e['pool_id']);assert start['liquidity']==0 and start['slot0'][0]==e['sqrt_price_x96'] and start['slot0'][3]==500
    assert balance(o['token'])>=WRAPPED and balance(USDC)>=QUOTE,'operator balances short'
    for name,addr in [('PositionManager',POS),('Permit2',PERMIT),('USDC',USDC)]:assert hx(Web3.keccak(w.eth.get_code(addr)))==load(OUT/'lp-qualification/result.json')['code_hashes'][name],name+' code differs'
    for addr,amount in budget.items():
        send('approve-permit2-'+addr[:8],addr,'approve(address,uint256)',['address','uint256'],[PERMIT,amount])
        send('permit2-position-manager-'+addr[:8],PERMIT,'approve(address,address,uint160,uint48)',['address','address','uint160','uint48'],[addr,POS,amount,w.eth.get_block('latest')['timestamp']+3600])
    nav=Decimal(e['NAV_atoms'])/1000000;wrapped0=int(o['token'],16)<int(USDC,16);ratio=nav if wrapped0 else 1/nav;ln=Decimal('1.0001').ln();tick=lambda p:p.ln()/ln
    low,high=nav*Decimal('.98'),nav*Decimal('1.02');rawlow,rawhigh=(low,high) if wrapped0 else (1/high,1/low);lo=int((tick(rawlow)/10).to_integral_value(rounding=ROUND_FLOOR))*10;hi=int((tick(rawhigh)/10).to_integral_value(rounding=ROUND_CEILING))*10
    s=Decimal(start['slot0'][0])/(Decimal(2)**96);positions=[]
    def seed(t0,t1,label):
        before=(balance(o['token']),balance(USDC));left=(budget[o['token']],budget[USDC]);a0,a1=left if wrapped0 else left[::-1];sa=Decimal('1.0001')**(Decimal(t0)/2);sb=Decimal('1.0001')**(Decimal(t1)/2)
        liquidity=int(Decimal(a0)*sa*sb/(sb-sa) if s<=sa else Decimal(a1)/(sb-sa) if s>=sb else min(Decimal(a0)*s*sb/(sb-s),Decimal(a1)/(s-sa)))-1;assert liquidity>0
        params=[encode(['(address,address,uint24,int24,address)','int24','int24','uint256','uint128','uint128','address','bytes'],[e['pool_key'],t0,t1,liquidity,a0,a1,o['address'],b'']),encode(['address','address'],e['pool_key'][:2])]
        token_id=read(POS,'nextTokenId()',[],[],['uint256'])[0];send(label,POS,'modifyLiquidities(bytes,uint256)',['bytes','uint256'],[encode(['bytes','bytes[]'],[bytes.fromhex('020d'),params]),w.eth.get_block('latest')['timestamp']+900]);after=(balance(o['token']),balance(USDC))
        assert read(POS,'ownerOf(uint256)',['uint256'],[token_id],['address'])[0].lower()==o['address'].lower();assert read(POS,'getPositionLiquidity(uint256)',['uint256'],[token_id],['uint128'])[0]==liquidity
        spent=(before[0]-after[0],before[1]-after[1]);budget[o['token']]-=spent[0];budget[USDC]-=spent[1];assert budget[o['token']]>=0 and budget[USDC]>=0
        positions.append(dict(label=label,ticks=[t0,t1],liquidity=liquidity,NFT_id=token_id,wrapped_atoms=spent[0],USDC_atoms=spent[1]))
    seed(lo,hi,'centered-band');near=int((tick(ratio)/10).to_integral_value(rounding=ROUND_CEILING if wrapped0 else ROUND_FLOOR))*10;seed(near if wrapped0 else lo,hi if wrapped0 else near,'remaining-wrapper-ask-band')
    assert sum(p['wrapped_atoms'] for p in positions)>=WRAPPED-10 and sum(p['USDC_atoms'] for p in positions)>=QUOTE-10
    end=pool(w,e['pool_id']);assert end['slot0'][0]==start['slot0'][0] and end['liquidity']>0
    save(d/'result.json',dict(passed=True,fork=fork,mainnet_transactions=0 if fork else len(rows),positions=positions,receipts=rows,pool=dict(sqrt_price_x96=str(end['slot0'][0]),tick=end['slot0'][1],liquidity=str(end['liquidity'])),gas_used=sum(x['gas_used'] for x in rows),operator_wrapped_left=balance(o['token']),operator_USDC_left=balance(USDC),operator_wei_left=w.eth.get_balance(o['address'])))
    print(json.dumps(load(d/'result.json')),flush=True)
def qualify():
    e=load(OUT/'deployment-expected.json');o=e['operator'];d=L/'lp-qualification';d.mkdir(mode=0o700,exist_ok=False);w0=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));block=w0.eth.block_number
    with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
    with (d/'anvil.log').open('w') as log:
        proc=subprocess.Popen(['anvil','--silent','--host','127.0.0.1','--port',str(port),'--chain-id','1','--fork-url',PUBLIC_RPC,'--fork-block-number',str(block)],stdout=log,stderr=log)
        try:
            w=Web3(Web3.HTTPProvider(f'http://127.0.0.1:{port}',request_kwargs={'timeout':60}))
            for _ in range(120):
                if w.is_connected():break
                time.sleep(.25)
            guard(w);w.provider.make_request('anvil_impersonateAccount',[o['address']])
            run(w,e,d,lambda tx,label:w.eth.wait_for_transaction_receipt(w.eth.send_transaction(tx)),True)
        finally:proc.terminate();proc.wait(timeout=10)
def execute():
    assert load(L/'lp-qualification/result.json')['passed'];e=load(OUT/'deployment-expected.json');d=L/'lp';d.mkdir(mode=0o700,exist_ok=False)
    w=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));assert w.eth.chain_id==1
    assert not KEY.is_symlink() and stat.S_IMODE(KEY.stat().st_mode)==0o600;account=Account.from_key(KEY.read_text().strip());assert account.address==e['operator']['address']
    def sender(tx,label):
        tx=dict(tx);tx.pop('from');priority=w.eth.max_priority_fee;tx.update(chainId=1,nonce=w.eth.get_transaction_count(account.address,'pending'),maxPriorityFeePerGas=priority,maxFeePerGas=max(2*w.eth.get_block('latest')['baseFeePerGas']+priority,2000000000))
        assert w.eth.get_balance(account.address)>=tx['gas']*tx['maxFeePerGas'];signed=account.sign_transaction(tx);save(d/(label+'-submission.json'),dict(expected_hash=hx(signed.hash),transaction=tx))
        h=w.eth.send_raw_transaction(signed.raw_transaction);receipt=w.eth.wait_for_transaction_receipt(h,timeout=600);assert receipt.status==1
        while w.eth.block_number<receipt.blockNumber+2:time.sleep(4)
        return receipt
    run(w,e,d,sender,False)
if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('action',choices=['qualify','execute']);a=ap.parse_args();os.umask(0o077);qualify() if a.action=='qualify' else execute()
