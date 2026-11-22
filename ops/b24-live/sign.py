"""Path-only native signing for the three specifically authorized controlled holders."""
import json
from pathlib import Path
import subprocess
import sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'b24-replacement'))
from common import *
BOB='pf5c266a1bdb18320e140961411a8c4d8383eaafb2'
TEST='pfa0c02873020b87a5dd32a5db8747c5e5d9487051'
HERE=Path(__file__).resolve().parent

def sign_fields(fields,kind,prefix):
    source=fields['source'];input_path=Path(str(prefix)+'-fields.json');output=Path(str(prefix)+'-signed.json');save(input_path,dict(kind=kind,fields=fields))
    if source in KEYS:
        args=[str(NODE),'wallet-sign-'+kind+'-transaction','--key-file',str(KEYS[source]),'--chain-id',fields['chain_id'],'--genesis-hash',fields['genesis_hash'],'--protocol-version',str(fields['protocol_version']),'--fee',str(fields['fee']),'--sequence',str(fields['sequence']),'--operation-json',json.dumps(fields['operation'],separators=(',',':'))]
        result=json.loads(subprocess.check_output(args,stderr=subprocess.PIPE));save(output,result)
    elif source==BOB:
        subprocess.run(['/home/postfiat/repos/StakeHub/.venv/bin/python',str(HERE/'sign_bob.py'),str(input_path),str(output)],check=True)
    elif source==TEST:
        subprocess.run(['node',str(HERE/'sign_test.mjs'),str(input_path),str(output)],check=True)
    else:raise ValueError('Unapproved signer; founder and custody wallets prohibited')
    return load(output)

def copy_signer(state,op,source,kind,prefix):
    t=state['tip'];account=item(state['ledger']['accounts'],'address',source)
    fields=dict(chain_id=t['chain_id'],genesis_hash=t['genesis_hash'],protocol_version=t['protocol_version'],source=source,fee=1000,sequence=account['sequence']+1,operation=op)
    return sign_fields(fields,kind,prefix)
