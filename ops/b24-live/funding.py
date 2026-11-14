#!/usr/bin/env python3
"""Refresh public funding evidence; optionally deploy only if the dedicated EOA is funded."""
import argparse,stat
from datetime import datetime,timezone
from decimal import Decimal
from eth_account import Account
from eth_abi import encode
from deployment import OUT,Web3,PUBLIC_RPC,SP1,USDC,artifact,save,load
from sign import ALICE
from rpc import call
KEY=__import__('pathlib').Path('/home/postfiat/.config/b24/eth-operator.key')
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--deploy-if-funded',action='store_true');a=ap.parse_args()
    w=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));assert w.eth.chain_id==1
    op=load(OUT/'operator.json');p=load(OUT/'deployment-pins.json');unsigned=load(OUT/'ethereum-unsigned.json');owner=op['address'];block=w.eth.block_number
    base=w.eth.get_block(block)['baseFeePerGas'];priority=w.eth.max_priority_fee;fee=2*base+priority;eth=w.eth.get_balance(owner,block_identifier=block)
    usdc=int.from_bytes(w.eth.call({'to':USDC,'data':'0x70a08231'+owner[2:].lower().zfill(64)},block_identifier=block),'big')
    gas=load(OUT/'gas-funding.json');budget=gas['budget_gas']+325000 # 250k ingress approve/deposit allowance plus 30%.
    wanted=((budget*fee+10**14-1)//10**14)*10**14
    route=call('vault_bridge_route',{'asset_id':load(OUT/'b24-app-config.json')['settlement_asset_id']});save(OUT/'bridge-route-current.json',route)
    vault=Web3.to_checksum_address(route['profile']['vault_address']);code=w.eth.get_code(vault,block_identifier=block)
    assert '0x'+Web3.keccak(code).hex()==route['profile']['vault_runtime_code_hash']
    paused=int.from_bytes(w.eth.call({'to':vault,'data':'0x'+Web3.keccak(text='paused()')[:4].hex()},block_identifier=block),'big');assert paused==0
    nonce=Web3.keccak(text='b24-creation-5000-phase5-20261114');binding=bytes.fromhex(route['route_binding'])
    data='0x'+(Web3.keccak(text='depositV2(uint256,string,bytes32,bytes32)')[:4]+encode(['uint256','string','bytes32','bytes32'],[5025000000,ALICE,nonce,binding])).hex()
    save(OUT/'bridge-funding-unsigned.json',dict(status='UNFUNDED_DO_NOT_SEND_WITHOUT_FRESH_ROUTE_AND_READINESS',chain_id=1,sender=owner,approve=dict(to=USDC,data='0x'+(Web3.keccak(text='approve(address,uint256)')[:4]+encode(['address','uint256'],[vault,5025000000])).hex()),deposit=dict(to=vault,data=data,value=0),native_recipient=ALICE,route_epoch=route['governance_route_epoch'],route_binding=route['route_binding'],USDC_atoms=5025000000,no_live_transaction=True))
    for row in unsigned['contracts']+[unsigned['pool_initialization']]:
        row['transaction']['maxFeePerGas']=fee;row['transaction']['maxPriorityFeePerGas']=priority
    unsigned['gas_refreshed_at_block']=block;save(OUT/'ethereum-unsigned.json',unsigned)
    result=dict(observed_at=datetime.now(timezone.utc).isoformat(),block=block,operator=owner,ETH_wei=eth,USDC_atoms=usdc,base_fee_wei=base,priority_fee_wei=priority,max_fee_per_gas_wei=fee,budget_gas=budget,exact_ETH_to_send=str(Decimal(max(0,wanted-eth))/Decimal(10**18)),total_ETH_budget=str(Decimal(wanted)/Decimal(10**18)),pool_USDC='2000',creation_USDC_at_NAV_1='5025',operator_total_USDC_if_sending_both='7025',creation_vault=vault,native_recipient=ALICE,source_S_accepted_without_conversion=True,source_proof='final-copy/conditional-source-5000/result.json',gas_basis='125% measured deployment/initialize estimates; 130% future proof/consume/LP/approvals/ingress allowances; maxFee=2*baseFee+priority; rounded up to 0.0001 ETH',funding_gate=eth==0,mainnet_deployed=False)
    save(OUT/'funding-final.json',result)
    if not a.deploy_if_funded or eth==0:print(__import__('json').dumps(result));return
    assert eth>=sum(x['transaction']['gas'] for x in unsigned['contracts'])*fee,'ETH below paused-deployment maximum fee budget'
    assert w.eth.get_transaction_count(owner,'pending')==0
    assert '0x'+Web3.keccak(w.eth.get_code(SP1)).hex()==p['sp1_runtime_code_hash']
    assert all(not w.eth.get_code(op[k]) for k in ('token','verifier','controller'))
    assert not KEY.is_symlink() and stat.S_IMODE(KEY.stat().st_mode)==0o600 and stat.S_IMODE(KEY.parent.stat().st_mode)==0o700
    account=Account.from_key(KEY.read_text().strip());assert account.address==owner
    d=OUT/'mainnet-deployment';d.mkdir(mode=0o700,exist_ok=False);receipts=[]
    for i,row in enumerate(unsigned['contracts']):
        tx=dict(row['transaction']);tx.pop('from');assert tx['nonce']==i and w.eth.get_transaction_count(owner,'pending')==i
        signed=account.sign_transaction(tx);save(d/f'{i}-submitted.json',dict(label=row['label'],expected_transaction_hash=signed.hash.hex()))
        h=w.eth.send_raw_transaction(signed.raw_transaction) # Exactly one send, never automatic resubmit.
        assert h==signed.hash;r=w.eth.wait_for_transaction_receipt(h,timeout=300);save(d/f'{i}-receipt.json',__import__('json').loads(w.to_json(r)));assert r.status==1
        if i<3:
            name=('token','verifier','controller')[i];assert r.contractAddress==op[name];assert '0x'+Web3.keccak(w.eth.get_code(op[name])).hex()==p[name+'_runtime_code_hash']
        receipts.append(dict(label=row['label'],tx_hash=h.hex(),block=r.blockNumber,status=r.status))
    token=w.eth.contract(address=op['token'],abi=artifact('WrappedVenueNAVCoin')['abi']);controller=w.eth.contract(address=op['controller'],abi=artifact('PFTLUniswapPrimaryMarketV3')['abi'])
    assert token.functions.controller_locked().call() and token.functions.totalSupply().call()==0 and token.functions.owner().call()==owner
    assert controller.functions.mintPaused().call() and controller.functions.returnPaused().call() and controller.functions.receiptVerifier().call()==op['verifier']
    result.update(mainnet_deployed=True,funding_gate=False,receipts=receipts,mint_paused=True,return_paused=True,supply=0);save(OUT/'funding-final.json',result);print(__import__('json').dumps(result))
if __name__=='__main__':main()
