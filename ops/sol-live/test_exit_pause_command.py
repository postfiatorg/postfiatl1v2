from pathlib import Path
import os
import subprocess
from tempfile import TemporaryDirectory
import unittest


class PauseRunbookTest(unittest.TestCase):
    def test_fresh_login_loads_reviewed_pause_imports(self):
        here = Path(__file__).resolve().parent
        book = (here / "RUNBOOK-LIVE.md").read_text()
        block = book.split("For an emergency Solana pause", 1)[1].split("```bash\n", 1)[1].split("\n```", 1)[0]
        # Stop before RPC creation and signer access. Run the documented import verbatim.
        imports = block.split("c=ExitCluster(", 1)[0] + (
            'from pathlib import Path\nimport exit_setup\n'
            'assert Path(exit_setup.__file__).resolve() == (Path.cwd()/"ops/sol-live/exit_setup.py").resolve()\nPY')
        with TemporaryDirectory() as tmp:
            checkout = Path(tmp) / "pause checkout"
            checkout.mkdir()
            (checkout / "ops").symlink_to(here.parent)
            subprocess.run(["git", "init", "-q", checkout], check=True)
            subprocess.run(["git", "-C", checkout, "add", "ops"], check=True)
            subprocess.run(["git", "-C", checkout, "-c", "user.name=Fixture", "-c",
                            "user.email=fixture@invalid", "commit", "-qm", "Pause fixture"], check=True)
            revision = subprocess.check_output(["git", "-C", checkout, "rev-parse", "HEAD"], text=True).strip()
            env = dict(HOME=os.environ["HOME"], PATH="/usr/bin:/bin", PAUSE_CHECKOUT=str(checkout),
                       RELEASE_COMMIT=revision, PYTHONDONTWRITEBYTECODE="1")
            result = subprocess.run(["bash", "-c", imports], cwd=tmp, env=env, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            env["RELEASE_COMMIT"] = "0" * 40
            self.assertNotEqual(subprocess.run(["bash", "-c", imports], cwd=tmp, env=env,
                                              capture_output=True).returncode, 0)
