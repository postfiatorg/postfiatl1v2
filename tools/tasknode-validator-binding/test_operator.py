import contextlib
import io
import json
from pathlib import Path
import types
import unittest
from unittest.mock import patch


source = Path(__file__).with_name('l1-validator.sh').read_text().split("<<'PY'\n", 1)[1].rsplit('\nPY', 1)[0]
source = source.split('\ntry:\n    main()')[0]
operator = types.ModuleType('operator_script')
exec(compile(source, 'l1-validator.sh', 'exec'), operator.__dict__)


class OperatorTest(unittest.TestCase):
    def discovery(self, processes, node_state='{"node_id":"validator-0"}\npftmac1:public-mac'):
        def read_bytes(path):
            return processes[str(path)]
        with patch.object(operator.glob, 'glob', return_value=list(processes)), \
             patch.object(operator.pathlib.Path, 'read_bytes', read_bytes), \
             patch.object(operator.pathlib.Path, 'read_text', return_value=node_state):
            return operator.discover()

    def test_exact_running_config_and_appended_mac(self):
        result = self.discovery({'/proc/1/cmdline': b'/opt/postfiat-node\0transport-validator-serve\0--key-file\0/private/key.json\0--data-dir\0/state\0'})
        self.assertEqual(result, ('/private/key.json','validator-0'))

    def test_no_validator_no_search_and_ambiguous_processes(self):
        for processes in [
            {'/proc/1/cmdline': b'/opt/postfiat-node\0rpc-serve\0--key-file\0/private/key.json\0--data-dir\0/state\0'},
            {'/proc/1/cmdline': b'/opt/postfiat-node\0transport-validator-serve\0--key-file\0relative.json\0--data-dir\0/state\0'},
            {'/proc/1/cmdline': b'/opt/postfiat-node\0transport-validator-serve\0--key-file\0/a.json\0--data-dir\0/state\0',
             '/proc/2/cmdline': b'/opt/postfiat-node\0transport-validator-serve\0--key-file\0/b.json\0--data-dir\0/state\0'},
        ]:
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit): self.discovery(processes)

    def test_invalid_token_exits_before_network_or_discovery(self):
        with patch.object(operator.sys, 'argv', ['script','secret-not-a-token']), \
             patch.object(operator, 'discover') as discover, patch.object(operator,'request') as request, \
             contextlib.redirect_stderr(io.StringIO()) as stderr, self.assertRaises(SystemExit):
            operator.main()
        self.assertNotIn('secret-not-a-token', stderr.getvalue())
        discover.assert_not_called(); request.assert_not_called()

    def test_hash_mismatch_prevents_challenge_and_signing(self):
        with patch.object(operator.sys, 'argv', ['script','vct_'+'a'*64]), \
             patch.object(operator, 'discover', return_value=('/fake','validator-0')), \
             patch.object(operator.subprocess,'run',return_value=types.SimpleNamespace(returncode=0)) as run, \
             patch.object(operator,'request',return_value=b'wrong binary') as request, \
             contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            operator.main()
        self.assertEqual(request.call_count,1)
        self.assertEqual(run.call_count,1)
        self.assertEqual(run.call_args.args[0],['unshare','--net','--','true'])

    def test_no_namespace_means_no_network_and_no_signing(self):
        with patch.object(operator.sys, 'argv', ['script','vct_'+'a'*64]), \
             patch.object(operator, 'discover', return_value=('/fake','validator-0')), \
             patch.object(operator.subprocess,'run',return_value=types.SimpleNamespace(returncode=1)), \
             patch.object(operator,'request') as request, \
             contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            operator.main()
        request.assert_not_called()


if __name__ == '__main__': unittest.main()
