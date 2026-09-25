#!/usr/bin/env python3
"""Rebase the pfETH deployment package onto the live PFTL fleet tip.

Reads a converged six-validator `status` + tip block (via StakeHub's
`scripts/pftl_fleet_status.py` helpers over SSH), the live Ethereum deployer
nonce, rewrites `input.json`, regenerates the package, and writes the PFTL
checkpoint gate as PASS with a redaction-safe receipt. Optionally writes a
manager review waiver. Never deploys.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
STAKEHUB = Path("/home/postfiat/repos/StakeHub")
DEFAULT_RPC = "https://ethereum-rpc.publicnode.com"


def load(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    spec.loader.exec_module(module)
    return module


def fleet_tip() -> dict:
    code = r'''
import json, sys
sys.path.insert(0, "scripts")
from pftl_fleet_status import node_json_allow_error, status_rows
from pft_wallet.ce22 import validators
from pft_wallet.config import load_config
rows = status_rows()
keys = ["block_height", "block_tip_hash", "chain_id", "genesis_hash", "state_root"]
views = [json.dumps({k: r.get(k) for k in keys}, sort_keys=True) for r in rows]
assert len(set(views)) == 1, "fleet not converged"
cfg = load_config()
commits = []
for v in validators(cfg):
    d = node_json_allow_error(cfg, v, ["rpc", "--endpoint", f"http://127.0.0.1:{27650 + v.index}",
        "--method", "blocks", "--from-height", str(rows[0]["block_height"]), "--limit", "1",
        "--data-dir", v.data_dir])
    h = d["result"][0]["header"]
    qc = h["consensus_v2_commit"]["precommit_qc"]
    commits.append({"block_id": qc["block"]["block_id"], "height": qc["block"]["height"],
        "committee_root": qc["domain"]["committee_root"], "committee_epoch": qc["domain"]["committee_epoch"],
        "chain_id": qc["domain"]["chain_id"], "genesis_hash": qc["domain"]["genesis_hash"], "quorum": qc["quorum"]})
assert len({json.dumps(c, sort_keys=True) for c in commits}) == 1, "commit views differ"
out = {**json.loads(views[0]), **commits[0], "validators": len(rows),
       "mempool_pending": [r.get("mempool_pending") for r in rows]}
print(json.dumps(out))
'''
    res = subprocess.run([str(STAKEHUB / ".venv/bin/python"), "-c", code], cwd=STAKEHUB,
                         capture_output=True, text=True, timeout=600, check=True)
    return json.loads(res.stdout.strip().splitlines()[-1])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", type=Path, default=ROOT / "deployments/pfeth-eth-mainnet-20260904")
    parser.add_argument("--rpc-url", default=DEFAULT_RPC)
    parser.add_argument("--waive-review", action="store_true")
    parser.add_argument("--waived-by", default="Post Fiat founder (manager), via Codex session 2026-09-04")
    args = parser.parse_args()

    package_mod = load(ROOT / "scripts/pfeth-eth-mainnet-package.py", "pfeth_package")
    deploy_mod = load(ROOT / "scripts/pfeth-eth-mainnet-deploy.py", "pfeth_deploy")

    tip = fleet_tip()
    assert tip["block_tip_hash"] == tip["block_id"]
    nonce = int(deploy_mod.rpc_call(args.rpc_url, "eth_getTransactionCount",
                                    [package_mod.read_json(args.package / "input.json")["deployer"], "latest"]), 16)

    input_path = args.package / "input.json"
    value = package_mod.read_json(input_path)
    value.update({
        "deployer_nonce": nonce,
        "initial_checkpoint_block_id": tip["block_id"],
        "initial_finalized_height": int(tip["height"]),
        "initial_committee_root": tip["committee_root"],
        "activation_height": int(tip["height"]) + 2,
    })
    assert value["pftl_chain_id"] == tip["chain_id"] and value["pftl_genesis_hash"] == tip["genesis_hash"]
    observed_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    value["snapshot_evidence"] = {
        "ethereum_deployer_nonce_observed_utc": observed_at,
        "pftl_checkpoint_source": "six-validator SSH status + blocks(precommit_qc) read, converged",
        "pftl_checkpoint_freshness": f"live tip height {tip['height']} at {observed_at}",
    }
    input_path.write_text(json.dumps(value, indent=2) + "\n")

    summary = package_mod.generate(value, args.package)
    manifest_digest = summary["manifest_sha256"]

    receipt = {
        "read_at": observed_at, "validators_read": tip["validators"], "quorum": tip["quorum"],
        "height": tip["height"], "block_id": tip["block_id"], "committee_root": tip["committee_root"],
        "committee_epoch": tip["committee_epoch"], "state_root": tip["state_root"],
        "mempool_pending": tip["mempool_pending"], "transport": "ssh BatchMode, remote loopback rpc-serve",
    }
    gate = {
        "schema": "postfiat.pfeth.pftl_checkpoint_gate.v1", "status": "PASS",
        "manifest_sha256": manifest_digest, "chain_id": tip["chain_id"], "genesis_hash": tip["genesis_hash"],
        "checkpoint_block_id": tip["block_id"], "finalized_height": int(tip["height"]),
        "committee_root": tip["committee_root"], "observed_at": observed_at,
        "source_receipt": json.dumps(receipt, sort_keys=True),
        "instruction": "Regenerated by scripts/pfeth-eth-mainnet-rebase.py from a converged six-validator read.",
    }
    (args.package / "pftl-checkpoint-gate.json").write_text(json.dumps(gate, indent=2) + "\n")

    if args.waive_review:
        waiver = {
            "schema": "postfiat.pfeth.external_review_gate.v1", "status": "WAIVED",
            "manifest_sha256": manifest_digest, "reviewer": None, "reviewed_at": None,
            "reviewed_contracts": [], "reviewed_programs": [], "signature_or_report": None,
            "waived_by": args.waived_by, "waived_at": observed_at,
            "waiver_statement": (
                "UNREVIEWED v1 release deployed under manager authority. ExitExecutorV1, "
                "WETHBridgeVaultL1, PFTLFinalityVerifierV1 and both SP1 guests have had no "
                "independent review. Caps and the pausable owner bound exposure; a reviewed "
                "release is a redeploy."
            ),
            "instruction": "Set to PASS only after an independent review of the exact manifest.",
        }
        (args.package / "external-review.json").write_text(json.dumps(waiver, indent=2) + "\n")

    pre = deploy_mod.preflight(args.package, args.rpc_url)
    print(json.dumps({"manifest_sha256": manifest_digest, "tip": receipt,
                      "predicted_addresses": summary["predicted_addresses"],
                      "preflight_ok": pre["ok"], "approval_phrase": f"DEPLOY PFETH {manifest_digest}"}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
