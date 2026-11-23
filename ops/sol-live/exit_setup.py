"""Build and simulate the reviewed Solana light-client bootstrap. CLI defaults to simulation."""

import argparse
import base64
import hashlib
import os
import sys
from pathlib import Path

from live import ASSET, ROOT, STATE, fleet, read, rpc, save, six
from bridge import (
    Cluster,
    DIRECTORY,
    Instruction,
    LOADER,
    SYSTEM,
    TOKEN,
    pda,
    pk,
    ro,
    rw,
)
import requests
from exit_keys import load
from solders.hash import Hash
from solders.message import Message
from solders.transaction import Transaction

sys.path.insert(0, str(ROOT / "solana/pfusdc-egress-verifier"))
from bootstrap import encode, packet_alias

VERIFIER = pk("8Dxp8bfgEh6ankapfBdkTds5cebqrtPdFxpZSP6JsGAF")
VAULT = pk("GFkpeTibgdRAAAndgUMYsJnSTck91tUbrpuWBWan6z1G")
MAINNET_PAYER = pk("4JJEuaYtJvgtMVixTbXfppHJPh6tcU5g7g7UtkvxK3sB")
FEE_PAYER = pk("8EiSLnZQv23HtA5adokE6xfKyCK6mf4HGkpxSPpop1fs")
PAUSE = pk("ow7wprz3MPEN15BGLyNvdJr5AUq4LNZK2ox1gY1QEJx")
CONFIG = Path(__file__).parent / "fixtures/exit-setup.json"
MINT = pk("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
GENESIS = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
BUCKET = "4c045e289eaaa2c7ae7592b96ec0eeabe9fa0cfdd032783d359409dcc08d9dd0dfdd7cc8d483d6a88c12dc713522d2cc"
SERIES = "e90627502bea5857aefd003d80af518f0ccb44ad2be8098ec6db2025ccaf8390e8bef31e7538b398a3c94f019d7b3b0f"
POLICY = "5bb5a3d99616945cb7330c07a51c47c0453ff54f6c622f44f18c5b505cc9b13f179a6796a537119a482d788537bd96c7"
EXIT = "37e3c05d8e353d93d56819bfdc3b10f314e5ae298cf43a34cefac477d06893953ceca391165ec3bc977c5e5b273b4046"
LIGHT = pda(VERIFIER, b"egress-light")
VERIFIER_SHA256 = "0x3b16fc309501814e0bba718ecadb80f02993f6f355153a13f37ae9d47d29354c"
PROFILE = ROOT / "crates/types/src/solana_exit_profile.json"
ANCHOR_TX = "9da535ba12b9f8eb7e877e8dfb985503b0ecb8eda0fcee06f2e98a40d8627c62f55c517db4e407ca0f61603489b02f2e"


def require(condition, message):
    if not condition:
        raise ValueError(message)


class ExitCluster(Cluster):
    def __init__(self, owner):
        self.owner = owner
        self.payer = None
        self.rpc = os.getenv("SOL_MAINNET_RPC", "https://api.mainnet-beta.solana.com")
        self.http = requests.Session()
        self.level = {"commitment": "finalized"}
        self.genesis = self.call("getGenesisHash")
        require(self.genesis == GENESIS, "Wrong Solana network.")

    def unlock(self, path):
        key = load(path)
        require(key.pubkey() == self.owner, "Incorrect signer.")
        self.payer = key

    def event(self, **record):
        # The journal records payout signatures. Never write a deployment key-directory log.
        pass


def config_from_records(activation, profile, checkpoint, bucket):
    require(
        activation.get("profile_hash") == POLICY
        and activation["profile"]["route_epoch"] == 8,
        "expected revision 8 ingress authorization",
    )
    sol_genesis = bytes(Hash.from_string(GENESIS)).hex()
    source = f"spl:{sol_genesis}:{bytes(VAULT).hex()}:{bytes(MINT).hex()}"
    require(
        bucket["bucket_id"] == BUCKET
        and bucket["policy_hash"] == POLICY
        and bucket["source_domain"] == source
        and bucket["source_series_id"] == SERIES,
        "wrong source bucket",
    )
    template = read(PROFILE)
    preimage = "".join(f"{k}={profile[k]}\n" for k in template)
    digest = hashlib.sha3_384(
        b"postfiat.vault_bridge.route_profile_hash.v1\0" + preimage.encode()
    ).hexdigest()
    require(
        digest == EXIT
        and profile == template
        and profile["vault_address"] == "0x" + packet_alias(bytes(VAULT)).hex()
        and profile["token_address"] == "0x" + packet_alias(bytes(MINT)).hex(),
        "wrong exit descriptor",
    )
    from route_binding import CHAIN, PFTL_GENESIS

    require(
        checkpoint["verified"]
        and checkpoint["chain_id"] == CHAIN.decode()
        and checkpoint["genesis_hash"] == PFTL_GENESIS.hex()
        and checkpoint["protocol_version"] == 1,
        "wrong checkpoint domain",
    )
    return dict(
        vault_program=bytes(VAULT).hex(),
        solana_genesis=sol_genesis,
        ingress_profile_hash=POLICY,
        ingress_epoch=8,
        asset_id=ASSET,
        chain_id=checkpoint["chain_id"],
        genesis_hash=checkpoint["genesis_hash"],
        protocol_version=1,
        exit_profile_hash=EXIT,
        exit_epoch=profile["route_epoch"],
        vault_code_hash=profile["vault_runtime_code_hash"],
        token_code_hash=profile["token_runtime_code_hash"],
        pause_authority=bytes(PAUSE).hex(),
        initial_checkpoint=checkpoint["block"],
        initial_committee=checkpoint["committee"],
        initial_height=checkpoint["height"],
    )


def checkpoint_pda(block):
    return pda(
        VERIFIER, b"egress-checkpoint", hashlib.sha256(bytes.fromhex(block)).digest()
    )


def build_initialize(config, payer):
    return Instruction(
        VERIFIER,
        b"\x03" + encode(config),
        [
            rw(payer, True),
            ro(VERIFIER, True),
            rw(LIGHT),
            rw(checkpoint_pda(config["initial_checkpoint"])),
            ro(SYSTEM),
        ],
    )


def expected_light(config):
    from route_binding import route_binding

    raw = encode(config)
    return (
        bytes(VAULT)
        + route_binding(GENESIS, VAULT)
        + packet_alias(bytes(VAULT))
        + packet_alias(bytes(MINT))
        + raw[84:212]
        + raw[212:260]
        + raw[-8:]
        + b"\0"
    )


def trusted(i):
    r = rpc(i, "tx", tx_id=ANCHOR_TX)
    h = r["block"]["header"]
    cp = read(Path(__file__).parent / "fixtures/exit-checkpoint.json")
    return dict(
        chain_id=r["chain_id"],
        genesis_hash=r["genesis_hash"],
        protocol_version=r["protocol_version"],
        height=h["height"],
        block=h["block_hash"],
        root=h["state_root"],
        committee=h["consensus_v2_commit"]["precommit_qc"]["domain"]["committee_root"],
        verified=r["confirmed"]
        and r["receipt"]["accepted"]
        and cp["verified"]
        and h["block_hash"] == cp["checkpoint_block_hash"]
        and h["state_root"] == cp["checkpoint_state_root"]
        and h["height"] == cp["checkpoint_height"]
        and r["receipt"]["tx_id"] == ANCHOR_TX,
    )


def profile_code(c, key):
    account = c.account(key)
    require(account and account["executable"], "executable program missing")
    data = account["data"]
    if account["owner"] == str(LOADER):
        require(data[:4] == (2).to_bytes(4, "little"), "invalid upgradeable program")
        programdata = c.account(type(key).from_bytes(data[4:36]))
        require(
            programdata
            and programdata["owner"] == str(LOADER)
            and programdata["data"][:4] == (3).to_bytes(4, "little"),
            "invalid program data",
        )
        data = programdata["data"][45:]
    return "0x" + hashlib.sha256(data).hexdigest()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--output", required=True, type=Path)
    ap.add_argument(
        "--execute",
        action="store_true",
        help="orchestrator only, requires six governed series reads",
    )
    args = ap.parse_args()
    c = ExitCluster(MAINNET_PAYER)
    profile = read(PROFILE)
    require(
        profile_code(c, VERIFIER) == VERIFIER_SHA256
        and profile_code(c, VAULT) == profile["vault_runtime_code_hash"]
        and profile_code(c, TOKEN) == profile["token_runtime_code_hash"],
        "deployed program hash changed",
    )
    activation = read(STATE / "mainnet/activation.json")
    if activation["profile_hash"] != POLICY:
        candidates = [
            read(p) for p in (STATE / "mainnet").glob("activation-revision-*.json")
        ]
        activation = next(a for a in candidates if a["profile_hash"] == POLICY)
    rows = six(trusted)
    require(all(r == rows[0] for r in rows), "checkpoint disagreement")
    bucket = next(
        b
        for b in rpc(0, "vault_bridge_status", asset_id=ASSET)["buckets"]
        if b["bucket_id"] == BUCKET
    )
    config = config_from_records(activation, profile, rows[0], bucket)
    ix = build_initialize(config, c.owner)
    anchor = ix.accounts[3].pubkey
    require(
        not c.account(LIGHT) and not c.account(anchor), "bootstrap PDAs already exist"
    )
    block = Hash.from_string(
        c.call("getLatestBlockhash", c.level)["value"]["blockhash"]
    )
    msg = Message.new_with_blockhash([ix], c.owner, block)
    tx = Transaction.new_unsigned(msg)
    fee = c.call("getFeeForMessage", base64.b64encode(bytes(msg)).decode(), c.level)[
        "value"
    ]
    rent = c.rent(289) + c.rent(48)
    sim = c.call(
        "simulateTransaction",
        base64.b64encode(bytes(tx)).decode(),
        dict(
            encoding="base64",
            sigVerify=False,
            replaceRecentBlockhash=True,
            commitment="finalized",
            accounts=dict(encoding="base64", addresses=[str(LIGHT), str(anchor)]),
        ),
    )
    record = dict(
        config=config,
        fee_lamports=fee,
        rent_lamports=rent,
        accounts=[str(a.pubkey) for a in ix.accounts],
        instruction_hex=ix.data.hex(),
        simulation=sim,
    )
    save(args.output, record)
    require(sim["value"]["err"] is None, f"simulation failed, see {args.output}")
    light, cp = sim["value"]["accounts"]
    require(
        light["owner"] == cp["owner"] == str(VERIFIER)
        and base64.b64decode(light["data"][0]) == expected_light(config)
        and base64.b64decode(cp["data"][0]).hex() == config["initial_committee"],
        "unexpected simulated state",
    )
    if args.execute:
        fleet()
        governed = six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES))
        require(
            all(
                r["active"] and r["profile_hash"] == EXIT and r["profile"] == profile
                for r in governed
            ),
            "waiting for governed series on six validators",
        )
        c.unlock(DIRECTORY / "mainnet-payer.json")
        deployment = load(DIRECTORY / "egress-verifier.json")
        require(deployment.pubkey() == VERIFIER, "incorrect program signer")
        sig = c.send("egress.initialize", [ix], deployment)
        require(
            c.account(LIGHT)["data"] == expected_light(config)
            and c.account(anchor)["data"].hex() == config["initial_committee"],
            "finalized bootstrap readback differs",
        )
        save(args.output, dict(record, signature=sig, finalized_readback=True))
    print(f"Simulation passed. Rent {rent} lamports, fee {fee} lamports.")


if __name__ == "__main__":
    main()
