"""Prepare the six-validator governance envelope or submit it during the reviewed roll."""

import argparse
import json
import re
import subprocess
from pathlib import Path
from exit_setup import (
    ASSET,
    BUCKET,
    EXIT,
    PROFILE,
    SERIES,
    fleet,
    read,
    require,
    rpc,
    six,
    save,
)
from govern import govern, ParentChanged
from exit_precheck import parent_of

ETH_HASH = "29c91daa1c8d707ea3245db091976219266c5deca82b2d4571fafb221fd2dd092eb8833a28f4422bf0b4bbde44069e28"


def ethereum():
    rows = six(lambda i: rpc(i, "vault_bridge_route", asset_id=ASSET))
    require(
        all(
            r["profile_hash"] == ETH_HASH
            and r["profile"]["route_epoch"] == 11
            and r["profile"] == rows[0]["profile"]
            for r in rows
        ),
        "Ethereum route changed",
    )
    return rows[0]["profile"]


def require_descriptor(rows):
    require(
        len(rows) == 6
        and all(
            r.get("active") is True
            and type(r.get("burns_open")) is bool
            and r["burns_open"] == rows[0]["burns_open"]
            and r.get("profile_hash") == EXIT
            and r.get("profile") == read(PROFILE)
            for r in rows
        ),
        "All six validators must agree on the reviewed Solana descriptor.",
    )


def require_closed(rows):
    require_descriptor(rows)
    require(not rows[0]["burns_open"], "All six validators must have burns closed.")


def require_open_accounting(routes, state, height):
    from exit_payout import reconcile_supply

    require(all(r["current_height"] == height for r in routes), "Stale route supply report.")
    outstanding = state.get("bucket_outstanding_atoms")
    require(isinstance(outstanding, str) and re.fullmatch(r"0|[1-9][0-9]*", outstanding)
            and state.get("series_supply_atoms") == outstanding,
            "Payout service lacks matching full supply and bucket outstanding.")
    reconcile_supply(routes, int(outstanding))


CLEARANCE_READER = """import os, stat, sys
with os.fdopen(os.open(sys.argv[1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), "rb") as stream:
    s = os.fstat(stream.fileno())
    if not stat.S_ISREG(s.st_mode) or s.st_uid != 0 or s.st_mode & 0o022 or s.st_size > 65536:
        sys.exit("Unsafe rotation clearance file")
    sys.stdout.buffer.write(stream.read(65537))
"""


def rotation_clearance(head):
    """Read the independent inventories over the six authenticated host connections."""
    from govern import remote

    require_closed(six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES)))

    def inspect(i):
        directory = f"/var/lib/postfiat/validator-{i}/consensus-v2-safety"
        files = remote(
            i,
            [
                "find",
                directory,
                "-maxdepth",
                "1",
                "-type",
                "f",
                "-name",
                "*.json",
                "-printf",
                "%f\\n",
            ],
        ).splitlines()
        require(files, f"Missing resident signing history on validator-{i}.")
        for name in files:
            prefix = r"epoch-[0-9]+-[a-f0-9]{96}-"
            match = re.fullmatch(r"height-([0-9]+)\.json", name) or re.fullmatch(
                prefix + r"height-([0-9]+)(?:-view-[0-9]+\.timeout-vote)?\.json", name
            )
            require(
                match and int(match[1]) <= head["block_height"],
                f"Unknown or uncommitted signing round on validator-{i}.",
            )
        return json.loads(
            remote(i, ["/usr/bin/python3", "-I", "-c", CLEARANCE_READER,
                       "/etc/postfiat/sol-exit-rotation-clearance.json"])
        )

    rows = six(inspect)
    for i, row in enumerate(rows):
        require(
            row["validator"] == f"validator-{i}"
            and row["parent"] == parent_of(head)
            and row["revision"] == head["build_git_revision"]
            and row["profile_hash"] == EXIT
            and row["hold_through_open"] is True
            and row["outstanding_signed_rotation_rounds"] == [],
            f"Missing, stale or outstanding rotation clearance on validator-{i}.",
        )
    require(
        parent_of(fleet()) == parent_of(head), "Tip advanced during rotation checks."
    )
    return rows


def verify_open_parent(rows, tx_id, parent):
    require(
        len(rows) == 6
        and all(
            r["confirmed"]
            and r["receipt"]["tx_id"] == tx_id
            and r["receipt"]["accepted"]
            and r["block"]["header"]["height"] == parent["block_height"] + 1
            and r["block"]["header"]["parent_hash"] == parent["block_tip_hash"]
            and r["block"]["header"] == rows[0]["block"]["header"]
            for r in rows
        ),
        "Open receipts disagree with the qualified parent.",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest="action", required=True)
    p = subs.add_parser("prepare")
    p.add_argument("--binary", required=True, type=Path)
    p.add_argument("--copied-store", required=True, type=Path)
    p.add_argument("--output", required=True, type=Path)
    for name in ("submit", "open", "close", "paid"):
        p = subs.add_parser(name)
        if name == "paid":
            p.add_argument("--paid", required=True, type=Path)
        if name in ("close", "paid"):
            p.add_argument("--output", required=True, type=Path)
        if name == "open":
            p.add_argument("--output", type=Path)
        p.add_argument("--revision", required=True)
        p.add_argument("--release-height", required=True, type=int)
    args = parser.parse_args()
    if args.action == "prepare":
        args.output.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            [
                args.binary,
                "vault-bridge-route-profile-governance",
                "--data-dir",
                args.copied_store,
                "--validators",
                ",".join(f"validator-{i}" for i in range(6)),
                "--profile-file",
                PROFILE,
                "--amendment-file",
                args.output / "amendment.json",
                "--batch-file",
                args.output / "batch.json",
            ],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        require(
            read(args.output / "amendment.json")["kind"].endswith(":" + EXIT),
            "Unexpected draft hash",
        )
        return
    head = fleet()
    require(
        head["build_git_revision"] == args.revision
        and head["block_height"] + 1 >= args.release_height,
        "Waiting for the reviewed release and height",
    )
    release_action = args.action in ("submit", "open")
    if release_action:
        before = ethereum()
    else:
        routes = six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES))
        require_descriptor(routes)
        require(all(r["current_height"] == head["block_height"] for r in routes)
                and parent_of(fleet()) == parent_of(head), "Recovery parent changed.")
    if args.action == "open":
        import requests

        for _ in range(3):
            candidate = fleet()
            start = parent_of(candidate)
            closed = six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES))
            require_closed(closed)
            status = requests.get(
                "http://127.0.0.1:18089/solana/withdrawals/readiness/qualify",
                timeout=600,
            )
            status.raise_for_status()
            state = status.json()
            if start != parent_of(fleet()):
                continue
            require(
                state.get("payout_ready") is True
                and state.get("burns_open") is False
                and state.get("bucket_id") == BUCKET,
                "Payout service is not qualified.",
            )
            require_open_accounting(closed, state, start["block_height"])
            try:
                if state["qualified_parent"] != start or start != parent_of(fleet()):
                    continue
                rotation_clearance(candidate)
                accepted = govern(
                    "exit-open",
                    compatible=args.revision,
                    qualified_parent=state["qualified_parent"],
                )
            except ParentChanged:
                continue
            break
        else:
            raise ParentChanged("Tip advanced repeatedly. Restart the open command.")
        verify_open_parent(
            six(lambda i: rpc(i, "tx", tx_id=accepted["tx_id"])),
            accepted["tx_id"],
            state["qualified_parent"],
        )
        rows = six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES))
        require(
            all(
                r["active"]
                and r.get("burns_open") is True
                and r["profile_hash"] == EXIT
                for r in rows
            ),
            "Open-burn readback failed.",
        )
        if args.output:
            save(
                args.output,
                dict(accepted, qualified_parent=state["qualified_parent"], routes=rows),
            )
    elif args.action == "paid":
        from exit_canary import review_paid

        ref = review_paid(read(args.paid))
        save(args.output, govern("exit-paid", compatible=args.revision, qualified_parent=parent_of(head), payout=ref))
    elif args.action == "close":
        result = govern("exit-close", compatible=args.revision, qualified_parent=parent_of(head))
        require_closed(six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES)))
        save(args.output, result)
    else:
        govern("route", read(PROFILE), compatible=args.revision)
        require_closed(six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES)))
    if release_action:
        require(ethereum() == before, "Ethereum route changed during governance.")
    print("Governance decision confirmed by six validators.")


if __name__ == "__main__":
    main()
