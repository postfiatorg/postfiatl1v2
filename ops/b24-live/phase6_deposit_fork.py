#!/usr/bin/env python3
"""Exercise prepared deposit calldata on a fresh fork with a labeled USDC fixture."""
from phase6_deploy import *
def main():
    d=OUT/'deposit-qualification';d.mkdir(mode=0o700,exist_ok=False);u=load(OUT/'funding-preparation/bridge-unsigned.json');owner=u['sender'];vault=u['transactions'][1]['transaction']['to'];amount=u['USDC_atoms'];w0=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));block=w0.eth.block_number
    with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
    with (d/'anvil.log').open('w') as log:
        proc=subprocess.Popen(['anvil','--silent','--host','127.0.0.1','--port',str(port),'--chain-id','1','--fork-url',PUBLIC_RPC,'--fork-block-number',str(block)],stdout=log,stderr=log)
        try:
            w=Web3(Web3.HTTPProvider(f'http://127.0.0.1:{port}',request_kwargs={'timeout':60}))
            for _ in range(120):
                if w.is_connected():break
                time.sleep(.25)
            guard(w);w.provider.make_request('anvil_impersonateAccount',[owner]);slot=next(x['balance_slot'] for x in load(OUT/'lp-qualification/result.json')['fixtures'] if x['token']==USDC);key=Web3.keccak(encode(['address','uint256'],[owner,slot]));result=w.provider.make_request('anvil_setStorageAt',[USDC,hx(key),hx(amount.to_bytes(32,'big'))]);assert 'error' not in result
            balance=lambda address:int.from_bytes(w.eth.call({'to':USDC,'data':Web3.keccak(text='balanceOf(address)')[:4]+encode(['address'],[address])}),'big');assert balance(owner)==amount;before=balance(vault);rows=[]
            for row in u['transactions']:
                guard(w);tx=dict(row['transaction'],**{'from':owner});tx['gas']=math.ceil(w.eth.estimate_gas(tx)*1.3);receipt=w.eth.wait_for_transaction_receipt(w.eth.send_transaction(tx));assert receipt.status==1;rows.append(dict(label=row['label'],gas_used=receipt.gasUsed,gas_budget=tx['gas']));save(d/(row['label']+'-receipt.json'),json.loads(w.to_json(receipt)))
            assert balance(owner)==0 and balance(vault)==before+amount;topic=Web3.keccak(text='ERC20BridgeDepositedV2(bytes32,address,bytes32,string,uint256,bytes32,bytes32,uint256,address,address)');logs=[x for x in receipt.logs if x['address']==vault and x['topics'][0]==topic];assert len(logs)==1;event=logs[0];p=decode(['string','uint256','bytes32','bytes32','uint256','address','address'],event['data']);assert p[0]==ALICE and p[1]==amount and hx(p[2])==u['deposit_nonce'] and p[3].hex()==u['route_binding'] and p[4]==1 and p[5].lower()==vault.lower() and p[6].lower()==USDC.lower();assert event['topics'][2][-20:].hex()==owner[2:].lower() and event['topics'][3]==Web3.keccak(text=ALICE)
            save(d/'result.json',dict(passed=True,fork_block=block,mainnet_transactions=0,real_USDC_and_vault_code=True,synthetic_USDC_funding_only=True,live_deposit_or_claim=False,fixture_USDC_atoms=amount,native_recipient=ALICE,route_binding=u['route_binding'],fixture_deposit_id=hx(event['topics'][1]),gas_used=sum(x['gas_used'] for x in rows),gas_budget=sum(x['gas_budget'] for x in rows),transactions=rows));print(json.dumps(load(d/'result.json')),flush=True)
        finally:proc.terminate();proc.wait(timeout=10)
if __name__=='__main__':main()
