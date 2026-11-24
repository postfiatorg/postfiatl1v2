"""Prepare the six-validator governance envelope or submit it during the reviewed roll."""

import argparse
import json
import re
import subprocess
from pathlib import Path
from live import agreeing, survey
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
from govern import govern, ParentChanged, read_operator_review
from exit_precheck import parent_of
from exit_payout import SOURCE_DOMAIN
from exit_qualification_socket import qualify
import exit_rules

ETH_HASH = "29c91daa1c8d707ea3245db091976219266c5deca82b2d4571fafb221fd2dd092eb8833a28f4422bf0b4bbde44069e28"


def agreed(rows, key, need, message):
    """The largest group of at least `need` answers with equal `key`; a lagging or
    different minority does not block."""
    rows = dict(enumerate(rows)) if isinstance(rows, list) else rows
    return [rows[i] for i in agreeing(rows, key, need, message)]


RULE_FIELDS = ("covered_bucket_ids", "decision_signatures", "min_withdrawal_atoms")


def chain_rules():
    """The exit rules five agreeing validators report (`exit_rules`): the first release's
    when they report none, so every decision then needs all six."""
    rows = agreed(
        routes(SERIES, 5),
        lambda r: json.dumps([r.get(k) for k in RULE_FIELDS], sort_keys=True),
        5,
        "Validators disagree on the exit rules.",
    )
    return exit_rules.agreed(rows)


def routes(asset_id, need=6):
    """Route reports of the validators that answered (at least `need`)."""
    return list(survey(lambda i: rpc(i, "vault_bridge_route", asset_id=asset_id), need).values())


def ethereum(need=6):
    """The Ethereum route of the largest group of at least `need` agreeing validators."""
    (row, *_) = agreed(
        routes(ASSET, need),
        lambda r: (r["profile_hash"], json.dumps(r["profile"], sort_keys=True)),
        need,
        "Ethereum route changed",
    )
    require(
        row["profile_hash"] == ETH_HASH and row["profile"]["route_epoch"] == 11,
        "Ethereum route changed",
    )
    return row["profile"]


def described(r):
    return (
        r.get("active") is True
        and type(r.get("burns_open")) is bool
        and r.get("profile_hash") == EXIT
        and r.get("profile") == read(PROFILE)
    )


def require_descriptor(rows, need=6):
    """At least `need` validators agree on the reviewed descriptor and on burn admission;
    all six where the chain needs six, five for close, paid and a re-opening."""
    message = f"At least {need} validators must agree on the reviewed Solana descriptor."
    try:
        group = agreed(rows, lambda r: (described(r), r.get("burns_open")), need, message)
    except RuntimeError:
        group = [None]
    require(group[0] is not None and described(group[0]), message)
    return group


def require_closed(rows, need=6):
    group = require_descriptor(rows, need)
    require(
        group[0]["burns_open"] is False,
        f"At least {need} validators must have the reviewed descriptor with burns closed.",
    )


def require_open_accounting(routes, state, height, need=6):
    from exit_payout import reconcile_supply

    current = [r for r in routes if r["current_height"] == height]
    require(len(routes) >= need and len(current) >= need, "Stale route supply report.")
    outstanding = state.get("bucket_outstanding_atoms")
    require(isinstance(outstanding, str) and re.fullmatch(r"0|[1-9][0-9]*", outstanding)
            and state.get("series_supply_atoms") == outstanding,
            "Payout service lacks matching full supply and bucket outstanding.")
    reconcile_supply(current, int(outstanding), need)
    reconcile_service_buckets(state, height, need)


def reconcile_service_buckets(state, height, need=6):
    """Every other covered bucket the service reports reconciles like revision 8: the
    service's supply equals its outstanding, and so do the validators' own reports."""
    from exit_payout import bucket_id, reconcile_supply, series_routes

    for b in state.get("buckets", []):
        if b.get("bucket_id") == BUCKET:
            continue
        outstanding = b.get("outstanding_atoms")
        require(isinstance(outstanding, str) and re.fullmatch(r"0|[1-9][0-9]*", outstanding)
                and b.get("series_supply_atoms") == outstanding,
                "Payout service lacks matching full supply and bucket outstanding.")
        rows = series_routes(b["series_asset_id"], need, height)
        policy = (rows[0].get("source_backing") or {}).get("policy_hash") if rows else None
        require(policy is not None and bucket_id(policy) == b["bucket_id"],
                "Missing validator supply report.")
        reconcile_supply(rows, int(outstanding), need, dict(bucket_id=b["bucket_id"], policy_hash=policy))


def rotation_clearance(head, need=6):
    """Read the independent inventories over the authenticated host connections: all six
    for setup and a first opening, the five answering ones for a re-opening."""
    from govern import remote

    require_closed(routes(SERIES, need), need)

    def fetch(i):
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
        try:
            return files, read_operator_review(i, "/etc/postfiat/sol-exit-rotation-clearance.json")
        except subprocess.CalledProcessError as error:
            # An unsafe clearance file fails the check; only an unreachable host is skipped.
            if "operator review file" in (error.stderr or ""):
                return files, error
            raise

    def inspect(i, files, clearance):
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
        if isinstance(clearance, Exception):
            raise clearance
        return clearance

    # Every answering validator's history and clearance must pass; at most the
    # validators beyond `need` may be unreachable.
    rows = {i: inspect(i, *answer) for i, answer in survey(fetch, need).items()}
    for i, row in sorted(rows.items()):
        require(
            row["validator"] == f"validator-{i}"
            and row["parent"] == parent_of(head)
            and row["revision"] == head["build_git_revision"]
            and row["profile_hash"] == EXIT
            and row["hold_through_open"] is True
            and row["outstanding_signed_rotation_rounds"] == [],
            f"Missing, stale or outstanding rotation clearance on validator-{i}.",
        )
    if parent_of(fleet(need)) != parent_of(head):
        raise ParentChanged("Tip advanced during rotation checks.")
    return [rows[i] for i in sorted(rows)]


def verify_open_parent(rows, tx_id, parent, need=6):
    """The largest group of at least `need` answers agrees on the committed open; one
    validator that answers differently does not fail the readback."""
    message = "Open receipts disagree with the qualified parent."
    require(len(rows) >= need, message)
    try:
        (r, *_) = agreed(
            rows,
            lambda r: json.dumps([r.get("confirmed"), r.get("receipt"),
                                  (r.get("block") or {}).get("header")], sort_keys=True),
            need,
            message,
        )
    except (RuntimeError, ValueError):
        raise ValueError(message) from None
    require(
        r["confirmed"]
        and r["receipt"]["tx_id"] == tx_id
        and r["receipt"]["accepted"]
        and r["block"]["header"]["height"] == parent["block_height"] + 1
        and r["block"]["header"]["parent_hash"] == parent["block_tip_hash"],
        message,
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
    # Each decision needs the signatures the chain reports for it now (six for all of
    # them under the first release); reads use the same quorum.
    rules = chain_rules()
    need = (
        exit_rules.signatures(rules, "exit-" + args.action)
        if args.action in ("open", "close", "paid")
        else 6
    )
    head = fleet(need)
    require(
        head["build_git_revision"] == args.revision
        and head["block_height"] + 1 >= args.release_height,
        "Waiting for the reviewed release and height",
    )
    release_action = args.action in ("submit", "open")
    if release_action:
        before = ethereum(need)
    else:
        # Close and paid are bound to the Solana parent that five agreeing validators report.
        recovery = [r for r in routes(SERIES, need) if r["current_height"] == head["block_height"]]
        require_descriptor(recovery, need)
        require(len(recovery) >= need and parent_of(fleet(need)) == parent_of(head),
                "Recovery parent changed.")
    if args.action == "open":
        for _ in range(3):
            candidate = fleet(need)
            start = parent_of(candidate)
            closed = routes(SERIES, need)
            require_closed(closed, need)
            state = qualify()
            if start != parent_of(fleet(need)):
                continue
            require(
                state.get("payout_ready") is True
                and state.get("burns_open") is False
                and state.get("source_domain") == SOURCE_DOMAIN,
                "Payout service is not qualified.",
            )
            require_open_accounting(closed, state, start["block_height"], need)
            try:
                if state["qualified_parent"] != start or start != parent_of(fleet(need)):
                    continue
                rotation_clearance(candidate, need)
                accepted = govern(
                    "exit-open",
                    compatible=args.revision,
                    qualified_parent=state["qualified_parent"],
                    rules=rules,
                )
            except ParentChanged:
                continue
            break
        else:
            raise ParentChanged("Tip advanced repeatedly. Restart the open command.")
        verify_open_parent(
            list(survey(lambda i: rpc(i, "tx", tx_id=accepted["tx_id"]), need).values()),
            accepted["tx_id"],
            state["qualified_parent"],
            need,
        )

        def opened(r):
            return r["active"] and r.get("burns_open") is True and r["profile_hash"] == EXIT

        rows = agreed(routes(SERIES, need), opened, need, "Open-burn readback failed.")
        require(opened(rows[0]), "Open-burn readback failed.")
        if args.output:
            save(
                args.output,
                dict(accepted, qualified_parent=state["qualified_parent"], routes=rows),
            )
    elif args.action == "paid":
        from exit_canary import review_paid

        ref = review_paid(read(args.paid), need=need)
        result = govern("exit-paid", compatible=args.revision,
                        qualified_parent=parent_of(head), payout=ref, rules=rules)
        save(args.output, result)
    elif args.action == "close":
        result = govern("exit-close", compatible=args.revision, qualified_parent=parent_of(head),
                        rules=rules)
        require_closed(routes(SERIES, need), need)
        save(args.output, result)
    else:
        result = govern("route", read(PROFILE), compatible=args.revision)
        require_closed(six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES)))
    if release_action:
        require(ethereum(need) == before, "Ethereum route changed during governance.")
    signers = (accepted if args.action == "open" else result).get("signers", [])
    print(f"Governance decision signed by {len(signers)} of 6: {', '.join(signers)}.")


if __name__ == "__main__":
    main()
