#!/usr/bin/env python3
"""Owner-side verification of retained ciphertext, without exporting plaintext.

Optionally preserve the enclave's OAuth rotation in the original credential
store using compare-and-swap on the pre-capture refresh-token commitment.
"""
import argparse
import base64
import hashlib
import json
import shlex
import subprocess
import sys
import tarfile
from pathlib import Path
from cryptography.hazmat.primitives import hashes,serialization
from cryptography.hazmat.primitives.asymmetric import padding
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from pftl_schwab_tee.canonical import canonical_json_bytes
from pftl_schwab_tee.nav_reading import account_commitment,strict_json,validate_manifest

def recover(capture,key_path):
    manifest=json.loads((capture/'manifest.json').read_bytes())
    mh=validate_manifest(manifest)
    if key_path.stat().st_mode&0o077:raise ValueError('recovery key permissions')
    key=serialization.load_pem_private_key(key_path.read_bytes(),password=None)
    public=key.public_key().public_bytes(serialization.Encoding.DER,serialization.PublicFormat.SubjectPublicKeyInfo)
    if public!=base64.b64decode(manifest['operator_public_key_der_base64']):raise ValueError('wrong recovery key')
    found={}
    with tarfile.open(capture/'encrypted-archive.tar') as archive:
        for member in archive:
            if not member.isfile():continue
            if not member.name.endswith('.ciphertext') or member.size>8*1024*1024:raise ValueError('archive file bound')
            envelope=json.loads(archive.extractfile(member).read())
            if envelope['schema']!='postfiat.yolo.operator_recoverable_capture.v1':raise ValueError('ciphertext schema')
            for name in ('account-list','account-detail','oauth-rotation'):
                aad=canonical_json_bytes(dict(schema='postfiat.b23.nav_archive.v1',manifest_sha256=mh,artifact_id=name))
                try:
                    secret=key.decrypt(base64.b64decode(envelope['operatorEncryptedDataKeyBase64']),
                        padding.OAEP(mgf=padding.MGF1(hashes.SHA256()),algorithm=hashes.SHA256(),label=aad))
                    plaintext=AESGCM(secret).decrypt(base64.b64decode(envelope['nonceBase64']),base64.b64decode(envelope['ciphertextBase64']),aad)
                except ValueError:continue
                if name in found:raise ValueError('duplicate archive artifact')
                found[name]=plaintext;break
            else:raise ValueError('archive artifact could not be authenticated')
    if set(found)!={'account-list','account-detail','oauth-rotation'}:raise ValueError('missing archive artifacts')
    reading=json.loads((capture/'result.json').read_bytes())['body']['reading']
    for name,field in [('account-list','account_list_response_sha256'),('account-detail','account_response_sha256')]:
        if hashlib.sha256(found[name]).hexdigest()!=reading[field]:raise ValueError('attested response hash mismatch')
    accounts=strict_json(found['account-list']);detail=strict_json(found['account-detail'])
    if len(accounts)!=1 or account_commitment(accounts[0]['hashValue'])!=manifest['account_commitment_sha256']:raise ValueError('archive account mismatch')
    account=detail['securitiesAccount']
    if account['accountNumber']!=accounts[0]['accountNumber']:raise ValueError('archive mapping mismatch')
    if account['currentBalances']['liquidationValue']*1_000_000!=reading['liquidation_value_atoms']:raise ValueError('archive NAV mismatch')
    credentials=json.loads(found['oauth-rotation'])
    if credentials['accountApplicationIdentitySha256']!=manifest['account_commitment_sha256']:raise ValueError('OAuth identity mismatch')
    return credentials,{'encrypted_artifacts_authenticated':3,'exact_response_hashes_match':True,
        'private_account_mapping_matches':True,'reported_nlv_matches_attested_atoms':True}

def preserve_rotation(credentials,baseline):
    expected=json.loads(baseline.read_bytes())['refresh_token_sha256']
    # The script and arguments contain only paths; secret material uses captured stdin.
    script='''import hashlib,json,os,sys,tempfile
from pathlib import Path
p=Path('/home/pfrpc/.config/navstrategies/schwab_tokens.json')
request=json.load(sys.stdin);current=json.loads(p.read_bytes())
digest=lambda x:hashlib.sha256(x.encode()).hexdigest()
if digest(current['refresh_token'])==digest(request['refresh_token']):
 print(json.dumps({'rotation_already_current':True,'credential_store_updated':False}))
else:
 if digest(current['refresh_token'])!=request['expected']:raise RuntimeError('credential store changed concurrently')
 current['refresh_token']=request['refresh_token']
 fd,name=tempfile.mkstemp(prefix='.b23-tee-',dir=p.parent)
 with os.fdopen(fd,'w') as f:json.dump(current,f);f.flush();os.fsync(f.fileno())
 os.replace(name,p)
 print(json.dumps({'rotation_already_current':False,'credential_store_updated':True}))
'''
    result=subprocess.run(['ssh','-o','BatchMode=yes','pfrpc@productionrpc','python3 -c '+shlex.quote(script)],
        input=canonical_json_bytes({'expected':expected,'refresh_token':credentials['refreshToken']}),
        stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30,check=True)
    return json.loads(result.stdout)

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--capture',type=Path,required=True);ap.add_argument('--recovery-key',type=Path,required=True)
    ap.add_argument('--oauth-baseline',type=Path);ap.add_argument('--output',type=Path,required=True)
    args=ap.parse_args();credentials,report=recover(args.capture,args.recovery_key)
    if args.oauth_baseline:report.update(preserve_rotation(credentials,args.oauth_baseline))
    args.output.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))

if __name__=='__main__':
    try:main()
    except Exception as error:
        print(json.dumps({'verification_failed':True,'error_type':type(error).__name__}),file=sys.stderr)
        raise SystemExit(1)
