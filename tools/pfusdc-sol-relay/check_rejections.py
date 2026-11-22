#!/usr/bin/env python3
"""Check finalized input binding and rejection of invalid deposit evidence in an isolated stub."""

import argparse
import json
import subprocess
import tempfile
from copy import deepcopy
from pathlib import Path

from jobs import Relay
from ledger import operation
from route import binding
from source import authenticated_transaction, bind, journal
from storage import Invalid, load


def rejected(label, action, *args):
    try:
        action(*args)
    except (Invalid, subprocess.CalledProcessError):
        print("PASS rejected " + label)
        return
    raise AssertionError("accepted " + label)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("config", type=Path)
    args = p.parse_args()
    cfg = load(args.config)
    assert cfg["ledger"] == "stub"
    relay = Relay(cfg)
    sig = (Path(cfg["capture_root"]) / "signature").read_text().strip()
    job, _ = relay.create("deposit", sig)
    assert load(job / "job.json")["status"] == "accepted"
    if "route_binding" in cfg:
        for field, value in (
            ("chain_id", "other-pftl-chain"),
            ("genesis_hash", "11" * 48),
            ("vault_program", "44" * 32),
            ("solana_genesis_hash", "55" * 32),
        ):
            assert binding(dict(cfg, **{field: value})) != cfg["route_binding"]
            rejected("changed route " + field, Relay, dict(cfg, **{field: value}))
    source = load(job / "source.json")
    archive = job / "archive"
    authenticated_transaction(cfg, source, archive)
    for field, value in (
        ("signature", "1" * 64),
        ("slot", source["slot"]),
        ("block_id", "01" * 32),
    ):
        changed = deepcopy(source)
        if field == "slot":
            # Exercise the proof-to-slot check independently of locating the archive.
            changed["slot"] = value + 1
            rejected(
                "wrong slot",
                bind,
                changed,
                journal((job / "public.bin").read_bytes()),
            )
        else:
            changed[field] = value
            rejected(
                "wrong " + field,
                authenticated_transaction,
                cfg,
                changed,
                archive,
            )
    for field in (
        "amount_atoms",
        "pftl_recipient",
        "nonce",
        "depositor",
        "receipt_address",
        "program_id",
    ):
        changed = deepcopy(source)
        orig = changed["receipts"][0][field]
        changed["receipts"][0][field] = orig[:-1] + ("1" if orig[-1] != "1" else "2")
        rejected(
            "wrong " + field,
            bind,
            changed,
            journal((job / "public.bin").read_bytes()),
        )

    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp)
        stub = Relay(dict(cfg, state=str(out / "state"))).ledger
        original = operation(cfg, "deposit", job)

        def submit(op):
            stub.submit(stub.sign(op, out), out)

        for field in ("proof", "public_values"):
            changed = deepcopy(original)
            changed[field][-1] ^= 1
            rejected("changed " + field, submit, changed)
        signed = stub.sign(original, out)
        changed = dict(signed, signature="00" * 64)
        rejected("wrong signer signature", stub.submit, changed, out)
        for field in ("proof_kind", "profile_hash", "submitter"):
            rejected("wrong " + field, submit, dict(original, **{field: "00"}))
        other = Relay(
            dict(cfg, state=str(out / "other"), deposit_vkey=cfg["handoff_vkey"])
        ).ledger
        rejected("wrong program vkey", other.submit, signed, out)
        for field in ("set_hash", "total_stake", "shred_version", "route_binding"):
            pins = dict(cfg["stub_deposit_pins"], **{field: 0})
            changed = Relay(
                dict(cfg, state=str(out / field), stub_deposit_pins=pins)
            ).ledger
            rejected("wrong pinned " + field, changed.submit, signed, out)
        later = Relay(dict(cfg, state=str(out / "advanced-epoch"))).ledger
        next_client = dict(later.client(), epoch=later.client()["epoch"] + 1)
        with later.connect() as db:
            db.execute(
                "UPDATE client SET value=? WHERE id=1", (json.dumps(next_client),)
            )
        rejected("deposit outside installed epoch", later.submit, signed, out)
        with stub.connect() as db:
            assert db.execute("SELECT COUNT(*) FROM tx").fetchone()[0] == 0
            assert db.execute("SELECT COUNT(*) FROM credits").fetchone()[0] == 0
        stub.submit(signed, out)
        stub.submit(signed, out)
        with stub.connect() as db:
            assert db.execute("SELECT COUNT(*) FROM tx").fetchone()[0] == 1
            assert db.execute("SELECT COUNT(*) FROM credits").fetchone()[0] == len(
                journal(bytes(original["public_values"]))["receipts"]
            )
        print(
            "PASS valid proof accepted; duplicate envelope preserved transaction and credit counts"
        )


if __name__ == "__main__":
    main()
