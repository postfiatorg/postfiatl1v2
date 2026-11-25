"""Finalized Ethereum event confirmation using local RPC responses only."""
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from eth_abi import encode
from hexbytes import HexBytes
from web3 import Web3
from web3.exceptions import TransactionNotFound

import service
from eth_finality import AwaitingFinality, confirm_action
from eth_journal import ReconciliationRequired, durable_json
from eth_send import VAULT


def terms(burn='11'*48):
    return dict(burn_tx_id=burn, withdrawal_id='22'*48, recipient='0x'+'33'*20,
                amount_atoms=42, packet_digest='0x'+'44'*32)


def chain(expected=None, tx='0x'+'ab'*32, height=100):
    expected = expected or terms()
    tx_hash, block_hash = HexBytes(tx), HexBytes('0x'+'ca'*32)
    sig = Web3.keccak(text='ProofNativeWithdrawal(bytes32,bytes32,bytes32,address,uint256)')
    topics = [sig, Web3.keccak(hexstr=expected['withdrawal_id']), Web3.keccak(hexstr=expected['burn_tx_id']),
              HexBytes(expected['packet_digest'])]
    payload = encode(['address','uint256'], [expected['recipient'],int(expected['amount_atoms'])])
    log = dict(address=VAULT, topics=topics, data=payload, transactionHash=tx_hash, blockHash=block_hash, logIndex=1)
    receipt = dict(status=1, transactionHash=tx_hash, blockNumber=height, blockHash=block_hash, to=VAULT, logs=[log])
    canonical = dict(hash=block_hash, number=height, transactions=[tx_hash])
    finalized = dict(hash=HexBytes('0x'+'ff'*32), number=height+2)
    eth = Mock(chain_id=1)
    eth.get_block.side_effect = lambda name: finalized if name=='finalized' else canonical
    eth.get_transaction_receipt.return_value = receipt
    return eth, receipt, canonical, finalized


class FinalityTests(unittest.TestCase):
    def test_reorg_until_same_transaction_reappears_in_a_finalized_block(self):
        expected = terms()
        eth, receipt, canonical, finalized = chain(expected)
        tx = Web3.to_hex(receipt['transactionHash'])
        finalized['number'] = 98
        with self.assertRaises(AwaitingFinality):
            confirm_action(eth, 'withdraw', tx, expected)
        finalized['number'] = 102
        canonical['hash'] = HexBytes('ee'*32)
        with self.assertRaises(AwaitingFinality):
            confirm_action(eth, 'withdraw', tx, expected)
        eth.get_transaction_receipt.side_effect = TransactionNotFound(tx)
        with self.assertRaises(AwaitingFinality):
            confirm_action(eth, 'withdraw', tx, expected)
        eth.get_transaction_receipt.side_effect = None
        receipt.update(blockNumber=101, blockHash=canonical['hash'])
        receipt['logs'][0]['blockHash'] = canonical['hash']
        canonical['number'] = 101
        done = confirm_action(eth, 'withdraw', tx, expected)
        self.assertEqual((done['block'], done['block_hash'], done['log_index']), (101, Web3.to_hex(canonical['hash']), 1))

    def test_unrelated_success_receipt_and_all_mismatched_withdrawal_fields_are_rejected(self):
        for wrong in (dict(burn_tx_id='99'*48),dict(withdrawal_id='99'*48),dict(packet_digest='99'*32),
                      dict(amount_atoms=41),dict(recipient='0x'+'99'*20)):
            eth, receipt, _, _ = chain()
            with self.subTest(wrong=wrong), self.assertRaises(ReconciliationRequired):
                confirm_action(eth, 'withdraw', receipt['transactionHash'], terms() | wrong)
        for changes in (dict(status=0),dict(to='0x'+'99'*20),dict(logs=[])):
            eth, receipt, _, _ = chain()
            receipt.update(changes)
            with self.subTest(changes=changes), self.assertRaises(ReconciliationRequired):
                confirm_action(eth,'withdraw',receipt['transactionHash'],terms())

    def test_historical_paid_is_hidden_then_checked_even_if_receipt_file_is_absent(self):
        expected = terms()
        eth, receipt, _, _ = chain(expected)
        tx = Web3.to_hex(receipt['transactionHash'])
        with tempfile.TemporaryDirectory() as tmp, patch.object(service,'JOBS',Path(tmp)), \
             patch.object(service, 'ethereum_reader') as web, patch.object(service,'eth_send') as send, \
             patch.object(service,'export_bundle') as archive:
            web.return_value = eth
            job = dict(expected,ethereum_tx=tx)
            service.stage(job,'paid','legacy status')
            self.assertEqual(service.Handler.public(None,job)['stage'],'awaiting_finality')
            eth.get_transaction_receipt.side_effect = TransactionNotFound(tx)
            service.run_jobs(startup=True)
            self.assertEqual(service.read_job(expected['burn_tx_id'])['stage'],'awaiting_finality')
            eth.get_transaction_receipt.side_effect = None
            service.run_jobs(startup=True)
            saved = service.read_job(expected['burn_tx_id'])
            self.assertEqual(saved['stage'],'paid')
            self.assertEqual(saved['ethereum_finality']['expected'],expected)
            self.assertEqual(saved['ethereum_finality']['block_hash'],Web3.to_hex(receipt['blockHash']))
            self.assertTrue((Path(tmp)/expected['burn_tx_id']/'eth-withdrawal.json').exists())
            send.assert_not_called()
            archive.assert_not_called()

    def test_paid_row_with_another_burns_cached_receipt_is_reconciled_without_sending(self):
        expected = terms()
        eth, receipt, _, _ = chain(terms('09'*48))
        tx = Web3.to_hex(receipt['transactionHash'])
        with tempfile.TemporaryDirectory() as tmp, patch.object(service,'JOBS',Path(tmp)), \
             patch.object(service, 'ethereum_reader') as web, patch.object(service,'eth_send') as send:
            web.return_value = eth
            job = dict(expected,ethereum_tx=tx)
            service.stage(job,'paid')
            durable_json(Path(tmp)/expected['burn_tx_id']/'eth-withdrawal.json',
                         dict(mode='withdraw',tx=tx,status=1,block=100))
            service.run_jobs(startup=True)
            self.assertEqual(service.read_job(expected['burn_tx_id'])['stage'],'reconciliation_required')
            send.assert_not_called()
