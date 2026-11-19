#!/usr/bin/env python3
"""Run the binary on a fresh six-node chain inside a network-disabled container."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

binary = "/opt/postfiat-node"
chain = "postfiat-release6-disposable"


def node(*args):
    proc = subprocess.run([binary, *map(str, args)], capture_output=True, text=True)
    if proc.returncode:
        raise RuntimeError(proc.stderr.splitlines()[0])
    return json.loads(proc.stdout)


def main():
    os.umask(0o077)
    if set(os.listdir("/sys/class/net")) != {"lo"}:
        raise RuntimeError("requires a loopback-only network namespace")
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        data = [work / f"validator-{n}" for n in range(6)]
        common = ["--chain-id", chain, "--validators", "6", "--activation-height", "1"]
        initial = node("init-consensus-v2", *common, "--node-id", "validator-0", "--data-dir", data[0])
        node("snapshot-export", "--data-dir", data[0], "--snapshot-dir", work/"checkpoint")
        for path in data[1:]:
            node("snapshot-import", "--snapshot-dir", work/"checkpoint", "--node-id", path.name, "--data-dir", path)
            # Point to disposable signers generated in this namespace at bootstrap.
            (path/"validator_keys.json").symlink_to(data[0]/"validator_keys.json")
        topology = work/"topology.json"
        node("topology-consensus-v2", *common, "--base-port", "42000", "--rpc-base-port", "42100",
             "--hosts", ",".join(["127.0.0.1"]*6), "--output", topology)
        destination = "pf" + "a"*40
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
            node("transport-peer-certified-batch-round", "--data-dir", data[1], "--topology", topology,
                 "--batch-kind", "transparent", "--batch-file", batch, "--key-file", data[1]/"validator_keys.json",
                 "--require-local-proposer", "--artifact-dir", work/"round", "--height", "1", "--timeout-ms", "90000")
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
