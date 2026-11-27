#!/usr/bin/env python3
"""CLI entrypoint for the Scheduled London-Style FX Fix Executor."""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .packet import FxFixPacketV1, PriceObservation
from .runner import BatchOrderIntent, ExecutorConfig, FxFixBatchRunner, SubprocessSigner
from .schedule import WindowScheduleConfig, WindowStateStore, evaluate_cutoff


def _load_config(path: Path) -> ExecutorConfig:
    raw = json.loads(path.read_text(encoding="utf-8"))
    sched_raw = raw.get("schedule", {})
    schedule = WindowScheduleConfig(
        timezone_name=sched_raw.get("timezone_name", "Europe/London"),
        fix_hour=int(sched_raw.get("fix_hour", 16)),
        fix_minute=int(sched_raw.get("fix_minute", 0)),
        cutoff_seconds_before_fix=int(sched_raw.get("cutoff_seconds_before_fix", 60)),
        max_lateness_seconds=int(sched_raw.get("max_lateness_seconds", 120)),
        weekdays_only=bool(sched_raw.get("weekdays_only", True)),
        holiday_dates_iso=tuple(sched_raw.get("holiday_dates_iso", ())),
    )
    return ExecutorConfig(
        chain_id=str(raw["chain_id"]),
        operator=str(raw["operator"]),
        base_asset_id=str(raw["base_asset_id"]),
        quote_asset_id=str(raw["quote_asset_id"]),
        base_precision=int(raw["base_precision"]),
        quote_precision=int(raw["quote_precision"]),
        band_bps=int(raw["band_bps"]),
        fee_bps=int(raw["fee_bps"]),
        minimum_base_atoms=int(raw["minimum_base_atoms"]),
        capacity_base_atoms=int(raw["capacity_base_atoms"]),
        capacity_quote_atoms=int(raw["capacity_quote_atoms"]),
        max_fills=int(raw["max_fills"]),
        source_label=str(raw["source_label"]),
        governance_policy_hash=str(raw["governance_policy_hash"]),
        window_validity_blocks=int(raw.get("window_validity_blocks", 900)),
        nav_tracked=bool(raw.get("nav_tracked", True)),
        nav_fix_fee_cap_bps=raw.get("nav_fix_fee_cap_bps"),
        max_observation_age_seconds=int(raw.get("max_observation_age_seconds", 120)),
        max_venue_spread_bps=int(raw.get("max_venue_spread_bps", 200)),
        max_dispersion_bps=int(raw.get("max_dispersion_bps", 150)),
        min_venues=int(raw.get("min_venues", 1)),
        schedule=schedule,
        rpc_url=raw.get("rpc_url"),
    )


def _load_observations(path: Path) -> list[PriceObservation]:
    raw = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(raw, list):
        raise ValueError("observations file must contain a JSON array")
    return [
        PriceObservation(
            venue=str(item["venue"]),
            pair=str(item["pair"]),
            bid=str(item["bid"]),
            ask=str(item["ask"]),
            observed_at_unix=int(item["observed_at_unix"]),
            sequence=int(item.get("sequence", 0)),
        )
        for item in raw
    ]


def _load_orders(path: Path | None) -> list[BatchOrderIntent]:
    if path is None:
        return []
    raw = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(raw, list):
        raise ValueError("orders file must contain a JSON array")
    return [
        BatchOrderIntent(
            action_binding_hash=str(item["action_binding_hash"]),
            base_atoms=int(item["base_atoms"]),
            wallet_intent_hash=str(item["wallet_intent_hash"]),
            reservation_nonce=str(item["reservation_nonce"]),
            submitted_at_unix=int(item["submitted_at_unix"]),
            reservation_ttl_blocks=int(item.get("reservation_ttl_blocks", 300)),
        )
        for item in raw
    ]


def _parse_now(raw: str | None) -> datetime:
    if not raw:
        return datetime.now(timezone.utc)
    dt = datetime.fromisoformat(raw.replace("Z", "+00:00"))
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt.astimezone(timezone.utc)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Post Fiat Scheduled London-Style FX Fix Executor")
    sub = parser.add_subparsers(dest="command", required=True)

    plan_cmd = sub.add_parser("plan-window", help="Evaluate London window cutoff status")
    plan_cmd.add_argument("--config", type=Path, required=True)
    plan_cmd.add_argument("--now-utc", default=None)
    plan_cmd.add_argument("--target-date", default=None)

    run_cmd = sub.add_parser("run", help="Build, validate, sign, and submit one scheduled FX fix batch")
    run_cmd.add_argument("--config", type=Path, required=True)
    run_cmd.add_argument("--state-file", type=Path, required=True)
    run_cmd.add_argument("--observations", type=Path, required=True)
    run_cmd.add_argument("--orders", type=Path, default=None)
    run_cmd.add_argument("--block-height", type=int, default=None)
    run_cmd.add_argument("--now-utc", default=None)
    run_cmd.add_argument("--target-date", default=None)
    run_cmd.add_argument("--signer-cmd", nargs="+", default=None)
    run_cmd.add_argument("--dry-run", action="store_true")
    run_cmd.add_argument("--allow-before-fix", action="store_true")
    run_cmd.add_argument("--output", type=Path, default=None)

    val_cmd = sub.add_parser("validate-packet", help="Validate an FxFixPacketV1 JSON file")
    val_cmd.add_argument("--packet", type=Path, required=True)
    val_cmd.add_argument("--nav-tracked", action="store_true")
    val_cmd.add_argument("--nav-fee-cap-bps", type=int, default=None)

    args = parser.parse_args(argv)

    if args.command == "plan-window":
        cfg = _load_config(args.config)
        status = evaluate_cutoff(
            _parse_now(args.now_utc),
            cfg.schedule,
            target_local_date=args.target_date,
        )
        print(
            json.dumps(
                {
                    "allowed": status.allowed,
                    "reason": status.reason,
                    "seconds_from_fix": status.seconds_from_fix,
                    "window": status.window.__dict__,
                },
                indent=2,
                sort_keys=True,
            )
        )
        return 0 if status.allowed else 2

    if args.command == "validate-packet":
        raw = json.loads(args.packet.read_text(encoding="utf-8"))
        packet = FxFixPacketV1.from_dict(raw)
        packet.validate(nav_tracked=args.nav_tracked, nav_fix_fee_cap_bps=args.nav_fee_cap_bps)
        print(json.dumps({"valid": True, "packet_hash": packet.packet_hash}, indent=2))
        return 0

    if args.command == "run":
        cfg = _load_config(args.config)
        store = WindowStateStore(args.state_file)
        signer = SubprocessSigner(args.signer_cmd) if args.signer_cmd else None
        runner = FxFixBatchRunner(cfg, store, signer=signer)
        result = runner.execute_window(
            now_utc=_parse_now(args.now_utc),
            observations=_load_observations(args.observations),
            current_block_height=args.block_height,
            order_intents=_load_orders(args.orders),
            target_local_date=args.target_date,
            dry_run=args.dry_run,
            allow_before_fix=args.allow_before_fix,
        )
        encoded = json.dumps(result, indent=2, sort_keys=True)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(encoded + "\n", encoding="utf-8")
        else:
            print(encoded)
        return 0

    return 1


if __name__ == "__main__":
    raise SystemExit(main())
