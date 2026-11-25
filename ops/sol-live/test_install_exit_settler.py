"""install-exit-settler.sh, run into a temporary root with fake system commands: its own user and
SSH key (mode 0600, never printed), the key line to add on each host, and no enabled unit unless
all six doors answer and the round needs no root login."""

import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).parent
SCRIPT = HERE / "install-exit-settler.sh"
FAKES = {
    "id": 'if [[ $1 == -u && ! -e "$STATE/user" ]]; then exit 1; fi; echo 1002',
    "useradd": 'echo "useradd $*" >> "$LOG"; touch "$STATE/user"',
    "systemctl": 'echo "systemctl $*" >> "$LOG"',
    "chown": 'echo "chown $*" >> "$LOG"',
    "install": 'args=(); skip=0; for a; do if ((skip)); then skip=0; continue; fi; '
               'case $a in -o|-g) skip=1;; *) args+=("$a");; esac; done; exec /usr/bin/install "${args[@]}"',
    "python3": 'if [[ "$1 $2" == "-m venv" ]]; then mkdir -p "$3/bin"; printf "#!/bin/bash\\nexit 0\\n" > "$3/bin/python"; '
               'chmod 755 "$3/bin/python"; exit 0; fi; exec "$REAL_PYTHON" "$@"',
    # The read-only door check, as the loop's user: the test says which hosts did not answer.
    "runuser": 'echo "runuser $*" >> "$LOG"; printf "%s\\n" "$DOORS_MISSING"',
}


class Relay:
    def __init__(self, d):
        self.d = Path(d)
        self.root = self.d / "root"
        for path in ("etc/postfiat", "etc/systemd/system"):
            (self.root / path).mkdir(parents=True)
        self.bin = self.d / "bin"
        self.bin.mkdir()
        for name, body in FAKES.items():
            (self.bin / name).write_text("#!/bin/bash\n" + body + "\n")
            (self.bin / name).chmod(0o755)
        self.log = self.d / "log"
        self.log.touch()
        self.archive = self.d / "exit-settler.tar"
        with tarfile.open(self.archive, "w") as tar:
            for name in ("exit-requirements.lock", "pft-sol-exit-settler.service", "pft-sol-exit-settler.timer",
                         "exit_settler.py"):
                tar.add(HERE / name, arcname=f"ops/sol-live/{name}")
            tar.add(HERE.parents[1] / "solana/deploy/route_binding.py", arcname="solana/deploy/route_binding.py")
        self.known = self.d / "known_hosts"
        self.known.write_text("192.0.2.1 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA\n")

    def run(self, missing=""):
        env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}", LOG=str(self.log), STATE=str(self.d),
                   REAL_PYTHON=sys.executable, INSTALL_ROOT=str(self.root), DOORS_MISSING=missing)
        return subprocess.run(["bash", str(SCRIPT), str(self.archive), str(self.known)],
                              capture_output=True, text=True, env=env)


@unittest.skipUnless(shutil.which("ssh-keygen"), "ssh-keygen is not installed")
class InstallSettlerTest(unittest.TestCase):
    def test_its_own_key_stays_private_and_its_line_is_printed_for_the_hosts(self):
        with tempfile.TemporaryDirectory() as d:
            relay = Relay(d)
            result = relay.run(missing="validator-2 (RuntimeError: Permission denied)")
            key = relay.root / "var/lib/pft-sol-exit-settler/.ssh/pft-sol-exit-settler"
            self.assertEqual(key.stat().st_mode & 0o777, 0o600)
            secret = key.read_text()
            self.assertNotIn(secret.strip().splitlines()[1], result.stdout + result.stderr)
            pub = key.with_suffix(".pub").read_text().strip()
            self.assertIn(f'restrict,command="/opt/postfiat/sol-exit-review/venv/bin/python -I '
                          f'/opt/postfiat/sol-exit-review/exit_review_door.py" {pub}', result.stdout)
            self.assertEqual((relay.root / "etc/postfiat/sol-exit-settler.env").stat().st_mode & 0o777, 0o640)
            self.assertTrue((relay.root / "etc/systemd/system/pft-sol-exit-settler.timer").is_file())

    def test_a_host_without_the_line_is_named_and_nothing_is_enabled(self):
        with tempfile.TemporaryDirectory() as d:
            relay = Relay(d)
            result = relay.run(missing="validator-2 (RuntimeError: Permission denied)")
            self.assertEqual(result.returncode, 3)
            self.assertIn("validator-2", result.stderr)
            self.assertIn("tmux loop stays in use", result.stderr)
            self.assertNotIn("enable", relay.log.read_text())

    def test_six_answering_doors_still_enable_nothing_while_the_round_needs_root(self):
        with tempfile.TemporaryDirectory() as d:
            relay = Relay(d)
            result = relay.run()
            self.assertEqual(result.returncode, 4)
            self.assertIn("root login", result.stderr)
            self.assertNotIn("enable", relay.log.read_text())

    def test_a_second_run_keeps_the_key(self):
        with tempfile.TemporaryDirectory() as d:
            relay = Relay(d)
            relay.run()
            key = relay.root / "var/lib/pft-sol-exit-settler/.ssh/pft-sol-exit-settler"
            first = key.read_bytes()
            relay.run()
            self.assertEqual(key.read_bytes(), first)


if __name__ == "__main__":
    unittest.main()
