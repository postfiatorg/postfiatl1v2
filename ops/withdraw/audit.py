#!/usr/bin/env python3
"""Read-only, block-pinned audit of the deployed PFUSDC exit contracts."""
import argparse
import concurrent.futures
import json
import socket
import time
from pathlib import Path
from web3 import Web3

ROOT = Path(__file__).resolve().parents[2]
VAULT = Web3.to_checksum_address('0xe7a432a28b20621a70e845c7f202004cf98e7a2c')
FAMILY = '02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b'

def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, default=lambda x: '0x'+x.hex() if isinstance(x,bytes) else str(x))+'\n')

def native(method, params=None, node=0):
    with socket.create_connection(('127.0.0.1',39650+node),20) as s:
        s.settimeout(60)
        s.sendall((json.dumps(dict(version='postfiat-local-rpc-v1',id='withdraw-read-'+str(time.time_ns()),method=method,params=params or {}))+'\n').encode())
        return json.loads(s.makefile().readline())

def artifact(name):
    return json.loads((ROOT/f'crates/ethereum-contracts/out/{name}.sol/{name}.json').read_text())

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--output',type=Path,required=True);ap.add_argument('--rpc',default='https://ethereum-rpc.publicnode.com');a=ap.parse_args()
    w=Web3(Web3.HTTPProvider(a.rpc,request_kwargs={'timeout':45}));assert w.eth.chain_id==1
    block=w.eth.block_number; b=w.eth.get_block(block)
    result={'block':block,'block_hash':b.hash,'timestamp':b.timestamp,'gas_price_wei':w.eth.gas_price,'base_fee_wei':b.baseFeePerGas,'max_priority_fee_wei':w.eth.max_priority_fee,'contracts':{}}
    v=w.eth.contract(address=VAULT,abi=artifact('ERC20BridgeVaultL1')['abi'])
    verifier=v.functions.finalityVerifier().call(block_identifier=block)
    for name,address in [('ERC20BridgeVaultL1',VAULT),('PFTLFinalityVerifierV1',verifier)]:
        art=artifact(name);c=w.eth.contract(address=address,abi=art['abi']);code=w.eth.get_code(address,block)
        (a.output/f'{name}.runtime.hex').parent.mkdir(parents=True,exist_ok=True)
        (a.output/f'{name}.runtime.hex').write_text('0x'+code.hex()+'\n')
        reads={}
        for f in art['abi']:
            if f['type']=='function' and not f['inputs'] and f['stateMutability'] in ('view','pure'):
                try: reads[f['name']]=getattr(c.functions,f['name'])().call(block_identifier=block)
                except Exception as e:reads[f['name']]={'error':str(e)}
        local=bytearray.fromhex(art['deployedBytecode']['object'].removeprefix('0x'));live=bytearray(code)
        for spans in art['deployedBytecode']['immutableReferences'].values():
            for span in spans:
                for buf in (local,live):buf[span['start']:span['start']+span['length']]=bytes(span['length'])
        # Solidity metadata hashes can differ with source paths; report separately.
        def executable(buf):return buf[:-(int.from_bytes(buf[-2:],'big')+2)]
        result['contracts'][name]={'address':address,'code_hash':Web3.keccak(code),'code_length':len(code),'reads':reads,'storage':{str(i):w.eth.get_storage_at(address,i,block) for i in range(9)},'source_match_except_immutables':local==live,'executable_match_except_immutables_metadata':executable(local)==executable(live),'external_functions':[f['name'] for f in art['abi'] if f['type']=='function']}
    token=result['contracts']['ERC20BridgeVaultL1']['reads']['token']
    balance_abi=[{'type':'function','name':'balanceOf','stateMutability':'view','inputs':[{'name':'owner','type':'address'}],'outputs':[{'name':'','type':'uint256'}]}]
    result['vault_usdc_atoms']=w.eth.contract(address=token,abi=balance_abi).functions.balanceOf(VAULT).call(block_identifier=block)
    save(a.output/'mainnet.json',result)
    queries=[('status',{}),('vault_bridge_route',{'route_id':'ethereum-mainnet-usdc-v1'}),('vault_bridge_status',{'asset_id':FAMILY})]
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        fs={pool.submit(native,m,p):m for m,p in queries}
        for f,m in fs.items():
            try:save(a.output/f'native-{m}.json',f.result())
            except Exception as e:save(a.output/f'native-{m}.json',{'error':str(e)})
    print(json.dumps({'block':block,'verifier':verifier,'balance_atoms':result['vault_usdc_atoms'],'route_epoch':result['contracts']['PFTLFinalityVerifierV1']['reads']['routeEpoch'],'source_matches':{n:c['executable_match_except_immutables_metadata'] for n,c in result['contracts'].items()}}))

if __name__=='__main__':main()
