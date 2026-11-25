"""Exercise crash boundaries using real local signing and a memory-only Ethereum provider."""
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

from eth_account import Account
from web3 import Web3
from web3.exceptions import TransactionNotFound

from test_eth_finality import chain, terms
import eth_send
import service
from eth_journal import durable_json, intent_path, read_eth_result


class WorkerStopped(BaseException):
    pass


class RestartTests(unittest.TestCase):
    def test_restart_at_each_send_and_persistence_boundary(self):
        for mode in ('withdraw', 'checkpoint'):
            for boundary in ('before_broadcast', 'after_broadcast', 'before_receipt_write', 'after_receipt_write'):
                with self.subTest(mode=mode, boundary=boundary), tempfile.TemporaryDirectory() as tmp:
                    self.restart_at(mode, boundary, Path(tmp))

    def restart_at(self, mode, boundary, jobs):
        burn = '11' * 48
        job_dir = jobs / burn
        folder = job_dir if mode == 'withdraw' else job_dir / 'checkpoint-1-3'
        folder.mkdir(parents=True)
        out = folder / ('eth-withdrawal.json' if mode == 'withdraw' else 'eth-result.json')
        (folder/'proof-report.json').write_text('{"verified":true,"proof_mode":"groth16"}')
        for name in ('public-values.bin','proof-calldata.bin'):
            (folder/name).write_bytes(b'fixture')
        account = Account.create()
        key = jobs/'test-key'
        key.write_text(account.key.hex())
        client = Mock(to_wei=Web3.to_wei)
        eth = client.eth
        eth.chain_id = 1
        eth.get_balance.return_value = 1000
        eth.get_block.return_value = {'baseFeePerGas': 100}
        eth.get_transaction_count.return_value = 0
        functions = eth.contract.return_value.functions
        functions.latestFinalizedHeight.return_value.call.return_value = 3
        functions.balanceOf.return_value.call.return_value = 500
        fn = getattr(functions, 'withdrawWithProof' if mode == 'withdraw' else 'advanceCheckpoint').return_value
        destination = eth_send.VAULT if mode == 'withdraw' else eth_send.VERIFIER
        fn.estimate_gas.return_value = 90000
        fn.build_transaction.side_effect = lambda params: dict(params, to=destination, data=b'fixture')
        accepted = set()
        attempted = []
        crash = True

        def broadcast(raw):
            tx = Web3.to_hex(Web3.keccak(raw))
            saved = json.loads(intent_path(out).read_text())
            self.assertEqual((saved['tx'], saved['signed']), (tx, Web3.to_hex(raw)))
            self.assertEqual(intent_path(out).stat().st_mode & 0o777, 0o600)
            attempted.append(tx)
            if crash and boundary == 'before_broadcast':
                raise WorkerStopped()
            self.assertNotIn(tx, accepted)
            accepted.add(tx)
            if crash and boundary == 'after_broadcast':
                raise WorkerStopped()
            return Web3.keccak(raw)

        def lookup(tx):
            if tx not in accepted:
                raise TransactionNotFound(tx)
            return {'hash': tx}

        def wait(tx, timeout):
            self.assertIn(tx, accepted)
            return SimpleNamespace(transactionHash=bytes.fromhex(tx[2:]), status=1, blockNumber=22,
                                   blockHash=bytes.fromhex('ca'*32), gasUsed=100, effectiveGasPrice=2)

        def persist(path, value):
            if crash and path == out and boundary == 'before_receipt_write':
                raise WorkerStopped()
            durable_json(path, value)
            if crash and path == out and boundary == 'after_receipt_write':
                raise WorkerStopped()

        eth.send_raw_transaction.side_effect = broadcast
        eth.get_transaction.side_effect = lookup
        eth.wait_for_transaction_receipt.side_effect = wait

        def sender(action, proof_dir, result):
            eth_send.submit(client, action, proof_dir, result)
            return read_eth_result(action, result)

        with patch.object(eth_send, 'OPERATOR', account.address), patch.object(eth_send, 'KEY', key), \
             patch.object(eth_send, 'durable_json', side_effect=persist), patch.object(service, 'JOBS', jobs), \
             patch.object(service, 'eth_send', side_effect=sender), \
             patch.object(service, 'export_bundle', side_effect=WorkerStopped()) as export, \
             patch.object(service, 'ethereum_reader') as web:
            job = dict(terms(burn), journal_version=1)
            service.stage(job, 'catching_up' if mode == 'checkpoint' else 'paying')
            with self.assertRaises(WorkerStopped):
                eth_send.submit(client, mode, folder, out)
            self.assertTrue(intent_path(out).is_file())
            crash = False
            key.unlink()  # Recovery must not sign another transaction.
            tx = json.loads(intent_path(out).read_text())['tx']
            net, mined, _, _ = chain(terms(burn), tx=tx, height=22)
            web.return_value = net
            if mode == 'checkpoint':
                prior, target = '12'*48, '34'*48
                durable_json(folder/'witness.json', dict(prior_checkpoint_block_id=prior, block={'header':{'block_hash':target,'height':3}}))
                log = mined['logs'][0]
                mined['to'] = log['address'] = eth_send.VERIFIER
                log['topics'] = [Web3.keccak(text='PFTLCheckpointAdvanced(bytes32,bytes32,uint64,bytes32)'),
                                 Web3.keccak(hexstr=prior),Web3.keccak(hexstr=target)]
                log['data'] = (3).to_bytes(32,'big') + bytes(32)
            if mode == 'withdraw':
                service.run_jobs()
                service.run_jobs()
                self.assertEqual(service.read_job(burn)['stage'], 'paid')
                export.assert_not_called()
            else:
                with self.assertRaises(WorkerStopped):
                    service.run_jobs()
                self.assertEqual(service.read_job(burn)['stage'], 'reading')
                export.assert_called_once()
            tx = read_eth_result(mode, out)['tx']
            self.assertEqual(accepted, {tx})
            self.assertEqual(set(attempted), {tx})
            self.assertEqual(len(attempted), 2 if boundary == 'before_broadcast' else 1)
            fn.build_transaction.assert_called_once()

    def test_legacy_unknown_send_is_visible_without_resending(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(service, 'JOBS', Path(tmp)), \
             patch.object(service, 'eth_send') as send:
            for stage in ('paying', 'catching_up'):
                job = dict(burn_tx_id=stage)
                service.stage(job, stage)
                if stage == 'catching_up':
                    folder = Path(tmp)/stage/'checkpoint-1-2'
                    folder.mkdir()
                    (folder/'proof-report.json').touch()
                service.run_jobs()
                self.assertEqual(service.read_job(stage)['stage'], 'reconciliation_required')
            send.assert_not_called()

    def test_pending_payment_reconciles_immediately_on_startup(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(service, 'JOBS', Path(tmp)):
            job = dict(terms(), journal_version=1)
            service.stage(job, 'reconciliation_required', 'receipt pending')
            out = Path(tmp)/job['burn_tx_id']/'eth-withdrawal.json'
            durable_json(intent_path(out), dict(tx='0x'+'11'*32, mode='withdraw'))
            tx = '0x'+'11'*32
            def mine(action, folder, result):
                durable_json(result, dict(mode=action, tx=tx, block=55, status=1))
            with patch.object(service, 'eth_send', side_effect=mine) as send, patch.object(service, 'ethereum_reader') as web:
                web.return_value = chain(terms(), tx, 55)[0]
                service.run_jobs()
                send.assert_not_called()
                service.run_jobs(startup=True)
                self.assertEqual(service.read_job(job['burn_tx_id'])['stage'], 'paid')
                send.assert_called_once()

    def test_worker_resumes_pre_send_stages_and_surfaces_stalled_jobs(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(service, 'JOBS', Path(tmp)):
            for name in service.ACTIVE_STAGES:
                service.stage(dict(burn_tx_id=name), name)
            with patch.object(service, 'process') as process:
                service.run_jobs()
                self.assertEqual({c.args[0]['stage'] for c in process.call_args_list}, service.ACTIVE_STAGES)
            stale = service.read_job('proving')
            stale['updated_at'] = '2000-01-01T00:00:00+00:00'
            service.write_job(stale)
            service.stage(dict(burn_tx_id='unknown'), 'reconciliation_required')
            service.stage(dict(burn_tx_id='unfinalized', error_scope='burn'), 'failed')
            bucket = {'policy_hash':'p', 'bucket_id':'b', 'outstanding_vault_bridge_atoms':5}
            status = dict(buckets=[bucket], issuer='i', finalized_epoch=2, finalized_reserve_packet_hash='r')
            route = dict(beacon_finality_anchor={'route_profile_hash':'p'}, current_height=1)
            with patch.object(service, 'bridge_status', return_value=status), \
                 patch.object(service, 'call', return_value=route), patch.object(service, 'payout_preflight',return_value=None):
                ready = service.readiness()
            self.assertTrue(ready['bucket_ready'])
            self.assertFalse(ready['ready'])
            self.assertFalse(ready['payout_ready'])
            self.assertEqual(ready['stalled_jobs'], ['proving'])
            self.assertEqual(ready['blocked_jobs'], ['unknown'])
