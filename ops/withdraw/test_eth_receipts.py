"""Ethereum withdrawal recovery with a local job store and no external calls."""
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import service
from web3 import Web3
from test_eth_finality import chain


class EthereumReceiptTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.jobs = Path(tmp.name)
        fixture = Path(__file__).with_name('fixtures') / 'witness.json'
        witness = json.loads(fixture.read_text())
        packet = witness['withdrawal_packet']
        self.burn = packet['burn_tx_id']
        self.terms = {k:packet[k] for k in ('burn_tx_id','withdrawal_id','recipient','amount_atoms')}
        self.terms['packet_digest'] = '42'*32
        self.eth = chain(self.terms, height=123)[0]
        self.height = packet['finalized_height']
        self.folder = self.jobs / self.burn
        self.folder.mkdir()
        self.result = self.folder / 'eth-withdrawal.json'
        self.receipt = dict(mode='withdraw', tx='ab' * 32, status=1, block=123)
        self.block = {'header': {'height': self.height}, 'receipt_ids': [self.burn]}
        self.archive = {
            'tip': {key: witness[key] for key in ('chain_id', 'genesis_hash', 'protocol_version')},
            'validator_registry': {'validators': []},
            'blocks': {'blocks': [self.block]},
            'ledger': {'vault_bridge_redemptions': [
                {'withdrawal_packet': packet, 'created_at_height': self.height}
            ]},
        }
        self.verifier = {'latestFinalizedHeight': self.height}
        audit = {'contracts': {'PFTLFinalityVerifierV1': {'reads': self.verifier}}}
        votes = [{'receipts': [{'tx_id': self.burn, 'accepted': True}]}] * 6
        for mocked in (
            patch.object(service, 'JOBS', self.jobs),
            patch.object(service, 'export_bundle', return_value=(None, self.archive)),
            patch.object(service, 'read_six', return_value=votes),
            patch.object(service, 'audit', return_value=audit),
        ):
            self.addCleanup(mocked.stop)
            mocked.start()

    def test_process_accepts_only_successful_cached_withdrawals(self):
        for changes in ({}, {'status': 0}, {'mode': 'checkpoint'}, {'status': None},
                        {'status': True}, {'tx': 'abc'}, {'block': -1}):
            with self.subTest(changes=changes), patch.object(service.subprocess, 'run') as send, patch.object(service, 'ethereum_reader') as web:
                web.return_value = self.eth
                self.result.write_text(json.dumps(self.receipt | changes))
                job = dict(self.terms)
                if changes:
                    with self.assertRaisesRegex(RuntimeError, 'reconcile'):
                        service.process(job)
                    self.assertNotEqual(service.read_job(self.burn)['stage'], 'paid')
                else:
                    service.process(job)
                    saved = service.read_job(self.burn)
                    self.assertEqual(saved['stage'], 'paid')
                    self.assertEqual(saved['ethereum_tx'], '0x' + self.receipt['tx'])
                    self.assertEqual(saved['ethereum_block'], self.receipt['block'])
                send.assert_not_called()

    def test_checkpoint_cannot_advance_past_saved_revert(self):
        start = self.height - service.MAX_STEP - 1
        end = self.height - 1
        self.verifier['latestFinalizedHeight'] = start
        for height in range(start, self.height):
            self.archive['blocks']['blocks'].append({'header': {
                'height': height, 'block_hash': 'ab' * 48,
                'consensus_v2_commit': {'proposal': {'domain': {'committee_epoch': 1}}},
            }})
        folder = self.folder / f'checkpoint-{start}-{end}'
        folder.mkdir()
        (folder / 'eth-result.json').write_text(json.dumps(self.receipt | {'mode': 'checkpoint', 'status': 0}))
        with patch.object(service, 'prove') as prove, patch.object(service.subprocess, 'run') as send:
            with self.assertRaisesRegex(RuntimeError, 'checkpoint receipt'):
                service.process({'burn_tx_id': self.burn})
            self.assertEqual(service.read_job(self.burn)['stage'], 'catching_up')
            send.assert_not_called()
            prove.assert_not_called()

    def test_new_receipt_must_confirm_even_if_sender_exits_zero(self):
        def sender(*args, **kwargs):
            self.result.write_text(json.dumps(self.receipt | {'status': 0}))
            return service.subprocess.CompletedProcess([], 0)

        with patch.object(service.subprocess, 'run', side_effect=sender) as send:
            with self.assertRaisesRegex(RuntimeError, 'reconcile'):
                service.eth_send('withdraw', self.folder, self.result)
            send.assert_called_once()

    def test_unreadable_saved_receipt_requires_reconciliation(self):
        self.result.write_text('{')
        with patch.object(service.subprocess, 'run') as send:
            with self.assertRaisesRegex(RuntimeError, 'reconcile'):
                service.process({'burn_tx_id': self.burn})
            send.assert_not_called()
