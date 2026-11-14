"""Offline primitives. This module has no RPC submission implementation."""
import hashlib
import json
from pathlib import Path
import subprocess
from web3 import Web3
import rlp

COORD=Path('/home/postfiat/repos/b23-coord')
EVIDENCE=COORD/'evidence/tee/phase4'
NODE=COORD/'artifacts/release4/postfiat-node'
CANARY=Path('/mnt/HC_Volume_106212907/sec-fundamentals-proxy/b23-tee-nav-canary-target/release/postfiat-b23-nav-canary')
NATIVE=Path('/mnt/HC_Volume_106212907/sec-fundamentals-proxy/b23-tee-nav-target/release/postfiat-b23-nav')
ISSUER='pffcb93d9f87a843a8aa34e1adf241f5d58143e81b'
ALICE='pfab9b9228942e5c529633a13aa271d5297bec6353'
FOUNDER='pf095716b973905221990b26050015874c68e30696'
B23='ca7845134c8e95819082fcd2f1612808e20a37dfabab98dab120f99fd2c54c18ffb50d98c0664480420c881773244627'
B24='52285e4ded1d58113cfaa12bbc034b8b10213f699b3224c74160450229d6bf7ccfe505c78f2599d975ae2ecf98f7d8af'
PFUSDC='02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b'
ROUTE='pftl-b24-ethereum-wB24-usdc-v1'
OLD_ROUTE='pftl-pfdep-schwab-primary-pfusdc-v1'
KEY_DIR=Path('/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private')
KEYS={ISSUER:KEY_DIR/'faucet-key.json',ALICE:KEY_DIR/'holder-key.json'}

def load(path):return json.loads(Path(path).read_bytes())
def save(path,value):Path(path).write_text(json.dumps(value,indent=2)+'\n')
def canonical(value):return json.dumps(value,sort_keys=True,separators=(',',':')).encode()
def sha48(label,value):return hashlib.sha3_384(label.encode()+b'\0'+canonical(value)).hexdigest()
def item(rows,key,value):return next(x for x in rows if x[key]==value)
def balance(ledger,owner,asset):return next((x['balance'] for x in ledger['trustlines'] if x['account']==owner and x['asset_id']==asset),0)
def invariant(route):return sum(route[k] for k in ('pftl_spendable_supply_atoms','ethereum_spendable_supply_atoms','other_registered_venue_supply_atoms','outstanding_bridge_claims_atoms','pending_return_import_claims_atoms'))==route['authorized_valid_supply_atoms']
def policy_hash(p):
    fields='policy_epoch issue_multiplier_bps redeem_multiplier_bps issue_capacity_atoms redeem_capacity_atoms max_order_atoms min_order_atoms valid_from_height expires_at_height max_nav_age_blocks pricing_nav_epoch pricing_reserve_packet_hash'.split()
    value=''.join(f'{k}={p[k]}\n' for k in fields)
    if p.get('spread_recipient') is not None:value+=f"spread_recipient={p['spread_recipient']}\n"
    return hashlib.sha3_384(b'postfiat.pftl_uniswap.primary_market_policy.v2\0'+value.encode()).hexdigest()

def predicted_addresses():
    # Public, unappointed rehearsal deployer, not an account this lane owns or signs for.
    deployer='0x'+Web3.keccak(text='PUBLIC REHEARSAL ONLY B24 deployer 20261114')[-20:].hex()
    addresses={name:'0x'+Web3.keccak(rlp.encode([bytes.fromhex(deployer[2:]),n]))[-20:].hex()
        for n,name in enumerate(('token','verifier','controller'))}
    return {'deployer':deployer,'nonce':0,**addresses,'recipient':'0x'+Web3.keccak(text='PUBLIC REHEARSAL ONLY B24 LP')[-20:].hex(),
        'production_roles_appointed':False,'derivation':'Ethereum CREATE keccak256(RLP(sender, nonce)), nonces 0,1,2','mainnet_deployed':False}

def export_operation(route,amount,label,reservation=None):
    p=route['v2']['primary_market_policy'];nonce=hashlib.sha256(label.encode()).hexdigest();packet=sha48('b24.export',label)
    due=(amount*10050+9999)//10000  # This rehearsal's exact NAV is 1.000000.
    destination=predicted_addresses()['recipient'];deadline=1794787200
    identifiers=[route['route_config_digest'],packet,reservation or packet,route['settlement_asset_id'],route['native_nav_asset_id'],p['pricing_reserve_packet_hash']]
    packed=Web3.keccak(text='postfiat.pftl_uniswap.mint_packet.v2')+b''.join(Web3.keccak(bytes.fromhex(x)) for x in identifiers)
    packed+=Web3.keccak(bytes.fromhex(p['policy_hash']))
    packed+=b''.join(x.to_bytes(8,'big') for x in (route['v2']['route_epoch'],p['pricing_nav_epoch'],deadline))
    packed+=bytes.fromhex(nonce)+(1).to_bytes(32,'big')
    packed+=b''.join(bytes.fromhex(x[2:]) for x in (route['handoff_controller'],route['wrapped_navcoin_token'],destination))
    packed+=amount.to_bytes(32,'big')+due.to_bytes(32,'big')
    op=dict(operation='pftl_uniswap_export_debit',owner=ALICE,route_id=ROUTE,packet_hash=packet,export_nonce=nonce,
        ethereum_recipient=destination,amount_atoms=amount,destination_deadline_seconds=deadline,refund_delay_blocks=100,
        settlement_value_atoms=due,ethereum_packet_digest=Web3.keccak(packed).hex(),ethereum_packet_schema_version=2)
    if reservation:op['reservation_id']=reservation
    return op

class Copy:
    def __init__(self,snapshot,output):
        self.start=Path(snapshot);self.current=self.start;self.output=Path(output)
        self.output.mkdir(parents=True,exist_ok=False);self.base=load(snapshot);self.receipts=[];self.operations=[]
        self.height=self.base['tip']['height']
    def ledger(self):return load(self.current)['ledger']
    def apply(self,label,op,source=ISSUER,accepted=True,code=None,kind='asset',advance=True,height=None):
        state=load(self.current);account=item(state['ledger']['accounts'],'address',source)
        args=[str(NODE),'wallet-sign-'+kind+'-transaction','--key-file',str(KEYS[source]),
            '--chain-id',self.base['tip']['chain_id'],'--genesis-hash',self.base['tip']['genesis_hash'],
            '--protocol-version',str(self.base['tip']['protocol_version']),'--fee','1000','--sequence',str(account['sequence']+1),
            '--operation-json',json.dumps(op,separators=(',',':'))]
        signed=json.loads(subprocess.check_output(args,stderr=subprocess.PIPE))
        h=height or self.height+1;steps=self.output/(label+'-steps.json');target=self.output/(label+'-state.json')
        save(steps,[dict(label=label,height=h,transaction=signed,kind=kind,expect_accepted=accepted,expect_code=code)])
        result=subprocess.run([str(CANARY),'apply','--snapshot',str(self.current),'--steps',str(steps),'--output',str(target)],capture_output=True)
        if target.exists():self.receipts.extend(load(target)['canary_receipts']);save(self.output/'receipts.json',self.receipts)
        if result.returncode:raise RuntimeError(label+': '+result.stderr.decode())
        if not accepted:assert load(target)['ledger']==state['ledger'],'rejection mutated state'
        if accepted and advance:
            self.current=target;self.height=h;self.operations.append(dict(label=label,source=source,kind=kind,operation=op))
            save(self.output/'operations.json',self.operations)
        return load(target)['ledger']
