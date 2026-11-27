#!/usr/bin/env python3
"""CLI for Pillar 8 zero-PII W3C VC & ZK-Passport credential adapters."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from .types_c4 import (
    delegated_accept_preimage,
    derive_address_from_pubkey_hex,
    derive_credential_id,
    derive_issuer_id,
    format_did_pftl,
)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Post Fiat Pillar 8 Credential Adapter CLI")
    sub = parser.add_subparsers(dest="command", required=True)

    did_cmd = sub.add_parser("did", help="Format did:pftl:<chain_id>:<account>")
    did_cmd.add_argument("--chain-id", required=True)
    did_cmd.add_argument("--account", default=None)
    did_cmd.add_argument("--pubkey-hex", default=None)

    cid_cmd = sub.add_parser("credential-id", help="Derive deterministic 96-hex credential_id")
    cid_cmd.add_argument("--subject", required=True)
    cid_cmd.add_argument("--issuer-id", required=True)
    cid_cmd.add_argument("--credential-type", required=True)

    iid_cmd = sub.add_parser("issuer-id", help="Derive deterministic 96-hex issuer_id")
    iid_cmd.add_argument("--chain-id", required=True)
    iid_cmd.add_argument("--admin-account", required=True)
    iid_cmd.add_argument("--nonce", type=int, required=True)

    pre_cmd = sub.add_parser("accept-preimage", help="Print exact Contract C4 delegated_accept preimage")
    pre_cmd.add_argument("--chain-id", required=True)
    pre_cmd.add_argument("--genesis-hash", required=True)
    pre_cmd.add_argument("--subject", required=True)
    pre_cmd.add_argument("--issuer-id", required=True)
    pre_cmd.add_argument("--credential-type", required=True)
    pre_cmd.add_argument("--commitment-hash", required=True)
    pre_cmd.add_argument("--expires-at-height", type=int, required=True)
    pre_cmd.add_argument("--nonce", type=int, required=True)
    pre_cmd.add_argument("--output", type=Path, default=None)

    args = parser.parse_args(argv)

    if args.command == "did":
        account = args.account
        if account is None:
            if not args.pubkey_hex:
                raise ValueError("either --account or --pubkey-hex is required")
            account = derive_address_from_pubkey_hex(args.pubkey_hex)
        print(json.dumps({"account": account, "did": format_did_pftl(args.chain_id, account)}, indent=2))
        return 0

    if args.command == "issuer-id":
        print(
            json.dumps(
                {"issuer_id": derive_issuer_id(args.chain_id, args.admin_account, args.nonce)},
                indent=2,
            )
        )
        return 0

    if args.command == "credential-id":
        print(
            json.dumps(
                {
                    "credential_id": derive_credential_id(
                        args.subject, args.issuer_id, args.credential_type
                    )
                },
                indent=2,
            )
        )
        return 0

    if args.command == "accept-preimage":
        raw = delegated_accept_preimage(
            chain_id=args.chain_id,
            genesis_hash=args.genesis_hash,
            subject=args.subject,
            issuer_id=args.issuer_id,
            credential_type=args.credential_type,
            commitment_hash=args.commitment_hash,
            expires_at_height=args.expires_at_height,
            subject_accept_nonce=args.nonce,
        )
        if args.output:
            args.output.write_bytes(raw)
        else:
            print(raw.decode("utf-8"), end="")
        return 0

    return 1


if __name__ == "__main__":
    raise SystemExit(main())
