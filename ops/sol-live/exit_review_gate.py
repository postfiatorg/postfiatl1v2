"""Run every exit_review regression discovered in the release test executable."""
import argparse
from pathlib import Path
import re
import subprocess


def run_gate(execute, evidence):
    listing = execute(["--list"])
    names = re.findall(r"^([\w:]*::exit_review\d*_\w+): test$", listing, re.M)
    if (sum("::exit_review_" in n for n in names) < 9
            or sum("::exit_review4_" in n for n in names) < 8 or len(set(names)) != len(names)):
        raise ValueError("Release executable lacks the reviewed regression inventory.")
    passed = 0
    for name in names:
        log = execute([name, "--include-ignored", "--exact"])
        (evidence / (name.replace("::", "-") + ".log")).write_text(log)
        if not re.search(r"^test result: ok\. 1 passed; 0 failed; 0 ignored;", log, re.M):
            raise ValueError(f"Expected exactly one passing test for {name}")
        passed += 1
    print(f"Executed {passed} of {len(names)} discovered exit review tests.")
    return passed


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("runner", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    def execute(flags):
        return subprocess.check_output(args.runner + flags, text=True)
    run_gate(execute, args.evidence)
