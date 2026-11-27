#!/usr/bin/env python3
"""CLI for Hyperliquid NAVCoin on Solana reserve verification & dust-swap rehearsal."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from .attestor_verifier import NonIssuerAttestation, RegisteredAttestor
from .hypercore_verifier import HyperCorePolicy, verify_hypercore_snapshot
from .solana_swap_bridge import (
    DustSwapRequest,
    SolanaClusterReadiness,
    SwapReadinessConfig,
    rehearse_dust_swap,
)


def _load_bundle(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Hyperliquid NAVCoin on Solana Reserve Attestation Bridge"
    )
    sub = parser.add_subparsers(dest="command", required=True)

    ver_cmd = sub.add_parser("verify-snapshot", help="Verify a HyperCoreReserveReader snapshot receipt")
    ver_cmd.add_argument("--bundle", type=Path, required=True)
    ver_cmd.add_argument("--now-unix", type=int, default=None)

    reh_cmd = sub.add_parser("rehearse-dust-swap", help="Run deterministic Solana NAVCoin dust-swap rehearsal")
    reh_cmd.add_argument("--bundle", type=Path, required=True)
    reh_cmd.add_argument("--now-unix", type=int, default=None)
    reh_cmd.add_argument("--output", type=Path, default=None)

    args = parser.parse_args(argv)
    bundle = _load_bundle(args.bundle)
    policy = HyperCorePolicy.from_dict(bundle["policy"])

    if args.command == "verify-snapshot":
        result = verify_hypercore_snapshot(
            bundle["snapshot_receipt"],
            policy,
            now_unix=args.now_unix,
        )
        print(json.dumps(result.to_dict(), indent=2, sort_keys=True))
        return 0

    if args.command == "rehearse-dust-swap":
        cfg_raw = bundle["config"]
        config = SwapReadinessConfig(**cfg_raw)
        reg_attestors = [RegisteredAttestor(**item) for item in bundle["registered_attestors"]]
        attestations = [NonIssuerAttestation(**item) for item in bundle["attestations"]]
        solana = SolanaClusterReadiness(**bundle["solana_readiness"])
        swap = DustSwapRequest(**bundle["dust_swap"])
        now_unix = int(args.now_unix if args.now_unix is not None else bundle["now_unix"])

        report = rehearse_dust_swap(
            config=config,
            policy=policy,
            snapshot_receipt=bundle["snapshot_receipt"],
            registered_attestors=reg_attestors,
            attestations=attestations,
            solana=solana,
            swap=swap,
            now_unix=now_unix,
        )
        encoded = json.dumps(report, indent=2, sort_keys=True)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(encoded + "\n", encoding="utf-8")
        else:
            print(encoded)
        return 0

    return 1


if __name__ == "__main__":
    raise SystemExit(main())
