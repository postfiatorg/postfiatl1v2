import copy
import os
from pathlib import Path
import tempfile
import unittest

import json
import stat
from unittest import mock

from postfiat_ops import validator_signer_check
from postfiat_ops.validator_signer_check import assess, key_copies, private_path


class DeploymentCheckTests(unittest.TestCase):
    def setUp(self):
        def process(uid, args, fds=()):
            return {'pid': uid, 'start_time': '1', 'uids': [uid] * 4, 'args': args,
                    'cwd': '/', 'fds': list(fds), 'exe_sha256': 'abc',
                    'capabilities': {'CapEff': 0, 'CapPrm': 0, 'CapAmb': 0}}
        self.expected = dict(data_dir='/srv/node', database='/srv/node/db/state.redb',
                             state_dir='/srv/signer/guard', key_file='/srv/signer/key.json',
                             socket='/run/signer.sock', node_sha256='abc')
        self.signer = process(1100, ['node', 'validator-signer-serve', '--key-file', '/srv/signer/key.json',
                                    '--state-dir', '/srv/signer/guard', '--socket', '/run/signer.sock',
                                    '--allow-peer-uid', '1101'])
        self.rpc = process(1101, ['node', 'rpc-serve', '--data-dir', '/srv/node',
                                '--finality-signer-socket', '/run/signer.sock'], ['/srv/node/db/state.redb'])
        self.gateway = process(1102, ['node', 'gateway.js'])
        self.report = {'ready': False, 'reasons': ['governance_not_activated'], 'custody': {
            'published_by': 'rpc_serve', 'validator_signer': 'socket', 'transactional_active': True,
            'signer_uid': 1100, 'serving_uid': 1101, 'signer_double_sign_protection': True,
            'signer_key_matches': True, 'validator_registry_readable': True,
            'data_dir_key_files': []}}

    def result(self):
        return assess(self.signer, self.rpc, self.gateway, self.report, self.expected, {'filesystem_checked': True})

    def test_custody_pass_does_not_imply_governance_or_release_readiness(self):
        result = self.result()
        self.assertTrue(result['passed'])
        self.assertFalse(result['readiness_ready'])
        self.assertNotIn('args', str(result))

    def test_copy_store_or_different_executable_refused(self):
        self.rpc['fds'] = ['/srv/copy/db/state.redb']
        self.assertFalse(self.result()['passed'])
        self.rpc['fds'] = ['/srv/node/db/state.redb']
        self.rpc['exe_sha256'] = 'old'
        self.assertFalse(self.result()['passed'])

    def test_gateway_access_and_shared_users_refused(self):
        for fd in ['/srv/node/validator_keys.json', '/srv/signer/key.json', '/srv/signer/guard/state.json', '/run/signer.sock']:
            self.gateway['fds'] = [fd]
            self.assertFalse(self.result()['passed'])
        self.gateway['fds'] = []
        self.gateway['uids'] = [1101] * 4
        self.assertFalse(self.result()['passed'])

    def test_missing_guard_live_signal_and_file_fallback_refused(self):
        self.report['custody'].pop('signer_double_sign_protection')
        self.assertFalse(self.result()['passed'])
        self.report['custody']['signer_double_sign_protection'] = True
        self.rpc['args'] += ['--finality-key-file', '/any/key']
        self.assertFalse(self.result()['passed'])

    def test_restart_initialization_and_extra_signing_uid_refused(self):
        self.signer['args'] += ['--initialize-state']
        self.assertFalse(self.result()['passed'])
        self.signer['args'].pop()
        self.signer['args'] += ['--allow-peer-uid', '1102']
        self.assertFalse(self.result()['passed'])

    def test_key_copy_in_the_data_directory_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            data = Path(temporary) / 'node'
            (data / 'db').mkdir(parents=True)
            self.assertEqual(key_copies(data, ['/srv/signer/proposal.json']), [])
            for name in ('validator_keys.json', 'db/proposal.json'):
                (data / name).write_text('{}')
                copies = key_copies(data, ['/srv/signer/proposal.json'])
                self.assertIn(str(data / name), copies)
                result = assess(self.signer, self.rpc, self.gateway, self.report, self.expected,
                                {'data_dir_holds_no_key_file': not copies})
                self.assertFalse(result['passed'])
                (data / name).unlink()
        self.report['custody']['data_dir_key_files'] = ['validator_keys.json']
        self.assertFalse(self.result()['passed'])

    def test_search_root_reports_a_key_copy_the_rpc_or_gateway_uid_can_read(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            os.chmod(root, 0o755)
            signer_dir = root / 'signer'
            backup = root / 'backup'
            for directory, mode in ((signer_dir, 0o700), (backup, 0o755)):
                directory.mkdir()
                os.chmod(directory, mode)
            (signer_dir / 'validator_keys.json').write_text('{}')
            decoy = backup / 'validator_keys.json'
            decoy.write_text('{}')
            private = backup / 'node' / 'validator_keys.json'
            private.parent.mkdir()
            private.write_text('{}')
            os.chmod(private, 0o600)
            os.chmod(decoy, 0o644)
            rpc = dict(self.rpc, gids=[1101] * 4, groups=[])
            readers = [rpc, dict(self.gateway, gids=[1102] * 4, groups=[])]
            copies = validator_signer_check.readable_key_copies(
                [root], signer_dir / 'validator_keys.json', [], readers)
            # The decoy is world-readable; the signer's own key and a copy
            # only its owner can read are not reported.
            self.assertEqual(copies, [str(decoy)])
            os.chmod(decoy, 0o600)
            self.assertEqual(validator_signer_check.readable_key_copies(
                [root], signer_dir / 'validator_keys.json', [], readers), [])
            os.chmod(decoy, 0o640)
            os.chown(decoy, -1, os.getgid())
            readers[0]['groups'] = [os.getgid()]
            self.assertEqual(validator_signer_check.readable_key_copies(
                [root], signer_dir / 'validator_keys.json', [], readers), [str(decoy)])

    def test_capabilities_on_rpc_or_gateway_refused(self):
        for role in ('rpc', 'gateway'):
            for name in ('CapEff', 'CapPrm', 'CapAmb'):
                setUp = getattr(self, role)
                setUp['capabilities'][name] = 1 << 2  # CAP_DAC_READ_SEARCH
                self.assertFalse(self.result()['passed'], (role, name))
                setUp['capabilities'][name] = 0
        del self.rpc['capabilities']
        self.assertFalse(self.result()['passed'])

    def test_non_object_readiness_result_exits_two(self):
        with tempfile.TemporaryDirectory() as temporary:
            out = Path(temporary) / 'report'
            procs = {1100: self.signer, 1101: self.rpc, 1102: self.gateway}
            with mock.patch.object(validator_signer_check, 'process', side_effect=procs.__getitem__), \
                 mock.patch.object(validator_signer_check, 'owns_listener', return_value=True), \
                 mock.patch.object(validator_signer_check, 'private_path', return_value=True), \
                 mock.patch.object(validator_signer_check, 'readiness_with_database_observation', return_value=['not', 'an', 'object']), \
                 mock.patch.object(validator_signer_check.Path, 'lstat', return_value=mock.Mock(st_mode=stat.S_IFSOCK | 0o660, st_uid=1100)), \
                 mock.patch('sys.stdout'):
                code = validator_signer_check.main([
                    '--signer-pid', '1100', '--rpc-pid', '1101', '--gateway-pid', '1102', '--rpc-port', '1',
                    '--node-sha256', 'abc', '--data-dir', temporary, '--database', 'db', '--key-file', 'k',
                    '--state-dir', 's', '--socket', 'sock', '--output', str(out)])
            self.assertEqual(code, 2)
            self.assertFalse(json.loads((out / 'report.json').read_text())['passed'])

    def test_posix_custody_refuses_symlink_shared_file_and_writable_parent(self):
        # A private root of the test's own, so TMPDIR's ancestors do not decide.
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            check = lambda path: private_path(path, os.geteuid(), test_root=root)
            key = root / 'key'
            key.write_bytes(b'not a real key')
            key.chmod(0o600)
            self.assertTrue(check(key))
            link = root / 'link'
            link.symlink_to(key)
            self.assertFalse(check(link))
            key.chmod(0o640)
            self.assertFalse(check(key))
            key.chmod(0o600)
            root.chmod(0o770)
            self.assertFalse(check(key))


if __name__ == '__main__':
    unittest.main()
