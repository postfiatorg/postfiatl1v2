"""The forced proof command takes the CUDA device and the staged tree from one settings file."""
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("exit-prover-command.sh")
FAKE = """#!/bin/sh
printf '%s\\n' "$@" > "$PWD/args"
printf '%s\\n' "$HOME" > "$PWD/home"
for a in "$@"; do last2=$prev; prev=$a; done
mkdir -p "$last2"
for f in public-values.bin proof-report.json proof-calldata.bin; do printf x > "$last2/$f"; done
"""


class ExitProverCommandTest(unittest.TestCase):
    def run_command(self, root, conf_text=None):
        work = root / "work"
        work.mkdir()
        (work / "egress.elf").write_bytes(b"elf")
        host = work / "proof-host"
        host.write_text(FAKE)
        host.chmod(0o700)
        conf = root / "pf-sol-exit-prove.conf"
        if conf_text is not None:
            conf.write_text(conf_text.replace("@root@", str(root)))
        runner = root / "runner"
        runner.write_text(SCRIPT.read_text()
                          .replace("/etc/pf-sol-exit-prove.conf", str(conf))
                          .replace("/workspace/b23-withdraw-20261116", str(work))
                          .replace("/workspace/b23-gpu/prover.lock", str(root / "gpu.lock")))
        runner.chmod(0o700)
        env = dict(os.environ, SSH_ORIGINAL_COMMAND="pf-sol-exit-withdrawal", HOME="/home/before")
        result = subprocess.run([str(runner)], input=b"{}", env=env, capture_output=True)
        return result, work

    def test_without_settings_the_call_is_unchanged(self):
        with tempfile.TemporaryDirectory() as tmp:
            result, work = self.run_command(Path(tmp))
            self.assertEqual(result.returncode, 0, result.stderr)
            args = (work / "args").read_text().split()
            self.assertNotIn("--cuda-device", args)
            self.assertEqual(args[:2], ["withdrawal", "egress.elf"])
            self.assertEqual((work / "home").read_text().strip(), "/home/before")

    def test_settings_choose_device_tree_lock_and_sp1_home(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "staged").mkdir()
            result, work = self.run_command(root, "# shared four-GPU machine\nEXIT_CUDA_DEVICE=3\n"
                                            "EXIT_WORK_DIR=@root@/work\nEXIT_LOCK=@root@/exit.lock\n"
                                            "EXIT_SP1_HOME=@root@/staged\n")
            self.assertEqual(result.returncode, 0, result.stderr)
            args = (work / "args").read_text().split()
            self.assertEqual(args[:4], ["--cuda-device", "3", "withdrawal", "egress.elf"])
            self.assertEqual((work / "home").read_text().strip(), str(root / "staged"))
            self.assertTrue((root / "exit.lock").exists())
            self.assertFalse((root / "gpu.lock").exists())

    def test_a_bad_setting_stops_before_the_prover_runs(self):
        for line in ("EXIT_CUDA_DEVICE=3,4", "EXIT_CUDA_DEVICE=", "EXIT_WORK_DIR=relative", "OTHER=1"):
            with self.subTest(line=line), tempfile.TemporaryDirectory() as tmp:
                result, work = self.run_command(Path(tmp), line + "\n")
                self.assertEqual(result.returncode, 78)
                self.assertIn(b"bad line", result.stderr)
                self.assertFalse((work / "args").exists())


if __name__ == "__main__":
    unittest.main()
