#!/usr/bin/env python3
"""Drive the bridge service job API exactly as the browser does:
GET /api/bridge/readiness ; POST /api/bridge/jobs ; poll GET /api/bridge/jobs/{id}.
Bearer token is read from the proxy token FILE (local-demo) and never printed.
Usage:
  bridge_job.py readiness OUT.json
  bridge_job.py submit DEPOSIT_RECORD.json OUTDIR      (creates job, then polls until terminal)
  bridge_job.py poll JOB_ID OUTDIR
"""
import json, sys, time, urllib.request
from pathlib import Path

BASE = "http://127.0.0.1:18082"
ORIGIN = "http://127.0.0.1:18082"
TOKEN = json.load(open("/home/postfiat/.local/state/postfiat-a666-wallet/proxy-tokens.json"))["local-demo"]
ROUTE = "ethereum-mainnet-usdc-v1"


def req(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    r = urllib.request.Request(BASE + path, data=data, method=method, headers={
        "Authorization": f"Bearer {TOKEN}", "Origin": ORIGIN, "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(r, timeout=60) as resp:
            return resp.status, json.loads(resp.read() or b"null")
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"null")


def poll(job_id, out):
    last = None
    while True:
        st, j = req("GET", f"/api/bridge/jobs/{job_id}")
        Path(out, "job-latest.json").write_text(json.dumps(j, indent=1))
        key = json.dumps({k: (j or {}).get(k) for k in ("status", "stage", "stage_index", "retryable", "error", "code")}, sort_keys=True)
        if key != last:
            print(time.strftime("%H:%M:%SZ", time.gmtime()), st, key, flush=True)
            with open(Path(out, "job-transitions.jsonl"), "a") as f:
                f.write(json.dumps({"t": time.time(), "http": st, "job": j}) + "\n")
            last = key
        status = str((j or {}).get("status", "")).lower()
        if status in ("complete", "completed", "succeeded", "failed", "error") or (j or {}).get("terminal") is True:
            print("TERMINAL", json.dumps(j)[:1500], flush=True)
            return j
        time.sleep(10)


def main():
    cmd = sys.argv[1]
    if cmd == "readiness":
        st, j = req("GET", f"/api/bridge/readiness?route={ROUTE}")
        Path(sys.argv[2]).write_text(json.dumps(j, indent=1))
        print(st, json.dumps(j)[:1200])
    elif cmd == "submit":
        rec = json.loads(Path(sys.argv[2]).read_text()); out = Path(sys.argv[3]); out.mkdir(parents=True, exist_ok=True)
        body = {"route_id": ROUTE, "source_chain_id": 1, "deposit_tx_hash": rec["deposit_tx_hash"],
                "deposit_id": rec["deposit_id"], "pftl_recipient": rec["pftl_recipient"],
                "depositor": rec["depositor"], "amount_atoms": rec["amount_atoms"],
                "idempotency_key": "ops-b23-g2-" + rec["deposit_tx_hash"][2:18]}
        (out / "job-request.json").write_text(json.dumps(body, indent=1))
        st, j = req("POST", "/api/bridge/jobs", body)
        (out / "job-create-response.json").write_text(json.dumps(j, indent=1))
        print("create", st, json.dumps(j)[:600], flush=True)
        if st not in (200, 202):
            sys.exit(1)
        poll(j["job_id"], out)
    elif cmd == "poll":
        out = Path(sys.argv[3]); out.mkdir(parents=True, exist_ok=True); poll(sys.argv[2], out)


if __name__ == "__main__":
    main()
