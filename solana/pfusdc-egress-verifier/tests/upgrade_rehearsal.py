"""Rehearse the exit verifier's upgrade and route migration on a local validator.

`fetch` reads the deployed verifier, its accounts, the three paid withdrawals of 23 November
and the vault accounts they touch from a public mainnet RPC (read-only calls only).
`run` starts solana-test-validator from that state with the dumped program under a local
upgrade authority, then upgrades, migrates and replays withdrawals. Nothing is sent to any
public cluster. Prints one JSON line per case.
"""
import argparse
import base64
import hashlib
import json
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

from solders.compute_budget import set_compute_unit_limit
from solders.hash import Hash
from solders.instruction import AccountMeta, Instruction
from solders.keypair import Keypair
from solders.message import Message
from solders.pubkey import Pubkey
from solders.transaction import Transaction

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from bootstrap import migrate  # noqa: E402

VERIFIER = Pubkey.from_string("8Dxp8bfgEh6ankapfBdkTds5cebqrtPdFxpZSP6JsGAF")
# The deployed verifier's code (ProgramData tail), byte-identical to main's build.
DEPLOYED_SHA256 = "3b16fc309501814e0bba718ecadb80f02993f6f355153a13f37ae9d47d29354c"
LIGHT = Pubkey.from_string("FQe34dCQg9m6yEW6NjbjoR8LfWwKGLX2hBvTdY2Lu6s1")
VAULT = Pubkey.from_string("GFkpeTibgdRAAAndgUMYsJnSTck91tUbrpuWBWan6z1G")
LOADER = Pubkey.from_string("BPFLoaderUpgradeab1e11111111111111111111111")
SYSTEM = Pubkey.from_string("11111111111111111111111111111111")
VAULT_ACCOUNTS = [
    "JByQZxACTnFXQDGLBUKAfTysSuqtpoLHoAWqUh4ri6WG",  # config
    "AhmUaCijypvcbYMDxeXVbUPqjPX1tiHfLH6NWkYnQTCi",  # custody
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",  # USDC mint
]
ERRORS = {1: "Encoding", 2: "Binding", 3: "Proof", 4: "StaleCheckpoint", 5: "UnknownCheckpoint",
          6: "Replay", 7: "Paused", 8: "WrongDestination", 9: "CancelNotAllowed"}
MAGIC = b"PFTL-PFUSDC-TIER4"
# Changed fields of the test-verifier cases (21 and 24 carry a fresh burn ID).
def burn_id(b):
    return {21: bytes([b]) * 48, 24: bytes([b]) * 48}


SOLANA_GENESIS = "45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0"


def destination_hash(owner):
    dest = f"solana-spl:{SOLANA_GENESIS}:{bytes(owner).hex()}"
    return hashlib.sha3_384(b"postfiat.vault_bridge.destination_ref_hash.v1\0"
                            + f"destination_ref_bytes={len(dest)}\ndestination_ref={dest}\n".encode()).digest()


R9_BUCKET = bytes.fromhex("4683dbe99fcb2aef277f095e459ff541e1ef0774222aa6c1962e091d8f18c14743b82a54518b1f7eb48c451c9c5b21ce")


def post(url, method, params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    for attempt in range(6):
        try:
            reply = json.load(urllib.request.urlopen(
                urllib.request.Request(url, body, {"content-type": "application/json"}), timeout=60))
            if "error" in reply:
                raise RuntimeError(reply["error"])
            return reply["result"]
        except (OSError, RuntimeError):
            if attempt == 5:
                raise
            time.sleep(2 + 3 * attempt)


def account_file(key, value):
    data = base64.b64decode(value["data"][0])
    return {"pubkey": key, "account": {"lamports": value["lamports"], "data": value["data"],
                                        "owner": value["owner"], "executable": value["executable"],
                                        "rentEpoch": 0, "space": len(data)}}


def fetch(args):
    """Read-only mainnet reads: getProgramAccounts, getMultipleAccounts, getAccountInfo,
    getSignaturesForAddress and getTransaction."""
    import base58

    out = args.snapshot
    (out / "accounts").mkdir(parents=True, exist_ok=True)
    rpc = args.rpc
    level = {"commitment": "finalized"}
    owned = post(rpc, "getProgramAccounts", [str(VERIFIER), dict(level, encoding="base64", withContext=True)])
    for row in owned["value"]:
        (out / "accounts" / f"{row['pubkey']}.json").write_text(json.dumps(account_file(row["pubkey"], row["account"])))
    txs = []
    for sig in reversed(post(rpc, "getSignaturesForAddress", [str(LIGHT), dict(level, limit=100)])):
        tx = post(rpc, "getTransaction", [sig["signature"], dict(level, encoding="json", maxSupportedTransactionVersion=0)])
        keys = tx["transaction"]["message"]["accountKeys"]
        for ix in tx["transaction"]["message"]["instructions"]:
            data = base58.b58decode(ix["data"])
            if keys[ix["programIdIndex"]] == str(VERIFIER):
                txs.append(dict(sig=sig["signature"], slot=sig["slot"], tag=data[0], data=data.hex(),
                                accounts=[keys[a] for a in ix["accounts"]], cu=tx["meta"]["computeUnitsConsumed"]))
    for t in txs:
        if t["tag"] not in (2, 8):
            continue
        chunks = []
        for sig in reversed(post(rpc, "getSignaturesForAddress", [t["accounts"][4], dict(level, limit=50)])):
            tx = post(rpc, "getTransaction", [sig["signature"], dict(level, encoding="json", maxSupportedTransactionVersion=0)])
            keys = tx["transaction"]["message"]["accountKeys"]
            for ix in tx["transaction"]["message"]["instructions"]:
                data = base58.b58decode(ix["data"])
                if keys[ix["programIdIndex"]] == str(VERIFIER) and data[0] == 5:
                    chunks.append((int.from_bytes(data[1:3], "little"), data[3:]))
        buffer = bytearray()
        for offset, chunk in chunks:
            buffer.extend(b"\0" * max(0, offset + len(chunk) - len(buffer)))
            buffer[offset:offset + len(chunk)] = chunk
        t["buffer"] = bytes(buffer).hex()
    (out / "verifier-txs.json").write_text(json.dumps(txs, indent=1))
    extra = VAULT_ACCOUNTS + sorted({t["accounts"][i] for t in txs if t["tag"] == 2 for i in (11, 12)})
    programdata = [Pubkey.find_program_address([bytes(p)], LOADER)[0] for p in (VERIFIER, VAULT)]
    reply = post(rpc, "getMultipleAccounts", [extra + [str(p) for p in programdata], dict(level, encoding="base64")])
    for key, value in zip(extra, reply["value"]):
        if value is not None:
            (out / "accounts" / f"{key}.json").write_text(json.dumps(account_file(key, value)))
    for name, value in zip(("verifier.so", "vault.so"), reply["value"][len(extra):]):
        (out / name).write_bytes(base64.b64decode(value["data"][0])[45:])
    print(json.dumps(dict(slot=reply["context"]["slot"], accounts=len(list((out / "accounts").iterdir())),
                          verifier_sha256=hashlib.sha256((out / "verifier.so").read_bytes()).hexdigest(),
                          vault_sha256=hashlib.sha256((out / "vault.so").read_bytes()).hexdigest())))


def light_height(raw):
    """The checkpoint height a light account stands at (u64 little-endian at byte 280)."""
    return int.from_bytes(raw[280:288], "little")


def light_check(live, snap):
    """The loaded light against the light the snapshot dumped; the checkpoint height is named so
    a light that moved since a fixture was pinned says so."""
    doc = json.loads((snap / "accounts" / f"{LIGHT}.json").read_text())
    dumped = base64.b64decode(doc["account"]["data"][0])
    return tuple(dict(sha256=hashlib.sha256(raw).hexdigest(), height=light_height(raw)) for raw in (live, dumped))


def fields(buffer):
    body = buffer[len(MAGIC) + 4 + int.from_bytes(buffer[len(MAGIC):len(MAGIC) + 4], "big"):]
    out = []
    while body:
        size = int.from_bytes(body[2:6], "big")
        out.append(body[6:6 + size])
        body = body[6 + size:]
    return out


def with_fields(buffer, changes):
    head = buffer[:len(MAGIC) + 4 + int.from_bytes(buffer[len(MAGIC):len(MAGIC) + 4], "big")]
    f = fields(buffer)
    for index, value in changes.items():
        f[index - 1] = value
    return head + b"".join((i + 1).to_bytes(2, "big") + len(v).to_bytes(4, "big") + v for i, v in enumerate(f))


class Local:
    def __init__(self, url, bin_dir, work):
        self.url, self.bin, self.work = url, bin_dir, work

    def call(self, method, *params):
        return post(self.url, method, list(params))

    def cli(self, *argv):
        done = subprocess.run([str(self.bin / "solana"), "-u", self.url, "--commitment", "confirmed", *argv],
                              capture_output=True, text=True)
        if done.returncode:
            raise RuntimeError(f"solana {' '.join(argv[:2])}: {done.stdout} {done.stderr}")
        return done.stdout

    def tx(self, instructions, payer, *signers, limit=None):
        ixs = ([set_compute_unit_limit(limit)] if limit else []) + instructions
        blockhash = Hash.from_string(self.call("getLatestBlockhash", {"commitment": "confirmed"})["value"]["blockhash"])
        return Transaction([payer, *signers], Message.new_with_blockhash(ixs, payer.pubkey(), blockhash), blockhash)

    def simulate(self, tx, watch=()):
        reply = self.call("simulateTransaction", base64.b64encode(bytes(tx)).decode(),
                          {"encoding": "base64", "sigVerify": True, "commitment": "confirmed",
                           "accounts": {"encoding": "base64", "addresses": [str(w) for w in watch]}})["value"]
        return reply

    def send(self, tx):
        sig = self.call("sendTransaction", base64.b64encode(bytes(tx)).decode(),
                        {"encoding": "base64", "skipPreflight": True})
        for _ in range(60):
            status = self.call("getSignatureStatuses", [sig])["value"][0]
            if status and status.get("confirmationStatus") in ("confirmed", "finalized"):
                meta = self.call("getTransaction", sig, {"encoding": "json", "commitment": "confirmed",
                                                         "maxSupportedTransactionVersion": 0})["meta"]
                return dict(err=status["err"], units=meta["computeUnitsConsumed"], logs=meta["logMessages"])
            time.sleep(0.5)
        raise RuntimeError("transaction not confirmed")

    def account(self, key):
        value = self.call("getAccountInfo", str(key), {"encoding": "base64", "commitment": "confirmed"})["value"]
        return None if value is None else base64.b64decode(value["data"][0])


def verdict(result):
    err = result.get("err")
    if err is None:
        return "ok"
    if isinstance(err, dict) and "InstructionError" in err:
        detail = err["InstructionError"][1]
        if isinstance(detail, dict) and "Custom" in detail:
            return ERRORS.get(detail["Custom"], f"Custom({detail['Custom']})")
        return str(detail)
    return json.dumps(err)


def verifier_units(logs):
    for line in logs or []:
        if line.startswith(f"Program {VERIFIER} consumed "):
            return int(line.split()[3])
    return None


def synthetic_key(changes):
    seed = hashlib.sha256(json.dumps(sorted((k, v.hex()) for k, v in changes.items())).encode()).digest()
    return Keypair.from_seed(seed).pubkey()


def withdrawal(t, payer, buffer_key, changes=None):
    keys = [Pubkey.from_string(k) for k in t["accounts"]]
    burn = None
    if changes:
        burn = changes.get(24)
        keys[6] = Pubkey.find_program_address([b"egress-burn", hashlib.sha256(burn).digest()], VERIFIER)[0]
    writable = {1, 3, 6, 10, 11}
    metas = [AccountMeta(payer.pubkey(), True, True)] + [
        AccountMeta(k, False, i in writable) for i, k in enumerate(keys) if i > 0]
    metas[4] = AccountMeta(buffer_key, False, False)
    return Instruction(VERIFIER, bytes.fromhex(t["data"]), metas), keys


def run(args):
    old_sha256 = hashlib.sha256(Path(args.old).read_bytes()).hexdigest()
    if old_sha256 != DEPLOYED_SHA256:
        raise SystemExit(f"--old is not the deployed verifier: sha256 {old_sha256}")
    snap, work = args.snapshot, args.work
    work.mkdir(parents=True, exist_ok=True)
    txs = json.loads((snap / "verifier-txs.json").read_text())
    paid = [t for t in txs if t["tag"] == 2]
    init = next(t for t in txs if t["tag"] == 3)
    payer, authority, local_id, stranger, owner = Keypair(), Keypair(), Keypair(), Keypair(), Keypair()
    for name, key in (("payer", payer), ("authority", authority), ("local-id", local_id)):
        (work / f"{name}.json").write_text(json.dumps(list(bytes(key))))
    # Burns two and three of 23 November stay unpaid locally: a real proof pays three once
    # and cancels two once.
    unpaid, spare = paid[2], paid[1]
    skip = {unpaid["accounts"][6], spare["accounts"][6]}
    buffers = {}
    accounts = []
    for path in sorted((snap / "accounts").iterdir()):
        if path.stem not in skip:
            accounts += ["--account", path.stem, str(path)]
    for i, t in enumerate(paid):
        key = Keypair().pubkey()
        buffers[t["sig"]] = key
        data = bytes.fromhex(t["buffer"])
        f = work / f"buffer-{i}.json"
        f.write_text(json.dumps({"pubkey": str(key), "account": {
            "lamports": 20_000_000, "data": [base64.b64encode(data).decode(), "base64"], "owner": str(VERIFIER),
            "executable": False, "rentEpoch": 0, "space": len(data)}}))
        accounts += ["--account", str(key), str(f)]
    r9 = {26: R9_BUCKET, **burn_id(0x91)}
    variants = [{26: R9_BUCKET, 21: b"\x99" * 48, 24: b"\x99" * 48}, r9, burn_id(0x92),
                {23: b"\x23" * 48, **burn_id(0x93)}, {33: b"\x33" * 20, **burn_id(0x94)},
                {34: b"\x34" * 32, **burn_id(0x95)}, {32: b"ETHEREUM", **burn_id(0x96)},
                {29: destination_hash(owner.pubkey()), **burn_id(0xB1)},
                {13: (1851 + 300).to_bytes(8, "big"), **burn_id(0xB3)}, burn_id(0xB2)]
    unpaid_values = bytes.fromhex(unpaid["buffer"])
    for i, changes in enumerate(variants):
        data = with_fields(unpaid_values, changes)
        f = work / f"variant-{i}.json"
        f.write_text(json.dumps({"pubkey": str(synthetic_key(changes)), "account": {
            "lamports": 20_000_000, "data": [base64.b64encode(data).decode(), "base64"], "owner": str(VERIFIER),
            "executable": False, "rentEpoch": 0, "space": len(data)}}))
        accounts += ["--account", str(synthetic_key(changes)), str(f)]
    port = args.port
    validator = subprocess.Popen(
        [str(args.tools / "solana-test-validator"), "--reset", "--quiet", "--ledger", str(work / "ledger"),
         "--rpc-port", str(port), "--faucet-port", str(port + 10), "--mint", str(payer.pubkey()),
         "--upgradeable-program", str(VERIFIER), str(args.old), str(authority.pubkey()),
         "--upgradeable-program", str(local_id.pubkey()), str(args.old), str(authority.pubkey()),
         "--upgradeable-program", str(VAULT), str(snap / "vault.so"), "none", *accounts,
         # A test validator activates every feature. Mainnet (24 November 2026) has SIMD-0433 and
         # SIMD-0685 inactive, so ExtendProgram still works there and an upgrade does not resize.
         "--deactivate-feature", "EhisBfVtGvEA8bVCVN5VMaYEaX6iTfoUrmcDi8LY7Kxy",
         "--deactivate-feature", "5Ach9H8V5NCNDQg7Md6HnJmVttF3tzHwiWWucnnFd4jz"],
        stdout=subprocess.DEVNULL, stderr=open(work / "validator.err", "w"))
    local = Local(f"http://127.0.0.1:{port}", args.tools, work)
    results = []

    def case(name, outcome, expected, **extra):
        row = dict(case=name, outcome=outcome, expected=expected, passed=outcome == expected, **extra)
        results.append(row)
        print(json.dumps(row), flush=True)

    try:
        for _ in range(120):
            try:
                if local.call("getHealth") == "ok":
                    break
            except OSError:
                pass
            time.sleep(1)
        time.sleep(3)
        programdata = Pubkey.find_program_address([bytes(VERIFIER)], LOADER)[0]
        local.call("requestAirdrop", str(authority.pubkey()), 1_000_000_000)
        live = local.account(LIGHT)
        case("dumped light loaded unchanged", *light_check(live, snap))

        # (a) The old program initialised exactly as on mainnet (the tag-3 arguments of the
        # 23 November initialisation), under a local program ID.
        init_args = bytes.fromhex(init["data"])
        light_pda = Pubkey.find_program_address([b"egress-light"], local_id.pubkey())[0]
        checkpoint = init_args[1 + 32 + 48 + 4 + 32 * 4:1 + 32 + 48 + 4 + 32 * 4 + 48]
        first = Pubkey.find_program_address([b"egress-checkpoint", hashlib.sha256(checkpoint).digest()], local_id.pubkey())[0]
        ix = Instruction(local_id.pubkey(), init_args, [
            AccountMeta(payer.pubkey(), True, True), AccountMeta(local_id.pubkey(), True, False),
            AccountMeta(light_pda, False, True), AccountMeta(first, False, True), AccountMeta(SYSTEM, False, False)])
        r = local.send(local.tx([ix], payer, local_id))
        fresh = local.account(light_pda)
        names = ["vault_program", "vault_route_binding", "vault_alias", "mint_alias", "chain", "route",
                 "solana_genesis", "pause_authority", "latest", "height", "paused"]
        sizes = [32, 32, 20, 20, 32, 32, 32, 32, 48, 8, 1]
        differ, at = [], 0
        for n, s in zip(names, sizes):
            if fresh[at:at + s] != live[at:at + s]:
                differ.append(n)
            at += s
        case("old program initialised as on mainnet: same layout", dict(err=verdict(r), length=len(fresh), differ=differ),
             dict(err="ok", length=289, differ=["latest", "height"]))

        def pay(t, changes=None, watch=False, send=False, limit=400_000):
            key = buffers[t["sig"]]
            if changes:
                key = synthetic_key(changes)
            ix, keys = withdrawal(t, payer, key, changes)
            tx = local.tx([ix], payer, limit=limit)
            if send:
                r = local.send(tx)
                return verdict(r), r["units"], verifier_units(r["logs"]), keys
            r = local.simulate(tx, watch=[keys[11]] if watch else ())
            return verdict(r), r.get("unitsConsumed"), verifier_units(r.get("logs")), r, keys

        def cancel(t, changes=None, signer=None, send=False):
            key = synthetic_key(changes) if changes else buffers[t["sig"]]
            ix, keys = withdrawal(t, payer, key, changes)
            who = signer.pubkey() if signer else keys[12]
            metas = list(ix.accounts[:7]) + [AccountMeta(who, signer is not None, False)]
            tx = local.tx([Instruction(VERIFIER, b"\x0a" + bytes.fromhex(t["data"])[1:], metas)], payer,
                          *([signer] if signer else []), limit=400_000)
            r = local.send(tx) if send else local.simulate(tx)
            units = r["units"] if send else r.get("unitsConsumed")
            return verdict(r), units, verifier_units(r.get("logs")), keys

        def pay_to(t, changes, owner_key):
            ix, _ = withdrawal(t, payer, synthetic_key(changes), changes)
            metas = list(ix.accounts)
            metas[12] = AccountMeta(owner_key, False, False)
            return verdict(local.simulate(local.tx([Instruction(VERIFIER, ix.data, metas)], payer, limit=400_000)))

        def marker_byte(t, burn=None):
            burn = burn or fields(bytes.fromhex(t["buffer"]))[23]
            key = Pubkey.find_program_address([b"egress-burn", hashlib.sha256(burn).digest()], VERIFIER)[0]
            data = local.account(key)
            return None if data is None else data[0]

        def ata_amount(raw):
            return int.from_bytes(raw[64:72], "little")

        # (b) The deployed program on the dumped state.
        v, units, own, r, keys = pay(unpaid, watch=True)
        before = ata_amount(local.account(keys[11]))
        after = ata_amount(base64.b64decode(r["accounts"][0]["data"][0])) if r.get("accounts") else None
        amount = int.from_bytes(fields(bytes.fromhex(unpaid["buffer"]))[26], "big")
        case("old program: real proof of burn 3 pays (simulated)", dict(v=v, paid=(after or 0) - before),
             dict(v="ok", paid=amount), units=units, verifier_units=own)
        old_units = (units, own)
        case("old program: paid burn 1 refused", pay(paid[0])[0], "Replay")
        case("old program: cancel unknown (refused before the upgrade)", cancel(spare)[0], "Encoding")
        fields_arg = migrate(json.loads(args.config.read_text()))
        migrate_ix = lambda signer: Instruction(VERIFIER, b"\x09" + fields_arg, [  # noqa: E731
            AccountMeta(signer.pubkey(), True, False), AccountMeta(LIGHT, False, True),
            AccountMeta(programdata, False, False)])
        case("old program: migrate unknown (red before the upgrade)",
             verdict(local.simulate(local.tx([migrate_ix(authority)], payer, authority))), "Encoding")

        # (c) Upgrade: extend ProgramData, write the buffer, upgrade.
        def upgrade(image):
            have = len(local.account(programdata)) - 45
            need = Path(image).stat().st_size
            grown = 0
            if need > have:
                grown = max(need - have, 10240)
                # The loader refuses an extension below 10,240 bytes.
                local.cli("program", "extend", str(VERIFIER), str(grown), "-k", str(work / "authority.json"),
                          "--payer", str(work / "payer.json"))
            buf = local.cli("program", "write-buffer", str(image), "-k", str(work / "payer.json"),
                            "--buffer-authority", str(work / "authority.json"), "--output", "json")
            buf = json.loads(buf)["buffer"]
            local.cli("program", "upgrade", buf, str(VERIFIER), "-k", str(work / "payer.json"),
                      "--upgrade-authority", str(work / "authority.json"))
            for _ in range(20):
                local.call("requestAirdrop", str(stranger.pubkey()), 1)
                time.sleep(0.5)
            return grown

        grew = upgrade(args.new)
        image = Path(args.new).read_bytes()
        tail = local.account(programdata)[45:]
        # The service pins the hash of the whole ProgramData tail: the image, zero-filled.
        case("upgrade to the new build", dict(code=hashlib.sha256(tail[:len(image)]).hexdigest(),
                                              fill=set(tail[len(image):]) <= {0}),
             dict(code=hashlib.sha256(image).hexdigest(), fill=True), extended_bytes=grew,
             programdata_bytes=len(tail) + 45, service_pin="0x" + hashlib.sha256(tail).hexdigest())
        case("new program before migrate: withdrawal refused", pay(paid[0])[0], "Binding")
        case("migrate by a wrong signer refused",
             verdict(local.send(local.tx([migrate_ix(stranger)], payer, stranger))), "MissingRequiredSignature")
        r = local.send(local.tx([migrate_ix(authority)], payer, authority))
        migrated = local.account(LIGHT)
        changed = [i for i in range(289) if migrated[i] != live[i]]
        case("migrate by the upgrade authority", dict(v=verdict(r), changed=[min(changed), max(changed) + 1],
                                                       route=migrated[136:168].hex(), length=len(migrated)),
             dict(v="ok", changed=[136, 168], route=(HERE / "route-vector.txt").read_text().split("route=")[1].strip(),
                  length=289), units=r["units"])
        case("second migrate refused", verdict(local.send(local.tx([migrate_ix(authority)], payer, authority))), "Binding")
        case("light after the failed second migrate unchanged", local.account(LIGHT) == migrated, True)

        # (d) Withdrawals after the upgrade.
        v, units, own, r, keys = pay(unpaid, watch=True)
        after = ata_amount(base64.b64decode(r["accounts"][0]["data"][0])) if r.get("accounts") else None
        case("new program: real proof of burn 3 (revision 8) pays (simulated)", dict(v=v, paid=(after or 0) - before),
             dict(v="ok", paid=amount), units=units, verifier_units=own, old_units=old_units[0],
             old_verifier_units=old_units[1])
        v, units, own, keys = pay(unpaid, send=True)
        case("new program: real proof of burn 3 pays (sent)", dict(v=v, paid=ata_amount(local.account(keys[11])) - before),
             dict(v="ok", paid=amount), units=units, verifier_units=own)
        for i in (0, 2):
            case(f"new program: paid burn {i + 1} of 23 November refused", pay(paid[i])[0], "Replay")
        case("new program: cancel after payout refused (burn 3, real proof)", cancel(unpaid)[0], "Replay")
        case("new program: cancel of burn 1 paid on mainnet refused", cancel(paid[0])[0], "Replay")
        case("new program: an unsigned cancel refused (burn 2, real proof)", cancel(spare)[0], "CancelNotAllowed")
        v, units, own, _ = cancel(spare, signer=stranger)
        case("new program: a cancel signed by a stranger refused (burn 2, real proof)", v, "CancelNotAllowed",
             units=units, verifier_units=own)
        case("new program: real proof with field 26 changed refused", pay(unpaid, variants[0])[0], "Proof")

        # Test-verifier image: the same new code with the Groth16 check removed.
        upgrade(args.test_verifier)
        for label, changes in (("revision-9", variants[1]), ("revision-8", variants[2])):
            start = ata_amount(local.account(keys[11]))
            v, units, own, _ = pay(unpaid, changes, send=True)
            case(f"test verifier: {label} bucket withdrawal pays (sent)",
                 dict(v=v, paid=ata_amount(local.account(keys[11])) - start), dict(v="ok", paid=amount),
                 units=units, verifier_units=own)
        case("test verifier: same revision-9 burn twice refused", pay(unpaid, variants[1])[0], "Replay")
        case("test verifier: another family (field 23) refused", pay(unpaid, variants[3])[0], "Binding")
        case("test verifier: another vault (field 33) refused", pay(unpaid, variants[4])[0], "Binding")
        case("test verifier: another vault code (field 34) refused", pay(unpaid, variants[5])[0], "Binding")
        case("test verifier: non-Solana chain tag (field 32) refused", pay(unpaid, variants[6])[0], "Binding")
        v, units, own, _ = cancel(unpaid, variants[7], signer=owner, send=True)
        case("test verifier: destination owner cancels at once", dict(v=v, marker=marker_byte(None, b"\xb1" * 48)),
             dict(v="ok", marker=2), units=units, verifier_units=own)
        case("test verifier: payout after the owner's cancel refused", pay_to(unpaid, variants[7], owner.pubkey()),
             "Replay")
        case("test verifier: second cancel refused", cancel(unpaid, variants[7], signer=owner)[0], "Replay")
        case("test verifier: payout proving block 2151 advances the light", pay(unpaid, variants[8], send=True)[0], "ok")
        case("test verifier: light height", int.from_bytes(local.account(LIGHT)[280:288], "little"), 2151)
        case("test verifier: a stranger cannot cancel even 300 blocks past the burn",
             cancel(unpaid, variants[9], signer=stranger)[0], "CancelNotAllowed")

        # Rollback: the deployed image on the migrated light.
        upgrade(args.old)
        case("rollback to the old program: withdrawals refused", pay(paid[0])[0], "Binding")
        upgrade(args.new)
        case("re-upgrade: paid burn refused as replay again", pay(paid[0])[0], "Replay")
        # Real proof of burn 2, now that the light stands 303 blocks past it: no block count opens a cancel.
        case("new program: 303 blocks later an unsigned cancel of burn 2 is still refused (real proof)",
             cancel(spare)[0], "CancelNotAllowed")
        case("new program: burn 2 has no marker after the refused cancels", marker_byte(spare), None)
        case("new program: burn 2 still pays after the refused cancels (real proof, simulated)", pay(spare)[0], "ok")
        case("new program: burn 3 marker stays paid", marker_byte(unpaid), 1)
    finally:
        validator.terminate()
        validator.wait(timeout=30)
    (work / "results.json").write_text(json.dumps(results, indent=1))
    failed = [r["case"] for r in results if not r["passed"]]
    print(json.dumps(dict(cases=len(results), failed=failed)))
    return 1 if failed else 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    f = sub.add_parser("fetch")
    f.add_argument("--rpc", default="https://api.mainnet-beta.solana.com")
    f.add_argument("--snapshot", type=Path, required=True)
    r = sub.add_parser("run")
    # --test-verifier: the image of tests/build-no-proof-rehearsal.sh, never a deploy image.
    for name in ("snapshot", "work", "old", "new", "test_verifier", "tools", "config"):
        r.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    r.add_argument("--port", type=int, default=18899)
    a = parser.parse_args()
    sys.exit(fetch(a) if a.command == "fetch" else run(a))
