"""Initialize egress from an accepted exit descriptor and a verified Post Fiat checkpoint."""

import hashlib
import sys
from live import ASSET, ROOT, rpc, six
from bridge import SYSTEM, Instruction, pda, require, ro, rw

sys.path.insert(0, str(ROOT / "solana/pfusdc-egress-verifier"))
from bootstrap import encode, packet_alias


class MissingSolanaDescriptor(RuntimeError):
    """The six nodes agree on an exit profile for another route."""


def descriptor(vault, mint):
    rows = six(lambda i: rpc(i, "vault_bridge_route", asset_id=ASSET))
    row = rows[0]
    profile = row["profile"]
    require(
        all(
            r["active"]
            and r["profile_hash"] == row["profile_hash"]
            and r["profile"] == profile
            for r in rows
        ),
        "exit descriptor is not active on all validators",
    )
    if not (
        profile["source_chain_id"] == int.from_bytes(b"SOLANA01", "big")
        and profile["vault_address"] == "0x" + packet_alias(bytes(vault)).hex()
        and profile["token_address"] == "0x" + packet_alias(bytes(mint)).hex()
    ):
        raise MissingSolanaDescriptor(
            f"active exit profile {profile['route_id']} ({row['profile_hash']}) "
            "does not bind the mainnet Solana vault and SPL USDC"
        )
    return row


def bootstrap(c, verifier, activation, exit_descriptor, trusted):
    p = activation["profile"]
    txid = trusted["tx_id"]

    def checkpoint(i):
        result = rpc(i, "tx", tx_id=txid)
        header = result["block"]["header"]
        require(
            result["block_log_verified"]
            and result["confirmed"]
            and result["receipt"]["accepted"]
            and header["state_root"] == trusted["validators"][i]["root"],
            "checkpoint receipt mismatch",
        )
        qc = header["consensus_v2_commit"]["precommit_qc"]
        return (
            header["block_hash"],
            header["height"],
            qc["domain"]["committee_root"],
            result["chain_id"],
            result["genesis_hash"],
            result["protocol_version"],
        )

    rows = six(checkpoint)
    require(len(set(rows)) == 1, "egress checkpoint disagreement")
    block, height, committee, chain, genesis, version = rows[0]
    x = exit_descriptor["profile"]
    config = dict(
        vault_program=p["vault_address"],
        solana_genesis=p["solana"]["genesis_hash"],
        ingress_profile_hash=activation["profile_hash"],
        ingress_epoch=p["route_epoch"],
        asset_id=ASSET,
        chain_id=chain,
        genesis_hash=genesis,
        protocol_version=version,
        exit_profile_hash=exit_descriptor["profile_hash"],
        exit_epoch=x["route_epoch"],
        vault_code_hash=x["vault_runtime_code_hash"],
        token_code_hash=x["token_runtime_code_hash"],
        pause_authority=bytes(c.owner).hex(),
        initial_checkpoint=block,
        initial_committee=committee,
        initial_height=height,
    )
    key = verifier.pubkey()
    light = pda(key, b"egress-light")
    anchor = pda(
        key, b"egress-checkpoint", hashlib.sha256(bytes.fromhex(block)).digest()
    )
    if not c.account(light):
        c.send(
            "egress.initialize",
            [
                Instruction(
                    key,
                    b"\3" + encode(config),
                    [
                        rw(c.owner, True),
                        ro(key, True),
                        rw(light),
                        rw(anchor),
                        ro(SYSTEM),
                    ],
                )
            ],
            verifier,
        )
    state = c.account(light)
    require(
        state
        and state["owner"] == str(key)
        and state["data"][:64]
        == bytes.fromhex(p["vault_address"] + p["solana"]["route_binding"])
        and c.account(anchor)["data"] == bytes.fromhex(committee),
        "egress checkpoint or vault binding mismatch",
    )
