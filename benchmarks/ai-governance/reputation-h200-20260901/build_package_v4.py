#!/usr/bin/env python3
"""v4 profile: v3 reasoning profile with a workable token budget.

Profile id qwen3.8-27b-fp8-h200-sglang-strict-fixed32-schema-v4-thinking.
v3 diagnosis: reasoning traces cluster at ~4k tokens, exactly the v3 cap, so
151/270 responses truncated with empty JSON (determinism was still perfect).
v4 changes, frozen before any v4 output:
  - max_tokens 8192 (reasoning median ~4k tokens + JSON tail)
  - one added prompt line requesting concise reasoning
Everything else is byte-identical to v3 (same packets, lookups, schema,
identity-verification rules, sealed-label commitment).
"""
from __future__ import annotations

import json
import pathlib

import build_package_v3 as v3
from build_package import LANE_TITLES, PAD_SCHEMA, PAD_SYSTEM, USER_TEMPLATE, js, sha_b, sha_s
from cohort import LANES

ROOT = pathlib.Path(__file__).resolve().parent
OUT = ROOT / "package_v4"

PROFILE = dict(v3.PROFILE)
PROFILE.update({
    "execution_profile_id": "qwen3.8-27b-fp8-h200-sglang-strict-fixed32-schema-v4-thinking",
    "sampling": {"temperature": 0, "top_p": 1, "thinking": True, "max_tokens": 8192},
    "predecessor_profile": v3.PROFILE["execution_profile_id"],
})

SYSTEM_TEMPLATE_V4 = v3.SYSTEM_TEMPLATE_V3.replace(
    "Think through the packet step by step in your reasoning before answering:",
    "Think through the packet step by step in your reasoning before answering — concisely; "
    "a few hundred words of reasoning suffice, then commit to the JSON answer:",
)
assert SYSTEM_TEMPLATE_V4 != v3.SYSTEM_TEMPLATE_V3


def build() -> None:
    (OUT / "inputs").mkdir(parents=True, exist_ok=True)
    for f in ("packets.json", "rubric.md", "testnet_snapshot.json", "lookup_snapshots.json",
              "augmentation_labels.SEALED.json", "batch_schedule.json"):
        (OUT / "inputs" / f).write_bytes((ROOT / "package_v3/inputs" / f).read_bytes())

    all_packets = json.loads((OUT / "inputs/packets.json").read_text())
    rubric = (OUT / "inputs/rubric.md").read_text()

    requests = []
    slot = 0
    for lane in LANES:
        for p in all_packets:
            body = {
                "model": PROFILE["model"],
                "messages": [
                    {"role": "system", "content": SYSTEM_TEMPLATE_V4.format(lane=lane, lane_title=LANE_TITLES[lane], rubric=rubric)},
                    {"role": "user", "content": USER_TEMPLATE.format(packet=js(p), lane=lane, vid=p["validator_id"])},
                ],
                "temperature": 0, "top_p": 1, "max_tokens": 8192,
                "response_format": {"type": "json_schema", "json_schema": {"name": "reputation_classification", "strict": True, "schema": v3.RESPONSE_SCHEMA_V3}},
                "chat_template_kwargs": {"enable_thinking": True},
            }
            requests.append({"slot": slot, "lane": lane, "validator_id": p["validator_id"], "padding": False,
                             "request_sha256": sha_s(js(body)), "body": body})
            slot += 1
        for i in range(6):
            body = {
                "model": PROFILE["model"],
                "messages": [{"role": "system", "content": PAD_SYSTEM}, {"role": "user", "content": f"pad {lane} {i}"}],
                "temperature": 0, "top_p": 1, "max_tokens": 256,
                "response_format": {"type": "json_schema", "json_schema": {"name": "pad", "strict": True, "schema": PAD_SCHEMA}},
                "chat_template_kwargs": {"enable_thinking": True},
            }
            requests.append({"slot": slot, "lane": lane, "validator_id": f"PAD-{lane}-{i}", "padding": True,
                             "request_sha256": sha_s(js(body)), "body": body})
            slot += 1
    assert len(requests) == 288
    requests_json = js(requests)
    (OUT / "inputs/requests.json").write_text(requests_json)

    manifest = json.loads((ROOT / "package_v3/manifest.json").read_text())
    manifest.update({
        "profile": PROFILE,
        "requests_sha256": sha_s(requests_json),
        "builder_sha256": sha_b(pathlib.Path(__file__).read_bytes()),
        "v3_manifest_sha256": sha_b((ROOT / "package_v3/manifest.json").read_bytes()),
        "revision_reason": (
            "v3 run: determinism held (864/864 content and reasoning byte-identical) but 151/270 "
            "responses truncated at the 4096 cap because reasoning traces cluster at ~4k tokens. "
            "v4 doubles max_tokens to 8192 and adds one concise-reasoning prompt line. Frozen "
            "before any v4 output; v2 and v3 results remain published and immutable."
        ),
    })
    (OUT / "manifest.json").write_text(js(manifest))
    print("v4 requests", manifest["requests_sha256"][:16], "manifest", sha_b((OUT / "manifest.json").read_bytes())[:16])


if __name__ == "__main__":
    build()
