"""install-exit-system.sh, sourced by bash with fake systemctl, getent, id, groupmod, usermod,
chgrp, chown and install, against a temporary directory as the filesystem root."""
from pathlib import Path
import os
import subprocess
import tempfile
import unittest

HERE = Path(__file__).parent
SCRIPT = HERE / "install-exit-system.sh"
FAKES = {
    # systemctl show -p ActiveState --value -- UNIT: the state named in $STATE_<unit with _>.
    "systemctl": 'u=${@: -1}; v=STATE_${u//[-.]/_}; echo "${!v:-inactive}"',
    "getent": 'case "$1:$2" in passwd:"$TAKEN_UID") echo "postgres:x:$2:$2::/:/bin/false";; '
              'group:pf-sol-exit) echo "pf-sol-exit:x:$OLD_GID:";; group:"$TAKEN_GID") echo "postgres:x:$2:";; esac; exit 0',
    "id": 'echo "$OLD_UID"',
    "groupmod": 'echo "groupmod $*" >> "$LOG"',
    "usermod": 'echo "usermod $*" >> "$LOG"',
    "chgrp": 'echo "chgrp $*" >> "$LOG"',
    "chown": 'echo "chown $*" >> "$LOG"',
}


class Installer:
    def __init__(self, root):
        self.root = Path(root)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for name, body in FAKES.items():
            (self.bin / name).write_text("#!/bin/bash\n" + body + "\n")
            (self.bin / name).chmod(0o755)
        self.log = self.root / "log"
        self.log.touch()

    def run(self, body, **env):
        """Source the installer with its paths under the root, then run `body`."""
        prelude = (f'source {SCRIPT}\n'
                   f'base={self.root}/opt; app=$base/sol-exit; trees=$base/sol-exit.d; '
                   f'previous=$base/sol-exit.previous; history=$base/sol-exit.history; '
                   f'passed={self.root}/passed; svc={self.root}/svc\n'
                   # install -d: create the named directories; owners and modes are not tested here.
                   'install() { local skip=0 a; for a; do if ((skip)); then skip=0; continue; fi; '
                   'case $a in -o|-g|-m) skip=1;; -*) ;; *) mkdir -p -- "$a";; esac; done; }\n')
        environment = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}", LOG=str(self.log),
                           **{"OLD_UID": "999", "OLD_GID": "999", **env})
        return subprocess.run(["bash", "-c", "set -euo pipefail\n" + prelude + body],
                              capture_output=True, text=True, env=environment)

    def tree(self, rev):
        """A live tree at `rev`, installed the new way."""
        result = self.run(f'mkdir -p "$trees"; t=$(mktemp -d "$trees/{rev}.XXXX"); '
                          f'switch_tree "$app" "$previous" "$t"; record_revision "$app" {rev} "$history"')
        assert result.returncode == 0, result.stderr


REV_A, REV_B, REV_C = "a" * 40, "b" * 40, "c" * 40


class UnitStateTests(unittest.TestCase):
    def test_only_inactive_or_failed_units_count_as_stopped(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            for state in ("inactive", "failed"):
                result = installer.run("require_stopped pft-sol-mainnet-withdraw.service",
                                       STATE_pft_sol_mainnet_withdraw_service=state)
                self.assertEqual(result.returncode, 0, state)
            for state in ("active", "activating", "deactivating", "reloading"):
                result = installer.run("require_stopped pft-sol-exit-self-test.service",
                                       STATE_pft_sol_exit_self_test_service=state)
                self.assertNotEqual(result.returncode, 0, state)
                self.assertIn(f"Stop pft-sol-exit-self-test.service before installing: it is {state}.", result.stderr)


class TreeTests(unittest.TestCase):
    def test_switch_keeps_the_previous_tree_and_rollback_puts_it_back(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            installer.tree(REV_A)
            installer.tree(REV_B)
            app, previous = Path(d, "opt/sol-exit"), Path(d, "opt/sol-exit.previous")
            self.assertTrue(app.is_symlink() and previous.is_symlink())
            self.assertEqual((app / "revision").read_text(), REV_B + "\n")
            self.assertEqual((previous / "revision").read_text(), REV_A + "\n")
            result = installer.run('rollback_tree "$app" "$previous"')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((app / "revision").read_text(), REV_A + "\n")
            self.assertEqual((previous / "revision").read_text(), REV_B + "\n")

    def test_failed_copy_or_self_test_leaves_the_live_tree_and_its_revision(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            installer.tree(REV_A)
            app = Path(d, "opt/sol-exit")
            before = (os.readlink(app), (app / "revision").read_text())
            Path(d, "passed").write_text(REV_A + "\n")
            for failing in ("build_tree() { return 1; }; self_test_tree() { :; }",
                            "build_tree() { :; }; self_test_tree() { return 1; }"):
                result = installer.run(failing + f'\ninstall_tree bundle {REV_B}')
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("the installed tree is unchanged", result.stderr)
                self.assertEqual((os.readlink(app), (app / "revision").read_text()), before)
                self.assertEqual(sorted(p.name[:40] for p in Path(d, "opt/sol-exit.d").iterdir()), [REV_A])
            self.assertTrue(Path(d, "passed").exists(), "a refused install keeps the earlier self-test pass")

    def test_revision_is_recorded_only_after_the_switch(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            installer.tree(REV_A)
            # The self-test sees the staged tree without a revision and the live tree still at A.
            result = installer.run(
                'build_tree() { :; }; self_test_tree() { [[ ! -e $1/revision && $(cat "$app/revision") == '
                f'{REV_A} ]]; }}\ninstall_tree bundle {REV_B}; cat "$app/revision"')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip().splitlines()[-1], REV_B)
            self.assertFalse(Path(d, "passed").exists(), "a switch clears the self-test pass")
            self.assertEqual(Path(d, "opt/sol-exit.history").read_text().split(), [REV_A, REV_B])


class DowngradeTests(unittest.TestCase):
    def test_older_revisions_are_refused_without_the_flag(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            installer.tree(REV_A)
            installer.tree(REV_B)
            check = 'check_not_downgrade {rev} {installed} {allow} "$app/ancestry" "$history"'
            refused = installer.run(check.format(rev=REV_A, installed=REV_B, allow=0))
            self.assertNotEqual(refused.returncode, 0)
            self.assertIn("installed here before", refused.stderr)
            self.assertEqual(installer.run(check.format(rev=REV_A, installed=REV_B, allow=1)).returncode, 0)
            self.assertEqual(installer.run(check.format(rev=REV_C, installed=REV_B, allow=0)).returncode, 0)
            # After a rollback to A, B (first installed after A) is an upgrade again.
            self.assertEqual(installer.run(check.format(rev=REV_B, installed=REV_A, allow=0)).returncode, 0)
            # An ancestry file in the installed tree names older commits never installed here.
            Path(d, "opt/sol-exit/ancestry").write_text(f"{REV_B}\n{REV_C}\n")
            refused = installer.run(check.format(rev=REV_C, installed=REV_B, allow=0))
            self.assertIn("is an ancestor of the installed", refused.stderr)


class SelfTestGateTests(unittest.TestCase):
    def test_worker_starts_only_on_the_revision_the_self_test_passed(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            result = installer.run(f'write_gates {d}/units')
            self.assertEqual(result.returncode, 0, result.stderr)
            gate = Path(d, "units/pft-sol-mainnet-withdraw.service.d/self-test-gate.conf").read_text()
            record = Path(d, "units/pft-sol-exit-self-test.service.d/record-pass.conf").read_text()
            self.assertIn("ExecStartPre=+/bin/sh -c 'test -s ", gate)
            self.assertIn('"$$(cat ', gate)
            self.assertIn("/sol-exit/revision)\"'", gate)
            self.assertIn("ExecStartPost=+/bin/sh -c '", record)
            # The shell command systemd runs, with $$ unescaped: no pass, a pass on another revision, a pass here.
            command = gate.split("-c '", 1)[1].rsplit("'", 1)[0].replace("$$", "$")
            installer.tree(REV_B)
            self.assertNotEqual(subprocess.run(["sh", "-c", command]).returncode, 0)
            Path(d, "passed").write_text(REV_A + "\n")
            self.assertNotEqual(subprocess.run(["sh", "-c", command]).returncode, 0)
            Path(d, "passed").write_text(REV_B + "\n")
            self.assertEqual(subprocess.run(["sh", "-c", command]).returncode, 0)


class AccountTests(unittest.TestCase):
    def test_the_service_id_is_its_own(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            self.assertIn("account_id=59900\n", SCRIPT.read_text())
            self.assertEqual(SCRIPT.read_text().count("account_id="), 1, "set in one place")
            for bad in ("999", "70", "1000", "60001", "65534", "0"):
                result = installer.run(f"check_account_id {bad} pf-sol-exit")
                self.assertNotEqual(result.returncode, 0, bad)
                self.assertIn("reserved range", result.stderr)
            taken = installer.run("check_account_id 59900 pf-sol-exit", TAKEN_UID="59900")
            self.assertIn("already belongs to postgres", taken.stderr)
            taken = installer.run("check_account_id 59900 pf-sol-exit", TAKEN_GID="59900")
            self.assertIn("already belongs to postgres", taken.stderr)
            self.assertEqual(installer.run("check_account_id 59900 pf-sol-exit").returncode, 0)

    def test_an_install_at_999_moves_group_then_user_then_its_files(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            svc = Path(d, "svc")
            (svc / "jobs").mkdir(parents=True)
            for name in ("fee-payer.json", "prover", "jobs/journal.db"):
                (svc / name).write_text("x")
                (svc / name).chmod(0o600)
            # find selects by numeric ID: stand in this test user's IDs for 999.
            ids = dict(OLD_UID=str(os.getuid()), OLD_GID=str(os.getgid()))
            # The fake chown changes nothing, so the final check finds the old IDs and refuses.
            left = installer.run('migrate_account_id pf-sol-exit 59900 "$svc"', **ids)
            self.assertNotEqual(left.returncode, 0)
            self.assertIn("still have the old IDs", left.stderr)
            installer.log.write_text("")
            result = installer.run('ids_left() { :; }; migrate_account_id pf-sol-exit 59900 "$svc"', **ids)
            self.assertEqual(result.returncode, 0, result.stderr)
            steps = installer.log.read_text().splitlines()
            self.assertEqual(steps[:2], ["groupmod -g 59900 pf-sol-exit", "usermod -u 59900 pf-sol-exit"])
            self.assertTrue(steps[2].startswith("chgrp -h 59900 "), steps)
            self.assertTrue(steps[3].startswith("chown -h 59900 "), steps)
            for name in ("fee-payer.json", "prover", "jobs/journal.db", "jobs"):
                self.assertIn(str(svc / name), steps[2])
                self.assertIn(str(svc / name), steps[3])
            for name in ("fee-payer.json", "prover", "jobs/journal.db"):
                self.assertEqual((svc / name).stat().st_mode & 0o777, 0o600)

    def test_migration_and_upgrade_wait_for_stopped_units(self):
        text = SCRIPT.read_text()
        body = text.split("then return 0; fi", 1)[1]
        stopped = body.index('require_stopped "$unit" "$test_unit"')
        for later in ("migrate_account_id \"$account\"", "install_tree \"$bundle\"", "rollback_tree \"$app\"",
                      "install -m644 /dev/null /var/log/pf-sol-exit/self-test.log"):
            self.assertLess(stopped, body.index(later), later)


if __name__ == "__main__":
    unittest.main()
