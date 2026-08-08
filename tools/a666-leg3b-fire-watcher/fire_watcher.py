#!/usr/bin/env python3
"""A666 leg-3b fire watcher — armed under the 2026-08-08 manager ruling (reversal note).

Deployed copy runs from /tmp/a666-s1g/leg3b/fire_watcher.py.

Polls every 30 s. Triggers when EITHER:
  - constrained signer 0xe01eaf... balance >= 0.005 ETH  (skip 3b0), OR
  - agentd global whitelist contains the signer          (fire 3b0 per WIRED packet).
Then, receipt-gated with STOP-no-retry:
  1. (maybe) 3b0: 0.01 ETH wallet -> signer via agentd evm_send leaf, max-fee 5,666,645,628,000 wei.
  2. checkpoint advance 691->756 via a666-mainnet-advance-pftl-checkpoint.py --execute;
     gate: verifier latestFinalizedHeight() == 756.
  3. leg 3b via a666-mainnet-accept-and-mint.py --execute;
     gates: phase minted-to-recipient, recipient and totalSupply deltas EXACTLY +11,012,575.
Any deviation: write STOP.txt, exit, never retry a mutation. Refuses to start within
30 min of the export-packet deadline (epoch 1786331925 = 2026-08-10 03:18:45 UTC).
"""
from __future__ import annotations

import fcntl
import json
import os
import subprocess
import sys
import time
from pathlib import Path

BASE = Path("/tmp/a666-s1g/leg3b")
FIRE = BASE / "fire"
REPO = Path("/home/postfiat/repos/a666-eth-fast-lane-combined-20260724")
SIGNER = "0xe01eaf76f155b2759402b39fe126b5a81655f424"
SIGNER_SOCKET = "/run/user/1000/postfiat-constrained-signer/a666-signer.sock"
STAKEHUB_HOME = "/home/postfiat/.stakehub"
RPC = "https://ethereum-rpc.publicnode.com"
VERIFIER = "0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A"
CAST = "/home/postfiat/.foundry/bin/cast"
DEADLINE = 1786331925  # 2026-08-10 03:18:45 UTC
DEADLINE_MARGIN_S = 30 * 60
MIN_SIGNER_WEI = 5 * 10**15  # 0.005 ETH
FUND_WEI = 10**16            # 0.01 ETH per WIRED 3b0 packet
FUND_MAX_FEE_WEI = 5_666_645_628_000
PRIOR_BLOCK = "bc3aef9a3b38b0c3030d4350af43addbf285b681cfa8fe750a52a97c236b54e701b66292ba1607525410e3cdf285da26"
TARGET_BLOCK = "4d5195acdbe8b80dac875f35b4a45eb5b31071f5393f07b4f4d54a58bf2a418fe6c45f7fe86d5a937ded828d30770265"
MINT_DELTA = 11_012_575


def log(msg: str) -> None:
    line = f"{time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())} {msg}"
    print(line, flush=True)


def stop(reason: str) -> None:
    (BASE / "STOP.txt").write_text(
        f"{time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())} STOP-no-retry: {reason}\n"
    )
    log(f"STOP-no-retry: {reason}")
    sys.exit(2)


def cast_call(args: list[str]) -> str:
    out = subprocess.run([CAST, *args, "--rpc-url", RPC], capture_output=True, text=True, timeout=60)
    if out.returncode != 0:
        raise RuntimeError(f"cast {' '.join(args[:2])} failed: {out.stderr.strip()[:200]}")
    return out.stdout.strip()


def signer_balance_wei() -> int:
    return int(cast_call(["balance", SIGNER]))


def verifier_height() -> int:
    return int(cast_call(["call", VERIFIER, "latestFinalizedHeight()(uint64)"]))


def whitelist_has_signer() -> bool:
    sys.path.insert(0, str(REPO / "scripts"))
    import native_agentd_leaf as leaf  # noqa: PLC0415
    status = leaf.session_status(STAKEHUB_HOME)
    if not status.get("unlocked"):
        raise RuntimeError("agentd not unlocked")
    return any(w.lower() == SIGNER for w in status["policy"]["whitelist"])


def run_step(name: str, cmd: list[str], env: dict | None = None, timeout: int = 1800) -> subprocess.CompletedProcess:
    FIRE.mkdir(parents=True, exist_ok=True)
    log(f"FIRE {name}: {' '.join(cmd)}")
    full_env = dict(os.environ)
    if env:
        full_env.update(env)
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, cwd=REPO, env=full_env)
    (FIRE / f"{name}.stdout.txt").write_text(proc.stdout)
    (FIRE / f"{name}.stderr.txt").write_text(proc.stderr)
    (FIRE / f"{name}.cmd.txt").write_text(json.dumps({"cmd": cmd, "rc": proc.returncode}, indent=1))
    log(f"{name} rc={proc.returncode}")
    return proc


def fire_sequence(need_funding: bool) -> None:
    if time.time() > DEADLINE - DEADLINE_MARGIN_S:
        stop("within 30 min of export-packet deadline; refusing to start")

    # ---- 3b0 funding (only via whitelist path) ----
    if need_funding:
        proc = run_step("leg3b0-funding", [
            "python3", "scripts/native_evm_leaf_send.py",
            "--stakehub-home", STAKEHUB_HOME,
            "--chain-id", "1",
            "--rpc-url", RPC,
            "--recipient", SIGNER,
            "--amount-wei", str(FUND_WEI),
            "--max-fee-wei", str(FUND_MAX_FEE_WEI),
            "--label", "leg3b0-signer-funding",
            "--report", str(FIRE / "leg3b0-report.json"),
        ])
        if proc.returncode != 0:
            stop(f"3b0 funding failed rc={proc.returncode}; see fire/leg3b0-funding.stderr.txt")
        for _ in range(24):
            if signer_balance_wei() >= MIN_SIGNER_WEI:
                break
            time.sleep(5)
        else:
            stop("3b0 reported success but signer balance never reached 0.005 ETH")
    bal = signer_balance_wei()
    if bal < MIN_SIGNER_WEI:
        stop(f"signer balance {bal} wei below floor at fire time")
    log(f"signer balance {bal} wei — proceeding")

    # ---- checkpoint advance 691 -> 756 ----
    height = verifier_height()
    if height == 756:
        log("verifier already at 756 — advance previously landed; skipping (idempotent)")
    elif height != 691:
        stop(f"verifier at unexpected height {height} (not 691/756)")
    else:
        proc = run_step("checkpoint-advance", [
            "python3", "scripts/a666-mainnet-advance-pftl-checkpoint.py",
            "--execute",
            "--proof-dir", str(BASE / "checkpoint-691-756/proof-cuda"),
            "--prior-block-id", PRIOR_BLOCK,
            "--target-block-id", TARGET_BLOCK,
            "--prior-height", "691",
            "--target-height", "756",
            "--state-file", str(BASE / "checkpoint-691-756/ethereum-state.json"),
        ], env={"POSTFIAT_SIGNER_SOCKET": SIGNER_SOCKET})
        if proc.returncode != 0:
            stop(f"checkpoint advance failed rc={proc.returncode}; see fire/checkpoint-advance.stderr.txt")
        if verifier_height() != 756:
            stop("advance tx completed but latestFinalizedHeight() != 756")
    log("GATE PASS: verifier latestFinalizedHeight == 756")

    # ---- leg 3b accept-and-mint ----
    proc = run_step("leg3b-accept-mint", [
        "python3", "scripts/a666-mainnet-accept-and-mint.py",
        "--execute",
        "--receipt-witness", str(BASE / "receipt-witness.json"),
        "--proof-dir", str(BASE / "proof-cuda"),
        "--state-file", str(BASE / "mint-state.json"),
        "--expected-finalized-height", "787",
    ], env={"POSTFIAT_SIGNER_SOCKET": SIGNER_SOCKET})
    if proc.returncode != 0:
        stop(f"leg 3b failed rc={proc.returncode}; see fire/leg3b-accept-mint.stderr.txt")
    state = json.loads((BASE / "mint-state.json").read_text())
    pre = state["pre_state"]
    post = state.get("post_state", {})
    recipient_delta = int(post.get("recipient_balance_atoms", 0)) - int(pre["recipient_balance_atoms"])
    supply_delta = int(post.get("token_total_supply", 0)) - int(pre["token_total_supply"])
    if state.get("phase") != "minted-to-recipient":
        stop(f"leg 3b terminal phase {state.get('phase')!r} != minted-to-recipient")
    if recipient_delta != MINT_DELTA or supply_delta != MINT_DELTA:
        stop(f"leg 3b delta mismatch: recipient {recipient_delta}, supply {supply_delta}, expected {MINT_DELTA}")
    log(f"GATE PASS: leg 3b minted-to-recipient, deltas exactly +{MINT_DELTA}")
    (BASE / "LEG3B-DONE.txt").write_text(json.dumps({
        "finished_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "transactions": state.get("transactions", []),
        "recipient_delta": recipient_delta,
        "supply_delta": supply_delta,
    }, indent=1))
    log("SEQUENCE COMPLETE — next per ruling: PR 7 + master checkout + restart + one unlock BEFORE 3c")


def main() -> None:
    lock_path = BASE / "fire_watcher.lock"
    lock = open(lock_path, "w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        print("another fire_watcher instance holds the lock; exiting", flush=True)
        sys.exit(1)
    if (BASE / "STOP.txt").exists():
        print("STOP.txt present; refusing to run until an operator removes it", flush=True)
        sys.exit(2)
    if (BASE / "LEG3B-DONE.txt").exists():
        print("LEG3B-DONE.txt present; nothing to do", flush=True)
        sys.exit(0)
    log("fire watcher armed: waiting for signer funding or whitelist entry")
    while True:
        try:
            if time.time() > DEADLINE - DEADLINE_MARGIN_S:
                stop("deadline margin reached while waiting; packet nearly expired")
            bal = signer_balance_wei()
            if bal >= MIN_SIGNER_WEI:
                log(f"TRIGGER: signer funded externally ({bal} wei)")
                fire_sequence(need_funding=False)
                return
            if whitelist_has_signer():
                log("TRIGGER: signer entered agentd whitelist")
                fire_sequence(need_funding=True)
                return
        except SystemExit:
            raise
        except Exception as error:  # noqa: BLE001 — read-only poll errors are retryable
            log(f"poll error (retryable): {error}")
        time.sleep(30)


if __name__ == "__main__":
    main()
