"""LOCAL-ONLY rehearsal of the pause-key cancel, with a TEST pause key. Never the real state.

The dumped mainnet light names the owner's pause authority, which is not held here. This
rehearsal loads a COPY of the dumped light in which exactly bytes 200..232 (the 32-byte
`pause_authority`, after vault_program 0..32, vault_route_binding 32..64, vault_alias 64..84,
mint_alias 84..104, chain 104..136, route 136..168 and solana_genesis 168..200) are replaced
by a fresh test key; every other byte is as dumped. The copy is written only into the work
directory, as `light-LOCAL-TEST-PAUSE-KEY.json`, and the work directory must lie outside the
repository and every deploy path. Cases use the real proofs of the 23 November withdrawals.

  python3 pause_cancel_rehearsal_LOCAL_TEST_KEY.py --snapshot SNAP --work SCRATCH --old OLD.so \\
      --new NEW.so --tools AGAVE_BIN --config ops/sol-live/fixtures/exit-setup.json
"""

import argparse
import base64
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path

from solders.instruction import AccountMeta, Instruction
from solders.keypair import Keypair
from solders.pubkey import Pubkey

from upgrade_rehearsal import (
    DEPLOYED_SHA256,
    LIGHT,
    LOADER,
    VAULT,
    VERIFIER,
    Local,
    migrate,
    verdict,
    verifier_units,
    withdrawal,
)

PAUSE_AUTHORITY = slice(200, 232)
REPO = Path(__file__).resolve().parents[3]


def scratch(work):
    path = Path(os.path.realpath(work))
    for forbidden in (REPO, Path("/opt/postfiat"), Path("/etc/postfiat"), Path("/var/lib/postfiat")):
        if path == forbidden or forbidden in path.parents:
            raise SystemExit(f"refused: {path} is under {forbidden}; this rehearsal writes only scratch files")
    for name in ("SOLANA_BRIDGE_IMAGES", "KIT"):
        if os.environ.get(name) and Path(os.path.realpath(os.environ[name])) in (path, *path.parents):
            raise SystemExit(f"refused: {path} is under ${name}")
    return path


def test_key_light(snap, work, pause):
    raw = json.loads((snap / "accounts" / f"{LIGHT}.json").read_text())
    data = bytearray(base64.b64decode(raw["account"]["data"][0]))
    assert len(data) == 289
    dumped = bytes(data)
    data[PAUSE_AUTHORITY] = bytes(pause)
    assert [i for i in range(289) if data[i] != dumped[i]] and all(
        200 <= i < 232 for i in range(289) if data[i] != dumped[i])
    raw["account"]["data"] = [base64.b64encode(bytes(data)).decode(), "base64"]
    path = work / "light-LOCAL-TEST-PAUSE-KEY.json"
    path.write_text(json.dumps(raw))
    return path


def run(args):
    if hashlib.sha256(Path(args.old).read_bytes()).hexdigest() != DEPLOYED_SHA256:
        raise SystemExit("--old is not the deployed verifier")
    work = scratch(args.work)
    work.mkdir(parents=True, exist_ok=True)
    snap = args.snapshot
    txs = json.loads((snap / "verifier-txs.json").read_text())
    paid = [t for t in txs if t["tag"] == 2]
    payer, authority, pause = Keypair(), Keypair(), Keypair()
    for name, key in (("payer", payer), ("authority", authority)):
        (work / f"{name}.json").write_text(json.dumps(list(bytes(key))))
    # Burns 2 and 3 stay unpaid locally; burn 1 keeps its mainnet marker (paid).
    skip = {paid[1]["accounts"][6], paid[2]["accounts"][6], str(LIGHT)}
    accounts = ["--account", str(LIGHT), str(test_key_light(snap, work, pause.pubkey()))]
    for path in sorted((snap / "accounts").iterdir()):
        if path.stem not in skip:
            accounts += ["--account", path.stem, str(path)]
    buffers = {}
    for i, t in enumerate(paid):
        key = Keypair().pubkey()
        buffers[t["sig"]] = key
        data = bytes.fromhex(t["buffer"])
        f = work / f"buffer-{i}.json"
        f.write_text(json.dumps({"pubkey": str(key), "account": {
            "lamports": 20_000_000, "data": [base64.b64encode(data).decode(), "base64"], "owner": str(VERIFIER),
            "executable": False, "rentEpoch": 0, "space": len(data)}}))
        accounts += ["--account", str(key), str(f)]
    port = args.port
    validator = subprocess.Popen(
        [str(args.tools / "solana-test-validator"), "--reset", "--quiet", "--ledger", str(work / "ledger"),
         "--rpc-port", str(port), "--faucet-port", str(port + 10), "--mint", str(payer.pubkey()),
         "--upgradeable-program", str(VERIFIER), str(args.old), str(authority.pubkey()),
         "--upgradeable-program", str(VAULT), str(snap / "vault.so"), "none", *accounts],
        stdout=subprocess.DEVNULL, stderr=open(work / "validator.err", "w"))
    local = Local(f"http://127.0.0.1:{port}", args.tools, work)
    results = []

    def case(name, outcome, expected, **extra):
        row = dict(group="LOCAL TEST PAUSE KEY", case=name, outcome=outcome, expected=expected,
                   passed=outcome == expected, **extra)
        results.append(row)
        print(json.dumps(row), flush=True)

    def cancel(t, signer, send=False):
        ix, keys = withdrawal(t, payer, buffers[t["sig"]])
        metas = list(ix.accounts[:7]) + [AccountMeta(signer.pubkey(), True, False)]
        tx = local.tx([Instruction(VERIFIER, b"\x0a" + bytes.fromhex(t["data"])[1:], metas)], payer, signer,
                      limit=400_000)
        r = local.send(tx) if send else local.simulate(tx)
        return verdict(r), (r["units"] if send else r.get("unitsConsumed")), verifier_units(r.get("logs"))

    def pay(t):
        ix, _ = withdrawal(t, payer, buffers[t["sig"]])
        return verdict(local.simulate(local.tx([ix], payer, limit=400_000)))

    def marker_byte(t):
        from upgrade_rehearsal import fields

        burn = fields(bytes.fromhex(t["buffer"]))[23]  # field 24, the burn ID
        key = Pubkey.find_program_address([b"egress-burn", hashlib.sha256(burn).digest()], VERIFIER)[0]
        data = local.account(key)
        return None if data is None else data[0]

    try:
        for _ in range(120):
            try:
                if local.call("getHealth") == "ok":
                    break
            except OSError:
                pass
            time.sleep(1)
        time.sleep(3)
        local.call("requestAirdrop", str(authority.pubkey()), 1_000_000_000)
        local.call("requestAirdrop", str(pause.pubkey()), 1_000_000_000)
        for _ in range(60):
            balances = [local.call("getBalance", str(k.pubkey()), {"commitment": "confirmed"})["value"]
                        for k in (authority, pause)]
            if all(balances):
                break
            time.sleep(0.5)
        programdata = Pubkey.find_program_address([bytes(VERIFIER)], LOADER)[0]
        light = local.account(LIGHT)
        case("the local light differs from the dump only in bytes 200..232",
             bytes(light[PAUSE_AUTHORITY]) == bytes(pause.pubkey()), True)
        image = Path(args.new).stat().st_size
        local.cli("program", "extend", str(VERIFIER), str(max(image - (len(local.account(programdata)) - 45), 10240)),
                  "-k", str(work / "authority.json"), "--payer", str(work / "payer.json"))
        buf = json.loads(local.cli("program", "write-buffer", str(args.new), "-k", str(work / "payer.json"),
                                   "--buffer-authority", str(work / "authority.json"), "--output", "json"))["buffer"]
        local.cli("program", "upgrade", buf, str(VERIFIER), "-k", str(work / "payer.json"),
                  "--upgrade-authority", str(work / "authority.json"))
        time.sleep(10)
        args_bin = migrate(json.loads(args.config.read_text()))
        r = local.send(local.tx([Instruction(VERIFIER, b"\x09" + args_bin, [
            AccountMeta(authority.pubkey(), True, False), AccountMeta(LIGHT, False, True),
            AccountMeta(programdata, False, False)])], payer, authority))
        case("upgrade and migrate", verdict(r), "ok")
        v, units, own = cancel(paid[1], pause, send=True)
        case("the test pause key cancels unpaid burn 2 (real proof)", dict(v=v, marker=marker_byte(paid[1])),
             dict(v="ok", marker=2), units=units, verifier_units=own)
        case("payout of the cancelled burn 2 refused (real proof)", pay(paid[1]), "Replay")
        case("cancel of burn 1, paid on mainnet, refused (real proof)", cancel(paid[0], pause)[0], "Replay")
        r = local.send(local.tx([Instruction(VERIFIER, bytes([4, 1]), [
            AccountMeta(pause.pubkey(), True, False), AccountMeta(LIGHT, False, True)])], payer, pause))
        case("the test pause key pauses the verifier", verdict(r), "ok")
        case("cancel of unpaid burn 3 while paused refused (real proof)", cancel(paid[2], pause)[0], "Paused")
        case("burn 3 has no marker", marker_byte(paid[2]), None)
    finally:
        validator.terminate()
        validator.wait(timeout=30)
    out = work / "pause-cancel-LOCAL-TEST-KEY-results.json"
    out.write_text(json.dumps(results, indent=1))
    failed = [r["case"] for r in results if not r["passed"]]
    print(json.dumps(dict(group="LOCAL TEST PAUSE KEY", cases=len(results), failed=failed, results=str(out))))
    return 1 if failed else 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for name in ("snapshot", "work", "old", "new", "tools", "config"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--port", type=int, default=18999)
    sys.exit(run(parser.parse_args()))
