"""The forced proof command takes the CUDA device and the staged tree from one settings file."""
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("exit-prover-command.sh")
START = Path(__file__).with_name("prover-start.sh")
FAKE = """#!/bin/sh
printf '%s\\n' "$@" > "$PWD/args"
printf '%s\\n' "$HOME" > "$PWD/home"
printf '%s\\n' "${PROOF_HOST_CUDA_DEVICE:-unset}" > "$PWD/env-device"
for a in "$@"; do last2=$prev; prev=$a; done
mkdir -p "$last2"
for f in public-values.bin proof-report.json proof-calldata.bin; do printf x > "$last2/$f"; done
"""


class ExitProverCommandTest(unittest.TestCase):
    def run_command(self, root, conf_text=None, *, env=None, mode=0o644, prefix=()):
        work = root / "work"
        work.mkdir()
        (work / "egress.elf").write_bytes(b"elf")
        host = work / "proof-host"
        host.write_text(FAKE)
        host.chmod(0o700)
        conf = root / "pf-sol-exit-prove.conf"
        if conf_text is not None:
            conf.write_text(conf_text.replace("@root@", str(root)))
            conf.chmod(mode)
        runner = root / "runner"
        runner.write_text(SCRIPT.read_text()
                          .replace("conf_owner=0", f"conf_owner={os.getuid()}")
                          .replace("/etc/pf-sol-exit-prove.conf", str(conf))
                          .replace("/workspace/b23-withdraw-20261116", str(work))
                          .replace("/workspace/b23-gpu/prover.lock", str(root / "gpu.lock")))
        runner.chmod(0o700)
        env = dict(os.environ, SSH_ORIGINAL_COMMAND="pf-sol-exit-withdrawal", HOME="/home/before", **(env or {}))
        result = subprocess.run([*prefix, str(runner)], input=b"{}", env=env, capture_output=True)
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


    def test_a_last_line_without_a_newline_is_read(self):
        with tempfile.TemporaryDirectory() as tmp:
            result, work = self.run_command(Path(tmp), "EXIT_CUDA_DEVICE=3")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((work / "args").read_text().split()[:2], ["--cuda-device", "3"])

    def test_a_settings_file_writable_by_others_is_refused(self):
        for mode in (0o664, 0o646):
            with self.subTest(mode=oct(mode)), tempfile.TemporaryDirectory() as tmp:
                result, work = self.run_command(Path(tmp), "EXIT_CUDA_DEVICE=3\n", mode=mode)
                self.assertEqual(result.returncode, 78)
                self.assertIn(b"owned by root", result.stderr)
                self.assertFalse((work / "args").exists())

    def test_the_callers_device_variable_never_reaches_the_proof_host(self):
        with tempfile.TemporaryDirectory() as tmp:
            result, work = self.run_command(Path(tmp), "EXIT_CUDA_DEVICE=3\n", env={"PROOF_HOST_CUDA_DEVICE": "1"})
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((work / "env-device").read_text().strip(), "unset")

    def test_the_withdrawal_device_may_not_be_in_the_deposit_list(self):
        for text in ("EXIT_CUDA_DEVICE=3\nDEPOSIT_CUDA_DEVICES=0,1,2,3\n", "DEPOSIT_CUDA_DEVICES=0,1,2\n"):
            with self.subTest(text=text), tempfile.TemporaryDirectory() as tmp:
                result, work = self.run_command(Path(tmp), text)
                self.assertEqual(result.returncode, 78)
                self.assertIn(b"also in DEPOSIT_CUDA_DEVICES", result.stderr)
                self.assertFalse((work / "args").exists())
        with tempfile.TemporaryDirectory() as tmp:
            result, _ = self.run_command(Path(tmp), "EXIT_CUDA_DEVICE=3\nDEPOSIT_CUDA_DEVICES=0,1,2\n")
            self.assertEqual(result.returncode, 0, result.stderr)


# Fake GPU servers: a script named sp1-gpu-server with CUDA_VISIBLE_DEVICES set, in a
# private PID namespace whose init (bwrap) adopts orphans, so "no live parent" is parent 1.
def namespace():
    return ["bwrap", "--unshare-pid", "--dev-bind", "/", "/", "--proc", "/proc", "--die-with-parent", "bash", "-c"]


BWRAP = shutil.which("bwrap") and subprocess.run(
    ["bwrap", "--unshare-pid", "--dev-bind", "/", "/", "--proc", "/proc", "true"], capture_output=True).returncode == 0


def servers(root, *specs):
    """Shell that starts fake servers: ("orphan", dev) leaves one with no parent, ("owned", dev)
    one whose parent stays alive; then runs "$@" and prints which servers are still alive."""
    server = root / "bin" / "sp1-gpu-server"
    server.parent.mkdir(exist_ok=True)
    # A script keeps its own name as the process name, which `pgrep -x` matches.
    server.write_text("#!/bin/bash\nwhile :; do sleep 1; done\n")
    server.chmod(0o755)
    lines = []
    for i, (kind, dev) in enumerate(specs):
        if kind == "orphan":
            lines.append(f'(CUDA_VISIBLE_DEVICES={dev} setsid {server} & echo $! > {root}/pid{i}) ; sleep 0.2')
        else:
            lines.append(f'(CUDA_VISIBLE_DEVICES={dev} {server} & echo $! > {root}/pid{i}; wait) & sleep 0.2')
    report = "; ".join(f'kill -0 $(cat {root}/pid{i}) 2>/dev/null && echo alive{i} >> {root}/alive'
                       for i in range(len(specs)))
    return "\n".join(lines) + f'\n"$@"; status=$?; sleep 0.2; {report}; exit $status'


@unittest.skipUnless(BWRAP, "needs bwrap with a PID namespace")
class StaleServerTest(unittest.TestCase):
    def test_an_orphaned_server_on_the_withdrawal_device_is_stopped_and_a_live_ones_refuses(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            script = servers(root, ("orphan", 3), ("orphan", 2))
            result, work = ExitProverCommandTest.run_command(
                ExitProverCommandTest(), root, "EXIT_CUDA_DEVICE=3\n", prefix=(*namespace(), script, "_"))
            self.assertEqual(result.returncode, 0, result.stderr)
            alive = (root / "alive").read_text().split() if (root / "alive").exists() else []
            self.assertEqual(alive, ["alive1"], "the orphan on device 3 is stopped, the one on 2 is not ours")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            script = servers(root, ("owned", 3))
            result, work = ExitProverCommandTest.run_command(
                ExitProverCommandTest(), root, "EXIT_CUDA_DEVICE=3\n", prefix=(*namespace(), script, "_"))
            self.assertEqual(result.returncode, 75, result.stderr)
            self.assertIn(b"device 3 is used by GPU server", result.stderr)
            self.assertFalse((work / "args").exists())
            self.assertEqual((root / "alive").read_text().split(), ["alive0"])


class ProverStartTest(unittest.TestCase):
    """prover-start.sh in a temporary tree: serve.py is a fake that records its device count."""

    def start(self, root, conf_text=None, *, env=None, prefix=()):
        tree = root / "speed-i2"
        (tree / "circuit-install/v6.1.0").mkdir(parents=True)
        (tree / "circuit-install/v6.1.0/x").write_text("x")
        (tree / "circuits.sha256").write_text(
            subprocess.run(["sha256sum", "circuit-install/v6.1.0/x"], cwd=tree, capture_output=True, text=True).stdout
            .replace("circuit-install/v6.1.0/", ""))
        (tree / "serve.py").write_text("import sys, pathlib; pathlib.Path('count').write_text(sys.argv[-1])\n")
        conf = root / "pf-sol-exit-prove.conf"
        if conf_text is not None:
            conf.write_text(conf_text)
        runner = root / "start"
        runner.write_text(START.read_text().replace("/workspace/sol-ingress/speed-i2", str(tree))
                          .replace("/etc/pf-sol-exit-prove.conf", str(conf))
                          .replace("conf_owner=0", f"conf_owner={os.getuid()}"))
        environment = {k: v for k, v in os.environ.items() if k != "SOL_DEPOSIT_GPUS"}
        result = subprocess.run([*prefix, "bash", str(runner)], env={**environment, **(env or {})}, capture_output=True)
        count = (tree / "count").read_text() if (tree / "count").exists() else None
        return result, count

    def test_without_the_settings_file_the_deposit_prover_takes_four_devices(self):
        with tempfile.TemporaryDirectory() as tmp:
            result, count = self.start(Path(tmp))
            self.assertEqual((result.returncode, count), (0, "4"), result.stderr)

    def test_the_settings_file_gives_the_deposit_devices(self):
        with tempfile.TemporaryDirectory() as tmp:
            result, count = self.start(Path(tmp), "EXIT_CUDA_DEVICE=3\nDEPOSIT_CUDA_DEVICES=0,1,2\n")
            self.assertEqual((result.returncode, count), (0, "3"), result.stderr)

    def test_an_overlap_or_a_contradicting_variable_is_refused(self):
        cases = {
            "overlap": ("EXIT_CUDA_DEVICE=3\nDEPOSIT_CUDA_DEVICES=0,1,2,3\n", {}, b"also the withdrawal device"),
            "no list": ("EXIT_CUDA_DEVICE=3\n", {}, b"names no DEPOSIT_CUDA_DEVICES"),
            "contradiction": ("EXIT_CUDA_DEVICE=3\nDEPOSIT_CUDA_DEVICES=0,1,2\n", {"SOL_DEPOSIT_GPUS": "4"},
                              b"contradicts"),
        }
        for name, (text, env, message) in cases.items():
            with self.subTest(name), tempfile.TemporaryDirectory() as tmp:
                result, count = self.start(Path(tmp), text, env=env)
                self.assertNotEqual(result.returncode, 0, name)
                self.assertIn(message, result.stderr, name)
                self.assertIsNone(count, name)

    @unittest.skipUnless(BWRAP, "needs bwrap with a PID namespace")
    def test_a_stale_server_on_a_device_no_longer_owned_is_stopped_and_a_live_withdrawal_server_kept(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            script = servers(root, ("orphan", 3), ("owned", 3), ("orphan", 1))
            result, count = self.start(root, "EXIT_CUDA_DEVICE=3\nDEPOSIT_CUDA_DEVICES=0,1,2\n",
                                       prefix=(*namespace(), script, "_"))
            self.assertEqual((result.returncode, count), (0, "3"), result.stderr)
            self.assertEqual((root / "alive").read_text().split(), ["alive1"])


if __name__ == "__main__":
    unittest.main()
