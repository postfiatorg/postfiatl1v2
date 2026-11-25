"""Isolated specification model. No network, wallet, signer, or deployment code.

Two kinds of evidence reach this model and they are never confused:

  Proof           an SHA-256 inclusion path to a fixture checkpoint pinned at
                  construction. REHEARSAL ONLY. Trusted by construction.
  LiveObservation a record read straight off the broker over an authenticated
                  session by schwab_deposits.py. Nothing signed it, no enclave
                  produced it. Its trust class is the operator's own machine.

Which of the two a ledger will accept is pinned in its terms (`deposit_source`)
and cannot change afterwards. The class of every admitted deposit is recorded.

Sale life cycle:

  Q  quoted     terms fixed and immutable; no coins reserved, nothing paid
  P  paid       the payment event is proven and permanently consumed; the coins
                are reserved to this sale and STILL LOCKED
  C  released   an attributed deposit of at least X arrived before the deadline
                and the reserve covered the release
  B  exiting    a holder asked to redeem; the exit cash is reserved
  D  retired    the exit was paid in full
  K  returned   the sale was cancelled and the buyer's cash was returned in full
  F  failed     the deadline passed with no release. The coins went back to the
                dormant locked pool and the payment is permanently closed.

This is NOT a chain finality verifier and NOT a hardware broker reader.
Production construction is intentionally refused.
"""
from contextlib import contextmanager
from dataclasses import dataclass, field
from hashlib import sha256
import json
import sqlite3

MODE_A = "A"            # hold until release: the buyer's USDC is non-withdrawable
MODE_B = "B"            # pull to convert: a recorded one-way pull, then a claim
MODES = (MODE_A, MODE_B)

SOURCE_LIVE = "live-schwab-read"        # the real read-only collector
SOURCE_FIXTURE = "rehearsal-fixture"    # hand-asserted synthetic evidence
SOURCES = (SOURCE_LIVE, SOURCE_FIXTURE)

# Where a payment's evidence comes from, pinned per ledger exactly like the
# deposit source. ONCHAIN means the buyer's USDC is read out of a deployed
# SaleEscrow contract; FIXTURE means the synthetic inclusion proof, which is a
# rehearsal construct and can never settle a live-payment ledger.
PAYMENT_ONCHAIN = "onchain-escrow"
PAYMENT_FIXTURE = "rehearsal-fixture"
PAYMENT_SOURCES = (PAYMENT_ONCHAIN, PAYMENT_FIXTURE)

# The only normalized transfer class that may trigger a release. The broker's own
# type strings never reach this model: schwab_deposits.py maps them onto these
# classes and only ever sets EXTERNAL_DEPOSIT for a confirmed external cash
# deposit. Everything else is recorded and refused.
EXTERNAL_DEPOSIT = "external_cash_deposit"

# States that hold coins. Q, K and F hold none: their coins are dormant locked.
COIN_STATES = ("P", "C", "X", "W", "B", "D")
OPEN_STATES = ("Q", "P")

LATE_DEPOSIT_ACKNOWLEDGEMENT = (
    "I accept a deposit that arrived after this sale's deadline. It releases no coins."
)


class Rejected(ValueError):
    pass


def require(ok, reason):
    if not ok:
        raise Rejected(reason)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def leaf(value):
    return sha256(b"s8-leaf\x00" + canonical(value)).digest()


def branch(left, right):
    return sha256(b"s8-node\x00" + left + right).digest()


@dataclass(frozen=True)
class Proof:
    record: dict
    path: tuple

    def verify(self, root):
        value = leaf(self.record)
        for side, sibling in self.path:
            require(side in ("left", "right") and len(sibling) == 32, "bad path")
            value = branch(sibling, value) if side == "left" else branch(value, sibling)
        require(value == root, "forged or untrusted proof")
        return self.record


@dataclass(frozen=True)
class LiveObservation:
    """A deposit record read live from the broker. Carries no inclusion proof.

    `receipt` records how it was obtained: endpoint, window, retrieval time and a
    digest of the response. It is provenance, not cryptographic authentication.
    """
    record: dict
    receipt: dict = field(default_factory=dict)


@dataclass(frozen=True)
class EscrowFunding:
    """The buyer's USDC, read out of a deployed SaleEscrow contract.

    This is not a proof of anything the model constructed: it is a report of what
    a JSON-RPC endpoint said the escrow holds, cross-checked between the
    SaleFunded log and the live sale state, with a confirmation depth. Its trust
    class is the endpoint. What it does carry that no synthetic proof can is that
    the money is in a contract whose only exits are a release to the seller before
    the deadline and a refund the buyer takes themselves afterwards.
    """
    record: dict
    receipt: dict = field(default_factory=dict)


@dataclass(frozen=True)
class Terms:
    opening: int | None = None
    price: int | None = None
    fee_bps: int | None = None
    exit_fee_bps: int | None = None
    original_basis: int | None = None
    max_age: int | None = None
    required_deposit: int | None = None     # X: the deposit the account must record
    deposit_window: int | None = None       # how long X has to appear, in seconds
    escrow_mode: str | None = None          # A (hold until release) or B (pull to convert)
    deposit_source: str | None = None       # which evidence class this ledger admits
    broker_account: str | None = None       # a COMMITMENT, never an account number
    deposit_currency: str | None = None     # the currency X is denominated in
    payment_asset: str | None = None        # the USDC contract, for an on-chain ledger
    payment_chain: str | None = None
    payment_lock: str | None = None         # the SaleEscrow contract, for an on-chain ledger
    payment_source: str | None = None       # which payment evidence class this ledger admits
    evidence: str | None = None

    NUMERIC = ("opening", "price", "fee_bps", "exit_fee_bps", "original_basis", "max_age",
               "required_deposit", "deposit_window")
    TEXT = ("broker_account", "deposit_currency", "payment_asset", "payment_chain",
            "payment_lock", "payment_source", "evidence")

    def validate(self):
        for name in self.NUMERIC:
            v = getattr(self, name)
            require(type(v) is int and v >= 0, "unset or invalid terms: " + name)
        require(self.opening > 0 and self.price > 0 and self.max_age > 0,
                "positive terms required")
        require(self.required_deposit > 0, "unset or invalid terms: required_deposit")
        require(self.deposit_window > 0, "unset or invalid terms: deposit_window")
        require(self.fee_bps < 10000 and self.exit_fee_bps < 10000, "invalid fee")
        require(self.escrow_mode in MODES, "unset or invalid terms: escrow_mode")
        require(self.deposit_source in SOURCES, "unset or invalid terms: deposit_source")
        require(self.payment_source in PAYMENT_SOURCES,
                "unset or invalid terms: payment_source")
        for name in self.TEXT:
            v = getattr(self, name)
            require(type(v) is str and bool(v), "unset or invalid terms: " + name)


class Inventory:
    DOMAIN = "isolated-s8-successor"
    ASSET = "fixture-native-inventory"

    def __init__(self, path, terms, trusted_root, *, isolated=False):
        terms.validate()
        require(isolated, "production disabled: seven gates and successor qualification required")
        require(type(trusted_root) is bytes and len(trusted_root) == 32, "checkpoint required")
        self.terms = terms
        self.root = trusted_root
        self.db = sqlite3.connect(path, isolation_level=None)
        self.db.row_factory = sqlite3.Row
        self.db.executescript("""
        CREATE TABLE IF NOT EXISTS meta (id INTEGER PRIMARY KEY CHECK(id=1), config TEXT,
          frozen INTEGER DEFAULT 0, seq INTEGER DEFAULT 0, observed INTEGER DEFAULT 0,
          net INTEGER DEFAULT 0, nav INTEGER DEFAULT 0, debits INTEGER DEFAULT 0,
          covered TEXT DEFAULT '[]');
        CREATE TABLE IF NOT EXISTS sales (sale TEXT PRIMARY KEY, nullifier TEXT UNIQUE,
          buyer TEXT, quantity INTEGER, price INTEGER, gross INTEGER, basis INTEGER,
          fee INTEGER, state TEXT, settled INTEGER DEFAULT 0, exit_due INTEGER DEFAULT 0,
          required_deposit INTEGER, deadline INTEGER, mode TEXT, quoted_at INTEGER,
          paid_at INTEGER, pulled_at INTEGER, payment_closed INTEGER DEFAULT 0,
          escrow_sale_id TEXT, escrow_address TEXT, escrow_chain_id INTEGER);
        CREATE TABLE IF NOT EXISTS deposits (nullifier TEXT PRIMARY KEY, sale TEXT UNIQUE,
          transfer_id TEXT, amount INTEGER, posted_at INTEGER, source TEXT,
          entry_type TEXT, memo_names_sale INTEGER DEFAULT 0);
        CREATE TABLE IF NOT EXISTS outputs (receipt TEXT PRIMARY KEY, sale TEXT UNIQUE);
        CREATE TABLE IF NOT EXISTS journal (id INTEGER PRIMARY KEY, action TEXT, sale TEXT,
          payload TEXT);
        """)
        config = canonical({"terms": vars(terms), "root": trusted_root.hex()}).decode()
        row = self.db.execute("SELECT config FROM meta").fetchone()
        if row:
            require(row["config"] == config, "pinned configuration mismatch")
        else:
            self.db.execute("INSERT INTO meta(id,config) VALUES(1,?)", (config,))

    def close(self):
        self.db.close()

    @contextmanager
    def atomic(self):
        self.db.execute("BEGIN IMMEDIATE")
        try:
            yield
            self.check_conservation()
            self.db.execute("COMMIT")
        except BaseException:
            self.db.execute("ROLLBACK")
            raise

    def meta(self):
        return self.db.execute("SELECT * FROM meta").fetchone()

    def sale(self, sale):
        row = self.db.execute("SELECT * FROM sales WHERE sale=?", (sale,)).fetchone()
        require(row is not None, "unknown sale")
        return row

    def deposit_for(self, sale):
        return self.db.execute("SELECT * FROM deposits WHERE sale=?", (sale,)).fetchone()

    def evidence(self, proof, kind):
        r = proof.verify(self.root)
        require(r.get("kind") == kind and r.get("domain") == self.DOMAIN, "proof domain mismatch")
        require(r.get("asset") == self.ASSET, "proof asset mismatch")
        return r

    def log(self, action, sale, payload):
        self.db.execute("INSERT INTO journal(action,sale,payload) VALUES(?,?,?)",
                        (action, sale, canonical(payload).decode()))

    # ------------------------------------------------------------------ quote

    def quote(self, sale, buyer, quantity, now, *, mode=None, required_deposit=None,
              window=None, escrow_sale_id=None, escrow_address=None,
              escrow_chain_id=None):
        """Create the sale. Quantity, price, fee, X, deadline and escrow mode are
        fixed here and are immutable afterwards. No coins are reserved yet.

        On a ledger pinned to on-chain payments the sale also carries the identity
        of the escrow that will hold the buyer's money: the contract's bytes32
        saleId, the contract address and the chain id. They are written once, here,
        and every later payment must match all three."""
        require(type(sale) is str and bool(sale), "a sale id is required")
        require(type(buyer) is str and bool(buyer), "a buyer destination is required")
        require(type(quantity) is int and quantity > 0, "invalid quantity")
        require(type(now) is int and now > 0, "authenticated time required")
        mode = self.terms.escrow_mode if mode is None else mode
        require(mode in MODES, "escrow mode must be A (hold until release) or B (pull to convert)")
        x = self.terms.required_deposit if required_deposit is None else required_deposit
        require(type(x) is int and x > 0, "the required deposit X must be a positive amount")
        window = self.terms.deposit_window if window is None else window
        require(type(window) is int and window > 0, "the deposit window must be positive")
        if self.terms.payment_source == PAYMENT_ONCHAIN:
            require(bool(escrow_sale_id) and bool(escrow_address) and
                    type(escrow_chain_id) is int and escrow_chain_id > 0,
                    "this ledger is pinned to on-chain escrow payments, so a quote must "
                    "name the escrow that will hold the money: its bytes32 saleId, its "
                    "contract address and its chain id")
        else:
            require(escrow_sale_id is None and escrow_address is None and
                    escrow_chain_id is None,
                    "this ledger's pinned payment source is '%s', so a quote cannot name "
                    "an on-chain escrow" % self.terms.payment_source)
        fee = (quantity * self.terms.price * self.terms.fee_bps + 9999) // 10000
        gross = quantity * self.terms.price + fee
        deadline = now + window
        with self.atomic():
            require(not self.meta()["frozen"], "frozen")
            require(self.available(now) >= quantity,
                    "not enough dormant unreserved inventory for this quote")
            try:
                self.db.execute(
                    "INSERT INTO sales(sale,buyer,quantity,price,gross,basis,fee,state,"
                    "required_deposit,deadline,mode,quoted_at,escrow_sale_id,escrow_address,"
                    "escrow_chain_id) VALUES(?,?,?,?,?,?,?,'Q',?,?,?,?,?,?,?)",
                    (sale, buyer, quantity, self.terms.price, gross,
                     quantity * self.terms.original_basis, fee, x, deadline, mode, now,
                     escrow_sale_id, escrow_address, escrow_chain_id))
            except sqlite3.IntegrityError as exc:
                raise Rejected("sale id already used") from exc
            self.log("quote", sale, {"quantity": quantity, "gross": gross,
                                     "required_deposit": x, "deadline": deadline,
                                     "mode": mode, "window": window,
                                     "escrow_sale_id": escrow_sale_id,
                                     "escrow_address": escrow_address,
                                     "escrow_chain_id": escrow_chain_id,
                                     "payment_source": self.terms.payment_source,
                                     "buyer_protection": self.protection_text(mode, False)})
        return self.sale(sale)

    def available(self, now):
        """Dormant locked coins that are not already promised to a live quote."""
        quoted = self.db.execute(
            "SELECT COALESCE(SUM(quantity),0) AS q FROM sales WHERE state='Q' AND deadline>=?",
            (now,)).fetchone()["q"]
        return self.supply()["L"] - quoted

    # ---------------------------------------------------------------- payment

    def accept_payment(self, observation, now):
        """Admit the buyer's payment.

        Two evidence classes and they are never confused, pinned per ledger by
        `payment_source` exactly as deposits are pinned by `deposit_source`:

          EscrowFunding  the buyer's USDC read out of a deployed SaleEscrow. The
                         money is in a contract the seller cannot reach.
          Proof          the synthetic inclusion path. REHEARSAL ONLY.
        """
        require(type(now) is int and now > 0, "authenticated time required")
        if isinstance(observation, Proof):
            require(self.terms.payment_source == PAYMENT_FIXTURE,
                    "this ledger's pinned payment source is '%s'; a synthetic payment "
                    "proof is refused. The buyer's money must be read out of the escrow "
                    "contract." % self.terms.payment_source)
            r, payment_source, receipt = self.evidence(observation, "payment"), \
                PAYMENT_FIXTURE, {}
        elif isinstance(observation, EscrowFunding):
            require(self.terms.payment_source == PAYMENT_ONCHAIN,
                    "this ledger's pinned payment source is '%s'; an on-chain escrow "
                    "funding is refused" % self.terms.payment_source)
            r, payment_source, receipt = observation.record, PAYMENT_ONCHAIN, \
                observation.receipt
            require(r.get("kind") == "payment" and r.get("domain") == self.DOMAIN and
                    r.get("asset") == self.ASSET, "escrow funding domain mismatch")
        else:
            raise Rejected("a payment needs an escrow funding reading or, in a "
                           "rehearsal, an inclusion proof")
        require(bool(r.get("buyer")) and bool(r.get("sale")), "missing sale binding")
        # Deliberately excludes the sale ID: one event cannot be replayed under
        # another sale label.
        nullifier = sha256(canonical(
            [r["chain"], r["transaction"], r["event"], r["cash_asset"]])).hexdigest()
        with self.atomic():
            require(not self.meta()["frozen"], "frozen")
            used = self.db.execute("SELECT sale FROM sales WHERE nullifier=?",
                                   (nullifier,)).fetchone()
            require(used is None, "payment already consumed by sale %s"
                    % (used["sale"] if used else ""))
            s = self.sale(r["sale"])
            require(s["state"] == "Q", "sale is not an open quote")
            require(now <= s["deadline"],
                    "the deadline has already passed: no coins are reserved")
            require(r["buyer"] == s["buyer"], "payment buyer does not match the quote")
            require(r.get("quantity") == s["quantity"], "payment quantity mismatch")
            require(r.get("cash_asset") == self.terms.payment_asset and
                    r.get("destination") == self.terms.payment_lock, "wrong payment route")
            require(r.get("chain") == self.terms.payment_chain, "wrong payment chain")
            require(r.get("finalized") is True and r.get("success") is True, "payment not final")
            require(r.get("amount") == s["gross"], "payment amount mismatch")
            if payment_source == PAYMENT_ONCHAIN:
                # All three escrow identifiers were written at quote time and are
                # immutable. A funding read out of some other contract, some other
                # chain or some other on-chain sale cannot reserve these coins.
                require(r.get("escrow_sale_id") == s["escrow_sale_id"],
                        "the escrow funding names on-chain sale %s, not this sale's %s"
                        % (r.get("escrow_sale_id"), s["escrow_sale_id"]))
                require(str(r.get("escrow_address") or "").lower()
                        == str(s["escrow_address"] or "").lower(),
                        "the escrow funding was read from %s, not from this sale's escrow %s"
                        % (r.get("escrow_address"), s["escrow_address"]))
                require(r.get("escrow_chain_id") == s["escrow_chain_id"],
                        "the escrow funding is on chain %s, not this sale's chain %s"
                        % (r.get("escrow_chain_id"), s["escrow_chain_id"]))
            require(self.supply()["L"] >= s["quantity"], "insufficient locked inventory")
            try:
                self.db.execute("UPDATE sales SET nullifier=?, state='P', paid_at=? WHERE sale=?",
                                (nullifier, now, s["sale"]))
            except sqlite3.IntegrityError as exc:
                raise Rejected("payment already consumed") from exc
            self.log("payment_reserved", s["sale"],
                     {"nullifier": nullifier, "quantity": s["quantity"],
                      "gross": s["gross"], "deadline": s["deadline"], "mode": s["mode"],
                      "payment_evidence_class": payment_source,
                      "escrow_sale_id": s["escrow_sale_id"],
                      "escrow_address": s["escrow_address"],
                      "escrow_chain_id": s["escrow_chain_id"],
                      "receipt": receipt})

    # ------------------------------------------- the deposit trigger, attributed

    def observe_deposit(self, sale, observation, now):
        """Record ONE incoming transfer into the committed account, identified at
        transaction level, of at least X, bound to exactly one sale for ever.

        A net balance increase reaches none of this. Only a transfer record does.
        """
        require(type(now) is int and now > 0, "authenticated time required")
        if isinstance(observation, Proof):
            require(self.terms.deposit_source == SOURCE_FIXTURE,
                    "this ledger's pinned deposit source is '%s'; hand-asserted rehearsal "
                    "deposit evidence is refused" % self.terms.deposit_source)
            r, source, receipt = self.evidence(observation, "deposit"), SOURCE_FIXTURE, {}
        elif isinstance(observation, LiveObservation):
            require(self.terms.deposit_source == SOURCE_LIVE,
                    "this ledger's pinned deposit source is '%s'; a live broker read is "
                    "refused" % self.terms.deposit_source)
            r, source, receipt = observation.record, SOURCE_LIVE, observation.receipt
            require(r.get("kind") == "deposit" and r.get("domain") == self.DOMAIN and
                    r.get("asset") == self.ASSET, "live observation domain mismatch")
        else:
            raise Rejected("a deposit needs an inclusion proof or a live broker observation")
        transfer_id = r.get("transfer_id")
        require(type(transfer_id) is str and bool(transfer_id),
                "the observation does not identify the incoming transfer at transaction level")
        nullifier = sha256(canonical(
            ["deposit", r.get("account"), transfer_id, r.get("currency")])).hexdigest()
        with self.atomic():
            s = self.sale(sale)
            require(s["state"] == "P", "sale is not reserved and awaiting a deposit")
            require(now <= s["deadline"],
                    "the deadline has passed: this sale can no longer settle")
            require(r.get("sale") == sale, "the observation names a different sale")
            require(r.get("account") == self.terms.broker_account,
                    "the transfer did not land in the account this sale commits to")
            require(r.get("currency") == self.terms.deposit_currency, "wrong deposit currency")
            require(r.get("direction") == "incoming", "not an incoming transfer")
            require(r.get("transfer_class") == EXTERNAL_DEPOSIT,
                    "not an external cash deposit: transfer class %r" % (r.get("transfer_class"),))
            require(r.get("status_ok") is True, "the transfer is not in a settled broker status")
            amount = r.get("amount")
            require(type(amount) is int and amount > 0, "invalid deposit amount")
            require(amount >= s["required_deposit"],
                    "deposit is smaller than X: %d < %d" % (amount, s["required_deposit"]))
            posted = r.get("posted_at")
            require(type(posted) is int, "the observation does not time the transfer")
            require(posted <= now, "the transfer is timed in the future")
            require(s["quoted_at"] <= posted <= s["deadline"],
                    "the transfer posted outside this sale's window")
            try:
                self.db.execute(
                    "INSERT INTO deposits(nullifier,sale,transfer_id,amount,posted_at,source,"
                    "entry_type,memo_names_sale) VALUES(?,?,?,?,?,?,?,?)",
                    (nullifier, sale, transfer_id, amount, posted, source,
                     str(r.get("entry_type") or ""), 1 if r.get("memo_names_sale") else 0))
            except sqlite3.IntegrityError as exc:
                raise Rejected("this incoming transfer is already bound to a sale, or this "
                               "sale already has a deposit") from exc
            self.db.execute("UPDATE sales SET settled=? WHERE sale=?", (amount, sale))
            self.log("deposit_attributed", sale,
                     {"transfer_id": transfer_id, "amount": amount, "required": s["required_deposit"],
                      "posted_at": posted, "evidence_class": source,
                      "broker_entry_type": str(r.get("entry_type") or ""),
                      "memo_names_sale": bool(r.get("memo_names_sale")), "receipt": receipt})

    def accept_late_deposit(self, sale, observation, now, acknowledgement):
        """Explicit, separately logged. It RELEASES NOTHING, ever, and it burns the
        transfer so it can never settle any sale."""
        require(acknowledgement == LATE_DEPOSIT_ACKNOWLEDGEMENT,
                "a late deposit needs an explicit written acknowledgement; it is never a default")
        if isinstance(observation, Proof):
            r = self.evidence(observation, "deposit")
        elif isinstance(observation, LiveObservation):
            r = observation.record
        else:
            raise Rejected("a deposit needs an inclusion proof or a live broker observation")
        transfer_id = r.get("transfer_id")
        require(type(transfer_id) is str and bool(transfer_id), "no transaction-level identity")
        nullifier = sha256(canonical(
            ["deposit", r.get("account"), transfer_id, r.get("currency")])).hexdigest()
        with self.atomic():
            s = self.sale(sale)
            require(s["state"] == "F" or now > s["deadline"],
                    "this sale has not timed out; use the ordinary deposit path")
            try:
                self.db.execute(
                    "INSERT INTO deposits(nullifier,sale,transfer_id,amount,posted_at,source,"
                    "entry_type) VALUES(?,?,?,?,?,?,?)",
                    (nullifier, "late:" + sale, transfer_id, int(r.get("amount") or 0),
                     int(r.get("posted_at") or 0), "late", str(r.get("entry_type") or "")))
            except sqlite3.IntegrityError as exc:
                raise Rejected("this incoming transfer is already recorded") from exc
            self.log("late_deposit_recorded_no_release", sale,
                     {"transfer_id": transfer_id, "amount": int(r.get("amount") or 0),
                      "released": False, "sale_state": s["state"],
                      "note": "arrived after the deadline; it releases no coins and the "
                              "transfer is now permanently used"})

    # ---------------------------------------------------------------- reserve

    def publish(self, proof, now):
        r = self.evidence(proof, "reserve")
        with self.atomic():
            m = self.meta()
            require(r.get("account") == self.terms.broker_account, "reserve account mismatch")
            require(type(now) is int and 0 <= now - r["observed"] <= self.terms.max_age,
                    "stale reserve")
            require(r["sequence"] > m["seq"], "reserve sequence replay")
            require(r["nav"] == self.terms.price, "quote fix mismatch")
            require(type(r["net"]) is int and r["net"] >= 0, "invalid net reserve")
            recorded = {d[0] for d in self.db.execute(
                "SELECT nullifier FROM deposits WHERE source!='late'")}
            paid = {s[0] for s in self.db.execute("SELECT receipt FROM outputs")}
            require(set(r["deposits"]) == recorded and set(r["paid_outputs"]) == paid,
                    "incomplete reserve reconciliation")
            self.db.execute("UPDATE meta SET seq=?,observed=?,net=?,nav=?,debits=0,covered=? "
                            "WHERE id=1",
                            (r["sequence"], r["observed"], r["net"], r["nav"],
                             json.dumps(sorted(recorded))))
            # An impaired observation must be recorded and freeze new releases.
            if r["net"] < self.obligation():
                self.db.execute("UPDATE meta SET frozen=1 WHERE id=1")
            self.log("reserve", "", {"sequence": r["sequence"], "net": r["net"]})

    def fresh(self, now):
        m = self.meta()
        require(m["seq"] > 0 and type(now) is int and
                0 <= now - m["observed"] <= self.terms.max_age, "stale reserve")

    def obligation(self):
        m = self.meta()
        total = 0
        for s in self.db.execute("SELECT * FROM sales"):
            if s["state"] in ("C", "X", "W"):
                total += s["quantity"] * m["nav"]
            elif s["state"] == "B":
                total += s["exit_due"]
            elif s["state"] == "P":
                total += s["settled"]
        return total

    # ---------------------------------------------------------------- release

    def release(self, sale, now, *, fault=False, escrow_attestation=None):
        """Let the coins leave the lock.

        On a ledger pinned to on-chain payments this additionally requires the
        escrow attestation: the signed statement the contract's own `release` will
        verify. It is checked here against the deposit this ledger actually
        attributed, so the coins and the money move on one and the same observed
        transfer. A release with no attestation, or with an attestation that names
        a different transfer, a different escrow or a different amount, is refused
        and the coins stay locked.
        """
        with self.atomic():
            require(not self.meta()["frozen"], "frozen")
            self.fresh(now)
            s = self.sale(sale)
            require(s["state"] == "P", "already released or cancelled")
            require(now <= s["deadline"],
                    "the deadline has passed: this sale failed and can never release")
            d = self.deposit_for(sale)
            require(d is not None,
                    "no attributed incoming transfer into the committed account for this sale")
            require(d["amount"] >= s["required_deposit"], "deposit is smaller than X")
            require(d["nullifier"] in json.loads(self.meta()["covered"]),
                    "the deposit is absent from the published reserve statement")
            attested = self.check_escrow_attestation(s, d, escrow_attestation)
            self.db.execute("UPDATE sales SET state='C' WHERE sale=?", (sale,))
            require(self.meta()["net"] - self.meta()["debits"] >= self.obligation(),
                    "underbacked release")
            if s["mode"] == MODE_A and attested:
                escrow = {"escrow_leg": "the buyer's escrowed USDC settles to the seller when "
                                        "this attestation is submitted to the SaleEscrow "
                                        "contract, which is a separate on-chain transaction "
                                        "the operator sends; it is not atomic with this "
                                        "ledger entry and the contract is the authority",
                          "amount": s["gross"], **attested}
            elif s["mode"] == MODE_A:
                escrow = {"escrow_leg": "the buyer's escrowed payment settles to the seller in "
                                        "the same transition that releases the coins",
                          "amount": s["gross"]}
            elif s["pulled_at"] is None:
                escrow = {"escrow_leg": "mode B, never pulled: the escrowed payment settles to "
                                        "the seller in the same transition", "amount": s["gross"]}
            else:
                escrow = {"escrow_leg": "mode B: the payment was already pulled at %d, so there "
                                        "is nothing left in escrow to settle" % s["pulled_at"],
                          "amount": 0}
            self.log("release", sale, {"quantity": s["quantity"], "original_basis": s["basis"],
                                       "gross_sale": s["gross"], "fee": s["fee"],
                                       "fix": s["price"], "mode": s["mode"],
                                       "deposit": d["transfer_id"], **escrow})
            if fault:
                raise Rejected("injected delivery failure")

    def check_escrow_attestation(self, s, d, attestation):
        """Bind the signed escrow attestation to the deposit this ledger attributed.

        Returns the fields to journal, or None on a rehearsal ledger. Every
        refusal names what did not match. Note what is NOT claimed: this check
        does not verify the signature cryptographically - that is the contract's
        job and the contract is the authority. It verifies that the statement
        being submitted is a statement about THIS sale's one observed transfer.
        """
        if self.terms.payment_source != PAYMENT_ONCHAIN:
            require(attestation is None,
                    "this ledger's pinned payment source is '%s', so there is no escrow "
                    "contract and an escrow attestation is refused"
                    % self.terms.payment_source)
            return None
        require(isinstance(attestation, dict),
                "this ledger holds the buyer's money in a SaleEscrow contract, so a "
                "release requires the signed escrow attestation that the contract will "
                "verify. There is no default and no release without it.")
        require(attestation.get("escrow_sale_id") == s["escrow_sale_id"],
                "the attestation names on-chain sale %s, not this sale's %s"
                % (attestation.get("escrow_sale_id"), s["escrow_sale_id"]))
        require(str(attestation.get("escrow_address") or "").lower()
                == str(s["escrow_address"] or "").lower(),
                "the attestation is for the escrow at %s, not this sale's %s"
                % (attestation.get("escrow_address"), s["escrow_address"]))
        require(attestation.get("escrow_chain_id") == s["escrow_chain_id"],
                "the attestation is for chain %s, not this sale's chain %s"
                % (attestation.get("escrow_chain_id"), s["escrow_chain_id"]))
        require(str(attestation.get("schwab_activity_id") or "") == str(d["transfer_id"]),
                "the attestation names incoming transfer %r, but the transfer this sale "
                "actually attributed is %r. One observed transfer, one release."
                % (attestation.get("schwab_activity_id"), d["transfer_id"]))
        require(attestation.get("observed_at") == d["posted_at"],
                "the attestation times the observation at %s, but the attributed "
                "transfer posted at %s"
                % (attestation.get("observed_at"), d["posted_at"]))
        require(attestation.get("amount_atoms") == s["gross"],
                "the attestation would release %s atoms, but this sale escrowed %s"
                % (attestation.get("amount_atoms"), s["gross"]))
        signature = attestation.get("signature")
        require(type(signature) is str and signature.startswith("0x")
                and len(signature) == 132,
                "the attestation carries no 65-byte attestor signature, so the contract "
                "would refuse it and the coins stay locked")
        require(bool(attestation.get("digest")), "the attestation carries no digest")
        return {"escrow_attestation_digest": attestation["digest"],
                "escrow_activity_id": attestation.get("escrow_activity_id"),
                "escrow_sale_id": s["escrow_sale_id"],
                "escrow_address": s["escrow_address"],
                "escrow_chain_id": s["escrow_chain_id"],
                "attestor_signature_present": True,
                "digest_confirmed_against_contract":
                    bool(attestation.get("digest_confirmed_against_contract"))}

    # --------------------------------------------------------- the two escrows

    def pull(self, sale, now):
        """Mode B only: a recorded, one-way pull of the buyer's payment. After it
        there is nothing to refund and the buyer holds a claim, not cash."""
        require(type(now) is int and now > 0, "authenticated time required")
        with self.atomic():
            s = self.sale(sale)
            require(s["mode"] == MODE_B,
                    "mode A escrow is non-withdrawable until release: there is no pull")
            require(s["state"] == "P", "only a paid, unreleased sale can be pulled")
            require(s["pulled_at"] is None, "this sale was already pulled; the pull is one-way")
            require(now <= s["deadline"],
                    "the deadline has passed: this sale failed and its payment cannot be pulled")
            self.db.execute("UPDATE sales SET pulled_at=? WHERE sale=?", (now, sale))
            self.log("escrow_pulled_one_way", sale,
                     {"amount": s["gross"], "at": now, "refund_available_after_this": False,
                      "buyer_now_holds": self.protection_text(MODE_B, True)})
        return self.buyer_protection(self.sale(sale))

    @staticmethod
    def protection_text(mode, pulled):
        if mode == MODE_A:
            return ("Mode A. The buyer's payment is non-withdrawable until release. If the "
                    "deadline passes the buyer takes it back unilaterally; the seller does "
                    "not have to act. The timeout refund is a real guarantee.")
        if not pulled:
            return ("Mode B, before the pull. The payment is still in the sale-bound escrow, "
                    "so for now a timeout refund works exactly as in mode A. That protection "
                    "ends the moment the seller pulls, which he may do at any time before "
                    "release without asking the buyer.")
        return ("Mode B, after the pull. There is NO cash to refund. The buyer holds an "
                "unsecured claim on the seller, not recoverable cash. If the sale now fails "
                "the coins go back to the locked pool and the buyer must rely on the seller "
                "repaying. This is not an escrow guarantee.")

    SETTLED_PROTECTION = {
        "C": "The sale completed. The buyer holds the coins and there is nothing left to "
             "refund, in either mode.",
        "B": "The sale completed and the holder is now redeeming. The exit cash is reserved "
             "against that redemption, not against the original payment.",
        "D": "The sale completed and the holder has been paid out in full. Nothing is "
             "outstanding.",
        "K": "The sale was cancelled and the buyer's payment was returned in full, proven.",
        "Q": "Nothing has been paid yet, so there is nothing to protect. The buyer's "
             "protection begins when the payment is accepted.",
    }

    FAILED_REFUND = (
        "The sale failed at its deadline. The buyer's payment never left the sale-bound "
        "escrow, so the buyer takes it back themselves, unilaterally. The seller does not "
        "have to act, agree, or even be reachable.")
    FAILED_CLAIM = (
        "The sale failed at its deadline and the payment had already been pulled. There is "
        "NO cash to refund. The buyer holds an unsecured claim on the seller, not "
        "recoverable cash. This is not an escrow guarantee.")

    def buyer_protection(self, s):
        pulled = s["pulled_at"] is not None
        if s["state"] in self.SETTLED_PROTECTION:
            return {"mode": s["mode"], "pulled": pulled, "recoverable": False,
                    "amount": 0, "kind": "nothing_outstanding",
                    "plain": self.SETTLED_PROTECTION[s["state"]]}
        recoverable = s["mode"] == MODE_A or not pulled
        if s["state"] == "F":
            if s["paid_at"] is None:
                return {"mode": s["mode"], "pulled": False, "recoverable": True, "amount": 0,
                        "kind": "nothing_paid",
                        "plain": "The quote expired before any payment arrived. No coins "
                                 "were ever reserved and the buyer paid nothing."}
            return {"mode": s["mode"], "pulled": pulled, "recoverable": recoverable,
                    "amount": s["gross"],
                    "kind": "escrow_refund" if recoverable else "unsecured_claim",
                    "plain": self.FAILED_REFUND if recoverable else self.FAILED_CLAIM}
        return {"mode": s["mode"], "pulled": pulled, "recoverable": recoverable,
                "amount": s["gross"],
                "kind": "escrow_refund" if recoverable else "unsecured_claim",
                "plain": self.protection_text(s["mode"], pulled)}

    # ---------------------------------------------------------------- timeout

    def fail_timeout(self, sale, now):
        """The deadline passed with no release. The coins go back to the dormant
        locked pool and the payment is permanently closed. Returns, in plain
        terms, what the buyer actually holds."""
        require(type(now) is int and now > 0, "authenticated time required")
        with self.atomic():
            s = self.sale(sale)
            require(s["state"] in OPEN_STATES, "this sale is not open")
            require(now > s["deadline"],
                    "the deadline has not passed: a refund is refused and only settlement "
                    "may proceed")
            if s["state"] == "Q":
                self.db.execute("UPDATE sales SET state='F' WHERE sale=?", (sale,))
                outcome = self.buyer_protection(self.sale(sale))
                self.log("quote_expired", sale, {"deadline": s["deadline"], "at": now})
                return outcome
            had_deposit = self.deposit_for(sale)
            self.db.execute("UPDATE sales SET state='F', payment_closed=1 WHERE sale=?", (sale,))
            outcome = self.buyer_protection(self.sale(sale))
            self.log("timeout_sale_failed", sale,
                     {"deadline": s["deadline"], "at": now, "quantity": s["quantity"],
                      "coins": "returned to the dormant locked pool",
                      "payment_closed": True, "buyer_outcome": outcome["kind"],
                      "refund_recoverable": outcome["recoverable"],
                      "amount": outcome["amount"], "plain": outcome["plain"],
                      "qualifying_deposit_was_recorded": had_deposit is not None})
            return outcome

    # ----------------------------------------------------------- exit and cancel

    def request_exit(self, sale, caller, now):
        with self.atomic():
            self.fresh(now)
            s = self.sale(sale)
            require(s["state"] == "C" and caller == s["buyer"], "exit ownership or state mismatch")
            # Queue preserves the full NAV obligation until exact payout finality.
            due = s["quantity"] * self.meta()["nav"]
            self.db.execute("UPDATE sales SET state='B', exit_due=? WHERE sale=?", (due, sale))
            require(self.meta()["net"] - self.meta()["debits"] >= self.obligation(),
                    "exit impaired")
            self.log("exit_queued", sale, {"gross_due": due})

    def finish_exit(self, sale, proof):
        r = self.evidence(proof, "exit_payment")
        with self.atomic():
            s = self.sale(sale)
            require(s["state"] == "B", "exit not pending")
            fee = (s["exit_due"] * self.terms.exit_fee_bps + 9999) // 10000
            require(r.get("sale") == sale and r.get("recipient") == s["buyer"] and
                    r.get("cash_asset") == self.terms.payment_asset, "exit binding mismatch")
            require(r.get("finalized") is True and r.get("success") is True and
                    r.get("amount") == s["exit_due"] - fee and r.get("fee_paid") == fee,
                    "partial or failed exit transfer")
            require(self.meta()["net"] - self.meta()["debits"] >= self.obligation(),
                    "exit impaired")
            try:
                self.db.execute("INSERT INTO outputs VALUES(?,?)", (r["receipt"], sale))
            except sqlite3.IntegrityError as exc:
                raise Rejected("exit receipt already used") from exc
            self.db.execute("UPDATE meta SET debits=debits+? WHERE id=1", (s["exit_due"],))
            self.db.execute("UPDATE sales SET state='D' WHERE sale=?", (sale,))
            self.log("exit_complete", sale, {"net_paid": r["amount"], "fee_paid": fee})

    def cancel_paid(self, sale, proof):
        r = self.evidence(proof, "cash_return")
        with self.atomic():
            s = self.sale(sale)
            require(s["state"] == "P", "sale not cancellable")
            require(r.get("sale") == sale and r.get("recipient") == s["buyer"] and
                    r.get("cash_asset") == self.terms.payment_asset and
                    r.get("amount") == s["gross"] and
                    r.get("finalized") is True and r.get("success") is True,
                    "cash return not complete")
            try:
                self.db.execute("INSERT INTO outputs VALUES(?,?)", (r["receipt"], sale))
            except sqlite3.IntegrityError as exc:
                raise Rejected("cash return receipt already used") from exc
            d = self.deposit_for(sale)
            if d is not None and d["nullifier"] in json.loads(self.meta()["covered"]):
                self.db.execute("UPDATE meta SET debits=debits+? WHERE id=1", (s["settled"],))
            self.db.execute("UPDATE sales SET state='K', payment_closed=1 WHERE sale=?", (sale,))
            self.log("cash_return_complete", sale, {"amount": s["gross"]})

    def freeze(self):
        with self.atomic():
            self.db.execute("UPDATE meta SET frozen=1 WHERE id=1")
            self.log("freeze", "", {})

    def govern(self, **changes):
        raise Rejected("governance cannot override inventory, proofs, terms, or accounting")

    # ----------------------------------------------------------------- supply

    def supply(self):
        result = dict.fromkeys(("L", "P", "C", "X", "W", "B", "D"), 0)
        for s in self.db.execute("SELECT state,quantity FROM sales"):
            if s["state"] in COIN_STATES:
                result[s["state"]] += s["quantity"]
        result["L"] = self.terms.opening - sum(result.values())
        return result

    def check_conservation(self):
        supply = self.supply()
        require(all(v >= 0 for v in supply.values()) and
                sum(supply.values()) == self.terms.opening,
                "inventory conservation violation")

    def sales_summary(self):
        rows = []
        for s in self.db.execute("SELECT * FROM sales ORDER BY rowid"):
            d = self.deposit_for(s["sale"])
            rows.append({"sale": s["sale"], "state": s["state"], "mode": s["mode"],
                         "quantity": s["quantity"], "gross": s["gross"],
                         "required_deposit": s["required_deposit"], "deadline": s["deadline"],
                         "quoted_at": s["quoted_at"], "paid_at": s["paid_at"],
                         "pulled_at": s["pulled_at"], "buyer": s["buyer"],
                         "payment_closed": bool(s["payment_closed"]),
                         "deposit": None if d is None else
                         {"transfer_id": d["transfer_id"], "amount": d["amount"],
                          "posted_at": d["posted_at"], "evidence_class": d["source"],
                          "broker_entry_type": d["entry_type"]},
                         "buyer_protection": self.buyer_protection(s)})
        return rows

    def public_state(self):
        supply = self.supply()
        m = self.meta()
        return {"supply": supply, "circulating": supply["C"] + supply["W"],
                "active": supply["C"] + supply["X"] + supply["W"],
                "net_after_debits": m["net"] - m["debits"], "obligation": self.obligation(),
                "frozen": bool(m["frozen"]),
                "events": [dict(x) for x in self.db.execute("SELECT * FROM journal ORDER BY id")]}
