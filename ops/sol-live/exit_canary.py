"""Orchestrator's one-burn mainnet proof. Keep the output directory for recovery."""

import argparse
import fcntl
import re
import json
import os
from pathlib import Path
import subprocess
import time

import requests
from exit_setup import (
    ASSET,
    ExitCluster,
    FEE_PAYER,
    VAULT,
    BUCKET,
    ROOT,
    SERIES,
    fleet,
    pk,
    read,
    require,
    rpc,
    save,
    six,
)
from exit_payout import ATA, MINT, PREFIX, TOKEN, marker, pda, signature

OWNER = "pf5c266a1bdb18320e140961411a8c4d8383eaafb2"
PHANTOM = pk("Ce5Yv528pKVrVq4JbQchZ8zasCAzxT3f9uCfatTZcfo8")
API = "http://127.0.0.1:18089/solana/withdrawals"
DEST = PREFIX + bytes(PHANTOM).hex()


def cluster():
    return ExitCluster(FEE_PAYER)


def expected_unsigned(fields):
    envelope = dict(fields)
    operation = envelope.pop("operation")
    return dict(
        envelope,
        **operation,
        address_namespace="postfiat.address.v1",
        transaction_kind=operation["operation"],
        signature_algorithm_id="ML-DSA-65",
    )


def check_unsigned(unsigned, fields):
    require(
        unsigned == expected_unsigned(fields),
        "Signed envelope differs from the reviewed quote.",
    )


def token_deltas(tx, owner=PHANTOM):
    meta = tx["meta"]
    require(meta["err"] is None, "Solana payout failed.")
    keys = tx["transaction"]["message"]["accountKeys"]
    addresses = [
        pda(VAULT, b"custody"),
        pda(ATA, bytes(owner), bytes(TOKEN), bytes(MINT)),
    ]
    deltas = {}
    for name, address in zip(("custody", "recipient"), addresses):
        index = keys.index(str(address))
        amounts = []
        for phase in ("preTokenBalances", "postTokenBalances"):
            rows = [r for r in meta[phase] if r["accountIndex"] == index]
            if not rows and name == "recipient" and phase == "preTokenBalances":
                amounts.append(0)
                continue
            require(
                len(rows) == 1
                and rows[0]["mint"] == str(MINT)
                and rows[0]["uiTokenAmount"]["decimals"] == 6,
                "Missing USDC payout token balance.",
            )
            amounts.append(int(rows[0]["uiTokenAmount"]["amount"]))
        deltas[name] = amounts[1] - amounts[0]
    require(
        deltas == {"custody": -20000, "recipient": 20000},
        "Canary transaction token deltas differ.",
    )
    return deltas


def balances(c):
    from exit_precheck import token_atoms

    addr = pda(ATA, bytes(PHANTOM), bytes(TOKEN), bytes(MINT))
    acc = c.account(addr)
    line = rpc(0, "account_assets", account=OWNER, asset_id=SERIES, limit=1)["assets"]
    require(
        len(line) == 1 and line[0]["source_bucket_id"] == BUCKET
        and line[0]["authorized"] and not line[0]["frozen"], "missing test series"
    )
    return dict(
        ata=str(addr),
        usdc_atoms=token_atoms(acc, PHANTOM, allow_empty=True),
        pfusdc_atoms=int(line[0]["balance"]),
    )


def prepare(out, *, allow_closed=False):
    r = requests.get(API + "/readiness", timeout=60).json()
    require(
        (r["ready"] or (allow_closed and r["payout_ready"] and not r["burns_open"]))
        and r["bucket_id"] == BUCKET and r["series_asset_id"] == SERIES,
        "Payout is unavailable",
    )
    before = balances(cluster())
    require(before["pfusdc_atoms"] >= 20000, "Insufficient Solana-backed pfUSDC")
    op = dict(
        operation="vault_bridge_burn_to_redeem",
        owner=OWNER,
        issuer=r["issuer"],
        asset_id=ASSET,
        bucket_id=BUCKET,
        amount_atoms=20000,
        epoch=r["epoch"],
        reserve_packet_hash=r["reserve_packet_hash"],
        destination_ref=DEST,
    )
    head = fleet()
    q = rpc(
        0,
        "asset_fee_quote",
        source=OWNER,
        operation_json=json.dumps(op, separators=(",", ":")),
    )
    require(
        q["source"] == OWNER
        and q["operation"] == op
        and q["chain_id"] == head["chain_id"]
        and q["genesis_hash"] == head["genesis_hash"],
        "Unexpected fee quote",
    )
    return dict(
        before=before,
        parent=head,
        fields=dict(
            chain_id=q["chain_id"],
            genesis_hash=q["genesis_hash"],
            protocol_version=q["protocol_version"],
            source=OWNER,
            fee=q["minimum_fee"],
            sequence=q["sequence"],
            operation=op,
        ),
    )


def burn(out):
    plan = read(out / "plan.json")
    require(
        not (out / "sign-request.json").exists(),
        "Signing already attempted. Use recover with this output directory.",
    )
    current = prepare(out)
    require(
        current["fields"] == plan["fields"]
        and current["parent"]["block_tip_hash"] == plan["parent"]["block_tip_hash"],
        "Quote or parent changed before signing. Prepare again.",
    )
    save(out / "sign-request.json", dict(kind="asset", fields=plan["fields"]))
    subprocess.run(
        [
            Path.home() / "repos/StakeHub/.venv/bin/python",
            ROOT / "ops/b24-live/sign_bob.py",
            out / "sign-request.json",
            out / "signed.json",
        ],
        check=True,
    )
    recover(out)


def verified_burn_id(raw, fields):
    signed = json.loads(raw)
    check_unsigned(signed["unsigned"], fields)
    op = fields["operation"]
    require(set(signed) == {"unsigned", "algorithm_id", "public_key_hex", "signature_hex"}
            and fields["source"] == op["owner"] == OWNER and op["asset_id"] == ASSET
            and op["destination_ref"] == DEST and op["bucket_id"] == BUCKET
            and op["amount_atoms"] == 20000, "Unexpected canary intent.")
    result = subprocess.run(
        [Path.home() / "repos/StakeHub/.venv/bin/python",
         Path(__file__).with_name("verify_canary_envelope.py")],
        input=raw, text=True, capture_output=True, check=True,
    ).stdout.strip()
    require(re.fullmatch("[a-f0-9]{96}", result), "Invalid signed burn ID.")
    return result


def recover(out):
    plan = read(out / "plan.json")
    require(read(out / "sign-request.json") == dict(kind="asset", fields=plan["fields"]),
            "Original signing request and plan differ.")
    with (out / "signed.json").open("rb") as f:
        raw = f.read().decode()
        os.fsync(f.fileno())
    fd = os.open(out, os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)
    tx_id = verified_burn_id(raw, plan["fields"])

    def status(i):
        try:
            return rpc(i, "tx", tx_id=tx_id)
        except RuntimeError as err:
            if "rpc_tx_not_found" not in str(err):
                raise
            return None

    rows = six(status)
    if any(row is not None for row in rows):
        require(all(r and r.get("confirmed") and r["receipt"]["accepted"]
                    and r["receipt"]["tx_id"] == tx_id for r in rows),
                "Original burn is pending, rejected or validators disagree.")
        return
    current = prepare(out)
    require(current["fields"] == plan["fields"], "Original envelope is no longer eligible.")
    head = current["parent"]
    params = dict(
        signed_asset_transaction_json=raw,
        proxy_required_current_height=head["block_height"],
        proxy_required_parent_hash=head["block_tip_hash"],
        proxy_required_state_root=head["state_root"],
    )
    for i in range(6):
        try:
            result = rpc(
                i, "mempool_submit_signed_asset_transaction_finality", **params
            )
        except RuntimeError as error:
            if "rpc_finality_wrong_proposer" in str(error):
                continue
            raise
        save(out / "submission.json", result)
        require(
            result["finality"]["confirmed"]
            and result["finality"]["receipt"]["accepted"]
            and result["finality"]["receipt"]["tx_id"] == tx_id,
            "burn not accepted",
        )
        return
    raise ValueError("Six validators refused proposer role")


def check(out):
    plan = read(out / "plan.json")
    reds = rpc(0, "vault_bridge_status", asset_id=ASSET)["redemptions"]
    rows = [
        r
        for r in reds
        if r["owner"] == OWNER
        and r["owner_sequence"] == plan["fields"]["sequence"]
        and r["bucket_id"] == BUCKET
        and r["amount_atoms"] == 20000
        and r["destination_ref"] == DEST
    ]
    require(len(rows) == 1, "Unique finalized canary burn not found")
    row = rows[0]
    tx = row["burn_tx_id"]
    require(tx == verified_burn_id((out / "signed.json").read_text(), plan["fields"]),
            "Finalized redemption differs from original signed envelope.")
    confirmations = six(lambda i: rpc(i, "tx", tx_id=tx))
    require(
        all(
            r["confirmed"]
            and r["receipt"]["accepted"]
            and r["block"]["header"]["height"] == row["created_at_height"]
            for r in confirmations
        ),
        "six-way burn confirmation failed",
    )
    res = requests.post(API, json=dict(burn_tx_id=tx), timeout=15)
    res.raise_for_status()
    deadline = time.monotonic() + 3600
    while time.monotonic() < deadline:
        res = requests.get(API + "/" + tx, timeout=15)
        res.raise_for_status()
        job = res.json()
        if job["stage"] == "failed":
            raise ValueError(job["error"])
        if job["stage"] == "paid":
            c = cluster()
            after = balances(c)
            before = plan["before"]
            require(
                job["solana_tx"],
                "A finalized transaction signature is required for the canary.",
            )
            finalized = c.call(
                "getTransaction",
                job["solana_tx"],
                dict(c.level, encoding="json", maxSupportedTransactionVersion=0),
            )
            deltas = token_deltas(finalized)
            require(
                signature(c, tx, job["solana_tx"]) == job["solana_tx"]
                and before["pfusdc_atoms"] - after["pfusdc_atoms"] == 20000,
                "Canary balance or signature mismatch",
            )
            evidence = dict(
                burn_tx_id=tx,
                height=row["created_at_height"],
                solana_tx=job["solana_tx"],
                before=before,
                after=after,
                token_deltas=deltas,
                finalized_slot=finalized["slot"],
                withdrawal_packet_hash=row["withdrawal_packet_hash"],
                marker=str(marker(tx)),
                recipient=str(PHANTOM),
            )
            save(out / "paid.json", evidence)
            print(json.dumps(evidence, indent=2))
            return
        time.sleep(8)
    raise TimeoutError("Payout pending. Run check again with the same directory.")


def settlement_state(status, paid):
    (row,) = [r for r in status["redemptions"] if r["burn_tx_id"] == paid["burn_tx_id"]]
    require(row["bucket_id"] == BUCKET and row["amount_atoms"] == 20000
            and row["withdrawal_packet_hash"] == paid["withdrawal_packet_hash"],
            "Settlement does not identify the original withdrawal packet.")
    (bucket,) = [b for b in status["buckets"] if b["bucket_id"] == BUCKET]
    supply = sum(a["remaining_atoms"] for a in status["allocations"]
                 if a["purpose"] == "vault_bridge_supply" and a["bucket_id"] == BUCKET)
    return dict(state=row["state"], settled=row["settled_atoms"], receipt=row["settlement_receipt_hash"],
                counted=bucket["counted_value_atoms"], queue=bucket["redemption_queue_atoms"],
                outstanding=bucket["outstanding_vault_bridge_atoms"], supply=supply)


def check_settlement(before, after, amendment):
    require(before["state"] == "pending" and before["settled"] == 0
            and after["state"] == "settled" and after["settled"] == 20000
            and after["receipt"] == amendment and after["outstanding"] == before["outstanding"]
            and all(before[k] - after[k] == 20000 for k in ("counted", "queue", "supply")),
            "Settlement receipt or revision-8 accounting differs.")


def settlement(out, amendment=None, height=None):
    from live import receipt_at
    from exit_precheck import agreement

    paid = read(out / "paid.json")
    before_path = out / "settlement-before.json"
    state = agreement(six(lambda i: settlement_state(rpc(i, "vault_bridge_status", asset_id=ASSET), paid)))
    if amendment is None:
        require(state["state"] == "pending" and state["settled"] == 0, "Expected pending redemption.")
        if before_path.exists():
            require(read(before_path) == state, "Pre-settlement accounting changed.")
        else:
            save(before_path, state)
        return
    require(re.fullmatch("[0-9a-f]{96}", amendment) and height, "Settlement ID and height required.")
    receipts = receipt_at(amendment, height)
    check_settlement(read(before_path), state, amendment)
    save(out / "settled.json", dict(state=state, validators=receipts, amendment=amendment))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("action", choices=("preflight", "prepare", "burn", "recover", "check", "settlement-before", "settlement-check"))
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--amendment")
    p.add_argument("--height", type=int)
    args = p.parse_args()
    os.umask(0o077)
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    with (out / "lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if args.action == "preflight":
            save(out / "preflight.json", prepare(out, allow_closed=True))
        elif args.action == "settlement-before":
            settlement(out)
        elif args.action == "settlement-check":
            require(args.amendment and args.height, "Specify the accepted amendment and its height.")
            settlement(out, args.amendment, args.height)
        elif args.action == "prepare":
            require(
                not (out / "sign-request.json").exists(),
                "Existing signing attempt. Use check.",
            )
            save(out / "plan.json", prepare(out))
        elif args.action == "burn":
            burn(out)
        elif args.action == "recover":
            recover(out)
        else:
            check(out)


if __name__ == "__main__":
    main()
