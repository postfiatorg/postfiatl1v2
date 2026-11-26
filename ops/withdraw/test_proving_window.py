"""Ethereum withdrawals report closed outside a configured proving window, and a proof the window's
close cut waits for the next window instead of using the route retries. No setting: as before."""
import os
import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path
from unittest.mock import patch

import service

BURN = 'ab' * 48


class WindowTest(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.dir = Path(tmp.name)
        self.jobs = self.dir / 'jobs'
        self.jobs.mkdir()
        self.mode = self.dir / 'window.mode'
        self.clock = datetime(2026, 11, 26, 12, 0, tzinfo=timezone.utc)
        for name, value in (('JOBS', self.jobs), ('utcnow', lambda: self.clock)):
            patcher = patch.object(service, name, value, create=True)
            patcher.start()
            self.addCleanup(patcher.stop)
        env = patch.dict(os.environ, {'WITHDRAW_PROVING_WINDOW_FILE': str(self.mode),
                                      'WITHDRAW_PROVING_WINDOW_HOURS': '17:00-19:00'})
        env.start()
        self.addCleanup(env.stop)
        probe = patch.object(service, 'prover_answers', return_value=True, create=True)
        probe.start()
        self.addCleanup(probe.stop)

    def ready(self):
        bucket = {'policy_hash': 'p', 'bucket_id': 'b', 'outstanding_vault_bridge_atoms': 5}
        status = dict(buckets=[bucket], issuer='i', finalized_epoch=2, finalized_reserve_packet_hash='r')
        route = dict(beacon_finality_anchor={'route_profile_hash': 'p'}, current_height=1)
        with patch.object(service, 'bridge_status', return_value=status), \
             patch.object(service, 'call', return_value=route), \
             patch.object(service, 'payout_preflight', return_value=None):
            return service.readiness()

    def test_readiness_is_closed_outside_the_window_and_says_when_it_opens(self):
        ready = self.ready()
        self.assertFalse(ready['ready'])
        self.assertFalse(ready['payout_ready'])
        self.assertIn('proving window closed', ready['message'])
        self.assertEqual(ready['next_window_opens_at'], '2026-11-26T17:00:00Z')

    def test_readiness_is_open_inside_the_window(self):
        self.mode.write_text('none\n')
        ready = self.ready()
        self.assertTrue(ready['ready'], ready.get('message'))
        self.assertIsNone(ready['next_window_opens_at'])

    def test_readiness_is_closed_when_the_prover_does_not_answer_in_an_open_window(self):
        self.mode.write_text('deposits\n')
        with patch.object(service, 'prover_answers', return_value=False, create=True):
            ready = self.ready()
        self.assertFalse(ready['ready'])
        self.assertIn('prover does not answer', ready['message'])

    def test_without_a_setting_readiness_is_as_before(self):
        with patch.dict(os.environ, clear=True):
            ready = self.ready()
        self.assertTrue(ready['ready'], ready.get('message'))

    def test_a_request_posted_while_closed_waits_for_the_window(self):
        service.stage(dict(burn_tx_id=BURN), 'queued', 'Waiting for the withdrawal worker.')
        with patch.object(service, 'process') as process:
            service.run_jobs()
            process.assert_not_called()
            job = service.read_job(BURN)
            self.assertEqual(job['stage'], 'waiting_for_window')
            self.assertIn('proving window', job['detail'])
            self.mode.write_text('none\n')
            service.run_jobs()
            process.assert_called_once()

    def test_a_proof_cut_by_the_close_waits_for_the_next_window_without_using_the_retries(self):
        self.mode.write_text('none\n')
        service.stage(dict(burn_tx_id=BURN), 'queued')

        def cut(job):
            self.mode.unlink()  # the 19:00 close removes window.mode, then the session goes
            raise RuntimeError('GPU proof failed: connection closed')

        with patch.object(service, 'process', side_effect=cut) as process:
            service.run_jobs()
            job = service.read_job(BURN)
            self.assertEqual(job['stage'], 'waiting_for_window')
            self.assertNotIn('route_attempts', job)
            self.assertEqual(self.ready()['blocked_jobs'], [])
            service.run_jobs()
            self.assertEqual(process.call_count, 1, 'nothing is tried while the window is closed')
        self.mode.write_text('none\n')
        with patch.object(service, 'process') as process:
            service.run_jobs()
            process.assert_called_once()



class EthereumActionTest(WindowTest):
    """A job that has, or may have, an Ethereum action is never parked by the window (B01)."""

    def test_a_route_failure_of_a_job_with_a_checkpoint_result_is_not_parked(self):
        folder = self.jobs / BURN / 'checkpoint-1-3'
        folder.mkdir(parents=True)
        (folder / 'eth-result.json').write_text('{}')
        service.stage(dict(burn_tx_id=BURN, journal_version=1), 'catching_up')
        with patch.object(service, 'process', side_effect=RuntimeError('archive export failed')):
            service.run_jobs()
        self.assertNotEqual(service.read_job(BURN)['stage'], 'waiting_for_window')

    def test_a_recorded_payout_without_files_is_reconciled_while_closed(self):
        job = dict(burn_tx_id=BURN, ethereum_tx='0x' + '11' * 32)
        service.stage(job, 'paid')
        service.stage(job, 'queued')
        with patch.object(service, 'process') as process:
            service.run_jobs(startup=True)
        process.assert_called_once()

    def test_a_parked_row_with_a_recorded_identity_recovers_by_itself(self):
        service.stage(dict(burn_tx_id=BURN, ethereum_tx='0x' + '11' * 32, reconcile_paid_only=True), 'waiting_for_window')
        with patch.object(service, 'process') as process:
            service.run_jobs(startup=True)
        process.assert_called_once()


class UnsentWorkTest(WindowTest):
    """Nothing unsent starts proof work or a new Ethereum action while the window is closed (B02)."""

    def test_restarted_unsent_jobs_in_every_active_stage_wait(self):
        for stage in ('reading', 'catching_up', 'proving', 'paying'):
            with self.subTest(stage=stage):
                service.stage(dict(burn_tx_id=BURN, journal_version=1), stage)
                with patch.object(service, 'process') as process:
                    service.run_jobs(startup=True)
                process.assert_not_called()
                self.assertEqual(service.read_job(BURN)['stage'], 'waiting_for_window')

    def test_a_due_route_retry_waits_without_using_an_attempt(self):
        service.stage(dict(burn_tx_id=BURN, journal_version=1, error_scope='route', route_attempts=1,
                           retry_at='2000-01-01T00:00:00+00:00', retry_exhausted=False), 'failed')
        with patch.object(service, 'process') as process:
            service.run_jobs()
        process.assert_not_called()
        job = service.read_job(BURN)
        self.assertEqual((job['stage'], job['route_attempts']), ('waiting_for_window', 1))

    def test_no_new_proof_and_no_new_send_while_closed_but_recovery_goes_on(self):
        folder = self.jobs / BURN
        folder.mkdir()
        with patch.object(service.subprocess, 'run') as run:
            with self.assertRaises(service.WindowClosed):
                service.prove('withdrawal', folder / 'w.json', folder / 'proof', 'x')
            with self.assertRaises(service.WindowClosed):
                service.eth_send('withdraw', folder / 'proof', folder / 'eth-withdrawal.json')
            run.assert_not_called()
            (folder / 'eth-withdrawal.intent.json').write_text('{}')  # a journaled payout: its recovery runs
            run.return_value.returncode = 0
            with patch.object(service, 'read_eth_result', return_value={}):
                service.eth_send('withdraw', folder / 'proof', folder / 'eth-withdrawal.json')
            run.assert_called_once()

    def test_a_proof_that_ends_as_the_window_closes_waits_before_its_send(self):
        service.stage(dict(burn_tx_id=BURN, journal_version=1), 'queued')
        self.mode.write_text('none\n')
        with patch.object(service, 'process', side_effect=service.WindowClosed('the send waits')):
            service.run_jobs()
        job = service.read_job(BURN)
        self.assertEqual(job['stage'], 'waiting_for_window')
        self.assertNotIn('route_attempts', job)


class SettingTest(WindowTest):
    """A supplied setting that is invalid fails closed with its own reason (B03)."""

    def refusal(self, **env):
        with patch.dict(os.environ, env):
            return self.ready()

    def test_invalid_hours_fail_closed_naming_the_setting(self):
        for hours in ('garbage', '17:00-17:00'):
            with self.subTest(hours=hours):
                ready = self.refusal(WITHDRAW_PROVING_WINDOW_HOURS=hours)
                self.assertFalse(ready['ready'])
                self.assertIn('WITHDRAW_PROVING_WINDOW_HOURS', ready['message'])
                self.assertNotIn('proving window closed', ready['message'])

    def test_invalid_hours_are_not_bypassed_by_an_open_state_file(self):
        self.mode.write_text('none\n')
        ready = self.refusal(WITHDRAW_PROVING_WINDOW_HOURS='garbage')
        self.assertFalse(ready['ready'])
        self.assertIn('WITHDRAW_PROVING_WINDOW_HOURS', ready['message'])

    def test_readiness_reports_the_state_files_condition(self):
        self.assertEqual(self.ready()['proving_window_file_state'], 'missing')
        self.mode.write_text('bogus\n')
        ready = self.ready()
        self.assertEqual(ready['proving_window_file_state'], 'invalid content')
        self.assertIn('WITHDRAW_PROVING_WINDOW_FILE', ready['message'])
        self.mode.unlink()
        self.mode.mkdir()
        self.assertTrue(self.ready()['proving_window_file_state'].startswith('unreadable'))

if __name__ == '__main__':
    unittest.main()
