"""One-command PFTL transfer helpers backed by the canonical wallet module."""

from __future__ import annotations

import argparse
import json
import os
import sys
from dataclasses import asdict, is_dataclass
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any, Sequence

from .client import PostFiatRpcClient
from .wallet import (
    FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,
    load_wallet,
    request_faucet_pft,
    send_pft,
    send_pft_and_poll_finality,
    unwrap_fastpay,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
PFTL_PRECISION = Decimal("1000000")


def pft_to_atoms(value: str) -> int:
    try:
        amount = Decimal(value.strip())
    except (AttributeError, InvalidOperation) as error:
        raise ValueError("amount must be a decimal PFTL value") from error
    if amount <= 0:
        raise ValueError("amount must be positive")
    atoms_decimal = amount * PFTL_PRECISION
    if atoms_decimal != atoms_decimal.to_integral_value():
        raise ValueError("amount has more than 6 decimal places")
    atoms = int(atoms_decimal)
    if atoms < 1:
        raise ValueError("amount is below PFTL precision")
    return atoms


def default_data_dir() -> Path:
    env_value = os.environ.get("PFTL_DATA_DIR")
    if env_value:
        return Path(env_value).expanduser()
    local_node0 = REPO_ROOT / "devnet" / "local" / "node0"
    if local_node0.exists():
        return local_node0
    return REPO_ROOT / ".postfiat"


def split_paths(raw: str | None) -> list[Path]:
    if not raw:
        return []
    return [Path(part).expanduser() for part in raw.split(",") if part.strip()]


def default_validator_data_dirs(data_dir: Path) -> list[Path]:
    env_dirs = split_paths(os.environ.get("PFTL_VALIDATOR_DATA_DIRS"))
    if env_dirs:
        return env_dirs
    local_root = REPO_ROOT / "devnet" / "local"
    local_nodes = sorted(
        path for path in local_root.glob("node*") if (path / "ledger.json").exists()
    )
    if local_nodes:
        return local_nodes
    return [data_dir]


def path_arg(value: str | None) -> Path | None:
    return Path(value).expanduser() if value else None


def optional_path_arg(value: str | None, env_name: str) -> Path | None:
    return path_arg(value or os.environ.get(env_name))


def result_to_jsonable(value: Any) -> Any:
    if is_dataclass(value):
        return result_to_jsonable(asdict(value))
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, dict):
        return {str(key): result_to_jsonable(child) for key, child in value.items()}
    if isinstance(value, (list, tuple)):
        return [result_to_jsonable(child) for child in value]
    return value


def transfer_report(action: str, amount_atoms: int, result: Any) -> dict[str, Any]:
    body = result_to_jsonable(result)
    return {
        "schema": "postfiat-pftl-transfer-cli-v1",
        "ok": True,
        "action": action,
        "amount_atoms": amount_atoms,
        "amount_pft": amount_atoms / 1_000_000,
        "tx_id": body.get("tx_id") if isinstance(body, dict) else None,
        "result": body,
    }


def run_faucet(args: argparse.Namespace) -> dict[str, Any]:
    data_dir = path_arg(args.data_dir) or default_data_dir()
    amount_atoms = args.amount_atoms if args.amount_atoms is not None else pft_to_atoms(args.amount)
    if amount_atoms < 1:
        raise ValueError("amount-atoms must be positive")
    topology = optional_path_arg(args.topology, "PFTL_TOPOLOGY")
    key_file = optional_path_arg(args.key_file, "PFTL_KEY_FILE")
    proposal_key_file = optional_path_arg(args.proposal_key_file, "PFTL_PROPOSAL_KEY_FILE")
    artifact_dir = optional_path_arg(args.artifact_dir, "PFTL_ARTIFACT_DIR")
    validator_dirs = split_paths(args.validator_data_dirs) or default_validator_data_dirs(data_dir)
    result = request_faucet_pft(
        data_dir=data_dir,
        to_address=args.to,
        amount=amount_atoms,
        validator_data_dirs=validator_dirs,
        work_dir=path_arg(args.work_dir),
        certify_topology=topology,
        certify_key_file=key_file,
        certify_proposal_key_file=proposal_key_file,
        certify_artifact_dir=artifact_dir,
        certify_timeout_ms=args.timeout_ms,
        certify_send_retries=args.send_retries,
        certify_retry_backoff_ms=args.retry_backoff_ms,
    )
    return transfer_report("faucet", amount_atoms, result)


def run_send(args: argparse.Namespace) -> dict[str, Any]:
    endpoint = args.endpoint or os.environ.get("PFTL_RPC_ENDPOINT")
    wallet_dir = args.wallet_dir or os.environ.get("PFTL_WALLET_DIR")
    chain_id = args.chain_id or os.environ.get("PFTL_CHAIN_ID") or "postfiat-local"
    if not endpoint:
        raise ValueError("--endpoint or PFTL_RPC_ENDPOINT is required for send")
    if not wallet_dir:
        raise ValueError("--wallet-dir or PFTL_WALLET_DIR is required for send")
    amount_atoms = args.amount_atoms if args.amount_atoms is not None else pft_to_atoms(args.amount)
    if amount_atoms < 1:
        raise ValueError("amount-atoms must be positive")
    data_dir = path_arg(args.finalize_data_dir)
    validator_dirs: Sequence[Path] | None = None
    if data_dir is not None:
        validator_dirs = split_paths(args.validator_data_dirs) or default_validator_data_dirs(data_dir)
    client = PostFiatRpcClient(endpoint, timeout_seconds=args.timeout_seconds)
    wallet = load_wallet(
        wallet_dir=wallet_dir,
        chain_id=chain_id,
        account_index=args.account_index,
    )
    if data_dir is not None:
        result = send_pft(
            client,
            wallet=wallet,
            to_address=args.to,
            amount=amount_atoms,
            work_dir=path_arg(args.work_dir),
            sequence=args.sequence,
            memo_type=args.memo_type,
            memo_format=args.memo_format,
            memo_data=args.memo_data,
            finalize_data_dir=data_dir,
            validator_data_dirs=validator_dirs,
        )
    elif args.poll_finality:
        result = send_pft_and_poll_finality(
            client,
            wallet=wallet,
            to_address=args.to,
            amount=amount_atoms,
            work_dir=path_arg(args.work_dir),
            sequence=args.sequence,
            memo_type=args.memo_type,
            memo_format=args.memo_format,
            memo_data=args.memo_data,
            poll_timeout_seconds=args.poll_timeout_seconds,
            poll_interval_seconds=args.poll_interval_seconds,
        )
    else:
        result = send_pft(
            client,
            wallet=wallet,
            to_address=args.to,
            amount=amount_atoms,
            work_dir=path_arg(args.work_dir),
            sequence=args.sequence,
            memo_type=args.memo_type,
            memo_format=args.memo_format,
            memo_data=args.memo_data,
        )
    return transfer_report("send", amount_atoms, result)


def run_unwrap_fastpay(args: argparse.Namespace) -> dict[str, Any]:
    endpoint = args.endpoint or os.environ.get("PFTL_RPC_ENDPOINT")
    wallet_dir = args.wallet_dir or os.environ.get("PFTL_WALLET_DIR")
    chain_id = args.chain_id or os.environ.get("PFTL_CHAIN_ID") or "postfiat-local"
    if not endpoint:
        raise ValueError("--endpoint or PFTL_RPC_ENDPOINT is required for unwrap-fastpay")
    if not wallet_dir:
        raise ValueError("--wallet-dir or PFTL_WALLET_DIR is required for unwrap-fastpay")
    amount_atoms = args.amount_atoms if args.amount_atoms is not None else pft_to_atoms(args.amount)
    if amount_atoms < 1:
        raise ValueError("amount-atoms must be positive")
    if args.fee_atoms < 0:
        raise ValueError("--fee-atoms must be nonnegative")
    client = PostFiatRpcClient(endpoint, timeout_seconds=args.timeout_seconds)
    wallet = load_wallet(
        wallet_dir=wallet_dir,
        chain_id=chain_id,
        account_index=args.account_index,
    )
    result = unwrap_fastpay(
        client,
        wallet=wallet,
        object_id=args.object_id,
        amount=amount_atoms,
        fee=args.fee_atoms,
        asset=args.asset,
        work_dir=path_arg(args.work_dir),
        object_limit=args.object_limit,
    )
    return transfer_report("unwrap-fastpay", amount_atoms, result)


def add_common_amount_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--to", required=True, help="Recipient PFTL address")
    parser.add_argument("--amount", default="1", help="Human PFTL amount, converted at 6dp")
    parser.add_argument("--amount-atoms", type=int, help="Raw PFTL atom amount")
    parser.add_argument("--work-dir", help="Directory for generated quote/batch artifacts")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)

    faucet = sub.add_parser("faucet", help="Fund an address from the operator faucet")
    add_common_amount_args(faucet)
    faucet.add_argument("--data-dir", help="Validator data dir; default PFTL_DATA_DIR or devnet/local/node0")
    faucet.add_argument("--validator-data-dirs", help="Comma-separated local validator dirs for apply mode")
    faucet.add_argument("--topology", help="Certified WAN topology; default PFTL_TOPOLOGY")
    faucet.add_argument("--key-file", help="Validator key file for certified mode; default PFTL_KEY_FILE")
    faucet.add_argument(
        "--proposal-key-file",
        help="Optional proposal key file for certified mode; default PFTL_PROPOSAL_KEY_FILE",
    )
    faucet.add_argument("--artifact-dir", help="Certified round artifact dir; default PFTL_ARTIFACT_DIR")
    faucet.add_argument("--timeout-ms", type=int, default=5000)
    faucet.add_argument("--send-retries", type=int, default=0)
    faucet.add_argument("--retry-backoff-ms", type=int, default=250)
    faucet.set_defaults(func=run_faucet)

    send = sub.add_parser("send", help="Quote, sign, and submit from a transparent wallet")
    add_common_amount_args(send)
    send.add_argument("--endpoint", help="RPC endpoint host:port; default PFTL_RPC_ENDPOINT")
    send.add_argument("--wallet-dir", help="Transparent wallet dir; default PFTL_WALLET_DIR")
    send.add_argument("--chain-id", help="Wallet chain id; default PFTL_CHAIN_ID or postfiat-local")
    send.add_argument("--account-index", type=int, default=0)
    send.add_argument("--sequence", type=int)
    send.add_argument("--memo-type")
    send.add_argument("--memo-format")
    send.add_argument("--memo-data")
    send.add_argument("--finalize-data-dir", help="Optional local data dir for mempool-batch finalization (local harness only)")
    send.add_argument("--validator-data-dirs", help="Comma-separated local validator dirs for finalization (local harness only)")
    send.add_argument("--poll-finality", action="store_true", help="Submit to RPC and poll for finality (WAN/testnet mode)")
    send.add_argument("--poll-timeout-seconds", type=float, default=120.0, help="Finality poll timeout in seconds")
    send.add_argument("--poll-interval-seconds", type=float, default=2.0, help="Finality poll interval in seconds")
    send.add_argument("--timeout-seconds", type=float, default=8.0)
    send.set_defaults(func=run_send)

    unwrap = sub.add_parser("unwrap-fastpay", help="Signed FastPay unwrap from owned lane to account lane")
    unwrap.add_argument("--amount", default="1", help="Human PFTL amount to unwrap, converted at 6dp")
    unwrap.add_argument("--amount-atoms", type=int, help="Raw PFTL atom amount to unwrap")
    unwrap.add_argument("--endpoint", help="Wallet-facing RPC endpoint; default PFTL_RPC_ENDPOINT")
    unwrap.add_argument("--wallet-dir", help="Transparent wallet dir; default PFTL_WALLET_DIR")
    unwrap.add_argument("--chain-id", help="Wallet chain id; default PFTL_CHAIN_ID or postfiat-local")
    unwrap.add_argument("--account-index", type=int, default=0)
    unwrap.add_argument("--object-id", help="Optional specific FastPay object id to consume")
    unwrap.add_argument("--asset", default="PFT")
    unwrap.add_argument("--fee-atoms", type=int, default=0)
    unwrap.add_argument("--object-limit", type=int, default=FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT)
    unwrap.add_argument("--work-dir", help="Directory for generated unwrap order/cert artifacts")
    unwrap.add_argument("--timeout-seconds", type=float, default=8.0)
    unwrap.set_defaults(func=run_unwrap_fastpay)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        report = args.func(args)
    except Exception as error:  # noqa: BLE001 - CLI must return a structured failure.
        report = {
            "schema": "postfiat-pftl-transfer-cli-v1",
            "ok": False,
            "action": getattr(args, "action", None),
            "error": f"{type(error).__name__}: {error}",
        }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report.get("ok") is True else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
