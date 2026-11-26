#!/usr/bin/env python3
"""Score each pillar spec against the founder's original prompt.

For every pillars/*.md page, each model gets two inputs, once (no repeated
sampling): (1) the founder's original prompt and (2) the pillar page. It is
asked the founder's question: how well the page corresponds to the founder's
intention and how likely it is to produce something that works (technically
robust, economically coherent), with reasoning that enumerates problems for
later agents. Results are written to feedback/round-N/<pillar>.md, raw JSON and
scores.json, and feedback/README.md compares rounds.

    python3 big_milestone/review.py --prompt-file big_milestone/_private/original_prompt.md \
        --key-file ~/repos/open2.txt --round 2 [--wait-for-credit 5]

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


def write_feedback(pillar: Path, reviews: list[dict], run_at: str, out_dir: Path, round_no: int) -> dict:
    title = page_title(pillar)
    scores = {r["model"]: parse_score(r["content"]) for r in reviews}
    valid = [s for s in scores.values() if s is not None]
    mean = round(sum(valid) / len(valid)) if valid else None
    lines = [f"# Feedback — {title}", "", f"*Review round {round_no}.*", "",
             f"> **Mean score:** {mean if mean is not None else 'n/a'}/100 · "
             + " · ".join(f"{MODELS[r['model']]} {scores[r['model']] if scores[r['model']] is not None else 'n/a'}" for r in reviews),
             ">",
             f"> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/{pillar.name}), "
             f"once, with the question below. Run {run_at} via OpenRouter.", "",
             "## The question", "", "> " + QUESTION.replace("\n\n", "\n>\n> "), ""]
    for review in reviews:
        score = scores[review["model"]]
        body = re.sub(r"^\s*SCORE:.*\n?", "", review["content"].strip(), count=1)
        lines += [f"## {MODELS[review['model']]} — {score if score is not None else 'n/a'}/100", "",
                  f"*Model `{review.get('served_model') or review['model']}` · {review['usage'].get('completion_tokens', '?')} output tokens*", "",
                  demote_headings(body.strip()), ""]
    (out_dir / pillar.name).write_text("\n".join(lines))
    return {"pillar": pillar.name, "title": title, "scores": scores, "mean": mean}


def write_overview() -> None:
    rounds = sorted((int(p.parent.name.split("-")[1]), json.loads(p.read_text()))
                    for p in (ROOT / "feedback").glob("round-*/scores.json"))
    if not rounds:
        return
    latest_no, latest = rounds[-1]
    head = ["Pillar"] + [f"Round {n} mean" for n, _ in rounds] + [f"R{latest_no} {name}" for name in MODELS.values()]
    if len(rounds) > 1:
        head.append(f"Change R{rounds[0][0]}→R{latest_no}")
    table = ["| " + " | ".join(head) + " |", "|" + "---|" * len(head)]
    for name, row in latest["pillars"].items():
        cells = [f"[{row['title']}](round-{latest_no}/{name})"]
        for n, data in rounds:
            mean = data["pillars"].get(name, {}).get("mean")
            cells.append(f"[{mean}](round-{n}/{name})" if mean is not None else "n/a")
        cells += [str(row["scores"].get(m)) if row["scores"].get(m) is not None else "n/a" for m in MODELS]
        if len(rounds) > 1:
            first = rounds[0][1]["pillars"].get(name, {}).get("mean")
            cells.append(f"**{row['mean'] - first:+d}**" if None not in (first, row["mean"]) else "n/a")
        table.append("| " + " | ".join(cells) + " |")
    overall = []
    for n, data in rounds:
        means = [r["mean"] for r in data["pillars"].values() if r["mean"] is not None]
        overall.append(f"round {n}: **{round(sum(means) / len(means))}**/100 ({data['run_at']})")
    latest_means = [r["mean"] for r in latest["pillars"].values() if r["mean"] is not None]
    text = ["# Feedback — Overview", "",
            f"> **Mean score:** {round(sum(latest_means) / len(latest_means))}/100 in round {latest_no} across {len(latest_means)} pillars",
            ">",
            f"> **Method:** for each pillar, {' and '.join(MODELS.values())} each received the founder's original prompt and that "
            "pillar's spec once, and scored 0–100 how well it matches the founder's intention and how likely it is to "
            "work (technically robust, economically coherent). Each round's pages hold both reviews in full, with "
            "numbered problems for follow-up agents. Pillars are revised from one round's problems before the next round.",
            "", "Overall: " + " · ".join(overall), "", *table, ""]
    (ROOT / "feedback" / "README.md").write_text("\n".join(text))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--prompt-file", required=True)
    parser.add_argument("--key-file", required=True)
    parser.add_argument("--round", type=int, required=True, help="review round number; output goes to feedback/round-N/")
    parser.add_argument("--overview-only", action="store_true", help="only rebuild feedback/README.md from saved rounds")
    parser.add_argument("--wait-for-credit", type=float, default=0.0, help="poll until this many USD of credit are available")
    args = parser.parse_args()
    if args.overview_only:
        write_overview()
        return
    key = Path(args.key_file).expanduser().read_text().split()[0]
    prompt = Path(args.prompt_file).read_text()
    if args.wait_for_credit:
        while (left := credit_left(key)) < args.wait_for_credit:
            print(f"waiting for credit: ${left:.2f} < ${args.wait_for_credit:.2f}", flush=True)
            time.sleep(60)
    pillars = sorted((ROOT / "pillars").glob("*.md"))
    out_dir = ROOT / "feedback" / f"round-{args.round}"
    (out_dir / "raw").mkdir(parents=True, exist_ok=True)
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
            raw = out_dir / "raw" / f"{pillar.stem}--{model.split('/')[-1]}.json"
            raw.write_text(json.dumps({**review, "pillar": pillar.name, "run_at": run_at}, indent=2))
            print(f"done {pillar.stem} {model} score={parse_score(review['content'])} "
                  f"out={review['usage'].get('completion_tokens')} cost={review['usage'].get('cost')}", flush=True)
    rows = [write_feedback(p, [results[(p, m)] for m in MODELS], run_at, out_dir, args.round) for p in pillars]
    (out_dir / "scores.json").write_text(json.dumps({"run_at": run_at, "pillars": {r["pillar"]: r for r in rows}}, indent=2))
    write_overview()
    total = sum(float(r["usage"].get("cost") or 0) for r in results.values())
    print(f"wrote {len(rows)} feedback pages; total cost ${total:.2f}")


if __name__ == "__main__":
    main()
