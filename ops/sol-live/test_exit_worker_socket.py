import os
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch


class QualificationSocketTest(unittest.TestCase):
    def test_socket_requires_worker_peer_and_requests_fresh_state(self):
        import exit_qualification_socket as q

        with TemporaryDirectory() as directory:
            path = Path(directory) / "qualify.sock"
            lookup = lambda name: SimpleNamespace(pw_uid=os.getuid())
            readiness = Mock(side_effect=[{"height": 1}, {"height": 2}, {"height": 3}])
            with patch.object(q.pwd, "getpwnam", side_effect=lookup), q.serve(path, readiness):
                self.assertEqual(q.qualify(path), {"height": 1})
                self.assertEqual(q.qualify(path), {"height": 2})
                with patch.object(q.pwd, "getpwnam", return_value=SimpleNamespace(pw_uid=os.getuid()+1)):
                    with self.assertRaisesRegex(PermissionError, "worker UID"):
                        q.qualify(path)
                self.assertEqual(readiness.call_count, 2)
            self.assertFalse(path.exists())

    def test_socket_refuses_a_shared_writable_directory(self):
        import exit_qualification_socket as q

        with TemporaryDirectory() as directory:
            os.chmod(directory, 0o777)
            with self.assertRaisesRegex(PermissionError, "directory"), q.serve(Path(directory)/"q", Mock()):
                self.fail("Unsafe socket directory")

    def test_open_uses_worker_socket_with_no_http_fallback(self):
        import exit_govern as g
        import sys

        with (patch.object(sys, "argv", ["govern", "open", "--revision", "rev", "--release-height", "8"]),
              patch.object(g, "fleet", return_value=dict(build_git_revision="rev", block_height=8,
                                                        block_tip_hash="tip", state_root="root")),
              patch.object(g, "ethereum"), patch.object(g, "six"), patch.object(g, "require_closed"),
              patch("requests.get", side_effect=AssertionError("Open attempted HTTP")),
              patch.object(g, "qualify", create=True, side_effect=PermissionError("wrong worker UID")),
              patch.object(g, "govern") as sign):
            with self.assertRaisesRegex(PermissionError, "worker UID"):
                g.main()
            sign.assert_not_called()
