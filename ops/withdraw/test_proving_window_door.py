"""The proving window at the door only: readiness says closed outside WITHDRAW_PROVING_WINDOW_HOURS,
so the screens, which ask readiness first, start no new withdrawal then. Nothing else changes:
a job underway, or a request posted anyway, is handled exactly as without the setting."""
import io
import json
import subprocess
import sys
import tempfile
import unittest
from datetime import datetime, time, timedelta, timezone
from pathlib import Path
from unittest.mock import patch

import service

BURN = 'ab' * 48
TODAY = datetime.now(timezone.utc).date()  # no fixed date: every test builds its times from today


def at(hour, minute, day=0):
    return datetime.combine(TODAY + timedelta(days=day), time(hour, minute), tzinfo=timezone.utc)


def iso(moment):
    return moment.strftime('%Y-%m-%dT%H:%M:%SZ')


class Door(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.jobs = Path(tmp.name)
        patcher = patch.object(service, 'JOBS', self.jobs)
        patcher.start()
        self.addCleanup(patcher.stop)

    def answer(self, hours, now):
        """readiness() for `hours` (None: unset) at `now`, with the chain and the preflight replaced."""
        bucket = {'policy_hash': 'p', 'bucket_id': 'b', 'outstanding_vault_bridge_atoms': 5}
        status = dict(buckets=[bucket], issuer='i', finalized_epoch=2, finalized_reserve_packet_hash='r')
        route = dict(beacon_finality_anchor={'route_profile_hash': 'p'}, current_height=1)
        window = service.parse_window_hours(hours) if hasattr(service, 'parse_window_hours') else None
        with patch.object(service, 'bridge_status', return_value=status), \
             patch.object(service, 'call', return_value=route), \
             patch.object(service, 'payout_preflight', return_value=None), \
             patch.object(service, 'WINDOW', window, create=True), \
             patch.object(service, 'window_now', lambda: now, create=True):
            return service.readiness()

    def test_readiness_follows_the_hours_at_their_edges(self):
        for now, is_open in ((at(16, 59), False), (at(17, 0), True), (at(18, 59), True), (at(19, 0), False)):
            with self.subTest(now=now.time()):
                ready = self.answer('17:00-19:00', now)
                self.assertEqual(ready['ready'], is_open, ready.get('message'))
                self.assertIs(ready['proving_window_open'], is_open)
                if is_open:
                    self.assertNotIn('next_window_opens_at', ready)
                else:
                    opens = at(17, 0) if now < at(17, 0) else at(17, 0, day=1)
                    self.assertEqual(ready['next_window_opens_at'], iso(opens))
                    self.assertEqual(ready['message'], 'Ethereum withdrawals are closed: the proving window is '
                                     f'17:00 to 19:00 UTC; it next opens at {iso(opens)}')

    def test_a_window_crossing_midnight(self):
        for now, is_open in ((at(23, 30), True), (at(0, 30), True), (at(1, 30), False)):
            with self.subTest(now=now.time()):
                ready = self.answer('23:00-01:00', now)
                self.assertIs(ready['proving_window_open'], is_open)
                if not is_open:
                    self.assertEqual(ready['next_window_opens_at'], iso(at(23, 0)))

    def test_an_invalid_setting_is_closed_and_named(self):
        for hours in ('garbage', '17:00-17:00', '25:00-26:00', ''):
            with self.subTest(hours=hours):
                ready = self.answer(hours, at(18, 0))
                self.assertFalse(ready['ready'])
                self.assertFalse(ready['proving_window_open'])
                self.assertIn('WITHDRAW_PROVING_WINDOW_HOURS', ready['message'])
                self.assertIn('invalid', ready['message'])

    def test_unset_gives_the_answer_of_main_whole(self):
        ready = self.answer(None, at(12, 0))
        self.assertFalse({'proving_window_open', 'next_window_opens_at'} & set(ready))
        open_answer = self.answer('00:00-23:59', at(12, 0))
        self.assertEqual(ready, {k: v for k, v in open_answer.items() if k != 'proving_window_open'})
        # The same inputs through main's own service.py, when this checkout has its history.
        try:
            source = subprocess.run(['git', 'show', 'b7506921e:ops/withdraw/service.py'], cwd=Path(__file__).parent,
                                    capture_output=True, text=True, check=True).stdout
        except (OSError, subprocess.CalledProcessError):
            self.skipTest('main (b7506921e) is not in this checkout')
        import importlib.util
        path = self.jobs / 'service_main.py'
        path.write_text(source)
        sys.path.insert(0, str(Path(__file__).parent))
        spec = importlib.util.spec_from_file_location('service_main', path)
        main = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(main)
        bucket = {'policy_hash': 'p', 'bucket_id': 'b', 'outstanding_vault_bridge_atoms': 5}
        status = dict(buckets=[bucket], issuer='i', finalized_epoch=2, finalized_reserve_packet_hash='r')
        route = dict(beacon_finality_anchor={'route_profile_hash': 'p'}, current_height=1)
        with patch.object(main, 'JOBS', self.jobs), patch.object(main, 'bridge_status', return_value=status), \
             patch.object(main, 'call', return_value=route), patch.object(main, 'payout_preflight', return_value=None):
            self.assertEqual(main.readiness(), ready)

    def test_run_jobs_and_a_post_behave_the_same_closed_and_unset(self):
        def trace(hours):
            for old in self.jobs.glob('*/job.json'):
                old.unlink()
            calls = []
            window = service.parse_window_hours(hours) if (hours and hasattr(service, 'parse_window_hours')) else None
            with patch.object(service, 'WINDOW', window, create=True), \
                 patch.object(service, 'window_now', lambda: at(3, 0), create=True), \
                 patch.object(service, 'process', side_effect=lambda job: calls.append(job['stage'])), \
                 patch.object(service, 'queue_event'):
                body = json.dumps(dict(burn_tx_id=BURN)).encode()
                handler = service.Handler.__new__(service.Handler)
                handler.path, handler.headers, handler.rfile = '/api/bridge/withdrawals', {'Content-Length': str(len(body))}, io.BytesIO(body)
                replies = []
                handler.reply = lambda code, value: replies.append((code, {k: v for k, v in value.items()
                                                                           if k not in ('created_at', 'updated_at')}))
                handler.do_POST()
                service.run_jobs()
                job = {k: v for k, v in service.read_job(BURN).items() if k not in ('created_at', 'updated_at', 'history')}
            return replies, calls, job

        self.assertEqual(trace('17:00-19:00'), trace(None))


if __name__ == '__main__':
    unittest.main()
