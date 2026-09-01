#!/usr/bin/env python3
"""Frozen input-package builder for the reputation-scoring H200 determinism run.

Implements docs/governance/reputation-scoring-h200-run-plan.md (commit 06a23b6b
plus the pre-output erratum recorded in the manifest): 90 scoring packets per
lane (42 base + 18 augmentation + 30 anchors), 3 lanes, 6 padding no-op slots
per lane -> 96 slots/lane, 288 slots total, 9 fixed batches of 32, lane-major.

All JSON is canonical (sorted keys, ",":" separators, ensure_ascii) so package
bytes are reproducible. Run freeze_lookups.py BEFORE this builder so the
baseline lookup snapshots are frozen into the same manifest.
"""
from __future__ import annotations

import hashlib
import json
import pathlib

from cohort import ANCHORS, AUGMENTATION, LANES, sample_packet

ROOT = pathlib.Path(__file__).resolve().parent
OUT = ROOT / "package"
SNAPSHOT_SRC = pathlib.Path.home() / "repos/dynamic-unl-scoring/data/testnet_snapshot.json"
PLAN = ROOT.parents[2] / "docs/governance/reputation-scoring-h200-run-plan.md"
FROZEN_AT = "2026-09-01T00:00:00Z"  # fixed label; real freeze time is the git commit

PROFILE = {
    "execution_profile_id": "qwen3.8-27b-fp8-h200-sglang-strict-fixed32-schema-v2",
    "model": "Qwen/Qwen3.8-27B-FP8",
    "model_revision": "017b9c7af6b5689d5dd426a76e0bc077eb5ca20a",
    "runtime_image": "lmsysorg/sglang:nightly-dev-cu13-20260817-d91c3682@sha256:fa8774dd128600a09fd6d46670b06fb69a55dac8a3881e50ccf0916a45eb39af",
    "hardware": "NVIDIA H200, single GPU",
    "tensor_parallelism": 1,
    "attention_backend": "triton",
    "linear_attention_backend": "triton",
    "radix_cache": False,
    "cuda_graphs": False,
    "overlap_schedule": False,
    "deterministic_inference": True,
    "random_seed": 438916795,
    "context_length": 32768,
    "max_running_requests": 32,
    "request_batch_size_per_host": 32,
    "sampling": {"temperature": 0, "top_p": 1, "thinking": False, "max_tokens": 1024},
    "launch_flag_erratum": (
        "runbook launch block plus --attention-backend triton "
        "--linear-attn-backend triton --disable-cuda-graph "
        "--disable-overlap-schedule per plan section 2 pre-output erratum"
    ),
}


def js(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def sha_s(data: str) -> str:
    return hashlib.sha256(data.encode("utf-8")).hexdigest()


def sha_b(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


# ---------------------------------------------------------------- base cohort
def base_packets(snapshot_bytes: bytes) -> list[dict]:
    snap = json.loads(snapshot_bytes)
    vals = sorted(snap["validators"], key=lambda v: v["master_key"])
    out = []
    for v in vals:
        fields = ["master_key", "domain_verified", "agreement_context"]
        if v.get("domain"):
            fields.insert(1, "domain")
        out.append({
            "packet_version": "rep-v1",
            "validator_id": v["master_key"],
            "master_key": v["master_key"],
            "domain": v.get("domain") or None,
            "domain_verified": bool(v.get("domain_verified")),
            "x_handle": None,
            "handle_binding_evidence": None,
            "organization_claim": None,
            "jurisdiction_claim": None,
            "public_footprint_fields": fields,
            "agreement_context": {
                "a24h": v.get("agreement_24h_score"),
                "a30d": v.get("agreement_30d_score"),
                "totals": {"t24h": v.get("agreement_24h_total"), "t30d": v.get("agreement_30d_total")},
            },
            "sample_data": False,
        })
    assert len(out) == 42, len(out)
    return out


def aug_packets() -> list[dict]:
    out = [sample_packet(a["id"], a["organization"], a["domain"], a["jurisdiction"]) for a in AUGMENTATION]
    assert len(out) == 18
    return out


def anchor_packets() -> list[dict]:
    out = [sample_packet(i, o, d, j) for (i, o, d, j, _cls, _win) in ANCHORS]
    assert len(out) == 30
    return out


# ---------------------------------------------------------------- rubric text
def rubric_text() -> str:
    doc = PLAN.read_text()
    return doc[doc.index("## 5."):doc.index("## 6.")]


SYSTEM_TEMPLATE = """You are a deterministic reputation classifier for PostFiat validator governance. This is a SHADOW_ONLY benchmark: your output has no governance authority.

You score exactly one dimension for one validator evidence packet: {lane_title}.

RUBRIC (score 0-100; each 5-point band's meaning for this dimension is the corresponding column):
{rubric}

CALIBRATION RULES:
1. Obscurity is not fabrication. A real-but-obscure operator with a thin footprint floors at 25-30 prestige and is never placed in the 0-15 fabrication bands; those bands require positive evidence of deception or nonexistence.
2. Absence of sanctions evidence is not safety. Unknown structure caps sanctions_safety at 45-50; high bands require affirmative evidence.
3. Prestige never rescues sanctions; score this lane honestly on its own column.
4. sanctions_safety is inverted risk: 100 means no plausible sanctions exposure.
5. If the packet evidence is insufficient for this dimension, set abstain true and output the most conservative supportable band.

OUTPUT: a single JSON object, no prose, exactly these fields:
{{"validator_id": string, "dimension": "{lane}", "score": integer 0-100, "band": "Bxx" where band = (min(score,99)//5)*5 zero-padded, "citations": [packet field ids you relied on], "weights_prior_claims": [each factual proposition you used that is NOT grounded in a cited packet field], "abstain": boolean}}

Citations may only use field ids present in the packet's public_footprint_fields, or "agreement_context". Any knowledge you use about the organization beyond the packet (its history, size, reputation, sanctions status) MUST be stated as a concise proposition in weights_prior_claims."""

USER_TEMPLATE = """ValidatorReputationEvidencePacket:
{packet}

Score dimension {lane} for validator {vid}. Output only the JSON object."""

PAD_SYSTEM = (
    "You are a no-op padding request in a fixed-batch deterministic inference schedule. "
    'Output exactly this JSON object and nothing else: {"pad": true}'
)

RESPONSE_SCHEMA = {
    "type": "object",
    "properties": {
        "validator_id": {"type": "string"},
        "dimension": {"type": "string", "enum": list(LANES)},
        "score": {"type": "integer", "minimum": 0, "maximum": 100},
        "band": {"type": "string", "pattern": "^B(0[05]|[1-9][05])$"},
        "citations": {"type": "array", "items": {"type": "string"}},
        "weights_prior_claims": {"type": "array", "items": {"type": "string"}},
        "abstain": {"type": "boolean"},
    },
    "required": ["validator_id", "dimension", "score", "band", "citations", "weights_prior_claims", "abstain"],
    "additionalProperties": False,
}
PAD_SCHEMA = {
    "type": "object",
    "properties": {"pad": {"type": "boolean"}},
    "required": ["pad"],
    "additionalProperties": False,
}

LANE_TITLES = {
    "prestige": "organization prestige (rubric column 1)",
    "censorship_resistance": "censorship resistance (rubric column 2)",
    "sanctions_safety": "sanctions risk expressed as safety (rubric column 3)",
}


def build() -> None:
    (OUT / "inputs").mkdir(parents=True, exist_ok=True)

    snapshot_bytes = SNAPSHOT_SRC.read_bytes()
    (OUT / "inputs/testnet_snapshot.json").write_bytes(snapshot_bytes)

    # freeze packets with their own sha (hash excludes the packet_sha256 field)
    all_packets = []
    for p in base_packets(snapshot_bytes) + aug_packets() + anchor_packets():
        p2 = dict(p)
        p2["packet_sha256"] = sha_s(js(p))
        all_packets.append(p2)
    assert len(all_packets) == 90
    packets_json = js(all_packets)
    (OUT / "inputs/packets.json").write_text(packets_json)

    # committed augmentation labels (hash published now, file sealed until run end)
    labels = {a["id"]: {"stratum": a["stratum"], "real": a["real"]} for a in AUGMENTATION}
    labels_json = js(labels)
    (OUT / "inputs/augmentation_labels.SEALED.json").write_text(labels_json)
    labels_commitment = sha_s(labels_json)

    rubric = rubric_text()
    (OUT / "inputs/rubric.md").write_text(rubric)

    # request construction: lane-major, frozen cohort order, 6 pads per lane
    requests = []
    slot = 0
    for lane in LANES:
        for p in all_packets:
            body = {
                "model": PROFILE["model"],
                "messages": [
                    {"role": "system", "content": SYSTEM_TEMPLATE.format(lane=lane, lane_title=LANE_TITLES[lane], rubric=rubric)},
                    {"role": "user", "content": USER_TEMPLATE.format(packet=js(p), lane=lane, vid=p["validator_id"])},
                ],
                "temperature": 0,
                "top_p": 1,
                "max_tokens": PROFILE["sampling"]["max_tokens"],
                "response_format": {"type": "json_schema", "json_schema": {"name": "reputation_classification", "strict": True, "schema": RESPONSE_SCHEMA}},
                "chat_template_kwargs": {"enable_thinking": False},
            }
            requests.append({"slot": slot, "lane": lane, "validator_id": p["validator_id"], "padding": False,
                             "request_sha256": sha_s(js(body)), "body": body})
            slot += 1
        for i in range(6):
            body = {
                "model": PROFILE["model"],
                "messages": [{"role": "system", "content": PAD_SYSTEM}, {"role": "user", "content": f"pad {lane} {i}"}],
                "temperature": 0, "top_p": 1, "max_tokens": 16,
                "response_format": {"type": "json_schema", "json_schema": {"name": "pad", "strict": True, "schema": PAD_SCHEMA}},
                "chat_template_kwargs": {"enable_thinking": False},
            }
            requests.append({"slot": slot, "lane": lane, "validator_id": f"PAD-{lane}-{i}", "padding": True,
                             "request_sha256": sha_s(js(body)), "body": body})
            slot += 1
    assert len(requests) == 288
    requests_json = js(requests)
    (OUT / "inputs/requests.json").write_text(requests_json)

    batches = [{"batch": i, "slots": list(range(i * 32, (i + 1) * 32))} for i in range(9)]
    (OUT / "inputs/batch_schedule.json").write_text(js(batches))

    lookups = OUT / "inputs/lookup_snapshots.json"
    sanctions = ROOT / "sanctions_jurisdictions.json"

    manifest = {
        "plan": "docs/governance/reputation-scoring-h200-run-plan.md",
        "plan_commit": "06a23b6b",
        "plan_erratum": "section 2 launch-flag erratum and section 2.4/6 Vast.ai single-provider deviation, recorded pre-output",
        "frozen_at": FROZEN_AT,
        "profile": PROFILE,
        "lane_order": list(LANES),
        "cohort_counts": {"base": 42, "augmentation": 18, "anchors": 30, "padding_per_lane": 6},
        "testnet_snapshot_sha256": sha_b(snapshot_bytes),
        "packets_sha256": sha_s(packets_json),
        "requests_sha256": sha_s(requests_json),
        "rubric_sha256": sha_s(rubric),
        "batch_schedule_sha256": sha_s(js(batches)),
        "augmentation_labels_commitment": labels_commitment,
        "lookup_snapshots_sha256": sha_b(lookups.read_bytes()) if lookups.exists() else None,
        "sanctions_table_sha256": sha_b(sanctions.read_bytes()) if sanctions.exists() else None,
        "cohort_sha256": sha_b((ROOT / "cohort.py").read_bytes()),
        "builder_sha256": sha_b(pathlib.Path(__file__).read_bytes()),
        "baseline_sha256": sha_b((ROOT / "baseline.py").read_bytes()) if (ROOT / "baseline.py").exists() else None,
        "weights_prior_audit": {
            "auditor": "session operator (Claude Code) under plan section 4.2",
            "frame": "270 non-padding classifications, canonical primary first run",
        },
        "fabrication_construction_disclosure": (
            "Augmentation identities, fabrications, and labels were constructed by the session "
            "operator, who also holds the rubric and baseline; no independence claim is made. "
            "Labels are additionally present in cohort.py in repository plaintext; the commitment "
            "hash binds them, and the baseline provably reads neither the labels file nor the "
            "real/stratum fields."
        ),
        "declared_deviation": "single rental provider and billing account (Vast.ai); two distinct machine owners (distinct machine_id and host_id)",
        "shadow_only": True,
    }
    (OUT / "manifest.json").write_text(js(manifest))
    print("packets", manifest["packets_sha256"][:16], "requests", manifest["requests_sha256"][:16])
    print("labels_commitment", labels_commitment[:16], "manifest", sha_b((OUT / "manifest.json").read_bytes())[:16])


if __name__ == "__main__":
    build()
