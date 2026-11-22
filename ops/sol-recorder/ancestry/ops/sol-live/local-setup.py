"""Reconcile immutable cluster identity and record each epoch on the local cluster."""

import json
from pathlib import Path
import sys
import time
from threading import Thread
from types import SimpleNamespace

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / "solana/deploy"))
from bridge import Cluster, DIRECTORY, deploy, initialize, load, pk, require
from route_binding import route_binding
from recorder import record_epoch, publish_epoch

from live import STATE as state, INSTANCE
from retain import start
from deposit_index import retain_transactions

start(state)

key_dir = DIRECTORY if INSTANCE == "local" else DIRECTORY / INSTANCE
spec = json.loads((state / "cluster.json").read_text())
url = json.loads((state / "rpc.json").read_text())[0]
c = Cluster(
    SimpleNamespace(
        rpc=url,
        payer=key_dir / "local-payer.json",
        log=state / "solana-transactions.jsonl",
    )
)
require(c.genesis == spec["genesis"], "local genesis changed")
# On restart Agave serves RPC while replaying genesis. Wait for its previous finalized work.
history = state / "solana-transactions.jsonl"
prior_slot = (
    max(
        (
            row.get("slot", 0)
            for row in (
                json.loads(line)
                for line in history.read_text().splitlines()
                if line.strip()
            )
        ),
        default=0,
    )
    if history.exists()
    else 0
)
while c.call("getSlot", {"commitment": "finalized"}) < prior_slot or c.rent(96) <= 1:
    time.sleep(1)
vault, recorder, egress = [
    load((key_dir if n == "vault" else DIRECTORY) / (n + ".json"))
    for n in ("vault", "recorder", "egress-verifier")
]
Thread(
    target=retain_transactions,
    args=(state, url, bytes(vault.pubkey()).hex()),
    daemon=True,
).start()
for name, key in (
    ("vault", vault),
    ("recorder", recorder),
    ("egress_verifier", egress),
):
    deploy(c, name, key)
initialize(
    c,
    vault,
    pk(spec["mint"]),
    route_binding(c.genesis, vault.pubkey(), local=True, test_route=INSTANCE),
)


def captured(epoch):
    for root in (state / "ledgers" / "certified-events", state / "archive"):
        for event in sorted(
            root.glob("*/events.json"), key=lambda p: int(p.parent.name), reverse=True
        ):
            try:
                info = json.loads(event.read_bytes())
            except FileNotFoundError:
                continue
            if info["epoch"] < epoch:
                break
            if info["epoch"] == epoch and any(
                e["tag"] == "PFSOLST1" for e in info["events"]
            ):
                return True
    return False


last = None
while True:
    epoch = c.call("getEpochInfo", c.level)["epoch"]
    if epoch != last and record_epoch(c, recorder.pubkey(), epoch):
        last = epoch
    if not captured(epoch):
        publish_epoch(c, recorder.pubkey(), epoch, refresh=True)
    time.sleep(3)
