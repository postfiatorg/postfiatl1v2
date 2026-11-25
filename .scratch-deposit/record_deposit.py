#!/usr/bin/env python3
"""Take the next Schwab deposit and put a record of it on the real PFTL chain.

One command. It observes the deposit read-only, applies the coverage test,
finalizes a reserve packet that represents THAT deposit on
postfiat-wan-devnet-2, mints the NAVCoin for it, reads the result back from all
six validators and says in plain English what it did.

It submits ordinary transactions and nothing else. It never resets, re-genesises
or reconfigures the chain, never touches a validator process or unit, never
changes consensus, membership or governance, and never contacts Ethereum.

Secret hygiene: the broker account appears only as a commitment hash; key files
are passed to the signer by path and no key, mnemonic, passphrase or API token
is ever printed.

  record-deposit                      record the newest deposit not yet on chain
  record-deposit --activity-id ID     record that exact deposit
  record-deposit --status             read the chain back and print, change nothing
  record-deposit --coverage-only      run the observation and coverage test only
  record-deposit --coverage-only --claim-override-atoms N   negative control
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
from decimal import Decimal
from pathlib import Path

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("pftl", ROOT / "pftl.py")
pftl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pftl)

# ---- the broker read happens on the credential host, read-only ----
CRED_HOST = "pfrpc@100.85.116.26"
CRED_DIR = "pftl-deposit-readonly"
CRED_PY = "python3"  # mint reader is stdlib-only (tools/nav-reserve-proof/schwab-tee/mint_reader)

# ---- chain identities. Keys are referenced by path and never read here. ----
ISSUER = "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b"
HOLDER = "pfab9b9228942e5c529633a13aa271d5297bec6353"
KEYS = Path("/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private")
ISSUER_KEY = KEYS / "faucet-key.json"
HOLDER_KEY = KEYS / "holder-key.json"

ASSET_CODE = "PFDEP"
ASSET_VERSION = 1
ASSET_PRECISION = 6
ASSET_DISPLAY_NAME = "Post Fiat NAVCoin - Schwab attributed deposits"
ASSET_MAX_SUPPLY = 1_000_000_000_000          # 1,000,000.000000 units of headroom
VALUATION_UNIT = "usd_1e6"
NAV_PER_UNIT = 1_000_000                      # 1.000000 USD per unit
MICRO = 100                                   # broker atoms (1e8/USD) per micro-USD

PROFILE = {
    "verifier_kind": "placeholder",
    "source_class": "yolo-broker-attested-v1",
    "max_snapshot_age_blocks": 1000,
    "challenge_window_blocks": 1,
    "max_epoch_gap_blocks": 128,
    "settle_deadline_blocks": 256,
    "min_challenge_bond": 0,
    "min_attestations": 0,
    "tolerance_bp": 0,
}

LEDGER = ROOT / "recorded-deposits.json"
RESERVE = ROOT / "reserve"
ADAPTER_DIR = ROOT.parent / "tools/nav-reserve-proof"


def policy_hash() -> str:
    """SHA3-384 over the exact admission policy. The on-chain profile commits to
    the coverage rule and to the names of the unreported categories."""
    return hashlib.sha3_384(
        (ROOT / "policy-statement.txt").read_bytes()).hexdigest()


def profile_id() -> str:
    """Mirror of the chain's content-addressed profile id."""
    preimage = (
        f"verifier_kind={PROFILE['verifier_kind']}\n"
        f"source_class={PROFILE['source_class']}\n"
        f"max_snapshot_age_blocks={PROFILE['max_snapshot_age_blocks']}\n"
        f"challenge_window_blocks={PROFILE['challenge_window_blocks']}\n"
        f"max_epoch_gap_blocks={PROFILE['max_epoch_gap_blocks']}\n"
        f"settle_deadline_blocks={PROFILE['settle_deadline_blocks']}\n"
        f"min_challenge_bond={PROFILE['min_challenge_bond']}\n"
        f"min_attestations={PROFILE['min_attestations']}\n"
        f"tolerance_bp={PROFILE['tolerance_bp']}\n"
        f"valuation_policy_hash={policy_hash()}\n"
        "sp1_program_vkey=\nsp1_proof_encoding=\nmax_proof_bytes=0\n"
        "max_public_values_bytes=0\n")
    h = hashlib.sha3_384()
    h.update(b"postfiat.nav_proof_profile_id.v1")
    h.update(b"\x00")
    h.update(preimage.encode())
    return h.hexdigest()


def ledger_read() -> dict:
    if LEDGER.exists():
        return json.loads(LEDGER.read_text())
    return {"asset_id": None, "deposits": []}


def ledger_write(value: dict) -> None:
    LEDGER.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def usd(atoms: int) -> str:
    return f"{Decimal(atoms) / 100_000_000:,.2f}"


def micro_usd(micro: int) -> str:
    return f"{Decimal(micro) / 1_000_000:,.2f}"


# ------------------------------------------------------------------ chain reads

def nav_state(asset_id: str, peer=pftl.PEERS[0]) -> dict:
    """Read the NAV asset's reserve state back off the chain.

    `nav_reserve_proof_status` exposes the finalized reserve packet rather than
    the nav-asset struct. That packet is where the asset's finalized_epoch,
    circulating_supply and finalized_reserve_packet_hash come from:
    nav_epoch_finalize copies them off the packet it finalizes, and every mint
    is gated against them. Returns None if the NAV asset is not registered."""
    report = pftl.rpc("nav_reserve_proof_status", {"asset_id": asset_id},
                      "nav-status", peer)
    if not report.get("found") or not report.get("active_profile"):
        return None
    finalized = [p for p in report.get("packets", []) if p.get("state") == "finalized"]
    latest = max(finalized, key=lambda p: int(p["epoch"])) if finalized else None
    return {"report": report, "packet": latest,
            "finalized_epoch": int(latest["epoch"]) if latest else 0,
            "circulating_supply": int(latest["circulating_supply"]) if latest else 0,
            "verified_net_assets": int(latest["verified_net_assets"]) if latest else 0,
            "finalized_reserve_packet_hash":
                latest["reserve_packet_hash"] if latest else ""}


def asset_exists(asset_id: str) -> bool:
    if not asset_id:
        return False
    return bool(pftl.rpc("asset_info", {"asset_id": asset_id}, "asset-info").get("found"))


def holder_balance(asset_id: str) -> int:
    lines = pftl.rpc("account_lines",
                     {"account": HOLDER, "issuer": ISSUER, "asset_id": asset_id,
                      "limit": 10}, "holder-lines").get("lines") or []
    return int(lines[0]["balance"]) if lines else 0


# ------------------------------------------------------------- the broker read

def observe(activity_id: str | None, prior_claim_atoms: int,
            claim_override_atoms: int | None, out_name: str) -> tuple[int, str, str]:
    """Run the read-only collector on the credential host and bring back only the
    public artifacts. Returns (exit code, stdout, stderr)."""
    argv = [f"{CRED_PY} {CRED_DIR}/observe_deposit.py",
            f"--out-dir ~/{CRED_DIR}/{out_name}",
            f"--prior-claim-atoms {prior_claim_atoms}"]
    if activity_id:
        argv.append(f"--activity-id {activity_id}")
    if claim_override_atoms is not None:
        argv.append(f"--claim-override-atoms {claim_override_atoms}")
    completed = subprocess.run(
        ["ssh", "-o", "BatchMode=yes", CRED_HOST, " ".join(argv)],
        text=True, capture_output=True, timeout=900, check=False)
    return completed.returncode, completed.stdout, completed.stderr


def fetch_artifacts(out_name: str) -> None:
    RESERVE.mkdir(parents=True, exist_ok=True)
    for name in ("disclosure.json", "observation.public.json",
                 "attributed-deposit.json", "coverage-test.json"):
        subprocess.run(["scp", "-q", "-o", "BatchMode=yes",
                        f"{CRED_HOST}:{CRED_DIR}/{out_name}/{name}",
                        str(RESERVE / name)], timeout=180, check=True)


def commitments(gross_micro: int, liabilities_micro: int) -> dict:
    """Derive the three on-chain commitments with the shipped adapter."""
    out = RESERVE / "commitments.json"
    if out.exists():
        out.unlink()          # the adapter refuses to overwrite its own output
    subprocess.run(
        ["cargo", "run", "--locked", "-q", "-p", "postfiat-reserve-proof", "--",
         "adapter", "yolo-broker-commitments",
         "--disclosure", str(RESERVE / "disclosure.json"),
         "--gross-assets", str(gross_micro),
         "--total-liabilities", str(liabilities_micro),
         "--valuation-scale", "1000000",
         "--output", str(out)],
        cwd=ADAPTER_DIR, timeout=1800, check=True, capture_output=True, text=True)
    return json.loads(out.read_text())


# ------------------------------------------------------------------- bootstrap

def bootstrap(state: dict) -> str:
    """Register the asset, the proof profile and the NAV asset, and open the
    holder's trustline. Ordinary asset operations signed by the issuer -- the
    same path a651 and a666 used on this chain. No governance, no membership
    change, no validator is touched. Idempotent: each step is skipped if the
    chain already has it."""
    asset_id = state.get("asset_id")
    if not asset_exists(asset_id):
        print("  registering the proof profile (verifier_kind=placeholder, "
              "source_class=yolo-broker-attested-v1)")
        op = dict(PROFILE)
        op.update({"operation": "nav_profile_register", "registrant": ISSUER,
                   "valuation_policy_hash": policy_hash()})
        try:
            pftl.asset_op("01-nav-profile-register", ISSUER, ISSUER_KEY, op)
        except RuntimeError as exc:
            if "already" not in str(exc).lower():
                raise
            print("    profile already registered")

        print(f"  creating the asset {ASSET_CODE} v{ASSET_VERSION}")
        pftl.asset_op("02-asset-create", ISSUER, ISSUER_KEY, {
            "operation": "asset_create", "issuer": ISSUER, "code": ASSET_CODE,
            "version": ASSET_VERSION, "precision": ASSET_PRECISION,
            "display_name": ASSET_DISPLAY_NAME, "max_supply": ASSET_MAX_SUPPLY,
            "requires_authorization": False, "freeze_enabled": False,
            "clawback_enabled": False})
        issued = pftl.rpc("issuer_assets", {"issuer": ISSUER, "limit": 200},
                          "resolve-asset")
        matches = [r for r in issued.get("assets", [])
                   if r.get("code") == ASSET_CODE
                   and r.get("display_name") == ASSET_DISPLAY_NAME]
        if len(matches) != 1:
            raise RuntimeError(f"asset did not resolve uniquely: {matches}")
        asset_id = matches[0]["asset_id"]
        state["asset_id"] = asset_id
        ledger_write(state)
        print(f"    asset_id {asset_id}")

    if nav_state(asset_id) is None:
        print("  registering the NAV asset")
        pftl.asset_op("03-nav-asset-register", ISSUER, ISSUER_KEY, {
            "operation": "nav_asset_register", "issuer": ISSUER, "asset_id": asset_id,
            "reserve_operator": ISSUER, "proof_profile": profile_id(),
            "valuation_unit": VALUATION_UNIT, "redemption_account": ISSUER})

    lines = pftl.rpc("account_lines",
                     {"account": HOLDER, "issuer": ISSUER, "asset_id": asset_id,
                      "limit": 10}, "bootstrap-lines").get("lines") or []
    if not lines:
        print("  opening the holder's trustline")
        pftl.asset_op("04-holder-trust", HOLDER, HOLDER_KEY, {
            "operation": "trust_set", "account": HOLDER, "issuer": ISSUER,
            "asset_id": asset_id, "limit": ASSET_MAX_SUPPLY,
            "authorized": False, "frozen": False, "reserve_paid": 10})
    return asset_id


# ---------------------------------------------------------------- the read-back

def read_back(asset_id: str, expect_packet: str | None = None,
              mint_tx_id: str | None = None, avoid_peer: str | None = None) -> dict:
    """Verify from outside the submitting process: re-read the chain and show the
    fields that prove the record is there and that the receipt was ACCEPTED, not
    merely committed. Reads from a validator that did NOT propose the block."""
    fleet = pftl.fleet_snapshot("readback")
    pftl.assert_identity(fleet)
    point = pftl.converged_point(fleet)
    peer = next((p for p in pftl.PEERS if p[0] != avoid_peer), pftl.PEERS[0])
    state = nav_state(asset_id, peer)
    if state is None:
        raise RuntimeError("NAV asset is not on the chain")
    balance = holder_balance(asset_id)

    print("--- read back from the chain in a fresh query, from "
          f"{peer[0]} (not the proposer) ---")
    print(f"  chain_id                        {fleet['validator-0']['chain_id']}")
    print(f"  genesis_hash                    {fleet['validator-0']['genesis_hash']}")
    print(f"  all six validators converged    {point is not None} at height "
          f"{point[0] if point else '?'}")
    print(f"  asset_id                        {asset_id}")
    print(f"  finalized_epoch                 {state['finalized_epoch']}")
    print(f"  circulating_supply              {state['circulating_supply']} "
          f"(= {micro_usd(state['circulating_supply'])} USD at NAV 1.000000)")
    print(f"  verified_net_assets             {state['verified_net_assets']}")
    print(f"  finalized_reserve_packet_hash   {state['finalized_reserve_packet_hash']}")
    if state["packet"]:
        print(f"  packet state                    {state['packet']['state']} "
              f"(packet_id {state['packet']['packet_id']}, submitted at height "
              f"{state['packet']['submitted_at_height']})")
    print(f"  holder balance                  {balance} "
          f"(= {micro_usd(balance)} NAVCoin units) for {HOLDER}")

    receipt = None
    if mint_tx_id:
        log = pftl.rpc("receipts", {"tx_id": mint_tx_id, "limit": 4},
                       "readback-receipt", peer)
        rows = log if isinstance(log, list) else (
            log.get("receipts") or log.get("entries") or [])
        receipt = next((r for r in rows if r.get("tx_id") == mint_tx_id), None)
        if receipt is None:
            raise RuntimeError(f"mint receipt {mint_tx_id} not found on chain")
        print(f"  mint receipt tx_id              {receipt.get('tx_id')}")
        print(f"  mint receipt accepted           {receipt.get('accepted')}  "
              f"code={receipt.get('error_code') or receipt.get('code') or 'accepted'}")
        if receipt.get("accepted") is not True:
            raise RuntimeError("the mint receipt committed but was NOT accepted: "
                               f"{receipt}")

    if expect_packet:
        match = state["finalized_reserve_packet_hash"] == expect_packet
        print(f"  packet hash is this run's       {match}")
        if not match:
            raise RuntimeError("the chain's finalized packet hash is not this run's packet")
    return {"fleet": fleet, "nav": state, "holder_balance": balance,
            "mint_receipt": receipt, "read_from": peer[0]}


# --------------------------------------------------------------------- the run

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--activity-id", default=None)
    ap.add_argument("--status", action="store_true",
                    help="read the chain back and print; change nothing")
    ap.add_argument("--coverage-only", action="store_true",
                    help="run the broker observation and the coverage test only")
    ap.add_argument("--claim-override-atoms", type=int, default=None,
                    help="negative control for --coverage-only")
    args = ap.parse_args()

    state = ledger_read()

    if args.status:
        if not state.get("asset_id"):
            print("nothing has been recorded yet")
            return 0
        last = state["deposits"][-1] if state["deposits"] else {}
        read_back(state["asset_id"], expect_packet=last.get("reserve_packet_hash"),
                  mint_tx_id=last.get("mint_tx_id"))
        print("\nDeposits recorded on chain:")
        for d in state["deposits"]:
            print(f"  activityId {d['activity_id']}  {usd(d['amount_atoms'])} USD  "
                  f"posted {d['posted_at_utc']}  epoch {d['epoch']}  "
                  f"mint at height {d['mint_height']}")
        return 0

    print("Checking the chain is the live six-validator network and is converged.")
    fleet = pftl.fleet_snapshot("preflight")
    pftl.assert_identity(fleet)
    point = pftl.converged_point(fleet)
    if point is None or any(int(r["mempool"]["pending"]) for r in fleet.values()):
        print("STOPPED: the fleet is not converged with an empty mempool. Nothing "
              "was submitted.", file=sys.stderr)
        return 1
    print(f"  live chain postfiat-wan-devnet-2, all six validators at height {point[0]}.")

    # What is already on chain decides what this deposit adds to.
    asset_id = state.get("asset_id")
    prior_claim_micro = 0
    if asset_id and asset_exists(asset_id):
        existing = nav_state(asset_id)
        if existing:
            prior_claim_micro = existing["circulating_supply"]
    prior_claim_atoms = prior_claim_micro * MICRO

    if args.coverage_only:
        print("\nObserving the brokerage account read-only and applying the coverage test.")
        rc, out, err = observe(args.activity_id, prior_claim_atoms,
                               args.claim_override_atoms, "run-coverage")
        print(out)
        if err:
            print(err, file=sys.stderr)
        return rc

    print("\nObserving the brokerage account read-only and applying the coverage test.")
    rc, out, err = observe(args.activity_id, prior_claim_atoms, None, "run-record")
    print(out)
    if rc != 0:
        print(err, file=sys.stderr)
        print("\nSTOPPED before touching the chain. No transaction was submitted.",
              file=sys.stderr)
        return rc

    fetch_artifacts("run-record")
    observation = json.loads((RESERVE / "observation.public.json").read_text())
    deposit = observation["attributed_deposit"]
    coverage = observation["coverage_test"]

    already = [d for d in state["deposits"]
               if d["activity_id"] == deposit["deposit_activity_id"]]
    if already:
        print(f"\nDeposit {deposit['deposit_activity_id']} is already on the chain "
              f"(epoch {already[0]['epoch']}). Nothing to do.")
        read_back(asset_id)
        return 0

    gross_micro = observation["gross_assets_atoms"] // MICRO
    liabilities_micro = observation["adapter_total_liabilities_atoms"] // MICRO
    claim_micro = observation["claim_atoms"] // MICRO
    deposit_micro = deposit["deposit_amount_atoms"] // MICRO

    print("Deriving the reserve-packet commitments with the shipped adapter.")
    c = commitments(gross_micro, liabilities_micro)
    packet_hash = c["disclosure_commitment"]
    print(f"  reserve_packet_hash {packet_hash}")

    print("\nBootstrapping anything the chain does not have yet.")
    asset_id = bootstrap(state)

    current = nav_state(asset_id)
    epoch = current["finalized_epoch"] + 1

    print(f"\nFinalizing the packet for this deposit on chain as epoch {epoch}.")
    submit = pftl.asset_op(f"05-reserve-submit-epoch{epoch}", ISSUER, ISSUER_KEY, {
        "operation": "nav_reserve_submit", "issuer": ISSUER, "submitter": ISSUER,
        "asset_id": asset_id, "epoch": epoch, "nav_per_unit": NAV_PER_UNIT,
        "circulating_supply": claim_micro, "verified_net_assets": claim_micro,
        "proof_profile": profile_id(),
        "source_root": c["quantity_evidence_commitment"],
        "attestor_root": c["valuation_evidence_commitment"],
        "reserve_packet_hash": packet_hash, "reserve_accounts": []})
    print(f"  nav_reserve_submit   height {submit['height']}  "
          f"receipt {submit['receipt_codes']}  proposer {submit['proposer']}")

    finalize = pftl.asset_op(f"06-epoch-finalize-epoch{epoch}", ISSUER, ISSUER_KEY, {
        "operation": "nav_epoch_finalize", "issuer": ISSUER, "asset_id": asset_id,
        "epoch": epoch, "reserve_packet_hash": packet_hash})
    print(f"  nav_epoch_finalize   height {finalize['height']}  "
          f"receipt {finalize['receipt_codes']}  proposer {finalize['proposer']}")

    print(f"\nMinting the NAVCoin for this deposit against that exact packet hash.")
    mint = pftl.asset_op(f"07-nav-mint-epoch{epoch}", ISSUER, ISSUER_KEY, {
        "operation": "nav_mint_at_nav", "issuer": ISSUER, "to": HOLDER,
        "asset_id": asset_id, "amount": deposit_micro, "epoch": epoch,
        "reserve_packet_hash": packet_hash})
    print(f"  nav_mint_at_nav      height {mint['height']}  "
          f"receipt {mint['receipt_codes']}  proposer {mint['proposer']}")

    state["deposits"].append({
        "activity_id": deposit["deposit_activity_id"],
        "amount_atoms": deposit["deposit_amount_atoms"],
        "posted_at_utc": deposit["deposit_posted_at_utc"],
        "epoch": epoch, "reserve_packet_hash": packet_hash,
        "reserve_submit_height": submit["height"],
        "finalize_height": finalize["height"], "mint_height": mint["height"],
        "coverage_multiple": coverage["coverage_multiple"],
        "reserve_submit_tx_id": submit["result"]["tx_id"],
        "finalize_tx_id": finalize["result"]["tx_id"],
        "mint_tx_id": mint["result"]["tx_id"],
        "account_commitment_sha256": deposit["account_commitment_sha256"]})
    ledger_write(state)

    print()
    result = read_back(asset_id, expect_packet=packet_hash,
                       mint_tx_id=mint["result"]["tx_id"],
                       avoid_peer=mint["proposer"])

    print(f"""
WHAT THIS DID, IN PLAIN ENGLISH

  A wire of {usd(deposit['deposit_amount_atoms'])} USD landed in your Schwab account on
  {deposit['deposit_posted_at_utc']}. The broker calls it {deposit['deposit_type']}, status
  {deposit['deposit_status']}, activityId {deposit['deposit_activity_id']}. That one
  transaction -- not a balance change -- is what this record represents.

  The claim of {usd(deposit['deposit_amount_atoms'])} USD is covered {coverage['coverage_multiple']}x
  by {usd(coverage['verified_net_equity_atoms'])} USD of verified net equity, which
  reconciles to Schwab's own liquidationValue to the cent. Four liability
  categories Schwab's API does not report are named in the packet and given no
  number; at this coverage they would have to exceed
  {usd(coverage['unreported_total_would_have_to_exceed_atoms'])} USD to matter.

  A packet committing to that deposit was finalized on the live Post Fiat chain
  at height {finalize['height']} as epoch {epoch}, and {micro_usd(deposit_micro)} NAVCoin
  were minted against that exact packet at height {mint['height']}. Every receipt came
  back literally 'accepted', and all six validators agree on the result.

  Run this again after your next deposit and it will record that one as epoch
  {epoch + 1}. Nothing on the chain was reset, reconfigured or governed; only
  ordinary transactions were sent.
""")
    return 0


if __name__ == "__main__":
    sys.exit(main())
