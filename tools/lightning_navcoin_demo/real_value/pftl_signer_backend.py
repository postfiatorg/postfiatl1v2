"""Signer-isolated PFTL escrow backend for the pinned persistent handoff.

Planning is read-only.  Quoting, signing and certification are unreachable
unless the backend is constructed with the exact explicit execution
acknowledgement.  This module is not invoked by the dry-check CLI.
"""

from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
import fcntl
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile
from typing import Any, Callable, Iterator, Mapping, Sequence

from ..coordinator.protocol import (
    SecretPreimage,
    decode_condition,
    encode_fulfillment,
)
from .pftl_effect_store import EffectStoreError, PftlEffectStore
from .pftl_handoff import (
    DEFAULT_HANDOFF_PATH,
    PersistentPftlHandoff,
    load_persistent_handoff,
    sha256_file,
)
from .pftl_handoff_check import (
    HandoffCheckError,
    PftlHandoffDryCheck,
    default_client_factory,
)


EXECUTION_ACK = "I_ACKNOWLEDGE_PINNED_PFTL_CHAIN_MUTATION"
ESCROW_ID_DOMAIN = b"postfiat.escrow_id.v1"
ESCROW_TX_ID_DOMAIN = b"postfiat.escrow_transaction.tx_id.v1"
MAX_SIGNED_JSON_BYTES = 512 * 1024


class PftlBackendError(RuntimeError):
    """A signer boundary, idempotency, or finalized-effect gate failed."""


@dataclass(frozen=True)
class SignerHandle:
    """Opaque signer locator; its contents are never opened by this module."""

    key_file: Path
    expected_address: str

    def validate_metadata(self) -> None:
        try:
            file_stat = self.key_file.lstat()
        except OSError as error:
            raise PftlBackendError("configured signer file is unavailable") from error
        if stat.S_ISLNK(file_stat.st_mode) or not stat.S_ISREG(file_stat.st_mode):
            raise PftlBackendError("signer file must be regular and non-symlink")
        if file_stat.st_uid != os.getuid():
            raise PftlBackendError("signer file is not owned by the coordinator user")
        if file_stat.st_mode & 0o077:
            raise PftlBackendError("signer file must not be group/world accessible")
        if file_stat.st_size < 1 or file_stat.st_size > 1024 * 1024:
            raise PftlBackendError("signer file size is outside the allowed bound")


@dataclass(frozen=True)
class CommandResult:
    returncode: int
    stderr: bytes


CommandRunner = Callable[[Sequence[str], Path, float], CommandResult]


def default_command_runner(
    command: Sequence[str],
    stdout_path: Path,
    timeout_seconds: float,
) -> CommandResult:
    """Run a pinned executable while keeping stdout in a private artifact."""

    with stdout_path.open("wb") as stdout_handle:
        os.chmod(stdout_path, 0o600)
        completed = subprocess.run(
            list(command),
            check=False,
            stdin=subprocess.DEVNULL,
            stdout=stdout_handle,
            stderr=subprocess.PIPE,
            timeout=timeout_seconds,
            env={
                "PATH": "/usr/bin:/bin",
                "LANG": "C",
                "LC_ALL": "C",
            },
        )
    return CommandResult(
        returncode=completed.returncode,
        stderr=completed.stderr,
    )


def _escrow_id(chain_id: str, owner: str, sequence: int) -> str:
    preimage = (
        f"chain_id={chain_id}\nowner={owner}\nowner_sequence={sequence}\n"
    ).encode("utf-8")
    digest = hashlib.sha3_384()
    digest.update(ESCROW_ID_DOMAIN)
    digest.update(b"\x00")
    digest.update(preimage)
    return digest.hexdigest()


def _operation_kind(operation: Mapping[str, Any]) -> str:
    value = operation.get("operation")
    if value not in {"escrow_create", "escrow_finish", "escrow_cancel"}:
        raise PftlBackendError("unsupported PFTL escrow operation")
    return str(value)


def _operation_source(operation: Mapping[str, Any]) -> str:
    kind = _operation_kind(operation)
    field = "recipient" if kind == "escrow_finish" else "owner"
    source = operation.get(field)
    if type(source) is not str or not source:
        raise PftlBackendError("PFTL escrow operation has no authorized source")
    return source


def _operation_fields(kind: str) -> tuple[str, ...]:
    if kind == "escrow_create":
        return (
            "operation",
            "owner",
            "recipient",
            "asset_id",
            "amount",
            "condition",
            "finish_after",
            "cancel_after",
        )
    if kind == "escrow_finish":
        return ("operation", "escrow_id", "owner", "recipient", "fulfillment")
    if kind == "escrow_cancel":
        return ("operation", "escrow_id", "owner")
    raise PftlBackendError("unsupported escrow operation kind")


def _operation_view(operation: Mapping[str, Any]) -> dict[str, Any]:
    """Normalize serde's omitted zero-valued create finish_after field."""

    kind = _operation_kind(operation)
    result: dict[str, Any] = {}
    for field in _operation_fields(kind):
        if kind == "escrow_create" and field == "finish_after":
            result[field] = operation.get(field, 0)
        else:
            if field not in operation:
                raise PftlBackendError(f"PFTL escrow operation lacks {field}")
            result[field] = operation[field]
    return result


def _canonical(value: Any) -> str:
    try:
        return json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
    except (TypeError, ValueError, UnicodeError) as error:
        raise PftlBackendError("PFTL operation is not canonical JSON") from error


def _signed_transaction_id(signed: Mapping[str, Any]) -> str:
    """Mirror SignedEscrowTransaction::tx_id_preimage_bytes exactly."""

    unsigned = signed.get("unsigned")
    if not isinstance(unsigned, Mapping):
        raise PftlBackendError("signed escrow transaction has no unsigned body")
    kind = _operation_kind(unsigned)
    required_unsigned = (
        "chain_id",
        "genesis_hash",
        "protocol_version",
        "address_namespace",
        "transaction_kind",
        "signature_algorithm_id",
        "source",
        "fee",
        "sequence",
    )
    for field in required_unsigned:
        if field not in unsigned:
            raise PftlBackendError(f"signed escrow transaction lacks {field}")
    operation_view = _operation_view(unsigned)
    header = (
        "postfiat.escrow_transaction.v1\n"
        f"chain_id={unsigned['chain_id']}\n"
        f"genesis_hash={unsigned['genesis_hash']}\n"
        f"protocol_version={unsigned['protocol_version']}\n"
        f"address_namespace={unsigned['address_namespace']}\n"
        f"transaction_kind={unsigned['transaction_kind']}\n"
        f"signature_algorithm_id={unsigned['signature_algorithm_id']}\n"
        f"source={unsigned['source']}\n"
        f"fee={unsigned['fee']}\n"
        f"sequence={unsigned['sequence']}\n"
        f"operation={kind}\n"
    )
    if kind == "escrow_create":
        condition = operation_view["condition"]
        if type(condition) is not str:
            raise PftlBackendError("signed escrow condition is not a string")
        operation = (
            f"owner={operation_view['owner']}\n"
            f"recipient={operation_view['recipient']}\n"
            f"asset_id={operation_view['asset_id']}\n"
            f"amount={operation_view['amount']}\n"
            f"condition_bytes={len(condition.encode('utf-8'))}\n"
            f"condition={condition}\n"
            f"finish_after={operation_view['finish_after']}\n"
            f"cancel_after={operation_view['cancel_after']}\n"
        )
    elif kind == "escrow_finish":
        fulfillment = operation_view["fulfillment"]
        if type(fulfillment) is not str:
            raise PftlBackendError("signed escrow fulfillment is not a string")
        operation = (
            f"escrow_id={operation_view['escrow_id']}\n"
            f"owner={operation_view['owner']}\n"
            f"recipient={operation_view['recipient']}\n"
            f"fulfillment_bytes={len(fulfillment.encode('utf-8'))}\n"
            f"fulfillment={fulfillment}\n"
        )
    else:
        operation = (
            f"escrow_id={operation_view['escrow_id']}\n"
            f"owner={operation_view['owner']}\n"
        )
    algorithm = signed.get("algorithm_id")
    public_key = signed.get("public_key_hex")
    signature = signed.get("signature_hex")
    if not all(type(value) is str and value for value in (algorithm, public_key, signature)):
        raise PftlBackendError("signed escrow cryptographic fields are malformed")
    preimage = (
        header
        + operation
        + f"algorithm={algorithm}\n"
        + f"public_key={public_key}\n"
        + f"signature={signature}\n"
    ).encode("utf-8")
    digest = hashlib.sha3_384()
    digest.update(ESCROW_TX_ID_DOMAIN)
    digest.update(b"\x00")
    digest.update(preimage)
    return digest.hexdigest()


class PersistentHandoffPftlBackend:
    """Read-only planner with a separately armed signer/certifier boundary."""

    def __init__(
        self,
        handoff: PersistentPftlHandoff,
        *,
        signer: SignerHandle | None = None,
        effect_store: PftlEffectStore | None = None,
        artifact_dir: str | Path | None = None,
        execution_ack: str | None = None,
        client_factory: Callable[[str], Any] = default_client_factory,
        command_runner: CommandRunner = default_command_runner,
    ) -> None:
        self.handoff = handoff
        self.signer = signer
        self.effect_store = effect_store
        self.artifact_dir = None if artifact_dir is None else Path(artifact_dir).resolve()
        self._execution_enabled = execution_ack == EXECUTION_ACK
        if execution_ack is not None and not self._execution_enabled:
            raise PftlBackendError("explicit PFTL execution acknowledgement is invalid")
        self._client_factory = client_factory
        self._command_runner = command_runner
        self._clients = tuple(client_factory(endpoint) for endpoint in handoff.rpc_endpoints)
        if self._execution_enabled:
            if signer is None or effect_store is None or self.artifact_dir is None:
                raise PftlBackendError(
                    "armed backend requires signer, effect store and artifact directory"
                )
            if signer.expected_address != handoff.coordinator_address:
                raise PftlBackendError("signer address does not match handoff coordinator")
            signer.validate_metadata()
            self.artifact_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
            os.chmod(self.artifact_dir, 0o700)

    @classmethod
    def from_pinned_release(
        cls,
        *,
        handoff_path: str | Path = DEFAULT_HANDOFF_PATH,
        signer: SignerHandle | None = None,
        effect_store: PftlEffectStore | None = None,
        artifact_dir: str | Path | None = None,
        execution_ack: str | None = None,
        client_factory: Callable[[str], Any] = default_client_factory,
        command_runner: CommandRunner = default_command_runner,
    ) -> "PersistentHandoffPftlBackend":
        """Compose from the reviewed release digests, not caller-supplied pins."""

        return cls(
            load_persistent_handoff(handoff_path),
            signer=signer,
            effect_store=effect_store,
            artifact_dir=artifact_dir,
            execution_ack=execution_ack,
            client_factory=client_factory,
            command_runner=command_runner,
        )

    def _dry_check(self) -> dict[str, Any]:
        return PftlHandoffDryCheck(
            self.handoff,
            client_factory=self._client_factory,
        ).run()

    def _status_identity(self, status: Any) -> dict[str, Any]:
        """Normalize the finalized identity used to bind one RPC read bundle."""

        if (
            not isinstance(status, Mapping)
            or status.get("chain_id") != self.handoff.chain_id
            or status.get("genesis_hash") != self.handoff.genesis_hash
            or status.get("protocol_version") != 1
            or status.get("rpc_schema") != self.handoff.rpc_protocol
            or status.get("validator_count") != 6
            or status.get("build_git_revision")
            != self.handoff.binary_build_git_revision
            or status.get("status") != "running"
        ):
            raise PftlBackendError(
                "validator identity or build mismatches pinned handoff"
            )
        node_id = status.get("node_id")
        height = status.get("block_height")
        tip = status.get("block_tip_hash")
        root = status.get("state_root")
        if (
            type(node_id) is not str
            or not node_id
            or type(height) is not int
            or height < 0
            or type(tip) is not str
            or not tip
            or type(root) is not str
            or not root
        ):
            raise PftlBackendError("validator finalized identity is malformed")
        return {
            "node_id": node_id,
            "height": height,
            "tip": tip,
            "root": root,
            "build_git_revision": status["build_git_revision"],
            "active_nav_profiles": status.get("active_nav_profiles"),
        }

    def _status_sandwich(
        self,
        status_before: Any,
        status_after: Any,
        *,
        read_label: str,
    ) -> dict[str, Any]:
        """Reject a multi-call state bundle if its finalized view moved."""

        before = self._status_identity(status_before)
        after = self._status_identity(status_after)
        if before != after:
            raise PftlBackendError(
                f"{read_label} finalized view changed during RPC read"
            )
        return before

    @staticmethod
    def _status_anchor(identity: Mapping[str, Any]) -> dict[str, Any]:
        """Consensus fields expected to agree; node_id is checked separately."""

        return {
            key: value
            for key, value in identity.items()
            if key != "node_id"
        }

    def _six_account_sequence(self, address: str) -> int:
        rows: list[str] = []
        sequences: list[int] = []
        node_ids: set[str] = set()
        try:
            for client in self._clients:
                status_before = client.status()
                account = client.account(address)
                status_after = client.status()
                identity = self._status_sandwich(
                    status_before,
                    status_after,
                    read_label="account-sequence",
                )
                if identity["node_id"] in node_ids:
                    raise PftlBackendError(
                        "account sequence views are not distinct validators"
                    )
                node_ids.add(identity["node_id"])
                if account.get("address") != address:
                    raise PftlBackendError("signer account address mismatch")
                sequence = int(account.get("sequence"))
                sequences.append(sequence)
                rows.append(
                    _canonical(
                        {
                            "identity": self._status_anchor(identity),
                            "address": account.get("address"),
                            "sequence": sequence,
                        }
                    )
                )
        except (AttributeError, TypeError, ValueError, OSError) as error:
            if isinstance(error, PftlBackendError):
                raise
            raise PftlBackendError("six-validator account sequence read failed") from error
        if (
            node_ids != {f"validator-{index}" for index in range(6)}
            or len(rows) != 6
            or len(set(rows)) != 1
            or not sequences
        ):
            raise PftlBackendError("signer account sequence is not converged six-of-six")
        if sequences[0] < 0:
            raise PftlBackendError("signer account sequence is invalid")
        return sequences[0]

    def _six_asset_account_view(self, address: str) -> dict[str, int]:
        """Read one prospective owner without routing it through the signer."""

        canonical: list[str] = []
        normalized_rows: list[dict[str, int]] = []
        node_ids: set[str] = set()
        try:
            for client in self._clients:
                status_before = client.status()
                account = client.account(address)
                lines = client.account_lines(
                    address,
                    asset_id=self.handoff.asset_id,
                    limit=8,
                )
                status_after = client.status()
                identity = self._status_sandwich(
                    status_before,
                    status_after,
                    read_label="prospective-owner",
                )
                if identity["node_id"] in node_ids:
                    raise PftlBackendError(
                        "prospective owner views are not distinct validators"
                    )
                node_ids.add(identity["node_id"])
                if (
                    account.get("address") != address
                    or lines.get("account") != address
                    or lines.get("asset_id") != self.handoff.asset_id
                ):
                    raise PftlBackendError("prospective owner response identity mismatch")
                matches = [
                    line
                    for line in lines.get("lines", [])
                    if isinstance(line, Mapping)
                    and line.get("asset_id") == self.handoff.asset_id
                ]
                if len(matches) != 1:
                    raise PftlBackendError(
                        "prospective owner needs one NAVcoin trustline"
                    )
                line = matches[0]
                if (
                    line.get("account") != address
                    or line.get("issuer") != self.handoff.asset_issuer
                    or line.get("authorized") is not True
                    or line.get("frozen") is not False
                ):
                    raise PftlBackendError(
                        "prospective owner NAVcoin trustline is not movable"
                    )
                row = {
                    "sequence": int(account.get("sequence")),
                    "native_balance": int(account.get("balance")),
                    "asset_balance": int(line.get("balance")),
                    "asset_limit": int(line.get("limit")),
                }
                if min(row.values()) < 0:
                    raise PftlBackendError("prospective owner balances are invalid")
                if row["asset_balance"] > row["asset_limit"]:
                    raise PftlBackendError(
                        "prospective owner NAVcoin balance exceeds its limit"
                    )
                normalized_rows.append(row)
                canonical.append(
                    _canonical(
                        {
                            "identity": self._status_anchor(identity),
                            "account": row,
                        }
                    )
                )
        except (AttributeError, TypeError, ValueError, OSError) as error:
            if isinstance(error, PftlBackendError):
                raise
            raise PftlBackendError(
                "six-validator prospective owner read failed"
            ) from error
        if (
            node_ids != {f"validator-{index}" for index in range(6)}
            or len(canonical) != 6
            or len(set(canonical)) != 1
        ):
            raise PftlBackendError(
                "prospective owner state is not converged six-of-six"
            )
        return normalized_rows[0]

    def _six_fee_eligibility(
        self,
        *,
        source: str,
        operation: Mapping[str, Any],
        sequence: int,
        request_label: str,
    ) -> int:
        """Require an exact reserve-safe fee quote from every finalized view."""

        canonical: list[str] = []
        minimum_fee: int | None = None
        node_ids: set[str] = set()
        try:
            for index, client in enumerate(self._clients):
                status_before = client.status()
                response = client.escrow_fee_quote_response(
                    source,
                    dict(operation),
                    sequence=sequence,
                    request_id=f"ln-mainnet-preflight-{request_label}-{index}",
                )
                status_after = client.status()
                identity = self._status_sandwich(
                    status_before,
                    status_after,
                    read_label=f"{request_label}-fee",
                )
                if identity["node_id"] in node_ids:
                    raise PftlBackendError(
                        f"{request_label} fee views are not distinct validators"
                    )
                node_ids.add(identity["node_id"])
                result = (
                    response.get("result")
                    if isinstance(response, Mapping)
                    and response.get("ok") is True
                    else None
                )
                if not isinstance(result, Mapping):
                    raise PftlBackendError(
                        f"{request_label} fee quote is unavailable"
                    )
                fee = int(result.get("minimum_fee"))
                reserve = int(result.get("account_reserve"))
                balance_after = int(result.get("sender_balance_after_fee"))
                if (
                    result.get("schema") != "postfiat-escrow-fee-quote-v1"
                    or result.get("chain_id") != self.handoff.chain_id
                    or result.get("genesis_hash") != self.handoff.genesis_hash
                    or result.get("protocol_version") != 1
                    or result.get("source") != source
                    or result.get("sequence") != sequence
                    or result.get("sequence_source") != "explicit"
                    or _operation_view(result.get("operation", {}))
                    != _operation_view(operation)
                    or result.get("transaction_kind")
                    != _operation_kind(operation)
                    or result.get("sender_sequence") != sequence - 1
                    or result.get("mempool_pending_for_sender") != 0
                    or result.get("sender_meets_reserve_after_fee") is not True
                    or fee < 1
                    or reserve < 0
                    or balance_after < reserve
                ):
                    raise PftlBackendError(
                        f"{request_label} source is not reserve-safe for its exact fee"
                    )
                normalized = {
                    "identity": self._status_anchor(identity),
                    "source": source,
                    "sequence": sequence,
                    "operation": dict(operation),
                    "minimum_fee": fee,
                    "account_reserve": reserve,
                    "sender_balance_after_fee": balance_after,
                }
                canonical.append(_canonical(normalized))
                minimum_fee = fee
        except (AttributeError, TypeError, ValueError, OSError) as error:
            if isinstance(error, PftlBackendError):
                raise
            raise PftlBackendError(
                f"six-validator {request_label} fee preflight failed"
            ) from error
        if (
            node_ids != {f"validator-{index}" for index in range(6)}
            or len(canonical) != 6
            or len(set(canonical)) != 1
            or minimum_fee is None
        ):
            raise PftlBackendError(
                f"{request_label} fee eligibility is not converged six-of-six"
            )
        return minimum_fee

    def plan_create(
        self,
        *,
        owner: str,
        recipient: str,
        asset_id: str,
        amount_atoms: int,
        condition: str,
        finish_after: int,
        cancel_after: int,
    ) -> Any:
        """Plan against finalized RPC state; never quote, sign, or submit."""

        if asset_id != self.handoff.asset_id:
            raise PftlBackendError("create asset is not the proven-NAV NAVcoin")
        if (
            type(owner) is not str
            or len(owner) != 42
            or not owner.startswith("pf")
            or any(character not in "0123456789abcdef" for character in owner[2:])
            or type(recipient) is not str
            or len(recipient) != 42
            or not recipient.startswith("pf")
            or any(
                character not in "0123456789abcdef"
                for character in recipient[2:]
            )
            or recipient == owner
        ):
            raise PftlBackendError("create owner or recipient is invalid")
        if type(amount_atoms) is not int or amount_atoms < 1:
            raise PftlBackendError("create amount must be positive")
        decode_condition(condition)
        if (
            type(finish_after) is not int
            or type(cancel_after) is not int
            or finish_after < 0
            or cancel_after <= finish_after
        ):
            raise PftlBackendError("create timelocks are invalid")
        self._dry_check()
        owner_view = self._six_asset_account_view(owner)
        recipient_view = self._six_asset_account_view(recipient)
        if amount_atoms > owner_view["asset_balance"]:
            raise PftlBackendError("create amount exceeds finalized owner inventory")
        if owner_view["native_balance"] < 1:
            raise PftlBackendError("create owner cannot pay an escrow fee")
        if amount_atoms > (
            recipient_view["asset_limit"] - recipient_view["asset_balance"]
        ):
            raise PftlBackendError(
                "create amount exceeds finalized recipient trustline headroom"
            )
        if recipient_view["native_balance"] < 1:
            raise PftlBackendError(
                "create recipient cannot pay the escrow-finish fee"
            )
        sequence = owner_view["sequence"] + 1
        operation = {
            "operation": "escrow_create",
            "owner": owner,
            "recipient": recipient,
            "asset_id": asset_id,
            "amount": amount_atoms,
            "condition": condition,
            "finish_after": finish_after,
            "cancel_after": cancel_after,
        }
        self._six_fee_eligibility(
            source=owner,
            operation=operation,
            sequence=sequence,
            request_label="escrow-create",
        )
        expected_escrow_id = _escrow_id(
            self.handoff.chain_id, owner, sequence
        )
        finish_operation = {
            "operation": "escrow_finish",
            "escrow_id": expected_escrow_id,
            "owner": owner,
            "recipient": recipient,
            # Quote weight and reserve eligibility only. This placeholder is
            # never signed or submitted and need not satisfy the hashlock.
            "fulfillment": encode_fulfillment(bytes(32)),
        }
        self._six_fee_eligibility(
            source=recipient,
            operation=finish_operation,
            sequence=recipient_view["sequence"] + 1,
            request_label="escrow-finish",
        )
        from .runtime import PftlEscrowPlan

        return PftlEscrowPlan(
            owner=owner,
            owner_sequence=sequence,
            recipient=recipient,
            expected_escrow_id=expected_escrow_id,
            operation=operation,
        )

    def _require_execution(self) -> tuple[SignerHandle, PftlEffectStore, Path]:
        if (
            not self._execution_enabled
            or self.signer is None
            or self.effect_store is None
            or self.artifact_dir is None
        ):
            raise PftlBackendError(
                "PFTL execution is disabled; planner remains read-only"
            )
        self.signer.validate_metadata()
        self.handoff.verify_artifacts()
        return self.signer, self.effect_store, self.artifact_dir

    @contextmanager
    def _submission_lock(self) -> Iterator[None]:
        _, _, artifact_dir = self._require_execution()
        lock_path = artifact_dir / ".submission.lock"
        descriptor = os.open(lock_path, os.O_CREAT | os.O_RDWR, 0o600)
        try:
            os.fchmod(descriptor, 0o600)
            fcntl.flock(descriptor, fcntl.LOCK_EX)
            yield
        finally:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
            os.close(descriptor)

    def _quote(
        self,
        *,
        source: str,
        operation: Mapping[str, Any],
        sequence: int,
        request_id: str,
    ) -> Mapping[str, Any]:
        converged_fee = self._six_fee_eligibility(
            source=source,
            operation=operation,
            sequence=sequence,
            request_label=f"{_operation_kind(operation)}-signing",
        )
        status_before = self._clients[0].status()
        response = self._clients[0].escrow_fee_quote_response(
            source,
            dict(operation),
            sequence=sequence,
            request_id=request_id,
        )
        status_after = self._clients[0].status()
        identity = self._status_sandwich(
            status_before,
            status_after,
            read_label="signing-quote",
        )
        if identity["node_id"] != "validator-0":
            raise PftlBackendError(
                "signing quote did not come from pinned validator-0"
            )
        if not isinstance(response, Mapping) or response.get("ok") is not True:
            raise PftlBackendError("pinned RPC did not return an accepted escrow quote")
        result = response.get("result")
        if not isinstance(result, Mapping):
            raise PftlBackendError("escrow quote has no result")
        expected = {
            "schema": "postfiat-escrow-fee-quote-v1",
            "chain_id": self.handoff.chain_id,
            "genesis_hash": self.handoff.genesis_hash,
            "protocol_version": 1,
            "source": source,
            "sequence": sequence,
            "sequence_source": "explicit",
            "transaction_kind": _operation_kind(operation),
            "sender_meets_reserve_after_fee": True,
            "mempool_pending_for_sender": 0,
        }
        if any(result.get(field) != value for field, value in expected.items()):
            raise PftlBackendError("escrow quote does not match the pinned request")
        if _operation_view(result.get("operation", {})) != _operation_view(
            operation
        ):
            raise PftlBackendError("escrow quote operation does not match request")
        if (
            type(result.get("minimum_fee")) is not int
            or result["minimum_fee"] < 1
            or result["minimum_fee"] != converged_fee
            or result.get("sender_sequence") != sequence - 1
        ):
            raise PftlBackendError("escrow quote fee or signer sequence is invalid")
        return response

    @staticmethod
    def _atomic_json(path: Path, value: Mapping[str, Any]) -> None:
        temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
        with temporary.open("x", encoding="utf-8") as handle:
            os.chmod(temporary, 0o600)
            json.dump(
                value,
                handle,
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=True,
                allow_nan=False,
            )
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)

    @staticmethod
    def _durable_replace_signed(temporary: Path, path: Path) -> None:
        """Publish a signed artifact durably before SQLite may reference it."""

        descriptor: int | None = None
        directory_descriptor: int | None = None
        try:
            os.replace(temporary, path)
            os.chmod(path, 0o600)
            descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
            file_stat = os.fstat(descriptor)
            if not stat.S_ISREG(file_stat.st_mode):
                raise PftlBackendError(
                    "published signed artifact is not a regular file"
                )
            os.fsync(descriptor)
            directory_descriptor = os.open(
                path.parent, os.O_RDONLY | os.O_DIRECTORY
            )
            os.fsync(directory_descriptor)
        except OSError as error:
            raise PftlBackendError(
                "signed artifact could not be durably published"
            ) from error
        finally:
            if descriptor is not None:
                os.close(descriptor)
            if directory_descriptor is not None:
                os.close(directory_descriptor)

    @staticmethod
    def _signed_artifact_sha256(path: Path) -> str:
        try:
            return sha256_file(path)
        except OSError as error:
            raise PftlBackendError(
                "durable signed artifact is unavailable"
            ) from error

    def _validate_signed(
        self,
        path: Path,
        *,
        source: str,
        operation: Mapping[str, Any],
        sequence: int,
        minimum_fee: int,
    ) -> tuple[Mapping[str, Any], str]:
        try:
            file_stat = path.lstat()
            if (
                stat.S_ISLNK(file_stat.st_mode)
                or not stat.S_ISREG(file_stat.st_mode)
                or file_stat.st_size < 2
                or file_stat.st_size > MAX_SIGNED_JSON_BYTES
            ):
                raise PftlBackendError("signed artifact metadata is invalid")
            value = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise PftlBackendError("signer returned invalid JSON") from error
        if not isinstance(value, Mapping):
            raise PftlBackendError("signed escrow transaction is not an object")
        unsigned = value.get("unsigned")
        if not isinstance(unsigned, Mapping):
            raise PftlBackendError("signed escrow transaction has no unsigned body")
        operation_view = _operation_view(unsigned)
        if (
            unsigned.get("chain_id") != self.handoff.chain_id
            or unsigned.get("genesis_hash") != self.handoff.genesis_hash
            or unsigned.get("protocol_version") != 1
            or unsigned.get("source") != source
            or unsigned.get("sequence") != sequence
            or unsigned.get("fee") != minimum_fee
            or unsigned.get("transaction_kind") != _operation_kind(operation)
            or unsigned.get("signature_algorithm_id") != "ML-DSA-65"
            or value.get("algorithm_id") != "ML-DSA-65"
            or _canonical(operation_view) != _canonical(_operation_view(operation))
        ):
            raise PftlBackendError("signer output does not match the pinned quote")
        return value, _signed_transaction_id(value)

    @staticmethod
    def _rejected_receipt_is_mutation_free(receipt: Mapping[str, Any]) -> bool:
        """Recognize the pinned Receipt::rejected zero-effect encoding."""

        if any(
            receipt.get(field) != 0
            for field in (
                "fee_charged",
                "fee_burned",
                "minimum_fee",
                "account_reserve",
                "state_expansion_fee",
            )
        ):
            return False
        return (
            receipt.get("nft_issuer_transfer_fee", 0) == 0
            and receipt.get("nft_issuer_transfer_fee_recipient") is None
            and receipt.get("nft_collection_flags", 0) == 0
            and receipt.get("offer_id") is None
            and receipt.get("offer_fills", []) == []
            and receipt.get("atomic_swap_legs") is None
        )

    def _receipt_quorum(self, tx_id: str) -> dict[str, Any] | None:
        rows: list[dict[str, Any]] = []
        node_ids: set[str] = set()
        try:
            for client in self._clients:
                status_before = client.status()
                receipts = client.receipts(tx_id=tx_id, limit=2)
                status_after = client.status()
                identity = self._status_sandwich(
                    status_before,
                    status_after,
                    read_label="receipt",
                )
                node_id = identity["node_id"]
                if node_id in node_ids:
                    raise PftlBackendError("receipt views are not distinct validators")
                node_ids.add(node_id)
                rows.append(
                    {
                        "height": identity["height"],
                        "tip": identity["tip"],
                        "root": identity["root"],
                        "anchor": self._status_anchor(identity),
                        "receipts": receipts,
                    }
                )
        except Exception as error:
            if isinstance(error, PftlBackendError):
                raise
            raise PftlBackendError("six-validator receipt reconciliation failed") from error
        if node_ids != {f"validator-{index}" for index in range(6)}:
            raise PftlBackendError("receipt validator identities mismatch")
        empty_count = sum(row["receipts"] == [] for row in rows)
        if empty_count == 6:
            state_views = {
                _canonical(row["anchor"])
                for row in rows
            }
            if len(state_views) != 1:
                raise PftlBackendError("absent receipt views are not converged")
            return None
        if empty_count:
            raise PftlBackendError("receipt is present on only part of the validator fleet")
        normalized: list[str] = []
        agreed_receipt: Mapping[str, Any] | None = None
        for row in rows:
            receipts = row["receipts"]
            if not isinstance(receipts, list) or len(receipts) != 1:
                raise PftlBackendError("expected exactly one receipt per validator")
            receipt = receipts[0]
            if (
                not isinstance(receipt, Mapping)
                or receipt.get("tx_id") != tx_id
            ):
                raise PftlBackendError("PFTL effect receipt identity is malformed")
            accepted = receipt.get("accepted")
            code = receipt.get("code")
            if accepted is True:
                if code != "accepted":
                    raise PftlBackendError(
                        "accepted PFTL effect lacks literal ACCEPTED receipt"
                    )
            elif accepted is False:
                if (
                    type(code) is not str
                    or not code
                    or len(code) > 256
                    or code == "accepted"
                    or not self._rejected_receipt_is_mutation_free(receipt)
                ):
                    raise PftlBackendError(
                        "rejected PFTL effect is not a zero-effect consensus receipt"
                    )
            else:
                raise PftlBackendError("PFTL receipt accepted flag is not boolean")
            agreed_receipt = receipt
            normalized.append(
                _canonical(
                    {
                        "anchor": row["anchor"],
                        "receipt": dict(receipt),
                    }
                )
            )
        if len(set(normalized)) != 1:
            raise PftlBackendError("PFTL receipt views are not converged six-of-six")
        agreed = rows[0]
        if agreed_receipt is None:
            raise PftlBackendError("PFTL receipt agreement is empty")
        accepted = agreed_receipt["accepted"] is True
        receipt_json = _canonical(dict(agreed_receipt))
        return {
            "tx_id": tx_id,
            "accepted": accepted,
            "code": str(agreed_receipt["code"]),
            "agreeing_validator_count": 6,
            "validator_count": 6,
            "finalized_height": int(agreed["height"]),
            "state_root": str(agreed["root"]),
            "block_tip_hash": str(agreed["tip"]),
            "mutation_free": False if accepted else True,
            "receipt_sha256": hashlib.sha256(
                receipt_json.encode("ascii")
            ).hexdigest(),
        }

    @staticmethod
    def _effect_from_evidence(evidence: Mapping[str, Any]) -> Any:
        from .runtime import PftlEffect

        return PftlEffect(
            tx_id=str(evidence["tx_id"]),
            accepted=evidence["accepted"] is True,
            code=str(evidence["code"]),
            agreeing_validator_count=int(evidence["agreeing_validator_count"]),
            validator_count=int(evidence["validator_count"]),
            finalized_height=int(evidence["finalized_height"]),
            state_root=str(evidence["state_root"]),
            block_tip_hash=str(evidence["block_tip_hash"]),
            mutation_free=(
                evidence.get("mutation_free")
                if type(evidence.get("mutation_free")) is bool
                else None
            ),
        )

    @staticmethod
    def _complete_receipt(
        store: PftlEffectStore,
        effect_key: str,
        tx_id: str,
        receipt: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Route accepted and rejected receipts to distinct durable terminals."""

        if receipt.get("accepted") is True:
            return store.mark_succeeded(
                effect_key,
                tx_id=tx_id,
                evidence=receipt,
            )
        if (
            receipt.get("accepted") is False
            and receipt.get("mutation_free") is True
        ):
            return store.mark_rejected(
                effect_key,
                tx_id=tx_id,
                evidence=receipt,
            )
        raise PftlBackendError(
            "receipt is neither accepted nor a mutation-free rejection"
        )

    def _execute(
        self,
        *,
        effect_key: str,
        kind: str,
        operation: Mapping[str, Any],
        signer_sequence: int | None,
        escrow_id: str,
    ) -> Any:
        signer, store, artifact_dir = self._require_execution()
        if _operation_source(operation) != signer.expected_address:
            raise PftlBackendError("operation is not authorized by the isolated signer")
        request = {
            "schema": "postfiat.lightning.pftl_effect_request.v1",
            "chain_id": self.handoff.chain_id,
            "genesis_hash": self.handoff.genesis_hash,
            "kind": kind,
            "operation": dict(operation),
            "escrow_id": escrow_id,
        }
        with self._submission_lock():
            row = store.begin(
                effect_key=effect_key,
                kind=kind,
                request=request,
                signer_address=signer.expected_address,
                escrow_id=escrow_id,
                signer_sequence=signer_sequence,
            )
            if row["status"] in {"SUCCEEDED", "REJECTED"}:
                return self._effect_from_evidence(row["evidence"])
            if row["signer_sequence"] is None:
                sequence = self._six_account_sequence(signer.expected_address) + 1
                row = store.reserve_sequence(effect_key, sequence)
            sequence = int(row["signer_sequence"])
            effect_tag = hashlib.sha256(effect_key.encode("ascii")).hexdigest()
            signed_path = artifact_dir / f"{effect_tag}.signed-escrow.json"

            signed: Mapping[str, Any] | None = None
            tx_id: str | None = None
            if row["signed_artifact_path"] is not None:
                if Path(row["signed_artifact_path"]) != signed_path:
                    raise PftlBackendError("journal signed-artifact path mismatch")
                if (
                    self._signed_artifact_sha256(signed_path)
                    != row["signed_artifact_sha256"]
                ):
                    raise PftlBackendError("durable signed artifact hash mismatch")
                signed, tx_id = self._validate_signed(
                    signed_path,
                    source=signer.expected_address,
                    operation=operation,
                    sequence=sequence,
                    minimum_fee=int(
                        signed_path.exists()
                        and json.loads(signed_path.read_text())["unsigned"]["fee"]
                    ),
                )
                receipt = self._receipt_quorum(tx_id)
                if receipt is not None:
                    completed = self._complete_receipt(
                        store, effect_key, tx_id, receipt
                    )
                    return self._effect_from_evidence(completed["evidence"])
                if row["status"] == "SUBMITTING":
                    raise PftlBackendError(
                        "prior certification outcome remains unresolved; "
                        "refusing an automatic duplicate submission"
                    )

            if signed is None:
                self._dry_check()
                if self._six_account_sequence(signer.expected_address) + 1 != sequence:
                    raise PftlBackendError(
                        "reserved signer sequence is stale before signing"
                    )
                quote = self._quote(
                    source=signer.expected_address,
                    operation=operation,
                    sequence=sequence,
                    request_id=f"lnnav-{effect_tag[:24]}",
                )
                with tempfile.TemporaryDirectory(
                    prefix=".pftl-quote-",
                    dir=artifact_dir,
                ) as temporary_directory:
                    quote_path = Path(temporary_directory) / "quote.json"
                    self._atomic_json(quote_path, quote)
                    unsigned_signed_path = Path(temporary_directory) / "signed.json"
                    result = self._command_runner(
                        (
                            str(self.handoff.binary_path),
                            "wallet-sign-escrow-transaction",
                            "--key-file",
                            str(signer.key_file),
                            "--quote-file",
                            str(quote_path),
                        ),
                        unsigned_signed_path,
                        60,
                    )
                    if result.returncode != 0:
                        raise PftlBackendError(
                            "pinned signer command failed; "
                            f"stderr_sha256={hashlib.sha256(result.stderr).hexdigest()}"
                        )
                    signed, tx_id = self._validate_signed(
                        unsigned_signed_path,
                        source=signer.expected_address,
                        operation=operation,
                        sequence=sequence,
                        minimum_fee=int(quote["result"]["minimum_fee"]),
                    )
                    self._durable_replace_signed(
                        unsigned_signed_path, signed_path
                    )
                row = store.mark_signed(
                    effect_key,
                    signed_artifact_path=signed_path,
                    signed_artifact_sha256=self._signed_artifact_sha256(
                        signed_path
                    ),
                )

            if tx_id is None:
                raise PftlBackendError("signed effect has no deterministic tx id")
            self._dry_check()
            self.handoff.verify_artifacts()
            if (
                self._signed_artifact_sha256(signed_path)
                != row["signed_artifact_sha256"]
            ):
                raise PftlBackendError("signed artifact changed before certification")
            receipt = self._receipt_quorum(tx_id)
            if receipt is not None:
                completed = self._complete_receipt(
                    store, effect_key, tx_id, receipt
                )
                return self._effect_from_evidence(completed["evidence"])

            store.mark_submitting(effect_key)
            helper_log = artifact_dir / f"{effect_tag}.certification.log"
            result = self._command_runner(
                (str(self.handoff.certification_helper), str(signed_path)),
                helper_log,
                180,
            )
            receipt = self._receipt_quorum(tx_id)
            if receipt is None:
                raise PftlBackendError(
                    "certification outcome is unresolved; effect remains SUBMITTING; "
                    f"helper_exit={result.returncode}; "
                    f"stderr_sha256={hashlib.sha256(result.stderr).hexdigest()}"
                )
            completed = self._complete_receipt(
                store, effect_key, tx_id, receipt
            )
            return self._effect_from_evidence(completed["evidence"])

    def submit_create(self, plan: Any, *, effect_key: str) -> Any:
        operation = dict(plan.operation)
        expected_id = _escrow_id(
            self.handoff.chain_id,
            plan.owner,
            plan.owner_sequence,
        )
        if (
            plan.owner != self.handoff.coordinator_address
            or operation.get("owner") != plan.owner
            or operation.get("recipient") != plan.recipient
            or operation.get("asset_id") != self.handoff.asset_id
            or plan.expected_escrow_id != expected_id
        ):
            raise PftlBackendError("create plan does not match pinned handoff")
        return self._execute(
            effect_key=effect_key,
            kind="PFTL_ESCROW_CREATE",
            operation=operation,
            signer_sequence=plan.owner_sequence,
            escrow_id=expected_id,
        )

    def _open_escrow(self, escrow_id: str) -> Mapping[str, Any]:
        rows: list[str] = []
        values: list[Mapping[str, Any]] = []
        node_ids: set[str] = set()
        try:
            for client in self._clients:
                status_before = client.status()
                report = client.escrow_info(escrow_id)
                status_after = client.status()
                identity = self._status_sandwich(
                    status_before,
                    status_after,
                    read_label="open-escrow",
                )
                if identity["node_id"] in node_ids:
                    raise PftlBackendError(
                        "escrow views are not from distinct validators"
                    )
                node_ids.add(identity["node_id"])
                escrow = report.get("escrow") if isinstance(report, Mapping) else None
                if (
                    not isinstance(escrow, Mapping)
                    or report.get("found") is not True
                    or escrow.get("escrow_id") != escrow_id
                ):
                    raise PftlBackendError("pinned escrow does not exist")
                values.append(escrow)
                rows.append(
                    _canonical(
                        {
                            "anchor": self._status_anchor(identity),
                            "escrow": dict(escrow),
                        }
                    )
                )
        except Exception as error:
            if isinstance(error, PftlBackendError):
                raise
            raise PftlBackendError("six-validator escrow read failed") from error
        if (
            node_ids != {f"validator-{index}" for index in range(6)}
            or len(rows) != 6
            or len(set(rows)) != 1
        ):
            raise PftlBackendError("escrow state is not converged six-of-six")
        if values[0].get("state") != "open":
            raise PftlBackendError("escrow is not open")
        return values[0]

    def submit_finish(
        self,
        *,
        owner: str,
        recipient: str,
        escrow_id: str,
        secret: SecretPreimage,
        effect_key: str,
    ) -> Any:
        if recipient != self.handoff.coordinator_address:
            raise PftlBackendError("isolated signer can finish only as coordinator")
        operation = {
            "operation": "escrow_finish",
            "escrow_id": escrow_id,
            "owner": owner,
            "recipient": recipient,
            "fulfillment": encode_fulfillment(secret),
        }
        # As with cancel, a committed finish makes the consensus escrow
        # non-open. Reconcile a previously signed/submitted immutable effect
        # before applying the first-attempt open-state guard.
        _, store, _ = self._require_execution()
        durable = store.get(effect_key)
        if durable is not None and durable["status"] in {
            "SIGNED",
            "SUBMITTING",
            "SUCCEEDED",
            "REJECTED",
        }:
            return self._execute(
                effect_key=effect_key,
                kind="PFTL_ESCROW_FINISH",
                operation=operation,
                signer_sequence=None,
                escrow_id=escrow_id,
            )
        escrow = self._open_escrow(escrow_id)
        if (
            escrow.get("owner") != owner
            or escrow.get("recipient") != recipient
            or escrow.get("asset_id") != self.handoff.asset_id
        ):
            raise PftlBackendError("finish request does not match finalized escrow")
        return self._execute(
            effect_key=effect_key,
            kind="PFTL_ESCROW_FINISH",
            operation=operation,
            signer_sequence=None,
            escrow_id=escrow_id,
        )

    def submit_cancel(
        self,
        *,
        owner: str,
        escrow_id: str,
        effect_key: str,
    ) -> Any:
        if owner != self.handoff.coordinator_address:
            raise PftlBackendError("isolated signer can cancel only coordinator escrow")
        operation = {
            "operation": "escrow_cancel",
            "escrow_id": escrow_id,
            "owner": owner,
        }
        # A certification can commit immediately before this process crashes.
        # In that case the consensus escrow is already ``canceled`` and cannot
        # satisfy an ``open`` precondition on retry.  Reconcile the immutable
        # durable request first; _execute still checks the exact request hash,
        # signed artifact, tx id, and six-validator receipt.
        _, store, _ = self._require_execution()
        durable = store.get(effect_key)
        if durable is not None and durable["status"] in {
            "SIGNED",
            "SUBMITTING",
            "SUCCEEDED",
            "REJECTED",
        }:
            return self._execute(
                effect_key=effect_key,
                kind="PFTL_ESCROW_CANCEL",
                operation=operation,
                signer_sequence=None,
                escrow_id=escrow_id,
            )
        escrow = self._open_escrow(escrow_id)
        if escrow.get("owner") != owner:
            raise PftlBackendError("cancel request does not match finalized escrow")
        cancel_after = escrow.get("cancel_after")
        if type(cancel_after) is not int or cancel_after < 1:
            raise PftlBackendError("escrow has no valid cancel-after height")
        height = self._dry_check()["live"]["height"]
        if height < cancel_after:
            raise PftlBackendError("escrow cancel is not yet eligible")
        return self._execute(
            effect_key=effect_key,
            kind="PFTL_ESCROW_CANCEL",
            operation=operation,
            signer_sequence=None,
            escrow_id=escrow_id,
        )
