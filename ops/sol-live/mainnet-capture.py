"""Observe a finalized deposit with the public Agave capture, then prove and credit it."""

from pathlib import Path
import hashlib
import re
import tempfile
import socket
import json
import shlex
import subprocess
import time
from solders.hash import Hash
from solders.pubkey import Pubkey
from live import STATE, read, save, receipt_at, rpc, six
from measure import balance
from bridge import require

SSH = [
    "ssh",
    "-oBatchMode=yes",
    "-oConnectTimeout=8",
    "-p",
    "63534",
    "root@207.216.48.74",
]
BASE = STATE / "mainnet"


def committed_height(tid, attempts=40, pause=0.5, lookup=None):
    # The submit reply can arrive before validator 0 has stored the receipt. Ask again for a
    # short while; any other error, or a receipt that never appears, still fails the wait.
    lookup = lookup or (lambda: rpc(0, "tx", tx_id=tid))
    for left in range(attempts - 1, -1, -1):
        try:
            return lookup()["block"]["header"]["height"]
        except RuntimeError as error:
            waiting = "rpc_tx_not_found" in str(error) or "RPC worker failed" in str(error)
            if not waiting or not left:
                raise
            time.sleep(pause)


def check_prover(path):
    # The warm server validates its request header after loading its guests and proving keys.
    with socket.socket(socket.AF_UNIX) as sock:
        sock.settimeout(10)
        sock.connect(str(path))
        sock.sendall(b"status\n")
        sock.shutdown(socket.SHUT_WR)
        with sock.makefile("rb") as stream:
            response = json.loads(stream.readline(512))
    require(
        response.get("error") == "expected guest mode byte_length",
        "unexpected prover response",
    )


def arm(c):
    child = subprocess.Popen(
        [
            *SSH,
            "cd /work/g && python3 -u time-window.py mainnet-beta main-ledger 18899",
        ],
        stdout=subprocess.PIPE,
        text=True,
    )
    line = child.stdout.readline()
    hit = re.fullmatch(r"armed (\d+)\.\.(\d+) public=\d+ observer=\d+\n", line)
    require(
        hit is not None, "the Solana observer did not arm a finalized capture window"
    )
    first, last = map(int, hit.groups())
    while True:
        current = c.call("getSlot", {"commitment": "processed"})
        if current >= first:
            require(current < last - 4, "missed the armed window")
            return child
        time.sleep(0.10)


class Missed(Exception):
    """The observer is past the deposit's block and never captured it."""


def observer_slot():
    body = '{"jsonrpc":"2.0","id":1,"method":"getSlot","params":[{"commitment":"finalized"}]}'
    run = subprocess.run(
        [*SSH, "curl -s -m 4 -X POST -H 'content-type: application/json' -d '" + body + "' http://127.0.0.1:18899"],
        capture_output=True,
        text=True,
    )
    try:
        return json.loads(run.stdout)["result"]
    except (ValueError, KeyError):
        return None


def export(slot, sig, vault, binding):
    """Wait in place for the observer's capture. Exiting here cost a service restart per try."""
    script = Path(__file__).with_name("observer-receipt.py").read_text()
    noted = None
    while True:
        run = subprocess.run(
            [*SSH, shlex.join(["python3", "-", str(slot), sig, vault, binding])],
            input=script,
            capture_output=True,
            text=True,
        )
        if run.returncode == 0:
            return
        reason = (run.stderr.strip().splitlines() or ["observer unreachable"])[-1]
        if reason != noted:
            print(f"{sig}: {reason}", flush=True)
            noted = reason
        if "awaiting" in reason:
            head = observer_slot()
            # A capture is written as its block is replayed, and its carrier within 64 slots.
            if head is not None and head > slot + 400:
                raise Missed(f"{reason}; observer finalized {head}")
        elif run.returncode != 255:
            raise RuntimeError(f"observer export failed: {run.stderr[-600:]}")
        time.sleep(2)


def credit(relay, activation, sig, slot, first_seen):
    """Prove and submit the deposit. Return the step that waits for the six validators' receipts
    and records the credit, so the caller can run it off its own path."""
    observed = BASE / "observations" / (sig + ".json")
    times = read(observed) if observed.exists() else {"finalized_seen": first_seen}
    save(observed, times)
    directory = BASE / "archive" / str(slot)
    if not (directory / "certificate").exists():
        p = activation["profile"]
        vault = str(Pubkey.from_bytes(bytes.fromhex(p["vault_address"])))
        binding = str(Hash.from_bytes(bytes.fromhex(p["solana"]["route_binding"])))
        export(slot, sig, vault, binding)
        with tempfile.TemporaryDirectory(dir=directory.parent) as tmp:
            stage = Path(tmp) / str(slot)
            subprocess.run(
                [
                    "rsync",
                    "-a",
                    "-e",
                    "ssh -oBatchMode=yes -p 63534",
                    f"root@207.216.48.74:/work/g/sol-live/receipts/{slot}/",
                    str(stage) + "/",
                ],
                check=True,
            )
            stage.rename(directory)
    if "archive_ready" not in times:
        times["archive_ready"] = time.time()
        save(observed, times)
    job = relay.create("deposit", sig)[0]
    received = relay.run(job)
    require(received["status"] == "accepted", "mainnet proof credit pending")

    def six_receipts():
        tid = received["tx_id"]
        height = committed_height(tid)
        validators = receipt_at(tid, height)
        receipt = read(job / "receipt.json")
        proof = read(job / "prover-timing.json")
        out = relay.outbox / received["outbox_id"]
        # A submission whose reply was lost is recovered from the committed receipt and
        # leaves no broadcast record. Fall back to the first-submit record and the job's
        # own stage times so one lost reply cannot stall the watcher.
        if (out / "broadcast-time.json").exists():
            broadcast = read(out / "broadcast-time.json")
        else:
            stages = {r["stage"]: r for r in received["stage_history"] if r["status"] == "complete"}
            first = sorted(out.glob("submit-error-*.json"))
            broadcast = dict(
                submit=read(first[0])["submit"] if first else stages["submitted"]["started_at"],
                finality=stages["submitted"]["ended_at"],
            )
        evidence = dict(
            signature=sig,
            slot=slot,
            block_id=(directory / "finalized-id").read_text().strip(),
            ledger_tx_id=tid,
            height=height,
            validators=validators,
            credits=receipt["credits"],
            credited_asset_id=received["credited_asset_id"],
            proof_sha256=hashlib.sha256((job / "proof.bin").read_bytes()).hexdigest(),
            proof_bytes=(job / "proof.bin").stat().st_size,
            program_vkey=proof["program_vkey"],
            witness_sha256=proof["witness_sha256"],
            prover_seconds=proof["seconds"],
            times=dict(
                **read(observed),
                **read(job / "timings.json"),
                leaves=proof["leaves_at"],
                aggregate=proof["aggregate_at"],
                ledger_submit=broadcast["submit"],
                ledger_finality=broadcast["finality"],
                six_receipts_seen=time.time(),
            ),
        )
        intent = BASE / "deposit-intent.json"
        if intent.exists():
            request = read(intent)
            for row in evidence["credits"]:
                if row["nonce"] != request["nonce"]:
                    continue
                evidence["times"]["solana_send"] = request.get("solana_send")
                if "opening_balances" in request:
                    require(
                        received["credited_asset_id"] == request["asset_id"],
                        "wrong credited series",
                    )
                    expected = [
                        before + request["amount_atoms"]
                        for before in request["opening_balances"]
                    ]
                    balances = six(
                        lambda i: balance(request["recipient"], request["asset_id"], i)
                    )
                    require(
                        balances == expected,
                        "recipient credit delta differs across validators",
                    )
                    evidence.update(
                        opening_balances=request["opening_balances"],
                        closing_balances=balances,
                    )
                    evidence["times"]["six_balances_seen"] = time.time()
                    observation = row.get("balance_observation")
                    if observation and observation["balance"] == expected[0]:
                        evidence["times"]["balance_visible"] = observation["time"]
        save(BASE / "credits" / (sig + ".json"), evidence)
        print(
            f"credited {sig} slot={slot} height={height} root={validators[0]['root']}",
            flush=True,
        )

    return six_receipts
