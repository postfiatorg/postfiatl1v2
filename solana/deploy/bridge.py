#!/usr/bin/env python3
"""Deploy and read back the pfUSDC programs over Solana JSON-RPC."""
import argparse
import base64
import fcntl
import hashlib
import json
import os
from pathlib import Path
import struct
import time
from urllib.parse import urlparse

import requests
from solders.hash import Hash
from solders.instruction import AccountMeta, Instruction
from solders.pubkey import Pubkey
from solders.system_program import create_account, ID as SYSTEM
from solders.transaction import Transaction

from keys import DIRECTORY, load, persistent
from route_binding import route_binding

ROOT = Path(__file__).resolve().parents[2]
IMAGES = Path(os.environ.get("SOLANA_BRIDGE_IMAGES", ROOT / "target/task-k/images"))
pk = Pubkey.from_string
LOADER = pk("BPFLoaderUpgradeab1e11111111111111111111111")
TOKEN = pk("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
VOTE = pk("Vote111111111111111111111111111111111111111")
MINTS = {
    "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG": "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU",
    "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
}
PER_CAP, TOTAL_CAP = 25_000_000, 500_000_000


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def pda(program, *seeds):
    return Pubkey.find_program_address(list(seeds), program)[0]


def rw(key, signer=False):
    return AccountMeta(key, signer, True)


def ro(key, signer=False):
    return AccountMeta(key, signer, False)


class Cluster:
    def __init__(self, args):
        self.rpc = args.rpc
        self.http = requests.Session()
        self.payer = load(args.payer)
        self.owner = self.payer.pubkey()
        self.genesis = self.call("getGenesisHash")
        self.log = args.log or DIRECTORY / f"{self.genesis}.jsonl"
        self.level = {"commitment": "confirmed"}

    def call(self, method, *params):
        for attempt in range(5):
            response = self.http.post(self.rpc, json={"jsonrpc": "2.0", "id": 1,
                                      "method": method, "params": params}, timeout=30)
            if response.status_code != 429:
                break
            time.sleep(min(20, 2 ** attempt))
        require(response.ok, f"{method} HTTP {response.status_code}")
        msg = response.json()
        require("error" not in msg, f"{method}: {msg.get('error')}")
        return msg["result"]

    def event(self, **record):
        line = json.dumps(dict(genesis=self.genesis, time=int(time.time()), **record), sort_keys=True)
        with self.log.open("a") as output:
            print(line, file=output, flush=True)
            os.fsync(output.fileno())
        print(line, flush=True)

    def account(self, key):
        account = self.call("getAccountInfo", str(key), {"encoding": "base64", **self.level})["value"]
        if account:
            account["data"] = base64.b64decode(account["data"][0], validate=True)
        return account

    def rent(self, size):
        return self.call("getMinimumBalanceForRentExemption", size)

    def send(self, label, instructions, *signers, wait=True):
        # Public RPC pools answer from different nodes; a fresh blockhash can be unknown to
        # the node that simulates. Nothing was broadcast in that case, so sign again.
        for attempt in range(8):
            block = self.call("getLatestBlockhash", self.level)["value"]["blockhash"]
            tx = Transaction.new_signed_with_payer(instructions, self.owner, [self.payer, *signers],
                                                   Hash.from_string(block))
            sig = str(tx.signatures[0])
            self.event(action=label, signature=sig, phase="signed")
            try:
                result = self.call("sendTransaction", base64.b64encode(bytes(tx)).decode(),
                                   {"encoding": "base64", "preflightCommitment": "confirmed", "maxRetries": 8})
                break
            except RuntimeError as error:
                if "BlockhashNotFound" not in str(error) or attempt == 7:
                    raise
                time.sleep(2)
        require(result == sig, "RPC returned a different signature")
        if wait:
            self.confirm([(label, sig)])
        return sig

    def confirm(self, transactions):
        pending = dict((sig, label) for label, sig in transactions)
        deadline = time.monotonic() + 150
        while pending:
            require(time.monotonic() < deadline, f"confirmation timeout {list(pending)}")
            signatures = list(pending)
            results = self.call("getSignatureStatuses", signatures)["value"]
            for sig, result in zip(signatures, results):
                if not result or result.get("confirmationStatus") not in ("confirmed", "finalized"):
                    continue
                tx = self.call("getTransaction", sig, {**self.level, "maxSupportedTransactionVersion": 0})
                if not tx:
                    continue
                meta = tx["meta"]
                self.event(action=pending.pop(sig), signature=sig, slot=tx["slot"], err=meta["err"],
                           fee_lamports=meta["fee"],
                           payer_debit_lamports=meta["preBalances"][0] - meta["postBalances"][0])
                require(meta["err"] is None, f"failed transaction {sig}")
            if pending:
                time.sleep(0.35)


def deploy(c, name, key, *, previous_image=None):
    program = key.pubkey()
    artifact = IMAGES / f"pfusdc_{name}.so"
    image = artifact.read_bytes()
    digest = hashlib.sha256(image).hexdigest()
    require(f"{digest}  {artifact.name}" in (IMAGES / "SHA256SUMS").read_text().splitlines(),
            f"image hash mismatch for {name}")
    data_key = pda(LOADER, bytes(program))
    existing = c.account(data_key)
    def matches(raw, expected):
        return raw[:len(expected)] == expected and not any(raw[len(expected):])
    upgrading = existing is not None and not matches(existing["data"][45:],image)
    if upgrading:
        require(previous_image is not None and matches(existing["data"][45:],previous_image)
                and existing["data"][12:45]==b"\1"+bytes(c.owner),"unexpected existing program version or authority")
    if not c.account(program) or upgrading:
        suffix = "-"+digest[:16] if upgrading else ""
        buffer = persistent(f"{c.genesis}-{name}-buffer"+suffix)
        address = buffer.pubkey()
        def create(address, size):
            return create_account(dict(from_pubkey=c.owner, to_pubkey=address,
                                       lamports=c.rent(size), space=size, owner=LOADER))
        if not c.account(address):
            c.send(f"{name}.buffer", [create(address, 37 + len(image)),
                   Instruction(LOADER, struct.pack("<I", 0), [rw(address), ro(c.owner)])], buffer)
        stored = c.account(address)
        require(stored["owner"] == str(LOADER) and stored["data"][:37] ==
                struct.pack("<IB", 1, 1) + bytes(c.owner) and len(stored["data"]) == 37 + len(image),
                "buffer header, authority or length mismatch")
        pending = []
        for offset in range(0, len(image), 960):
            chunk = image[offset:offset + 960]
            if stored["data"][37 + offset:37 + offset + len(chunk)] == chunk:
                continue
            ix = Instruction(LOADER, struct.pack("<IIQ", 1, offset, len(chunk)) + chunk,
                             [rw(address), ro(c.owner, True)])
            label = f"{name}.write.{offset}"
            pending.append((label, c.send(label, [ix], wait=False)))
            if len(pending) == 8:
                c.confirm(pending)
                pending.clear()
        c.confirm(pending)
        require(c.account(address)["data"][37:] == image, "uploaded image mismatch")
        ix = Instruction(LOADER, struct.pack("<IQ", 2, len(image)), [
            rw(c.owner, True), rw(data_key), rw(program), rw(address),
            ro(pk("SysvarRent111111111111111111111111111111111")),
            ro(pk("SysvarC1ock11111111111111111111111111111111")), ro(SYSTEM), ro(c.owner, True)])
        if upgrading:
            # SIMD-0433 resizes ProgramData during Upgrade on the Alpenglow runtime.
            ix=Instruction(LOADER,struct.pack("<I",3),[rw(data_key),rw(program),rw(address),rw(c.owner,True),
                           ro(pk("SysvarRent111111111111111111111111111111111")),
                           ro(pk("SysvarC1ock11111111111111111111111111111111")),ro(c.owner,True)])
            c.send(f"{name}.upgrade",[ix])
        else:
            c.send(f"{name}.deploy", [create(program, 36), ix], key)
    program_account, data = c.account(program), c.account(data_key)
    require(program_account["executable"] and program_account["owner"] == str(LOADER)
            and program_account["data"] == struct.pack("<I", 2) + bytes(data_key)
            and data["owner"] == str(LOADER) and data["data"][:4] == struct.pack("<I", 3)
            and (data["data"][12] == 0 or data["data"][12:45] == b"\1" + bytes(c.owner))
            and matches(data["data"][45:],image),
            f"{name} deployed bytes or upgrade authority mismatch")
    c.event(program=name, address=str(program), sha256=digest, upgrade_authority=str(c.owner) if data["data"][12] else None)


def initialize(c, vault, mint, route):
    program = vault.pubkey()
    config, custody = pda(program, b"config"), pda(program, b"custody")
    params = route + struct.pack("<QQ", PER_CAP, TOTAL_CAP)
    if not c.account(config):
        ix = Instruction(program, b"\0" + params, [rw(c.owner, True), ro(program, True),
                         rw(config), rw(custody), ro(mint), ro(TOKEN), ro(SYSTEM)])
        c.send("vault.initialize", [ix], vault)
    cfg, token = c.account(config), c.account(custody)
    require(cfg["owner"] == str(program) and len(cfg["data"]) == 96
            and cfg["data"][:80] == bytes(mint) + params, "configuration mismatch")
    require(token["owner"] == str(TOKEN) and len(token["data"]) == 165
            and token["data"][:64] == bytes(mint) + bytes(config) and token["data"][108] == 1,
            "custody mismatch")
    c.event(config=str(config), custody=str(custody), mint=str(mint), route=route.hex(),
            per_deposit_atoms=PER_CAP, total_cap_atoms=TOTAL_CAP)


def deposit(c, vault, mint, args):
    require(c.genesis != "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d", "test deposit requires a test cluster")
    nonce = bytes.fromhex(args.nonce)
    recipient = bytes.fromhex(args.recipient)
    require(len(nonce) == 32 and len(recipient) == 20, "invalid nonce or PFTL recipient")
    config, custody = pda(vault, b"config"), pda(vault, b"custody")
    receipt = pda(vault, b"receipt", nonce)
    source = pk(args.source)
    ix = Instruction(vault, b"\1" + struct.pack("<Q", 1) + recipient + nonce,
                     [rw(c.owner, True), rw(source), rw(config), rw(custody), ro(mint),
                      rw(receipt), ro(TOKEN), ro(SYSTEM)])
    if not c.account(receipt):
        c.send("vault.deposit.1-atom", [ix])
    cfg, record = c.account(config), c.account(receipt)
    require(record["owner"] == str(vault) and record["data"] ==
            b"PFSOLRC1" + cfg["data"][32:64] + recipient + struct.pack("<Q", 1) + nonce + bytes(c.owner),
            "unexpected deposit receipt")
    c.event(receipt=str(receipt), amount_atoms=1, depositor=str(c.owner))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("command", choices=["deploy", "initialize", "setup", "local-setup", "record", "epoch", "audit-deposit", "deposit"])
    p.add_argument("--rpc", required=True)
    p.add_argument("--payer", type=Path, required=True)
    p.add_argument("--log", type=Path, help="append-only transaction ledger, defaults to per-genesis file")
    p.add_argument("--mint", help="defaults to Circle USDC for public clusters")
    p.add_argument("--route", help="optional cross-check of the computed route binding, in hex")
    p.add_argument("--source", help="depositor's USDC token account")
    p.add_argument("--signature", help="landed deposit transaction for accounting")
    p.add_argument("--nonce", help="32-byte deposit nonce, hex")
    p.add_argument("--recipient", help="20-byte PFTL recipient, hex")
    args = p.parse_args()
    vault, recorder = (load(DIRECTORY / f"{name}.json") for name in ("vault", "recorder"))
    c = Cluster(args)
    with (DIRECTORY / f"{c.genesis}.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if args.command == "epoch":
            print(c.call("getEpochInfo", c.level)["epoch"])
            return
        if args.command == "audit-deposit":
            c.confirm([("vault.deposit.1-atom", args.signature)])
            return
        local = args.command == "local-setup"
        if local:
            require(urlparse(args.rpc).hostname in ("localhost", "127.0.0.1", "::1"), "loopback RPC required")
        mint = MINTS.get(c.genesis) or args.mint
        if args.mint and c.genesis in MINTS:
            require(args.mint == mint, "expected Circle USDC")
        if args.command in ("setup", "local-setup", "initialize"):
            require(mint, "initialize needs the cluster mint")
            local_route = local or (c.genesis not in MINTS and urlparse(args.rpc).hostname in
                                    ("localhost", "127.0.0.1", "::1"))
            route = route_binding(c.genesis, vault.pubkey(), local=local_route)
            if args.route is not None:
                require(bytes.fromhex(args.route.removeprefix("0x")) == route, "route binding mismatch")
            mint_account = c.account(pk(mint))
            require(mint_account and mint_account["owner"] == str(TOKEN)
                    and len(mint_account["data"]) == 82 and mint_account["data"][44:46] == b"\x06\x01",
                    "invalid six-decimal SPL mint")
        if args.command in ("setup", "local-setup", "deploy"):
            deploy(c, "vault", vault)
            deploy(c, "recorder", recorder)
        if args.command in ("setup", "local-setup", "initialize"):
            initialize(c, vault, pk(mint), route)
        if args.command in ("record", "local-setup"):
            from recorder import record
            record(c, recorder.pubkey())
        if args.command == "deposit":
            require(all([mint, args.nonce, args.source, args.recipient]), "missing deposit arguments")
            deposit(c, vault.pubkey(), pk(mint), args)


if __name__ == "__main__":
    main()
