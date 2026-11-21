"""Durable stage journal and transactional proof outbox, shared by CLI and HTTP."""

from concurrent.futures import ThreadPoolExecutor
from contextlib import ExitStack, contextmanager, nullcontext
import math
import threading
import re
import time
from pathlib import Path

from ledger import KINDS, Node, NodeReplyError, Stub, operation
from prover import guest_prefix, native_journal, prove
from route import ROUTES, binding
from source import base58, bind, build, capture, journal, locate
from storage import (
    ROUTE,
    AwaitingSource,
    Invalid,
    Rejected,
    digest,
    encode,
    load,
    locked,
    mkdir,
    save,
    sync_dir,
)

STAGES = (
    "finality_seen",
    "capture_done",
    "witness_built",
    "proof_done",
    "signed",
    "submitted",
    "credited",
)


def now():
    return time.time_ns() / 1e9


def job_id(kind, ref, route=ROUTE):
    if kind == "deposit":
        base58(ref, 64)
    elif (
        kind not in ("keys", "handoff")
        or not re.fullmatch(r"(?:0|[1-9][0-9]{0,19})(?:-(?:0|[1-9][0-9]{0,19}))?", ref)
        or any(int(n) >= 2**64 for n in ref.split("-"))
    ):
        raise Invalid("invalid recorder epoch")
    return "0x" + digest(encode([route, kind, ref]))


class Relay:
    def __init__(self, cfg):
        self.cfg = cfg
        self.route = cfg.get("route_id", ROUTE)
        if self.route not in ROUTES:
            raise Invalid("unknown Solana route ID")
        if (cfg["ledger"] == "node" or "route_id" in cfg) and cfg.get(
            "route_binding"
        ) != binding(cfg):
            raise Invalid("route binding differs from the configured chain and vault")

        self.root = Path(cfg["state"])
        mkdir(self.root)
        self.jobs = self.root / "jobs"
        self.outbox = self.root / "outbox"
        mkdir(self.jobs)
        mkdir(self.outbox)
        if cfg["ledger"] not in ("stub", "node"):
            raise Invalid("unknown ledger")
        # A state volume represents one ledger and one signing account.
        identity = {
            k: cfg.get(k)
            for k in (
                "ledger",
                "profile_hash",
                "submitter",
                "chain_id",
                "genesis_hash",
                "deposit_vkey",
                "handoff_vkey",
                "keys_vkey",
                "recorder_vkey",
            )
        }
        if "route_id" in cfg:
            identity.update(route_id=self.route, route_binding=cfg["route_binding"])
        with locked(self.root / "identity.lock"):
            identity_file = self.root / "identity.json"
            if identity_file.exists() and load(identity_file) != identity:
                raise Invalid(
                    "state directory belongs to a different ledger, signer or route"
                )
            save(identity_file, identity)
        self.signer_state = Path(cfg.get("signer_state", self.outbox))
        mkdir(self.signer_state)
        self.ledger = Stub(cfg) if cfg["ledger"] == "stub" else Node(cfg)

    def create(self, kind, ref):
        jid = job_id(kind, ref, self.route)
        path = self.jobs / jid[2:]
        mkdir(path)
        with locked(path / "create.lock"):
            exists = (path / "job.json").exists()
            if not exists:
                t = now()
                req = {"route_id": self.route, "kind": kind, "reference": ref}
                if kind == "deposit":
                    req["deposit_tx_hash"] = ref
                state = {
                    "schema": "postfiat-trustless-bridge-job-v2",
                    "ok": True,
                    "job_id": jid,
                    "request": req,
                    "route_id": self.route,
                    "source_chain_id": 0,
                    "source_proof_kind": KINDS[kind],
                    "program_vkey": self.cfg[guest_prefix(self.cfg, kind) + "_vkey"],
                    "route_profile_hash": self.cfg["profile_hash"],
                    "status": "created",
                    "ledger": self.cfg["ledger"],
                    "stage_history": [],
                    "created_at_unix": int(t),
                    "created_at": t,
                    "updated_at_unix": int(t),
                    "timings": {},
                    "observer_attestor_enabled": False,
                }
                save(path / "job.json", state)
        return path, exists

    def status(self, jid):
        if not re.fullmatch(r"0x[0-9a-f]{64}", jid):
            raise Invalid("invalid job id")
        return load(self.jobs / jid[2:] / "job.json")

    @contextmanager
    def signing_turn(self, priority, optional_handoff):
        # Once a handoff is on the wire its sequence must resolve. Before that
        # point give deposits already being proved first use of the shared signer.
        with ExitStack() as release:
            gate = locked(priority) if priority and optional_handoff else nullcontext()
            with gate:
                release.enter_context(locked(self.signer_state / "submitter.lock"))
            yield

    def run(self, path, *, wait=True, proof_only=False):
        priority = (
            self.signer_state / "deposit-priority.lock"
            if self.cfg.get("signer_state")
            else None
        )
        request = load(path / "job.json")["request"]
        deposit = (
            locked(priority, shared=True)
            if priority and request["kind"] == "deposit"
            else nullcontext()
        )
        with locked(path / "worker.lock"), deposit:
            state = load(path / "job.json")
            if state["status"] == "accepted":
                return state
            kind, ref = (state["request"][k] for k in ("kind", "reference"))

            def stage(name, action):
                if name in state["timings"]:
                    return
                t = now()
                for old in state["stage_history"]:
                    if old["status"] == "running":
                        old.update(
                            status="interrupted", completed_at_unix=int(t), ended_at=t
                        )
                row = {
                    "stage": name,
                    "status": "running",
                    "started_at": t,
                    "ended_at": None,
                    "started_at_unix": int(t),
                    "completed_at_unix": None,
                }
                if (
                    state["stage_history"]
                    and state["stage_history"][-1]["status"] == "waiting"
                ):
                    row = state["stage_history"][-1]
                    row["status"] = "running"
                else:
                    state["stage_history"].append(row)
                state.update(
                    status="running", stage=name, retryable=True, updated_at_unix=int(t)
                )
                state.pop("error", None)
                save(path / "job.json", state)
                try:
                    action()
                except AwaitingSource:
                    row["status"] = "waiting"
                    state["status"] = "waiting"
                    save(path / "job.json", state)
                    raise
                except Exception as error:
                    end = now()
                    row.update(
                        status="failed", ended_at=end, completed_at_unix=int(end)
                    )
                    state.update(
                        status="failed",
                        retryable=not isinstance(error, Invalid),
                        error=str(error)[:512],
                        updated_at_unix=int(end),
                    )
                    save(path / "job.json", state)
                    raise
                end = now()
                row.update(status="complete", ended_at=end, completed_at_unix=int(end))
                state["timings"][name] = end
                state["updated_at_unix"] = int(end)
                save(path / "job.json", state)
                save(path / "timings.json", state["timings"])

            def finalized():
                location = locate(self.cfg, kind, ref, wait)
                if location.get("receipts"):
                    state["request"].update(
                        {
                            k: location["receipts"][0][k]
                            for k in ("depositor", "pftl_recipient", "amount_atoms")
                        }
                    )
                state["slot"] = location["slot"]
                save(path / "source.json", location)

            stage("finality_seen", finalized)
            location = load(path / "source.json")
            stage("capture_done", lambda: capture(self.cfg, location, ref, path, wait))

            # A warm deposit proof is requested as soon as the witness is built; the host's
            # native check runs meanwhile and gates the proof's journal and the submit.
            alongside = kind == "deposit" and self.cfg.get("prover") == "warm"

            def witness():
                build(self.cfg, location, path)
                if not alongside:
                    check_native()

            def check_native():
                public = native_journal(self.cfg, kind, path)
                decoded = journal(public)
                bind(location, decoded)
                if kind == "handoff" and self.cfg.get("verify_certified_epoch_set"):
                    ranks = (
                        Path(self.cfg["archive"]) / str(location["slot"]) / "validators"
                    ).read_bytes()
                    expected = digest(
                        b"postfiat.pfusdc.solana.validator_set.v1" + ranks
                    )
                    if decoded["next_set"] != expected or decoded[
                        "total_stake"
                    ] != int.from_bytes(ranks[:16], "little"):
                        raise Invalid(
                            "handoff differs from the epoch set used for the certified block"
                        )
                if (
                    kind == "deposit"
                    and "route_binding" in self.cfg
                    and public[195:227].hex() != self.cfg["route_binding"]
                ):
                    raise Invalid("proven vault binding differs from configured route")

            stage("witness_built", witness)
            # Set once native.bin is on disk or the native check has failed. The signing
            # preparation starts before the proof stage and reads that file.
            native_written = threading.Event()
            if not alongside or "proof_done" in state["timings"]:
                native_written.set()

            def native_then_signal():
                try:
                    check_native()
                finally:
                    native_written.set()

            def proof():
                if not alongside:
                    return request_proof(None)
                with ThreadPoolExecutor(max_workers=1) as pool:
                    checked = pool.submit(native_then_signal)
                    try:
                        request_proof(checked.result)
                    finally:
                        # A native refusal wins over any proof outcome, as when it ran first.
                        checked.result()

            def request_proof(native):
                if kind == "deposit" and self.cfg.get("prover") == "warm":
                    # Different signatures in one block have the same authenticated witness.
                    # Publish the verified artifacts before exposing the cache entry.
                    cache = (
                        self.root
                        / "proof-cache"
                        / digest((path / "witness.bin").read_bytes())
                    )
                    mkdir(cache)
                    with locked(cache / "lock"):
                        artifacts = ("public.bin", "proof.bin", "prover-timing.json")
                        if not (cache / "complete").exists():
                            state["prover"] = prove(self.cfg, kind, path, native)
                            report = load(path / "prover-timing.json")
                            report["witness_built_at"] = state["timings"][
                                "witness_built"
                            ]
                            save(path / "prover-timing.json", report)
                            for name in artifacts:
                                save(cache / name, (path / name).read_bytes())
                            save(cache / "complete", True)
                        else:
                            if native is not None:
                                native()
                            for name in artifacts:
                                save(path / name, (cache / name).read_bytes())
                            if (path / "public.bin").read_bytes() != (
                                path / "native.bin"
                            ).read_bytes():
                                raise Invalid(
                                    "cached proof journal differs from native verification"
                                )
                            state["prover"] = "warm"
                else:
                    state["prover"] = prove(self.cfg, kind, path)
                selected = bind(location, journal((path / "public.bin").read_bytes()))
                if selected:
                    state["receipts"] = selected
                    state["request"].update(
                        {
                            k: selected[0][k]
                            for k in (
                                "amount_atoms",
                                "pftl_recipient",
                                "deposit_id",
                                "depositor",
                            )
                        }
                    )

            prepared = None
            generation = self.signer_state / "last-signed.json"

            def generation_id():
                return load(generation) if generation.exists() else None

            if (
                self.cfg.get("warm_signer")
                and not proof_only
                and "signed" not in state["timings"]
            ):
                def prepare(generation):
                    native_written.wait()
                    return self.ledger.prepare(kind, path, generation)

                prepared = self.ledger.pool.submit(prepare, generation_id())
            stage("proof_done", proof)
            if proof_only:
                return state
            public = (path / "public.bin").read_bytes()
            # All transactions in the same journal share the exact signed envelope.
            # Fast and slow certificates for the same deposit block credit the same receipts.
            values = journal(public)
            identity = (
                [values["slot"], values["block_id"]]
                if kind == "deposit"
                else digest(public)
            )
            oid = digest(encode([self.cfg["profile_hash"], kind, identity]))
            if "signed" in state["timings"]:
                oid = state.get("outbox_id", oid)
                if not (self.outbox / oid / "signed.json").exists():
                    # The original outbox used a digest of the complete journal.
                    oid = digest(encode([self.cfg["profile_hash"], kind]) + public)
            out = self.outbox / oid
            mkdir(out)
            with self.signing_turn(
                priority, kind == "handoff" and "signed" not in state["timings"]
            ):
                warm = None
                fresh_signature = False
                if prepared:
                    warm = prepared.result()
                    if warm[0] != generation_id():
                        warm[3].close()
                        warm = None

                def sign():
                    nonlocal warm, fresh_signature
                    pending = self.signer_state / "pending.json"
                    # Older per-route outboxes contain just a relative envelope ID.
                    for marker in dict.fromkeys(
                        (self.outbox / "pending.json", pending)
                    ):
                        if marker.exists():
                            prev = Path(load(marker))
                            previous = (
                                prev if prev.is_absolute() else self.outbox / prev
                            )
                            if previous != out and (previous / "signed.json").exists():
                                self.deliver(previous)
                                self.confirm(previous, allow_rejection=True)
                                if warm:
                                    warm[3].close()
                                    warm = None
                            if marker != pending:
                                marker.unlink()
                    save(pending, str(out.resolve()))
                    if not (out / "signed.json").exists():
                        op = operation(self.cfg, kind, path)
                        signed = (
                            self.ledger.sign(op, out, warm)
                            if warm
                            else self.ledger.sign(op, out)
                        )
                        save(out / "signed.json", signed)
                        fresh_signature = True
                    save(generation, ["pending", oid])
                    signed = load(out / "signed.json")
                    state["outbox_id"] = oid
                    state["tx_id"] = self.ledger.tx_id(signed)
                    save(path / "signed.json", signed)

                stage("signed", sign)
                stage("submitted", lambda: self.deliver(out, warm, fresh_signature))
                if warm:
                    warm[3].close()

                def confirm():
                    receipt = self.confirm(out)
                    if kind == "deposit":
                        if (
                            self.cfg.get("warm_signer")
                            and (out / "finality.json").exists()
                        ):
                            finality = load(out / "finality.json")["finality"]
                            sent = load(out / "broadcast-time.json")
                            credits = self.ledger.governed_credits(
                                values,
                                state["receipts"],
                                committed=(
                                    sent.get("validator"),
                                    finality["block"]["header"],
                                ),
                            )
                        else:
                            credits = self.ledger.credits(values, state["receipts"])
                        state["credited_asset_id"] = credits[0]["credited_asset_id"]
                        receipt = dict(receipt, credits=credits)
                    save(path / "receipt.json", receipt)
                    state.update(
                        receipt_code="ACCEPTED",
                        receipt_id=receipt["credits"][0]["receipt_id"]
                        if kind == "deposit"
                        else receipt["tx_id"],
                        terminal_checkpoint_sha256=digest(encode(receipt)),
                    )

                if isinstance(self.ledger, Node):
                    current = load(out / "signed.json")
                    state["tx_id"] = self.ledger.tx_id(current)
                    save(path / "signed.json", current)
                    if (out / "finality.json").exists() and self.cfg.get(
                        "signer_state"
                    ):
                        finality = load(out / "finality.json")["finality"]
                        header = finality["block"]["header"]
                        validator = load(out / "broadcast-time.json").get("validator")
                        last = self.ledger.committed()
                        if validator is not None and (
                            not last or last["height"] <= header["height"]
                        ):
                            save(
                                self.signer_state / "confirmed.json",
                                dict(
                                    height=header["height"],
                                    sequence=current["unsigned"]["sequence"],
                                    validator=validator,
                                    tx_id=state["tx_id"],
                                ),
                            )
                stage("credited", confirm)
                save(generation, ["confirmed", oid])
                pending = self.signer_state / "pending.json"
                if pending.exists() and load(pending) in (oid, str(out.resolve())):
                    pending.unlink()
                    sync_dir(self.signer_state)
            state.update(status="accepted", retryable=False, stage="accepted")
            save(path / "timings.json", state["timings"])
            save(path / "job.json", state)
            return state

    def deliver(self, out, prepared=None, fresh=False):
        signed = load(out / "signed.json")
        tid = self.ledger.tx_id(signed)
        if isinstance(self.ledger, Node):
            if self.ledger.final_receipt(out, tid):
                return tid
            # Only a prior interrupted delivery needs a pre-broadcast ledger lookup.
            marker = out / "delivery-attempt.json"
            if not fresh and self.ledger.receipt(tid):
                return tid
            save(marker, {"tx_id": tid, "time": now()})
            try:
                self.ledger.submit(signed, out, prepared)
            except NodeReplyError as error:
                updated = self.ledger.replace_spent_sequence(signed, out, error)
                tid = self.ledger.tx_id(updated)
                self.ledger.submit(updated, out)
        elif not self.ledger.receipt(tid):
            self.ledger.submit(signed, out)
        # Do not release the source sequence until it appears in the committed ledger.
        # On recovery an existing receipt suppresses the retransmission.
        return tid

    def confirm(self, out, *, allow_rejection=False):
        tid = self.ledger.tx_id(load(out / "signed.json"))
        while True:
            receipt = (
                self.ledger.final_receipt(out, tid)
                if isinstance(self.ledger, Node)
                else None
            )
            receipt = receipt or self.ledger.receipt(tid)
            if receipt:
                if receipt["tx_id"] != tid:
                    raise Invalid("ledger rejected signed transaction " + tid)
                if not allow_rejection and (
                    receipt["accepted"] is not True or receipt["code"] != "accepted"
                ):
                    raise Rejected(
                        f"ledger rejected signed transaction {tid}: {receipt['code']}",
                        receipt["code"],
                    )
                return receipt
            time.sleep(0.25)

    def all_jobs(self):
        return [load(path) for path in self.jobs.glob("*/job.json")]

    def summary(self):
        groups = {}
        for job in self.all_jobs():
            key = job["request"]["kind"]
            group = groups.setdefault(key, {})
            for row in job["stage_history"]:
                if row["status"] == "complete":
                    group.setdefault(row["stage"], []).append(
                        row["ended_at"] - row["started_at"]
                    )
            if "credited" in job["timings"]:
                group.setdefault("end_to_end", []).append(
                    job["timings"]["credited"] - job["created_at"]
                )

        def stats(values):
            values.sort()
            return {
                "n": len(values),
                **{
                    f"p{p}": round(values[math.ceil(p * len(values) / 100) - 1], 6)
                    for p in (50, 95, 99, 100)
                },
            }

        return {
            k: {stage: stats(v) for stage, v in stages.items()}
            for k, stages in groups.items()
        }
