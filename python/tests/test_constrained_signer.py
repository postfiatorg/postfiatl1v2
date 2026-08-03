from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import socket
import sys
from types import SimpleNamespace

from eth_account import Account
from web3 import Web3


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "python"))
SPEC = importlib.util.spec_from_file_location(
    "postfiat_signer", ROOT / "tools/postfiat-signer/postfiat_signer.py"
)
assert SPEC is not None and SPEC.loader is not None
postfiat_signer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(postfiat_signer)


ROUTE_ID = "pftl-a666-ethereum-wA666-usdc-v1"
ROUTE_DIGEST = "12ed00ca87e29554ce4b978da1710fffc0830767e84e62f08df257f727db953efdd89bcf6ea99f5634d6e5ea8aca2933"
TARGET = "0x9a0262c0572fb4db08765408eb225e207f40c3d9"


class FakeEth:
    chain_id = 1
    gas_price = 10
    max_priority_fee = 1

    def __init__(self) -> None:
        self.last_hash: bytes | None = None

    def get_transaction_count(self, _sender: str, _state: str) -> int:
        return 7

    def estimate_gas(self, _transaction: dict) -> int:
        return 21_000

    def get_block(self, _block: str) -> dict:
        return {"baseFeePerGas": 10}

    def send_raw_transaction(self, raw: bytes) -> bytes:
        self.last_hash = Web3.keccak(raw)
        return self.last_hash

    def wait_for_transaction_receipt(self, transaction_hash: str, timeout: int) -> SimpleNamespace:
        assert timeout == 600
        return SimpleNamespace(
            status=1,
            transactionHash=bytes.fromhex(transaction_hash.removeprefix("0x")),
            blockNumber=99,
            gasUsed=21_000,
            effectiveGasPrice=11,
        )


class FakeWeb3:
    def __init__(self, chain_id: int = 1) -> None:
        self.eth = FakeEth()
        self.eth.chain_id = chain_id

    def is_connected(self) -> bool:
        return True


def config(tmp_path: Path) -> dict:
    return {
        "schema": postfiat_signer.CONFIG_SCHEMA,
        "backend": "local_keystore",
        "keystore_path": str(tmp_path / "keystore.json"),
        "state_path": str(tmp_path / "state.json"),
        "socket_path": str(tmp_path / "signer.sock"),
        "chains": {
            "1": {
                "rpc_url": "https://rpc.invalid",
                "contracts": {
                    TARGET: {
                        "selectors": ["0x12345678"],
                        "transaction_kinds": ["a666_test"],
                    }
                },
            }
        },
        "routes": {ROUTE_ID: ROUTE_DIGEST},
        "maximum_native_value_wei_per_call": 100,
        "maximum_fee_wei_per_call": 1_000_000,
        "rolling_native_value_limit_wei": 150,
        "rolling_window_seconds": 3600,
        "maximum_calldata_bytes": 1024,
        "policy_hash": "ab" * 32,
    }


def request(**updates: object) -> dict:
    value = {
        "schema": postfiat_signer.REQUEST_SCHEMA,
        "op": "submit_evm_transaction",
        "chain_id": 1,
        "transaction_kind": "a666_test",
        "target_contract": TARGET,
        "calldata": "0x12345678",
        "native_value_wei": 25,
        "maximum_fee_wei": 1_000_000,
        "route_id": ROUTE_ID,
        "route_config_digest": ROUTE_DIGEST,
        "label": "test A666 transaction",
        "idempotency_key": "test-request-1",
    }
    value.update(updates)
    return value


def unlocked_service(tmp_path: Path) -> postfiat_signer.SignerService:
    service = postfiat_signer.SignerService(config(tmp_path), web3_factory=lambda _: FakeWeb3())
    account = Account.create()
    service._account = account
    return service


def test_signer_fails_closed_while_locked(tmp_path: Path) -> None:
    service = postfiat_signer.SignerService(config(tmp_path), web3_factory=lambda _: FakeWeb3())
    response = service.handle(request())
    assert response["ok"] is False
    assert response["code"] == "signer_locked"


def test_signer_rejects_wrong_chain_contract_selector_kind_route_value_and_fee(tmp_path: Path) -> None:
    service = unlocked_service(tmp_path)
    cases = [
        ({"chain_id": 2}, "signer_wrong_chain"),
        ({"target_contract": "0x" + "11" * 20}, "signer_wrong_contract"),
        ({"calldata": "0x87654321"}, "signer_wrong_selector"),
        ({"transaction_kind": "other"}, "signer_wrong_transaction_kind"),
        ({"route_config_digest": "00" * 48}, "signer_wrong_route"),
        ({"native_value_wei": 101}, "signer_excess_value"),
        ({"maximum_fee_wei": 1_000_001}, "signer_excess_fee"),
    ]
    for index, (updates, code) in enumerate(cases):
        response = service.handle(request(idempotency_key=f"bad-{index}", **updates))
        assert response["ok"] is False
        assert response["code"] == code


def test_signer_persists_idempotency_and_replays_after_restart_while_locked(tmp_path: Path) -> None:
    service = unlocked_service(tmp_path)
    first = service.handle(request())
    assert first["ok"] is True
    assert first["transaction_hash"].startswith("0x")

    restarted = postfiat_signer.SignerService(config(tmp_path), web3_factory=lambda _: FakeWeb3())
    replay = restarted.handle(request())
    assert replay["ok"] is True
    assert replay["idempotent_replay"] is True
    assert replay["transaction_hash"] == first["transaction_hash"]

    conflict = restarted.handle(request(calldata="0x1234567800"))
    assert conflict["ok"] is False
    assert conflict["code"] == "signer_idempotency_conflict"


def test_signer_enforces_rolling_native_value_limit(tmp_path: Path) -> None:
    service = unlocked_service(tmp_path)
    assert service.handle(request(native_value_wei=100))["ok"] is True
    response = service.handle(request(idempotency_key="test-request-2", native_value_wei=51))
    assert response["ok"] is False
    assert response["code"] == "signer_rolling_value_exceeded"


def test_config_rejects_group_readable_policy_file(tmp_path: Path) -> None:
    policy = tmp_path / "config.json"
    policy.write_text("{}")
    policy.chmod(0o640)
    try:
        postfiat_signer.load_config(policy)
    except postfiat_signer.SignerFailure as error:
        assert error.code == "signer_insecure_file"
    else:
        raise AssertionError("group-readable signer config was accepted")


def test_create_keystore_writes_owner_only_encrypted_key(tmp_path: Path) -> None:
    password = tmp_path / "password"
    password.write_text("correct horse battery staple\n")
    password.chmod(0o600)
    keystore = tmp_path / "private" / "relay.keystore.json"

    created = postfiat_signer.create_keystore(keystore, password)

    assert created["ok"] is True
    assert created["address"].startswith("0x")
    assert len(created["address"]) == 42
    assert keystore.stat().st_mode & 0o777 == 0o600
    encrypted = json.loads(keystore.read_text())
    private_key = Account.decrypt(encrypted, "correct horse battery staple")
    assert Account.from_key(private_key).address.lower() == created["address"]


def test_create_keystore_refuses_existing_target_and_insecure_password(tmp_path: Path) -> None:
    password = tmp_path / "password"
    password.write_text("correct horse battery staple\n")
    password.chmod(0o640)
    keystore = tmp_path / "relay.keystore.json"
    try:
        postfiat_signer.create_keystore(keystore, password)
    except postfiat_signer.SignerFailure as error:
        assert error.code == "signer_insecure_file"
    else:
        raise AssertionError("group-readable keystore password was accepted")

    password.chmod(0o600)
    keystore.write_text("do not replace\n")
    keystore.chmod(0o600)
    try:
        postfiat_signer.create_keystore(keystore, password)
    except postfiat_signer.SignerFailure as error:
        assert error.code == "signer_keystore_exists"
    else:
        raise AssertionError("existing keystore target was replaced")


def test_socket_preparation_refuses_regular_file_and_live_socket(tmp_path: Path) -> None:
    socket_path = tmp_path / "signer.sock"
    socket_path.write_text("do not overwrite")
    try:
        postfiat_signer.prepare_socket_path(socket_path)
    except postfiat_signer.SignerFailure as error:
        assert error.code == "signer_unsafe_socket_path"
    else:
        raise AssertionError("regular file at signer socket path was replaced")
    socket_path.unlink()

    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    listener.bind(str(socket_path))
    listener.listen(1)
    try:
        try:
            postfiat_signer.prepare_socket_path(socket_path)
        except postfiat_signer.SignerFailure as error:
            assert error.code == "signer_socket_in_use"
        else:
            raise AssertionError("live signer socket was replaced")
    finally:
        listener.close()
        socket_path.unlink()


def test_socket_preparation_removes_owned_stale_socket(tmp_path: Path) -> None:
    socket_path = tmp_path / "signer.sock"
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    listener.bind(str(socket_path))
    listener.close()
    assert socket_path.exists()
    postfiat_signer.prepare_socket_path(socket_path)
    assert not socket_path.exists()


def test_signer_rejects_rpc_chain_substitution(tmp_path: Path) -> None:
    service = postfiat_signer.SignerService(
        config(tmp_path), web3_factory=lambda _: FakeWeb3(chain_id=42161)
    )
    service._account = Account.create()
    response = service.handle(request())
    assert response["ok"] is False
    assert response["code"] == "signer_rpc_chain_mismatch"


def test_signer_idempotency_state_is_bounded_and_fails_closed(tmp_path: Path) -> None:
    service = unlocked_service(tmp_path)
    service._state["idempotency"] = {
        f"existing-{index}": {"status": "finalized"}
        for index in range(postfiat_signer.MAX_IDEMPOTENCY_RECORDS)
    }
    response = service.handle(request(idempotency_key="new-request"))
    assert response["ok"] is False
    assert response["code"] == "signer_state_capacity_exhausted"


def test_signer_rejects_malformed_durable_state(tmp_path: Path) -> None:
    state_path = Path(config(tmp_path)["state_path"])
    state_path.write_text(
        '{"schema":"postfiat.constrained_signer.state.v1",'
        '"idempotency":{},"rolling_native_value":[{"timestamp":true,"value_wei":1}]}\n'
    )
    state_path.chmod(0o600)
    try:
        postfiat_signer.SignerService(config(tmp_path), web3_factory=lambda _: FakeWeb3())
    except postfiat_signer.SignerFailure as error:
        assert error.code == "signer_state_invalid"
    else:
        raise AssertionError("malformed signer durable state was accepted")
