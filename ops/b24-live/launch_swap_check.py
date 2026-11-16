#!/usr/bin/env python3
"""One small real USDC -> wB24 swap from the operator through Uniswap's Universal Router (v4), to prove the public path."""
import sys
from phase6_deploy import *
L=COORD/'evidence/launch';AMOUNT=500000
PERMIT=Web3.to_checksum_address('0x000000000022d473030f116ddee9f6b43ac78ba3');ROUTER=Web3.to_checksum_address('0x66a9893cc07d91d95644aedd05d03f95e1dba8af');QUOTER=Web3.to_checksum_address('0x52f0e24d1c21c8a0cb1e5a5dd6198556bd9e1203')
KT='(address,address,uint24,int24,address)'
def main():
    e=load(OUT/'deployment-expected.json');o=e['operator'];T=o['token'];key=tuple(e['pool_key']);d=L/'swap-check';d.mkdir(mode=0o700,exist_ok=False)
    w=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));assert w.eth.chain_id==1;acct=Account.from_key(KEY.read_text().strip());assert acct.address==o['address']
    call=lambda sig,types,args:Web3.keccak(text=sig)[:4]+encode(types,args)
    bal=lambda t:decode(['uint256'],w.eth.call({'to':t,'data':call('balanceOf(address)',['address'],[acct.address])}))[0]
    rows=[]
    def send(label,to,data):
        tx={'from':acct.address,'to':to,'data':hx(data),'value':0};tx['gas']=math.ceil(w.eth.estimate_gas(tx)*1.3);tx.pop('from');p=w.eth.max_priority_fee
        tx.update(chainId=1,nonce=w.eth.get_transaction_count(acct.address,'pending'),maxPriorityFeePerGas=p,maxFeePerGas=max(2*w.eth.get_block('latest')['baseFeePerGas']+p,2000000000))
        h=w.eth.send_raw_transaction(acct.sign_transaction(tx).raw_transaction);r=w.eth.wait_for_transaction_receipt(h,timeout=600);assert r.status==1
        while w.eth.block_number<r.blockNumber+2:time.sleep(4)
        rows.append(dict(label=label,tx_hash=hx(h),block=r.blockNumber,gas_used=r.gasUsed));print(json.dumps(rows[-1]),flush=True)
    before=(bal(USDC),bal(T));assert before[0]>=AMOUNT
    send('approve-permit2',USDC,call('approve(address,uint256)',['address','uint256'],[PERMIT,AMOUNT]))
    send('permit2-router',PERMIT,call('approve(address,address,uint160,uint48)',['address','address','uint160','uint48'],[USDC,ROUTER,AMOUNT,w.eth.get_block('latest')['timestamp']+900]))
    quote=decode(['uint256','uint256'],w.eth.call({'to':QUOTER,'data':call('quoteExactInputSingle(('+KT+',bool,uint128,bytes))',['('+KT+',bool,uint128,bytes)'],[(key,False,AMOUNT,b'')])}))[0];minimum=quote*9950//10000
    params=[encode(['('+KT+',bool,uint128,uint128,bytes)'],[(key,False,AMOUNT,minimum,b'')]),encode(['address','uint256'],[USDC,AMOUNT]),encode(['address','uint256'],[T,minimum])]
    send('swap',ROUTER,call('execute(bytes,bytes[],uint256)',['bytes','bytes[]','uint256'],[b'\x10',[encode(['bytes','bytes[]'],[bytes.fromhex('060c0f'),params])],w.eth.get_block('latest')['timestamp']+600]))
    after=(bal(USDC),bal(T));res=dict(passed=after[1]-before[1]==quote and before[0]-after[0]==AMOUNT,USDC_in_atoms=before[0]-after[0],wB24_out_atoms=after[1]-before[1],quote_atoms=quote,router=ROUTER,receipts=rows);save(d/'result.json',res);print(json.dumps(res));assert res['passed']
if __name__=='__main__':main()
