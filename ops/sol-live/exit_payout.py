"""Prove finalized Post Fiat burns and release to the owner's canonical USDC ATA."""

import base64
import contextlib
import hashlib
import json
import logging
import os
from pathlib import Path
import re
import shutil
import tempfile
import threading
import time
from solders.transaction import Transaction

from solders.compute_budget import set_compute_unit_limit
from solders.pubkey import Pubkey
from solders.hash import Hash
from solders.message import Message
from solders.system_program import create_account
from exit_setup import (
    ASSET,
    BUCKET,
    DIRECTORY,
    EXIT,
    LIGHT,
    FEE_PAYER,
    fleet,
    check_payout_authorities,
    ExitCluster,
    MINT,
    POLICY,
    SERIES,
    SYSTEM,
    TOKEN,
    VAULT,
    VERIFIER,
    Instruction,
    checkpoint_pda,
    expected_light,
    profile_code,
    verifier_pin,
    load,
    pk,
    pda,
    read,
    require,
    ro,
    rpc,
    rw,
    six,
)
from live import EmptyReply, ReplyTooLarge, agreeing, survey
import exit_rules
from exit_segments import (
    public_checkpoint,
    select_target,
    validate_segment,
    committee,
)
from exit_precheck import inspect_vault, recipient_atoms, parent_of, qualify_path
from exit_proof import available, prove

ATA = pk("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
PREFIX = "solana-spl:45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0:"
FLOOR = 20_000_000
READ_QUORUM = 5
# The shared Solana custody (genesis, vault, mint), as in `postfiat_types::solana_exit`.
SOURCE_DOMAIN = (
    "spl:45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0"
    ":e2a6fbfcf1e65a0f3e08d400c97d913e3e4eca81f84c814efa0bbcada24aac8b"
    ":c6fa7af3bedbad3a3d65f36aabc97431b1bbe4c2d2f6e0e47ca60203452f5d61"
)
ACCOUNT = re.compile(r"pf[0-9a-f]{40}")


# A validator that failed a read is asked last for this long, by every thread.
SKIP_SECONDS = 300
# Seconds a read may take: small reads are quick; witness and page reads are large.
SMALL_READ_SECONDS = 20
READ_SECONDS = {"pfusdc_egress_witness": 120, "pfusdc_checkpoint_witness": 120, "blocks": 60,
                "vault_bridge_status": 60, "validators": 60}
# One payout attempt's reads must end within this; proving and Solana sends are not reads.
ATTEMPT_SECONDS = 1800
_reads = threading.Lock()
_skip = {}
_preferred = [0]
_attempt = threading.local()


class AttemptExpired(RuntimeError):
    """This payout attempt ran out of time; the job is retried later."""


def reset_reads():
    with _reads:
        _skip.clear()
        _preferred[0] = 0


@contextlib.contextmanager
def attempt_deadline(deadline):
    """Bound the reads of one payout attempt (this thread only); admission and status
    threads have no deadline and share only the skip memory."""
    previous = getattr(_attempt, "deadline", None)
    _attempt.deadline = deadline
    try:
        yield
    finally:
        _attempt.deadline = previous


def rpc_any(method, **params):
    """Read from the validator that answered last, else the next one; a validator that
    failed is asked last for SKIP_SECONDS. Any answer is safe: the proof checks
    committee-signed finality and Solana checks the proof. A second read as a
    cross-check would double the full-state reads for no safety. A reply too large for
    the node's frame is too large on every validator, so it is not asked elsewhere."""
    deadline = getattr(_attempt, "deadline", None)
    limit = READ_SECONDS.get(method, SMALL_READ_SECONDS)
    with _reads:
        now = time.monotonic()
        order = [(_preferred[0] + k) % 6 for k in range(6)]
        order = [i for i in order if _skip.get(i, 0) <= now] + [
            i for i in order if _skip.get(i, 0) > now
        ]
    errors, empty, unreachable = [], 0, 0
    for i in order:
        timeout = limit
        if deadline is not None:
            left = deadline - time.monotonic()
            if left <= 0:
                raise AttemptExpired(f"payout attempt deadline passed before {method}")
            timeout = min(timeout, left)
        try:
            result = rpc(i, method, timeout=timeout, **params)
        except EmptyReply as error:
            # A restarting validator also sends an empty line: try the next one.
            empty += 1
            with _reads:
                _skip[i] = time.monotonic() + SKIP_SECONDS
            errors.append(f"validator-{i}: {error}")
            continue
        except ReplyTooLarge:
            raise
        except Exception as error:
            unreachable += isinstance(error, OSError)
            with _reads:
                _skip[i] = time.monotonic() + SKIP_SECONDS
            errors.append(f"validator-{i}: {error}")
            continue
        with _reads:
            _preferred[0] = i
        return result
    # Every validator that could be reached sent an empty line: the reply does not fit.
    if empty and empty == len(order) - unreachable:
        raise ReplyTooLarge(f"empty reply from every reachable validator for {method}")
    raise RuntimeError("no validator answered " + method + "; " + "; ".join(errors))


def find_redemption(burn):
    """The burn's redemption and the status it came from. A validator a block behind has not
    seen it yet, so the others are asked before the burn counts as unknown (then retried)."""
    status = rpc_any("vault_bridge_status", asset_id=ASSET)
    red = next((r for r in status["redemptions"] if r["burn_tx_id"] == burn), None)
    for i in range(6) if red is None else ():
        try:
            other = rpc(i, "vault_bridge_status", timeout=READ_SECONDS["vault_bridge_status"], asset_id=ASSET)
        except Exception:
            continue
        red = next((r for r in other["redemptions"] if r["burn_tx_id"] == burn), None)
        if red is not None:
            return red, other
    if red is None:
        raise UnknownBurn("No finalized redemption for this burn on any validator yet.")
    return red, status


BLOCK_TOO_LARGE = "refused: one block's reply exceeds the node's frame limit"
ROTATED = "refused: the validator set changed in the block before the burn"


class BlockTooLarge(ValueError):
    """Even a one-block page does not fit the node's reply limit."""

    def __init__(self, detail):
        super().__init__(f"{BLOCK_TOO_LARGE} ({detail})")


class CommitteeRotated(ValueError):
    """The burn block's committee differs from its parent's; the paged witness cannot
    span it. A checkpoint across the rotation must be advanced first."""

    def __init__(self):
        super().__init__(ROTATED)


def ancestry_pages(start, end):
    """Blocks start..end-1 in pages of eight, halved down to one block while a reply is
    too large; checked to link block to block before anything is proved."""
    blocks, first, size = [], start, 8
    while first < end:
        count = min(size, end - first)
        try:
            page = rpc_any("blocks", from_height=first, limit=count)
        except ReplyTooLarge as error:
            if count == 1:
                raise BlockTooLarge(f"block {first}: {error}") from error
            size = max(1, count // 2)
            continue
        require(
            len(page) == count and page[0]["header"]["height"] == first,
            "Incomplete ancestry page.",
        )
        blocks.extend(page)
        first += count
    for prev, block in zip(blocks, blocks[1:]):
        require(
            block["header"]["parent_hash"] == prev["header"]["block_hash"]
            and block["header"]["height"] == prev["header"]["height"] + 1,
            f"Ancestry pages do not link at height {block['header']['height']}.",
        )
    return blocks


def fetch_any(_i, method, **params):
    """`rpc`-shaped reader for the checkpoint helpers: falls back like `rpc_any`, so
    losing validator 0 does not stop the checkpoint advance or the qualification."""
    return rpc_any(method, **params)


def bucket_id(policy_hash, source_domain=SOURCE_DOMAIN):
    """`vault_bridge_bucket_id` of the exit family: each route revision has its own bucket."""
    preimage = (
        f"asset_id={ASSET}\nsource_domain_bytes={len(source_domain)}\n"
        f"source_domain={source_domain}\npolicy_hash={policy_hash}\n"
    )
    return hashlib.sha3_384(
        b"postfiat.vault_bridge_bucket_id.v1\0" + preimage.encode()
    ).hexdigest()


def covered_buckets(buckets, covered_ids=None):
    """Buckets of the shared custody that the governed exit descriptor covers, by ID; with
    `covered_ids`, only those the chain reports (`exit_rules`)."""
    return {
        b["bucket_id"]: b
        for b in buckets
        if b["source_domain"] == SOURCE_DOMAIN
        and b["bucket_id"] == bucket_id(b["policy_hash"])
        and (covered_ids is None or b["bucket_id"] in covered_ids)
    }


def chain_rules():
    """The exit rules the chain reports (`exit_rules`), read like the redemption itself:
    the first release's (revision 8 only) when the report states none."""
    return exit_rules.of_route(rpc_any("vault_bridge_route", asset_id=SERIES))
CONFIG = Path(__file__).parent / "fixtures/exit-setup.json"


def marker(burn):
    return pda(VERIFIER, b"egress-burn", hashlib.sha256(bytes.fromhex(burn)).digest())


def validate_burn(row, burn, buckets):
    require(
        row["burn_tx_id"] == burn
        and row["bucket_id"] in buckets
        and row["amount_atoms"] > 0,
        "Burn is outside the supported Solana series.",
    )
    destination = row["destination_ref"]
    require(destination.startswith(PREFIX), "Burn has a different destination network.")
    key = Pubkey.from_bytes(bytes.fromhex(destination[len(PREFIX) :]))
    require(
        destination == PREFIX + bytes(key).hex(), "Burn destination is not canonical."
    )
    require(key.is_on_curve(), "Burn destination owner is not a wallet key.")
    return key


NOT_WALLET = "refused: destination not a wallet"


class DestinationNotWallet(ValueError):
    """The destination owner cannot receive a payout; the burn is not retried."""


def destination_refusal(c, owner):
    """Why `owner` cannot receive a payout, or None. Wallets and the payout service
    run this same check: the chain cannot read Solana accounts."""
    if not owner.is_on_curve():
        return NOT_WALLET + " (key is not an Ed25519 wallet key)"
    account = c.account(owner)
    if account is not None and (
        account["owner"] != str(SYSTEM) or account["executable"]
    ):
        return NOT_WALLET + " (account belongs to a program)"
    return None


def check_owner(c, owner):
    """Refuse an existing owner that is not a plain wallet before any proof work."""
    reason = destination_refusal(c, owner)
    if reason is not None:
        raise DestinationNotWallet(reason)


def check_verifier(c):
    require(
        profile_code(c, VERIFIER) == verifier_pin(),
        "Deployed Solana verifier differs from the pinned rebuilt verifier.",
    )


def cluster():
    c = ExitCluster(FEE_PAYER)
    check_payout_authorities(c)
    # Before any proof time or SOL: only the pinned, rebuilt custody-route verifier.
    check_verifier(c)
    c.unlock(Path(os.getenv("EXIT_FEE_KEY", DIRECTORY / "exit-fee-payer.json")))
    return c


def balance(c, debit=0):
    value = c.call("getBalance", str(c.owner), c.level)["value"]
    require(value >= FLOOR + debit, "Relayer is below the SOL balance floor.")
    return value


def funded_send(c, label, instructions, *signers, rent=0, before_send=None):
    recent = c.call("getLatestBlockhash", c.level)["value"]
    block = Hash.from_string(recent["blockhash"])
    message = Message.new_with_blockhash(instructions, c.owner, block)
    fee = c.call(
        "getFeeForMessage", base64.b64encode(bytes(message)).decode(), c.level
    )["value"]
    require(isinstance(fee, int), "Solana fee quote unavailable.")
    balance(c, rent + fee)
    if before_send is None:
        require(label != "exit.withdrawal", "Withdrawal journal is required.")
    if label in {"exit.buffer", "exit.upload", "exit.checkpoint", "exit.withdrawal"}:
        check_custody(c)
        # Again just before sending: an upgrade during proving must not be paid into.
        check_verifier(c)
    tx = Transaction([c.payer, *signers], message, block)
    sig = str(tx.signatures[0])
    if before_send is not None:
        before_send(sig, recent["lastValidBlockHeight"])
    result = c.call(
        "sendTransaction",
        base64.b64encode(bytes(tx)).decode(),
        dict(encoding="base64", preflightCommitment="finalized", maxRetries=5),
    )
    require(result == sig, "Solana signature mismatch.")
    c.confirm([(label, sig)])
    return sig


def light(c):
    account = c.account(LIGHT)
    expected = expected_light(read(CONFIG))
    require(
        account
        and account["owner"] == str(VERIFIER)
        and len(account["data"]) == len(expected)
        and account["data"][:232] == expected[:232]
        and account["data"][-1:] == b"\0",
        "Solana verifier is uninitialized, paused or has a different route.",
    )
    return int.from_bytes(account["data"][-9:-1], "little"), account["data"][
        -57:-9
    ].hex()


def shared_liabilities(buckets, domain):
    # Count each bucket once. Queues include paid-but-unsettled burns until L1 retires
    # them; `check_custody` subtracts those whose finalized marker shows the payout.
    total = 0
    seen = set()
    for bucket in buckets:
        if bucket["source_domain"] != domain:
            continue
        if "bucket_id" in bucket:
            require(bucket["bucket_id"] not in seen, "Duplicate custody bucket.")
            seen.add(bucket["bucket_id"])
        allocations = sum(
            bucket[k]
            for k in (
                "outstanding_vault_bridge_atoms",
                "redemption_queue_atoms",
                "other_allocations_atoms",
                "nav_subscription_allocations_atoms",
            )
        )
        total += max(allocations, bucket["counted_value_atoms"])
    return total


def proof_count(latest, burn_height, finalized):
    count = 1
    while burn_height - latest > 65:
        target = select_target(latest, fetch_any, end=min(latest + 65, finalized - 1))
        require(
            latest < target["height"] <= latest + 65, "Invalid checkpoint selection."
        )
        latest = target["height"]
        count += 1
    return count


def payout_budget(c, proofs):
    block = Hash.from_string(
        c.call("getLatestBlockhash", c.level)["value"]["blockhash"]
    )
    msg = Message.new_with_blockhash(
        [Instruction(VERIFIER, b"", [ro(MINT, True)])], c.owner, block
    )
    fee = c.call("getFeeForMessage", base64.b64encode(bytes(msg)).decode(), c.level)[
        "value"
    ]
    require(isinstance(fee, int) and fee > 0, "Solana fee quote unavailable.")
    # Allocate, upload up to three chunks, consume and reclaim. Charge two signatures throughout.
    return proofs * (6 * fee + c.rent(48)) + c.rent(2048) + c.rent(1) + c.rent(165)


class FinalizedPath:
    def __init__(self):
        self.cursor = None
        self.members = None

    def check(self, head):
        if self.cursor is None:
            cfg = read(CONFIG)
            cert = read(CONFIG.with_name("exit-checkpoint.json"))
            witness = read(Path(os.environ["EXIT_ANCHOR_WITNESS"]))
            h = witness["block"]["header"]
            require(
                h["block_hash"] == cfg["initial_checkpoint"]
                and committee(h) == cfg["initial_committee"],
                "Wrong anchor witness.",
            )
            from exit_precheck import execute_guest

            with tempfile.TemporaryDirectory() as tmp:
                anchor = execute_guest(witness, Path(tmp))
            require(
                (anchor["height"], anchor["result"], anchor["end"])
                == (
                    cfg["initial_height"],
                    cfg["initial_checkpoint"],
                    cfg["initial_committee"],
                ),
                "Frozen checker disagrees with the trusted anchor.",
            )
            self.cursor = dict(
                cfg,
                height=cfg["initial_height"],
                block=cfg["initial_checkpoint"],
                root=cert["checkpoint_state_root"],
                committee=cfg["initial_committee"],
            )
            self.members = witness["committee"]
        with tempfile.TemporaryDirectory() as tmp:
            path = qualify_path(self.members, self.cursor, head, Path(tmp), fetch=fetch_any)
        self.cursor, self.members = path["cursor"], path["members"]
        return path["qualified_parent"]


FINALIZED_PATH = FinalizedPath()


def check_light_path(c, height, block, tip):
    require(
        read(CONFIG)["initial_height"] <= height <= tip,
        "Light checkpoint outside finalized path.",
    )
    count = 1 if height == tip else 2
    page = rpc_any("blocks", from_height=height, limit=count)
    require(len(page) == count, "Missing light checkpoint header.")
    h = page[0]["header"]
    account = c.account(checkpoint_pda(block))
    require(
        (h["block_hash"], h["height"]) == (block, height)
        and account
        and not account["executable"]
        and account["owner"] == str(VERIFIER)
        and account["data"].hex() == committee(h),
        "Light checkpoint differs from finalized path.",
    )
    if height < tip:
        next_h = page[1]["header"]
        require(
            next_h["height"] == height + 1
            and next_h["parent_hash"] == block
            and committee(next_h) == committee(h),
            "Light checkpoint cannot verify its successor.",
        )


def paid_unsettled(c, redemptions, covered):
    """Atoms of pending custody burns whose one-use marker is finalized on Solana.
    That USDC has left the custody; until settlement retires the claim on Post Fiat
    it must not be counted again, or every later payout waits for settlement."""
    pending = [
        r
        for r in redemptions
        if r["bucket_id"] in covered and r["state"] == "pending" and r["settled_atoms"] == 0
    ]
    paid = 0
    for start in range(0, len(pending), 100):
        page = pending[start : start + 100]
        markers = c.call(
            "getMultipleAccounts",
            [str(marker(r["burn_tx_id"])) for r in page],
            dict(c.level, encoding="base64"),
        )["value"]
        # Only a paid marker (byte 1) means the USDC left the custody; a cancelled one
        # (byte 2) leaves it there for the refund on Post Fiat.
        paid += sum(
            r["amount_atoms"]
            for r, account in zip(page, markers)
            if account is not None
            and account["owner"] == str(VERIFIER)
            and base64.b64decode(account["data"][0] if isinstance(account["data"], list) else account["data"])
            == b"\x01"
        )
    return paid


def check_custody(c, covered_ids=None):
    s = rpc_any("vault_bridge_status", asset_id=ASSET)
    covered = covered_buckets(s["buckets"], covered_ids)
    require(
        covered
        and all(
            b["status"] == "active"
            and b["redeemable_at_par"] is True
            and b["impairment_factor_bps"] == 10000
            and b["counted_value_atoms"]
            >= b["outstanding_vault_bridge_atoms"] + b["redemption_queue_atoms"]
            for b in covered.values()
            if b["outstanding_vault_bridge_atoms"] + b["redemption_queue_atoms"] > 0
        ),
        "Solana source bucket is unavailable.",
    )
    required_atoms = shared_liabilities(s["buckets"], SOURCE_DOMAIN) - paid_unsettled(
        c, s.get("redemptions", []), covered
    )
    snapshot = inspect_vault(c)
    require(
        snapshot["custody_atoms"] >= required_atoms,
        "Shared custody is insufficient.",
    )
    return s, covered, required_atoms


def governed_minimum(routes):
    """The governed minimum Solana exit burn, or None under the first release, which has
    none; every given validator must report the same rules (readiness passes its agreeing
    group, so a lagging one does not block)."""
    try:
        return exit_rules.agreed(routes)["minimum"]
    except ValueError as error:
        raise ValueError("Validators do not agree on the Solana minimum withdrawal.") from error


def holdings(account, buckets=None, minimum=1):
    """The Solana-source series an account may offer, each with its redeemable amount.
    `buckets` is the readiness snapshot (bucket_id, series_asset_id, outstanding_atoms);
    only without one is the full status read."""
    require(ACCOUNT.fullmatch(account or "") is not None, "Invalid account.")
    if buckets is None:
        buckets = [
            dict(
                bucket_id=b["bucket_id"],
                series_asset_id=b["source_series_id"],
                outstanding_atoms=str(b["outstanding_vault_bridge_atoms"]),
            )
            for b in covered_buckets(
                rpc_any("vault_bridge_status", asset_id=ASSET)["buckets"]
            ).values()
        ]
    offer = []
    for b in buckets:
        lines = rpc_any(
            "account_assets", account=account, asset_id=b["series_asset_id"], limit=1
        )["assets"]
        if not lines or not lines[0]["authorized"] or lines[0]["frozen"]:
            continue
        held = int(lines[0]["balance"])
        redeemable = min(held, int(b["outstanding_atoms"]))
        # A series below the governed minimum cannot be burned; do not offer it.
        if held > 0 and redeemable >= minimum:
            offer.append(
                dict(
                    bucket_id=b["bucket_id"],
                    series_asset_id=b["series_asset_id"],
                    balance_atoms=str(held),
                    redeemable_atoms=str(redeemable),
                )
            )
    return offer


def reconcile_supply(routes, outstanding, need=6, bucket=None):
    """Every given route of `bucket`'s series (revision 8 by default) reports its series
    supply equal to its bucket outstanding and to `outstanding`. Open qualification
    passes all six (five for a re-open); readiness passes its agreeing group of five.
    `revision` is the descriptor epoch, 8 for every covered bucket."""
    bucket = bucket or dict(bucket_id=BUCKET, policy_hash=POLICY)
    require(len(routes) >= need, "Missing validator supply report.")
    require(bucket["bucket_id"] == bucket_id(bucket["policy_hash"]),
            "Bucket is not a covered bucket of the shared custody.")
    for route in routes:
        backing = route.get("source_backing") or {}
        supply = backing.get("series_supply_atoms")
        reported = backing.get("outstanding_vault_bridge_atoms")
        require(
            (backing.get("revision"), backing.get("bucket_id"), backing.get("policy_hash"))
            == (8, bucket["bucket_id"], bucket["policy_hash"])
            and type(supply) is int and type(reported) is int
            and type(outstanding) is int and 0 <= supply == reported == outstanding,
            "Series supply and bucket outstanding must reconcile on every reporting validator.",
        )
    return outstanding


def series_routes(series, need, height):
    """Route reports of one series at `height` from at least `need` validators."""
    rows = survey(lambda i: rpc(i, "vault_bridge_route", asset_id=series), need)
    return [r for r in rows.values() if r.get("current_height") == height]


def reconcile_covered(covered, routes, need, height):
    """Series supply of every covered bucket, reconciled like revision 8: revision 8 on the
    given routes, every other bucket on the route reports of its own series."""
    supplies = {}
    for key, b in sorted(covered.items()):
        rows = routes if key == BUCKET else series_routes(b["source_series_id"], need, height)
        supplies[key] = reconcile_supply(rows, b["outstanding_vault_bridge_atoms"], need, b)
    return supplies


def readiness():
    message = "Waiting for the governed Solana descriptor on five of six validators."
    try:
        head = fleet(READ_QUORUM)
        # Readiness only feeds the wallet; five agreeing validators suffice, so one
        # lost validator does not stop withdrawals.
        rows = survey(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES), READ_QUORUM)
        # The largest group of five that agree; a lagging validator is the one left out.
        routes = [
            rows[i]
            for i in agreeing(
                rows,
                lambda r: json.dumps(
                    [r.get(k) for k in ("active", "profile_hash", "current_height",
                                        "burns_open", "min_withdrawal_atoms", "source_backing",
                                        "covered_bucket_ids", "decision_signatures")]
                    + [r.get("profile", {}).get("route_epoch")],
                    sort_keys=True,
                ),
                READ_QUORUM,
                message,
            )
        ]
        require(
            all(
                r["active"]
                and r["profile_hash"] == EXIT
                and r["profile"]["route_epoch"] == 8
                for r in routes
            ),
            "Waiting for the Solana series exit on five of six validators.",
        )
        minimum = governed_minimum(routes)
        rules = exit_rules.agreed(routes)
        message = "Solana verifier or relayer unavailable."
        c = cluster()
        latest, anchor = light(c)
        message = "Relayer cannot fund the payout above its balance floor."
        tip = head["block_height"]
        require(
            all(r["current_height"] == tip for r in routes),
            "Descriptor and tip differ.",
        )
        budget = payout_budget(c, proof_count(latest, tip + 1, tip))
        lamports = balance(c, budget)
        message = "Withdrawal prover unavailable."
        available()
        message = "Vault custody or shared-bucket accounting unavailable."
        s, covered, required_atoms = check_custody(c, rules["covered"])
        message = "Revision-8 supply or bucket outstanding differs at the qualified parent."
        r8 = covered.get(BUCKET)
        require(r8 is not None, "Revision-8 bucket missing from the status report.")
        message = "Series supply or bucket outstanding differs at the qualified parent."
        supplies = reconcile_covered(covered, routes, READ_QUORUM, tip)
        supply = supplies[BUCKET]
        burns_open = routes[0].get("burns_open")
        require(type(burns_open) is bool and all(r.get("burns_open") is burns_open for r in routes),
                "Mixed burn admission status.")
        message = "Waiting for a compatible finalized committee path."
        qualified = FINALIZED_PATH.check(head)
        check_light_path(c, latest, anchor, tip)
        require(qualified == parent_of(fleet(READ_QUORUM)), "Tip advanced during qualification.")
        return dict(
            ok=True,
            ready=burns_open,
            payout_ready=True,
            accepting_jobs=True,
            qualified_parent=qualified,
            message=None,
            burns_open=burns_open,
            required_custody_atoms=str(required_atoms),
            payout_budget_lamports=budget,
            asset_id=ASSET,
            source_domain=SOURCE_DOMAIN,
            # None under the first release, which has no minimum.
            min_withdrawal_atoms=None if minimum is None else str(minimum),
            exit_release=rules["release"],
            buckets=[
                dict(
                    bucket_id=b["bucket_id"],
                    series_asset_id=b["source_series_id"],
                    outstanding_atoms=str(b["outstanding_vault_bridge_atoms"]),
                    series_supply_atoms=str(supplies[b["bucket_id"]]),
                )
                for b in covered.values()
            ],
            issuer=s["issuer"],
            epoch=s["finalized_epoch"],
            reserve_packet_hash=s["finalized_reserve_packet_hash"],
            series_supply_atoms=str(supply),
            bucket_outstanding_atoms=str(r8["outstanding_vault_bridge_atoms"]),
            current_height=routes[0]["current_height"],
            destination_prefix=PREFIX,
            vault_address=str(VAULT),
            relayer_lamports=lamports,
            relayer_floor_lamports=FLOOR,
        )
    except Exception:
        return dict(ok=True, ready=False, payout_ready=False, message=message)


PAID_NO_HISTORY = object()


CANCELLED = "cancelled: refund on Post Fiat"


class BurnCancelled(ValueError):
    """The burn's marker on Solana reads cancelled: it can never be paid."""

    def __init__(self):
        super().__init__(CANCELLED)


class UnknownBurn(ValueError):
    """No finalized L1 redemption exists."""


class CheckpointAdvanced(Exception):
    """A verified checkpoint completed this scheduling turn."""


def matches_payout(c, key, sig):
    import base58

    tx = c.call(
        "getTransaction",
        sig,
        dict(c.level, encoding="json", maxSupportedTransactionVersion=0),
    )
    if (
        not tx
        or tx["meta"]["err"] is not None
        or tx["transaction"]["signatures"][0] != sig
    ):
        return False
    msg = tx["transaction"]["message"]
    keys = msg["accountKeys"]
    return any(
        keys[ix["programIdIndex"]] == str(VERIFIER)
        and len(ix["accounts"]) >= 7
        and keys[ix["accounts"][6]] == str(key)
        and base58.b58decode(ix["data"])[:1] == b"\x02"
        for ix in msg["instructions"]
    )


REFUND_KIND = "solana_exit_refund:"


def cancel_slot(c, key, sig):
    """The finalized slot of `sig` if it is a successful verifier cancel (tag 10) of `key`."""
    import base58

    tx = c.call(
        "getTransaction",
        sig,
        dict(c.level, encoding="json", maxSupportedTransactionVersion=0),
    )
    if not tx or tx["meta"]["err"] is not None or tx["transaction"]["signatures"][0] != sig:
        return None
    msg = tx["transaction"]["message"]
    keys = msg["accountKeys"]
    found = any(
        keys[ix["programIdIndex"]] == str(VERIFIER)
        and len(ix["accounts"]) >= 7
        and keys[ix["accounts"][6]] == str(key)
        and base58.b58decode(ix["data"])[:1] == b"\x0a"
        for ix in msg["instructions"]
    )
    return tx["slot"] if found else None


def refund_kind(c, red, *, history_limit=100):
    """The refund decision for a redemption whose marker reads cancelled at the cluster's
    (finalized) commitment, bound to the cancel transaction. Anything else is refused."""
    import base58

    require(
        c.level.get("commitment") == "finalized",
        "Refund reads need a cluster set to finalized; refund refused.",
    )
    key = marker(red["burn_tx_id"])
    account = c.account(key)
    require(
        account is not None
        and account["owner"] == str(VERIFIER)
        and account["data"] == b"\x02",
        "No cancelled marker for this burn; refund refused.",
    )
    for row in c.call("getSignaturesForAddress", str(key), dict(c.level, limit=history_limit)):
        if row["err"] is None:
            slot = cancel_slot(c, key, row["signature"])
            if slot:
                sig = base58.b58decode(row["signature"]).hex()
                return f"{REFUND_KIND}{red['withdrawal_packet_hash']}:{slot}:{sig}"
    raise ValueError("No finalized cancel transaction for this burn; refund refused.")


def signature(c, burn, known=None, *, history_limit=100):
    key = marker(burn)
    account = c.account(key)
    if not account or (account["owner"] == str(SYSTEM) and not account["data"]):
        return None
    if account["owner"] == str(VERIFIER) and account["data"] == b"\x02":
        raise BurnCancelled()
    require(
        account["owner"] == str(VERIFIER) and account["data"] == b"\x01",
        "Unexpected burn marker.",
    )
    # A verified permanent marker is authoritative even when RPC history is pruned.
    try:
        if known and matches_payout(c, key, known):
            return known
        history = c.call(
            "getSignaturesForAddress", str(key), dict(c.level, limit=history_limit)
        )
        for row in history[:history_limit]:
            if row["err"] is None and matches_payout(c, key, row["signature"]):
                return row["signature"]
    except (RuntimeError, OSError, KeyError, ValueError, TypeError):
        pass
    return PAID_NO_HISTORY


def finish_payment(update, result):
    update(
        "paid",
        solana_tx=None if result is PAID_NO_HISTORY else result,
        detail="Finalized marker. Signature unavailable."
        if result is PAID_NO_HISTORY
        else None,
        error=None,
    )


def prune_work(c, work, reclaim=False):
    if not work.exists():
        return
    for path in work.iterdir():
        if path.is_symlink():
            continue
        if re.fullmatch("[a-f0-9]{64}", path.name) and path.is_dir():
            shutil.rmtree(path)
        elif re.fullmatch(r"[a-f0-9]{64}\.key", path.name):
            signer = load(path)
            account = c.account(signer.pubkey())
            if reclaim and account and account["owner"] == str(VERIFIER):
                funded_send(
                    c,
                    "exit.reclaim",
                    [
                        Instruction(
                            VERIFIER, b"\x06", [rw(signer.pubkey(), True), rw(c.owner)]
                        )
                    ],
                    signer,
                )
                account = c.account(signer.pubkey())
            if account is None:
                path.unlink()
    if not any(work.iterdir()):
        work.rmdir()


def verify_endpoints(c, witness, prior, target):
    start = c.account(checkpoint_pda(prior))
    require(start and start["owner"] == str(VERIFIER), "Starting checkpoint missing.")
    validate_segment(witness, prior, target, start["data"].hex())
    result = c.account(checkpoint_pda(target["block_hash"]))
    require(
        not result
        or (result["owner"] == str(SYSTEM) and not result["data"])
        or (
            result["owner"] == str(VERIFIER)
            and result["data"].hex() == committee(target)
        ),
        "Conflicting result checkpoint.",
    )


def upload(c, vals, work):
    # Each distinct witness owns one restart-safe upload buffer.
    path = work / (hashlib.sha256(vals).hexdigest() + ".key")
    if not path.exists():
        from exit_keys import create_key

        create_key(path)
    signer = load(path)
    buf = signer.pubkey()
    account = c.account(buf)
    if account is None:
        alloc = create_account(
            dict(
                from_pubkey=c.owner,
                to_pubkey=buf,
                lamports=c.rent(len(vals)),
                space=len(vals),
                owner=VERIFIER,
            )
        )
        funded_send(c, "exit.buffer", [alloc], signer, rent=c.rent(len(vals)))
        existing = bytes(len(vals))
    else:
        require(
            account["owner"] == str(VERIFIER)
            and not account["executable"]
            and len(account["data"]) == len(vals),
            "Unexpected proof buffer owner or layout.",
        )
        existing = account["data"]
    for off in range(0, len(vals), 700):
        chunk = vals[off : off + 700]
        if existing[off : off + len(chunk)] == chunk:
            continue
        funded_send(
            c,
            "exit.upload",
            [
                Instruction(
                    VERIFIER,
                    b"\x05" + off.to_bytes(2, "little") + chunk,
                    [rw(buf, True)],
                )
            ],
            signer,
        )
    return signer


def check_work_capacity(work):
    if not work.exists():
        return
    count = sum(1 for p in work.iterdir() if p.is_dir())
    keys = sum(1 for p in work.glob("*.key"))
    size = sum(p.stat().st_size for p in work.rglob("*") if p.is_file())
    require(
        count < 4 and keys < 4 and size < 96 * 1024 * 1024,
        "Burn artifact capacity reached.",
    )


def rent_shortfall(c, account, size, owner):
    if account is None:
        return c.rent(size)
    require(not account["executable"], "Unexpected executable account.")
    if account["owner"] == str(SYSTEM) and not account["data"]:
        return max(0, c.rent(size) - account["lamports"])
    require(
        account["owner"] == str(owner) and len(account["data"]) == size,
        "Unexpected allocated account.",
    )
    return 0


def finalized_reclaim(c, buffer):
    import base58

    history = c.call("getSignaturesForAddress", str(buffer), dict(c.level, limit=1))
    if not history or history[0]["err"] is not None:
        return False
    sig = history[0]["signature"]
    tx = c.call(
        "getTransaction",
        sig,
        dict(c.level, encoding="json", maxSupportedTransactionVersion=0),
    )
    if (
        not tx
        or tx["meta"]["err"] is not None
        or tx["transaction"]["signatures"][0] != sig
    ):
        return False
    msg = tx["transaction"]["message"]
    keys = msg["accountKeys"]
    return any(
        keys[ix["programIdIndex"]] == str(VERIFIER)
        and [keys[i] for i in ix["accounts"]] == [str(buffer), str(c.owner)]
        and base58.b58decode(ix["data"]) == b"\x06"
        for ix in msg["instructions"]
    )


def reclaim_completed(c, work):
    from exit_precheck import checkpoint_values

    def completed(raw):
        try:
            values = checkpoint_values(raw)
        except ValueError:
            return False
        result = c.account(checkpoint_pda(values["result"]))
        return (
            result
            and result["owner"] == str(VERIFIER)
            and result["data"].hex() == values["end"]
        )

    for cache in work.glob("[a-f0-9]" * 64):
        native = cache / "native-public-values.bin"
        if (
            cache.is_dir()
            and not cache.is_symlink()
            and native.is_file()
            and not native.is_symlink()
            and native.stat().st_size <= 2048
            and completed(native.read_bytes())
        ):
            shutil.rmtree(cache)
    for path in work.glob("*.key"):
        signer = load(path)
        account = c.account(signer.pubkey())
        if account is None:
            # An allocation may still be in flight. Only a finalized close proves this key retired.
            if finalized_reclaim(c, signer.pubkey()):
                path.unlink()
            continue
        if account["owner"] != str(VERIFIER):
            continue
        raw = account["data"]
        if hashlib.sha256(raw).hexdigest() != path.stem:
            continue
        if not completed(raw):
            continue
        funded_send(
            c,
            "exit.reclaim",
            [Instruction(VERIFIER, b"\x06", [rw(signer.pubkey(), True), rw(c.owner)])],
            signer,
        )
        if c.account(signer.pubkey()) is None:
            path.unlink()


def submit(c, kind, witness, work, owner=None, on_pay=None, before_send=None):
    check_custody(c)
    if owner is not None:
        ata = pda(ATA, bytes(owner), bytes(TOKEN), bytes(MINT))
        recipient_atoms(ata, c.account(ata), owner)
    reclaim_completed(c, work)
    check_work_capacity(work)
    vals, proof = prove(kind, witness, work)
    require(0 < len(vals) <= 2048 and len(proof) == 356, "Invalid proof size.")
    balance(c)
    check_custody(c)
    if on_pay:
        on_pay()
    signer = upload(c, vals, work)
    buf = signer.pubkey()
    prior = checkpoint_pda(witness["prior_checkpoint_block_id"])
    result = checkpoint_pda(witness["block"]["header"]["block_hash"])
    accounts = [
        rw(c.owner, True),
        rw(LIGHT),
        ro(prior),
        rw(result),
        ro(buf),
        ro(SYSTEM),
    ]
    rent = rent_shortfall(c, c.account(result), 48, VERIFIER)
    ixs = [set_compute_unit_limit(400_000)]
    if owner is not None:
        destination = c.account(ata)
        recipient_atoms(ata, destination, owner)
        burn_marker = marker(witness["receipt"]["tx_id"])
        rent += rent_shortfall(c, c.account(burn_marker), 1, VERIFIER)
        rent += rent_shortfall(c, destination, 165, TOKEN)
        ixs.append(
            Instruction(
                ATA,
                b"\x01",
                [
                    rw(c.owner, True),
                    rw(ata),
                    ro(owner),
                    ro(MINT),
                    ro(SYSTEM),
                    ro(TOKEN),
                ],
            )
        )
        accounts.extend(
            [
                rw(burn_marker),
                ro(VAULT),
                ro(pda(VERIFIER, b"egress-release", bytes(VAULT))),
                ro(pda(VAULT, b"config")),
                rw(pda(VAULT, b"custody")),
                rw(ata),
                ro(owner),
                ro(MINT),
                ro(TOKEN),
            ]
        )
    ixs.append(
        Instruction(VERIFIER, bytes([2 if owner is not None else 8]) + proof, accounts)
    )
    sig = funded_send(c, "exit." + kind, ixs, rent=rent, before_send=before_send)
    try:
        funded_send(
            c,
            "exit.reclaim",
            [Instruction(VERIFIER, b"\x06", [rw(buf, True), rw(c.owner)])],
            signer,
        )
    except (ValueError, RuntimeError, OSError):
        # The payout or checkpoint is finalized. Preserve the buffer key for rent recovery.
        logging.warning("Buffer rent recovery pending.")
    return sig


def compatible_prior(headers, states):
    require(len(headers) == len(states) + 1, "Incomplete historical checkpoint lookup.")
    for h, next_h, account in reversed(list(zip(headers, headers[1:], states))):
        require(
            next_h["parent_hash"] == h["block_hash"]
            and next_h["height"] == h["height"] + 1,
            "Discontinuous historical headers.",
        )
        if (
            account
            and account["owner"] == str(VERIFIER)
            and base64.b64decode(account["data"][0], validate=True).hex()
            == committee(next_h)
        ):
            return h["block_hash"]
    raise ValueError("No compatible checkpoint precedes this burn.")


def bind_witness(w, red, burn):
    """The proof witness is this burn's own packet, from its own bucket."""
    require(
        w["receipt"]["tx_id"] == burn
        and w["receipt"]["accepted"]
        and w["route_profile"]["profile_hash"] == EXIT
        and w["withdrawal_packet_hash"] == red["withdrawal_packet_hash"]
        and w["withdrawal_packet"]["burn_tx_id"] == burn
        and w["withdrawal_packet"]["source_bucket_id"] == red["bucket_id"],
        "Witness does not match this burn.",
    )


def egress_witness(c, red, prior, h, work):
    """Assemble the withdrawal witness from replies that fit the node's 4 MB frame.

    The node sends nothing when a reply exceeds its frame limit, and one block of
    finality ancestry is about 200 KB. Ask the node for the burn block alone, then
    rebuild the ancestry from the prior checkpoint in eight-block pages. The result
    equals the node's own single reply. The frozen guest runs on it before proving.
    """
    target = rpc_any("blocks", from_height=h, limit=1)[0]["header"]
    require(target["height"] == h, "Unexpected burn height.")
    parent = target["parent_hash"]
    w = rpc_any(
        "pfusdc_egress_witness",
        withdrawal_id=red["redemption_id"],
        prior_checkpoint=parent,
    )
    require(
        w["prior_checkpoint_block_id"] == parent
        and not w.get("finality_ancestry")
        and w["block"]["header"] == target,
        "Unexpected single-block witness.",
    )
    if prior == parent:
        return w
    blocks = ancestry_pages(max(1, h - 65), h)
    first = next(
        (i for i, b in enumerate(blocks) if b["header"]["parent_hash"] == prior), None
    )
    require(first is not None, "Prior checkpoint outside the guest ancestry bound.")
    blocks = blocks[first:]
    last = blocks[-1]["header"]
    require(last["block_hash"] == parent, "Ancestry does not reach the burn block.")
    if committee(last) != committee(target):
        raise CommitteeRotated()
    cache = work.parent / "committee.json"
    saved = read(cache) if cache.exists() else {}
    members = saved.get(c.account(checkpoint_pda(prior))["data"].hex())
    if members is None:
        members = [dict(row, active=True) for row in rpc_any("validators")["validators"]]
    segment = public_checkpoint(
        fetch_any, blocks[0]["header"]["height"] - 1, prior, last, members, read(CONFIG)
    )
    steps = list(segment.get("finality_ancestry", []))
    steps.append(
        dict(
            block=segment["block"],
            committee=segment["committee"],
            committee_epoch=segment["committee_epoch"],
        )
    )
    return dict(w, prior_checkpoint_block_id=prior, finality_ancestry=steps)


def process(burn, work, update, *, attempt=None, before_send=None):
    update("reading")
    c = cluster()
    red, status = find_redemption(burn)
    if red.get("state") == "refunded":
        # Refunded on Post Fiat after a cancel on Solana: nothing left to pay.
        raise BurnCancelled()
    owner = validate_burn(red, burn, covered_buckets(status["buckets"], chain_rules()["covered"]))
    check_owner(c, owner)
    h = red["created_at_height"]
    update(
        "reading",
        amount_atoms=str(red["amount_atoms"]),
        recipient=str(owner),
        burn_height=h,
    )
    paid = signature(c, burn, attempt[0] if attempt else None)
    if paid:
        finish_payment(update, paid)
        prune_work(c, work)
        return
    if attempt:
        status = c.call(
            "getSignatureStatuses", [attempt[0]], {"searchTransactionHistory": True}
        )["value"][0]
        failed = (
            status
            and status.get("confirmationStatus") == "finalized"
            and status["err"] is not None
        )
        if not failed:
            require(
                not status and c.call("getBlockHeight", c.level) > attempt[1],
                "Prior payout confirmation pending.",
            )
    balance(c)
    latest, prior = light(c)
    if latest >= h:
        start = max(1, h - 65)
        headers = []
        for first in range(start, h + 1, 8):
            page = rpc_any("blocks", from_height=first, limit=min(8, h - first + 1))
            headers.extend(b["header"] for b in page)
        require(
            headers and headers[0]["height"] == start and headers[-1]["height"] == h,
            "Missing historical headers.",
        )
        states = c.call(
            "getMultipleAccounts",
            [str(checkpoint_pda(v["block_hash"])) for v in headers[:-1]],
            dict(c.level, encoding="base64"),
        )["value"]
        prior = compatible_prior(headers, states)
    update("proving")
    require(
        work.exists()
        or sum(1 for path in work.parent.glob("[0-9a-f]" * 96) if path.is_dir()) < 64,
        "Worker artifact capacity reached.",
    )
    work.mkdir(parents=True, exist_ok=True)
    if h - latest > 65:
        block = select_target(latest, fetch_any)
        cache = work.parent / "committee.json"
        from live import save

        try:
            w = rpc_any(
                "pfusdc_checkpoint_witness",
                prior_checkpoint=prior,
                target_block=block["block_hash"],
            )
        except RuntimeError as err:
            if "rpc_method_not_allowed" not in str(err):
                raise
            saved = read(cache) if cache.exists() else {}
            root = c.account(checkpoint_pda(prior))["data"].hex()
            members = saved.get(root)
            if members is None and os.getenv("EXIT_ANCHOR_WITNESS"):
                anchor = read(Path(os.environ["EXIT_ANCHOR_WITNESS"]))
                if committee(anchor["block"]["header"]) == root:
                    members = anchor["committee"]
            if members is None:
                members = [
                    dict(row, active=True) for row in rpc_any("validators")["validators"]
                ]
            w = public_checkpoint(fetch_any, latest, prior, block, members, read(CONFIG))
        first = (w.get("finality_ancestry") or [w])[0]
        verify_endpoints(c, w, prior, block)
        submit(c, "checkpoint", w, work)
        latest, prior = light(c)
        require(
            (latest, prior) == (block["height"], block["block_hash"]),
            "Checkpoint readback differs.",
        )
        # Only a checkpoint the verifier accepted feeds the committee cache.
        save(
            cache,
            {
                committee(first["block"]["header"]): first["committee"],
                committee(block): w["committee"],
            },
        )
        prune_work(c, work)
        raise CheckpointAdvanced()
    w = egress_witness(c, red, prior, h, work)
    bind_witness(w, red, burn)
    target = rpc_any("blocks", from_height=h, limit=1)[0]["header"]
    require(target["height"] == h, "Unexpected burn height.")
    verify_endpoints(c, w, prior, target)
    sig = signature(c, burn)
    if not sig:

        def paying():
            update("paying")

        sent = submit(
            c, "withdrawal", w, work, owner, on_pay=paying, before_send=before_send
        )
        sig = signature(c, burn, sent)
    require(sig is not None, "Waiting for finalized payout marker.")
    from live import save

    # A withdrawal can itself advance the light client across a registry update.
    if h >= latest:
        save(
            work.parent / "committee.json",
            {committee(w["block"]["header"]): w["committee"]},
        )
    finish_payment(update, sig)
    prune_work(c, work)
