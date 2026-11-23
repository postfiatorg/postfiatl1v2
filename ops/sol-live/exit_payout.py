"""Prove finalized Post Fiat burns and release to the owner's canonical USDC ATA."""

import base64
import hashlib
import logging
import os
from pathlib import Path
import re
import shutil
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
from exit_segments import (
    RotationScan,
    public_checkpoint,
    select_target,
    validate_segment,
    committee,
)
from exit_precheck import inspect_vault, token_atoms
from exit_proof import available, prove

ATA = pk("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
PREFIX = "solana-spl:45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0:"
FLOOR = 20_000_000
CONFIG = Path(__file__).parent / "fixtures/exit-setup.json"


def marker(burn):
    return pda(VERIFIER, b"egress-burn", hashlib.sha256(bytes.fromhex(burn)).digest())


def validate_burn(row, burn):
    require(
        row["burn_tx_id"] == burn
        and row["bucket_id"] == BUCKET
        and row["amount_atoms"] > 0,
        "Burn is outside the supported Solana series.",
    )
    destination = row["destination_ref"]
    require(destination.startswith(PREFIX), "Burn has a different destination network.")
    key = Pubkey.from_bytes(bytes.fromhex(destination[len(PREFIX) :]))
    require(
        destination == PREFIX + bytes(key).hex(), "Burn destination is not canonical."
    )
    return key


def cluster():
    c = ExitCluster(FEE_PAYER)
    check_payout_authorities(c)
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
        return c.send(label, instructions, *signers)
    tx = Transaction.new_signed_with_payer(
        instructions, c.owner, [c.payer, *signers], block
    )
    sig = str(tx.signatures[0])
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
    # Count each bucket once. Paid-but-unsettled queues remain reserved until L1 retires them.
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


def payout_budget(c, distance):
    # Even if committee changes shorten each checkpoint to one block, admission covers the full path.
    proofs = 1 + max(0, distance - 65)
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
    return proofs * (6 * fee + c.rent(48) + c.rent(2048)) + c.rent(1) + c.rent(165)


ROTATIONS = RotationScan()


def readiness():
    message = "Waiting for the governed Solana descriptor on six validators."
    try:
        routes = six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES))
        require(
            all(
                r["active"]
                and r["profile_hash"] == EXIT
                and r["profile"]["route_epoch"] == 8
                for r in routes
            ),
            "Waiting for the Solana series exit on all six validators.",
        )
        message = "Solana verifier or relayer unavailable."
        c = cluster()
        latest, anchor = light(c)
        message = "Relayer cannot fund the payout above its balance floor."
        budget = payout_budget(c, max(r["current_height"] for r in routes) + 1 - latest)
        lamports = balance(c, budget)
        message = "Withdrawal prover unavailable."
        available()
        message = "Vault custody or shared-bucket accounting unavailable."
        s = rpc(0, "vault_bridge_status", asset_id=ASSET)
        b = next((b for b in s["buckets"] if b["bucket_id"] == BUCKET), None)
        require(
            b is not None
            and b["policy_hash"] == POLICY
            and b["source_series_id"] == SERIES
            and b["status"] == "active"
            and b["redeemable_at_par"] is True
            and b["impairment_factor_bps"] == 10000
            and b["counted_value_atoms"]
            >= b["outstanding_vault_bridge_atoms"] + b["redemption_queue_atoms"],
            "Solana source bucket is unavailable.",
        )
        required_atoms = shared_liabilities(s["buckets"], b["source_domain"])
        snapshot = inspect_vault(c)
        require(
            snapshot["custody_atoms"] >= required_atoms,
            "Shared custody is insufficient.",
        )
        burns_open = all(r.get("burns_open") is True for r in routes)
        message = "Waiting for a compatible finalized committee path."
        compatible = ROTATIONS.check(
            rpc, latest, anchor, max(r["current_height"] for r in routes)
        )
        return dict(
            ok=True,
            ready=compatible and burns_open,
            payout_ready=compatible,
            accepting_jobs=True,
            message=None if compatible else message,
            burns_open=burns_open,
            required_custody_atoms=str(required_atoms),
            payout_budget_lamports=budget,
            asset_id=ASSET,
            series_asset_id=SERIES,
            bucket_id=BUCKET,
            issuer=s["issuer"],
            epoch=s["finalized_epoch"],
            reserve_packet_hash=s["finalized_reserve_packet_hash"],
            bucket_outstanding_atoms=str(b["outstanding_vault_bridge_atoms"]),
            current_height=routes[0]["current_height"],
            destination_prefix=PREFIX,
            vault_address=str(VAULT),
            relayer_lamports=lamports,
            relayer_floor_lamports=FLOOR,
        )
    except Exception:
        return dict(ok=True, ready=False, payout_ready=False, message=message)


PAID_NO_HISTORY = object()


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


def signature(c, burn, known=None):
    key = marker(burn)
    account = c.account(key)
    if not account or (account["owner"] == str(SYSTEM) and not account["data"]):
        return None
    require(
        account["owner"] == str(VERIFIER) and account["data"] == b"\x01",
        "Unexpected burn marker.",
    )
    # A verified permanent marker is authoritative even when RPC history is pruned.
    try:
        if known and matches_payout(c, key, known):
            return known
        history = c.call("getSignaturesForAddress", str(key), dict(c.level, limit=100))
        for row in history:
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
    if not c.account(buf):
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
    for off in range(0, len(vals), 700):
        funded_send(
            c,
            "exit.upload",
            [
                Instruction(
                    VERIFIER,
                    b"\x05" + off.to_bytes(2, "little") + vals[off : off + 700],
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


def submit(c, kind, witness, work, owner=None, on_pay=None, before_send=None):
    check_work_capacity(work)
    vals, proof = prove(kind, witness, work)
    require(0 < len(vals) <= 2048 and len(proof) == 356, "Invalid proof size.")
    balance(c)
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
    rent = 0 if c.account(result) else c.rent(48)
    ixs = [set_compute_unit_limit(400_000)]
    if owner is not None:
        ata = pda(ATA, bytes(owner), bytes(TOKEN), bytes(MINT))
        destination = c.account(ata)
        token_atoms(destination, owner, allow_empty=True)
        rent += c.rent(1) + (0 if destination else c.rent(165))
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
                rw(marker(witness["receipt"]["tx_id"])),
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


def process(burn, work, update, *, attempt=None, before_send=None):
    update("reading")
    c = cluster()
    red = next(
        (
            r
            for r in rpc(0, "vault_bridge_status", asset_id=ASSET)["redemptions"]
            if r["burn_tx_id"] == burn
        ),
        None,
    )
    if red is None:
        raise UnknownBurn("No finalized redemption for this burn.")
    owner = validate_burn(red, burn)
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
            page = rpc(0, "blocks", from_height=first, limit=min(8, h - first + 1))
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
        block = select_target(latest, rpc)
        cache = work.parent / "committee.json"
        from live import save

        try:
            w = rpc(
                0,
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
                    dict(row, active=True) for row in rpc(0, "validators")["validators"]
                ]
            w = public_checkpoint(rpc, latest, prior, block, members, read(CONFIG))
        first = (w.get("finality_ancestry") or [w])[0]
        save(
            cache,
            {
                committee(first["block"]["header"]): first["committee"],
                committee(block): w["committee"],
            },
        )
        verify_endpoints(c, w, prior, block)
        submit(c, "checkpoint", w, work)
        latest, prior = light(c)
        require(
            (latest, prior) == (block["height"], block["block_hash"]),
            "Checkpoint readback differs.",
        )
        prune_work(c, work)
        raise CheckpointAdvanced()
    w = rpc(
        0,
        "pfusdc_egress_witness",
        withdrawal_id=red["redemption_id"],
        prior_checkpoint=prior,
    )
    require(
        w["receipt"]["tx_id"] == burn
        and w["receipt"]["accepted"]
        and w["route_profile"]["profile_hash"] == EXIT
        and w["withdrawal_packet"]["source_bucket_id"] == BUCKET,
        "Witness does not match this burn.",
    )
    target = rpc(0, "blocks", from_height=h, limit=1)[0]["header"]
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
