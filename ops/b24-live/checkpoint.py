#!/usr/bin/env python3
"""Authenticate a real live checkpoint using public committee keys and the frozen guest."""
from concurrent.futures import ThreadPoolExecutor
from sign import *
from rpc import call
OUT=COORD/'evidence/tee/phase5'
ELF=Path('/mnt/HC_Volume_106212907/stakehub-hyperliquid/l1-security-evidence-20261031/receipt-v2-final-elf/pftl-uniswap-receipt-v2-program')
PROVER=Path('/mnt/HC_Volume_106212907/stakehub-hyperliquid/l1-security-target-20261031/release/pftl-uniswap-prover')
def main():
    s=load(OUT/'before-state.json');height=s['tip']['height']
    def read(i):return dict(info=call('server_info',node=i),blocks=call('blocks',{'from_height':height-1,'limit':2},i))
    with ThreadPoolExecutor(max_workers=6) as pool:rows=list(pool.map(read,range(6)))
    save(OUT/'checkpoint-six-validator.json',rows)
    assert all(r['info']['ledger']['height']==height and r['info']['ledger']['state_root']==s['tip']['state_root'] for r in rows)
    assert all(r['blocks']==rows[0]['blocks'] for r in rows)
    prior,block=rows[0]['blocks'];assert prior['header']['height']==height-1 and block['header']['height']==height
    assert block['header']['block_hash']==s['tip']['block_hash']
    registry=load(OUT/'checkpoint-public-validator-registry.json')['validators'];committee=[dict(r,active=True) for r in sorted(registry,key=lambda r:r['node_id'])]
    domain=block['header']['consensus_v2_commit']['proposal']['domain']
    w=dict(schema='postfiat-pftl-uniswap-checkpoint-proof-witness-v2',chain_id=s['tip']['chain_id'],genesis_hash=s['tip']['genesis_hash'],protocol_version=1,prior_checkpoint_block_id=prior['header']['block_hash'],finality_ancestry=[],block=block,committee_epoch=domain['committee_epoch'],committee=committee,transition_announcements=[])
    save(OUT/'checkpoint-witness-v2.json',w)
    with (OUT/'checkpoint-verify.log').open('wb') as log:subprocess.run([str(PROVER),'checkpoint','--witness',str(OUT/'checkpoint-witness-v2.json'),'--elf',str(ELF),'--proof-program-version','2','--output-dir',str(OUT/'checkpoint-execute')],stdout=log,stderr=subprocess.STDOUT,check=True)
    values=(OUT/'checkpoint-execute/public-values.bin').read_bytes();assert len(values)==320 and int.from_bytes(values[:32],'big')==2
    checkpoint=Web3.keccak(bytes.fromhex(block['header']['block_hash'])).hex();committee_root=Web3.keccak(bytes.fromhex(domain['committee_root'])).hex()
    assert values[160:192].hex()==checkpoint and int.from_bytes(values[192:224],'big')==height and values[256:288].hex()==committee_root
    save(OUT/'checkpoint-pins.json',dict(passed=True,real_live_finality=True,synthetic_committee=False,initial_height=height,initial_checkpoint_block_id=block['header']['block_hash'],initial_checkpoint_commitment=checkpoint,initial_committee_root=domain['committee_root'],initial_committee_root_commitment=committee_root,guest_elf_sha256=hashlib.sha256(ELF.read_bytes()).hexdigest(),public_values_sha256=hashlib.sha256(values).hexdigest(),six_validators_agree=True,native_and_guest_verification=True,groth16_proof_generated=False))
    print('Real committee signatures and frozen V2 checkpoint execution verified on six-node checkpoint')
if __name__=='__main__':main()
