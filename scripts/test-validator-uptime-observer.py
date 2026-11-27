#!/usr/bin/env python3
"""End-to-end test for scripts/validator-uptime-observer.py + postfiat-node ML-DSA-65 signing."""

from __future__ import annotations

import datetime as dt
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
OBSERVER_SCRIPT = ROOT / "scripts/validator-uptime-observer.py"


def find_node_bin() -> str:
    env_bin = os.environ.get("POSTFIAT_NODE_BIN")
    if env_bin and Path(env_bin).is_file():
        return env_bin
    for candidate in (
        ROOT / "target/debug/postfiat-node",
        ROOT / "target/release/postfiat-node",
    ):
        if candidate.is_file():
            return str(candidate)
    raise FileNotFoundError("postfiat-node binary not found; build with cargo build -p postfiat-node")


def run_cmd(cmd: list[str], *, expect_ok: bool = True) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(cmd, text=True, capture_output=True)
    if expect_ok and proc.returncode != 0:
        raise AssertionError(
            f"command failed ({proc.returncode}): {' '.join(cmd)}\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
        )
    if not expect_ok and proc.returncode == 0:
        raise AssertionError(f"command unexpectedly succeeded: {' '.join(cmd)}")
    return proc


def main() -> int:
    node_bin = find_node_bin()
    with tempfile.TemporaryDirectory(prefix="postfiat-uptime-obs-test-") as tmp:
        root = Path(tmp)

        # 1. Create 3 distinct geographically independent ML-DSA-65 observer keys via postfiat-node.
        observer_specs = [
            ("observer-us-east-1", "us-east", "foundation-obs-a"),
            ("observer-eu-central-2", "eu-central", "foundation-obs-b"),
            ("observer-ap-northeast-3", "ap-northeast", "foundation-obs-c"),
        ]
        key_files: list[Path] = []
        keyset_entries: list[dict[str, str]] = []
        for oid, region, group in observer_specs:
            kf = root / f"{oid}.key.json"
            proc = run_cmd(
                [
                    node_bin,
                    "observer-key-create",
                    "--key-file",
                    str(kf),
                    "--observer-id",
                    oid,
                    "--region",
                    region,
                    "--operator-group",
                    group,
                ]
            )
            pub = json.loads(proc.stdout)
            key_files.append(kf)
            keyset_entries.append(
                {
                    "observer_id": oid,
                    "region": region,
                    "operator_group": group,
                    "public_key_hex": pub["public_key_hex"],
                }
            )

        keyset_path = root / "observer-keyset.json"
        keyset_path.write_text(
            json.dumps(
                {
                    "schema": "postfiat.observer_keyset.v1",
                    "keyset_id": "test-observer-keyset-v1",
                    "observer_count": 3,
                    "quorum_threshold": 2,
                    "algorithm_id": "ML-DSA-65",
                    "signing_context": "postfiat-l1-v2/validator-uptime-feed/v1",
                    "observers": keyset_entries,
                },
                indent=2,
            ),
            encoding="utf-8",
        )

        # Generate 4 validator public keys (reuse observer-key-create cleanly for valid ML-DSA-65 pubkeys).
        val_pks: list[str] = []
        for idx in range(4):
            proc = run_cmd(
                [
                    node_bin,
                    "observer-key-create",
                    "--key-file",
                    str(root / f"val-{idx}.tmp.json"),
                    "--observer-id",
                    f"val-tmp-{idx}",
                    "--region",
                    f"r{idx}",
                    "--operator-group",
                    f"g{idx}",
                ]
            )
            val_pks.append(json.loads(proc.stdout)["public_key_hex"])

        chain = {
            "network": "l1-testnet",
            "chain_id": "postfiat-testnet-1",
            "genesis_hash": "81" * 48,
            "committee_epoch": 12,
            "registry_root": "33" * 48,
            "finalized_height": 432000,
            "finalized_block_hash": "2d" * 48,
        }
        bpd = 14400
        observed_at = "2026-11-27T01:40:00Z"
        now_unix = int(dt.datetime(2026, 11, 27, 1, 40, 30, tzinfo=dt.timezone.utc).timestamp())

        # Build 3 observer inputs with slight latency-fair score variance (median=9900 for validator-0)
        # covering active, Day-0 ramp-in, lapsed (14d grace), and revoked (grace expired).
        bundle_paths: list[Path] = []
        v0_credits = [14270, 14256, 14242]  # 9909, 9900, 9890 bps -> median 9900
        for idx, kf in enumerate(key_files):
            obs_input = {
                "chain": chain,
                "blocks_per_height_day": bpd,
                "validators": [
                    {
                        "validator_id": "validator-0",
                        "ml_dsa_public_key": val_pks[0],
                        "operator_identity_id": "hive:op-alpha-01",
                        "public_rpc_url": "https://rpc-alpha.example.postfiat.org",
                        "registry_active": True,
                        "previous_state": "active",
                        "first_active_height": 1,
                        "eligible_heights_by_day": {str(d): bpd for d in range(30)},
                        "credited_heights_by_day": {str(d): v0_credits[idx] for d in range(30)},
                        "precommit_qc_heights_by_day": {str(d): 13800 for d in range(30)},
                    },
                    {
                        "validator_id": "validator-1",
                        "ml_dsa_public_key": val_pks[1],
                        "operator_identity_id": "hive:op-beta-02",
                        "public_rpc_url": None,
                        "registry_active": True,
                        "previous_state": "pending",
                        "first_active_height": 432000,
                        "eligible_heights_by_day": {},
                        "credited_heights_by_day": {},
                        "precommit_qc_heights_by_day": {},
                    },
                    {
                        "validator_id": "validator-2",
                        "ml_dsa_public_key": val_pks[2],
                        "operator_identity_id": "hive:op-gamma-03",
                        "public_rpc_url": None,
                        "registry_active": True,
                        "previous_state": "active",
                        "first_active_height": 1,
                        "eligible_heights_by_day": {str(d): bpd for d in range(30)},
                        "credited_heights_by_day": {str(d): 12960 for d in range(30)},  # 9000 bps
                        "precommit_qc_heights_by_day": {str(d): 12000 for d in range(30)},
                    },
                    {
                        "validator_id": "validator-3",
                        "ml_dsa_public_key": val_pks[3],
                        "operator_identity_id": "hive:op-delta-04",
                        "public_rpc_url": None,
                        "registry_active": True,
                        "previous_state": "lapsed",
                        "first_active_height": 1,
                        "grace_started_height": 432000 - 14 * bpd,
                        "grace_started_at": "2026-11-13T01:40:00Z",
                        "eligible_heights_by_day": {str(d): bpd for d in range(30)},
                        "credited_heights_by_day": {str(d): 0 for d in range(30)},
                        "precommit_qc_heights_by_day": {str(d): 0 for d in range(30)},
                    },
                ],
            }
            in_path = root / f"obs-in-{idx}.json"
            out_path = root / f"obs-bundle-{idx}.json"
            in_path.write_text(json.dumps(obs_input), encoding="utf-8")
            run_cmd(
                [
                    sys.executable,
                    str(OBSERVER_SCRIPT),
                    "collect-observer",
                    "--node-bin",
                    node_bin,
                    "--key-file",
                    str(kf),
                    "--input-observations",
                    str(in_path),
                    "--observed-at",
                    observed_at,
                    "--output",
                    str(out_path),
                ]
            )
            bundle_paths.append(out_path)

        # 2. Reconcile 3-of-3 signed bundles and verify ML-DSA-65 signatures + semantics.
        feed_path = root / "uptime-feed.json"
        rec_proc = run_cmd(
            [
                sys.executable,
                str(OBSERVER_SCRIPT),
                "reconcile",
                "--bundles",
                *[str(p) for p in bundle_paths],
                "--output",
                str(feed_path),
                "--keyset-file",
                str(keyset_path),
                "--node-bin",
                node_bin,
                "--generated-at",
                "2026-11-27T01:40:10Z",
                "--now-unix",
                str(now_unix),
            ]
        )
        rec = json.loads(rec_proc.stdout)
        assert rec["ok"] is True
        assert rec["verification"]["valid"] is True
        assert rec["verification"]["feed_stale"] is False
        assert rec["verification"]["fresh_observer_count"] == 3

        feed = json.loads(feed_path.read_text(encoding="utf-8"))
        by_id = {v["validator_id"]: v for v in feed["validators"]}
        assert by_id["validator-0"]["mean_daily_participation_bps"] == 9900
        assert by_id["validator-0"]["access_state"] == "active"
        assert by_id["validator-1"]["access_state"] == "active"
        assert by_id["validator-1"]["access_state_reason"] == "day0_ramp_in"
        assert by_id["validator-2"]["access_state"] == "lapsed"
        assert by_id["validator-2"]["finance_hub_access"] is True
        assert by_id["validator-2"]["reward_eligible"] is False
        assert by_id["validator-3"]["access_state"] == "revoked"
        assert by_id["validator-3"]["research_api_keys_active"] is False
        assert by_id["validator-3"]["removal_proposal_eligible"] is True

        # 3. Verify against Phase 0 Contract C2 JSON Schema if present in worktree or sibling p0-p3-spec.
        schema_candidates = [
            ROOT / "docs/specs/schemas/postfiat-validator-uptime-v1.schema.json",
            Path("/home/postfiat/repos/_bm_worktrees/p0-p3-spec/docs/specs/schemas/postfiat-validator-uptime-v1.schema.json"),
        ]
        schema_arg: list[str] = []
        for candidate in schema_candidates:
            if candidate.is_file():
                schema_arg = ["--schema-file", str(candidate)]
                break

        ver_proc = run_cmd(
            [
                sys.executable,
                str(OBSERVER_SCRIPT),
                "verify",
                "--feed-file",
                str(feed_path),
                "--keyset-file",
                str(keyset_path),
                "--node-bin",
                node_bin,
                "--now-unix",
                str(now_unix),
                *schema_arg,
            ]
        )
        verified = json.loads(ver_proc.stdout)
        assert verified["valid"] is True and verified["feed_stale"] is False

        # 4. Stale check (>3600s later): preserves existing access, blocks new bindings/credits.
        stale_proc = run_cmd(
            [
                sys.executable,
                str(OBSERVER_SCRIPT),
                "verify",
                "--feed-file",
                str(feed_path),
                "--keyset-file",
                str(keyset_path),
                "--node-bin",
                node_bin,
                "--now-unix",
                str(now_unix + 3700),
            ]
        )
        stale = json.loads(stale_proc.stdout)
        assert stale["feed_stale"] is True
        assert stale["allow_new_bindings"] is False
        assert stale["revoke_existing_on_stale"] is False

        # 5. Tampered feed must fail closed.
        tampered_path = root / "tampered-feed.json"
        feed["validators"][0]["mean_daily_participation_bps"] = 9999
        tampered_path.write_text(json.dumps(feed), encoding="utf-8")
        run_cmd(
            [
                sys.executable,
                str(OBSERVER_SCRIPT),
                "verify",
                "--feed-file",
                str(tampered_path),
                "--keyset-file",
                str(keyset_path),
                "--node-bin",
                node_bin,
                "--now-unix",
                str(now_unix),
            ],
            expect_ok=False,
        )

    print("validator_uptime_observer_test=ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
