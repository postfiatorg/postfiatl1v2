"""Forward the relay socket to the dedicated Vast prover."""

import json
import os
from datetime import datetime, timezone
import subprocess
from pathlib import Path

state = Path("/mnt/HC_Volume_106212907/stakehub-hyperliquid/sol-live/gpu.json")
local = Path(os.environ["RUNTIME_DIRECTORY"]) / "prover.sock"
number = json.loads(state.read_text())["new_contract"]
instances = json.loads(
    subprocess.check_output(
        [Path.home() / ".local/bin/vastai", "show", "instances", "--raw"]
    )
)
instance = next(
    (i for i in instances if i["id"] == number and i["label"] == "solbridge-live-b"), None
)
if instance is None:
    if datetime.now(timezone.utc) >= datetime(2026, 11, 19, 12, tzinfo=timezone.utc):
        raise SystemExit(0)
    raise RuntimeError("dedicated lease unavailable")

# A direct port avoids the Vast SSH proxy, which took over 100 s to carry a 19 MB witness.
direct = (instance.get("ports") or {}).get("22/tcp")
host, port = (
    (instance["public_ipaddr"], direct[0]["HostPort"])
    if direct
    else (instance["ssh_host"], instance["ssh_port"])
)

os.execvp(
    "ssh",
    [
        "ssh",
        "-T",
        "-C",
        "-oBatchMode=yes",
        "-oServerAliveInterval=10",
        "-oServerAliveCountMax=3",
        "-oStreamLocalBindUnlink=yes",
        "-oExitOnForwardFailure=yes",
        "-L",
        f"{local}:/workspace/sol-ingress/speed-i2/prover.sock",
        "-p",
        str(port),
        "root@" + host,
        "cd /workspace/sol-ingress/speed-i2 || exit 1; "
        "while :; do ./prover-boot.sh; sleep 10; done",
    ],
)
