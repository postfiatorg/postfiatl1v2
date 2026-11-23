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
            with (
                patch.object(exit_proof, "prover_ssh", return_value=["bash", "-c"]),
                patch.object(exit_proof, "REMOTE", tmp),
                patch.object(exit_proof, "GPU_LOCK", str(path / "gpu.lock")),
            ):
                self.assertEqual(
                    exit_proof.remote_proof("checkpoint", b"{}"),
                    [b"values", b"{}", b"proof"],
                )
                self.assertEqual(list((path / "jobs").iterdir()), [])
                fake.write_text("#!/bin/sh\nexit 1\n")
                with self.assertRaisesRegex(RuntimeError, "unavailable"):
                    exit_proof.remote_proof("checkpoint", b"{}")
                self.assertEqual(list((path / "jobs").iterdir()), [])
