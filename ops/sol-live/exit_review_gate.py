"""Run and count the exit_review regressions from the integrated release source."""
import argparse
from collections import Counter
import json
import os
from pathlib import Path
import re
import subprocess
import tomllib


def source_cases(root):
    cases = {}
    for crate in (root / "crates").iterdir():
        manifest = crate / "Cargo.toml"
        if not manifest.is_file():
            continue
        names = []
        for path in crate.rglob("*.rs"):
            names += re.findall(r"^\s*fn (exit_review\d*_\w+)\s*\(", path.read_text(), re.M)
        if names:
            package = tomllib.loads(manifest.read_text())["package"]["name"]
            cases[package] = sorted(names)
    return cases


def run_gate(execute, evidence, expected):
    listing = execute(["--list"])
    names = re.findall(r"^((?:\w+::)*exit_review\d*_\w+): test$", listing, re.M)
    if not names or Counter(n.split("::")[-1] for n in names) != Counter(expected):
        raise ValueError("Release source and executable exit review inventories differ.")
    for name in names:
        log = execute([name, "--include-ignored", "--exact"])
        (evidence / (name.replace("::", "-") + ".log")).write_text(log)
        if not re.search(r"^test result: ok\. 1 passed; 0 failed; 0 ignored;", log, re.M):
            raise ValueError(f"Expected exactly one passing test for {name}")
    return len(names)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    for command in ("packages", "run"):
        p = commands.add_parser(command)
        p.add_argument("source", type=Path)
    p.add_argument("evidence", type=Path)
    p.add_argument("archive", type=Path)
    p.add_argument("integrity", type=Path)
    p.add_argument("faucet", type=Path)
    args = parser.parse_args()
    cases = source_cases(args.source)
    node = cases.get("postfiat-node", [])
    if sum(n.startswith("exit_review_") for n in node) < 9 or sum(n.startswith("exit_review4_") for n in node) < 8:
        raise ValueError("Missing reviewed node tests in release source.")
    if args.action == "packages":
        print("\n".join(sorted(cases)))
        return
    records = [json.loads(line) for line in (args.evidence/"interlock-build.jsonl").read_text().splitlines()]
    passed = 0
    vd = "/var/lib/postfiat/validator-3"
    for package, expected in sorted(cases.items()):
        (binary,) = [r["executable"] for r in records
                     if r.get("executable") and r["profile"]["test"]
                     and r["target"]["name"] == package.replace("-", "_")
                     and "lib" in r["target"]["kind"]]
        cmd = ["docker", "run", "--rm", "--network", "none", "--cap-drop", "ALL",
               "--security-opt", "no-new-privileges", "-u", f"{os.getuid()}:{os.getgid()}",
               "-v", f"{args.archive.resolve()}:{vd}", "-v", f"{args.integrity.resolve()}:{vd}/.integrity.key:ro",
               "-v", f"{args.faucet.resolve()}:{vd}/faucet_account.json:ro",
               "-v", f"{Path(binary).resolve()}:/opt/test:ro", "ubuntu:24.04", "/opt/test"]
        def execute(flags):
            return subprocess.check_output(cmd + flags, text=True)
        count = run_gate(execute, args.evidence, expected)
        passed += count
        print(f"{package}: executed {count} review tests.", flush=True)
    print(f"Executed {passed} of {sum(map(len, cases.values()))} source review tests.")


if __name__ == "__main__":
    main()
