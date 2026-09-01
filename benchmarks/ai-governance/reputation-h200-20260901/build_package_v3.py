#!/usr/bin/env python3
"""v3 profile: reasoning enabled, hardened prompt, truncation-proof schema.

Profile id qwen3.8-27b-fp8-h200-sglang-strict-fixed32-schema-v3-thinking.
Same frozen cohort/packets/lookups as v2 (comparable results); changes:
  - chat_template_kwargs.enable_thinking = true (reasoning-parser qwen3
    separates reasoning_content from the JSON answer channel)
  - max_tokens 4096 (reasoning tokens included)
  - system prompt: explicit absence-of-knowledge fabrication rule and
    obscure-operator protection, reasoning guidance
  - schema: weights_prior_claims maxItems 12 / maxLength 240, citations
    maxItems 8 -> the v2 repetition spiral is structurally impossible
Writes package_v3/. Reuses v2 packets byte-identically.
"""
from __future__ import annotations

import json
import pathlib

import build_package as v2
from build_package import LANE_TITLES, PAD_SCHEMA, PAD_SYSTEM, USER_TEMPLATE, js, sha_b, sha_s
from cohort import LANES

ROOT = pathlib.Path(__file__).resolve().parent
V2PKG = ROOT / "package"
OUT = ROOT / "package_v3"

PROFILE = dict(v2.PROFILE)
PROFILE.update({
    "execution_profile_id": "qwen3.8-27b-fp8-h200-sglang-strict-fixed32-schema-v3-thinking",
    "sampling": {"temperature": 0, "top_p": 1, "thinking": True, "max_tokens": 4096},
    "predecessor_profile": "qwen3.8-27b-fp8-h200-sglang-strict-fixed32-schema-v2",
})

SYSTEM_TEMPLATE_V3 = """You are a reputation classifier for PostFiat validator governance. This is a SHADOW_ONLY benchmark: your output has no governance authority.

You score exactly one dimension for one validator evidence packet: {lane_title}.

RUBRIC (score 0-100; each 5-point band's meaning for this dimension is the corresponding column):
{rubric}

Think through the packet step by step in your reasoning before answering: identify the claimed organization, recall what you actually know about it, check that knowledge against the claimed domain and jurisdiction, and only then pick a band.

IDENTITY VERIFICATION RULES (apply before scoring):
1. Ask yourself: do you have genuine prior knowledge of this exact organization under this exact name and domain from your training data? Genuine knowledge means you can recall independent facts about it (founding era, what it does, where it operates) that are NOT restated from the packet.
2. If you have no genuine prior knowledge of a claimed institution, that absence IS evidence: an organization claiming institutional standing that you have never encountered belongs in the 0-15 fabrication/shell bands for prestige. State this reasoning in weights_prior_claims.
3. Watch for name-squats: a name that embeds or imitates a famous brand (e.g. a "council", "foundation", or "validator" suffix attached to a well-known company) but is not that company's actual entity is a fabrication signal, not a prestige signal. Check whether the domain is the brand's real domain.
4. The mirror rule: small real operators exist. If you DO have genuine prior knowledge of a small or obscure organization (a real hosting company, a real volunteer project), it floors at 25-30 prestige regardless of how thin its footprint is. Obscurity with genuine existence is never scored as fabrication.
5. For base-cohort packets with no organization claim, score what the packet supports; anonymity without an institutional claim is the 10-20 region of prestige, not the fabrication bands.

CALIBRATION RULES:
1. Absence of sanctions evidence is not safety. Unknown structure caps sanctions_safety at 45-50; high bands require affirmative evidence.
2. Prestige never rescues sanctions; score this lane honestly on its own column.
3. sanctions_safety is inverted risk: 100 means no plausible sanctions exposure.
4. If the packet evidence is insufficient for this dimension, set abstain true and output the most conservative supportable band.

OUTPUT after your reasoning: a single JSON object, no prose, exactly these fields:
{{"validator_id": string, "dimension": "{lane}", "score": integer 0-100, "band": "Bxx" where band = (min(score,99)//5)*5 zero-padded, "citations": [packet field ids you relied on, max 8], "weights_prior_claims": [max 12 concise propositions you used that are NOT grounded in a cited packet field], "abstain": boolean}}

Citations may only use field ids present in the packet's public_footprint_fields, or "agreement_context". Any knowledge you use about the organization beyond the packet MUST be stated as one concise proposition in weights_prior_claims; never repeat a proposition."""

RESPONSE_SCHEMA_V3 = {
    "type": "object",
    "properties": {
        "validator_id": {"type": "string", "maxLength": 80},
        "dimension": {"type": "string", "enum": list(LANES)},
        "score": {"type": "integer", "minimum": 0, "maximum": 100},
        "band": {"type": "string", "pattern": "^B(0[05]|[1-9][05])$"},
        "citations": {"type": "array", "items": {"type": "string", "maxLength": 40}, "maxItems": 8},
        "weights_prior_claims": {"type": "array", "items": {"type": "string", "maxLength": 240}, "maxItems": 12},
        "abstain": {"type": "boolean"},
    },
    "required": ["validator_id", "dimension", "score", "band", "citations", "weights_prior_claims", "abstain"],
    "additionalProperties": False,
}


def build() -> None:
    (OUT / "inputs").mkdir(parents=True, exist_ok=True)
    # reuse v2 frozen inputs byte-identically
    for f in ("packets.json", "rubric.md", "testnet_snapshot.json", "lookup_snapshots.json",
              "augmentation_labels.SEALED.json", "batch_schedule.json"):
        (OUT / "inputs" / f).write_bytes((V2PKG / "inputs" / f).read_bytes())

    all_packets = json.loads((OUT / "inputs/packets.json").read_text())
    rubric = (OUT / "inputs/rubric.md").read_text()

    requests = []
    slot = 0
    for lane in LANES:
        for p in all_packets:
            body = {
                "model": PROFILE["model"],
                "messages": [
                    {"role": "system", "content": SYSTEM_TEMPLATE_V3.format(lane=lane, lane_title=LANE_TITLES[lane], rubric=rubric)},
                    {"role": "user", "content": USER_TEMPLATE.format(packet=js(p), lane=lane, vid=p["validator_id"])},
                ],
                "temperature": 0, "top_p": 1, "max_tokens": 4096,
                "response_format": {"type": "json_schema", "json_schema": {"name": "reputation_classification", "strict": True, "schema": RESPONSE_SCHEMA_V3}},
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

    v2man = json.loads((V2PKG / "manifest.json").read_text())
    manifest = dict(v2man)
    manifest.update({
        "profile": PROFILE,
        "requests_sha256": sha_s(requests_json),
        "builder_sha256": sha_b(pathlib.Path(__file__).read_bytes()),
        "v2_manifest_sha256": sha_b((V2PKG / "manifest.json").read_bytes()),
        "revision_reason": (
            "v2 published run: two lanes lost validity to 1024-token repetition truncations and "
            "no lift over baseline. v3 enables reasoning, adds identity-verification prompt rules, "
            "raises max_tokens to 4096, and bounds citations/weights_prior_claims in-schema. "
            "Frozen before any v3 output; v2 results remain published and immutable."
        ),
    })
    (OUT / "manifest.json").write_text(js(manifest))
    print("v3 requests", manifest["requests_sha256"][:16], "manifest", sha_b((OUT / "manifest.json").read_bytes())[:16])


if __name__ == "__main__":
    build()
