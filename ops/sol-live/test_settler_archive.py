"""The settlement loop's archive, built as install-exit-settler.sh extracts it, imports its whole
runtime: the loop, the reviewed settlement path and the payout module with its bootstrap."""

import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]


def extracted_paths():
    script = (HERE / "install-exit-settler.sh").read_text()
    (line,) = re.findall(r'^tar -xf "\$archive" -C "\$app" --no-same-owner --no-same-permissions (.+?) \\$', script, re.M)
    return line.split()


class SettlerArchiveTest(unittest.TestCase):
    def test_the_installed_archive_imports_the_settlement_runtime(self):
        paths = extracted_paths()
        with tempfile.TemporaryDirectory() as tmp:
            archive = Path(tmp) / "exit-settler.tar"
            try:
                subprocess.run(["git", "archive", "-o", str(archive), "HEAD", *paths], cwd=REPO, check=True,
                               capture_output=True)
            except (OSError, subprocess.CalledProcessError) as error:
                raise unittest.SkipTest(f"no git checkout to archive: {error}")
            app = Path(tmp) / "app"
            app.mkdir()
            subprocess.run(["tar", "-xf", str(archive), "-C", str(app), *paths], check=True)
            # Only the dependencies the installer's venv provides come from outside the archive.
            deps = os.pathsep.join(p for p in sys.path if p and not p.startswith(str(REPO)))
            done = subprocess.run(
                [sys.executable, "-c", "import exit_settler, exit_govern, exit_canary, exit_payout, exit_review"],
                cwd=app / "ops/sol-live", env=dict(os.environ, PYTHONPATH=deps), capture_output=True, text=True)
            self.assertEqual(done.returncode, 0, done.stderr[-600:])


if __name__ == "__main__":
    unittest.main()
