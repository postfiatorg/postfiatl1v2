#!/usr/bin/env python3
"""Execute two synthetic signed Nitro readings in the new guest. No AWS or RPC.

Synthetic certificates/PCRs test the changed guest's rules; they cannot qualify
the production profile and must never be registered on the live ledger.
"""
import argparse
import base64
import copy
from datetime import datetime,timezone
import hashlib
import json
from pathlib import Path
import subprocess
import sys

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    for name in ('collector_repo','native_cli','elf','output'):
        ap.add_argument('--'+name.replace('_','-'),type=Path,required=True)
    args=ap.parse_args();args.output.mkdir(parents=True,exist_ok=False)
    sys.path[:0]=[str(args.collector_repo),str(args.collector_repo/'tests')]
    from cryptography.hazmat.primitives import serialization
    from navstrategies.yolo_indices import nav_reading as nav
    from navstrategies.yolo_indices.canonical import canonical_json_bytes,domain_sha256
    from navstrategies.yolo_indices.nitro_runtime import NitroStatementKeys
    from test_nav_reading import manifest as fixture_manifest,collect,PCRS
    import test_yolo_nitro_attestation as fixtures

    cutoff=int(datetime(2026,9,4,17,7,tzinfo=timezone.utc).timestamp()*1000)
    fixtures.NOW=datetime.fromtimestamp(cutoff/1000,timezone.utc)
    certificates=fixtures._certificates()
    seed=fixture_manifest.__wrapped__()
    seed['schedule']=dict(opening_epoch=1,opening_unix_ms=cutoff-3600000,
                          first_daily_epoch=2,first_daily_unix_ms=cutoff-1800000)
    reports=[]
    def execute(label,witness,accept,guest=False):
        path=args.output/(label+'.json');path.write_bytes(canonical_json_bytes(witness))
        command=[str(args.native_cli),'execute' if guest else 'native','--witness',str(path),'--output',str(args.output/(label+'.bin'))]
        if guest:command+=['--elf',str(args.elf)]
        result=subprocess.run(command,capture_output=True,text=True)
        (args.output/(label+'.log')).write_text(result.stdout+result.stderr)
        if (result.returncode==0)!=accept:raise AssertionError(label+': '+result.stderr[-2000:])
        reports.append(dict(case=label,accepted=accept,guest_execution=guest))
    for index in range(2):
        at=cutoff+index*120000;m=dict(seed,cutoff_unix_ms=at,observation_epoch=4+index,challenge_sha256=hashlib.sha256(str(index).encode()).hexdigest())
        p=nav.nav_source_policy(m,PCRS,certificates[1].public_bytes(serialization.Encoding.DER))
        m.update(source_manifest_hash=nav.commitment48(p['schema'],p),valuation_policy_hash=domain_sha256(p['valuation_policy']['schema'],p['valuation_policy']))
        keys=NitroStatementKeys.generate()
        class Nsm:
            def attest(self,*,user_data,nonce,public_key):
                user=json.loads(user_data)
                return fixtures._document(timestamp=datetime.fromtimestamp((at+3000)/1000,timezone.utc),nonce=nonce,
                    statement_digest=user['statementBindingDigest'],statement_key=user['statementKey'],
                    recipient_key=keys.recipient_private_key,certificates=certificates)[0]
        reading,_,_=collect(m,times=[at+1000,at+2000])
        e=nav.attest_reading(m,reading,keys,Nsm());w=dict(manifest=m,policy=p,envelope=e)
        execute(f'reading-{index+1}',w,True,guest=True)
        for label,field,value in (
            ('backdated','started_unix_ms',at-1),
            ('late','observed_unix_ms',at+60001)):
            bad=copy.deepcopy(w);bad['envelope']['body']['reading'][field]=value
            execute(f'{index+1}-{label}',bad,False)
        bad=copy.deepcopy(w);bad['manifest']['observation_epoch']+=1
        execute(f'{index+1}-replay-as-next-epoch',bad,False)
        bad=copy.deepcopy(w);bad['manifest']['challenge_sha256']='bb'*32
        execute(f'{index+1}-replay-with-new-challenge',bad,False)
    report=dict(passed=True,synthetic_only=True,production_qualification=False,aws_calls=0,chain_writes=0,checks=reports)
    (args.output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report))

if __name__=='__main__':main()
