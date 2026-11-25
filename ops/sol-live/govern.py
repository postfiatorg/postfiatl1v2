"""Submit signed activation or route governance using each resident validator signer."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import time
from live import HOSTS, R6, STATE, fleet, read, receipt_at, rpc, save
import exit_rules


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


# The settlement loop's restricted path: its own key, the reviewer's user, a forced command that
# accepts only "review" and "result" (exit_review_door.py). No root login and no command.
DOOR_USER = "pf-sol-exit-review"
DOOR_KEY = Path(os.environ.get("PFT_SETTLER_SSH_KEY", str(Path.home() / ".ssh/pft-sol-exit-settler")))
DOOR_POLLS = 40
DOOR_PAUSE = 3


def door(i, request):
    """One request through host i's restricted door; a named refusal raises ReviewRefused."""
    out = subprocess.run(
        ["ssh", "-T", "-oBatchMode=yes", "-oConnectTimeout=8", "-oIdentitiesOnly=yes",
         "-i", str(DOOR_KEY), f"{DOOR_USER}@{HOSTS[i]}"],
        input=json.dumps(request), text=True, capture_output=True, timeout=60,
    )
    if out.returncode == 2:
        raise ReviewRefused(((out.stderr or "").strip().splitlines() or ["refused"])[-1])
    if out.returncode != 0:
        raise RuntimeError(f"validator-{i} door: exit {out.returncode}: {(out.stderr or '').strip()[-200:]}")
    return json.loads(out.stdout)


def door_sign(i, amend, h):
    """Hand the drafted decision to host i's own reviewer and wait for its result: the host's
    signature for this slot, or its refusal with the reason."""
    sha = hashlib.sha256(amend["kind"].encode()).hexdigest()
    door(i, dict(request="review", amendment=amend, proposal_slot=h, expires_at_height=h + 30))
    for _ in range(DOOR_POLLS):
        result = door(i, dict(request="result", decision_sha256=sha))
        if result.get("proposal_slot") == h and result.get("expires_at_height") == h + 30:
            if "refused" in result:
                raise ReviewRefused(result["refused"])
            if "error" in result:
                raise RuntimeError(f"validator-{i}: {result['error']}")
            return json.loads(result["authorization"])
        time.sleep(DOOR_PAUSE)
    raise RuntimeError(f"validator-{i}: no review result for this decision in time")


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


def _unavailable(i):
    raise RuntimeError(f"validator-{i} unavailable")


def required_signatures(kind, n=6, *, rules=None):
    """Signatures the chain requires, as its route report states them (`exit_rules`).
    Without reported rules the first release applies: every validator signs."""
    if kind.startswith("exit-") and rules is not None:
        return exit_rules.signatures(rules, kind)
    return n


def allowed_quorums(kind, n=6, rules=None):
    """The draft quorum the wrapper accepts: the one the chain reports for this action."""
    return {required_signatures(kind, n, rules=rules)}


class ReviewRefused(RuntimeError):
    """A host's own reviewer refused to sign; its reason is reported, apart from unreachable hosts."""


def collect(kind, step, n=6, need=None):
    """Run `step` on every validator; keep going while the chain's quorum can still sign. A host
    whose reviewer refused is reported with its reason even when the others reach the quorum."""
    with ThreadPoolExecutor(n) as pool:
        futures = {i: pool.submit(step, i) for i in range(n)}
    done, lost, refused = {}, [], []
    for i, future in futures.items():
        try:
            done[i] = future.result()
        except ReviewRefused as error:
            refused.append(f"validator-{i}: {error}")
        except Exception:
            lost.append(f"validator-{i}")
    if refused:
        print(f"{kind}: refused by its own reviewer: " + "; ".join(refused), flush=True)
    need = required_signatures(kind, n) if need is None else need
    if len(done) < need:
        raise RuntimeError(
            f"{kind}: {len(done)} of {n} validators available, {need} required; "
            f"unavailable: {', '.join(lost) or 'none'}"
            + (f"; refused by its own reviewer: {'; '.join(refused)}" if refused else "")
        )
    return done


# The per-host settlement reviewer, installed on every validator host (RUNBOOK-LIVE.md,
# "Independent settlement reviews"). Its `sign` runs the node's signing command only for a
# decision that host's own reviewer approved within the last 15 minutes.
REVIEWER = "/opt/postfiat/sol-exit-review/exit_review.py"
REVIEWER_PYTHON = "/opt/postfiat/sol-exit-review/venv/bin/python"


def independent_review(kind, rules):
    """Under the second release, paid and refund are signed only through each host's own
    reviewer. A first-release chain keeps the go-live review files and six signatures."""
    return kind in ("exit-paid", "exit-refund") and rules is not None and rules.get("release") == 2


# Solana exit decisions bound to a qualified Solana parent and drafted by `ratify-solana-<kind>`.
EXIT_DECISIONS = ("exit-open", "exit-close", "exit-paid", "exit-refund")


def read_quorum(kind, n=6, *, rules=None):
    """Validators whose agreeing reads suffice: the signatures the chain requires."""
    return required_signatures(kind, n, rules=rules)


def govern(kind, profile=None, compatible=None, *, qualified_parent=None, payout=None,
           rules=None, refund=None, restricted=False):
    """`restricted`: the hosts' signatures come only through their restricted doors (`door_sign`),
    never through a root login to the signing step."""
    need = read_quorum(kind, rules=rules)
    head = fleet(need)
    if head["build_git_revision"] != (compatible or R6):
        raise RuntimeError("waiting for compatible six-validator release")
    if kind in EXIT_DECISIONS:
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

    # Prepare on every reachable validator; the draft's quorum decides how many must sign.
    runtime = collect(kind, setup, need=min(allowed_quorums(kind, rules=rules)))
    prop = h % 6
    if prop not in runtime:
        raise RuntimeError(f"Proposer validator-{prop} unavailable; retry at the next height.")
    # The first available validator drafts and assembles; every signer checks the draft.
    lead = min(runtime)

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
        put(lead, "profile", profile)
        tool(
            lead,
            "vault-bridge-route-profile-governance",
            "--data-dir",
            dirs[lead],
            "--validators",
            ",".join(names),
            "--profile-file",
            work[lead] + "/profile",
            "--amendment-file",
            work[lead] + "/amend",
            "--batch-file",
            work[lead] + "/draft",
        )
    elif kind in EXIT_DECISIONS:
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
        if kind == "exit-refund":
            extra = [
                "--packet-hash",
                refund["packet"],
                "--solana-slot",
                refund["slot"],
                "--signature-hex",
                refund["signature_hex"],
            ]
        tool(
            lead,
            "ratify-solana-" + kind,
            *extra,
            "--data-dir",
            dirs[lead],
            "--validators",
            ",".join(names),
            "--height",
            h,
            "--amendment-file",
            work[lead] + "/amend",
        )
    elif kind == "activation":
        tool(
            lead,
            "ratify-solana-bridge-activation-height",
            "--data-dir",
            dirs[lead],
            "--validators",
            ",".join(names),
            "--height",
            h + 1,
            "--amendment-file",
            work[lead] + "/amend",
        )
    else:
        raise ValueError(kind)
    amend = get(lead, "amend")
    if kind == "exit-paid":
        from exit_canary import match_reference

        match_reference(amend, payout)
    if kind == "exit-refund" and amend["kind"] != refund["kind"]:
        raise ValueError("Refund draft differs from the cancelled marker's reference.")
    if kind in EXIT_DECISIONS and (
        (
            kind not in ("exit-paid", "exit-refund")
            and amend["kind"] != "solana_exit_r8_" + kind.split("-")[1]
        )
        or amend["activation_height"] != h
        or amend["value"] != 1
        or amend.get("paused", False)
        or amend["quorum"] not in allowed_quorums(kind, rules=rules)
        or amend["validators"] != names
    ):
        raise ValueError("Unexpected Solana exit amendment.")
    save(local / "amendment.json", amend)

    reviewed = set(runtime)
    independent = independent_review(kind, rules)
    if kind in ("exit-paid", "exit-refund") and not independent:
        # Each signer's operator has checked the payout, or the finalized cancelled marker.
        def verify_review(i):
            if i not in runtime:
                _unavailable(i)
            try:
                approval = read_operator_review(
                    i, f"/etc/postfiat/sol-{kind}-review.json"
                )
            except subprocess.CalledProcessError as error:
                # An unsafe or oversized review file stops the round; an unreachable
                # host or a missing review only keeps that validator from signing.
                if "operator review file" in (error.stderr or ""):
                    return error
                raise
            if approval != dict(kind=amend["kind"], parent=qualified_parent):
                return ValueError(
                    f"Validator-{i} has not verified this {kind.split('-')[1]} and proposal parent."
                )
            return None

        # Every reachable review is read before anyone signs. A review that differs
        # or is unprotected stops the round; the chain's quorum must have reviewed.
        reviews = collect(kind, verify_review, need=amend["quorum"])
        for i in sorted(reviews):
            if reviews[i] is not None:
                raise reviews[i]
        reviewed = set(reviews)

    if restricted and not independent:
        raise ValueError("The restricted path signs only paid and refund under the second release.")

    def sign(i):
        if restricted:
            return door_sign(i, amend, h)
        if i != lead:
            put(i, "amend", amend)
        if independent:
            # The host's own reviewer signs exactly the amendment it reviewed: it builds the
            # node's signing command itself and takes no command from this caller.
            try:
                remote(i, ["runuser", "-u", "postfiat", "--", REVIEWER_PYTHON, "-I", REVIEWER, "sign",
                           "--amendment-file", work[i] + "/amend", "--proposal-slot", h,
                           "--expires-at-height", h + 30, "--authorization-file", work[i] + "/auth"])
            except subprocess.CalledProcessError as error:
                if error.returncode == 2:  # the reviewer's named refusal, not an unreachable host
                    reason = ((error.stderr or "").strip().splitlines() or ["refused"])[-1]
                    raise ReviewRefused(reason) from None
                raise
            return get(i, "auth")
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

    if kind in EXIT_DECISIONS and parent_of(fleet(need)) != qualified_parent:
        raise ParentChanged("Parent changed before signing.")
    need = amend["quorum"] if kind.startswith("exit-") else required_signatures(kind)
    auth = collect(
        kind, lambda i: sign(i) if i in runtime and i in reviewed else _unavailable(i),
        need=need,
    )
    signers = sorted(auth)
    print(
        f"{kind} signed by {len(signers)} of 6: "
        + ", ".join(names[i] for i in signers),
        flush=True,
    )
    for i, a in auth.items():
        put(lead, f"auth-{i}", a)
    tool(
        lead,
        "governance-amendment-assemble",
        "--data-dir",
        dirs[lead],
        "--amendment-file",
        work[lead] + "/amend",
        "--authorization-files",
        ",".join(f"{work[lead]}/auth-{i}" for i in signers),
        "--proposal-slot",
        h,
        "--output",
        work[lead] + "/signed",
    )
    signed = get(lead, "signed")
    if kind == "route":
        tool(
            lead,
            "vault-bridge-route-profile-governance-assemble",
            "--data-dir",
            dirs[lead],
            "--profile-file",
            work[lead] + "/profile",
            "--signed-amendment-file",
            work[lead] + "/signed",
            "--proposal-slot",
            h,
            "--batch-file",
            work[lead] + "/batch",
        )
    else:
        tool(
            lead,
            "governance-batch",
            "--data-dir",
            dirs[lead],
            "--amendment-file",
            work[lead] + "/signed",
            "--batch-file",
            work[lead] + "/batch",
        )
    if prop != lead:
        put(prop, "batch", get(lead, "batch"))
    if fleet(need)["block_tip_hash"] != head["block_tip_hash"]:
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
    evidence = dict(tx_id=txid, height=h, signers=[names[i] for i in signers])
    if profile:
        evidence.update(profile=profile, profile_hash=amend["kind"].split(":")[-1])
    # Record the submitted action before the survey: a validator lost now must not
    # leave a committed action without its evidence.
    save(local / "accepted.json", evidence)
    evidence["validators"] = receipt_at(txid, h, need=need)
    save(local / "accepted.json", evidence)
    return evidence


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("kind", choices=("activation", "route"))
    p.add_argument("--profile", type=Path)
    a = p.parse_args()
    print(json.dumps(govern(a.kind, read(a.profile) if a.profile else None)))
