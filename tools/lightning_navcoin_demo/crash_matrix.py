"""Real process-crash recovery matrix for the coordinator SQLite journal."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Any, Mapping, Sequence

from .coordinator.journal import CoordinatorJournal, ExposureLimits, SwapState
from .coordinator.protocol import SecretPreimage
from .coordinator.service import CoordinatorService
from .lightning import DirectLncliGrpc


CRASH_EXIT = 86


def _write_private_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n")
    os.chmod(path, 0o600)


def _worker(
    *,
    database: Path,
    envelope_path: Path,
    principal: str,
    action: str,
    event_key: str,
    per_principal_atoms: int,
    aggregate_atoms: int,
    request_path: Path | None,
) -> None:
    envelope = json.loads(envelope_path.read_text())
    request = (
        json.loads(request_path.read_text())
        if request_path is not None
        else {}
    )
    journal = CoordinatorJournal(
        database,
        ExposureLimits(per_principal_atoms, aggregate_atoms),
    )
    service = CoordinatorService(journal)
    swap_id = envelope["quote"]["swap_id"]
    if action == "ADMIT":
        secret_hex = request.get("coordinator_secret_hex")
        service.admit_quote(
            principal,
            envelope,
            coordinator_secret=(
                SecretPreimage.from_hex(secret_hex)
                if secret_hex is not None
                else None
            ),
        )
    elif action == SwapState.PFTL_LOCK_SUBMITTED.value:
        service.mark_lock_submitted(
            swap_id,
            effect_key=request["effect_key"],
            operation=request["operation"],
        )
    elif action == SwapState.PFTL_LOCK_FINAL.value:
        service.mark_lock_final(
            swap_id,
            finality_evidence=request["evidence"],
        )
    elif action == SwapState.LN_IN_FLIGHT.value:
        service.mark_ln_in_flight(
            swap_id,
            payment_evidence=request["evidence"],
            effect_key=request.get("effect_key"),
            payment_request=request.get("payment_request"),
        )
    elif action == SwapState.LN_SETTLED.value:
        learned_secret = request.get("learned_secret_hex")
        service.mark_ln_settled(
            swap_id,
            settlement_evidence=request["evidence"],
            learned_secret=(
                SecretPreimage.from_hex(learned_secret)
                if learned_secret is not None
                else None
            ),
            effect_key=request.get("effect_key"),
            finish_operation=request.get("finish_operation"),
        )
    elif action == SwapState.PFTL_FINISH_FINAL.value:
        service.mark_finish_final(
            swap_id,
            finality_evidence=request["evidence"],
        )
    elif action == SwapState.REFUND_ELIGIBLE.value:
        service.mark_refund_eligible(
            swap_id,
            reason_evidence=request["evidence"],
            effect_key=request["effect_key"],
            cancel_operation=request["cancel_operation"],
        )
    elif action == SwapState.PFTL_CANCEL_FINAL.value:
        service.mark_cancel_final(
            swap_id,
            finality_evidence=request["evidence"],
        )
    else:
        raise ValueError(f"unsupported crash worker action: {action}")
    # Deliberately bypass context-manager cleanup, connection close, and WAL
    # checkpoint. The committed transition must survive an abrupt process exit.
    os._exit(CRASH_EXIT)


def _crash_once(
    *,
    database: Path,
    envelope_path: Path,
    principal: str,
    action: str,
    event_key: str,
    limits: ExposureLimits,
    request_path: Path | None = None,
) -> dict[str, Any]:
    command = [
        sys.executable,
        "-m",
        "tools.lightning_navcoin_demo.crash_matrix",
        "worker",
        "--database",
        str(database),
        "--envelope",
        str(envelope_path),
        "--principal",
        principal,
        "--action",
        action,
        "--event-key",
        event_key,
        "--per-principal-atoms",
        str(limits.per_principal_atoms),
        "--aggregate-atoms",
        str(limits.aggregate_atoms),
    ]
    if request_path is not None:
        command.extend(["--request", str(request_path)])
    completed = subprocess.run(
        command,
        cwd=Path(__file__).resolve().parents[2],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        timeout=30,
    )
    if completed.returncode != CRASH_EXIT:
        raise RuntimeError(
            f"crash worker failed for {action}: rc={completed.returncode}; "
            f"stderr={completed.stderr.strip()}"
        )
    return {
        "action": action,
        "event_key": event_key,
        "process_exit": completed.returncode,
        "unclean_exit": True,
    }


def crash_service_transition(
    *,
    root: Path,
    database: Path,
    envelope_path: Path,
    principal: str,
    action: str,
    request: Mapping[str, Any],
    limits: ExposureLimits,
    ordinal: int,
) -> dict[str, Any]:
    """Commit one real service transition, then terminate the worker uncleanly."""

    request_path = root / f"{ordinal:02d}-{action}.request.json"
    _write_private_json(request_path, dict(request))
    return _crash_once(
        database=database,
        envelope_path=envelope_path,
        principal=principal,
        action=action,
        event_key=f"live:{ordinal}:{action}",
        limits=limits,
        request_path=request_path,
    )


def _lightning_payment_worker(
    *,
    env_script: Path,
    request_path: Path,
) -> None:
    request = json.loads(request_path.read_text())
    payment = DirectLncliGrpc(env_script).pay_invoice(
        str(request["node"]),
        str(request["payment_request"]),
        fee_limit_sat=int(request["fee_limit_sat"]),
        max_total_cltv_delta=int(request["max_total_cltv_delta"]),
        timeout_seconds=int(request["timeout_seconds"]),
    )
    if (
        payment.status != "SUCCEEDED"
        or payment.payment_preimage is None
        or payment.payment_hash != request["payment_hash"]
    ):
        raise RuntimeError("outgoing Lightning crash worker did not settle")
    # Deliberately discard the result and die before touching the SQLite
    # journal. Recovery must query the payer's LND by durable payment hash.
    os._exit(CRASH_EXIT)


def crash_after_outgoing_lightning_payment(
    *,
    root: Path,
    env_script: Path,
    node: str,
    payment_request: str,
    payment_hash: str,
    fee_limit_sat: int,
    max_total_cltv_delta: int,
    timeout_seconds: int,
) -> dict[str, Any]:
    request_path = root / "outgoing-lightning-payment.request.json"
    _write_private_json(
        request_path,
        {
            "node": node,
            "payment_request": payment_request,
            "payment_hash": payment_hash,
            "fee_limit_sat": fee_limit_sat,
            "max_total_cltv_delta": max_total_cltv_delta,
            "timeout_seconds": timeout_seconds,
        },
    )
    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "tools.lightning_navcoin_demo.crash_matrix",
            "lightning-payment-worker",
            "--env-script",
            str(env_script),
            "--request",
            str(request_path),
        ],
        cwd=Path(__file__).resolve().parents[2],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=float(timeout_seconds) + 60,
    )
    if completed.returncode != CRASH_EXIT:
        raise RuntimeError(
            "outgoing Lightning crash worker failed; "
            f"rc={completed.returncode}; "
            f"stderr_bytes={len(completed.stderr)}; "
            f"stderr_sha256={hashlib.sha256(completed.stderr).hexdigest()}"
        )
    return {
        "process_exit": completed.returncode,
        "unclean_exit": True,
        "journal_updated_after_payment": False,
        "recovery_key": "payment_hash",
        "payment_hash": payment_hash,
    }


def _run_path(
    *,
    root: Path,
    name: str,
    signed_quote: Mapping[str, Any],
    states: Sequence[SwapState],
    limits: ExposureLimits,
) -> dict[str, Any]:
    path_root = root / name
    path_root.mkdir(parents=True, exist_ok=False)
    os.chmod(path_root, 0o700)
    database = path_root / "coordinator.sqlite3"
    envelope_path = path_root / "signed-quote.json"
    _write_private_json(envelope_path, signed_quote)
    swap_id = str(signed_quote["quote"]["swap_id"])
    principal = f"crash-probe-{name}"

    steps = [
        _crash_once(
            database=database,
            envelope_path=envelope_path,
            principal=principal,
            action="ADMIT",
            event_key=f"{name}:admit",
            limits=limits,
            request_path=None,
        )
    ]
    with CoordinatorJournal(database, limits) as recovered:
        current = recovered.get_swap(swap_id)
        if current["state"] != SwapState.QUOTED.value:
            raise RuntimeError("quote admission did not survive unclean process exit")
        steps[-1]["recovered_state"] = current["state"]

    for ordinal, state in enumerate(states, start=1):
        event_key = f"{name}:transition:{ordinal}:{state.value}"
        quote = signed_quote["quote"]
        request: dict[str, Any]
        if state is SwapState.PFTL_LOCK_SUBMITTED:
            request = {
                "effect_key": f"{name}:pftl-lock",
                "operation": {
                    "escrow_id": quote["expected_escrow_id"],
                    "owner": quote["pftl_owner"],
                    "recipient": quote["pftl_recipient"],
                    "asset_id": quote["pftl_asset_id"],
                    "amount_atoms": quote["pftl_amount_atoms"],
                    "condition": quote["condition"],
                    "cancel_after": quote["cancel_after"],
                },
            }
        elif state is SwapState.PFTL_LOCK_FINAL:
            request = {"evidence": {"observed_final": True}}
        elif state is SwapState.LN_IN_FLIGHT:
            outgoing = name == "happy"
            request = {
                "evidence": {
                    "payment_hash": quote["payment_hash"],
                    "recovery_probe": True,
                },
                **(
                    {
                        "effect_key": f"{name}:lightning-payment",
                        "payment_request": {
                            "payment_hash": quote["payment_hash"],
                            "amount_msat": quote["invoice_amount_msat"],
                            "max_parts": 1,
                        },
                    }
                    if outgoing
                    else {}
                ),
            }
        elif state is SwapState.LN_SETTLED:
            request = {
                "evidence": {
                    "status": "SUCCEEDED",
                    "payment_hash": quote["payment_hash"],
                },
                "effect_key": f"{name}:pftl-finish",
                "finish_operation": {
                    "escrow_id": quote["expected_escrow_id"],
                    "payment_hash": quote["payment_hash"],
                },
            }
        elif state is SwapState.REFUND_ELIGIBLE:
            request = {
                "evidence": {"lightning_settled": False},
                "effect_key": f"{name}:pftl-cancel",
                "cancel_operation": {
                    "escrow_id": quote["expected_escrow_id"],
                    "owner": quote["pftl_owner"],
                },
            }
        else:
            request = {"evidence": {"observed_final": True}}
        request_path = path_root / f"{ordinal:02d}-{state.value}.request.json"
        _write_private_json(request_path, request)
        step = _crash_once(
            database=database,
            envelope_path=envelope_path,
            principal=principal,
            action=state.value,
            event_key=event_key,
            limits=limits,
            request_path=request_path,
        )
        with CoordinatorJournal(database, limits) as recovered:
            service = CoordinatorService(recovered)
            current = recovered.get_swap(swap_id)
            if current["state"] != state.value:
                raise RuntimeError(
                    f"{state.value} did not survive unclean process exit"
                )
            step["recovered_state"] = current["state"]
            step["event_count"] = len(recovered.events(swap_id))
            # CoordinatorJournal construction runs PRAGMA quick_check and
            # raises before this point on any corruption.
            step["sqlite_quick_check"] = "ok"
            plan = service.recovery_plan()
            step["recovery_action"] = (
                plan[0].action if len(plan) == 1 else "terminal"
            )
            pending = recovered.pending_side_effects()
            step["pending_effect_keys"] = [
                row["effect_key"] for row in pending
            ]
            for effect in pending:
                recovered.record_side_effect_attempt(
                    effect["effect_key"],
                    f"{effect['effect_key']}:crash-matrix-attempt",
                    "SUCCEEDED",
                    result={"reconciled": True},
                )
        steps.append(step)

    return {
        "name": name,
        "swap_id": swap_id,
        "terminal_state": states[-1].value,
        "steps": steps,
        "all_unclean_restarts_recovered": True,
    }


def run_crash_matrix(
    root: Path,
    *,
    happy_signed_quote: Mapping[str, Any],
    refund_signed_quote: Mapping[str, Any],
    limits: ExposureLimits,
) -> dict[str, Any]:
    root = root.resolve()
    if root.exists() and any(root.iterdir()):
        raise RuntimeError("crash-matrix root must be absent or empty")
    root.mkdir(parents=True, exist_ok=True)
    os.chmod(root, 0o700)
    happy = _run_path(
        root=root,
        name="happy",
        signed_quote=happy_signed_quote,
        states=(
            SwapState.PFTL_LOCK_SUBMITTED,
            SwapState.PFTL_LOCK_FINAL,
            SwapState.LN_IN_FLIGHT,
            SwapState.LN_SETTLED,
            SwapState.PFTL_FINISH_FINAL,
        ),
        limits=limits,
    )
    refund = _run_path(
        root=root,
        name="refund",
        signed_quote=refund_signed_quote,
        states=(
            SwapState.PFTL_LOCK_SUBMITTED,
            SwapState.PFTL_LOCK_FINAL,
            SwapState.LN_IN_FLIGHT,
            SwapState.REFUND_ELIGIBLE,
            SwapState.PFTL_CANCEL_FINAL,
        ),
        limits=limits,
    )
    return {
        "schema": "postfiat.lightning.coordinator_crash_matrix.v1",
        "happy": happy,
        "refund": refund,
        "transition_crash_count": len(happy["steps"]) + len(refund["steps"]),
        "result": "PASS",
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    worker = subparsers.add_parser("worker")
    worker.add_argument("--database", required=True, type=Path)
    worker.add_argument("--envelope", required=True, type=Path)
    worker.add_argument("--principal", required=True)
    worker.add_argument("--action", required=True)
    worker.add_argument("--event-key", required=True)
    worker.add_argument("--per-principal-atoms", required=True, type=int)
    worker.add_argument("--aggregate-atoms", required=True, type=int)
    worker.add_argument("--request", type=Path)
    lightning_worker = subparsers.add_parser("lightning-payment-worker")
    lightning_worker.add_argument("--env-script", required=True, type=Path)
    lightning_worker.add_argument("--request", required=True, type=Path)
    arguments = parser.parse_args(argv)
    if arguments.command == "worker":
        _worker(
            database=arguments.database,
            envelope_path=arguments.envelope,
            principal=arguments.principal,
            action=arguments.action,
            event_key=arguments.event_key,
            per_principal_atoms=arguments.per_principal_atoms,
            aggregate_atoms=arguments.aggregate_atoms,
            request_path=arguments.request,
        )
    elif arguments.command == "lightning-payment-worker":
        _lightning_payment_worker(
            env_script=arguments.env_script,
            request_path=arguments.request,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
