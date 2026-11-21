"""Pinned proof operations for the node, and a separately labelled local receipt ledger."""

import hashlib
import json
import re
import select
import socket
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed
import sqlite3
import subprocess
import tempfile
import time
import uuid
from pathlib import Path

from prover import verify
from source import journal
from storage import DEPOSIT, Invalid, encode, load, save

KINDS = {
    "deposit": DEPOSIT,
    "handoff": "sp1-solana-alpenglow-handoff-v1",
    "keys": "stub-key-registration",
}
OP = "solana_bridge_proof_v1"
OP_FIELDS = ("submitter", "profile_hash", "proof_kind", "proof", "public_values")


def operation(cfg, kind, job):
    return {
        "operation": OP,
        "submitter": cfg["submitter"],
        "profile_hash": cfg["profile_hash"],
        "proof_kind": KINDS[kind],
        "proof": list((job / "proof.bin").read_bytes()),
        "public_values": list((job / "public.bin").read_bytes()),
    }


def node_tx_id(signed):
    # SignedAssetTransaction::tx_id_preimage_bytes and SolanaProofOperation::signing_bytes.
    u = signed["unsigned"]
    fields = (
        "chain_id",
        "genesis_hash",
        "protocol_version",
        "address_namespace",
        "transaction_kind",
        "signature_algorithm_id",
        "source",
        "fee",
        "sequence",
        "operation",
    )
    data = (
        "postfiat.asset_transaction.v1\n" + "".join(f"{k}={u[k]}\n" for k in fields)
    ).encode()
    data += encode({k: u[k] for k in OP_FIELDS})
    data += f"algorithm={signed['algorithm_id']}\npublic_key={signed['public_key_hex']}\nsignature={signed['signature_hex']}\n".encode()
    return hashlib.sha3_384(b"postfiat.asset_transaction.tx_id.v1\0" + data).hexdigest()


class NodeReplyError(RuntimeError):
    """An error response correlated to a particular node RPC request."""

    def __init__(self, request_id, error):
        super().__init__(str(error))
        self.request_id = request_id
        self.error = error


class Node:
    def __init__(self, cfg):
        self.cfg = cfg
        self.pool = ThreadPoolExecutor(max_workers=10)
        self.signer_lock = threading.Lock()
        self.signer = None
        self.checked_activation = None

    @staticmethod
    def connect(addr):
        sock = socket.create_connection(tuple(addr), timeout=10)
        sock.settimeout(120)
        return sock

    def rpc(self, addr, method, params, *, connection=None):
        rid = uuid.uuid4().hex
        req = {
            "version": "postfiat-local-rpc-v1",
            "id": rid,
            "method": method,
            "params": params,
        }
        with connection or self.connect(addr) as sock:
            sock.sendall(encode(req) + b"\n")
            with sock.makefile("rb") as stream:
                reply = json.loads(stream.readline(8 << 20))
        if reply.get("id") != rid:
            raise RuntimeError("mismatched node RPC response id")
        if reply.get("ok") is False and isinstance(reply.get("error"), dict):
            raise NodeReplyError(rid, reply["error"])
        if reply.get("ok") is not True:
            raise RuntimeError("invalid node RPC reply")
        return reply["result"]

    def route(self):
        cfg = self.cfg
        if self.checked_activation is not None:
            return self.checked_activation
        if "governance_receipt" in cfg:
            # Release 6's family-level read selects the active EVM route. The
            # signed Solana activation is bound to its profile hash in governance.
            evidence = cfg["governance_receipt"]
            result = self.rpc(cfg["rpc_peers"][0], "tx", {"tx_id": evidence["tx_id"]})
            if not (
                result["confirmed"]
                and result["receipt"]["accepted"]
                and result["receipt"]["tx_id"] == evidence["tx_id"]
                and result["block"]["header"]["state_root"] == evidence["root"]
            ):
                raise Invalid("Solana activation receipt mismatch")
            route = {
                "chain_id": result["chain_id"],
                "genesis_hash": result["genesis_hash"],
                "profile": evidence["profile"],
                "profile_hash": evidence["profile_hash"],
                "active": True,
            }
        else:
            route = self.rpc(
                cfg["rpc_peers"][0], "vault_bridge_route", {"asset_id": cfg["asset_id"]}
            )
        profile = route["profile"]
        pins = profile.get("solana") or {}
        if not (
            route["active"] is True
            and route["chain_id"] == cfg["chain_id"]
            and route["genesis_hash"] == cfg["genesis_hash"]
            and route["profile_hash"] == cfg["profile_hash"]
            and profile["asset_id"] == cfg["asset_id"]
            and profile["route_id"] == cfg["route_id"]
            and pins.get("genesis_hash") == cfg["solana_genesis_hash"]
            and pins.get(
                "route_binding",
                profile.get("route_binding", route.get("route_binding")),
            )
            == cfg["route_binding"]
            and profile["verifier_kind"] == DEPOSIT
            and profile["verifier_program_vkey"] == cfg["deposit_vkey"]
            and profile["vault_address"] == cfg["vault_program"]
            and pins.get("recorder_program") == cfg["recorder_program"]
            and pins.get("handoff_program_vkey")
            == cfg.get("handoff_vkey", cfg.get("recorder_vkey"))
        ):
            raise Invalid("governed Solana route differs from relay configuration")
        if "governance_receipt" in cfg:
            self.checked_activation = route
        return route

    def credits(self, values, selected):
        cfg = self.cfg
        if cfg.get("warm_signer") and "governance_receipt" in cfg:
            return self.governed_credits(values, selected)
        try:
            report = self.rpc(
                cfg["rpc_peers"][0],
                "vault_bridge_status",
                {"asset_id": cfg["asset_id"]},
            )
        except RuntimeError as error:
            if (
                "governance_receipt" not in cfg
                or "bucket source domain does not match governed route"
                not in str(error)
            ):
                raise
            # Stock R6's family report expects decimal EVM chain IDs in every bucket.
            # The caller already checked the signed transaction's accepted receipt.
            # Derive its consensus IDs and read the beneficiary's source series directly.
            return self.governed_credits(values, selected)
        indexed = {r["source_tx_or_attestation"]: r for r in report["receipts"]}
        result = []
        for item in selected:
            row = indexed.get("solana_deposit:" + item["deposit_id"])
            if row is None:
                raise RuntimeError(
                    "committed Solana backing receipt is not yet readable"
                )
            finality = f"solana_slot:{values['slot']}:{values['block_id']}:{item['receipt_address']}"
            if (
                str(row["amount_atoms"]) != item["amount_atoms"]
                or row["vault_id"] != cfg["vault_program"]
                or row["finality_ref"] != finality
                or report["asset_id"] != cfg["asset_id"]
            ):
                raise Invalid("committed backing receipt differs from proven deposit")
            series = hashlib.sha3_384(
                b"postfiat.pfusdc.solana.source_series.v1\0"
                + f"{cfg['chain_id']}\n{cfg['asset_id']}\n{row['bucket_id']}".encode()
            ).hexdigest()
            result.append(
                dict(item, receipt_id=row["receipt_id"], credited_asset_id=series)
            )
        return result

    def governed_credits(self, values, selected, committed=None):
        cfg = self.cfg
        profile = cfg["governance_receipt"]["profile"]
        domain = f"spl:{cfg['solana_genesis_hash']}:{cfg['vault_program']}:{profile['token_address']}"
        chain, asset, policy = cfg["chain_id"], cfg["asset_id"], cfg["profile_hash"]

        def identifier(tag, body):
            return hashlib.sha3_384(tag.encode() + b"\0" + body.encode()).hexdigest()

        bucket = identifier(
            "postfiat.vault_bridge_bucket_id.v1",
            f"asset_id={asset}\nsource_domain_bytes={len(domain)}\nsource_domain={domain}\npolicy_hash={policy}\n",
        )
        series = identifier(
            "postfiat.pfusdc.solana.source_series.v1", f"{chain}\n{asset}\n{bucket}"
        )
        result = []
        for item in selected:
            addr = item["pftl_recipient"]
            observation = self.fast_balance(addr, series, committed)
            source = "solana_deposit:" + item["deposit_id"]
            finality = f"solana_slot:{values['slot']}:{values['block_id']}:{item['receipt_address']}"
            preimage = (
                f"chain_id={chain}\nasset_id={asset}\nsource_domain_bytes={len(domain)}\nsource_domain={domain}\n"
                f"source_tx_or_attestation_bytes={len(source)}\nsource_tx_or_attestation={source}\n"
                f"finality_ref_bytes={len(finality)}\nfinality_ref={finality}\namount_atoms={item['amount_atoms']}\npolicy_hash={policy}\n"
            )
            result.append(
                dict(
                    item,
                    credited_asset_id=series,
                    receipt_id=identifier(
                        "postfiat.vault_bridge_receipt_id.v1", preimage
                    ),
                    readback="accepted_operation_and_account_lines",
                    balance_observation=observation,
                )
            )
        return result

    def fast_balance(self, address, series, committed=None):
        params = {"account": address, "asset_id": series, "limit": 500}

        def observe(i, peer):
            if committed and i != committed[0]:
                # Deferred propagation can leave a perfectly valid older balance
                # on a fast peer. Only race reads after the peer has applied this height.
                head = self.rpc(peer, "status", {})
                header = committed[1]
                if head["block_height"] < header["height"] or (
                    head["block_height"] == header["height"]
                    and head["state_root"] != header["state_root"]
                ):
                    return None
            report = self.rpc(peer, "account_lines", params)
            matching = [
                int(row["balance"])
                for row in report["lines"]
                if row["asset_id"] == series
            ]
            if (
                report["account"] == address
                and report["chain_id"] == self.cfg["chain_id"]
                and report["genesis_hash"] == self.cfg["genesis_hash"]
                and matching
            ):
                return dict(validator=i, balance=sum(matching), time=time.time())
            return None

        # Deferred replication uses the same database writer as the status RPC.
        # The proposer has already committed. Hedge to the remaining peers if
        # its read stalls or fails, without competing with every inbound apply.
        peers = self.cfg["rpc_peers"]
        first = committed[0] if committed else None
        reads = []
        if first is not None:
            preferred = self.pool.submit(observe, first, peers[first])
            reads.append(preferred)
            try:
                observation = preferred.result(timeout=1.25)
                if observation:
                    return observation
            except (OSError, RuntimeError):
                pass
        reads.extend(
            self.pool.submit(observe, i, peer)
            for i, peer in enumerate(peers)
            if i != first
        )
        try:
            for future in as_completed(reads):
                try:
                    observation = future.result()
                except (OSError, RuntimeError):
                    continue
                if observation:
                    return observation
            raise RuntimeError("source series not visible on any configured validator")
        finally:
            for future in reads:
                future.cancel()

    def quote(self, op, peer):
        cfg = self.cfg
        quote = self.rpc(
            peer,
            "asset_fee_quote",
            {"source": cfg["submitter"], "operation_json": encode(op).decode()},
        )
        if (
            quote["operation"] != op
            or quote["source"] != cfg["submitter"]
            or quote["genesis_hash"] != cfg["genesis_hash"]
            or quote["chain_id"] != cfg["chain_id"]
        ):
            raise Invalid("fee quote differs from requested chain or operation")
        return quote

    def prepare(self, kind, path, generation):
        cfg = self.cfg
        route = self.pool.submit(self.route)
        # Rust charges the encoded operation weight and decoded receipt count.
        # The actual native journal is known now. Three decimal digits per proof
        # byte give an upper bound for the fixed-size Groth16 wire encoding.
        template = dict(
            operation=OP,
            submitter=cfg["submitter"],
            profile_hash=cfg["profile_hash"],
            proof_kind=KINDS[kind],
            proof=[255] * 356,
            public_values=list((path / "native.bin").read_bytes()),
        )
        head = self.rpc(cfg["rpc_peers"][0], "status", {})
        if (head["genesis_hash"], head["chain_id"]) != (
            cfg["genesis_hash"],
            cfg["chain_id"],
        ):
            raise Invalid("proposer prediction used another chain")
        peer = cfg["rpc_peers"][(head["block_height"] + 1) % len(cfg["rpc_peers"])]
        committed = self.committed()
        if committed and committed["height"] >= head["block_height"]:
            peer = cfg["rpc_peers"][(committed["height"] + 1) % len(cfg["rpc_peers"])]
        quote_peer = cfg["rpc_peers"][committed["validator"]] if committed else peer
        quote = self.quote(template, quote_peer)
        route.result()
        return generation, quote, peer, self.connect(peer)

    def committed(self):
        if "signer_state" not in self.cfg:
            return None
        path = Path(self.cfg["signer_state"]) / "confirmed.json"
        return load(path) if path.exists() else None

    def sign(self, op, out, prepared=None):
        committed = self.committed()
        cfg = self.cfg
        if op["proof_kind"] == KINDS["keys"]:
            raise Invalid("configured ledger version has no key registration operation")
        if prepared:
            quoted = prepared[1]
            template = quoted["operation"]
            if committed and quoted["sequence"] <= committed["sequence"]:
                quoted = self.quote(
                    template, self.cfg["rpc_peers"][committed["validator"]]
                )
            if dict(template, proof=op["proof"]) != op or len(op["proof"]) != len(
                template["proof"]
            ):
                raise Invalid("verified operation differs from the prequoted template")
            # Replace only the proof encoding's contribution to the quoted wire weight.
            # Account expansion depends on the (identical) native journal's receipt count.
            weight = (
                quoted["asset_weight_bytes"]
                + len(encode(op["proof"]))
                - len(encode(template["proof"]))
            )
            quantum = quoted["transfer_fee_byte_quantum"]
            fee = (weight + quantum - 1) // quantum * quoted[
                "transfer_fee_per_quantum"
            ] + quoted["state_expansion_fee"]
            # A fee with fewer decimal digits can only decrease the wire size.
            if not 0 < fee <= quoted["minimum_fee"]:
                raise Invalid("precomputed proof fee outside quoted bounds")
            quote = dict(
                quoted,
                operation=op,
                minimum_fee=fee,
                asset_weight_bytes=weight,
                base_asset_fee=fee - quoted["state_expansion_fee"],
            )
            save(out / "quote-template.json", quoted)
        else:
            self.route()
            quote = self.quote(
                op, cfg["rpc_peers"][committed["validator"] if committed else 0]
            )
        if committed and quote["sequence"] <= committed["sequence"]:
            raise RuntimeError("quote is behind the last confirmed signer sequence")
        save(out / "quote.json", quote)
        if cfg.get("warm_signer"):
            with self.signer_lock:
                if self.signer is None or self.signer.poll() is not None:
                    self.signer = subprocess.Popen(
                        [
                            cfg["warm_signer"],
                            cfg["key_file"],
                            cfg["chain_id"],
                            cfg["genesis_hash"],
                            cfg["submitter"],
                            cfg["profile_hash"],
                        ],
                        stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE,
                    )
                self.signer.stdin.write(encode(quote) + b"\n")
                self.signer.stdin.flush()
                signed = json.loads(self.signer.stdout.readline())["signed"]
        else:
            signed = json.loads(
                subprocess.check_output(
                    [
                        cfg["node"],
                        "wallet-sign-asset-transaction",
                        "--key-file",
                        cfg["key_file"],
                        "--quote-file",
                        out / "quote.json",
                    ]
                )
            )
        u = signed["unsigned"]
        fields = {
            "source": cfg["submitter"],
            "chain_id": cfg["chain_id"],
            "genesis_hash": cfg["genesis_hash"],
            "protocol_version": quote["protocol_version"],
            "sequence": quote["sequence"],
            "fee": quote["minimum_fee"],
            **op,
        }
        if any(u.get(k) != v for k, v in fields.items()):
            raise Invalid("signed operation differs from quote")
        return signed

    tx_id = staticmethod(node_tx_id)

    def receipt(self, txid):
        rows = self.rpc(
            self.cfg["rpc_peers"][0], "receipts", {"tx_id": txid, "limit": 1}
        )
        return next((row for row in rows if row["tx_id"] == txid), None)

    def replace_spent_sequence(self, signed, out, error):
        """Replace only after all peers have spent the sequence and lack this receipt."""
        u = signed["unsigned"]
        if (
            self.cfg.get("warm_signer") is None
            or error.error.get("code") != "rpc_finality_submit_failed"
            or "mempool admission rejected `bad_sequence`"
            not in error.error.get("message", "")
            or u["source"] != self.cfg["submitter"]
            or u["profile_hash"] != self.cfg["profile_hash"]
            or u["chain_id"] != self.cfg["chain_id"]
            or u["genesis_hash"] != self.cfg["genesis_hash"]
        ):
            raise error
        tid = self.tx_id(signed)

        def check(peer):
            account = self.rpc(peer, "account", {"address": u["source"]})
            if account["address"] != u["source"] or account["sequence"] < u["sequence"]:
                raise RuntimeError("waiting for durable signer sequence convergence")
            receipts = self.rpc(peer, "receipts", {"tx_id": tid, "limit": 1})
            if receipts:
                raise RuntimeError("signed envelope already has a ledger receipt")
            return {
                "peer": peer,
                "committed_sequence": account["sequence"],
                "receipts": [],
            }

        evidence = list(self.pool.map(check, self.cfg["rpc_peers"]))
        save(
            out / ("superseded-" + tid + ".json"),
            dict(signed=signed, observations=evidence, error=error.error),
        )
        op = {"operation": u["operation"], **{k: u[k] for k in OP_FIELDS}}
        updated = self.sign(op, out)
        if updated["unsigned"]["sequence"] <= u["sequence"]:
            raise RuntimeError("replacement fee quote has a spent sequence")
        save(out / "signed.json", updated)
        return updated

    def final_receipt(self, out, tid):
        if not (out / "finality.json").exists():
            return None
        reply = load(out / "finality.json")
        finality = reply.get("finality") or {}
        receipt = finality.get("receipt") or {}
        if (
            reply["tx_id"] != tid
            or finality.get("confirmed") is not True
            or finality.get("genesis_hash") != self.cfg["genesis_hash"]
            or finality.get("chain_id") != self.cfg["chain_id"]
            or receipt.get("tx_id") != tid
        ):
            raise Invalid("invalid persisted finality receipt")
        return receipt

    def submit(self, signed, out, prepared=None):
        peers = self.cfg["rpc_peers"]
        params = {"signed_asset_transaction_json": encode(signed).decode()}
        conn = None
        if prepared:
            _, _, addr, conn = prepared
            # A node retires an idle connection after its read deadline.
            if select.select([conn], [], [], 0)[0]:
                conn.close()
                conn = None
        else:
            # Deferred delivery can leave validator 0 one round behind the
            # previous signer's confirmed reply, especially across routes.
            last = self.committed()
            height = (
                last["height"]
                if last
                else self.rpc(peers[0], "status", {})["block_height"]
            )
            addr = peers[(height + 1) % len(peers)]
        first = out / ("first-submit-" + self.tx_id(signed) + ".json")
        first_submit = time.time()
        if first.exists():
            first_submit = load(first)["at"]
        else:
            save(first, {"at": first_submit})
        for attempt in range(2 * len(peers)):
            try:
                submitted_at = time.time()
                reply = self.rpc(
                    addr,
                    "mempool_submit_signed_asset_transaction_finality",
                    params,
                    connection=conn,
                )
                if reply["tx_id"] != self.tx_id(signed):
                    raise Invalid("finality RPC returned another transaction")
                received = time.time()
                finality = reply.get("finality") or {}
                if finality.get("confirmed") is not True:
                    raise RuntimeError("node has not confirmed the signed transaction")
                receipt = finality.get("receipt") or {}
                if receipt.get("tx_id") != reply["tx_id"]:
                    raise Invalid(
                        "confirmed receipt identifies a different transaction"
                    )
                if (
                    receipt.get("accepted") is not True
                    or receipt.get("code") != "accepted"
                ):
                    raise Invalid(
                        "ledger rejected signed transaction: "
                        + str(receipt.get("code"))
                    )
                if (
                    finality.get("genesis_hash") != self.cfg["genesis_hash"]
                    or finality.get("chain_id") != self.cfg["chain_id"]
                ):
                    raise Invalid("finality reply is from another chain")
                save(
                    out / "broadcast-time.json",
                    {
                        "submit": first_submit,
                        "finality": received,
                        "validator": peers.index(addr),
                        "successful_attempt": submitted_at,
                    },
                )
                save(out / "finality.json", reply)
                return
            except NodeReplyError as error:
                save(
                    out / f"submit-error-{error.request_id}.json",
                    {
                        "request_id": error.request_id,
                        "validator": peers.index(addr),
                        "tx_id": self.tx_id(signed),
                        "method": "mempool_submit_signed_asset_transaction_finality",
                        "submit": submitted_at,
                        "received": time.time(),
                        "error": error.error,
                    },
                )
                if error.error.get("code") in (
                    "rpc_finality_submit_busy",
                    "rpc_mempool_mutation_busy",
                    "rpc_mempool_submit_rate_limited",
                ):
                    conn = None
                    time.sleep(min(10, 2**attempt))
                    continue
                if error.error.get("code") != "rpc_finality_wrong_proposer":
                    raise
                match = re.search(
                    r"retry the signed request at `validator-(\d+)`",
                    error.error.get("message", ""),
                )
                if not match or int(match[1]) >= len(peers):
                    raise
                addr = peers[int(match[1])]
                conn = None
        raise RuntimeError("no configured peer is the current proposer")


class Stub:
    def __init__(self, cfg):
        self.cfg = cfg
        self.db = Path(cfg["state"]) / "stub.sqlite"
        with self.connect() as db:
            db.executescript("""
                CREATE TABLE IF NOT EXISTS tx(id TEXT PRIMARY KEY, receipt TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS credits(deposit_id TEXT PRIMARY KEY, recipient TEXT, atoms TEXT, txid TEXT);
                CREATE TABLE IF NOT EXISTS client(id INTEGER PRIMARY KEY, value TEXT NOT NULL);
            """)
            db.execute(
                "INSERT OR IGNORE INTO client VALUES(1,?)", (json.dumps(cfg["anchor"]),)
            )

    def connect(self):
        return sqlite3.connect(self.db, timeout=30)

    def client(self):
        with self.connect() as db:
            return json.loads(
                db.execute("SELECT value FROM client WHERE id=1").fetchone()[0]
            )

    def credits(self, values, selected):
        result = []
        with self.connect() as db:
            for item in selected:
                row = db.execute(
                    "SELECT recipient,atoms FROM credits WHERE deposit_id=?",
                    (item["deposit_id"],),
                ).fetchone()
                if row != (item["pftl_recipient"], item["amount_atoms"]):
                    raise Invalid("stub credit differs from proven deposit")
                result.append(
                    dict(
                        item,
                        receipt_id=item["deposit_id"],
                        credited_asset_id="stub-pfUSDC",
                    )
                )
        return result

    def sign(self, op, out):
        save(out / "operation.json", op)
        sig = subprocess.check_output(
            [
                "openssl",
                "pkeyutl",
                "-sign",
                "-rawin",
                "-inkey",
                self.cfg["key_file"],
                "-in",
                out / "operation.json",
            ]
        )
        return {"stub": "ed25519-v1", "operation": op, "signature": sig.hex()}

    @staticmethod
    def tx_id(signed):
        return hashlib.sha3_384(encode(signed)).hexdigest()

    def receipt(self, txid):
        with self.connect() as db:
            row = db.execute("SELECT receipt FROM tx WHERE id=?", (txid,)).fetchone()
            return json.loads(row[0]) if row else None

    def submit(self, signed, out):
        if (
            set(signed) != {"stub", "operation", "signature"}
            or signed["stub"] != "ed25519-v1"
        ):
            raise Invalid("invalid stub envelope")
        op = signed["operation"]
        if set(op) != {"operation", *OP_FIELDS}:
            raise Invalid("invalid stub operation fields")
        pub = bytes(op["public_values"])
        values = journal(pub)
        kind = values["kind"]
        if (
            op["operation"] != OP
            or op["proof_kind"] != KINDS[kind]
            or op["profile_hash"] != self.cfg["profile_hash"]
            or op["submitter"] != self.cfg["submitter"]
        ):
            raise Invalid("unsupported stub operation")
        with tempfile.TemporaryDirectory(dir=out) as tmp:
            path = Path(tmp)
            save(path / "signature", bytes.fromhex(signed["signature"]))
            save(path / "operation", encode(op))
            subprocess.run(
                [
                    "openssl",
                    "pkeyutl",
                    "-verify",
                    "-rawin",
                    "-inkey",
                    self.cfg["key_file"],
                    "-in",
                    path / "operation",
                    "-sigfile",
                    path / "signature",
                ],
                check=True,
                stdout=subprocess.DEVNULL,
            )
            save(path / "proof.bin", bytes(op["proof"]))
            save(path / "public.bin", pub)
            verify(self.cfg, kind, path)
        txid = self.tx_id(signed)
        with self.connect() as db:
            db.execute("BEGIN IMMEDIATE")
            if db.execute("SELECT 1 FROM tx WHERE id=?", (txid,)).fetchone():
                return
            client = json.loads(
                db.execute("SELECT value FROM client WHERE id=1").fetchone()[0]
            )
            if kind == "deposit":
                pins = self.cfg["stub_deposit_pins"]
                # The verified predecessor installs the successor set and runtime stake
                # atomically. The bootstrap pins apply only to the anchored epoch.
                installed_set = client["set_hash"]
                installed_stake = str(client.get("total_stake", pins["total_stake"]))
                if client["epoch"] == self.cfg["anchor"]["epoch"] and (
                    pins["set_hash"] != installed_set
                    or pins["total_stake"] != installed_stake
                ):
                    raise Invalid(
                        "deposit anchor differs from the configured light client"
                    )
                schedule = self.cfg.get("stub_recorder_pins", {})
                if schedule:
                    slots = schedule["slots_per_epoch"]
                    # Agave's warmup epochs double from the 32-slot minimum.
                    first = (slots - 1).bit_length() - 5 if schedule["warmup"] else 0
                    normal_start = 32 * ((1 << first) - 1)
                    slot = values["slot"]
                    epoch = (
                        first + (slot - normal_start) // slots
                        if slot >= normal_start
                        else ((slot // 32) + 1).bit_length() - 1
                    )
                    if epoch != client["epoch"]:
                        raise Invalid("deposit slot is outside the installed epoch")
                if (
                    pub[8:40].hex() != installed_set
                    or str(int.from_bytes(pub[40:56], "little")) != installed_stake
                    or pub[195:227].hex() != pins["route_binding"]
                    or pub[195:227].hex()
                    != self.cfg.get("route_binding", pins["route_binding"])
                    or pub[163:195].hex() != self.cfg["vault_program"]
                    or int.from_bytes(pub[97:99], "little") != pins["shred_version"]
                ):
                    raise Invalid("deposit differs from stub route pins")
                for row in values["receipts"]:
                    db.execute(
                        "INSERT INTO credits VALUES(?,?,?,?)",
                        (
                            row["deposit_id"],
                            row["pftl_recipient"],
                            row["amount_atoms"],
                            txid,
                        ),
                    )
            else:
                pins = self.cfg.get("stub_recorder_pins", {})
                if values["recorder_program"] != self.cfg["recorder_program"] or any(
                    values[key] != value for key, value in pins.items()
                ):
                    raise Invalid("recorder proof differs from the pinned route")
                if kind == "keys":
                    if (
                        values["epoch"] != client["epoch"]
                        or values["set_hash"] != client["set_hash"]
                    ):
                        raise Invalid("key registration is outside the installed epoch")
                    client["keys"] = sorted(set(client["keys"] + values["keys"]))
                else:
                    replacing = values["epoch"] == client["epoch"]
                    if replacing:
                        if (
                            "listed_stake" not in values
                            or "listed_stake" not in client
                            or values["listed_stake"] <= client["listed_stake"]
                            or values["total_stake"] != client["total_stake"]
                        ):
                            raise Invalid(
                                "same-epoch handoff must increase listed stake with unchanged runtime total"
                            )
                    elif values["epoch"] == client["epoch"] + 1:
                        keys = sorted(client["keys"])
                        raw = (
                            b"postfiat.pfusdc.solana.known_keys.v1"
                            + len(keys).to_bytes(4, "little")
                            + bytes.fromhex("".join(keys))
                        )
                        client.update(
                            predecessor=client["set_hash"],
                            predecessor_table=hashlib.sha256(raw).hexdigest(),
                        )
                    else:
                        raise Invalid("handoff must target current or next epoch")
                    if values["set_hash"] != client.get("predecessor") or values[
                        "table_hash"
                    ] != client.get("predecessor_table"):
                        raise Invalid(
                            "handoff predecessor or known-key commitment differs"
                        )
                    client.update(
                        epoch=values["epoch"],
                        set_hash=values["next_set"],
                        total_stake=values["total_stake"],
                    )
                    if "listed_stake" in values:
                        client["listed_stake"] = values["listed_stake"]
                db.execute(
                    "UPDATE client SET value=? WHERE id=1", (json.dumps(client),)
                )
            receipt = {
                "accepted": True,
                "code": "accepted",
                "tx_id": txid,
                "ledger": "stub",
                "fee_charged": 0,
            }
            db.execute("INSERT INTO tx VALUES(?,?)", (txid, json.dumps(receipt)))
