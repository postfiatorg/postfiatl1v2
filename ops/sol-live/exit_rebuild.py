"""Fixed service-account entry point for an offline journal rebuild."""

import os
from pathlib import Path
import sys

if __name__ == "__main__":
    state = Path("/var/lib/pf-sol-exit")
    os.environ["EXIT_FEE_KEY"] = str(state / "fee-payer.json")
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import exit_service

    sys.argv = [__file__, "--state", str(state / "jobs"), "--rebuild"]
    exit_service.main()
