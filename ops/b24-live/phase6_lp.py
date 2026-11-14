#!/usr/bin/env python3
"""Qualify bounded approvals/direct LP seeding on a fork with explicit funding fixtures."""
from decimal import Decimal,getcontext,ROUND_FLOOR,ROUND_CEILING
from phase6_deploy import *
getcontext().prec=90
POS=Web3.to_checksum_address('0xbd216513d74c8cf14cf4747e6aaa6420ff64ee9e')
PERMIT=Web3.to_checksum_address('0x000000000022d473030f116ddee9f6b43ac78ba3')
def main():
    e=load(OUT/'deployment-expected.json');assert load(OUT/'mainnet-deployment/result.json')['pool_initialized'];o=e['operator'];d=OUT/'lp-qualification';d.mkdir(mode=0o700,exist_ok=False);w0=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));block=w0.eth.block_number
    with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
    with (d/'anvil.log').open('w') as log:
        proc=subprocess.Popen(['anvil','--silent','--host','127.0.0.1','--port',str(port),'--chain-id','1','--fork-url',PUBLIC_RPC,'--fork-block-number',str(block)],stdout=log,stderr=log)
        try:
            w=Web3(Web3.HTTPProvider(f'http://127.0.0.1:{port}',request_kwargs={'timeout':60}))
            for _ in range(120):
                if w.is_connected():break
                time.sleep(.25)
            guard(w);assert pool(w,e['pool_id'])['liquidity']==0;w.provider.make_request('anvil_impersonateAccount',[o['address']]);rows=[];fixtures=[]
            def read(addr,sig,types,args,outputs):return decode(outputs,w.eth.call({'to':addr,'data':Web3.keccak(text=sig)[:4]+encode(types,args)}))
            def balance(addr):return read(addr,'balanceOf(address)',['address'],[o['address']],['uint256'])[0]
            def send(label,addr,sig,types,args):
                guard(w);tx={'from':o['address'],'to':addr,'data':hx(Web3.keccak(text=sig)[:4]+encode(types,args)),'value':0};tx['gas']=math.ceil(w.eth.estimate_gas(tx)*1.3);receipt=w.eth.wait_for_transaction_receipt(w.eth.send_transaction(tx));assert receipt.status==1;rows.append(dict(label=label,transaction=tx,gas_used=receipt.gasUsed));return receipt
            for addr,amount in [(o['token'],5000000000),(USDC,2000000000)]:
                for slot in range(30):
                    key=Web3.keccak(encode(['address','uint256'],[o['address'],slot]));old=w.eth.get_storage_at(addr,key);guard(w);result=w.provider.make_request('anvil_setStorageAt',[addr,hx(key),hx(amount.to_bytes(32,'big'))]);assert 'error' not in result
                    if balance(addr)==amount:fixtures.append(dict(token=addr,balance_slot=slot,fixture_atoms=amount));break
                    w.provider.make_request('anvil_setStorageAt',[addr,hx(key),hx(old)])
                else:raise RuntimeError('Funding fixture balance mapping not located')
                send('approve-'+addr,addr,'approve(address,uint256)',['address','uint256'],[PERMIT,amount])
                send('permit2-position-manager-'+addr,PERMIT,'approve(address,address,uint160,uint48)',['address','address','uint160','uint48'],[addr,POS,amount,w.eth.get_block('latest')['timestamp']+3600])
            nav=Decimal(e['NAV_atoms'])/1000000;wrapped0=int(o['token'],16)<int(USDC,16);ratio=nav if wrapped0 else 1/nav;ln=Decimal('1.0001').ln();tick=lambda p:p.ln()/ln
            low,high=nav*Decimal('.98'),nav*Decimal('1.02');rawlow,rawhigh=(low,high) if wrapped0 else (1/high,1/low);lo=int((tick(rawlow)/10).to_integral_value(rounding=ROUND_FLOOR))*10;hi=int((tick(rawhigh)/10).to_integral_value(rounding=ROUND_CEILING))*10
            current=pool(w,e['pool_id']);assert current['slot0'][0]==e['sqrt_price_x96'];s=Decimal(current['slot0'][0])/(Decimal(2)**96);positions=[]
            def seed(t0,t1,label):
                before=(balance(o['token']),balance(USDC));a0,a1=before if wrapped0 else before[::-1];sa=Decimal('1.0001')**(Decimal(t0)/2);sb=Decimal('1.0001')**(Decimal(t1)/2)
                liquidity=int(Decimal(a0)*sa*sb/(sb-sa) if s<=sa else Decimal(a1)/(sb-sa) if s>=sb else min(Decimal(a0)*s*sb/(sb-s),Decimal(a1)/(s-sa)))-1;assert liquidity>0
                params=[encode(['(address,address,uint24,int24,address)','int24','int24','uint256','uint128','uint128','address','bytes'],[e['pool_key'],t0,t1,liquidity,a0,a1,o['address'],b'']),encode(['address','address'],e['pool_key'][:2])]
                token_id=read(POS,'nextTokenId()',[],[],['uint256'])[0];send(label,POS,'modifyLiquidities(bytes,uint256)',['bytes','uint256'],[encode(['bytes','bytes[]'],[bytes.fromhex('020d'),params]),w.eth.get_block('latest')['timestamp']+600]);after=(balance(o['token']),balance(USDC))
                assert read(POS,'ownerOf(uint256)',['uint256'],[token_id],['address'])[0].lower()==o['address'].lower();assert read(POS,'getPositionLiquidity(uint256)',['uint256'],[token_id],['uint128'])[0]==liquidity
                positions.append(dict(label=label,ticks=[t0,t1],liquidity=liquidity,fixture_NFT_id=token_id,wrapped_atoms=before[0]-after[0],USDC_atoms=before[1]-after[1]))
            seed(lo,hi,'centered-band');near=int((tick(ratio)/10).to_integral_value(rounding=ROUND_CEILING if wrapped0 else ROUND_FLOOR))*10;seed(near if wrapped0 else lo,hi if wrapped0 else near,'remaining-wrapper-ask-band')
            assert sum(p['wrapped_atoms'] for p in positions)>=4999999995 and sum(p['USDC_atoms'] for p in positions)>=1999999995
            save(d/'unsigned-LP-template.json',dict(status='DO_NOT_SEND_FUNDING_FIXTURE_ONLY_REFRESH_DEADLINES_NONCES_NAV_PRICE',transactions=rows,positions=positions,pool_key=e['pool_key'],pool_id=e['pool_id'],NAV_atoms=e['NAV_atoms'],owner=o['address']))
            save(d/'result.json',dict(passed=True,fork_block=block,real_deployed_token_and_pool=True,real_USDC_Permit2_PositionManager=True,mainnet_transactions=0,synthetic_funding_only=True,fixtures=fixtures,positions=positions,gas_used=sum(x['gas_used'] for x in rows),gas_budget=sum(x['transaction']['gas'] for x in rows),wrapped_dust_atoms=balance(o['token']),USDC_dust_atoms=balance(USDC),code_hashes={n:hx(Web3.keccak(w.eth.get_code(a))) for n,a in [('Permit2',PERMIT),('PositionManager',POS),('USDC',USDC)]}))
            print(json.dumps(load(d/'result.json')),flush=True)
        finally:proc.terminate();proc.wait(timeout=10)
if __name__=='__main__':main()
