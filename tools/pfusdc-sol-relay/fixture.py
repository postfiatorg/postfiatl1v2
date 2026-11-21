"""Provision a disposable relay configuration against the signed differential fixture."""

import hashlib
import subprocess
from pathlib import Path

from route import PFTL_CHAIN, PFTL_GENESIS, binding
from storage import digest, load, mkdir, save


def configure(args):
    directory = args.directory.resolve()
    mkdir(directory)
    cfgfile = directory / "config.json"
    root = args.capture_root.resolve()
    if not (root / "signature").exists():
        if not args.difftest:
            raise ValueError(
                "capture index is missing; pass --difftest to generate a fixture"
            )
        subprocess.run([args.difftest, "relay-fixture", root], check=True)
    signature = (root / "signature").read_text().strip()
    meta = load(root / f"deposit-{signature}" / "source.json")
    route = {
        "route_id": "solana-local-usdc-v1",
        "chain_id": PFTL_CHAIN,
        "genesis_hash": PFTL_GENESIS,
        "vault_program": meta["program_id"],
        "solana_genesis_hash": meta.get("solana_genesis_hash", "00" * 32),
    }
    route_binding = binding(route)
    if meta.get("source") != "agave-differential" and (
        meta.get("route_binding") != route_binding
        or meta.get("route_id") != route["route_id"]
        or meta.get("genesis_hash") != route["genesis_hash"]
        or meta.get("chain_id") != route["chain_id"]
    ):
        raise ValueError(
            "captured vault was not initialized with this local route binding"
        )
    if cfgfile.exists():
        cfg = load(cfgfile)
        previous_signature = (
            (Path(cfg["capture_root"]) / "signature").read_text().strip()
        )
        if cfg.get("route_binding") != route_binding or previous_signature != signature:
            raise ValueError(
                "run directory belongs to a different capture or route binding"
            )
        return cfgfile
    slot = root / "archive" / str(meta["slot"])
    context = (slot / "validators").read_bytes()
    set_hash = hashlib.sha256(
        b"postfiat.pfusdc.solana.validator_set.v1" + context
    ).hexdigest()
    shred_version = int.from_bytes((slot / "shreds").read_bytes()[129:131], "little")
    anchor = (
        load(root / "anchor.json")
        if (root / "anchor.json").exists()
        else {
            "epoch": 0,
            "set_hash": set_hash,
            "total_stake": str(int.from_bytes(context[:16], "little")),
            "keys": [],
        }
    )

    key = directory / "stub-signer.pem"
    if not key.exists():
        subprocess.run(
            ["openssl", "genpkey", "-algorithm", "ED25519", "-out", key], check=True
        )
    manifest = load(args.program_info)
    programs = {}
    names = {"deposit": "deposit", "key_table": "keys", "handoff": "handoff"}
    for entry in manifest["programs"]:
        name = names[entry["kind"]]
        elf = args.program_info.parent.joinpath(entry["elf_path"]).resolve()
        if digest(elf.read_bytes()) != entry["elf_sha256"]:
            raise ValueError("ELF differs from program-info.json")
        programs[name + "_vkey"] = entry["program_vkey"]
        programs[name + "_elf"] = str(elf)
    token = directory / "http.token"
    if not token.is_file():
        import secrets

        save(token, secrets.token_hex(32).encode())
    save(
        cfgfile,
        {
            "state": str(directory / "state"),
            "host": str(args.host.resolve()),
            "http_token_file": str(token),
            "capture_root": str(root),
            "ledger": "stub",
            "prover": args.prover,
            "archive": str(root / "archive"),
            "capture_binary": str(args.capture_binary.resolve(strict=True)),
            **programs,
            "key_file": str(key),
            **route,
            "route_binding": route_binding,
            "recorder_program": "11" * 32,
            "profile_hash": "ab" * 48,
            "submitter": "pf" + "05" * 20,
            "anchor": anchor,
            "stub_recorder_pins": {
                "shred_version": shred_version,
                "slots_per_epoch": 64
                if meta.get("source") == "agave-differential"
                else 2048,
                "warmup": False,
            },
            "stub_deposit_pins": {
                "set_hash": set_hash,
                "total_stake": str(int.from_bytes(context[:16], "little")),
                "route_binding": route_binding,
                "shred_version": shred_version,
            },
        },
    )
    return cfgfile
