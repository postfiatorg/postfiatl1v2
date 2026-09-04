#!/usr/bin/env python3
"""Cross-model sanity check: score the same 55 profiles with z-ai/glm-5.3-flash via OpenRouter
and correlate against the pinned Qwen H200 scores.

This is NOT part of the replayable runtime. OpenRouter is an external API with no
determinism guarantee; the output is a comparison artifact only.
"""

from __future__ import annotations

import concurrent.futures
import hashlib
import json
import math
import pathlib
import time
import urllib.request
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
KEY_PATH = pathlib.Path.home() / ".config/text-improvement-harness/keys/openrouter.txt"
URL = "https://openrouter.ai/api/v1/chat/completions"
MODEL = "z-ai/glm-5.3-flash"
SEED = 438916795


def canonical(v: Any) -> str:
    return json.dumps(v, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha(s: str) -> str:
    return hashlib.sha256(s.encode()).hexdigest()


def validate(parsed: dict[str, Any]) -> None:
    score = parsed.get("score")
    if not isinstance(score, int) or isinstance(score, bool) or not 0 <= score <= 100:
        raise ValueError("score invalid")
    if parsed.get("band") != f"B{min(score, 99) // 5 * 5:02d}":
        raise ValueError("band mismatch")
    if parsed.get("recognized") is False and score != 0:
        raise ValueError("unrecognized entity did not score zero")


def one(row: dict[str, Any], key: str) -> dict[str, Any]:
    body = dict(row["body"])
    body["model"] = MODEL
    body["seed"] = SEED
    body.pop("chat_template_kwargs", None)
    body["reasoning"] = {"effort": "low"}
    last_error = None
    for attempt in range(4):
        req = urllib.request.Request(
            URL,
            data=canonical(body).encode(),
            headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=300) as resp:
                doc = json.load(resp)
            content = doc["choices"][0]["message"]["content"] or ""
            parsed = json.loads(content)
            validate(parsed)
            return {
                "slot": row["slot"],
                "validator_id": row["validator_id"],
                "network": row["network"],
                "packet_sha256": row["packet_sha256"],
                "content": content,
                "content_sha256": sha(content),
                "parsed": parsed,
                "usage": doc.get("usage"),
                "attempts": attempt + 1,
            }
        except Exception as exc:  # noqa: BLE001
            last_error = repr(exc)
            time.sleep(3 * (attempt + 1))
    return {"slot": row["slot"], "validator_id": row["validator_id"], "network": row["network"], "error": last_error}


def pearson(a: list[float], b: list[float]) -> float:
    n = len(a)
    ma, mb = sum(a) / n, sum(b) / n
    cov = sum((x - ma) * (y - mb) for x, y in zip(a, b))
    va = math.sqrt(sum((x - ma) ** 2 for x in a))
    vb = math.sqrt(sum((y - mb) ** 2 for y in b))
    return cov / (va * vb) if va and vb else float("nan")


def ranks(v: list[float]) -> list[float]:
    order = sorted(range(len(v)), key=lambda i: v[i])
    r = [0.0] * len(v)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and v[order[j + 1]] == v[order[i]]:
            j += 1
        avg = (i + j) / 2 + 1
        for k in range(i, j + 1):
            r[order[k]] = avg
        i = j + 1
    return r


def main() -> None:
    key = KEY_PATH.read_text().strip()
    requests = [r for r in json.loads((ROOT / "inputs/requests.json").read_text()) if not r["padding"]]
    started = time.time()
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as ex:
        results = sorted(ex.map(lambda r: one(r, key), requests), key=lambda r: r["slot"])
    errors = [r for r in results if "error" in r]
    (ROOT / "outputs").mkdir(exist_ok=True)
    (ROOT / "outputs/openrouter-glm-5.3-flash.json").write_text(
        canonical({"model": MODEL, "seed": SEED, "temperature": 0, "provider": "openrouter",
                   "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(started)),
                   "wall_seconds": round(time.time() - started, 1), "results": results}) + "\n")
    if errors:
        print("errors:", len(errors), [e["validator_id"][:12] for e in errors])

    qwen = {r["validator_id"]: r for r in json.loads((ROOT / "outputs/scores.json").read_text())["scores"]}
    rows = []
    for r in results:
        if "error" in r:
            continue
        q = qwen[r["validator_id"]]
        rows.append({
            "validator_id": r["validator_id"], "network": r["network"],
            "qwen_entity": q["entity"], "qwen_score": q["score"], "qwen_recognized": q["recognized"],
            "glm_entity": r["parsed"]["entity"], "glm_score": r["parsed"]["score"], "glm_recognized": r["parsed"]["recognized"],
            "glm_sanctions_risk": r["parsed"]["sanctions_risk"], "delta": r["parsed"]["score"] - q["score"],
        })
    qs = [float(x["qwen_score"]) for x in rows]
    gs = [float(x["glm_score"]) for x in rows]
    both_nonzero = [(a, b) for a, b in zip(qs, gs) if a > 0 and b > 0]
    summary = {
        "n": len(rows),
        "pearson_all": round(pearson(qs, gs), 4),
        "spearman_all": round(pearson(ranks(qs), ranks(gs)), 4),
        "pearson_both_recognized": round(pearson([a for a, _ in both_nonzero], [b for _, b in both_nonzero]), 4) if len(both_nonzero) > 2 else None,
        "recognition_agreement": sum((x["qwen_score"] > 0) == (x["glm_score"] > 0) for x in rows),
        "qwen_zero_glm_positive": sum(x["qwen_score"] == 0 and x["glm_score"] > 0 for x in rows),
        "qwen_positive_glm_zero": sum(x["qwen_score"] > 0 and x["glm_score"] == 0 for x in rows),
        "qwen_mean": round(sum(qs) / len(qs), 2), "glm_mean": round(sum(gs) / len(gs), 2),
        "qwen_zeros": sum(1 for x in qs if x == 0), "glm_zeros": sum(1 for x in gs if x == 0),
        "mean_abs_delta_both_recognized": round(sum(abs(a - b) for a, b in both_nonzero) / len(both_nonzero), 2) if both_nonzero else None,
    }
    rows.sort(key=lambda x: (-max(x["qwen_score"], x["glm_score"]), x["validator_id"]))
    (ROOT / "outputs/cross-model-glm-5.3-flash.json").write_text(canonical({"summary": summary, "rows": rows}) + "\n")
    print(canonical(summary))
    for x in rows:
        print(f"qwen {x['qwen_score']:3d}  glm {x['glm_score']:3d}  {x['network']:8s} {str(x['qwen_entity'] or x['glm_entity'])[:52]}")


if __name__ == "__main__":
    main()
