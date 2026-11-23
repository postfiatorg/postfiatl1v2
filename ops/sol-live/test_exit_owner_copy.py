import os
from pathlib import Path
import shlex
import subprocess
from tempfile import TemporaryDirectory
import unittest

DOC = Path(__file__).with_name("RUNBOOK-LIVE.md")


class OwnerCopyTest(unittest.TestCase):
    def test_owner_copy_is_bounded_and_requires_exact_size(self):
        line = next(s for s in DOC.read_text().splitlines() if s.startswith("sudo ") and "bundle.tar" in s)
        body = shlex.split(line)[shlex.split(line).index("-ceu") + 1]
        copy = body.split('cd "$d"; ', 1)[1].split('printf "%s  bundle.tar', 1)[0]
        with TemporaryDirectory() as tmp:
            directory = Path(tmp)
            # Drop the privilege transition in this ordinary-user filesystem fixture.
            (directory / "runuser").write_text('#!/bin/sh\nshift 3\nexec "$@"\n')
            (directory / "runuser").chmod(0o755)
            env = dict(PATH=tmp + ":/usr/bin:/bin")
            def run(source, expected):
                return subprocess.run(["bash", "-ceu", copy, "_", "", str(source), str(expected)],
                                      cwd=directory, env=env, capture_output=True, timeout=3)

            source = directory / "input"
            source.write_bytes(b"x" * 42)
            self.assertEqual(run(source, 42).returncode, 0)
            self.assertNotEqual(run(source, 15).returncode, 0)
            self.assertLessEqual((directory / "bundle.tar").stat().st_size, 16)
            self.assertNotEqual(run(source, 43).returncode, 0)
            # Exercise the same deadline with a shorter clock for the FIFO fixture.
            (directory / "timeout").write_text(
                '#!/bin/sh\n[ "$1 $2" = "--kill-after=5s 120s" ] || exit 70\n'
                'shift 2\nexec /usr/bin/timeout --kill-after=1s 0.2s "$@"\n')
            (directory / "timeout").chmod(0o755)
            source.unlink()
            os.mkfifo(source)
            self.assertEqual(run(source, 42).returncode, 124)
