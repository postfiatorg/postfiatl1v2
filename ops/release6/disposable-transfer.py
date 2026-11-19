#!/usr/bin/env python3
"""Certify a transfer and optionally a pinned Solana deposit with a fresh, isolated committee."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

binary = "/opt/postfiat-node"
def read_public_record(path):
    return json.JSONDecoder().raw_decode(path.read_text())[0]


def write_json(path, value):
    path.write_text(json.dumps(value))
    return path


def node(*args):
    proc = subprocess.run([binary, *map(str, args)], capture_output=True, text=True)
    if proc.returncode:
        raise RuntimeError(proc.stderr.splitlines()[0])
    return json.loads(proc.stdout)


def deposit(fixture, chain, work, data, certify):
    store = data[0]
    issuer = read_public_record(store/"faucet_account.json")["address"]
    validators = ",".join(path.name for path in data)
    bootstrap = work/"bootstrap"
    result = node("vault-bridge-bootstrap-bundle", "--pftl-chain-id", chain, "--source-chain-id", 1,
                  "--vault-address", "0x"+"12"*20, "--token-address", "0x"+"34"*20, "--issuer", issuer,
                  "--asset-code", "PFUSDC", "--asset-precision", 6, "--valuation-unit", "USDC",
                  "--valuation-policy-hash", "ab"*48, "--bundle", bootstrap)

    def asset(height, operation):
        quote = node("asset-fee-quote", "--data-dir", store, "--source", issuer, "--operation-json", json.dumps(operation))
        quote_file = write_json(work/f"quote-{height}.json", quote)
        tx = node("wallet-sign-asset-transaction", "--key-file", store/"faucet_key.json", "--quote-file", quote_file)
        signed = write_json(work/f"signed-{height}.json", tx)
        batch = work/f"batch-{height}.json"
        node("signed-asset-batch", "--data-dir", store, "--batch-file", batch, "--signed-asset-transaction-files", signed)
        return certify(height, "transparent", batch)

    for height, op in enumerate(["profile-register", "asset-create", "nav-asset-register"], 2):
        asset(height, json.loads((bootstrap/f"{op}.operation.json").read_text()))

    def authorize(height, amendment):
        files = []
        for path in data:
            auth = work/f"auth-{height}-{path.name}.json"
            node("governance-authorization-sign", "--data-dir", path, "--amendment-file", amendment,
                 "--validator", path.name, "--validator-key-file", path/"validator_keys.json",
                 "--proposal-slot", height, "--expires-at-height", height+10, "--authorization-file", auth)
            files.append(str(auth))
        signed = work/f"signed-amendment-{height}.json"
        node("governance-amendment-assemble", "--data-dir", store, "--amendment-file", amendment,
             "--authorization-files", ",".join(files), "--proposal-slot", height, "--output", signed)
        return signed

    for height, name in [(5,"vault-bridge-route-authority"), (6,"solana-bridge")]:
        amendment = work/f"amendment-{height}.json"
        node(f"ratify-{name}-activation-height", "--data-dir", store, "--validators", validators,
             "--height", 7, "--amendment-file", amendment)
        batch = work/f"batch-{height}.json"
        node("governance-batch", "--data-dir", store, "--amendment-file", authorize(height, amendment),
             "--batch-file", batch)
        certify(height, "governance", batch)
    profile = json.loads((fixture/"profile.json").read_text())
    profile.update(asset_id=result["asset_id"], activation_height=7)
    profile_file = write_json(work/"route.json", profile)
    amendment = work/"route-amendment.json"
    node("vault-bridge-route-profile-governance", "--data-dir", store, "--validators", validators,
                 "--profile-file", profile_file, "--amendment-file", amendment, "--batch-file", work/"draft.json")
    route_hash = json.loads(amendment.read_text())["kind"].split(":")[-1]
    batch = work/"route-batch.json"
    node("vault-bridge-route-profile-governance-assemble", "--data-dir", store, "--profile-file", profile_file,
         "--signed-amendment-file", authorize(7, amendment), "--proposal-slot", 7, "--batch-file", batch)
    certify(7, "governance", batch)
    op = dict(operation="solana_bridge_proof_v1", submitter=issuer, profile_hash=route_hash,
              proof_kind=profile["verifier_kind"], proof=list((fixture/"deposit.groth16").read_bytes()),
              public_values=list((fixture/"deposit.values").read_bytes()))
    accepted = asset(8, op)
    points = set()
    for path in data:
        checked = node("verify-finalized-checkpoint", "--data-dir", path)
        receipt = node("receipts", "--data-dir", path)[-1]
        assert checked["verified"] and checked["checkpoint_height"] == 8 and receipt == accepted
        lines = node("account-lines", "--data-dir", path, "--account", "pf"+"33"*20)
        rows = [r for r in lines["lines"] if r["asset_id"] != result["asset_id"] and r["balance"] == 5_000_000]
        assert len(rows) == 1
        points.add((checked["checkpoint_block_hash"], checked["checkpoint_state_root"], rows[0]["asset_id"]))
    assert len(points) == 1
    tip, root, series = points.pop()
    print(json.dumps(dict(fresh_committee=True, nodes=6, chain=chain, height=8, tip=tip, root=root,
                          tx=accepted["tx_id"], profile=route_hash, minted_series=series, atoms=5_000_000,
                          slot=24, source_program_vkey=profile["verifier_program_vkey"])), flush=True)


def main():
    os.umask(0o077)
    if set(os.listdir("/sys/class/net")) != {"lo"}:
        raise RuntimeError("requires a loopback-only network namespace")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--deposit-fixture", type=Path)
    parser.add_argument("--fleet-public-registry", type=Path)
    args = parser.parse_args()
    fixture = args.deposit_fixture
    if bool(fixture) != bool(args.fleet_public_registry):
        parser.error("provide both the public registry and the bound deposit fixture")
    chain = "postfiat-wan-devnet-2" if fixture else "postfiat-release6-disposable"
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        data = [work / f"validator-{n}" for n in range(6)]
        common = ["--chain-id", chain, "--validators", "6", "--activation-height", "1"]
        initial = node("init-consensus-v2", *common, "--node-id", "validator-0", "--data-dir", data[0])
        if fixture:
            registry = lambda p: {v["public_key_hex"] for v in read_public_record(p)["validators"]}
            fresh = registry(data[0]/"validator_registry.json")
            original = registry(args.fleet_public_registry)
            if len(fresh) != 6 or len(original) != 6 or fresh & original:
                raise RuntimeError("expected six fresh validator identities disjoint from the fleet")
            assert initial["genesis_hash"] == "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9"
        node("snapshot-export", "--data-dir", data[0], "--snapshot-dir", work/"checkpoint")
        for path in data[1:]:
            node("snapshot-import", "--snapshot-dir", work/"checkpoint", "--node-id", path.name, "--data-dir", path)
            # Point to disposable signers generated in this namespace at bootstrap.
            (path/"validator_keys.json").symlink_to(data[0]/"validator_keys.json")
        topology = work/"topology.json"
        node("topology-consensus-v2", *common, "--base-port", "42000", "--rpc-base-port", "42100",
             "--hosts", ",".join(["127.0.0.1"]*6), "--output", topology)
        destination = "pf" + ("3" if fixture else "a")*40
        batch = work/"transfer.json"
        node("batch-transfer", "--data-dir", data[1], "--key-file", data[0]/"faucet_key.json",
             "--to", destination, "--amount", "100000", "--batch-file", batch)
        servers = []
        try:
            for path in data:
                env = {**os.environ, "POSTFIAT_PREWARM_SHIELDED_VERIFIER": "1",
                       "RAYON_NUM_THREADS": "3", "POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE": str(work/(path.name+".ready"))}
                servers.append(subprocess.Popen([binary, "transport-validator-serve", "--unsafe-devnet-file-signer",
                    "--unsafe-devnet-json-storage", "--data-dir", str(path), "--topology", str(topology),
                    "--key-file", str(path/"validator_keys.json"), "--vote-dir", str(path/"votes"),
                    "--bind-host", "127.0.0.1", "--max-connections", "100", "--timeout-ms", "90000"],
                    env=env, stdout=subprocess.DEVNULL, stderr=(work/(path.name+".log")).open("w"), text=True))
            deadline = time.monotonic() + 600
            while len(list(work.glob("*.ready"))) != 6:
                for path, proc in zip(data, servers):
                    if proc.poll() is not None:
                        log = (work/(path.name+".log")).read_text()
                        raise RuntimeError(log.split("usage:")[0][-1200:])
                if time.monotonic() > deadline:
                    raise TimeoutError("six transport listeners did not become ready")
                time.sleep(0.1)
            def certify(height, kind, batch_file):
                proposer = data[height % len(data)]
                node("transport-peer-certified-batch-round", "--data-dir", proposer, "--topology", topology,
                     "--batch-kind", kind, "--batch-file", batch_file, "--key-file", proposer/"validator_keys.json",
                     "--require-local-proposer", "--artifact-dir", work/f"round-{height}",
                     "--height", height, "--timeout-ms", "90000")
                recent = node("receipts", "--data-dir", proposer)[-1]
                if not recent["accepted"]:
                    raise RuntimeError(f"height {height}: {recent['code']} {recent['message']}")
                return recent

            certify(1, "transparent", batch)
            points = []
            for path in data:
                status = node("status", "--data-dir", path)
                checked = node("verify-finalized-checkpoint", "--data-dir", path)
                receipt, = node("receipts", "--data-dir", path)
                assert checked["verified"] and checked["checkpoint_height"] == 1 and receipt["accepted"]
                points.append((status["block_height"], status["block_tip_hash"], status["state_root"], receipt["tx_id"]))
            assert len(set(points)) == 1
            print(json.dumps(dict(chain=chain, revision=initial["build_git_revision"], replicas=6,
                                  height=points[0][0], tip=points[0][1], root=points[0][2],
                                  tx=points[0][3], atoms=100000, quorum=checked["quorum"])))
            if fixture:
                deposit(fixture, chain, work, data, certify)
        finally:
            for proc in servers:
                proc.terminate()
            for proc in servers:
                try:
                    proc.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()


if __name__ == "__main__":
    main()
