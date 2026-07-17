from __future__ import annotations

from threading import Barrier
import unittest

from postfiat_rpc.transparent_swap_latency import (
    PhaseTimeline,
    instrument_certified_leg,
    prepare_two_settlement_legs_concurrent,
    timing_names,
    verify_six_concurrent,
    verify_six_sequential,
)


class FakeClock:
    def __init__(self) -> None:
        self.value = 0

    def __call__(self) -> int:
        self.value += 1_000_000
        return self.value


class TransparentSwapLatencyTests(unittest.TestCase):
    def test_certified_leg_emits_every_required_timer(self) -> None:
        timeline = PhaseTimeline(clock_ns=FakeClock())
        result = instrument_certified_leg(
            label="pfusdc",
            timeline=timeline,
            discover=lambda: "validator-2",
            quote=lambda proposer: {"proposer": proposer},
            sign=lambda quote: {"signed": quote},
            submit=lambda signed, proposer: {"signed": signed, "proposer": proposer, "certified": True},
            response_is_certified=lambda response: response["certified"],
        )

        self.assertTrue(result["certified"])
        self.assertEqual(
            timing_names(timeline.artifact()),
            {
                "pfusdc.discovery",
                "pfusdc.quote",
                "pfusdc.sign",
                "pfusdc.submit_to_first_response",
                "pfusdc.submit_to_certified_finality",
            },
        )

    def test_missing_certified_finality_fails_closed_with_partial_timeline(self) -> None:
        timeline = PhaseTimeline(clock_ns=FakeClock())

        with self.assertRaisesRegex(RuntimeError, "did not contain certified finality"):
            instrument_certified_leg(
                label="a651",
                timeline=timeline,
                discover=lambda: "validator-1",
                quote=lambda _proposer: {},
                sign=lambda quote: quote,
                submit=lambda _signed, _proposer: {"certified": False},
                response_is_certified=lambda response: response["certified"],
            )

        self.assertIn("a651.submit_to_first_response", timing_names(timeline.artifact()))
        self.assertNotIn("a651.submit_to_certified_finality", timing_names(timeline.artifact()))

    def test_sequential_verification_requires_and_times_all_six(self) -> None:
        timeline = PhaseTimeline(clock_ns=FakeClock())
        validators = {f"validator-{index}": f"endpoint-{index}" for index in range(6)}

        rows = verify_six_sequential(
            validators,
            lambda validator_id, endpoint: {"validator": validator_id, "endpoint": endpoint},
            timeline,
        )

        self.assertEqual(set(rows), set(validators))
        self.assertEqual(
            {name for name in timing_names(timeline.artifact()) if name.startswith("verify.")},
            {f"verify.validator-{index}" for index in range(6)},
        )

    def test_sequential_verification_rejects_diluted_fleet(self) -> None:
        with self.assertRaisesRegex(ValueError, "exactly 6"):
            verify_six_sequential(
                {"validator-0": "endpoint"},
                lambda _validator, _endpoint: {},
                PhaseTimeline(clock_ns=FakeClock()),
            )

    def test_concurrent_verification_requires_all_six_exact_results(self) -> None:
        timeline = PhaseTimeline()
        validators = {f"validator-{index}": f"endpoint-{index}" for index in range(6)}
        rendezvous = Barrier(6, timeout=2)

        def verify(validator_id: str, endpoint: str) -> dict[str, str]:
            rendezvous.wait()
            return {"validator": validator_id, "endpoint": endpoint, "balance": "exact"}

        rows = verify_six_concurrent(validators, verify, timeline)

        self.assertEqual(list(rows), list(validators))
        self.assertTrue(all(row["balance"] == "exact" for row in rows.values()))
        names = timing_names(timeline.artifact())
        self.assertIn("verify.six_concurrent_total", names)
        self.assertEqual(
            {name for name in names if name.startswith("verify.validator-")},
            {f"verify.validator-{index}" for index in range(6)},
        )

    def test_concurrent_verification_propagates_one_validator_failure(self) -> None:
        validators = {f"validator-{index}": f"endpoint-{index}" for index in range(6)}

        def verify(validator_id: str, _endpoint: str) -> dict[str, bool]:
            if validator_id == "validator-4":
                raise RuntimeError("terminal balance mismatch")
            return {"ok": True}

        with self.assertRaisesRegex(RuntimeError, "terminal balance mismatch"):
            verify_six_concurrent(validators, verify, PhaseTimeline())

    def test_concurrent_verification_rejects_diluted_fleet(self) -> None:
        with self.assertRaisesRegex(ValueError, "exactly 6"):
            verify_six_concurrent(
                {"validator-0": "endpoint"},
                lambda _validator, _endpoint: {},
                PhaseTimeline(),
            )

    def test_two_settlement_legs_prepare_concurrently_in_stable_order(self) -> None:
        rendezvous = Barrier(2, timeout=2)
        timeline = PhaseTimeline()
        legs = {"pfusdc": {"source": "buyer"}, "a651": {"source": "seller"}}

        def prepare(label: str, leg: dict[str, str]) -> dict[str, str]:
            rendezvous.wait()
            return {"label": label, **leg}

        prepared = prepare_two_settlement_legs_concurrent(legs, prepare, timeline)

        self.assertEqual(list(prepared), ["pfusdc", "a651"])
        self.assertEqual(prepared["pfusdc"]["source"], "buyer")
        self.assertEqual(prepared["a651"]["source"], "seller")
        self.assertIn(
            "settlement.prepare_two_concurrent_total",
            {record.name for record in timeline.records},
        )

    def test_one_preparation_failure_returns_no_partial_result(self) -> None:
        legs = {"pfusdc": {}, "a651": {}}

        def prepare(label: str, _leg: dict) -> dict:
            if label == "a651":
                raise RuntimeError("quote rejected")
            return {"ok": True}

        timeline = PhaseTimeline()
        with self.assertRaisesRegex(RuntimeError, "quote rejected"):
            prepare_two_settlement_legs_concurrent(legs, prepare, timeline)
        total = next(
            record
            for record in timeline.records
            if record.name == "settlement.prepare_two_concurrent_total"
        )
        self.assertLess(total.metadata["prepared_leg_count"], 2)

    def test_settlement_preparation_rejects_wrong_leg_count(self) -> None:
        with self.assertRaisesRegex(ValueError, "exactly 2"):
            prepare_two_settlement_legs_concurrent(
                {"pfusdc": {}}, lambda _label, leg: leg, PhaseTimeline()
            )


if __name__ == "__main__":
    unittest.main()
