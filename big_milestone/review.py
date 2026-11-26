#!/usr/bin/env python3
"""Score each pillar spec against the founder's original prompt.

For every pillars/*.md page, each model gets two inputs, once (no repeated
sampling): (1) the founder's original prompt and (2) the pillar page. It is
asked the founder's question: how well the page corresponds to the founder's
intention and how likely it is to produce something that works (technically
robust, economically coherent), with reasoning that enumerates problems for
later agents. Results are written to feedback/<pillar>.md, feedback/README.md
and feedback/raw/*.json.

    python3 big_milestone/review.py --prompt-file big_milestone/_private/original_prompt.md \
        --key-file ~/repos/open2.txt [--wait-for-credit 5]

The original prompt stays outside git (big_milestone/_private/ is ignored).
"""

from __future__ import annotations

import argparse
import concurrent.futures as cf
import datetime as dt
import json
import re
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent
API = "https://openrouter.ai/api/v1"
MODELS = {
    "google/gemini-4-neon": "Gemini 4 Neon",
    "anthropic/claude-opus-5.5": "Claude Opus 5.5",
}
QUESTION = (
    "On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, "
    "and how likely is it to output something that works for the founder and the project — with "
    "'works' meaning technically robust and economically coherent?\n\n"
    "Output your reasoning at length, enumerating problems clearly so that subsequent agents can "
    "investigate and improve."
)
FORMAT = (
    "Format your answer as Markdown:\n"
    "- First line exactly: `SCORE: <integer 0-100>`\n"
    "- Then sections: Summary judgement; Correspondence to the founder's intention; "
    "Technical robustness; Economic coherence; Problems.\n"
    "- In Problems, number each item P1, P2, … and give for each: severity (critical / major / minor), "
    "where in the sub-page it occurs, what is wrong, why it matters, how an agent should investigate it, "
    "and a suggested direction for the fix."
)


def build_message(prompt: str, page: str) -> str:
    return (
        "You are reviewing one sub-page of a planning document for Post Fiat.\n\n"
        "## Input 1 — the founder's original prompt\n\n<<<ORIGINAL_PROMPT\n" + prompt.strip() + "\nORIGINAL_PROMPT>>>\n\n"
        "## Input 2 — the sub-page (a spec written from that prompt for one deliverable)\n\n<<<SUB_PAGE\n"
        + page.strip() + "\nSUB_PAGE>>>\n\n## Question\n\n" + QUESTION + "\n\n" + FORMAT
    )


def post(path: str, key: str, body: dict | None = None, timeout: int = 900) -> dict:
    data = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request(API + path, data=data, method="POST" if data else "GET",
                                     headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.load(response)


def credit_left(key: str) -> float:
    data = post("/credits", key)["data"]
    return float(data["total_credits"]) - float(data["total_usage"])


def call_model(model: str, message: str, key: str) -> dict:
    body = {"model": model, "max_tokens": 32000, "messages": [{"role": "user", "content": message}], "usage": {"include": True}}
    # One sample per model. Retries cover transport and provider failures only;
    # a returned answer is never resampled.
    for attempt in range(3):
        try:
            started = time.time()
            result = post("/chat/completions", key, body)
            content = result["choices"][0]["message"].get("content") or ""
            if not content.strip():
                raise RuntimeError("empty_response")
            return {"model": model, "served_model": result.get("model"), "content": content,
                    "usage": result.get("usage", {}), "latency_s": round(time.time() - started, 1)}
        except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError, RuntimeError, KeyError) as error:
            detail = error.read().decode()[:300] if isinstance(error, urllib.error.HTTPError) else str(error)
            if isinstance(error, urllib.error.HTTPError) and error.code == 402:
                raise RuntimeError(f"{model}: insufficient OpenRouter credit: {detail}") from error
            if attempt == 2:
                raise RuntimeError(f"{model}: {detail}") from error
            time.sleep(10 * (attempt + 1))
    raise AssertionError("unreachable")


def parse_score(content: str) -> int | None:
    match = re.search(r"SCORE:\s*\**\s*(\d{1,3})", content)
    return min(100, int(match.group(1))) if match else None


def demote_headings(content: str, levels: int = 2) -> str:
    out, fence = [], False
    for line in content.splitlines():
        if line.startswith("```"):
            fence = not fence
        if not fence and re.match(r"^#{1,4} ", line):
            line = "#" * levels + line
        out.append(line)
    return "\n".join(out)


def page_title(path: Path) -> str:
    return next(line[2:].strip() for line in path.read_text().splitlines() if line.startswith("# "))


def write_feedback(pillar: Path, reviews: list[dict], run_at: str) -> dict:
    title = page_title(pillar)
    scores = {r["model"]: parse_score(r["content"]) for r in reviews}
    valid = [s for s in scores.values() if s is not None]
    mean = round(sum(valid) / len(valid)) if valid else None
    lines = [f"# Feedback — {title}", "",
             f"> **Mean score:** {mean if mean is not None else 'n/a'}/100 · "
             + " · ".join(f"{MODELS[r['model']]} {scores[r['model']] if scores[r['model']] is not None else 'n/a'}" for r in reviews),
             ">",
             f"> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../pillars/{pillar.name}), "
             f"once, with the question below. Run {run_at} via OpenRouter.", "",
             "## The question", "", "> " + QUESTION.replace("\n\n", "\n>\n> "), ""]
    for review in reviews:
        score = scores[review["model"]]
        body = re.sub(r"^\s*SCORE:.*\n?", "", review["content"].strip(), count=1)
        lines += [f"## {MODELS[review['model']]} — {score if score is not None else 'n/a'}/100", "",
                  f"*Model `{review.get('served_model') or review['model']}` · {review['usage'].get('completion_tokens', '?')} output tokens*", "",
                  demote_headings(body.strip()), ""]
    out = ROOT / "feedback" / pillar.name
    out.write_text("\n".join(lines))
    return {"pillar": pillar, "title": title, "scores": scores, "mean": mean}


def write_overview(rows: list[dict], run_at: str) -> None:
    head = "| Pillar | " + " | ".join(MODELS.values()) + " | Mean |"
    sep = "|---|" + "---|" * len(MODELS) + "---|"
    table = [head, sep]
    for row in rows:
        cells = [str(row["scores"].get(m)) if row["scores"].get(m) is not None else "n/a" for m in MODELS]
        table.append(f"| [{row['title']}]({row['pillar'].name}) | " + " | ".join(cells) + f" | **{row['mean']}** |")
    means = [r["mean"] for r in rows if r["mean"] is not None]
    text = ["# Feedback — Overview", "",
            f"> **Mean score:** {round(sum(means) / len(means)) if means else 'n/a'}/100 across {len(rows)} pillars",
            ">",
            f"> **Method:** for each pillar, {' and '.join(MODELS.values())} each received the founder's original prompt and that "
            f"pillar's spec once, and scored 0–100 how well it matches the founder's intention and how likely it is to "
            f"work (technically robust, economically coherent). Run {run_at}. Each pillar's page has both reviews in full, "
            "with numbered problems for follow-up agents.", "", *table, ""]
    (ROOT / "feedback" / "README.md").write_text("\n".join(text))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--prompt-file", required=True)
    parser.add_argument("--key-file", required=True)
    parser.add_argument("--wait-for-credit", type=float, default=0.0, help="poll until this many USD of credit are available")
    args = parser.parse_args()
    key = Path(args.key_file).expanduser().read_text().split()[0]
    prompt = Path(args.prompt_file).read_text()
    if args.wait_for_credit:
        while (left := credit_left(key)) < args.wait_for_credit:
            print(f"waiting for credit: ${left:.2f} < ${args.wait_for_credit:.2f}", flush=True)
            time.sleep(60)
    pillars = sorted((ROOT / "pillars").glob("*.md"))
    (ROOT / "feedback" / "raw").mkdir(parents=True, exist_ok=True)
    run_at = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    jobs = {}
    with cf.ThreadPoolExecutor(max_workers=10) as pool:
        for pillar in pillars:
            message = build_message(prompt, pillar.read_text())
            for model in MODELS:
                jobs[pool.submit(call_model, model, message, key)] = (pillar, model)
        results: dict[tuple[Path, str], dict] = {}
        for future in cf.as_completed(jobs):
            pillar, model = jobs[future]
            review = future.result()
            results[(pillar, model)] = review
            raw = ROOT / "feedback" / "raw" / f"{pillar.stem}--{model.split('/')[-1]}.json"
            raw.write_text(json.dumps({**review, "pillar": pillar.name, "run_at": run_at}, indent=2))
            print(f"done {pillar.stem} {model} score={parse_score(review['content'])} "
                  f"out={review['usage'].get('completion_tokens')} cost={review['usage'].get('cost')}", flush=True)
    rows = [write_feedback(p, [results[(p, m)] for m in MODELS], run_at) for p in pillars]
    write_overview(rows, run_at)
    total = sum(float(r["usage"].get("cost") or 0) for r in results.values())
    print(f"wrote {len(rows)} feedback pages; total cost ${total:.2f}")


if __name__ == "__main__":
    main()
