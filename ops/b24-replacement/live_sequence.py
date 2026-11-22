#!/usr/bin/env python3
"""Prepare the future live sequence and unsigned payloads. Explicit --dry-run only.

There is deliberately no --execute or broadcast path: Phase 4 has no founder go.
Checkpoint/roles, fresh quotes, holder signatures and cleared funding remain
mandatory inputs to any subsequent live runner. Copy receipts are not finality.
"""
import argparse
from eth_abi import encode
import shlex
from common import *
from retirement import prepare

ROOT=Path(__file__).resolve().parent
ELF='/mnt/HC_Volume_106212907/stakehub-hyperliquid/l1-security-evidence-20261031/receipt-v2-final-elf/pftl-uniswap-receipt-v2-program'
PROVER='/mnt/HC_Volume_106212907/stakehub-hyperliquid/l1-security-target-20261031/release/pftl-uniswap-prover'
SECURITY_NODE='/mnt/HC_Volume_106212907/stakehub-hyperliquid/l1-security-target-20261031/release/postfiat-node'
def artifact(name):
    paths=list((ROOT/'contracts/out').glob('*/'+name+'.json'));assert len(paths)==1
    return load(paths[0])

def deployment():
    pins=predicted_addresses();w=Web3();address=lambda key:Web3.to_checksum_address(pins[key])
    sp1=load(EVIDENCE/'sp1-runtime-provenance.json');runtime=load(EVIDENCE/'deployment-pins.json')
    genesis=bytes.fromhex(load(EVIDENCE/'before-capture-snapshot.json')['tip']['genesis_hash'])
    usdc=Web3.to_checksum_address('0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48')
    key=(min(address('token'),usdc,key=lambda x:int(x,16)),max(address('token'),usdc,key=lambda x:int(x,16)),500,10,'0x'+'00'*20)
    poolid=Web3.keccak(encode(['address','address','uint24','int24','address'],key))
    config=(Web3.to_checksum_address(sp1['address']),bytes.fromhex(sp1['runtime_keccak256'].removeprefix('0x')),bytes.fromhex('4388a21c'),
        bytes.fromhex('0060d83567657466eaac782e0be174b57277df6ac349f42288852a93d72f0fe1'),
        Web3.keccak(text='postfiat-wan-devnet-2'),Web3.keccak(genesis),1,Web3.keccak(text=ROUTE),Web3.keccak(bytes.fromhex(B24)),Web3.keccak(bytes.fromhex(PFUSDC)),
        1,address('controller'),address('token'),bytes.fromhex(runtime['token_runtime_code_hash'][2:]),356,1120,
        Web3.keccak(text='UNQUALIFIED REHEARSAL CHECKPOINT'),Web3.keccak(text='UNQUALIFIED REHEARSAL COMMITTEE'),1310)
    steps=[]
    constructors=[('WrappedVenueNAVCoin',['Wrapped Schwab NAVCoin B24','wB24',6,address('deployer')]),
        ('PFTLReceiptFinalityVerifierV2',[config]),
        ('PFTLUniswapPrimaryMarketV3',[address('token'),address('verifier'),(1,bytes.fromhex(PFUSDC),bytes.fromhex(B24),poolid,2_000_000_000_000,250_000_000_000,address('deployer')),('postfiat-wan-devnet-2',genesis,1,ROUTE)])]
    for nonce,(name,args) in enumerate(constructors):
        a=artifact(name);data=w.eth.contract(abi=a['abi'],bytecode=a['bytecode']['object']).constructor(*args).data_in_transaction
        steps.append(dict(label='deploy-'+name,chainId=1,nonce=nonce,source=address('deployer'),data=data,value=0))
    token=w.eth.contract(address=address('token'),abi=artifact('WrappedVenueNAVCoin')['abi'])
    for fn,args in [('setController',[address('controller')]),('lockController',[])]:
        steps.append(dict(label=fn,chainId=1,source=address('deployer'),to=token.address,data=token.encode_abi(fn,args=args),value=0))
    return dict(status='UNSIGNED_REHEARSAL_ONLY_NOT_APPROVED',fixture_checkpoint_and_roles=True,addresses=pins,
        pool_key=key,pool_id=poolid.hex(),initial_sqrt_price_x96=str(2**96),transactions=steps,
        return_paused_at_deployment=True,mainnet_transactions=0,
        required_replacements=['Appointed deployer/nonce, governance and LP recipient; regenerate every predicted address and native route pin.',
            'Independently authenticated live checkpoint/committee anchor and finality height; fixture strings may NEVER be deployed.',
            'Build/source/runtime review of the local return-pause patch, real SP1 runtime re-read, token/controller/verifier runtime readback.'])

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--dry-run',action='store_true',required=True)
    for name in ('snapshot','canary','output'):ap.add_argument('--'+name,type=Path,required=True)
    a=ap.parse_args();result=load(a.canary/'result.json');assert result['passed'] and result['live_transactions']==0
    retire=prepare(a.snapshot);a.output.mkdir(parents=True,exist_ok=False)
    native=retire['operations']+load(a.canary/'opening/operations.json')+load(a.canary/'conditional-funded-5000/operations.json')
    for n,row in enumerate(native,1):save(a.output/f'{n:02d}-{row["label"]}.json',row)
    save(a.output/'native-sequence.json',native);save(a.output/'retirement-terms.json',retire);save(a.output/'ethereum-unsigned.json',deployment())
    command_rows=[]
    for n,row in enumerate(native,1):
        label=f'{n:02d}-{row["label"]}';kind=row['kind'];file=a.output/(label+'.json')
        command_rows.append({'step':n,'label':label,'source':row['source'],'operation_file':str(file),
            'quote_rpc':{'method':kind+'_fee_quote','params':{'source':row['source'],'operation_json':json.dumps(row['operation'],separators=(',',':'))}},
            'owner_sign_command':shlex.join([str(NODE),'wallet-sign-'+kind+'-transaction','--key-file','OWNER_APPROVED_KEY_FILE','--quote-file',str(a.output/(label+'-live-quote.json'))]),
            'submit_rpc':{'method':'mempool_submit_signed_'+kind+'_transaction_finality','params':{'signed_'+kind+'_transaction_json':'OWNER_SIGNED_TRANSACTION_JSON_STRING',
                'proxy_required_current_height':'FRESH_SERVER_INFO_HEIGHT','proxy_required_parent_hash':'FRESH_SERVER_INFO_LEDGER_HASH','proxy_required_state_root':'FRESH_SERVER_INFO_STATE_ROOT'}},
            'after_each':'Require literal finality.confirmed=true, receipt.accepted=true and six-validator state convergence; never blindly repeat a submitted transaction.'})
    save(a.output/'native-command-sequence.json',command_rows)
    proof_commands=[
        shlex.join([SECURITY_NODE,'pftl-uniswap-receipt-witness-v2','--data-dir','VERIFIED_OFFLINE_COPY_OF_FINALIZED_NODE_ARCHIVE','--packet-hash','ACTUAL_ACCEPTED_EXPORT_PACKET_HASH','--prior-checkpoint','INDEPENDENTLY_PINNED_LIVE_CHECKPOINT_BLOCK_ID'])+' > actual-live-receipt-witness-v2.json',
        shlex.join([PROVER,'receipt','--witness','actual-live-receipt-witness-v2.json','--elf',ELF,'--proof-program-version','2','--output-dir','receipt-execute']),
        'On existing Vast 53579074 only, copy the public witness and exact ELF to a new job directory; hold flock -w 600 /workspace/b23-gpu/prover.lock for the complete job.',
        'SP1_PROVER=cuda RAYON_NUM_THREADS=4 /workspace/b23-pool-phase2/pftl-uniswap-prover receipt --witness actual-live-receipt-witness-v2.json --elf receipt-v2.elf --proof-program-version 2 --require-prover cuda --prove --output-dir receipt-proof',
        'Independently verify native witness, 1120-byte public values, 356-byte Groth16 proof, exact vkey/selector/runtime, and every route/asset/controller/recipient/amount/checkpoint binding.',
        'Appointed Ethereum relayer: verifier.verifyAndAccept(actualPublicValues, actualProofBytes); require accepted receipt state. Governance: controller.setMintPaused(false); keep returnPaused=true. Relayer: controller.consumeMintOnly(actualMintPacket); require exactly 5000000000 wrapper atoms credited once.',
        'Native issuer/relay acknowledges proven destination consumption through pftl_uniswap_destination_consume; verify global conservation before LP funding.',
        'LP supplies a separate 2000 real USDC plus the 5000 proof-backed wrapped units; initialize price at latest NAV and use the qualified PoolKey/bands, bounded Permit2 approvals and PositionManager transactions.']
    phases=[
        'FUTURE FOUNDER GO ONLY: approve holder economics, roles, funding and return-gate candidate; refresh the live snapshot/capture as required and rerun the entire copy.',
        'Disable B23 daily timer only under that future go, wait for service completion, acquire chain.lock=TEE, refresh all balances/offers/reserves and refuse changed obligations.',
        'Deploy fresh Ethereum contracts paused with genuine checkpoint pins; verify code and all bindings. No wrapper inventory exists yet.',
        'Execute native retirement operations first, owner-signed. Confirm zero B23 holder/open-offer/escrow/bridge claims, zero reserve, NAV halted and old route paused/disabled; retain honest historical counters.',
        'Only after retirement, create/register/finalize B24 and mint the full new account-value opening supply to Alice; initialize route disabled, then epoch-advance with 10050/9995 and absent spread_recipient/funding_sequence.',
        'Fund an additional 5025 PFUSDC through a genuinely proven deposit/backing path. Reserve/subscription mints 5000 new B24; use its entitlement to export exactly 5000. The copy funding grant is not a deposit proof and cannot be submitted.',
        'Wait for the export to be finalized by the actual live committee. Export the V2 witness from an OFFLINE verified archive copy; the released node only offers the legacy V1 exporter. Keep segment/rotation announcements within the guest bound of 64.',
        'Prove and relay only the actual accepted live export; then destination acknowledgment, LP funding and invariant checks. Disable all return UI/relay actions and keep on-chain returnPaused=true until a separately approved V3-capable L1 release.',
        'Clear chain.lock after final six-validator checks. New daily B24 operation/profile requires a separately reviewed enablement; do not run the old B23 timer against retired backing.']
    save(a.output/'sequence.json',dict(status='DRY_RUN_ONLY_NO_FOUNDER_GO',phases=phases,proof_commands=proof_commands,
        unresolved=['production signer roles and checkpoint anchors','cleared 5025 PFUSDC subscription funding and 2000 real USDC LP funding',
            'holder-signed retirement approval, including founder wallet','live export with genuine committee finality and exact new receipt proof',
            'review/deployment of return-pause candidate; V3 native return support remains absent'],rpc_writes=0,mainnet_transactions=0))
    print(json.dumps({'dry_run':True,'native_operations':len(native),'output':str(a.output),'broadcast_capability':False}))

if __name__=='__main__':main()
