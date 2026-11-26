#!/usr/bin/env python3
"""Execution-hole auditor for Post Fiat big_milestone pillar specs.

For every pillars/*.md page, each model receives four inputs, once (no repeated
sampling):
  (1) the founder's original prompt (_private/original_prompt.md),
  (2) the locked founder decisions & live state (_private/founder_decisions.md),
  (3) the master cross-pillar map (README.md), and
  (4) the pillar spec under review.

It scores 0-100 across five explicit sub-scores designed to surface REAL
execution holes (consensus bugs, DoS/griefing, deadlocks, race conditions,
security bypasses, broken migrations, economic loopholes) without rewarding
dense jargon ("claudese") or deferring decisions into "Open questions".

    python3 big_milestone/review.py \
        --prompt-file big_milestone/_private/original_prompt.md \
        --decisions-file big_milestone/_private/founder_decisions.md \
        --key-file ~/repos/open2.txt --round 4 [--wait-for-credit 5]
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
    "Audit this pillar spec as a hostile, pragmatic principal engineer and protocol economist "
    "whose job is to catch REAL EXECUTION HOLES before engineering starts building.\n\n"
    "Rules you MUST follow:\n"
    "1. Input 2 (Locked Founder Decisions & Live State) SUPERSEDES any ambiguity in Input 1. "
    "NEVER re-litigate locked founder decisions, NEVER suggest adding an 'Open questions for the founder' section, "
    "and NEVER penalize a pillar for work explicitly owned by another pillar in Input 3 (README.md).\n"
    "2. Hunt CONCRETE EXECUTION HOLES: consensus/replay bugs, state-machine deadlocks, DoS or griefing vectors, "
    "security/auth bypasses, key-management single points of failure, broken upgrade/migration paths, "
    "cross-pillar interface mismatches, and economic/spam loopholes.\n"
    "3. Do NOT reward dense jargon or 'claudese' (walls of R1..R31 requirement codes). Specs should stay concise, "
    "plain English, and directly executable with clear [ ] milestones and the 4 mandatory ship gates "
    "(1. Executed, 2. Tested by user, 3. Added to docs, 4. Public blog post document).\n\n"
    "Score 0-100 as the EXACT sum of five sub-scores:\n"
    "- Execution Holes & Technical Robustness (0-30)\n"
    "- Economic & Incentive Coherence (0-20)\n"
    "- Founder Decision Alignment & Zero Open Questions (0-20)\n"
    "- Actionable Milestones & 4 Ship Gates (0-15)\n"
    "- Plain-English Readability & Zero Jargon Bloat (0-15)"
)
FORMAT = (
    "Format your answer as Markdown:\n"
    "- First line exactly: `SCORE: <integer 0-100>`\n"
    "- Second line exactly: `SUBSCORES: tech=<0-30> econ=<0-20> align=<0-20> ship=<0-15> read=<0-15>`\n"
    "- Then sections:\n"
    "  - `## Summary judgement` (concise 1-2 paragraphs)\n"
    "  - `## Rubric breakdown` (1-2 lines per sub-score)\n"
    "  - `## Execution holes` — list ONLY real execution holes numbered `H1`, `H2`, ... "
    "(omit cosmetic nits). For each hole give:\n"
    "    - **Severity:** `critical` | `major` | `minor`\n"
    "    - **Where:** section of the pillar spec\n"
    "    - **Hole:** exact technical, security, or economic failure mode\n"
    "    - **Why it breaks execution:** concrete exploit or failure scenario\n"
    "    - **Minimal plain-English fix:** the tightest fix that closes the hole without adding jargon bloat"
)


def build_message(prompt: str, decisions: str, readme: str, page: str) -> str:
    parts = [
        "You are auditing one pillar specification of Post Fiat's Big Milestone plan.\n",
        "## Input 1 — Founder's original rough prompt\n\n<<<ORIGINAL_PROMPT\n"
        + prompt.strip() + "\nORIGINAL_PROMPT>>>\n",
    ]
    if decisions.strip():
        parts.append(
            "## Input 2 — Locked Founder Decisions & Live State (AUTHORITATIVE — overrides Input 1)\n\n"
            "<<<FOUNDER_DECISIONS\n" + decisions.strip() + "\nFOUNDER_DECISIONS>>>\n"
        )
    if readme.strip():
        parts.append(
            "## Input 3 — Master Index & Cross-Pillar Map (README.md)\n\n"
            "<<<MASTER_README\n" + readme.strip() + "\nMASTER_README>>>\n"
        )
    parts.append(
        "## Input 4 — Pillar Spec Under Audit\n\n<<<PILLAR_SPEC\n"
        + page.strip() + "\nPILLAR_SPEC>>>\n"
    )
    parts.append("## Audit Instructions\n\n" + QUESTION + "\n\n" + FORMAT)
    return "\n".join(parts)


def post(path: str, key: str, body: dict | None = None, timeout: int = 900) -> dict:
    data = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request(
        API + path,
        data=data,
        method="POST" if data else "GET",
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.load(response)


def credit_left(key: str) -> float:
    data = post("/credits", key)["data"]
    return float(data["total_credits"]) - float(data["total_usage"])


def call_model(model: str, message: str, key: str) -> dict:
    body = {
        "model": model,
        "max_tokens": 32000,
        "messages": [{"role": "user", "content": message}],
        "usage": {"include": True},
    }
    for attempt in range(3):
        try:
            started = time.time()
            result = post("/chat/completions", key, body)
            content = result["choices"][0]["message"].get("content") or ""
            if not content.strip():
                raise RuntimeError("empty_response")
            finish = result["choices"][0].get("finish_reason")
            if finish == "length":
                raise RuntimeError("truncated_response")
            return {
                "model": model,
                "served_model": result.get("model"),
                "content": content,
                "finish_reason": finish,
                "usage": result.get("usage", {}),
                "latency_s": round(time.time() - started, 1),
            }
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


def parse_subscores(content: str) -> dict[str, int]:
    match = re.search(
        r"SUBSCORES:\s*tech=(\d+)\s+econ=(\d+)\s+align=(\d+)\s+ship=(\d+)\s+read=(\d+)",
        content,
        re.I,
    )
    if not match:
        return {}
    return {
        "tech": int(match.group(1)),
        "econ": int(match.group(2)),
        "align": int(match.group(3)),
        "ship": int(match.group(4)),
        "read": int(match.group(5)),
    }


def extract_holes(content: str) -> list[dict]:
    holes = []
    pat = re.compile(
        r"^(?:#{2,6}\s+|\*\*)(H\d+)(?:\*\*)?(?:[ \t]*[:—–.-]+[ \t]*([^\n]+))?[ \t]*\n"
        r"(.*?)(?=\n(?:#{2,6}\s+|\*\*)H\d+(?:\*\*|\s|:|—|–|-|\.)|\n##\s|\Z)",
        re.M | re.S,
    )
    for m in pat.finditer(content):
        hid, title, body = m.group(1), (m.group(2) or "").strip().strip("*"), m.group(3)
        sev_m = re.search(r"\*\*Severity:?\*\*:?\s*`?(critical|major|minor)`?", body, re.I)
        if not sev_m:
            continue
        sev = sev_m.group(1).lower()
        if not title:
            hole_m = re.search(r"\*\*Hole:?\*\*:?\s*(.+)", body, re.I)
            if hole_m:
                title = hole_m.group(1).strip().split(". ")[0][:160]
            else:
                title = "(see review)"
        holes.append({"id": hid, "severity": sev, "title": title})
    return holes


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
    subscores = {r["model"]: parse_subscores(r["content"]) for r in reviews}
    holes = {r["model"]: extract_holes(r["content"]) for r in reviews}
    valid = [s for s in scores.values() if s is not None]
    mean = round(sum(valid) / len(valid)) if valid else None
    lines = [
        f"# Feedback — {title}",
        "",
        f"*Execution-hole audit · Round {round_no}.*",
        "",
        f"> **Mean score:** {mean if mean is not None else 'n/a'}/100 · "
        + " · ".join(
            f"{MODELS[r['model']]} {scores[r['model']] if scores[r['model']] is not None else 'n/a'}"
            for r in reviews
        ),
        ">",
        f"> **Method:** each model received (1) the founder's original prompt, (2) locked founder decisions & live state, "
        f"(3) master `README.md`, and (4) [this pillar's spec](../../pillars/{pillar.name}), once, scored across 5 explicit "
        f"rubric dimensions (Execution Holes /30, Economics /20, Founder Alignment /20, Milestones & 4 Gates /15, Readability /15). "
        f"Run {run_at} via OpenRouter.",
        "",
    ]
    for review in reviews:
        score = scores[review["model"]]
        subs = subscores[review["model"]]
        sub_txt = (
            f" · `tech={subs['tech']}/30 econ={subs['econ']}/20 align={subs['align']}/20 "
            f"ship={subs['ship']}/15 read={subs['read']}/15`"
            if subs
            else ""
        )
        body = re.sub(r"^\s*SCORE:.*\n?", "", review["content"].strip(), count=1)
        body = re.sub(r"^\s*SUBSCORES:.*\n?", "", body.strip(), count=1)
        lines += [
            f"## {MODELS[review['model']]} — {score if score is not None else 'n/a'}/100",
            "",
            f"*Model `{review.get('served_model') or review['model']}` · "
            f"{review['usage'].get('completion_tokens', '?')} output tokens{sub_txt}*",
            "",
            demote_headings(body.strip()),
            "",
        ]
    (out_dir / pillar.name).write_text("\n".join(lines))
    return {
        "pillar": pillar.name,
        "title": title,
        "scores": scores,
        "subscores": subscores,
        "holes": holes,
        "mean": mean,
    }


def write_holes_summary(rows: list[dict], out_dir: Path, round_no: int, run_at: str) -> None:
    lines = [
        f"# Execution Holes Summary — Round {round_no}",
        "",
        f"*Generated {run_at}. Lists all `critical` and `major` execution holes flagged across all 10 pillars.*",
        "",
    ]
    for row in rows:
        crit_maj = []
        for model, hlist in row.get("holes", {}).items():
            label = MODELS.get(model, model)
            for h in hlist:
                if h["severity"] in ("critical", "major"):
                    crit_maj.append((h["severity"], label, h["id"], h["title"]))
        lines.append(f"## [{row['title']}]({row['pillar']}) — {row['mean']}/100")
        lines.append("")
        if not crit_maj:
            lines.append("- No critical or major execution holes flagged.")
        else:
            for sev, label, hid, title in sorted(crit_maj, key=lambda x: (0 if x[0] == "critical" else 1, x[1], x[2])):
                lines.append(f"- **[{sev.upper()}]** ({label} {hid}) {title}")
        lines.append("")
    (out_dir / "HOLES_SUMMARY.md").write_text("\n".join(lines))


def write_overview() -> None:
    rounds = sorted(
        (int(p.parent.name.split("-")[1]), json.loads(p.read_text()))
        for p in (ROOT / "feedback").glob("round-*/scores.json")
    )
    if not rounds:
        return
    names = sorted({name for _, data in rounds for name in data["pillars"]})
    latest = {
        name: max((n, data["pillars"][name]) for n, data in rounds if name in data["pillars"])
        for name in names
    }
    head = ["Pillar"] + [f"Round {n}" for n, _ in rounds] + ["Current", *MODELS.values(), "Change"]
    table = ["| " + " | ".join(head) + " |", "|" + "---|" * len(head)]
    for name in names:
        n_last, row = latest[name]
        cells = [f"[{row['title']}](round-{n_last}/{name})"]
        for n, data in rounds:
            mean = data["pillars"].get(name, {}).get("mean")
            cells.append(f"[{mean}](round-{n}/{name})" if mean is not None else "—")
        cells.append(f"**{row['mean']}** (round {n_last})")
        cells += [
            str(row["scores"].get(m)) if row["scores"].get(m) is not None else "n/a"
            for m in MODELS
        ]
        first = rounds[0][1]["pillars"].get(name, {}).get("mean")
        cells.append(f"{row['mean'] - first:+d}" if None not in (first, row["mean"]) else "n/a")
        table.append("| " + " | ".join(cells) + " |")
    per_round = []
    for n, data in rounds:
        means = [r["mean"] for r in data["pillars"].values() if r["mean"] is not None]
        per_round.append(f"round {n}: {round(sum(means) / len(means))} over {len(means)} pillars ({data['run_at']})")
    current = [row["mean"] for _, row in latest.values() if row["mean"] is not None]
    last_round = rounds[-1][0]
    holes_link = (
        f" · [Round {last_round} Execution Holes Summary](round-{last_round}/HOLES_SUMMARY.md)"
        if (ROOT / "feedback" / f"round-{last_round}" / "HOLES_SUMMARY.md").exists()
        else ""
    )
    text = [
        "# Feedback — Overview",
        "",
        f"> **Mean score:** {round(sum(current) / len(current))}/100 across the current {len(current)} pillar specs{holes_link}",
        ">",
        f"> **Method (Round 4+):** For each pillar, {' and '.join(MODELS.values())} receive (1) the founder's original prompt, "
        "(2) locked founder decisions & live state, (3) master `README.md`, and (4) the pillar spec, scored across five explicit "
        "dimensions: Execution Holes & Technical Robustness (30), Economics (20), Founder Alignment & Zero Open Questions (20), "
        "Actionable Milestones & 4 Ship Gates (15), and Plain-English Readability (15). Revisions are evaluated by closing real "
        "critical/major execution holes — never by blindly reverting on noisy ±1-point score jitter.",
        "",
        "Rounds: " + " · ".join(per_round),
        "",
        *table,
        "",
    ]
    (ROOT / "feedback" / "README.md").write_text("\n".join(text))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--prompt-file", required=True)
    parser.add_argument("--decisions-file", default=str(ROOT / "_private" / "founder_decisions.md"))
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
    decisions_path = Path(args.decisions_file)
    decisions = decisions_path.read_text() if decisions_path.exists() else ""
    readme = (ROOT / "README.md").read_text() if (ROOT / "README.md").exists() else ""
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
            message = build_message(prompt, decisions, readme, pillar.read_text())
            for model in MODELS:
                jobs[pool.submit(call_model, model, message, key)] = (pillar, model)
        results: dict[tuple[Path, str], dict] = {}
        for future in cf.as_completed(jobs):
            pillar, model = jobs[future]
            review = future.result()
            results[(pillar, model)] = review
            raw = out_dir / "raw" / f"{pillar.stem}--{model.split('/')[-1]}.json"
            raw.write_text(json.dumps({**review, "pillar": pillar.name, "run_at": run_at}, indent=2))
            print(
                f"done {pillar.stem} {model} score={parse_score(review['content'])} "
                f"subs={parse_subscores(review['content'])} "
                f"out={review['usage'].get('completion_tokens')} cost={review['usage'].get('cost')}",
                flush=True,
            )
    rows = [write_feedback(p, [results[(p, m)] for m in MODELS], run_at, out_dir, args.round) for p in pillars]
    (out_dir / "scores.json").write_text(json.dumps({"run_at": run_at, "pillars": {r["pillar"]: r for r in rows}}, indent=2))
    write_holes_summary(rows, out_dir, args.round, run_at)
    write_overview()
    total = sum(float(r["usage"].get("cost") or 0) for r in results.values())
    print(f"wrote {len(rows)} feedback pages + HOLES_SUMMARY.md; total cost ${total:.2f}")


if __name__ == "__main__":
    main()
