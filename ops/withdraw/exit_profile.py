#!/usr/bin/env python3
"""Select the deployed exit profile without rewriting the authenticated burn.

The bucket/leaf keep their ingress epoch. Only the witness's exit descriptor
uses the profile pinned by Ethereum. The frozen guest authenticates all payout
fields against the voted leaf and checks the descriptor's asset and destination.
This script makes no transactions and does not handle signing keys.
"""
import argparse,copy,json
from pathlib import Path
from web3 import Web3
from audit import save

def select(witness,snapshot,audit):
    c=audit['contracts'];v=c['PFTLFinalityVerifierV1']['reads'];vault=c['ERC20BridgeVaultL1']
    if not all(x['executable_match_except_immutables_metadata'] for x in c.values()):raise ValueError('unqualified contract code')
    if vault['reads']['paused']:raise ValueError('vault paused')
    if vault['reads']['finalityVerifier'].lower()!=c['PFTLFinalityVerifierV1']['address'].lower():raise ValueError('verifier mismatch')
    if v['programVKey']!='0x0015b046ba4b80c0ca7e2d9429a1f5fd88bc6d1d328cca6acec29ffdf48a9d87':raise ValueError('unqualified frozen program')
    digest=lambda s:'0x'+Web3.keccak(bytes.fromhex(s.removeprefix('0x'))).hex()
    rows=[r for r in snapshot['governance']['vault_bridge_route_profiles'] if digest(r['profile_hash'])==v['routeProfileHashCommitment'] and r['profile']['route_epoch']==v['routeEpoch']]
    if len(rows)!=1:raise ValueError('deployed exit profile not unique in exported governance')
    exit_record=rows[0];p=exit_record['profile'];packet=witness['withdrawal_packet'];leaf=witness['merkle_proof']['leaf'];height=witness['block']['header']['height']
    for name in ('asset_id','source_chain_id','vault_address','token_address'):
        packet_name='vault_bridge_asset_id' if name=='asset_id' else name
        if p[name]!=packet[packet_name]:raise ValueError('exit profile destination mismatch: '+name)
    if p['vault_address'].lower()!=vault['address'].lower() or p['token_address'].lower()!=v['token'].lower():raise ValueError('wrong live contracts')
    for f in ('vault_runtime_code_hash','token_runtime_code_hash'):
        vf={'vault_runtime_code_hash':'vaultRuntimeCodeHash','token_runtime_code_hash':'tokenRuntimeCodeHash'}[f]
        if p[f]!=v[vf]:raise ValueError('wrong code pin: '+f)
    if digest(p['asset_id'])!=v['assetIdCommitment']:raise ValueError('wrong family')
    if not p['activation_height']<=height<p['expires_at_height']:raise ValueError('exit profile outside validity interval')
    if ('0x'+Web3.keccak(text=witness['chain_id']).hex()!=v['pftlChainIdHash'] or digest(witness['genesis_hash'])!=v['pftlGenesisHashCommitment'] or witness['protocol_version']!=v['pftlProtocolVersion']):raise ValueError('wrong native chain')
    if packet['amount_atoms']<=0 or packet['amount_atoms']>vault['reads']['totalObligations'] or packet['amount_atoms']>audit['vault_usdc_atoms']:raise ValueError('payout exceeds live solvency')
    for k,pk in [('asset_id','vault_bridge_asset_id'),('source_bucket_id','source_bucket_id'),('amount_atoms','amount_atoms'),('recipient','recipient'),('burn_tx_id','burn_tx_id'),('withdrawal_id','withdrawal_id')]:
        if leaf[k]!=packet[pk]:raise ValueError('leaf/packet mismatch: '+k)
    buckets=[b for b in snapshot['ledger']['vault_bridge_bucket_states'] if b['bucket_id']==packet['source_bucket_id']]
    if len(buckets)!=1 or buckets[0]['asset_id']!=p['asset_id']:raise ValueError('unknown source bucket')
    bucket=buckets[0]
    source=f"erc20_bridge_vault:{p['source_chain_id']}:{p['vault_address']}:{p['token_address']}"
    if bucket['source_domain']!=source:raise ValueError('source bucket belongs to another vault')
    ingress=[r for r in snapshot['governance']['vault_bridge_route_profiles'] if r['profile_hash']==bucket['policy_hash']]
    if len(ingress)!=1 or leaf['route_epoch']!=ingress[0]['profile']['route_epoch']:raise ValueError('leaf ingress epoch mismatch')
    if witness['route_profile']!=ingress[0]:raise ValueError('input must contain original bucket profile')
    result=copy.deepcopy(witness);result['route_profile']=copy.deepcopy(exit_record)
    assert {k:v for k,v in result.items() if k!='route_profile'}=={k:v for k,v in witness.items() if k!='route_profile'}
    return result

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--witness',type=Path,required=True);p.add_argument('--snapshot',type=Path,required=True);p.add_argument('--audit',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    load=lambda p:json.loads(p.read_text());save(a.output,select(load(a.witness),load(a.snapshot),load(a.audit)))
