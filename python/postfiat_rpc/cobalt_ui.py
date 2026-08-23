"""Read-only browser interface for the real Cobalt CLI and node surfaces.

Run from the repository root after initializing a node data directory and a
Cobalt shadow fleet::

    PYTHONPATH=python python3 -m postfiat_rpc.cobalt_ui \
      --node-data-dir /path/to/node \
      --shadow-root /path/to/shadow-fleet

The service exposes no mutation route. It runs the existing Cobalt CLI checks,
reads the node's bounded ``governance.json`` state, and asks each persisted
shadow node for its signed status through the existing Rust binary.
"""

from __future__ import annotations

import argparse
import json
import mimetypes
import re
import subprocess
import threading
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Callable, Sequence
from urllib.parse import parse_qs, urlparse

from postfiat_rpc import cobalt


UI_SCHEMA = "postfiat-cobalt-governance-ui-snapshot-v2"
MAX_GOVERNANCE_BYTES = 16 * 1024 * 1024
ASSET_DIR = Path(__file__).with_name("cobalt_ui_assets")
STATIC_ROUTES = {
    "/": "index.html",
    "/index.html": "index.html",
    "/styles.css": "styles.css",
    "/app.js": "app.js",
    "/fonts/RobotoCondensed-Variable.ttf": "fonts/RobotoCondensed-Variable.ttf",
    "/fonts/IBMPlexMono-Regular.ttf": "fonts/IBMPlexMono-Regular.ttf",
    "/fonts/IBMPlexMono-Bold.ttf": "fonts/IBMPlexMono-Bold.ttf",
}


def utc_timestamp() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def read_bounded_bytes(path: Path, limit: int = MAX_GOVERNANCE_BYTES) -> bytes:
    size = path.stat().st_size
    if size > limit:
        raise cobalt.CobaltCliError(f"{path.name} exceeds the {limit}-byte read limit")
    return path.read_bytes()


def decode_node_json(path: Path, raw: bytes) -> dict[str, Any]:
    framed = raw[:-1] if raw.endswith(b"\n") else raw
    payload = framed
    if b"\npftmac1:" in framed:
        payload, trailer = framed.rsplit(b"\n", 1)
        if re.fullmatch(rb"pftmac1:[0-9a-f]{96}", trailer) is None:
            raise cobalt.CobaltCliError(f"{path.name} has a malformed integrity trailer")
    try:
        value = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise cobalt.CobaltCliError(f"{path.name} does not contain valid node JSON") from error
    if not isinstance(value, dict):
        raise cobalt.CobaltCliError(f"{path.name} does not contain a JSON object")
    return value


def short_hash(value: Any) -> str:
    text = str(value or "unavailable")
    return text if len(text) <= 24 else f"{text[:14]}…{text[-8:]}"


def governance_snapshot(data_dir: Path, raw: bytes | None = None) -> dict[str, Any]:
    path = data_dir / "governance.json"
    state = decode_node_json(path, raw if raw is not None else read_bounded_bytes(path))
    transitions = state.get("cobalt_authority_transitions", [])
    registry_updates = state.get("validator_registry_updates", [])
    amendments = state.get("amendments", [])
    if not all(isinstance(rows, list) for rows in (transitions, registry_updates, amendments)):
        raise cobalt.CobaltCliError("governance collections are malformed")

    items: list[dict[str, Any]] = []
    for row in transitions[-8:]:
        if isinstance(row, dict):
            items.append(
                {
                    "type": "authority transition",
                    "id": short_hash(row.get("transition_id")),
                    "detail": f"{row.get('from_authority_mode', '?')} → "
                    f"{row.get('to_authority_mode', '?')} · sequence "
                    f"{row.get('amendment_sequence', '?')}",
                    "height": row.get("activation_height"),
                    "status": "ordered",
                }
            )
    for row in registry_updates[-8:]:
        if isinstance(row, dict):
            items.append(
                {
                    "type": "validator update",
                    "id": short_hash(row.get("update_id")),
                    "detail": f"{row.get('operation', 'update')} · "
                    f"{row.get('subject_node_id', 'unknown node')}",
                    "height": row.get("activation_height"),
                    "status": "recorded",
                }
            )
    for row in amendments[-8:]:
        if isinstance(row, dict):
            items.append(
                {
                    "type": "governance amendment",
                    "id": short_hash(row.get("amendment_id")),
                    "detail": f"{row.get('kind', 'unknown')} = {row.get('value', '?')}",
                    "height": row.get("activation_height", 0),
                    "status": "recorded",
                }
            )

    authority_mode = state.get("authority_mode", 0)
    return {
        "source": str(path.resolve()),
        "authority_mode": authority_mode,
        "authority_label": "Cobalt validator-trust lane" if authority_mode == 1 else "Foundation registry",
        "active_validator_count": state.get("active_validator_count", 0),
        "transition_count": len(transitions),
        "registry_update_count": len(registry_updates),
        "amendment_count": len(amendments),
        "items": items[-12:],
        "latest_transition": transitions[-1] if transitions and isinstance(transitions[-1], dict) else None,
    }


@dataclass(frozen=True)
class CollectorOptions:
    root: Path
    node_data_dir: Path
    shadow_root: Path
    benchmark_packet: Path
    handoff_packet: Path
    benchmark_manifest_sha256: str
    handoff_manifest_sha256: str
    handoff_verifier_sha256: str
    cargo: str
    target: str | None
    timeout_seconds: float


ExampleRunner = Callable[[cobalt.ExampleSpec], dict[str, Any]]
ShadowRunner = Callable[[Path], dict[str, Any]]
NodeRunner = Callable[[], dict[str, Any]]


class SnapshotCollector:
    def __init__(
        self,
        options: CollectorOptions,
        *,
        example_runner: ExampleRunner | None = None,
        shadow_runner: ShadowRunner | None = None,
        node_runner: NodeRunner | None = None,
    ) -> None:
        self.options = options
        self.example_runner = example_runner or self._run_example
        self.shadow_runner = shadow_runner or self._run_shadow_status
        self.node_runner = node_runner or self._run_node_status

    def _run_example(self, spec: cobalt.ExampleSpec) -> dict[str, Any]:
        return cobalt.run_example(
            spec,
            root=self.options.root,
            cargo=self.options.cargo,
            target=self.options.target,
            timeout_seconds=self.options.timeout_seconds,
        )

    def _run_shadow_status(self, data_dir: Path) -> dict[str, Any]:
        return cobalt.run_shadow_service(
            "status",
            root=self.options.root,
            data_dir=data_dir,
            cargo=self.options.cargo,
            target=self.options.target,
            timeout_seconds=self.options.timeout_seconds,
        )

    def _run_node_status(self) -> dict[str, Any]:
        command = [
            self.options.cargo,
            "run",
            "--quiet",
            "--package",
            "postfiat-node",
            "--locked",
        ]
        if self.options.target:
            command.extend(["--target", self.options.target])
        command.extend(
            [
                "--bin",
                "postfiat-node",
                "--",
                "status",
                "--data-dir",
                str(self.options.node_data_dir),
            ]
        )
        try:
            completed = subprocess.run(
                command,
                cwd=self.options.root,
                capture_output=True,
                check=False,
                text=False,
                timeout=self.options.timeout_seconds,
            )
        except subprocess.TimeoutExpired as error:
            raise cobalt.CobaltCliError("node status exceeded the execution limit") from error
        except OSError as error:
            raise cobalt.CobaltCliError(f"could not execute node status: {error}") from error
        if (
            len(completed.stdout) > cobalt.MAX_REPORT_BYTES
            or len(completed.stderr) > cobalt.MAX_REPORT_BYTES
        ):
            raise cobalt.CobaltCliError("node status exceeded the bounded output limit")
        if completed.returncode != 0:
            stderr = completed.stderr.decode("utf-8", errors="replace").strip()
            detail = stderr.rsplit("\n", 1)[-1] if stderr else "no diagnostic emitted"
            raise cobalt.CobaltCliError(f"node status failed: {detail}")
        try:
            report = json.loads(completed.stdout)
        except json.JSONDecodeError as error:
            raise cobalt.CobaltCliError("node status did not emit valid JSON") from error
        if not isinstance(report, dict):
            raise cobalt.CobaltCliError("node status emitted a non-object report")
        return report

    def collect(self) -> dict[str, Any]:
        errors: list[str] = []
        trust: dict[str, Any] = {"ok": False, "report": {}, "source": {}}
        readiness: dict[str, Any] = {
            "ok": False,
            "status": "HOLD",
            "checks": [],
            "scenario": {},
            "packets": {},
            "decision_scope": (
                "separately authorized controlled-testnet validator-trust cutover"
            ),
            "recommendation": "HOLD_AND_REMEDIATE_FAILED_EVIDENCE",
        }
        governance: dict[str, Any] = {"items": [], "source": "unavailable"}
        shadow_nodes: list[dict[str, Any]] = []

        try:
            trust = cobalt.result_envelope(
                cobalt.EXAMPLES["trust-graph"],
                self.example_runner(cobalt.EXAMPLES["trust-graph"]),
            )
        except (OSError, ValueError, cobalt.CobaltCliError) as error:
            errors.append(f"trust graph: {error}")
        try:
            readiness = cobalt.readiness_result(
                self.options.benchmark_packet,
                self.options.handoff_packet,
                benchmark_manifest_sha256=self.options.benchmark_manifest_sha256,
                handoff_manifest_sha256=self.options.handoff_manifest_sha256,
                handoff_verifier_sha256=self.options.handoff_verifier_sha256,
            )
        except (OSError, ValueError, cobalt.CobaltCliError) as error:
            errors.append(f"readiness evidence: {error}")
        try:
            governance_path = self.options.node_data_dir / "governance.json"
            before = read_bounded_bytes(governance_path)
            node_status = self.node_runner()
            after = read_bounded_bytes(governance_path)
            if before != after:
                raise cobalt.CobaltCliError(
                    "governance state changed during the validated read; refresh again"
                )
            governance = governance_snapshot(self.options.node_data_dir, after)
            governance["node_status"] = {
                "chain_id": node_status.get("chain_id"),
                "block_height": node_status.get("block_height"),
                "state_root": node_status.get("state_root"),
                "node_id": node_status.get("node_id"),
            }
        except (OSError, ValueError, json.JSONDecodeError, cobalt.CobaltCliError) as error:
            errors.append(f"node governance: {error}")

        if self.options.shadow_root.is_dir():
            shadow_dirs = sorted(
                path
                for path in self.options.shadow_root.iterdir()
                if path.is_dir() and (path / "state.json").is_file()
            )
            for path in shadow_dirs:
                try:
                    status = self.shadow_runner(path)
                    status["source"] = str(path.resolve())
                    shadow_nodes.append(status)
                except (OSError, ValueError, cobalt.CobaltCliError) as error:
                    errors.append(f"shadow node {path.name}: {error}")
        else:
            errors.append(f"shadow root does not exist: {self.options.shadow_root}")

        digests = {node.get("governance_digest") for node in shadow_nodes}
        shadow_converged = bool(shadow_nodes) and len(digests) == 1
        shadow_healthy = shadow_converged and all(
            node.get("transport_healthy") is True
            and node.get("catch_up_status") == "current"
            and node.get("contiguous_sequence") == node.get("protocol_decision_count")
            and node.get("live_authority") is False
            and node.get("controls_block_consensus") is False
            for node in shadow_nodes
        )
        trust_report = trust.get("report", {}) if isinstance(trust.get("report"), dict) else {}
        authority_mode = governance.get("authority_mode")
        authority_known = authority_mode in {0, 1}
        cobalt_authority_active = authority_mode == 1
        actual_authority = {
            "known": authority_known,
            "mode": (
                "cobalt-validator-trust"
                if cobalt_authority_active
                else "foundation-validator-trust"
                if authority_mode == 0
                else "unavailable"
            ),
            "label": (
                "Cobalt validator-trust lane"
                if cobalt_authority_active
                else "Foundation registry"
                if authority_mode == 0
                else "Unavailable"
            ),
            "foundation_active": authority_mode == 0,
            "cobalt_active": cobalt_authority_active,
            "block_finality": "consensus-v2",
            "controls_block_consensus": False,
            "transition_count": governance.get("transition_count", 0),
            "source": governance.get("source", "unavailable"),
        }
        return {
            "schema": UI_SCHEMA,
            "collected_at": utc_timestamp(),
            "read_only": True,
            "authority_notice": "No governance actions are exposed. Consensus v2 finality is unchanged.",
            "trust": {
                "ok": trust.get("ok") is True,
                "mode": trust_report.get("cobalt_mode", "unavailable"),
                "active_graph": trust_report.get("active_graph", "unavailable"),
                "view_count": trust_report.get("g1_trust_view_count", 0),
                "activation_height": trust_report.get("g1_activation_height"),
                "root": trust_report.get("trust_graph_root", "unavailable"),
                "non_identical_views": trust_report.get("g1_non_identical_trust_views", False),
                "source": "PYTHONPATH=python python3 -m postfiat_rpc.cobalt trust-graph --json",
            },
            "proposals": governance,
            "shadow_health": {
                "ok": shadow_healthy,
                "converged": shadow_converged,
                "node_count": len(shadow_nodes),
                "digest": next(iter(digests), None) if len(digests) == 1 else None,
                "nodes": shadow_nodes,
                "source": str(self.options.shadow_root.resolve()),
            },
            "rehearsal_readiness": {
                "status": readiness.get("status", "HOLD"),
                "ready": readiness.get("ok") is True,
                "checks": readiness.get("checks", []),
                "scope": readiness.get("decision_scope"),
                "recommendation": readiness.get("recommendation"),
                "activation_performed": False,
                "packets": readiness.get("packets", {}),
            },
            "scenario": readiness.get("scenario", {}),
            "actual_authority": actual_authority,
            "errors": errors,
        }


class SnapshotCache:
    def __init__(self, collector: SnapshotCollector, ttl_seconds: float) -> None:
        self.collector = collector
        self.ttl_seconds = ttl_seconds
        self._lock = threading.Lock()
        self._snapshot: dict[str, Any] | None = None
        self._collected_monotonic = 0.0

    def get(self, *, force: bool = False) -> dict[str, Any]:
        with self._lock:
            expired = time.monotonic() - self._collected_monotonic >= self.ttl_seconds
            if force or self._snapshot is None or expired:
                self._snapshot = self.collector.collect()
                self._collected_monotonic = time.monotonic()
            return self._snapshot


class CobaltUiServer(ThreadingHTTPServer):
    cache: SnapshotCache


class CobaltUiHandler(BaseHTTPRequestHandler):
    server: CobaltUiServer

    def log_message(self, format_string: str, *args: object) -> None:
        print(f"cobalt-ui: {format_string % args}")

    def do_HEAD(self) -> None:  # noqa: N802
        self._serve(send_body=False)

    def do_GET(self) -> None:  # noqa: N802
        self._serve(send_body=True)

    def do_POST(self) -> None:  # noqa: N802
        self.send_error(HTTPStatus.METHOD_NOT_ALLOWED, "read-only interface")

    def _headers(self, content_type: str, length: int, *, cache: str) -> None:
        self.send_header("content-type", content_type)
        self.send_header("content-length", str(length))
        self.send_header("cache-control", cache)
        self.send_header("x-content-type-options", "nosniff")
        self.send_header("x-frame-options", "DENY")
        self.send_header("referrer-policy", "no-referrer")
        self.send_header(
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; "
            "connect-src 'self'; font-src 'self'; object-src 'none'; base-uri 'none'; "
            "frame-ancestors 'none'",
        )

    def _serve(self, *, send_body: bool) -> None:
        parsed = urlparse(self.path)
        if parsed.path == "/api/snapshot":
            force = parse_qs(parsed.query).get("refresh") == ["1"]
            payload = json.dumps(
                self.server.cache.get(force=force), separators=(",", ":")
            ).encode()
            self.send_response(HTTPStatus.OK)
            self._headers("application/json; charset=utf-8", len(payload), cache="no-store")
            self.end_headers()
            if send_body:
                self.wfile.write(payload)
            return

        asset_name = STATIC_ROUTES.get(parsed.path)
        if asset_name is None:
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        asset_path = ASSET_DIR / asset_name
        payload = asset_path.read_bytes()
        content_type = mimetypes.guess_type(asset_name)[0] or "application/octet-stream"
        self.send_response(HTTPStatus.OK)
        self._headers(f"{content_type}; charset=utf-8", len(payload), cache="no-cache")
        self.end_headers()
        if send_body:
            self.wfile.write(payload)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node-data-dir", required=True, type=Path)
    parser.add_argument("--shadow-root", required=True, type=Path)
    parser.add_argument(
        "--benchmark-packet",
        type=Path,
        default=Path("benchmarks/cobalt-rippled-liveness/packet"),
    )
    parser.add_argument(
        "--benchmark-sha256", default=cobalt.DEFAULT_BENCHMARK_PACKET_SHA256
    )
    parser.add_argument(
        "--handoff-packet",
        type=Path,
        default=Path("benchmarks/cobalt-handoff-rehearsal/packet"),
    )
    parser.add_argument("--handoff-sha256", default=cobalt.DEFAULT_HANDOFF_PACKET_SHA256)
    parser.add_argument(
        "--handoff-verifier-sha256",
        default=cobalt.DEFAULT_HANDOFF_VERIFIER_SHA256,
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--cargo")
    parser.add_argument("--target")
    parser.add_argument("--timeout", type=float, default=120.0)
    parser.add_argument("--refresh-seconds", type=float, default=15.0)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if not 0 <= args.port <= 65535:
        raise SystemExit("--port must be between 0 and 65535")
    if args.timeout <= 0 or args.refresh_seconds <= 0:
        raise SystemExit("--timeout and --refresh-seconds must be greater than zero")
    root = cobalt.repository_root()
    options = CollectorOptions(
        root=root,
        node_data_dir=args.node_data_dir,
        shadow_root=args.shadow_root,
        benchmark_packet=(
            args.benchmark_packet
            if args.benchmark_packet.is_absolute()
            else root / args.benchmark_packet
        ),
        handoff_packet=(
            args.handoff_packet
            if args.handoff_packet.is_absolute()
            else root / args.handoff_packet
        ),
        benchmark_manifest_sha256=args.benchmark_sha256,
        handoff_manifest_sha256=args.handoff_sha256,
        handoff_verifier_sha256=args.handoff_verifier_sha256,
        cargo=cobalt.resolve_cargo(args.cargo),
        target=args.target,
        timeout_seconds=args.timeout,
    )
    server = CobaltUiServer((args.host, args.port), CobaltUiHandler)
    server.cache = SnapshotCache(SnapshotCollector(options), args.refresh_seconds)
    server.cache.get(force=True)
    host, port = server.server_address[:2]
    print(f"Cobalt governance interface: http://{host}:{port}")
    print("Read-only: no governance mutation routes are registered")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
