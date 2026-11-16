#!/usr/bin/env python3
"""Send one proof-carrying call to Ethereum mainnet from the dedicated operator (key by path, never printed).
Usage: eth_send.py checkpoint|withdraw <proof_dir> <out.json>"""
import json,sys
from pathlib import Path
from eth_account import Account
from web3 import Web3
KEY=Path('/home/postfiat/.config/b24/eth-operator.key');RPC='https://ethereum-rpc.publicnode.com'
VERIFIER='0xc398a26BD997168C3C966763B0A97829d14dB655';VAULT='0xE7A432a28b20621A70E845C7F202004Cf98e7A2C';USDC='0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48'
ABI=[{"type":"function","name":n,"stateMutability":"nonpayable","inputs":[{"name":"publicValues","type":"bytes"},{"name":"proofBytes","type":"bytes"}],"outputs":o} for n,o in (('advanceCheckpoint',[]),('withdrawWithProof',[{"name":"","type":"bytes32"}]))]+[{"type":"function","name":"latestFinalizedHeight","stateMutability":"view","inputs":[],"outputs":[{"name":"","type":"uint64"}]},{"type":"function","name":"balanceOf","stateMutability":"view","inputs":[{"name":"a","type":"address"}],"outputs":[{"name":"","type":"uint256"}]}]
def main():
    mode,d,out=sys.argv[1],Path(sys.argv[2]),Path(sys.argv[3]);assert not out.exists()
    report=json.loads((d/'proof-report.json').read_text());assert report['verified'] and report['proof_mode']=='groth16'
    values=(d/'public-values.bin').read_bytes();proof=(d/'proof-calldata.bin').read_bytes()
    w=Web3(Web3.HTTPProvider(RPC,request_kwargs={'timeout':60}));assert w.eth.chain_id==1
    acct=Account.from_key(KEY.read_text().strip());assert acct.address=='0xF77B1876803c01967C5a62EF28C5f6de4574F7E1'
    ver=w.eth.contract(address=VERIFIER,abi=ABI);vault=w.eth.contract(address=VAULT,abi=ABI);usdc=w.eth.contract(address=USDC,abi=ABI)
    fn=ver.functions.advanceCheckpoint(values,proof) if mode=='checkpoint' else vault.functions.withdrawWithProof(values,proof)
    before=dict(height=ver.functions.latestFinalizedHeight().call(),vault_usdc=usdc.functions.balanceOf(VAULT).call(),operator_usdc=usdc.functions.balanceOf(acct.address).call(),operator_wei=w.eth.get_balance(acct.address))
    fn.call({'from':acct.address});gas=fn.estimate_gas({'from':acct.address})
    base=w.eth.get_block('latest')['baseFeePerGas'];tip=w.to_wei('0.05','gwei');maxfee=base*2+tip;assert maxfee*gas*2<w.to_wei('0.002','ether'),'gas cost guard'
    tx=fn.build_transaction({'from':acct.address,'nonce':w.eth.get_transaction_count(acct.address),'gas':int(gas*1.3),'maxFeePerGas':maxfee,'maxPriorityFeePerGas':tip,'chainId':1})
    signed=acct.sign_transaction(tx);h=w.eth.send_raw_transaction(signed.raw_transaction);r=w.eth.wait_for_transaction_receipt(h,timeout=600)
    after=dict(height=ver.functions.latestFinalizedHeight().call(),vault_usdc=usdc.functions.balanceOf(VAULT).call(),operator_usdc=usdc.functions.balanceOf(acct.address).call(),operator_wei=w.eth.get_balance(acct.address))
    res=dict(mode=mode,tx=h.hex(),status=r.status,block=r.blockNumber,gas_used=r.gasUsed,cost_wei=r.gasUsed*r.effectiveGasPrice,before=before,after=after)
    out.write_text(json.dumps(res,indent=1));print(json.dumps(res));assert r.status==1
if __name__=='__main__':main()
