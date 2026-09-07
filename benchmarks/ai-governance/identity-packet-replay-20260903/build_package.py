#!/usr/bin/env python3
"""Build the packet-input H200 replay package from the frozen identity corpus.

Inputs are the exact Markdown bytes of
``validator-identity-packets-20260901/packets/<network>/<validator>.md``. Every
scoring request binds the per-packet SHA-256 recorded in that corpus's
``index.json`` and the corpus packet-set SHA-256 recorded in its
``manifest.json``. No live source, web search, agent rerun, JSONL log, or
``index.md`` display text is consulted.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import pathlib
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
INPUTS = ROOT / "inputs"
CORPUS = ROOT.parent / "validator-identity-packets-20260901"
CORPUS_NAME = "validator-identity-packets-20260901"

MODEL = "Qwen/Qwen3.8-27B-FP8"
MODEL_REVISION = "017b9c7af6b5689d5dd426a76e0bc077eb5ca20a"
IMAGE = (
    "lmsysorg/sglang:nightly-dev-cu13-20260817-d91c3682@"
    "sha256:fa8774dd128600a09fd6d46670b06fb69a55dac8a3881e50ccf0916a45eb39af"
)
SEED = 438916795
BATCH_SIZE = 32
MAX_TOKENS = 2048

BEGIN_MARK = "----- BEGIN FROZEN IDENTITY PACKET (exact bytes) -----\n"
END_MARK = "\n----- END FROZEN IDENTITY PACKET -----\n"

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

PROMPT = f"""You score the institutional legitimacy and Layer-1 reputational value of the institution behind one blockchain validator.

Your only inputs are your own model knowledge and one frozen validator identity packet that the user supplies verbatim. The packet is external SHADOW_ONLY research evidence produced earlier by a separate cited-research process. It is not consensus data, it is not a score, and it does not prove that the named institution controls the validator key. A claimed domain, TOML declaration, WHOIS record, registry listing, or social handle inside the packet is identity evidence, not legitimacy. The packet's confidence labels and profile-size tier are descriptive and must not be treated as a score. Do not browse, search, or claim to have checked anything live.

This is a SHADOW_ONLY measurement. Do not use validator uptime, consensus performance, hardware, hosting, or validator-list membership to raise the score. Score only the institution the packet identifies as the validator's canonical public entity.

Mandatory recognition rule: first identify the canonical institution the packet attributes to the validator. Then ask whether you genuinely recognize that exact institution from your own model knowledge, independently of the packet's citations, and whether the packet's claimed domain and identity attribution plausibly fit that institution. If you do not recognize it, if the packet does not establish an institution (for example the canonical entity is unknown or not established, is an individual person rather than an organization, or is only a bare domain), or if the institution/domain pairing looks wrong or like a name-squat, set recognized=false and score=0. Do not guess, do not infer legitimacy from a plausible name or from the packet's citations alone, and do not give an obscure or unrecognized entity partial credit. The packet may help you identify which institution is claimed; it cannot substitute for your own recognition of that institution.

For a recognized institution, weigh all three considerations together:
1. institutional prestige and demonstrated legitimacy;
2. sanctions, fraud, corruption, state-control, or integrity risk, including any material uncertainty or conflict the packet reports; and
3. the positive or negative reputational value its participation would bring to a Layer-1 blockchain.

Use these exhaustive integer bands:
{BANDS}
Choose an integer score inside the supported band. Set institution to the exact name of the institution you scored, or null when recognized=false. Copy validator_id, network, and packet_sha256 exactly from the user message. Return the explanation as an explanation_paragraphs array containing two or three short paragraphs. State the independent facts you genuinely remember about the institution, the sanctions/integrity assessment, and the value or harm its participation would bring to a Layer-1. Do not claim to have browsed or performed a live sanctions search.

Return exactly one JSON object matching the response schema and no markdown.
"""

RESPONSE_SCHEMA = {
    "type": "object",
    "additionalProperties": False,
    "properties": {
        "validator_id": {"type": "string", "maxLength": 100},
        "network": {"type": "string", "enum": ["xrpl", "postfiat"]},
        "packet_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
        "institution": {"type": ["string", "null"], "maxLength": 253},
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
        "validator_id",
        "network",
        "packet_sha256",
        "institution",
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


def user_message(row: dict[str, Any], packet_text: str, packet_set_sha256: str) -> str:
    header = (
        "Score the institution behind this validator using only the frozen identity "
        "packet below and your own model knowledge.\n\n"
        f"network: {row['network']}\n"
        f"validator_id: {row['validator_id']}\n"
        f"corpus: {CORPUS_NAME}\n"
        f"packet_path: {row['packet_path']}\n"
        f"packet_sha256: {row['packet_sha256']}\n"
        f"corpus_packet_set_sha256: {packet_set_sha256}\n\n"
    )
    return header + BEGIN_MARK + packet_text + END_MARK


def load_corpus() -> tuple[list[dict[str, Any]], dict[str, Any]]:
    corpus_manifest = json.loads((CORPUS / "manifest.json").read_bytes())
    index_bytes = (CORPUS / "index.json").read_bytes()
    if sha(index_bytes) != corpus_manifest["hashes"]["index_json_sha256"]:
        raise SystemExit("corpus index.json hash does not match corpus manifest")
    index = json.loads(index_bytes)
    if len(index) != corpus_manifest["counts"]["packets"]:
        raise SystemExit("corpus index length does not match corpus manifest")

    packets: list[dict[str, Any]] = []
    packet_lines = []
    for row in index:
        path = CORPUS / row["packet_path"]
        packet_bytes = path.read_bytes()
        digest = sha(packet_bytes)
        if digest != row["packet_sha256"]:
            raise SystemExit(f"packet hash mismatch for {row['packet_path']}")
        packet_text = packet_bytes.decode("utf-8")
        if packet_text.encode("utf-8") != packet_bytes:
            raise SystemExit(f"packet is not round-trip UTF-8: {row['packet_path']}")
        packet_lines.append(f"{row['network']}|{row['validator_id']}|{digest}\n")
        packets.append(
            {
                "network": row["network"],
                "validator_id": row["validator_id"],
                "packet_path": row["packet_path"],
                "packet_sha256": digest,
                "packet_bytes": len(packet_bytes),
                "packet_text": packet_text,
            }
        )
    packet_set = sha("".join(packet_lines))
    if packet_set != corpus_manifest["hashes"]["packet_set_sha256"]:
        raise SystemExit("recomputed packet-set hash does not match corpus manifest")
    return packets, corpus_manifest


def build() -> None:
    packets, corpus_manifest = load_corpus()
    packet_set_sha256 = corpus_manifest["hashes"]["packet_set_sha256"]
    INPUTS.mkdir(parents=True, exist_ok=True)
    (INPUTS / "prompt.txt").write_text(PROMPT)

    requests = []
    packet_index = []
    for slot, packet in enumerate(packets):
        body = {
            "model": MODEL,
            "messages": [
                {"role": "system", "content": PROMPT},
                {"role": "user", "content": user_message(packet, packet["packet_text"], packet_set_sha256)},
            ],
            "temperature": 0,
            "top_p": 1,
            "max_tokens": MAX_TOKENS,
            "chat_template_kwargs": {"enable_thinking": False},
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "identity_packet_institution_score",
                    "strict": True,
                    "schema": RESPONSE_SCHEMA,
                },
            },
        }
        requests.append(
            {
                "slot": slot,
                "validator_id": packet["validator_id"],
                "network": packet["network"],
                "packet_path": packet["packet_path"],
                "packet_sha256": packet["packet_sha256"],
                "corpus_packet_set_sha256": packet_set_sha256,
                "padding": False,
                "request_sha256": sha(canonical(body)),
                "body": body,
            }
        )
        packet_index.append(
            {
                "slot": slot,
                "network": packet["network"],
                "validator_id": packet["validator_id"],
                "packet_path": packet["packet_path"],
                "packet_sha256": packet["packet_sha256"],
                "packet_bytes": packet["packet_bytes"],
            }
        )

    pad_count = (-len(requests)) % BATCH_SIZE
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
                "packet_path": None,
                "packet_sha256": None,
                "corpus_packet_set_sha256": None,
                "padding": True,
                "request_sha256": sha(canonical(body)),
                "body": body,
            }
        )

    batches = [
        {"batch": index // BATCH_SIZE, "slots": list(range(index, index + BATCH_SIZE))}
        for index in range(0, len(requests), BATCH_SIZE)
    ]
    write_json(INPUTS / "packet_index.json", packet_index)
    write_json(INPUTS / "requests.json", requests)
    write_json(INPUTS / "batch_schedule.json", batches)

    manifest = {
        "artifact": "identity-packet-replay-20260903",
        "frozen_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "shadow_only": True,
        "consensus_input": False,
        "openrouter_used": False,
        "live_web_search": False,
        "agent_rerun": False,
        "input_contract": "exact Markdown packet bytes; per-packet SHA-256 from corpus index.json; corpus packet-set SHA-256 from corpus manifest.json",
        "identity_corpus": {
            "artifact": corpus_manifest["artifact"],
            "finalized_at": corpus_manifest["finalized_at"],
            "packet_set_sha256": packet_set_sha256,
            "index_json_sha256": corpus_manifest["hashes"]["index_json_sha256"],
            "source_validators_sha256": corpus_manifest["hashes"]["source_validators_sha256"],
            "corpus_manifest_sha256": sha((CORPUS / "manifest.json").read_bytes()),
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
            "random_seed": SEED,
            "deterministic_inference": True,
            "radix_cache": False,
            "cuda_graphs": False,
            "overlap_schedule": False,
            "attention_backend": "triton",
            "linear_attn_backend": "triton",
            "chunked_prefill_size": 4096,
            "context_length": 32768,
            "max_running_requests": BATCH_SIZE,
            "batch_size": BATCH_SIZE,
            "loopback_only": True,
        },
        "counts": {
            "xrpl": sum(row["network"] == "xrpl" for row in packets),
            "postfiat": sum(row["network"] == "postfiat" for row in packets),
            "scoring": len(packets),
            "padding": pad_count,
            "slots": len(requests),
            "batches": len(batches),
        },
        "padding": {
            "count": pad_count,
            "system": 'Output exactly {"pad":true} and nothing else.',
            "user": "pad <index>",
            "expected_content": '{"pad":true}',
        },
        "prompt_sha256": sha((INPUTS / "prompt.txt").read_bytes()),
        "packet_index_sha256": sha((INPUTS / "packet_index.json").read_bytes()),
        "requests_sha256": sha((INPUTS / "requests.json").read_bytes()),
        "batch_schedule_sha256": sha((INPUTS / "batch_schedule.json").read_bytes()),
        "builder_sha256": sha(pathlib.Path(__file__).read_bytes()),
    }
    write_json(ROOT / "manifest.json", manifest)
    print(canonical({"counts": manifest["counts"], "identity_corpus": manifest["identity_corpus"]}))


if __name__ == "__main__":
    build()
