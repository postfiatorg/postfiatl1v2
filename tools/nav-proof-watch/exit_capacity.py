#!/usr/bin/env python3
"""Buyer-visible exit capacity verifier (plan 07, criterion 6, condition 1).

Read-only. It answers one question with measured, dated facts: **can a
buyer actually get out, and through what, within what capacity, today?**

It performs exactly two kinds of network I/O, both reads:

1. HTTP GET of the wallet API routes endpoint (`postfiat.navcoin.routes.v1`)
   — the same public endpoint `execution/review-03` used;
2. read-only `eth_call` against a public Ethereum RPC, for the pfUSDC
   vault's physical USDC balance, its `totalObligations()` and `paused()`.

It never signs, submits, pauses, proposes or deploys anything.

Founder/operator inputs, never invented
---------------------------------------
The pfUSDC→USDC exit lane's liveness (`live_value_enabled`,
`operator_test_only` per `inventory/D-navcoin-checkout-schwab.md`, dated
10 November 2026: "live_value_enabled: false, operator_test_only: true")
is **not exposed by any public endpoint**. It lives in the operator node's
`navcoin-bridge-supply-status` output on the validator host. This tool
therefore takes it from a `--usdc-lane-status FILE` JSON supplied by the
operator (or, post-deploy, from whatever endpoint exposes it). Without
that file the USDC-leg verdict is REFUSED (exit 3), not guessed.

Exit codes: 0 buyer exit verified; 2 NOT verified (gates failed — this is
the alarm); 3 REFUSED (operator input missing/incomplete); 4 read error.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
import urllib.request
from pathlib import Path
from typing import Any

from nav_proof_watch import DEFAULT_ROUTES_URL, fetch_routes

DEFAULT_ETH_RPC = "https://ethereum-rpc.publicnode.com"
# On-chain constants, provenance dated:
#   vault + USDC addresses verified by execution/review-02-bridge-deposits-
#   returns.md §2.1 (10 November 2026, block ~26,381,400): the live epoch-10
#   pfUSDC vault is 0xe7a4…7a2c holding 76.067140 USDC against equal
#   totalObligations. USDC contract address is the canonical mainnet one.
#   Override with --config; do not edit these silently.
DEFAULT_CONFIG = {
    "_provenance": (
        "addresses dated 2026-11-10 by execution/review-02-bridge-deposits-"
        "returns.md section 2.1; override rather than edit in place"),
    "pfusdc_vault": "0xe7a432a28b20621a70e845c7f202004cf98e7a2c",
    "usdc_token": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
}

LANE_STATUS_FIELDS = {
    "live_value_enabled": (
        "must be true for the pfUSDC→USDC exit lane to serve real users "
        "(operator; from navcoin-bridge-supply-status on the validator host)"),
    "operator_test_only": (
        "must be false; a lane that is operator-test-only does not verify a "
        "buyer exit (operator)"),
    "observed_at_unix_ms": (
        "when the operator captured the lane status (operator)"),
    "source": ("where the status came from, quoted in the verdict (operator)"),
}


# ---------------------------------------------------------------------------
# Read-only Ethereum calls (eth_call only)
# ---------------------------------------------------------------------------


def eth_call(rpc: str, to: str, data: str, timeout: float = 20.0) -> str:
    body = json.dumps({
        "jsonrpc": "2.0", "id": 1, "method": "eth_call",
        "params": [{"to": to, "data": data}, "latest"],
    }).encode()
    request = urllib.request.Request(
        rpc, data=body, headers={"Content-Type": "application/json",
                                 "User-Agent": "nav-proof-watch/1 (read-only)"})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        payload = json.loads(response.read().decode("utf-8"))
    if "error" in payload:
        raise ValueError(f"eth_call error: {payload['error']}")
    result = payload.get("result")
    if not isinstance(result, str) or not result.startswith("0x"):
        raise ValueError(f"eth_call returned {result!r}")
    return result


def selector(signature: str) -> str:
    # keccak256 is not in the stdlib; selectors for the three functions used
    # here are stable, well-known constants and are asserted by tests against
    # cast when available. Do not add new calls by editing this mapping
    # without re-deriving the selector.
    known = {
        # Derived 2026-11-10 with `cast sig`:
        #   totalObligations() -> 0xfbe1f9eb
        #   paused()           -> 0x5c975abb
        #   balanceOf(address) -> 0x70a08231 (used inline above)
        "totalObligations()": "0x" + "fbe1f9eb",
        "paused()": "0x" + "5c975abb",
    }
    if signature in known:
        return known[signature]
    raise ValueError(f"selector not pre-derived for {signature}; derive it "
                     "with cast keccak/sig and add it explicitly")


def balance_of_data(address: str) -> str:
    return "0x70a08231" + address.lower().replace("0x", "").rjust(64, "0")


def read_uint(result: str) -> int:
    return int(result, 16)


# ---------------------------------------------------------------------------
# Lane status input (operator-supplied; never invented)
# ---------------------------------------------------------------------------


def load_lane_status(path: Path | None) -> tuple[dict[str, Any] | None, list[str]]:
    if path is None or not path.is_file():
        return None, [
            f"USDC lane status file not found at {path}. The pfUSDC→USDC "
            "exit lane's liveness is not exposed by any public endpoint; an "
            "operator must capture navcoin-bridge-supply-status and supply "
            "it. This tool will not guess a lane state."
        ]
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        return None, [f"lane status file {path} is not valid JSON: {exc}"]
    missing = [f"MISSING: {key} — {what}"
               for key, what in LANE_STATUS_FIELDS.items()
               if key not in raw or raw[key] in (None, "")]
    if missing:
        return None, missing
    return raw, []


# ---------------------------------------------------------------------------
# Verdict
# ---------------------------------------------------------------------------


def check(routes_url: str, eth_rpc: str, config: dict[str, Any],
          lane_status: dict[str, Any] | None, lane_missing: list[str],
          timeout: float = 20.0) -> dict[str, Any]:
    payload = fetch_routes(routes_url, timeout=timeout)
    evaluated_at = int(time.time() * 1000)

    routes: list[dict[str, Any]] = []
    for route in payload["routes"]:
        supply = (route.get("source") or {}).get("supply") or {}
        state = route.get("state") or {}
        settlement_reserve = int(supply.get("settlement_reserve_atoms") or 0)
        reserve_backed = int(supply.get("reserve_backed_redeem_atoms") or 0)
        precision = int(route.get("precision") or 6)
        routes.append({
            "route_id": route.get("route_id"),
            "name": route.get("name"),
            "sell_allowed": state.get("sell") is True,
            "buy_allowed": state.get("buy") is True,
            "state_label": state.get("state"),
            "state_reason": state.get("reason"),
            "settlement_asset_code": (route.get("source", {}).get("route")
                                      or {}).get("settlement_asset_code"),
            "redemption_capacity_pfusdc": settlement_reserve / (10 ** precision),
            "reserve_backed_redeem_pfusdc": reserve_backed / (10 ** precision),
            "settlement_reserve_atoms": str(settlement_reserve),
            "reserve_backed_redeem_atoms": str(reserve_backed),
        })

    # Ethereum leg: physical USDC vs the vault's recorded obligations.
    vault = config["pfusdc_vault"]
    usdc = config["usdc_token"]
    vault_usdc_raw = read_uint(eth_call(eth_rpc, usdc,
                                        balance_of_data(vault), timeout))
    obligations_raw = read_uint(eth_call(eth_rpc, vault,
                                         selector("totalObligations()"),
                                         timeout))
    paused = read_uint(eth_call(eth_rpc, vault, selector("paused()"),
                                timeout)) == 1
    ethereum = {
        "rpc": eth_rpc,
        "pfusdc_vault": vault,
        "vault_usdc": vault_usdc_raw / 10 ** 6,
        "vault_total_obligations_usdc": obligations_raw / 10 ** 6,
        "vault_paused": paused,
        "obligations_covered": vault_usdc_raw >= obligations_raw,
    }

    # Gates — every failure names exactly what blocks a buyer exit today.
    gates: dict[str, dict[str, Any]] = {}

    sellable = [r for r in routes if r["sell_allowed"]
                and r["settlement_reserve_atoms"] != "0"]
    gates["route_redemption_lane"] = {
        "pass": bool(sellable),
        "measured": [{
            "route_id": r["route_id"],
            "capacity_pfusdc": r["redemption_capacity_pfusdc"],
            "state_reason": r["state_reason"],
        } for r in routes],
        "why": (
            "at least one route must allow sells against positive settlement "
            "reserve for a buyer to redeem at NAV into pfUSDC"
        ) if not sellable else "sell lane open with measured reserve",
    }

    if lane_status is not None:
        lane_ok = (lane_status["live_value_enabled"] is True
                   and lane_status["operator_test_only"] is False)
        gates["usdc_exit_lane"] = {
            "pass": lane_ok,
            "measured": lane_status,
            "why": ("pfUSDC→USDC lane must be live for real users and not "
                    "operator-test-only"
                    ) if not lane_ok else "lane reports live for users",
        }
    else:
        gates["usdc_exit_lane"] = {
            "pass": False,
            "refused": True,
            "missing": lane_missing,
            "why": "operator input missing; verdict refused, not guessed",
        }

    gates["vault_usdc_coverage"] = {
        "pass": ethereum["obligations_covered"] and not paused,
        "measured": ethereum,
        "why": (
            "the vault must hold at least its obligations in physical USDC "
            "and not be paused"
        ),
    }

    verified = all(g["pass"] for g in gates.values())
    capacity = {
        "redemption_at_nav_pfusdc_total": sum(
            r["redemption_capacity_pfusdc"] for r in sellable),
        "usdc_exit_backing_usdc": ethereum["vault_usdc"],
        "note": (
            "measured facts only: route settlement reserves and vault USDC. "
            "Any *stated* capacity above these is a founder term "
            "(D-SETTLEMENT); this tool reports what the chain actually pays."
        ),
    }
    return {
        "schema": "postfiat.nav-proof-watch.exit-capacity.v1",
        "evaluated_at_unix_ms": evaluated_at,
        "buyer_exit_verified": verified,
        "gates": gates,
        "measured_capacity": capacity,
        "routes": routes,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    parser.add_argument("--routes-url", default=DEFAULT_ROUTES_URL)
    parser.add_argument("--eth-rpc", default=DEFAULT_ETH_RPC)
    parser.add_argument("--config", default=None,
                        help="JSON with pfusdc_vault/usdc_token overrides")
    parser.add_argument("--usdc-lane-status", default=None,
                        help="operator-captured lane status JSON "
                             "(navcoin-bridge-supply-status); without it the "
                             "USDC-leg verdict is refused, not guessed")
    parser.add_argument("--out", default=None, help="write the JSON verdict here")
    args = parser.parse_args(argv)

    config = dict(DEFAULT_CONFIG)
    if args.config:
        override = json.loads(Path(args.config).read_text(encoding="utf-8"))
        config.update({k: v for k, v in override.items()
                       if not k.startswith("_")})

    lane_status, lane_missing = load_lane_status(
        Path(args.usdc_lane_status) if args.usdc_lane_status else None)
    try:
        verdict = check(args.routes_url, args.eth_rpc, config, lane_status,
                        lane_missing)
    except Exception as exc:  # noqa: BLE001 - report, never half-run
        print(f"ERROR: read failed: {exc}", file=sys.stderr)
        return 4

    if args.out:
        Path(args.out).write_text(
            json.dumps(verdict, indent=2, sort_keys=True) + "\n",
            encoding="utf-8")
    print(json.dumps(verdict, indent=2, sort_keys=True))
    if verdict["buyer_exit_verified"]:
        print("VERDICT: buyer exit VERIFIED within measured capacity")
        return 0
    refused = [name for name, gate in verdict["gates"].items()
               if gate.get("refused")]
    if refused:
        print(f"VERDICT: REFUSED — operator input missing for: "
              f"{', '.join(refused)}; nothing guessed")
        return 3
    print("VERDICT: buyer exit NOT VERIFIED (gates failed; see above)")
    return 2


if __name__ == "__main__":
    sys.exit(main())
