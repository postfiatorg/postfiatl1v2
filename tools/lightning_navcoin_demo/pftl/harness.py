"""Six-validator synthetic PFTL harness for the Lightning/NAVcoin demo.

This module never modifies consensus code.  It consumes a caller-supplied
``postfiat-node`` binary, gates its revision and escrow semantics, and drives
the existing peer-certified transport/RPC/wallet surfaces.
"""

from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time
from typing import Any, Callable, Iterator, Sequence


REPO_ROOT = Path(__file__).resolve().parents[3]
PYTHON_ROOT = REPO_ROOT / "python"
if str(PYTHON_ROOT) not in sys.path:
    sys.path.insert(0, str(PYTHON_ROOT))

from postfiat_rpc.client import PostFiatRpcClient  # noqa: E402
import postfiat_rpc.wallet as wallet_api  # noqa: E402

from .binary_gate import BinaryGateError, verify_binary, wallet_binary  # noqa: E402
from .protocol import decode_condition, decode_fulfillment, fulfillment_satisfies  # noqa: E402


VALIDATOR_COUNT = 6
MANIFEST_SCHEMA = "postfiat.lightning.pftl_devnet.v1"
RUNTIME_SCHEMA = "postfiat.lightning.pftl_rpc_runtime.v1"
EFFECT_SCHEMA = "postfiat.lightning.pftl_effects.v1"
TEST_ASSET_CODE = "LNNAVTEST"
TEST_ASSET_PRECISION = 6
TEST_ASSET_MAX_SUPPLY = 10_000_000_000
COORDINATOR_INVENTORY = 2_000_000_000
USER_INVENTORY = 1_000_000_000
TRUSTLINE_LIMIT = 5_000_000_000
PFT_FUNDING = 2_000_000
_EFFECT_KEY = re.compile(r"^[A-Za-z0-9._:-]{1,128}$")
_PUBLIC_SECRET_FIELD_MARKERS = (
    "secret",
    "preimage",
    "fulfillment",
    "seed",
    "mnemonic",
    "macaroon",
    "private_key",
    "wallet_password",
)
_STATE_FILES = (
    "genesis.json",
    "governance.json",
    "ledger.json",
    "blocks.json",
    "batch_archive.json",
    "mempool.json",
    "ordered_batches.json",
    "receipts.json",
    "shielded.json",
    "bridge.json",
    "faucet_account.json",
    "faucet_key.json",
    "validator_registry.json",
    "validator_registry_genesis.json",
    "chain_tip.json",
)


class HarnessError(RuntimeError):
    """The synthetic PFTL environment violated a harness invariant."""


def _atomic_json(path: Path, value: Any, *, mode: int = 0o600) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(temporary, mode)
    os.replace(temporary, path)


def _read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise HarnessError(f"cannot read valid JSON from {path}: {error}") from error


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _assert_secret_free_public(value: Any, path: str = "$") -> None:
    """Reject secret-bearing field names from public harness artifacts."""

    if isinstance(value, dict):
        for key, child in value.items():
            key_text = str(key).lower()
            child_path = f"{path}.{key}"
            if any(marker in key_text for marker in _PUBLIC_SECRET_FIELD_MARKERS):
                raise HarnessError(
                    f"secret-bearing field is forbidden in public evidence: {child_path}"
                )
            _assert_secret_free_public(child, child_path)
    elif isinstance(value, (list, tuple)):
        for index, child in enumerate(value):
            _assert_secret_free_public(child, f"{path}[{index}]")


def _command_audit_label(command: Sequence[str]) -> str:
    encoded = json.dumps(
        list(command),
        ensure_ascii=True,
        separators=(",", ":"),
    ).encode("ascii")
    executable = Path(command[0]).name if command else "<empty>"
    return (
        f"executable={executable}; argv_count={len(command)}; "
        f"argv_sha256={hashlib.sha256(encoded).hexdigest()}"
    )


def _run_json_value(
    command: Sequence[str],
    *,
    cwd: Path = REPO_ROOT,
    env: dict[str, str] | None = None,
    timeout: float = 120,
) -> Any:
    completed = subprocess.run(
        list(command),
        cwd=cwd,
        env=env,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )
    if completed.returncode != 0:
        stderr_bytes = completed.stderr.encode("utf-8", errors="replace")
        raise HarnessError(
            f"command failed ({completed.returncode}); "
            f"{_command_audit_label(command)}; "
            f"stderr_bytes={len(stderr_bytes)}; "
            f"stderr_sha256={hashlib.sha256(stderr_bytes).hexdigest()}"
        )
    try:
        value = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise HarnessError(
            f"command returned non-JSON output; {_command_audit_label(command)}"
        ) from error
    return value


def _run_json(
    command: Sequence[str],
    *,
    cwd: Path = REPO_ROOT,
    env: dict[str, str] | None = None,
    timeout: float = 120,
) -> dict[str, Any]:
    value = _run_json_value(command, cwd=cwd, env=env, timeout=timeout)
    if not isinstance(value, dict):
        raise HarnessError(
            f"command JSON is not an object; {_command_audit_label(command)}"
        )
    return value


def _safe_root(root: str | Path) -> Path:
    path = Path(root).expanduser().resolve()
    forbidden = {
        Path("/"),
        Path.home().resolve(),
        REPO_ROOT.resolve(),
        REPO_ROOT.parent.resolve(),
    }
    if path in forbidden or len(path.parts) < 4:
        raise HarnessError(f"refusing unsafe devnet root: {path}")
    return path


def _wait_file(path: Path, process: subprocess.Popen[str], timeout: float = 30) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise HarnessError(f"service exited before readiness: {path}")
        if path.is_file() and path.stat().st_size:
            return
        time.sleep(0.05)
    raise HarnessError(f"service readiness timed out: {path}")


def _wallet_from_manifest(
    manifest: dict[str, Any], role: str
) -> wallet_api.TransparentWallet:
    role_value = manifest["roles"][role]
    return wallet_api.load_wallet(
        wallet_dir=role_value["wallet_dir"],
        chain_id=manifest["chain_id"],
    )


@dataclass(frozen=True)
class FinalizedEffect:
    """Secret-free coordinator-facing PFTL effect evidence."""

    accepted: bool
    reason: str
    tx_id: str
    finalized_height: int
    state_root: str
    block_tip_hash: str
    agreeing_validator_count: int
    validator_count: int
    receipt_count: int
    certificate_id: str
    effect_key: str
    escrow_id: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "accepted": self.accepted,
            "reason": self.reason,
            "tx_id": self.tx_id,
            "finalized_height": self.finalized_height,
            "state_root": self.state_root,
            "block_tip_hash": self.block_tip_hash,
            "agreeing_validator_count": self.agreeing_validator_count,
            "validator_count": self.validator_count,
            "receipt_count": self.receipt_count,
            "certificate_id": self.certificate_id,
            "effect_key": self.effect_key,
            "escrow_id": self.escrow_id,
        }


class PftlDevnet:
    """A six-replica local devnet driven through peer-certified finality."""

    def __init__(self, root: str | Path) -> None:
        self.root = _safe_root(root)
        self.manifest_path = self.root / "manifest.json"
        manifest = _read_json(self.manifest_path)
        if not isinstance(manifest, dict) or manifest.get("schema") != MANIFEST_SCHEMA:
            raise HarnessError(f"invalid PFTL devnet manifest: {self.manifest_path}")
        if manifest.get("validator_count") != VALIDATOR_COUNT:
            raise HarnessError("Lightning demo requires exactly six PFTL validators")
        if not str(manifest.get("chain_id", "")).startswith(
            ("local-", "devnet-", "regtest-")
        ):
            raise HarnessError("manifest chain is not a synthetic PFTL domain")
        self.manifest = manifest
        self.binary = Path(manifest["binary"]["path"]).resolve()
        if not self.binary.is_file() or not os.access(self.binary, os.X_OK):
            raise HarnessError(f"manifest node binary is not executable: {self.binary}")
        observed_sha = self._sha256_binary()
        if observed_sha != manifest["binary"]["sha256"]:
            raise HarnessError("POSTFIAT_NODE_BIN content changed since initialization")
        sdk = self.binary.with_name("postfiat-rpc-sdk")
        if (
            not sdk.is_file()
            or not os.access(sdk, os.X_OK)
            or _sha256_file(sdk) != manifest["binary"]["wallet_sdk_sha256"]
        ):
            raise HarnessError(
                "adjacent postfiat-rpc-sdk changed since initialization"
            )

    @classmethod
    def initialize(
        cls,
        root: str | Path,
        *,
        binary: str | Path,
        expected_revision: str,
        expected_binary_sha256: str | None = None,
        expected_wallet_sdk_sha256: str | None = None,
        chain_id: str = "local-postfiat-lightning-navcoin-demo",
        p2p_base_port: int = 29660,
        rpc_base_port: int = 30660,
    ) -> "PftlDevnet":
        """Create and bootstrap the synthetic six-validator environment."""

        target = _safe_root(root)
        if target.exists() and any(target.iterdir()):
            raise HarnessError(f"devnet root must be absent or empty: {target}")
        if not (1024 <= p2p_base_port <= 65525 and 1024 <= rpc_base_port <= 65530):
            raise HarnessError("PFTL port bases must leave room for six validators")
        p2p_ports = {
            p2p_base_port + (2 * index) for index in range(VALIDATOR_COUNT)
        }
        if p2p_ports & set(
            range(rpc_base_port, rpc_base_port + VALIDATOR_COUNT)
        ):
            raise HarnessError("PFTL transport and RPC port ranges overlap")
        if not chain_id.startswith(("local-", "devnet-", "regtest-")):
            raise HarnessError(
                "synthetic PFTL chain_id must start local-, devnet-, or regtest-"
            )

        try:
            gate = verify_binary(
                binary,
                expected_revision=expected_revision,
                expected_binary_sha256=expected_binary_sha256,
                expected_wallet_sdk_sha256=expected_wallet_sdk_sha256,
                run_semantic_probe=True,
            )
        except BinaryGateError as error:
            raise HarnessError(f"hardened PFTL binary gate failed: {error}") from error

        if not target.exists():
            target.mkdir(parents=True)
        os.chmod(target, 0o700)
        for child in ("nodes", "private", "public", "runtime", "evidence"):
            path = target / child
            path.mkdir()
            os.chmod(path, 0o700 if child in {"private", "runtime"} else 0o755)

        binary_path = Path(gate["binary"])
        node0 = target / "nodes" / "validator-0"
        init = _run_json(
            [
                str(binary_path),
                "init",
                "--data-dir",
                str(node0),
                "--chain-id",
                chain_id,
                "--node-id",
                "validator-0",
                "--validators",
                str(VALIDATOR_COUNT),
            ]
        )
        _run_json(
            [
                str(binary_path),
                "validator-keys",
                "--data-dir",
                str(node0),
                "--validators",
                str(VALIDATOR_COUNT),
            ]
        )
        combined_keys = _read_json(node0 / "validator_keys.json")
        private_keys = target / "private" / "validator-keys"
        private_keys.mkdir()
        os.chmod(private_keys, 0o700)
        _atomic_json(private_keys / "combined.json", combined_keys)

        for index in range(VALIDATOR_COUNT):
            node_id = f"validator-{index}"
            node_dir = target / "nodes" / node_id
            if index:
                node_dir.mkdir()
                for filename in _STATE_FILES:
                    source = node0 / filename
                    if source.exists():
                        shutil.copy2(source, node_dir / filename)
                _atomic_json(
                    node_dir / "node_state.json",
                    {"node_id": node_id, "status": "initialized", "last_run_unix": 0},
                )
            local_keys = {
                "validators": [
                    row
                    for row in combined_keys.get("validators", [])
                    if row.get("node_id") == node_id
                ]
            }
            if len(local_keys["validators"]) != 1:
                raise HarnessError(f"did not resolve exactly one key for {node_id}")
            _atomic_json(private_keys / f"{node_id}.json", local_keys)
            _atomic_json(node_dir / "validator_keys.json", local_keys)
            os.chmod(node_dir / "faucet_key.json", 0o600)
            _run_json(
                [
                    str(binary_path),
                    "run",
                    "--unsafe-devnet-json-storage",
                    "--data-dir",
                    str(node_dir),
                ]
            )

        topology = target / "public" / "topology.json"
        _run_json(
            [
                str(binary_path),
                "topology",
                "--chain-id",
                chain_id,
                "--validators",
                str(VALIDATOR_COUNT),
                "--base-port",
                str(p2p_base_port),
                "--hosts",
                ",".join(["127.0.0.1"] * VALIDATOR_COUNT),
                "--rpc-base-port",
                str(rpc_base_port),
                "--output",
                str(topology),
            ]
        )

        roles: dict[str, Any] = {}
        with wallet_binary(binary_path):
            for role in ("issuer", "coordinator", "user"):
                wallet_dir = target / "private" / "wallets" / role
                wallet = wallet_api.create_wallet(
                    chain_id=chain_id, wallet_dir=wallet_dir
                )
                roles[role] = {
                    "address": wallet.address,
                    "wallet_dir": str(wallet_dir),
                }

        manifest = {
            "schema": MANIFEST_SCHEMA,
            "scope": "fully synthetic; local-only; zero real value",
            "chain_id": chain_id,
            "genesis_hash": init["genesis_hash"],
            "protocol_version": init["protocol_version"],
            "validator_count": VALIDATOR_COUNT,
            "binary": {
                "path": str(binary_path),
                "sha256": gate["binary_sha256"],
                "wallet_sdk": gate["wallet_sdk"],
                "wallet_sdk_sha256": gate["wallet_sdk_sha256"],
                "git_revision": gate["observed_git_revision"],
                "expected_git_revision": gate["expected_git_revision"],
                "gate_report": str(target / "evidence" / "binary-gate.json"),
            },
            "topology": str(topology),
            "ports": {
                "p2p_base": p2p_base_port,
                "rpc_base": rpc_base_port,
            },
            "roles": roles,
            "asset": None,
            "created_unix": int(time.time()),
        }
        _atomic_json(target / "evidence" / "binary-gate.json", gate, mode=0o644)
        _atomic_json(target / "manifest.json", manifest, mode=0o644)

        devnet = cls(target)
        devnet.start_rpc()
        try:
            asset = devnet._bootstrap_test_asset()
            devnet.manifest["asset"] = asset
            _atomic_json(devnet.manifest_path, devnet.manifest, mode=0o644)
            snapshot = devnet.consensus_snapshot(
                asset_id=asset["asset_id"],
                accounts=[
                    roles["coordinator"]["address"],
                    roles["user"]["address"],
                ],
            )
            _atomic_json(
                target / "evidence" / "bootstrap-snapshot.json",
                snapshot,
                mode=0o644,
            )
        finally:
            devnet.stop_rpc()
        return cls(target)

    def _sha256_binary(self) -> str:
        import hashlib

        digest = hashlib.sha256()
        with self.binary.open("rb") as handle:
            while chunk := handle.read(1024 * 1024):
                digest.update(chunk)
        return digest.hexdigest()

    def node_dir(self, index: int) -> Path:
        if not 0 <= index < VALIDATOR_COUNT:
            raise HarnessError(f"validator index out of range: {index}")
        return self.root / "nodes" / f"validator-{index}"

    def key_file(self, index: int) -> Path:
        return (
            self.root
            / "private"
            / "validator-keys"
            / f"validator-{index}.json"
        )

    def endpoint(self, index: int) -> str:
        return f"127.0.0.1:{int(self.manifest['ports']['rpc_base']) + index}"

    def rpc_clients(self) -> list[PostFiatRpcClient]:
        return [
            PostFiatRpcClient(self.endpoint(index), timeout_seconds=30)
            for index in range(VALIDATOR_COUNT)
        ]

    def _runtime_path(self) -> Path:
        return self.root / "runtime" / "rpc-processes.json"

    def start_rpc(self) -> dict[str, Any]:
        """Start one local RPC endpoint per validator."""

        runtime_path = self._runtime_path()
        if runtime_path.exists():
            runtime = _read_json(runtime_path)
            if all(self._pid_matches(row) for row in runtime.get("processes", [])):
                return runtime
            raise HarnessError("stale or foreign RPC runtime file; run stop after inspection")

        processes: list[dict[str, Any]] = []
        started: list[subprocess.Popen[str]] = []
        try:
            for index in range(VALIDATOR_COUNT):
                node_id = f"validator-{index}"
                log_path = self.root / "runtime" / f"{node_id}.rpc.log"
                ready_path = self.root / "runtime" / f"{node_id}.rpc.ready.json"
                ready_path.unlink(missing_ok=True)
                log = log_path.open("a")
                process = subprocess.Popen(
                    [
                        str(self.binary),
                        "rpc-serve",
                        "--unsafe-devnet-json-storage",
                        "--data-dir",
                        str(self.node_dir(index)),
                        "--port",
                        str(int(self.manifest["ports"]["rpc_base"]) + index),
                        "--bind-host",
                        "127.0.0.1",
                        "--ready-file",
                        str(ready_path),
                        "--allow-mempool-submit",
                        "--max-requests",
                        "100000",
                        "--keep-alive",
                    ],
                    cwd=REPO_ROOT,
                    text=True,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    start_new_session=True,
                )
                log.close()
                started.append(process)
                _wait_file(ready_path, process)
                processes.append(
                    {
                        "node_id": node_id,
                        "pid": process.pid,
                        "endpoint": self.endpoint(index),
                        "data_dir": str(self.node_dir(index)),
                        "log": str(log_path),
                    }
                )
            runtime = {
                "schema": RUNTIME_SCHEMA,
                "binary_sha256": self.manifest["binary"]["sha256"],
                "processes": processes,
            }
            _atomic_json(runtime_path, runtime)
            self._require_rpc_domains()
            return runtime
        except Exception:
            for process in started:
                if process.poll() is None:
                    process.terminate()
            for process in started:
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
            raise

    def _pid_matches(self, row: dict[str, Any]) -> bool:
        pid = row.get("pid")
        if not isinstance(pid, int) or pid <= 1:
            return False
        command_path = Path("/proc") / str(pid) / "cmdline"
        try:
            command = command_path.read_bytes().replace(b"\x00", b" ").decode()
        except OSError:
            return False
        return str(self.binary) in command and str(row.get("data_dir", "")) in command

    def stop_rpc(self) -> None:
        """Stop only the RPC processes recorded for this exact devnet."""

        runtime_path = self._runtime_path()
        if not runtime_path.exists():
            return
        runtime = _read_json(runtime_path)
        rows = runtime.get("processes", [])
        for row in rows:
            if self._pid_matches(row):
                os.kill(int(row["pid"]), signal.SIGTERM)
        deadline = time.monotonic() + 10
        pending = [row for row in rows if self._pid_matches(row)]
        while pending and time.monotonic() < deadline:
            time.sleep(0.05)
            pending = [row for row in pending if self._pid_matches(row)]
        for row in pending:
            if self._pid_matches(row):
                os.kill(int(row["pid"]), signal.SIGKILL)
        runtime_path.unlink(missing_ok=True)

    def _require_rpc_domains(self) -> None:
        values = [client.server_capabilities() for client in self.rpc_clients()]
        for value in values:
            if (
                value.get("chain_id") != self.manifest["chain_id"]
                or value.get("genesis_hash") != self.manifest["genesis_hash"]
                or value.get("validator_count") != VALIDATOR_COUNT
                or value.get("mempool_submit_enabled") is not True
            ):
                raise HarnessError(f"RPC domain/capability mismatch: {value}")

    def statuses(self) -> list[dict[str, Any]]:
        return [client.status() for client in self.rpc_clients()]

    def _next_proposer(self) -> tuple[int, int, dict[str, Any]]:
        statuses = self.statuses()
        heights = {int(value["block_height"]) for value in statuses}
        roots = {value["state_root"] for value in statuses}
        tips = {value["block_tip_hash"] for value in statuses}
        if len(heights) != 1 or len(roots) != 1 or len(tips) != 1:
            raise HarnessError("cannot submit while validator replicas diverge")
        height = heights.pop() + 1
        report = _run_json(
            [
                str(self.binary),
                "block-proposer",
                "--data-dir",
                str(self.node_dir(0)),
                "--height",
                str(height),
                "--view",
                "0",
            ]
        )
        proposer = report.get("proposer")
        if not isinstance(proposer, str) or not proposer.startswith("validator-"):
            raise HarnessError(f"invalid proposer report: {report}")
        index = int(proposer.removeprefix("validator-"))
        return height, index, report

    @contextmanager
    def _validator_services(
        self,
        *,
        source_index: int,
        height: int,
        offline_index: int | None,
    ) -> Iterator[list[subprocess.Popen[str]]]:
        processes: list[subprocess.Popen[str]] = []
        readiness: list[tuple[Path, subprocess.Popen[str]]] = []
        logs = []
        try:
            for index in range(VALIDATOR_COUNT):
                if index in {source_index, offline_index}:
                    continue
                node_id = f"validator-{index}"
                round_dir = self.root / "runtime" / "transport" / f"height-{height}"
                round_dir.mkdir(parents=True, exist_ok=True)
                ready = round_dir / f"{node_id}.ready.json"
                ready.unlink(missing_ok=True)
                log = (round_dir / f"{node_id}.log").open("w")
                error_log = (round_dir / f"{node_id}.err").open("w")
                environment = dict(os.environ)
                environment["POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE"] = str(ready)
                # A readiness marker is emitted only after the production
                # verifier caches are warm.  The harness never exercises
                # shielded proving, but it must not weaken validator startup
                # readiness to obtain a faster synthetic test.
                environment["POSTFIAT_PREWARM_SHIELDED_VERIFIER"] = "1"
                environment["POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER"] = "1"
                environment[
                    "POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER"
                ] = "1"
                process = subprocess.Popen(
                    [
                        str(self.binary),
                        "transport-validator-serve",
                        "--unsafe-devnet-json-storage",
                        "--unsafe-devnet-file-signer",
                        "--data-dir",
                        str(self.node_dir(index)),
                        "--topology",
                        self.manifest["topology"],
                        "--key-file",
                        str(self.key_file(index)),
                        "--vote-dir",
                        str(round_dir / f"{node_id}-votes"),
                        "--max-connections",
                        "2",
                        "--timeout-ms",
                        "10000",
                        "--require-signed-proposal",
                        "--event-log",
                        str(round_dir / f"{node_id}.events.jsonl"),
                    ],
                    cwd=REPO_ROOT,
                    env=environment,
                    text=True,
                    stdout=log,
                    stderr=error_log,
                )
                logs.extend((log, error_log))
                processes.append(process)
                readiness.append((ready, process))
            # Verifier-cache warmup is intentionally mandatory when a
            # readiness marker is requested. Launch every peer first so the
            # independent warmups run concurrently instead of serializing the
            # six-validator test.
            for ready, process in readiness:
                _wait_file(ready, process)
            yield processes
        finally:
            for process in processes:
                if process.poll() is None:
                    process.terminate()
            for process in processes:
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            for log in logs:
                log.close()

    def _write_public_finality_proof(
        self,
        *,
        height: int,
        effect_key: str,
        source_index: int,
        round_value: dict[str, Any],
        certification: dict[str, Any],
        post_statuses: list[dict[str, Any]],
        expected_rejection_code: str | None,
    ) -> dict[str, str]:
        """Publish the cryptographic finality material without transaction data."""

        private_finality_root = (self.root / "private" / "finality").resolve()

        def certified_file(field: str) -> Path:
            raw = certification.get(field)
            if not isinstance(raw, str) or not raw:
                raise HarnessError(f"certification is missing {field}")
            path = Path(raw).resolve()
            if private_finality_root not in path.parents or not path.is_file():
                raise HarnessError(f"certification {field} escapes its private round")
            return path

        certificate = _read_json(certified_file("certificate_file"))
        proposal = _read_json(certified_file("proposal_file"))
        registries = [
            _read_json(self.node_dir(index) / "validator_registry.json")
            for index in range(VALIDATOR_COUNT)
        ]
        registry_encodings = {
            json.dumps(value, sort_keys=True, separators=(",", ":"))
            for value in registries
        }
        if len(registry_encodings) != 1:
            raise HarnessError("validator public registries do not agree 6/6")
        registry = registries[0]

        validator_ids = {f"validator-{index}" for index in range(VALIDATOR_COUNT)}
        registry_rows = registry.get("validators", [])
        registry_by_id = {
            row.get("node_id"): row
            for row in registry_rows
            if isinstance(row, dict)
        }
        if (
            len(registry_rows) != VALIDATOR_COUNT
            or set(registry_by_id) != validator_ids
            or any(
                not isinstance(row.get("public_key_hex"), str)
                or not row["public_key_hex"]
                or not isinstance(row.get("algorithm_id"), str)
                or not row["algorithm_id"]
                for row in registry_rows
            )
        ):
            raise HarnessError("public validator registry is incomplete")

        body = certificate.get("certificate", {})
        votes = body.get("votes", []) if isinstance(body, dict) else []
        vote_validators = [
            vote.get("validator") for vote in votes if isinstance(vote, dict)
        ]
        proposal_signature = proposal.get("signature", {})
        proposer = f"validator-{source_index}"
        if not votes or (
            certificate.get("certificate_id") != certification.get("certificate_id")
            or certificate.get("proposal_hash") != certification.get("proposal_hash")
            or certificate.get("block_height") != height
            or proposal.get("block_height") != height
            or proposal.get("proposer") != proposer
            or proposal.get("batch_id") != certification.get("batch_id")
            or proposal.get("state_root") != post_statuses[0].get("state_root")
            or body.get("registry_root") != votes[0].get("registry_root")
        ):
            raise HarnessError("certificate/proposal/public state linkage failed")
        if (
            len(votes) != certification.get("vote_count")
            or set(body.get("validators", [])) != validator_ids
            or type(body.get("quorum")) is not int
            or not 5 <= body["quorum"] <= len(votes)
            or len(set(vote_validators)) != len(votes)
            or not set(vote_validators).issubset(validator_ids)
            or any(
                vote.get("accept") is not True
                or not isinstance(vote.get("signature_hex"), str)
                or not vote["signature_hex"]
                or vote.get("registry_root") != body.get("registry_root")
                for vote in votes
                if isinstance(vote, dict)
            )
            or len(vote_validators) != len(votes)
        ):
            raise HarnessError("certificate vote material is incomplete")
        if (
            not isinstance(proposal_signature, dict)
            or proposal_signature.get("signer") != proposer
            or proposal_signature.get("public_key_hex")
            != registry_by_id[proposer].get("public_key_hex")
            or proposal_signature.get("algorithm_id")
            != registry_by_id[proposer].get("algorithm_id")
            or not isinstance(proposal_signature.get("signature_hex"), str)
            or not proposal_signature["signature_hex"]
        ):
            raise HarnessError("signed proposal is not bound to the public registry")

        hot_finality = round_value.get("local_hot_finality", [])
        receipts = [
            row["receipt"]
            for row in hot_finality
            if isinstance(row, dict) and isinstance(row.get("receipt"), dict)
        ]
        if (
            len(receipts) != round_value.get("local_receipt_count")
            or proposal.get("receipt_count") != len(receipts)
            or proposal.get("receipt_ids")
            != [receipt.get("tx_id") for receipt in receipts]
        ):
            raise HarnessError("certified proposal receipt aggregate is incomplete")

        proof = {
            "schema": "postfiat.lightning.pftl_public_finality_proof.v1",
            "scope": "fully synthetic; local-only; zero real value",
            "effect_key": effect_key,
            "chain": {
                "chain_id": self.manifest["chain_id"],
                "genesis_hash": self.manifest["genesis_hash"],
                "protocol_version": self.manifest["protocol_version"],
                "height": height,
            },
            "batch": {
                "kind": proposal.get("batch_kind"),
                "id": proposal.get("batch_id"),
                "payload_hash": proposal.get("payload_hash"),
            },
            "proposal": proposal,
            "certificate": certificate,
            "validator_registry": registry,
            "receipt_aggregate": {
                "receipt_count": len(receipts),
                "accepted_count": round_value.get("local_accepted_count"),
                "rejected_count": round_value.get("local_rejected_count"),
                "expected_rejection_code": expected_rejection_code,
                "receipts": receipts,
            },
            "post_statuses": post_statuses,
            "checks": {
                "certificate_id": certification.get("certificate_id"),
                "proposal_hash": certification.get("proposal_hash"),
                "vote_count": len(votes),
                "quorum": body.get("quorum"),
                "validator_registry_count": len(registry_rows),
                "post_status_count": len(post_statuses),
                "post_status_converged": True,
                "transaction_payload_excluded": True,
                "sensitive_fields_excluded": True,
            },
        }
        _assert_secret_free_public(proof)
        destination = (
            self.root / "evidence" / "finality" / f"{height}-{effect_key}.json"
        )
        _atomic_json(destination, proof, mode=0o644)
        return {
            "schema": proof["schema"],
            "path": str(destination),
            "sha256": _sha256_file(destination),
        }

    def _certify(
        self,
        *,
        height: int,
        source_index: int,
        effect_key: str,
        signed_kind: str | None = None,
        signed_value: dict[str, Any] | None = None,
        batch_file: Path | None = None,
        offline_index: int | None = None,
        expected_rejection_code: str | None = None,
    ) -> dict[str, Any]:
        if signed_value is not None and batch_file is not None:
            raise HarnessError("certification cannot take both signed JSON and a batch file")
        if signed_value is not None and signed_kind not in {"asset", "escrow"}:
            raise HarnessError(f"unsupported signed operation kind: {signed_kind}")
        artifact = self.root / "private" / "finality" / f"{height}-{effect_key}"
        artifact.mkdir(parents=True, exist_ok=False)
        os.chmod(artifact, 0o700)
        with self._validator_services(
            source_index=source_index,
            height=height,
            offline_index=offline_index,
        ) as services:
            if batch_file is not None:
                command = [
                    str(self.binary),
                    "transport-peer-certified-batch-round",
                    "--data-dir",
                    str(self.node_dir(source_index)),
                    "--topology",
                    self.manifest["topology"],
                    "--batch-kind",
                    "transparent",
                    "--batch-file",
                    str(batch_file),
                    "--key-file",
                    str(self.key_file(source_index)),
                    "--artifact-dir",
                    str(artifact / "peer-certified-round"),
                    "--height",
                    str(height),
                    "--view",
                    "0",
                    "--timeout-ms",
                    "10000",
                    "--send-retries",
                    "1",
                    "--retry-backoff-ms",
                    "100",
                    "--require-local-proposer",
                ]
            else:
                command = [
                    str(self.binary),
                    "transport-peer-certified-mempool-round",
                    "--data-dir",
                    str(self.node_dir(source_index)),
                    "--topology",
                    self.manifest["topology"],
                    "--key-file",
                    str(self.key_file(source_index)),
                    "--artifact-dir",
                    str(artifact),
                    "--height",
                    str(height),
                    "--view",
                    "0",
                    "--timeout-ms",
                    "10000",
                    "--send-retries",
                    "1",
                    "--retry-backoff-ms",
                    "100",
                    "--max-transactions",
                    "1",
                    "--require-local-proposer",
                ]
                if signed_value is not None:
                    signed_flag = (
                        "--signed-asset-transaction-json"
                        if signed_kind == "asset"
                        else "--signed-escrow-transaction-json"
                    )
                    command.extend(
                        [
                            signed_flag,
                            json.dumps(signed_value, separators=(",", ":")),
                        ]
                    )
            if offline_index is not None:
                command.append("--allow-peer-failures")
            report = _run_json(command, timeout=120)
            for process in services:
                try:
                    process.wait(timeout=20)
                except subprocess.TimeoutExpired as error:
                    raise HarnessError("validator service did not finish the round") from error

        round_value = report["round"] if "round" in report else report
        certification = round_value.get("certification", {})
        quorum = certification.get("vote_count", 0)
        expected_minimum = VALIDATOR_COUNT if offline_index is None else VALIDATOR_COUNT - 1
        hot_finality = round_value.get("local_hot_finality", [])
        if expected_rejection_code is None:
            if offline_index is None:
                round_valid = (
                    report.get("round_ok") is True
                    and round_value.get("round_ok") is True
                    and round_value.get("local_apply_verified") is True
                )
            else:
                # `allow-peer-failures` deliberately reports the certified
                # send to the absent peer as failed, so aggregate `round_ok`
                # is false. The certificate and local application must still
                # be valid; below, filesystem convergence proves every
                # non-absent replica applied, then the exact certified batch
                # and certificate catch up the absent replica.
                round_valid = (
                    certification.get("round_ok") is True
                    and round_value.get("all_vote_requests_verified") is True
                    and round_value.get("local_apply_verified") is True
                    and round_value.get("local_receipt_count", 0) >= 1
                    and round_value.get("local_rejected_count", 0) == 0
                    and round_value.get("local_accepted_count")
                    == round_value.get("local_receipt_count")
                )
        else:
            receipt = (
                hot_finality[0].get("receipt", {})
                if isinstance(hot_finality, list) and len(hot_finality) == 1
                else {}
            )
            sends = round_value.get("sends", [])
            round_valid = (
                certification.get("round_ok") is True
                and round_value.get("local_receipt_count") == 1
                and round_value.get("local_accepted_count") == 0
                and round_value.get("local_rejected_count") == 1
                and receipt.get("accepted") is False
                and receipt.get("code") == expected_rejection_code
                and receipt.get("fee_charged", 0) == 0
                and receipt.get("fee_burned", 0) == 0
                and len(sends) == VALIDATOR_COUNT - 1
                and all(
                    send.get("ack", {}).get("applied") is True
                    and send.get("ack", {}).get("rejected_count") == 1
                    for send in sends
                )
            )
        if (
            not round_valid
            or not isinstance(quorum, int)
            or quorum < expected_minimum
        ):
            raise HarnessError(f"peer-certified finality failed closed: {report}")

        pre_recovery = self._filesystem_statuses()
        if offline_index is not None:
            online = [
                row for index, row in enumerate(pre_recovery) if index != offline_index
            ]
            if len({(row["block_height"], row["state_root"]) for row in online}) != 1:
                raise HarnessError("five online validators did not converge")
            if pre_recovery[offline_index]["block_height"] >= height:
                raise HarnessError("offline validator unexpectedly applied the certified block")
            certified_batch = Path(
                report.get("batch_file") or round_value.get("batch_file", "")
            )
            certificate_file = Path(certification["certificate_file"])
            catchup_receipts = _run_json_value(
                [
                    str(self.binary),
                    "apply-batch",
                    "--data-dir",
                    str(self.node_dir(offline_index)),
                    "--batch-file",
                    str(certified_batch),
                    "--certificate-file",
                    str(certificate_file),
                ]
            )
            if (
                not isinstance(catchup_receipts, list)
                or not catchup_receipts
                or any(
                    not isinstance(receipt, dict)
                    or receipt.get("accepted") is not True
                    for receipt in catchup_receipts
                )
            ):
                raise HarnessError(
                    f"offline-validator certified catch-up failed: {catchup_receipts}"
                )

        post = self._filesystem_statuses()
        convergence_keys = {
            (row["block_height"], row["block_tip_hash"], row["state_root"])
            for row in post
        }
        if len(convergence_keys) != 1 or post[0]["block_height"] != height:
            raise HarnessError("six validators did not converge after certified finality")

        public_proof = self._write_public_finality_proof(
            height=height,
            effect_key=effect_key,
            source_index=source_index,
            round_value=round_value,
            certification=certification,
            post_statuses=post,
            expected_rejection_code=expected_rejection_code,
        )
        summary = {
            "schema": "postfiat.lightning.pftl_certified_round.v1",
            "effect_key": effect_key,
            "height": height,
            "source_validator": f"validator-{source_index}",
            "offline_validator": (
                None if offline_index is None else f"validator-{offline_index}"
            ),
            "vote_count": quorum,
            "certificate_id": certification.get("certificate_id"),
            "round_ok": True,
            "expected_rejection_code": expected_rejection_code,
            "pre_recovery_statuses": pre_recovery,
            "post_recovery_statuses": post,
            "public_finality_proof": public_proof,
            "full_report_private": str(artifact / "round.json"),
        }
        _atomic_json(artifact / "round.json", report)
        _atomic_json(
            self.root / "evidence" / f"{height}-{effect_key}.summary.json",
            summary,
            mode=0o644,
        )
        return {
            "report": report,
            "summary": summary,
            "public_finality_proof": public_proof,
        }

    def _sign_escrow_operation(
        self,
        *,
        signer_role: str,
        operation: dict[str, Any],
        private_dir: Path,
        sequence: int | None = None,
    ) -> dict[str, Any]:
        """Quote and sign without submitting to a mempool."""

        wallet = _wallet_from_manifest(self.manifest, signer_role)
        client = self.rpc_clients()[0]
        quote = client.escrow_fee_quote_response(
            wallet.address,
            operation,
            sequence=sequence,
            request_id=f"ln-demo-raw-quote-{int(time.time_ns())}",
        )
        private_dir.mkdir(parents=True, exist_ok=True)
        os.chmod(private_dir, 0o700)
        quote_file = private_dir / "quote.json"
        _atomic_json(quote_file, quote)
        return _run_json(
            [
                str(self.binary),
                "wallet-sign-escrow-transaction",
                "--key-file",
                str(wallet.key_file),
                "--quote-file",
                str(quote_file),
            ]
        )

    def _raw_escrow_batch(
        self,
        signed: dict[str, Any],
        *,
        output: Path,
    ) -> Path:
        """Build the canonical single-escrow batch reference.

        This mirrors ``postfiat_mempool_dag::
        reference_for_mixed_transactions_with_assets_and_escrows`` exactly and
        is checked again by every validator before voting.
        """

        signed = self._canonical_signed_escrow(signed)
        domain = (
            self.manifest["chain_id"],
            self.manifest["genesis_hash"],
            int(self.manifest["protocol_version"]),
        )
        payload_value = [
            domain[0],
            domain[1],
            domain[2],
            [],
            [],
            [],
            [signed],
        ]
        payload = json.dumps(
            payload_value, separators=(",", ":"), ensure_ascii=False
        ).encode()

        def hash_hex(label: str, value: bytes) -> str:
            digest = hashlib.sha3_384()
            digest.update(label.encode())
            digest.update(b"\x00")
            digest.update(value)
            return digest.hexdigest()

        payload_hash = hash_hex("postfiat.mempool.payload.v5", payload)
        reference = (
            f"chain_id={domain[0]}\n"
            f"genesis_hash={domain[1]}\n"
            f"protocol_version={domain[2]}\n"
            f"payload_hash={payload_hash}\n"
            "transaction_count=1\n"
            "legacy_transfer_count=0\n"
            "payment_v2_count=0\n"
            "asset_transaction_count=0\n"
            "escrow_transaction_count=1\n"
        ).encode()
        batch_id = hash_hex("postfiat.mempool.batch_reference.v4", reference)
        batch = {
            "batch_id": batch_id,
            "transactions": [],
            "asset_transactions": [],
            "escrow_transactions": [signed],
        }
        _atomic_json(output, batch)
        return output

    @staticmethod
    def _canonical_signed_escrow(signed: dict[str, Any]) -> dict[str, Any]:
        """Reproduce the Rust struct's serde field order for batch hashing."""

        unsigned = signed.get("unsigned")
        if not isinstance(unsigned, dict):
            raise HarnessError("signed escrow transaction is missing unsigned fields")
        operation = unsigned.get("operation")
        operation_fields = {
            "escrow_create": (
                "owner",
                "recipient",
                "asset_id",
                "amount",
                "condition",
                "finish_after",
                "cancel_after",
            ),
            "escrow_finish": ("escrow_id", "owner", "recipient", "fulfillment"),
            "escrow_cancel": ("escrow_id", "owner"),
        }.get(operation)
        if operation_fields is None:
            raise HarnessError(f"unknown escrow operation: {operation}")
        canonical_unsigned: dict[str, Any] = {}
        for field in (
            "chain_id",
            "genesis_hash",
            "protocol_version",
            "address_namespace",
            "transaction_kind",
            "signature_algorithm_id",
            "source",
            "fee",
            "sequence",
        ):
            if field not in unsigned:
                raise HarnessError(f"signed escrow is missing {field}")
            canonical_unsigned[field] = unsigned[field]
        canonical_unsigned["operation"] = operation
        for field in operation_fields:
            if field in unsigned:
                canonical_unsigned[field] = unsigned[field]
        canonical: dict[str, Any] = {"unsigned": canonical_unsigned}
        for field in ("algorithm_id", "public_key_hex", "signature_hex"):
            if field not in signed:
                raise HarnessError(f"signed escrow is missing {field}")
            canonical[field] = signed[field]
        return canonical

    def _filesystem_statuses(self) -> list[dict[str, Any]]:
        return [
            _run_json(
                [
                    str(self.binary),
                    "status",
                    "--data-dir",
                    str(self.node_dir(index)),
                ]
            )
            for index in range(VALIDATOR_COUNT)
        ]

    def _validate_effect_key(self, effect_key: str) -> str:
        if not isinstance(effect_key, str) or not _EFFECT_KEY.fullmatch(effect_key):
            raise HarnessError("effect_key must match [A-Za-z0-9._:-]{1,128}")
        return effect_key

    def _effects_path(self) -> Path:
        return self.root / "runtime" / "effects.json"

    def _effect_get(self, effect_key: str) -> dict[str, Any] | None:
        path = self._effects_path()
        if not path.exists():
            return None
        value = _read_json(path)
        if value.get("schema") != EFFECT_SCHEMA:
            raise HarnessError("invalid PFTL effect journal schema")
        effect = value.get("effects", {}).get(effect_key)
        return effect if isinstance(effect, dict) else None

    def _effect_put(self, effect_key: str, effect: dict[str, Any]) -> None:
        path = self._effects_path()
        value = (
            _read_json(path)
            if path.exists()
            else {"schema": EFFECT_SCHEMA, "effects": {}}
        )
        existing = value["effects"].get(effect_key)
        if existing is not None and existing != effect:
            raise HarnessError(f"effect_key collision with different result: {effect_key}")
        value["effects"][effect_key] = effect
        _atomic_json(path, value)

    def public_finality_proof(self, effect_key: str) -> dict[str, Any]:
        """Load and hash-check the secret-free certificate bundle for an effect."""

        key = self._validate_effect_key(effect_key)
        effect = self._effect_get(key)
        if effect is None:
            raise HarnessError(f"unknown finalized effect: {key}")
        raw_path = effect.get("finality_proof_path")
        expected_hash = effect.get("finality_proof_sha256")
        if not isinstance(raw_path, str) or not isinstance(expected_hash, str):
            raise HarnessError(f"effect has no public finality proof: {key}")
        path = Path(raw_path).resolve()
        public_root = (self.root / "evidence" / "finality").resolve()
        if public_root not in path.parents or not path.is_file():
            raise HarnessError("public finality proof path escapes the evidence root")
        if _sha256_file(path) != expected_hash:
            raise HarnessError("public finality proof hash mismatch")
        proof = _read_json(path)
        certificate = proof.get("certificate") if isinstance(proof, dict) else None
        if (
            not isinstance(proof, dict)
            or not isinstance(certificate, dict)
            or proof.get("schema") != effect.get("finality_proof_schema")
            or proof.get("effect_key") != key
            or certificate.get("certificate_id") != effect.get("certificate_id")
        ):
            raise HarnessError("public finality proof is not bound to its effect")
        _assert_secret_free_public(proof)
        return proof

    def _submit_wallet_operation(
        self,
        *,
        effect_key: str,
        signed_kind: str,
        build: Callable[[PostFiatRpcClient], Any],
        escrow_id: str | None = None,
        offline_index: int | None = None,
    ) -> dict[str, Any]:
        key = self._validate_effect_key(effect_key)
        if signed_kind not in {"asset", "escrow"}:
            raise HarnessError(f"unsupported signed operation kind: {signed_kind}")
        prior = self._effect_get(key)
        if prior is not None:
            return prior
        height, proposer, _ = self._next_proposer()
        with wallet_binary(self.binary):
            result = build(self.rpc_clients()[proposer])
        finality = self._certify(
            height=height,
            source_index=proposer,
            effect_key=key,
            offline_index=offline_index,
        )
        tx_id = result.tx_id or finality["report"].get("submitted_tx_id")
        if not isinstance(tx_id, str) or not tx_id:
            raise HarnessError("accepted operation did not produce a tx id")
        observed = self.observe_tx(tx_id)
        certification = (
            finality["report"]["round"]["certification"]
            if "round" in finality["report"]
            else finality["report"]["certification"]
        )
        effect = FinalizedEffect(
            accepted=observed["accepted"],
            reason=observed["reason"],
            tx_id=tx_id,
            finalized_height=observed["finalized_height"],
            state_root=observed["state_root"],
            block_tip_hash=observed["block_tip_hash"],
            agreeing_validator_count=observed["agreeing_validator_count"],
            validator_count=VALIDATOR_COUNT,
            receipt_count=observed["receipt_count"],
            certificate_id=str(certification["certificate_id"]),
            effect_key=key,
            escrow_id=escrow_id or getattr(result, "escrow_id", None),
        ).to_dict()
        public_proof = finality["public_finality_proof"]
        effect.update(
            {
                "finality_proof_schema": public_proof["schema"],
                "finality_proof_path": public_proof["path"],
                "finality_proof_sha256": public_proof["sha256"],
            }
        )
        if effect["accepted"] is not True:
            raise HarnessError(f"certified operation receipt rejected: {effect}")
        self._effect_put(key, effect)
        return effect

    def _bootstrap_test_asset(self) -> dict[str, Any]:
        roles = {
            role: _wallet_from_manifest(self.manifest, role)
            for role in ("issuer", "coordinator", "user")
        }
        for role, wallet in roles.items():
            self._faucet(
                wallet.address,
                PFT_FUNDING,
                effect_key=f"bootstrap-fund-{role}",
            )

        create = self._submit_wallet_operation(
            effect_key="bootstrap-asset-create",
            signed_kind="asset",
            build=lambda client: wallet_api.create_issued_asset(
                client,
                wallet=roles["issuer"],
                code=TEST_ASSET_CODE,
                precision=TEST_ASSET_PRECISION,
                display_name="Synthetic Lightning NAVcoin",
                max_supply=TEST_ASSET_MAX_SUPPLY,
                requires_authorization=False,
                freeze_enabled=False,
                clawback_enabled=False,
                work_dir=self.root / "private" / "wallet-work",
            ),
        )
        # FinalizedEffect intentionally has no operation details; derive the
        # deterministic asset id from the helper's public algorithm.
        asset_id = wallet_api._issued_asset_id(
            self.manifest["chain_id"],
            roles["issuer"].address,
            TEST_ASSET_CODE,
            1,
        )
        if not re.fullmatch(r"[0-9a-f]{96}", asset_id):
            raise HarnessError("derived synthetic asset id is not canonical 96-hex")
        for role in ("coordinator", "user"):
            self._submit_wallet_operation(
                effect_key=f"bootstrap-trustline-{role}",
                signed_kind="asset",
                build=lambda client, role=role: wallet_api.create_asset_trustline(
                    client,
                    wallet=roles[role],
                    issuer=roles["issuer"].address,
                    asset_id=asset_id,
                    limit=TRUSTLINE_LIMIT,
                    work_dir=self.root / "private" / "wallet-work",
                ),
            )
        for role, amount in (
            ("coordinator", COORDINATOR_INVENTORY),
            ("user", USER_INVENTORY),
        ):
            self._submit_wallet_operation(
                effect_key=f"bootstrap-issue-{role}",
                signed_kind="asset",
                build=lambda client, role=role, amount=amount: wallet_api.send_issued_asset(
                    client,
                    wallet=roles["issuer"],
                    to_address=roles[role].address,
                    issuer=roles["issuer"].address,
                    asset_id=asset_id,
                    amount=amount,
                    work_dir=self.root / "private" / "wallet-work",
                ),
            )

        info = self.rpc_clients()[0].asset_info(asset_id)
        asset = info.get("asset")
        if not isinstance(asset, dict):
            raise HarnessError("bootstrapped asset is not queryable")
        if any(
            (
                asset.get("requires_authorization") is not False,
                asset.get("freeze_enabled") is not False,
                asset.get("clawback_enabled") is not False,
                asset.get("code") != TEST_ASSET_CODE,
                asset.get("precision") != TEST_ASSET_PRECISION,
                asset.get("outstanding_supply")
                != COORDINATOR_INVENTORY + USER_INVENTORY,
            )
        ):
            raise HarnessError(f"test asset controls/supply are unsafe: {asset}")
        return {
            "asset_id": asset_id,
            "issuer": roles["issuer"].address,
            "code": TEST_ASSET_CODE,
            "precision": TEST_ASSET_PRECISION,
            "requires_authorization": False,
            "freeze_enabled": False,
            "clawback_enabled": False,
            "max_supply": TEST_ASSET_MAX_SUPPLY,
            "initial_supply": COORDINATOR_INVENTORY + USER_INVENTORY,
        }

    def _faucet(self, address: str, amount: int, *, effect_key: str) -> dict[str, Any]:
        key = self._validate_effect_key(effect_key)
        prior = self._effect_get(key)
        if prior is not None:
            return prior
        height, proposer, _ = self._next_proposer()
        batch_file = (
            self.root / "private" / "faucet-batches" / f"{height}-{effect_key}.json"
        )
        batch_file.parent.mkdir(parents=True, exist_ok=True)
        _run_json(
            [
                str(self.binary),
                "batch-transfer",
                "--data-dir",
                str(self.node_dir(proposer)),
                "--to",
                address,
                "--amount",
                str(amount),
                "--batch-file",
                str(batch_file),
            ]
        )
        finality = self._certify(
            height=height,
            source_index=proposer,
            effect_key=key,
            batch_file=batch_file,
        )
        round_value = finality["report"]
        receipts = round_value.get("local_hot_finality", [])
        if not receipts:
            receipts = round_value.get("local_receipts", [])
        status = self.statuses()[0]
        effect = {
            "accepted": True,
            "reason": "accepted",
            "tx_id": "",
            "finalized_height": status["block_height"],
            "state_root": status["state_root"],
            "block_tip_hash": status["block_tip_hash"],
            "agreeing_validator_count": VALIDATOR_COUNT,
            "validator_count": VALIDATOR_COUNT,
            "receipt_count": max(1, len(receipts)),
            "certificate_id": finality["report"]["certification"]["certificate_id"],
            "effect_key": key,
            "escrow_id": None,
        }
        public_proof = finality["public_finality_proof"]
        effect.update(
            {
                "finality_proof_schema": public_proof["schema"],
                "finality_proof_path": public_proof["path"],
                "finality_proof_sha256": public_proof["sha256"],
            }
        )
        self._effect_put(key, effect)
        return effect

    def submit_create(
        self,
        *,
        owner_role: str,
        recipient_role: str,
        amount: int,
        condition: str,
        cancel_after: int,
        effect_key: str,
        expected_escrow_id: str,
        offline_index: int | None = None,
    ) -> dict[str, Any]:
        """Create a typed issued-asset escrow and return secret-free evidence."""

        decode_condition(condition)
        if not isinstance(expected_escrow_id, str) or len(expected_escrow_id) != 96:
            raise HarnessError("submit_create requires the planned 96-hex escrow id")
        if cancel_after <= self.statuses()[0]["block_height"] + 1:
            raise HarnessError("cancel_after leaves no finalized claim window")
        owner = _wallet_from_manifest(self.manifest, owner_role)
        recipient = _wallet_from_manifest(self.manifest, recipient_role)
        result = self._submit_wallet_operation(
            effect_key=effect_key,
            signed_kind="escrow",
            build=lambda client: wallet_api.create_escrow(
                client,
                owner_wallet=owner,
                destination=recipient.address,
                amount=amount,
                asset_id=self.manifest["asset"]["asset_id"],
                condition=condition,
                finish_after=0,
                cancel_after=cancel_after,
                work_dir=self.root / "private" / "wallet-work",
            ),
            offline_index=offline_index,
        )
        if result.get("escrow_id") != expected_escrow_id:
            raise HarnessError(
                "finalized escrow id does not match the signed pre-lock quote plan"
            )
        return result

    def plan_create(
        self,
        *,
        owner_role: str,
        recipient_role: str,
        amount: int,
        condition: str,
        cancel_after: int,
    ) -> dict[str, Any]:
        """Resolve the finalized owner sequence and deterministic escrow id."""

        decode_condition(condition)
        if amount <= 0:
            raise HarnessError("escrow amount must be positive")
        snapshot = self.consensus_snapshot(
            asset_id=self.manifest["asset"]["asset_id"],
            accounts=[
                self.manifest["roles"]["coordinator"]["address"],
                self.manifest["roles"]["user"]["address"],
            ],
        )
        if cancel_after <= snapshot["finalized_height"] + 1:
            raise HarnessError("cancel_after leaves no finalized claim window")
        owner = _wallet_from_manifest(self.manifest, owner_role)
        recipient = _wallet_from_manifest(self.manifest, recipient_role)
        operation = {
            "operation": "escrow_create",
            "owner": owner.address,
            "recipient": recipient.address,
            "asset_id": self.manifest["asset"]["asset_id"],
            "amount": amount,
            "condition": condition,
            "finish_after": 0,
            "cancel_after": cancel_after,
        }
        quote = self.rpc_clients()[0].escrow_fee_quote_response(
            owner.address,
            operation,
            request_id=f"ln-demo-create-plan-{time.time_ns()}",
        )
        quote_result = quote.get("result", {})
        sequence = quote_result.get("sequence")
        if not isinstance(sequence, int) or sequence <= 0:
            raise HarnessError("escrow quote did not return a positive owner sequence")
        escrow_id = wallet_api._escrow_id(
            self.manifest["chain_id"], owner.address, sequence
        )
        return {
            "schema": "postfiat.lightning.pftl_create_plan.v1",
            "chain_id": self.manifest["chain_id"],
            "genesis_hash": self.manifest["genesis_hash"],
            "protocol_version": self.manifest["protocol_version"],
            "asset_id": self.manifest["asset"]["asset_id"],
            "owner": owner.address,
            "recipient": recipient.address,
            "owner_sequence": sequence,
            "expected_escrow_id": escrow_id,
            "amount": amount,
            "condition": condition,
            "finish_after": 0,
            "cancel_after": cancel_after,
            "finalized_height": snapshot["finalized_height"],
            "state_root": snapshot["state_root"],
            "agreeing_validator_count": snapshot["agreeing_validator_count"],
        }

    def submit_finish(
        self,
        *,
        owner_role: str,
        recipient_role: str,
        escrow_id: str,
        fulfillment: str,
        expected_condition: str,
        effect_key: str,
        offline_index: int | None = None,
    ) -> dict[str, Any]:
        """Finish an escrow without returning the preimage in public evidence."""

        decode_fulfillment(fulfillment)
        if not fulfillment_satisfies(expected_condition, fulfillment):
            raise HarnessError("fulfillment does not match the expected payment hash")
        owner = _wallet_from_manifest(self.manifest, owner_role)
        recipient = _wallet_from_manifest(self.manifest, recipient_role)
        return self._submit_wallet_operation(
            effect_key=effect_key,
            signed_kind="escrow",
            build=lambda client: wallet_api.finish_escrow(
                client,
                recipient_wallet=recipient,
                escrow_id=escrow_id,
                owner=owner.address,
                fulfillment=fulfillment,
                work_dir=self.root / "private" / "wallet-work",
            ),
            escrow_id=escrow_id,
            offline_index=offline_index,
        )

    def submit_cancel(
        self,
        *,
        owner_role: str,
        escrow_id: str,
        effect_key: str,
        offline_index: int | None = None,
    ) -> dict[str, Any]:
        owner = _wallet_from_manifest(self.manifest, owner_role)
        return self._submit_wallet_operation(
            effect_key=effect_key,
            signed_kind="escrow",
            build=lambda client: wallet_api.cancel_escrow(
                client,
                owner_wallet=owner,
                escrow_id=escrow_id,
                work_dir=self.root / "private" / "wallet-work",
            ),
            escrow_id=escrow_id,
            offline_index=offline_index,
        )

    def submit_expected_rejection(
        self,
        *,
        signer_role: str,
        operation: dict[str, Any] | None,
        expected_code: str,
        effect_key: str,
        escrow_id: str | None = None,
        signed_escrow_transaction: dict[str, Any] | None = None,
        result_metadata: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Certify a rejected escrow tx and prove its value effects are empty.

        Supplying ``signed_escrow_transaction`` is reserved for literal replay
        tests. Otherwise the harness quotes/signs ``operation`` without using
        mempool admission, builds the canonical batch, and makes all six
        validators execute it.
        """

        key = self._validate_effect_key(effect_key)
        prior = self._effect_get(key)
        if prior is not None:
            return prior
        if not isinstance(expected_code, str) or not expected_code:
            raise HarnessError("expected_code is required")
        if (operation is None) == (signed_escrow_transaction is None):
            raise HarnessError(
                "supply exactly one operation or signed_escrow_transaction"
            )
        before_status = self.statuses()[0]
        before = self._value_snapshot(escrow_id=escrow_id)
        height, proposer, _ = self._next_proposer()
        private_dir = (
            self.root / "private" / "rejections" / f"{height}-{effect_key}"
        )
        if signed_escrow_transaction is None:
            signed = self._sign_escrow_operation(
                signer_role=signer_role,
                operation=operation or {},
                private_dir=private_dir,
            )
        else:
            signed = signed_escrow_transaction
            private_dir.mkdir(parents=True, exist_ok=True)
            os.chmod(private_dir, 0o700)
        batch_file = self._raw_escrow_batch(
            signed, output=private_dir / "rejected-batch.json"
        )
        finality = self._certify(
            height=height,
            source_index=proposer,
            effect_key=key,
            batch_file=batch_file,
            expected_rejection_code=expected_code,
        )
        round_value = finality["report"]
        hot = round_value["local_hot_finality"]
        if len(hot) != 1:
            raise HarnessError("expected one rejected hot-finality receipt")
        receipt = hot[0]["receipt"]
        tx_id = receipt["tx_id"]
        observed = self.observe_tx(tx_id)
        after_status = self.statuses()[0]
        after = self._value_snapshot(escrow_id=escrow_id)
        mutation_free = before == after
        if not mutation_free:
            raise HarnessError(
                f"rejected operation mutated balances/supply/escrow: {expected_code}"
            )
        if (
            int(after_status["block_height"])
            != int(before_status["block_height"]) + 1
            or after_status["block_tip_hash"] == before_status["block_tip_hash"]
        ):
            raise HarnessError(
                "rejected receipt did not produce exactly one consensus-progress block"
            )
        if (
            observed["accepted"] is not False
            or observed["reason"] != expected_code
            or receipt.get("fee_charged", 0) != 0
            or receipt.get("fee_burned", 0) != 0
        ):
            raise HarnessError("rejected receipt does not satisfy the mutation-free gate")
        effect = {
            **observed,
            "certificate_id": round_value["certification"]["certificate_id"],
            "effect_key": key,
            "escrow_id": escrow_id,
            "mutation_free": True,
            "mutation_free_projection": (
                "asset; both role account records (native balance/sequence); "
                "trustlines; owner/recipient escrow indexes; target escrow"
            ),
            "value_projection_sha256": hashlib.sha256(
                json.dumps(
                    before, sort_keys=True, separators=(",", ":")
                ).encode()
            ).hexdigest(),
            "pre_finalized_height": before_status["block_height"],
            "pre_state_root": before_status["state_root"],
            "post_finalized_height": after_status["block_height"],
            "post_block_tip_hash": after_status["block_tip_hash"],
            "post_state_root": after_status["state_root"],
            "pre_block_tip_hash": before_status["block_tip_hash"],
            "consensus_progress_only": True,
            "fee_charged": 0,
            "fee_burned": 0,
        }
        public_proof = finality["public_finality_proof"]
        effect.update(
            {
                "finality_proof_schema": public_proof["schema"],
                "finality_proof_path": public_proof["path"],
                "finality_proof_sha256": public_proof["sha256"],
            }
        )
        if result_metadata:
            overlap = set(effect).intersection(result_metadata)
            if overlap:
                raise HarnessError(
                    "rejection metadata cannot replace consensus evidence fields: "
                    + ", ".join(sorted(overlap))
                )
            effect.update(result_metadata)
        self._effect_put(key, effect)
        return effect

    def submit_duplicate(
        self,
        *,
        original_tx_id: str,
        expected_code: str,
        effect_key: str,
        escrow_id: str | None = None,
    ) -> dict[str, Any]:
        """Submit a fresh signed envelope for the finalized operation.

        A byte-identical replay reconstructs the already-applied batch id and
        is rejected before proposal, so it cannot yield a consensus receipt.
        The acceptance suite instead preserves the original operation and stale
        sequence, increments only the fee, and signs a fresh transaction. That
        produces a new batch which all six validators execute and reject with
        the requested semantic replay code (normally ``bad_sequence``).
        """

        key = self._validate_effect_key(effect_key)
        prior = self._effect_get(key)
        if prior is not None:
            return prior
        if not isinstance(expected_code, str) or not expected_code:
            raise HarnessError("expected_code is required")
        original = self._signed_escrow_transaction(original_tx_id)
        unsigned = original.get("unsigned")
        if not isinstance(unsigned, dict):
            raise HarnessError("finalized escrow transaction has no unsigned payload")
        operation_name = unsigned.get("operation")
        fields = {
            "escrow_create": (
                "owner",
                "recipient",
                "asset_id",
                "amount",
                "condition",
                "finish_after",
                "cancel_after",
            ),
            "escrow_finish": ("escrow_id", "owner", "recipient", "fulfillment"),
            "escrow_cancel": ("escrow_id", "owner"),
        }.get(operation_name)
        if fields is None:
            raise HarnessError(f"unsupported replay operation: {operation_name}")
        operation = {"operation": operation_name}
        for field in fields:
            if field not in unsigned:
                defaults = {
                    "condition": "",
                    "finish_after": 0,
                    "cancel_after": 0,
                }
                if field not in defaults:
                    raise HarnessError(
                        f"finalized replay payload is missing {field}"
                    )
                operation[field] = defaults[field]
            else:
                operation[field] = unsigned[field]

        source = unsigned.get("source")
        signer_role = next(
            (
                role
                for role in ("coordinator", "user", "issuer")
                if self.manifest["roles"][role]["address"] == source
            ),
            None,
        )
        if signer_role is None:
            raise HarnessError("finalized replay source is not a harness wallet")
        wallet = _wallet_from_manifest(self.manifest, signer_role)
        original_fee = unsigned.get("fee")
        original_sequence = unsigned.get("sequence")
        if (
            type(original_fee) is not int
            or original_fee < 0
            or type(original_sequence) is not int
            or original_sequence <= 0
        ):
            raise HarnessError("finalized replay fee/sequence is invalid")
        replay_fee = original_fee + 1
        private_dir = self.root / "private" / "semantic-replays" / key
        private_dir.mkdir(parents=True, exist_ok=False)
        os.chmod(private_dir, 0o700)
        fresh = _run_json(
            [
                str(self.binary),
                "wallet-sign-escrow-transaction",
                "--key-file",
                str(wallet.key_file),
                "--chain-id",
                self.manifest["chain_id"],
                "--genesis-hash",
                self.manifest["genesis_hash"],
                "--protocol-version",
                str(self.manifest["protocol_version"]),
                "--fee",
                str(replay_fee),
                "--sequence",
                str(original_sequence),
                "--operation-json",
                json.dumps(operation, separators=(",", ":")),
            ]
        )
        _atomic_json(private_dir / "fresh-signed-replay.json", fresh)
        if fresh == original:
            raise HarnessError("fresh semantic replay did not change its envelope")
        return self.submit_expected_rejection(
            signer_role=signer_role,
            operation=None,
            expected_code=expected_code,
            effect_key=key,
            escrow_id=escrow_id,
            signed_escrow_transaction=fresh,
            result_metadata={
                "replay_class": "fresh_envelope_same_operation_stale_sequence",
                "original_tx_id": original_tx_id,
                "original_sequence": original_sequence,
                "replay_fee_delta": 1,
                "consensus_rejected_receipt": True,
            },
        )

    def _signed_escrow_transaction(self, tx_id: str) -> dict[str, Any]:
        client = self.rpc_clients()[0]
        tx = client.tx(tx_id, audit_block_log=True)
        header = tx.get("block", {}).get("header", {})
        rows = client.batch_archive(
            batch_kind=header.get("batch_kind"),
            batch_id=header.get("batch_id"),
            limit=1,
        )
        if len(rows) != 1:
            raise HarnessError("finalized transaction batch is unavailable")
        payload = rows[0].get("payload_json")
        batch = json.loads(payload) if isinstance(payload, str) else payload
        escrows = batch.get("escrow_transactions", []) if isinstance(batch, dict) else []
        if len(escrows) != 1:
            raise HarnessError("harness transaction batch is not single-escrow")
        return escrows[0]

    def _value_snapshot(self, *, escrow_id: str | None) -> dict[str, Any]:
        """Snapshot application state a rejected escrow operation must preserve.

        A certified rejected transaction still commits a receipt-bearing block,
        so height, tip, and the aggregate state commitment can advance. This
        projection intentionally excludes those consensus-progress fields and
        includes every value/authorization/sequence surface relevant to the
        two test-asset principals.
        """

        client = self.rpc_clients()[0]
        asset_id = self.manifest["asset"]["asset_id"]
        roles = ("coordinator", "user")
        accounts = {
            role: client.account(self.manifest["roles"][role]["address"])
            for role in roles
        }
        lines = {
            role: client.account_lines(
                self.manifest["roles"][role]["address"],
                asset_id=asset_id,
                limit=4,
            )
            for role in roles
        }
        escrows = {
            role: {
                view_role: client.account_escrows(
                    self.manifest["roles"][role]["address"],
                    role=view_role,
                    limit=128,
                )
                for view_role in ("owner", "recipient")
            }
            for role in roles
        }
        return {
            "asset": client.asset_info(asset_id),
            "accounts": accounts,
            "lines": lines,
            "escrows": escrows,
            "target_escrow": client.escrow_info(escrow_id) if escrow_id else None,
        }

    def restart_rpc_proof(self, *, effect_key: str) -> dict[str, Any]:
        """Hard-stop all RPC readers, restart, and prove durable state equality."""

        key = self._validate_effect_key(effect_key)
        before = self.status_report()["snapshot"]
        runtime = _read_json(self._runtime_path())
        killed: list[str] = []
        for row in runtime.get("processes", []):
            if self._pid_matches(row):
                os.kill(int(row["pid"]), signal.SIGKILL)
                killed.append(str(row["node_id"]))
        deadline = time.monotonic() + 10
        while any(self._pid_matches(row) for row in runtime.get("processes", [])):
            if time.monotonic() >= deadline:
                raise HarnessError("RPC crash processes did not exit")
            time.sleep(0.05)
        self._runtime_path().unlink(missing_ok=True)
        self.start_rpc()
        after = self.status_report()["snapshot"]
        stable_fields = (
            "finalized_height",
            "block_tip_hash",
            "state_root",
            "outstanding_supply",
            "spendable_supply",
            "open_escrow_total",
            "supply_conservation_verified",
        )
        if any(before[field] != after[field] for field in stable_fields):
            raise HarnessError("RPC crash/restart changed durable PFTL state")
        report = {
            "schema": "postfiat.lightning.pftl_rpc_restart_proof.v1",
            "effect_key": key,
            "killed": killed,
            "stable_fields": list(stable_fields),
            "before": {field: before[field] for field in stable_fields},
            "after": {field: after[field] for field in stable_fields},
            "agreeing_validator_count": after["agreeing_validator_count"],
            "verified": True,
        }
        _atomic_json(
            self.root / "evidence" / f"{key}.rpc-restart.json",
            report,
            mode=0o644,
        )
        return report

    def advance_height(
        self,
        *,
        effect_key: str,
        one_validator_down: bool = False,
    ) -> dict[str, Any]:
        """Advance one certified height with an inert synthetic faucet transfer."""

        height, proposer, _ = self._next_proposer()
        offline = None
        if one_validator_down:
            offline = next(
                index for index in range(VALIDATOR_COUNT) if index != proposer
            )
        key = self._validate_effect_key(effect_key)
        batch_file = self.root / "private" / "height-batches" / f"{height}-{key}.json"
        batch_file.parent.mkdir(parents=True, exist_ok=True)
        user = _wallet_from_manifest(self.manifest, "user")
        _run_json(
            [
                str(self.binary),
                "batch-transfer",
                "--data-dir",
                str(self.node_dir(proposer)),
                "--to",
                user.address,
                "--amount",
                "1",
                "--batch-file",
                str(batch_file),
            ]
        )
        finality = self._certify(
            height=height,
            source_index=proposer,
            effect_key=key,
            batch_file=batch_file,
            offline_index=offline,
        )
        return finality["summary"]

    def observe_tx(self, tx_id: str) -> dict[str, Any]:
        """Read one finalized receipt independently from all six RPCs."""

        if not isinstance(tx_id, str) or not tx_id:
            raise HarnessError("tx_id is required")
        clients = self.rpc_clients()
        statuses = [client.status() for client in clients]
        receipts = [client.receipts(tx_id=tx_id, limit=2) for client in clients]
        normalized: list[tuple[bool, str]] = []
        for rows in receipts:
            if len(rows) != 1:
                raise HarnessError(f"expected one finalized receipt per validator: {rows}")
            normalized.append((bool(rows[0].get("accepted")), str(rows[0].get("code"))))
        agreement_key = (
            statuses[0]["block_height"],
            statuses[0]["block_tip_hash"],
            statuses[0]["state_root"],
            normalized[0],
        )
        agreeing = sum(
            (
                status["block_height"],
                status["block_tip_hash"],
                status["state_root"],
                receipt,
            )
            == agreement_key
            for status, receipt in zip(statuses, normalized, strict=True)
        )
        if agreeing != VALIDATOR_COUNT:
            raise HarnessError("validator receipt/root observations do not agree 6/6")
        transaction = clients[0].tx(tx_id, audit_block_log=True)
        block = transaction.get("block", {}).get("header", {})
        return {
            "accepted": normalized[0][0],
            "reason": normalized[0][1],
            "tx_id": tx_id,
            "finalized_height": int(block.get("height", statuses[0]["block_height"])),
            "state_root": statuses[0]["state_root"],
            "block_tip_hash": statuses[0]["block_tip_hash"],
            "agreeing_validator_count": agreeing,
            "validator_count": VALIDATOR_COUNT,
            "receipt_count": len(receipts),
        }

    def consensus_snapshot(
        self,
        *,
        asset_id: str | None = None,
        accounts: Sequence[str] = (),
        escrow_id: str | None = None,
        tx_id: str | None = None,
    ) -> dict[str, Any]:
        """Return an independently read, convergence-checked public snapshot."""

        clients = self.rpc_clients()
        rows: list[dict[str, Any]] = []
        for index, client in enumerate(clients):
            status = client.status()
            escrow_report = (
                client.escrow_info(escrow_id) if escrow_id else None
            )
            finish_fee_quote = None
            if (
                isinstance(escrow_report, dict)
                and isinstance(escrow_report.get("escrow"), dict)
            ):
                escrow = escrow_report["escrow"]
                finish_fee_quote = client.escrow_fee_quote(
                    str(escrow["recipient"]),
                    {
                        "operation": "escrow_finish",
                        "owner": escrow["owner"],
                        "recipient": escrow["recipient"],
                        "escrow_id": escrow["escrow_id"],
                        # Fee sizing depends on the canonical fixed-width
                        # fulfillment, not its secret value. This placeholder
                        # is never signed or submitted.
                        "fulfillment": "a0228020" + ("00" * 32),
                    },
                )
            row: dict[str, Any] = {
                "node_id": f"validator-{index}",
                "status": status,
                "asset": client.asset_info(asset_id) if asset_id else None,
                "accounts": {
                    account: client.account_lines(
                        account, asset_id=asset_id, limit=8
                    )
                    if asset_id
                    else client.account(account)
                    for account in accounts
                },
                "native_accounts": {
                    account: client.account(account) for account in accounts
                },
                "escrow": escrow_report,
                "finish_fee_quote": finish_fee_quote,
                "receipts": client.receipts(tx_id=tx_id, limit=2) if tx_id else None,
            }
            rows.append(row)

        def consensus_view(row: dict[str, Any]) -> str:
            public = {
                "height": row["status"]["block_height"],
                "tip": row["status"]["block_tip_hash"],
                "root": row["status"]["state_root"],
                "asset": row["asset"],
                "accounts": row["accounts"],
                "native_accounts": row["native_accounts"],
                "escrow": row["escrow"],
                "finish_fee_quote": row["finish_fee_quote"],
                "receipts": row["receipts"],
            }
            return json.dumps(public, sort_keys=True, separators=(",", ":"))

        views = [consensus_view(row) for row in rows]
        agreeing = sum(view == views[0] for view in views)
        if agreeing != VALIDATOR_COUNT:
            raise HarnessError("independent PFTL RPC snapshot did not converge 6/6")

        open_escrow_total = None
        spendable_supply = None
        if asset_id:
            owner_addresses = [
                self.manifest["roles"][role]["address"]
                for role in ("coordinator", "user")
            ]
            open_rows = [
                clients[0].account_escrows(
                    address, role="owner", state="open", limit=128
                )
                for address in owner_addresses
            ]
            open_escrow_total = sum(
                int(escrow["amount"])
                for report in open_rows
                for escrow in report.get("escrows", [])
                if escrow.get("asset_id") == asset_id
            )
            trustline_count = rows[0]["asset"]["asset"]["trustline_count"]
            if trustline_count != len(owner_addresses):
                raise HarnessError(
                    "open escrow derivation no longer covers every test-asset trustline"
                )
            spendable_supply = sum(
                int(line["balance"])
                for account_report in rows[0]["accounts"].values()
                for line in account_report.get("lines", [])
                if line.get("asset_id") == asset_id
            )
            outstanding = int(rows[0]["asset"]["asset"]["outstanding_supply"])
            if spendable_supply + open_escrow_total != outstanding:
                raise HarnessError(
                    "synthetic asset conservation failed: "
                    "spendable plus open escrow does not equal outstanding supply"
                )

        return {
            "schema": "postfiat.lightning.pftl_consensus_snapshot.v1",
            "chain_id": self.manifest["chain_id"],
            "genesis_hash": self.manifest["genesis_hash"],
            "finalized_height": rows[0]["status"]["block_height"],
            "block_tip_hash": rows[0]["status"]["block_tip_hash"],
            "state_root": rows[0]["status"]["state_root"],
            "agreeing_validator_count": agreeing,
            "validator_count": VALIDATOR_COUNT,
            "asset_id": asset_id,
            "outstanding_supply": (
                rows[0]["asset"]["asset"]["outstanding_supply"] if asset_id else None
            ),
            "spendable_supply": spendable_supply,
            "open_escrow_total": open_escrow_total,
            "supply_conservation_verified": (
                spendable_supply + open_escrow_total
                == rows[0]["asset"]["asset"]["outstanding_supply"]
                if asset_id
                else None
            ),
            "rows": rows,
        }

    def status_report(self) -> dict[str, Any]:
        snapshot = self.consensus_snapshot(
            asset_id=(
                self.manifest["asset"]["asset_id"]
                if isinstance(self.manifest.get("asset"), dict)
                else None
            ),
            accounts=[
                self.manifest["roles"]["coordinator"]["address"],
                self.manifest["roles"]["user"]["address"],
            ],
        )
        return {
            "schema": "postfiat.lightning.pftl_devnet_status.v1",
            "binary": self.manifest["binary"],
            "snapshot": snapshot,
            "rpc_runtime": (
                _read_json(self._runtime_path())
                if self._runtime_path().exists()
                else None
            ),
        }
