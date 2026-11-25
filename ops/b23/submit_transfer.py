#!/usr/bin/env python3
"""B23 OPS lane: ONE plain PFT transfer through rpc-serve's own finality path
(mempool_submit_signed_transfer_finality), only while chain.lock says "ops".
Optionally performs consensus-v2 view recovery (timeout votes for the stalled view,
then submit at the next view with a timeout certificate), exactly as the wallet-proxy
does (rpc-routing.js collectFinalityTimeoutVotes / recoverFinalityAcrossViews).

Usage: PYTHONDONTWRITEBYTECODE=1 python3 -u ops/b23/submit_transfer.py LABEL SOURCE KEY_FILE DEST AMOUNT [--recover-view]
"""
from __future__ import annotations

import base64
import gzip
import json
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".scratch-deposit"))
import pftl  # noqa: E402

LOCK = Path("/home/postfiat/repos/b23-coord/chain.lock")
NODE = Path("/home/postfiat/var/b23-release2/bin/postfiat-node")
EVIDENCE = Path("/home/postfiat/repos/b23-coord/evidence/ops/submissions")
METHOD = "mempool_submit_signed_transfer_finality"


def lock() -> None:
    if not LOCK.exists() or LOCK.read_text().strip().lower() != "ops":
        raise SystemExit("refusing: chain.lock does not contain ops")


def wj(p: Path, v: object) -> None:
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(v, indent=2, sort_keys=True) + "\n")


def main() -> int:
    label, source, key_file, dest, amount = sys.argv[1:6]
    recover = "--recover-view" in sys.argv[6:]
    lock()
    out = EVIDENCE / label
    if (out / "finality.json").exists():
        raise SystemExit(f"refusing: {label} already finalized")
    acct = pftl.rpc("account", {"address": source}, f"{label}-acct")
    quote = pftl.rpc("transfer_fee_quote", {"from": source, "to": dest, "amount": int(amount),
                                            "sequence": int(acct["sequence"]) + 1}, f"{label}-quote")
    wj(out / "quote.json", quote)
    run = subprocess.run([str(NODE), "wallet-sign-transfer", "--key-file", key_file, "--quote-file",
                          str(out / "quote.json")], text=True, capture_output=True, timeout=120, check=False)
    if run.returncode != 0:
        raise SystemExit("signing failed: " + run.stderr[-500:])
    signed = json.loads(run.stdout)
    wj(out / "signed.json", signed)
    before = pftl.rpc("server_info", {}, f"{label}-before")["ledger"]
    params = {"signed_transfer_json": json.dumps(signed, separators=(",", ":")),
              "proxy_required_current_height": int(before["height"]),
              "proxy_required_parent_hash": before["hash"],
              "proxy_required_state_root": before["state_root"]}
    if recover:
        height = int(before["height"]) + 1
        votes = {}
        for name, host, port in pftl.PEERS:
            r = pftl.ssh_rpc(host, port, "consensus_v2_timeout_vote", {
                "block_height": height, "view": 0,
                "proxy_required_current_height": int(before["height"]),
                "proxy_required_state_root": before["state_root"],
                "proxy_required_parent_hash": before["hash"],
                "proxy_readiness_timeout_ms": 30000}, f"{label}-tv-{name}", 90)
            res = r.get("result") or {}
            ok = (r.get("ok") is True and res.get("schema") == "postfiat.block_timeout_vote.v1"
                  and int(res.get("block_height", -1)) == height and int(res.get("view", -1)) == 0
                  and (res.get("vote") or {}).get("validator") == name)
            print(f"timeout vote {name}: {'ok' if ok else json.dumps(r.get('error'))[:200]}")
            if ok:
                votes[name] = res
        wj(out / "timeout-votes.json", votes)
        if len(votes) < 5:
            raise SystemExit(f"only {len(votes)} timeout votes; need 5")
        chosen = [votes[k] for k in sorted(votes)][:5]
        blob = base64.b64encode(gzip.compress(json.dumps(chosen).encode(), compresslevel=1)).decode()
        chunks = [blob[i:i + 4000] for i in range(0, len(blob), 4000)]
        params.update({"proxy_consensus_view": 1, "proxy_timeout_votes_encoding": "gzip-base64-chunks-v1",
                       "proxy_timeout_votes_chunk_count": len(chunks)})
        for i, c in enumerate(chunks):
            params[f"proxy_timeout_votes_chunk_{i:04d}"] = c
    lock()
    errors, result, proposer = {}, None, None
    for name, host, port in pftl.PEERS:
        p = pftl.ssh_rpc(host, port, METHOD, params, f"{label}-{name}-{time.time_ns()}", 150)
        if p.get("ok") is True:
            result, proposer = p["result"], name
            break
        errors[name] = p.get("error") or p
        if "proposer" not in json.dumps(errors[name]).lower():
            wj(out / "refused.json", errors)
            raise SystemExit(f"{label} refused at {name}: {json.dumps(errors[name])[:600]}")
    if result is None:
        wj(out / "refused.json", errors)
        raise SystemExit(f"{label}: no proposer accepted: {json.dumps(errors)[:600]}")
    codes = pftl.accepted_codes(result)
    rec = {"label": label, "proposer": proposer, "result": result, "codes": codes}
    if not codes or any(c != "accepted" for c in codes):
        wj(out / "not-accepted.json", rec)
        raise SystemExit(f"{label}: NOT accepted {codes}")
    height = int(result["finality"]["block"]["header"]["height"])
    rec["height"] = height
    rec["fleet"] = pftl.wait_converged(height)
    wj(out / "finality.json", rec)
    print(json.dumps({"label": label, "height": height, "proposer": proposer, "codes": codes,
                      "view": result["finality"]["block"]["header"].get("view")}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
