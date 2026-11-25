"""install-exit-review.sh, run into a temporary root with fake system commands: the reviewer user
alone holds its key, the signing history is readable by name only, and every failed step stops it."""
import json
import os
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).parent
SCRIPT = HERE / "install-exit-review.sh"
FAKES = {
    "id": 'if [[ $1 == -u && $2 == pf-sol-exit-review && ! -e "$STATE/user" ]]; then exit 1; fi; echo 1001',
    "useradd": 'echo "useradd $*" >> "$LOG"; touch "$STATE/user"',
    "usermod": 'echo "usermod $*" >> "$LOG"',
    "setfacl": 'echo "setfacl $*" >> "$LOG"',
    "systemctl": 'echo "systemctl $*" >> "$LOG"; '
                 '[[ "$*" != "start pft-sol-exit-review.service" || -z ${FIRST_RUN_FAILS:-} ]]',
    "chown": 'echo "chown $*" >> "$LOG"; [[ -z ${CHOWN_FAILS:-} ]]',
    # Ownership needs root: keep the modes, drop -o and -g.
    "install": 'args=(); skip=0; for a; do if ((skip)); then skip=0; continue; fi; '
               'case $a in -o|-g) skip=1;; *) args+=("$a");; esac; done; exec /usr/bin/install "${args[@]}"',
    # A venv whose python is this test's python (without -I, to see the test's packages) and whose
    # pip only records its arguments.
    "python3": 'if [[ "$1 $2" == "-m venv" ]]; then mkdir -p "$3/bin"; '
               'printf "#!/bin/bash\\nif [[ \\"\\$1 \\$2\\" == \\"-m pip\\" ]]; then echo \\"pip \\$*\\" >> \\"\\$LOG\\"; exit 0; fi\\n'
               'a=(); for x; do [[ \\$x == -I ]] || a+=(\\"\\$x\\"); done; exec \\"$REAL_PYTHON\\" \\"\\${a[@]}\\"\\n" > "$3/bin/python"; '
               'chmod 755 "$3/bin/python"; exit 0; fi; '
               'if [[ -n ${OLD_PYTHON:-} && $1 == -c ]]; then exit 1; fi; exec "$REAL_PYTHON" "$@"',
}
FILES = ("exit_review.py", "pft-sol-exit-review.service", "pft-sol-exit-review.timer", "exit-requirements.lock")


class Host:
    def __init__(self, d):
        self.d = Path(d)
        self.root = self.d / "root"
        (self.root / "etc/postfiat").mkdir(parents=True)
        (self.root / "etc/systemd/system").mkdir(parents=True)
        (self.root / "var/lib/postfiat/validator-3/consensus-v2-safety").mkdir(parents=True)
        self.bin = self.d / "bin"
        self.bin.mkdir()
        for name, body in FAKES.items():
            (self.bin / name).write_text("#!/bin/bash\n" + body + "\n")
            (self.bin / name).chmod(0o755)
        self.log = self.d / "log"
        self.log.touch()
        # The archive as `git archive -o exit-review.tar <commit> ops/sol-live` lays it out.
        self.archive = self.d / "exit-review.tar"
        with tarfile.open(self.archive, "w") as tar:
            for name in FILES:
                tar.add(HERE / name, arcname=f"ops/sol-live/{name}")

    def run(self, **env):
        environment = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}", LOG=str(self.log), STATE=str(self.d),
                           REAL_PYTHON=sys.executable, INSTALL_ROOT=str(self.root), **env)
        return subprocess.run(["bash", str(SCRIPT), "validator-3", "27653", "https://solana.example/host-3",
                               str(self.archive)], capture_output=True, text=True, env=environment)


class InstallTest(unittest.TestCase):
    def test_installs_the_reviewer_as_its_own_user_with_its_own_key(self):
        with tempfile.TemporaryDirectory() as d:
            host = Host(d)
            result = host.run()
            self.assertEqual(result.returncode, 0, result.stderr)
            etc = host.root / "etc/postfiat"
            self.assertEqual((etc / "sol-exit-review.key").stat().st_mode & 0o777, 0o600)
            self.assertEqual((etc / "sol-exit-review.pub").stat().st_mode & 0o777, 0o644)
            self.assertEqual((etc / "sol-exit-review.json").stat().st_mode & 0o777, 0o640)
            config = json.loads((etc / "sol-exit-review.json").read_text())
            self.assertEqual(config, dict(validator="validator-3", node=["127.0.0.1", 27653],
                                          solana_rpc="https://solana.example/host-3",
                                          review_dir="/var/lib/postfiat-exit-review",
                                          data_dir="/var/lib/postfiat/validator-3"))
            log = host.log.read_text().splitlines()
            self.assertIn(f"chown pf-sol-exit-review:pf-sol-exit-review {etc}/sol-exit-review.key", log)
            self.assertIn("usermod -a -G pf-sol-exit-review postfiat", log)
            data = host.root / "var/lib/postfiat/validator-3"
            self.assertIn(f"setfacl -m u:pf-sol-exit-review:x {data}", log)
            self.assertIn(f"setfacl -m u:pf-sol-exit-review:rx {data}/consensus-v2-safety", log)
            self.assertTrue(any(line.startswith("pip -m pip install --quiet --require-hashes") for line in log))
            self.assertIn("systemctl enable --now pft-sol-exit-review.timer", log)
            self.assertEqual((host.root / "var/lib/postfiat-exit-review").stat().st_mode & 0o777, 0o750)

    def test_a_rerun_keeps_the_key_and_restores_a_missing_public_key(self):
        with tempfile.TemporaryDirectory() as d:
            host = Host(d)
            self.assertEqual(host.run().returncode, 0)
            etc = host.root / "etc/postfiat"
            key, public = (etc / "sol-exit-review.key").read_bytes(), (etc / "sol-exit-review.pub").read_text()
            (etc / "sol-exit-review.pub").unlink()
            result = host.run()
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(((etc / "sol-exit-review.key").read_bytes(), (etc / "sol-exit-review.pub").read_text()),
                             (key, public))

    def test_every_failed_step_stops_the_install_with_its_reason(self):
        cases = {
            "CHOWN_FAILS": "cannot give the reviewer key",
            "FIRST_RUN_FAILS": "the first review failed",
            "OLD_PYTHON": "3.10 or newer",
        }
        for variable, reason in cases.items():
            with self.subTest(variable), tempfile.TemporaryDirectory() as d:
                host = Host(d)
                result = host.run(**{variable: "1"})
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(reason, result.stderr)
                self.assertNotIn("enable --now", host.log.read_text())
                key = host.root / "etc/postfiat/sol-exit-review.key"
                if key.exists():
                    self.assertEqual(key.stat().st_mode & 0o077, 0, "never readable by others")

    def test_the_runbook_install_commands_work_in_a_temporary_root(self):
        text = (HERE / "RUNBOOK-LIVE.md").read_text().split("### Independent settlement reviews", 1)[1]
        block = text.split("```bash", 1)[1].split("```", 1)[0]
        self.assertNotIn("< ops/sol-live/install-exit-review.sh", block)
        self.addCleanup(lambda: (HERE.parents[1] / "exit-review.tar").unlink(missing_ok=True))
        with tempfile.TemporaryDirectory() as d:
            host = Host(d)
            (host.root / "root").mkdir()
            # scp copies into the host's /root; ssh runs the command on the "host" (the temporary root).
            (host.bin / "scp").write_text('#!/bin/bash\nfor a in "${@:1:$#-1}"; do cp "$a" "$INSTALL_ROOT/root/"; done\n')
            (host.bin / "ssh").write_text('#!/bin/bash\nshift\nexec bash -c "${*//\\/root\\//$INSTALL_ROOT/root/}"\n')
            for name in ("scp", "ssh"):
                (host.bin / name).chmod(0o755)
            env = dict(os.environ, PATH=f"{host.bin}:{os.environ['PATH']}", LOG=str(host.log), STATE=str(host.d),
                       REAL_PYTHON=sys.executable, INSTALL_ROOT=str(host.root), COMMIT="HEAD", HOST="validator-host",
                       VALIDATOR="validator-3", PORT="27653", URL="https://solana.example/host-3")
            result = subprocess.run(["bash", "-euo", "pipefail", "-c", block], cwd=HERE.parents[1], env=env,
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("Reviewer installed for validator-3", result.stdout)
            self.assertTrue((host.root / "opt/postfiat/sol-exit-review/exit_review.py").exists())


if __name__ == "__main__":
    unittest.main()
