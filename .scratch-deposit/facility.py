#!/usr/bin/env python3
"""A1 + the collateral pool: prove the primary-market facility for a
brokerage-backed NAVCoin, on the real postfiat-wan-devnet-2 chain.

What this does, in order, each step verified against consensus before the next:

  1. preflight   -- read the NAV asset, routes, balances, height. Writes nothing.
  2. route-init  -- register a primary-market facility for the asset, live value OFF
  3. activate    -- advance the route epoch, live value ON, policy valid from now
  4. fund-pool   -- post pfUSDC into the facility: this is the collateral pool
                    a holder can redeem against without trusting anybody
  5. issue       -- order_reserve + primary_subscribe_v2: buyer pays pfUSDC,
                    receives newly created NAVCoin at the finalized NAV
  6. redeem      -- primary_redeem: NAVCoin retired, pfUSDC returned from the pool

It moves no real money. pfUSDC on this chain is devnet value and the NAVCoin is
priced off the already-finalized epoch-1 reserve packet. Nothing here touches
Schwab, and both Ethereum mints stay paused -- `live_value_enabled` governs PFTL
primary-market pricing, not the Ethereum export lane, and no export is submitted.

Scale note: the opening claim behind this asset is 2,000.00 USD, admitted by the
coverage test at 208x. Net equity is 418,039 USD, but claiming it would need the
four unreported liability categories resolved first (deliverable A2). The
mechanism proven here is size-independent; the claim is not.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
from pathlib import Path

import pftl
import record_deposit as deposit_run

ROOT = Path(__file__).resolve().parent
STATE = ROOT / "facility-state.json"

ISSUER = "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b"
HOLDER = "pfab9b9228942e5c529633a13aa271d5297bec6353"
KEYS = Path("/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private")
ISSUER_KEY = KEYS / "faucet-key.json"
HOLDER_KEY = KEYS / "holder-key.json"

# Resolved at runtime by bootstrap(). The epoch-1 deposit asset (PFDEP,
# 1489e967...) CANNOT be used here: it was created with a permanent max supply,
# and a primary-market facility refuses such an asset because issuance must be
# able to create new supply (nav_vault_asset_execution.rs:4210).
NAV_ASSET = ""
ASSET_CODE = "B23"
ASSET_VERSION = 1
ASSET_PRECISION = 6
ASSET_DISPLAY_NAME = "Post Fiat NAVCoin - Schwab brokerage account (facility proof)"
VALUATION_UNIT = "usd_1e6"
OPENING_CLAIM_ATOMS = 2_000_000_000   # 2,000.00 USD, inside the coverage test
NAV_PER_UNIT = 1_000_000              # 1.000000 USD per coin at the opening mint
PFUSDC = ("02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c2"
          "33f6830bd5221fe2717fb6a1a7005d7b")
ROUTE_ID = "pftl-pfdep-schwab-primary-pfusdc-v1"
SALT = "20261111c"   # bump to get a fresh redemption nonce

# Sizes in atoms. Both assets are precision 6.
POOL_ATOMS = 20_000_000          # 20.000000 pfUSDC posted as the collateral pool
ISSUE_SETTLEMENT_ATOMS = 5_000_000   # buyer pays 5.000000 pfUSDC
REDEEM_NAV_ATOMS = 1_000_000     # retire 1.000000 NAVCoin on the way back

# Spreads. 1.0000x both ways keeps the arithmetic checkable by eye on the first
# run; a real facility charges a spread and these become the entry/exit fee.
ISSUE_MULTIPLIER_BPS = 10_000
REDEEM_MULTIPLIER_BPS = 10_000

# Ethereum export is NOT part of this proof. These fields must be non-zero to
# pass EthereumRouteVerificationPolicyV1::validate, and they are never used
# because no export packet is submitted and the Ethereum mints stay paused.
# The code hashes are the real deployed A666 values, so nothing here invents a
# contract identity; the committee root is a labelled devnet placeholder.
# These are byte arrays on the wire: FastSwapCommitteeRootV1 uses
# serialize_bytes over [u8; 48], and the code hashes are [u8; 32]. JSON carries
# them as integer arrays, not hex strings.
ETH_POLICY = {
    # Mempool admission requires an exact match against a committee already
    # governed on this chain (epoch, root, chain_id, genesis, protocol version).
    # These are the real governed values recorded in the deployment artifacts.
    # The committee governed on this ledger, taken verbatim from the stored
    # A666 production route-init operation that consensus accepted on this same
    # chain: deployments/a666-mainnet-20260727/09-production-route-init.ops.json
    "authority_epoch": 1,
    "committee_root": list(bytes.fromhex(
        "a2eebcbaa026e9a527861001dcc8f362f1173c5c2001a0eb4b3b7b5114e959c4"
        "ccdc6b01cc400f985e34fbc02095968e")),
    "minimum_confirmations": 12,
    "handoff_controller_code_hash": list(bytes.fromhex(
        "4c62b7d8b3a7928fd9667445f8fd68b3336ba0ec9a8f3e59b463b684fe6ceaaf")),
    "wrapped_navcoin_code_hash": list(bytes.fromhex(
        "671ee905050e2969995a8c6db8b05e4c2f30bd690eeff55093c03f9722be66b0")),
}
HANDOFF_CONTROLLER = "0x9a0262c0572fb4db08765408eb225e207f40c3d9"
SETTLEMENT_ADAPTER = "0x8583409ddbac984ec195dfa06a21103d92403c1e"
WRAPPED_TOKEN = "0xee4c92edb03efdd9b519339edc19ad70c69a9be5"

TRUST_OUTBOUND = "TRUSTLESS_FINALITY"
TRUST_RETURN = "BFT_CHECKPOINT"


# --------------------------------------------------------------------- helpers

def sha32(domain: bytes, payload: bytes) -> str:
    """64 lowercase hex. funding_nonce, subscription_nonce and redemption_nonce
    are all validated at exactly 64 characters; reservation_id wants 96."""
    return sha48(domain, payload)[:64]


def sha48(domain: bytes, payload: bytes) -> str:
    """The chain's hash48: SHA3-384 over domain || 0x00 || payload."""
    h = hashlib.sha3_384()
    h.update(domain)
    h.update(b"\x00")
    h.update(payload)
    return h.hexdigest()


def policy_hash(policy: dict) -> str:
    """Mirror of PftlUniswapPrimaryMarketPolicyV2::computed_hash."""
    preimage = (
        f"policy_epoch={policy['policy_epoch']}\n"
        f"issue_multiplier_bps={policy['issue_multiplier_bps']}\n"
        f"redeem_multiplier_bps={policy['redeem_multiplier_bps']}\n"
        f"issue_capacity_atoms={policy['issue_capacity_atoms']}\n"
        f"redeem_capacity_atoms={policy['redeem_capacity_atoms']}\n"
        f"max_order_atoms={policy['max_order_atoms']}\n"
        f"min_order_atoms={policy['min_order_atoms']}\n"
        f"valid_from_height={policy['valid_from_height']}\n"
        f"expires_at_height={policy['expires_at_height']}\n"
        f"max_nav_age_blocks={policy['max_nav_age_blocks']}\n"
        f"pricing_nav_epoch={policy['pricing_nav_epoch']}\n"
        f"pricing_reserve_packet_hash={policy['pricing_reserve_packet_hash']}\n")
    return sha48(b"postfiat.pftl_uniswap.primary_market_policy.v2",
                 preimage.encode())


def state() -> dict:
    """Prefer the local note, but fall back to the evidence the submitter wrote.

    A crash between a successful submission and the state write used to lose the
    policy epoch and hash, which made later steps unrunnable and made an
    already-advanced epoch look un-advanced. The evidence directory is written by
    pftl.asset_op itself, so it is the authority on what was actually submitted.
    """
    local = json.loads(STATE.read_text()) if STATE.exists() else {}
    if local.get("policy") and local.get("route_epoch"):
        return local
    op = ROOT / "evidence/pftl_uniswap_route_epoch_advance/operation.json"
    if op.exists():
        rec = json.loads(op.read_text())
        policy = rec.get("next_primary_market_policy")
        if policy:
            recovered = {"route_epoch": int(rec["next_route_epoch"]),
                         "policy": policy}
            recovered.update({k: v for k, v in local.items() if k not in recovered})
            return recovered
    return local


def save(**kw) -> None:
    cur = state()
    cur.update(kw)
    STATE.write_text(json.dumps(cur, indent=1, sort_keys=True) + "\n")


def height() -> int:
    return int(pftl.rpc("server_info", {}, "h")["ledger"]["height"])


def nav_asset() -> dict:
    """The read-only peers do not expose `nav_asset`; derive the finalized tuple
    from the newest finalized packet in nav_reserve_proof_status instead."""
    r = pftl.rpc("nav_reserve_proof_status", {"asset_id": NAV_ASSET}, "nav-status")
    if not r.get("found"):
        raise SystemExit("nav asset not found on chain")
    if not r.get("found"):
        raise SystemExit("nav asset not found on chain")
    finalized = [p for p in (r.get("packets") or [])
                 if p.get("state") == "finalized"]
    if not finalized:
        raise SystemExit("nav asset has no finalized packet; nothing can price")
    newest = max(finalized, key=lambda p: int(p.get("epoch") or 0))
    return {
        "finalized_epoch": int(newest["epoch"]),
        "finalized_reserve_packet_hash": newest["reserve_packet_hash"],
        "nav_per_unit": int(newest["nav_per_unit"]),
        "circulating_supply": int(newest["circulating_supply"]),
        "halted": bool(r.get("halted", False)),
        "current_height": int(r.get("current_height") or 0),
    }


def route() -> dict | None:
    for row in pftl.rpc("navcoin_bridge_routes", {}, "routes").get("routes") or []:
        if row.get("route_id") == ROUTE_ID:
            return row
    return None


def line(account: str, asset_id: str) -> int:
    for l in pftl.rpc("account_lines", {"account": account, "limit": 60},
                      "lines").get("lines") or []:
        if l.get("asset_id") == asset_id:
            return int(l.get("balance") or 0)
    return 0


def usd(atoms: int) -> str:
    return f"{atoms / 1_000_000:,.6f}"


# Conditions that mean "try again", not "this operation is wrong". pftl's
# submit_finality rotates peers but aborts on the first error that does not
# mention a proposer, so a momentary busy lock kills a whole nine-step run.
TRANSIENT = (
    "rpc_mempool_mutation_busy",      # another mutation in flight
    "storage_writer_busy",            # redb writer lease contention
    "RPC returned no response",       # peer dropped the connection
    "RPC transport failed",           # ssh/nc failure
    "no proposer accepted",           # every peer was busy this round
    "writer lease deadline exhausted",
)


def submit(label: str, source: str, key: Path, operation: dict,
           attempts: int = 6) -> dict:
    """Submit one operation and refuse to continue unless consensus accepted it.

    Retries only the transient conditions above. A real refusal -- a bad field,
    a policy mismatch, a failed invariant -- is raised on the first attempt so a
    design error is never papered over by repetition.
    """
    for attempt in range(1, attempts + 1):
        try:
            result = pftl.asset_op(label, source, key, operation)
            break
        except RuntimeError as exc:
            text = str(exc)
            if not any(t in text for t in TRANSIENT) or attempt == attempts:
                raise
            wait = min(8 * attempt, 40)
            reason = next(t for t in TRANSIENT if t in text)
            print(f"  {label}: transient ({reason}); retry {attempt}/{attempts - 1} in {wait}s")
            time.sleep(wait)
    codes = pftl.accepted_codes(result)
    if not any(c == "accepted" for c in codes):
        print(f"\n  REFUSED {label}: {codes}")
        print(json.dumps(result, indent=1)[:1500])
        raise SystemExit(f"{label} was not accepted by consensus")
    h = result.get("height") or result.get("finalized_height")
    print(f"  {label:<34} accepted at height {h}")
    return result


def build_policy(policy_epoch: int, valid_from: int, nav: dict) -> dict:
    return {
        "policy_epoch": policy_epoch,
        "issue_multiplier_bps": ISSUE_MULTIPLIER_BPS,
        "redeem_multiplier_bps": REDEEM_MULTIPLIER_BPS,
        "issue_capacity_atoms": 100_000_000,
        "redeem_capacity_atoms": 100_000_000,
        "max_order_atoms": 50_000_000,
        "min_order_atoms": 1_000_000,
        "valid_from_height": valid_from,
        "expires_at_height": valid_from + 500_000,
        "max_nav_age_blocks": 1_000_000,
        "pricing_nav_epoch": int(nav["finalized_epoch"]),
        "pricing_reserve_packet_hash": nav["finalized_reserve_packet_hash"],
    }


def with_hash(policy: dict) -> dict:
    policy = dict(policy)
    policy["policy_hash"] = policy_hash(policy)
    return policy


# ----------------------------------------------------------------------- steps


def bootstrap() -> str:
    """Create a facility-eligible asset and give it a finalized NAV epoch.

    Facility-eligible means: no permanent max supply. Everything else mirrors
    record_deposit.py, including the proof profile and the real broker evidence
    commitments, so this asset is backed by the same attested read.
    """
    global NAV_ASSET
    print("=== 1b. bootstrap (asset + finalized NAV epoch)")
    issued = pftl.rpc("issuer_assets", {"issuer": ISSUER, "limit": 200},
                      "issuer-assets").get("assets") or []
    match = [a for a in issued if a.get("code") == ASSET_CODE
             and a.get("display_name") == ASSET_DISPLAY_NAME]
    if match:
        NAV_ASSET = match[0]["asset_id"]
        print(f"  asset exists            {NAV_ASSET[:24]}...")
    else:
        # max_supply omitted on purpose: a permanent cap disqualifies the asset.
        submit("asset_create (no max supply)", ISSUER, ISSUER_KEY, {
            "operation": "asset_create", "issuer": ISSUER, "code": ASSET_CODE,
            "version": ASSET_VERSION, "precision": ASSET_PRECISION,
            "display_name": ASSET_DISPLAY_NAME,
            "requires_authorization": False, "freeze_enabled": False,
            "clawback_enabled": False})
        issued = pftl.rpc("issuer_assets", {"issuer": ISSUER, "limit": 200},
                          "issuer-assets").get("assets") or []
        match = [a for a in issued if a.get("code") == ASSET_CODE
                 and a.get("display_name") == ASSET_DISPLAY_NAME]
        if len(match) != 1:
            raise SystemExit(f"asset did not resolve uniquely: {match}")
        NAV_ASSET = match[0]["asset_id"]
        print(f"  asset_id                {NAV_ASSET}")

    status = pftl.rpc("nav_reserve_proof_status", {"asset_id": NAV_ASSET}, "s")
    if not status.get("found"):
        submit("nav_asset_register", ISSUER, ISSUER_KEY, {
            "operation": "nav_asset_register", "issuer": ISSUER,
            "asset_id": NAV_ASSET, "reserve_operator": ISSUER,
            "proof_profile": deposit_run.profile_id(),
            "valuation_unit": VALUATION_UNIT, "redemption_account": ISSUER})

    if not line(HOLDER, NAV_ASSET):
        existing = pftl.rpc("account_lines",
                            {"account": HOLDER, "issuer": ISSUER,
                             "asset_id": NAV_ASSET, "limit": 10},
                            "lines").get("lines") or []
        if not existing:
            submit("trust_set (holder)", HOLDER, HOLDER_KEY, {
                "operation": "trust_set", "account": HOLDER, "issuer": ISSUER,
                "asset_id": NAV_ASSET, "limit": 1_000_000_000_000,
                "authorized": False, "frozen": False, "reserve_paid": 10})

    status = pftl.rpc("nav_reserve_proof_status", {"asset_id": NAV_ASSET}, "s")
    finalized = [p for p in (status.get("packets") or [])
                 if p.get("state") == "finalized"]
    if finalized:
        print(f"  nav epoch               {max(int(p['epoch']) for p in finalized)} already finalized")
        return NAV_ASSET

    commitments = json.loads((ROOT / "reserve/commitments.json").read_text())
    packet = commitments["disclosure_commitment"]
    epoch = 1
    submit(f"nav_reserve_submit epoch {epoch}", ISSUER, ISSUER_KEY, {
        "operation": "nav_reserve_submit", "issuer": ISSUER, "submitter": ISSUER,
        "asset_id": NAV_ASSET, "epoch": epoch, "nav_per_unit": NAV_PER_UNIT,
        "circulating_supply": OPENING_CLAIM_ATOMS,
        "verified_net_assets": OPENING_CLAIM_ATOMS,
        "proof_profile": deposit_run.profile_id(),
        "source_root": commitments["quantity_evidence_commitment"],
        "attestor_root": commitments["valuation_evidence_commitment"],
        "reserve_packet_hash": packet, "reserve_accounts": []})
    submit(f"nav_epoch_finalize epoch {epoch}", ISSUER, ISSUER_KEY, {
        "operation": "nav_epoch_finalize", "issuer": ISSUER,
        "asset_id": NAV_ASSET, "epoch": epoch, "reserve_packet_hash": packet})
    submit(f"nav_mint_at_nav {usd(OPENING_CLAIM_ATOMS)} (opening inventory)",
           ISSUER, ISSUER_KEY, {
        "operation": "nav_mint_at_nav", "issuer": ISSUER, "to": HOLDER,
        "asset_id": NAV_ASSET, "amount": OPENING_CLAIM_ATOMS, "epoch": epoch,
        "reserve_packet_hash": packet})
    return NAV_ASSET


def preflight() -> dict:
    print("=== 1. preflight (reads only)")
    info = pftl.rpc("server_info", {}, "si")
    nav = nav_asset()
    print(f"  chain                  {info['chain_id']}  height {info['ledger']['height']}")
    print(f"  nav asset              {NAV_ASSET[:24]}...")
    print(f"  finalized epoch        {nav.get('finalized_epoch')}")
    print(f"  nav per unit           {usd(int(nav.get('nav_per_unit') or 0))}")
    print(f"  circulating supply     {usd(int(nav.get('circulating_supply') or 0))}")
    print(f"  reserve packet         {str(nav.get('finalized_reserve_packet_hash'))[:24]}...")
    print(f"  halted                 {nav.get('halted')}")
    existing = route()
    print(f"  facility route         {'PRESENT' if existing else 'none yet'}")
    print(f"  HOLDER pfUSDC          {usd(line(HOLDER, PFUSDC))}")
    print(f"  HOLDER navcoin         {usd(line(HOLDER, NAV_ASSET))}")
    print(f"  ISSUER navcoin         {usd(line(ISSUER, NAV_ASSET))}")
    if not nav.get("finalized_reserve_packet_hash"):
        raise SystemExit("the asset has no finalized reserve packet; nothing can price")
    return nav


def do_route_init(nav: dict) -> None:
    print("=== 2. route-init (facility registered, live value OFF)")
    if route():
        print("  route already exists; skipping")
        return
    policy = with_hash(build_policy(1, height() + 1, nav))
    op = {
        "operation": "pftl_uniswap_route_init_v2",
        "operator": ISSUER,
        "route_id": ROUTE_ID,
        "route_config_digest": "b2" * 48,
        "native_nav_asset_id": NAV_ASSET,
        "settlement_asset_id": PFUSDC,
        "opening_inventory_atoms": int(nav["circulating_supply"]),
        "opening_inventory_holder": HOLDER,
        "handoff_controller": HANDOFF_CONTROLLER,
        "settlement_adapter": SETTLEMENT_ADAPTER,
        "wrapped_navcoin_token": WRAPPED_TOKEN,
        "ethereum_chain_id": 1,
        "route_supply_cap_atoms": 1_000_000_000_000,
        "packet_notional_cap_atoms": 50_000_000,
        "latest_finalized_nav_epoch": int(nav["finalized_epoch"]),
        "return_finality_blocks": 12,
        "route_epoch": 1,
        "outbound_verification_class": TRUST_OUTBOUND,
        "return_verification_class": TRUST_RETURN,
        "live_value_enabled": False,
        "ethereum_verification_policy": ETH_POLICY,
        "primary_market_policy": policy,
    }
    submit("pftl_uniswap_route_init_v2", ISSUER, ISSUER_KEY, op)
    save(route_epoch=1, policy=policy)


def do_activate(nav: dict) -> None:
    print("=== 3. activate (live value ON, policy valid from this height)")
    r = route()
    if r and (r.get("live_value_enabled") or r.get("route_live")):
        print(f"  route already live (epoch {state().get('route_epoch')}); skipping")
        return
    prior = int(state().get("route_epoch") or 1)
    policy = with_hash(build_policy(prior + 1, height() + 1, nav))
    op = {
        "operation": "pftl_uniswap_route_epoch_advance",
        "operator": ISSUER,
        "route_id": ROUTE_ID,
        "prior_route_epoch": prior,
        "next_route_epoch": prior + 1,
        "next_route_config_digest": "b3" * 48,
        "live_value_enabled": True,
        "next_primary_market_policy": policy,
    }
    submit("pftl_uniswap_route_epoch_advance", ISSUER, ISSUER_KEY, op)
    save(route_epoch=prior + 1, policy=policy)


def do_fund_pool() -> None:
    """RETIRED BY THE CHAIN, kept as documentation of a wrong assumption.

    Submitting this returns `pftl_uniswap_redemption_fund_retired`: "separate
    redemption funding is retired; primary redemption releases
    subscription-funded NAV reserve principal."

    So there is no separately-posted collateral pool. The redeemable reserve IS
    the accumulated subscription principal: a buyer's pfUSDC becomes settlement
    reserve on issue, and redemption releases it. To seed a pool the operator
    subscribes himself -- pays pfUSDC in, receives coins -- which is what the
    A666 primary-market accounting calls subscription-funded reserve. A standing
    pool therefore cannot exist before any coins have been sold.
    """
    print("=== 4. fund-pool -- SKIPPED: retired on chain, reserve comes from issuance")
    return
    print("=== 4. fund-pool (the collateral pool a holder redeems against)")
    nonce = sha32(b"facility.pool.nonce", f"{ROUTE_ID}:{POOL_ATOMS}".encode())
    op = {
        "operation": "pftl_uniswap_redemption_fund",
        "funder": HOLDER,
        "route_id": ROUTE_ID,
        "settlement_asset_id": PFUSDC,
        "funding_nonce": nonce,
        "amount_atoms": POOL_ATOMS,
    }
    submit(f"redemption_fund {usd(POOL_ATOMS)} pfUSDC", HOLDER, HOLDER_KEY, op)
    save(pool_atoms=POOL_ATOMS, pool_nonce=nonce)


def do_issue(nav: dict) -> None:
    print("=== 5. issue (buyer pays pfUSDC, receives new NAVCoin at the fix)")
    policy = state()["policy"]
    epoch = int(state()["route_epoch"])
    mint = ISSUE_SETTLEMENT_ATOMS   # NAV is 1.000000 and the spread is 1.0000x
    rid = sha48(b"facility.reservation", f"{ROUTE_ID}:{mint}:{epoch}".encode())
    reserve_op = {
        "operation": "pftl_uniswap_order_reserve",
        # Omitted deliberately: an explicit source asset must be a source-series
        # child of the route family, not the family asset itself.
        "settlement_source_asset_id": None,
        "subscriber": HOLDER,
        "route_id": ROUTE_ID,
        "reservation_id": rid,
        "ethereum_recipient": "0x0000000000000000000000000000000000000001",
        "route_epoch": epoch,
        "policy_epoch": int(policy["policy_epoch"]),
        "policy_hash": policy["policy_hash"],
        "mint_amount_atoms": mint,
        "max_settlement_value_atoms": ISSUE_SETTLEMENT_ATOMS,
        "expires_at_height": height() + 400,
    }
    try:
        submit("pftl_uniswap_order_reserve", HOLDER, HOLDER_KEY, reserve_op)
    except RuntimeError as exc:
        # The reservation id is deterministic, so a re-run after a crash
        # collides with its own earlier success. That refusal means the
        # reservation is already on chain and the subscription can proceed.
        if "duplicate_pftl_uniswap_reservation" not in str(exc):
            raise
        print("  order_reserve: reservation already on chain; continuing")

    submit(f"primary_subscribe_v2 {usd(mint)}", HOLDER, HOLDER_KEY, {
        "operation": "pftl_uniswap_primary_subscribe_v2",
        "subscriber": HOLDER,
        "route_id": ROUTE_ID,
        "reservation_id": rid,
        "subscription_nonce": sha32(b"facility.subscription", rid.encode()),
        "settlement_asset_id": PFUSDC,
        "settlement_value_atoms": ISSUE_SETTLEMENT_ATOMS,
        "pricing_nav_epoch": int(nav["finalized_epoch"]),
        "pricing_reserve_packet_hash": nav["finalized_reserve_packet_hash"],
    })
    save(issued_atoms=mint, reservation_id=rid)


NATIVE_ISSUE_ATOMS = 3_000_000   # 3.000000 pfUSDC -> 3.000000 B23, PFTL-native


def do_issue_native(nav: dict) -> None:
    """Buy without putting the coins on the Ethereum export path.

    pftl_uniswap_order_reserve requires an `ethereum_recipient`, so the v2
    reservation flow mints an EXPORT ENTITLEMENT: the coins are earmarked for a
    wrapped token, settlement_reserve stays 0, and available_redeem stays 0.
    The v1 subscribe has no reservation and no recipient, so the coins stay
    PFTL-native and the buyer's pfUSDC becomes settlement reserve -- which is
    what a redemption pays out of.

    Price is asserted, not chosen: it must equal the ledger-derived settlement
    atoms per NAV atom, which is 1 when NAV is 1.000000 and both assets are
    precision 6 against the same valuation unit.
    """
    print("=== 5b. issue-native (PFTL-native buy; funds the settlement reserve)")
    nonce = sha32(b"facility.native.subscription",
                  f"{ROUTE_ID}:{NATIVE_ISSUE_ATOMS}:{SALT}".encode())
    submit(f"primary_subscribe {usd(NATIVE_ISSUE_ATOMS)} native",
           HOLDER, HOLDER_KEY, {
        "operation": "pftl_uniswap_primary_subscribe",
        "subscriber": HOLDER,
        "route_id": ROUTE_ID,
        "settlement_asset_id": PFUSDC,
        "subscription_nonce": nonce,
        "settlement_value_atoms": NATIVE_ISSUE_ATOMS,
        "nav_price_settlement_atoms_per_nav_atom": 1,
        "pricing_nav_epoch": int(nav["finalized_epoch"]),
        "pricing_reserve_packet_hash": nav["finalized_reserve_packet_hash"],
    })


def do_redeem(nav: dict) -> None:
    print("=== 6. redeem (NAVCoin retired, pfUSDC released from reserve)")
    policy = state()["policy"]
    submit(f"primary_redeem {usd(REDEEM_NAV_ATOMS)}", HOLDER, HOLDER_KEY, {
        "operation": "pftl_uniswap_primary_redeem",
        "settlement_source_asset_id": None,
        "owner": HOLDER,
        "settlement_recipient": HOLDER,
        "route_id": ROUTE_ID,
        "redemption_nonce": sha32(b"facility.redemption",
                                  f"{ROUTE_ID}:{REDEEM_NAV_ATOMS}:{SALT}".encode()),
        "nav_amount_atoms": REDEEM_NAV_ATOMS,
        "min_settlement_value_atoms": 1,
        "route_epoch": int(state()["route_epoch"]),
        "policy_epoch": int(policy["policy_epoch"]),
        "policy_hash": policy["policy_hash"],
        "pricing_nav_epoch": int(nav["finalized_epoch"]),
        "pricing_reserve_packet_hash": nav["finalized_reserve_packet_hash"],
        "expires_at_height": height() + 400,
    })
    save(redeemed_atoms=REDEEM_NAV_ATOMS)


def do_release(nav: dict) -> None:
    """Release the stray export entitlement from the first (wrong) buy.

    The v2 reservation flow requires an `ethereum_recipient`, so that purchase
    minted an EXPORT ENTITLEMENT -- coins earmarked for a wrapped Ethereum token
    that was never exported, with settlement_reserve left at 0. Releasing it
    returns the reservation so nothing stays earmarked for a bridge leg this
    product does not use.
    """
    print("=== 7. release (return the stray export entitlement)")
    rid = state().get("reservation_id")
    if not rid:
        rid = sha48(b"facility.reservation",
                    f"{ROUTE_ID}:{ISSUE_SETTLEMENT_ATOMS}:{state()['route_epoch']}".encode())
    submit("pftl_uniswap_order_release", HOLDER, HOLDER_KEY, {
        "operation": "pftl_uniswap_order_release",
        "releaser": HOLDER,
        "route_id": ROUTE_ID,
        "reservation_id": rid,
    })


STEPS = {
    "route-init": do_route_init,
    "activate": do_activate,
    "fund-pool": lambda nav: do_fund_pool(),
    "issue": do_issue,
    "issue-native": do_issue_native,
    "redeem": do_redeem,
    "release": do_release,
}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("step", nargs="?", default="all",
                    choices=["all", "preflight", *STEPS])
    args = ap.parse_args()

    bootstrap()
    nav = preflight()
    if args.step == "preflight":
        return 0

    todo = list(STEPS) if args.step == "all" else [args.step]
    pf_before, nc_before = line(HOLDER, PFUSDC), line(HOLDER, NAV_ASSET)
    for name in todo:
        STEPS[name](nav)
        time.sleep(2)

    print("=== end state")
    pf_after, nc_after = line(HOLDER, PFUSDC), line(HOLDER, NAV_ASSET)
    print(f"  HOLDER pfUSDC   {usd(pf_before)} -> {usd(pf_after)}"
          f"   ({pf_after - pf_before:+d} atoms)")
    print(f"  HOLDER navcoin  {usd(nc_before)} -> {usd(nc_after)}"
          f"   ({nc_after - nc_before:+d} atoms)")
    r = route()
    if r:
        print(f"  route live={r.get('route_live')} paused={r.get('paused')}"
              f" supply={usd(int(r.get('authorized_valid_supply_atoms') or 0))}"
              f" subscriptions={r.get('primary_subscription_count')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
