#!/usr/bin/env python3
"""Negative verification of the real signed Nitro reading; no source API calls."""
import argparse
import base64
import copy
import json
from pathlib import Path
import subprocess
import sys
from cryptography import x509
from cryptography.hazmat.primitives import serialization

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    for name in ('capture','collector_repo','native_cli','root_der','output'):
        ap.add_argument('--'+name.replace('_','-'),type=Path,required=True)
    args=ap.parse_args();sys.path.insert(0,str(args.collector_repo))
    from pftl_schwab_tee.nav_reading import verify_reading
    from pftl_schwab_tee.canonical import canonical_json_bytes
    args.output.mkdir(parents=True,mode=0o700,exist_ok=False)
    load=lambda name:json.loads((args.capture/name).read_bytes())
    m,e,p=load('manifest.json'),load('result.json'),load('pcrs.json')
    root=x509.load_der_x509_certificate(args.root_der.read_bytes()).public_bytes(serialization.Encoding.PEM)
    # Verify at this reading's window end, retaining the guest's 65s bound.
    at=m['cutoff_unix_ms']+65000
    verify_reading(e,manifest=m,root_pem=root,expected_pcrs=p,now_ms=at,max_age_ms=65000)
    checks={}
    cases={
        'wrong_pcr':dict(envelope=e,manifest=m,expected_pcrs={**p,'0':'99'*48},now_ms=at),
        'wrong_account':dict(envelope=e,manifest={**m,'account_commitment_sha256':'aa'*32},expected_pcrs=p,now_ms=at),
        'stale_attestation':dict(envelope=e,manifest=m,expected_pcrs=p,now_ms=at+65001),
    }
    altered=copy.deepcopy(e);altered['body']['reading']['liquidation_value_atoms']+=1
    cases['altered_value']=dict(envelope=altered,manifest=m,expected_pcrs=p,now_ms=at)
    for label,case in cases.items():
        try:verify_reading(**case,root_pem=root,max_age_ms=65000)
        except ValueError as error:checks['native_'+label]={'rejected':True,'reason':str(error)}
        else:raise AssertionError('accepted invalid native reading: '+label)
    witness=load('witness.json')
    attacks={
        'wrong_pcr_policy':lambda w:w['policy']['nitro_policy']['approvedPcrSets'][0][0].update(sha384='99'*48),
        'wrong_account':lambda w:w['manifest'].update(account_commitment_sha256='aa'*32),
        'stale_reading':lambda w:w['envelope']['body']['reading'].update(observed_unix_ms=m['cutoff_unix_ms']-1),
        'wrong_epoch':lambda w:w['manifest'].update(observation_epoch=m['observation_epoch']+1),
        'bad_signature':lambda w:w['envelope'].update(signature=base64.b64encode(bytes(64)).decode()),
    }
    for label,mutate in attacks.items():
        bad=copy.deepcopy(witness);mutate(bad);path=args.output/(label+'.json');path.write_bytes(canonical_json_bytes(bad))
        result=subprocess.run([str(args.native_cli),'native','--witness',str(path),
            '--output',str(args.output/(label+'.bin'))],capture_output=True)
        if result.returncode==0:raise AssertionError('accepted invalid guest witness: '+label)
        checks['guest_logic_'+label]={'rejected':True,'reason':result.stderr.decode()[:400]}
    (args.output/'report.json').write_text(json.dumps({'real_attestation':True,'historical_verification_time_ms':at,
        'freshness_bound_ms':65000,'checks':checks,'all_rejected':True},indent=2)+'\n')
    print(json.dumps({'real_negative_checks_passed':len(checks)}))
if __name__=='__main__':main()
