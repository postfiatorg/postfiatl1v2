"""Fail-closed provenance and semantic gate for the supplied PFTL node binary."""

from __future__ import annotations

from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any, Iterator


REPO_ROOT = Path(__file__).resolve().parents[3]
PYTHON_ROOT = REPO_ROOT / "python"
if str(PYTHON_ROOT) not in sys.path:
    sys.path.insert(0, str(PYTHON_ROOT))

from postfiat_rpc.client import PostFiatRpcClient, RpcError  # noqa: E402
import postfiat_rpc.wallet as wallet_api  # noqa: E402

from .protocol import canonical_vector  # noqa: E402


_WALLET_BINARY_LOCK = threading.Lock()


class BinaryGateError(RuntimeError):
    """The supplied node binary is not the required hardened build."""


def _json_command(binary: Path, *arguments: str) -> dict[str, Any]:
    completed = subprocess.run(
        [str(binary), *arguments],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        raise BinaryGateError(
            f"postfiat-node {' '.join(arguments[:2])} failed "
            f"({completed.returncode}): {completed.stderr.strip()}"
        )
    try:
        value = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise BinaryGateError("postfiat-node returned non-JSON output") from error
    if not isinstance(value, dict):
        raise BinaryGateError("postfiat-node JSON output must be an object")
    return value


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _reserve_port() -> int:
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def _wait_ready(path: Path, process: subprocess.Popen[str]) -> None:
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise BinaryGateError(
                f"capability-probe RPC exited early with {process.returncode}"
            )
        if path.is_file() and path.stat().st_size > 0:
            return
        time.sleep(0.05)
    raise BinaryGateError("capability-probe RPC did not become ready")


def _disk_height(binary: Path, data_dir: Path) -> int:
    return int(
        _json_command(binary, "status", "--data-dir", str(data_dir))["block_height"]
    )


def _wait_rpc_height(client: PostFiatRpcClient, expected: int) -> None:
    """Wait for the resident RPC reader to observe an offline-applied batch."""

    deadline = time.monotonic() + 10
    observed = -1
    while time.monotonic() < deadline:
        observed = int(client.status()["block_height"])
        if observed == expected:
            return
        if observed > expected:
            break
        time.sleep(0.05)
    raise BinaryGateError(
        f"capability-probe RPC height mismatch: expected={expected}, observed={observed}"
    )


@contextmanager
def wallet_binary(binary: Path) -> Iterator[None]:
    """Route wallet helpers to binaries from the hardened build directory."""

    with _WALLET_BINARY_LOCK:
        sdk = binary.with_name("postfiat-rpc-sdk")
        if not sdk.is_file() or not os.access(sdk, os.X_OK):
            raise BinaryGateError(
                "hardened build is missing adjacent executable postfiat-rpc-sdk"
            )
        original_node = wallet_api._node_bin
        original_sdk = wallet_api._sdk_bin
        wallet_api._node_bin = lambda: [str(binary)]
        wallet_api._sdk_bin = lambda: [str(sdk)]
        try:
            yield
        finally:
            wallet_api._node_bin = original_node
            wallet_api._sdk_bin = original_sdk


def _one_receipt(result: wallet_api.EscrowTransactionResult) -> dict[str, Any]:
    rows = list(result.receipts_by_validator)
    if len(rows) != 1 or not isinstance(rows[0], list) or len(rows[0]) != 1:
        raise BinaryGateError(f"unexpected capability-probe receipt shape: {rows!r}")
    receipt = rows[0][0]
    if not isinstance(receipt, dict):
        raise BinaryGateError("capability-probe receipt is not an object")
    return receipt


def _snapshot(
    client: PostFiatRpcClient,
    *,
    owner: str,
    recipient: str,
    escrow_id: str | None = None,
) -> dict[str, Any]:
    value: dict[str, Any] = {
        "status": client.status(),
        "owner": client.account(owner),
        "recipient": client.account(recipient),
    }
    if escrow_id is not None:
        value["escrow"] = client.escrow_info(escrow_id)
    return value


def _expect_rejection(call, expected_fragment: str | None = None) -> dict[str, Any]:
    try:
        call()
    except RpcError as error:
        encoded = json.dumps(error.error, sort_keys=True)
        if expected_fragment is not None and expected_fragment not in encoded:
            raise BinaryGateError(
                f"semantic probe rejected for the wrong reason: {encoded}"
            ) from error
        return {"method": error.method, "error": error.error}
    except wallet_api.WalletCommandError as error:
        encoded = str(error)
        if expected_fragment is not None and expected_fragment not in encoded:
            raise BinaryGateError(
                f"semantic probe rejected locally for the wrong reason: {encoded}"
            ) from error
        return {"method": "wallet", "error": {"message": encoded}}
    raise BinaryGateError("semantic probe unexpectedly accepted an invalid operation")


def _semantic_probe(binary: Path) -> dict[str, Any]:
    vector = canonical_vector(bytes(range(32)))
    with tempfile.TemporaryDirectory(
        prefix="postfiat-lightning-binary-gate-"
    ) as temporary:
        root = Path(temporary)
        data_dir = root / "node"
        wallets = root / "wallets"
        work = root / "work"
        ready = root / "rpc-ready.json"
        rpc_log = root / "rpc.log"
        chain_id = "postfiat-lightning-binary-gate"

        init = _json_command(
            binary,
            "init",
            "--data-dir",
            str(data_dir),
            "--chain-id",
            chain_id,
            "--node-id",
            "validator-0",
            "--validators",
            "1",
        )
        _json_command(
            binary,
            "run",
            "--unsafe-devnet-json-storage",
            "--data-dir",
            str(data_dir),
        )
        with wallet_binary(binary):
            owner = wallet_api.create_wallet(
                chain_id=chain_id, wallet_dir=wallets / "owner"
            )
            recipient = wallet_api.create_wallet(
                chain_id=chain_id, wallet_dir=wallets / "recipient"
            )
            for participant in (owner, recipient):
                wallet_api.request_faucet_pft(
                    data_dir=data_dir,
                    to_address=participant.address,
                    amount=100_000,
                    validator_data_dirs=[data_dir],
                    work_dir=work,
                )

            port = _reserve_port()
            with rpc_log.open("w") as log:
                process = subprocess.Popen(
                    [
                        str(binary),
                        "rpc-serve",
                        "--unsafe-devnet-json-storage",
                        "--data-dir",
                        str(data_dir),
                        "--port",
                        str(port),
                        "--bind-host",
                        "127.0.0.1",
                        "--ready-file",
                        str(ready),
                        "--allow-mempool-submit",
                        "--max-requests",
                        "256",
                        "--keep-alive",
                    ],
                    text=True,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                )
                try:
                    _wait_ready(ready, process)
                    client = PostFiatRpcClient(
                        f"127.0.0.1:{port}", timeout_seconds=20
                    )

                    canonical_before = _snapshot(
                        client, owner=owner.address, recipient=recipient.address
                    )
                    uppercase_rejection = _expect_rejection(
                        lambda: wallet_api.create_escrow(
                            client,
                            owner_wallet=owner,
                            destination=recipient.address,
                            amount=1_000,
                            condition=vector["condition"].upper(),
                            finish_after=0,
                            cancel_after=20,
                            work_dir=work,
                            finalize_data_dir=data_dir,
                            validator_data_dirs=[data_dir],
                        )
                    )
                    canonical_after = _snapshot(
                        client, owner=owner.address, recipient=recipient.address
                    )
                    if canonical_before != canonical_after:
                        raise BinaryGateError(
                            "non-canonical condition rejection mutated state"
                        )

                    cancel_after = _disk_height(binary, data_dir) + 4
                    create = wallet_api.create_escrow(
                        client,
                        owner_wallet=owner,
                        destination=recipient.address,
                        amount=2_000,
                        condition=vector["condition"],
                        finish_after=0,
                        cancel_after=cancel_after,
                        work_dir=work,
                        finalize_data_dir=data_dir,
                        validator_data_dirs=[data_dir],
                    )
                    create_receipt = _one_receipt(create)
                    if create_receipt.get("accepted") is not True:
                        raise BinaryGateError("canonical hashlock create was not accepted")
                    if not create.escrow_id:
                        raise BinaryGateError("canonical hashlock create returned no escrow id")
                    _wait_rpc_height(client, _disk_height(binary, data_dir))

                    wrong_before = _snapshot(
                        client,
                        owner=owner.address,
                        recipient=recipient.address,
                        escrow_id=create.escrow_id,
                    )
                    wrong = _expect_rejection(
                        lambda: wallet_api.finish_escrow(
                            client,
                            recipient_wallet=recipient,
                            escrow_id=create.escrow_id or "",
                            owner=owner.address,
                            fulfillment=canonical_vector(bytes([0xFF]) * 32)[
                                "fulfillment"
                            ],
                            work_dir=work,
                            finalize_data_dir=data_dir,
                            validator_data_dirs=[data_dir],
                        ),
                        "escrow_condition_unsatisfied",
                    )
                    wrong_after = _snapshot(
                        client,
                        owner=owner.address,
                        recipient=recipient.address,
                        escrow_id=create.escrow_id,
                    )
                    if wrong_before != wrong_after:
                        raise BinaryGateError("wrong-hashlock rejection mutated state")

                    early_before = wrong_after
                    early = _expect_rejection(
                        lambda: wallet_api.cancel_escrow(
                            client,
                            owner_wallet=owner,
                            escrow_id=create.escrow_id or "",
                            work_dir=work,
                            finalize_data_dir=data_dir,
                            validator_data_dirs=[data_dir],
                        ),
                        "escrow_cancel_too_early",
                    )
                    early_after = _snapshot(
                        client,
                        owner=owner.address,
                        recipient=recipient.address,
                        escrow_id=create.escrow_id,
                    )
                    if early_before != early_after:
                        raise BinaryGateError("early-cancel rejection mutated state")

                    observed_heights = [_disk_height(binary, data_dir)]
                    while observed_heights[-1] < cancel_after:
                        wallet_api.request_faucet_pft(
                            data_dir=data_dir,
                            to_address=recipient.address,
                            amount=1,
                            validator_data_dirs=[data_dir],
                            work_dir=work,
                        )
                        observed_heights.append(_disk_height(binary, data_dir))
                    if observed_heights[-1] != cancel_after:
                        raise BinaryGateError(
                            "probe did not stop at cancel_after boundary: "
                            f"target={cancel_after}, observed={observed_heights}"
                        )
                    _wait_rpc_height(client, cancel_after)

                    late_before = _snapshot(
                        client,
                        owner=owner.address,
                        recipient=recipient.address,
                        escrow_id=create.escrow_id,
                    )
                    late = _expect_rejection(
                        lambda: wallet_api.finish_escrow(
                            client,
                            recipient_wallet=recipient,
                            escrow_id=create.escrow_id or "",
                            owner=owner.address,
                            fulfillment=vector["fulfillment"],
                            work_dir=work,
                            finalize_data_dir=data_dir,
                            validator_data_dirs=[data_dir],
                        )
                    )
                    late_after = _snapshot(
                        client,
                        owner=owner.address,
                        recipient=recipient.address,
                        escrow_id=create.escrow_id,
                    )
                    if late_before != late_after:
                        raise BinaryGateError("late-finish rejection mutated state")

                    cancel = wallet_api.cancel_escrow(
                        client,
                        owner_wallet=owner,
                        escrow_id=create.escrow_id,
                        work_dir=work,
                        finalize_data_dir=data_dir,
                        validator_data_dirs=[data_dir],
                    )
                    cancel_receipt = _one_receipt(cancel)
                    if cancel_receipt.get("accepted") is not True:
                        raise BinaryGateError(
                            "cancel at the exclusive boundary was not accepted"
                        )
                    closed = client.escrow_info(create.escrow_id)
                    if closed.get("escrow", {}).get("state") != "canceled":
                        raise BinaryGateError("boundary cancel did not close the escrow")
                finally:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)

        return {
            "schema": "postfiat.lightning.pftl_binary_semantic_probe.v1",
            "chain_id": chain_id,
            "build_git_revision": init.get("build_git_revision"),
            "canonical_condition_rejection": uppercase_rejection,
            "canonical_create_receipt": create_receipt,
            "wrong_hashlock_rejection": wrong,
            "early_cancel_rejection": early,
            "late_finish_rejection": late,
            "boundary_cancel_receipt": cancel_receipt,
            "checks": {
                "canonical_lowercase_only": True,
                "sha256_hashlock_profile": True,
                "wrong_hashlock_mutation_free": True,
                "early_cancel_mutation_free": True,
                "late_finish_mutation_free": True,
                "half_open_finish_cancel_window": True,
            },
        }


def verify_binary(
    binary: str | Path,
    *,
    expected_revision: str,
    expected_binary_sha256: str | None = None,
    expected_wallet_sdk_sha256: str | None = None,
    run_semantic_probe: bool = True,
) -> dict[str, Any]:
    """Verify provenance and hardened escrow behavior.

    ``expected_revision`` is mandatory so a path swap cannot silently change
    the consensus build between orchestration runs.
    """

    path = Path(binary).expanduser().resolve()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise BinaryGateError(f"POSTFIAT_NODE_BIN is not executable: {path}")
    sdk_path = path.with_name("postfiat-rpc-sdk")
    if not sdk_path.is_file() or not os.access(sdk_path, os.X_OK):
        raise BinaryGateError(
            "hardened build is missing adjacent executable postfiat-rpc-sdk"
        )
    expected = expected_revision.strip().lower()
    if len(expected) < 8 or any(character not in "0123456789abcdef" for character in expected):
        raise BinaryGateError("expected revision must be at least eight lowercase hex characters")

    with tempfile.TemporaryDirectory(prefix="postfiat-lightning-provenance-") as temporary:
        data_dir = Path(temporary) / "node"
        status = _json_command(
            path,
            "init",
            "--data-dir",
            str(data_dir),
            "--chain-id",
            "postfiat-lightning-provenance",
            "--node-id",
            "validator-0",
            "--validators",
            "1",
        )
    observed = str(status.get("build_git_revision", "")).lower()
    if observed in {"", "unknown"}:
        raise BinaryGateError("node binary does not report a build git revision")
    if not (expected.startswith(observed) or observed.startswith(expected)):
        raise BinaryGateError(
            f"node build revision mismatch: expected {expected}, observed {observed}"
        )
    observed_binary_sha256 = _sha256_file(path)
    observed_wallet_sdk_sha256 = _sha256_file(sdk_path)
    for label, expected_sha, observed_sha in (
        ("node binary", expected_binary_sha256, observed_binary_sha256),
        ("wallet SDK", expected_wallet_sdk_sha256, observed_wallet_sdk_sha256),
    ):
        if expected_sha is None:
            continue
        normalized = expected_sha.strip().lower()
        if (
            len(normalized) != 64
            or any(character not in "0123456789abcdef" for character in normalized)
        ):
            raise BinaryGateError(f"expected {label} SHA-256 is not canonical")
        if normalized != observed_sha:
            raise BinaryGateError(
                f"{label} SHA-256 mismatch: expected {normalized}, "
                f"observed {observed_sha}"
            )

    help_result = subprocess.run(
        [str(path), "--help"],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    required_commands = (
        "transport-peer-certified-mempool-round",
        "transport-validator-serve",
        "rpc-serve",
        "escrow_info",
        "asset_info",
    )
    missing = [name for name in required_commands if name not in help_result.stdout]
    if help_result.returncode != 0 or missing:
        raise BinaryGateError(
            f"node binary is missing required harness commands: {', '.join(missing)}"
        )

    semantic = _semantic_probe(path) if run_semantic_probe else None
    if (
        _sha256_file(path) != observed_binary_sha256
        or _sha256_file(sdk_path) != observed_wallet_sdk_sha256
    ):
        raise BinaryGateError("hardened binary pair changed during verification")
    return {
        "schema": "postfiat.lightning.pftl_binary_gate.v1",
        "binary": str(path),
        "binary_sha256": observed_binary_sha256,
        "wallet_sdk": str(sdk_path),
        "wallet_sdk_sha256": observed_wallet_sdk_sha256,
        "expected_binary_sha256": expected_binary_sha256,
        "expected_wallet_sdk_sha256": expected_wallet_sdk_sha256,
        "expected_git_revision": expected,
        "observed_git_revision": observed,
        "build_profile": status.get("build_profile"),
        "protocol_version": status.get("protocol_version"),
        "required_commands": list(required_commands),
        "semantic_probe": semantic,
        "verified": semantic is not None,
    }
