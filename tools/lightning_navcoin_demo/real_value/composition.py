"""Fail-closed process composition for the real-value coordinator.

This module owns local configuration and secret loading.  It does not create
an LND wallet, acquire liquidity, sign an operator authorization, or submit a
PFTL transaction merely by being imported.  ``DRY_RUN`` rejects every signer
input.  ``ARMED`` requires an exact process acknowledgement and an opaque PFTL
signer handle before the signer-capable backend can be constructed.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import importlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
from typing import Any, Callable, Mapping

from ..coordinator.journal import CoordinatorJournal, ExposureLimits
from ..coordinator.signing import Ed25519Signer
from .budget import RealValueBudget
from .lnd_connection import (
    ConnectedMainnetLnd,
    MainnetLndConnection,
    connect_mainnet_lnd,
)
from .pftl_effect_store import PftlEffectStore
from .pftl_handoff import (
    DEFAULT_HANDOFF_PATH,
    PersistentPftlHandoff,
    load_persistent_handoff,
)
from .pftl_quorum import PftlQuorumObserver
from .pftl_valuation_binding import (
    PftlValuationBinding,
    PftlValuationBindingError,
)
from .pftl_signer_backend import (
    EXECUTION_ACK,
    PersistentHandoffPftlBackend,
    SignerHandle,
)
from .policy import (
    ExecutionMode,
    PriceObservation,
    RealValuePolicy,
    RealValuePolicyError,
)
from .runtime import MainnetCoordinatorRuntime


ARMED_PROCESS_ACK = "I_ACKNOWLEDGE_REAL_BTC_AND_PFTL_VALUE"
MAX_CONFIG_BYTES = 128 * 1024
QUOTE_SEED_BYTES = 32
SESSION_TOKEN_BYTES = 32
DEFAULT_STATE_ROOT = Path("/home/postfiat/.pft/lightning-navcoin-mainnet")
DEFAULT_LND_PROTO_DIR = DEFAULT_STATE_ROOT / "lnd-grpc-v0.20.1"
RECEIVE_ONLY_MACAROON_RELATIVE_PATH = Path(
    "lnd/data/chain/bitcoin/mainnet/"
    "lightning-navcoin-receive-only.macaroon"
)
SOURCE_RELEASE_SCHEMA = "postfiat.lightning_coordinator_source_release.v1"
SOURCE_RELEASE_TARGETS = (
    "tools/lightning_navcoin_demo",
    "scripts/lightning-navcoin-mainnet-coordinator",
    "scripts/lightning-navcoin-mainnet-env",
    "python/postfiat_rpc",
    "wallet-web",
)
_BROAD_STATE_ROOTS = frozenset(
    {
        Path("/"),
        Path("/home"),
        Path("/home/postfiat"),
        Path("/home/postfiat/repos"),
    }
)
LND_PROTO_SHA256 = {
    "lightning_pb2.py": (
        "638d62e3e1faf48be6b9172e2472d650e4127207597741b9cedd52732f14cde7"
    ),
    "lightning_pb2_grpc.py": (
        "09b22f3a7ed4d3369726a8d3a2e9ebf174ba4e2b94da4ba1cb3068af33c4d9b8"
    ),
    "router_pb2.py": (
        "829e72a81f07f020ebab1e12a01fe1460f187b17166aaa1bbe30b51b54fe900a"
    ),
    "router_pb2_grpc.py": (
        "e016b031ec3dfdd05b749818f24ed8945bc775ee6b7711decf9cfe5a6523b09b"
    ),
}
GIT_OBJECT_ID = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")


class CompositionError(RealValuePolicyError):
    """Local process configuration failed before a value surface was enabled."""


class GraduationRuntimeFacade:
    """Hold new ARMED reverse swaps until their separate cadence/refund drill.

    Recovery remains available after an outgoing Lightning attempt is already
    durable; disabling reconciliation at that point would increase loss risk.
    """

    _OFFRAMP_RECOVERY_STATES = frozenset(
        {
            "LN_IN_FLIGHT",
            "LN_SETTLED",
            "REFUND_ELIGIBLE",
            "PFTL_FINISH_FINAL",
            "PFTL_CANCEL_FINAL",
            "LOCK_FAILED",
            "QUOTE_EXPIRED",
            "ABORTED_NO_VALUE",
        }
    )

    def __init__(
        self,
        runtime: MainnetCoordinatorRuntime,
        *,
        release_guard: Callable[[], Mapping[str, Any]] | None = None,
    ) -> None:
        self._runtime = runtime
        self._release_guard = release_guard

    def _require_release(self) -> Mapping[str, Any] | None:
        if self._release_guard is None:
            return None
        return self._release_guard()

    def __getattr__(self, name: str) -> Any:
        return getattr(self._runtime, name)

    def public_status(self) -> Mapping[str, Any]:
        status = dict(self._runtime.public_status())
        try:
            release = self._require_release()
        except Exception:
            status["mode"] = "HOLD"
            status["can_execute"] = False
            reasons = list(status.get("hold_reasons", []))
            if "coordinator_or_wallet_release_changed" not in reasons:
                reasons.append("coordinator_or_wallet_release_changed")
            status["hold_reasons"] = reasons
            status["source_release"] = {"status": "HOLD"}
        else:
            status["source_release"] = (
                {"status": "GREEN", **dict(release)}
                if isinstance(release, Mapping)
                else {"status": "NOT_CONFIGURED"}
            )
        status["direction_execution"] = {
            "lightning_to_pftl": "ENABLED_IF_ALL_GATES_PASS",
            "pftl_to_lightning": "HOLD_PENDING_REVERSE_REFUND_CADENCE_DRILL",
        }
        observer = getattr(self._runtime, "pftl_observer", None)
        evidence = getattr(observer, "last_valuation_evidence", None)
        status["pftl_valuation_binding"] = (
            dict(evidence)
            if isinstance(evidence, Mapping)
            else {
                "status": "HOLD",
                "reason": "six-ledger valuation binding has not passed",
            }
        )
        assurance = getattr(observer, "proof_assurance", None)
        status["pftl_proof_assurance"] = (
            dict(assurance)
            if isinstance(assurance, Mapping)
            else {
                "status": "HOLD",
                "reason": "reviewed PFTL proof-assurance boundary is unavailable",
            }
        )
        return status

    def create_quote(self, request: Mapping[str, Any]) -> Mapping[str, Any]:
        self._require_release()
        if request.get("direction") == "pftl_to_lightning":
            raise CompositionError(
                "ARMED reverse direction is held pending its refund/cadence drill"
            )
        return self._runtime.create_quote(request)

    def authorize_swap(
        self, swap_id: str, authorization_envelope: Mapping[str, Any]
    ) -> Mapping[str, Any]:
        self._require_release()
        swap = self._runtime.journal.get_swap(swap_id)
        if swap.get("direction") == "pftl_to_lightning":
            raise CompositionError(
                "ARMED reverse authorization is held pending its refund/cadence drill"
            )
        return self._runtime.authorize_swap(swap_id, authorization_envelope)

    def public_swap(self, swap_id: str) -> Mapping[str, Any]:
        swap = self._runtime.journal.get_swap(swap_id)
        if (
            swap.get("direction") == "lightning_to_pftl"
            and swap.get("state")
            in {"PFTL_LOCK_SUBMITTED", "PFTL_LOCK_FINAL", "LN_IN_FLIGHT"}
        ):
            self._require_release()
        return self._runtime.public_swap(swap_id)

    def execute_offramp(self, swap_id: str) -> Mapping[str, Any]:
        swap = self._runtime.journal.get_swap(swap_id)
        if swap.get("state") not in self._OFFRAMP_RECOVERY_STATES:
            raise CompositionError(
                "new ARMED reverse Lightning payment is disabled"
            )
        return self._runtime.execute_offramp(swap_id)

    def recover_swap(self, swap_id: str) -> Mapping[str, Any]:
        swap = self._runtime.journal.get_swap(swap_id)
        if (
            swap.get("direction") == "pftl_to_lightning"
            and swap.get("state") not in self._OFFRAMP_RECOVERY_STATES
        ):
            raise CompositionError(
                "new ARMED reverse Lightning payment is disabled"
            )
        return self._runtime.recover_swap(swap_id)


class PinnedUsdE8PftlObserver:
    """Bind the unitless RPC NAV integer to six finalized USD-e8 ledgers."""

    valuation_unit = "USD_E8_PER_WHOLE_ASSET_UNIT"
    valuation_scale = 100_000_000

    def __init__(
        self,
        observer: PftlQuorumObserver,
        expected_nav_usd_e8: int,
        valuation_binding: PftlValuationBinding,
    ) -> None:
        if type(expected_nav_usd_e8) is not int or expected_nav_usd_e8 <= 0:
            raise CompositionError("pinned handoff NAV USD-e8 value is invalid")
        self._observer = observer
        self.expected_nav_usd_e8 = expected_nav_usd_e8
        self._valuation_binding = valuation_binding
        self.last_valuation_evidence: Mapping[str, Any] | None = None
        handoff = valuation_binding.handoff
        self.proof_assurance: Mapping[str, Any] = {
            "schema": "postfiat.lightning_pftl_proof_assurance.v1",
            "lifecycle": list(handoff.proof_lifecycle),
            "profile": handoff.proof_profile,
            "attestation_count": handoff.proof_attestation_count,
            "proof_bytes_stored_on_chain": handoff.proof_bytes_stored_on_chain,
            "consensus_native_groth16_verification": (
                handoff.consensus_native_groth16_verification
            ),
        }

    def __getattr__(self, name: str) -> Any:
        return getattr(self._observer, name)

    def route_snapshot(self) -> Any:
        snapshot = self._observer.route_snapshot()
        if snapshot.nav_per_unit != self.expected_nav_usd_e8:
            raise CompositionError(
                "PFTL raw NAV does not match the handoff USD-e8 valuation pin"
            )
        try:
            evidence = self._valuation_binding.verify(snapshot)
        except PftlValuationBindingError as error:
            raise CompositionError(
                "PFTL finalized ledgers do not bind the RPC NAV to USD-e8"
            ) from error
        self.last_valuation_evidence = evidence.to_dict()
        return snapshot


def _canonical_absolute(path: str | Path, label: str) -> Path:
    candidate = Path(path)
    if not candidate.is_absolute():
        raise CompositionError(f"{label} must be an absolute path")
    absolute = Path(os.path.abspath(candidate))
    try:
        resolved = candidate.resolve(strict=False)
    except OSError as error:
        raise CompositionError(f"{label} could not be resolved") from error
    if absolute != resolved:
        raise CompositionError(f"{label} may not traverse a symbolic link")
    return absolute


def _secure_directory(path: Path, *, create: bool) -> None:
    if create:
        path.mkdir(mode=0o700, parents=True, exist_ok=True)
    try:
        metadata = path.lstat()
    except OSError as error:
        raise CompositionError(f"private directory is unavailable: {path}") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise CompositionError(f"private path is not a real directory: {path}")
    if metadata.st_uid != os.geteuid():
        raise CompositionError(f"private directory is not coordinator-owned: {path}")
    os.chmod(path, 0o700)


def _validate_private_file(path: Path, label: str, *, exact_bytes: int) -> bytes:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise CompositionError(f"{label} is unavailable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise CompositionError(f"{label} must be a regular, non-symlink file")
    if metadata.st_uid != os.geteuid():
        raise CompositionError(f"{label} is not coordinator-owned")
    if metadata.st_mode & (stat.S_IRWXG | stat.S_IRWXO):
        raise CompositionError(f"{label} must be mode 0600")
    if metadata.st_size != exact_bytes:
        raise CompositionError(f"{label} has an invalid size")
    try:
        value = path.read_bytes()
    except OSError as error:
        raise CompositionError(f"{label} could not be read") from error
    if len(value) != exact_bytes:
        raise CompositionError(f"{label} changed while it was read")
    return value


def _atomic_private_random(path: Path, length: int) -> None:
    if path.exists() or path.is_symlink():
        _validate_private_file(path, path.name, exact_bytes=length)
        return
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags, 0o600)
    except FileExistsError:
        _validate_private_file(path, path.name, exact_bytes=length)
        return
    try:
        value = os.urandom(length)
        written = 0
        while written < len(value):
            count = os.write(descriptor, value[written:])
            if count <= 0:
                raise CompositionError(f"short write while creating {path.name}")
            written += count
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.chmod(path, 0o600)
    directory_fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)


@dataclass(frozen=True)
class SecureStatePaths:
    root: Path
    config_dir: Path
    secrets_dir: Path
    database_dir: Path
    control_dir: Path
    artifact_dir: Path
    quote_seed: Path
    api_session_token: Path
    policy: Path
    price: Path
    lnd_connection: Path
    receive_only_macaroon: Path
    journal: Path
    budget: Path
    pftl_effects: Path
    source_release: Path
    process_lock: Path
    operator_socket: Path

    @classmethod
    def under(cls, root: str | Path = DEFAULT_STATE_ROOT) -> "SecureStatePaths":
        root_path = _canonical_absolute(root, "coordinator state root")
        if root_path in _BROAD_STATE_ROOTS:
            raise CompositionError("refusing a broad coordinator state root")
        return cls(
            root=root_path,
            config_dir=root_path / "coordinator-config",
            secrets_dir=root_path / "coordinator-secrets",
            database_dir=root_path / "coordinator-db",
            control_dir=root_path / "coordinator-control",
            artifact_dir=root_path / "coordinator-artifacts",
            quote_seed=root_path / "coordinator-secrets" / "quote-ed25519.seed",
            api_session_token=root_path
            / "coordinator-secrets"
            / "api-session.token",
            policy=root_path / "coordinator-config" / "policy.json",
            price=root_path / "coordinator-config" / "btc-price.json",
            lnd_connection=root_path
            / "coordinator-config"
            / "lnd-connection.json",
            receive_only_macaroon=(
                root_path / RECEIVE_ONLY_MACAROON_RELATIVE_PATH
            ),
            journal=root_path / "coordinator-db" / "swaps.sqlite",
            budget=root_path / "coordinator-db" / "real-value-budget.sqlite",
            pftl_effects=root_path / "coordinator-db" / "pftl-effects.sqlite",
            source_release=root_path
            / "coordinator-python-runtime-v2"
            / "source-release.json",
            process_lock=root_path
            / "coordinator-control"
            / "coordinator-process.lock",
            operator_socket=root_path
            / "coordinator-control"
            / "operator-authorization.sock",
        )


def prepare_secure_state(
    root: str | Path = DEFAULT_STATE_ROOT,
) -> dict[str, Any]:
    """Create only local state, a quote seed, and an API token.

    The function is idempotent.  It never prints or returns either secret.
    """

    paths = SecureStatePaths.under(root)
    for directory in (
        paths.root,
        paths.config_dir,
        paths.secrets_dir,
        paths.database_dir,
        paths.control_dir,
        paths.artifact_dir,
    ):
        _secure_directory(directory, create=True)
    _atomic_private_random(paths.quote_seed, QUOTE_SEED_BYTES)
    _atomic_private_random(paths.api_session_token, SESSION_TOKEN_BYTES)
    signer = Ed25519Signer.from_private_bytes(
        _validate_private_file(
            paths.quote_seed,
            "quote signer seed",
            exact_bytes=QUOTE_SEED_BYTES,
        )
    )
    token = _validate_private_file(
        paths.api_session_token,
        "API session token",
        exact_bytes=SESSION_TOKEN_BYTES,
    )
    return {
        "schema": "postfiat.lightning_coordinator_state.v1",
        "state_root": str(paths.root),
        "quote_signer_public_key_hex": signer.public_key_bytes().hex(),
        "api_session_token_sha256": hashlib.sha256(token).hexdigest(),
        "api_session_token_path": str(paths.api_session_token),
        "operator_socket": str(paths.operator_socket),
        "value_moved": False,
    }


def _reject_duplicate_json(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise CompositionError(f"duplicate JSON field: {key}")
        result[key] = value
    return result


def load_strict_json(path: str | Path, label: str) -> Mapping[str, Any]:
    """Load an owner-controlled, non-world-writable bounded JSON object."""

    config_path = _canonical_absolute(path, label)
    try:
        metadata = config_path.lstat()
    except OSError as error:
        raise CompositionError(f"{label} is unavailable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise CompositionError(f"{label} must be a regular, non-symlink file")
    if metadata.st_uid not in {0, os.geteuid()}:
        raise CompositionError(f"{label} has an untrusted owner")
    if metadata.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
        raise CompositionError(f"{label} must not be group/world writable")
    if metadata.st_size < 2 or metadata.st_size > MAX_CONFIG_BYTES:
        raise CompositionError(f"{label} size is invalid")
    try:
        value = json.loads(
            config_path.read_text(encoding="ascii"),
            object_pairs_hook=_reject_duplicate_json,
            parse_constant=lambda item: (_ for _ in ()).throw(
                CompositionError(f"non-finite JSON value: {item}")
            ),
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise CompositionError(f"{label} is not valid ASCII JSON") from error
    if not isinstance(value, Mapping):
        raise CompositionError(f"{label} must contain one JSON object")
    return value


def load_quote_signer(
    seed_path: str | Path, policy: RealValuePolicy
) -> Ed25519Signer:
    path = _canonical_absolute(seed_path, "quote signer seed path")
    signer = Ed25519Signer.from_private_bytes(
        _validate_private_file(
            path,
            "quote signer seed",
            exact_bytes=QUOTE_SEED_BYTES,
        )
    )
    observed = signer.public_key_bytes().hex()
    if observed != policy.quote_signer_public_key_hex:
        raise CompositionError("quote signer seed does not match the policy pin")
    return signer


def load_api_session_token(path: str | Path) -> bytes:
    token_path = _canonical_absolute(path, "API session token path")
    return _validate_private_file(
        token_path,
        "API session token",
        exact_bytes=SESSION_TOKEN_BYTES,
    )


def validate_receive_only_macaroon_path(
    paths: SecureStatePaths,
    connection: MainnetLndConnection,
) -> Path:
    """Forbid substituting an admin macaroon behind a profile/hash claim."""

    observed = _canonical_absolute(
        connection.macaroon_path,
        "LND receive-only macaroon path",
    )
    if observed != paths.receive_only_macaroon:
        raise CompositionError(
            "LND macaroon path is not the canonical state-dir receive-only path"
        )
    return observed


@dataclass(frozen=True)
class LndProtoModules:
    lightning_pb2: Any
    lightning_pb2_grpc: Any
    router_pb2: Any
    router_pb2_grpc: Any


def load_pinned_lnd_proto_modules(
    proto_dir: str | Path = DEFAULT_LND_PROTO_DIR,
) -> LndProtoModules:
    """Import only the digest-pinned LND v0.20.1 generated modules."""

    root = _canonical_absolute(proto_dir, "LND protobuf module directory")
    try:
        metadata = root.lstat()
    except OSError as error:
        raise CompositionError("LND protobuf module directory is unavailable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise CompositionError("LND protobuf module path must be a real directory")
    if (
        metadata.st_uid not in {0, os.geteuid()}
        or metadata.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
    ):
        raise CompositionError(
            "LND protobuf module directory has unsafe ownership or permissions"
        )
    for filename, expected in LND_PROTO_SHA256.items():
        path = root / filename
        try:
            file_metadata = path.lstat()
            encoded = path.read_bytes()
        except OSError as error:
            raise CompositionError(f"generated LND module is absent: {filename}") from error
        if stat.S_ISLNK(file_metadata.st_mode) or not stat.S_ISREG(
            file_metadata.st_mode
        ):
            raise CompositionError(f"generated LND module is not regular: {filename}")
        if (
            file_metadata.st_uid not in {0, os.geteuid()}
            or file_metadata.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        ):
            raise CompositionError(
                f"generated LND module ownership or permissions are unsafe: {filename}"
            )
        if hashlib.sha256(encoded).hexdigest() != expected:
            raise CompositionError(
                f"generated LND module digest mismatch: {filename}"
            )

    module_names = (
        "lightning_pb2",
        "lightning_pb2_grpc",
        "router_pb2",
        "router_pb2_grpc",
    )
    for name in module_names:
        existing = sys.modules.get(name)
        if existing is None:
            continue
        existing_file = getattr(existing, "__file__", None)
        if existing_file is None or Path(existing_file).resolve().parent != root:
            raise CompositionError(
                f"generated LND module was already imported from another path: {name}"
            )
    sys.path.insert(0, str(root))
    try:
        modules = tuple(importlib.import_module(name) for name in module_names)
    except Exception as error:
        raise CompositionError("pinned LND protobuf modules failed to import") from error
    finally:
        if sys.path and sys.path[0] == str(root):
            sys.path.pop(0)
    return LndProtoModules(*modules)


def _git_read(repo_root: Path, *arguments: str) -> str:
    try:
        completed = subprocess.run(
            ["/usr/bin/git", "-C", str(repo_root), *arguments],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=False,
            timeout=10,
            env={
                "PATH": "/usr/bin:/bin",
                "LANG": "C",
                "LC_ALL": "C",
            },
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise CompositionError("coordinator source git verification failed") from error
    if completed.returncode != 0 or len(completed.stdout) > 1024 * 1024:
        raise CompositionError("coordinator source git verification failed")
    try:
        return completed.stdout.decode("ascii").strip()
    except UnicodeError as error:
        raise CompositionError("coordinator source git output is not ASCII") from error


def validate_armed_source_release(
    pin_path: str | Path,
    *,
    repo_root: str | Path | None = None,
) -> dict[str, Any]:
    """Require the ARMED coordinator/UI surfaces to equal one clean commit."""

    value = load_strict_json(pin_path, "coordinator source release pin")
    expected_fields = frozenset(
        {"schema", "git_commit", "git_tree", "clean", "targets"}
    )
    if frozenset(value.keys()) != expected_fields:
        raise CompositionError("coordinator source release field set mismatch")
    if value["schema"] != SOURCE_RELEASE_SCHEMA:
        raise CompositionError("unsupported coordinator source release schema")
    commit = value["git_commit"]
    tree = value["git_tree"]
    if (
        type(commit) is not str
        or GIT_OBJECT_ID.fullmatch(commit) is None
        or type(tree) is not str
        or GIT_OBJECT_ID.fullmatch(tree) is None
    ):
        raise CompositionError("coordinator source release git pin is invalid")
    if value["clean"] is not True:
        raise CompositionError("ARMED coordinator source release is not clean")
    targets = value["targets"]
    if (
        not isinstance(targets, list)
        or tuple(targets) != SOURCE_RELEASE_TARGETS
    ):
        raise CompositionError("coordinator source release targets mismatch")
    root = (
        Path(__file__).resolve().parents[3]
        if repo_root is None
        else _canonical_absolute(repo_root, "coordinator repository root")
    )
    observed_commit = _git_read(root, "rev-parse", "--verify", "HEAD^{commit}")
    observed_tree = _git_read(root, "rev-parse", "--verify", "HEAD^{tree}")
    if observed_commit != commit or observed_tree != tree:
        raise CompositionError("coordinator source commit/tree pin mismatch")
    dirty = _git_read(
        root,
        "status",
        "--porcelain=v1",
        "--untracked-files=all",
        "--",
        *SOURCE_RELEASE_TARGETS,
    )
    if dirty:
        raise CompositionError("ARMED coordinator source paths are dirty")
    return {
        "schema": SOURCE_RELEASE_SCHEMA,
        "git_commit": commit,
        "git_tree": tree,
        "clean": True,
        "targets": list(SOURCE_RELEASE_TARGETS),
    }


def validate_activation(
    policy: RealValuePolicy,
    *,
    armed_ack: str | None,
    signer_key_file: str | Path | None,
) -> None:
    if policy.mode is ExecutionMode.DRY_RUN:
        if armed_ack is not None or signer_key_file is not None:
            raise CompositionError("DRY_RUN refuses arming acknowledgement and signer")
        return
    if armed_ack != ARMED_PROCESS_ACK:
        raise CompositionError("ARMED mode requires the exact process acknowledgement")
    if signer_key_file is None:
        raise CompositionError("ARMED mode requires an explicit PFTL signer handle")


def exposure_limits(
    policy: RealValuePolicy,
    *,
    nav_per_unit_usd_e8: int,
) -> ExposureLimits:
    """Convert the immutable USD ceilings to conservative NAVcoin atom caps."""

    if type(nav_per_unit_usd_e8) is not int or nav_per_unit_usd_e8 <= 0:
        raise CompositionError("finalized NAV USD-e8 value must be positive")
    atoms_per_unit = 10**policy.pftl_asset_precision

    def ceil_atoms(usd_e8: int) -> int:
        value = (
            usd_e8 * atoms_per_unit
            + nav_per_unit_usd_e8
            - 1
        ) // nav_per_unit_usd_e8
        if value <= 0 or value > (1 << 63) - 1:
            raise CompositionError("derived exposure cap is outside uint63")
        return value

    return ExposureLimits(
        per_principal_atoms=ceil_atoms(policy.max_per_run_usd_e8),
        aggregate_atoms=ceil_atoms(policy.max_lifetime_usd_e8),
    )


def validate_aggregate_onramp_capacity(
    limits: ExposureLimits,
    *,
    coordinator_inventory_atoms: int,
    user_receive_headroom_atoms: int,
) -> None:
    for value, label in (
        (coordinator_inventory_atoms, "coordinator inventory"),
        (user_receive_headroom_atoms, "user receive headroom"),
    ):
        if type(value) is not int or value < 0 or value > (1 << 63) - 1:
            raise CompositionError(f"finalized {label} is outside uint63")
    if limits.aggregate_atoms > coordinator_inventory_atoms:
        raise CompositionError(
            "aggregate on-ramp exposure exceeds finalized coordinator inventory"
        )
    if limits.aggregate_atoms > user_receive_headroom_atoms:
        raise CompositionError(
            "aggregate on-ramp exposure exceeds finalized user receive headroom"
        )


@dataclass
class RuntimeComposition:
    policy: RealValuePolicy
    price: PriceObservation
    handoff: PersistentPftlHandoff
    lnd_connection: ConnectedMainnetLnd
    pftl_observer: Any
    pftl_backend: PersistentHandoffPftlBackend
    journal: CoordinatorJournal
    budget: RealValueBudget
    runtime: Any
    api_session_token: bytes

    def close(self) -> None:
        errors: list[Exception] = []
        for closer in (
            self.journal.close,
            self.budget.close,
            self.lnd_connection.channel.close,
        ):
            try:
                closer()
            except Exception as error:  # pragma: no cover - best-effort cleanup
                errors.append(error)
        if errors:
            raise CompositionError("one or more coordinator resources failed to close")

    def __enter__(self) -> "RuntimeComposition":
        return self

    def __exit__(self, *_exc: object) -> None:
        self.close()


def compose_runtime(
    *,
    paths: SecureStatePaths,
    policy_path: str | Path | None = None,
    price_path: str | Path | None = None,
    lnd_connection_path: str | Path | None = None,
    handoff_path: str | Path = DEFAULT_HANDOFF_PATH,
    lnd_proto_dir: str | Path = DEFAULT_LND_PROTO_DIR,
    armed_ack: str | None = None,
    signer_key_file: str | Path | None = None,
    fee_bps: int = 0,
    lnd_connector: Callable[..., ConnectedMainnetLnd] = connect_mainnet_lnd,
) -> RuntimeComposition:
    """Compose the concrete runtime after all mode and identity pins validate."""

    policy = RealValuePolicy.from_mapping(
        load_strict_json(policy_path or paths.policy, "real-value policy")
    )
    validate_activation(
        policy,
        armed_ack=armed_ack,
        signer_key_file=signer_key_file,
    )
    if policy.mode is ExecutionMode.ARMED:
        validate_armed_source_release(paths.source_release)
    price = PriceObservation.from_mapping(
        load_strict_json(price_path or paths.price, "BTC price observation")
    )
    connection = MainnetLndConnection.from_mapping(
        load_strict_json(
            lnd_connection_path or paths.lnd_connection,
            "LND connection",
        )
    )
    validate_receive_only_macaroon_path(paths, connection)
    handoff = load_persistent_handoff(handoff_path)
    handoff.assert_policy_matches(policy)
    if (
        getattr(policy, "pftl_build_git_revision", None)
        != handoff.binary_build_git_revision
    ):
        raise CompositionError(
            "policy PFTL build revision does not match the pinned handoff"
        )
    if policy.pftl_asset_precision != handoff.asset_precision:
        raise CompositionError("policy asset precision does not match PFTL handoff")
    if policy.pftl_user_address != handoff.user_address:
        raise CompositionError("policy user address does not match PFTL handoff")
    handoff.verify_artifacts()
    quote_signer = load_quote_signer(paths.quote_seed, policy)
    session_token = load_api_session_token(paths.api_session_token)
    proto = load_pinned_lnd_proto_modules(lnd_proto_dir)

    connected: ConnectedMainnetLnd | None = None
    journal: CoordinatorJournal | None = None
    budget: RealValueBudget | None = None
    try:
        connected = lnd_connector(
            connection,
            lightning_pb2=proto.lightning_pb2,
            lightning_pb2_grpc=proto.lightning_pb2_grpc,
            router_pb2=proto.router_pb2,
            router_pb2_grpc=proto.router_pb2_grpc,
        )
        observer = PinnedUsdE8PftlObserver(
            PftlQuorumObserver(policy),
            handoff.nav_per_unit_usd_e8,
            PftlValuationBinding(handoff),
        )
        route = observer.route_snapshot()
        limits = exposure_limits(
            policy,
            nav_per_unit_usd_e8=handoff.nav_per_unit_usd_e8,
        )
        validate_aggregate_onramp_capacity(
            limits,
            coordinator_inventory_atoms=route.coordinator_inventory_atoms,
            user_receive_headroom_atoms=route.user_receive_headroom_atoms,
        )
        journal = CoordinatorJournal(paths.journal, limits)
        budget = RealValueBudget(paths.budget, policy)

        if policy.mode is ExecutionMode.ARMED:
            assert signer_key_file is not None  # validated above
            signer = SignerHandle(
                key_file=_canonical_absolute(
                    signer_key_file, "PFTL signer handle path"
                ),
                expected_address=handoff.coordinator_address,
            )
            backend = PersistentHandoffPftlBackend(
                handoff,
                signer=signer,
                effect_store=PftlEffectStore(paths.pftl_effects),
                artifact_dir=paths.artifact_dir,
                execution_ack=EXECUTION_ACK,
            )
        else:
            backend = PersistentHandoffPftlBackend(handoff)

        concrete_runtime = MainnetCoordinatorRuntime(
            policy=policy,
            price=price,
            lnd=connected.adapter,
            pftl_observer=observer,
            pftl_backend=backend,
            journal=journal,
            budget=budget,
            quote_signer=quote_signer,
            fee_bps=fee_bps,
        )
        runtime: Any = (
            GraduationRuntimeFacade(
                concrete_runtime,
                release_guard=lambda: validate_armed_source_release(
                    paths.source_release
                ),
            )
            if policy.mode is ExecutionMode.ARMED
            else concrete_runtime
        )
        return RuntimeComposition(
            policy=policy,
            price=price,
            handoff=handoff,
            lnd_connection=connected,
            pftl_observer=observer,
            pftl_backend=backend,
            journal=journal,
            budget=budget,
            runtime=runtime,
            api_session_token=session_token,
        )
    except Exception:
        if journal is not None:
            journal.close()
        if budget is not None:
            budget.close()
        if connected is not None:
            connected.channel.close()
        raise
