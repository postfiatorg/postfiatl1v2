"""Build a governed profile from a certificate-verified frozen bank archive."""

import argparse
import hashlib
import json
import struct
import subprocess
from pathlib import Path
from solders.hash import Hash
from solders.pubkey import Pubkey
from live import ASSET, INSTANCE, ROOT, STATE, read, save, sol
from keys import DIRECTORY, load
from route_binding import route_binding, PUBLIC_ROUTES

CAPTURE = ROOT / "target/task-l/release/pfusdc-sol-capture"
MANIFEST = ROOT / "programs/pfusdc-sol-ingress/program-info.json"


def table(ranks):
    (count,) = struct.unpack_from("<I", ranks, 16)
    if not count or len(ranks) != 20 + 56 * count:
        raise ValueError("bad epoch rank table")
    keys = sorted(ranks[i : i + 48] for i in range(20, len(ranks), 56))
    if len(set(keys)) != count:
        raise ValueError("duplicate BLS key")
    return struct.pack("<I", count) + b"".join(keys)


def profile_hash(p):
    fields = (
        "genesis_hash",
        "route_binding",
        "shred_version",
        "handoff_program_vkey",
        "slots_per_epoch",
        "epoch_warmup",
        "recorder_program",
        "anchor_slot",
        "anchor",
    )
    top = (
        "schema",
        "route_id",
        "asset_id",
        "source_chain_id",
        "vault_address",
        "vault_runtime_code_hash",
        "token_address",
        "token_runtime_code_hash",
        "route_epoch",
        "verifier_kind",
        "evidence_tier",
        "verifier_policy_hash",
        "verifier_program_vkey",
        "verifier_proof_encoding",
        "max_proof_bytes",
        "max_public_values_bytes",
        "max_snapshot_age_blocks",
        "challenge_window_blocks",
        "max_epoch_gap_blocks",
        "settle_deadline_blocks",
        "min_challenge_bond",
        "min_attestations",
        "minimum_confirmations",
        "activation_height",
        "expires_at_height",
    )
    pins = {k: p["solana"][k] for k in fields}
    pins["anchor"] = {
        k: pins["anchor"][k]
        for k in ("epoch", "validator_set_hash", "total_stake", "known_key_table_hash")
    }
    canonical = {"solana": pins, **{k: p[k] for k in top}}
    return hashlib.sha3_384(
        b"postfiat.vault_bridge.route_profile_hash.v1\0"
        + json.dumps(canonical, separators=(",", ":")).encode()
    ).hexdigest()


def profile(url, directory, activation_height, mainnet=False, *, historical_epoch=None):
    slot = int(directory.name)
    schedule = sol(url, "getEpochSchedule")
    epoch = sol(url, "getEpochInfo", {"commitment": "finalized"})
    if historical_epoch is None:
        if (
            not epoch["absoluteSlot"] - epoch["slotIndex"]
            <= slot
            <= epoch["absoluteSlot"]
        ):
            raise ValueError("archive is outside the current finalized epoch")
        anchor_epoch = epoch["epoch"]
    else:
        if (
            mainnet
            or slot > epoch["absoluteSlot"]
            or read(directory / "events.json")["epoch"] != historical_epoch
        ):
            raise ValueError("invalid local historical capture")
        first = schedule["firstNormalSlot"]
        anchor_epoch = (
            schedule["firstNormalEpoch"] + (slot - first) // schedule["slotsPerEpoch"]
            if slot >= first
            else ((slot + 32) // 32).bit_length() - 1
        )
        if anchor_epoch != historical_epoch:
            raise ValueError("historical epoch differs from genesis schedule")
    genesis = sol(url, "getGenesisHash")
    spec = read(STATE / "cluster.json") if not mainnet else {}
    if (
        mainnet
        and PUBLIC_ROUTES.get(genesis) != b"solana-mainnet-usdc-v1"
        or not mainnet
        and genesis != spec["genesis"]
    ):
        raise ValueError("wrong source genesis")
    out = STATE / "anchors" / genesis / str(slot)
    out.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [CAPTURE, "build", directory.parent, str(slot), out / "witness"], check=True
    )
    ranks = (directory / "validators").read_bytes()
    shred = (directory / "shreds").read_bytes()
    vault, recorder = [
        load(
            (
                DIRECTORY / INSTANCE
                if not mainnet and INSTANCE == "devnet" and n == "vault"
                else DIRECTORY
            )
            / (n + ".json")
        ).pubkey()
        for n in ("vault", "recorder")
    ]
    mint = Pubkey.from_string(
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" if mainnet else spec["mint"]
    )
    pins = {r["kind"]: r["program_vkey"] for r in read(MANIFEST)["programs"]}
    return dict(
        schema="postfiat.vault_bridge.route_profile.v1",
        route_id=f"solana-{'mainnet' if mainnet else INSTANCE}-usdc-v1",
        asset_id=ASSET,
        source_chain_id=0,
        vault_address=bytes(vault).hex(),
        vault_runtime_code_hash="",
        token_address=bytes(mint).hex(),
        token_runtime_code_hash="",
        route_epoch=1,
        verifier_kind="sp1-solana-alpenglow-finality-v1",
        evidence_tier="receipt-proven",
        verifier_policy_hash="",
        verifier_program_vkey=pins["aggregate"],
        verifier_proof_encoding="groth16",
        max_proof_bytes=4096,
        max_public_values_bytes=16384,
        max_snapshot_age_blocks=100,
        challenge_window_blocks=1,
        max_epoch_gap_blocks=100,
        settle_deadline_blocks=10,
        min_challenge_bond=0,
        min_attestations=0,
        minimum_confirmations=0,
        activation_height=activation_height,
        expires_at_height=4294967295,
        solana=dict(
            genesis_hash=bytes(Hash.from_string(genesis)).hex(),
            shred_version=int.from_bytes(shred[52 + 77 : 52 + 79], "little"),
            handoff_program_vkey=pins["handoff"],
            slots_per_epoch=schedule["slotsPerEpoch"],
            epoch_warmup=schedule["warmup"],
            recorder_program=bytes(recorder).hex(),
            anchor_slot=slot,
            route_binding=route_binding(
                genesis, vault, local=not mainnet, test_route=INSTANCE
            ).hex(),
            anchor=dict(
                epoch=anchor_epoch,
                validator_set_hash=hashlib.sha256(
                    b"postfiat.pfusdc.solana.validator_set.v1" + ranks
                ).hexdigest(),
                total_stake=str(int.from_bytes(ranks[:16], "little")),
                known_key_table_hash=hashlib.sha256(
                    b"postfiat.pfusdc.solana.known_keys.v1" + table(ranks)
                ).hexdigest(),
            ),
        ),
    )


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("archive", type=Path)
    p.add_argument("output", type=Path)
    p.add_argument("--rpc", required=True)
    p.add_argument("--height", required=True, type=int)
    p.add_argument("--mainnet", action="store_true")
    a = p.parse_args()
    save(a.output, profile(a.rpc, a.archive, a.height, a.mainnet))
