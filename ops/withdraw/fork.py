#!/usr/bin/env python3
"""Send only to an explicitly local Anvil fork; retain exact traces and deltas."""
import argparse,json
from pathlib import Path
from urllib.parse import urlparse
from web3 import Web3
from audit import VAULT,artifact,save

def field(data,tag,replacement):
    out=bytearray(data);cursor=21+int.from_bytes(out[17:21],'big')
    while cursor<len(out):
        t=int.from_bytes(out[cursor:cursor+2],'big');n=int.from_bytes(out[cursor+2:cursor+6],'big');start=cursor+6
        if t==tag:
            assert n==len(replacement);out[start:start+n]=replacement;return bytes(out)
        cursor=start+n
    raise ValueError('missing tag')

def main():
    p=argparse.ArgumentParser();p.add_argument('--rpc',default='http://127.0.0.1:18561');p.add_argument('--mode',choices=['diagnose','historical','copy'],required=True);p.add_argument('--proof-dir',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--witness',type=Path);a=p.parse_args()
    u=urlparse(a.rpc);assert u.scheme=='http' and u.hostname=='127.0.0.1' and u.port==18561,'local fork only'
    w=Web3(Web3.HTTPProvider(a.rpc,request_kwargs={'timeout':60}));assert 'anvil' in w.client_version.lower();assert w.eth.chain_id==1
    v=w.eth.contract(address=VAULT,abi=artifact('ERC20BridgeVaultL1')['abi']);f=w.eth.contract(address=v.functions.finalityVerifier().call(),abi=artifact('PFTLFinalityVerifierV1')['abi']);sender=w.eth.accounts[0]
    pv=(a.proof_dir/'public-values.bin').read_bytes();proof=(a.proof_dir/'proof-calldata.bin').read_bytes()
    decoded=f.functions.decodePublicValues(pv).call();recipient=decoded[17]
    token=w.eth.contract(address=v.functions.token().call(),abi=[{'type':'function','name':'balanceOf','stateMutability':'view','inputs':[{'name':'x','type':'address'}],'outputs':[{'name':'','type':'uint256'}]}])
    def balances():return {'vault':token.functions.balanceOf(VAULT).call(),'recipient':token.functions.balanceOf(recipient).call(),'obligations':v.functions.totalObligations().call()}
    def rpc(m,p):
        r=w.provider.make_request(m,p);assert 'error' not in r,r;return r['result']
    def send(label,values,proof_bytes,expected):
        before=balances();tx=v.functions.withdrawWithProof(values,proof_bytes).transact({'from':sender,'gas':3_000_000});receipt=w.eth.wait_for_transaction_receipt(tx);after=balances();trace=rpc('debug_traceTransaction',[tx.hex(),{'tracer':'callTracer'}]);save(a.output/f'{label}-trace.json',trace)
        row={'tx_hash':tx,'receipt':dict(receipt),'before':before,'after':after,'trace_output':trace.get('output'),'expected_status':expected};save(a.output/f'{label}.json',row);assert receipt.status==expected,(label,row)
        if not expected:assert before==after,(label,'failed call mutated money')
        return row
    initial=rpc('evm_snapshot',[]);overrides=[]
    try:
        if a.mode=='diagnose':
            live=json.loads((a.output.parent/'audit/native-vault_bridge_route.json').read_text())['result']
            current=field(field(pv,6,bytes.fromhex(live['profile_hash'])),7,(11).to_bytes(8,'big'))
            send('epoch11-profile-rejected',current,proof,0)
            send('epoch11-epoch-only-rejected',field(pv,7,(11).to_bytes(8,'big')),proof,0)
            # Neither selector is present in deployed vault; owner has no setter.
            owner=v.functions.owner().call();rpc('anvil_impersonateAccount',[owner]);rpc('anvil_setBalance',[owner,hex(10**18)])
            for sig in ('setFinalityVerifier(address)','setVerifier(address)'):
                data=Web3.keccak(text=sig)[:4]+bytes(12)+bytes.fromhex(f.address[2:]);tx=w.eth.send_transaction({'from':owner,'to':VAULT,'data':data,'gas':100000});r=w.eth.wait_for_transaction_receipt(tx);assert r.status==0
                save(a.output/(sig.split('(')[0]+'-trace.json'),rpc('debug_traceTransaction',[tx.hex(),{'tracer':'callTracer'}]))
            rpc('anvil_stopImpersonatingAccount',[owner])
        else:
            if a.mode=='copy':
                assert a.witness;wi=json.loads(a.witness.read_text());prior=bytes.fromhex(wi['prior_checkpoint_block_id']);root=decoded[8]
                # Explicitly disclosed fixture trust anchor. No code, USDC
                # balances, obligations, nullifiers, or route pins are changed.
                for slot,value in [(Web3.keccak(Web3.keccak(prior)+(3).to_bytes(32,'big')),(1).to_bytes(32,'big')),(Web3.keccak(Web3.keccak(prior)+(4).to_bytes(32,'big')),root)]:
                    old=w.eth.get_storage_at(f.address,int.from_bytes(slot,'big'));rpc('anvil_setStorageAt',[f.address,'0x'+slot.hex(),'0x'+value.hex()]);overrides.append({'contract':f.address,'slot':slot,'before':old,'after':value})
                save(a.output/'fixture-storage-overrides.json',overrides)
                send('stale-signer-set',field(pv,11,bytes.fromhex('99'*48)),proof,0)
            wrong_recipient=field(pv,28,bytes.fromhex('00000000000000000000000000000000000b0b00'))
            send('wrong-recipient',wrong_recipient,proof,0)
            send('wrong-amount',field(pv,27,(decoded[16]+1).to_bytes(8,'big')),proof,0)
            good=send('withdrawal',pv,proof,1)
            assert good['after']['recipient']-good['before']['recipient']==decoded[16]
            assert good['before']['vault']-good['after']['vault']==decoded[16]
            assert good['before']['obligations']-good['after']['obligations']==decoded[16]
            send('replay',pv,proof,0)
            save(a.output/'result.json',{'passed':True,'amount_atoms':decoded[16],'recipient':recipient,'gas':good['receipt']['gasUsed'],'real_groth16':True,'deployed_vault_and_verifier_code_unchanged':True,'state_override_count':len(overrides),'real_live_finality':a.mode=='historical','historical_receipt_only':a.mode=='historical','new_epoch11_copy_burn':a.mode=='copy','no_mainnet_transaction':True})
    finally:rpc('evm_revert',[initial])
    print(a.mode+' passed')
if __name__=='__main__':main()
