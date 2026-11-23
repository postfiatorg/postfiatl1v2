import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import exit_proof


class RemoteProofTest(unittest.TestCase):
    def test_locked_remote_scratch_is_removed_on_success_and_failure(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)
            fake = path / "proof-host"
            fake.write_text("""#!/bin/sh
mkdir -p "$4"
echo -n values > "$4/public-values.bin"
echo -n '{}' > "$4/proof-report.json"
echo -n proof > "$4/proof-calldata.bin"
""")
            fake.chmod(0o700)
            runner = path / "runner"
            script = (Path(__file__).parent / "exit-prover-command.sh").read_text()
            runner.write_text(script.replace("/workspace/b23-withdraw-20261116", tmp)
                               .replace("/workspace/b23-gpu/prover.lock", str(path / "gpu.lock")))
            runner.chmod(0o700)
            transport = path / "ssh"
            transport.write_text(f'#!/bin/sh\nSSH_ORIGINAL_COMMAND="$1" exec "{runner}"\n')
            transport.chmod(0o700)
            with patch.object(exit_proof, "prover_ssh", return_value=[str(transport)]):
                self.assertEqual(
                    exit_proof.remote_proof("checkpoint", b"{}"),
                    [b"values", b"{}", b"proof"],
                )
                self.assertEqual(list((path / "jobs").iterdir()), [])
                fake.write_text("#!/bin/sh\nexit 1\n")
                with self.assertRaisesRegex(RuntimeError, "unavailable"):
                    exit_proof.remote_proof("checkpoint", b"{}")
                self.assertEqual(list((path / "jobs").iterdir()), [])

    def test_archive_marker_requires_local_verifier_acceptance(self):
        import os
        import subprocess
        from exit_sandbox_check import proof
        from exit_setup import CONFIG
        from live import read
        cfg = read(CONFIG)
        parsed = dict(height=cfg["initial_height"], result=cfg["initial_checkpoint"],
                      end=cfg["initial_committee"])
        with (patch.dict(os.environ, EXIT_ANCHOR_WITNESS=str(CONFIG)),
              patch.object(exit_proof, "prove", return_value=(b"native values", bytes(356))),
              patch("exit_precheck.checkpoint_values", return_value=parsed),
              patch("subprocess.run", side_effect=subprocess.CalledProcessError(1, "verifier")) as verify):
            with self.assertRaises(subprocess.CalledProcessError):
                proof(check_anchor=True)
            self.assertEqual(verify.call_args.kwargs["input"], bytes(356) + b"native values")
            self.assertTrue(verify.call_args.kwargs["check"])
