import copy
import os
from pathlib import Path
import tempfile
import unittest

from postfiat_ops.validator_signer_check import assess, private_path


class DeploymentCheckTests(unittest.TestCase):
    def setUp(self):
        def process(uid, args, fds=()):
            return {'pid': uid, 'start_time': '1', 'uids': [uid] * 4, 'args': args,
                    'cwd': '/', 'fds': list(fds), 'exe_sha256': 'abc'}
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
            'signer_key_matches': True, 'validator_registry_readable': True}}

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

    def test_posix_custody_refuses_symlink_shared_file_and_writable_parent(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            key = root / 'key'
            key.write_bytes(b'not a real key')
            key.chmod(0o600)
            self.assertTrue(private_path(key, os.geteuid()))
            link = root / 'link'
            link.symlink_to(key)
            self.assertFalse(private_path(link, os.geteuid()))
            key.chmod(0o640)
            self.assertFalse(private_path(key, os.geteuid()))
            key.chmod(0o600)
            root.chmod(0o770)
            self.assertFalse(private_path(key, os.geteuid()))


if __name__ == '__main__':
    unittest.main()
