"""Canonical Python wallet helpers for PostFiat controlled-testnet flows.

The Python layer owns transport and orchestration. Protocol cryptography stays
in the Rust node/SDK binaries so Python callers do not reimplement ML-DSA or
Orchard/Halo2 serialization.
"""

from __future__ import annotations

import hashlib
import json
import os
import secrets
import subprocess
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Sequence

from .client import PostFiatRpcClient, PostFiatWebSocketRpcClient


REPO_ROOT = Path(__file__).resolve().parents[2]
ISSUED_ASSET_ID_DOMAIN = "postfiat.issued_asset_id.v1"
ESCROW_ID_DOMAIN = "postfiat.escrow_id.v1"
NFT_ID_DOMAIN = "postfiat.nft_id.v1"
OFFER_ID_DOMAIN = "postfiat.offer_id.v1"
NATIVE_PFT_ESCROW_ASSET_ID = "PFT"
MAX_ISSUED_ASSET_CODE_BYTES = 32
MAX_ISSUED_ASSET_PRECISION = 18
MAX_NFT_COLLECTION_ID_BYTES = 64
MAX_NFT_METADATA_HASH_BYTES = 64
MAX_NFT_METADATA_HASH_HEX_CHARS = MAX_NFT_METADATA_HASH_BYTES * 2
MAX_NFT_METADATA_URI_BYTES = 256
NFT_FLAG_TRANSFERABLE = 0x0000_0001
NFT_FLAG_ISSUER_BURNABLE = 0x0000_0002
NFT_ALLOWED_FLAGS = NFT_FLAG_TRANSFERABLE | NFT_FLAG_ISSUER_BURNABLE
NFT_COLLECTION_FLAG_TRANSFER_LOCKED = 0x0000_0001
NFT_COLLECTION_FLAG_BURN_LOCKED = 0x0000_0002
NFT_COLLECTION_ALLOWED_FLAGS = (
    NFT_COLLECTION_FLAG_TRANSFER_LOCKED | NFT_COLLECTION_FLAG_BURN_LOCKED
)
TRUSTLINE_STATE_EXPANSION_FEE = 10
FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT = 2048


def _elapsed_ms(started: float) -> float:
    return round((time.monotonic() - started) * 1000.0, 3)


class WalletCommandError(RuntimeError):
    """A local Rust wallet command failed."""


@dataclass(frozen=True)
class TransparentWallet:
    chain_id: str
    account_index: int
    address: str
    public_key_hex: str
    key_file: Path
    backup_file: Path
    key_report: dict[str, Any]


@dataclass(frozen=True)
class OrchardWallet:
    account_index: int
    address_raw_hex: str
    key_file: Path
    view_key_file: Path
    key_report: dict[str, Any]
    view_key_report: dict[str, Any]


@dataclass(frozen=True)
class FaucetPftResult:
    tx_id: str | None
    batch_file: Path
    batch: dict[str, Any]
    receipts_by_validator: tuple[Any, ...]
    certified_round: dict[str, Any] | None = None
    certified_artifact_dir: Path | None = None


@dataclass(frozen=True)
class SendPftResult:
    tx_id: str | None
    quote_response: dict[str, Any]
    signed_transfer: dict[str, Any]
    submit_result: dict[str, Any]
    finalized_batch_file: Path | None
    receipts_by_validator: tuple[Any, ...]
    submit_mode: str = "local_apply"
    pending: bool = False
    finalized: bool = False
    finality_receipt: dict[str, Any] | None = None
    finality_timeout: bool = False


@dataclass(frozen=True)
class FastPayResult:
    operation: str
    owner_public_key_hex: str
    result: Any
    object_id: str | None = None
    objects_snapshot: dict[str, Any] | None = None
    order: dict[str, Any] | None = None
    signed_order: dict[str, Any] | None = None
    certificate: dict[str, Any] | None = None
    votes: tuple[dict[str, Any], ...] = ()
    timings: dict[str, float] | None = None


@dataclass(frozen=True)
class AssetTransactionResult:
    tx_id: str | None
    operation: dict[str, Any]
    quote_response: dict[str, Any]
    signed_asset_transaction: dict[str, Any]
    submit_result: dict[str, Any]
    finalized_batch_file: Path | None
    receipts_by_validator: tuple[Any, ...]
    asset_id: str | None = None


@dataclass(frozen=True)
class EscrowTransactionResult:
    tx_id: str | None
    operation: dict[str, Any]
    quote_response: dict[str, Any]
    signed_escrow_transaction: dict[str, Any]
    submit_result: dict[str, Any]
    finalized_batch_file: Path | None
    receipts_by_validator: tuple[Any, ...]
    escrow_id: str | None = None


@dataclass(frozen=True)
class NftTransactionResult:
    tx_id: str | None
    operation: dict[str, Any]
    quote_response: dict[str, Any]
    signed_nft_transaction: dict[str, Any]
    submit_result: dict[str, Any]
    finalized_batch_file: Path | None
    receipts_by_validator: tuple[Any, ...]
    nft_id: str | None = None


@dataclass(frozen=True)
class OfferTransactionResult:
    tx_id: str | None
    operation: dict[str, Any]
    quote_response: dict[str, Any]
    signed_offer_transaction: dict[str, Any]
    submit_result: dict[str, Any]
    finalized_batch_file: Path | None
    receipts_by_validator: tuple[Any, ...]
    offer_id: str | None = None


@dataclass(frozen=True)
class AtomicSettlementTemplateResult:
    settlement_id: str
    template: dict[str, Any]
    left_operation: dict[str, Any]
    right_operation: dict[str, Any]
    left_escrow_id: str
    right_escrow_id: str


@dataclass(frozen=True)
class AtomicSettlementExecutionResult:
    settlement_id: str
    template: AtomicSettlementTemplateResult
    left_create: EscrowTransactionResult
    right_create: EscrowTransactionResult
    left_finish: EscrowTransactionResult
    right_finish: EscrowTransactionResult
    left_create_escrow_info: dict[str, Any] | None = None
    right_create_escrow_info: dict[str, Any] | None = None


@dataclass(frozen=True)
class ShieldedPftResult:
    tx_id: str | None
    deposit_file: Path
    batch_file: Path | None
    deposit_report: dict[str, Any]
    batch_result: dict[str, Any]
    receipts_by_validator: tuple[Any, ...]


def ensure_wallet_binaries() -> None:
    """Build the Rust wallet binaries used by the Python helpers."""

    _run(["cargo", "build", "-p", "postfiat-node", "-p", "postfiat-rpc-sdk"], json_output=False)


def create_wallet(
    *,
    chain_id: str,
    wallet_dir: str | Path,
    account_index: int = 0,
    master_seed_hex: str | None = None,
    overwrite: bool = False,
) -> TransparentWallet:
    """Create a transparent ML-DSA wallet and backup file."""

    if account_index < 0:
        raise ValueError("account_index must be non-negative")
    wallet_dir = Path(wallet_dir)
    wallet_dir.mkdir(parents=True, exist_ok=True)
    master_seed_hex = master_seed_hex or secrets.token_hex(32)
    key_file = wallet_dir / f"transparent-{account_index}.key.json"
    backup_file = wallet_dir / f"transparent-{account_index}.backup.json"
    args = [
        *_node_bin(),
        "wallet-keygen",
        "--chain-id",
        chain_id,
        "--account-index",
        str(account_index),
        "--key-file",
        str(key_file),
        "--backup-file",
        str(backup_file),
    ]
    if overwrite:
        args.append("--overwrite")
    report = _run_json_with_secret_file(args, "--master-seed-hex-file", master_seed_hex)
    return TransparentWallet(
        chain_id=chain_id,
        account_index=account_index,
        address=_required_str(report, "address"),
        public_key_hex=_required_str(report, "public_key_hex"),
        key_file=key_file,
        backup_file=backup_file,
        key_report=report,
    )


def load_wallet(
    *,
    wallet_dir: str | Path,
    chain_id: str,
    account_index: int = 0,
) -> TransparentWallet:
    """Load a transparent wallet previously created by :func:`create_wallet`."""

    if account_index < 0:
        raise ValueError("account_index must be non-negative")
    wallet_dir = Path(wallet_dir)
    key_file = wallet_dir / f"transparent-{account_index}.key.json"
    backup_file = wallet_dir / f"transparent-{account_index}.backup.json"
    key_report = _read_json(key_file)
    if not backup_file.exists():
        raise WalletCommandError(f"wallet backup file not found: {backup_file}")
    backup_report = _read_json(backup_file)
    backup_chain_id = backup_report.get("chain_id")
    if backup_chain_id != chain_id:
        found = backup_chain_id if isinstance(backup_chain_id, str) else "<missing>"
        raise WalletCommandError(
            f"wallet backup chain_id mismatch for {backup_file}: "
            f"expected {chain_id}, found {found}"
        )
    return TransparentWallet(
        chain_id=chain_id,
        account_index=account_index,
        address=_required_str(key_report, "address"),
        public_key_hex=_required_str(key_report, "public_key_hex"),
        key_file=key_file,
        backup_file=backup_file,
        key_report=key_report,
    )


def create_orchard_wallet(
    *,
    wallet_dir: str | Path,
    account_index: int = 0,
    master_seed_hex: str | None = None,
    overwrite: bool = False,
) -> OrchardWallet:
    """Create an Orchard wallet key plus its full viewing key file."""

    if account_index < 0:
        raise ValueError("account_index must be non-negative")
    wallet_dir = Path(wallet_dir)
    wallet_dir.mkdir(parents=True, exist_ok=True)
    master_seed_hex = master_seed_hex or secrets.token_hex(32)
    key_file = wallet_dir / f"orchard-{account_index}.key.json"
    view_key_file = wallet_dir / f"orchard-{account_index}.view-key.json"
    key_args = [
        *_node_bin(),
        "orchard-keygen",
        "--account-index",
        str(account_index),
        "--key-file",
        str(key_file),
    ]
    if overwrite:
        key_args.append("--overwrite")
    key_report = _run_json_with_secret_file(
        key_args, "--master-seed-hex-file", master_seed_hex
    )
    view_args = [
        *_node_bin(),
        "orchard-view-key-export",
        "--key-file",
        str(key_file),
        "--view-key-file",
        str(view_key_file),
    ]
    if overwrite:
        view_args.append("--overwrite")
    view_key_report = _run_json(view_args)
    return OrchardWallet(
        account_index=account_index,
        address_raw_hex=_required_str(key_report, "address_raw_hex"),
        key_file=key_file,
        view_key_file=view_key_file,
        key_report=key_report,
        view_key_report=view_key_report,
    )


def request_faucet_pft(
    *,
    data_dir: str | Path,
    to_address: str,
    amount: int,
    validator_data_dirs: Sequence[str | Path] | None = None,
    work_dir: str | Path | None = None,
    certify_topology: str | Path | None = None,
    certify_key_file: str | Path | None = None,
    certify_proposal_key_file: str | Path | None = None,
    certify_artifact_dir: str | Path | None = None,
    certify_timeout_ms: int = 2400000,
    certify_send_retries: int = 0,
    certify_retry_backoff_ms: int = 250,
) -> FaucetPftResult:
    """Fund an address from the operator faucet.

    By default this is the local devnet helper: it creates a transparent
    transfer batch and applies it to ``validator_data_dirs``. When
    ``certify_topology`` and ``certify_key_file`` are supplied, it instead
    submits the same batch through the peer-certified finality path.
    """

    if amount < 1:
        raise ValueError("amount must be positive")
    if not to_address:
        raise ValueError("to_address is required")
    data_dir = Path(data_dir)
    work_dir = _work_dir(work_dir)
    batch_file = work_dir / f"faucet-{secrets.token_hex(8)}.batch.json"
    batch = _run_json(
        [
            *_node_bin(),
            "batch-transfer",
            "--data-dir",
            str(data_dir),
            "--to",
            to_address,
            "--amount",
            str(amount),
            "--batch-file",
            str(batch_file),
        ]
    )
    certified_round = None
    certified_artifact = None
    if certify_topology is not None or certify_key_file is not None:
        if certify_topology is None or certify_key_file is None:
            raise ValueError("certify_topology and certify_key_file must be supplied together")
        certified_artifact = (
            Path(certify_artifact_dir)
            if certify_artifact_dir is not None
            else work_dir / f"{batch_file.stem}.certified"
        )
        certify_args = [
            *_node_bin(),
            "transport-peer-certified-batch-round",
            "--data-dir",
            str(data_dir),
            "--topology",
            str(certify_topology),
            "--batch-kind",
            "transparent",
            "--batch-file",
            str(batch_file),
            "--key-file",
            str(certify_key_file),
            "--artifact-dir",
            str(certified_artifact),
            "--timeout-ms",
            str(certify_timeout_ms),
            "--send-retries",
            str(certify_send_retries),
            "--retry-backoff-ms",
            str(certify_retry_backoff_ms),
        ]
        if certify_proposal_key_file is not None:
            certify_args.extend(["--proposal-key-file", str(certify_proposal_key_file)])
        certified_round = _run_json(certify_args)
        return FaucetPftResult(
            tx_id=_first_certified_tx_id(certified_round),
            batch_file=batch_file,
            batch=batch,
            receipts_by_validator=(),
            certified_round=certified_round,
            certified_artifact_dir=certified_artifact,
        )
    receipts = _apply_batch(
        batch_file=batch_file,
        validator_data_dirs=validator_data_dirs or [data_dir],
    )
    tx_id = _first_receipt_tx_id(receipts)
    return FaucetPftResult(
        tx_id=tx_id,
        batch_file=batch_file,
        batch=batch,
        receipts_by_validator=tuple(receipts),
    )


def send_pft(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    to_address: str,
    amount: int,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    memo_type: str | None = None,
    memo_format: str | None = None,
    memo_data: str | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
    use_finality_submit: bool = False,
) -> SendPftResult:
    """Quote, sign, and submit a transparent PFT transfer.

    When ``finalize_data_dir`` is provided, the helper also seals the pending
    mempool transfer into a local batch and applies it to the supplied validator
    data dirs.

    When ``use_finality_submit`` is True, the helper submits via
    ``mempool_submit_signed_transfer_finality`` instead of the basic
    ``mempool_submit_signed_transfer``. This requires the RPC server to
    have ``--allow-mempool-submit-finality`` enabled and the local
    validator to be the proposer for the next block height.
    """

    if amount < 1:
        raise ValueError("amount must be positive")
    if not to_address:
        raise ValueError("to_address is required")
    work_dir = _work_dir(work_dir)
    quote_response = client.transfer_fee_quote_response(
        wallet.address,
        to_address,
        amount,
        sequence=sequence,
        memo_type=memo_type,
        memo_format=memo_format,
        memo_data=memo_data,
        request_id=f"py-wallet-quote-{secrets.token_hex(4)}",
    )
    quote_file = work_dir / f"quote-{secrets.token_hex(8)}.response.json"
    uses_payment_v2 = any(value for value in (memo_type, memo_format, memo_data))
    signed_file = work_dir / (
        f"signed-{secrets.token_hex(8)}.payment-v2.json"
        if uses_payment_v2
        else f"signed-{secrets.token_hex(8)}.transfer.json"
    )
    _write_json(quote_file, quote_response)
    if uses_payment_v2:
        quote_result = quote_response.get("result")
        if not isinstance(quote_result, dict):
            raise ValueError("transfer_fee_quote response missing result object")
        sign_args = [
            *_sdk_bin(),
            "wallet-sign-payment-v2",
            "--backup-file",
            str(wallet.backup_file),
            "--chain-id",
            str(quote_result["chain_id"]),
            "--genesis-hash",
            str(quote_result["genesis_hash"]),
            "--protocol-version",
            str(quote_result["protocol_version"]),
            "--to",
            to_address,
            "--amount",
            str(amount),
            "--fee",
            str(quote_result["minimum_fee"]),
            "--sequence",
            str(quote_result["sequence"]),
            "--output",
            str(signed_file),
        ]
        if memo_type:
            sign_args.extend(["--memo-type", memo_type])
        if memo_format:
            sign_args.extend(["--memo-format", memo_format])
        if memo_data:
            sign_args.extend(["--memo-data", memo_data])
        _run(sign_args, json_output=False)
    else:
        _run(
            [
                *_sdk_bin(),
                "wallet-sign-quote",
                "--backup-file",
                str(wallet.backup_file),
                "--quote-response",
                str(quote_file),
                "--output",
                str(signed_file),
            ],
            json_output=False,
        )
    signed_transfer = _read_json(signed_file)
    if uses_payment_v2 and use_finality_submit:
        submit_result = client.mempool_submit_signed_payment_v2_finality(
            signed_transfer,
            request_id=f"py-wallet-submit-{secrets.token_hex(4)}",
        )
    elif uses_payment_v2:
        submit_result = client.mempool_submit_signed_payment_v2(
            signed_transfer,
            request_id=f"py-wallet-submit-{secrets.token_hex(4)}",
        )
    elif use_finality_submit:
        submit_result = client.mempool_submit_signed_transfer_finality(
            signed_transfer,
            request_id=f"py-wallet-submit-{secrets.token_hex(4)}",
        )
    else:
        submit_result = client.mempool_submit_signed_transfer(
            signed_transfer,
            request_id=f"py-wallet-submit-{secrets.token_hex(4)}",
        )
    tx_id = submit_result.get("tx_id") if isinstance(submit_result.get("tx_id"), str) else None
    finalized_batch_file = None
    receipts: list[Any] = []
    if finalize_data_dir is not None:
        finalized_batch_file = work_dir / f"transparent-finalize-{secrets.token_hex(8)}.batch.json"
        _run_json(
            [
                *_node_bin(),
                "mempool-batch",
                "--data-dir",
                str(finalize_data_dir),
                "--batch-file",
                str(finalized_batch_file),
                "--max-transactions",
                "1",
            ]
        )
        receipts = _apply_batch(
            batch_file=finalized_batch_file,
            validator_data_dirs=validator_data_dirs or [finalize_data_dir],
        )
    submit_mode = "local_apply" if finalize_data_dir is not None else "submit_only"
    return SendPftResult(
        tx_id=tx_id,
        quote_response=quote_response,
        signed_transfer=signed_transfer,
        submit_result=submit_result,
        finalized_batch_file=finalized_batch_file,
        receipts_by_validator=tuple(receipts),
        submit_mode=submit_mode,
        pending=submit_mode == "submit_only" and tx_id is not None,
        finalized=submit_mode == "local_apply",
    )


def send_pft_and_poll_finality(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    to_address: str,
    amount: int,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    memo_type: str | None = None,
    memo_format: str | None = None,
    memo_data: str | None = None,
    poll_timeout_seconds: float = 120.0,
    poll_interval_seconds: float = 2.0,
    use_finality_submit: bool = True,
) -> SendPftResult:
    """Sign, submit a PFT transfer to RPC, then poll for finality.

    This is the WAN/testnet wallet path. It does not use local validator
    data directories or ``apply-batch``. The transaction is submitted to
    the RPC mempool and the helper polls ``tx``/``receipts`` until the
    transaction appears in a certified block or the timeout expires.

    When ``use_finality_submit`` is True (default), the helper submits via
    ``mempool_submit_signed_transfer_finality`` which runs a peer-certified
    mempool round and returns finality evidence inline. This requires the
    RPC server to have ``--allow-mempool-submit-finality`` enabled and the
    connected validator to be the proposer for the next block height.

    When ``use_finality_submit`` is False, the helper submits via the basic
    ``mempool_submit_signed_transfer`` method and polls for finality.
    """
    result = send_pft(
        client,
        wallet=wallet,
        to_address=to_address,
        amount=amount,
        work_dir=work_dir,
        sequence=sequence,
        memo_type=memo_type,
        memo_format=memo_format,
        memo_data=memo_data,
        use_finality_submit=use_finality_submit,
    )
    # When using finality submit, the round itself returns finality evidence.
    # The submit_result contains the finality block and hot finality receipts.
    if use_finality_submit and result.submit_result:
        finality = result.submit_result.get("finality")
        if isinstance(finality, dict):
            block = finality.get("block", {})
            header = block.get("header", {})
            hot_finality = finality.get("local_hot_finality", [])
            tx_id = result.tx_id
            # Check if our tx_id is in the hot finality receipts
            for report in hot_finality if isinstance(hot_finality, list) else []:
                if isinstance(report, dict):
                    receipt = report.get("receipt", {})
                    if receipt.get("tx_id") == tx_id and receipt.get("accepted"):
                        return SendPftResult(
                            tx_id=tx_id,
                            quote_response=result.quote_response,
                            signed_transfer=result.signed_transfer,
                            submit_result=result.submit_result,
                            finalized_batch_file=None,
                            receipts_by_validator=(),
                            submit_mode="submit_and_poll",
                            pending=False,
                            finalized=True,
                            finality_receipt=receipt,
                            finality_timeout=False,
                        )
            # Even if we don't find the specific receipt, the block was certified
            block_height = header.get("height")
            if block_height is not None and block_height > 0:
                return SendPftResult(
                    tx_id=tx_id,
                    quote_response=result.quote_response,
                    signed_transfer=result.signed_transfer,
                    submit_result=result.submit_result,
                    finalized_batch_file=None,
                    receipts_by_validator=(),
                    submit_mode="submit_and_poll",
                    pending=False,
                    finalized=True,
                    finality_receipt={"block_height": block_height, "certified": True},
                    finality_timeout=False,
                )
    if result.tx_id is None:
        return result

    deadline = time.monotonic() + poll_timeout_seconds
    finality_receipt: dict[str, Any] | None = None
    finalized = False

    while time.monotonic() < deadline:
        time.sleep(poll_interval_seconds)
        try:
            tx_info = client.tx(result.tx_id)
            if isinstance(tx_info, dict):
                block_height = tx_info.get("block_height")
                certified = tx_info.get("certified", False)
                if block_height is not None and block_height > 0:
                    finality_receipt = tx_info
                    finalized = certified or bool(block_height)
                    break
        except Exception:
            pass
        try:
            tx_receipts = client.receipts(tx_id=result.tx_id)
            if isinstance(tx_receipts, list) and len(tx_receipts) > 0:
                finality_receipt = tx_receipts[0]
                finalized = True
                break
        except Exception:
            pass

    timed_out = not finalized
    return SendPftResult(
        tx_id=result.tx_id,
        quote_response=result.quote_response,
        signed_transfer=result.signed_transfer,
        submit_result=result.submit_result,
        finalized_batch_file=None,
        receipts_by_validator=(),
        submit_mode="submit_and_poll",
        pending=not finalized,
        finalized=finalized,
        finality_receipt=finality_receipt,
        finality_timeout=timed_out,
    )


def wrap_fastpay(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    amount: int,
    asset: str = "PFT",
    object_limit: int = FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,
    check_capabilities: bool = True,
    refresh_snapshot: bool = True,
    work_dir: str | Path | None = None,
    fee: int = 1,
    validity_blocks: int = 100,
    poll_timeout_seconds: float = 120.0,
    poll_interval_seconds: float = 0.25,
) -> FastPayResult:
    """Fund FastPay through a locally signed, consensus-ordered deposit."""

    if amount < 1:
        raise ValueError("amount must be positive")
    if asset != "PFT":
        raise ValueError("signed FastPay account deposits support only native PFT")
    if fee < 0:
        raise ValueError("fee must be nonnegative")
    if validity_blocks < 1:
        raise ValueError("validity_blocks must be positive")
    if poll_timeout_seconds <= 0 or poll_interval_seconds <= 0:
        raise ValueError("FastPay deposit polling bounds must be positive")
    timings: dict[str, float] = {}
    work_dir = _work_dir(work_dir)
    if check_capabilities:
        started = time.monotonic()
        _require_fastpay_broadcast_rpc(client)
        timings["capabilities_ms"] = _elapsed_ms(started)

    started = time.monotonic()
    capabilities = client.server_capabilities()
    account_result = client.account(wallet.address)
    before_snapshot = client.owned_objects(
        wallet.public_key_hex,
        asset=asset,
        limit=object_limit,
    )
    timings["deposit_preflight_ms"] = _elapsed_ms(started)
    if not (
        capabilities.get("fastpay_bridge_enabled") is True
        and capabilities.get("fastpay_bridge_mode") == "proxy_broadcast_devnet"
        and capabilities.get("fastpay_owned_apply_broadcast_enabled") is True
    ):
        raise WalletCommandError("signed FastPay deposit submission is unavailable")
    chain_id = capabilities.get("chain_id")
    genesis_hash = capabilities.get("genesis_hash")
    protocol_version = capabilities.get("protocol_version")
    block_height = capabilities.get("block_height")
    if chain_id != wallet.chain_id:
        raise WalletCommandError("FastPay deposit chain does not match the wallet")
    if not isinstance(genesis_hash, str) or len(genesis_hash) != 96:
        raise WalletCommandError("FastPay deposit genesis hash is unavailable")
    if not isinstance(protocol_version, int) or protocol_version < 1:
        raise WalletCommandError("FastPay deposit protocol version is unavailable")
    if not isinstance(block_height, int) or block_height < 0:
        raise WalletCommandError("FastPay deposit block height is unavailable")
    account = account_result.get("account", account_result) if isinstance(account_result, dict) else {}
    sequence = account.get("sequence")
    balance = account.get("balance")
    if not isinstance(sequence, int) or sequence < 0 or not isinstance(balance, int):
        raise WalletCommandError("FastPay deposit account state is malformed")
    if balance < amount + fee:
        raise WalletCommandError("insufficient account balance for FastPay deposit and fee")
    try:
        genesis_bytes = list(bytes.fromhex(genesis_hash))
        public_key_bytes = list(bytes.fromhex(wallet.public_key_hex))
    except ValueError as error:
        raise WalletCommandError("FastPay deposit domain or wallet key is not valid hex") from error
    if len(genesis_bytes) != 48 or len(public_key_bytes) != 1952:
        raise WalletCommandError("FastPay deposit domain or wallet key has the wrong length")
    deposit = {
        "domain": {
            "chain_id": chain_id,
            "genesis_hash": genesis_bytes,
            "protocol_version": protocol_version,
        },
        "source_address": wallet.address,
        "source_pubkey": public_key_bytes,
        "sequence": sequence + 1,
        "fee_pft": fee,
        "destination_owner_pubkey": public_key_bytes,
        "asset": asset,
        "amount_atoms": amount,
        "valid_through_height": block_height + validity_blocks,
        "nonce": list(secrets.token_bytes(32)),
    }
    deposit_file = work_dir / f"owned-deposit-{secrets.token_hex(8)}.json"
    signed_file = work_dir / f"signed-owned-deposit-{secrets.token_hex(8)}.json"
    _write_json(deposit_file, deposit)
    started = time.monotonic()
    _run(
        [
            *_sdk_bin(),
            "wallet-sign-owned-deposit",
            "--backup-file",
            str(wallet.backup_file),
            "--deposit-file",
            str(deposit_file),
            "--output",
            str(signed_file),
        ],
        json_output=False,
    )
    transaction = _read_json(signed_file)
    timings["owner_sign_ms"] = _elapsed_ms(started)
    started = time.monotonic()
    submit_result = client.mempool_submit_fastlane_primary_finality(transaction)
    timings["submit_ms"] = _elapsed_ms(started)
    tx_id = submit_result.get("tx_id") if isinstance(submit_result, dict) else None
    if not isinstance(tx_id, str) or not tx_id:
        raise WalletCommandError("signed FastPay deposit submit omitted tx_id")

    started = time.monotonic()
    deadline = time.monotonic() + poll_timeout_seconds
    receipt = None
    while time.monotonic() < deadline:
        receipts = client.receipts(tx_id=tx_id)
        if receipts:
            receipt = receipts[0]
            break
        time.sleep(poll_interval_seconds)
    timings["receipt_ms"] = _elapsed_ms(started)
    if not isinstance(receipt, dict):
        raise WalletCommandError("signed FastPay deposit did not reach an on-chain receipt")
    if receipt.get("accepted") is not True or receipt.get("code") != "owned_deposit_applied":
        raise WalletCommandError(
            f"signed FastPay deposit rejected with receipt code {receipt.get('code', 'unknown')}"
        )

    started = time.monotonic()
    after_snapshot = client.owned_objects(
        wallet.public_key_hex,
        asset=asset,
        limit=object_limit,
    )
    timings["snapshot_ms"] = _elapsed_ms(started)
    before_objects = before_snapshot.get("objects", []) if isinstance(before_snapshot, dict) else []
    after_objects = after_snapshot.get("objects", []) if isinstance(after_snapshot, dict) else []
    before_ids = {
        item.get("id") for item in before_objects if isinstance(item, dict) and item.get("id")
    }
    created = [
        item
        for item in after_objects
        if isinstance(item, dict)
        and item.get("id") not in before_ids
        and item.get("owner_pubkey_hex") == wallet.public_key_hex
        and item.get("asset") == asset
        and item.get("value") == amount
    ]
    if len(created) != 1 or not isinstance(created[0].get("id"), str):
        raise WalletCommandError("signed FastPay deposit did not create one exact owned object")
    object_id = str(created[0]["id"])
    result = {
        "tx_id": tx_id,
        "object_id": object_id,
        "receipt": receipt,
        "submit": submit_result,
    }
    return FastPayResult(
        operation="wrap",
        owner_public_key_hex=wallet.public_key_hex,
        result=result,
        object_id=object_id,
        objects_snapshot=after_snapshot if refresh_snapshot else None,
        timings=timings,
    )


def unwrap_fastpay(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    object_id: str | None = None,
    amount: int | None = None,
    fee: int = 0,
    asset: str = "PFT",
    work_dir: str | Path | None = None,
    owned_objects: Sequence[dict[str, Any]] | None = None,
    validators: Sequence[dict[str, Any]] | None = None,
    object_limit: int = FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,
    check_capabilities: bool = True,
) -> FastPayResult:
    """Unwrap FastPay owned value back into the wallet account lane.

    If ``amount`` is provided, selected input objects are partially unwrapped
    and any remainder after ``fee`` returns as FastPay change. If ``amount`` is
    omitted, the selected value is fully unwrapped for compatibility.
    """

    if amount is not None and amount < 1:
        raise ValueError("amount must be positive")
    if fee < 0:
        raise ValueError("fee must be nonnegative")
    if not asset:
        raise ValueError("asset is required")
    timings: dict[str, float] = {}
    if check_capabilities:
        started = time.monotonic()
        _require_fastpay_broadcast_rpc(client)
        timings["capabilities_ms"] = _elapsed_ms(started)
    work_dir = _work_dir(work_dir)

    objects_snapshot = None
    available_objects = list(owned_objects or [])
    if not available_objects:
        started = time.monotonic()
        objects_snapshot = client.owned_objects(wallet.public_key_hex, asset=asset, limit=object_limit)
        timings["object_lookup_ms"] = _elapsed_ms(started)
        objects = objects_snapshot.get("objects") if isinstance(objects_snapshot, dict) else None
        available_objects = list(objects) if isinstance(objects, list) else []
    else:
        timings["object_lookup_ms"] = 0.0

    selected_inputs: list[dict[str, Any]]
    selected = _find_fastpay_object(available_objects, object_id, asset) if object_id else None
    if selected is not None:
        selected_inputs = [selected]
    else:
        required = (amount if amount is not None else 1) + fee
        selected_inputs = _select_fastpay_inputs(available_objects, required, asset)
    input_value = sum(int(obj["value"]) for obj in selected_inputs)
    unwrap_amount = amount if amount is not None else input_value - fee
    if unwrap_amount < 1:
        raise ValueError("selected FastPay object does not cover unwrap fee")
    if input_value < unwrap_amount + fee:
        raise ValueError("selected FastPay object does not cover amount plus fee")

    started = time.monotonic()
    validator_records = list(validators or _fastpay_validator_records(client))
    timings["validators_ms"] = _elapsed_ms(started)
    if not validator_records:
        raise ValueError("validator list is empty")
    started = time.monotonic()
    recovery_capabilities = client.owned_recovery_capabilities()
    recovery = _fastpay_recovery_window(
        recovery_capabilities,
        wallet=wallet,
        validators=validator_records,
    )
    timings["recovery_capabilities_ms"] = _elapsed_ms(started)
    order = {
        "domain": recovery_capabilities["domain"],
        "recovery": recovery,
        "inputs": [
            {"id": str(obj["id"]), "version": int(obj["version"])}
            for obj in selected_inputs
        ],
        "to_address": wallet.address,
        "amount": unwrap_amount,
        "asset": asset,
        "fee": fee,
        "nonce": time.time_ns(),
        "memos": [],
    }

    started = time.monotonic()
    signed_order = _sign_fastpay_unwrap_order_v3(
        wallet=wallet,
        order=order,
        capabilities=recovery_capabilities,
        work_dir=work_dir,
    )
    timings["owner_sign_ms"] = _elapsed_ms(started)
    quorum = int(recovery_capabilities["quorum"])
    started = time.monotonic()
    votes = _collect_fastpay_unwrap_votes_v3(
        client, signed_order, validator_records, quorum
    )
    timings["vote_collection_ms"] = _elapsed_ms(started)
    if len(votes) < quorum:
        raise WalletCommandError(
            f"FastPay owned-unwrap collected {len(votes)} validator votes, need {quorum}"
        )

    certificate = {
        "order": signed_order["order"],
        "owner_pubkey_hex": signed_order["owner_pubkey_hex"],
        "owner_signature_hex": signed_order["owner_signature_hex"],
        "votes": sorted(votes, key=lambda vote: str(vote["validator_id"])),
    }
    started = time.monotonic()
    apply_result = client.owned_unwrap_apply_v3(
        json.dumps(certificate, separators=(",", ":"))
    )
    timings["apply_ms"] = _elapsed_ms(started)
    started = time.monotonic()
    authenticated_acknowledgements = _verify_fastpay_apply_v3(
        operation="unwrap",
        certificate=certificate,
        apply_response=apply_result,
        capabilities=recovery_capabilities,
        validators=validator_records,
        work_dir=work_dir,
    )
    timings["apply_verification_ms"] = _elapsed_ms(started)
    return FastPayResult(
        operation="unwrap",
        owner_public_key_hex=wallet.public_key_hex,
        result={
            "apply": apply_result,
            "authenticated_acknowledgements": list(authenticated_acknowledgements),
        },
        object_id=str(selected_inputs[0]["id"]),
        objects_snapshot=objects_snapshot,
        order=signed_order["order"],
        signed_order=signed_order,
        certificate=certificate,
        votes=tuple(votes),
        timings=timings,
    )


def send_fastpay(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    recipient_public_key_hex: str,
    amount: int,
    fee: int = 1,
    asset: str = "PFT",
    work_dir: str | Path | None = None,
    owned_objects: Sequence[dict[str, Any]] | None = None,
    validators: Sequence[dict[str, Any]] | None = None,
    object_limit: int = FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,
    check_capabilities: bool = True,
) -> FastPayResult:
    """Send FastPay owned value through the same RPC flow used by the web wallet."""

    if amount < 1:
        raise ValueError("amount must be positive")
    if fee < 0:
        raise ValueError("fee must be nonnegative")
    if not recipient_public_key_hex:
        raise ValueError("recipient_public_key_hex is required")
    if not asset:
        raise ValueError("asset is required")
    timings: dict[str, float] = {}
    if check_capabilities:
        started = time.monotonic()
        _require_fastpay_broadcast_rpc(client)
        timings["capabilities_ms"] = _elapsed_ms(started)
    work_dir = _work_dir(work_dir)

    objects_snapshot = None
    available_objects = list(owned_objects or [])
    if not available_objects:
        started = time.monotonic()
        objects_snapshot = client.owned_objects(
            wallet.public_key_hex, asset=asset, limit=object_limit
        )
        timings["object_lookup_ms"] = _elapsed_ms(started)
        objects = objects_snapshot.get("objects") if isinstance(objects_snapshot, dict) else None
        available_objects = list(objects) if isinstance(objects, list) else []
    else:
        timings["object_lookup_ms"] = 0.0
    input_object = _select_fastpay_input(available_objects, amount + fee, asset)
    input_value = int(input_object["value"])
    change = input_value - amount - fee
    outputs: list[dict[str, Any]] = [
        {
            "owner_pubkey_hex": recipient_public_key_hex,
            "value": amount,
            "asset": asset,
        }
    ]
    if change > 0:
        outputs.append(
            {
                "owner_pubkey_hex": wallet.public_key_hex,
                "value": change,
                "asset": asset,
            }
        )
    started = time.monotonic()
    validator_records = list(validators or _fastpay_validator_records(client))
    timings["validators_ms"] = _elapsed_ms(started)
    if not validator_records:
        raise ValueError("validator list is empty")
    started = time.monotonic()
    recovery_capabilities = client.owned_recovery_capabilities()
    recovery = _fastpay_recovery_window(
        recovery_capabilities,
        wallet=wallet,
        validators=validator_records,
    )
    timings["recovery_capabilities_ms"] = _elapsed_ms(started)
    order = {
        "domain": recovery_capabilities["domain"],
        "recovery": recovery,
        "inputs": [{"id": str(input_object["id"]), "version": int(input_object["version"])}],
        "outputs": outputs,
        "fee": fee,
        "nonce": time.time_ns(),
        "memos": [],
    }

    started = time.monotonic()
    signed_order = _sign_fastpay_order_v3(
        wallet=wallet,
        order=order,
        capabilities=recovery_capabilities,
        work_dir=work_dir,
    )
    timings["owner_sign_ms"] = _elapsed_ms(started)
    signed_order_envelope = signed_order
    quorum = int(recovery_capabilities["quorum"])
    started = time.monotonic()
    votes = _collect_fastpay_votes_v3(client, signed_order_envelope, validator_records, quorum)
    timings["vote_collection_ms"] = _elapsed_ms(started)
    if len(votes) < quorum:
        raise WalletCommandError(
            f"FastPay owned-transfer collected {len(votes)} validator votes, need {quorum}"
        )

    certificate = {
        "order": signed_order["order"],
        "owner_pubkey_hex": signed_order["owner_pubkey_hex"],
        "owner_signature_hex": signed_order["owner_signature_hex"],
        "votes": sorted(votes, key=lambda vote: str(vote["validator_id"])),
    }
    started = time.monotonic()
    apply_result = client.owned_apply_v3(json.dumps(certificate, separators=(",", ":")))
    timings["apply_ms"] = _elapsed_ms(started)
    started = time.monotonic()
    authenticated_acknowledgements = _verify_fastpay_apply_v3(
        operation="transfer",
        certificate=certificate,
        apply_response=apply_result,
        capabilities=recovery_capabilities,
        validators=validator_records,
        work_dir=work_dir,
    )
    timings["apply_verification_ms"] = _elapsed_ms(started)
    return FastPayResult(
        operation="send",
        owner_public_key_hex=wallet.public_key_hex,
        result={
            "apply": apply_result,
            "authenticated_acknowledgements": list(authenticated_acknowledgements),
        },
        object_id=str(input_object["id"]),
        objects_snapshot=objects_snapshot,
        order=signed_order["order"],
        signed_order=signed_order,
        certificate=certificate,
        votes=tuple(votes),
        timings=timings,
    )


def submit_asset_transaction(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    operation: dict[str, Any],
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
    asset_id: str | None = None,
) -> AssetTransactionResult:
    """Quote, sign, and submit a ledger-native issued-asset transaction."""

    if not isinstance(operation, dict) or not operation.get("operation"):
        raise ValueError("operation must be a nonempty asset operation object")
    work_dir = _work_dir(work_dir)
    operation = dict(operation)
    quote_response = client.asset_fee_quote_response(
        wallet.address,
        operation,
        sequence=sequence,
        request_id=f"py-asset-quote-{secrets.token_hex(4)}",
    )
    quote_file = work_dir / f"asset-quote-{secrets.token_hex(8)}.response.json"
    signed_file = work_dir / f"signed-{secrets.token_hex(8)}.asset-transaction.json"
    _write_json(quote_file, quote_response)
    _run(
        [
            *_sdk_bin(),
            "wallet-sign-asset-transaction",
            "--backup-file",
            str(wallet.backup_file),
            "--quote-response",
            str(quote_file),
            "--output",
            str(signed_file),
        ],
        json_output=False,
    )
    signed_asset_transaction = _read_json(signed_file)
    submit_result = client.mempool_submit_signed_asset_transaction(
        signed_asset_transaction,
        request_id=f"py-asset-submit-{secrets.token_hex(4)}",
    )
    tx_id = submit_result.get("tx_id") if isinstance(submit_result.get("tx_id"), str) else None
    finalized_batch_file = None
    receipts: list[Any] = []
    if finalize_data_dir is not None:
        finalized_batch_file = work_dir / f"asset-finalize-{secrets.token_hex(8)}.batch.json"
        _run_json(
            [
                *_node_bin(),
                "mempool-batch",
                "--data-dir",
                str(finalize_data_dir),
                "--batch-file",
                str(finalized_batch_file),
                "--max-transactions",
                "1",
            ]
        )
        receipts = _apply_batch(
            batch_file=finalized_batch_file,
            validator_data_dirs=validator_data_dirs or [finalize_data_dir],
        )
    return AssetTransactionResult(
        tx_id=tx_id,
        operation=operation,
        quote_response=quote_response,
        signed_asset_transaction=signed_asset_transaction,
        submit_result=submit_result,
        finalized_batch_file=finalized_batch_file,
        receipts_by_validator=tuple(receipts),
        asset_id=asset_id,
    )


def create_issued_asset(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    code: str,
    precision: int,
    version: int = 1,
    display_name: str = "",
    max_supply: int | None = None,
    requires_authorization: bool = False,
    freeze_enabled: bool = False,
    clawback_enabled: bool = False,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Create a ledger-native issued asset from the issuer wallet."""

    _validate_asset_code(code)
    if version < 1:
        raise ValueError("version must be positive")
    if not (0 <= precision <= MAX_ISSUED_ASSET_PRECISION):
        raise ValueError(f"precision must be between 0 and {MAX_ISSUED_ASSET_PRECISION}")
    if max_supply is not None and max_supply < 1:
        raise ValueError("max_supply must be positive when provided")
    operation: dict[str, Any] = {
        "operation": "asset_create",
        "issuer": wallet.address,
        "code": code,
        "version": version,
        "precision": precision,
        "display_name": display_name,
        "requires_authorization": bool(requires_authorization),
        "freeze_enabled": bool(freeze_enabled),
        "clawback_enabled": bool(clawback_enabled),
    }
    if max_supply is not None:
        operation["max_supply"] = max_supply
    asset_id = _issued_asset_id(wallet.chain_id, wallet.address, code, version)
    return submit_asset_transaction(
        client,
        wallet=wallet,
        operation=operation,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        asset_id=asset_id,
    )


def create_asset_trustline(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    issuer: str,
    asset_id: str,
    limit: int,
    reserve_paid: int = TRUSTLINE_STATE_EXPANSION_FEE,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Create or update a holder-signed issued-asset trustline."""

    if not issuer:
        raise ValueError("issuer is required")
    _validate_asset_id(asset_id)
    if limit < 1:
        raise ValueError("limit must be positive")
    if reserve_paid < TRUSTLINE_STATE_EXPANSION_FEE:
        raise ValueError(f"reserve_paid must be at least {TRUSTLINE_STATE_EXPANSION_FEE}")
    return submit_asset_transaction(
        client,
        wallet=wallet,
        operation={
            "operation": "trust_set",
            "account": wallet.address,
            "issuer": issuer,
            "asset_id": asset_id,
            "limit": limit,
            "authorized": False,
            "frozen": False,
            "reserve_paid": reserve_paid,
        },
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        asset_id=asset_id,
    )


def _current_asset_line_for_issuer_control(
    client: PostFiatRpcClient,
    *,
    account: str,
    issuer: str,
    asset_id: str,
) -> dict[str, Any]:
    report = client.account_lines(account, issuer=issuer, asset_id=asset_id, limit=2)
    lines = report.get("lines")
    if not isinstance(lines, list):
        raise ValueError("account_lines result missing lines")
    matches = [
        line
        for line in lines
        if isinstance(line, dict)
        and line.get("account") == account
        and line.get("issuer") == issuer
        and line.get("asset_id") == asset_id
    ]
    if len(matches) != 1:
        raise ValueError("issuer control requires exactly one existing trustline")
    line = matches[0]
    for key in ("limit", "reserve_paid"):
        if not isinstance(line.get(key), int) or isinstance(line.get(key), bool):
            raise ValueError(f"account_lines trustline missing integer {key}")
    for key in ("authorized", "frozen"):
        if not isinstance(line.get(key), bool):
            raise ValueError(f"account_lines trustline missing boolean {key}")
    return line


def set_asset_trustline_control(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    account: str,
    asset_id: str,
    authorized: bool | None = None,
    frozen: bool | None = None,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Set issuer-controlled authorization and freeze flags for an existing trustline."""

    if not account:
        raise ValueError("account is required")
    _validate_asset_id(asset_id)
    if authorized is not None and not isinstance(authorized, bool):
        raise ValueError("authorized must be a boolean when provided")
    if frozen is not None and not isinstance(frozen, bool):
        raise ValueError("frozen must be a boolean when provided")
    if authorized is None and frozen is None:
        raise ValueError("authorized or frozen must be provided")
    line = _current_asset_line_for_issuer_control(
        client,
        account=account,
        issuer=wallet.address,
        asset_id=asset_id,
    )
    return submit_asset_transaction(
        client,
        wallet=wallet,
        operation={
            "operation": "trust_set",
            "account": account,
            "issuer": wallet.address,
            "asset_id": asset_id,
            "limit": int(line["limit"]),
            "authorized": bool(line["authorized"] if authorized is None else authorized),
            "frozen": bool(line["frozen"] if frozen is None else frozen),
            "reserve_paid": int(line["reserve_paid"]),
        },
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        asset_id=asset_id,
    )


def authorize_asset_trustline(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    account: str,
    asset_id: str,
    **kwargs: Any,
) -> AssetTransactionResult:
    """Authorize an existing trustline using the issuer wallet."""

    return set_asset_trustline_control(
        client,
        wallet=wallet,
        account=account,
        asset_id=asset_id,
        authorized=True,
        **kwargs,
    )


def revoke_asset_trustline_authorization(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    account: str,
    asset_id: str,
    **kwargs: Any,
) -> AssetTransactionResult:
    """Clear issuer authorization on an existing trustline."""

    return set_asset_trustline_control(
        client,
        wallet=wallet,
        account=account,
        asset_id=asset_id,
        authorized=False,
        **kwargs,
    )


def freeze_asset_trustline(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    account: str,
    asset_id: str,
    **kwargs: Any,
) -> AssetTransactionResult:
    """Freeze an existing trustline using the issuer wallet."""

    return set_asset_trustline_control(
        client,
        wallet=wallet,
        account=account,
        asset_id=asset_id,
        frozen=True,
        **kwargs,
    )


def unfreeze_asset_trustline(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    account: str,
    asset_id: str,
    **kwargs: Any,
) -> AssetTransactionResult:
    """Unfreeze an existing trustline using the issuer wallet."""

    return set_asset_trustline_control(
        client,
        wallet=wallet,
        account=account,
        asset_id=asset_id,
        frozen=False,
        **kwargs,
    )


def send_issued_asset(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    to_address: str,
    issuer: str,
    asset_id: str,
    amount: int,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Send an issued asset from the wallet address to another account."""

    if not to_address:
        raise ValueError("to_address is required")
    if not issuer:
        raise ValueError("issuer is required")
    _validate_asset_id(asset_id)
    if amount < 1:
        raise ValueError("amount must be positive")
    return submit_asset_transaction(
        client,
        wallet=wallet,
        operation={
            "operation": "issued_payment",
            "from": wallet.address,
            "to": to_address,
            "issuer": issuer,
            "asset_id": asset_id,
            "amount": amount,
        },
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        asset_id=asset_id,
    )


def clawback_issued_asset(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    owner: str,
    asset_id: str,
    amount: int,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Claw back an issued-asset balance using the issuer wallet."""

    if not owner:
        raise ValueError("owner is required")
    if owner == wallet.address:
        raise ValueError("owner must differ from issuer wallet address")
    _validate_asset_id(asset_id)
    if amount < 1:
        raise ValueError("amount must be positive")
    return submit_asset_transaction(
        client,
        wallet=wallet,
        operation={
            "operation": "asset_clawback",
            "owner": owner,
            "issuer": wallet.address,
            "asset_id": asset_id,
            "amount": amount,
        },
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        asset_id=asset_id,
    )


def submit_escrow_transaction(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    operation: dict[str, Any],
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    submit_finality: bool = False,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
    escrow_id: str | None = None,
) -> EscrowTransactionResult:
    """Quote, sign, and submit a ledger-native escrow transaction."""

    if not isinstance(operation, dict) or not operation.get("operation"):
        raise ValueError("operation must be a nonempty escrow operation object")
    work_dir = _work_dir(work_dir)
    operation = dict(operation)
    quote_response = client.escrow_fee_quote_response(
        wallet.address,
        operation,
        sequence=sequence,
        request_id=f"py-escrow-quote-{secrets.token_hex(4)}",
    )
    quote_file = work_dir / f"escrow-quote-{secrets.token_hex(8)}.response.json"
    signed_file = work_dir / f"signed-{secrets.token_hex(8)}.escrow-transaction.json"
    _write_json(quote_file, quote_response)
    if wallet.key_file.exists():
        signed_escrow_transaction = _run_json(
            [
                *_node_bin(),
                "wallet-sign-escrow-transaction",
                "--key-file",
                str(wallet.key_file),
                "--quote-file",
                str(quote_file),
            ],
        )
        _write_json(signed_file, signed_escrow_transaction)
    else:
        _run(
            [
                *_sdk_bin(),
                "wallet-sign-escrow-transaction",
                "--backup-file",
                str(wallet.backup_file),
                "--quote-response",
                str(quote_file),
                "--output",
                str(signed_file),
            ],
            json_output=False,
        )
        signed_escrow_transaction = _read_json(signed_file)
    request_id = f"py-escrow-submit-{secrets.token_hex(4)}"
    if submit_finality:
        submit_result = client.mempool_submit_signed_escrow_transaction_finality(
            signed_escrow_transaction,
            request_id=request_id,
        )
    else:
        submit_result = client.mempool_submit_signed_escrow_transaction(
            signed_escrow_transaction,
            request_id=request_id,
        )
    tx_id = submit_result.get("tx_id") if isinstance(submit_result.get("tx_id"), str) else None
    finalized_batch_file = None
    receipts: list[Any] = []
    if finalize_data_dir is not None:
        finalized_batch_file = work_dir / f"escrow-finalize-{secrets.token_hex(8)}.batch.json"
        _run_json(
            [
                *_node_bin(),
                "mempool-batch",
                "--data-dir",
                str(finalize_data_dir),
                "--batch-file",
                str(finalized_batch_file),
                "--max-transactions",
                "1",
            ]
        )
        receipts = _apply_batch(
            batch_file=finalized_batch_file,
            validator_data_dirs=validator_data_dirs or [finalize_data_dir],
        )
    return EscrowTransactionResult(
        tx_id=tx_id,
        operation=operation,
        quote_response=quote_response,
        signed_escrow_transaction=signed_escrow_transaction,
        submit_result=submit_result,
        finalized_batch_file=finalized_batch_file,
        receipts_by_validator=tuple(receipts),
        escrow_id=escrow_id,
    )


def build_atomic_settlement_template(
    client: PostFiatRpcClient,
    *,
    left_wallet: TransparentWallet,
    right_wallet: TransparentWallet,
    left_asset_id: str,
    left_amount: int,
    right_asset_id: str,
    right_amount: int,
    condition: str,
    cancel_after: int,
    finish_after: int = 0,
    left_sequence: int | None = None,
    right_sequence: int | None = None,
) -> AtomicSettlementTemplateResult:
    """Build a reciprocal PFT/issued-asset escrow template without signing legs."""

    _validate_atomic_asset_id(left_asset_id)
    _validate_atomic_asset_id(right_asset_id)
    if (left_asset_id == NATIVE_PFT_ESCROW_ASSET_ID) == (
        right_asset_id == NATIVE_PFT_ESCROW_ASSET_ID
    ):
        raise ValueError("atomic settlement requires exactly one PFT leg and one issued-asset leg")
    if left_wallet.address == right_wallet.address:
        raise ValueError("left_wallet must differ from right_wallet")
    if not condition:
        raise ValueError("condition is required")
    if cancel_after < 1:
        raise ValueError("cancel_after must be positive")
    _validate_escrow_create(
        owner=left_wallet.address,
        recipient=right_wallet.address,
        amount=left_amount,
        condition=condition,
        finish_after=finish_after,
        cancel_after=cancel_after,
    )
    _validate_escrow_create(
        owner=right_wallet.address,
        recipient=left_wallet.address,
        amount=right_amount,
        condition=condition,
        finish_after=finish_after,
        cancel_after=cancel_after,
    )
    if left_sequence is not None and left_sequence < 1:
        raise ValueError("left_sequence must be positive")
    if right_sequence is not None and right_sequence < 1:
        raise ValueError("right_sequence must be positive")

    template = client.atomic_settlement_template(
        left_owner=left_wallet.address,
        left_recipient=right_wallet.address,
        left_asset_id=left_asset_id,
        left_amount=left_amount,
        right_owner=right_wallet.address,
        right_recipient=left_wallet.address,
        right_asset_id=right_asset_id,
        right_amount=right_amount,
        condition=condition,
        finish_after=finish_after,
        cancel_after=cancel_after,
        left_sequence=left_sequence,
        right_sequence=right_sequence,
    )
    settlement_id = template.get("settlement_id")
    left = template.get("left")
    right = template.get("right")
    if not isinstance(settlement_id, str) or not isinstance(left, dict) or not isinstance(
        right, dict
    ):
        raise ValueError("atomic_settlement_template result missing settlement or leg objects")
    left_operation = left.get("operation")
    right_operation = right.get("operation")
    left_escrow_id = left.get("escrow_id")
    right_escrow_id = right.get("escrow_id")
    if not isinstance(left_operation, dict) or not isinstance(right_operation, dict):
        raise ValueError("atomic_settlement_template result missing escrow operations")
    if not isinstance(left_escrow_id, str) or not isinstance(right_escrow_id, str):
        raise ValueError("atomic_settlement_template result missing escrow ids")
    return AtomicSettlementTemplateResult(
        settlement_id=settlement_id,
        template=template,
        left_operation=left_operation,
        right_operation=right_operation,
        left_escrow_id=left_escrow_id,
        right_escrow_id=right_escrow_id,
    )


def execute_atomic_settlement(
    client: PostFiatRpcClient,
    *,
    left_wallet: TransparentWallet,
    right_wallet: TransparentWallet,
    left_asset_id: str,
    left_amount: int,
    right_asset_id: str,
    right_amount: int,
    condition: str,
    cancel_after: int,
    finish_after: int = 0,
    left_sequence: int | None = None,
    right_sequence: int | None = None,
    left_finish_sequence: int | None = None,
    right_finish_sequence: int | None = None,
    submit_finality: bool = False,
    wait_for_create_timeout_seconds: float = 45.0,
    wait_for_create_poll_seconds: float = 1.0,
    work_dir: str | Path | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AtomicSettlementExecutionResult:
    """Execute an ESCROW-009 reciprocal settlement using the existing escrow rails.

    Each wallet signs only the escrow leg it owns. The shared fulfillment is
    revealed only after both create legs have been submitted successfully.
    """

    template = build_atomic_settlement_template(
        client,
        left_wallet=left_wallet,
        right_wallet=right_wallet,
        left_asset_id=left_asset_id,
        left_amount=left_amount,
        right_asset_id=right_asset_id,
        right_amount=right_amount,
        condition=condition,
        cancel_after=cancel_after,
        finish_after=finish_after,
        left_sequence=left_sequence,
        right_sequence=right_sequence,
    )
    left_leg = template.template.get("left")
    right_leg = template.template.get("right")
    if not isinstance(left_leg, dict) or not isinstance(right_leg, dict):
        raise ValueError("atomic_settlement_template result missing leg metadata")
    left_create_sequence = _template_leg_sequence(left_leg, "left")
    right_create_sequence = _template_leg_sequence(right_leg, "right")

    left_create = submit_escrow_transaction(
        client,
        wallet=left_wallet,
        operation=template.left_operation,
        work_dir=work_dir,
        sequence=left_create_sequence,
        submit_finality=submit_finality,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        escrow_id=template.left_escrow_id,
    )
    right_create = submit_escrow_transaction(
        client,
        wallet=right_wallet,
        operation=template.right_operation,
        work_dir=work_dir,
        sequence=right_create_sequence,
        submit_finality=submit_finality,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        escrow_id=template.right_escrow_id,
    )
    _require_escrow_submit_accepted(left_create, "left escrow_create")
    _require_escrow_submit_accepted(right_create, "right escrow_create")
    left_create_escrow_info = _wait_for_open_escrow(
        client,
        template.left_escrow_id,
        "left escrow_create",
        timeout_seconds=wait_for_create_timeout_seconds,
        poll_seconds=wait_for_create_poll_seconds,
    )
    right_create_escrow_info = _wait_for_open_escrow(
        client,
        template.right_escrow_id,
        "right escrow_create",
        timeout_seconds=wait_for_create_timeout_seconds,
        poll_seconds=wait_for_create_poll_seconds,
    )

    left_finish = finish_escrow(
        client,
        recipient_wallet=right_wallet,
        escrow_id=template.left_escrow_id,
        owner=left_wallet.address,
        fulfillment=condition,
        work_dir=work_dir,
        sequence=left_finish_sequence,
        submit_finality=submit_finality,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )
    right_finish = finish_escrow(
        client,
        recipient_wallet=left_wallet,
        escrow_id=template.right_escrow_id,
        owner=right_wallet.address,
        fulfillment=condition,
        work_dir=work_dir,
        sequence=right_finish_sequence,
        submit_finality=submit_finality,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )
    return AtomicSettlementExecutionResult(
        settlement_id=template.settlement_id,
        template=template,
        left_create=left_create,
        right_create=right_create,
        left_finish=left_finish,
        right_finish=right_finish,
        left_create_escrow_info=left_create_escrow_info,
        right_create_escrow_info=right_create_escrow_info,
    )


def create_issued_asset_escrow(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    recipient: str,
    asset_id: str,
    amount: int,
    condition: str = "",
    finish_after: int = 0,
    cancel_after: int = 0,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> EscrowTransactionResult:
    """Create an issued-asset escrow from the owner wallet."""

    _validate_asset_id(asset_id)
    _validate_escrow_create(
        owner=wallet.address,
        recipient=recipient,
        amount=amount,
        condition=condition,
        finish_after=finish_after,
        cancel_after=cancel_after,
    )
    operation: dict[str, Any] = {
        "operation": "escrow_create",
        "owner": wallet.address,
        "recipient": recipient,
        "asset_id": asset_id,
        "amount": amount,
        "condition": condition,
        "finish_after": finish_after,
        "cancel_after": cancel_after,
    }
    result = submit_escrow_transaction(
        client,
        wallet=wallet,
        operation=operation,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )
    quote_result = result.quote_response.get("result")
    if not isinstance(quote_result, dict):
        raise ValueError("escrow_fee_quote response missing result object")
    quote_sequence = quote_result.get("sequence")
    quote_chain_id = quote_result.get("chain_id")
    if not isinstance(quote_sequence, int) or not isinstance(quote_chain_id, str):
        raise ValueError("escrow_fee_quote result missing chain_id or sequence")
    escrow_id = _escrow_id(quote_chain_id, wallet.address, quote_sequence)
    return EscrowTransactionResult(
        tx_id=result.tx_id,
        operation=result.operation,
        quote_response=result.quote_response,
        signed_escrow_transaction=result.signed_escrow_transaction,
        submit_result=result.submit_result,
        finalized_batch_file=result.finalized_batch_file,
        receipts_by_validator=result.receipts_by_validator,
        escrow_id=escrow_id,
    )


def create_pft_escrow(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    recipient: str,
    amount: int,
    condition: str = "",
    finish_after: int = 0,
    cancel_after: int = 0,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> EscrowTransactionResult:
    """Create a native PFT escrow from the owner wallet."""

    _validate_escrow_create(
        owner=wallet.address,
        recipient=recipient,
        amount=amount,
        condition=condition,
        finish_after=finish_after,
        cancel_after=cancel_after,
    )
    operation: dict[str, Any] = {
        "operation": "escrow_create",
        "owner": wallet.address,
        "recipient": recipient,
        "asset_id": NATIVE_PFT_ESCROW_ASSET_ID,
        "amount": amount,
        "condition": condition,
        "finish_after": finish_after,
        "cancel_after": cancel_after,
    }
    result = submit_escrow_transaction(
        client,
        wallet=wallet,
        operation=operation,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )
    quote_result = result.quote_response.get("result")
    if not isinstance(quote_result, dict):
        raise ValueError("escrow_fee_quote response missing result object")
    quote_sequence = quote_result.get("sequence")
    quote_chain_id = quote_result.get("chain_id")
    if not isinstance(quote_sequence, int) or not isinstance(quote_chain_id, str):
        raise ValueError("escrow_fee_quote result missing chain_id or sequence")
    escrow_id = _escrow_id(quote_chain_id, wallet.address, quote_sequence)
    return EscrowTransactionResult(
        tx_id=result.tx_id,
        operation=result.operation,
        quote_response=result.quote_response,
        signed_escrow_transaction=result.signed_escrow_transaction,
        submit_result=result.submit_result,
        finalized_batch_file=result.finalized_batch_file,
        receipts_by_validator=result.receipts_by_validator,
        escrow_id=escrow_id,
    )


def finish_pft_escrow(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    escrow_id: str,
    owner: str,
    fulfillment: str = "",
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    submit_finality: bool = False,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> EscrowTransactionResult:
    """Finish a native PFT escrow from the recipient wallet."""

    _validate_escrow_id(escrow_id)
    if not owner:
        raise ValueError("owner is required")
    if owner == wallet.address:
        raise ValueError("owner must differ from recipient wallet")
    operation = {
        "operation": "escrow_finish",
        "escrow_id": escrow_id,
        "owner": owner,
        "recipient": wallet.address,
        "fulfillment": fulfillment,
    }
    return submit_escrow_transaction(
        client,
        wallet=wallet,
        operation=operation,
        work_dir=work_dir,
        sequence=sequence,
        submit_finality=submit_finality,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        escrow_id=escrow_id,
    )


def cancel_pft_escrow(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    escrow_id: str,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> EscrowTransactionResult:
    """Cancel a native PFT escrow from the owner wallet."""

    _validate_escrow_id(escrow_id)
    operation = {
        "operation": "escrow_cancel",
        "escrow_id": escrow_id,
        "owner": wallet.address,
    }
    return submit_escrow_transaction(
        client,
        wallet=wallet,
        operation=operation,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        escrow_id=escrow_id,
    )


def submit_nft_transaction(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    operation: dict[str, Any],
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
    nft_id: str | None = None,
) -> NftTransactionResult:
    """Quote, sign, and submit a ledger-native NFT transaction."""

    if not isinstance(operation, dict) or not operation.get("operation"):
        raise ValueError("operation must be a nonempty NFT operation object")
    work_dir = _work_dir(work_dir)
    operation = dict(operation)
    quote_response = client.nft_fee_quote_response(
        wallet.address,
        operation,
        sequence=sequence,
        request_id=f"py-nft-quote-{secrets.token_hex(4)}",
    )
    quote_file = work_dir / f"nft-quote-{secrets.token_hex(8)}.response.json"
    signed_file = work_dir / f"signed-{secrets.token_hex(8)}.nft-transaction.json"
    _write_json(quote_file, quote_response)
    _run(
        [
            *_sdk_bin(),
            "wallet-sign-nft-transaction",
            "--backup-file",
            str(wallet.backup_file),
            "--quote-response",
            str(quote_file),
            "--output",
            str(signed_file),
        ],
        json_output=False,
    )
    signed_nft_transaction = _read_json(signed_file)
    submit_result = client.mempool_submit_signed_nft_transaction(
        signed_nft_transaction,
        request_id=f"py-nft-submit-{secrets.token_hex(4)}",
    )
    tx_id = submit_result.get("tx_id") if isinstance(submit_result.get("tx_id"), str) else None
    finalized_batch_file = None
    receipts: list[Any] = []
    if finalize_data_dir is not None:
        finalized_batch_file = work_dir / f"nft-finalize-{secrets.token_hex(8)}.batch.json"
        _run_json(
            [
                *_node_bin(),
                "mempool-batch",
                "--data-dir",
                str(finalize_data_dir),
                "--batch-file",
                str(finalized_batch_file),
                "--max-transactions",
                "1",
            ]
        )
        receipts = _apply_batch(
            batch_file=finalized_batch_file,
            validator_data_dirs=validator_data_dirs or [finalize_data_dir],
        )
    return NftTransactionResult(
        tx_id=tx_id,
        operation=operation,
        quote_response=quote_response,
        signed_nft_transaction=signed_nft_transaction,
        submit_result=submit_result,
        finalized_batch_file=finalized_batch_file,
        receipts_by_validator=tuple(receipts),
        nft_id=nft_id,
    )


def mint_nft(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    collection_id: str,
    serial: int,
    metadata_hash: str,
    owner: str | None = None,
    metadata_uri: str = "",
    flags: int = NFT_FLAG_TRANSFERABLE,
    collection_flags: int = 0,
    issuer_transfer_fee: int = 0,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> NftTransactionResult:
    """Mint a ledger-native NFT from the issuer wallet."""

    owner = wallet.address if owner is None else owner
    _validate_nft_collection_id(collection_id)
    _validate_positive_u64(serial, "serial")
    if not owner:
        raise ValueError("owner is required")
    _validate_nft_metadata_hash(metadata_hash)
    _validate_nft_metadata_uri(metadata_uri)
    _validate_nft_flags(flags)
    _validate_nft_collection_flags(collection_flags)
    _validate_nonnegative_u64(issuer_transfer_fee, "issuer_transfer_fee")
    nft_id = _nft_id(wallet.chain_id, wallet.address, collection_id, serial)
    operation = {
        "operation": "nft_mint",
        "issuer": wallet.address,
        "collection_id": collection_id,
        "serial": serial,
        "owner": owner,
        "metadata_hash": metadata_hash,
        "metadata_uri": metadata_uri,
        "flags": flags,
    }
    if collection_flags:
        operation["collection_flags"] = collection_flags
    if issuer_transfer_fee:
        operation["issuer_transfer_fee"] = issuer_transfer_fee
    return submit_nft_transaction(
        client,
        wallet=wallet,
        operation=operation,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        nft_id=nft_id,
    )


def transfer_nft(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    nft_id: str,
    to_address: str,
    from_address: str | None = None,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> NftTransactionResult:
    """Transfer an NFT from the wallet address to another account."""

    _validate_nft_id(nft_id)
    from_address = wallet.address if from_address is None else from_address
    if from_address != wallet.address:
        raise ValueError("from_address must match wallet address")
    if not to_address:
        raise ValueError("to_address is required")
    if to_address == wallet.address:
        raise ValueError("to_address must differ from wallet address")
    operation = {
        "operation": "nft_transfer",
        "nft_id": nft_id,
        "from": wallet.address,
        "to": to_address,
    }
    return submit_nft_transaction(
        client,
        wallet=wallet,
        operation=operation,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        nft_id=nft_id,
    )


def burn_nft(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    nft_id: str,
    owner: str | None = None,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> NftTransactionResult:
    """Burn an NFT owned by the wallet address."""

    _validate_nft_id(nft_id)
    owner = wallet.address if owner is None else owner
    if owner != wallet.address:
        raise ValueError("owner must match wallet address")
    operation = {
        "operation": "nft_burn",
        "nft_id": nft_id,
        "owner": wallet.address,
    }
    return submit_nft_transaction(
        client,
        wallet=wallet,
        operation=operation,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        nft_id=nft_id,
    )


def submit_offer_transaction(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    operation: dict[str, Any],
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
    offer_id: str | None = None,
) -> OfferTransactionResult:
    """Quote, sign, and submit a ledger-native DEX offer transaction."""

    if not isinstance(operation, dict) or not operation.get("operation"):
        raise ValueError("operation must be a nonempty offer operation object")
    work_dir = _work_dir(work_dir)
    operation = dict(operation)
    quote_response = client.offer_fee_quote_response(
        wallet.address,
        operation,
        sequence=sequence,
        request_id=f"py-offer-quote-{secrets.token_hex(4)}",
    )
    quote_file = work_dir / f"offer-quote-{secrets.token_hex(8)}.response.json"
    signed_file = work_dir / f"signed-{secrets.token_hex(8)}.offer-transaction.json"
    _write_json(quote_file, quote_response)
    _run(
        [
            *_sdk_bin(),
            "wallet-sign-offer-transaction",
            "--backup-file",
            str(wallet.backup_file),
            "--quote-response",
            str(quote_file),
            "--output",
            str(signed_file),
        ],
        json_output=False,
    )
    signed_offer_transaction = _read_json(signed_file)
    submit_result = client.mempool_submit_signed_offer_transaction(
        signed_offer_transaction,
        request_id=f"py-offer-submit-{secrets.token_hex(4)}",
    )
    tx_id = submit_result.get("tx_id") if isinstance(submit_result.get("tx_id"), str) else None
    finalized_batch_file = None
    receipts: list[Any] = []
    if finalize_data_dir is not None:
        finalized_batch_file = work_dir / f"offer-finalize-{secrets.token_hex(8)}.batch.json"
        _run_json(
            [
                *_node_bin(),
                "mempool-batch",
                "--data-dir",
                str(finalize_data_dir),
                "--batch-file",
                str(finalized_batch_file),
                "--max-transactions",
                "1",
            ]
        )
        receipts = _apply_batch(
            batch_file=finalized_batch_file,
            validator_data_dirs=validator_data_dirs or [finalize_data_dir],
        )
    return OfferTransactionResult(
        tx_id=tx_id,
        operation=operation,
        quote_response=quote_response,
        signed_offer_transaction=signed_offer_transaction,
        submit_result=submit_result,
        finalized_batch_file=finalized_batch_file,
        receipts_by_validator=tuple(receipts),
        offer_id=offer_id,
    )


def create_offer(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    taker_gets_asset_id: str,
    taker_gets_amount: int,
    taker_pays_asset_id: str,
    taker_pays_amount: int,
    expiration_height: int = 0,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> OfferTransactionResult:
    """Create a ledger-native DEX offer owned by the wallet address."""

    _validate_dex_asset_id(taker_gets_asset_id)
    _validate_dex_asset_id(taker_pays_asset_id)
    if taker_gets_asset_id == taker_pays_asset_id:
        raise ValueError("DEX asset ids must differ")
    _validate_positive_u64(taker_gets_amount, "taker_gets_amount")
    _validate_positive_u64(taker_pays_amount, "taker_pays_amount")
    if not isinstance(expiration_height, int) or isinstance(expiration_height, bool):
        raise ValueError("expiration_height must be an integer")
    if expiration_height < 0 or expiration_height > 2**64 - 1:
        raise ValueError("expiration_height must be a non-negative u64")
    operation = {
        "operation": "offer_create",
        "owner": wallet.address,
        "taker_gets_asset_id": taker_gets_asset_id,
        "taker_gets_amount": taker_gets_amount,
        "taker_pays_asset_id": taker_pays_asset_id,
        "taker_pays_amount": taker_pays_amount,
        "expiration_height": expiration_height,
    }
    result = submit_offer_transaction(
        client,
        wallet=wallet,
        operation=operation,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )
    quote_result = result.quote_response.get("result")
    offer_sequence = (
        quote_result.get("sequence") if isinstance(quote_result, dict) else sequence
    )
    offer_id = (
        _offer_id(wallet.chain_id, wallet.address, offer_sequence)
        if isinstance(offer_sequence, int) and not isinstance(offer_sequence, bool)
        else None
    )
    return OfferTransactionResult(
        tx_id=result.tx_id,
        operation=result.operation,
        quote_response=result.quote_response,
        signed_offer_transaction=result.signed_offer_transaction,
        submit_result=result.submit_result,
        finalized_batch_file=result.finalized_batch_file,
        receipts_by_validator=result.receipts_by_validator,
        offer_id=offer_id,
    )


def cancel_offer(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    offer_id: str,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> OfferTransactionResult:
    """Cancel an open DEX offer owned by the wallet address."""

    _validate_offer_id(offer_id)
    operation = {
        "operation": "offer_cancel",
        "offer_id": offer_id,
        "owner": wallet.address,
    }
    return submit_offer_transaction(
        client,
        wallet=wallet,
        operation=operation,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
        offer_id=offer_id,
    )


def send_payment(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    destination: str,
    amount: int,
    memo_type: str | None = None,
    memo_format: str | None = None,
    memo_data: str | None = None,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> SendPftResult:
    """Send native PFT using xrpl-py-style Payment field names."""

    return send_pft(
        client,
        wallet=wallet,
        to_address=destination,
        amount=amount,
        memo_type=memo_type,
        memo_format=memo_format,
        memo_data=memo_data,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def mint_token(
    client: PostFiatRpcClient,
    *,
    issuer_wallet: TransparentWallet,
    currency: str,
    precision: int = 0,
    version: int = 1,
    display_name: str = "",
    max_supply: int | None = None,
    requires_authorization: bool = False,
    freeze_enabled: bool = False,
    clawback_enabled: bool = False,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Create an issued-token definition using the issuer wallet.

    XRPL represents issued currencies through issuer trustlines and payments;
    this helper names the asset-definition step as token minting for wallet UX.
    """

    return create_issued_asset(
        client,
        wallet=issuer_wallet,
        code=currency,
        precision=precision,
        version=version,
        display_name=display_name,
        max_supply=max_supply,
        requires_authorization=requires_authorization,
        freeze_enabled=freeze_enabled,
        clawback_enabled=clawback_enabled,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def set_trustline(
    client: PostFiatRpcClient,
    *,
    holder_wallet: TransparentWallet,
    issuer: str,
    asset_id: str,
    limit: int,
    reserve_paid: int = TRUSTLINE_STATE_EXPANSION_FEE,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Create or update a holder trustline using TrustSet-style naming."""

    return create_asset_trustline(
        client,
        wallet=holder_wallet,
        issuer=issuer,
        asset_id=asset_id,
        limit=limit,
        reserve_paid=reserve_paid,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def authorize_trustline(
    client: PostFiatRpcClient,
    *,
    issuer_wallet: TransparentWallet,
    account: str,
    asset_id: str,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Authorize an existing trustline with issuer-wallet naming."""

    return authorize_asset_trustline(
        client,
        wallet=issuer_wallet,
        account=account,
        asset_id=asset_id,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def revoke_trustline_authorization(
    client: PostFiatRpcClient,
    *,
    issuer_wallet: TransparentWallet,
    account: str,
    asset_id: str,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Clear issuer authorization on an existing trustline."""

    return revoke_asset_trustline_authorization(
        client,
        wallet=issuer_wallet,
        account=account,
        asset_id=asset_id,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def freeze_trustline(
    client: PostFiatRpcClient,
    *,
    issuer_wallet: TransparentWallet,
    account: str,
    asset_id: str,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Freeze an existing trustline with issuer-wallet naming."""

    return freeze_asset_trustline(
        client,
        wallet=issuer_wallet,
        account=account,
        asset_id=asset_id,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def unfreeze_trustline(
    client: PostFiatRpcClient,
    *,
    issuer_wallet: TransparentWallet,
    account: str,
    asset_id: str,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Unfreeze an existing trustline with issuer-wallet naming."""

    return unfreeze_asset_trustline(
        client,
        wallet=issuer_wallet,
        account=account,
        asset_id=asset_id,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def send_token(
    client: PostFiatRpcClient,
    *,
    sender_wallet: TransparentWallet,
    destination: str,
    issuer: str,
    asset_id: str,
    value: int,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Send an issued token using Payment-style destination/value naming."""

    return send_issued_asset(
        client,
        wallet=sender_wallet,
        to_address=destination,
        issuer=issuer,
        asset_id=asset_id,
        amount=value,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def clawback_token(
    client: PostFiatRpcClient,
    *,
    issuer_wallet: TransparentWallet,
    owner: str,
    asset_id: str,
    value: int,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> AssetTransactionResult:
    """Claw back an issued-token balance using issuer-wallet naming."""

    return clawback_issued_asset(
        client,
        wallet=issuer_wallet,
        owner=owner,
        asset_id=asset_id,
        amount=value,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def create_escrow(
    client: PostFiatRpcClient,
    *,
    owner_wallet: TransparentWallet,
    destination: str,
    amount: int,
    asset_id: str = NATIVE_PFT_ESCROW_ASSET_ID,
    condition: str = "",
    finish_after: int = 0,
    cancel_after: int = 0,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> EscrowTransactionResult:
    """Create a PFT or issued-token escrow with EscrowCreate-style naming."""

    if asset_id == NATIVE_PFT_ESCROW_ASSET_ID:
        return create_pft_escrow(
            client,
            wallet=owner_wallet,
            recipient=destination,
            amount=amount,
            condition=condition,
            finish_after=finish_after,
            cancel_after=cancel_after,
            work_dir=work_dir,
            sequence=sequence,
            finalize_data_dir=finalize_data_dir,
            validator_data_dirs=validator_data_dirs,
        )
    return create_issued_asset_escrow(
        client,
        wallet=owner_wallet,
        recipient=destination,
        asset_id=asset_id,
        amount=amount,
        condition=condition,
        finish_after=finish_after,
        cancel_after=cancel_after,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def finish_escrow(
    client: PostFiatRpcClient,
    *,
    recipient_wallet: TransparentWallet,
    escrow_id: str,
    owner: str,
    fulfillment: str = "",
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    submit_finality: bool = False,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> EscrowTransactionResult:
    """Finish an escrow from the recipient wallet."""

    return finish_pft_escrow(
        client,
        wallet=recipient_wallet,
        escrow_id=escrow_id,
        owner=owner,
        fulfillment=fulfillment,
        work_dir=work_dir,
        sequence=sequence,
        submit_finality=submit_finality,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def cancel_escrow(
    client: PostFiatRpcClient,
    *,
    owner_wallet: TransparentWallet,
    escrow_id: str,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> EscrowTransactionResult:
    """Cancel an escrow from the owner wallet."""

    return cancel_pft_escrow(
        client,
        wallet=owner_wallet,
        escrow_id=escrow_id,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def mint_non_fungible_token(
    client: PostFiatRpcClient,
    *,
    issuer_wallet: TransparentWallet,
    collection_id: str,
    serial: int,
    metadata_hash: str,
    owner: str | None = None,
    metadata_uri: str = "",
    flags: int = NFT_FLAG_TRANSFERABLE,
    collection_flags: int = 0,
    issuer_transfer_fee: int = 0,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> NftTransactionResult:
    """Mint an NFT with NFTokenMint-style naming."""

    return mint_nft(
        client,
        wallet=issuer_wallet,
        collection_id=collection_id,
        serial=serial,
        metadata_hash=metadata_hash,
        owner=owner,
        metadata_uri=metadata_uri,
        flags=flags,
        collection_flags=collection_flags,
        issuer_transfer_fee=issuer_transfer_fee,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def transfer_non_fungible_token(
    client: PostFiatRpcClient,
    *,
    owner_wallet: TransparentWallet,
    nft_id: str,
    destination: str,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> NftTransactionResult:
    """Transfer an NFT using destination naming."""

    return transfer_nft(
        client,
        wallet=owner_wallet,
        nft_id=nft_id,
        to_address=destination,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def burn_non_fungible_token(
    client: PostFiatRpcClient,
    *,
    owner_wallet: TransparentWallet,
    nft_id: str,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> NftTransactionResult:
    """Burn an NFT from the owner wallet."""

    return burn_nft(
        client,
        wallet=owner_wallet,
        nft_id=nft_id,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def place_offer(
    client: PostFiatRpcClient,
    *,
    wallet: TransparentWallet,
    taker_gets_asset_id: str,
    taker_gets_value: int,
    taker_pays_asset_id: str,
    taker_pays_value: int,
    expiration_height: int = 0,
    work_dir: str | Path | None = None,
    sequence: int | None = None,
    finalize_data_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
) -> OfferTransactionResult:
    """Create a DEX offer with OfferCreate-style value names."""

    return create_offer(
        client,
        wallet=wallet,
        taker_gets_asset_id=taker_gets_asset_id,
        taker_gets_amount=taker_gets_value,
        taker_pays_asset_id=taker_pays_asset_id,
        taker_pays_amount=taker_pays_value,
        expiration_height=expiration_height,
        work_dir=work_dir,
        sequence=sequence,
        finalize_data_dir=finalize_data_dir,
        validator_data_dirs=validator_data_dirs,
    )


def build_atomic_swap_template(
    client: PostFiatRpcClient,
    *,
    left_wallet: TransparentWallet,
    right_wallet: TransparentWallet,
    left_asset_id: str,
    left_amount: int,
    right_asset_id: str,
    right_amount: int,
    condition: str,
    cancel_after: int,
    finish_after: int = 0,
    left_sequence: int | None = None,
    right_sequence: int | None = None,
) -> AtomicSettlementTemplateResult:
    """Build reciprocal escrow legs with atomic-swap naming."""

    return build_atomic_settlement_template(
        client,
        left_wallet=left_wallet,
        right_wallet=right_wallet,
        left_asset_id=left_asset_id,
        left_amount=left_amount,
        right_asset_id=right_asset_id,
        right_amount=right_amount,
        condition=condition,
        cancel_after=cancel_after,
        finish_after=finish_after,
        left_sequence=left_sequence,
        right_sequence=right_sequence,
    )


def send_shielded_pft(
    *,
    data_dir: str | Path,
    from_wallet: TransparentWallet,
    recipient: OrchardWallet,
    amount: int,
    fee: int = 0,
    work_dir: str | Path | None = None,
    validator_data_dirs: Sequence[str | Path] | None = None,
    memo_hex: str | None = None,
    policy_id: str | None = None,
    disclosure_hash: str | None = None,
    client: PostFiatRpcClient | None = None,
) -> ShieldedPftResult:
    """Send transparent PFT into an Orchard shielded output.

    With ``client`` omitted, this creates and applies the local Orchard deposit
    batch. With ``client`` supplied, it uses the gated RPC batch-create method
    and returns the batch result from the write edge.
    """

    if amount < 1:
        raise ValueError("amount must be positive")
    if fee < 0:
        raise ValueError("fee must be non-negative")
    data_dir = Path(data_dir)
    work_dir = _work_dir(work_dir)
    deposit_file = work_dir / f"orchard-deposit-{secrets.token_hex(8)}.json"
    deposit_args = [
        *_node_bin(),
        "orchard-deposit-create",
        "--data-dir",
        str(data_dir),
        "--key-file",
        str(from_wallet.key_file),
        "--recipient-view-key-file",
        str(recipient.view_key_file),
        "--amount",
        str(amount),
        "--fee",
        str(fee),
        "--deposit-file",
        str(deposit_file),
        "--overwrite",
    ]
    if memo_hex is not None:
        deposit_args.extend(["--memo-hex", memo_hex])
    if policy_id is not None:
        deposit_args.extend(["--policy-id", policy_id])
    if disclosure_hash is not None:
        deposit_args.extend(["--disclosure-hash", disclosure_hash])
    deposit_report = _run_json(deposit_args)

    if client is not None:
        batch_result = client.shield_batch_orchard_deposit(
            _read_json(deposit_file),
            request_id=f"py-orchard-deposit-{secrets.token_hex(4)}",
        )
        return ShieldedPftResult(
            tx_id=batch_result.get("batch_id") if isinstance(batch_result.get("batch_id"), str) else None,
            deposit_file=deposit_file,
            batch_file=None,
            deposit_report=deposit_report,
            batch_result=batch_result,
            receipts_by_validator=(),
        )

    batch_file = work_dir / f"orchard-deposit-{secrets.token_hex(8)}.batch.json"
    batch_result = _run_json(
        [
            *_node_bin(),
            "shield-batch-orchard-deposit",
            "--data-dir",
            str(data_dir),
            "--deposit-file",
            str(deposit_file),
            "--batch-file",
            str(batch_file),
        ]
    )
    receipts = _apply_shield_batch(
        batch_file=batch_file,
        validator_data_dirs=validator_data_dirs or [data_dir],
    )
    tx_id = _first_receipt_tx_id(receipts)
    return ShieldedPftResult(
        tx_id=tx_id,
        deposit_file=deposit_file,
        batch_file=batch_file,
        deposit_report=deposit_report,
        batch_result=batch_result,
        receipts_by_validator=tuple(receipts),
    )


def scan_orchard_wallet(*, data_dir: str | Path, wallet: OrchardWallet) -> dict[str, Any]:
    """Scan the local Orchard pool for notes visible to ``wallet``."""

    return _run_json(
        [
            *_node_bin(),
            "orchard-scan",
            "--data-dir",
            str(data_dir),
            "--key-file",
            str(wallet.key_file),
        ]
    )


def _apply_batch(
    *,
    batch_file: Path,
    validator_data_dirs: Sequence[str | Path],
) -> list[Any]:
    receipts = []
    for validator_data_dir in validator_data_dirs:
        receipts.append(
            _run(
                [
                    *_node_bin(),
                    "apply-batch",
                    "--data-dir",
                    str(validator_data_dir),
                    "--batch-file",
                    str(batch_file),
                ],
                json_output=True,
            )
        )
    return receipts


def _apply_shield_batch(
    *,
    batch_file: Path,
    validator_data_dirs: Sequence[str | Path],
) -> list[Any]:
    receipts = []
    for validator_data_dir in validator_data_dirs:
        receipts.append(
            _run(
                [
                    *_node_bin(),
                    "apply-shield-batch",
                    "--data-dir",
                    str(validator_data_dir),
                    "--batch-file",
                    str(batch_file),
                ],
                json_output=True,
            )
        )
    return receipts


def _node_bin() -> list[str]:
    binary = REPO_ROOT / "target" / "debug" / "postfiat-node"
    if binary.exists():
        return [str(binary)]
    return ["cargo", "run", "-p", "postfiat-node", "--"]


def _sdk_bin() -> list[str]:
    binary = REPO_ROOT / "target" / "debug" / "postfiat-rpc-sdk"
    if binary.exists():
        return [str(binary)]
    return ["cargo", "run", "-p", "postfiat-rpc-sdk", "--"]


def _work_dir(work_dir: str | Path | None) -> Path:
    if work_dir is None:
        path = Path(tempfile.mkdtemp(prefix="postfiat-python-wallet-"))
        path.chmod(0o700)
    else:
        path = Path(work_dir)
    path.mkdir(parents=True, exist_ok=True)
    return path


def _issued_asset_id(chain_id: str, issuer: str, code: str, version: int) -> str:
    if not chain_id:
        raise ValueError("chain_id is required")
    if not issuer:
        raise ValueError("issuer is required")
    _validate_asset_code(code)
    if version < 1:
        raise ValueError("version must be positive")
    code_bytes = code.encode("utf-8")
    preimage = (
        f"chain_id={chain_id}\nissuer={issuer}\ncode_bytes={len(code_bytes)}\n"
        f"code={code}\nversion={version}\n"
    ).encode("utf-8")
    digest = hashlib.sha3_384()
    digest.update(ISSUED_ASSET_ID_DOMAIN.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(preimage)
    return digest.hexdigest()


def _escrow_id(chain_id: str, owner: str, owner_sequence: int) -> str:
    if not chain_id:
        raise ValueError("chain_id is required")
    if not owner:
        raise ValueError("owner is required")
    if owner_sequence < 1:
        raise ValueError("owner_sequence must be positive")
    preimage = f"chain_id={chain_id}\nowner={owner}\nowner_sequence={owner_sequence}\n".encode(
        "utf-8"
    )
    digest = hashlib.sha3_384()
    digest.update(ESCROW_ID_DOMAIN.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(preimage)
    return digest.hexdigest()


def _nft_id(chain_id: str, issuer: str, collection_id: str, serial: int) -> str:
    if not chain_id:
        raise ValueError("chain_id is required")
    if not issuer:
        raise ValueError("issuer is required")
    _validate_nft_collection_id(collection_id)
    _validate_positive_u64(serial, "serial")
    collection_id_bytes = collection_id.encode("utf-8")
    preimage = (
        f"chain_id={chain_id}\nissuer={issuer}\n"
        f"collection_id_bytes={len(collection_id_bytes)}\n"
        f"collection_id={collection_id}\nserial={serial}\n"
    ).encode("utf-8")
    digest = hashlib.sha3_384()
    digest.update(NFT_ID_DOMAIN.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(preimage)
    return digest.hexdigest()


def _offer_id(chain_id: str, owner: str, owner_sequence: int) -> str:
    if not chain_id:
        raise ValueError("chain_id is required")
    if not owner:
        raise ValueError("owner is required")
    _validate_positive_u64(owner_sequence, "owner_sequence")
    preimage = f"chain_id={chain_id}\nowner={owner}\nowner_sequence={owner_sequence}\n".encode(
        "utf-8"
    )
    digest = hashlib.sha3_384()
    digest.update(OFFER_ID_DOMAIN.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(preimage)
    return digest.hexdigest()


def _validate_positive_u64(value: int, name: str) -> None:
    if not isinstance(value, int) or isinstance(value, bool) or value < 1:
        raise ValueError(f"{name} must be positive")
    if value > 2**64 - 1:
        raise ValueError(f"{name} must fit in u64")


def _validate_nonnegative_u64(value: int, name: str) -> None:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ValueError(f"{name} must be nonnegative")
    if value > 2**64 - 1:
        raise ValueError(f"{name} must fit in u64")


def _validate_asset_code(code: str) -> None:
    if (
        not code
        or code != code.strip()
        or any(ord(char) < 32 or ord(char) == 127 for char in code)
    ):
        raise ValueError("code must be nonempty without leading, trailing, or control whitespace")
    if len(code.encode("utf-8")) > MAX_ISSUED_ASSET_CODE_BYTES:
        raise ValueError(f"code must not exceed {MAX_ISSUED_ASSET_CODE_BYTES} bytes")


def _validate_asset_id(asset_id: str) -> None:
    if len(asset_id) != 96 or any(char not in "0123456789abcdef" for char in asset_id):
        raise ValueError("asset_id must be 96 lowercase hex characters")


def _validate_atomic_asset_id(asset_id: str) -> None:
    if asset_id == NATIVE_PFT_ESCROW_ASSET_ID:
        return
    _validate_asset_id(asset_id)


def _template_leg_sequence(leg: dict[str, Any], label: str) -> int | None:
    sequence = leg.get("sequence")
    if sequence is None:
        return None
    if not isinstance(sequence, int) or sequence < 1:
        raise ValueError(f"{label} atomic template leg has invalid sequence")
    return sequence


def _require_escrow_submit_accepted(result: EscrowTransactionResult, label: str) -> None:
    submit_result = result.submit_result
    if submit_result.get("ok") is False or submit_result.get("error") is not None:
        error = submit_result.get("error")
        message = None
        if isinstance(error, dict):
            message = error.get("message") or error.get("code")
        raise ValueError(f"{label} was not accepted: {message or 'escrow submit failed'}")


def _wait_for_open_escrow(
    client: PostFiatRpcClient,
    escrow_id: str,
    label: str,
    *,
    timeout_seconds: float = 45.0,
    poll_seconds: float = 1.0,
) -> dict[str, Any] | None:
    if timeout_seconds <= 0:
        return None
    if poll_seconds <= 0:
        raise ValueError("poll_seconds must be positive")
    deadline = time.monotonic() + timeout_seconds
    last_info: dict[str, Any] | None = None
    while True:
        info = client.escrow_info(escrow_id)
        last_info = info
        escrow = info.get("escrow") if isinstance(info, dict) else None
        if info.get("found") is True and isinstance(escrow, dict):
            state = escrow.get("state") or escrow.get("status")
            if state is None or state == "open":
                return info
            raise ValueError(f"{label} escrow {escrow_id} is {state}")
        if time.monotonic() >= deadline:
            break
        time.sleep(min(poll_seconds, max(0.0, deadline - time.monotonic())))
    state = None
    if isinstance(last_info, dict):
        escrow = last_info.get("escrow")
        if isinstance(escrow, dict):
            state = escrow.get("state") or escrow.get("status")
    suffix = f"; last state {state}" if state else ""
    raise TimeoutError(f"{label} escrow {escrow_id} was not open after {timeout_seconds:.1f}s{suffix}")


def _validate_dex_asset_id(asset_id: str) -> None:
    if asset_id == NATIVE_PFT_ESCROW_ASSET_ID:
        return
    _validate_asset_id(asset_id)


def _validate_escrow_id(escrow_id: str) -> None:
    if len(escrow_id) != 96 or any(char not in "0123456789abcdef" for char in escrow_id):
        raise ValueError("escrow_id must be 96 lowercase hex characters")


def _validate_offer_id(offer_id: str) -> None:
    if len(offer_id) != 96 or any(char not in "0123456789abcdef" for char in offer_id):
        raise ValueError("offer_id must be 96 lowercase hex characters")


def _validate_nft_id(nft_id: str) -> None:
    if len(nft_id) != 96 or any(char not in "0123456789abcdef" for char in nft_id):
        raise ValueError("nft_id must be 96 lowercase hex characters")


def _validate_nft_collection_id(collection_id: str) -> None:
    if (
        not isinstance(collection_id, str)
        or not collection_id
        or collection_id != collection_id.strip()
        or _contains_control_char(collection_id)
    ):
        raise ValueError(
            "collection_id must be nonempty without leading, trailing, or control whitespace"
        )
    if len(collection_id.encode("utf-8")) > MAX_NFT_COLLECTION_ID_BYTES:
        raise ValueError(f"collection_id must not exceed {MAX_NFT_COLLECTION_ID_BYTES} bytes")


def _validate_nft_metadata_hash(metadata_hash: str) -> None:
    if (
        not isinstance(metadata_hash, str)
        or not metadata_hash
        or len(metadata_hash) > MAX_NFT_METADATA_HASH_HEX_CHARS
        or len(metadata_hash) % 2 != 0
        or any(char not in "0123456789abcdef" for char in metadata_hash)
    ):
        raise ValueError(
            "metadata_hash must be nonempty even-length lowercase hex no longer than "
            f"{MAX_NFT_METADATA_HASH_HEX_CHARS} characters"
        )


def _validate_nft_metadata_uri(metadata_uri: str) -> None:
    if not isinstance(metadata_uri, str):
        raise ValueError("metadata_uri must be a string")
    if metadata_uri and (
        metadata_uri != metadata_uri.strip() or _contains_control_char(metadata_uri)
    ):
        raise ValueError("metadata_uri must not have leading, trailing, or control whitespace")
    if len(metadata_uri.encode("utf-8")) > MAX_NFT_METADATA_URI_BYTES:
        raise ValueError(f"metadata_uri must not exceed {MAX_NFT_METADATA_URI_BYTES} bytes")


def _validate_nft_flags(flags: int) -> None:
    if not isinstance(flags, int) or isinstance(flags, bool):
        raise ValueError("flags must be an integer")
    if flags < 0 or flags > 2**32 - 1:
        raise ValueError("flags must fit in u32")
    if flags & ~NFT_ALLOWED_FLAGS:
        raise ValueError("flags contains unsupported NFT bits")


def _validate_nft_collection_flags(collection_flags: int) -> None:
    if not isinstance(collection_flags, int) or isinstance(collection_flags, bool):
        raise ValueError("collection_flags must be an integer")
    if collection_flags < 0 or collection_flags > 2**32 - 1:
        raise ValueError("collection_flags must fit in u32")
    if collection_flags & ~NFT_COLLECTION_ALLOWED_FLAGS:
        raise ValueError("collection_flags contains unsupported NFT collection bits")


def _contains_control_char(value: str) -> bool:
    return any(ord(char) < 32 or 127 <= ord(char) <= 159 for char in value)


def _validate_escrow_create(
    *,
    owner: str,
    recipient: str,
    amount: int,
    condition: str,
    finish_after: int,
    cancel_after: int,
) -> None:
    if not owner:
        raise ValueError("owner is required")
    if not recipient:
        raise ValueError("recipient is required")
    if owner == recipient:
        raise ValueError("owner must differ from recipient")
    if amount < 1:
        raise ValueError("amount must be positive")
    if not isinstance(condition, str):
        raise ValueError("condition must be a string")
    if finish_after < 0:
        raise ValueError("finish_after must be non-negative")
    if cancel_after < 0:
        raise ValueError("cancel_after must be non-negative")
    if not condition and finish_after == 0 and cancel_after == 0:
        raise ValueError("escrow create requires condition, finish_after, or cancel_after")
    if finish_after and cancel_after and cancel_after <= finish_after:
        raise ValueError("cancel_after must be greater than finish_after")


def _run_json(args: Sequence[str], *, cwd: Path = REPO_ROOT) -> dict[str, Any]:
    value = _run(args, cwd=cwd, json_output=True)
    if not isinstance(value, dict):
        raise WalletCommandError("wallet command returned non-object JSON")
    return value


def _run_json_with_secret_file(
    args: Sequence[str],
    secret_file_flag: str,
    secret_value: str,
    *,
    cwd: Path = REPO_ROOT,
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="postfiat-wallet-secret-") as temp_dir:
        secret_path = Path(temp_dir) / "secret.txt"
        _write_private_text(secret_path, f"{secret_value}\n")
        return _run_json(
            [*args, secret_file_flag, str(secret_path)],
            cwd=cwd,
        )


def _run(
    args: Sequence[str],
    *,
    cwd: Path = REPO_ROOT,
    json_output: bool,
) -> Any:
    completed = subprocess.run(
        list(args),
        cwd=cwd,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        raise WalletCommandError(
            "command failed with exit "
            f"{completed.returncode}: {' '.join(args)}\n{completed.stderr.strip()}"
        )
    if not json_output:
        return None
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise WalletCommandError(
            f"command did not return JSON: {' '.join(args)}\n{completed.stdout[:500]}"
        ) from error


def _read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise WalletCommandError(f"{path} did not contain a JSON object")
    return value


def _write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    _write_private_text(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


def _write_private_text(path: Path, value: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            fd = -1
            handle.write(value)
    finally:
        if fd >= 0:
            os.close(fd)
    path.chmod(0o600)


def _required_str(value: dict[str, Any], key: str) -> str:
    found = value.get(key)
    if not isinstance(found, str) or not found:
        raise WalletCommandError(f"wallet command response missing string field {key}")
    return found


def _first_receipt_tx_id(receipts_by_validator: Sequence[Any]) -> str | None:
    if not receipts_by_validator:
        return None
    first = receipts_by_validator[0]
    if isinstance(first, list) and first and isinstance(first[0], dict):
        tx_id = first[0].get("tx_id")
        return tx_id if isinstance(tx_id, str) else None
    receipts = first.get("receipts") if isinstance(first, dict) else None
    if isinstance(receipts, list) and receipts and isinstance(receipts[0], dict):
        tx_id = receipts[0].get("tx_id")
        return tx_id if isinstance(tx_id, str) else None
    tx_id = first.get("tx_id") if isinstance(first, dict) else None
    return tx_id if isinstance(tx_id, str) else None


def _first_certified_tx_id(certified_round: dict[str, Any]) -> str | None:
    hot_finality = certified_round.get("local_hot_finality")
    if isinstance(hot_finality, list) and hot_finality:
        first = hot_finality[0]
        if isinstance(first, dict):
            tx_id = first.get("tx_id")
            if isinstance(tx_id, str):
                return tx_id
            receipt = first.get("receipt")
            if isinstance(receipt, dict) and isinstance(receipt.get("tx_id"), str):
                return receipt["tx_id"]
    certification = certified_round.get("certification")
    receipt_ids = certification.get("receipt_ids") if isinstance(certification, dict) else None
    if isinstance(receipt_ids, list) and receipt_ids and isinstance(receipt_ids[0], str):
        return receipt_ids[0]
    return None


def _select_fastpay_input(
    owned_objects: Sequence[dict[str, Any]],
    required_value: int,
    asset: str,
) -> dict[str, Any]:
    if required_value < 1:
        raise ValueError("required_value must be positive")
    candidates = []
    for obj in owned_objects:
        if not isinstance(obj, dict):
            continue
        if obj.get("asset", asset) != asset:
            continue
        try:
            value = int(obj.get("value", 0))
        except (TypeError, ValueError):
            continue
        if value >= required_value and obj.get("id") is not None and obj.get("version") is not None:
            candidates.append((value, obj))
    if not candidates:
        raise ValueError("no FastPay owned object covers amount plus fee")
    candidates.sort(key=lambda item: item[0])
    return candidates[0][1]


def _select_fastpay_inputs(
    owned_objects: Sequence[dict[str, Any]],
    required_value: int,
    asset: str,
    *,
    max_inputs: int = 2048,
) -> list[dict[str, Any]]:
    if required_value < 1:
        raise ValueError("required_value must be positive")
    if max_inputs < 1:
        raise ValueError("max_inputs must be positive")
    candidates: list[tuple[int, int, dict[str, Any]]] = []
    for index, obj in enumerate(owned_objects):
        if not isinstance(obj, dict):
            continue
        if obj.get("asset", asset) != asset:
            continue
        if obj.get("id") is None or obj.get("version") is None:
            continue
        try:
            value = int(obj.get("value", 0))
        except (TypeError, ValueError):
            continue
        if value > 0:
            candidates.append((value, index, obj))
    total = sum(value for value, _index, _obj in candidates)
    if total < required_value:
        raise ValueError("insufficient FastPay balance for amount plus fee")

    single = [(value, index, obj) for value, index, obj in candidates if value >= required_value]
    if single:
        single.sort(key=lambda item: (item[0], item[1]))
        return [single[0][2]]

    largest_first = sorted(
        candidates,
        key=lambda item: (-item[0], item[1]),
    )
    selected: list[dict[str, Any]] = []
    selected_total = 0
    for value, _index, obj in largest_first:
        selected.append(obj)
        selected_total += value
        if selected_total >= required_value:
            return selected
        if len(selected) >= max_inputs:
            break
    raise ValueError(f"no combination of up to {max_inputs} FastPay objects covers amount plus fee")


def _find_fastpay_object(
    owned_objects: Sequence[dict[str, Any]],
    object_id: str | None,
    asset: str,
) -> dict[str, Any] | None:
    if not object_id:
        return None
    for obj in owned_objects:
        if not isinstance(obj, dict):
            continue
        if str(obj.get("id", "")) != object_id:
            continue
        if obj.get("asset", asset) != asset:
            raise ValueError("selected FastPay object uses a different asset")
        if obj.get("version") is None:
            raise ValueError("selected FastPay object is missing version")
        try:
            int(obj.get("value", 0))
        except (TypeError, ValueError) as error:
            raise ValueError("selected FastPay object has invalid value") from error
        return obj
    raise ValueError("selected FastPay object was not found")


def _validators_list(validators_response: Any) -> list[dict[str, Any]]:
    if isinstance(validators_response, list):
        return [item for item in validators_response if isinstance(item, dict)]
    if isinstance(validators_response, dict):
        validators = validators_response.get("validators")
        if isinstance(validators, list):
            return [item for item in validators if isinstance(item, dict)]
    return []


def _fastpay_session_get_or_load(client: PostFiatRpcClient, key: str, loader):
    session_loader = getattr(client, "session_get_or_load", None)
    if callable(session_loader):
        return session_loader(key, loader)
    return loader()


def _fastpay_validator_records(client: PostFiatRpcClient) -> list[dict[str, Any]]:
    response = _fastpay_session_get_or_load(
        client,
        "fastpay.validators",
        client.validators,
    )
    return _validators_list(response)


def _require_fastpay_broadcast_rpc(client: PostFiatRpcClient) -> None:
    try:
        capabilities = _fastpay_session_get_or_load(
            client,
            "fastpay.server_capabilities",
            client.server_capabilities,
        )
    except Exception as error:
        raise WalletCommandError(
            "FastPay WAN helpers require a wallet-facing broadcast RPC endpoint; "
            f"capability discovery failed: {error}"
        ) from error
    if not isinstance(capabilities, dict):
        raise WalletCommandError("FastPay RPC capabilities response is malformed")
    if (
        capabilities.get("fastpay_bridge_enabled") is True
        and capabilities.get("fastpay_bridge_mode") == "proxy_broadcast_devnet"
        and capabilities.get("fastpay_owned_apply_broadcast_enabled") is True
    ):
        return
    raise WalletCommandError(
        "FastPay WAN helpers require a wallet-facing broadcast RPC endpoint "
        "(fastpay_bridge_mode=proxy_broadcast_devnet). Raw single-validator RPC "
        "would split owned-object state."
    )


def _fastpay_recovery_window(
    capabilities: dict[str, Any],
    *,
    wallet: TransparentWallet,
    validators: Sequence[dict[str, Any]],
) -> dict[str, Any]:
    if not isinstance(capabilities, dict):
        raise WalletCommandError("FastPay recovery capabilities are malformed")
    domain = capabilities.get("domain")
    policy = capabilities.get("policy")
    if (
        capabilities.get("schema") != "postfiat-fastpay-recovery-capabilities-v1"
        or not isinstance(domain, dict)
        or domain.get("schema") != "postfiat-owned-certificate-domain-v3"
        or domain.get("chain_id") != wallet.chain_id
        or not isinstance(policy, dict)
        or policy.get("schema") != "postfiat-fastpay-recovery-policy-v1"
    ):
        raise WalletCommandError("FastPay recovery capability domain is invalid")
    current_height = capabilities.get("current_height")
    committee_epoch = capabilities.get("committee_epoch")
    validator_count = capabilities.get("validator_count")
    quorum = capabilities.get("quorum")
    activation_height = policy.get("activation_height")
    validity_blocks = policy.get("max_validity_blocks")
    recovery_blocks = policy.get("max_recovery_blocks")
    numeric = (
        current_height,
        committee_epoch,
        validator_count,
        quorum,
        activation_height,
        validity_blocks,
        recovery_blocks,
    )
    if any(isinstance(value, bool) or not isinstance(value, int) for value in numeric):
        raise WalletCommandError("FastPay recovery capability heights are malformed")
    validator_ids = [_validator_id(record) for record in validators]
    validator_keys = [record.get("public_key_hex") for record in validators]
    expected_quorum = validator_count - (validator_count - 1) // 3 if validator_count > 0 else 0
    if (
        current_height < activation_height
        or current_height < 1
        or committee_epoch < 1
        or validity_blocks < 1
        or recovery_blocks < 1
        or validator_count != len(validators)
        or quorum != expected_quorum
        or any(not isinstance(value, str) or not value for value in validator_ids)
        or len(set(validator_ids)) != validator_count
        or any(not isinstance(value, str) or not value for value in validator_keys)
    ):
        raise WalletCommandError("FastPay recovery capability or validator roster is invalid")
    expires_at_height = current_height + validity_blocks
    recovery_closes_at_height = expires_at_height + recovery_blocks
    if recovery_closes_at_height > (1 << 64) - 1:
        raise WalletCommandError("FastPay recovery window overflows u64")
    return {
        "schema": "postfiat-fastpay-order-recovery-v1",
        "committee_epoch": committee_epoch,
        "lock_id": "0" * 96,
        "valid_from_height": current_height,
        "expires_at_height": expires_at_height,
        "recovery_closes_at_height": recovery_closes_at_height,
    }


def _owned_sign_vote(
    client: PostFiatRpcClient,
    order_json: str,
    validator_id: str,
) -> dict[str, Any]:
    websocket_url = getattr(client, "url", None)
    if isinstance(websocket_url, str) and websocket_url.startswith(("ws://", "wss://")):
        worker_factory = getattr(client, "persistent_worker", None)
        if callable(worker_factory):
            vote_client = worker_factory(f"fastpay-owned-sign:{validator_id}")
            return vote_client.owned_sign(order_json, validator_id)
        vote_client = PostFiatWebSocketRpcClient(
            websocket_url,
            timeout_seconds=client.timeout_seconds,
            response_byte_cap=client.response_byte_cap,
        )
        try:
            return vote_client.owned_sign(order_json, validator_id)
        finally:
            vote_client.close()
    return client.owned_sign(order_json, validator_id)


def _owned_unwrap_sign_vote(
    client: PostFiatRpcClient,
    order_json: str,
    validator_id: str,
) -> dict[str, Any]:
    websocket_url = getattr(client, "url", None)
    if isinstance(websocket_url, str) and websocket_url.startswith(("ws://", "wss://")):
        worker_factory = getattr(client, "persistent_worker", None)
        if callable(worker_factory):
            vote_client = worker_factory(f"fastpay-owned-unwrap-sign:{validator_id}")
            return vote_client.owned_unwrap_sign(order_json, validator_id)
        vote_client = PostFiatWebSocketRpcClient(
            websocket_url,
            timeout_seconds=client.timeout_seconds,
            response_byte_cap=client.response_byte_cap,
        )
        try:
            return vote_client.owned_unwrap_sign(order_json, validator_id)
        finally:
            vote_client.close()
    return client.owned_unwrap_sign(order_json, validator_id)


def _owned_sign_vote_v3(
    client: PostFiatRpcClient,
    order_json: str,
    validator_id: str,
) -> dict[str, Any]:
    websocket_url = getattr(client, "url", None)
    if isinstance(websocket_url, str) and websocket_url.startswith(("ws://", "wss://")):
        worker_factory = getattr(client, "persistent_worker", None)
        if callable(worker_factory):
            vote_client = worker_factory(f"fastpay-owned-sign-v3:{validator_id}")
            return vote_client.owned_sign_v3(order_json, validator_id)
        vote_client = PostFiatWebSocketRpcClient(
            websocket_url,
            timeout_seconds=client.timeout_seconds,
            response_byte_cap=client.response_byte_cap,
        )
        try:
            return vote_client.owned_sign_v3(order_json, validator_id)
        finally:
            vote_client.close()
    return client.owned_sign_v3(order_json, validator_id)


def _owned_unwrap_sign_vote_v3(
    client: PostFiatRpcClient,
    order_json: str,
    validator_id: str,
) -> dict[str, Any]:
    websocket_url = getattr(client, "url", None)
    if isinstance(websocket_url, str) and websocket_url.startswith(("ws://", "wss://")):
        worker_factory = getattr(client, "persistent_worker", None)
        if callable(worker_factory):
            vote_client = worker_factory(f"fastpay-owned-unwrap-sign-v3:{validator_id}")
            return vote_client.owned_unwrap_sign_v3(order_json, validator_id)
        vote_client = PostFiatWebSocketRpcClient(
            websocket_url,
            timeout_seconds=client.timeout_seconds,
            response_byte_cap=client.response_byte_cap,
        )
        try:
            return vote_client.owned_unwrap_sign_v3(order_json, validator_id)
        finally:
            vote_client.close()
    return client.owned_unwrap_sign_v3(order_json, validator_id)


def _validator_id(record: dict[str, Any]) -> str | None:
    for key in ("node_id", "validator_id", "id"):
        value = record.get(key)
        if isinstance(value, str) and value:
            return value
    return None


def _collect_fastpay_votes(
    client: PostFiatRpcClient,
    signed_order_envelope: dict[str, Any],
    validators: Sequence[dict[str, Any]],
    quorum: int | None = None,
) -> list[dict[str, Any]]:
    order_json = json.dumps(signed_order_envelope, separators=(",", ":"))
    validator_ids = [
        validator_id
        for validator in validators
        if (validator_id := _validator_id(validator)) is not None
    ]
    if not validator_ids:
        return []

    votes = []
    executor = ThreadPoolExecutor(max_workers=len(validator_ids))
    futures = {
        executor.submit(_owned_sign_vote, client, order_json, validator_id): validator_id
        for validator_id in validator_ids
    }
    try:
        for future in as_completed(futures):
            try:
                vote = future.result()
            except Exception:
                continue
            if isinstance(vote, dict) and vote.get("validator_id") and vote.get("signature_hex"):
                votes.append(
                    {
                        "validator_id": str(vote["validator_id"]),
                        "signature_hex": str(vote["signature_hex"]),
                    }
                )
                if quorum is not None and len(votes) >= quorum:
                    for pending in futures:
                        if not pending.done():
                            pending.cancel()
                    break
    finally:
        executor.shutdown(wait=False, cancel_futures=True)
    return votes


def _collect_fastpay_unwrap_votes(
    client: PostFiatRpcClient,
    signed_order_envelope: dict[str, Any],
    validators: Sequence[dict[str, Any]],
    quorum: int | None = None,
) -> list[dict[str, Any]]:
    order_json = json.dumps(signed_order_envelope, separators=(",", ":"))
    validator_ids = [
        validator_id
        for validator in validators
        if (validator_id := _validator_id(validator)) is not None
    ]
    if not validator_ids:
        return []

    votes = []
    executor = ThreadPoolExecutor(max_workers=len(validator_ids))
    futures = {
        executor.submit(_owned_unwrap_sign_vote, client, order_json, validator_id): validator_id
        for validator_id in validator_ids
    }
    try:
        for future in as_completed(futures):
            try:
                vote = future.result()
            except Exception:
                continue
            if isinstance(vote, dict) and vote.get("validator_id") and vote.get("signature_hex"):
                votes.append(
                    {
                        "validator_id": str(vote["validator_id"]),
                        "signature_hex": str(vote["signature_hex"]),
                    }
                )
                if quorum is not None and len(votes) >= quorum:
                    for pending in futures:
                        if not pending.done():
                            pending.cancel()
                    break
    finally:
        executor.shutdown(wait=False, cancel_futures=True)
    return votes


def _collect_fastpay_votes_v3(
    client: PostFiatRpcClient,
    signed_order: dict[str, Any],
    validators: Sequence[dict[str, Any]],
    quorum: int,
) -> list[dict[str, Any]]:
    return _collect_fastpay_votes_with(
        client,
        signed_order,
        validators,
        quorum,
        _owned_sign_vote_v3,
    )


def _collect_fastpay_unwrap_votes_v3(
    client: PostFiatRpcClient,
    signed_order: dict[str, Any],
    validators: Sequence[dict[str, Any]],
    quorum: int,
) -> list[dict[str, Any]]:
    return _collect_fastpay_votes_with(
        client,
        signed_order,
        validators,
        quorum,
        _owned_unwrap_sign_vote_v3,
    )


def _collect_fastpay_votes_with(
    client: PostFiatRpcClient,
    signed_order: dict[str, Any],
    validators: Sequence[dict[str, Any]],
    quorum: int,
    vote_call,
) -> list[dict[str, Any]]:
    order_json = json.dumps(signed_order, separators=(",", ":"))
    validator_ids = [
        validator_id
        for validator in validators
        if (validator_id := _validator_id(validator)) is not None
    ]
    votes: list[dict[str, Any]] = []
    seen: set[str] = set()
    executor = ThreadPoolExecutor(max_workers=len(validator_ids))
    futures = {
        executor.submit(vote_call, client, order_json, validator_id): validator_id
        for validator_id in validator_ids
    }
    try:
        for future in as_completed(futures):
            expected_validator_id = futures[future]
            try:
                vote = future.result()
            except Exception:
                continue
            validator_id = vote.get("validator_id") if isinstance(vote, dict) else None
            signature_hex = vote.get("signature_hex") if isinstance(vote, dict) else None
            if (
                validator_id != expected_validator_id
                or not isinstance(signature_hex, str)
                or not signature_hex
                or validator_id in seen
            ):
                continue
            seen.add(validator_id)
            votes.append({"validator_id": validator_id, "signature_hex": signature_hex})
            if len(votes) >= quorum:
                for pending in futures:
                    if not pending.done():
                        pending.cancel()
                break
    finally:
        executor.shutdown(wait=False, cancel_futures=True)
    return votes


def _sign_fastpay_order(
    *,
    wallet: TransparentWallet,
    order: dict[str, Any],
    work_dir: Path,
) -> dict[str, Any]:
    order_file = work_dir / f"fastpay-order-{secrets.token_hex(8)}.json"
    signed_file = work_dir / f"fastpay-order-{secrets.token_hex(8)}.signed.json"
    _write_json(order_file, order)
    _run(
        [
            *_sdk_bin(),
            "wallet-sign-owned-transfer",
            "--backup-file",
            str(wallet.backup_file),
            "--order-file",
            str(order_file),
            "--output",
            str(signed_file),
        ],
        json_output=False,
    )
    return _read_json(signed_file)


def _sign_fastpay_unwrap_order(
    *,
    wallet: TransparentWallet,
    order: dict[str, Any],
    work_dir: Path,
) -> dict[str, Any]:
    order_file = work_dir / f"fastpay-unwrap-order-{secrets.token_hex(8)}.json"
    signed_file = work_dir / f"fastpay-unwrap-order-{secrets.token_hex(8)}.signed.json"
    _write_json(order_file, order)
    _run(
        [
            *_sdk_bin(),
            "wallet-sign-owned-unwrap",
            "--backup-file",
            str(wallet.backup_file),
            "--order-file",
            str(order_file),
            "--output",
            str(signed_file),
        ],
        json_output=False,
    )
    return _read_json(signed_file)


def _sign_fastpay_order_v3(
    *,
    wallet: TransparentWallet,
    order: dict[str, Any],
    capabilities: dict[str, Any],
    work_dir: Path,
) -> dict[str, Any]:
    order_file = work_dir / f"fastpay-v3-order-{secrets.token_hex(8)}.json"
    capabilities_file = work_dir / f"fastpay-v3-capabilities-{secrets.token_hex(8)}.json"
    signed_file = work_dir / f"fastpay-v3-order-{secrets.token_hex(8)}.signed.json"
    _write_json(order_file, order)
    _write_json(capabilities_file, capabilities)
    _run(
        [
            *_sdk_bin(),
            "wallet-sign-owned-transfer-v3",
            "--backup-file",
            str(wallet.backup_file),
            "--order-file",
            str(order_file),
            "--capabilities-file",
            str(capabilities_file),
            "--output",
            str(signed_file),
        ],
        json_output=False,
    )
    signed = _read_json(signed_file)
    if (
        signed.get("owner_pubkey_hex") != wallet.public_key_hex
        or not isinstance(signed.get("owner_signature_hex"), str)
        or not isinstance(signed.get("order"), dict)
        or signed["order"].get("recovery", {}).get("lock_id") in (None, "0" * 96)
    ):
        raise WalletCommandError("FastPay v3 signer returned a mismatched owner or lock")
    return signed


def _sign_fastpay_unwrap_order_v3(
    *,
    wallet: TransparentWallet,
    order: dict[str, Any],
    capabilities: dict[str, Any],
    work_dir: Path,
) -> dict[str, Any]:
    order_file = work_dir / f"fastpay-v3-unwrap-{secrets.token_hex(8)}.json"
    capabilities_file = work_dir / f"fastpay-v3-capabilities-{secrets.token_hex(8)}.json"
    signed_file = work_dir / f"fastpay-v3-unwrap-{secrets.token_hex(8)}.signed.json"
    _write_json(order_file, order)
    _write_json(capabilities_file, capabilities)
    _run(
        [
            *_sdk_bin(),
            "wallet-sign-owned-unwrap-v3",
            "--backup-file",
            str(wallet.backup_file),
            "--order-file",
            str(order_file),
            "--capabilities-file",
            str(capabilities_file),
            "--output",
            str(signed_file),
        ],
        json_output=False,
    )
    signed = _read_json(signed_file)
    if (
        signed.get("owner_pubkey_hex") != wallet.public_key_hex
        or not isinstance(signed.get("owner_signature_hex"), str)
        or not isinstance(signed.get("order"), dict)
        or signed["order"].get("recovery", {}).get("lock_id") in (None, "0" * 96)
    ):
        raise WalletCommandError("FastPay v3 unwrap signer returned a mismatched owner or lock")
    return signed


def _verify_fastpay_apply_v3(
    *,
    operation: str,
    certificate: dict[str, Any],
    apply_response: dict[str, Any],
    capabilities: dict[str, Any],
    validators: Sequence[dict[str, Any]],
    work_dir: Path,
) -> tuple[dict[str, Any], ...]:
    certificate_file = work_dir / f"fastpay-v3-{operation}-certificate-{secrets.token_hex(8)}.json"
    response_file = work_dir / f"fastpay-v3-{operation}-apply-{secrets.token_hex(8)}.json"
    capabilities_file = work_dir / f"fastpay-v3-{operation}-capabilities-{secrets.token_hex(8)}.json"
    validators_file = work_dir / f"fastpay-v3-{operation}-validators-{secrets.token_hex(8)}.json"
    output_file = work_dir / f"fastpay-v3-{operation}-verification-{secrets.token_hex(8)}.json"
    _write_json(certificate_file, certificate)
    _write_json(response_file, apply_response)
    _write_json(capabilities_file, capabilities)
    _write_json(validators_file, {"validators": list(validators)})
    _run(
        [
            *_sdk_bin(),
            "wallet-verify-fastpay-apply-v3",
            "--operation",
            operation,
            "--certificate-file",
            str(certificate_file),
            "--apply-response-file",
            str(response_file),
            "--capabilities-file",
            str(capabilities_file),
            "--validators-file",
            str(validators_file),
            "--output",
            str(output_file),
        ],
        json_output=False,
    )
    verification = _read_json(output_file)
    acknowledgements = verification.get("authenticated_acknowledgements")
    quorum = capabilities.get("quorum")
    if (
        verification.get("schema") != "postfiat-fastpay-apply-verification-v1"
        or verification.get("operation") != operation
        or not isinstance(acknowledgements, list)
        or not isinstance(quorum, int)
        or len(acknowledgements) < quorum
    ):
        raise WalletCommandError("FastPay v3 apply verification omitted an authenticated quorum")
    return tuple(item for item in acknowledgements if isinstance(item, dict))
