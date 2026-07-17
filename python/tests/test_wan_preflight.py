from __future__ import annotations

import unittest

from postfiat_rpc.wan_preflight import default_validator_endpoints, summarize_preflight


def entry(
    validator_id: str,
    *,
    height: int = 10,
    root: str = "root-a",
    tip: str = "tip-a",
    pending: int = 0,
    reachable: bool = True,
    finality: bool = True,
) -> dict:
    return {
        "validator_id": validator_id,
        "reachable": reachable,
        "status": {
            "ok": reachable,
            "result": {
                "block_height": height,
                "block_tip_hash": tip,
                "state_root": root,
            },
        },
        "server_info": {
            "ok": reachable,
            "result": {
                "rpc": {
                    "read_only": False,
                    "mempool_submit_finality_enabled": finality,
                }
            },
        },
        "mempool_status": {"summary": {"total": pending}},
    }


class WanPreflightTests(unittest.TestCase):
    def test_default_preflight_never_silently_targets_live_infrastructure(self) -> None:
        endpoints = default_validator_endpoints()
        self.assertEqual(6, len(endpoints))
        self.assertTrue(all(endpoint.host == "127.0.0.1" for endpoint in endpoints))

    def test_summarize_preflight_green_when_fleet_is_converged(self) -> None:
        entries = [entry(f"validator-{idx}") for idx in range(6)]

        summary = summarize_preflight(entries)

        self.assertTrue(summary["healthy"])
        self.assertEqual(summary["largest_ledger_group"], 6)
        self.assertEqual(summary["red_reasons"], [])

    def test_summarize_preflight_red_for_stale_validator_and_pending_mempool(self) -> None:
        entries = [entry(f"validator-{idx}") for idx in range(6)]
        entries[1] = entry("validator-1", height=9, root="old-root", tip="old-tip", pending=1)

        summary = summarize_preflight(entries)

        self.assertFalse(summary["healthy"])
        reasons = "\n".join(summary["red_reasons"])
        self.assertIn("ledger divergence", reasons)
        self.assertIn("validator-1=1", reasons)

    def test_summarize_preflight_red_when_finality_is_disabled(self) -> None:
        entries = [entry(f"validator-{idx}") for idx in range(6)]
        entries[4] = entry("validator-4", finality=False)

        summary = summarize_preflight(entries)

        self.assertFalse(summary["healthy"])
        self.assertIn("finality RPC disabled: validator-4", summary["red_reasons"])


if __name__ == "__main__":
    unittest.main()
