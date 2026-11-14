#!/usr/bin/env python3
"""Prepare the unfunded 7025-USDC launch; never broadcast or create a bridge job."""
import argparse,copy
from phase6_deploy import *
from phase6_export import operations
from qualify import LiveCopy,S
from rpc import call

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--snapshot',type=Path,required=True);ap.add_argument('--output',type=Path,required=True);a=ap.parse_args();d=a.output;d.mkdir(mode=0o700,exist_ok=False);s=load(a.snapshot);e=load(OUT/'deployment-expected.json');o=e['operator'];w=Web3(Web3.HTTPProvider(PUBLIC_RPC,request_kwargs={'timeout':60}));block=w.eth.block_number
    route=call('vault_bridge_route',{'asset_id':PFUSDC});save(d/'bridge-route.json',route);subprocess.run(['python3','/home/postfiat/repos/postfiatl1v2-b23-ops/ops/b23/bridge_job.py','readiness',str(d/'bridge-readiness.json')],check=True,capture_output=True);ready=load(d/'bridge-readiness.json');assert ready['ready'] and ready['credited_asset_id']==S and ready['route_epoch']==route['governance_route_epoch'] and ready['route_profile_hash']==route['profile_hash'] and ready['beacon_anchor_authenticated']
    vault=Web3.to_checksum_address(route['profile']['vault_address']);assert hx(Web3.keccak(w.eth.get_code(vault)))==route['profile']['vault_runtime_code_hash'];assert int.from_bytes(w.eth.call({'to':vault,'data':Web3.keccak(text='paused()')[:4]}),'big')==0
    nonce=Web3.keccak(text='b24-phase6-future-5000-'+str(s['tip']['height']));binding=bytes.fromhex(route['route_binding']);amount=5025000000
    approve=dict(to=USDC,data=hx(Web3.keccak(text='approve(address,uint256)')[:4]+encode(['address','uint256'],[vault,amount])),value=0)
    deposit=dict(to=vault,data=hx(Web3.keccak(text='depositV2(uint256,string,bytes32,bytes32)')[:4]+encode(['uint256','string','bytes32','bytes32'],[amount,ALICE,nonce,binding])),value=0)
    save(d/'bridge-unsigned.json',dict(status='PREPARED_NOT_AUTHORIZED_TO_EXECUTE_NO_USDC',sender=o['address'],chainId=1,USDC_atoms=amount,native_recipient=ALICE,credited_asset_id=S,route_epoch=route['governance_route_epoch'],route_binding=route['route_binding'],deposit_nonce=hx(nonce),transactions=[dict(label='approve-vault-exact-5025',transaction=approve),dict(label='depositV2-to-Alice',transaction=deposit)],refresh_before_signing=['readiness','route/profile/binding','USDC balance','fees/nonces','NAV/subscription due']))
    plan=operations(s,5000000000,'future-funded-5000',S);due=plan[0]['operation']['max_settlement_value_atoms'];save(d/'native-operations-template.json',plan)
    # Isolated conditional funding fixture. No deposit, claim or finality is fabricated as live evidence.
    fixture=copy.deepcopy(s);line=item([x for x in fixture['ledger']['trustlines'] if x['account']==ALICE],'asset_id',S);line['balance']+=amount;line['limit']=max(line['limit'],line['balance']);definition=item(fixture['ledger']['asset_definitions'],'asset_id',S);bucket=item(fixture['ledger']['vault_bridge_bucket_states'],'bucket_id',definition['source_bucket_id'])
    for field in ('gross_receipt_atoms','counted_value_atoms','outstanding_vault_bridge_atoms'):bucket[field]+=amount
    item(fixture['ledger']['nav_assets'],'asset_id',PFUSDC)['circulating_supply']+=amount;save(d/'conditional-source-fixture.json',fixture)
    qualified=False
    if due<=amount:
        c=LiveCopy(d/'conditional-source-fixture.json',d/'conditional-copy');before=balance(c.ledger(),ALICE,S)
        for row in plan:c.apply(row['label'],row['operation'],ALICE)
        assert before-balance(c.ledger(),ALICE,S)==due and invariant(item(c.ledger()['pftl_uniswap_routes'],'route_id',ROUTE));qualified=True
    job=dict(route_id='ethereum-mainnet-usdc-v1',source_chain_id=1,deposit_tx_hash='ACTUAL_CONFIRMED_DEPOSIT_TX_HASH',deposit_id='ACTUAL_ERC20BridgeDepositedV2_EVENT_ID',pftl_recipient=ALICE,depositor=o['address'].lower(),amount_atoms=str(amount),idempotency_key='b24-launch-ACTUAL_DEPOSIT_TX_HASH')
    save(d/'bridge-job-template.json',dict(method='POST',path='/api/bridge/jobs',body=job,status='DO_NOT_SUBMIT_UNTIL_REAL_DEPOSIT_AND_EXPLICIT_LAUNCH_AUTHORIZATION'))
    steps=[
        'Receive exactly 7025 USDC at the dedicated operator; retain ETH for fees. Recheck fresh bridge readiness, epoch/profile/binding, code hashes, Alice S trustline and B24 finalized NAV.',
        'Operator approve existing USDC vault for exactly 5025000000 atoms, then depositV2(5025000000, Alice, fresh unique nonce, current binding). Require successful receipts and decode the actual deposit event; a direct token transfer is insufficient.',
        'Submit the actual event fields to the bridge job endpoint. Existing relay verifies Ethereum beacon finality and real deposit proof, grows backed cap if required, and proposes/finalizes/claims S to Alice. Hold the appropriate native chain lock for relay writes. Verify three literal accepted receipts and exact 5025000000 S credit on all six validators.',
        'Under chain.lock TEE, refresh exact native parent/NAV/policy/fee quotes. Reserve 5000000000 B24 with settlement_source_asset_id S; subscribe with family settlement_asset_id F and deterministic current due; no S-to-F conversion. Sign each by Alice, copy-check exact signed transaction, then submit and verify six receipts/full ledger.',
        'Export the paid 5000000000 entitlement to the dedicated operator with current route digest, policy, epochs, unique packet/reservation/nonce, current due and deadline. Immediately capture the authentic public finalized archive at that receipt boundary while still holding chain.lock.',
        'Import archive into a fresh offline store and use the frozen V2 exporter with an accepted verifier checkpoint within 64 ancestry steps. Execute native and frozen guest and compare public values; never synthesize committee signatures or finality. Release chain.lock after verified archive capture.',
        'On existing Vast only, hold /workspace/b23-gpu/prover.lock for identity/setup/proving/verification. Produce real receipt Groth16 proof; compare exact public values, ELF/vkey/selector and all route/amount/recipient bindings.',
        'Fresh mainnet fork the exact actual proof and MintPacket. Verify code/pins, balance/fee budget and replay negatives. On mainnet verifyAndAccept, governance setMintPaused(false), then consumeMintOnly exact 5000000000 once. Keep returns paused and ownership unchanged.',
        'After Ethereum depth >=12, construct the real receipt MPT proof and collect governed validator checkpoint votes, each independently reproducing Ethereum. Submit issuer destination_consume with the actual certificate/log to close pending claims; verify native Ethereum supply equals wrapper total supply and invariant holds.',
        'Recheck latest NAV against current hookless pool price and zero liquidity. Regenerate the qualified direct PositionManager LP template with fresh deadlines/nonces: exact ERC20->Permit2 allowances 5000000000 wB24 and 2000000000 USDC; bounded Permit2->PositionManager approvals; centered then remaining-wrapper ask-band positions owned by operator. At NAV 1: ticks [-210,200] and [0,200]. Do not spend the separate one-B24 test inventory.',
        'Verify actual NFTs/ownership/liquidity, exact token debits and dust, pool price, all supply and reserve invariants. Report live receipts and pause state; no return support is implied.'
    ]
    result=dict(prepared=True,executed=False,operator=o['address'],operator_USDC_atoms=int.from_bytes(w.eth.call({'to':USDC,'data':Web3.keccak(text='balanceOf(address)')[:4]+encode(['address'],[o['address']])}),'big'),native_snapshot_height=s['tip']['height'],NAV_atoms=item(s['ledger']['nav_assets'],'asset_id',B24)['nav_per_unit'],deposit_USDC_atoms=amount,LP_USDC_atoms=2000000000,total_USDC_atoms=7025000000,subscribe_B24_atoms=5000000000,current_subscription_due_atoms=due,extra_USDC_required_if_NAV_changed=max(0,due-amount),conditional_S_copy_passed=qualified,source_conversion_needed=False,bridge_ready_now=True,real_deposit_claim_unexecuted=True,LP_fork_proof=str(OUT/'lp-qualification/result.json'),steps=steps)
    save(d/'sequence.json',result);print(json.dumps({k:v for k,v in result.items() if k!='steps'}),flush=True)
if __name__=='__main__':main()
