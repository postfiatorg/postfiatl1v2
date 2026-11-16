#!/usr/bin/env python3
"""Export bounded, authentic checkpoint witnesses from a public node archive.

This version fails closed on committee changes; those require the node's
authenticated transition exporter. It never opens a NodeStore or key file.
"""
import argparse,hashlib,json,subprocess
from pathlib import Path
from web3 import Web3
from audit import save

def main():
    p=argparse.ArgumentParser();p.add_argument('--archive',type=Path,required=True);p.add_argument('--audit',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--host',type=Path,required=True);p.add_argument('--elf',type=Path,required=True);a=p.parse_args()
    s=json.loads(a.archive.read_text());audit=json.loads(a.audit.read_text());v=audit['contracts']['PFTLFinalityVerifierV1']['reads'];blocks={b['header']['height']:b for b in s['blocks']['blocks']};prior=v['latestFinalizedHeight'];tip=s['tip']['height'];c=s['validator_registry']['validators'];c=[dict(r,active=r.get('active',True)) for r in c]
    k=lambda s:'0x'+Web3.keccak(bytes.fromhex(s)).hex()
    assert k(blocks[prior]['header']['block_hash'])==v['latestCheckpointCommitment']
    assert blocks[tip]['header']['block_hash']==s['tip']['block_hash']
    for height in range(prior+1,tip+1):
        d=blocks[height]['header']['consensus_v2_commit']['proposal']['domain']
        assert k(d['committee_root'])==v['latestCommitteeRootCommitment'],'committee transition requires authenticated exporter'
        assert blocks[height]['header']['parent_hash']==blocks[height-1]['header']['block_hash']
    sequence=[]
    while prior<tip:
        end=min(prior+65,tip);b=blocks[end];domain=b['header']['consensus_v2_commit']['proposal']['domain'];d=a.output/f'{prior}-{end}';d.mkdir(parents=True,exist_ok=True)
        step=lambda b:dict(block=b,committee_epoch=b['header']['consensus_v2_commit']['proposal']['domain']['committee_epoch'],committee=c)
        witness=dict(schema='postfiat.pfusdc.checkpoint_proof_witness.v1',chain_id=s['tip']['chain_id'],genesis_hash=s['tip']['genesis_hash'],protocol_version=s['tip']['protocol_version'],prior_checkpoint_block_id=blocks[prior]['header']['block_hash'],finality_ancestry=[step(blocks[h]) for h in range(prior+1,end)],**step(b))
        save(d/'witness.json',witness)
        subprocess.run([str(a.host),'checkpoint',str(a.elf),str(d/'witness.json'),str(d/'native'),'native'],check=True)
        sequence.append(dict(prior_height=prior,target_height=end,prior_block=blocks[prior]['header']['block_hash'],target_block=b['header']['block_hash'],witness=str(d/'witness.json'),witness_sha256=hashlib.sha256((d/'witness.json').read_bytes()).hexdigest(),ancestor_count=end-prior-1,native_verified=True))
        prior=end
    save(a.output/'sequence.json',dict(source_archive_sha256=hashlib.sha256(a.archive.read_bytes()).hexdigest(),source_tip=s['tip'],initial_ethereum_block=audit['block'],committee_root_commitment=v['latestCommitteeRootCommitment'],committee_fixture=False,checkpoints=sequence))
    print(json.dumps({'source_tip':tip,'checkpoint_count':len(sequence),'targets':[r['target_height'] for r in sequence]}))
if __name__=='__main__':main()
