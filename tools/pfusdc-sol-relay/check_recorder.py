#!/usr/bin/env python3
"""Replay the retained Groth16 proofs into an isolated recorder anchored in epoch 5."""

import argparse
import tempfile
from pathlib import Path

from check_rejections import rejected
from jobs import Relay
from ledger import operation
from storage import load


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", type=Path)
    parser.add_argument("--evidence", type=Path, default=Path(__file__).with_name("evidence"))
    args = parser.parse_args()
    cfg = load(args.config)
    assert cfg["ledger"] == "stub" and cfg["anchor"]["epoch"] == 5
    proofs = args.evidence
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp)
        cfg.update(
            state=str(out / "state"),
            stub_recorder_pins={
                "shred_version": 50093,
                "slots_per_epoch": 64,
                "warmup": False,
            },
        )
        stub = Relay(cfg).ledger

        def send(kind, epoch):
            op = operation(cfg, kind, proofs / f"{kind}-{epoch}")
            stub.submit(stub.sign(op, out), out)

        rejected("skipped epoch", send, "handoff", 7)
        rejected("handoff without registered key", send, "handoff", 6)
        send("keys", 5)
        assert len(stub.client()["keys"]) == 4
        for epoch in (6, 7):
            send("handoff", epoch)
            client = stub.client()
            assert client["epoch"] == epoch and client["listed_stake"] == 100
            assert client["total_stake"] == (100 if epoch == 6 else 105)
            print(
                f"PASS epoch={epoch} listed={client['listed_stake']} runtime_total={client['total_stake']}"
            )
        with stub.connect() as db:
            assert db.execute("SELECT count(*) FROM tx").fetchone()[0] == 3
        print("PASS 3 verified recorder transactions; 2 consecutive handoffs")


if __name__ == "__main__":
    main()
