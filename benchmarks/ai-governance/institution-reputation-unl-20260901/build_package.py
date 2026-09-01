#!/usr/bin/env python3
"""Freeze current XRPL/PostFiat UNLs and build the H200 replay package."""

from __future__ import annotations

import base64
import datetime as dt
import hashlib
import json
import pathlib
import urllib.request
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
SOURCES = ROOT / "sources"
INPUTS = ROOT / "inputs"

XRPL_PUBLISHERS = {
    "ripple": "https://vl.ripple.com",
    "xrpl_foundation": "https://unl.xrplf.org",
}
XRPL_METADATA_URL = "https://api.xrpscan.com/api/v1/validator"
POSTFIAT_UNL_URL = "https://scoring-testnet.postfiat.org/api/scoring/unl/current"
POSTFIAT_ROUND_BASE = "https://scoring-testnet.postfiat.org/api/scoring/rounds/{round_number}"
RIPPLE_ALPHABET = "rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz"
MODEL = "Qwen/Qwen3.8-27B-FP8"
MODEL_REVISION = "017b9c7af6b5689d5dd426a76e0bc077eb5ca20a"
IMAGE = (
    "lmsysorg/sglang:nightly-dev-cu13-20260817-d91c3682@"
    "sha256:fa8774dd128600a09fd6d46670b06fb69a55dac8a3881e50ccf0916a45eb39af"
)

# Name expansions only. They do not assert legitimacy or set a score; the model
# must still recognize the institution and confirm the declared domain fits it.
AFFILIATION_BY_DOMAIN = {
    "anodos.finance": "Anodos Finance",
    "arrington-xrp-capital.blockdaemon.com": "Blockdaemon",
    "bithomp.com": "Bithomp",
    "onxrp.com": "onXRP",
    "peersyst.cloud": "Peersyst",
    "postfiat.org": "Post Fiat",
    "ripple.com": "Ripple",
    "ripple.ittc.ku.edu": "University of Kansas",
    "v2.xrpl-commons.org": "XRPL Commons",
    "validator.gatehub.net": "GateHub",
    "validator.xrpl-labs.com": "XRPL Labs",
    "xrp-validator.interledger.org": "Interledger",
    "xrp.unic.ac.cy": "University of Nicosia",
    "xpmarket.com": "XPMarket",
    "xrpscan.com": "XRPScan",
}

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

PROMPT = f"""You score the institutional legitimacy and reputational value of one blockchain validator operator.

This is a SHADOW_ONLY measurement. Do not use validator uptime, consensus performance, hardware, or list membership to raise the score. Score only the institution claimed by the supplied entity/domain.

Mandatory recognition rule: first ask whether you genuinely recognize the exact institution from your model knowledge and whether the supplied domain plausibly belongs to that institution. If you do not recognize it, or the institution/domain pairing is wrong or looks like a name-squat, set recognized=false and score=0. Do not guess, infer legitimacy from a plausible name, or give an obscure/unrecognized entity partial credit. A missing entity/domain also scores 0.

For a recognized institution, weigh all three considerations together:
1. institutional prestige and demonstrated legitimacy;
2. sanctions, fraud, corruption, state-control, or integrity risk; and
3. the positive or negative reputational value its participation would bring to a Layer-1 blockchain.

Use these exhaustive integer bands:
{BANDS}
Choose an integer score inside the supported band. Return the explanation as an explanation_paragraphs array containing two or three short paragraphs. State the independent facts you genuinely remember, the sanctions/integrity assessment, and the value or harm the institution would bring to a Layer-1. Do not claim to have browsed or performed a live sanctions search.

Return exactly one JSON object matching the response schema and no markdown.
"""

RESPONSE_SCHEMA = {
    "type": "object",
    "additionalProperties": False,
    "properties": {
        "validator_id": {"type": "string", "maxLength": 100},
        "network": {"type": "string", "enum": ["xrpl", "postfiat"]},
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
        "validator_id",
        "network",
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


def fetch_json(url: str) -> Any:
    request = urllib.request.Request(url, headers={"User-Agent": "postfiat-unl-reputation/1"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def write_json(path: pathlib.Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(canonical(value) + "\n")


def node_public_key(public_hex: str) -> str:
    payload = b"\x1c" + bytes.fromhex(public_hex)
    raw = payload + hashlib.sha256(hashlib.sha256(payload).digest()).digest()[:4]
    number = int.from_bytes(raw, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = RIPPLE_ALPHABET[remainder] + encoded
    zeroes = len(raw) - len(raw.lstrip(b"\x00"))
    return RIPPLE_ALPHABET[0] * zeroes + encoded


def parse_postfiat_validator_data(model_request: dict[str, Any]) -> dict[str, Any]:
    content = model_request["messages"][-1]["content"]
    marker = "VALIDATOR DATA:\n"
    start = content.index(marker) + len(marker)
    validators, _ = json.JSONDecoder().raw_decode(content[start:])
    return {row["validator_id"]: row for row in validators}


def build_validators() -> tuple[list[dict[str, Any]], dict[str, Any]]:
    SOURCES.mkdir(parents=True, exist_ok=True)
    publisher_docs = {}
    decoded_lists = {}
    membership: dict[str, list[str]] = {}
    for publisher, url in XRPL_PUBLISHERS.items():
        document = fetch_json(url)
        publisher_docs[publisher] = document
        write_json(SOURCES / f"xrpl-{publisher}-validator-list.json", document)
        decoded = json.loads(base64.b64decode(document["blob"]))
        decoded_lists[publisher] = decoded
        write_json(SOURCES / f"xrpl-{publisher}-decoded.json", decoded)
        for row in decoded["validators"]:
            key = node_public_key(row["validation_public_key"])
            membership.setdefault(key, []).append(publisher)

    xrpl_metadata = fetch_json(XRPL_METADATA_URL)
    write_json(SOURCES / "xrpl-validator-metadata.json", xrpl_metadata)
    metadata_by_key = {row["master_key"]: row for row in xrpl_metadata}

    validators: list[dict[str, Any]] = []
    for key, publishers in sorted(membership.items()):
        metadata = metadata_by_key.get(key, {})
        domain = metadata.get("domain") or None
        affiliation = AFFILIATION_BY_DOMAIN.get(domain)
        validators.append(
            {
                "validator_id": key,
                "network": "xrpl",
                "validation_public_key": key,
                "entity": affiliation or domain,
                "domain": domain,
                "domain_verified": None,
                "institutional_affiliation": affiliation,
                "list_publishers": sorted(publishers),
                "metadata_source": XRPL_METADATA_URL if metadata else None,
            }
        )

    postfiat_unl = fetch_json(POSTFIAT_UNL_URL)
    write_json(SOURCES / "postfiat-current-unl.json", postfiat_unl)
    round_number = postfiat_unl["round_number"]
    round_base = POSTFIAT_ROUND_BASE.format(round_number=round_number)
    validator_map = fetch_json(f"{round_base}/inputs/validator_map.json")
    model_request = fetch_json(f"{round_base}/inputs/model_request.json")
    write_json(SOURCES / f"postfiat-round-{round_number}-validator-map.json", validator_map)
    write_json(SOURCES / f"postfiat-round-{round_number}-model-request.json", model_request)
    data_by_id = parse_postfiat_validator_data(model_request)
    id_by_key = {value["master_key"]: key for key, value in validator_map.items()}
    for key in postfiat_unl["unl"]:
        anonymous_id = id_by_key.get(key)
        row = data_by_id.get(anonymous_id, {})
        domain = row.get("domain") or None
        affiliation = AFFILIATION_BY_DOMAIN.get(domain)
        validators.append(
            {
                "validator_id": key,
                "network": "postfiat",
                "validation_public_key": key,
                "entity": affiliation or domain,
                "domain": domain,
                "domain_verified": row.get("domain_verified"),
                "institutional_affiliation": affiliation,
                "list_publishers": [f"postfiat-round-{round_number}"],
                "metadata_source": f"{round_base}/inputs/model_request.json",
            }
        )

    source_summary = {
        "xrpl": {
            "publishers": {
                name: {
                    "url": XRPL_PUBLISHERS[name],
                    "public_key": publisher_docs[name]["public_key"],
                    "sequence": decoded_lists[name]["sequence"],
                    "expiration_ripple_time": decoded_lists[name]["expiration"],
                    "validator_count": len(decoded_lists[name]["validators"]),
                }
                for name in sorted(XRPL_PUBLISHERS)
            },
            "union_count": len(membership),
            "intersection_count": len(
                set.intersection(
                    *[
                        {node_public_key(row["validation_public_key"]) for row in decoded["validators"]}
                        for decoded in decoded_lists.values()
                    ]
                )
            ),
        },
        "postfiat": {
            "url": POSTFIAT_UNL_URL,
            "round_number": round_number,
            "status": postfiat_unl["status"],
            "validator_count": len(postfiat_unl["unl"]),
        },
    }
    return validators, source_summary


def build() -> None:
    validators, source_summary = build_validators()
    INPUTS.mkdir(parents=True, exist_ok=True)
    write_json(INPUTS / "validators.json", validators)
    (INPUTS / "prompt.txt").write_text(PROMPT)

    requests = []
    for slot, validator in enumerate(validators):
        body = {
            "model": MODEL,
            "messages": [
                {"role": "system", "content": PROMPT},
                {
                    "role": "user",
                    "content": "Score this validator institution:\n" + canonical(validator),
                },
            ],
            "temperature": 0,
            "top_p": 1,
            "max_tokens": 2048,
            "chat_template_kwargs": {"enable_thinking": False},
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "institution_reputation",
                    "strict": True,
                    "schema": RESPONSE_SCHEMA,
                },
            },
        }
        requests.append(
            {
                "slot": slot,
                "validator_id": validator["validator_id"],
                "network": validator["network"],
                "entity": validator["entity"],
                "padding": False,
                "request_sha256": sha(canonical(body)),
                "body": body,
            }
        )

    pad_count = (-len(requests)) % 32
    for index in range(pad_count):
        body = {
            "model": MODEL,
            "messages": [
                {
                    "role": "system",
                    "content": 'Output exactly {"pad":true} and nothing else.',
                },
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
                "entity": None,
                "padding": True,
                "request_sha256": sha(canonical(body)),
                "body": body,
            }
        )

    batches = [
        {"batch": index // 32, "slots": list(range(index, index + 32))}
        for index in range(0, len(requests), 32)
    ]
    write_json(INPUTS / "requests.json", requests)
    write_json(INPUTS / "batch_schedule.json", batches)

    source_hashes = {
        path.name: sha(path.read_bytes()) for path in sorted(SOURCES.glob("*.json"))
    }
    manifest = {
        "artifact": "institution-reputation-unl-20260901",
        "frozen_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "shadow_only": True,
        "openrouter_used": False,
        "profile": {
            "model": MODEL,
            "model_revision": MODEL_REVISION,
            "runtime_image": IMAGE,
            "hardware": "NVIDIA H200, one GPU per host",
            "distinct_owner_hosts_required": 2,
            "runs_per_host": 2,
            "temperature": 0,
            "top_p": 1,
            "thinking": False,
            "max_tokens": 2048,
            "random_seed": 438916795,
            "radix_cache": False,
            "cuda_graphs": False,
            "overlap_schedule": False,
            "batch_size": 32,
        },
        "source_summary": source_summary,
        "counts": {
            "xrpl": sum(row["network"] == "xrpl" for row in validators),
            "postfiat": sum(row["network"] == "postfiat" for row in validators),
            "scoring": len(validators),
            "padding": pad_count,
            "slots": len(requests),
        },
        "source_sha256": source_hashes,
        "validators_sha256": sha((INPUTS / "validators.json").read_bytes()),
        "prompt_sha256": sha((INPUTS / "prompt.txt").read_bytes()),
        "requests_sha256": sha((INPUTS / "requests.json").read_bytes()),
        "batch_schedule_sha256": sha((INPUTS / "batch_schedule.json").read_bytes()),
        "builder_sha256": sha(pathlib.Path(__file__).read_bytes()),
    }
    write_json(ROOT / "manifest.json", manifest)
    print(canonical({"counts": manifest["counts"], "source_summary": source_summary}))


if __name__ == "__main__":
    build()
