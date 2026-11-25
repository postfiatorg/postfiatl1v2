"""Confirm a coordinator action against a mainnet RPC's finalized chain and receipt logs."""
from eth_abi import encode
from hexbytes import HexBytes
from web3 import Web3
from web3.exceptions import TransactionNotFound

from eth_journal import ReconciliationRequired
from eth_send import VAULT, VERIFIER


class AwaitingFinality(ReconciliationRequired):
    """Ethereum has not yet supplied a canonical, finalized receipt for this identity."""


def commitment(value):
    raw = HexBytes(value)
    if len(raw) != 48:
        raise ValueError('invalid Post Fiat identifier')
    return Web3.keccak(raw)


def confirm_action(eth, mode, tx, expected):
    if eth.chain_id != 1:
        raise ReconciliationRequired('Unexpected Ethereum chain; reconcile the RPC endpoint.')
    tx_hash = HexBytes(tx)
    if len(tx_hash) != 32:
        raise ReconciliationRequired('Invalid Ethereum transaction id; reconcile the job.')
    try:
        receipt = eth.get_transaction_receipt(tx_hash)
    except TransactionNotFound as error:
        raise AwaitingFinality('Ethereum has no canonical receipt yet for '+Web3.to_hex(tx_hash)) from error

    height = receipt['blockNumber']
    block_hash = receipt['blockHash']
    finalized = eth.get_block('finalized')
    if height is None or finalized['number'] < height:
        raise AwaitingFinality('Waiting for Ethereum finality for '+Web3.to_hex(tx_hash))
    canonical = eth.get_block(height)
    if (len(block_hash) != 32 or len(finalized['hash']) != 32
            or (height == finalized['number'] and block_hash != finalized['hash'])
            or canonical['hash'] != block_hash or canonical['number'] != height
            or tx_hash not in canonical['transactions'] or receipt['transactionHash'] != tx_hash):
        raise AwaitingFinality('Receipt is outside the canonical finalized chain; reconcile '+Web3.to_hex(tx_hash))
    contract = VAULT if mode == 'withdraw' else VERIFIER
    if receipt['status'] != 1 or (receipt.get('to') or '').lower() != contract.lower():
        raise ReconciliationRequired('Finalized Ethereum call failed or targeted a different contract; reconcile the receipt.')
    if mode == 'withdraw':
        signature = 'ProofNativeWithdrawal(bytes32,bytes32,bytes32,address,uint256)'
        topics = [Web3.keccak(text=signature), commitment(expected['withdrawal_id']),
                  commitment(expected['burn_tx_id']), HexBytes(expected['packet_digest'])]
        data = encode(['address','uint256'], [expected['recipient'], int(expected['amount_atoms'])])
    elif mode == 'checkpoint':
        signature = 'PFTLCheckpointAdvanced(bytes32,bytes32,uint64,bytes32)'
        topics = [Web3.keccak(text=signature), commitment(expected['prior']), commitment(expected['target'])]
        data = encode(['uint64'], [expected['height']])
    else:
        raise ValueError('unknown Ethereum action')
    matches = []
    for log in receipt['logs']:
        if (log['address'].lower() != contract.lower() or log.get('removed', False)
                or log['transactionHash'] != tx_hash or log['blockHash'] != block_hash
                or list(log['topics']) != topics):
            continue
        payload = HexBytes(log['data'])
        event_matches = payload == data if mode == 'withdraw' else len(payload) == 64 and payload[:32] == data
        if event_matches:
            matches.append(log['logIndex'])
    if len(matches) != 1:
        raise ReconciliationRequired('Finalized Ethereum event does not match the requested '+mode+'; reconcile the job.')
    return dict(chain_id=1, tx=Web3.to_hex(tx_hash), block=height, block_hash=Web3.to_hex(block_hash),
                finalized_head=finalized['number'], finalized_head_hash=Web3.to_hex(finalized['hash']),
                contract=contract, event=signature, log_index=matches[0], expected=expected)
