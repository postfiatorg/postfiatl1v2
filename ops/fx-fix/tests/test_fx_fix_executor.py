"""Unit and integration tests for the Scheduled London-Style FX Fix Executor."""

from __future__ import annotations

import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from fx_fix_executor import (  # noqa: E402
    BatchOrderIntent,
    ExecutorConfig,
    FX_FIX_PACKET_SCHEMA_V1,
    FX_FIX_PACKET_VERSION_V1,
    FxFixBatchRunner,
    FxFixPacketV1,
    LocalEd25519BatchSigner,
    PriceObservation,
    WindowScheduleConfig,
    WindowStateStore,
    build_fx_fix_pause_op,
    build_fx_fix_reservation_release_op,
    cli,
    compute_median_ratio,
    evaluate_cutoff,
    fx_fix_reservation_id,
    resolve_london_window,
)
from fx_fix_executor.schedule import DuplicateWindowError  # noqa: E402

OPERATOR = "pf1111111111111111111111111111111111111111"
BASE_ASSET = "aa" * 48
QUOTE_ASSET = "bb" * 48
GOV_POLICY = "22" * 48


def _sample_config(**overrides) -> ExecutorConfig:
    params = dict(
        chain_id="postfiat-wan-devnet-2",
        operator=OPERATOR,
        base_asset_id=BASE_ASSET,
        quote_asset_id=QUOTE_ASSET,
        base_precision=6,
        quote_precision=0,
        band_bps=15,
        fee_bps=10,
        minimum_base_atoms=20_000_000,
        capacity_base_atoms=100_000_000,
        capacity_quote_atoms=2_000,
        max_fills=8,
        source_label="london_1600_wm_fix",
        governance_policy_hash=GOV_POLICY,
        window_validity_blocks=900,
        nav_tracked=True,
        nav_fix_fee_cap_bps=25,
        min_venues=2,
        schedule=WindowScheduleConfig(
            timezone_name="Europe/London",
            fix_hour=16,
            fix_minute=0,
            cutoff_seconds_before_fix=60,
            max_lateness_seconds=120,
            weekdays_only=True,
            holiday_dates_iso=("2026-12-25",),
        ),
    )
    params.update(overrides)
    return ExecutorConfig(**params)


def _sample_observations(fix_ts: int) -> list[PriceObservation]:
    return [
        PriceObservation(
            venue="venue-a",
            pair="PFUSDC/PNOK",
            bid="10.49",
            ask="10.51",
            observed_at_unix=fix_ts - 15,
            sequence=101,
        ),
        PriceObservation(
            venue="venue-b",
            pair="PFUSDC/PNOK",
            bid="10.48",
            ask="10.52",
            observed_at_unix=fix_ts - 10,
            sequence=202,
        ),
        PriceObservation(
            venue="venue-c",
            pair="PFUSDC/PNOK",
            bid="10.495",
            ask="10.505",
            observed_at_unix=fix_ts - 5,
            sequence=303,
        ),
    ]


def test_packet_canonical_hash_and_validation_matches_rust_wire_rules() -> None:
    packet = FxFixPacketV1(
        version=FX_FIX_PACKET_VERSION_V1,
        schema=FX_FIX_PACKET_SCHEMA_V1,
        operator=OPERATOR,
        base_asset_id=BASE_ASSET,
        quote_asset_id=QUOTE_ASSET,
        epoch=1,
        ratio_numerator=21,
        ratio_denominator=2_000_000,
        band_bps=0,
        fee_bps=0,
        valid_from_height=0,
        expires_at_height=100,
        minimum_base_atoms=20_000_000,
        capacity_base_atoms=40_000_000,
        capacity_quote_atoms=420,
        max_fills=2,
        source_label="demo_fix",
        source_observation_commitment="11" * 48,
        governance_policy_hash="22" * 48,
        previous_fix_hash=None,
    ).with_computed_hash()
    packet.validate()
    assert len(packet.packet_hash) == 96
    quote_atoms, exact = packet.quote_atoms_for_base(20_000_000)
    assert quote_atoms == 210
    assert exact is True

    # Epoch 1 with previous_fix_hash rejected
    bad_epoch1 = FxFixPacketV1(**{**packet.to_dict(), "previous_fix_hash": "99" * 48}).with_computed_hash()
    with pytest.raises(ValueError, match="epoch 1 cannot name a previous fix"):
        bad_epoch1.validate()

    # Epoch 2 without previous_fix_hash rejected
    bad_epoch2 = FxFixPacketV1(**{**packet.to_dict(), "epoch": 2}).with_computed_hash()
    with pytest.raises(ValueError, match="requires previous_fix_hash"):
        bad_epoch2.validate()

    # Pillar 7 NAV fee cap enforcement (protocol 100 bps & schedule cap)
    high_fee = FxFixPacketV1(**{**packet.to_dict(), "fee_bps": 101}).with_computed_hash()
    with pytest.raises(ValueError, match="fx_fix_fee_exceeds_nav_cap"):
        high_fee.validate(nav_tracked=True)
    sched_cap_exceeded = FxFixPacketV1(**{**packet.to_dict(), "fee_bps": 30}).with_computed_hash()
    with pytest.raises(ValueError, match="fx_fix_fee_exceeds_nav_cap"):
        sched_cap_exceeded.validate(nav_tracked=True, nav_fix_fee_cap_bps=25)


def test_london_dst_and_cutoff_enforcement() -> None:
    schedule = WindowScheduleConfig(
        timezone_name="Europe/London",
        fix_hour=16,
        fix_minute=0,
        cutoff_seconds_before_fix=60,
        max_lateness_seconds=120,
        weekdays_only=True,
        holiday_dates_iso=("2026-12-25",),
    )

    # Winter (GMT = UTC+0): 2026-11-27 (Friday) 16:00 London == 16:00 UTC
    gmt_now = datetime(2026, 11, 27, 16, 0, 5, tzinfo=timezone.utc)
    gmt_status = evaluate_cutoff(gmt_now, schedule)
    assert gmt_status.allowed is True
    assert gmt_status.window.fix_time_utc_iso == "2026-11-27T16:00:00+00:00"

    # Summer (BST = UTC+1): 2026-07-15 (Wednesday) 16:00 London == 15:00 UTC
    bst_now = datetime(2026, 7, 15, 15, 0, 10, tzinfo=timezone.utc)
    bst_status = evaluate_cutoff(bst_now, schedule)
    assert bst_status.allowed is True
    assert bst_status.window.fix_time_utc_iso == "2026-07-15T15:00:00+00:00"

    # Before order cutoff (15:58:00 UTC on GMT day) rejected
    early = evaluate_cutoff(datetime(2026, 11, 27, 15, 58, 0, tzinfo=timezone.utc), schedule)
    assert early.allowed is False
    assert early.reason.startswith("before_order_cutoff")

    # Between cutoff and fix time rejected unless allow_before_fix=True
    between = evaluate_cutoff(datetime(2026, 11, 27, 15, 59, 30, tzinfo=timezone.utc), schedule)
    assert between.allowed is False
    assert between.reason.startswith("before_fix_time")
    assert evaluate_cutoff(
        datetime(2026, 11, 27, 15, 59, 30, tzinfo=timezone.utc),
        schedule,
        allow_before_fix=True,
    ).allowed is True

    # Past deadline (16:02:01 UTC) rejected
    late = evaluate_cutoff(datetime(2026, 11, 27, 16, 2, 1, tzinfo=timezone.utc), schedule)
    assert late.allowed is False
    assert late.reason.startswith("after_window_deadline")

    # Weekend (Saturday 2026-11-28) rejected
    weekend = evaluate_cutoff(datetime(2026, 11, 28, 16, 0, 0, tzinfo=timezone.utc), schedule)
    assert weekend.allowed is False
    assert weekend.reason.startswith("weekend_closed")

    # Holiday (Friday 2026-12-25) rejected
    holiday = evaluate_cutoff(datetime(2026, 12, 25, 16, 0, 0, tzinfo=timezone.utc), schedule)
    assert holiday.allowed is False
    assert holiday.reason.startswith("holiday_closed")


def test_dry_run_live_execution_duplicate_window_guard_and_chaining(tmp_path: Path) -> None:
    state_file = tmp_path / "window-state.json"
    store = WindowStateStore(state_file)
    cfg = _sample_config()
    seed = hashlib.sha256(b"fx-fix-unit-test-signer-v1").digest()
    signer = LocalEd25519BatchSigner(seed)
    runner = FxFixBatchRunner(cfg, store, signer=signer)

    now_day1 = datetime(2026, 11, 27, 16, 0, 15, tzinfo=timezone.utc)
    win1 = resolve_london_window(now_day1, cfg.schedule)
    obs1 = _sample_observations(win1.fix_timestamp_unix)
    order1 = BatchOrderIntent(
        action_binding_hash="33" * 64,
        base_atoms=20_000_000,
        wallet_intent_hash="34" * 48,
        reservation_nonce="44" * 48,
        submitted_at_unix=win1.cutoff_timestamp_unix - 5,
    )

    # 1. Dry-run builds, validates, signs, and does NOT mutate durable state.
    dry_result = runner.execute_window(
        now_utc=now_day1,
        observations=obs1,
        current_block_height=100_000,
        order_intents=[order1],
        dry_run=True,
    )
    assert dry_result["batch"]["dry_run"] is True
    assert dry_result["batch"]["epoch"] == 1
    assert dry_result["batch"]["summary"]["reservation_count"] == 1
    assert dry_result["batch"]["summary"]["total_quote_atoms"] == 210
    assert LocalEd25519BatchSigner.verify_batch(dry_result["batch"], dry_result["signature"])
    assert not state_file.exists()

    # 2. Live execution records durable state for Day 1.
    live_result1 = runner.execute_window(
        now_utc=now_day1,
        observations=obs1,
        current_block_height=100_000,
        order_intents=[order1],
        dry_run=False,
    )
    assert live_result1["batch"]["epoch"] == 1
    assert state_file.exists()
    # Reservation ID matches canonical helper.
    expected_res_id = fx_fix_reservation_id(
        fix_packet_hash=live_result1["batch"]["packet_hash"],
        operator=OPERATOR,
        action_binding_hash=order1.action_binding_hash,
        base_atoms=20_000_000,
        quote_atoms=210,
        wallet_intent_hash=order1.wallet_intent_hash,
        reservation_nonce=order1.reservation_nonce,
    )
    assert live_result1["reservation_ids"] == [expected_res_id]

    # 3. Duplicate-window guard blocks re-execution of the same window (both live and dry-run).
    with pytest.raises(DuplicateWindowError):
        runner.execute_window(
            now_utc=now_day1,
            observations=obs1,
            current_block_height=100_010,
            dry_run=False,
        )
    with pytest.raises(DuplicateWindowError):
        runner.execute_window(
            now_utc=now_day1,
            observations=obs1,
            current_block_height=100_010,
            dry_run=True,
        )

    # 4. Next business day (Monday 2026-11-30) chains epoch=2 and previous_fix_hash automatically.
    now_day2 = datetime(2026, 11, 30, 16, 0, 10, tzinfo=timezone.utc)
    win2 = resolve_london_window(now_day2, cfg.schedule)
    live_result2 = runner.execute_window(
        now_utc=now_day2,
        observations=_sample_observations(win2.fix_timestamp_unix),
        current_block_height=350_000,
        dry_run=False,
    )
    pkt2 = live_result2["batch"]["register_operation"]["packet"]
    assert pkt2["epoch"] == 2
    assert pkt2["previous_fix_hash"] == live_result1["batch"]["packet_hash"]


def test_order_after_cutoff_and_stale_or_dispersed_observations_rejected(tmp_path: Path) -> None:
    store = WindowStateStore(tmp_path / "state.json")
    cfg = _sample_config()
    runner = FxFixBatchRunner(cfg, store)

    now = datetime(2026, 11, 27, 16, 0, 5, tzinfo=timezone.utc)
    win = resolve_london_window(now, cfg.schedule)

    # Late order submitted 1 second after cutoff must fail closed.
    late_order = BatchOrderIntent(
        action_binding_hash="33" * 64,
        base_atoms=20_000_000,
        wallet_intent_hash="34" * 48,
        reservation_nonce="44" * 48,
        submitted_at_unix=win.cutoff_timestamp_unix + 1,
    )
    with pytest.raises(ValueError, match="order_after_cutoff"):
        runner.execute_window(
            now_utc=now,
            observations=_sample_observations(win.fix_timestamp_unix),
            current_block_height=10_000,
            order_intents=[late_order],
            dry_run=True,
        )

    # Stale venue observation rejected.
    stale_obs = [
        PriceObservation("venue-a", "PFUSDC/PNOK", "10.50", "10.50", win.fix_timestamp_unix - 500),
        PriceObservation("venue-b", "PFUSDC/PNOK", "10.50", "10.50", win.fix_timestamp_unix - 10),
    ]
    with pytest.raises(ValueError, match="stale"):
        compute_median_ratio(
            stale_obs,
            base_precision=6,
            quote_precision=0,
            fix_timestamp_unix=win.fix_timestamp_unix,
            max_observation_age_seconds=120,
        )

    # Cross-venue dispersion above threshold rejected.
    dispersed_obs = [
        PriceObservation("venue-a", "PFUSDC/PNOK", "10.00", "10.00", win.fix_timestamp_unix - 5),
        PriceObservation("venue-b", "PFUSDC/PNOK", "11.00", "11.00", win.fix_timestamp_unix - 5),
    ]
    with pytest.raises(ValueError, match="dispersion"):
        compute_median_ratio(
            dispersed_obs,
            base_precision=6,
            quote_precision=0,
            fix_timestamp_unix=win.fix_timestamp_unix,
            max_dispersion_bps=150,
        )


def test_pause_release_helpers_and_cli_dry_run(tmp_path: Path) -> None:
    pause_op = build_fx_fix_pause_op(OPERATOR, "aa" * 48, True)
    assert pause_op["operation"] == "fx_fix_pause_v1"
    release_op = build_fx_fix_reservation_release_op(OPERATOR, "bb" * 48)
    assert release_op["operation"] == "fx_fix_reservation_release_v1"

    cfg_path = Path(__file__).resolve().parents[1] / "config.example.json"
    obs_path = tmp_path / "obs.json"
    out_path = tmp_path / "out.json"
    state_path = tmp_path / "state.json"

    fix_dt = datetime(2026, 11, 27, 16, 0, 10, tzinfo=timezone.utc)
    fix_ts = int(datetime(2026, 11, 27, 16, 0, 0, tzinfo=timezone.utc).timestamp())
    obs_path.write_text(
        json.dumps(
            [
                {"venue": "v1", "pair": "BASE/QUOTE", "bid": "1.0500", "ask": "1.0502", "observed_at_unix": fix_ts - 10},
                {"venue": "v2", "pair": "BASE/QUOTE", "bid": "1.0499", "ask": "1.0503", "observed_at_unix": fix_ts - 8},
            ]
        ),
        encoding="utf-8",
    )

    rc = cli.main(
        [
            "run",
            "--config",
            str(cfg_path),
            "--state-file",
            str(state_path),
            "--observations",
            str(obs_path),
            "--block-height",
            "50000",
            "--now-utc",
            fix_dt.isoformat(),
            "--dry-run",
            "--output",
            str(out_path),
        ]
    )
    assert rc == 0
    report = json.loads(out_path.read_text(encoding="utf-8"))
    assert report["batch"]["dry_run"] is True
    assert len(report["batch"]["packet_hash"]) == 96
