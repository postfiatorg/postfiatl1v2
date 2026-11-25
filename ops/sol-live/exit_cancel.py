"""Prepare, verify and send a Solana exit cancel signed offline by the pause authority.

RUNBOOK-LIVE.md, "Cancel a burn with the pause key". The tool never holds or reads the pause
key: `prepare` writes the UNSIGNED transaction and a summary for the key's holder, `verify`
checks a signed transaction against that summary, and `send` (explicit) uploads the burn's
public values and sends it, only while the chain reports the second release.

  exit_cancel.py prepare --burn BURN --proof-dir WORK/<witness hash> --journal JOBS --out DIR
  exit_cancel.py verify  --out DIR --signed SIGNED.hex
  exit_cancel.py send    --out DIR --signed SIGNED.hex
"""

import argparse
import hashlib
import json
import sqlite3
from pathlib import Path

from solders.hash import Hash
from solders.message import Message
from solders.transaction import Transaction

from exit_payout import (
    LIGHT,
    SYSTEM,
    VERIFIER,
    chain_rules,
    checkpoint_pda,
    cluster,
    find_redemption,
    marker,
    require,
    upload,
)
from exit_review import canonical_owner, destination_hash, fields
from exit_setup import PAUSE, Instruction, ro, rw



def check_cancellable(red, marker_account, pending):
    """The runbook's conditions: a pending, unsettled burn, no marker, nothing in flight."""
    require(red["state"] == "pending" and red["settled_atoms"] == 0, "The burn is not pending and unsettled.")
    require(marker_account is None, "The burn's marker exists at finalized: it is paid or cancelled.")
    require(not pending, "A payout for this burn is in flight (paying, or an attempt still valid); "
                         "stop the worker and wait until the attempt's block height has passed.")


def pending_attempts(journal, burn, block_height):
    """The worker's job for this burn while paying, and any recorded payout attempt whose
    blockhash is still valid at Solana block height `block_height` (about 60 to 90 s)."""
    if journal is None or not Path(journal).exists():
        return []
    with sqlite3.connect(f"file:{journal}?mode=ro", uri=True) as db:
        db.row_factory = sqlite3.Row
        jobs = [dict(r) for r in db.execute("SELECT * FROM jobs WHERE burn_tx_id=? AND stage='paying'", (burn,))]
        attempts = [dict(r) for r in db.execute("SELECT * FROM attempts WHERE burn=? AND expiry>=?",
                                                (burn, block_height))]
    return jobs + attempts


def prepare(red, marker_account, pending, proof_dir, out, blockhash, last_valid_block_height):
    check_cancellable(red, marker_account, pending)
    burn = red["burn_tx_id"]
    owner = canonical_owner(red["destination_ref"])
    values = (proof_dir / "public-values.bin").read_bytes()
    proof = (proof_dir / "proof-calldata.bin").read_bytes()
    witness = json.loads((proof_dir / "witness.json").read_text())
    f = fields(values)
    require(f[23].hex() == burn and int.from_bytes(f[26], "big") == red["amount_atoms"]
            and f[28] == destination_hash(owner), "The proof files are for another burn.")
    require(len(proof) == 356, "Invalid proof size.")
    buffer_key = out / (hashlib.sha256(values).hexdigest() + ".key")
    out.mkdir(parents=True, exist_ok=True)
    (out / "public-values.bin").write_bytes(values)
    # The values buffer's address: the same restart-safe key file `upload` creates and uses.
    from exit_keys import create_key, load

    if not buffer_key.exists():
        create_key(buffer_key)
    buffer = load(buffer_key).pubkey()
    accounts = [rw(PAUSE, True), rw(LIGHT), ro(checkpoint_pda(witness["prior_checkpoint_block_id"])),
                rw(checkpoint_pda(witness["block"]["header"]["block_hash"])), ro(buffer), ro(SYSTEM),
                rw(marker(burn)), ro(PAUSE, True)]
    message = Message.new_with_blockhash([Instruction(VERIFIER, b"\x0a" + proof, accounts)], PAUSE, blockhash)
    unsigned = Transaction.new_unsigned(message)
    (out / "unsigned.hex").write_text(bytes(unsigned).hex())
    summary = dict(burn=burn, amount_atoms=red["amount_atoms"], destination_owner=str(owner),
                   marker=str(marker(burn)), program=str(VERIFIER), signer=str(PAUSE), buffer=str(buffer),
                   message_sha256=hashlib.sha256(bytes(message)).hexdigest(),
                   last_valid_block_height=last_valid_block_height)
    (out / "summary.json").write_text(json.dumps(summary, indent=1))
    (out / "summary.txt").write_text(
        "Solana exit cancel to sign with the pause key, offline\n"
        + "".join(f"  {k}: {v}\n" for k, v in summary.items())
        + "Sign before the block height above, then run `exit_cancel.py verify` and `send`.\n")
    return summary


def verify(out, signed):
    """The signed transaction is exactly the prepared message, signed by the pause key alone."""
    summary = json.loads((out / "summary.json").read_text())
    tx = Transaction.from_bytes(signed)
    require(hashlib.sha256(bytes(tx.message)).hexdigest() == summary["message_sha256"],
            "The signed transaction is not the prepared cancel.")
    require(tx.message.header.num_required_signatures == 1 and tx.message.account_keys[0] == PAUSE
            and str(PAUSE) == summary["signer"], "The cancel is not signed by the pause authority alone.")
    try:
        tx.verify()
    except Exception as error:
        raise ValueError(f"Invalid pause-key signature: {error}") from None
    return summary, tx


def require_second_release(rules):
    require(rules["release"] == 2, "The chain does not report the second release: no cancel is sent before it.")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="action", required=True)
    p = sub.add_parser("prepare")
    p.add_argument("--burn", required=True)
    p.add_argument("--proof-dir", required=True, type=Path)
    p.add_argument("--journal", type=Path)
    p.add_argument("--out", required=True, type=Path)
    for name in ("verify", "send"):
        s = sub.add_parser(name)
        s.add_argument("--out", required=True, type=Path)
        s.add_argument("--signed", required=True, type=Path)
    args = parser.parse_args()
    c = cluster()
    if args.action == "prepare":
        red, _ = find_redemption(args.burn)
        account = c.account(marker(args.burn))
        latest = c.call("getLatestBlockhash", c.level)["value"]
        height = c.call("getBlockHeight", c.level)
        summary = prepare(red, account, pending_attempts(args.journal, args.burn, height), args.proof_dir, args.out,
                          Hash.from_string(latest["blockhash"]), latest["lastValidBlockHeight"])
        print((args.out / "summary.txt").read_text())
        return summary
    summary, tx = verify(args.out, bytes.fromhex(args.signed.read_text().strip()))
    if args.action == "verify":
        print(f"verified: the pause authority signed the cancel of {summary['burn']}")
        return summary
    require_second_release(chain_rules())
    red, _ = find_redemption(summary["burn"])
    check_cancellable(red, c.account(marker(summary["burn"])), [])
    require(c.call("getBlockHeight", c.level) <= summary["last_valid_block_height"],
            "The signed cancel has expired; prepare and sign again.")
    signer = upload(c, (args.out / "public-values.bin").read_bytes(), args.out)
    require(str(signer.pubkey()) == summary["buffer"], "Unexpected values buffer.")
    sig = c.call("sendTransaction", __import__("base64").b64encode(bytes(tx)).decode(),
                 dict(encoding="base64", preflightCommitment="finalized"))
    print(f"sent cancel {sig}; read the marker back at finalized: byte 2")
    return sig


if __name__ == "__main__":
    main()
