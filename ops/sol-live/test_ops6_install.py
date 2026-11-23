from pathlib import Path
import os
import tempfile
import unittest

HERE = Path(__file__).parent
SCRIPT = (HERE / "install-exit-system.sh").read_text()
DOC = (HERE / "RUNBOOK-LIVE.md").read_text()


class InstallTests(unittest.TestCase):
    def test_bundle_is_verified_in_private_stage_before_execution(self):
        owner = next(line for line in DOC.splitlines() if line.startswith("sudo ") and "bundle.tar" in line)
        for earlier, later in [("mktemp -d", "bundle.tar"), ("bundle.tar", "sha256sum"),
                               ("sha256sum", "tar -xf"), ("tar -xf", "install-exit-system.sh")]:
            self.assertLess(owner.index(earlier), owner.index(later))
        self.assertNotIn("git -C", SCRIPT)
        self.assertNotIn("repo_git", SCRIPT)

    def test_failed_build_cannot_publish_a_partial_bundle(self):
        import subprocess
        import sys
        with tempfile.TemporaryDirectory() as d:
            kit = Path(d)
            bundle = kit / "release.tar"
            result = subprocess.run(
                [sys.executable, HERE / "build-exit-bundle.py", "--kit", kit, "--output", bundle,
                 "--proof", kit, "--wheels", kit, "--verifier", kit / "verify", "--trust", kit / "trust"],
                capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(bundle.exists())

    def test_sudoers_exact_commands(self):
        grants = SCRIPT.split('<<RULES\n')[1].split('\nRULES')[0].splitlines()
        self.assertEqual(grants, [
            "$operator ALL=(root) NOPASSWD: /usr/bin/systemctl start pft-sol-exit-self-test.service, "
            "/usr/bin/systemctl start $unit, /usr/bin/systemctl stop $unit, /usr/bin/systemctl --no-pager status $unit"])

    def test_reinstall_refusals_precede_proof_invalidation(self):
        invalidate = SCRIPT.index("install -m644 /dev/null /var/log/pf-sol-exit/self-test.log")
        for guard in ('cat "$app/revision"', "systemctl is-active", '$state == disabled', 'check_port\n'):
            self.assertLess(SCRIPT.index(guard), invalidate)
        self.assertLess(invalidate, SCRIPT.index("useradd --system"))

    def test_private_input_is_copied_only_from_approved_open_descriptor(self):
        from exit_install_credential import copy_input
        with tempfile.TemporaryDirectory() as d:
            source = Path(d) / "input"
            source.write_bytes(b"fixture only")
            source.chmod(0o600)
            with tempfile.TemporaryFile() as out:
                copy_input(source, out, os.getuid())
                out.seek(0)
                self.assertEqual(out.read(), b"fixture only")
                for mode, uid in ((0o640, os.getuid()), (0o600, os.getuid()+1)):
                    source.chmod(mode)
                    with self.assertRaises(ValueError):
                        copy_input(source, out, uid)
                alias = Path(d) / "alias"
                alias.symlink_to(source)
                with self.assertRaises(OSError):
                    copy_input(alias, out, os.getuid())
                with self.assertRaises(ValueError):
                    copy_input(Path(d), out, os.getuid())
