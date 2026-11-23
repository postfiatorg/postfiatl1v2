"""Prepare the six-validator governance envelope or submit it during the reviewed roll."""

import argparse
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
)
from govern import govern

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


def require_closed(rows):
    require(
        len(rows) == 6
        and all(
            r.get("active") is True
            and r.get("burns_open") is False
            and r.get("profile_hash") == EXIT
            and r.get("profile") == read(PROFILE)
            for r in rows
        ),
        "All six validators must have the reviewed descriptor with burns closed.",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest="action", required=True)
    p = subs.add_parser("prepare")
    p.add_argument("--binary", required=True, type=Path)
    p.add_argument("--copied-store", required=True, type=Path)
    p.add_argument("--output", required=True, type=Path)
    for name in ("submit", "open"):
        p = subs.add_parser(name)
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
    before = ethereum()
    if args.action == "open":
        import requests

        require_closed(six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES)))
        status = requests.get(
            "http://127.0.0.1:18089/solana/withdrawals/readiness", timeout=90
        )
        status.raise_for_status()
        state = status.json()
        require(
            state.get("payout_ready") is True
            and state.get("burns_open") is False
            and state.get("bucket_id") == BUCKET,
            "Payout service is not qualified.",
        )
        govern("exit-open", compatible=args.revision)
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
    else:
        govern("route", read(PROFILE), compatible=args.revision)
        require_closed(six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES)))
    require(ethereum() == before, "Ethereum route changed during governance.")
    print("Governance decision confirmed by six validators.")


if __name__ == "__main__":
    main()
