"""Submit signed activation or route governance using each resident validator signer."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import re
import shlex
import subprocess
import time
from live import HOSTS, R6, STATE, fleet, read, receipt_at, rpc, save


def remote(i, args, data=None):
    return subprocess.run(
        [
            "ssh",
            "-T",
            "-oBatchMode=yes",
            "-oConnectTimeout=8",
            "root@" + HOSTS[i],
            shlex.join(list(map(str, args))),
        ],
        input=data,
        text=True,
        capture_output=True,
        check=True,
        timeout=150,
    ).stdout


REVIEW_READER = """import os, stat, sys
with os.fdopen(os.open(sys.argv[1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), "rb") as stream:
    s = os.fstat(stream.fileno())
    if not stat.S_ISREG(s.st_mode) or s.st_uid != 0 or s.st_mode & 0o022 or s.st_size > 65536:
        sys.exit("Unsafe operator review file")
    data = stream.read(65537)
    if len(data) > 65536:
        sys.exit("Oversized operator review file")
    sys.stdout.buffer.write(data)
"""


def read_operator_review(i, path):
    return json.loads(remote(i, ["/usr/bin/python3", "-I", "-c", REVIEW_READER, path]))


def draft(profile):
    """Build the public governance proposal on a read replica without signing it."""
    pid = remote(
        0,
        [
            "pgrep",
            "-f",
            "^/opt/postfiat/releases/[^ ]+/postfiat-node transport-validator-serve ",
        ],
    ).strip()
    if not pid.isdecimal():
        raise RuntimeError("expected one validator")
    binary = remote(0, ["readlink", "-f", f"/proc/{pid}/exe"]).strip()
    if not re.fullmatch(
        r"/opt/postfiat/releases/[A-Za-z0-9_.-]+/postfiat-node", binary
    ):
        raise RuntimeError("unexpected node image")

    def user(*args, data=None):
        return remote(0, ["runuser", "-u", "postfiat", "--", *args], data)

    work = user("mktemp", "-d", "/tmp/sol-live-draft-XXXXXXXX").strip()
    if not re.fullmatch(r"/tmp/sol-live-draft-[A-Za-z0-9]{8}", work):
        raise RuntimeError("unexpected draft path")
    user("tee", work + "/profile", data=json.dumps(profile))
    user(
        binary,
        "vault-bridge-route-profile-governance",
        "--data-dir",
        "/var/lib/postfiat/validator-0",
        "--validators",
        ",".join(f"validator-{i}" for i in range(6)),
        "--profile-file",
        work + "/profile",
        "--amendment-file",
        work + "/amendment",
        "--batch-file",
        work + "/batch",
    )
    return json.loads(user("cat", work + "/amendment"))


def recover(profile):
    """Recover a committed round if publication of its local evidence was interrupted."""
    for submitted in (STATE / "governance").glob("route-*/profile.json"):
        if read(submitted) != profile:
            continue
        path = submitted.parent
        if (path / "accepted.json").exists():
            return read(path / "accepted.json")
        if not (path / "amendment.json").exists():
            continue
        amendment = read(path / "amendment.json")
        txid = amendment["amendment_id"]
        try:
            transaction = rpc(0, "tx", tx_id=txid)
        except RuntimeError as error:
            if "rpc_tx_not_found" not in str(error):
                raise
            # A tip change during signing leaves a draft with no committed amendment.
            continue
        if transaction.get("confirmed"):
            height = transaction["block"]["header"]["height"]
            evidence = dict(
                tx_id=txid,
                height=height,
                validators=receipt_at(txid, height),
                profile=profile,
                profile_hash=amendment["kind"].split(":")[-1],
            )
            save(path / "accepted.json", evidence)
            return evidence
    return None


class ParentChanged(RuntimeError):
    """Qualification expired before submitting the round."""


def govern(kind, profile=None, compatible=None, *, qualified_parent=None, payout=None):
    head = fleet()
    if head["build_git_revision"] != (compatible or R6):
        raise RuntimeError("waiting for compatible six-validator release")
    if kind in ("exit-open", "exit-close", "exit-paid"):
        from exit_precheck import parent_of

        if parent_of(head) != qualified_parent:
            raise ParentChanged("Fresh parent qualification required.")
    h = head["block_height"] + 1
    local = STATE / "governance" / f"{kind}-{h}-{time.time_ns()}"
    local.mkdir(mode=0o700, parents=True)
    names = [f"validator-{i}" for i in range(6)]
    dirs = [f"/var/lib/postfiat/{name}" for name in names]
    work = [f"{d}/sol-live/{local.name}" for d in dirs]

    def setup(i):
        # Read argv of the resident validator to bind both program and topology.
        pid = remote(
            i,
            [
                "pgrep",
                "-f",
                "^/opt/postfiat/releases/[^ ]+/postfiat-node transport-validator-serve ",
            ],
        ).strip()
        if not pid.isdecimal():
            raise RuntimeError("expected one transport validator")
        args = remote(i, ["cat", f"/proc/{pid}/cmdline"]).split("\0")
        binary = args[0]
        if args[args.index("--data-dir") + 1] != dirs[i]:
            raise ValueError("Unexpected resident data directory.")
        topo = args[args.index("--topology") + 1]
        if not re.fullmatch(
            r"/opt/postfiat/releases/[A-Za-z0-9_.-]+/postfiat-node", binary
        ) or not topo.startswith("/etc/postfiat/releases/"):
            raise ValueError("unexpected release paths")
        remote(i, ["runuser", "-u", "postfiat", "--", "mkdir", "-p", work[i]])
        return binary, topo

    with ThreadPoolExecutor(6) as pool:
        runtime = list(pool.map(setup, range(6)))

    def tool(i, *args):
        return remote(i, ["runuser", "-u", "postfiat", "--", runtime[i][0], *args])

    def put(i, name, value):
        remote(
            i,
            ["runuser", "-u", "postfiat", "--", "tee", work[i] + "/" + name],
            json.dumps(value),
        )

    def get(i, name):
        return json.loads(remote(i, ["cat", work[i] + "/" + name]))

    if kind == "route":
        if profile["activation_height"] > h:
            raise ValueError("premature activation")
        save(local / "profile.json", profile)
        put(0, "profile", profile)
        tool(
            0,
            "vault-bridge-route-profile-governance",
            "--data-dir",
            dirs[0],
            "--validators",
            ",".join(names),
            "--profile-file",
            work[0] + "/profile",
            "--amendment-file",
            work[0] + "/amend",
            "--batch-file",
            work[0] + "/draft",
        )
    elif kind in ("exit-open", "exit-close", "exit-paid"):
        extra = []
        if kind == "exit-paid":
            extra = [
                "--packet-hash",
                payout["packet"],
                "--solana-slot",
                payout["slot"],
                "--payout-index",
                payout["ordinal"],
                "--signature-hex",
                payout["signature_hex"],
            ]
        tool(
            0,
            "ratify-solana-" + kind,
            *extra,
            "--data-dir",
            dirs[0],
            "--validators",
            ",".join(names),
            "--height",
            h,
            "--amendment-file",
            work[0] + "/amend",
        )
    elif kind == "activation":
        tool(
            0,
            "ratify-solana-bridge-activation-height",
            "--data-dir",
            dirs[0],
            "--validators",
            ",".join(names),
            "--height",
            h + 1,
            "--amendment-file",
            work[0] + "/amend",
        )
    else:
        raise ValueError(kind)
    amend = get(0, "amend")
    if kind == "exit-paid":
        from exit_canary import match_reference

        match_reference(amend, payout)
    if kind in ("exit-open", "exit-close", "exit-paid") and (
        (
            kind != "exit-paid"
            and amend["kind"] != "solana_exit_r8_" + kind.split("-")[1]
        )
        or amend["activation_height"] != h
        or amend["value"] != 1
        or amend.get("paused", False)
        or amend["quorum"] != 6
        or amend["validators"] != names
    ):
        raise ValueError("Unexpected Solana exit amendment.")
    save(local / "amendment.json", amend)

    if kind == "exit-paid":
        def verify_review(i):
            approval = read_operator_review(i, "/etc/postfiat/sol-exit-paid-review.json")
            if approval != dict(kind=amend["kind"], parent=qualified_parent):
                raise ValueError(
                    f"Validator-{i} has not verified this payout and proposal parent."
                )

        with ThreadPoolExecutor(6) as pool:
            list(pool.map(verify_review, range(6)))

    def sign(i):
        if i:
            put(i, "amend", amend)
        tool(
            i,
            "governance-authorization-sign",
            "--data-dir",
            dirs[i],
            "--amendment-file",
            work[i] + "/amend",
            "--validator",
            names[i],
            "--validator-key-file",
            dirs[i] + "/validator_keys.json",
            "--proposal-slot",
            h,
            "--expires-at-height",
            h + 30,
            "--authorization-file",
            work[i] + "/auth",
        )
        return get(i, "auth")

    if kind in ("exit-open", "exit-close", "exit-paid") and parent_of(fleet()) != qualified_parent:
        raise ParentChanged("Parent changed before signing.")
    with ThreadPoolExecutor(6) as pool:
        auth = list(pool.map(sign, range(6)))
    for i, a in enumerate(auth):
        put(0, f"auth-{i}", a)
    tool(
        0,
        "governance-amendment-assemble",
        "--data-dir",
        dirs[0],
        "--amendment-file",
        work[0] + "/amend",
        "--authorization-files",
        ",".join(f"{work[0]}/auth-{i}" for i in range(6)),
        "--proposal-slot",
        h,
        "--output",
        work[0] + "/signed",
    )
    signed = get(0, "signed")
    if kind == "route":
        tool(
            0,
            "vault-bridge-route-profile-governance-assemble",
            "--data-dir",
            dirs[0],
            "--profile-file",
            work[0] + "/profile",
            "--signed-amendment-file",
            work[0] + "/signed",
            "--proposal-slot",
            h,
            "--batch-file",
            work[0] + "/batch",
        )
    else:
        tool(
            0,
            "governance-batch",
            "--data-dir",
            dirs[0],
            "--amendment-file",
            work[0] + "/signed",
            "--batch-file",
            work[0] + "/batch",
        )
    prop = h % 6
    if prop:
        put(prop, "batch", get(0, "batch"))
    if fleet()["block_tip_hash"] != head["block_tip_hash"]:
        raise ParentChanged("tip advanced during signing")
    txid = signed.get("amendment_id") or signed["amendment"]["amendment_id"]
    print(f"{kind} height={h} proposer={prop} tx_id={txid}", flush=True)
    tool(
        prop,
        "transport-peer-certified-batch-round",
        "--data-dir",
        dirs[prop],
        "--topology",
        runtime[prop][1],
        "--batch-kind",
        "governance",
        "--batch-file",
        work[prop] + "/batch",
        "--key-file",
        dirs[prop] + "/validator_keys.json",
        "--proposal-key-file",
        dirs[prop] + "/validator_keys.json",
        "--require-local-proposer",
        "--quorum-early-full-propagation",
        "--local-apply-before-certified-send",
        "--artifact-dir",
        work[prop] + "/round",
        "--height",
        h,
        "--view",
        0,
        "--timeout-ms",
        60000,
        "--send-retries",
        16,
        "--retry-backoff-ms",
        250,
    )
    evidence = dict(tx_id=txid, height=h, validators=receipt_at(txid, h))
    if profile:
        evidence.update(profile=profile, profile_hash=amend["kind"].split(":")[-1])
    save(local / "accepted.json", evidence)
    return evidence


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("kind", choices=("activation", "route"))
    p.add_argument("--profile", type=Path)
    a = p.parse_args()
    print(json.dumps(govern(a.kind, read(a.profile) if a.profile else None)))
