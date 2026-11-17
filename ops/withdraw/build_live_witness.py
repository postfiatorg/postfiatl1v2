#!/usr/bin/env python3
"""Build a real egress witness from the public finalized archive, the six-validator burn receipt and the
finalized redemption record. Nothing is trusted here: the frozen verifier re-checks every field.
Usage: build_live_witness.py <archive.json> <actual-state.json> <after-six.json> <withdrawal_id> <prior_height> <out.json>"""
import json,sys
a,state,six,wid,prior_h,out=sys.argv[1:7];prior_h=int(prior_h)
A=json.load(open(a));S=json.load(open(state));six=json.load(open(six))
blocks={b['header']['height']:b for b in A['blocks']['blocks']}
red=[r for r in S['ledger']['vault_bridge_redemptions'] if r['redemption_id']==wid];assert len(red)==1;red=red[0]
p=red['withdrawal_packet'];assert p['withdrawal_id']==wid;h=red['created_at_height'];blk=blocks[h];assert A['tip']['height']>=h
rs=[json.dumps(x['receipts'],sort_keys=True) for x in six];assert len(six)==6 and len(set(rs))==1
receipt=[r for r in six[0]['receipts'] if r['tx_id']==p['burn_tx_id']];assert len(receipt)==1 and receipt[0]['accepted'] and blk['receipt_ids']==[p['burn_tx_id']]
c=[dict(r,active=r.get('active',True)) for r in A['validator_registry']['validators']]
step=lambda b:dict(block=b,committee_epoch=b['header']['consensus_v2_commit']['proposal']['domain']['committee_epoch'],committee=c)
bucket=[b for b in S['ledger']['vault_bridge_bucket_states'] if b['bucket_id']==red['bucket_id']][0]
prof=[r for r in A['governance']['vault_bridge_route_profiles'] if r['profile_hash']==bucket['policy_hash']];assert len(prof)==1
leaf=dict(schema='postfiat.bridge_exit_leaf.v1',accepted_receipt_code='accepted',accepted_receipt_id=p['burn_tx_id'],amount_atoms=p['amount_atoms'],asset_id=p['vault_bridge_asset_id'],burn_tx_id=p['burn_tx_id'],destination_hash=p['destination_hash'],evidence_root=p['evidence_root'],finalized_height=p['finalized_height'],recipient=p['recipient'],route_epoch=prof[0]['profile']['route_epoch'],source_bucket_id=p['source_bucket_id'],withdrawal_id=wid,withdrawal_packet_evm_digest=red['withdrawal_packet_evm_digest'],withdrawal_packet_hash=red['withdrawal_packet_hash'])
w=dict(schema='postfiat.pfusdc.egress_proof_witness.v1',chain_id=A['tip']['chain_id'],genesis_hash=A['tip']['genesis_hash'],protocol_version=A['tip']['protocol_version'],bridge_exit_root_activation_height=1,prior_checkpoint_block_id=blocks[prior_h]['header']['block_hash'],finality_ancestry=[step(blocks[x]) for x in range(prior_h+1,h)],route_profile=prof[0],receipt=receipt[0],merkle_proof=dict(leaf=leaf,leaf_count=1,leaf_index=0,siblings=[]),withdrawal_packet=p,withdrawal_packet_evm_digest=red['withdrawal_packet_evm_digest'],withdrawal_packet_hash=red['withdrawal_packet_hash'],**step(blk))
json.dump(w,open(out,'w'));print(json.dumps(dict(height=h,prior=prior_h,ancestry=len(w['finality_ancestry']),amount=p['amount_atoms'],recipient=p['recipient'],ingress_epoch=leaf['route_epoch'])))
