"""Command-line control for the synthetic PFTL Lightning-demo cluster."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys

from .binary_gate import BinaryGateError, verify_binary
from .harness import HarnessError, PftlDevnet


def _print(value) -> None:
    print(json.dumps(value, indent=2, sort_keys=True))


def _required(value: str | None, label: str) -> str:
    if not value:
        raise HarnessError(f"{label} is required")
    return value


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="six-validator synthetic PFTL Lightning/NAVcoin devnet"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    probe = subparsers.add_parser("probe", help="gate an orc2 node binary")
    probe.add_argument(
        "--binary", default=os.environ.get("POSTFIAT_NODE_BIN")
    )
    probe.add_argument(
        "--expected-revision",
        default=os.environ.get("POSTFIAT_NODE_GIT_REV"),
    )
    probe.add_argument(
        "--expected-binary-sha256",
        default=os.environ.get("POSTFIAT_NODE_SHA256"),
    )
    probe.add_argument(
        "--expected-wallet-sdk-sha256",
        default=os.environ.get("POSTFIAT_RPC_SDK_SHA256"),
    )
    probe.add_argument("--output", type=Path)

    initialize = subparsers.add_parser(
        "init", help="create and bootstrap a fresh synthetic cluster"
    )
    initialize.add_argument("--root", required=True, type=Path)
    initialize.add_argument(
        "--binary", default=os.environ.get("POSTFIAT_NODE_BIN")
    )
    initialize.add_argument(
        "--expected-revision",
        default=os.environ.get("POSTFIAT_NODE_GIT_REV"),
    )
    initialize.add_argument(
        "--expected-binary-sha256",
        default=os.environ.get("POSTFIAT_NODE_SHA256"),
    )
    initialize.add_argument(
        "--expected-wallet-sdk-sha256",
        default=os.environ.get("POSTFIAT_RPC_SDK_SHA256"),
    )
    initialize.add_argument(
        "--chain-id", default="local-postfiat-lightning-navcoin-demo"
    )
    initialize.add_argument("--p2p-base-port", type=int, default=29660)
    initialize.add_argument("--rpc-base-port", type=int, default=30660)

    for name in ("up", "down", "status"):
        command = subparsers.add_parser(name)
        command.add_argument("--root", required=True, type=Path)

    snapshot = subparsers.add_parser("snapshot")
    snapshot.add_argument("--root", required=True, type=Path)
    snapshot.add_argument("--escrow-id")
    snapshot.add_argument("--tx-id")

    outage = subparsers.add_parser(
        "one-validator-down",
        help="certify one inert height with one voter absent, then catch it up",
    )
    outage.add_argument("--root", required=True, type=Path)
    outage.add_argument("--effect-key", required=True)

    restart = subparsers.add_parser("restart-proof")
    restart.add_argument("--root", required=True, type=Path)
    restart.add_argument("--effect-key", required=True)

    finality = subparsers.add_parser(
        "finality-proof",
        help="print the hash-checked public certificate bundle for one effect",
    )
    finality.add_argument("--root", required=True, type=Path)
    finality.add_argument("--effect-key", required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "probe":
            report = verify_binary(
                _required(args.binary, "--binary or POSTFIAT_NODE_BIN"),
                expected_revision=_required(
                    args.expected_revision,
                    "--expected-revision or POSTFIAT_NODE_GIT_REV",
                ),
                expected_binary_sha256=args.expected_binary_sha256,
                expected_wallet_sdk_sha256=args.expected_wallet_sdk_sha256,
            )
            if args.output:
                args.output.parent.mkdir(parents=True, exist_ok=True)
                args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
                os.chmod(args.output, 0o644)
            _print(report)
            return 0
        if args.command == "init":
            devnet = PftlDevnet.initialize(
                args.root,
                binary=_required(args.binary, "--binary or POSTFIAT_NODE_BIN"),
                expected_revision=_required(
                    args.expected_revision,
                    "--expected-revision or POSTFIAT_NODE_GIT_REV",
                ),
                expected_binary_sha256=args.expected_binary_sha256,
                expected_wallet_sdk_sha256=args.expected_wallet_sdk_sha256,
                chain_id=args.chain_id,
                p2p_base_port=args.p2p_base_port,
                rpc_base_port=args.rpc_base_port,
            )
            _print(devnet.manifest)
            return 0

        devnet = PftlDevnet(args.root)
        if args.command == "up":
            _print(devnet.start_rpc())
        elif args.command == "down":
            devnet.stop_rpc()
            _print({"ok": True, "root": str(devnet.root), "rpc": "stopped"})
        elif args.command == "status":
            _print(devnet.status_report())
        elif args.command == "snapshot":
            asset = devnet.manifest["asset"]
            _print(
                devnet.consensus_snapshot(
                    asset_id=asset["asset_id"],
                    accounts=[
                        devnet.manifest["roles"]["coordinator"]["address"],
                        devnet.manifest["roles"]["user"]["address"],
                    ],
                    escrow_id=args.escrow_id,
                    tx_id=args.tx_id,
                )
            )
        elif args.command == "one-validator-down":
            _print(
                devnet.advance_height(
                    effect_key=args.effect_key,
                    one_validator_down=True,
                )
            )
        elif args.command == "restart-proof":
            _print(devnet.restart_rpc_proof(effect_key=args.effect_key))
        elif args.command == "finality-proof":
            _print(devnet.public_finality_proof(args.effect_key))
        else:
            raise HarnessError(f"unsupported command: {args.command}")
        return 0
    except (HarnessError, BinaryGateError, OSError, ValueError) as error:
        print(f"lightning-navcoin-pftl-devnet: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
