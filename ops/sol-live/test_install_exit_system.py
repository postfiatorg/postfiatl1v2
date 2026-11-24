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
    "systemctl": 'echo "systemctl $* live=$(readlink "${APP:-/none}" 2>/dev/null)" >> "$SYSTEMCTL_LOG"; '
                 'if [[ $1 == show ]]; then u=${@: -1}; v=STATE_${u//[-.]/_}; echo "${!v:-inactive}"; fi',
    "getent": 'case "$1:$2" in passwd:"$TAKEN_UID") echo "postgres:x:$2:$2::/:/bin/false";; '
              'group:pf-sol-exit) echo "pf-sol-exit:x:$OLD_GID:";; group:"$TAKEN_GID") echo "postgres:x:$2:";; esac; exit 0',
    "id": 'echo "$OLD_UID"',
    "groupmod": 'echo "groupmod $*" >> "$LOG"',
    "usermod": 'echo "usermod $*" >> "$LOG"',
    "chgrp": 'echo "chgrp $*" >> "$LOG"',
    "chown": 'echo "chown $*" >> "$LOG"',
    # The commands inside the real build_tree and self_test_tree: a copy, a venv with pip, and the
    # worker's checks run through runuser. Each one fails when its variable is set.
    "cp": '[[ -z ${CP_FAILS:-} ]] || exit 1; exec /bin/cp "$@"',
    "python3": 'if [[ "$1 $2 $3" == "-I -m venv" ]]; then mkdir -p "$4/bin"; '
               'printf "#!/bin/bash\\n[[ -z \\${PIP_FAILS:-} ]]\\n" > "$4/bin/python"; chmod 755 "$4/bin/python"; exit 0; fi; '
               'exec /usr/bin/python3 "$@"',
    "ln": '[[ -z ${LN_FAILS:-} ]] || exit 1; exec /bin/ln "$@"',
    "runuser": 'case "$*" in *verify_local_host*) [[ -z ${HOST_CHECK_FAILS:-} ]] || exit 1;; '
               '*FEE_PAYER*) [[ -z ${PAYER_CHECK_FAILS:-} ]] || exit 1;; esac; exit 0',
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
        self.systemctl_log = self.root / "systemctl.log"
        self.systemctl_log.touch()
        (self.root / "mountinfo").touch()

    def run(self, body, **env):
        """Source the installer with its paths under the root, then run `body`."""
        prelude = (f'source {SCRIPT}\n'
                   f'base={self.root}/opt; app=$base/sol-exit; trees=$base/sol-exit.d; '
                   f'previous=$base/sol-exit.previous; history=$base/sol-exit.history; '
                   f'passed={self.root}/passed; svc={self.root}/svc; unit_dir={self.root}/units; '
                   f'mask_dir={self.root}/run-control; mountinfo={self.root}/mountinfo\n'
                   # install -d: create the named directories; owners and modes are not tested here.
                   'install() { local skip=0 a; for a; do if ((skip)); then skip=0; continue; fi; '
                   'case $a in -o|-g|-m) skip=1;; -*) ;; *) mkdir -p -- "$a";; esac; done; }\n')
        environment = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}", LOG=str(self.log),
                           SYSTEMCTL_LOG=str(self.systemctl_log), APP=str(self.root / "opt/sol-exit"),
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
                f'build_tree() {{ cp -R {bundle(d)}/runtime/. "$2/"; }}; '
                'self_test_tree() { [[ ! -e $1/revision && $(cat "$app/revision") == '
                f'{REV_A} ]]; }}\ninstall_tree bundle {REV_B}; cat "$app/revision"')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip().splitlines()[-1], REV_B)
            self.assertFalse(Path(d, "passed").exists(), "a switch clears the self-test pass")
            self.assertEqual(Path(d, "opt/sol-exit.history").read_text().split(), [REV_A, REV_B])


def bundle(root):
    """A bundle whose runtime holds the two unit files and nothing else."""
    path = Path(root, "bundle")
    (path / "runtime/ops/sol-live").mkdir(parents=True)
    (path / "wheels").mkdir()
    for name in ("pft-sol-mainnet-withdraw.service", "pft-sol-exit-self-test.service"):
        (path / "runtime/ops/sol-live" / name).write_text(f"[Service]\n# {name} from the bundle\n")
    return path


class RealBuildAndCheckTests(unittest.TestCase):
    """The real build_tree and self_test_tree; only the system commands they call are fakes."""

    def install(self, d, **env):
        installer = Installer(d)
        installer.tree(REV_A)
        Path(d, "svc").mkdir(exist_ok=True)
        Path(d, "svc/fee-payer.json").write_text("{}")
        app = Path(d, "opt/sol-exit")
        before = os.readlink(app)
        result = installer.run(f'install_tree {bundle(d)} {REV_B}', **env)
        return result, app, before

    def test_every_failed_step_of_the_build_and_the_check_refuses_the_switch(self):
        for failing in ("CP_FAILS", "PIP_FAILS", "HOST_CHECK_FAILS", "PAYER_CHECK_FAILS"):
            with self.subTest(failing=failing), tempfile.TemporaryDirectory() as d:
                result, app, before = self.install(d, **{failing: "1"})
                self.assertNotEqual(result.returncode, 0, result.stderr)
                self.assertEqual(os.readlink(app), before)
                self.assertEqual((app / "revision").read_text(), REV_A + "\n")
                self.assertIn("the installed tree is unchanged", result.stderr)

    def test_the_same_fakes_let_a_good_tree_through(self):
        with tempfile.TemporaryDirectory() as d:
            result, app, before = self.install(d)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertNotEqual(os.readlink(app), before)
            self.assertEqual((app / "revision").read_text(), REV_B + "\n")


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


def unit_commands(text):
    """The shell commands of the ExecStart* lines of a drop-in, as systemd runs them."""
    return [line.split("-c '", 1)[1].rsplit("'", 1)[0].replace("$$", "$")
            for line in text.splitlines() if line.startswith("ExecStart")]


class SelfTestGateTests(unittest.TestCase):
    def test_the_pass_names_the_tested_tree_and_the_worker_starts_only_on_it(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            result = installer.run(f'write_gates {d}/units')
            self.assertEqual(result.returncode, 0, result.stderr)
            gate, = unit_commands(Path(d, "units/pft-sol-mainnet-withdraw.service.d/self-test-gate.conf").read_text())
            before, after = unit_commands(
                Path(d, "units/pft-sol-exit-self-test.service.d/record-pass.conf").read_text())
            sh = lambda command: subprocess.run(["sh", "-c", command]).returncode
            installer.tree(REV_B)
            tested = os.path.realpath(Path(d, "opt/sol-exit"))
            self.assertNotEqual(sh(gate), 0, "no pass yet")
            self.assertEqual((sh(before), sh(after)), (0, 0))
            self.assertEqual(Path(d, "passed").read_text(), f"{tested} {REV_B}\n")
            self.assertEqual(sh(gate), 0)
            # Another tree with the same revision is not the tree that was tested.
            installer.tree(REV_B)
            self.assertNotEqual(sh(gate), 0)
            # A self-test that saw the link move while it ran records nothing.
            self.assertEqual(sh(before), 0)
            installer.tree(REV_C)
            self.assertNotEqual(sh(after), 0)
            self.assertFalse(Path(d, "passed").exists())


class InstallOrderTests(unittest.TestCase):
    def test_units_and_gate_are_loaded_before_the_switch(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            installer.tree(REV_A)
            old = os.readlink(Path(d, "opt/sol-exit"))
            Path(d, "svc").mkdir()
            result = installer.run(f'install_tree {bundle(d)} {REV_B}')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("from the bundle", Path(d, "units/pft-sol-mainnet-withdraw.service").read_text())
            self.assertTrue(Path(d, "units/pft-sol-mainnet-withdraw.service.d/self-test-gate.conf").exists())
            reloads = [line for line in installer.systemctl_log.read_text().splitlines() if "daemon-reload" in line]
            self.assertEqual(reloads, [f"systemctl daemon-reload live={old}"], "loaded while the old tree was live")
            self.assertNotEqual(os.readlink(Path(d, "opt/sol-exit")), old)

    def test_a_unit_started_after_the_build_stops_the_switch(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            installer.tree(REV_A)
            old = os.readlink(Path(d, "opt/sol-exit"))
            Path(d, "svc").mkdir()
            result = installer.run(f'install_tree {bundle(d)} {REV_B}',
                                   STATE_pft_sol_mainnet_withdraw_service="activating")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Stop pft-sol-mainnet-withdraw.service", result.stderr)
            self.assertEqual(os.readlink(Path(d, "opt/sol-exit")), old)


class MaskTests(unittest.TestCase):
    def test_both_units_are_masked_until_the_run_ends_whatever_the_outcome(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            control = Path(d, "run-control")
            for ending in ("true", "false"):
                result = installer.run(f'trap cleanup EXIT; mask_units; ls "$mask_dir"; readlink "$mask_dir"/*; {ending}')
                self.assertEqual(result.returncode, 0 if ending == "true" else 1, result.stderr)
                self.assertEqual(result.stdout.split(), ["pft-sol-exit-self-test.service",
                                                         "pft-sol-mainnet-withdraw.service", "/dev/null", "/dev/null"])
                self.assertEqual(list(control.iterdir()), [], f"unmasked after {ending}")

    def test_the_masks_cover_the_whole_install_and_the_stop_check_is_repeated(self):
        body = SCRIPT.read_text().split("!= \"$0\" ]]; then return 0; fi", 1)[1]
        masked = body.index("mask_units")
        self.assertLess(body.index("trap cleanup EXIT"), masked)
        for later in ("rollback_install", "migrate_account_id", "install_tree", "create_account"):
            self.assertLess(masked, body.index(later), later)
        # Checked again after masking, right before the ID move, and in switch_in before the switch.
        self.assertGreaterEqual(body.count('require_stopped "$unit" "$test_unit"'), 3)
        move = body.index('if id "$account"')
        self.assertEqual(body.rindex('require_stopped "$unit" "$test_unit"', 0, move),
                         body.rindex("\n", 0, move - 1) + 1)
        switch = SCRIPT.read_text().split("switch_in() {", 1)[1].split("}", 1)[0]
        self.assertLess(switch.index("require_stopped"), switch.index("switch_tree"))


class LegacyAndHousekeepingTests(unittest.TestCase):
    def test_an_interrupted_move_of_todays_tree_is_finished_by_a_rerun(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            app, legacy = Path(d, "opt/sol-exit"), Path(d, f"opt/sol-exit.d/{REV_A}.legacy")
            app.mkdir(parents=True)
            (app / "revision").write_text(REV_A + "\n")
            # Cut short after the move, before the link: the worker's path is missing.
            cut = installer.run("adopt_legacy_tree", LN_FAILS="1")
            self.assertNotEqual(cut.returncode, 0)
            self.assertFalse(app.exists() or app.is_symlink())
            self.assertTrue((legacy / "revision").exists())
            self.assertEqual(Path(d, "opt/sol-exit.history").read_text(), REV_A + "\n")
            again = installer.run("adopt_legacy_tree")
            self.assertEqual(again.returncode, 0, again.stderr)
            self.assertEqual(os.readlink(app), str(legacy))
            # The downgrade check and the rollback target survive: install B, then A is refused and is the previous tree.
            installer.tree(REV_B)
            self.assertEqual(os.readlink(Path(d, "opt/sol-exit.previous")), str(legacy))
            refused = installer.run(f'check_not_downgrade {REV_A} {REV_B} 0 "$app/ancestry" "$history"')
            self.assertIn("installed here before", refused.stderr)
            self.assertEqual(Path(d, "opt/sol-exit.history").read_text().split(), [REV_A, REV_B])

    def test_rollback_puts_back_the_previous_trees_unit_files(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            for rev in (REV_A, REV_B):
                result = installer.run(
                    f'mkdir -p "$trees"; t=$(mktemp -d "$trees/{rev}.XXXX"); mkdir -p "$t/ops/sol-live"; '
                    f'for u in "$unit" "$test_unit"; do echo "{rev[0]}" > "$t/ops/sol-live/$u"; done; '
                    f'install_units "$t"; switch_tree "$app" "$previous" "$t"; record_revision "$app" {rev} "$history"')
                self.assertEqual(result.returncode, 0, result.stderr)
            Path(d, "passed").write_text("x")
            result = installer.run("rollback_install")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((Path(d, "opt/sol-exit") / "revision").read_text(), REV_A + "\n")
            for name in ("pft-sol-mainnet-withdraw.service", "pft-sol-exit-self-test.service"):
                self.assertEqual(Path(d, "units", name).read_text(), "a\n")
            self.assertTrue(Path(d, "units/pft-sol-mainnet-withdraw.service.d/self-test-gate.conf").exists())
            self.assertFalse(Path(d, "passed").exists())

    def test_interrupted_stages_are_cleaned_and_old_trees_pruned_only_on_request(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            for rev in (REV_A, REV_B, REV_C):
                installer.tree(rev)
            stage = Path(d, f"opt/sol-exit.d/{REV_C}.stage")
            stage.mkdir()
            names = lambda: sorted(p.name[:40] for p in Path(d, "opt/sol-exit.d").iterdir())
            result = installer.run("clean_stages")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(stage.exists())
            self.assertEqual(names(), [REV_A, REV_B, REV_C], "installed trees stay without --prune")
            result = installer.run("prune_trees")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(names(), [REV_B, REV_C])
            self.assertIn("--prune) prune=1", SCRIPT.read_text())

    def test_the_id_move_refuses_a_mount_under_the_service_directory(self):
        with tempfile.TemporaryDirectory() as d:
            installer = Installer(d)
            Path(d, "svc").mkdir()
            Path(d, "mountinfo").write_text(f"36 35 98:0 / {d}/svc/jobs rw - ext4 /dev/x rw\n"
                                            f"37 35 98:0 / {d}/svcother rw - ext4 /dev/y rw\n")
            result = installer.run('migrate_account_id pf-sol-exit 59900 "$svc"')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(f"mounted under {d}/svc", result.stderr)
            self.assertEqual(installer.log.read_text(), "", "nothing was moved")
            Path(d, "mountinfo").write_text(f"37 35 98:0 / {d}/svcother rw - ext4 /dev/y rw\n")
            self.assertEqual(installer.run('refuse_submounts "$svc"').returncode, 0)


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
        body = text.split("!= \"$0\" ]]; then return 0; fi", 1)[1]
        stopped = body.index('require_stopped "$unit" "$test_unit"')
        for later in ("migrate_account_id \"$account\"", "install_tree \"$bundle\"", "rollback_install",
                      "install -m644 /dev/null /var/log/pf-sol-exit/self-test.log"):
            self.assertLess(stopped, body.index(later), later)


if __name__ == "__main__":
    unittest.main()
