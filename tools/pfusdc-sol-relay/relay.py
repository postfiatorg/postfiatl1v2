#!/usr/bin/env python3
"""Relay a finalized Solana deposit or recorder block and report its durable job."""

import argparse
import json
import logging
import os
import subprocess
import time
from pathlib import Path

from fixture import configure
from jobs import Relay
from route import binding
from storage import AwaitingSource, Invalid, load, save


def follow(relay, through=None):
    cfg = relay.cfg
    cursor = relay.root / "epoch.json"
    epoch = (
        relay.ledger.client()["epoch"]
        if cfg["ledger"] == "stub"
        else load(cursor)
        if cursor.exists()
        else int(cfg["current_epoch"])
        if "current_epoch" in cfg
        else int(relay.ledger.route()["profile"]["solana"]["anchor"]["epoch"])
    )
    root = Path(cfg["capture_root"])

    def records(kind, number):
        for path in sorted(
            root.glob(f"{kind}-{number}*/source.json"),
            key=lambda path: int(load(path)["slot"]),
        ):
            ref = path.parent.name.removeprefix(kind + "-")
            if ref.split("-")[0] == str(number):
                yield relay.create(kind, ref)[0]

    def run_ready(kind, number):
        for job in records(kind, number):
            state = load(job / "job.json")
            if state["status"] == "accepted":
                yield state
            elif state.get("retryable", True):
                try:
                    yield relay.run(job, wait=False)
                except Invalid as error:
                    logging.getLogger(__name__).error(
                        "invalid recorder input %s: %s", job.name, error
                    )

    while through is None or epoch < through:
        try:
            if cfg["ledger"] == "stub":
                for kind in ("handoff", "keys"):
                    for _ in run_ready(kind, epoch):
                        pass
            result = next(run_ready("handoff", epoch + 1), None)
            if result is not None:
                epoch += 1
                save(cursor, epoch)
                print(
                    json.dumps(
                        {
                            "epoch": epoch,
                            "tx_id": result["tx_id"],
                            "timings": result["timings"],
                        }
                    ),
                    flush=True,
                )
                continue
        except (
            OSError,
            RuntimeError,
            subprocess.SubprocessError,
            AwaitingSource,
        ) as error:
            logging.getLogger(__name__).warning(
                "retrying recorder epoch %s: %s", epoch + 1, error
            )
        time.sleep(0.25)


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=Path("relay.json"))
    sub = parser.add_subparsers(dest="cmd", required=True)
    fixture = sub.add_parser(
        "fixture-config",
        help="create a local stub profile with pins from the built guests",
    )
    for opt in ("directory", "capture-root", "host", "capture-binary"):
        fixture.add_argument("--" + opt, type=Path, required=True)
    fixture.add_argument(
        "--difftest", type=Path, help="generate missing differential fixture"
    )
    fixture.add_argument(
        "--program-info",
        type=Path,
        default=Path(__file__).resolve().parents[2]
        / "programs/pfusdc-sol-ingress/program-info.json",
    )
    fixture.add_argument("--prover", choices=("cpu", "cuda", "auto"), default="auto")
    sub.add_parser("deposit").add_argument("signature")
    for kind in ("handoff", "keys"):
        recorder = sub.add_parser(kind)
        recorder.add_argument("epoch", type=int)
        recorder.add_argument("--slot", type=int)
    sub.add_parser("follow").add_argument("--through", type=int)
    sub.add_parser("status").add_argument("id")
    sub.add_parser("summary")
    sub.add_parser(
        "binding", help="compute the route binding from the configured chain and vault"
    )
    http = sub.add_parser("serve")
    http.add_argument("--port", type=int, default=8187)
    args = parser.parse_args()
    if args.cmd == "fixture-config":
        print(configure(args))
        return
    cfg = load(args.config)
    if args.cmd == "binding":
        print(binding(cfg))
        return
    relay = Relay(cfg)
    if args.cmd == "follow":
        return follow(relay, args.through)
    if args.cmd == "serve":
        from http_api import serve

        return serve(relay, args.port)
    if args.cmd == "status":
        result = relay.status(args.id)
    elif args.cmd == "summary":
        result = relay.summary()
    else:
        ref = args.signature if args.cmd == "deposit" else str(args.epoch)
        if getattr(args, "slot", None) is not None:
            ref += "-" + str(args.slot)
        job, replay = relay.create(args.cmd, ref)
        result = dict(relay.run(job), idempotent_replay=replay)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
