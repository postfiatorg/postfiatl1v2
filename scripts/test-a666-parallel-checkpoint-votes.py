#!/usr/bin/env python3
"""A failed fanout must retain evidence without weakening certificate quorum."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

path = Path(__file__).with_name('a666-parallel-checkpoint-votes.py')
spec = importlib.util.spec_from_file_location('checkpoint_fanout', path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class FanoutEvidenceTests(unittest.TestCase):
    def test_incomplete_quorum_reports_votes_and_errors_then_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            hosts = root / 'hosts.json'; hosts.write_text(json.dumps({f'validator-{i}': f'host-{i}' for i in range(6)}))
            checkpoint = root / 'checkpoint.json'; checkpoint.write_text('{}')
            args = SimpleNamespace(hosts_file=hosts, checkpoint_file=checkpoint, proof_dir=root,
                workflow_id='recovery', remote_suffix='test', remote_node='/node', ethereum_rpc='http://127.0.0.1:28703', validator2_remote_root='/votes')
            def collect(**kwargs):
                validator = kwargs['validator']
                if validator in {'validator-3', 'validator-5'}:
                    raise subprocess.CalledProcessError(1, ['ssh', validator], stderr='retained failure detail')
                return {'validator': validator, 'remote_vote_file': '/votes/' + validator + '.json'}
            output = io.StringIO()
            with patch.object(module, 'parse_args', return_value=args), patch.object(module, 'collect_vote', side_effect=collect), contextlib.redirect_stdout(output):
                with self.assertRaisesRegex(RuntimeError, '4/6'):
                    module.main()
            evidence = json.loads(output.getvalue())
            self.assertEqual(evidence['validator_count'], 4)
            self.assertEqual(len(evidence['remote_vote_files']), 4)
            self.assertEqual([r['validator'] for r in evidence['failures']], ['validator-3', 'validator-5'])
            self.assertTrue(all(r['stderr'] == 'retained failure detail' for r in evidence['failures']))

if __name__ == '__main__': unittest.main()
