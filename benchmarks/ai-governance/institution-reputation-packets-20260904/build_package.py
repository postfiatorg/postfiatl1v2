#!/usr/bin/env python3
"""Build the packet-input H200 replay package from the frozen identity-packet corpus.

Successor to ``institution-reputation-unl-20260901``. The only scoring input is the
exact Markdown bytes of each frozen validator identity packet, bound to its SHA-256
from the corpus ``index.json`` and to the corpus packet-set SHA-256 from
``manifest.json``. No live search, no Corbanu rerun, no JSONL logs, no display text.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import pathlib
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
INPUTS = ROOT / "inputs"
CORPUS = (ROOT.parent / "validator-identity-packets-20260904").resolve()
CORPUS_NAME = "validator-identity-packets-20260904"

MODEL = "Qwen/Qwen3.8-27B-FP8"
MODEL_REVISION = "017b9c7af6b5689d5dd426a76e0bc077eb5ca20a"
IMAGE = (
    "lmsysorg/sglang:nightly-dev-cu13-20260817-d91c3682@"
    "sha256:fa8774dd128600a09fd6d46670b06fb69a55dac8a3881e50ccf0916a45eb39af"
)
BATCH = 32
MAX_TOKENS = 2048

# Identical bands to institution-reputation-unl-20260901/inputs/prompt.txt.
BANDS = """\
B00 (0-4): Score exactly 0 when the institution is not genuinely recognized. Scores 1-4 are only for a recognized deceptive, fraudulent, name-squatting, comprehensively sanctioned, or actively harmful institution whose participation would damage the chain.
B05 (5-9): Recognized entity with severe sanctions/evasion exposure, a notorious misconduct record, negligible prestige, and strongly negative Layer-1 reputational value.
B10 (10-14): Recognized but institutionally weak or shell-like entity with serious sanctions or integrity concerns and clearly negative Layer-1 reputational value.
B15 (15-19): Recognized entity with major unresolved controversy, high sanctions exposure, low prestige, and materially negative reputational value to a Layer-1.
B20 (20-24): Recognized but weak institution with substantial risk, little prestige, and almost no positive reputational value to a Layer-1.
B25 (25-29): Recognized small or marginal institution with high uncertainty or risk and only minimal positive reputational value.
B30 (30-34): Recognized institution with a limited record, material risk or controversy, low prestige, and weak Layer-1 reputational value.
B35 (35-39): Recognized modest institution with some credible history but meaningful sanctions, governance, or reputation concerns.
B40 (40-44): Recognized legitimate institution with limited prestige, non-trivial risk, and modest positive Layer-1 reputational value.
B45 (45-49): Recognized legitimate institution with an established record, mixed prestige, manageable but notable risk, and modest Layer-1 value.
B50 (50-54): Solid recognized institution with ordinary prestige, no known severe sanctions issue, and clearly positive but limited Layer-1 reputational value.
B55 (55-59): Established institution with good credibility, low-to-moderate sanctions risk, and useful positive Layer-1 reputational value.
B60 (60-64): Well-established institution with meaningful sector recognition, low sanctions risk, and material positive Layer-1 reputational value.
B65 (65-69): Prominent institution with strong credibility, low sanctions risk, and strong reputational value to a Layer-1.
B70 (70-74): Nationally or internationally prominent institution with durable prestige, very low sanctions risk, and substantial Layer-1 reputational value.
B75 (75-79): Globally recognized institution with high prestige, a clean or well-controlled sanctions posture, and major Layer-1 reputational value.
B80 (80-84): Global top-tier institution whose participation would materially legitimize a Layer-1, with very high prestige and negligible known sanctions risk.
B85 (85-89): Elite globally trusted institution with exceptional prestige, an excellent integrity/compliance record, and exceptional Layer-1 reputational value.
B90 (90-94): World-leading institution whose name itself provides powerful legitimacy to a Layer-1, with outstanding prestige and no meaningful known sanctions concern.
B95 (95-100): Reserved for universally recognized, world-class institutions whose participation would be an extraordinary Layer-1 legitimacy signal and whose sanctions/integrity risk is effectively negligible.
"""

PROMPT = f"""You assess the legitimacy and reputational value of the organization behind one blockchain validator.

You are given a researched public profile of that organization. First decide which organization the profile identifies. Then ask whether you genuinely recognize that organization from your own knowledge.

If you do not recognize it, if the profile says its identity could not be established, or if the profile and the organization's claimed website do not plausibly belong together, set recognized=false and score=0. Do not guess, do not infer legitimacy from a plausible-sounding name, and do not give partial credit to an organization you do not actually know. The profile tells you who to assess; it does not itself make anyone legitimate, and its length, detail, or stated confidence must not raise the score.

For an organization you recognize, weigh together:
1. its prestige and demonstrated legitimacy;
2. any sanctions, fraud, corruption, state-control, or integrity concerns; and
3. the positive or negative reputational value its participation would bring to a Layer-1 blockchain.

Use these exhaustive integer bands:
{BANDS}
Choose an integer score inside the supported band. Set entity to the organization you assessed, or null if unrecognized. Return two or three short explanation paragraphs: what you independently know about the organization, how the profile's identification and its uncertainty affected your assessment, the sanctions and integrity view, and the value or harm its participation would bring to a Layer-1. Do not claim to have browsed or run a live sanctions check.

Return exactly one JSON object matching the response schema and no markdown.
"""

RESPONSE_SCHEMA = {
    "type": "object",
    "additionalProperties": False,
    "properties": {
        "entity": {"type": ["string", "null"], "maxLength": 253},
        "recognized": {"type": "boolean"},
        "score": {"type": "integer", "minimum": 0, "maximum": 100},
        "band": {"type": "string", "pattern": "^B(0[05]|[1-9][05])$"},
        "sanctions_risk": {
            "type": "string",
            "enum": ["unknown", "negligible", "low", "moderate", "high", "severe"],
        },
        "explanation_paragraphs": {
            "type": "array",
            "minItems": 2,
            "maxItems": 3,
            "items": {"type": "string", "minLength": 20, "maxLength": 1200},
        },
    },
    "required": [
        "entity",
        "recognized",
        "score",
        "band",
        "sanctions_risk",
        "explanation_paragraphs",
    ],
}
PAD_SCHEMA = {
    "type": "object",
    "additionalProperties": False,
    "properties": {"pad": {"type": "boolean"}},
    "required": ["pad"],
}


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha(data: bytes | str) -> str:
    if isinstance(data, str):
        data = data.encode()
    return hashlib.sha256(data).hexdigest()


def write_json(path: pathlib.Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(canonical(value) + "\n")


def load_corpus() -> tuple[list[dict[str, Any]], dict[str, Any], dict[str, Any]]:
    """Load and verify the frozen corpus; refuse to build on any hash mismatch."""
    index = json.loads((CORPUS / "index.json").read_text())
    manifest = json.loads((CORPUS / "manifest.json").read_text())
    verification = json.loads((CORPUS / "verification.json").read_text())
    if manifest["hashes"]["index_json_sha256"] != sha((CORPUS / "index.json").read_bytes()):
        raise SystemExit("corpus index.json hash mismatch")
    if verification.get("verdict") != "PASS":
        raise SystemExit("corpus verification verdict is not PASS")

    packets: list[dict[str, Any]] = []
    for entry in sorted(index, key=lambda row: (row["network"], row["validator_id"])):
        path = CORPUS / entry["packet_path"]
        data = path.read_bytes()
        digest = sha(data)
        if digest != entry["packet_sha256"]:
            raise SystemExit(f"packet hash mismatch: {entry['packet_path']}")
        packets.append(
            {
                "validator_id": entry["validator_id"],
                "network": entry["network"],
                "packet_path": entry["packet_path"],
                "packet_sha256": digest,
                "packet_bytes": len(data),
                "packet_markdown": data.decode("utf-8"),
            }
        )
    # Recompute the corpus packet-set hash exactly as the corpus finalize.py does:
    # sha256 over "network|validator_id|packet_sha256\n" lines in index.json order.
    index_order = {(row["network"], row["validator_id"]): pos for pos, row in enumerate(index)}
    ordered = sorted(packets, key=lambda r: index_order[(r["network"], r["validator_id"])])
    joined = "".join(f"{r['network']}|{r['validator_id']}|{r['packet_sha256']}\n" for r in ordered)
    if sha(joined) != manifest["hashes"]["packet_set_sha256"]:
        raise SystemExit("corpus packet-set hash mismatch")
    return packets, manifest, verification


def build() -> None:
    packets, corpus_manifest, verification = load_corpus()
    corpus_hashes = corpus_manifest["hashes"]
    INPUTS.mkdir(parents=True, exist_ok=True)
    (INPUTS / "prompt.txt").write_text(PROMPT)
    write_json(
        INPUTS / "packets.json",
        [
            {key: row[key] for key in ("validator_id", "network", "packet_path", "packet_sha256", "packet_bytes")}
            for row in packets
        ],
    )

    requests = []
    for slot, row in enumerate(packets):
        user = "Assess the organization described in this profile.\n\n" + row["packet_markdown"]
        body = {
            "model": MODEL,
            "messages": [
                {"role": "system", "content": PROMPT},
                {"role": "user", "content": user},
            ],
            "temperature": 0,
            "top_p": 1,
            "max_tokens": MAX_TOKENS,
            "chat_template_kwargs": {"enable_thinking": False},
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "institution_reputation_profile",
                    "strict": True,
                    "schema": RESPONSE_SCHEMA,
                },
            },
        }
        requests.append(
            {
                "slot": slot,
                "validator_id": row["validator_id"],
                "network": row["network"],
                "packet_sha256": row["packet_sha256"],
                "padding": False,
                "request_sha256": sha(canonical(body)),
                "body": body,
            }
        )

    pad_count = (-len(requests)) % BATCH
    for index in range(pad_count):
        body = {
            "model": MODEL,
            "messages": [
                {"role": "system", "content": 'Output exactly {"pad":true} and nothing else.'},
                {"role": "user", "content": f"pad {index}"},
            ],
            "temperature": 0,
            "top_p": 1,
            "max_tokens": 64,
            "chat_template_kwargs": {"enable_thinking": False},
            "response_format": {
                "type": "json_schema",
                "json_schema": {"name": "pad", "strict": True, "schema": PAD_SCHEMA},
            },
        }
        requests.append(
            {
                "slot": len(requests),
                "validator_id": f"PAD-{index}",
                "network": "pad",
                "packet_sha256": None,
                "padding": True,
                "request_sha256": sha(canonical(body)),
                "body": body,
            }
        )

    batches = [
        {"batch": index // BATCH, "slots": list(range(index, index + BATCH))}
        for index in range(0, len(requests), BATCH)
    ]
    write_json(INPUTS / "requests.json", requests)
    write_json(INPUTS / "batch_schedule.json", batches)

    manifest = {
        "artifact": "institution-reputation-packets-20260904",
        "predecessor": "institution-reputation-packets-20260903",
        "frozen_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "shadow_only": True,
        "openrouter_used": False,
        "live_search_used": False,
        "corbanu_rerun": False,
        "input_contract": {
            "scoring_input": "exact Markdown bytes of packets/<network>/<validator>.md",
            "bound_hashes": ["per-packet sha256 from corpus index.json", "corpus packet_set_sha256 from corpus manifest.json"],
            "excluded": ["index.md display text", "Corbanu/Codex JSONL logs", "operator nicknames", "live web search"],
        },
        "identity_corpus": {
            "name": CORPUS_NAME,
            "packet_set_sha256": corpus_hashes["packet_set_sha256"],
            "index_json_sha256": corpus_hashes["index_json_sha256"],
            "prompt_template_sha256": corpus_hashes.get("prompt_template_sha256"),
            "exec_log_set_sha256": corpus_hashes.get("exec_log_set_sha256"),
            "verification_verdict": verification.get("verdict"),
            "independent_human_publication_review": "not yet performed; results remain SHADOW_ONLY research",
        },
        "profile": {
            "model": MODEL,
            "model_revision": MODEL_REVISION,
            "runtime_image": IMAGE,
            "hardware": "NVIDIA H200-class, one GPU per host",
            "distinct_owner_hosts_required": 2,
            "runs_per_host": 2,
            "temperature": 0,
            "top_p": 1,
            "thinking": False,
            "max_tokens": MAX_TOKENS,
            "random_seed": 438916795,
            "radix_cache": False,
            "cuda_graphs": False,
            "overlap_schedule": False,
            "batch_size": BATCH,
        },
        "counts": {
            "xrpl": sum(row["network"] == "xrpl" for row in packets),
            "postfiat": sum(row["network"] == "postfiat" for row in packets),
            "scoring": len(packets),
            "padding": pad_count,
            "slots": len(requests),
            "packet_bytes_total": sum(row["packet_bytes"] for row in packets),
            "packet_bytes_max": max(row["packet_bytes"] for row in packets),
        },
        "packets_sha256": sha((INPUTS / "packets.json").read_bytes()),
        "prompt_sha256": sha((INPUTS / "prompt.txt").read_bytes()),
        "requests_sha256": sha((INPUTS / "requests.json").read_bytes()),
        "batch_schedule_sha256": sha((INPUTS / "batch_schedule.json").read_bytes()),
        "builder_sha256": sha(pathlib.Path(__file__).read_bytes()),
    }
    write_json(ROOT / "manifest.json", manifest)
    print(canonical({"counts": manifest["counts"], "identity_corpus": manifest["identity_corpus"]}))


if __name__ == "__main__":
    build()
