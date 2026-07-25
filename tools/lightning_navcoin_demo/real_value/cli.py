"""Runnable operator CLI for the mainnet Lightning/NAVcoin coordinator."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import signal
import sys
import threading
from typing import Any, Mapping, Sequence

from .composition import (
    ARMED_PROCESS_ACK,
    DEFAULT_HANDOFF_PATH,
    DEFAULT_LND_PROTO_DIR,
    DEFAULT_STATE_ROOT,
    SecureStatePaths,
    compose_runtime,
    prepare_secure_state,
)
from .http_api import LightningNavcoinApi, serve_loopback
from .liquidity_budget import (
    mark_liquidity_setup_spent,
    reserve_liquidity_setup,
)
from .operator_control import OperatorControlServer, send_authorization
from .policy import ExecutionMode
from .process_lock import CoordinatorProcessLock


class _RecoveryWorker:
    """Bounded recovery scan for already-durable swaps."""

    def __init__(self, runtime: Any, *, interval_seconds: int = 5) -> None:
        if (
            type(interval_seconds) is not int
            or interval_seconds < 1
            or interval_seconds > 60
        ):
            raise ValueError("recovery interval must be within [1, 60]")
        self.runtime = runtime
        self.interval_seconds = interval_seconds
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None

    def _run(self) -> None:
        while not self._stop.is_set():
            try:
                recovery_ids = getattr(
                    self.runtime, "recovery_swap_ids", None
                )
                if callable(recovery_ids):
                    swap_ids = recovery_ids(limit=256)
                else:
                    swap_ids = tuple(
                        action.swap_id
                        for action in self.runtime.service.recovery_plan()[:256]
                    )
            except Exception as error:
                _stderr_event(
                    "recovery_scan_failed",
                    error_class=type(error).__name__,
                )
                swap_ids = ()
            for swap_id in swap_ids:
                if self._stop.is_set():
                    break
                if type(swap_id) is not str:
                    _stderr_event("recovery_row_malformed")
                    continue
                try:
                    self.runtime.recover_swap(swap_id)
                except Exception as error:
                    # Never log an invoice, authorization, preimage, or signer
                    # path.  The operator can inspect the public swap by id.
                    _stderr_event(
                        "swap_recovery_hold",
                        swap_id=swap_id,
                        error_class=type(error).__name__,
                    )
            self._stop.wait(self.interval_seconds)

    def start(self) -> None:
        if self._thread is not None:
            return
        self._thread = threading.Thread(
            target=self._run,
            name="lightning-navcoin-recovery",
            daemon=False,
        )
        self._thread.start()

    def close(self) -> None:
        self._stop.set()
        if self._thread is not None:
            # A recovery call may be inside a bounded LND/PFTL RPC.  Never
            # close journals or channels underneath a value reconciliation.
            self._thread.join()
            if self._thread.is_alive():  # defensive: join() has no timeout
                raise RuntimeError("recovery worker did not terminate")
            self._thread = None


def _json_line(value: Mapping[str, Any], stream: Any = sys.stdout) -> None:
    print(
        json.dumps(
            dict(value),
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ),
        file=stream,
        flush=True,
    )


def _stderr_event(event: str, **fields: Any) -> None:
    _json_line(
        {
            "schema": "postfiat.lightning_coordinator_event.v1",
            "event": event,
            **fields,
        },
        stream=sys.stderr,
    )


def _add_state_argument(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--state-dir",
        type=Path,
        default=DEFAULT_STATE_ROOT,
        help="absolute owner-only coordinator/LND state root",
    )


def _add_composition_arguments(parser: argparse.ArgumentParser) -> None:
    _add_state_argument(parser)
    parser.add_argument("--policy", type=Path)
    parser.add_argument("--price", type=Path)
    parser.add_argument("--lnd-connection", type=Path)
    parser.add_argument(
        "--handoff",
        type=Path,
        default=DEFAULT_HANDOFF_PATH,
    )
    parser.add_argument(
        "--lnd-proto-dir",
        type=Path,
        default=DEFAULT_LND_PROTO_DIR,
    )
    parser.add_argument("--fee-bps", type=int, default=0)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="lightning-navcoin-mainnet-coordinator",
        description=(
            "Fail-closed mainnet Lightning/NAVcoin coordinator. "
            "No command creates an LND wallet or signs an operator permit."
        ),
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    prepare = subparsers.add_parser(
        "prepare",
        help="create owner-only local coordinator state and secrets",
    )
    _add_state_argument(prepare)

    dry_check = subparsers.add_parser(
        "dry-check",
        help="read pinned LND/PFTL state without a signer or value mutation",
    )
    _add_composition_arguments(dry_check)

    serve = subparsers.add_parser(
        "serve",
        help="serve the loopback wallet API; mode is fixed by policy.json",
    )
    _add_composition_arguments(serve)
    serve.add_argument("--host", default="127.0.0.1")
    serve.add_argument("--port", type=int, default=18831)
    serve.add_argument(
        "--allowed-origin",
        default="http://127.0.0.1:5173",
    )
    serve.add_argument(
        "--armed-ack",
        help=f"ARMED only; exact value is {ARMED_PROCESS_ACK}",
    )
    serve.add_argument(
        "--pftl-signer-handle",
        type=Path,
        help="ARMED only; opaque owner-only signer file locator",
    )
    serve.add_argument(
        "--recovery-interval-seconds",
        type=int,
        default=5,
    )

    authorize = subparsers.add_parser(
        "authorize",
        help="send one externally signed nazgul permit over the owner-only socket",
    )
    _add_state_argument(authorize)
    authorize.add_argument("--swap-id", required=True)
    authorize.add_argument(
        "--authorization",
        type=Path,
        required=True,
        help="public Ed25519 authorization envelope; never a private key",
    )
    authorize.add_argument("--timeout-seconds", type=float, default=10.0)

    liquidity_reserve = subparsers.add_parser(
        "liquidity-reserve",
        help=(
            "reserve a signed LIQUIDITY_SETUP ceiling before a manual order; "
            "does not call an LSP or move value"
        ),
    )
    _add_state_argument(liquidity_reserve)
    liquidity_reserve.add_argument("--policy", type=Path)
    liquidity_reserve.add_argument("--price", type=Path)
    liquidity_reserve.add_argument(
        "--authorization",
        type=Path,
        required=True,
        help="owner-only public nazgul-signed LIQUIDITY_SETUP envelope",
    )

    liquidity_spent = subparsers.add_parser(
        "liquidity-mark-spent",
        help=(
            "conservatively charge a reserved LIQUIDITY_SETUP ceiling from "
            "strict public terminal evidence"
        ),
    )
    _add_state_argument(liquidity_spent)
    liquidity_spent.add_argument("--policy", type=Path)
    liquidity_spent.add_argument(
        "--evidence",
        type=Path,
        required=True,
        help=(
            "owner-only terminal evidence for a succeeded external payment "
            "and active confirmed channel"
        ),
    )
    return parser


def _composition_kwargs(args: argparse.Namespace) -> dict[str, Any]:
    paths = SecureStatePaths.under(args.state_dir)
    return {
        "paths": paths,
        "policy_path": args.policy,
        "price_path": args.price,
        "lnd_connection_path": args.lnd_connection,
        "handoff_path": args.handoff,
        "lnd_proto_dir": args.lnd_proto_dir,
        "fee_bps": args.fee_bps,
    }


def _dry_check(args: argparse.Namespace) -> int:
    kwargs = _composition_kwargs(args)
    with compose_runtime(**kwargs) as composition:
        if composition.policy.mode is not ExecutionMode.DRY_RUN:
            raise ValueError("dry-check requires a DRY_RUN policy")
        status = dict(composition.runtime.public_status())
        _json_line(
            {
                "schema": "postfiat.lightning_mainnet_composed_dry_check.v1",
                "status": status.get("status", "HOLD"),
                "value_moved": False,
                "signer_loaded": False,
                "policy_mode": composition.policy.mode.value,
                "runtime": status,
            }
        )
    return 0


def _serve(args: argparse.Namespace) -> int:
    paths = SecureStatePaths.under(args.state_dir)
    process_lock = CoordinatorProcessLock(paths.process_lock)
    process_lock.acquire()
    composition = None
    control: OperatorControlServer | None = None
    recovery: _RecoveryWorker | None = None
    httpd = None
    try:
        kwargs = _composition_kwargs(args)
        kwargs.update(
            {
                "armed_ack": args.armed_ack,
                "signer_key_file": args.pftl_signer_handle,
            }
        )
        composition = compose_runtime(**kwargs)
        api = LightningNavcoinApi(
            composition.runtime,
            session_token=composition.api_session_token,
            allowed_origin=args.allowed_origin,
        )
        httpd = serve_loopback(api, host=args.host, port=args.port)
        # server_close() must join every request that may be inside a durable
        # state transition before composition resources are closed.
        httpd.daemon_threads = False
        httpd.block_on_close = True
        httpd.timeout = 0.5

        if composition.policy.mode is ExecutionMode.ARMED:
            control = OperatorControlServer(
                paths.operator_socket,
                composition.runtime,
            )
            control.start()
            recovery = _RecoveryWorker(
                composition.runtime,
                interval_seconds=args.recovery_interval_seconds,
            )
            recovery.start()

        stop = threading.Event()

        def stop_handler(_signum: int, _frame: Any) -> None:
            stop.set()

        previous_handlers = {
            signum: signal.signal(signum, stop_handler)
            for signum in (signal.SIGINT, signal.SIGTERM)
        }
        try:
            _stderr_event(
                "coordinator_listening",
                host=args.host,
                port=args.port,
                policy_mode=composition.policy.mode.value,
                operator_control=(
                    str(paths.operator_socket)
                    if composition.policy.mode is ExecutionMode.ARMED
                    else None
                ),
            )
            while not stop.is_set():
                httpd.handle_request()
        finally:
            for signum, handler in previous_handlers.items():
                signal.signal(signum, handler)
        return 0
    finally:
        # Quiesce the local authorization ingress first, then wait for every
        # already-accepted HTTP request, then stop recovery.  This prevents a
        # new permit from entering while shutdown is draining value work.
        if control is not None:
            control.close()
        if httpd is not None:
            httpd.server_close()
        if recovery is not None:
            recovery.close()
        if composition is not None:
            composition.close()
        process_lock.close()


def _authorize(args: argparse.Namespace) -> int:
    paths = SecureStatePaths.under(args.state_dir)
    result = send_authorization(
        socket_path=paths.operator_socket,
        swap_id=args.swap_id,
        authorization_path=args.authorization,
        timeout_seconds=args.timeout_seconds,
    )
    _json_line(
        {
            "schema": "postfiat.lightning_operator_authorize_cli.v1",
            "ok": True,
            "result": dict(result),
        }
    )
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        if args.command == "prepare":
            _json_line(prepare_secure_state(args.state_dir))
            return 0
        if args.command == "dry-check":
            return _dry_check(args)
        if args.command == "serve":
            return _serve(args)
        if args.command == "authorize":
            return _authorize(args)
        if args.command == "liquidity-reserve":
            _json_line(
                reserve_liquidity_setup(
                    state_dir=args.state_dir,
                    policy_path=args.policy,
                    price_path=args.price,
                    authorization_path=args.authorization,
                )
            )
            return 0
        if args.command == "liquidity-mark-spent":
            _json_line(
                mark_liquidity_setup_spent(
                    state_dir=args.state_dir,
                    policy_path=args.policy,
                    evidence_path=args.evidence,
                )
            )
            return 0
        parser.error("unknown command")
    except Exception as error:
        message = str(error)
        if (
            not message
            or len(message) > 1024
            or not message.isascii()
            or any(ord(character) < 0x20 for character in message)
        ):
            message = "coordinator command failed closed"
        _json_line(
            {
                "schema": "postfiat.lightning_coordinator_cli_error.v1",
                "ok": False,
                "error": {
                    "class": type(error).__name__,
                    "message": message,
                },
                "outcome": "HOLD_CHECK_DURABLE_JOURNALS",
            },
            stream=sys.stderr,
        )
        return 1
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
