from __future__ import annotations

import json
import hashlib
from dataclasses import replace
from datetime import date, datetime, timedelta, timezone
from pathlib import Path

import pytest

from pftl_schwab_tee import (
    CollectionEpoch,
    ContractQuote,
    ContractReferenceTerms,
    CurrentPosition,
    MethodologySnapshot,
    OptionChainQuery,
    PortfolioParameters,
    PortfolioTargetInput,
    SchwabOptionChainCollector,
    create_portfolio_target,
    methodology_snapshot_from_schwab,
    target_quantity_from_midpoint,
)


PARAMETERS_PATH = Path(__file__).parent / "fixtures" / "yolo_portfolio_parameters_v1.json"


def parameters() -> PortfolioParameters:
    return PortfolioParameters.from_dict(json.loads(PARAMETERS_PATH.read_text()))


def quote(
    symbol: str,
    strike_dollars: int,
    expiration: date,
    observed: datetime,
    *,
    bid: int = 4_900_000,
    ask: int = 5_100_000,
    open_interest: int = 2_500,
    volume: int = 500,
    size: int = 10,
    halted: bool = False,
) -> ContractQuote:
    return ContractQuote(
        occ_symbol=symbol,
        expiration_date=expiration,
        strike_microdollars=strike_dollars * 1_000_000,
        bid_microdollars=bid,
        ask_microdollars=ask,
        bid_size=size,
        ask_size=size,
        session_volume=volume,
        open_interest=open_interest,
        quote_timestamp_utc=observed,
        option_type="CALL",
        multiplier=100,
        deliverable_shares=100,
        exercise_style="AMERICAN",
        standard=True,
        option_halted=halted,
    )


def snapshots(
    *,
    trade_date: date,
    expirations: tuple[tuple[date, tuple[int, ...]], ...],
    underlier_halted: bool = False,
) -> tuple[MethodologySnapshot, ...]:
    start = datetime(2026, 9, 4, 14, 30, tzinfo=timezone.utc)
    output = []
    for sequence, offset in enumerate((0, 450, 900, 1_350, 1_800)):
        scheduled = start + timedelta(seconds=offset)
        observed = scheduled + timedelta(seconds=1)
        contracts = []
        for expiration, strikes in expirations:
            for strike in strikes:
                symbol = f"HOOD-{expiration.isoformat()}-{strike:03d}C"
                contracts.append(quote(symbol, strike, expiration, observed))
        output.append(
            MethodologySnapshot(
                sequence=sequence,
                scheduled_at_utc=scheduled,
                observed_at_utc=observed,
                underlier_midpoint_microdollars=50_000_000,
                underlier_halted=underlier_halted,
                contracts=tuple(contracts),
            )
        )
    return tuple(output)


def target_input(
    snapshots_value: tuple[MethodologySnapshot, ...],
    *,
    trade_date: date,
    incumbent_expiration: date | None = None,
    incumbent_symbols: tuple[str, ...] = (),
    positions: tuple[CurrentPosition, ...] = (),
    cash: int = 100_000_000_000,
) -> PortfolioTargetInput:
    return PortfolioTargetInput(
        series_id="YOLO-HOOD-v1",
        underlier_id="HOOD-COMMON",
        trade_date=trade_date,
        methodology_sha256="aa" * 32,
        collection_sha256="bb" * 32,
        snapshots=snapshots_value,
        positions=positions,
        settled_cash_microdollars=cash,
        unsettled_cash_microdollars=0,
        accrued_liabilities_microdollars=0,
        incumbent_expiration=incumbent_expiration,
        incumbent_symbols=incumbent_symbols,
    )


def test_reference_manifest_is_explicit_and_strict() -> None:
    manifest = json.loads(PARAMETERS_PATH.read_text())
    loaded = PortfolioParameters.from_dict(manifest)

    assert loaded.target_premium_ppb == 900_000_000
    assert loaded.basket_size == 5
    assert loaded.spread_weight_ppb == 500_000_000
    assert len(loaded.sha256) == 64
    with pytest.raises(ValueError, match="missing"):
        PortfolioParameters.from_dict({key: value for key, value in manifest.items() if key != "roll_dte"})
    with pytest.raises(ValueError, match="unknown"):
        PortfolioParameters.from_dict({**manifest, "hidden_default": 1})


def test_documented_midpoint_sizing_counterexample() -> None:
    target_quantity = target_quantity_from_midpoint(
        sleeve_microdollars=10_000_000_000,
        midpoint_microdollars=5_000_000,
        multiplier=100,
    )

    assert target_quantity == 20
    assert target_quantity - 19 == 1


def test_portfolio_input_round_trips_strict_json() -> None:
    trade_date = date(2026, 9, 4)
    expiry = trade_date + timedelta(days=60)
    original = target_input(
        snapshots(
            trade_date=trade_date,
            expirations=((expiry, (48, 49, 50, 51, 52)),),
        ),
        trade_date=trade_date,
    )

    assert PortfolioTargetInput.from_dict(original.to_dict()) == original
    with pytest.raises(ValueError, match="unknown"):
        PortfolioTargetInput.from_dict({**original.to_dict(), "orderSide": "BUY"})


def test_schwab_capture_normalizes_only_with_explicit_reference_and_halt_facts() -> None:
    scheduled = datetime(2026, 9, 4, 14, 30, tzinfo=timezone.utc)
    expiry = date(2026, 11, 3)
    symbol = "HOOD  261103C00050000"
    payload = {
        "symbol": "HOOD",
        "numberOfContracts": 1,
        "callExpDateMap": {
            "2026-11-03:60": {
                "50.0": [
                    {
                        "symbol": symbol,
                        "putCall": "CALL",
                        "expirationDate": "2026-11-03",
                        "strikePrice": 50,
                        "bid": 4.9,
                        "ask": 5.1,
                        "bidSize": 10,
                        "askSize": 12,
                        "totalVolume": 500,
                        "openInterest": 2500,
                        "quoteTimeInLong": int(scheduled.timestamp() * 1000),
                        "multiplier": 100,
                    }
                ]
            }
        },
        "putExpDateMap": {},
    }

    class Response:
        status_code = 200
        headers = {}
        content = json.dumps(payload).encode()

    class Client:
        def request(self, method, path, *, params=None, payload=None):
            return globals_payload, Response()

    globals_payload = payload
    query = OptionChainQuery(
        symbol="HOOD",
        contract_type="ALL",
        strategy="SINGLE",
        include_underlying_quote=True,
        from_date=date(2026, 9, 4),
        to_date=expiry,
        strike_count=None,
    )
    epoch = CollectionEpoch(
        epoch_id="adapter-test",
        methodology_sha256="aa" * 32,
        underlier="HOOD",
        source_id="schwab",
        account_application_identity_sha256="bb" * 32,
        collector_code_sha256="cc" * 32,
        prior_epoch_sha256=None,
        required_response_fields=(
            "callExpDateMap",
            "numberOfContracts",
            "putExpDateMap",
            "symbol",
        ),
        query=query,
        window_start_utc=scheduled,
        window_end_utc=scheduled,
        expected_snapshot_count=1,
        snapshot_offsets_seconds=(0,),
    )
    captured = SchwabOptionChainCollector(
        Client(), clock=lambda: scheduled
    ).collect(epoch, sequence=0, previous_snapshot_sha256=None)
    reference = ContractReferenceTerms(
        occ_symbol=symbol,
        expiration_date=expiry,
        strike_microdollars=50_000_000,
        option_type="CALL",
        multiplier=100,
        deliverable_shares=100,
        exercise_style="AMERICAN",
        standard=True,
    )

    normalized = methodology_snapshot_from_schwab(
        captured,
        underlier_midpoint_microdollars=50_000_000,
        underlier_halted=False,
        option_halt_flags={symbol: False},
        contract_reference={symbol: reference},
    )

    assert normalized.contracts[0].bid_microdollars == 4_900_000
    assert normalized.contracts[0].ask_microdollars == 5_100_000
    with pytest.raises(ValueError, match="halt facts"):
        methodology_snapshot_from_schwab(
            captured,
            underlier_midpoint_microdollars=50_000_000,
            underlier_halted=False,
            option_halt_flags={},
            contract_reference={symbol: reference},
        )


def test_mvp_selects_five_calls_and_sizes_equal_premium_sleeves() -> None:
    trade_date = date(2026, 9, 4)
    expiry = trade_date + timedelta(days=60)
    result = create_portfolio_target(
        target_input(
            snapshots(
                trade_date=trade_date,
                expirations=((expiry, (47, 48, 49, 50, 51, 52, 53)),),
            ),
            trade_date=trade_date,
        ),
        parameters(),
    )

    assert result.status == "TARGET_COMPUTED"
    assert result.selected_expiration == expiry
    assert len(result.positions) == 5
    assert result.decision_nav_microdollars == 100_000_000_000
    assert result.premium_budget_microdollars == 90_000_000_000
    assert result.sleeve_microdollars == 18_000_000_000
    assert {position.target_quantity for position in result.positions} == {36}
    assert {position.liquidity_ppb for position in result.positions} == {500_000_000}
    assert all(position.target_delta == 36 for position in result.positions)
    assert {reason for item in result.excluded_contracts for reason in item.exclusions} == {
        "ranked_out"
    }
    assert len(result.sha256) == 64


def test_incumbent_expiry_is_retained_at_44_dte() -> None:
    trade_date = date(2026, 9, 4)
    incumbent = trade_date + timedelta(days=44)
    successor = trade_date + timedelta(days=60)
    incumbent_symbols = tuple(
        f"HOOD-{incumbent.isoformat()}-{strike:03d}C"
        for strike in (48, 49, 50, 51, 52)
    )
    result = create_portfolio_target(
        target_input(
            snapshots(
                trade_date=trade_date,
                expirations=(
                    (incumbent, (48, 49, 50, 51, 52)),
                    (successor, (48, 49, 50, 51, 52)),
                ),
            ),
            trade_date=trade_date,
            incumbent_expiration=incumbent,
            incumbent_symbols=incumbent_symbols,
        ),
        parameters(),
    )

    assert result.status == "TARGET_COMPUTED"
    assert result.selected_expiration == incumbent
    assert all(position.retained_incumbent for position in result.positions)


def test_incomplete_collection_and_halt_do_not_create_target() -> None:
    trade_date = date(2026, 9, 4)
    expiry = trade_date + timedelta(days=60)
    complete = snapshots(
        trade_date=trade_date,
        expirations=((expiry, (48, 49, 50, 51, 52)),),
    )

    incomplete = create_portfolio_target(
        target_input(complete[:-1], trade_date=trade_date),
        parameters(),
    )
    halted_snapshots = tuple(
        replace(snapshot, underlier_halted=snapshot.sequence == 4)
        for snapshot in complete
    )
    halted = create_portfolio_target(
        target_input(halted_snapshots, trade_date=trade_date),
        parameters(),
    )

    assert incomplete.status == "NO_RECONSTITUTION"
    assert halted.status == "HALTED_INSTRUMENT"
    assert incomplete.positions == ()
    assert halted.positions == ()


def test_liquidity_and_quote_boundaries_publish_exclusion_reasons() -> None:
    trade_date = date(2026, 9, 4)
    expiry = trade_date + timedelta(days=60)
    baseline = snapshots(
        trade_date=trade_date,
        expirations=((expiry, (48, 49, 50, 51, 52)),),
    )
    symbol = f"HOOD-{expiry.isoformat()}-048C"
    cases = (
        (
            lambda item, sequence: replace(item, open_interest=499),
            "open_interest",
        ),
        (
            lambda item, sequence: replace(item, session_volume=99),
            "session_volume",
        ),
        (
            lambda item, sequence: replace(item, bid_size=0),
            "two_sided_size",
        ),
        (
            lambda item, sequence: replace(
                item, bid_microdollars=4_500_000, ask_microdollars=5_500_000
            ),
            "relative_spread",
        ),
        (
            lambda item, sequence: replace(
                item, bid_microdollars=0 if sequence >= 3 else item.bid_microdollars
            ),
            "insufficient_valid_quotes",
        ),
        (
            lambda item, sequence: replace(
                item, option_halted=sequence == 4
            ),
            "halted_or_missing_target_flag",
        ),
    )
    for mutate, expected_reason in cases:
        altered = tuple(
            replace(
                snapshot,
                contracts=tuple(
                    mutate(contract, snapshot.sequence)
                    if contract.occ_symbol == symbol
                    else contract
                    for contract in snapshot.contracts
                ),
            )
            for snapshot in baseline
        )
        result = create_portfolio_target(
            target_input(altered, trade_date=trade_date),
            parameters(),
        )
        excluded = next(
            item for item in result.excluded_contracts if item.occ_symbol == symbol
        )
        assert expected_reason in excluded.exclusions


def test_missing_current_holding_mark_prevents_reconstitution() -> None:
    trade_date = date(2026, 9, 4)
    expiry = trade_date + timedelta(days=60)
    result = create_portfolio_target(
        target_input(
            snapshots(
                trade_date=trade_date,
                expirations=((expiry, (48, 49, 50, 51, 52)),),
            ),
            trade_date=trade_date,
            positions=(CurrentPosition("HOOD-MISSING", 1),),
        ),
        parameters(),
    )

    assert result.status == "NO_RECONSTITUTION"
    assert "lacks a valid decision mark" in result.reason


def test_current_call_outside_new_basket_gets_zero_target() -> None:
    trade_date = date(2026, 9, 4)
    expiry = trade_date + timedelta(days=60)
    held_symbol = f"HOOD-{expiry.isoformat()}-047C"
    result = create_portfolio_target(
        target_input(
            snapshots(
                trade_date=trade_date,
                expirations=((expiry, (47, 48, 49, 50, 51, 52, 53)),),
            ),
            trade_date=trade_date,
            positions=(CurrentPosition(held_symbol, 3),),
        ),
        parameters(),
    )

    held_target = next(
        position for position in result.positions if position.occ_symbol == held_symbol
    )
    assert result.status == "TARGET_COMPUTED"
    assert held_target.target_quantity == 0
    assert held_target.target_delta == -3


def test_zero_contract_sleeves_preserve_unresolved_capital_policy() -> None:
    trade_date = date(2026, 9, 4)
    expiry = trade_date + timedelta(days=60)
    result = create_portfolio_target(
        target_input(
            snapshots(
                trade_date=trade_date,
                expirations=((expiry, (48, 49, 50, 51, 52)),),
            ),
            trade_date=trade_date,
            cash=1_000_000,
        ),
        parameters(),
    )

    assert result.status == "UNRESOLVED_CAPITAL_POLICY"
    assert all(position.target_quantity == 0 for position in result.positions)


def cross_language_target_vectors():
    trade_date = date(2026, 9, 4)
    strikes = (48, 49, 50, 51, 52)

    vectors = []

    def assert_vector(
        name: str,
        value: PortfolioTargetInput,
        expected_status: str,
        expected_expiration: date | None,
        expected_sha256: str,
    ) -> None:
        vectors.append((name, value, expected_status, expected_expiration, expected_sha256))

    for days, expected_days, expected_sha256 in (
        (
            30,
            60,
            "c6e6389b55602c7dbb35a117c8019889de7bd76012456b2772d69ecab6a4dd68",
        ),
        (
            31,
            31,
            "3e7522496fab841fd968de82d60b23936f4cfd7b8deb11a7d4b079cdcfd330b9",
        ),
        (
            44,
            44,
            "c38ced08289d3e23c5687dd8ba691bd4a3590809688240c4a6b99ffe2f971dfc",
        ),
    ):
        incumbent = trade_date + timedelta(days=days)
        successor = trade_date + timedelta(days=60)
        incumbent_symbols = tuple(
            f"HOOD-{incumbent.isoformat()}-{strike:03d}C"
            for strike in strikes
        )
        assert_vector(
            f"incumbent_{days}",
            target_input(
                snapshots(
                    trade_date=trade_date,
                    expirations=(
                        (incumbent, strikes),
                        (successor, strikes),
                    ),
                ),
                trade_date=trade_date,
                incumbent_expiration=incumbent,
                incumbent_symbols=incumbent_symbols,
            ),
            "TARGET_COMPUTED",
            trade_date + timedelta(days=expected_days),
            expected_sha256,
        )

    assert_vector(
        "successor_45_75",
        target_input(
            snapshots(
                trade_date=trade_date,
                expirations=tuple(
                    (trade_date + timedelta(days=days), strikes)
                    for days in (45, 75)
                ),
            ),
            trade_date=trade_date,
        ),
        "TARGET_COMPUTED",
        trade_date + timedelta(days=45),
        "ea9301964d0df3cc13d648cb0fcc133024e255c7e5a112c9a7714320158af3e9",
    )
    assert_vector(
        "successor_44_76",
        target_input(
            snapshots(
                trade_date=trade_date,
                expirations=tuple(
                    (trade_date + timedelta(days=days), strikes)
                    for days in (44, 76)
                ),
            ),
            trade_date=trade_date,
        ),
        "NO_ELIGIBLE_SUCCESSOR",
        None,
        "b3a1c3d3f0824505861817baae846cbfc039d2c051bffadfd1fa92dba618b64a",
    )

    expiry = trade_date + timedelta(days=60)
    six = snapshots(
        trade_date=trade_date,
        expirations=((expiry, (47, 48, 49, 50, 51, 52)),),
    )
    excluded_symbol = f"HOOD-{expiry.isoformat()}-047C"
    missing_final = tuple(
        replace(
            snapshot,
            contracts=tuple(
                contract
                for contract in snapshot.contracts
                if not (
                    snapshot.sequence == 4
                    and contract.occ_symbol == excluded_symbol
                )
            ),
        )
        for snapshot in six
    )
    assert_vector(
        "missing_final_quote",
        target_input(missing_final, trade_date=trade_date),
        "TARGET_COMPUTED",
        expiry,
        "32cfe352f1f047e538eaf69daed4a70ea1444fd21d684a7b2cb71b9553dcc9cb",
    )
    adjusted = tuple(
        replace(
            snapshot,
            contracts=tuple(
                replace(contract, standard=False)
                if contract.occ_symbol == excluded_symbol
                else contract
                for contract in snapshot.contracts
            ),
        )
        for snapshot in six
    )
    assert_vector(
        "adjusted_contract",
        target_input(adjusted, trade_date=trade_date),
        "TARGET_COMPUTED",
        expiry,
        "1b188715dc93259e666e6583b76eed924a37b592722b892b1e4b63632914ebc8",
    )

    exact_thresholds = tuple(
        replace(
            snapshot,
            contracts=tuple(
                replace(
                    contract,
                    bid_microdollars=4_800_000,
                    ask_microdollars=5_200_000,
                    bid_size=1,
                    ask_size=1,
                    session_volume=100,
                    open_interest=500,
                )
                for contract in snapshot.contracts
            ),
        )
        for snapshot in snapshots(
            trade_date=trade_date,
            expirations=((expiry, (45, 47, 50, 53, 55)),),
        )
    )
    assert_vector(
        "exact_thresholds",
        target_input(exact_thresholds, trade_date=trade_date),
        "TARGET_COMPUTED",
        expiry,
        "60150aaeb429facd8086a2f2c98242c9a9feccc0d126d2e7818caac1d6fd94b6",
    )
    baseline = snapshots(
        trade_date=trade_date,
        expirations=((expiry, strikes),),
    )
    assert_vector(
        "zero_sleeve",
        target_input(baseline, trade_date=trade_date, cash=1_000_000),
        "UNRESOLVED_CAPITAL_POLICY",
        expiry,
        "9ac08a4e368160c3c33d8609658533c4ed1fc1575949d0c45d4480609236fc97",
    )
    assert_vector(
        "zero_exit",
        target_input(
            snapshots(
                trade_date=trade_date,
                expirations=((expiry, (47, 48, 49, 50, 51, 52, 53)),),
            ),
            trade_date=trade_date,
            positions=(CurrentPosition(excluded_symbol, 3),),
        ),
        "TARGET_COMPUTED",
        expiry,
        "bdc51bf806b5534159d03e18007af2aab2de2b8dbc8a17603d07d2c265a70edd",
    )
    assert_vector(
        "underlier_halt",
        target_input(
            tuple(
                replace(snapshot, underlier_halted=snapshot.sequence == 4)
                for snapshot in baseline
            ),
            trade_date=trade_date,
        ),
        "HALTED_INSTRUMENT",
        None,
        "4232d4800d36a502382a4850bf0a395d31f96937589004c937dfeca7547b7f6a",
    )
    assert_vector(
        "incomplete",
        target_input(baseline[:-1], trade_date=trade_date),
        "NO_RECONSTITUTION",
        None,
        "16b67ed2ff1a32faba0068d73293363d1b00c1e780a693bcb2de02e039a5491c",
    )
    assert_vector(
        "missing_mark",
        target_input(
            baseline,
            trade_date=trade_date,
            positions=(CurrentPosition("HOOD-MISSING", 1),),
        ),
        "NO_RECONSTITUTION",
        expiry,
        "8f026c05e836521ca90214eb841db364cf8654acd350d1583cd7ebcd34aa3690",
    )

    assert_vector(
        "baseline", target_input(baseline, trade_date=trade_date), "TARGET_COMPUTED", expiry,
        "3aa0c89fb073c393a44c45061431f6cb53d8ab45161a2675818fe2cf98dd0cc9",
    )
    return vectors


def test_cross_language_target_vectors_match_rust_hashes() -> None:
    for name, value, expected_status, expected_expiration, expected_sha256 in cross_language_target_vectors():
        result = create_portfolio_target(value, parameters())
        assert result.status == expected_status, name
        assert result.selected_expiration == expected_expiration, name
        assert result.sha256 == expected_sha256, name


def test_quote_age_and_schedule_tolerance_boundaries_are_inclusive() -> None:
    trade_date = date(2026, 9, 4)
    expiry = trade_date + timedelta(days=60)
    baseline = snapshots(
        trade_date=trade_date,
        expirations=((expiry, (48, 49, 50, 51, 52)),),
    )
    exact = tuple(
        replace(
            snapshot,
            observed_at_utc=snapshot.scheduled_at_utc + timedelta(seconds=30),
            contracts=tuple(
                replace(
                    contract,
                    quote_timestamp_utc=(
                        snapshot.scheduled_at_utc - timedelta(seconds=90)
                    ),
                )
                for contract in snapshot.contracts
            ),
        )
        for snapshot in baseline
    )
    assert (
        create_portfolio_target(
            target_input(exact, trade_date=trade_date),
            parameters(),
        ).status
        == "TARGET_COMPUTED"
    )

    late = tuple(
        replace(
            snapshot,
            observed_at_utc=snapshot.scheduled_at_utc + timedelta(seconds=31),
        )
        for snapshot in exact
    )
    assert (
        create_portfolio_target(
            target_input(late, trade_date=trade_date),
            parameters(),
        ).status
        == "NO_RECONSTITUTION"
    )

    stale = tuple(
        replace(
            snapshot,
            contracts=tuple(
                replace(
                    contract,
                    quote_timestamp_utc=(
                        snapshot.observed_at_utc - timedelta(seconds=121)
                    ),
                )
                for contract in snapshot.contracts
            ),
        )
        for snapshot in baseline
    )
    assert (
        create_portfolio_target(
            target_input(stale, trade_date=trade_date),
            parameters(),
        ).status
        == "NO_ELIGIBLE_SUCCESSOR"
    )
