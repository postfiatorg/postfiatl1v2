import fcntl
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest


class UploadTest(unittest.TestCase):
    def test_incomplete_upload_times_out_without_reserving_gpu(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            lock = root / "gpu.lock"
            runner = root / "runner"
            script = Path(__file__).with_name("exit-prover-command.sh").read_text()
            runner.write_text(script.replace("/workspace/b23-withdraw-20261116", d)
                              .replace("/workspace/b23-gpu/prover.lock", str(lock)))
            # Shorten the upload deadline in this test environment.
            timeout = root / "timeout"
            timeout.write_text('#!/bin/bash\nshift; exec /usr/bin/timeout 1 "$@"\n')
            timeout.chmod(0o755)
            env = dict(os.environ, PATH=d + ":" + os.environ["PATH"],
                       SSH_ORIGINAL_COMMAND="pf-sol-exit-checkpoint")
            with (lock.open("w") as gpu, subprocess.Popen(
                ["bash", runner], stdin=subprocess.PIPE, env=env) as child):
                try:
                    until = time.monotonic() + 3
                    while not list((root / "jobs").glob("*/witness.json")):
                        if time.monotonic() > until:
                            self.fail("Upload did not start")
                        time.sleep(0.01)
                    fcntl.flock(gpu, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    self.assertEqual(child.wait(timeout=3), 124)
                    self.assertEqual(list((root / "jobs").iterdir()), [])
                finally:
                    child.stdin.close()
