"""Read-only job API for the Task Node wallet's Solana deposit screen.

Serves the relay's /api/bridge endpoints on loopback :8187 from the mainnet relay's own
job files. It never proves or submits: pft-sol-mainnet-relay.service does that after it
sees the deposit in the vault's custody history. A POST only registers the job record.
"""

import os
from pathlib import Path
import runpy
import secrets
import sys

from live import ROOT, STATE, read, save

BASE = STATE / "mainnet"
URL = os.environ.get("SOL_MAINNET_RPC", "https://api.mainnet-beta.solana.com")
PORT = int(os.environ.get("SOL_WALLET_API_PORT", "8187"))


def main():
    activation = read(BASE / "activation.json")
    module = runpy.run_path(str(Path(__file__).with_name("relay-service.py")))
    cfg = module["configure"](activation, URL, archive=BASE / "archive")
    token = BASE / "wallet-api-token"
    if not token.exists():
        old = os.umask(0o077)
        token.write_text(secrets.token_hex(32) + "\n")
        os.umask(old)
    cfg["http_token_file"] = str(token)
    cfg["health_file"] = str(BASE / "node-health.json")
    relay = module["Relay"](cfg)
    # The custody watcher owns proving and submission.
    relay.run = lambda *args, **kwargs: None
    # What the wallet server reads next to the token.
    save(
        BASE / "wallet-relay.json",
        dict(
            ledger="node",
            http_token_file=token.name,
            deposit_vkey=cfg["deposit_vkey"],
            vault_program=cfg["vault_program"],
            profile_hash=cfg["profile_hash"],
            route_binding=cfg["route_binding"],
            state=str(BASE / "wallet-state"),
            current_epoch=cfg["current_epoch"],
        ),
    )
    (BASE / "wallet-state").mkdir(exist_ok=True)
    sys.path.insert(0, str(ROOT / "tools/pfusdc-sol-relay"))
    from http_api import serve

    serve(relay, PORT)


if __name__ == "__main__":
    main()
