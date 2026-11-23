"""Build and simulate the reviewed Solana light-client bootstrap. CLI defaults to simulation."""

import argparse
import base64
import hashlib
import os
import sys
from pathlib import Path

from live import ASSET, ROOT, fleet, read, rpc, save, six
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


def prove_roles(config):
    require(
        config["pause_authority"] == bytes(PAUSE).hex(), "Unreviewed pause authority."
    )
    digest = hashlib.sha256(b"\x03" + encode(config)).hexdigest()
    nonce = os.urandom(32).hex()
    roles = {}
    for role, expected, name in (
        ("fee_payer", FEE_PAYER, "exit-fee-payer.json"),
        ("pause_authority", PAUSE, "exit-pause-authority.json"),
    ):
        challenge = f"postfiat.sol-exit.offline-possession.v1/{digest}/{nonce}/{role}/{expected}".encode()
        sig = load(DIRECTORY / name).sign_message(challenge)
        require(sig.verify(expected, challenge), f"Offline {role} challenge failed.")
        roles[role] = dict(
            address=str(expected), challenge_hex=challenge.hex(), signature=str(sig)
        )
    return dict(tag3_sha256=digest, nonce=nonce, roles=roles)


def check_payout_authorities(c):
    addresses = [str(pda(LOADER, bytes(program))) for program in (VAULT, VERIFIER)]
    result = c.call(
        "getMultipleAccounts",
        addresses,
        dict(
            commitment="finalized",
            encoding="base64",
            dataSlice={"offset": 0, "length": 45},
        ),
    )
    require(len(result["value"]) == 2, "Program authorities unavailable.")
    for account in result["value"]:
        require(
            account and account["owner"] == str(LOADER),
            "Program authority unavailable.",
        )
        data = base64.b64decode(account["data"][0], validate=True)
        require(
            len(data) == 45
            and data[:4] == (3).to_bytes(4, "little")
            and data[12] in (0, 1)
            and (not data[12] or data[13:45] not in (bytes(FEE_PAYER), bytes(PAUSE))),
            "An exit credential controls a program upgrade.",
        )


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


def send_initialize(c, ix, deployment, qualified_parent):
    from exit_precheck import parent_of

    recent = c.call("getLatestBlockhash", c.level)["value"]
    block = Hash.from_string(recent["blockhash"])
    require(
        parent_of(fleet()) == qualified_parent, "Tip advanced. Repeat qualification."
    )
    tx = Transaction.new_signed_with_payer([ix], c.owner, [c.payer, deployment], block)
    sig = str(tx.signatures[0])
    result = c.call(
        "sendTransaction",
        base64.b64encode(bytes(tx)).decode(),
        dict(encoding="base64", preflightCommitment="finalized", maxRetries=0),
    )
    require(result == sig, "Initialization signature mismatch.")
    c.confirm([("egress.initialize", sig)])
    return sig


def check_reviewed_instruction(config, ix):
    expected = [
        (MAINNET_PAYER, True, True),
        (VERIFIER, True, False),
        (LIGHT, False, True),
        (pk("BATyPAkKcpPBjFAttaHAVR9raxdNuWtQUNrdKgaWkait"), False, True),
        (SYSTEM, False, False),
    ]
    require(
        ix.program_id == VERIFIER
        and [(a.pubkey, a.is_signer, a.is_writable) for a in ix.accounts] == expected,
        "Unreviewed bootstrap account permissions.",
    )
    require(
        config == read(CONFIG)
        and hashlib.sha256(ix.data).hexdigest()
        == "bb32524804b64a9d7d40fec86c9735f8c1f5d5d03d7a8a6963e8c21f83903fe4",
        "Unreviewed bootstrap bytes.",
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
        parent=h["parent_hash"],
        certificate_id=h["certificate_id"],
        committee=h["consensus_v2_commit"]["precommit_qc"]["domain"]["committee_root"],
        verified=r["confirmed"]
        and r["receipt"]["accepted"]
        and h["certificate_id"] == cp["certificate_id"]
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
        if key in (VERIFIER, VAULT):
            require(
                programdata["data"][12:45] == b"\x01" + bytes(MAINNET_PAYER)
                and MAINNET_PAYER not in (PAUSE, FEE_PAYER),
                "Unexpected program upgrade authority.",
            )
        data = programdata["data"][45:]
    return "0x" + hashlib.sha256(data).hexdigest()


def main():
    from exit_precheck import (
        agreement,
        archive_cmd,
        recover_bootstrap,
        check_certificate,
        committee,
        qualify_path,
        execute_guest,
        inspect_vault,
        records,
        certified_witness,
    )

    if sys.argv[1:2] == ["prove-roles"]:
        parser = argparse.ArgumentParser(description="Verify both exit roles offline")
        parser.add_argument("--output", required=True, type=Path)
        save(parser.parse_args(sys.argv[2:]).output, prove_roles(read(CONFIG)))
        return
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--qualify", action="store_true", help="require registered descriptor and six rotation clearances")
    ap.add_argument("--output", required=True, type=Path)
    ap.add_argument(
        "--execute",
        action="store_true",
        help="orchestrator only, requires six governed series reads",
    )
    ap.add_argument("--binary", type=Path, required=True, help="reviewed replay binary")
    ap.add_argument(
        "--archive",
        type=Path,
        required=True,
        help="writable scratch archive at height 1753",
    )
    ap.add_argument(
        "--integrity-file", type=Path, help="mount archive integrity file read-only"
    )
    ap.add_argument(
        "--init-signature", help="finalized signature for historical recovery"
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
    vault = inspect_vault(c)
    head = fleet()
    from exit_govern import rotation_clearance

    clearances = rotation_clearance(head) if args.qualify or args.execute else None
    rows = six(trusted)
    anchor_row = agreement(rows)
    cert = archive_cmd(args, args.archive, "verify-finalized-checkpoint")
    archived = certified_witness(args, cert, anchor_row["parent"])
    check_certificate(cert, anchor_row, committee(archived["block"]["header"]))
    inputs = six(records)
    agreed = agreement(inputs)
    config = config_from_records(
        agreed["activation"], profile, anchor_row, agreed["bucket"]
    )
    save(args.output.parent / "anchor-witness.json", archived)
    ix = build_initialize(config, c.owner)
    check_reviewed_instruction(config, ix)
    anchor = ix.accounts[3].pubkey
    initialized = recover_bootstrap(c, config, ix, args.init_signature)
    path = qualify_path(
        archived["committee"],
        anchor_row,
        head,
        args.output.parent / "guest",
        simulation_height=initialized["height"] if initialized else None,
    )
    guest = path["guest"]
    if head["block_height"] == anchor_row["height"]:
        values = execute_guest(archived, args.output.parent / "guest")
        require(
            (values["result"], values["end"], values["height"])
            == (anchor_row["block"], anchor_row["committee"], anchor_row["height"]),
            "Frozen guest disagrees with the archived anchor.",
        )
        guest = dict(values, forward=False)
    evidence = dict(
        vault=vault,
        fleet_height=head["block_height"],
        qualified_parent=path["qualified_parent"],
        rotation_clearances=clearances,
        segments=path["segments"],
        anchors=rows,
        certificate=cert,
        inputs=inputs,
        guest=guest,
        config=config,
        accounts=[str(a.pubkey) for a in ix.accounts],
        instruction_hex=ix.data.hex(),
    )

    if initialized:
        save(args.output, dict(evidence, **initialized))
        print("Verified both initialized accounts and the finalized tag-3 transaction.")
        return
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
        **evidence,
        fee_lamports=fee,
        rent_lamports=rent,
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
        require(
            guest["forward"],
            "Signing requires a finalized segment starting at the trusted block.",
        )
        governed = six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES))
        require(
            all(
                r["active"]
                and r.get("burns_open") is False
                and r["profile_hash"] == EXIT
                and r["profile"] == profile
                for r in governed
            ),
            "waiting for governed series on six validators",
        )
        inspect_vault(c)
        prove_roles(config)
        c.unlock(DIRECTORY / "mainnet-payer.json")
        deployment = load(DIRECTORY / "egress-verifier.json")
        require(deployment.pubkey() == VERIFIER, "incorrect program signer")
        sig = send_initialize(c, ix, deployment, path["qualified_parent"])
        initialized = recover_bootstrap(c, config, ix, sig)
        require(initialized, "Waiting for finalized bootstrap readback.")
        save(args.output, dict(record, **initialized))
    print(f"Simulation passed. Rent {rent} lamports, fee {fee} lamports.")


if __name__ == "__main__":
    main()
