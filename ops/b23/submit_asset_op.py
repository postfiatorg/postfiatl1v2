#!/usr/bin/env python3
"""B23 OPS lane: submit ONE ordinary asset transaction, only while chain.lock says "ops".

Mirrors .scratch-deposit/pftl.py (quote -> offline sign -> finality submit ->
literal receipt check -> six-node convergence) but carries the OPS lock check
instead of the MINT one. Keys are passed to the signer by PATH only; nothing
key-bearing is printed or written here.

Usage:
  PYTHONDONTWRITEBYTECODE=1 python3 ops/b23/submit_asset_op.py LABEL SOURCE KEY_FILE OPERATION_JSON_FILE
Evidence goes to /home/postfiat/repos/b23-coord/evidence/ops/submissions/LABEL/.
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".scratch-deposit"))
import pftl  # noqa: E402  (read helpers only: rpc, ssh_rpc, accepted_codes, wait_converged)

LOCK = Path("/home/postfiat/repos/b23-coord/chain.lock")
# Byte-identical copy of the live release binary (sha256 35513b7ce319162e...).
NODE = Path("/home/postfiat/.local/lib/postfiat/releases/r2-1-activation-bound-20261112/postfiat-node")
EVIDENCE = Path("/home/postfiat/repos/b23-coord/evidence/ops/submissions")


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
    quote = pftl.rpc("asset_fee_quote", {"source": source,
                     "operation_json": json.dumps(operation, separators=(",", ":"))},
                     f"{label}-quote")
    write_json(out / "quote.json", quote)
    signed_run = subprocess.run([str(NODE), "wallet-sign-asset-transaction", "--key-file", key_file,
                                 "--quote-file", str(out / "quote.json")],
                                text=True, capture_output=True, timeout=180, check=False)
    if signed_run.returncode != 0:
        raise SystemExit("signing failed: " + (signed_run.stderr or "")[-600:])
    signed = json.loads(signed_run.stdout)
    write_json(out / "signed.json", signed)
    require_ops_lock()
    before = pftl.rpc("server_info", {}, f"{label}-before")
    params = {"signed_asset_transaction_json": json.dumps(signed, separators=(",", ":")),
              "proxy_required_current_height": int(before["ledger"]["height"]),
              "proxy_required_parent_hash": before["ledger"]["hash"],
              "proxy_required_state_root": before["ledger"]["state_root"]}
    errors, result, proposer = {}, None, None
    for peer in pftl.PEERS:
        payload = pftl.ssh_rpc(peer[1], peer[2], "mempool_submit_signed_asset_transaction_finality",
                               params, f"{label}-{peer[0]}-{time.time_ns()}", 150)
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
    codes = pftl.accepted_codes(result)
    height = int(result["finality"]["block"]["header"]["height"])
    record = {"label": label, "proposer": proposer, "height": height, "receipt_codes": codes,
              "result": result}
    if not codes or any(c != "accepted" for c in codes):
        write_json(out / "rejected-finality.json", record)
        raise SystemExit(f"{label}: receipt NOT accepted: {codes}")
    record["fleet"] = pftl.wait_converged(height)
    write_json(out / "finality.json", record)
    print(json.dumps({"label": label, "height": height, "proposer": proposer, "codes": codes}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
