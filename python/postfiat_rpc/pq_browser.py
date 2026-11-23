"""Owner-local wallet-web bridge. Only public pages and proven actions traverse the gateway."""
import base64
import contextlib
import fcntl
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import http.client
import json
import mimetypes
import os
from pathlib import Path
import re
import secrets
import signal
import sqlite3
import threading
from urllib.parse import urlsplit

from .client import Endpoint, PostFiatRpcClient, PQ_POOL_ID, private_action_hash, private_action_intent_id
from .pq_wallet import issued_fee, local, snapshot, sync

NATIVE = "00" * 48
ORCHARD = "asset-orchard-v1"
MODES = ("Shield", "Internal", "Exit")
SCHEMA = "postfiat-private-actions-v1"
MAX_RESPONSE = 4 * 1024 * 1024
ORCHARD_FIELDS = ("kind version schema pool_id to asset_id amount fee policy_id disclosure_hash proof_system_id "
                      "circuit_id pool_domain anchor nullifier randomized_verification_key asset_tag_lo asset_tag_hi "
                      "exit_binding_hash proof spend_authorization_signature").split()


def compact(value):
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False)


def check(condition, message):
    if not condition:
        raise ValueError(message)


def atomic_number(value):
    check(isinstance(value, str) and re.fullmatch(r"[1-9][0-9]{0,19}", value)
          and int(value) < 2**64, "Use positive integer atoms.")
    return int(value)


class AdmissionRefused(ValueError):
    """The authority confirmed the submitted intent has no reservation."""


class Gateway:
    def __init__(self, url, token_file, origin):
        self.url = urlsplit(url)
        check(self.url.scheme == "https" or (self.url.scheme == "http" and self.url.hostname == "127.0.0.1"),
              "The gateway needs HTTPS or numeric loopback.")
        check(self.url.hostname and not self.url.username and not self.url.password
              and self.url.path in ("", "/") and not self.url.query and not self.url.fragment,
              "Supply a gateway origin.")
        self.token = token_file.read_text().strip()
        self.origin = origin

    def call(self, route, body, pool=PQ_POOL_ID):
        connection = http.client.HTTPSConnection if self.url.scheme == "https" else http.client.HTTPConnection
        prefix = "/api/private/v1/pq/" if pool == PQ_POOL_ID else "/api/private/v1/"
        with contextlib.closing(connection(self.url.hostname, self.url.port, timeout=90)) as conn:
            conn.request("POST", prefix + route, compact(body), {
                "Content-Type": "application/json", "Authorization": "Bearer " + self.token, "Origin": self.origin,
            })
            response = conn.getresponse()
            raw = response.read(MAX_RESPONSE + 1)
            check(len(raw) <= MAX_RESPONSE, "Gateway response exceeded its size limit.")
            if response.status == 409 and route == "submit":
                error = json.loads(raw)
                if (error.get("schema") == SCHEMA and error.get("reserved") is False
                        and re.fullmatch("[a-z0-9_]{1,64}", str(error.get("code", "")))):
                    raise AdmissionRefused(error["code"])
            check(response.status in (200, 202), f"Gateway refused {route} (HTTP {response.status}).")
            return json.loads(raw)

    def private_pool_page(self, token, section, *, cursor, limit):
        return self.call("page", dict(snapshot_token=token, section=section, cursor=cursor, limit=limit))


class BrowserWallet:
    def __init__(self, args, origin):
        self.home = args.wallet.absolute()
        self.validator = PostFiatRpcClient(Endpoint.parse(args.validator_endpoint or args.endpoint), timeout_seconds=90,
                                           response_byte_cap=MAX_RESPONSE)
        self.gateway = Gateway(args.gateway, args.gateway_token_file, origin)
        self.account = args.payer
        self.key = str(args.account_key.absolute()) if args.account_key else None
        check(bool(self.account) == bool(self.key), "Supply both deposit account and signing key for shielding.")
        self.issue = args.issue
        check((self.account is None or re.fullmatch(r"pf[0-9a-f]{40}", self.account)) and re.fullmatch(r"[0-9a-f]{96}", self.issue)
              and self.issue != NATIVE, "Invalid public account or source-series ID.")
        self.net = local(self.home, "checkpoint")["network"]
        self.exit_action = None
        if args.orchard_exit_batch:
            batch = json.loads(args.orchard_exit_batch.read_bytes())
            [exit_action] = batch["actions"]
            check(set(exit_action) == set(ORCHARD_FIELDS) and exit_action["kind"] == "asset_orchard_private_egress_v1"
                  and exit_action["pool_id"] == ORCHARD and exit_action["asset_id"] == self.issue
                  and exit_action["to"] == self.account and exit_action["fee"] == 0
                  and all(isinstance(v, (str, int)) for v in exit_action.values()),
                  "Expected one public Orchard exit to this account and issue.")
            self.exit_action = {field: exit_action[field] for field in ORCHARD_FIELDS}
        self.reviewed = None
        self.synced = False
        self.mutex = threading.Lock()
        self.db = self.home / "browser-payments.sqlite3"
        with contextlib.closing(self.connection()) as db:
            db.execute("CREATE TABLE IF NOT EXISTS payments "
                       "(intent TEXT PRIMARY KEY, pool TEXT, wire TEXT, phase TEXT, migration TEXT, network TEXT, statement_id TEXT)")
            if "statement_id" not in {r["name"] for r in db.execute("PRAGMA table_info(payments)")}:
                db.execute("ALTER TABLE payments ADD COLUMN statement_id TEXT")
            db.execute("CREATE UNIQUE INDEX IF NOT EXISTS migration_step ON payments(migration,network) "
                       "WHERE migration IS NOT NULL")

    def connection(self):
        conn = sqlite3.connect(self.db, isolation_level=None)
        conn.row_factory = sqlite3.Row
        return conn

    def exit_id(self):
        return private_action_intent_id(**self.net, action_json=compact(self.exit_action)) if self.exit_action else None

    def row(self, db, intent):
        row = db.execute("SELECT * FROM payments WHERE intent=? AND network=?", (intent, compact(self.net))).fetchone()
        check(row is not None, "Unknown wallet payment.")
        return row

    def action_row(self, db, action, pool, migration=None, statement_id=None):
        wire = compact(action)
        intent = private_action_intent_id(**self.net, action_json=wire)
        db.execute("INSERT INTO payments (intent,pool,wire,phase,migration,network,statement_id) VALUES (?,?,?,?,?,?,?)",
                   (intent, pool, wire, "prepared", migration, compact(self.net), statement_id))
        return self.row(db, intent)

    def request(self, row):
        address = self.account
        if row["pool"] == PQ_POOL_ID:
            wire = base64.b64decode(json.loads(row["wire"])["body"], validate=True)
            if wire[71] == 0:
                address = None
        return dict(address=address, intent_id=row["intent"], action_json=row["wire"])

    def send(self, db, row):
        # Commit the uncertain state before the first network byte. No background resend.
        changed = db.execute("UPDATE payments SET phase='submitted' WHERE intent=? AND phase='prepared'",
                             (row["intent"],)).rowcount
        check(changed == 1, "Already submitted. Check finality.")
        try:
            r = self.gateway.call("submit", self.request(row), row["pool"])
        except AdmissionRefused as exc:
            if row["pool"] == PQ_POOL_ID:
                local(self.home, "rejected", action_hash=private_action_hash(row["wire"]))
                db.execute("DELETE FROM payments WHERE intent=?", (row["intent"],))
            raise ValueError(f"Validator refused payment ({exc}). Review again.") from exc
        check(r["schema"] == SCHEMA and r["intent_id"] == row["intent"]
              and r["action_hash"] == private_action_hash(row["wire"]), "Admission returned different bytes.")
        if row["pool"] == PQ_POOL_ID:
            check(row["statement_id"] and r.get("statement_id") == row["statement_id"], "Gateway statement mismatch.")
        return dict(intent=row["intent"], state=r["state"])

    def receipt(self, db, intent):
        row = self.row(db, intent)
        r = self.gateway.call("receipt", self.request(row), row["pool"])
        check(r["schema"] == SCHEMA and r["intent_id"] == intent and r["action_hash"] == private_action_hash(row["wire"]),
              "Receipt identity mismatch.")
        if r["state"] != "accepted":
            return dict(intent=intent, state=r["state"], code=r.get("code"))
        preimage = compact([self.net["chain_id"], self.net["genesis_hash"], self.net["protocol_version"],
                            "shielded", [json.loads(row["wire"])]])
        expected_batch = hashlib.sha3_384(b"postfiat.shielded_action_batch.atomic.v2\0" + preimage.encode()).hexdigest()
        check(r["batch_id"] == expected_batch, "Receipt covers a different payment.")
        h = r["inclusion_height"]
        [receipt] = self.validator.receipts(tx_id=r["transaction_id"], limit=1)
        check(receipt["tx_id"] == r["transaction_id"] and receipt["accepted"] is True
              and receipt["code"] == r["code"] and receipt["fee_charged"] == r["fee"],
              "Receipt differs from the trusted validator.")
        [block] = self.validator.blocks(from_height=h, limit=1)
        b = block["header"]
        cert = b["certificate"]
        votes = {v["validator"] for v in cert["votes"] if v["accept"]}
        check(r["accepted"] is True and b["height"] == h and b["batch_id"] == r["batch_id"]
              and r["transaction_id"] in block["receipt_ids"] and b["state_root"] == r["state_root"]
              and b["block_hash"] == r["block_hash"] and json.loads(r["finality"]["certificate_json"]) == cert
              and 0 < cert["quorum"] <= len(votes.intersection(cert["validators"])), "Unconfirmed validator receipt.")
        db.execute("UPDATE payments SET phase='final' WHERE intent=?", (intent,))
        return dict(intent=intent, state="final", height=h, receipt=r["transaction_id"], certificate=b["certificate_id"])

    def migration(self, db):
        exit_id = self.exit_id()
        if not exit_id:
            return None
        action = self.exit_action
        data = {k: action[k] for k in ("asset_id", "amount", "fee", "to", "policy_id", "disclosure_hash")}
        data.update(amount=str(action["amount"]), fee=str(action["fee"]))
        rows = {r["migration"]:r for r in db.execute("SELECT * FROM payments WHERE migration IN (?,?) AND network=?",
                (exit_id, exit_id + "/shield", compact(self.net)))}
        exit_row = rows.get(exit_id)
        shield = rows.get(exit_id + "/shield")
        state = "Review Orchard exit"
        if exit_row:
            state = "Exit final — review new shield" if exit_row["phase"] == "final" else "Awaiting Orchard finality"
        if shield:
            state = "Migration final" if shield["phase"] == "final" else "PQ shield awaiting finality"
        current = shield or exit_row
        return dict(**data, state=state, exit_intent=exit_id, intent=current["intent"] if current else None)

    def overview(self, db, *, public=False):
        balance = local(self.home, "balance")
        checkpoint = local(self.home, "checkpoint")
        tip = self.gateway.call("snapshot", {})
        check(all(tip[k] == v for k, v in self.net.items()) and tip["pool_id"] == PQ_POOL_ID,
              "Gateway snapshot is on a different network.")
        trusted = self.validator.private_pool_snapshot(PQ_POOL_ID)
        account = self.validator.account(self.account) if public and self.account else None
        lines = self.validator.account_lines(self.account, asset_id=self.issue)["lines"] if public and self.account else []
        last = db.execute("SELECT intent,phase FROM payments WHERE network=? ORDER BY rowid DESC LIMIT 1",
                          (compact(self.net),)).fetchone()
        fee = None
        if trusted.get("pq_issued_fee"):
            fee = str(issued_fee(trusted, self.issue))
        return dict(issued_fee=fee, network=self.net, account=self.account, issue=self.issue,
                    address=local(self.home, "address")["address"], sequence=str(account["sequence"] + 1) if account else None,
                    public={NATIVE:str(account["balance"]), self.issue:str(sum(l["balance"] for l in lines))} if account else {},
                    private={k:str(v) for k,v in balance["finalized"].items()},
                    pending=({k:str(v) for k,v in balance["pending"].items()} if balance["pending"] else None),
                    height=checkpoint["height"], tip=trusted["finalized_height"],
                    synced=self.synced and tip["snapshot_token"] == trusted["snapshot_token"]
                    and checkpoint["height"] == trusted["finalized_height"]
                    and checkpoint["last_page"] == tip.get("pq_last_page"),
                    migration=self.migration(db), last=dict(last) if last else None)

    def run(self, cmd, body):
        if cmd == "progress":
            return local(self.home, "progress")
        if cmd in {"status", "activity"}:
            with contextlib.closing(self.connection()) as db:
                return self.dispatch(db, cmd, body)
        check(self.mutex.acquire(blocking=False), "Wallet busy.")
        try:
            with contextlib.closing(self.connection()) as db:
                return self.dispatch(db, cmd, body)
        finally:
            self.mutex.release()

    def dispatch(self, db, cmd, body):
        if cmd == "status":
            return self.overview(db, public=body.get("public") is True)
        if cmd == "sync":
            self.synced = False
            self.reviewed = None
            result = sync(self.home, self.gateway, self.validator)
            self.synced = result["sync_lag_blocks"] == 0
            return self.overview(db, public=body.get("public") is True)
        if cmd == "activity":
            asset, mode = body["asset"], body["mode"]
            check(asset in (NATIVE, self.issue) and mode in MODES, "Unknown issue or route.")
            token, point = snapshot(self.validator)
            # A bounded recent public range is an upper bound on plausible actions.
            # No inference about time/fee/RPC joint partitions follows from this count.
            from .client import pq_public_pages
            offset = max(0, point["pairs"] - 128)
            count = sum(row["asset"] == asset and row["kind"] == mode
                        for page in pq_public_pages(self.validator, token, "pq_pairs", point["pairs"], offset)
                        for row in page)
            return dict(count=count, sampled=point["pairs"]-offset, height=point["height"])
        if cmd == "receipt":
            return self.receipt(db, body["intent"])
        if cmd == "exit":
            check(self.exit_action and body == {"intent":self.exit_id()}, "Changed Orchard exit.")
            return self.send(db, self.action_row(db, self.exit_action, ORCHARD, self.exit_id()))
        if cmd == "review":
            self.reviewed = None
            state = self.overview(db, public=body.get("mode") == "Shield")
            check(state["synced"] and state["pending"] is None, "Sync the wallet and resolve pending payments first.")
            check(set(body) == {"mode","asset","amount","fee","recipient","migration"}, "Unexpected payment field.")
            mode, asset = body["mode"], body["asset"]
            check(mode in MODES and asset in (NATIVE, self.issue), "Unknown payment mode or issue.")
            atomic_number(body["amount"])
            if mode != "Shield" and asset != NATIVE:
                check(state["issued_fee"] is not None and body["fee"] == state["issued_fee"],
                      "Issued fee changed or unavailable. Sync and review again.")
            atomic_number(body["fee"])
            recipient = body["recipient"]
            check((mode == "Shield" and recipient is None) or
                  (mode == "Exit" and isinstance(recipient, str) and re.fullmatch(r"pf[0-9a-f]{40}", recipient)) or
                  (mode == "Internal" and isinstance(recipient, str) and len(recipient) == 2241),
                  "Invalid destination.")
            check(type(body["migration"]) is bool, "Invalid migration request.")
            if body["migration"]:
                migration = state["migration"]
                check(migration and migration["state"] == "Exit final — review new shield"
                      and self.receipt(db, self.exit_id())["state"] == "final"
                      and mode == "Shield" and body["amount"] == migration["amount"] and asset == migration["asset_id"],
                      "Confirm the exact Orchard exit before shielding.")
            public = mode == "Shield"
            check(not public or self.account, "Configure a deposit account to shield funds.")
            terms = dict(**body, payer=self.account if public else None,
                         sequence=state["sequence"] if public else None)
            token = secrets.token_urlsafe(32)
            self.reviewed = (token, terms)
            return dict(review=token, terms=terms)
        if cmd == "prove":
            review = self.reviewed
            self.reviewed = None
            check(review and body == dict(review=review[0], terms=review[1]), "Reviewed terms changed or expired.")
            terms = review[1]
            current = self.overview(db, public=terms["mode"] == "Shield")
            if terms["mode"] != "Shield" and terms["asset"] != NATIVE:
                check(terms["fee"] == current["issued_fee"], "Issued fee changed. Sync and review again.")
            check(current["synced"], "Sync again before signing.")
            check(not terms["payer"] or terms["sequence"] == current["sequence"],
                  "Public account sequence changed. Review the payment again.")
            order = {k:v for k,v in terms.items() if k != "migration"}
            order.update(amount=int(order["amount"]), fee=int(order["fee"]),
                         sequence=int(order["sequence"]) if order["payer"] else None,
                         account_key=self.key if order["payer"] else None)
            metrics = local(self.home, "prove", order=order)
            action = local(self.home, "prepared")
            action = dict(kind=action["kind"], body=action["body"])
            check(private_action_hash(compact(action)) == metrics["action_hash"], "Prepared payment changed.")
            migration = self.exit_id() + "/shield" if terms["migration"] else None
            row = self.action_row(db, action, PQ_POOL_ID, migration, metrics["statement_id"])
            return dict(intent=row["intent"], action_json=row["wire"], action_hash=metrics["action_hash"])
        if cmd == "submit":
            row = self.row(db, body["intent"])
            check(row["pool"] == PQ_POOL_ID and body["action_hash"] == private_action_hash(row["wire"]),
                  "Prepared payment changed.")
            return self.send(db, row)
        raise ValueError("Unknown wallet operation.")


def serve(args):
    check(args.validator_endpoint or args.endpoint, "A trusted validator endpoint is required.")
    os.umask(0o077)
    dist = args.dist.resolve(strict=True)
    index = (dist / "index.html").read_bytes()
    check(not args.pair_file.resolve().is_relative_to(dist), "Keep the pairing file outside the served wallet.")
    home = args.wallet.absolute()
    # One browser bridge owns the review state for this daemon at a time.
    lock = open(home / "browser.lock", "a")
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    token = secrets.token_urlsafe(48)

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def reply(self, status, payload, mime="application/json"):
            self.send_response(status)
            self.send_header("Content-Type", mime)
            self.send_header("Content-Length", str(len(payload)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("Referrer-Policy", "no-referrer")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.send_header("Content-Security-Policy", "default-src 'self'; connect-src 'self'; "
                             "script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; "
                             "base-uri 'none'; frame-ancestors 'none'; form-action 'none'; object-src 'none'")
            self.end_headers()
            self.wfile.write(payload)

        def do_GET(self):
            if self.headers.get("Host") != authority:
                return self.reply(403, b"{}")
            if self.path == "/private":
                return self.reply(200, index, "text/html; charset=utf-8")
            if re.fullmatch(r"/assets/[a-zA-Z0-9_.-]+", self.path):
                asset = dist / self.path[1:]
                if asset.is_file() and asset.resolve().parent == dist / "assets":
                    return self.reply(200, asset.read_bytes(), mimetypes.guess_type(asset)[0] or "application/octet-stream")
            self.reply(404, b"{}")

        def do_POST(self):
            if (self.headers.get("Host") != authority or self.headers.get("Origin") != origin
                    or not secrets.compare_digest(self.headers.get("Authorization", "").encode(),
                                                  ("Bearer " + token).encode())):
                return self.reply(403, b'{"error":"Open the pairing link from your local wallet."}')
            if (self.path != "/local/pq/v1" or self.headers.get("Content-Type") != "application/json"
                    or not self.headers.get("Content-Length", "").isdigit()):
                return self.reply(400, b"{}")
            size = int(self.headers["Content-Length"])
            if size > 16_384:
                return self.reply(413, b"{}")
            try:
                data = json.loads(self.rfile.read(size))
                result = wallet.run(data["cmd"], data.get("body", {}))
                payload = compact(result).encode()
            except (ConnectionRefusedError, FileNotFoundError):
                return self.reply(503, b'{"error":"Disconnected. Check the local wallet daemon and validator."}')
            except ValueError as exc:
                # Only local validation messages; never reflect malformed upstream bodies.
                msg = str(exc) if type(exc) is ValueError else "Invalid wallet response."
                return self.reply(409, compact({"error":msg}).encode())
            except Exception:
                return self.reply(503, b'{"error":"Wallet request could not be confirmed. Check status before retrying."}')
            self.reply(200, payload)

    with ThreadingHTTPServer(("127.0.0.1", args.port), Handler) as server:
        authority = f"127.0.0.1:{server.server_port}"
        origin = "http://" + authority
        wallet = BrowserWallet(args, origin)
        # Never place the session capability in a URL query, logs or remote storage.
        link = origin + "/private#" + token
        def stop(*_):
            raise KeyboardInterrupt

        signal.signal(signal.SIGTERM, stop)
        with open(args.pair_file, "x") as pair:
            pair.write(link + "\n")
        print(f"Private wallet listening at {origin}/private. Open the link in your owner-only pairing file.", flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass
        finally:
            args.pair_file.unlink(missing_ok=True)
            lock.close()
