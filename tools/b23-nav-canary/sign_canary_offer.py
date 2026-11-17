"""Sign a local canary offer with Bob's existing wallet; contains no submit path."""
import json
from pathlib import Path
import subprocess
import sys
sys.path.insert(0,'/home/postfiat/repos/StakeHub')
from pft_wallet.wallets import signing_files

node,quote=map(Path,sys.argv[1:])
request=json.loads(quote.read_bytes())
path=Path('/home/postfiat/.pft/b23-bob.passphrase')
if path.stat().st_mode&0o077:raise ValueError('passphrase file permissions')
with signing_files('b23-bob',path.read_text().rstrip('\r\n')) as (public,key,_backup):
    if public.ce22_address!='pf5c266a1bdb18320e140961411a8c4d8383eaafb2':raise ValueError('wallet identity')
    # Signed transaction only on stdout; the temporary private key never leaves its file.
    subprocess.run([str(node),'wallet-sign-offer-transaction','--key-file',str(key),'--quote-file',str(quote)],check=True)
