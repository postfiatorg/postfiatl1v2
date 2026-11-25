#!/usr/bin/env python3
"""Send one proof-carrying call to Ethereum mainnet from the dedicated operator (key by path, never printed).
Usage: eth_send.py checkpoint|withdraw <proof_dir> <out.json>"""
import fcntl,json,sys
from eth_journal import durable_json, intent_path, read_eth_result
from pathlib import Path
from eth_account import Account
from web3 import Web3
from web3.exceptions import TransactionNotFound
from hexbytes import HexBytes
KEY=Path('/home/postfiat/.config/b24/eth-operator.key');RPC='https://ethereum-rpc.publicnode.com'
VERIFIER='0xc398a26BD997168C3C966763B0A97829d14dB655';VAULT='0xE7A432a28b20621A70E845C7F202004Cf98e7A2C';USDC='0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48'
ABI=[{"type":"function","name":n,"stateMutability":"nonpayable","inputs":[{"name":"publicValues","type":"bytes"},{"name":"proofBytes","type":"bytes"}],"outputs":o} for n,o in (('advanceCheckpoint',[]),('withdrawWithProof',[{"name":"","type":"bytes32"}]))]+[{"type":"function","name":"latestFinalizedHeight","stateMutability":"view","inputs":[],"outputs":[{"name":"","type":"uint64"}]},{"type":"function","name":"balanceOf","stateMutability":"view","inputs":[{"name":"a","type":"address"}],"outputs":[{"name":"","type":"uint256"}]}]
OPERATOR='0xF77B1876803c01967C5a62EF28C5f6de4574F7E1'


def balances(eth, block):
    verifier = eth.contract(address=VERIFIER, abi=ABI)
    usdc = eth.contract(address=USDC, abi=ABI)
    return dict(height=verifier.functions.latestFinalizedHeight().call(block_identifier=block),
                vault_usdc=usdc.functions.balanceOf(VAULT).call(block_identifier=block),
                operator_usdc=usdc.functions.balanceOf(OPERATOR).call(block_identifier=block),
                operator_wei=eth.get_balance(OPERATOR, block_identifier=block))


def record_receipt(eth, mode, journal, out, recover=False):
    intent = json.loads(journal.read_text())
    destination = VERIFIER if mode == 'checkpoint' else VAULT
    if (intent['mode'], intent['chain_id'], intent['from'], intent['to']) != (mode, 1, OPERATOR, destination):
        raise ValueError('transaction journal does not match requested action')
    # A crash can leave a signed transaction on disk before the first RPC send.
    # Only replay those exact bytes, and only while the original nonce is unused.
    if recover:
        try:
            eth.get_transaction(intent['tx'])
        except TransactionNotFound:
            raw = HexBytes(intent['signed'])
            if Web3.to_hex(Web3.keccak(raw)) != intent['tx'] or Account.recover_transaction(raw) != OPERATOR:
                raise ValueError('signed transaction does not match journal')
            if eth.get_transaction_count(OPERATOR, 'pending') > intent['nonce']:
                raise RuntimeError('unknown transaction at an occupied nonce; reconcile journal')
            if Web3.to_hex(eth.send_raw_transaction(raw)) != intent['tx']:
                raise RuntimeError('unexpected hash for journaled transaction')
    # Recovery does not need the signing key, proof, or gas estimate.
    receipt = eth.wait_for_transaction_receipt(intent['tx'], timeout=60)
    if Web3.to_hex(receipt.transactionHash).lower() != intent['tx'].lower():
        raise ValueError('receipt does not match the signed transaction')
    result = dict(mode=mode, tx=intent['tx'], status=receipt.status, block=receipt.blockNumber,
                  gas_used=receipt.gasUsed, cost_wei=receipt.gasUsed * receipt.effectiveGasPrice,
                  before=intent['before'])
    durable_json(out, result)
    if receipt.status != 1:
        raise RuntimeError('mined Ethereum transaction reverted; reconcile recorded receipt')
    # Balance telemetry is optional and must never delay the durable receipt.
    try:
        result['after'] = balances(eth, receipt.blockNumber)
    except Exception as error:
        result['after_error'] = str(error)[:200]
    durable_json(out, result)


def submit(w, mode, d, out):
    if mode not in ('checkpoint', 'withdraw') or w.eth.chain_id != 1:
        raise ValueError('unexpected action or Ethereum network')
    if out.exists():
        read_eth_result(mode, out)
        return
    journal = intent_path(out)
    if journal.exists():
        record_receipt(w.eth, mode, journal, out, recover=True)
        return
    report=json.loads((d/'proof-report.json').read_text());assert report['verified'] and report['proof_mode']=='groth16'
    values=(d/'public-values.bin').read_bytes();proof=(d/'proof-calldata.bin').read_bytes()
    acct=Account.from_key(KEY.read_text().strip());assert acct.address==OPERATOR
    ver=w.eth.contract(address=VERIFIER,abi=ABI);vault=w.eth.contract(address=VAULT,abi=ABI)
    fn=ver.functions.advanceCheckpoint(values,proof) if mode=='checkpoint' else vault.functions.withdrawWithProof(values,proof)
    before = balances(w.eth, 'latest')
    fn.call({'from':acct.address});gas=fn.estimate_gas({'from':acct.address})
    base=w.eth.get_block('latest')['baseFeePerGas'];tip=w.to_wei('0.05','gwei');maxfee=base*2+tip;assert maxfee*gas*2<w.to_wei('0.002','ether'),'gas cost guard'
    tx=fn.build_transaction({'from':acct.address,'nonce':w.eth.get_transaction_count(acct.address, 'pending'),'gas':int(gas*1.3),'maxFeePerGas':maxfee,'maxPriorityFeePerGas':tip,'chainId':1})
    signed = acct.sign_transaction(tx)
    tx_hash = Web3.to_hex(signed.hash)
    durable_json(journal, dict(mode=mode, tx=tx_hash, chain_id=1, nonce=tx['nonce'],
                               to=tx['to'], signed=Web3.to_hex(signed.raw_transaction),
                               **{'from': acct.address}, before=before))
    broadcast_hash = w.eth.send_raw_transaction(signed.raw_transaction)
    if Web3.to_hex(broadcast_hash) != tx_hash:
        raise RuntimeError('provider returned a different transaction id; reconcile journal')
    record_receipt(w.eth, mode, journal, out)


def main():
    mode, directory, out = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
    # A sender can outlive its worker. Serialize reconciliation and first send for this action.
    with out.with_suffix('.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        submit(Web3(Web3.HTTPProvider(RPC, request_kwargs={'timeout':60})), mode, directory, out)


if __name__=='__main__':main()
