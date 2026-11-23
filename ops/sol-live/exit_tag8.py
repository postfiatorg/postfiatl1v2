"""Stage a checkpoint proof buffer, then simulate tag 8 without a signature."""

import argparse
import base64
from pathlib import Path
from solders.compute_budget import set_compute_unit_limit
from solders.hash import Hash
from solders.message import Message
from solders.transaction import Transaction

from exit_precheck import checkpoint_values, light_fields
from exit_proof import prove
from exit_payout import cluster, funded_send, upload
from exit_setup import (
    CONFIG,
    ExitCluster,
    FEE_PAYER,
    LIGHT,
    SYSTEM,
    VERIFIER,
    Instruction,
    checkpoint_pda,
    pk,
    read,
    require,
    ro,
    rw,
    save,
)


def simulate(c, values, proof, buffer, light):
    result = checkpoint_pda(values["result"])
    ix = Instruction(
        VERIFIER,
        b"\x08" + proof,
        [
            rw(c.owner, True),
            rw(LIGHT),
            ro(checkpoint_pda(values["prior"])),
            rw(result),
            ro(buffer),
            ro(SYSTEM),
        ],
    )
    block = Hash.from_string(
        c.call("getLatestBlockhash", c.level)["value"]["blockhash"]
    )
    msg = Message.new_with_blockhash(
        [set_compute_unit_limit(400_000), ix], c.owner, block
    )
    raw = base64.b64encode(bytes(Transaction.new_unsigned(msg))).decode()
    reply = c.call(
        "simulateTransaction",
        raw,
        dict(
            c.level,
            encoding="base64",
            sigVerify=False,
            replaceRecentBlockhash=True,
            accounts=dict(encoding="base64", addresses=[str(LIGHT), str(result)]),
        ),
    )
    require(reply["value"]["err"] is None, "Tag-8 simulation failed.")
    expected = [
        light[:-57]
        + bytes.fromhex(values["result"])
        + values["height"].to_bytes(8, "little")
        + light[-1:],
        bytes.fromhex(values["end"]),
    ]
    rows = reply["value"]["accounts"]
    require(
        len(rows) == 2
        and all(
            a and a["owner"] == str(VERIFIER) and base64.b64decode(a["data"][0]) == d
            for a, d in zip(rows, expected)
        ),
        "Tag-8 simulated state differs.",
    )
    return reply


def require_forward(c, cfg, light, values):
    height, block = light_fields(cfg, light)
    require(values["prior"] == block and values["height"] > height and not light["data"][-1],
            "A forward witness from the current unpaused checkpoint is required.")
    cp = c.account(checkpoint_pda(block))
    require(cp and not cp["executable"] and cp["owner"] == str(VERIFIER)
            and cp["data"].hex() == values["start"], "Current checkpoint committee differs.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["stage", "simulate", "reclaim"])
    parser.add_argument("--guest", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    args = parser.parse_args()
    raw = (args.guest / "public-values.bin").read_bytes()
    values = checkpoint_values(raw)
    cfg = read(CONFIG)
    c = ExitCluster(FEE_PAYER) if args.action == "simulate" else cluster()
    if args.action != "reclaim":
        light = c.account(LIGHT)
        require_forward(c, cfg, light, values)
    args.work.mkdir(mode=0o700, parents=True, exist_ok=True)
    if args.action == "stage":
        data, proof = prove("checkpoint", read(args.guest / "witness.json"), args.work)
        require(data == raw, "Proof public values differ from executed guest.")
        signer = upload(c, raw, args.work)
        save(
            args.work / "buffer.json",
            dict(address=str(signer.pubkey()), proof=proof.hex()),
        )
        return
    staged = read(args.work / "buffer.json")
    buf = pk(staged["address"])
    acc = c.account(buf)
    require(
        acc and acc["owner"] == str(VERIFIER) and acc["data"] == raw,
        "Staged public values differ.",
    )
    if args.action == "reclaim":
        from exit_keys import load
        import hashlib

        signer = load(args.work / (hashlib.sha256(raw).hexdigest() + ".key"))
        require(signer.pubkey() == buf, "Wrong buffer signer.")
        funded_send(
            c,
            "exit.reclaim",
            [Instruction(VERIFIER, b"\x06", [rw(buf, True), rw(c.owner)])],
            signer,
        )
        return
    proof = bytes.fromhex(staged["proof"])
    require(len(proof) == 356, "Wrong checkpoint proof length.")
    save(args.work / "simulation.json", simulate(c, values, proof, buf, light["data"]))


if __name__ == "__main__":
    main()
