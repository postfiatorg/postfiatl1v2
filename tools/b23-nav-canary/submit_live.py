#!/usr/bin/env python3
"""Execute only the five canary-qualified, user-authorized B23 NAV operations.

Requires a passing real-proof canary, unchanged six-node parent, exact release-4
runtime hashes, and chain.lock=TEE. Never mints, burns, redeems or places offers.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import fcntl
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

sys.path.insert(0,'/home/postfiat/repos/postfiatl1v2-navcoin-proof-b2.2/.scratch-deposit')
import pftl
import facility as f

COORD=Path('/home/postfiat/repos/b23-coord')
LOCK=COORD/'chain.lock'
NODE=COORD/'artifacts/release4/postfiat-node'
RELEASE_SHA='3039c21af67420a9842b5644eb580b2809081a97419e3e8e1f849d49a487cb6f'
EXPECTED_OPS=['nav_profile_register','nav_asset_register','nav_reserve_submit','nav_epoch_finalize','pftl_uniswap_route_epoch_advance']
def load(p):return json.loads(p.read_bytes())
def save(p,v):pftl.write_json(p,v)
def top(text):
    path=COORD/'status/tee.md';lines=path.read_text().splitlines();lines[0]=text;path.write_text('\n'.join(lines[:60])+'\n')
def require_lock():
    if LOCK.read_text().strip()!='TEE':raise RuntimeError('TEE chain lock lost')

def runtime(peer):
    name,host,_=peer
    # Names come from the fixed six-peer inventory, never caller input.
    command=f'for unit in postfiat-{name}.service postfiat-{name}-rpc.service; do pid=$(systemctl show --value -p MainPID "$unit"); test "$pid" -gt 0 || exit 1; sha256sum /proc/$pid/exe; done'
    r=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=10','root@'+host,command],capture_output=True,text=True,timeout=30)
    hashes=[line.split()[0] for line in r.stdout.splitlines()]
    return name,dict(returncode=r.returncode,hashes=hashes,matches=r.returncode==0 and hashes==[RELEASE_SHA]*2)

def reads(peer,asset):
    name=peer[0]
    return name,{method:pftl.rpc(method,params,'tee-final-'+method,peer) for method,params in [
        ('server_info',{}),('status',{}),('asset_info',{'asset_id':asset}),('nav_reserve_proof_status',{'asset_id':asset}),
        ('navcoin_bridge_supply_status',{'route_id':f.ROUTE_ID})]}

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--canary',type=Path,required=True)
    ap.add_argument('--output',type=Path,required=True);ap.add_argument('--execute',action='store_true')
    args=ap.parse_args();gate=load(args.canary/'result.json');expected=load(args.canary/'expected.json')
    plan_path=args.canary/'authorized-live-operations.json';plan=load(plan_path)
    assert gate['passed'] is True and gate['no_live_transactions'] is True
    assert gate['authorized_operations_sha256']==hashlib.sha256(plan_path.read_bytes()).hexdigest()
    assert [r['operation']['operation'] for r in plan]==EXPECTED_OPS
    assert all(r['source']==f.ISSUER for r in plan)
    asset=plan[1]['operation']['asset_id']
    assert asset=='ca7845134c8e95819082fcd2f1612808e20a37dfabab98dab120f99fd2c54c18ffb50d98c0664480420c881773244627'
    assert all(plan[i]['operation']['asset_id']==asset for i in (1,2,3))
    assert plan[4]['operation']['route_id']==f.ROUTE_ID
    assert hashlib.sha256(NODE.read_bytes()).hexdigest()==RELEASE_SHA
    if not args.execute:
        print(json.dumps({'canary_gate_passed':True,'operation_count':5,'chain_writes':0}));return
    args.output.mkdir(parents=True,exist_ok=False,mode=0o700)
    pftl.NODE=NODE;pftl.EVIDENCE=args.output;pftl.require_chain_lock=require_lock
    deadline=time.monotonic()+3600;handle=LOCK.open('a+')
    acquired=False
    try:
        while time.monotonic()<deadline:
            fcntl.flock(handle,fcntl.LOCK_EX);handle.seek(0);holder=handle.read().strip()
            if not holder:
                handle.seek(0);handle.truncate();handle.write('TEE\n');handle.flush();acquired=True;break
            fcntl.flock(handle,fcntl.LOCK_UN)
            top('PHASE 2 STEP 5 IN PROGRESS: waiting for chain.lock; no TEE live writes yet.')
            time.sleep(5)
        if not acquired:raise TimeoutError('chain.lock remained occupied for 60 minutes')
        top('PHASE 2 STEP 5 IN PROGRESS: chain.lock=TEE; checking fleet and canary parent before authorized writes.')
        with ThreadPoolExecutor(max_workers=6) as pool:versions=dict(pool.map(runtime,pftl.PEERS))
        save(args.output/'runtime-hashes.json',versions);assert all(v['matches'] for v in versions.values())
        before=pftl.fleet_snapshot('tee-live-parent');save(args.output/'before-six-node.json',before)
        pftl.assert_identity(before)
        assert pftl.converged_point(before)==(gate['base_tip']['height'],gate['base_tip']['state_root']),'live parent changed; refresh canary before writing'
        assert all(v['mempool']['pending']==0 for v in before.values())
        for row in plan:
            require_lock()
            top('PHASE 2 STEP 5 IN PROGRESS: '+row['label']+'; full six-validator completion is pending.')
            receipt=pftl.asset_op('tee-'+row['label'],f.ISSUER,f.ISSUER_KEY,row['operation'])
            print(json.dumps({'operation':row['label'],'height':receipt['height'],'receipt_codes':receipt['receipt_codes']}),flush=True)
        with ThreadPoolExecutor(max_workers=6) as pool:after=dict(pool.map(lambda peer:reads(peer,asset),pftl.PEERS))
        save(args.output/'after-six-node.json',after)
        checks={}
        for name,row in after.items():
            status=row['nav_reserve_proof_status'];profile=status['active_profile'];route=row['navcoin_bridge_supply_status']
            active=next(x for x in row['status']['active_nav_profiles'] if x['asset_id']==asset)
            packets=[p for p in status['packets'] if p['epoch']==expected['epoch']]
            assert len(packets)==1
            packet=packets[0]
            checks[name]=dict(profile=profile['profile_id']==expected['profile'] and profile['verifier_kind']=='sp1-nav-reserve-v1',
                supply=row['asset_info']['asset']['outstanding_supply']==expected['supply_atoms'],
                attested_value=packet['attested_value']==expected['attested_value'],
                cryptographic_value=packet['cryptographically_verified_value']==0,source_count=packet['source_count']==1,
                finalized=packet['state']=='finalized' and active['finalized_epoch']==expected['epoch'],
                nav=packet['nav_per_unit']==expected['nav_per_unit']==active['nav_per_unit'],
                not_halted=active['halted'] is False,
                packet_hash=packet['reserve_packet_hash']==expected['reserve_packet_hash']==active['finalized_reserve_packet_hash'],
                quantity_trust=packet['quantity_trust_counts']==dict(attested=1,controlled=0,cryptographic=0),
                valuation_trust=packet['valuation_trust_counts']==dict(attested=1,controlled=0,cryptographic=0),
                verified_net_assets=packet['verified_net_assets']==expected['verified_net_assets'],
                route_epoch=route['pricing_nav_epoch']==expected['epoch'],
                route_packet=route['pricing_reserve_packet_hash']==expected['reserve_packet_hash'],
                route_supply=route['authorized_valid_supply_atoms']==expected['supply_atoms'],
                route_live=route['live_value_enabled'] is True)
        assert all(all(v.values()) for v in checks.values())
        fleet={k:v['server_info'] for k,v in after.items()};pftl.assert_identity(fleet)
        assert pftl.converged_point(fleet) is not None
        save(args.output/'verification.json',dict(passed=True,checks=checks,expected=expected,fleet_point=pftl.converged_point(fleet)))
        top(f"B23 PROFILE ON CHAIN: sp1-nav-reserve-v1 {expected['profile']} epoch {expected['epoch']} attested_value {expected['attested_value']} atoms; six validators verified.")
    except BaseException as error:
        save(args.output/'failure.json',{'error_type':type(error).__name__,'message':str(error)[:1200]})
        top('PHASE 2 STEP 5 BLOCKED: live sequence stopped; inspect literal receipts and failure.json before further writes.')
        raise
    finally:
        if acquired and LOCK.read_text().strip()=='TEE':handle.seek(0);handle.truncate();handle.flush()
        fcntl.flock(handle,fcntl.LOCK_UN);handle.close()

if __name__=='__main__':main()
