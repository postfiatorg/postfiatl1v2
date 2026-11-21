#!/usr/bin/env python3
"""Unlock only named test wallet b23-bob for one native signature; no broadcast."""
import json,os,sys,subprocess
from pathlib import Path
sys.path.insert(0,'/home/postfiat/repos/StakeHub')
from pft_wallet.wallets import signing_files
os.umask(0o077)
request=json.loads(Path(sys.argv[1]).read_text());f=request['fields'];kind=request['kind']
assert f['source']=='pf5c266a1bdb18320e140961411a8c4d8383eaafb2' and kind in ('asset','offer')
p=Path('/home/postfiat/.pft/b23-bob.passphrase');assert p.stat().st_mode & 0o077==0
with signing_files('b23-bob',p.read_text().rstrip('\r\n')) as (public,key,_):
    assert public.ce22_address==f['source']
    args=['/home/postfiat/repos/b23-coord/artifacts/release4/postfiat-node','wallet-sign-'+kind+'-transaction','--key-file',str(key),'--chain-id',f['chain_id'],'--genesis-hash',f['genesis_hash'],'--protocol-version',str(f['protocol_version']),'--fee',str(f['fee']),'--sequence',str(f['sequence']),'--operation-json',json.dumps(f['operation'],separators=(',',':'))]
    result=subprocess.check_output(args,stderr=subprocess.PIPE)
    Path(sys.argv[2]).write_bytes(result)
