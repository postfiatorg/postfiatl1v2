#!/usr/bin/env python3
"""Create once at the founder-approved path; disclose only the public address."""
import argparse
import json
import os
from pathlib import Path
import secrets
import stat
from datetime import datetime, timezone
from eth_account import Account
from web3 import Web3
import rlp

KEY=Path('/home/postfiat/.config/b24/eth-operator.key')
OUT=Path('/home/postfiat/repos/b23-coord/evidence/tee/phase5')
RPC='https://ethereum-rpc.publicnode.com'
USDC='0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48'

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--create-once',action='store_true',required=True);ap.parse_args()
    os.umask(0o077);KEY.parent.mkdir(mode=0o700,exist_ok=True)
    assert not KEY.parent.is_symlink() and stat.S_IMODE(KEY.parent.stat().st_mode)==0o700
    created=False
    if not KEY.exists():
        account=Account.from_key(secrets.token_bytes(32))
        fd=os.open(KEY,os.O_CREAT|os.O_EXCL|os.O_WRONLY|os.O_NOFOLLOW,0o600)
        with os.fdopen(fd,'w') as stream:
            stream.write(account.key.hex()+'\n');stream.flush();os.fsync(stream.fileno())
        created=True
    else:
        assert not KEY.is_symlink() and stat.S_IMODE(KEY.stat().st_mode)==0o600
        account=Account.from_key(KEY.read_text().strip())
    address=account.address
    w=Web3(Web3.HTTPProvider(RPC,request_kwargs={'timeout':30}));assert w.eth.chain_id==1
    block=w.eth.block_number;nonce=w.eth.get_transaction_count(address,block_identifier=block);assert nonce==0
    predicted={name:Web3.to_checksum_address(Web3.keccak(rlp.encode([bytes.fromhex(address[2:]),n]))[-20:]) for n,name in enumerate(('token','verifier','controller'))}
    usdc=w.eth.call({'to':USDC,'data':'0x70a08231'+address[2:].lower().zfill(64)},block_identifier=block)
    result=dict(created=created,address=address,deployer=address,governance=address,recipient=address,lp_owner=address,gas_payer=address,nonce=nonce,**predicted,production_roles_appointed=True,mainnet_deployed=False,
        observed_at=datetime.now(timezone.utc).isoformat(),chain_id=1,block=block,ETH_wei=w.eth.get_balance(address,block_identifier=block),USDC_atoms=int.from_bytes(usdc,'big'),key_path=str(KEY),key_mode='600',directory_mode='700',rpc=RPC)
    (OUT/'operator.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))

if __name__=='__main__':main()
