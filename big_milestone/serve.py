#!/usr/bin/env python3
"""Serve the big_milestone specs as a small browsable site.

Pages are the markdown files in this folder: README.md is the index and
pillars/*.md are the pillar specs, changelog/*.md
record how each review round was handled, feedback/ holds the reviews. Rendering happens per request, so editing
a file and refreshing the browser is the whole publishing loop.

    python3 big_milestone/serve.py --host 100.89.219.44 --port 8844

Diagrams are ```mermaid fences rendered in the browser. mermaid.min.js is
served from big_milestone/_vendor/ when present (untracked; see README) and
otherwise loaded from a pinned CDN URL.
"""

from __future__ import annotations

import argparse
import html
import re
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import markdown

ROOT = Path(__file__).resolve().parent
VENDOR = ROOT / "_vendor"
MERMAID_CDN = "https://cdn.jsdelivr.net/npm/mermaid@11.17.2/dist/mermaid.min.js"
STATUS_RE = re.compile(r"^>\s*\*\*Status:\*\*\s*(.+)$", re.MULTILINE)
SCORE_RE = re.compile(r"^>\s*\*\*Mean score:\*\*\s*(\d+)", re.MULTILINE)


def route_for(path: Path) -> str:
    rel = path.resolve().relative_to(ROOT)
    if rel.name == "README.md":
        return "/" if len(rel.parts) == 1 else f"/{rel.parts[0]}"
    return "/" + str(rel.with_suffix(""))


def pages() -> list[dict]:
    files = [ROOT / "README.md", *sorted((ROOT / "pillars").glob("*.md"))]
    rounds = sorted((ROOT / "feedback").glob("round-*"), key=lambda d: int(d.name.split("-")[1]))
    if (ROOT / "feedback" / "README.md").exists():
        files.append(ROOT / "feedback" / "README.md")
    files += sorted((page for round_dir in rounds for page in round_dir.glob("[0-9]*.md")),
                    key=lambda page: (page.name, int(page.parent.name.split("-")[1])))
    files += sorted((ROOT / "changelog").glob("*.md"))
    # Each pillar's feedback entry in the nav is its latest surviving review:
    # a reverted revision's review is deleted, so later rounds can be partial.
    latest_for = {}
    for round_dir in rounds:
        for page in round_dir.glob("[0-9]*.md"):
            latest_for[page.name] = round_dir.name
    items = []
    for path in files:
        text = path.read_text(encoding="utf-8")
        title = next((line[2:].strip() for line in text.splitlines() if line.startswith("# ")), path.stem)
        status = STATUS_RE.search(text)
        score = SCORE_RE.search(text)
        is_round_page = "feedback" in path.parts and path.parent.name.startswith("round-")
        if is_round_page:
            title = f"{title} (round {path.parent.name.split('-')[1]})"
        items.append({"route": route_for(path), "path": path, "title": title,
                      "status": status.group(1).strip() if status else (f"{score.group(1)}/100" if score else ""),
                      "section": "Specs" if "feedback" not in path.parts else "Feedback · latest",
                      "in_nav": "changelog" not in path.parts
                      and (not is_round_page or latest_for.get(path.name) == path.parent.name)})
    return items


def rewrite_links(body: str, page: Path) -> str:
    # Links between pages are written as relative .md files so they also work
    # on GitHub; turn them into site routes here.
    def fix(match: re.Match) -> str:
        target = match.group(1)
        if target.startswith(("http://", "https://", "#", "mailto:", "/")):
            return match.group(0)
        anchor = ""
        if "#" in target:
            target, anchor = target.split("#", 1)
            anchor = "#" + anchor
        if not target.endswith(".md"):
            return match.group(0)
        resolved = (page.parent / target).resolve()
        try:
            return f'href="{route_for(resolved)}{anchor}"'
        except ValueError:
            return match.group(0)

    return re.sub(r'href="([^"]+)"', fix, body)


def render_checkboxes(body: str) -> str:
    body = re.sub(
        r"<li>\s*\[ \]\s*",
        '<li class="task-item"><input type="checkbox" disabled> ',
        body,
    )
    body = re.sub(
        r"<li>\s*\[[xX]\]\s*",
        '<li class="task-item is-done"><input type="checkbox" disabled checked> ',
        body,
    )
    return body


def render(item: dict, nav: list[dict]) -> str:
    text = item["path"].read_text(encoding="utf-8")
    md = markdown.Markdown(extensions=["tables", "fenced_code", "toc", "sane_lists", "attr_list"],
                           extension_configs={"toc": {"permalink": False}})
    body = render_checkboxes(rewrite_links(md.convert(text), item["path"]))
    links = []
    section = ""
    for entry in (e for e in nav if e["in_nav"]):
        if entry["section"] != section and entry["section"].startswith("Feedback"):
            links.append(f'<div class="nav-section">{html.escape(entry["section"])}</div>')
        section = entry["section"]
        active = " is-active" if entry["route"] == item["route"] else ""
        title = entry["title"].replace("Feedback — ", "")
        if entry["route"] in ("/", "/feedback"):
            label = "Overview"
        else:
            # "Pillar 3 — Validator Rewards Program" -> "3 · Validator Rewards Program"
            number, _, name = title.partition(" — ")
            label = f'{number.replace("Pillar ", "")} · {name}' if name else title
        label = html.escape(label)
        word = entry["status"].split(" ")[0].strip(".,;:") if entry["status"] else ""
        status = f'<span class="nav-status">{html.escape(word)}</span>' if word else ""
        links.append(f'<a class="nav-link{active}" href="{entry["route"]}">{label}{status}</a>')
    toc = md.toc if item["route"] not in ("/", "/feedback") else ""
    mermaid_src = "/_vendor/mermaid.min.js" if (VENDOR / "mermaid.min.js").exists() else MERMAID_CDN
    return PAGE.format(title=html.escape(item["title"]), nav="\n".join(links), body=body, toc=toc, mermaid=mermaid_src)


class Handler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802
        route = self.path.split("?", 1)[0].rstrip("/") or "/"
        if route == "/_vendor/mermaid.min.js" and (VENDOR / "mermaid.min.js").exists():
            return self.send(200, (VENDOR / "mermaid.min.js").read_bytes(), "text/javascript")
        nav = pages()
        item = next((entry for entry in nav if entry["route"] == route), None)
        if item is None:
            return self.send(404, b"Not found", "text/plain")
        self.send(200, render(item, nav).encode("utf-8"), "text/html; charset=utf-8")

    def send(self, code: int, body: bytes, kind: str) -> None:
        self.send_response(code)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args) -> None:
        pass


PAGE = """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · Post Fiat Big Milestone</title>
<style>
:root {{ --bg:#f7f6f2; --panel:#fff; --ink:#16161a; --muted:#6b6b73; --line:#e4e2db; --accent:#1f4fd8; --code:#f1efe8; }}
* {{ box-sizing:border-box; }}
body {{ margin:0; background:var(--bg); color:var(--ink); font:16px/1.6 -apple-system, "Segoe UI", Inter, sans-serif; }}
.layout {{ display:grid; grid-template-columns:300px minmax(0,1fr) 220px; min-height:100vh; }}
nav.side {{ border-right:1px solid var(--line); padding:24px 14px; position:sticky; top:0; height:100vh; overflow:auto; background:var(--panel); }}
.brand {{ font-weight:700; letter-spacing:.02em; margin:0 8px 18px; font-size:14px; text-transform:uppercase; color:var(--muted); }}
.nav-link {{ display:flex; justify-content:space-between; align-items:baseline; gap:8px; padding:7px 10px; line-height:1.35; border-radius:7px; color:var(--ink); text-decoration:none; font-size:14px; }}
.nav-link:hover {{ background:var(--code); }}
.nav-link.is-active {{ background:var(--ink); color:#fff; }}
.nav-section {{ margin:22px 8px 6px; font-size:12px; font-weight:700; text-transform:uppercase; letter-spacing:.04em; color:var(--muted); }}
.nav-status {{ font-size:11px; opacity:.7; white-space:nowrap; }}
main {{ padding:40px 56px 96px; max-width:980px; }}
aside.toc {{ padding:40px 16px; position:sticky; top:0; height:100vh; overflow:auto; font-size:13px; }}
aside.toc ul {{ list-style:none; padding-left:10px; margin:0; }}
aside.toc > .toc > ul {{ padding-left:0; }}
aside.toc a {{ color:var(--muted); text-decoration:none; display:block; padding:2px 0; }}
aside.toc a:hover {{ color:var(--accent); }}
h1 {{ font-size:34px; line-height:1.2; margin:0 0 8px; }}
h2 {{ margin-top:44px; padding-top:12px; border-top:1px solid var(--line); font-size:23px; }}
h3 {{ margin-top:28px; font-size:18px; }}
a {{ color:var(--accent); }}
blockquote {{ margin:16px 0; padding:10px 16px; background:var(--panel); border-left:3px solid var(--accent); border-radius:0 8px 8px 0; color:var(--muted); }}
blockquote p {{ margin:4px 0; }}
code {{ background:var(--code); padding:1px 5px; border-radius:4px; font-size:.88em; }}
pre {{ background:#16161a; color:#e9e7e1; padding:14px 16px; border-radius:9px; overflow:auto; font-size:13px; line-height:1.5; }}
pre code {{ background:none; padding:0; color:inherit; }}
table {{ border-collapse:collapse; width:100%; margin:16px 0; font-size:14px; background:var(--panel); }}
th, td {{ border:1px solid var(--line); padding:7px 10px; text-align:left; vertical-align:top; }}
th {{ background:var(--code); }}
.mermaid {{ background:var(--panel); border:1px solid var(--line); border-radius:9px; padding:16px; margin:18px 0; text-align:center; }}
li.task-item {{ list-style:none; margin-left:-20px; }}
li.task-item input[type="checkbox"] {{ margin-right:8px; accent-color:var(--accent); transform:scale(1.1); vertical-align:middle; }}
li.task-item.is-done {{ color:var(--muted); text-decoration:line-through; }}
@media (max-width: 1100px) {{ .layout {{ grid-template-columns:220px minmax(0,1fr); }} aside.toc {{ display:none; }} main {{ padding:28px; }} }}
@media (max-width: 720px) {{ .layout {{ display:block; }} nav.side {{ position:static; height:auto; }} }}
</style></head>
<body><div class="layout">
<nav class="side"><div class="brand">Post Fiat · Big Milestone</div>
{nav}
</nav>
<main>{body}</main>
<aside class="toc">{toc}</aside>
</div>
<script src="{mermaid}"></script>
<script>
for (const block of document.querySelectorAll("pre > code.language-mermaid")) {{
  const div = document.createElement("div"); div.className = "mermaid"; div.textContent = block.textContent;
  block.parentElement.replaceWith(div);
}}
if (window.mermaid) {{ mermaid.initialize({{ startOnLoad:false, theme:"neutral", securityLevel:"strict" }}); mermaid.run(); }}
</script>
</body></html>
"""


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8844)
    args = parser.parse_args()
    ThreadingHTTPServer((args.host, args.port), Handler).serve_forever()


if __name__ == "__main__":
    main()
