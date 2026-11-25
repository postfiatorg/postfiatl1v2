"""Read-only Schwab deposit collector for the NAVCoin inventory sale.

It reads ONE endpoint:

    GET /trader/v1/accounts/{account_hash}/transactions?startDate=&endDate=

and returns the incoming cash transfers inside a sale's window. It never places
an order, never moves funds, never writes anything to the broker, and never
prints an account number, a token or a key. It does not handle credentials: the
caller passes in `BrokerHubService._schwab_request`, which already owns the
OAuth refresh, so there is exactly one Schwab client in this codebase.

WHAT A DEPOSIT IS, AND WHAT IT IS NOT
-------------------------------------
A net balance increase is not a deposit. This account trades and moves by more
than a sale's X every day. Only a transaction-level incoming cash transfer can
settle a sale, and the same transfer can settle one sale, once, ever.

The `type` census below is real: 3,000 transactions over the last 90 days on the
live account, read by the founder, not by this process.

    TRADE                 2988   not cash
    RECEIVE_AND_DELIVER      6   netAmount 0.0, securities movement, not cash
    JOURNAL                  4   INTERNAL cash moves between sub-accounts
    DIVIDEND_OR_INTEREST     2   observed +122.58 and -6.33, not a deposit

The JOURNAL rows are the trap. They arrive as matched +/- pairs at the same
timestamp moving cash between MARGIN and CASH sub-accounts:

    net=-34294.61  sub=MARGIN  "TRF FUNDS TO TYPE 1"   assetType=CURRENCY
    net=+34294.61  sub=CASH    "TRF FUNDS FRM TYPE 2"  assetType=CURRENCY

A naive "incoming cash of at least X" rule fires on the +34,294.61 leg and
releases coins against money that never entered the account. JOURNAL is excluded
from the allowlist outright, AND the offsetting-pair pattern is detected and
refused on its own, so neither defence stands alone.

THE ALLOWLIST CARRIES EXACTLY ONE CONFIRMED RAIL
------------------------------------------------
Nothing on this list was guessed. `WIRE_IN` is on it because one real incoming
wire landed on the committed account and was read off the live transactions
endpoint on 2026-11-10 at 20:09:09Z:

    type=WIRE_IN  netAmount=+2000.0  subAccount=CASH  status=VALID
    description="WIRED FUNDS RECEIVED"  activityId=135969961166

That single observation confirms two constants at once: the `type` string for a
wire, and that `VALID` is the status value meaning posted-and-valid.

EVERY OTHER FUNDING RAIL IS STILL UNCONFIRMED AND STILL REFUSES. ACH in
particular has never been observed on this account, so whatever string Schwab
uses for it is not here and must not be added on a guess. Every incoming
CURRENCY transaction whose type is not on the list is reported in full - type,
description, subAccount, status - so the first real example of each new rail
confirms its own string the same way this one did.
"""
from dataclasses import dataclass, replace
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
from hashlib import sha256
import json
from pathlib import Path

from inventory_model import EXTERNAL_DEPOSIT, LiveObservation, Rejected

SCALE = 100_000_000          # integer accounting atoms per currency unit
ACCOUNT_COMMITMENT_DOMAIN = b"s8-broker-account\x00"
TRANSACTIONS_PATH = "/trader/v1/accounts/%s/transactions"

# CONFIRMED, one entry, from one real observation. "WIRE_IN" was read off the
# live transactions endpoint on 2026-11-10T20:09:09Z for a genuine external
# incoming wire: netAmount +2000.0, subAccount CASH, status VALID, description
# "WIRED FUNDS RECEIVED", activityId 135969961166. It was not guessed and it was
# not inferred from a journal leg.
#
# NOTHING ELSE GOES ON THIS LIST WITHOUT ITS OWN REAL OBSERVATION. ACH and every
# other funding rail remain UNCONFIRMED and are refused as `unconfirmed_type`
# with their exact string reported, which is how this entry was established.
ACCEPTED_DEPOSIT_TYPES: tuple = ("WIRE_IN",)

# Observed on the live account and excluded with a reason. These are refusals,
# not omissions: a type on this list can never become a deposit by accident.
REFUSED_TYPES = {
    "TRADE": "trade",
    "RECEIVE_AND_DELIVER": "securities_movement",
    "JOURNAL": "internal_journal",
    "DIVIDEND_OR_INTEREST": "dividend_or_interest",
}

# CONFIRMED by the same 2026-11-10T20:09:09Z observation: the real WIRE_IN row
# carried status "VALID". Anything else is still refused and the observed string
# is reported rather than silently accepted.
REQUIRED_STATUS = "VALID"
REQUIRED_STATUS_CONFIRMED = True

CASH_ASSET_TYPE = "CURRENCY"


class CollectorRefusal(Rejected):
    """The collector will not produce a deposit. It never guesses instead."""


@dataclass(frozen=True)
class Candidate:
    """One normalized transaction from the account. `classification` is why it is
    or is not a deposit; only EXTERNAL_DEPOSIT may ever trigger a release."""
    transfer_id: str
    amount_atoms: int
    net_amount: str
    entry_type: str
    status: str
    sub_account: str
    description: str
    posted_at: int
    asset_type: str
    classification: str
    reason: str

    def is_deposit(self):
        return self.classification == EXTERNAL_DEPOSIT

    def report(self):
        """Everything a human needs to confirm the allowlist from one response.
        Carries no account number, no token and no key."""
        return {"transfer_id": self.transfer_id, "type": self.entry_type,
                "description": self.description, "subAccount": self.sub_account,
                "status": self.status, "netAmount": self.net_amount,
                "assetType": self.asset_type, "posted_at": self.posted_at,
                "classification": self.classification, "reason": self.reason}


def account_commitment(account_hash):
    """A commitment to the committed account. The account hash never leaves this
    function and is never printed or stored."""
    if not account_hash:
        raise CollectorRefusal("no Schwab account is configured for this ledger")
    return sha256(ACCOUNT_COMMITMENT_DOMAIN + str(account_hash).encode()).hexdigest()


def allowlist_status():
    return {
        "accepted_deposit_types": list(ACCEPTED_DEPOSIT_TYPES),
        "confirmed": bool(ACCEPTED_DEPOSIT_TYPES),
        "refused_types": dict(REFUSED_TYPES),
        "required_status": REQUIRED_STATUS,
        "required_status_confirmed": REQUIRED_STATUS_CONFIRMED,
        "confirmed_from": "WIRE_IN and status VALID were read off one real incoming wire "
                          "on the committed account at 2026-11-10T20:09:09Z "
                          "(netAmount +2000.0, subAccount CASH, activityId 135969961166).",
        "still_unconfirmed": "Every other funding rail, ACH included. None has been observed "
                             "on this account and none is guessed here.",
        "how_to_confirm": "For each new rail: one real incoming transfer of that kind on the "
                          "committed account. Save that day's transactions response and run "
                          "`navcoin.py preflight --transactions FILE`; it prints the exact "
                          "`type` of every incoming CURRENCY credit it refused. Put that "
                          "exact string into ACCEPTED_DEPOSIT_TYPES and nothing else.",
    }


def _atoms(net_amount):
    """Exact dollars to integer atoms. No float arithmetic touches money."""
    try:
        value = Decimal(str(net_amount))
    except (InvalidOperation, TypeError):
        raise CollectorRefusal("transaction netAmount is not a number: %r" % (net_amount,))
    scaled = value * SCALE
    if scaled != scaled.to_integral_value():
        raise CollectorRefusal("netAmount %s is finer than one accounting atom" % value)
    return int(scaled)


def _posted_at(transaction):
    raw = transaction.get("time") or transaction.get("tradeDate") or ""
    text = str(raw).replace("Z", "+00:00")
    try:
        moment = datetime.fromisoformat(text)
    except ValueError:
        raise CollectorRefusal("transaction has no readable time: %r" % (raw,))
    if moment.tzinfo is None:
        moment = moment.replace(tzinfo=timezone.utc)
    return int(moment.timestamp())


def _iso(epoch_seconds):
    return datetime.fromtimestamp(epoch_seconds, timezone.utc).isoformat().replace("+00:00", "Z")


def classify(transactions):
    """Normalize and classify every transaction. Returns Candidates in input order.

    Nothing here trusts a single field on its own: the asset type, the sign, the
    broker type string, the status and the offsetting-pair shape must all agree
    before a row is called an external cash deposit.
    """
    rows = []
    for transaction in transactions or []:
        transfer_id = transaction.get("activityId") or transaction.get("transactionId")
        items = transaction.get("transferItems") or []
        instrument = (items[0].get("instrument") or {}) if items else {}
        asset_type = str(instrument.get("assetType") or "").upper()
        entry_type = str(transaction.get("type") or "").upper()
        status = str(transaction.get("status") or "")
        try:
            atoms = _atoms(transaction.get("netAmount"))
            posted = _posted_at(transaction)
        except CollectorRefusal as exc:
            rows.append(Candidate(str(transfer_id or ""), 0, str(transaction.get("netAmount")),
                                  entry_type, status, str(transaction.get("subAccount") or ""),
                                  str(transaction.get("description") or ""), 0, asset_type,
                                  "unreadable", str(exc)))
            continue
        rows.append(Candidate(
            transfer_id=str(transfer_id or ""), amount_atoms=atoms,
            net_amount=str(transaction.get("netAmount")), entry_type=entry_type, status=status,
            sub_account=str(transaction.get("subAccount") or ""),
            description=str(transaction.get("description") or ""), posted_at=posted,
            asset_type=asset_type, classification="", reason=""))

    # Offsetting pairs: equal absolute amount, same timestamp, opposite sign. This
    # catches an internal sub-account move even if its type string were ever
    # allowlisted by mistake.
    paired = set()
    for i, row in enumerate(rows):
        for j, other in enumerate(rows):
            if i != j and row.amount_atoms == -other.amount_atoms != 0 \
                    and row.posted_at == other.posted_at:
                paired.add(i)
                paired.add(j)

    classified = []
    for i, row in enumerate(rows):
        if row.classification == "unreadable":
            classified.append(row)
            continue
        if not row.transfer_id:
            verdict, reason = "no_identity", "no activityId or transactionId"
        elif row.asset_type != CASH_ASSET_TYPE:
            verdict, reason = "non_cash", "assetType %r is not %s" % (row.asset_type, CASH_ASSET_TYPE)
        elif row.amount_atoms <= 0:
            verdict, reason = "outgoing_or_zero", "netAmount %s is not a positive credit" % row.net_amount
        elif i in paired:
            verdict, reason = ("internal_offsetting_pair",
                               "an equal and opposite leg posted at the same time: this is an "
                               "internal sub-account move, not money entering the account")
        elif row.entry_type in REFUSED_TYPES:
            verdict, reason = (REFUSED_TYPES[row.entry_type],
                               "type %s is excluded from the deposit allowlist" % row.entry_type)
        elif row.entry_type not in ACCEPTED_DEPOSIT_TYPES:
            verdict, reason = ("unconfirmed_type",
                               "type %r is an incoming cash credit but is NOT on the confirmed "
                               "deposit allowlist. The collector refuses rather than guess. "
                               "Confirm this exact string against one real deposit."
                               % row.entry_type)
        elif row.status != REQUIRED_STATUS:
            verdict, reason = ("unsettled_status",
                               "status %r is not the required %r" % (row.status, REQUIRED_STATUS))
        else:
            verdict, reason = EXTERNAL_DEPOSIT, "incoming cash credit on the confirmed allowlist"
        classified.append(replace(row, classification=verdict, reason=reason))
    return classified


def resolve_account_hash(request):
    """Read the linked account's opaque hash. The hash is returned for immediate
    use in a request path and is never printed, logged or stored: only a
    commitment to it ever leaves this module."""
    numbers = request("/trader/v1/accounts/accountNumbers")
    if not numbers:
        raise CollectorRefusal("no linked Schwab account")
    return str(numbers[0]["hashValue"])


def collect(request, account_hash, start, end):
    """Read the account's transactions over [start, end] and classify them.

    `request` must be BrokerHubService._schwab_request (or a test double with the
    same signature). This function performs exactly one read and no writes.
    """
    params = {"startDate": _iso(start), "endDate": _iso(end)}
    payload = request(TRANSACTIONS_PATH % account_hash, params=params)
    digest = sha256(json.dumps(payload, sort_keys=True, default=str).encode()).hexdigest()
    receipt = {"endpoint": "GET " + TRANSACTIONS_PATH % "<account-hash-withheld>",
               "window": params, "transactions_seen": len(payload or []),
               "response_sha256": digest,
               "evidence_class": "live broker read over an authenticated session; nothing "
                                 "signed it and no enclave produced it"}
    return classify(payload), receipt


def select_for_sale(candidates, *, sale, required_deposit, window_start, window_end):
    """Pick the ONE incoming transfer that settles this sale, or refuse.

    Attribution is the hard part and it is not papered over. A Schwab transaction
    carries no sale identifier, so the binding is: the committed account, the
    sale's own window, an amount of at least X, a confirmed external-deposit type
    and one-transfer-one-sale. If two transfers could each be the one, this
    refuses rather than choosing.
    """
    deposits = [c for c in candidates if c.is_deposit()
                and window_start <= c.posted_at <= window_end
                and c.amount_atoms >= required_deposit]
    if not deposits:
        near = [c for c in candidates
                if c.classification in ("unconfirmed_type", "unsettled_status")
                and window_start <= c.posted_at <= window_end]
        detail = ""
        if near:
            detail = (" The window did contain %d incoming cash credit(s) whose broker type is "
                      "not confirmed as a deposit. Their full records are reported so one real "
                      "deposit can confirm the allowlist: %s"
                      % (len(near), json.dumps([c.report() for c in near], sort_keys=True)))
        raise CollectorRefusal(
            "no qualifying incoming transfer of at least %d atoms into the committed account "
            "between %s and %s for sale %s.%s"
            % (required_deposit, _iso(window_start), _iso(window_end), sale, detail))
    if len(deposits) > 1:
        raise CollectorRefusal(
            "%d incoming transfers in this sale's window could each be the deposit for %s "
            "(%s). A Schwab transaction carries no sale identifier, so this is ambiguous and "
            "the collector refuses to choose. Settle the sales one window at a time."
            % (len(deposits), sale, ", ".join(c.transfer_id for c in deposits)))
    return deposits[0]


def observation(candidate, receipt, *, sale, account_commitment_hex, currency, domain, asset):
    """Turn the selected transfer into the record the model consumes."""
    memo = candidate.description.lower()
    return LiveObservation(
        record={"kind": "deposit", "domain": domain, "asset": asset, "sale": sale,
                "account": account_commitment_hex, "transfer_id": candidate.transfer_id,
                "amount": candidate.amount_atoms, "currency": currency,
                "direction": "incoming", "transfer_class": candidate.classification,
                "entry_type": candidate.entry_type, "status_ok": candidate.status == REQUIRED_STATUS,
                "posted_at": candidate.posted_at,
                "memo_names_sale": sale.lower() in memo},
        receipt={**receipt, "selected": candidate.report()})


# ------------------------------------------------------------------ live wiring

def live_request(token_path=None, credentials_path=None):
    """Return SchwabClient.request. Credentials and the OAuth refresh stay inside
    schwab_client; nothing is read or printed here."""
    from schwab_client import SchwabClient
    service = SchwabClient()
    if token_path is not None:
        service.token_path = Path(token_path)
    if credentials_path is not None:
        service.credentials_path = Path(credentials_path)
    return service.request


# Mirrors schwab_client.SCHWAB_TOKEN_PATH (where the Schwab login keeps its token).
# Never opened for its contents beyond one timestamp field.
DEFAULT_TOKEN_PATH = Path.home() / ".config" / "navstrategies" / "schwab_tokens.json"


def broker_hub_importable():
    """Can the existing Schwab client be imported by this interpreter?"""
    try:
        import schwab_client                                    # noqa: F401
        return True, "schwab_client imports in this interpreter"
    except Exception as exc:
        return False, "%s: %s" % (type(exc).__name__, exc)


def token_status(token_path=None):
    """Login age, without opening the token file at all.

    Only the path's existence and its modification time are read. The file is
    never opened, so no token, secret or account number can leak through here.
    broker_hub rewrites it on every refresh, so the mtime IS the last refresh.
    """
    if token_path is None:
        token_path = DEFAULT_TOKEN_PATH
    path = Path(token_path)
    if not path.is_file():
        return {"configured": False,
                "reason": "no Schwab token file at the configured path, so there is no "
                          "Schwab session on this machine at all"}
    status = {"configured": True, "last_refreshed_epoch": int(path.stat().st_mtime)}
    status["seven_day_clock"] = (
        "Schwab's refresh token is a 7-day interactive grant that only the founder can renew. "
        "Nothing in this codebase records when that grant was issued, so its remaining life "
        "cannot be checked without a call. A collector run after it expires must fail loudly "
        "with the broker's own error, never report 'no deposit found'.")
    return status
