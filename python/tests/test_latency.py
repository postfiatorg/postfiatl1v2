from __future__ import annotations

import json
import tempfile
import time
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import postfiat_rpc.latency as latency_module
from postfiat_rpc.latency import (
    LatencyCounts,
    _finality_summary,
    account_submit_capability_gate,
    build_latency_report,
    load_wallet_descriptor,
    markdown_report,
    percentile,
    read_proxy_auth_token_file,
    summarize_durations,
    write_latency_outputs,
)


class LatencyTests(unittest.TestCase):
    def test_proxy_auth_token_file_must_be_private_and_nonempty(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            token_file = Path(tmp) / "proxy.token"
            token_file.write_text("t" * 32 + "\n", encoding="utf-8")
            token_file.chmod(0o600)
            self.assertEqual(read_proxy_auth_token_file(token_file), "t" * 32)

            token_file.chmod(0o640)
            with self.assertRaisesRegex(ValueError, "group or other"):
                read_proxy_auth_token_file(token_file)

    def test_cli_requires_explicit_wallet_descriptors(self) -> None:
        with self.assertRaises(SystemExit) as missing_wallets:
            latency_module.main(["--output-dir", "/tmp/postfiat-latency-test"])

        self.assertEqual(missing_wallets.exception.code, 2)

    def test_account_submit_capability_gate_blocks_oversized_run(self) -> None:
        gate = account_submit_capability_gate(
            server_info={
                "rpc": {
                    "mempool_submit_finality_enabled": True,
                    "max_mempool_submit_per_peer": 16,
                    "max_mempool_submit_total": 64,
                }
            },
            counts=LatencyCounts(native=50, payment_v2=50, fastpay=20),
        )

        self.assertFalse(gate["ok"])
        self.assertIn("native-count 50 exceeds per-peer submit cap 16", gate["reasons"])
        self.assertIn("payment-v2-count 50 exceeds per-peer submit cap 16", gate["reasons"])
        self.assertIn("account-lane submits 100 exceed total submit cap 64", gate["reasons"])

    def test_account_submit_capability_gate_allows_rolling_window_caps(self) -> None:
        gate = account_submit_capability_gate(
            server_info={
                "rpc": {
                    "mempool_submit_finality_enabled": True,
                    "max_mempool_submit_per_peer": 16,
                    "max_mempool_submit_total": 64,
                    "mempool_submit_rate_limit_window_secs": 60,
                }
            },
            counts=LatencyCounts(native=50, payment_v2=50, fastpay=20),
        )

        self.assertTrue(gate["ok"])
        self.assertEqual(gate["reasons"], [])
        self.assertTrue(gate["warnings"])

    def test_account_submit_capability_gate_allows_smoke_run(self) -> None:
        gate = account_submit_capability_gate(
            server_info={
                "rpc": {
                    "mempool_submit_finality_enabled": True,
                    "max_mempool_submit_per_peer": 16,
                    "max_mempool_submit_total": 64,
                }
            },
            counts=LatencyCounts(native=1, payment_v2=1, fastpay=1),
        )

        self.assertTrue(gate["ok"])
        self.assertEqual(gate["reasons"], [])

    def test_percentile_uses_nearest_rank(self) -> None:
        values = [10.0, 20.0, 30.0, 40.0]

        self.assertEqual(percentile(values, 50), 20.0)
        self.assertEqual(percentile(values, 90), 40.0)
        self.assertEqual(percentile(values, 100), 40.0)
        self.assertIsNone(percentile([], 50))

    def test_summarize_durations_counts_failures(self) -> None:
        summary = summarize_durations(
            [
                {"category": "native_pft", "ok": True, "duration_ms": 10},
                {"category": "native_pft", "ok": True, "duration_ms": 30},
                {"category": "native_pft", "ok": False, "duration_ms": 5},
                {"category": "payment_v2", "ok": True, "duration_ms": 99},
            ],
            "native_pft",
        )

        self.assertEqual(summary["count"], 2)
        self.assertEqual(summary["failure_count"], 1)
        self.assertEqual(summary["p50_ms"], 10.0)
        self.assertEqual(summary["max_ms"], 30.0)

    def test_finality_summary_preserves_proxy_route_timing(self) -> None:
        summary = _finality_summary({
            "tx_id": "tx1",
            "total_ms": 1200.5,
            "readiness_wait_ms": 77.0,
            "mempool_submit_ms": 10.0,
            "mempool_batch_ms": 11.0,
            "certified_round_ms": 900.0,
            "proxy_route": {
                "routed": True,
                "proposer": "validator-0",
                "route_attempts": 3,
                "route_wait_ms": 502,
                "converged_count": 6,
            },
            "finality": {
                "block": {
                    "header": {
                        "height": 42,
                        "proposer": "validator-0",
                        "state_root": "root",
                    },
                },
                "receipt": {
                    "code": "accepted",
                    "accepted": True,
                },
            },
        })

        self.assertEqual(summary["proxy_route_attempts"], 3)
        self.assertEqual(summary["proxy_route_wait_ms"], 502)
        self.assertEqual(summary["readiness_wait_ms"], 77.0)
        self.assertEqual(summary["proxy_route_converged_count"], 6)
        self.assertEqual(summary["proxy_route"]["proposer"], "validator-0")

    def test_load_wallet_descriptor_accepts_stakehub_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "wallet.json"
            backup = Path(tmp) / "buyer.backup.json"
            key = Path(tmp) / "buyer.key.json"
            path.write_text(
                json.dumps({
                    "chain_id": "postfiat-wan-devnet",
                    "account_index": 2,
                    "address": "pfabc",
                    "public_key_hex": "00",
                    "backup_file": str(backup),
                    "key_file": str(key),
                }),
                encoding="utf-8",
            )

            wallet = load_wallet_descriptor(path)

        self.assertEqual(wallet.address, "pfabc")
        self.assertEqual(wallet.public_key_hex, "00")
        self.assertEqual(wallet.backup_file, backup)
        self.assertEqual(wallet.key_file, key)
        self.assertEqual(wallet.account_index, 2)

    def test_load_wallet_descriptor_derives_backup_from_key_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "holder.key.json"
            backup = Path(tmp) / "holder.backup.json"
            path.write_text(
                json.dumps({"address": "pfabc", "public_key_hex": "00"}),
                encoding="utf-8",
            )
            backup.write_text(
                json.dumps({"chain_id": "postfiat-wan-devnet-2"}),
                encoding="utf-8",
            )

            wallet = load_wallet_descriptor(path)

        self.assertEqual(wallet.backup_file, Path(tmp) / "holder.backup.json")
        self.assertEqual(wallet.chain_id, "postfiat-wan-devnet-2")

    def test_red_fleet_blocks_mutable_latency_run(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            with (
                mock.patch.object(
                    latency_module,
                    "collect_rpc_preflight",
                    return_value=[{"validator_id": "validator-0", "reachable": False}],
                ),
                mock.patch.object(
                    latency_module,
                    "summarize_preflight",
                    return_value={
                        "healthy": False,
                        "reachable_count": 0,
                        "validator_count": 1,
                        "largest_ledger_group": 0,
                        "red_reasons": ["unreachable validators: validator-0"],
                    },
                ),
            ):
                report = build_latency_report(
                    proxy_url="ws://127.0.0.1:8080",
                    wallet_a_path=Path(tmp) / "missing-a.json",
                    wallet_b_path=Path(tmp) / "missing-b.json",
                    output_dir=Path(tmp),
                    counts=LatencyCounts(native=1, payment_v2=1, fastpay=1),
                    amount=1,
                    fastpay_fee=1,
                    timeout_seconds=1.0,
                    allow_red_fleet=False,
                )

        self.assertEqual(report["status"], "blocked_red_fleet")
        self.assertEqual(report["events"], [])

    def test_rate_limit_gate_blocks_before_wallet_loading(self) -> None:
        class FakeClient:
            def __init__(self, *_args, **_kwargs) -> None:
                pass

            def server_info(self) -> dict:
                return {
                    "rpc": {
                        "mempool_submit_finality_enabled": True,
                        "max_mempool_submit_per_peer": 16,
                        "max_mempool_submit_total": 64,
                    }
                }

        with tempfile.TemporaryDirectory() as tmp:
            with (
                mock.patch.object(
                    latency_module,
                    "collect_rpc_preflight",
                    return_value=[{"validator_id": "validator-0", "reachable": True}],
                ),
                mock.patch.object(
                    latency_module,
                    "summarize_preflight",
                    return_value={
                        "healthy": True,
                        "reachable_count": 1,
                        "validator_count": 1,
                        "largest_ledger_group": 1,
                        "red_reasons": [],
                    },
                ),
                mock.patch.object(latency_module, "PostFiatWebSocketRpcClient", FakeClient),
            ):
                report = build_latency_report(
                    proxy_url="ws://127.0.0.1:8080",
                    wallet_a_path=Path(tmp) / "missing-a.json",
                    wallet_b_path=Path(tmp) / "missing-b.json",
                    output_dir=Path(tmp),
                    counts=LatencyCounts(native=50, payment_v2=50, fastpay=20),
                    amount=1,
                    fastpay_fee=1,
                    timeout_seconds=1.0,
                    allow_red_fleet=False,
                )

        self.assertEqual(report["status"], "blocked_rate_limit_config")
        self.assertEqual(report["events"], [])
        self.assertFalse(report["capability_gate"]["ok"])

    def test_write_outputs_includes_raw_jsonl_and_markdown(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            output_dir = Path(tmp)
            report = {
                "created_at": "now",
                "status": "complete",
                "proxy_url": "ws://example",
                "preflight": {"summary": {"healthy": True}},
                "events": [{"category": "native_pft", "ok": True, "duration_ms": 12.3}],
                "capability_gate": {
                    "ok": False,
                    "planned_account_submits": 100,
                    "reasons": ["cap too low"],
                    "rpc": {
                        "max_mempool_submit_per_peer": 16,
                        "max_mempool_submit_total": 64,
                    },
                },
                "summary": {
                    "native_pft": {"count": 1, "failure_count": 0},
                    "payment_v2": {"count": 0, "failure_count": 0},
                    "fastpay_cycle": {"count": 0, "failure_count": 0},
                    "full_stage8_sample_size_met": False,
                    "unsupported_breakdowns": ["gap"],
                },
            }

            paths = write_latency_outputs(report, output_dir)

            self.assertTrue(paths["raw"].exists())
            self.assertTrue(paths["summary"].exists())
            self.assertIn("WAN Latency Evidence", paths["markdown"].read_text())
            self.assertIn("gap", markdown_report(report))
            self.assertIn("RPC Capability Gate", markdown_report(report))

    def test_fastpay_send_only_records_setup_outside_hot_duration(self) -> None:
        wallet_a = SimpleNamespace(address="pfa", public_key_hex="owner-pk")
        wallet_b = SimpleNamespace(public_key_hex="recipient-pk")
        events = []

        def fake_wrap_fastpay(*_args, **_kwargs):
            time.sleep(0.02)
            return SimpleNamespace(
                result={"object_id": "object-1"},
                timings={"wrap_rpc_ms": 20.0},
            )

        def fake_send_fastpay(*_args, **kwargs):
            self.assertEqual(kwargs["owned_objects"][0]["id"], "object-1")
            return SimpleNamespace(
                result={"applied_count": 6},
                timings={"vote_collection_ms": 3.0, "apply_ms": 4.0},
                votes=tuple({"validator_id": f"validator-{idx}"} for idx in range(5)),
            )

        with tempfile.TemporaryDirectory() as tmp:
            with (
                mock.patch.object(latency_module, "wrap_fastpay", fake_wrap_fastpay),
                mock.patch.object(latency_module, "send_fastpay", fake_send_fastpay),
            ):
                event = latency_module._record_fastpay_send_only_event(
                    events,
                    iteration=1,
                    client=object(),
                    wallet_a=wallet_a,
                    wallet_b=wallet_b,
                    amount=1,
                    fee=1,
                    work_dir=Path(tmp),
                    validator_records=[{"validator_id": f"validator-{idx}"} for idx in range(6)],
                )

        self.assertTrue(event["ok"])
        self.assertEqual(event["category"], "fastpay_send_only")
        self.assertEqual(event["setup"]["setup_wrap_result"]["object_id"], "object-1")
        self.assertEqual(event["substeps"]["vote_count"], 5)
        self.assertLess(event["duration_ms"], event["setup"]["setup_wrap_ms"])


if __name__ == "__main__":
    unittest.main()
