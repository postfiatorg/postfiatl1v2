"""Small stdlib-only PostFiat RPC client.

The transport stays stdlib-only. Wallet signing and Orchard action creation
live in :mod:`postfiat_rpc.wallet`, which delegates protocol cryptography to
the Rust binaries already used by the node and SDK.
"""

from __future__ import annotations

import hashlib
import json
import socket
import threading
from dataclasses import dataclass, field
from typing import Any


RPC_VERSION = "postfiat-local-rpc-v1"
DEFAULT_TIMEOUT_SECONDS = 8.0
DEFAULT_RESPONSE_BYTE_CAP = 1_048_576
DEFAULT_READ_LIMIT = 100
# Mirrors crates/node/src/lifecycle_queries.rs MAX_READ_QUERY_LIMIT: the node
# refuses an `archive_window` range longer than this many blocks.
MAX_ARCHIVE_WINDOW_BLOCKS = 512
MAX_ACCOUNT_TX_SCAN_LIMIT = 512
ESCROW_ID_DOMAIN = "postfiat.escrow_id.v1"
ESCROW_CONDITION_HASH_DOMAIN = "postfiat.escrow_condition_hash.v1"
NFT_ID_DOMAIN = "postfiat.nft_id.v1"
OFFER_ID_DOMAIN = "postfiat.offer_id.v1"


class RpcClientError(RuntimeError):
    """Base client error."""


class RpcProtocolError(RpcClientError):
    """Response envelope or transport was malformed."""


class RpcError(RpcClientError):
    """RPC endpoint returned ok=false."""

    def __init__(self, method: str, error: dict[str, Any]) -> None:
        self.method = method
        self.error = error
        code = error.get("code", "rpc_error")
        message = error.get("message", "")
        super().__init__(f"{method} failed with {code}: {message}")


@dataclass(frozen=True)
class Endpoint:
    host: str
    port: int

    @classmethod
    def parse(cls, value: str) -> "Endpoint":
        endpoint = value.strip()
        if endpoint.startswith("tcp://"):
            endpoint = endpoint.removeprefix("tcp://")
        if endpoint.startswith("["):
            closing = endpoint.find("]")
            if closing < 0 or closing + 2 > len(endpoint) or endpoint[closing + 1] != ":":
                raise ValueError(f"unsupported endpoint syntax: {value}")
            host = endpoint[1:closing]
            port_text = endpoint[closing + 2 :]
        else:
            if ":" not in endpoint:
                raise ValueError(f"endpoint must be host:port: {value}")
            host, port_text = endpoint.rsplit(":", 1)
        port = int(port_text)
        if not (0 < port < 65536):
            raise ValueError(f"endpoint port out of range: {value}")
        return cls(host=host, port=port)


@dataclass(frozen=True)
class AccountTxRow:
    tx_id: str | None
    block_height: int
    batch_kind: str
    batch_id: str
    transaction_index: int
    transaction_kind: str | None
    sender: str | None
    recipient: str | None
    amount: int | None
    fee: int | None
    sequence: int | None
    memo_hash: str | None
    memo_count: int | None
    memo_bytes: int | None
    asset_id: str | None
    issuer: str | None
    trustline_authorized: bool | None
    trustline_frozen: bool | None
    nft_id: str | None
    nft_issuer_transfer_fee: int | None
    nft_collection_flags: int | None
    escrow_id: str | None
    offer_id: str | None
    tx_role: str | None
    counterparty_offer_id: str | None
    fill_index: int | None
    condition_hash: str | None
    accepted: bool | None
    receipt_code: str | None
    # payment_v2 memo contents: one dict per memo with the signed hex
    # memo_type/memo_format/memo_data and, when valid UTF-8, *_text readings.
    # None on every other transaction kind.
    memos: tuple[dict[str, str], ...] | None = field(default=None, hash=False)

    def to_dict(self) -> dict[str, Any]:
        return {
            "tx_id": self.tx_id,
            "block_height": self.block_height,
            "batch_kind": self.batch_kind,
            "batch_id": self.batch_id,
            "transaction_index": self.transaction_index,
            "transaction_kind": self.transaction_kind,
            "from": self.sender,
            "to": self.recipient,
            "amount": self.amount,
            "fee": self.fee,
            "sequence": self.sequence,
            "memo_hash": self.memo_hash,
            "memo_count": self.memo_count,
            "memo_bytes": self.memo_bytes,
            "memos": None if self.memos is None else [dict(memo) for memo in self.memos],
            "asset_id": self.asset_id,
            "issuer": self.issuer,
            "trustline_authorized": self.trustline_authorized,
            "trustline_frozen": self.trustline_frozen,
            "nft_id": self.nft_id,
            "nft_issuer_transfer_fee": self.nft_issuer_transfer_fee,
            "nft_collection_flags": self.nft_collection_flags,
            "escrow_id": self.escrow_id,
            "offer_id": self.offer_id,
            "tx_role": self.tx_role,
            "counterparty_offer_id": self.counterparty_offer_id,
            "fill_index": self.fill_index,
            "condition_hash": self.condition_hash,
            "accepted": self.accepted,
            "receipt_code": self.receipt_code,
        }


@dataclass(frozen=True)
class AccountTxScan:
    address: str
    from_height: int | None
    to_height: int | None
    scan_limit: int
    index_used: bool
    scanned_block_count: int
    archive_lookup_count: int
    truncated: bool
    rows: tuple[AccountTxRow, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "address": self.address,
            "from_height": self.from_height,
            "to_height": self.to_height,
            "scan_limit": self.scan_limit,
            "index_used": self.index_used,
            "scanned_block_count": self.scanned_block_count,
            "archive_lookup_count": self.archive_lookup_count,
            "truncated": self.truncated,
            "row_count": len(self.rows),
            "rows": [row.to_dict() for row in self.rows],
        }


@dataclass(frozen=True)
class AccountTxHistory:
    address: str
    from_height: int
    to_height: int
    window_size: int
    limit_per_window: int
    max_windows: int
    window_count: int
    max_windows_exceeded: bool
    truncated_window_count: int
    all_index_used: bool
    total_scanned_block_count: int
    total_archive_lookup_count: int
    rows: tuple[AccountTxRow, ...]
    windows: tuple[dict[str, Any], ...]

    @property
    def complete(self) -> bool:
        return not self.max_windows_exceeded and self.truncated_window_count == 0

    def to_dict(self) -> dict[str, Any]:
        return {
            "address": self.address,
            "from_height": self.from_height,
            "to_height": self.to_height,
            "window_size": self.window_size,
            "limit_per_window": self.limit_per_window,
            "max_windows": self.max_windows,
            "window_count": self.window_count,
            "max_windows_exceeded": self.max_windows_exceeded,
            "truncated_window_count": self.truncated_window_count,
            "all_index_used": self.all_index_used,
            "total_scanned_block_count": self.total_scanned_block_count,
            "total_archive_lookup_count": self.total_archive_lookup_count,
            "complete": self.complete,
            "row_count": len(self.rows),
            "rows": [row.to_dict() for row in self.rows],
            "windows": list(self.windows),
        }


@dataclass(frozen=True)
class AccountTxIndexStatus:
    chain_id: str | None
    genesis_hash: str | None
    protocol_version: int | None
    index_path: str
    disk_index_path: str | None
    index_present: bool
    index_usable: bool
    reason: str | None
    disk_index_present: bool
    disk_index_usable: bool
    disk_index_reason: str | None
    indexed_from_height: int | None
    indexed_to_height: int | None
    indexed_block_count: int
    indexed_row_count: int
    account_count: int
    disk_account_shard_count: int
    tip_hash: str | None
    current_tip_hash: str | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "chain_id": self.chain_id,
            "genesis_hash": self.genesis_hash,
            "protocol_version": self.protocol_version,
            "index_path": self.index_path,
            "disk_index_path": self.disk_index_path,
            "index_present": self.index_present,
            "index_usable": self.index_usable,
            "reason": self.reason,
            "disk_index_present": self.disk_index_present,
            "disk_index_usable": self.disk_index_usable,
            "disk_index_reason": self.disk_index_reason,
            "indexed_from_height": self.indexed_from_height,
            "indexed_to_height": self.indexed_to_height,
            "indexed_block_count": self.indexed_block_count,
            "indexed_row_count": self.indexed_row_count,
            "account_count": self.account_count,
            "disk_account_shard_count": self.disk_account_shard_count,
            "tip_hash": self.tip_hash,
            "current_tip_hash": self.current_tip_hash,
        }


class PostFiatRpcClient:
    def __init__(
        self,
        endpoint: Endpoint | str,
        *,
        timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
        response_byte_cap: int = DEFAULT_RESPONSE_BYTE_CAP,
    ) -> None:
        self.endpoint = Endpoint.parse(endpoint) if isinstance(endpoint, str) else endpoint
        if timeout_seconds <= 0:
            raise ValueError("timeout_seconds must be positive")
        if response_byte_cap < 1024:
            raise ValueError("response_byte_cap must be at least 1024")
        self.timeout_seconds = timeout_seconds
        self.response_byte_cap = response_byte_cap
        self._request_counter = 0

    def status(self) -> dict[str, Any]:
        return self._call("status")

    def server_info(self) -> dict[str, Any]:
        return self._call("server_info")

    def server_capabilities(self) -> dict[str, Any]:
        """Discover RPC write/finality capabilities.

        Returns a dict with keys:
        - ``read_only``: bool — True if write methods are disabled.
        - ``mempool_submit_enabled``: bool — True if mempool submit is allowed.
        - ``mempool_submit_finality_enabled``: bool — True if finality RPC is allowed.
        - ``block_height``: int — Current block height.
        - ``mempool_pending``: int — Pending mempool transactions.
        - ``chain_id``: str — Chain identifier.
        - ``validator_count``: int — Number of active validators.
        """
        info = self._call("server_info")
        status = self._call("status")
        rpc_section = info.get("rpc", {}) if isinstance(info, dict) else {}
        mempool_section = info.get("mempool", {}) if isinstance(info, dict) else {}
        return {
            "read_only": rpc_section.get("read_only", True),
            "mempool_submit_enabled": rpc_section.get("mempool_submit_enabled", False),
            "mempool_submit_finality_enabled": rpc_section.get(
                "mempool_submit_finality_enabled", False
            ),
            "fastpay_bridge_enabled": rpc_section.get("fastpay_bridge_enabled", False),
            "fastpay_bridge_mode": rpc_section.get("fastpay_bridge_mode", ""),
            "fastpay_owned_apply_broadcast_enabled": rpc_section.get(
                "fastpay_owned_apply_broadcast_enabled", False
            ),
            "block_height": status.get("block_height", 0),
            "mempool_pending": status.get("mempool_pending", 0),
            "chain_id": status.get("chain_id", ""),
            "genesis_hash": status.get("genesis_hash", ""),
            "protocol_version": status.get("protocol_version", 0),
            "validator_count": status.get("validator_count", 0),
            "last_run_unix": status.get("last_run_unix", 0),
        }

    def ledger(self, *, limit: int | None = None) -> dict[str, Any]:
        return self._call("ledger", self._limit_params(limit))

    def fee(self) -> dict[str, Any]:
        return self._call("fee")

    def transfer_fee_quote(
        self,
        from_address: str,
        to_address: str,
        amount: int,
        *,
        sequence: int | None = None,
        memo_type: str | None = None,
        memo_format: str | None = None,
        memo_data: str | None = None,
    ) -> dict[str, Any]:
        if not from_address:
            raise ValueError("from_address is required")
        if not to_address:
            raise ValueError("to_address is required")
        if amount < 1:
            raise ValueError("amount must be positive")
        if sequence is not None and sequence < 1:
            raise ValueError("sequence must be positive")
        params: dict[str, Any] = {
            "from": from_address,
            "to": to_address,
            "amount": amount,
        }
        if sequence is not None:
            params["sequence"] = sequence
        if memo_type is not None:
            params["memo_type"] = memo_type
        if memo_format is not None:
            params["memo_format"] = memo_format
        if memo_data is not None:
            params["memo_data"] = memo_data
        return self._call("transfer_fee_quote", params)

    def transfer_fee_quote_response(
        self,
        from_address: str,
        to_address: str,
        amount: int,
        *,
        sequence: int | None = None,
        memo_type: str | None = None,
        memo_format: str | None = None,
        memo_data: str | None = None,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if not from_address:
            raise ValueError("from_address is required")
        if not to_address:
            raise ValueError("to_address is required")
        if amount < 1:
            raise ValueError("amount must be positive")
        if sequence is not None and sequence < 1:
            raise ValueError("sequence must be positive")
        params: dict[str, Any] = {
            "from": from_address,
            "to": to_address,
            "amount": amount,
        }
        if sequence is not None:
            params["sequence"] = sequence
        if memo_type is not None:
            params["memo_type"] = memo_type
        if memo_format is not None:
            params["memo_format"] = memo_format
        if memo_data is not None:
            params["memo_data"] = memo_data
        return self.call_response("transfer_fee_quote", params, request_id=request_id)

    def asset_fee_quote(
        self,
        source: str,
        operation: dict[str, Any] | str,
        *,
        sequence: int | None = None,
    ) -> dict[str, Any]:
        if not source:
            raise ValueError("source is required")
        if isinstance(operation, str):
            operation_json = operation
        else:
            operation_json = json.dumps(operation, separators=(",", ":"))
        if not operation_json.strip():
            raise ValueError("operation must not be empty")
        if sequence is not None and sequence < 1:
            raise ValueError("sequence must be positive")
        params: dict[str, Any] = {
            "source": source,
            "operation_json": operation_json,
        }
        if sequence is not None:
            params["sequence"] = sequence
        return self._call("asset_fee_quote", params)

    def asset_fee_quote_response(
        self,
        source: str,
        operation: dict[str, Any] | str,
        *,
        sequence: int | None = None,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if not source:
            raise ValueError("source is required")
        if isinstance(operation, str):
            operation_json = operation
        else:
            operation_json = json.dumps(operation, separators=(",", ":"))
        if not operation_json.strip():
            raise ValueError("operation must not be empty")
        if sequence is not None and sequence < 1:
            raise ValueError("sequence must be positive")
        params: dict[str, Any] = {
            "source": source,
            "operation_json": operation_json,
        }
        if sequence is not None:
            params["sequence"] = sequence
        return self.call_response("asset_fee_quote", params, request_id=request_id)

    def escrow_fee_quote(
        self,
        source: str,
        operation: dict[str, Any] | str,
        *,
        sequence: int | None = None,
    ) -> dict[str, Any]:
        if not source:
            raise ValueError("source is required")
        if isinstance(operation, str):
            operation_json = operation
        else:
            operation_json = json.dumps(operation, separators=(",", ":"))
        if not operation_json.strip():
            raise ValueError("operation must not be empty")
        if sequence is not None and sequence < 1:
            raise ValueError("sequence must be positive")
        params: dict[str, Any] = {
            "source": source,
            "operation_json": operation_json,
        }
        if sequence is not None:
            params["sequence"] = sequence
        return self._call("escrow_fee_quote", params)

    def escrow_fee_quote_response(
        self,
        source: str,
        operation: dict[str, Any] | str,
        *,
        sequence: int | None = None,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if not source:
            raise ValueError("source is required")
        if isinstance(operation, str):
            operation_json = operation
        else:
            operation_json = json.dumps(operation, separators=(",", ":"))
        if not operation_json.strip():
            raise ValueError("operation must not be empty")
        if sequence is not None and sequence < 1:
            raise ValueError("sequence must be positive")
        params: dict[str, Any] = {
            "source": source,
            "operation_json": operation_json,
        }
        if sequence is not None:
            params["sequence"] = sequence
        return self.call_response("escrow_fee_quote", params, request_id=request_id)

    def nft_fee_quote(
        self,
        source: str,
        operation: dict[str, Any] | str,
        *,
        sequence: int | None = None,
    ) -> dict[str, Any]:
        if not source:
            raise ValueError("source is required")
        if isinstance(operation, str):
            operation_json = operation
        else:
            operation_json = json.dumps(operation, separators=(",", ":"))
        if not operation_json.strip():
            raise ValueError("operation must not be empty")
        if sequence is not None and sequence < 1:
            raise ValueError("sequence must be positive")
        params: dict[str, Any] = {
            "source": source,
            "operation_json": operation_json,
        }
        if sequence is not None:
            params["sequence"] = sequence
        return self._call("nft_fee_quote", params)

    def nft_fee_quote_response(
        self,
        source: str,
        operation: dict[str, Any] | str,
        *,
        sequence: int | None = None,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if not source:
            raise ValueError("source is required")
        if isinstance(operation, str):
            operation_json = operation
        else:
            operation_json = json.dumps(operation, separators=(",", ":"))
        if not operation_json.strip():
            raise ValueError("operation must not be empty")
        if sequence is not None and sequence < 1:
            raise ValueError("sequence must be positive")
        params: dict[str, Any] = {
            "source": source,
            "operation_json": operation_json,
        }
        if sequence is not None:
            params["sequence"] = sequence
        return self.call_response("nft_fee_quote", params, request_id=request_id)

    def offer_fee_quote(
        self,
        source: str,
        operation: dict[str, Any] | str,
        *,
        sequence: int | None = None,
    ) -> dict[str, Any]:
        if not source:
            raise ValueError("source is required")
        if isinstance(operation, str):
            operation_json = operation
        else:
            operation_json = json.dumps(operation, separators=(",", ":"))
        if not operation_json.strip():
            raise ValueError("operation must not be empty")
        if sequence is not None and sequence < 1:
            raise ValueError("sequence must be positive")
        params: dict[str, Any] = {
            "source": source,
            "operation_json": operation_json,
        }
        if sequence is not None:
            params["sequence"] = sequence
        return self._call("offer_fee_quote", params)

    def offer_fee_quote_response(
        self,
        source: str,
        operation: dict[str, Any] | str,
        *,
        sequence: int | None = None,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if not source:
            raise ValueError("source is required")
        if isinstance(operation, str):
            operation_json = operation
        else:
            operation_json = json.dumps(operation, separators=(",", ":"))
        if not operation_json.strip():
            raise ValueError("operation must not be empty")
        if sequence is not None and sequence < 1:
            raise ValueError("sequence must be positive")
        params: dict[str, Any] = {
            "source": source,
            "operation_json": operation_json,
        }
        if sequence is not None:
            params["sequence"] = sequence
        return self.call_response("offer_fee_quote", params, request_id=request_id)

    def atomic_settlement_template(
        self,
        *,
        left_owner: str,
        left_recipient: str,
        left_asset_id: str,
        left_amount: int,
        right_owner: str,
        right_recipient: str,
        right_asset_id: str,
        right_amount: int,
        condition: str,
        cancel_after: int,
        finish_after: int = 0,
        left_sequence: int | None = None,
        right_sequence: int | None = None,
    ) -> dict[str, Any]:
        for name, value in [
            ("left_owner", left_owner),
            ("left_recipient", left_recipient),
            ("left_asset_id", left_asset_id),
            ("right_owner", right_owner),
            ("right_recipient", right_recipient),
            ("right_asset_id", right_asset_id),
            ("condition", condition),
        ]:
            if not value:
                raise ValueError(f"{name} is required")
        if left_amount < 1:
            raise ValueError("left_amount must be positive")
        if right_amount < 1:
            raise ValueError("right_amount must be positive")
        if finish_after < 0:
            raise ValueError("finish_after must be non-negative")
        if cancel_after < 1:
            raise ValueError("cancel_after must be positive")
        if left_sequence is not None and left_sequence < 1:
            raise ValueError("left_sequence must be positive")
        if right_sequence is not None and right_sequence < 1:
            raise ValueError("right_sequence must be positive")
        params: dict[str, Any] = {
            "left_owner": left_owner,
            "left_recipient": left_recipient,
            "left_asset_id": left_asset_id,
            "left_amount": left_amount,
            "right_owner": right_owner,
            "right_recipient": right_recipient,
            "right_asset_id": right_asset_id,
            "right_amount": right_amount,
            "condition": condition,
            "finish_after": finish_after,
            "cancel_after": cancel_after,
        }
        if left_sequence is not None:
            params["left_sequence"] = left_sequence
        if right_sequence is not None:
            params["right_sequence"] = right_sequence
        result = self._call("atomic_settlement_template", params)
        if not isinstance(result, dict):
            raise RpcProtocolError("atomic_settlement_template result must be an object")
        return result

    def offer_info(self, offer_id: str) -> dict[str, Any]:
        if not offer_id:
            raise ValueError("offer_id is required")
        result = self._call("offer_info", {"offer_id": offer_id})
        return self._bound_info_result("offer_info", result, "offer_id", "offer", offer_id)

    def account_offers(
        self,
        account: str,
        *,
        state: str | None = None,
        limit: int | None = None,
    ) -> dict[str, Any]:
        if not account:
            raise ValueError("account is required")
        params: dict[str, Any] = {"account": account}
        if state is not None:
            if state not in {"open", "filled", "canceled", "unfunded"}:
                raise ValueError("state must be open, filled, canceled, or unfunded")
            params["state"] = state
        if limit is not None:
            params["limit"] = self._bounded_limit(limit)
        result = self._call("account_offers", params)
        if not isinstance(result, dict):
            raise RpcProtocolError("account_offers result must be an object")
        return self._bound_account_collection_result(
            "account_offers", result, account, state=state
        )

    def book_offers(
        self,
        taker_gets_asset_id: str,
        taker_pays_asset_id: str,
        *,
        limit: int | None = None,
    ) -> dict[str, Any]:
        if not taker_gets_asset_id:
            raise ValueError("taker_gets_asset_id is required")
        if not taker_pays_asset_id:
            raise ValueError("taker_pays_asset_id is required")
        if taker_gets_asset_id == taker_pays_asset_id:
            raise ValueError("DEX asset ids must differ")
        params: dict[str, Any] = {
            "taker_gets_asset_id": taker_gets_asset_id,
            "taker_pays_asset_id": taker_pays_asset_id,
        }
        if limit is not None:
            params["limit"] = self._bounded_limit(limit)
        result = self._call("book_offers", params)
        if not isinstance(result, dict):
            raise RpcProtocolError("book_offers result must be an object")
        return result

    def asset_info(self, asset_id: str) -> dict[str, Any]:
        if not asset_id:
            raise ValueError("asset_id is required")
        result = self._call("asset_info", {"asset_id": asset_id})
        return self._bound_info_result("asset_info", result, "asset_id", "asset", asset_id)

    def account_lines(
        self,
        account: str,
        *,
        issuer: str | None = None,
        asset_id: str | None = None,
        limit: int | None = None,
    ) -> dict[str, Any]:
        if not account:
            raise ValueError("account is required")
        params: dict[str, Any] = {"account": account}
        if issuer is not None:
            if not issuer:
                raise ValueError("issuer must not be empty")
            params["issuer"] = issuer
        if asset_id is not None:
            if not asset_id:
                raise ValueError("asset_id must not be empty")
            params["asset_id"] = asset_id
        if limit is not None:
            params["limit"] = self._bounded_limit(limit)
        result = self._call("account_lines", params)
        if not isinstance(result, dict):
            raise RpcProtocolError("account_lines result must be an object")
        return result

    def account_assets(
        self,
        account: str,
        *,
        asset_id: str | None = None,
        limit: int | None = None,
    ) -> dict[str, Any]:
        if not account:
            raise ValueError("account is required")
        params: dict[str, Any] = {"account": account}
        if asset_id is not None:
            if not asset_id:
                raise ValueError("asset_id must not be empty")
            params["asset_id"] = asset_id
        if limit is not None:
            params["limit"] = self._bounded_limit(limit)
        result = self._call("account_assets", params)
        if not isinstance(result, dict):
            raise RpcProtocolError("account_assets result must be an object")
        return result

    def issuer_assets(self, issuer: str, *, limit: int | None = None) -> dict[str, Any]:
        if not issuer:
            raise ValueError("issuer is required")
        params: dict[str, Any] = {"issuer": issuer}
        if limit is not None:
            params["limit"] = self._bounded_limit(limit)
        result = self._call("issuer_assets", params)
        if not isinstance(result, dict):
            raise RpcProtocolError("issuer_assets result must be an object")
        return result

    def escrow_info(self, escrow_id: str) -> dict[str, Any]:
        if not escrow_id:
            raise ValueError("escrow_id is required")
        result = self._call("escrow_info", {"escrow_id": escrow_id})
        return self._bound_info_result("escrow_info", result, "escrow_id", "escrow", escrow_id)

    def account_escrows(
        self,
        account: str,
        *,
        role: str | None = None,
        state: str | None = None,
        limit: int | None = None,
    ) -> dict[str, Any]:
        if not account:
            raise ValueError("account is required")
        params: dict[str, Any] = {"account": account}
        if role is not None:
            if role not in {"owner", "recipient"}:
                raise ValueError("role must be owner or recipient")
            params["role"] = role
        if state is not None:
            if state not in {"open", "finished", "canceled"}:
                raise ValueError("state must be open, finished, or canceled")
            params["state"] = state
        if limit is not None:
            params["limit"] = self._bounded_limit(limit)
        result = self._call("account_escrows", params)
        if not isinstance(result, dict):
            raise RpcProtocolError("account_escrows result must be an object")
        return self._bound_account_collection_result(
            "account_escrows", result, account, state=state, role=role
        )

    @staticmethod
    def _bound_account_collection_result(
        method: str,
        result: dict[str, Any],
        account: str,
        *,
        state: str | None,
        role: str | None = None,
    ) -> dict[str, Any]:
        if type(result.get("account")) is not str or result["account"] != account:
            raise RpcProtocolError(f"{method} result.account must match requested account")
        collection = "offers" if method == "account_offers" else "escrows"
        filters = {"role": role, "state": state} if collection == "escrows" else {"state": state}
        for field, requested in filters.items():
            value = result.get(field)
            matches = value is None if requested is None else type(value) is str and value == requested
            if field not in result or not matches:
                raise RpcProtocolError(f"{method} result.{field} must match requested {field}")
        rows = result.get(collection)
        if not isinstance(rows, list):
            raise RpcProtocolError(f"{method} result.{collection} must be an array")
        fields = ("owner", "recipient", "state") if collection == "escrows" else ("owner", "state")
        for index, row in enumerate(rows):
            label = f"{method} result.{collection}[{index}]"
            if not isinstance(row, dict):
                raise RpcProtocolError(f"{label} must be an object")
            for field in fields:
                if type(row.get(field)) is not str:
                    raise RpcProtocolError(f"{label}.{field} must be a string")
            if collection == "offers":
                if row["owner"] != account:
                    raise RpcProtocolError(f"{label}.owner must match requested account")
            elif role is not None:
                if row[role] != account:
                    raise RpcProtocolError(f"{label}.{role} must match requested account for role {role}")
            elif row["owner"] != account and row["recipient"] != account:
                raise RpcProtocolError(f"{label} must belong to requested account")
            if state is not None and row["state"] != state:
                raise RpcProtocolError(f"{label}.state must match requested state")
        return result

    def nft_info(self, nft_id: str) -> dict[str, Any]:
        if not nft_id:
            raise ValueError("nft_id is required")
        result = self._call("nft_info", {"nft_id": nft_id})
        return self._bound_info_result("nft_info", result, "nft_id", "nft", nft_id)

    def account_nfts(
        self,
        account: str,
        *,
        include_burned: bool = False,
        limit: int | None = None,
    ) -> dict[str, Any]:
        if not account:
            raise ValueError("account is required")
        params: dict[str, Any] = {"account": account}
        if include_burned:
            params["include_burned"] = True
        if limit is not None:
            params["limit"] = self._bounded_limit(limit)
        result = self._call("account_nfts", params)
        if not isinstance(result, dict):
            raise RpcProtocolError("account_nfts result must be an object")
        return result

    def issuer_nfts(
        self,
        issuer: str,
        *,
        collection_id: str | None = None,
        include_burned: bool = False,
        limit: int | None = None,
    ) -> dict[str, Any]:
        if not issuer:
            raise ValueError("issuer is required")
        params: dict[str, Any] = {"issuer": issuer}
        if collection_id is not None:
            if not collection_id:
                raise ValueError("collection_id must not be empty")
            params["collection_id"] = collection_id
        if include_burned:
            params["include_burned"] = True
        if limit is not None:
            params["limit"] = self._bounded_limit(limit)
        result = self._call("issuer_nfts", params)
        if not isinstance(result, dict):
            raise RpcProtocolError("issuer_nfts result must be an object")
        return result

    def mempool_submit_signed_transfer(
        self,
        signed_transfer: dict[str, Any] | str,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if isinstance(signed_transfer, str):
            signed_transfer_json = signed_transfer
        else:
            signed_transfer_json = json.dumps(signed_transfer, separators=(",", ":"))
        if not signed_transfer_json.strip():
            raise ValueError("signed_transfer must not be empty")
        result = self._call(
            "mempool_submit_signed_transfer",
            {"signed_transfer_json": signed_transfer_json},
            request_id=request_id,
        )
        if not isinstance(result, dict):
            raise RpcProtocolError("mempool_submit_signed_transfer result must be an object")
        return result

    def mempool_submit_signed_transfer_finality(
        self,
        signed_transfer: dict[str, Any] | str,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        """Submit a signed transfer with on-chain finality via peer-certified mempool round.

        This method requires the RPC server to have --allow-mempool-submit-finality enabled.
        The server runs a transport-peer-certified-mempool-round, which proposes a block,
        collects votes from a quorum of validators, and returns finality evidence.
        """
        if isinstance(signed_transfer, str):
            signed_transfer_json = signed_transfer
        else:
            signed_transfer_json = json.dumps(signed_transfer, separators=(",", ":"))
        if not signed_transfer_json.strip():
            raise ValueError("signed_transfer must not be empty")
        response = self.call_response(
            "mempool_submit_signed_transfer_finality",
            {"signed_transfer_json": signed_transfer_json},
            request_id=request_id,
        )
        result = response["result"]
        if not isinstance(result, dict):
            raise RpcProtocolError("mempool_submit_signed_transfer_finality result must be an object")
        proxy_route = response.get("proxy_route")
        if isinstance(proxy_route, dict):
            result = dict(result)
            result["proxy_route"] = proxy_route
        return result

    def mempool_submit_signed_payment_v2(
        self,
        signed_payment_v2: dict[str, Any] | str,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if isinstance(signed_payment_v2, str):
            signed_payment_v2_json = signed_payment_v2
        else:
            signed_payment_v2_json = json.dumps(signed_payment_v2, separators=(",", ":"))
        if not signed_payment_v2_json.strip():
            raise ValueError("signed_payment_v2 must not be empty")
        result = self._call(
            "mempool_submit_signed_payment_v2",
            {"signed_payment_v2_json": signed_payment_v2_json},
            request_id=request_id,
        )
        if not isinstance(result, dict):
            raise RpcProtocolError("mempool_submit_signed_payment_v2 result must be an object")
        return result

    def mempool_submit_signed_payment_v2_finality(
        self,
        signed_payment_v2: dict[str, Any] | str,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        """Submit a signed payment v2 with on-chain finality via peer-certified mempool round."""
        if isinstance(signed_payment_v2, str):
            signed_payment_v2_json = signed_payment_v2
        else:
            signed_payment_v2_json = json.dumps(signed_payment_v2, separators=(",", ":"))
        if not signed_payment_v2_json.strip():
            raise ValueError("signed_payment_v2 must not be empty")
        response = self.call_response(
            "mempool_submit_signed_payment_v2_finality",
            {"signed_payment_v2_json": signed_payment_v2_json},
            request_id=request_id,
        )
        result = response["result"]
        if not isinstance(result, dict):
            raise RpcProtocolError(
                "mempool_submit_signed_payment_v2_finality result must be an object"
            )
        proxy_route = response.get("proxy_route")
        if isinstance(proxy_route, dict):
            result = dict(result)
            result["proxy_route"] = proxy_route
        return result

    def mempool_submit_signed_asset_transaction(
        self,
        signed_asset_transaction: dict[str, Any] | str,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if isinstance(signed_asset_transaction, str):
            signed_asset_transaction_json = signed_asset_transaction
        else:
            signed_asset_transaction_json = json.dumps(
                signed_asset_transaction, separators=(",", ":")
            )
        if not signed_asset_transaction_json.strip():
            raise ValueError("signed_asset_transaction must not be empty")
        result = self._call(
            "mempool_submit_signed_asset_transaction",
            {"signed_asset_transaction_json": signed_asset_transaction_json},
            request_id=request_id,
        )
        if not isinstance(result, dict):
            raise RpcProtocolError(
                "mempool_submit_signed_asset_transaction result must be an object"
            )
        return result

    def mempool_submit_signed_escrow_transaction(
        self,
        signed_escrow_transaction: dict[str, Any] | str,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if isinstance(signed_escrow_transaction, str):
            signed_escrow_transaction_json = signed_escrow_transaction
        else:
            signed_escrow_transaction_json = json.dumps(
                signed_escrow_transaction, separators=(",", ":")
            )
        if not signed_escrow_transaction_json.strip():
            raise ValueError("signed_escrow_transaction must not be empty")
        result = self._call(
            "mempool_submit_signed_escrow_transaction",
            {"signed_escrow_transaction_json": signed_escrow_transaction_json},
            request_id=request_id,
        )
        if not isinstance(result, dict):
            raise RpcProtocolError(
                "mempool_submit_signed_escrow_transaction result must be an object"
            )
        return result

    def mempool_submit_signed_escrow_transaction_finality(
        self,
        signed_escrow_transaction: dict[str, Any] | str,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if isinstance(signed_escrow_transaction, str):
            signed_escrow_transaction_json = signed_escrow_transaction
        else:
            signed_escrow_transaction_json = json.dumps(
                signed_escrow_transaction, separators=(",", ":")
            )
        if not signed_escrow_transaction_json.strip():
            raise ValueError("signed_escrow_transaction must not be empty")
        result = self._call(
            "mempool_submit_signed_escrow_transaction_finality",
            {"signed_escrow_transaction_json": signed_escrow_transaction_json},
            request_id=request_id,
        )
        if not isinstance(result, dict):
            raise RpcProtocolError(
                "mempool_submit_signed_escrow_transaction_finality result must be an object"
            )
        return result

    def mempool_submit_signed_nft_transaction(
        self,
        signed_nft_transaction: dict[str, Any] | str,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if isinstance(signed_nft_transaction, str):
            signed_nft_transaction_json = signed_nft_transaction
        else:
            signed_nft_transaction_json = json.dumps(
                signed_nft_transaction, separators=(",", ":")
            )
        if not signed_nft_transaction_json.strip():
            raise ValueError("signed_nft_transaction must not be empty")
        result = self._call(
            "mempool_submit_signed_nft_transaction",
            {"signed_nft_transaction_json": signed_nft_transaction_json},
            request_id=request_id,
        )
        if not isinstance(result, dict):
            raise RpcProtocolError(
                "mempool_submit_signed_nft_transaction result must be an object"
            )
        return result

    def mempool_submit_signed_offer_transaction(
        self,
        signed_offer_transaction: dict[str, Any] | str,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if isinstance(signed_offer_transaction, str):
            signed_offer_transaction_json = signed_offer_transaction
        else:
            signed_offer_transaction_json = json.dumps(
                signed_offer_transaction, separators=(",", ":")
            )
        if not signed_offer_transaction_json.strip():
            raise ValueError("signed_offer_transaction must not be empty")
        result = self._call(
            "mempool_submit_signed_offer_transaction",
            {"signed_offer_transaction_json": signed_offer_transaction_json},
            request_id=request_id,
        )
        if not isinstance(result, dict):
            raise RpcProtocolError(
                "mempool_submit_signed_offer_transaction result must be an object"
            )
        return result

    def shield_batch_orchard_deposit(
        self,
        deposit: dict[str, Any] | str,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        if isinstance(deposit, str):
            deposit_json = deposit
        else:
            deposit_json = json.dumps(deposit, separators=(",", ":"))
        if not deposit_json.strip():
            raise ValueError("deposit must not be empty")
        result = self._call(
            "shield_batch_orchard_deposit",
            {"deposit_json": deposit_json},
            request_id=request_id,
        )
        if not isinstance(result, dict):
            raise RpcProtocolError("shield_batch_orchard_deposit result must be an object")
        return result

    def validators(self) -> dict[str, Any]:
        return self._call("validators")

    def owned_objects(
        self,
        owner_public_key_hex: str,
        *,
        asset: str | None = None,
        limit: int | None = None,
    ) -> dict[str, Any]:
        params: dict[str, Any] = {"owner_public_key_hex": owner_public_key_hex}
        if asset is not None:
            params["asset"] = asset
        if limit is not None:
            params["limit"] = limit
        return self._call("owned_objects", params)

    def owned_sign(
        self,
        order_json: str,
        validator_id: str,
    ) -> dict[str, Any]:
        """Submit an owned-transfer order to a validator for signing (FastPay vote)."""
        return self._call(
            "owned_sign",
            {"order_json": order_json, "validator_id": validator_id},
        )

    def owned_apply(self, cert_json: str) -> dict[str, Any]:
        """Apply a certified owned-transfer to the ledger (FastPay landing)."""
        return self._call("owned_apply", {"cert_json": cert_json})

    def owned_recovery_capabilities(self) -> dict[str, Any]:
        """Read the governed FastPay v3 domain, committee, and recovery window."""
        return self._call("owned_recovery_capabilities")

    def owned_recovery_status(self, lock_id: str) -> dict[str, Any]:
        """Read the FastPay v3 recovery fence and reveal status for a lock."""
        if not lock_id:
            raise ValueError("lock_id is required")
        return self._call("owned_recovery_status", {"lock_id": lock_id})

    def owned_certificate(
        self,
        *,
        lock_id: str | None = None,
        certificate_digest: str | None = None,
    ) -> dict[str, Any]:
        """Read a retained FastPay certificate by exactly one selector.

        The node treats ``lock_id`` and ``certificate_digest`` as alternatives;
        sending both is rejected client-side so the request can never be
        answered from the wrong selector.
        """
        if (lock_id is None) == (certificate_digest is None):
            raise ValueError(
                "owned_certificate takes exactly one of lock_id or certificate_digest"
            )
        if lock_id is not None:
            if not lock_id:
                raise ValueError("lock_id must not be empty")
            return self._call("owned_certificate", {"lock_id": lock_id})
        if not certificate_digest:
            raise ValueError("certificate_digest must not be empty")
        return self._call(
            "owned_certificate", {"certificate_digest": certificate_digest}
        )

    def owned_sign_v3(
        self,
        order_json: str,
        validator_id: str,
    ) -> dict[str, Any]:
        """Request a validator vote for a recovery-safe owned transfer."""
        return self._call(
            "owned_sign_v3",
            {"order_json": order_json, "validator_id": validator_id},
        )

    def owned_apply_v3(self, cert_json: str) -> dict[str, Any]:
        """Broadcast a recovery-safe transfer certificate for durable apply."""
        return self._call("owned_apply_v3", {"cert_json": cert_json})

    def owned_unwrap_sign(
        self,
        order_json: str,
        validator_id: str,
    ) -> dict[str, Any]:
        """Submit an owned-unwrap order to a validator for signing."""
        return self._call(
            "owned_unwrap_sign",
            {"order_json": order_json, "validator_id": validator_id},
        )

    def owned_unwrap_apply(self, cert_json: str) -> dict[str, Any]:
        """Apply a certified owned-unwrap to the ledger."""
        return self._call("owned_unwrap_apply", {"cert_json": cert_json})

    def owned_unwrap_sign_v3(
        self,
        order_json: str,
        validator_id: str,
    ) -> dict[str, Any]:
        """Request a validator vote for a recovery-safe owned unwrap."""
        return self._call(
            "owned_unwrap_sign_v3",
            {"order_json": order_json, "validator_id": validator_id},
        )

    def owned_unwrap_apply_v3(self, cert_json: str) -> dict[str, Any]:
        """Broadcast a recovery-safe unwrap certificate for durable apply."""
        return self._call("owned_unwrap_apply_v3", {"cert_json": cert_json})

    def mempool_submit_fastlane_primary(
        self,
        transaction: dict[str, Any] | str,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        """Submit a locally signed, consensus-ordered FastLane transaction."""
        transaction_json = (
            transaction
            if isinstance(transaction, str)
            else json.dumps(transaction, separators=(",", ":"))
        )
        if not transaction_json.strip():
            raise ValueError("fastlane primary transaction must not be empty")
        result = self._call(
            "mempool_submit_fastlane_primary",
            {"fastlane_primary_json": transaction_json},
            request_id=request_id,
        )
        if not isinstance(result, dict):
            raise RpcProtocolError("mempool_submit_fastlane_primary result must be an object")
        return result

    def mempool_submit_fastlane_primary_finality(
        self,
        transaction: dict[str, Any] | str,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        """Submit and finalize a signed FastLane primary transaction."""
        transaction_json = (
            transaction
            if isinstance(transaction, str)
            else json.dumps(transaction, separators=(",", ":"))
        )
        if not transaction_json.strip():
            raise ValueError("fastlane primary transaction must not be empty")
        result = self._call(
            "mempool_submit_fastlane_primary_finality",
            {"fastlane_primary_json": transaction_json},
            request_id=request_id,
        )
        if not isinstance(result, dict):
            raise RpcProtocolError(
                "mempool_submit_fastlane_primary_finality result must be an object"
            )
        return result

    def manifests(self) -> dict[str, Any]:
        return self._call("manifests")

    def metrics(self) -> dict[str, Any]:
        return self._call("metrics")

    def blocks(
        self,
        *,
        from_height: int | None = None,
        limit: int | None = None,
    ) -> list[dict[str, Any]]:
        params = self._limit_params(limit)
        if from_height is not None:
            if from_height < 0:
                raise ValueError("from_height must be non-negative")
            params["from_height"] = from_height
        result = self._call("blocks", params)
        if not isinstance(result, list):
            raise RpcProtocolError("blocks result must be a list")
        return result

    def receipts(
        self,
        *,
        tx_id: str | None = None,
        limit: int | None = None,
    ) -> list[dict[str, Any]]:
        params = self._limit_params(limit)
        if tx_id is not None:
            params["tx_id"] = tx_id
        result = self._call("receipts", params)
        if not isinstance(result, list):
            raise RpcProtocolError("receipts result must be a list")
        return result

    def tx(self, tx_id: str, *, audit_block_log: bool = False) -> dict[str, Any]:
        if not tx_id:
            raise ValueError("tx_id is required")
        params: dict[str, Any] = {"tx_id": tx_id}
        if audit_block_log:
            params["audit_block_log"] = True
        return self._call("tx", params)

    def account(self, address: str) -> dict[str, Any]:
        if not address:
            raise ValueError("address is required")
        return self._call("account", {"address": address})

    def mempool_status(self) -> dict[str, Any]:
        return self._call("mempool_status")

    def bridge_status(self) -> dict[str, Any]:
        return self._call("bridge_status")

    def navcoin_bridge_routes(self) -> dict[str, Any]:
        return self._call("navcoin_bridge_routes")

    def navcoin_bridge_packet(
        self,
        route_id: str,
        packet_hash: str,
    ) -> dict[str, Any]:
        if not route_id:
            raise ValueError("route_id is required")
        if not packet_hash:
            raise ValueError("packet_hash is required")
        return self._call(
            "navcoin_bridge_packet",
            {"route_id": route_id, "packet_hash": packet_hash},
        )

    def navcoin_bridge_claims(
        self,
        route_id: str,
        *,
        limit: int | None = None,
        include_terminal: bool = False,
    ) -> dict[str, Any]:
        if not route_id:
            raise ValueError("route_id is required")
        params: dict[str, Any] = {"route_id": route_id}
        if limit is not None:
            params["limit"] = self._bounded_limit(limit)
        if include_terminal:
            params["include_terminal"] = True
        return self._call("navcoin_bridge_claims", params)

    def navcoin_bridge_supply_status(self, route_id: str) -> dict[str, Any]:
        if not route_id:
            raise ValueError("route_id is required")
        return self._call("navcoin_bridge_supply_status", {"route_id": route_id})

    def navcoin_bridge_receipt_replay(self, route_id: str) -> dict[str, Any]:
        if not route_id:
            raise ValueError("route_id is required")
        return self._call("navcoin_bridge_receipt_replay", {"route_id": route_id})

    def shield_turnstile(self) -> dict[str, Any]:
        return self._call("shield_turnstile")

    def shield_batch_finality(
        self,
        batch_json: str,
        *,
        required_current_height: int | None = None,
        required_parent_hash: str | None = None,
        required_state_root: str | None = None,
    ) -> dict[str, Any]:
        """Certify a shielded batch using the remote validator's sole proposer key.

        The proxy parent pins are all-or-none around the height: a parent hash
        or state root only means something relative to ``required_current_height``
        and the node rejects a pin sent without it, so it is refused here instead
        of being sent.
        """
        if not isinstance(batch_json, str) or not batch_json.strip():
            raise ValueError("batch_json is required")
        if required_current_height is None:
            if required_parent_hash is not None:
                raise ValueError("required_parent_hash requires required_current_height")
            if required_state_root is not None:
                raise ValueError("required_state_root requires required_current_height")
        params: dict[str, Any] = {"batch_json": batch_json}
        if required_current_height is not None:
            if required_current_height < 0:
                raise ValueError("required_current_height must be non-negative")
            params["proxy_required_current_height"] = required_current_height
        if required_parent_hash is not None:
            if not required_parent_hash:
                raise ValueError("required_parent_hash must not be empty")
            params["proxy_required_parent_hash"] = required_parent_hash
        if required_state_root is not None:
            if not required_state_root:
                raise ValueError("required_state_root must not be empty")
            params["proxy_required_state_root"] = required_state_root
        return self._call("shield_batch_finality", params)

    def orchard_pool_report(self) -> dict[str, Any]:
        return self._call("orchard_pool_report")

    def account_tx_index_status(self) -> AccountTxIndexStatus:
        result = self._call("account_tx_index_status")
        if not isinstance(result, dict):
            raise RpcProtocolError("account_tx_index_status result must be an object")
        if result.get("schema") != "postfiat-account-tx-index-status-v1":
            raise RpcProtocolError("account_tx_index_status result has wrong schema")
        index_path = result.get("index_path")
        if not isinstance(index_path, str) or "/" in index_path or "\\" in index_path:
            raise RpcProtocolError("account_tx_index_status leaked a filesystem path")
        disk_index_path = result.get("disk_index_path")
        if disk_index_path is not None and (
            not isinstance(disk_index_path, str)
            or "/" in disk_index_path
            or "\\" in disk_index_path
        ):
            raise RpcProtocolError("account_tx_index_status leaked a disk filesystem path")
        return AccountTxIndexStatus(
            chain_id=result.get("chain_id") if isinstance(result.get("chain_id"), str) else None,
            genesis_hash=(
                result.get("genesis_hash")
                if isinstance(result.get("genesis_hash"), str)
                else None
            ),
            protocol_version=(
                result.get("protocol_version")
                if isinstance(result.get("protocol_version"), int)
                else None
            ),
            index_path=index_path,
            disk_index_path=disk_index_path if isinstance(disk_index_path, str) else None,
            index_present=bool(result.get("index_present", False)),
            index_usable=bool(result.get("index_usable", False)),
            reason=result.get("reason") if isinstance(result.get("reason"), str) else None,
            disk_index_present=bool(result.get("disk_index_present", False)),
            disk_index_usable=bool(result.get("disk_index_usable", False)),
            disk_index_reason=(
                result.get("disk_index_reason")
                if isinstance(result.get("disk_index_reason"), str)
                else None
            ),
            indexed_from_height=(
                result.get("indexed_from_height")
                if isinstance(result.get("indexed_from_height"), int)
                else None
            ),
            indexed_to_height=(
                result.get("indexed_to_height")
                if isinstance(result.get("indexed_to_height"), int)
                else None
            ),
            indexed_block_count=self._optional_int(result, "indexed_block_count", default=0),
            indexed_row_count=self._optional_int(result, "indexed_row_count", default=0),
            account_count=self._optional_int(result, "account_count", default=0),
            disk_account_shard_count=self._optional_int(
                result, "disk_account_shard_count", default=0
            ),
            tip_hash=result.get("tip_hash") if isinstance(result.get("tip_hash"), str) else None,
            current_tip_hash=(
                result.get("current_tip_hash")
                if isinstance(result.get("current_tip_hash"), str)
                else None
            ),
        )

    def batch_archive(
        self,
        *,
        batch_kind: str | None = None,
        batch_id: str | None = None,
        limit: int | None = None,
    ) -> list[dict[str, Any]]:
        params = self._limit_params(limit)
        if batch_kind is not None:
            params["batch_kind"] = batch_kind
        if batch_id is not None:
            params["batch_id"] = batch_id
        result = self._call("batch_archive", params)
        if not isinstance(result, list):
            raise RpcProtocolError("batch_archive result must be a list")
        return result

    def archive_window(
        self,
        from_height: int,
        to_height: int,
        *,
        archive_uri: str | None = None,
    ) -> dict[str, Any]:
        """Build a bounded history handoff bundle for an inclusive height window.

        Mirrors the node's `archive_window` read: both heights are
        non-negative, `to_height >= from_height`, and the window spans at most
        `MAX_ARCHIVE_WINDOW_BLOCKS` blocks, checked here so a bad range fails
        before a request is sent. The bundle is returned as the node encodes
        it; nothing is published.
        """
        for name, value in (("from_height", from_height), ("to_height", to_height)):
            if isinstance(value, bool) or not isinstance(value, int):
                raise ValueError(f"{name} must be an integer")
            if value < 0:
                raise ValueError(f"{name} must be non-negative")
        if to_height < from_height:
            raise ValueError("to_height must be >= from_height")
        if to_height - from_height + 1 > MAX_ARCHIVE_WINDOW_BLOCKS:
            raise ValueError(
                f"archive window must not exceed {MAX_ARCHIVE_WINDOW_BLOCKS} blocks"
            )
        params: dict[str, Any] = {"from_height": from_height, "to_height": to_height}
        if archive_uri is not None:
            if not archive_uri:
                raise ValueError("archive_uri must not be empty")
            params["archive_uri"] = archive_uri
        result = self._call("archive_window", params)
        if not isinstance(result, dict):
            raise RpcProtocolError("archive_window result must be an object")
        return result

    def _verification_report(self, method: str) -> dict[str, Any]:
        result = self._call(method, {})
        if not isinstance(result, dict):
            raise RpcProtocolError(f"{method} result must be an object")
        return result

    def verify_blocks(self) -> dict[str, Any]:
        """Re-verify the node's local block-log chain and signatures (public read)."""
        return self._verification_report("verify_blocks")

    def verify_state(self) -> dict[str, Any]:
        """Re-verify the aggregate local state, including block-log integrity."""
        return self._verification_report("verify_state")

    def verify_bridge(self) -> dict[str, Any]:
        """Re-verify local bridge-state invariants."""
        return self._verification_report("verify_bridge")

    def verify_mempool(self) -> dict[str, Any]:
        """Re-verify admitted mempool entries against local policy."""
        return self._verification_report("verify_mempool")

    def verify_shielded(self) -> dict[str, Any]:
        """Re-verify the local shielded-state commitment and accounting."""
        return self._verification_report("verify_shielded")

    @staticmethod
    def _required_text(value: object, field: str) -> str:
        if not isinstance(value, str) or not value:
            raise ValueError(f"{field} must be a non-empty string")
        return value

    @staticmethod
    def _bound_info_result(
        method: str,
        result: Any,
        id_field: str,
        object_field: str,
        requested_id: str,
    ) -> dict[str, Any]:
        """Bind a single-object ``*_info`` read to the id that was requested.

        The node echoes the queried id verbatim (it only accepts lowercase hex,
        so no case folding applies), reports ``found`` as a bool, and serializes
        the object as null when it is absent (``Option<...Report>``). Any other
        shape describes a different object or is malformed.
        """
        if not isinstance(result, dict):
            raise RpcProtocolError(f"{method} result must be an object")
        echoed_id = result.get(id_field)
        if type(echoed_id) is not str or echoed_id != requested_id:
            raise RpcProtocolError(f"{method} {id_field} does not match the requested id")
        found = result.get("found")
        if type(found) is not bool:
            raise RpcProtocolError(f"{method} found must be a boolean")
        if object_field not in result:
            raise RpcProtocolError(f"{method} {object_field} is missing")
        report = result[object_field]
        if not found:
            if report is not None:
                raise RpcProtocolError(f"{method} {object_field} must be null when found is false")
            return result
        if not isinstance(report, dict):
            raise RpcProtocolError(f"{method} {object_field} must be an object when found is true")
        report_id = report.get(id_field)
        if type(report_id) is not str or report_id != requested_id:
            raise RpcProtocolError(
                f"{method} {object_field}.{id_field} does not match the requested id"
            )
        return result

    def _object_read(self, method: str, params: dict[str, Any]) -> dict[str, Any]:
        result = self._call(method, params)
        if not isinstance(result, dict):
            raise RpcProtocolError(f"{method} result must be an object")
        return result

    def shield_scan(self, owner: str) -> list[dict[str, Any]]:
        """List the shielded notes visible to `owner` (public read, `--owner`)."""
        result = self._call("shield_scan", {"owner": self._required_text(owner, "owner")})
        if not isinstance(result, list):
            raise RpcProtocolError("shield_scan result must be a list")
        return result

    def shield_disclose(self, note_id: str) -> dict[str, Any]:
        """Disclose one shielded note by id (public read, `--note-id`)."""
        return self._object_read(
            "shield_disclose", {"note_id": self._required_text(note_id, "note_id")}
        )

    def vault_bridge_route(self, asset_id: str) -> dict[str, Any]:
        """Verify and report the governed vault bridge route for an asset."""
        return self._object_read(
            "vault_bridge_route", {"asset_id": self._required_text(asset_id, "asset_id")}
        )

    def market_ops_status(self, asset_id: str, *, epoch: int | None = None) -> dict[str, Any]:
        """Market-operations status for an asset, optionally pinned to an epoch."""
        params: dict[str, Any] = {"asset_id": self._required_text(asset_id, "asset_id")}
        if epoch is not None:
            if isinstance(epoch, bool) or not isinstance(epoch, int) or epoch < 0:
                raise ValueError("epoch must be a non-negative integer")
            params["epoch"] = epoch
        return self._object_read("market_ops_status", params)

    def asset_orchard_action_status(
        self,
        nullifiers: tuple[str, str] | list[str],
        output_commitments: tuple[str, str] | list[str],
    ) -> dict[str, Any]:
        """Status of one Asset-Orchard action's two nullifiers and two output commitments.

        The node takes exactly two of each (`--nullifier-1`, `--nullifier-2`,
        `--output-commitment-1`, `--output-commitment-2`).
        """
        params: dict[str, Any] = {}
        for prefix, values in (("nullifier", nullifiers), ("output_commitment", output_commitments)):
            if not isinstance(values, (list, tuple)) or len(values) != 2:
                raise ValueError(f"{prefix}s must hold exactly two values")
            for index, value in enumerate(values, start=1):
                params[f"{prefix}_{index}"] = self._required_text(value, f"{prefix}_{index}")
        return self._object_read("asset_orchard_action_status", params)

    @staticmethod
    def _required_u64(value: object, field: str) -> int:
        if isinstance(value, bool) or not isinstance(value, int) or value < 0:
            raise ValueError(f"{field} must be a non-negative integer")
        return value

    def nav_reserve_proof_status(self, asset_id: str) -> dict[str, Any]:
        """NAV reserve-proof status for an asset (public read, `--asset-id`)."""
        return self._object_read(
            "nav_reserve_proof_status",
            {"asset_id": self._required_text(asset_id, "asset_id")},
        )

    def vault_bridge_status(self, asset_id: str) -> dict[str, Any]:
        """Vault bridge status for an asset (public read, `--asset-id`)."""
        return self._object_read(
            "vault_bridge_status", {"asset_id": self._required_text(asset_id, "asset_id")}
        )

    def fx_fix_list(
        self,
        *,
        base_asset_id: str | None = None,
        quote_asset_id: str | None = None,
        active_only: bool = False,
        limit: int | None = None,
    ) -> dict[str, Any]:
        """List FX fix packets, optionally filtered by pair and activity.

        `active_only` is a presence flag on the node (`--active-only`), so it is
        sent only when true. `limit` is bounded like the other list reads.
        """
        params = self._limit_params(limit)
        if base_asset_id is not None:
            params["base_asset_id"] = self._required_text(base_asset_id, "base_asset_id")
        if quote_asset_id is not None:
            params["quote_asset_id"] = self._required_text(quote_asset_id, "quote_asset_id")
        if active_only:
            params["active_only"] = True
        return self._object_read("fx_fix_list", params)

    def fx_fix_info(self, fix_packet_hash: str) -> dict[str, Any]:
        """One FX fix packet by hash (public read, `--fix-packet-hash`)."""
        return self._object_read(
            "fx_fix_info",
            {"fix_packet_hash": self._required_text(fix_packet_hash, "fix_packet_hash")},
        )

    def fx_fix_reservation_info(self, reservation_id: str) -> dict[str, Any]:
        """One FX fix reservation by id (public read, `--reservation-id`)."""
        return self._object_read(
            "fx_fix_reservation_info",
            {"reservation_id": self._required_text(reservation_id, "reservation_id")},
        )

    def fx_fix_quote(self, fix_packet_hash: str, base_atoms: int) -> dict[str, Any]:
        """Quote `base_atoms` against an FX fix packet (public read).

        The node parses `--base-atoms` as a u64, so a negative, boolean or
        non-integer value is rejected here before any request.
        """
        return self._object_read(
            "fx_fix_quote",
            {
                "fix_packet_hash": self._required_text(fix_packet_hash, "fix_packet_hash"),
                "base_atoms": self._required_u64(base_atoms, "base_atoms"),
            },
        )

    def asset_orchard_pricing_claim(
        self, base_asset_id: str, quote_asset_id: str, inclusion_height: int
    ) -> dict[str, Any]:
        """Exact at-NAV Asset-Orchard pricing claim consensus accepts for the
        full base/quote asset ids at `inclusion_height` (public read).

        `status` is `ok` with `pricing_claim`, or `stale`/`halted`/`mismatch`
        with a `refusal` code and reason.
        """
        return self._object_read(
            "asset_orchard_pricing_claim",
            self._asset_orchard_pair_params(base_asset_id, quote_asset_id, inclusion_height),
        )

    def asset_orchard_private_egress_binding(
        self, base_asset_id: str, quote_asset_id: str, inclusion_height: int
    ) -> dict[str, Any]:
        """Governed private-egress policy/disclosure binding for each asset of
        the pair (public read); `ungoverned` when finalized state has none."""
        return self._object_read(
            "asset_orchard_private_egress_binding",
            self._asset_orchard_pair_params(base_asset_id, quote_asset_id, inclusion_height),
        )

    def private_pool_snapshot(
        self,
        pool_id: str,
        *,
        address: str | None = None,
        base_asset_id: str | None = None,
        quote_asset_id: str | None = None,
        inclusion_height: int | None = None,
    ) -> dict[str, Any]:
        """Pin one finalized private-pool snapshot (public read).

        Returns the `snapshot_token` every page and account-line read must
        name, the identity, counts, page limits, pricing claim, egress binding
        and completeness flags.
        """
        params: dict[str, Any] = {"pool_id": self._required_text(pool_id, "pool_id")}
        if address is not None:
            params["address"] = self._required_text(address, "address")
        if (base_asset_id is None) != (quote_asset_id is None):
            raise ValueError("base_asset_id and quote_asset_id must be given together")
        if base_asset_id is not None and quote_asset_id is not None:
            params["base_asset_id"] = self._required_text(base_asset_id, "base_asset_id")
            params["quote_asset_id"] = self._required_text(quote_asset_id, "quote_asset_id")
        if inclusion_height is not None:
            params["inclusion_height"] = self._required_u64(inclusion_height, "inclusion_height")
        return self._object_read("private_pool_snapshot", params)

    def private_pool_page(
        self,
        snapshot_token: str,
        section: str,
        *,
        cursor: str | None = None,
        limit: int | None = None,
    ) -> dict[str, Any]:
        """One bounded page of a snapshot section (public read).

        `status` is `ok` with `items` and `next_cursor`, or `refused` with a
        `refusal` code such as `snapshot_unavailable` or `cursor_malformed`.
        """
        params: dict[str, Any] = {
            "snapshot_token": self._required_text(snapshot_token, "snapshot_token"),
            "section": self._required_text(section, "section"),
        }
        if cursor is not None:
            params["cursor"] = self._required_text(cursor, "cursor")
        if limit is not None:
            if isinstance(limit, bool) or not isinstance(limit, int) or limit < 1:
                raise ValueError("limit must be a positive integer")
            params["limit"] = limit
        return self._object_read("private_pool_page", params)

    def private_account_lines(self, snapshot_token: str, address: str) -> dict[str, Any]:
        """The account's asset lines at a snapshot, zero balances included (public read)."""
        return self._object_read(
            "private_account_lines",
            {
                "snapshot_token": self._required_text(snapshot_token, "snapshot_token"),
                "address": self._required_text(address, "address"),
            },
        )

    def private_readiness(self, *, pool_id: str | None = None) -> dict[str, Any]:
        """Private-action readiness of the node (public read).

        `ready` is true only when both verifiers are built with their pinned
        identity, the governed activations are active at the next height and
        the governed A666/pfUSDC route exists; otherwise `reasons` says why.
        """
        params: dict[str, Any] = {}
        if pool_id is not None:
            params["pool_id"] = self._required_text(pool_id, "pool_id")
        return self._object_read("private_readiness", params)

    def _asset_orchard_pair_params(
        self, base_asset_id: str, quote_asset_id: str, inclusion_height: int
    ) -> dict[str, Any]:
        return {
            "base_asset_id": self._required_text(base_asset_id, "base_asset_id"),
            "quote_asset_id": self._required_text(quote_asset_id, "quote_asset_id"),
            "inclusion_height": self._required_u64(inclusion_height, "inclusion_height"),
        }

    def private_action_submit(
        self,
        *,
        chain_id: str,
        genesis_hash: str,
        protocol_version: int,
        pool_id: str,
        session_scope: str,
        intent_id: str,
        action_json: str,
        action_hash: str,
    ) -> dict[str, Any]:
        """Reserve a user-proven shielded action on a validator that enables
        private action admission. Returns the durable record; an acknowledgement
        means admitted and pending, never committed."""
        return self._object_read(
            "private_action_submit",
            {
                "chain_id": self._required_text(chain_id, "chain_id"),
                "genesis_hash": self._required_text(genesis_hash, "genesis_hash"),
                "protocol_version": self._required_u64(protocol_version, "protocol_version"),
                "pool_id": self._required_text(pool_id, "pool_id"),
                "session_scope": self._required_text(session_scope, "session_scope"),
                "intent_id": self._required_text(intent_id, "intent_id"),
                "action_json": self._required_text(action_json, "action_json"),
                "action_hash": self._required_text(action_hash, "action_hash"),
            },
        )

    def private_action_receipt(self, intent_id: str) -> dict[str, Any]:
        """Read-only receipt for an intent: its state and, once included, the
        inclusion height, block hash, state root, fee and finality evidence.
        An unknown intent is `pending_unknown`."""
        return self._object_read(
            "private_action_receipt",
            {"intent_id": self._required_text(intent_id, "intent_id")},
        )

    def pfusdc_ingress_preflight(
        self,
        asset_id: str,
        *,
        recipient: str,
        depositor: str,
        amount_atoms: int,
    ) -> dict[str, Any]:
        """Preflight a pfUSDC ingress (public read; all four arguments required by the node)."""
        return self._object_read(
            "pfusdc_ingress_preflight",
            {
                "asset_id": self._required_text(asset_id, "asset_id"),
                "recipient": self._required_text(recipient, "recipient"),
                "depositor": self._required_text(depositor, "depositor"),
                "amount_atoms": self._required_u64(amount_atoms, "amount_atoms"),
            },
        )

    def pfusdc_egress_witness(
        self, withdrawal_id: str, *, prior_checkpoint: str | None = None
    ) -> dict[str, Any]:
        """Export the proof-ready witness for a pfUSDC withdrawal.

        `prior_checkpoint` (the node's optional `--prior-checkpoint` block id)
        is sent only when given.
        """
        params: dict[str, Any] = {
            "withdrawal_id": self._required_text(withdrawal_id, "withdrawal_id")
        }
        if prior_checkpoint is not None:
            params["prior_checkpoint"] = self._required_text(prior_checkpoint, "prior_checkpoint")
        return self._object_read("pfusdc_egress_witness", params)

    def yolo_target_receipt(self, registration_id: str) -> dict[str, Any]:
        """Target receipt for a YOLO registration (public read, `--registration-id`)."""
        return self._object_read(
            "yolo_target_receipt",
            {"registration_id": self._required_text(registration_id, "registration_id")},
        )

    def account_tx(
        self,
        address: str,
        *,
        from_height: int | None = None,
        to_height: int | None = None,
        limit: int | None = None,
    ) -> AccountTxScan:
        if not address:
            raise ValueError("address is required")
        scan_limit = self._bounded_limit(limit)
        if from_height is not None and from_height < 0:
            raise ValueError("from_height must be non-negative")
        if to_height is not None and to_height < 0:
            raise ValueError("to_height must be non-negative")
        if from_height is not None and to_height is not None and from_height > to_height:
            raise ValueError("from_height cannot exceed to_height")

        # Always send the limit: the node echoes the limit it used as
        # scan_limit, and without one it would use its own maximum.
        params: dict[str, Any] = {"address": address, "limit": scan_limit}
        if from_height is not None:
            params["from_height"] = from_height
        if to_height is not None:
            params["to_height"] = to_height
        try:
            result = self._call("account_tx", params)
        except RpcError as error:
            if str(error.error.get("code")) != "rpc_method_not_allowed":
                raise
        else:
            return self._account_tx_scan_from_server_result(
                result,
                fallback_address=address,
                fallback_from_height=from_height,
                fallback_to_height=to_height,
                fallback_scan_limit=scan_limit,
            )

        return self._account_tx_client_side_scan(
            address=address,
            from_height=from_height,
            to_height=to_height,
            scan_limit=scan_limit,
        )

    def account_tx_history(
        self,
        address: str,
        *,
        from_height: int = 0,
        to_height: int | None = None,
        window_size: int = 100,
        limit_per_window: int = 512,
        max_windows: int = 1000,
        allow_truncated: bool = False,
    ) -> AccountTxHistory:
        if not address:
            raise ValueError("address is required")
        if from_height < 0:
            raise ValueError("from_height must be non-negative")
        if to_height is not None and to_height < 0:
            raise ValueError("to_height must be non-negative")
        if window_size < 1:
            raise ValueError("window_size must be positive")
        if max_windows < 1:
            raise ValueError("max_windows must be positive")
        per_window_limit = self._bounded_limit(limit_per_window)
        if to_height is None:
            status = self.status()
            current_height = status.get("block_height")
            if not isinstance(current_height, int):
                raise RpcProtocolError("status.block_height is unavailable")
            to_height = current_height
        if from_height > to_height:
            raise ValueError("from_height cannot exceed to_height")

        rows_by_identity: dict[str, AccountTxRow] = {}
        windows: list[dict[str, Any]] = []
        truncated_window_count = 0
        total_scanned_blocks = 0
        total_archive_lookups = 0
        all_index_used = True
        window_start = from_height
        while window_start <= to_height and len(windows) < max_windows:
            window_end = min(to_height, window_start + window_size - 1)
            scan = self.account_tx(
                address,
                from_height=window_start,
                to_height=window_end,
                limit=per_window_limit,
            )
            for row in scan.rows:
                rows_by_identity[self._account_tx_row_identity(row)] = row
            if scan.truncated:
                truncated_window_count += 1
            if not scan.index_used:
                all_index_used = False
            total_scanned_blocks += scan.scanned_block_count
            total_archive_lookups += scan.archive_lookup_count
            windows.append(
                {
                    "from_height": window_start,
                    "to_height": window_end,
                    "row_count": len(scan.rows),
                    "index_used": scan.index_used,
                    "scanned_block_count": scan.scanned_block_count,
                    "archive_lookup_count": scan.archive_lookup_count,
                    "truncated": scan.truncated,
                }
            )
            window_start = window_end + 1

        max_windows_exceeded = window_start <= to_height
        rows = tuple(
            sorted(rows_by_identity.values(), key=self._account_tx_row_sort_key)
        )
        history = AccountTxHistory(
            address=address,
            from_height=from_height,
            to_height=to_height,
            window_size=window_size,
            limit_per_window=per_window_limit,
            max_windows=max_windows,
            window_count=len(windows),
            max_windows_exceeded=max_windows_exceeded,
            truncated_window_count=truncated_window_count,
            all_index_used=all_index_used,
            total_scanned_block_count=total_scanned_blocks,
            total_archive_lookup_count=total_archive_lookups,
            rows=rows,
            windows=tuple(windows),
        )
        if not allow_truncated and not history.complete:
            raise RpcProtocolError(
                "account_tx_history did not complete without truncation "
                f"(max_windows_exceeded={history.max_windows_exceeded}, "
                f"truncated_window_count={history.truncated_window_count})"
            )
        return history

    def _account_tx_client_side_scan(
        self,
        *,
        address: str,
        from_height: int | None,
        to_height: int | None,
        scan_limit: int,
    ) -> AccountTxScan:
        # Same answer as the server archive scan: the oldest `scan_limit` blocks
        # of the range with a start height, the newest without one. `truncated`
        # means a matching row in that window was omitted, or the range continues
        # past the window (possibly incomplete: those blocks are not read).
        blocks = self.blocks(from_height=from_height, limit=scan_limit)
        if from_height is None and to_height is not None:
            heights = [h for h in map(self._block_height, blocks) if h is not None]
            if heights and max(heights) > to_height:
                # The end height is below the tip: read the newest blocks at or
                # below it, as the server does, not the chain's newest blocks.
                window_start = max(0, to_height + 1 - scan_limit)
                blocks = self.blocks(from_height=window_start, limit=scan_limit)
        selected_blocks = []
        for block in blocks:
            height = self._block_height(block)
            if height is None:
                continue
            if to_height is not None and height > to_height:
                continue
            selected_blocks.append(block)

        truncated = len(selected_blocks) >= scan_limit and self._account_tx_range_continues(
            selected_blocks, from_height, to_height
        )
        if from_height is None:
            selected_blocks.reverse()
        block_rows: list[list[AccountTxRow]] = []
        row_count = 0
        archive_lookup_count = 0
        for block in selected_blocks:
            header = block.get("header", {})
            if not isinstance(header, dict):
                continue
            batch_kind = str(header.get("batch_kind") or "")
            batch_id = str(header.get("batch_id") or "")
            height = self._block_height(block)
            if height is None or not batch_kind or not batch_id:
                continue
            if batch_kind not in ("transparent", "shielded"):
                continue
            archive_lookup_count += 1
            rows_in_block: list[AccountTxRow] = []
            for entry in self.batch_archive(
                batch_kind=batch_kind,
                batch_id=batch_id,
                limit=1,
            ):
                rows_in_block.extend(
                    self._account_tx_rows_from_archive_entry(
                        address=address,
                        block=block,
                        batch_kind=batch_kind,
                        batch_id=batch_id,
                        height=height,
                        entry=entry,
                    )
                )
            block_rows.append(rows_in_block)
            row_count += len(rows_in_block)
            if row_count > scan_limit:
                truncated = True
                break
        if from_height is None:
            block_rows.reverse()
        rows = [row for rows_in_block in block_rows for row in rows_in_block]
        if len(rows) > scan_limit:
            rows = rows[:scan_limit] if from_height is not None else rows[-scan_limit:]
        return AccountTxScan(
            address=address,
            from_height=from_height,
            to_height=to_height,
            scan_limit=scan_limit,
            index_used=False,
            scanned_block_count=len(selected_blocks),
            archive_lookup_count=archive_lookup_count,
            truncated=truncated,
            rows=tuple(rows),
        )

    def _account_tx_range_continues(
        self,
        blocks: list[dict[str, Any]],
        from_height: int | None,
        to_height: int | None,
    ) -> bool:
        """Whether the height range holds a block outside a full scan window."""
        heights = [h for h in map(self._block_height, blocks) if h is not None]
        if not heights:
            return False
        if from_height is None:
            first = self.blocks(from_height=0, limit=1)
            first_height = self._block_height(first[0]) if first else None
            return first_height is not None and first_height < min(heights)
        if to_height is not None and max(heights) >= to_height:
            return False
        following = self.blocks(from_height=max(heights) + 1, limit=1)
        following_height = self._block_height(following[0]) if following else None
        return following_height is not None and (
            to_height is None or following_height <= to_height
        )

    def _account_tx_scan_from_server_result(
        self,
        result: Any,
        *,
        fallback_address: str,
        fallback_from_height: int | None,
        fallback_to_height: int | None,
        fallback_scan_limit: int,
    ) -> AccountTxScan:
        if not isinstance(result, dict):
            raise RpcProtocolError("account_tx result must be an object")
        rows_value = result.get("rows")
        if not isinstance(rows_value, list):
            raise RpcProtocolError("account_tx rows must be a list")
        if type(result.get("truncated")) is not bool:
            raise RpcProtocolError("account_tx truncated must be a boolean")
        if (
            result.get("address") != fallback_address
            or result.get("from_height") != fallback_from_height
            or result.get("to_height") != fallback_to_height
            or type(result.get("scan_limit")) is not int
            or result["scan_limit"] != fallback_scan_limit
            or type(result.get("row_count")) is not int
            or result["row_count"] != len(rows_value)
        ):
            raise RpcProtocolError("account_tx query or row count mismatch")
        rows = []
        for row in rows_value:
            if not isinstance(row, dict):
                raise RpcProtocolError("account_tx row must be an object")
            rows.append(
                AccountTxRow(
                    tx_id=row.get("tx_id") if isinstance(row.get("tx_id"), str) else None,
                    block_height=self._required_int(row, "block_height", "account_tx row"),
                    batch_kind=str(row.get("batch_kind") or ""),
                    batch_id=str(row.get("batch_id") or ""),
                    transaction_index=self._required_int(
                        row,
                        "transaction_index",
                        "account_tx row",
                    ),
                    transaction_kind=(
                        row.get("transaction_kind")
                        if isinstance(row.get("transaction_kind"), str)
                        else None
                    ),
                    sender=row.get("from") if isinstance(row.get("from"), str) else None,
                    recipient=row.get("to") if isinstance(row.get("to"), str) else None,
                    amount=row.get("amount") if isinstance(row.get("amount"), int) else None,
                    fee=row.get("fee") if isinstance(row.get("fee"), int) else None,
                    sequence=(
                        row.get("sequence") if isinstance(row.get("sequence"), int) else None
                    ),
                    memo_hash=(
                        row.get("memo_hash") if isinstance(row.get("memo_hash"), str) else None
                    ),
                    memo_count=(
                        row.get("memo_count") if isinstance(row.get("memo_count"), int) else None
                    ),
                    memo_bytes=(
                        row.get("memo_bytes") if isinstance(row.get("memo_bytes"), int) else None
                    ),
                    asset_id=(
                        row.get("asset_id") if isinstance(row.get("asset_id"), str) else None
                    ),
                    issuer=row.get("issuer") if isinstance(row.get("issuer"), str) else None,
                    trustline_authorized=(
                        row.get("trustline_authorized")
                        if isinstance(row.get("trustline_authorized"), bool)
                        else None
                    ),
                    trustline_frozen=(
                        row.get("trustline_frozen")
                        if isinstance(row.get("trustline_frozen"), bool)
                        else None
                    ),
                    nft_id=row.get("nft_id") if isinstance(row.get("nft_id"), str) else None,
                    nft_issuer_transfer_fee=(
                        row.get("nft_issuer_transfer_fee")
                        if isinstance(row.get("nft_issuer_transfer_fee"), int)
                        else None
                    ),
                    nft_collection_flags=(
                        row.get("nft_collection_flags")
                        if isinstance(row.get("nft_collection_flags"), int)
                        else None
                    ),
                    escrow_id=(
                        row.get("escrow_id") if isinstance(row.get("escrow_id"), str) else None
                    ),
                    offer_id=(
                        row.get("offer_id") if isinstance(row.get("offer_id"), str) else None
                    ),
                    tx_role=row.get("tx_role") if isinstance(row.get("tx_role"), str) else None,
                    counterparty_offer_id=(
                        row.get("counterparty_offer_id")
                        if isinstance(row.get("counterparty_offer_id"), str)
                        else None
                    ),
                    fill_index=(
                        row.get("fill_index") if isinstance(row.get("fill_index"), int) else None
                    ),
                    condition_hash=(
                        row.get("condition_hash")
                        if isinstance(row.get("condition_hash"), str)
                        else None
                    ),
                    accepted=(
                        row.get("accepted") if isinstance(row.get("accepted"), bool) else None
                    ),
                    receipt_code=(
                        row.get("receipt_code")
                        if isinstance(row.get("receipt_code"), str)
                        else None
                    ),
                    memos=_account_tx_memos_from_server_row(row),
                )
            )
        return AccountTxScan(
            address=(
                result.get("address")
                if isinstance(result.get("address"), str)
                else fallback_address
            ),
            from_height=(
                result.get("from_height")
                if isinstance(result.get("from_height"), int)
                else fallback_from_height
            ),
            to_height=(
                result.get("to_height")
                if isinstance(result.get("to_height"), int)
                else fallback_to_height
            ),
            scan_limit=(
                result.get("scan_limit")
                if isinstance(result.get("scan_limit"), int)
                else fallback_scan_limit
            ),
            index_used=(
                result.get("index_used") if isinstance(result.get("index_used"), bool) else False
            ),
            scanned_block_count=self._optional_int(result, "scanned_block_count", default=0),
            archive_lookup_count=self._optional_int(result, "archive_lookup_count", default=0),
            truncated=bool(result.get("truncated", False)),
            rows=tuple(rows),
        )

    def _account_tx_rows_from_archive_entry(
        self,
        *,
        address: str,
        block: dict[str, Any],
        batch_kind: str,
        batch_id: str,
        height: int,
        entry: dict[str, Any],
    ) -> list[AccountTxRow]:
        payload = self._archive_payload(entry)
        if batch_kind == "shielded":
            return self._account_tx_rows_from_shielded_payload(
                address=address,
                block=block,
                batch_kind=batch_kind,
                batch_id=batch_id,
                height=height,
                payload=payload,
            )
        transactions = payload.get("transactions", [])
        if not isinstance(transactions, list):
            return []
        payments_v2 = payload.get("payments_v2", [])
        if not isinstance(payments_v2, list):
            payments_v2 = []
        asset_transactions = payload.get("asset_transactions", [])
        if not isinstance(asset_transactions, list):
            asset_transactions = []
        escrow_transactions = payload.get("escrow_transactions", [])
        if not isinstance(escrow_transactions, list):
            escrow_transactions = []
        nft_transactions = payload.get("nft_transactions", [])
        if not isinstance(nft_transactions, list):
            nft_transactions = []
        offer_transactions = payload.get("offer_transactions", [])
        if not isinstance(offer_transactions, list):
            offer_transactions = []
        atomic_swap_transactions = payload.get("atomic_swap_transactions", [])
        if not isinstance(atomic_swap_transactions, list):
            atomic_swap_transactions = []
        fastlane_primary_transactions = payload.get("fastlane_primary_transactions", [])
        if not isinstance(fastlane_primary_transactions, list):
            fastlane_primary_transactions = []
        receipt_ids = block.get("receipt_ids", [])
        if not isinstance(receipt_ids, list):
            receipt_ids = []
        rows: list[AccountTxRow] = []
        for index, transaction in enumerate(transactions):
            if not isinstance(transaction, dict):
                continue
            unsigned = transaction.get("unsigned", {})
            if not isinstance(unsigned, dict):
                continue
            sender = unsigned.get("from")
            recipient = unsigned.get("to")
            if sender != address and recipient != address:
                continue
            tx_id = receipt_ids[index] if index < len(receipt_ids) else None
            receipt = self._receipt_for_tx_id(tx_id)
            rows.append(
                AccountTxRow(
                    tx_id=tx_id,
                    block_height=height,
                    batch_kind=batch_kind,
                    batch_id=batch_id,
                    transaction_index=index,
                    transaction_kind="transparent_transfer",
                    sender=sender if isinstance(sender, str) else None,
                    recipient=recipient if isinstance(recipient, str) else None,
                    amount=unsigned.get("amount") if isinstance(unsigned.get("amount"), int) else None,
                    fee=unsigned.get("fee") if isinstance(unsigned.get("fee"), int) else None,
                    sequence=(
                        unsigned.get("sequence")
                        if isinstance(unsigned.get("sequence"), int)
                        else None
                    ),
                    memo_hash=None,
                    memo_count=None,
                    memo_bytes=None,
                    asset_id=None,
                    issuer=None,
                    trustline_authorized=None,
                    trustline_frozen=None,
                    nft_id=None,
                    nft_issuer_transfer_fee=None,
                    nft_collection_flags=None,
                    escrow_id=None,
                    offer_id=None,
                    tx_role=None,
                    counterparty_offer_id=None,
                    fill_index=None,
                    condition_hash=None,
                    accepted=receipt.get("accepted") if isinstance(receipt.get("accepted"), bool) else None,
                    receipt_code=(
                        receipt.get("code") if isinstance(receipt.get("code"), str) else None
                    ),
                )
            )
        offset = len(transactions)
        for payment_index, payment in enumerate(payments_v2):
            if not isinstance(payment, dict):
                continue
            unsigned = payment.get("unsigned", {})
            if not isinstance(unsigned, dict):
                continue
            sender = unsigned.get("from")
            recipient = unsigned.get("to")
            if sender != address and recipient != address:
                continue
            index = offset + payment_index
            tx_id = receipt_ids[index] if index < len(receipt_ids) else None
            receipt = self._receipt_for_tx_id(tx_id)
            memos = unsigned.get("memos", [])
            memo_count = len(memos) if isinstance(memos, list) else 0
            memo_bytes = 0
            memo_views: list[dict[str, str]] = []
            if isinstance(memos, list):
                for memo in memos:
                    if not isinstance(memo, dict):
                        continue
                    for key in ("memo_type", "memo_format", "memo_data"):
                        value = memo.get(key)
                        if isinstance(value, str):
                            memo_bytes += len(value) // 2
                    memo_views.append(_payment_memo_view(memo))
            rows.append(
                AccountTxRow(
                    tx_id=tx_id,
                    block_height=height,
                    batch_kind=batch_kind,
                    batch_id=batch_id,
                    transaction_index=index,
                    transaction_kind="payment_v2",
                    sender=sender if isinstance(sender, str) else None,
                    recipient=recipient if isinstance(recipient, str) else None,
                    amount=unsigned.get("amount") if isinstance(unsigned.get("amount"), int) else None,
                    fee=unsigned.get("fee") if isinstance(unsigned.get("fee"), int) else None,
                    sequence=(
                        unsigned.get("sequence")
                        if isinstance(unsigned.get("sequence"), int)
                        else None
                    ),
                    memo_hash=None,
                    memo_count=memo_count,
                    memo_bytes=memo_bytes,
                    memos=tuple(memo_views),
                    asset_id=None,
                    issuer=None,
                    trustline_authorized=None,
                    trustline_frozen=None,
                    nft_id=None,
                    nft_issuer_transfer_fee=None,
                    nft_collection_flags=None,
                    escrow_id=None,
                    offer_id=None,
                    tx_role=None,
                    counterparty_offer_id=None,
                    fill_index=None,
                    condition_hash=None,
                    accepted=receipt.get("accepted") if isinstance(receipt.get("accepted"), bool) else None,
                    receipt_code=(
                        receipt.get("code") if isinstance(receipt.get("code"), str) else None
                    ),
                )
            )
        offset += len(payments_v2)
        for asset_index, transaction in enumerate(asset_transactions):
            if not isinstance(transaction, dict):
                continue
            unsigned = transaction.get("unsigned", {})
            if not isinstance(unsigned, dict):
                continue
            operation = unsigned.get("operation", {})
            if not isinstance(operation, dict):
                continue
            transaction_kind = unsigned.get("transaction_kind")
            source = unsigned.get("source")
            op_kind = operation.get("operation")
            sender: Any = source
            recipient: Any = source
            amount: Any = 0
            asset_id: Any = operation.get("asset_id")
            issuer: Any = operation.get("issuer")
            trustline_authorized: bool | None = None
            trustline_frozen: bool | None = None
            if op_kind == "asset_create":
                recipient = operation.get("issuer")
            elif op_kind == "trust_set":
                recipient = operation.get("account")
                trustline_authorized = (
                    operation.get("authorized")
                    if isinstance(operation.get("authorized"), bool)
                    else None
                )
                trustline_frozen = (
                    operation.get("frozen") if isinstance(operation.get("frozen"), bool) else None
                )
            elif op_kind in ("issued_payment", "issued_payment_v2"):
                sender = operation.get("from")
                recipient = operation.get("to")
                amount = operation.get("amount")
            elif op_kind == "account_set":
                recipient = operation.get("account")
            elif op_kind == "asset_burn":
                sender = operation.get("owner")
                recipient = operation.get("issuer")
                amount = operation.get("amount")
            asset_memo_count: int | None = None
            asset_memo_bytes: int | None = None
            asset_memos = operation.get("memos")
            if op_kind == "issued_payment_v2" and isinstance(asset_memos, list):
                asset_memo_count = len(asset_memos)
                asset_memo_bytes = sum(
                    len(memo.get(key)) // 2
                    for memo in asset_memos
                    if isinstance(memo, dict)
                    for key in ("memo_type", "memo_format", "memo_data")
                    if isinstance(memo.get(key), str)
                )
            if sender != address and recipient != address:
                continue
            index = offset + asset_index
            tx_id = receipt_ids[index] if index < len(receipt_ids) else None
            receipt = self._receipt_for_tx_id(tx_id)
            rows.append(
                AccountTxRow(
                    tx_id=tx_id,
                    block_height=height,
                    batch_kind=batch_kind,
                    batch_id=batch_id,
                    transaction_index=index,
                    transaction_kind=(
                        transaction_kind if isinstance(transaction_kind, str) else None
                    ),
                    sender=sender if isinstance(sender, str) else None,
                    recipient=recipient if isinstance(recipient, str) else None,
                    amount=amount if isinstance(amount, int) else None,
                    fee=unsigned.get("fee") if isinstance(unsigned.get("fee"), int) else None,
                    sequence=(
                        unsigned.get("sequence")
                        if isinstance(unsigned.get("sequence"), int)
                        else None
                    ),
                    memo_hash=None,
                    memo_count=asset_memo_count,
                    memo_bytes=asset_memo_bytes,
                    memos=(
                        tuple(_payment_memo_view(memo) for memo in asset_memos if isinstance(memo, dict))
                        if op_kind == "issued_payment_v2" and isinstance(asset_memos, list)
                        else None
                    ),
                    asset_id=asset_id if isinstance(asset_id, str) else None,
                    issuer=issuer if isinstance(issuer, str) else None,
                    trustline_authorized=trustline_authorized,
                    trustline_frozen=trustline_frozen,
                    nft_id=None,
                    nft_issuer_transfer_fee=None,
                    nft_collection_flags=None,
                    escrow_id=None,
                    offer_id=None,
                    tx_role=None,
                    counterparty_offer_id=None,
                    fill_index=None,
                    condition_hash=None,
                    accepted=receipt.get("accepted") if isinstance(receipt.get("accepted"), bool) else None,
                    receipt_code=(
                        receipt.get("code") if isinstance(receipt.get("code"), str) else None
                    ),
                )
            )
        offset += len(asset_transactions)
        # Atomic swaps sit between asset and escrow transactions in the batch,
        # so they occupy receipt ids here and shift every later kind's
        # transaction_index. The archive scan emits one row per leg the
        # account owns or receives, tagged with the leg as tx_role.
        for swap_index, transaction in enumerate(atomic_swap_transactions):
            if not isinstance(transaction, dict):
                continue
            unsigned = transaction.get("unsigned", {})
            if not isinstance(unsigned, dict):
                continue
            index = offset + swap_index
            tx_id = receipt_ids[index] if index < len(receipt_ids) else None
            receipt: dict[str, Any] | None = None
            for role in ("leg_0", "leg_1"):
                leg = unsigned.get(role)
                if not isinstance(leg, dict):
                    continue
                sender = leg.get("owner")
                recipient = leg.get("recipient")
                if sender != address and recipient != address:
                    continue
                if receipt is None:
                    receipt = self._receipt_for_tx_id(tx_id)
                rows.append(
                    AccountTxRow(
                        tx_id=tx_id,
                        block_height=height,
                        batch_kind=batch_kind,
                        batch_id=batch_id,
                        transaction_index=index,
                        transaction_kind="atomic_swap",
                        sender=sender if isinstance(sender, str) else None,
                        recipient=recipient if isinstance(recipient, str) else None,
                        amount=leg.get("amount") if isinstance(leg.get("amount"), int) else None,
                        fee=leg.get("fee") if isinstance(leg.get("fee"), int) else None,
                        sequence=(
                            leg.get("sequence") if isinstance(leg.get("sequence"), int) else None
                        ),
                        memo_hash=None,
                        memo_count=None,
                        memo_bytes=None,
                        asset_id=(
                            leg.get("asset_id") if isinstance(leg.get("asset_id"), str) else None
                        ),
                        issuer=leg.get("issuer") if isinstance(leg.get("issuer"), str) else None,
                        trustline_authorized=None,
                        trustline_frozen=None,
                        nft_id=None,
                        nft_issuer_transfer_fee=None,
                        nft_collection_flags=None,
                        escrow_id=None,
                        offer_id=None,
                        tx_role=role,
                        counterparty_offer_id=None,
                        fill_index=None,
                        condition_hash=None,
                        accepted=(
                            receipt.get("accepted")
                            if isinstance(receipt.get("accepted"), bool)
                            else None
                        ),
                        receipt_code=(
                            receipt.get("code") if isinstance(receipt.get("code"), str) else None
                        ),
                    )
                )
        offset += len(atomic_swap_transactions)
        # FastLane primary transactions have no account_tx rows but take the
        # receipt ids between atomic swaps and escrows.
        offset += len(fastlane_primary_transactions)
        for escrow_index, transaction in enumerate(escrow_transactions):
            if not isinstance(transaction, dict):
                continue
            unsigned = transaction.get("unsigned", {})
            if not isinstance(unsigned, dict):
                continue
            operation = unsigned.get("operation", {})
            if not isinstance(operation, dict):
                continue
            transaction_kind = unsigned.get("transaction_kind")
            op_kind = operation.get("operation")
            sender: Any = unsigned.get("source")
            recipient: Any = unsigned.get("source")
            amount: Any = 0
            asset_id: Any = None
            escrow_id: Any = operation.get("escrow_id")
            condition_hash: str | None = None
            if op_kind == "escrow_create":
                sender = operation.get("owner")
                recipient = operation.get("recipient")
                amount = operation.get("amount")
                raw_asset_id = operation.get("asset_id")
                if (
                    isinstance(raw_asset_id, str)
                    and len(raw_asset_id) == 96
                    and all(char in "0123456789abcdef" for char in raw_asset_id)
                ):
                    asset_id = raw_asset_id
                chain_id = unsigned.get("chain_id")
                sequence = unsigned.get("sequence")
                if isinstance(chain_id, str) and isinstance(sender, str) and isinstance(sequence, int):
                    escrow_id = _escrow_id(chain_id, sender, sequence)
                condition = operation.get("condition")
                if isinstance(condition, str) and condition:
                    condition_hash = _escrow_condition_hash(condition)
            elif op_kind == "escrow_finish":
                sender = operation.get("owner")
                recipient = operation.get("recipient")
            elif op_kind == "escrow_cancel":
                sender = operation.get("owner")
                recipient = operation.get("owner")
            if sender != address and recipient != address:
                continue
            index = offset + escrow_index
            tx_id = receipt_ids[index] if index < len(receipt_ids) else None
            receipt = self._receipt_for_tx_id(tx_id)
            rows.append(
                AccountTxRow(
                    tx_id=tx_id,
                    block_height=height,
                    batch_kind=batch_kind,
                    batch_id=batch_id,
                    transaction_index=index,
                    transaction_kind=(
                        transaction_kind if isinstance(transaction_kind, str) else None
                    ),
                    sender=sender if isinstance(sender, str) else None,
                    recipient=recipient if isinstance(recipient, str) else None,
                    amount=amount if isinstance(amount, int) else None,
                    fee=unsigned.get("fee") if isinstance(unsigned.get("fee"), int) else None,
                    sequence=(
                        unsigned.get("sequence")
                        if isinstance(unsigned.get("sequence"), int)
                        else None
                    ),
                    memo_hash=None,
                    memo_count=None,
                    memo_bytes=None,
                    asset_id=asset_id if isinstance(asset_id, str) else None,
                    issuer=None,
                    trustline_authorized=None,
                    trustline_frozen=None,
                    nft_id=None,
                    nft_issuer_transfer_fee=None,
                    nft_collection_flags=None,
                    escrow_id=escrow_id if isinstance(escrow_id, str) else None,
                    offer_id=None,
                    tx_role=None,
                    counterparty_offer_id=None,
                    fill_index=None,
                    condition_hash=condition_hash,
                    accepted=receipt.get("accepted") if isinstance(receipt.get("accepted"), bool) else None,
                    receipt_code=(
                        receipt.get("code") if isinstance(receipt.get("code"), str) else None
                    ),
                )
            )
        offset += len(escrow_transactions)
        for nft_index, transaction in enumerate(nft_transactions):
            if not isinstance(transaction, dict):
                continue
            unsigned = transaction.get("unsigned", {})
            if not isinstance(unsigned, dict):
                continue
            operation = unsigned.get("operation", {})
            if not isinstance(operation, dict):
                continue
            transaction_kind = unsigned.get("transaction_kind")
            op_kind = operation.get("operation")
            sender: Any = unsigned.get("source")
            recipient: Any = unsigned.get("source")
            issuer: Any = None
            nft_id: Any = operation.get("nft_id")
            nft_issuer_transfer_fee: int | None = None
            nft_collection_flags: int | None = None
            if op_kind == "nft_mint":
                sender = operation.get("issuer")
                recipient = operation.get("owner")
                issuer = operation.get("issuer")
                collection_flags = operation.get("collection_flags")
                if isinstance(collection_flags, int) and collection_flags != 0:
                    nft_collection_flags = collection_flags
                chain_id = unsigned.get("chain_id")
                collection_id = operation.get("collection_id")
                serial = operation.get("serial")
                if (
                    isinstance(chain_id, str)
                    and isinstance(sender, str)
                    and isinstance(collection_id, str)
                    and isinstance(serial, int)
                ):
                    nft_id = _nft_id(chain_id, sender, collection_id, serial)
            elif op_kind == "nft_transfer":
                sender = operation.get("from")
                recipient = operation.get("to")
                issuer = operation.get("issuer")
                transfer_fee = operation.get("issuer_transfer_fee")
                if isinstance(transfer_fee, int) and transfer_fee != 0:
                    nft_issuer_transfer_fee = transfer_fee
            elif op_kind == "nft_burn":
                sender = operation.get("owner")
                recipient = operation.get("owner")
            if sender != address and recipient != address:
                continue
            index = offset + nft_index
            tx_id = receipt_ids[index] if index < len(receipt_ids) else None
            receipt = self._receipt_for_tx_id(tx_id)
            rows.append(
                AccountTxRow(
                    tx_id=tx_id,
                    block_height=height,
                    batch_kind=batch_kind,
                    batch_id=batch_id,
                    transaction_index=index,
                    transaction_kind=(
                        transaction_kind if isinstance(transaction_kind, str) else None
                    ),
                    sender=sender if isinstance(sender, str) else None,
                    recipient=recipient if isinstance(recipient, str) else None,
                    amount=0,
                    fee=unsigned.get("fee") if isinstance(unsigned.get("fee"), int) else None,
                    sequence=(
                        unsigned.get("sequence")
                        if isinstance(unsigned.get("sequence"), int)
                        else None
                    ),
                    memo_hash=None,
                    memo_count=None,
                    memo_bytes=None,
                    asset_id=None,
                    issuer=issuer if isinstance(issuer, str) else None,
                    trustline_authorized=None,
                    trustline_frozen=None,
                    nft_id=nft_id if isinstance(nft_id, str) else None,
                    nft_issuer_transfer_fee=nft_issuer_transfer_fee,
                    nft_collection_flags=nft_collection_flags,
                    escrow_id=None,
                    offer_id=None,
                    tx_role=None,
                    counterparty_offer_id=None,
                    fill_index=None,
                    condition_hash=None,
                    accepted=(
                        receipt.get("accepted") if isinstance(receipt.get("accepted"), bool) else None
                    ),
                    receipt_code=(
                        receipt.get("code") if isinstance(receipt.get("code"), str) else None
                    ),
                )
            )
        offset += len(nft_transactions)
        for offer_index, transaction in enumerate(offer_transactions):
            if not isinstance(transaction, dict):
                continue
            unsigned = transaction.get("unsigned", {})
            if not isinstance(unsigned, dict):
                continue
            operation = unsigned.get("operation", {})
            if not isinstance(operation, dict):
                continue
            transaction_kind = unsigned.get("transaction_kind")
            op_kind = operation.get("operation")
            sender: Any = unsigned.get("source")
            recipient: Any = unsigned.get("source")
            amount: Any = 0
            asset_id: Any = None
            offer_id: Any = operation.get("offer_id")
            if op_kind == "offer_create":
                sender = operation.get("owner")
                recipient = operation.get("owner")
                amount = operation.get("taker_gets_amount")
                taker_gets_asset_id = operation.get("taker_gets_asset_id")
                taker_pays_asset_id = operation.get("taker_pays_asset_id")
                if isinstance(taker_gets_asset_id, str) and taker_gets_asset_id != "PFT":
                    asset_id = taker_gets_asset_id
                elif isinstance(taker_pays_asset_id, str) and taker_pays_asset_id != "PFT":
                    asset_id = taker_pays_asset_id
                chain_id = unsigned.get("chain_id")
                sequence = unsigned.get("sequence")
                if isinstance(chain_id, str) and isinstance(sender, str) and isinstance(sequence, int):
                    offer_id = _offer_id(chain_id, sender, sequence)
            elif op_kind == "offer_cancel":
                sender = operation.get("owner")
                recipient = operation.get("owner")
            index = offset + offer_index
            tx_id = receipt_ids[index] if index < len(receipt_ids) else None
            receipt = self._receipt_for_tx_id(tx_id)
            if sender == address or recipient == address:
                rows.append(
                    AccountTxRow(
                        tx_id=tx_id,
                        block_height=height,
                        batch_kind=batch_kind,
                        batch_id=batch_id,
                        transaction_index=index,
                        transaction_kind=(
                            transaction_kind if isinstance(transaction_kind, str) else None
                        ),
                        sender=sender if isinstance(sender, str) else None,
                        recipient=recipient if isinstance(recipient, str) else None,
                        amount=amount if isinstance(amount, int) else None,
                        fee=unsigned.get("fee") if isinstance(unsigned.get("fee"), int) else None,
                        sequence=(
                            unsigned.get("sequence")
                            if isinstance(unsigned.get("sequence"), int)
                            else None
                        ),
                        memo_hash=None,
                        memo_count=None,
                        memo_bytes=None,
                        asset_id=asset_id if isinstance(asset_id, str) else None,
                        issuer=None,
                        trustline_authorized=None,
                        trustline_frozen=None,
                        nft_id=None,
                        nft_issuer_transfer_fee=None,
                        nft_collection_flags=None,
                        escrow_id=None,
                        offer_id=offer_id if isinstance(offer_id, str) else None,
                        tx_role=(
                            "offer_taker"
                            if op_kind == "offer_create"
                            else "offer_cancel"
                            if op_kind == "offer_cancel"
                            else None
                        ),
                        counterparty_offer_id=None,
                        fill_index=None,
                        condition_hash=None,
                        accepted=(
                            receipt.get("accepted")
                            if isinstance(receipt.get("accepted"), bool)
                            else None
                        ),
                        receipt_code=(
                            receipt.get("code") if isinstance(receipt.get("code"), str) else None
                        ),
                    )
                )
            fills = receipt.get("offer_fills", [])
            if op_kind == "offer_create" and isinstance(fills, list):
                for fill in fills:
                    if not isinstance(fill, dict):
                        continue
                    maker_owner = fill.get("maker_owner")
                    taker = fill.get("taker")
                    if maker_owner != address and taker != address:
                        continue
                    maker_sends_asset_id = fill.get("maker_sends_asset_id")
                    rows.append(
                        AccountTxRow(
                            tx_id=tx_id,
                            block_height=height,
                            batch_kind=batch_kind,
                            batch_id=batch_id,
                            transaction_index=index,
                            transaction_kind=(
                                transaction_kind if isinstance(transaction_kind, str) else None
                            ),
                            sender=maker_owner if isinstance(maker_owner, str) else None,
                            recipient=taker if isinstance(taker, str) else None,
                            amount=(
                                fill.get("maker_sends_amount")
                                if isinstance(fill.get("maker_sends_amount"), int)
                                else None
                            ),
                            fee=0,
                            sequence=0,
                            memo_hash=None,
                            memo_count=None,
                            memo_bytes=None,
                            asset_id=(
                                maker_sends_asset_id
                                if isinstance(maker_sends_asset_id, str)
                                and maker_sends_asset_id != "PFT"
                                else None
                            ),
                            issuer=None,
                            trustline_authorized=None,
                            trustline_frozen=None,
                            nft_id=None,
                            nft_issuer_transfer_fee=None,
                            nft_collection_flags=None,
                            escrow_id=None,
                            offer_id=(
                                fill.get("maker_offer_id")
                                if isinstance(fill.get("maker_offer_id"), str)
                                else None
                            ),
                            tx_role="offer_maker",
                            counterparty_offer_id=(
                                offer_id if isinstance(offer_id, str) else None
                            ),
                            fill_index=(
                                fill.get("fill_index")
                                if isinstance(fill.get("fill_index"), int)
                                else None
                            ),
                            condition_hash=None,
                            accepted=(
                                receipt.get("accepted")
                                if isinstance(receipt.get("accepted"), bool)
                                else None
                            ),
                            receipt_code=(
                                receipt.get("code")
                                if isinstance(receipt.get("code"), str)
                                else None
                            ),
                        )
                    )
        return rows

    def _account_tx_rows_from_shielded_payload(
        self,
        *,
        address: str,
        block: dict[str, Any],
        batch_kind: str,
        batch_id: str,
        height: int,
        payload: dict[str, Any],
    ) -> list[AccountTxRow]:
        """Shielded-withdrawal credits, as the server archive scan lists them:
        one row per orchard_withdraw_v1 / asset_orchard_*egress_v1 action, from
        the shielded pool to the transparent recipient, with the action index
        as transaction_index and tx_role "shielded_withdrawal"."""
        actions = payload.get("actions", [])
        if not isinstance(actions, list):
            return []
        receipt_ids = block.get("receipt_ids", [])
        if not isinstance(receipt_ids, list):
            receipt_ids = []
        rows: list[AccountTxRow] = []
        for index, action in enumerate(actions):
            credit = _shielded_withdrawal_credit(action)
            if credit is None:
                continue
            kind, pool_id, recipient, amount, fee, asset_id = credit
            if pool_id != address and recipient != address:
                continue
            tx_id = receipt_ids[index] if index < len(receipt_ids) else None
            receipt = self._receipt_for_tx_id(tx_id)
            rows.append(
                AccountTxRow(
                    tx_id=tx_id,
                    block_height=height,
                    batch_kind=batch_kind,
                    batch_id=batch_id,
                    transaction_index=index,
                    transaction_kind=kind,
                    sender=pool_id,
                    recipient=recipient,
                    amount=amount,
                    fee=fee,
                    sequence=0,
                    memo_hash=None,
                    memo_count=None,
                    memo_bytes=None,
                    asset_id=asset_id,
                    issuer=None,
                    trustline_authorized=None,
                    trustline_frozen=None,
                    nft_id=None,
                    nft_issuer_transfer_fee=None,
                    nft_collection_flags=None,
                    escrow_id=None,
                    offer_id=None,
                    tx_role=SHIELDED_WITHDRAWAL_TX_ROLE,
                    counterparty_offer_id=None,
                    fill_index=None,
                    condition_hash=None,
                    accepted=(
                        receipt.get("accepted")
                        if isinstance(receipt.get("accepted"), bool)
                        else None
                    ),
                    receipt_code=(
                        receipt.get("code") if isinstance(receipt.get("code"), str) else None
                    ),
                )
            )
        return rows

    def _receipt_for_tx_id(self, tx_id: str | None) -> dict[str, Any]:
        if not tx_id:
            return {}
        receipts = self.receipts(tx_id=tx_id, limit=1)
        if not receipts:
            return {}
        receipt = receipts[0]
        return receipt if isinstance(receipt, dict) else {}

    @staticmethod
    def _archive_payload(entry: dict[str, Any]) -> dict[str, Any]:
        payload = entry.get("payload_json")
        if isinstance(payload, str):
            parsed = json.loads(payload)
            return parsed if isinstance(parsed, dict) else {}
        if isinstance(payload, dict):
            return payload
        return {}

    @staticmethod
    def _block_height(block: dict[str, Any]) -> int | None:
        header = block.get("header", {})
        if not isinstance(header, dict):
            return None
        height = header.get("height")
        return height if isinstance(height, int) else None

    @staticmethod
    def _account_tx_row_sort_key(row: AccountTxRow) -> tuple[int, str, int, str]:
        return (
            row.block_height,
            row.batch_id,
            row.transaction_index,
            row.tx_id or "",
        )

    @staticmethod
    def _account_tx_row_identity(row: AccountTxRow) -> str:
        # One transaction can list several rows for an account: one per atomic
        # swap leg (`tx_role`) and one per offer fill (`fill_index`).
        height, batch_id, tx_index, tx_id = PostFiatRpcClient._account_tx_row_sort_key(row)
        fill_index = "" if row.fill_index is None else row.fill_index
        return f"{height}:{batch_id}:{tx_index}:{tx_id}:{row.tx_role or ''}:{fill_index}"

    @staticmethod
    def _required_int(value: dict[str, Any], key: str, label: str) -> int:
        found = value.get(key)
        if not isinstance(found, int):
            raise RpcProtocolError(f"{label} field {key} must be an integer")
        return found

    @staticmethod
    def _optional_int(value: dict[str, Any], key: str, *, default: int) -> int:
        found = value.get(key)
        return found if isinstance(found, int) else default

    @staticmethod
    def _limit_params(limit: int | None) -> dict[str, Any]:
        return {} if limit is None else {"limit": PostFiatRpcClient._bounded_limit(limit)}

    @staticmethod
    def _bounded_limit(limit: int | None) -> int:
        if limit is None:
            return DEFAULT_READ_LIMIT
        if limit < 1:
            raise ValueError("limit must be positive")
        return min(limit, MAX_ACCOUNT_TX_SCAN_LIMIT)

    def _call(
        self,
        method: str,
        params: dict[str, Any] | None = None,
        *,
        request_id: str | None = None,
    ) -> Any:
        request = self._request(method, params or {}, request_id=request_id)
        response = self._send(request)
        return self._unwrap_response(method, request["id"], response)

    def call_response(
        self,
        method: str,
        params: dict[str, Any] | None = None,
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        request = self._request(method, params or {}, request_id=request_id)
        response = self._send(request)
        self._unwrap_response(method, request["id"], response)
        return response

    def _request(
        self,
        method: str,
        params: dict[str, Any],
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        self._request_counter += 1
        return {
            "version": RPC_VERSION,
            "id": request_id or f"py-{self._request_counter:08d}",
            "method": method,
            "params": params,
        }

    def _send(self, request: dict[str, Any]) -> dict[str, Any]:
        wire = json.dumps(request, separators=(",", ":")).encode("utf-8") + b"\n"
        with socket.create_connection(
            (self.endpoint.host, self.endpoint.port),
            timeout=self.timeout_seconds,
        ) as stream:
            stream.settimeout(self.timeout_seconds)
            stream.sendall(wire)
            stream.shutdown(socket.SHUT_WR)
            chunks: list[bytes] = []
            received = 0
            while True:
                chunk = stream.recv(65536)
                if not chunk:
                    break
                received += len(chunk)
                if received > self.response_byte_cap:
                    raise RpcProtocolError(
                        f"response exceeded byte cap {self.response_byte_cap}"
                    )
                chunks.append(chunk)
        try:
            response = json.loads(b"".join(chunks).decode("utf-8"))
        except json.JSONDecodeError as error:
            raise RpcProtocolError(f"response was not valid JSON: {error}") from error
        if not isinstance(response, dict):
            raise RpcProtocolError("response envelope must be an object")
        return response

    @staticmethod
    def _unwrap_response(method: str, request_id: str, response: dict[str, Any]) -> Any:
        if response.get("version") != RPC_VERSION:
            raise RpcProtocolError("response version mismatch")
        if response.get("id") != request_id:
            raise RpcProtocolError("response id mismatch")
        if response.get("ok") is not True:
            error = response.get("error", {})
            if not isinstance(error, dict):
                error = {"code": "rpc_error", "message": "missing error object"}
            raise RpcError(method, error)
        if "result" not in response:
            raise RpcProtocolError("response missing result")
        return response["result"]


class PostFiatWebSocketRpcClient(PostFiatRpcClient):
    """Synchronous WebSocket RPC client for wallet-facing proxy endpoints.

    As on the persistent TCP client, request ids are never reused on a
    session, so a late or stray message can never match a later request.
    """

    def __init__(
        self,
        url: str,
        *,
        timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
        response_byte_cap: int = DEFAULT_RESPONSE_BYTE_CAP,
        origin: str | None = None,
        proxy_auth_token: str | None = None,
    ) -> None:
        if not url.startswith(("ws://", "wss://")):
            raise ValueError("WebSocket RPC endpoint must use ws:// or wss://")
        super().__init__(
            Endpoint("127.0.0.1", 1),
            timeout_seconds=timeout_seconds,
            response_byte_cap=response_byte_cap,
        )
        self.url = url
        self.origin = origin
        self.proxy_auth_token = proxy_auth_token
        self._websocket = None
        self._sent_request_ids: set[str] = set()
        self._send_lock = threading.Lock()
        self._worker_lock = threading.Lock()
        self._worker_clients: dict[str, PostFiatWebSocketRpcClient] = {}
        self._session_cache_lock = threading.Lock()
        self._session_cache: dict[str, Any] = {}

    def close(self) -> None:
        with self._worker_lock:
            workers = list(self._worker_clients.values())
            self._worker_clients.clear()
        for worker in workers:
            worker.close()
        with self._send_lock:
            self._close_websocket()

    def _close_websocket(self) -> None:
        websocket = self._websocket
        self._websocket = None
        self._sent_request_ids.clear()
        if websocket is not None:
            try:
                websocket.close()
            except Exception:
                pass

    def persistent_worker(self, key: str) -> "PostFiatWebSocketRpcClient":
        """Return a client-lifetime WebSocket session for one concurrent lane."""

        with self._worker_lock:
            worker = self._worker_clients.get(key)
            if worker is None:
                worker = PostFiatWebSocketRpcClient(
                    self.url,
                    timeout_seconds=self.timeout_seconds,
                    response_byte_cap=self.response_byte_cap,
                    origin=self.origin,
                    proxy_auth_token=self.proxy_auth_token,
                )
                self._worker_clients[key] = worker
            return worker

    def session_get_or_load(self, key: str, loader):
        """Cache discovery data for this transport session.

        The server remains the authority for admission and certificate
        verification. A stale committee cache therefore fails closed at apply;
        it cannot make an invalid payment acceptable.
        """

        with self._session_cache_lock:
            if key not in self._session_cache:
                self._session_cache[key] = loader()
            return self._session_cache[key]

    def clear_session_cache(self) -> None:
        with self._session_cache_lock:
            self._session_cache.clear()

    def _request(
        self,
        method: str,
        params: dict[str, Any],
        *,
        request_id: str | None = None,
    ) -> dict[str, Any]:
        request = super()._request(method, params, request_id=request_id)
        if self.proxy_auth_token is not None:
            request["proxy_auth_token"] = self.proxy_auth_token
        return request

    def _connect_websocket(self):
        if self._websocket is not None:
            return self._websocket
        try:
            from websockets.sync.client import connect
        except ImportError as error:
            raise RpcProtocolError(
                "websockets package is required for WebSocket RPC"
            ) from error
        try:
            self._websocket = connect(
                self.url,
                origin=self.origin,
                open_timeout=self.timeout_seconds,
                close_timeout=self.timeout_seconds,
                max_size=self.response_byte_cap,
                proxy=None,
            )
        except Exception as error:
            self._websocket = None
            raise RpcProtocolError(f"WebSocket RPC failed: {error}") from error
        return self._websocket

    def _send(self, request: dict[str, Any]) -> dict[str, Any]:
        wire = json.dumps(request, separators=(",", ":"))
        with self._send_lock:
            try:
                websocket = self._connect_websocket()
                wire_id = json.dumps(request["id"])
                if wire_id in self._sent_request_ids:
                    raise RpcProtocolError("request id reused on this WebSocket session")
                self._sent_request_ids.add(wire_id)
                websocket.send(wire)
                message = websocket.recv(timeout=self.timeout_seconds)
                if isinstance(message, bytes):
                    raw = message.decode("utf-8")
                else:
                    raw = str(message)
                if len(raw.encode("utf-8")) > self.response_byte_cap:
                    raise RpcProtocolError(f"response exceeded byte cap {self.response_byte_cap}")
                try:
                    response = json.loads(raw)
                except json.JSONDecodeError as error:
                    raise RpcProtocolError(f"response was not valid JSON: {error}") from error
                if not isinstance(response, dict):
                    raise RpcProtocolError("response envelope must be an object")
                # Bind and validate before another call can reuse this session.
                # A bound RPC refusal is valid; the caller will raise RpcError.
                try:
                    self._unwrap_response(request["method"], request["id"], response)
                except RpcError:
                    pass
                return response
            except TimeoutError as error:
                self._close_websocket()
                raise RpcProtocolError(f"WebSocket RPC timeout: {request.get('method')}") from error
            except BaseException as error:
                self._close_websocket()
                if isinstance(error, RpcProtocolError) or not isinstance(error, Exception):
                    raise
                raise RpcProtocolError(f"WebSocket RPC failed: {error}") from error


def _hash_hex_domain(domain: str, payload: bytes) -> str:
    digest = hashlib.sha3_384()
    digest.update(domain.encode("utf-8"))
    digest.update(b"\x00")
    digest.update(payload)
    return digest.hexdigest()


def _escrow_id(chain_id: str, owner: str, owner_sequence: int) -> str:
    preimage = f"chain_id={chain_id}\nowner={owner}\nowner_sequence={owner_sequence}\n"
    return _hash_hex_domain(ESCROW_ID_DOMAIN, preimage.encode("utf-8"))


SHIELDED_WITHDRAWAL_TX_ROLE = "shielded_withdrawal"
_ORCHARD_DEFAULT_POOL_ID = "orchard-v1"
_MEMO_HEX_FIELDS = ("memo_type", "memo_format", "memo_data")


def _memo_hex_text(value: str) -> str | None:
    """The UTF-8 reading of a nonempty hex memo field, or None."""
    if not value:
        return None
    try:
        return bytes.fromhex(value).decode("utf-8")
    except ValueError:
        return None


def _payment_memo_view(memo: dict[str, Any]) -> dict[str, str]:
    """A signed payment_v2 memo in the account_tx `memos` shape."""
    view: dict[str, str] = {}
    for key in _MEMO_HEX_FIELDS:
        value = memo.get(key)
        view[key] = value if isinstance(value, str) else ""
    for key in _MEMO_HEX_FIELDS:
        text = _memo_hex_text(view[key])
        if text is not None:
            view[f"{key}_text"] = text
    return view


def _account_tx_memos_from_server_row(row: dict[str, Any]) -> tuple[dict[str, str], ...] | None:
    memos = row.get("memos")
    if not isinstance(memos, list):
        return None
    views = []
    for memo in memos:
        if not isinstance(memo, dict):
            continue
        views.append(
            {
                key: value
                for key, value in memo.items()
                if isinstance(key, str) and isinstance(value, str)
            }
        )
    return tuple(views)


def _account_tx_clean_key(value: Any) -> bool:
    return (
        isinstance(value, str)
        and bool(value)
        and value == value.strip()
        and not any(ord(char) < 0x20 or 0x7F <= ord(char) < 0xA0 for char in value)
        and len(value.encode("utf-8")) <= 256
    )


def _shielded_withdrawal_credit(
    action: Any,
) -> tuple[str, str, str, int, int, str | None] | None:
    """(kind, pool_id, recipient, amount, fee, asset_id) for a shielded action
    that credits a transparent account, else None."""
    if not isinstance(action, dict):
        return None
    kind = action.get("kind")
    if kind == "orchard_withdraw_v1":
        pool_id: Any = _ORCHARD_DEFAULT_POOL_ID
        action_json = action.get("action_json")
        if isinstance(action_json, str):
            try:
                parsed = json.loads(action_json)
            except ValueError:
                parsed = None
            if isinstance(parsed, dict) and isinstance(parsed.get("pool_id"), str):
                pool_id = parsed["pool_id"]
        fee = action.get("fee")
        asset_id = None
    elif kind in ("asset_orchard_egress_v1", "asset_orchard_private_egress_v1"):
        pool_id = action.get("pool_id")
        fee = action.get("fee", 0) if kind == "asset_orchard_private_egress_v1" else 0
        asset_id = action.get("asset_id")
        if not (
            isinstance(asset_id, str)
            and len(asset_id) == 96
            and all(char in "0123456789abcdef" for char in asset_id)
        ):
            return None
    else:
        return None
    recipient = action.get("to")
    amount = action.get("amount")
    if not (
        _account_tx_clean_key(pool_id)
        and _account_tx_clean_key(recipient)
        and isinstance(amount, int)
        and isinstance(fee, int)
    ):
        return None
    return kind, pool_id, recipient, amount, fee, asset_id


def _escrow_condition_hash(condition: str) -> str:
    return _hash_hex_domain(ESCROW_CONDITION_HASH_DOMAIN, condition.encode("utf-8"))


def _nft_id(chain_id: str, issuer: str, collection_id: str, serial: int) -> str:
    preimage = (
        f"chain_id={chain_id}\n"
        f"issuer={issuer}\n"
        f"collection_id_bytes={len(collection_id)}\n"
        f"collection_id={collection_id}\n"
        f"serial={serial}\n"
    )
    return _hash_hex_domain(NFT_ID_DOMAIN, preimage.encode("utf-8"))


def _offer_id(chain_id: str, owner: str, owner_sequence: int) -> str:
    preimage = f"chain_id={chain_id}\nowner={owner}\nowner_sequence={owner_sequence}\n"
    return _hash_hex_domain(OFFER_ID_DOMAIN, preimage.encode("utf-8"))


PRIVATE_ACTION_INTENT_DOMAIN = "tasknode.private-intent.v1"
PRIVATE_ACTION_HASH_DOMAIN = "postfiat.private-actions.action.v1"


def _domain_hash_hex(domain: str, payload: bytes) -> str:
    return hashlib.sha3_384(domain.encode() + b"\x00" + payload).hexdigest()


def private_action_intent_id(
    chain_id: str, genesis_hash: str, protocol_version: int, action_json: str
) -> str:
    """The wallet's intent id: SHA3-384 under `tasknode.private-intent.v1`
    over the compact JSON pair (domain, action text)."""
    pair = [
        {"chain_id": chain_id, "genesis_hash": genesis_hash, "protocol_version": protocol_version},
        action_json,
    ]
    payload = json.dumps(pair, separators=(",", ":"), ensure_ascii=False).encode()
    return _domain_hash_hex(PRIVATE_ACTION_INTENT_DOMAIN, payload)


def private_action_hash(action_json: str) -> str:
    """SHA3-384 of the action text under `postfiat.private-actions.action.v1`.

    The contract hashes the canonical serialization of the decoded action;
    this equals it when the text is canonical, which is what the wallet
    persists. The node recomputes it and refuses a mismatch."""
    return _domain_hash_hex(PRIVATE_ACTION_HASH_DOMAIN, action_json.encode())
