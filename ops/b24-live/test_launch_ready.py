"""Focused refusal tests; no network access and no key-file reads."""
import copy
import json
import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path
from unittest.mock import patch

import launch_pool as launch
from clear_launch_stall import verify_candidate


class LaunchGuards(unittest.TestCase):
    def test_live_rpc_refuses_broadcast_signing_and_admin(self):
        provider = launch.ReadOnlyProvider('http://unused.invalid')
        for method in ('eth_sendRawTransaction', 'eth_sendTransaction', 'eth_sign', 'anvil_setStorageAt', 'personal_unlockAccount'):
            with self.assertRaisesRegex(RuntimeError, 'forbidden'):
                provider.make_request(method, [])

    def test_lock_owner_check_never_acquires_lock(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'chain.lock'
            path.write_text('')
            st = path.stat()
            import os
            ident = f'{os.major(st.st_dev):02x}:{os.minor(st.st_dev):02x}:{st.st_ino}'
            self.assertEqual(launch.held_locks(path, ''), [])
            self.assertTrue(launch.held_locks(path, f'1: FLOCK ADVISORY WRITE 42 {ident} 0 EOF'))

    def test_busy_chain_or_daily_refuses(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(launch, 'COORD', Path(tmp)):
            lock = Path(tmp) / 'chain.lock'
            lock.write_text('OTHER\n')
            with self.assertRaisesRegex(RuntimeError, 'occupied'):
                launch.check_coordination()
            lock.write_text('')
            with patch.object(launch, 'held_locks', return_value=['owned']):
                with self.assertRaisesRegex(RuntimeError, 'occupied'):
                    launch.check_coordination()
            for status in ('LoadState=not-found\n',
                           'LoadState=loaded\nMainPID=42\nActiveState=active\nResult=success\n',
                           'LoadState=loaded\nMainPID=0\nActiveState=failed\nResult=exit-code\n'):
                with patch.object(launch, 'held_locks', return_value=[]), patch.object(launch.subprocess, 'check_output', return_value=status):
                    with self.assertRaises(RuntimeError):
                        launch.check_coordination()

    def test_daily_window_refuses_and_idle_outside_window_passes(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(launch, 'COORD', Path(tmp)):
            (Path(tmp) / 'chain.lock').write_text('')
            state = 'LoadState=loaded\nMainPID=0\nActiveState=inactive\nResult=success\n'
            with patch.object(launch, 'held_locks', return_value=[]), patch.object(launch.subprocess, 'check_output', return_value=state), patch.object(launch, 'datetime') as clock:
                clock.now.return_value = datetime(2026, 11, 16, 17, 54, tzinfo=timezone.utc)
                with self.assertRaisesRegex(RuntimeError, 'Daily capture window'):
                    launch.check_coordination()
                clock.now.return_value = datetime(2026, 11, 16, 16, 0, tzinfo=timezone.utc)
                self.assertTrue(launch.check_coordination()['chain_lock_free'])

    def test_manual_repair_requires_all_six_matching_finalized_blocks(self):
        job = dict(last_error='storage_duplicate_record: ordered batch `x` already exists',
                   block_height=9, block_hash='a', expected_state_root='b')
        blocks = [[dict(header=dict(height=9, block_hash='a', state_root='b'))] for _ in range(6)]
        verify_candidate(job, blocks)
        for key in ('height', 'block_hash', 'state_root'):
            bad = copy.deepcopy(blocks)
            bad[5][0]['header'][key] = 'different'
            with self.assertRaises(ValueError):
                verify_candidate(job, bad)
        with self.assertRaises(ValueError):
            verify_candidate(job, blocks[:5])
        with self.assertRaises(ValueError):
            verify_candidate(dict(job, last_error='storage_writer_busy'), blocks)


if __name__ == '__main__':
    unittest.main()
