#!/usr/bin/env python3
"""Policy-constrained, provider-neutral Ethereum signer for PostFiat relays."""

from __future__ import annotations

import argparse
import getpass
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import socketserver
import stat
import sys
import tempfile
import threading
import time
from typing import Any, Callable

from eth_account import Account
from web3 import Web3


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "python"))
from postfiat_ops.constrained_signer import (  # noqa: E402
    MAX_MESSAGE_BYTES,
    REQUEST_SCHEMA,
    RESPONSE_SCHEMA,
    call_signer,
)


CONFIG_SCHEMA = "postfiat.constrained_signer.config.v1"
STATE_SCHEMA = "postfiat.constrained_signer.state.v1"
MAX_IDEMPOTENCY_RECORDS = 4096
MAX_ROLLING_VALUE_RECORDS = 4096
ROUTE_DIGEST_RE = re.compile(r"^(?:[0-9a-f]{64}|[0-9a-f]{96})$")
EVM_ADDRESS_RE = re.compile(r"^0x[0-9a-f]{40}$")
SELECTOR_RE = re.compile(r"^0x[0-9a-f]{8}$")
SAFE_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$")


class SignerFailure(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def canonical_json(value: Any) -> bytes:
    return json.dumps(value, separators=(",", ":"), sort_keys=True).encode()


def atomic_write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    descriptor, name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary = Path(name)
    try:
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "w") as handle:
            json.dump(value, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        temporary.unlink(missing_ok=True)


def create_keystore(keystore_path: Path, passphrase_file: Path) -> dict[str, Any]:
    """Create one encrypted relay key without exposing its private material."""

    password_path = secure_regular_file(passphrase_file, "passphrase file", 16 * 1024)
    passphrase = password_path.read_text().rstrip("\r\n")
    if len(passphrase) < 12 or len(passphrase) > 4096:
        raise SignerFailure(
            "signer_passphrase_invalid",
            "keystore passphrase must contain between 12 and 4096 characters",
        )
    absolute = keystore_path.expanduser().resolve()
    if absolute.exists() or absolute.is_symlink():
        raise SignerFailure(
            "signer_keystore_exists",
            "refusing to replace an existing keystore path",
        )
    absolute.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    parent_info = absolute.parent.stat()
    if (
        not stat.S_ISDIR(parent_info.st_mode)
        or parent_info.st_uid != os.getuid()
        or parent_info.st_mode & 0o077
    ):
        raise SignerFailure(
            "signer_insecure_keystore_directory",
            "keystore directory must be owner-only",
        )
    account = Account.create()
    encrypted = Account.encrypt(account.key, passphrase)
    address = account.address.lower()
    del account
    del passphrase
    atomic_write_json(absolute, encrypted)
    return {
        "ok": True,
        "schema": RESPONSE_SCHEMA,
        "op": "create-keystore",
        "address": address,
        "keystore_path": str(absolute),
    }


def secure_regular_file(path: Path, label: str, maximum_bytes: int) -> Path:
    absolute = path.expanduser().resolve()
    info = absolute.lstat()
    if (
        not stat.S_ISREG(info.st_mode)
        or absolute.is_symlink()
        or info.st_uid != os.getuid()
        or info.st_mode & 0o077
        or info.st_size <= 0
        or info.st_size > maximum_bytes
    ):
        raise SignerFailure(
            "signer_insecure_file",
            f"{label} must be an owner-only regular file no larger than {maximum_bytes} bytes",
        )
    return absolute


def parse_nonnegative_int(value: Any, field: str, maximum: int = 2**256 - 1) -> int:
    if isinstance(value, bool):
        raise SignerFailure("signer_request_invalid", f"{field} must be an integer")
    try:
        parsed = int(value)
    except (TypeError, ValueError) as error:
        raise SignerFailure("signer_request_invalid", f"{field} must be an integer") from error
    if parsed < 0 or parsed > maximum or str(value).strip() != str(parsed):
        raise SignerFailure("signer_request_invalid", f"{field} is outside its canonical range")
    return parsed


def normalize_address(value: Any, field: str) -> str:
    address = str(value or "").lower()
    if not EVM_ADDRESS_RE.fullmatch(address):
        raise SignerFailure("signer_request_invalid", f"{field} must be one EVM address")
    return address


def load_config(path: Path) -> dict[str, Any]:
    absolute = secure_regular_file(path, "signer config", 128 * 1024)
    try:
        config = json.loads(absolute.read_text())
    except json.JSONDecodeError as error:
        raise SignerFailure("signer_config_invalid", "signer config is not valid JSON") from error
    if not isinstance(config, dict) or config.get("schema") != CONFIG_SCHEMA:
        raise SignerFailure("signer_config_invalid", "signer config schema mismatch")
    if config.get("backend") != "local_keystore":
        raise SignerFailure("signer_config_invalid", "only the local_keystore backend is enabled")
    config["keystore_path"] = str(
        secure_regular_file(Path(str(config.get("keystore_path", ""))), "keystore", 2 * 1024 * 1024)
    )
    config["state_path"] = str(Path(str(config.get("state_path", ""))).expanduser().resolve())
    config["socket_path"] = str(Path(str(config.get("socket_path", ""))).expanduser().resolve())
    chains = config.get("chains")
    routes = config.get("routes")
    if not isinstance(chains, dict) or not chains or not isinstance(routes, dict) or not routes:
        raise SignerFailure("signer_config_invalid", "chains and routes must be nonempty objects")
    for raw_chain_id, chain in chains.items():
        chain_id = parse_nonnegative_int(raw_chain_id, "chain id", 2**63 - 1)
        if chain_id == 0 or not isinstance(chain, dict):
            raise SignerFailure("signer_config_invalid", "chain policy is malformed")
        rpc_url = str(chain.get("rpc_url") or "")
        if not rpc_url.startswith(("https://", "http://127.0.0.1:", "http://localhost:")):
            raise SignerFailure("signer_config_invalid", "RPC URL must use HTTPS or loopback HTTP")
        contracts = chain.get("contracts")
        if not isinstance(contracts, dict) or not contracts:
            raise SignerFailure("signer_config_invalid", "chain contracts must be nonempty")
        normalized_contracts: dict[str, Any] = {}
        for raw_address, contract in contracts.items():
            address = normalize_address(raw_address, "contract address")
            selectors = contract.get("selectors") if isinstance(contract, dict) else None
            kinds = contract.get("transaction_kinds") if isinstance(contract, dict) else None
            if (
                not isinstance(selectors, list)
                or not selectors
                or len(selectors) > 64
                or any(not SELECTOR_RE.fullmatch(str(item).lower()) for item in selectors)
                or not isinstance(kinds, list)
                or not kinds
                or len(kinds) > 64
                or any(not SAFE_ID_RE.fullmatch(str(item)) for item in kinds)
            ):
                raise SignerFailure("signer_config_invalid", "contract selectors or transaction kinds are invalid")
            normalized_contracts[address] = {
                "selectors": sorted({str(item).lower() for item in selectors}),
                "transaction_kinds": sorted({str(item) for item in kinds}),
            }
        chain["contracts"] = normalized_contracts
    for route_id, digest in routes.items():
        if not SAFE_ID_RE.fullmatch(str(route_id)) or not ROUTE_DIGEST_RE.fullmatch(str(digest).lower()):
            raise SignerFailure("signer_config_invalid", "route policy is malformed")
    for field in (
        "maximum_native_value_wei_per_call",
        "maximum_fee_wei_per_call",
        "rolling_native_value_limit_wei",
        "rolling_window_seconds",
        "maximum_calldata_bytes",
    ):
        config[field] = parse_nonnegative_int(config.get(field), field, 2**128 - 1)
    if config["rolling_window_seconds"] == 0 or config["maximum_calldata_bytes"] < 4:
        raise SignerFailure("signer_config_invalid", "rolling window and calldata bound must be positive")
    policy_view = {
        key: value
        for key, value in config.items()
        if key not in {"keystore_path", "state_path", "socket_path"}
    }
    config["policy_hash"] = hashlib.sha256(canonical_json(policy_view)).hexdigest()
    return config


class SignerService:
    def __init__(
        self,
        config: dict[str, Any],
        *,
        web3_factory: Callable[[str], Web3] | None = None,
        now: Callable[[], int] | None = None,
    ) -> None:
        self.config = config
        self.state_path = Path(config["state_path"])
        self.web3_factory = web3_factory or (
            lambda url: Web3(Web3.HTTPProvider(url, request_kwargs={"timeout": 120}))
        )
        self.now = now or (lambda: int(time.time()))
        self._lock = threading.Lock()
        self._account: Any | None = None
        self._state = self._load_state()

    def _load_state(self) -> dict[str, Any]:
        if not self.state_path.exists():
            return {"schema": STATE_SCHEMA, "idempotency": {}, "rolling_native_value": []}
        absolute = secure_regular_file(self.state_path, "signer state", 16 * 1024 * 1024)
        try:
            state = json.loads(absolute.read_text())
        except json.JSONDecodeError as error:
            raise SignerFailure("signer_state_invalid", "signer state is not valid JSON") from error
        if (
            not isinstance(state, dict)
            or state.get("schema") != STATE_SCHEMA
            or not isinstance(state.get("idempotency"), dict)
            or not isinstance(state.get("rolling_native_value"), list)
            or len(state["idempotency"]) > MAX_IDEMPOTENCY_RECORDS
            or len(state["rolling_native_value"]) > MAX_ROLLING_VALUE_RECORDS
        ):
            raise SignerFailure("signer_state_invalid", "signer state schema mismatch")
        if any(
            not SAFE_ID_RE.fullmatch(str(key)) or not isinstance(entry, dict)
            for key, entry in state["idempotency"].items()
        ):
            raise SignerFailure("signer_state_invalid", "signer idempotency state is malformed")
        if any(
            not isinstance(row, dict)
            or isinstance(row.get("timestamp"), bool)
            or not isinstance(row.get("timestamp"), int)
            or row["timestamp"] < 0
            or isinstance(row.get("value_wei"), bool)
            or not isinstance(row.get("value_wei"), int)
            or row["value_wei"] < 0
            for row in state["rolling_native_value"]
        ):
            raise SignerFailure("signer_state_invalid", "signer rolling-value state is malformed")
        return state

    def _persist(self) -> None:
        atomic_write_json(self.state_path, self._state)

    def unlock(self, passphrase: str) -> dict[str, Any]:
        if not isinstance(passphrase, str) or len(passphrase) > 4096:
            raise SignerFailure("signer_unlock_invalid", "unlock passphrase is invalid")
        keystore = json.loads(Path(self.config["keystore_path"]).read_text())
        try:
            private_key = Account.decrypt(keystore, passphrase)
        except Exception as error:  # library provides heterogeneous errors
            raise SignerFailure("signer_unlock_failed", "keystore unlock failed") from error
        account = Account.from_key(private_key)
        del private_key
        self._account = account
        return self.status()

    def lock(self) -> dict[str, Any]:
        self._account = None
        return self.status()

    def status(self) -> dict[str, Any]:
        return {
            "ok": True,
            "schema": RESPONSE_SCHEMA,
            "op": "status",
            "ready": self._account is not None,
            "locked": self._account is None,
            "backend": self.config["backend"],
            "address": self._account.address.lower() if self._account is not None else None,
            "policy_hash": self.config["policy_hash"],
            "chains": sorted(int(value) for value in self.config["chains"]),
            "routes": sorted(self.config["routes"]),
        }

    def _authorize(self, request: dict[str, Any]) -> dict[str, Any]:
        if request.get("schema") != REQUEST_SCHEMA or request.get("op") != "submit_evm_transaction":
            raise SignerFailure("signer_request_schema_invalid", "signer request schema or operation is invalid")
        chain_id = parse_nonnegative_int(request.get("chain_id"), "chain_id", 2**63 - 1)
        chain = self.config["chains"].get(str(chain_id))
        if chain is None:
            raise SignerFailure("signer_wrong_chain", "chain is not permitted by signer policy")
        target = normalize_address(request.get("target_contract"), "target_contract")
        contract = chain["contracts"].get(target)
        if contract is None:
            raise SignerFailure("signer_wrong_contract", "contract is not permitted by signer policy")
        calldata = str(request.get("calldata") or "").lower()
        if not calldata.startswith("0x") or len(calldata) % 2 or any(c not in "0123456789abcdef" for c in calldata[2:]):
            raise SignerFailure("signer_calldata_invalid", "calldata must be canonical hexadecimal")
        calldata_bytes = (len(calldata) - 2) // 2
        if calldata_bytes < 4 or calldata_bytes > self.config["maximum_calldata_bytes"]:
            raise SignerFailure("signer_calldata_invalid", "calldata length is outside policy bounds")
        selector = calldata[:10]
        if selector not in contract["selectors"]:
            raise SignerFailure("signer_wrong_selector", "function selector is not permitted by signer policy")
        transaction_kind = str(request.get("transaction_kind") or "")
        if transaction_kind not in contract["transaction_kinds"]:
            raise SignerFailure("signer_wrong_transaction_kind", "transaction kind is not permitted for contract")
        route_id = str(request.get("route_id") or "")
        digest = str(request.get("route_config_digest") or "").lower()
        if self.config["routes"].get(route_id, "").lower() != digest:
            raise SignerFailure("signer_wrong_route", "route ID or deployment digest is not permitted")
        native_value = parse_nonnegative_int(request.get("native_value_wei"), "native_value_wei")
        maximum_fee = parse_nonnegative_int(request.get("maximum_fee_wei"), "maximum_fee_wei")
        if native_value > self.config["maximum_native_value_wei_per_call"]:
            raise SignerFailure("signer_excess_value", "native value exceeds per-call policy")
        if maximum_fee == 0 or maximum_fee > self.config["maximum_fee_wei_per_call"]:
            raise SignerFailure("signer_excess_fee", "maximum fee is outside per-call policy")
        idempotency_key = str(request.get("idempotency_key") or "")
        label = str(request.get("label") or "")
        if not SAFE_ID_RE.fullmatch(idempotency_key) or not label or len(label.encode()) > 256:
            raise SignerFailure("signer_request_invalid", "idempotency key or label is invalid")
        canonical = {
            "schema": REQUEST_SCHEMA,
            "op": "submit_evm_transaction",
            "chain_id": chain_id,
            "transaction_kind": transaction_kind,
            "target_contract": target,
            "calldata": calldata,
            "native_value_wei": native_value,
            "maximum_fee_wei": maximum_fee,
            "route_id": route_id,
            "route_config_digest": digest,
            "label": label,
            "idempotency_key": idempotency_key,
        }
        return {**canonical, "request_hash": hashlib.sha256(canonical_json(canonical)).hexdigest(), "chain": chain}

    def _check_rolling_limit(self, value_wei: int) -> None:
        cutoff = self.now() - self.config["rolling_window_seconds"]
        retained = [
            row
            for row in self._state["rolling_native_value"]
            if isinstance(row, dict)
            and int(row.get("timestamp", -1)) >= cutoff
            and int(row.get("value_wei", -1)) >= 0
        ]
        used = sum(int(row["value_wei"]) for row in retained)
        if used + value_wei > self.config["rolling_native_value_limit_wei"]:
            raise SignerFailure("signer_rolling_value_exceeded", "rolling native-value policy is exhausted")
        self._state["rolling_native_value"] = retained

    def _complete_pending(self, entry: dict[str, Any], web3: Web3) -> dict[str, Any]:
        raw = bytes.fromhex(str(entry["raw_transaction"]).removeprefix("0x"))
        try:
            web3.eth.send_raw_transaction(raw)
        except Exception as error:
            message = str(error).lower()
            if not any(marker in message for marker in ("already known", "known transaction", "nonce too low")):
                raise SignerFailure("signer_broadcast_failed", "transaction broadcast failed") from error
        receipt = web3.eth.wait_for_transaction_receipt(entry["transaction_hash"], timeout=600)
        response = {
            "ok": int(receipt.status) == 1,
            "schema": RESPONSE_SCHEMA,
            "op": "submit_evm_transaction",
            "code": None if int(receipt.status) == 1 else "signer_transaction_reverted",
            "message": "transaction finalized" if int(receipt.status) == 1 else "transaction reverted",
            "transaction_hash": Web3.to_hex(receipt.transactionHash).lower(),
            "block_number": int(receipt.blockNumber),
            "gas_used": int(receipt.gasUsed),
            "effective_gas_price": int(receipt.effectiveGasPrice),
            "idempotency_key": entry["request"]["idempotency_key"],
            "request_hash": entry["request_hash"],
        }
        entry["response"] = response
        entry.pop("raw_transaction", None)
        entry["status"] = "finalized" if response["ok"] else "reverted"
        if response["ok"]:
            self._state["rolling_native_value"].append(
                {"timestamp": self.now(), "value_wei": entry["request"]["native_value_wei"]}
            )
        self._persist()
        if not response["ok"]:
            raise SignerFailure("signer_transaction_reverted", "transaction reverted")
        return response

    def submit(self, request: dict[str, Any]) -> dict[str, Any]:
        with self._lock:
            authorized = self._authorize(request)
            key = authorized["idempotency_key"]
            existing = self._state["idempotency"].get(key)
            if existing is not None:
                if existing.get("request_hash") != authorized["request_hash"]:
                    raise SignerFailure("signer_idempotency_conflict", "idempotency key was used for another request")
                if existing.get("response", {}).get("ok") is True:
                    return {**existing["response"], "idempotent_replay": True}
                if existing.get("status") == "reverted":
                    raise SignerFailure("signer_transaction_reverted", "the idempotent transaction reverted")
            elif len(self._state["idempotency"]) >= MAX_IDEMPOTENCY_RECORDS:
                raise SignerFailure(
                    "signer_state_capacity_exhausted",
                    "signer idempotency capacity is exhausted; rotate policy and state under operator review",
                )
            if self._account is None:
                raise SignerFailure("signer_locked", "signer is locked")
            chain = authorized.pop("chain")
            web3 = self.web3_factory(chain["rpc_url"])
            if not web3.is_connected() or int(web3.eth.chain_id) != authorized["chain_id"]:
                raise SignerFailure("signer_rpc_chain_mismatch", "configured RPC is unavailable or on the wrong chain")
            if existing is not None:
                return {**self._complete_pending(existing, web3), "idempotent_replay": True}
            self._check_rolling_limit(authorized["native_value_wei"])
            sender = self._account.address
            tx_base = {
                "chainId": authorized["chain_id"],
                "from": sender,
                "to": Web3.to_checksum_address(authorized["target_contract"]),
                "data": authorized["calldata"],
                "value": authorized["native_value_wei"],
                "nonce": web3.eth.get_transaction_count(sender, "pending"),
                "type": 2,
            }
            gas = int(web3.eth.estimate_gas(tx_base))
            latest = web3.eth.get_block("latest")
            base_fee = int(latest.get("baseFeePerGas") or web3.eth.gas_price)
            priority_fee = int(getattr(web3.eth, "max_priority_fee", 0) or 0)
            max_fee_per_gas = base_fee * 2 + priority_fee
            total_fee = gas * max_fee_per_gas
            if total_fee > authorized["maximum_fee_wei"]:
                raise SignerFailure("signer_excess_fee", "estimated EIP-1559 fee exceeds request maximum")
            signed = self._account.sign_transaction(
                {**tx_base, "gas": gas, "maxFeePerGas": max_fee_per_gas, "maxPriorityFeePerGas": priority_fee}
            )
            raw = bytes(signed.raw_transaction)
            tx_hash = Web3.to_hex(Web3.keccak(raw)).lower()
            entry = {
                "request_hash": authorized["request_hash"],
                "request": {key: value for key, value in authorized.items() if key != "request_hash"},
                "status": "broadcasting",
                "transaction_hash": tx_hash,
                "raw_transaction": "0x" + raw.hex(),
                "created_at": self.now(),
            }
            self._state["idempotency"][key] = entry
            self._persist()
            return self._complete_pending(entry, web3)

    def handle(self, request: Any) -> dict[str, Any]:
        try:
            if not isinstance(request, dict) or request.get("schema") != REQUEST_SCHEMA:
                raise SignerFailure("signer_request_schema_invalid", "signer request schema mismatch")
            op = request.get("op")
            if op == "status":
                return self.status()
            if op == "unlock":
                return self.unlock(request.get("passphrase"))
            if op == "lock":
                return self.lock()
            if op == "submit_evm_transaction":
                return self.submit(request)
            raise SignerFailure("signer_operation_unsupported", "signer operation is unsupported")
        except SignerFailure as error:
            return {
                "ok": False,
                "schema": RESPONSE_SCHEMA,
                "op": request.get("op") if isinstance(request, dict) else None,
                "code": error.code,
                "message": str(error),
            }
        except Exception:
            return {
                "ok": False,
                "schema": RESPONSE_SCHEMA,
                "op": request.get("op") if isinstance(request, dict) else None,
                "code": "signer_internal_error",
                "message": "signer encountered an internal error",
            }


class RequestHandler(socketserver.StreamRequestHandler):
    def handle(self) -> None:
        raw = self.rfile.readline(MAX_MESSAGE_BYTES + 1)
        if len(raw) > MAX_MESSAGE_BYTES or not raw.endswith(b"\n"):
            response = {"ok": False, "schema": RESPONSE_SCHEMA, "code": "signer_request_too_large", "message": "request is missing a bounded newline terminator"}
        else:
            try:
                request = json.loads(raw)
            except (UnicodeDecodeError, json.JSONDecodeError):
                request = None
            response = self.server.service.handle(request)  # type: ignore[attr-defined]
        self.wfile.write(canonical_json(response) + b"\n")


class UnixSignerServer(socketserver.ThreadingUnixStreamServer):
    daemon_threads = True
    allow_reuse_address = False

    def __init__(self, socket_path: str, service: SignerService) -> None:
        self.service = service
        super().__init__(socket_path, RequestHandler)


def prepare_socket_path(socket_path: Path) -> None:
    """Validate the socket directory and remove only an owned stale socket."""

    socket_path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    parent_info = socket_path.parent.stat()
    if (
        not stat.S_ISDIR(parent_info.st_mode)
        or parent_info.st_uid != os.getuid()
        or parent_info.st_mode & 0o077
    ):
        raise SignerFailure(
            "signer_insecure_socket_directory",
            "signer socket directory must be owner-only",
        )
    if not socket_path.exists() and not socket_path.is_symlink():
        return
    info = socket_path.lstat()
    if socket_path.is_symlink() or not stat.S_ISSOCK(info.st_mode) or info.st_uid != os.getuid():
        raise SignerFailure(
            "signer_unsafe_socket_path",
            "refusing to replace a non-socket, symlink, or foreign-owned socket path",
        )
    probe = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    probe.settimeout(0.25)
    try:
        probe.connect(str(socket_path))
    except (ConnectionRefusedError, FileNotFoundError):
        socket_path.unlink()
        return
    except OSError as error:
        raise SignerFailure(
            "signer_socket_probe_failed", "could not safely classify the existing signer socket"
        ) from error
    finally:
        probe.close()
    raise SignerFailure("signer_socket_in_use", "another signer is already listening on this socket")


def serve(config_path: Path) -> None:
    config = load_config(config_path)
    socket_path = Path(config["socket_path"])
    prepare_socket_path(socket_path)
    server = UnixSignerServer(str(socket_path), SignerService(config))
    os.chmod(socket_path, 0o600)
    try:
        server.serve_forever(poll_interval=0.25)
    finally:
        server.server_close()
        socket_path.unlink(missing_ok=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    daemon = subparsers.add_parser("daemon")
    daemon.add_argument("--config", type=Path, required=True)
    create = subparsers.add_parser("create-keystore")
    create.add_argument("--keystore", type=Path, required=True)
    create.add_argument("--passphrase-file", type=Path, required=True)
    for command in ("status", "lock", "unlock"):
        client = subparsers.add_parser(command)
        client.add_argument("--socket", type=Path, required=True)
        if command == "unlock":
            client.add_argument("--passphrase-file", type=Path)
    args = parser.parse_args()
    if args.command == "daemon":
        serve(args.config)
        return
    if args.command == "create-keystore":
        print(
            json.dumps(
                create_keystore(args.keystore, args.passphrase_file),
                indent=2,
                sort_keys=True,
            )
        )
        return
    request: dict[str, Any] = {"op": args.command}
    if args.command == "unlock":
        if args.passphrase_file:
            password_path = secure_regular_file(args.passphrase_file, "passphrase file", 16 * 1024)
            request["passphrase"] = password_path.read_text().rstrip("\r\n")
        else:
            request["passphrase"] = getpass.getpass("Keystore passphrase: ")
    response = call_signer(args.socket, request, timeout=120.0)
    print(json.dumps(response, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
