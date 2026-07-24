"""Create or load two faucet-funded XRPL Testnet-only wallets."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path

from xrpl.clients import JsonRpcClient
from xrpl.wallet import generate_faucet_wallet

from .xrpl_adapter import TESTNET_URL, XrplTestnet, load_wallet, save_wallet


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime-root", required=True, type=Path)
    args = parser.parse_args()
    private = args.runtime_root / "private/xrpl"
    public = args.runtime_root / "public"
    public.mkdir(parents=True, exist_ok=True)

    client = JsonRpcClient(TESTNET_URL)
    wallets = {}
    for role in ("user", "coordinator"):
        path = private / f"{role}.wallet.json"
        if path.exists():
            wallet = load_wallet(path)
        else:
            wallet = generate_faucet_wallet(client, debug=False)
            save_wallet(path, wallet)
        wallets[role] = wallet

    xrpl = XrplTestnet(public / "xrpl")
    manifest = {
        "schema": "postfiat.xrpl_navcoin.testnet_accounts.v1",
        "network": "XRPL Testnet",
        "endpoint": TESTNET_URL,
        "accounts": {
            role: {
                "classic_address": wallet.classic_address,
                "public_key": wallet.public_key,
                "validated_account": xrpl.account(wallet.classic_address),
            }
            for role, wallet in wallets.items()
        },
        "wallet_secret_location": str(private),
        "wallet_secret_mode": "0600",
        "value_disclaimer": "faucet test XRP only; no mainnet or real money",
    }
    path = public / "xrpl-testnet-accounts.json"
    path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    os.chmod(path, 0o644)
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

