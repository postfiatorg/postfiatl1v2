#!/usr/bin/env python3
"""Cross-model sanity check: score the same 55 profiles with several hosted models via OpenRouter
and correlate every model against the pinned Qwen H200 scores and against each other.

usage: python3 openrouter_cross_model.py [model ...]   (default: all in MODELS)

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
MODELS = [
    "z-ai/glm-5.3-flash",
    "openai/gpt-5.6-luna",
    "moonshotai/kimi-k3",
    "anthropic/claude-fable-5",
    "deepseek/deepseek-v4-pro",
]
NO_SEED = {"anthropic/claude-fable-5"}
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


def one(row: dict[str, Any], key: str, model: str) -> dict[str, Any]:
    body = dict(row["body"])
    body["model"] = model
    if model not in NO_SEED:
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


def slug(model: str) -> str:
    return model.replace("/", "_").replace(":", "_")


def run_model(model: str, key: str, requests: list[dict[str, Any]]) -> list[dict[str, Any]]:
    out = ROOT / f"outputs/openrouter-{slug(model)}.json"
    if out.exists():
        doc = json.loads(out.read_text())
        if not any("error" in r for r in doc["results"]):
            print(f"{model}: cached", flush=True)
            return doc["results"]
    started = time.time()
    with concurrent.futures.ThreadPoolExecutor(max_workers=16) as ex:
        results = sorted(ex.map(lambda r: one(r, key, model), requests), key=lambda r: r["slot"])
    errors = [r for r in results if "error" in r]
    out.write_text(canonical({"model": model, "seed": None if model in NO_SEED else SEED, "temperature": 0,
                              "provider": "openrouter", "reasoning": "low",
                              "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(started)),
                              "wall_seconds": round(time.time() - started, 1), "results": results}) + "\n")
    print(f"{model}: {len(results) - len(errors)}/{len(results)} ok in {time.time() - started:.0f}s"
          + (f"; errors e.g. {errors[0]['error'][:160]}" if errors else ""), flush=True)
    return results


def main() -> None:
    import sys
    key = KEY_PATH.read_text().strip()
    models = sys.argv[1:] or MODELS
    requests = [r for r in json.loads((ROOT / "inputs/requests.json").read_text()) if not r["padding"]]
    (ROOT / "outputs").mkdir(exist_ok=True)
    qwen = {r["validator_id"]: r for r in json.loads((ROOT / "outputs/scores.json").read_text())["scores"]}
    columns: dict[str, dict[str, dict[str, Any]]] = {
        "qwen3.8-27b-fp8 (pinned H200)": {v: {"score": r["score"], "entity": r["entity"]} for v, r in qwen.items()}
    }
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(models)) as pool:
        per_model_results = dict(zip(models, pool.map(lambda m: run_model(m, key, requests), models)))
    for model in models:
        results = per_model_results[model]
        columns[model] = {r["validator_id"]: {"score": r["parsed"]["score"], "entity": r["parsed"]["entity"]}
                          for r in results if "error" not in r}
    names = list(columns)
    ids = sorted(qwen, key=lambda v: (-qwen[v]["score"], v))
    matrix = {}
    for a in names:
        matrix[a] = {}
        for b in names:
            common = [v for v in ids if v in columns[a] and v in columns[b]]
            xa = [float(columns[a][v]["score"]) for v in common]
            xb = [float(columns[b][v]["score"]) for v in common]
            matrix[a][b] = {
                "n": len(common),
                "pearson": round(pearson(xa, xb), 3),
                "spearman": round(pearson(ranks(xa), ranks(xb)), 3),
                "recognition_agreement": sum((p > 0) == (q > 0) for p, q in zip(xa, xb)),
            }
    rows = []
    for v in ids:
        scores = {n: columns[n].get(v, {}).get("score") for n in names}
        present = [s for s in scores.values() if s is not None]
        rows.append({"validator_id": v, "network": qwen[v]["network"], "entity": qwen[v]["entity"]
                     or next((columns[n][v]["entity"] for n in names[1:] if v in columns[n] and columns[n][v]["entity"]), None),
                     "scores": scores, "models_recognizing": sum(1 for s in present if s > 0),
                     "mean": round(sum(present) / len(present), 1) if present else None})
    per_model = {n: {"zeros": sum(1 for v in columns[n].values() if v["score"] == 0),
                     "mean": round(sum(v["score"] for v in columns[n].values()) / len(columns[n]), 2),
                     "n": len(columns[n])} for n in names}
    (ROOT / "outputs/cross-model.json").write_text(canonical({"models": names, "per_model": per_model, "matrix": matrix, "rows": rows}) + "\n")

    short = {n: n.split("/")[-1].replace(" (pinned H200)", "")[:14] for n in names}
    print("\nPearson (upper) / Spearman (lower):")
    print(" " * 16 + "".join(f"{short[b]:>15s}" for b in names))
    for a in names:
        cells = []
        for b in names:
            if a == b: cells.append(f"{'-':>15s}")
            elif names.index(b) > names.index(a): cells.append(f"{matrix[a][b]['pearson']:>15.2f}")
            else: cells.append(f"{matrix[a][b]['spearman']:>15.2f}")
        print(f"{short[a]:>16s}" + "".join(cells))
    print("\nrecognition agreement with Qwen: " + ", ".join(f"{short[n]} {matrix[names[0]][n]['recognition_agreement']}/55" for n in names[1:]))
    print("zeros / mean: " + ", ".join(f"{short[n]} {per_model[n]['zeros']}/{per_model[n]['mean']}" for n in names))
    print()
    print(f"{'entity':44s}" + "".join(f"{short[n]:>15s}" for n in names))
    for r in rows:
        if r["models_recognizing"] == 0: continue
        print(f"{str(r['entity'])[:44]:44s}" + "".join(f"{(r['scores'][n] if r['scores'][n] is not None else '?'):>15}" for n in names))


if __name__ == "__main__":
    main()
