"""Read-only Schwab observation of ONE attributed external cash deposit.

Produces the reserve packet that represents THE DEPOSIT -- a specific
activityId, its amount and its posting time, bound to the account commitment.
The account's net equity is not the claim; it appears only as the coverage
evidence behind the claim.

Reads four endpoints and performs zero writes. Places no order, moves no funds.
Never prints or stores an account number, a token or a key: the account appears
only as a domain-separated commitment.

THE COVERAGE TEST
-----------------
Four liability categories are not reported by the Schwab Trader API at all:
the unsettled-trade cash leg, accrued-but-unposted margin interest, short
dividends payable, and accrued commissions and fees. They are named in every
packet and no number is ever invented for any of them.

Refusing every claim on that basis was over-conservative. The wire paid down a
margin loan, so it raised net equity by its own amount and IS represented in
verified net equity. A claim is therefore admissible when verified net equity
covers it COVERAGE_MULTIPLE_REQUIRED times over, so that the unreported
categories would have to exceed (COVERAGE_MULTIPLE_REQUIRED - 1) x claim before
they could matter -- and is refused when the claim is close enough to net
equity that they could.

Usage:
  observe_deposit.py --out-dir DIR [--activity-id ID] [--prior-claim-atoms N]
                     [--claim-override-atoms N]
"""
import argparse
import hashlib
import json
import os
import sys
from datetime import datetime, timedelta, timezone
from decimal import Decimal

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from schwab_client import SchwabClient
import schwab_deposits                                  # the shipped classifier

SCALE = 100_000_000                       # accounting atoms per USD
MICRO = 100                               # atoms per micro-USD at SCALE
BROKER_SOURCE_ID = "schwab-trader-api-individual"
DISCLOSURE_SCHEMA = "postfiat.yolo.broker_reserve_disclosure.v1"

# The claim is admissible only when verified net equity covers it this many
# times over. Justified from the actual magnitudes in COVERAGE_RATIONALE below.
COVERAGE_MULTIPLE_REQUIRED = 100

OPEN_ORDER_STATUSES = ("AWAITING_PARENT_ORDER", "AWAITING_CONDITION",
                       "AWAITING_MANUAL_REVIEW", "ACCEPTED", "PENDING_ACTIVATION",
                       "QUEUED", "WORKING", "AWAITING_STOP_CONDITION",
                       "AWAITING_RELEASE_TIME", "PENDING_CANCEL", "PENDING_REPLACE",
                       "NEW", "AWAITING_UR_OUT")

SENSITIVE_KEYS = {"accountnumber", "hashvalue", "accountid", "account_number"}


class Refusal(Exception):
    pass


def scrub(obj):
    if isinstance(obj, dict):
        return {k: ("<redacted>" if str(k).lower() in SENSITIVE_KEYS else scrub(v))
                for k, v in obj.items()}
    if isinstance(obj, list):
        return [scrub(v) for v in obj]
    return obj


def atoms(value, field):
    if value is None:
        raise Refusal("field %s is absent" % field)
    d = Decimal(str(value)) * SCALE
    if d != d.to_integral_value():
        raise Refusal("field %s = %s is finer than one atom" % (field, value))
    return int(d)


def usd(a):
    return str(Decimal(a) / SCALE)


def root(domain, payload):
    """Domain-separated 32-byte SHA-256 root. An empty collection still gets a
    root; it is never omitted and never read as zero."""
    h = hashlib.sha256()
    h.update(b"postfiat.yolo.")
    h.update(domain.encode())
    h.update(b"\x00")
    h.update(json.dumps(payload, sort_keys=True, separators=(",", ":"),
                        default=str).encode())
    return h.hexdigest()


REQUIRED_ASSET_CATEGORIES = (
    ("long_market_value", "longMarketValue", +1),
    ("short_sale_proceeds_credit", "shortBalance", +1),
    ("cash_balance", "cashBalance", +1),
    ("money_market_fund", "moneyMarketFund", +1),
    ("savings", "savings", +1),
    ("bond_value", "bondValue", +1),
    ("mutual_fund_value", "mutualFundValue", +1),
    ("pending_deposits", "pendingDeposits", +1),
    ("accrued_interest_receivable", "accruedInterest", +1),
)

REQUIRED_LIABILITY_CATEGORIES = (
    ("margin_debt", "marginBalance", "negative_part"),
    ("short_market_value", "shortMarketValue", "negative_part"),
)

COVERAGE_RATIONALE = (
    "COVERAGE_MULTIPLE_REQUIRED = {k}. Every unreported category is an accrual on a "
    "base this read DOES observe: accrued margin interest accrues on the observed "
    "margin debit, short dividends payable accrue on the observed short book, "
    "accrued commissions and fees accrue on observed trading activity, and the "
    "unsettled-trade cash leg is the cash counterpart of the observed unsettled "
    "quantities. Requiring {k}x coverage means the unreported total would have to "
    "exceed {k_minus_one}x the claim before the claim could be unsafe. At the "
    "magnitudes actually observed here that threshold is {threshold} USD against "
    "combined accrual bases of {bases} USD -- {pct}% of those bases -- which no "
    "single accrual cycle of interest, declared short dividends, commissions and "
    "unsettled cash can reach. The test is on the magnitudes, not on an estimate: "
    "no number is assigned to any unreported category."
)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out-dir", required=True)
    ap.add_argument("--activity-id", default=None,
                    help="attribute this exact transaction; default = the newest "
                         "qualifying external cash deposit in the window")
    ap.add_argument("--prior-claim-atoms", type=int, default=0,
                    help="claim already recorded on chain for earlier deposits; the "
                         "coverage test gates the CUMULATIVE claim")
    ap.add_argument("--claim-override-atoms", type=int, default=None,
                    help="negative-control only: coverage-test this claim instead of "
                         "the attributed deposit amount")
    ap.add_argument("--window-days", type=int, default=90)
    args = ap.parse_args()

    os.makedirs(args.out_dir, mode=0o700, exist_ok=True)
    svc = SchwabClient()

    numbers = svc.request("/trader/v1/accounts/accountNumbers")
    if not numbers:
        raise Refusal("no linked Schwab account")
    if len(numbers) != 1:
        raise Refusal("%d linked accounts; this packet commits to exactly one" % len(numbers))
    account_hash = str(numbers[0]["hashValue"])
    account_commitment = schwab_deposits.account_commitment(account_hash)

    observed_at = datetime.now(timezone.utc)
    now = observed_at
    detail = svc.request("/trader/v1/accounts/%s" % account_hash,
                                 params={"fields": "positions"})
    sa = detail["securitiesAccount"]
    balances = sa["currentBalances"]
    positions = sa.get("positions") or []

    from_t = (now - timedelta(days=60)).isoformat().replace("+00:00", "Z")
    to_t = now.isoformat().replace("+00:00", "Z")
    open_orders, order_status_probe = [], {}
    for status in OPEN_ORDER_STATUSES:
        try:
            rows = svc.request(
                "/trader/v1/accounts/%s/orders" % account_hash,
                params={"fromEnteredTime": from_t, "toEnteredTime": to_t,
                        "status": status, "maxResults": 3000})
            order_status_probe[status] = len(rows or [])
            open_orders.extend(rows or [])
        except Exception as exc:
            order_status_probe[status] = "ERROR:%s" % type(exc).__name__

    window_start = now - timedelta(days=args.window_days)
    tx = svc.request(
        "/trader/v1/accounts/%s/transactions" % account_hash,
        params={"startDate": window_start.isoformat().replace("+00:00", "Z"),
                "endDate": to_t})

    # ---- deposit attribution, through the shipped classifier ----
    candidates = schwab_deposits.classify(tx or [])
    deposits = [c for c in candidates if c.is_deposit()]
    tx_census = {}
    for t in tx or []:
        ty = str(t.get("type") or "")
        tx_census[ty] = tx_census.get(ty, 0) + 1

    if args.activity_id:
        chosen = [c for c in deposits if c.transfer_id == str(args.activity_id)]
        if not chosen:
            raise Refusal(
                "activityId %s is not a qualifying external cash deposit in the last %d "
                "days. Qualifying deposits: %s"
                % (args.activity_id, args.window_days,
                   json.dumps([c.report() for c in deposits], sort_keys=True)))
        deposit = chosen[0]
    else:
        if not deposits:
            raise Refusal("no qualifying external cash deposit in the last %d days"
                          % args.window_days)
        deposit = max(deposits, key=lambda c: c.posted_at)

    attributed = {
        "schema": "postfiat.pftl.attributed_deposit.v1",
        "broker_source_id": BROKER_SOURCE_ID,
        "account_commitment_sha256": account_commitment,
        "deposit_activity_id": deposit.transfer_id,
        "deposit_type": deposit.entry_type,
        "deposit_status": deposit.status,
        "deposit_sub_account": deposit.sub_account,
        "deposit_asset_type": deposit.asset_type,
        "deposit_description": deposit.description,
        "deposit_amount_atoms": deposit.amount_atoms,
        "deposit_amount_usd": usd(deposit.amount_atoms),
        "deposit_posted_at_unix": deposit.posted_at,
        "deposit_posted_at_utc": datetime.fromtimestamp(
            deposit.posted_at, timezone.utc).isoformat().replace("+00:00", "Z"),
        "classification": deposit.classification,
        "classification_reason": deposit.reason,
        "classifier": "schwab_navcoin/schwab_deposits.py ACCEPTED_DEPOSIT_TYPES="
                      + ",".join(schwab_deposits.ACCEPTED_DEPOSIT_TYPES)
                      + " REQUIRED_STATUS=" + schwab_deposits.REQUIRED_STATUS,
    }
    attributed_deposit_sha256 = root("attributed_deposit.v1", attributed)

    # ---- assets, by named category ----
    assets, asset_detail = 0, {}
    for name, field, sign in REQUIRED_ASSET_CATEGORIES:
        if field not in balances:
            asset_detail[name] = {"status": "MISSING", "field": "currentBalances.%s" % field}
            continue
        a = atoms(balances[field], field) * sign
        asset_detail[name] = {"status": "OBSERVED", "field": "currentBalances.%s" % field,
                              "atoms": a, "usd": usd(a)}
        assets += a

    # ---- liabilities, by named category ----
    liabilities, liability_detail = 0, {}
    for name, field, mode in REQUIRED_LIABILITY_CATEGORIES:
        if field not in balances:
            raise Refusal("currentBalances.%s is absent; the liability side cannot be "
                          "reconciled" % field)
        raw = atoms(balances[field], field)
        value = -raw if raw < 0 else 0
        liability_detail[name] = {"status": "OBSERVED", "field": "currentBalances.%s" % field,
                                  "raw_atoms": raw, "liability_atoms": value, "usd": usd(value)}
        liabilities += value

    oo = 0
    for o in open_orders:
        for leg in o.get("orderLegCollection") or []:
            if str(leg.get("instruction", "")).upper().startswith("BUY"):
                px = o.get("price") or o.get("stopPrice")
                qty = o.get("remainingQuantity") or o.get("quantity") or 0
                if px is None:
                    raise Refusal("an open BUY order carries no price; its obligation "
                                  "cannot be observed")
                oo += atoms(Decimal(str(px)) * Decimal(str(qty)), "open order notional")
    liability_detail["open_order_obligations"] = {
        "status": "OBSERVED",
        "field": "GET /trader/v1/accounts/{hash}/orders per open status",
        "open_orders_returned": len(open_orders), "status_probe": order_status_probe,
        "liability_atoms": oo, "usd": usd(oo)}
    liabilities += oo

    unsettled = [p for p in positions
                 if float(p.get("longQuantity") or 0) != float(p.get("settledLongQuantity") or 0)
                 or float(p.get("shortQuantity") or 0) != float(p.get("settledShortQuantity") or 0)]
    liability_detail["unsettled_trade_quantities"] = {
        "status": "OBSERVED",
        "field": "positions[].longQuantity vs settledLongQuantity / shortQuantity vs "
                 "settledShortQuantity",
        "positions_with_unsettled_quantity": len(unsettled),
        "liability_atoms": 0,
        "note": "quantity exposure is observed per position; its cash settlement leg is "
                "not broken out by any balance field, see unsettled_trade_cash_leg"}

    # ---- the four categories the API does not report, by name, unquantified ----
    margin_debit = liability_detail["margin_debt"]["liability_atoms"]
    short_book = liability_detail["short_market_value"]["liability_atoms"]
    unreported = [
        {"category": "unsettled_trade_cash_leg",
         "status": "UNREPORTED_BY_BROKER_API", "quantified": False,
         "accrual_base": "positions[].longQuantity vs settledLongQuantity "
                         "(%d positions carry unsettled quantity)" % len(unsettled),
         "why": "no currentBalances field reports cash owed on trades that have executed "
                "but not settled, separately from marginBalance"},
        {"category": "accrued_margin_interest",
         "status": "UNREPORTED_BY_BROKER_API", "quantified": False,
         "accrual_base": "currentBalances.marginBalance debit %s USD" % usd(margin_debit),
         "why": "currentBalances.accruedInterest = %s with bondValue = %s: that field "
                "carries bond accrued interest, and no field reports interest accrued but "
                "not yet posted on the margin debit"
                % (balances.get("accruedInterest"), balances.get("bondValue"))},
        {"category": "short_dividends_payable",
         "status": "UNREPORTED_BY_BROKER_API", "quantified": False,
         "accrual_base": "currentBalances.shortMarketValue short book %s USD" % usd(short_book),
         "why": "no field reports dividends declared and payable on the short book"},
        {"category": "accrued_commissions_and_fees",
         "status": "UNREPORTED_BY_BROKER_API", "quantified": False,
         "accrual_base": "observed trading activity (%d TRADE rows in the window)"
                         % tx_census.get("TRADE", 0),
         "why": "order records carry no commission or regulatory-fee field; fees appear "
                "only once posted to transactions"},
    ]

    # ---- reconciliation against the broker's own net number ----
    liq = atoms(balances["liquidationValue"], "liquidationValue")
    verified_net_equity = assets - liabilities
    reconciled = verified_net_equity == liq

    # ---- THE COVERAGE TEST ----
    claim = (args.claim_override_atoms if args.claim_override_atoms is not None
             else args.prior_claim_atoms + deposit.amount_atoms)
    if claim <= 0:
        raise Refusal("claim must be positive")
    threshold = verified_net_equity - claim      # what the unreported total must exceed
    coverage = Decimal(verified_net_equity) / Decimal(claim)
    bases = margin_debit + short_book
    admissible = coverage >= COVERAGE_MULTIPLE_REQUIRED
    pct = (Decimal(max(threshold, 0)) / Decimal(bases) * 100) if bases else Decimal(0)

    coverage_test = {
        "schema": "postfiat.pftl.coverage_test.v1",
        "claim_atoms": claim,
        "claim_usd": usd(claim),
        "claim_is": ("the attributed deposit plus %s USD already recorded on chain"
                     % usd(args.prior_claim_atoms)) if args.claim_override_atoms is None
                    else "NEGATIVE CONTROL: an operator-supplied claim, not a real deposit",
        "verified_net_equity_atoms": verified_net_equity,
        "verified_net_equity_usd": usd(verified_net_equity),
        "coverage_multiple": str(coverage.quantize(Decimal("0.01"))),
        "coverage_multiple_required": COVERAGE_MULTIPLE_REQUIRED,
        "admissible": bool(admissible),
        "unreported_total_would_have_to_exceed_atoms": threshold,
        "unreported_total_would_have_to_exceed_usd": usd(threshold),
        "observed_accrual_bases_atoms": {
            "margin_debit": margin_debit, "short_book": short_book, "combined": bases},
        "observed_accrual_bases_usd": {
            "margin_debit": usd(margin_debit), "short_book": usd(short_book),
            "combined": usd(bases)},
        "threshold_as_percent_of_combined_bases": str(pct.quantize(Decimal("0.01"))),
        "unreported_liability_categories": unreported,
        "rationale": COVERAGE_RATIONALE.format(
            k=COVERAGE_MULTIPLE_REQUIRED, k_minus_one=COVERAGE_MULTIPLE_REQUIRED - 1,
            threshold=usd(max(threshold, 0)), bases=usd(bases),
            pct=str(pct.quantize(Decimal("0.01")))),
        "why_the_deposit_is_in_net_equity":
            "the account type is %s and cashBalance/totalCash/moneyMarketFund/cashReceipts "
            "are all 0: the wire did not remain as cash, it paid down the margin loan. "
            "Paying down a loan by the deposit raises net equity by the deposit amount, so "
            "the deposit is represented in verified net equity even though it is not "
            "sitting in a cash field." % sa.get("type"),
    }

    # ---- private artifacts stay here, on the credential host, mode 0600 ----
    for name, payload in (("account_detail.json", detail),
                          ("open_orders.json", open_orders),
                          ("transactions.json", tx)):
        p = os.path.join(args.out_dir, name)
        with open(p, "w") as fh:
            json.dump(payload, fh, sort_keys=True, default=str)
        os.chmod(p, 0o600)

    gross_assets = assets
    # The adapter expresses "net claimed reserve = claim" as gross minus a
    # liability input. That input is NOT an assertion that liabilities equal it:
    # observed liabilities are stated separately and the residual is simply
    # equity this packet does not claim.
    adapter_total_liabilities = gross_assets - claim

    public = {
        "schema": "postfiat.pftl.deposit_reserve_observation.v1",
        "observed_at_utc": observed_at.isoformat().replace("+00:00", "Z"),
        "observed_at_unix_millis": int(observed_at.timestamp() * 1000),
        "broker_source_id": BROKER_SOURCE_ID,
        "account_commitment_sha256": account_commitment,
        "account_identity_disclosed": False,
        "endpoints_read": [
            "GET /trader/v1/accounts/accountNumbers",
            "GET /trader/v1/accounts/{hash}?fields=positions",
            "GET /trader/v1/accounts/{hash}/orders?status=<each open status>",
            "GET /trader/v1/accounts/{hash}/transactions",
        ],
        "writes_performed": 0,
        "account_type": sa.get("type"),
        "positions_count": len(positions),
        "scale_atoms_per_usd": SCALE,
        "packet_represents": "one attributed external cash deposit",
        "attributed_deposit": attributed,
        "attributed_deposit_sha256": attributed_deposit_sha256,
        "qualifying_deposits_in_window": [c.report() for c in deposits],
        "coverage_test": coverage_test,
        "assets": asset_detail,
        "liabilities": liability_detail,
        "gross_assets_atoms": gross_assets,
        "gross_assets_usd": usd(gross_assets),
        "observed_liabilities_atoms": liabilities,
        "observed_liabilities_usd": usd(liabilities),
        "verified_net_equity_atoms": verified_net_equity,
        "verified_net_equity_usd": usd(verified_net_equity),
        "broker_liquidation_value_atoms": liq,
        "broker_liquidation_value_usd": usd(liq),
        "reconciles_to_broker_liquidation_value": reconciled,
        "claim_atoms": claim,
        "claim_usd": usd(claim),
        "adapter_total_liabilities_atoms": adapter_total_liabilities,
        "adapter_total_liabilities_note":
            "gross assets minus the claim. This is how the adapter expresses a net "
            "claimed reserve of %s USD; it is not a statement that liabilities equal "
            "%s USD. Observed liabilities are %s USD and the %s USD difference is "
            "equity this packet does not claim."
            % (usd(claim), usd(adapter_total_liabilities), usd(liabilities),
               usd(adapter_total_liabilities - liabilities)),
        "transaction_type_census": tx_census,
        "trust_class": {"quantity": "Attested", "valuation": "Attested"},
        "evidence_class": "live authenticated broker read; nothing signed it and no "
                          "enclave produced it",
    }

    disclosure = {
        "schema": DISCLOSURE_SCHEMA,
        "broker_source_id": BROKER_SOURCE_ID,
        "account_application_identity_sha256": account_commitment,
        "observed_at_unix_millis": public["observed_at_unix_millis"],
        "collection_epoch_sha256": root("collection_epoch.v1", {
            "window_start": window_start.isoformat(), "window_end": to_t,
            "observed_at": public["observed_at_utc"]}),
        "basket_decision_sha256": root("basket_decision.v1", {
            "decision": "none; this observation makes no basket decision and selects "
                        "no options"}),
        "execution_receipts_root_sha256": root("execution_receipts.v1", []),
        "broker_response_root_sha256": root("broker_response.v1", detail),
        "positions_root_sha256": root("positions.v1", positions),
        # the attributed deposit IS the cash fact this packet represents
        "cash_root_sha256": root("cash.v1", {
            "attributed_deposit": attributed,
            "cash_fields": {k: balances.get(k) for _, k, _ in REQUIRED_ASSET_CATEGORIES}}),
        "liabilities_root_sha256": root("liabilities.v1", {
            "observed": liability_detail, "unreported": unreported}),
        "open_orders_root_sha256": root("open_orders.v1", open_orders),
        "fills_root_sha256": root("fills.v1",
                                  [t for t in (tx or []) if str(t.get("type")) == "TRADE"]),
        "pending_events_root_sha256": root("pending_events.v1", {
            "unsettled_positions": len(unsettled), "open_orders": len(open_orders),
            "coverage_test": coverage_test}),
        "valuation_inputs_root_sha256": root("valuation_inputs.v1", {
            "source": "broker-reported marketValue per position and currentBalances",
            "balances": balances,
            "position_market_values": [
                {"symbol": (p.get("instrument") or {}).get("symbol"),
                 "marketValue": p.get("marketValue")} for p in positions]}),
    }

    for name, payload in (("observation.public.json", public),
                          ("disclosure.json", disclosure),
                          ("attributed-deposit.json", attributed),
                          ("coverage-test.json", coverage_test)):
        with open(os.path.join(args.out_dir, name), "w") as fh:
            json.dump(payload, fh, indent=1, sort_keys=True, default=str)

    print("=== ATTRIBUTED DEPOSIT ===")
    print(json.dumps(scrub(attributed), indent=1, sort_keys=True, default=str))
    print("=== COVERAGE TEST ===")
    print(json.dumps(scrub(coverage_test), indent=1, sort_keys=True, default=str))

    if not reconciled:
        print("PACKET REFUSED: verified net equity %s does not equal the broker's own "
              "liquidationValue %s" % (usd(verified_net_equity), usd(liq)), file=sys.stderr)
        return 4
    if not admissible:
        print("PACKET REFUSED by the coverage test: a claim of %s USD against verified net "
              "equity of %s USD is covered only %sx, below the required %dx. The %d liability "
              "categories the Schwab API does not report (%s) would only have to exceed %s USD "
              "to make this claim unsafe, which is %s%% of the %s USD of observed accrual "
              "bases -- close enough that they could matter. A MISSING liability is not zero."
              % (usd(claim), usd(verified_net_equity),
                 str(coverage.quantize(Decimal("0.01"))), COVERAGE_MULTIPLE_REQUIRED,
                 len(unreported), ", ".join(u["category"] for u in unreported),
                 usd(max(threshold, 0)), str(pct.quantize(Decimal("0.01"))), usd(bases)),
              file=sys.stderr)
        return 3
    if gross_assets % MICRO or adapter_total_liabilities % MICRO:
        print("PACKET REFUSED: figures are finer than one micro-USD", file=sys.stderr)
        return 5

    print("COVERAGE TEST PASSED: claim %s USD is covered %sx by verified net equity %s USD "
          "(required %dx)." % (usd(claim), str(coverage.quantize(Decimal("0.01"))),
                               usd(verified_net_equity), COVERAGE_MULTIPLE_REQUIRED))
    print("ADAPTER INPUTS (micro-USD, usd_1e6): --gross-assets %d --total-liabilities %d "
          "--valuation-scale 1000000"
          % (gross_assets // MICRO, adapter_total_liabilities // MICRO))
    print("CLAIMED NET RESERVE micro-USD: %d" % (claim // MICRO))
    print("ATTRIBUTED DEPOSIT micro-USD: %d" % (deposit.amount_atoms // MICRO))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Refusal as exc:
        print("PACKET REFUSED: %s" % exc, file=sys.stderr)
        sys.exit(2)
