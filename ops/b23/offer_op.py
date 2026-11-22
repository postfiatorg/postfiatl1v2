#!/usr/bin/env python3
"""B23 OPS lane: submit ONE offer transaction (offer_create / offer_cancel) through the
release-2 `mempool_submit_signed_offer_transaction_finality`, only while chain.lock says "ops".

quote (offer_fee_quote) -> offline sign (wallet-sign-offer-transaction, key by PATH) ->
finality submit at the deterministic proposer -> literal receipt check (result.finality.receipt)
-> six-node convergence. A mempool-admission preview (confirmed:false) is NOT acceptance.

Usage: PYTHONDONTWRITEBYTECODE=1 python3 -u ops/b23/offer_op.py LABEL SOURCE KEY_FILE OPERATION_JSON_FILE
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, "/home/postfiat/repos/postfiatl1v2-navcoin-proof-b2.2/.scratch-deposit")
import pftl  # noqa: E402

LOCK = Path("/home/postfiat/repos/b23-coord/chain.lock")
NODE = Path("/home/postfiat/var/b23-release2/bin/postfiat-node")  # release-2 build, sha b4a6d3fc...
EVIDENCE = Path("/home/postfiat/repos/b23-coord/evidence/ops/submissions")
METHOD = "mempool_submit_signed_offer_transaction_finality"


def require_ops_lock() -> None:
    if not LOCK.exists() or LOCK.read_text().strip().lower() != "ops":
        raise SystemExit("refusing: chain.lock does not contain ops")


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    label, source, key_file, op_file = sys.argv[1:5]
    require_ops_lock()
    out = EVIDENCE / label
    if (out / "finality.json").exists():
        raise SystemExit(f"refusing: {label} already has a finality record")
    operation = json.loads(Path(op_file).read_text())
    write_json(out / "operation.json", operation)
    quote = pftl.rpc("offer_fee_quote", {"source": source,
                     "operation_json": json.dumps(operation, separators=(",", ":"))}, f"{label}-quote")
    write_json(out / "quote.json", quote)
    signed_run = subprocess.run([str(NODE), "wallet-sign-offer-transaction", "--key-file", key_file,
                                 "--quote-file", str(out / "quote.json")],
                                text=True, capture_output=True, timeout=180, check=False)
    if signed_run.returncode != 0:
        raise SystemExit("signing failed: " + (signed_run.stderr or "")[-600:])
    signed = json.loads(signed_run.stdout)
    write_json(out / "signed.json", signed)
    require_ops_lock()
    params = {"signed_offer_transaction_json": json.dumps(signed, separators=(",", ":"))}
    errors, result, proposer = {}, None, None
    for peer in pftl.PEERS:
        payload = pftl.ssh_rpc(peer[1], peer[2], METHOD, params, f"{label}-{peer[0]}-{time.time_ns()}", 150)
        if payload.get("ok") is True:
            result, proposer = payload["result"], peer[0]
            break
        errors[peer[0]] = payload.get("error") or payload
        if "proposer" not in json.dumps(errors[peer[0]]).lower():
            write_json(out / "refused.json", errors)
            raise SystemExit(f"{label} refused at {peer[0]}: {json.dumps(errors[peer[0]])[:800]}")
    if result is None:
        write_json(out / "refused.json", errors)
        raise SystemExit(f"{label}: no proposer accepted")
    fin = result.get("finality") or {}
    receipt = fin.get("receipt") or {}
    record = {"label": label, "proposer": proposer, "result": result}
    if fin.get("confirmed") is not True or receipt.get("accepted") is not True:
        write_json(out / "not-accepted.json", record)
        raise SystemExit(f"{label}: NOT accepted: confirmed={fin.get('confirmed')} receipt={json.dumps(receipt)[:400]}")
    height = int(fin["block"]["header"]["height"])
    record["height"] = height
    record["fleet"] = pftl.wait_converged(height)
    write_json(out / "finality.json", record)
    print(json.dumps({"label": label, "height": height, "proposer": proposer,
                      "receipt": {k: receipt.get(k) for k in ("accepted", "code", "message", "tx_id")}}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
