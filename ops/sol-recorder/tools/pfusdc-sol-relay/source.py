"""Resolve a deposit signature at finalized commitment, then obtain its block witness parts."""

import json
import subprocess
import tempfile
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from storage import AwaitingSource, Invalid, encode, load, mkdir, save

ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def base58(value, size=None, *, maximum=128):
    if not isinstance(value, str) or not value or len(value) > maximum:
        raise Invalid("invalid base58 data")
    number = 0
    for c in value:
        if c not in ALPHABET:
            raise Invalid("invalid base58 character")
        number = number * 58 + ALPHABET.index(c)
    raw = b"\0" * (len(value) - len(value.lstrip("1"))) + number.to_bytes(
        (number.bit_length() + 7) // 8, "big"
    )
    if size and len(raw) != size:
        raise Invalid("invalid base58 length")
    return raw


def rpc(config, method, params):
    req = urllib.request.Request(
        config["solana_rpc"],
        encode({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}),
        {"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=10) as response:
        payload = json.load(response)
    if "error" in payload:
        raise RuntimeError(f"Solana {method} failed: " + str(payload["error"]))
    return payload["result"]


def deposit_location(result, signature, vault):
    tx, meta = result["transaction"], result["meta"]
    if tx["signatures"][0] != signature or meta is None or meta["err"] is not None:
        raise Invalid("transaction signature or execution result differs")
    msg = tx["message"]
    loaded = meta.get("loadedAddresses") or {}
    keys = msg["accountKeys"] + loaded.get("writable", []) + loaded.get("readonly", [])
    calls = list(msg["instructions"])
    for group in meta.get("innerInstructions") or []:
        calls.extend(group["instructions"])
    matches = []
    for call in calls:
        if base58(keys[call["programIdIndex"]], 32).hex() != vault:
            continue
        # Other vault instructions in the same transaction can have arbitrary data.
        data = (
            base58(call["data"]) if call["data"] and len(call["data"]) <= 128 else b""
        )
        if len(data) != 61 or data[0] != 1 or len(call["accounts"]) != 8:
            continue
        addresses = [base58(keys[i], 32).hex() for i in call["accounts"]]
        matches.append(
            {
                "program_id": vault,
                "receipt_address": addresses[5],
                "depositor": addresses[0],
                "amount_atoms": str(int.from_bytes(data[1:9], "little")),
                "pftl_recipient": "pf" + data[9:29].hex(),
                "nonce": data[29:].hex(),
            }
        )
    if not matches:
        raise Invalid(
            "finalized transaction contains no deposit to the configured vault"
        )
    return {
        "kind": "deposit",
        "slot": result["slot"],
        "signature": signature,
        "receipts": matches,
        "signed_message": {
            "keys": msg["accountKeys"],
            "recentBlockhash": msg["recentBlockhash"],
            "lookups": msg.get("addressTableLookups") or [],
            "instructions": [
                {
                    "programIdIndex": ix["programIdIndex"],
                    "accounts": ix["accounts"],
                    "data_hex": base58(ix["data"], maximum=1800).hex()
                    if ix["data"]
                    else "",
                }
                for ix in msg["instructions"]
            ],
        },
    }


def locate(config, kind, ref, wait=True):
    if kind == "deposit" and config.get("solana_rpc"):
        params = [
            ref,
            {
                "commitment": "finalized",
                "encoding": "json",
                "maxSupportedTransactionVersion": 0,
            },
        ]
        with ThreadPoolExecutor(max_workers=2) as pool:
            first = pool.submit(rpc, config, "getTransaction", params)
            if "solana_genesis_hash" in config:
                genesis = rpc(config, "getGenesisHash", [])
                if base58(genesis, 32).hex() != config["solana_genesis_hash"]:
                    raise Invalid("Solana RPC genesis differs from configured route")
            result = first.result()
        saved = None
        if config.get("archived_transactions"):
            base58(ref, 64)
            archive = Path(config["archived_transactions"])
            mkdir(archive)
            saved = archive / (ref + ".json")
            if result is None and saved.exists():
                result = load(saved)
            location_file = archive / (ref + ".location.json")
            if result is None and location_file.exists():
                found = load(location_file)
                if (
                    found.get("signature") != ref
                    or found.get("kind") != "deposit"
                    or not found.get("signed_message")
                    or not found.get("receipts")
                    or any(
                        i.get("program_id") != config["vault_program"]
                        for i in found["receipts"]
                    )
                ):
                    raise Invalid("archived deposit identity differs")
                # build() will compare the complete signed message to the certified shreds.
                return found
        while result is None:
            if not wait:
                raise AwaitingSource
            time.sleep(0.25)
            result = rpc(config, "getTransaction", params)
        location = deposit_location(result, ref, config["vault_program"])
        if saved is not None:
            save(saved, result)
        return location
    path = Path(config["capture_root"]) / f"{kind}-{ref}" / "source.json"
    while not path.is_file():
        if not wait:
            raise AwaitingSource
        time.sleep(0.25)
    record = load(path)
    if (
        record["kind"] != kind
        or str(record["signature" if kind == "deposit" else "epoch"])
        != (ref if kind == "deposit" else ref.split("-")[0])
        or (
            kind != "deposit"
            and "-" in ref
            and str(record["slot"]) != ref.split("-")[1]
        )
    ):
        raise Invalid("capture index identity differs")
    if kind == "deposit":
        if record["program_id"] != config["vault_program"]:
            raise Invalid("capture vault differs")
        record["receipts"] = [record.copy()]
    return record


def capture(config, location, ref, job, wait=True):
    kind = location["kind"]
    if kind == "deposit" and config.get("capture_binary"):
        slot = str(location["slot"])
        if not slot.isdecimal():
            raise Invalid("invalid captured slot")
        src = Path(config["archive"]) / slot
        dst = job / "archive" / slot
        mkdir(dst)
        # The capture producer atomically publishes the rooted slot directory.
        while not src.is_dir():
            if not wait:
                raise AwaitingSource
            time.sleep(0.25)

        def copy(name):
            save(dst / name, (src / name).read_bytes())

        with ThreadPoolExecutor(max_workers=5) as pool:
            list(
                pool.map(
                    copy,
                    ("validators", "certificate", "shreds", "state", "finalized-id"),
                )
            )
        return
    if config.get("capture_command"):
        with tempfile.TemporaryDirectory(dir=job) as tmp:
            directory = Path(tmp)
            subprocess.run(
                [*config["capture_command"], kind, ref, str(location["slot"]), tmp],
                check=True,
            )
            copy_parts(directory, job)
    else:
        directory = Path(config["capture_root"]) / f"{kind}-{ref}"
        copy_parts(directory, job)


def copy_parts(directory, job):
    def copy(name):
        save(job / name, (directory / name).read_bytes())

    with ThreadPoolExecutor(max_workers=2) as pool:
        list(pool.map(copy, ["context.bin", "block.bin"]))


def authenticated_transaction(config, location, archive):
    try:
        captured = json.loads(
            subprocess.check_output(
                [
                    config["capture_binary"],
                    "transaction",
                    archive,
                    str(location["slot"]),
                    location["signature"],
                ]
            )
        )
    except subprocess.CalledProcessError as error:
        raise Invalid(
            "signature or block is absent from the finalized capture"
        ) from error
    block_id = captured.pop("block_id")
    if location.get("block_id", block_id) != block_id:
        raise Invalid("indexed block differs from finalized shreds")
    if "signed_message" in location:
        if captured != location["signed_message"]:
            raise Invalid("Solana RPC message differs from the finalized entry")
        return
    # Offline indices have no RPC execution metadata. Bind their direct vault instructions
    # to the signed static keys, data and receipt address in the actual finalized entry.
    keys = [base58(key, 32).hex() for key in captured["keys"]]
    for row in location["receipts"]:
        wanted = (
            b"\x01"
            + int(row["amount_atoms"]).to_bytes(8, "little")
            + bytes.fromhex(row["pftl_recipient"][2:])
            + bytes.fromhex(row["nonce"])
        ).hex()
        for ix in captured["instructions"]:
            accounts = ix["accounts"]
            indexes = [ix["programIdIndex"], *accounts]
            if len(accounts) != 8 or max(indexes) >= len(keys):
                continue
            if (
                keys[indexes[0]] == row["program_id"]
                and keys[accounts[0]] == row["depositor"]
                and keys[accounts[5]] == row["receipt_address"]
                and ix["data_hex"] == wanted
            ):
                break
        else:
            raise Invalid("indexed deposit differs from the finalized instruction")


def build(config, location, job):
    if location["kind"] == "deposit" and config.get("capture_binary"):
        with tempfile.TemporaryDirectory(dir=job) as tmp:
            witness = Path(tmp) / "witness.bin"
            with ThreadPoolExecutor(max_workers=2) as pool:
                membership = pool.submit(
                    authenticated_transaction, config, location, job / "archive"
                )
                subprocess.run(
                    [
                        config["capture_binary"],
                        "build",
                        job / "archive",
                        str(location["slot"]),
                        witness,
                    ],
                    check=True,
                )
                membership.result()
            save(job / "witness.bin", witness.read_bytes())
        return
    # The guest takes one vector: finality context followed by the authenticated block.
    save(
        job / "witness.bin",
        (job / "context.bin").read_bytes() + (job / "block.bin").read_bytes(),
    )


def journal(data):
    u64 = lambda pos: int.from_bytes(data[pos : pos + 8], "little")
    if data[:8] == b"PFSOLDP1" and len(data) >= 235:
        count = int.from_bytes(data[231:235], "little")
        if count == 0 or len(data) != 235 + count * 156:
            raise Invalid("malformed deposit journal")
        receipts = []
        for at in range(235, len(data), 156):
            receipts.append(
                {
                    "deposit_id": data[at : at + 32].hex(),
                    "receipt_address": data[at + 32 : at + 64].hex(),
                    "pftl_recipient": "pf" + data[at + 64 : at + 84].hex(),
                    "amount_atoms": str(u64(at + 84)),
                    "nonce": data[at + 92 : at + 124].hex(),
                    "depositor": data[at + 124 : at + 156].hex(),
                    "program_id": data[163:195].hex(),
                }
            )
        return {
            "kind": "deposit",
            "slot": u64(57),
            "block_id": data[65:97].hex(),
            "receipts": receipts,
        }
    if data[:8] not in (b"PFSOLKP1", b"PFSOLHP1", b"PFSOLHP2") or len(data) < 99:
        raise Invalid("unknown recorder journal")
    result = {
        "epoch": u64(59),
        "slot": u64(40),
        "recorder_program": data[8:40].hex(),
        "set_hash": data[67:99].hex(),
        "shred_version": int.from_bytes(data[48:50], "little"),
        "slots_per_epoch": u64(50),
        "warmup": bool(data[58]),
    }
    if data[:8] == b"PFSOLHP2" and len(data) == 179:
        return dict(
            result,
            kind="handoff",
            next_set=data[99:131].hex(),
            total_stake=u64(131),
            listed_stake=u64(139),
            table_hash=data[147:].hex(),
        )
    if data[:8] == b"PFSOLHP1" and len(data) == 171:
        return dict(
            result,
            kind="handoff",
            next_set=data[99:131].hex(),
            total_stake=u64(131),
            table_hash=data[139:].hex(),
        )
    if data[:8] == b"PFSOLKP1" and len(data) == 103 + 48 * int.from_bytes(
        data[99:103], "little"
    ):
        return dict(
            result,
            kind="keys",
            keys=[data[i : i + 48].hex() for i in range(103, len(data), 48)],
        )
    raise Invalid("malformed recorder journal")


def bind(location, public):
    fields = (
        ["kind", "slot", "block_id"]
        if public["kind"] == "deposit"
        else ["kind", "slot", "epoch", "recorder_program"]
    )
    if any(location[k] != public.get(k) for k in fields if k in location):
        raise Invalid("proof does not match the resolved block")
    selected = []
    for expected in location.get("receipts", []):
        matches = [
            r
            for r in public["receipts"]
            if all(
                r[k] == expected[k]
                for k in (
                    "receipt_address",
                    "pftl_recipient",
                    "depositor",
                    "nonce",
                    "amount_atoms",
                    "program_id",
                )
            )
        ]
        if len(matches) != 1:
            raise Invalid("proven receipt does not match the signed vault instruction")
        selected.extend(matches)
    return selected
