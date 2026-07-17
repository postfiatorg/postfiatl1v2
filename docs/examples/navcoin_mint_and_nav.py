#!/usr/bin/env python3
"""Calculate NAVCOIN NAV and print native PostFiat NAV operation JSON."""

from __future__ import annotations

import argparse
import json

from postfiat_rpc.navcoin import DEFAULT_PROOF_PROFILE, NavInputs, build_packet_and_operations


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--issuer", default="pfissuer-example")
    parser.add_argument("--ap-account", default="pfap-example")
    parser.add_argument("--asset-code", default="NAV")
    parser.add_argument("--epoch", type=int, default=1)
    parser.add_argument("--circulating-supply", type=int, default=1_000)
    parser.add_argument("--mint-amount", type=int, default=1_000)
    parser.add_argument("--redeem-amount", type=int, default=10)
    parser.add_argument("--cash-micro-usd", type=int, default=622_300_000)
    parser.add_argument("--broker-positions-micro-usd", type=int, default=400_000_000)
    parser.add_argument("--liabilities-micro-usd", type=int, default=40_000_000)
    parser.add_argument("--pending-redemptions-micro-usd", type=int, default=0)
    parser.add_argument("--proof-profile", default=DEFAULT_PROOF_PROFILE)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    inputs = NavInputs(
        issuer=args.issuer,
        ap_account=args.ap_account,
        asset_code=args.asset_code,
        epoch=args.epoch,
        circulating_supply=args.circulating_supply,
        mint_amount=args.mint_amount,
        redeem_amount=args.redeem_amount,
        cash_micro_usd=args.cash_micro_usd,
        broker_positions_micro_usd=args.broker_positions_micro_usd,
        liabilities_micro_usd=args.liabilities_micro_usd,
        pending_redemptions_micro_usd=args.pending_redemptions_micro_usd,
        proof_profile=args.proof_profile,
    )
    print(json.dumps(build_packet_and_operations(inputs), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
