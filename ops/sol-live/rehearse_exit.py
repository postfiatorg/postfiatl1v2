"""Exercise the HTTP worker with a signed local Post Fiat witness and an isolated Agave validator."""

from contextlib import contextmanager
import argparse
import base64
import hashlib
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import threading
import time
from types import SimpleNamespace
from unittest.mock import patch

import requests
from solders.keypair import Keypair
import exit_payout as payout
import live
from exit_service import BASE, Handler, Journal, ThreadingHTTPServer, process_next
from exit_setup import (
    GENESIS,
    LIGHT,
    MINT,
    TOKEN,
    VAULT,
    VERIFIER,
    VERIFIER_SHA256,
    checkpoint_pda,
    expected_light,
)
from bridge import Cluster, pda
from live import read, save
from route_binding import route_binding


@contextmanager
def local_post_fiat(node, evidence, directory, port):
    ready = directory / "pft-ready"
    bind = "127.0.0.1"
    with (evidence / "post-fiat-rpc.log").open("w") as log:
        proc = subprocess.Popen(
            [
                node.resolve(),
                "rpc-serve",
                "--unsafe-devnet-json-storage",
                "--data-dir",
                evidence / "local-store",
                "--bind-host",
                bind,
                "--port",
                str(port),
                "--ready-file",
                ready,
            ],
            stdout=log,
            stderr=log,
        )
        try:
            for _ in range(100):
                if proc.poll() is not None:
                    raise RuntimeError(
                        "local Post Fiat RPC exited; see post-fiat-rpc.log"
                    )
                if ready.exists():
                    break
                time.sleep(0.2)
            else:
                raise TimeoutError("local Post Fiat RPC did not start")
            with patch.object(live, "PEERS", [(bind, port)]):
                yield
        finally:
            if proc.poll() is None:
                proc.terminate()
                proc.wait(timeout=30)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--validator", required=True, type=Path)
    ap.add_argument("--node", required=True, type=Path)
    ap.add_argument("--images", required=True, type=Path)
    ap.add_argument("--evidence", required=True, type=Path)
    ap.add_argument("--rpc-port", type=int, default=21899)
    args = ap.parse_args()
    pins = {
        "pfusdc_egress_verifier.so": VERIFIER_SHA256,
        "pfusdc_vault.so": read(payout.CONFIG)["vault_code_hash"],
    }
    for image, digest in pins.items():
        with (args.images / image).open("rb") as stream:
            assert "0x" + hashlib.file_digest(stream, "sha256").hexdigest() == digest
    evidence = args.evidence.resolve()
    witness = read(evidence / "local-witness.json")
    burn = read(evidence / "local-burn.json")
    row = dict(burn["redemption"], burn_tx_id=witness["receipt"]["tx_id"])
    pk = Keypair()
    owner = payout.validate_burn(row, row["burn_tx_id"])
    config = read(payout.CONFIG)
    config.update(
        chain_id=witness["chain_id"],
        genesis_hash=witness["genesis_hash"],
        protocol_version=witness["protocol_version"],
        initial_checkpoint=witness["prior_checkpoint_block_id"],
        initial_height=row["created_at_height"] - 1,
        initial_committee=witness["block"]["header"]["consensus_v2_commit"][
            "precommit_qc"
        ]["domain"]["committee_root"],
        pause_authority=bytes(pk.pubkey()).hex(),
    )
    # The public keys of the vault and token programs match mainnet. The private
    # local genesis supplies test tokens and trusts only the local committee.
    cfg = pda(VAULT, b"config")
    custody = pda(VAULT, b"custody")
    ata = pda(payout.ATA, bytes(owner), bytes(TOKEN), bytes(MINT))
    mint = bytearray(82)
    mint[36:44] = (1_000_000).to_bytes(8, "little")
    mint[44:46] = bytes([6, 1])
    tokens = bytearray(165)
    tokens[:64] = bytes(MINT) + bytes(cfg)
    tokens[64:72] = (1_000_000).to_bytes(8, "little")
    tokens[108] = 1
    accounts = [
        (LIGHT, VERIFIER, expected_light(config)),
        (
            checkpoint_pda(config["initial_checkpoint"]),
            VERIFIER,
            bytes.fromhex(config["initial_committee"]),
        ),
        (MINT, TOKEN, mint),
        (cfg, VAULT, bytes(MINT) + route_binding(GENESIS, VAULT) + bytes(32)),
        (custody, TOKEN, tokens),
    ]
    with tempfile.TemporaryDirectory(prefix="bank-", dir=evidence) as temp:
        path = Path(temp)
        save(path / "payer.json", json.loads(pk.to_json()))
        save(path / "config.json", config)
        cmd = [
            args.validator.resolve(),
            "--ledger",
            path / "ledger",
            "--bind-address",
            "127.0.0.1",
            "--rpc-port",
            str(args.rpc_port),
            "--faucet-port",
            str(args.rpc_port + 2),
            "--gossip-port",
            str(args.rpc_port + 3),
            "--dynamic-port-range",
            f"{args.rpc_port + 10}-{args.rpc_port + 48}",
            "--mint",
            str(pk.pubkey()),
            "--bpf-program",
            str(VERIFIER),
            args.images / "pfusdc_egress_verifier.so",
            "--bpf-program",
            str(VAULT),
            args.images / "pfusdc_vault.so",
        ]
        for i, (key, program, data) in enumerate(accounts):
            fn = path / f"account-{i}.json"
            save(
                fn,
                dict(
                    pubkey=str(key),
                    account=dict(
                        owner=str(program),
                        lamports=10_000_000,
                        executable=False,
                        rentEpoch=0,
                        data=[base64.b64encode(data).decode(), "base64"],
                        space=len(data),
                    ),
                ),
            )
            cmd.extend(["--account", str(key), fn])
        url = f"http://127.0.0.1:{args.rpc_port}"
        # Refuse to share a listener with another test or an existing cluster.
        with socket.socket() as s:
            s.bind(("127.0.0.1", args.rpc_port))
        with (evidence / "local-bank.log").open("w") as log:
            bank = subprocess.Popen(cmd, stdout=log, stderr=log, cwd=path)
            try:
                for _ in range(120):
                    if bank.poll() is not None:
                        raise RuntimeError(f"local bank exited {bank.returncode}")
                    try:
                        r = requests.post(
                            url,
                            json=dict(jsonrpc="2.0", method="getHealth", id=1),
                            timeout=1,
                        )
                        if r.json().get("result") == "ok":
                            break
                    except requests.RequestException:
                        pass
                    time.sleep(0.5)
                c = Cluster(
                    SimpleNamespace(
                        rpc=url,
                        payer=path / "payer.json",
                        log=evidence / "bank-transactions.jsonl",
                    )
                )
                c.level = {"commitment": "finalized"}
                assert (
                    c.genesis != GENESIS
                    and c.owner == pk.pubkey()
                    and not c.account(ata)
                )

                with (
                    local_post_fiat(args.node, evidence, path, args.rpc_port + 4),
                    patch.object(payout, "CONFIG", path / "config.json"),
                    patch.object(payout, "cluster", lambda: c),
                ):
                    with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as http:
                        http.journal = Journal(path / "jobs.sqlite")
                        http.wake = threading.Event()
                        threading.Thread(target=http.serve_forever, daemon=True).start()
                        endpoint = f"http://127.0.0.1:{http.server_port}{BASE}"
                        txid = row["burn_tx_id"]
                        requests.post(
                            endpoint, json={"burn_tx_id": txid}, timeout=5
                        ).raise_for_status()

                        # The production queue processor writes below the burn ID. Reuse
                        # an existing proof only if its full witness digest matches.
                        jobs = evidence / "local-job"
                        jobs.mkdir(exist_ok=True)
                        thread = threading.Thread(
                            target=process_next, args=(http.journal, jobs)
                        )
                        thread.start()
                        thread.join(timeout=1800)
                        job = requests.get(endpoint + "/" + txid, timeout=5).json()
                        assert not thread.is_alive() and job["stage"] == "paid", job
                        assert (
                            requests.post(
                                endpoint, json={"burn_tx_id": txid}, timeout=5
                            ).json()
                            == job
                        )
                        # Resume from the journal as if the worker died after confirmation.
                        http.journal.update(txid, "paying")
                        assert process_next(
                            Journal(path / "jobs.sqlite"), path / "retry"
                        )
                        assert (
                            http.journal.get(txid) == job
                            and not (path / "retry").exists()
                        )
                        after = int.from_bytes(c.account(ata)["data"][64:72], "little")
                        vault_after = int.from_bytes(
                            c.account(custody)["data"][64:72], "little"
                        )
                        assert (
                            after == 20000
                            and vault_after == 980000
                            and c.account(payout.marker(txid))["data"] == b"\x01"
                        )
                        save(
                            evidence / "local-payout.json",
                            dict(
                                job=job,
                                ata=str(ata),
                                before=0,
                                after=after,
                                custody_after=vault_after,
                                solana_genesis=c.genesis,
                                committee=config["initial_committee"],
                            ),
                        )
                        http.shutdown()
            finally:
                if bank.poll() is None:
                    bank.terminate()
                    bank.wait(timeout=60)


if __name__ == "__main__":
    main()
