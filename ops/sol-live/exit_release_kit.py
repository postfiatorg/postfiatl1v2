"""Stage the inputs for the orchestrator's main-r64-style rolling release."""

import argparse
import hashlib
from pathlib import Path
import re
import shutil
import subprocess
from exit_setup import ROOT, fleet, rpc, save, six

OLD = "main-r64-20261121"
KIT = Path.home() / "var" / OLD


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--release", required=True)
    p.add_argument("--binary", type=Path, required=True)
    args = p.parse_args()
    if not re.fullmatch(r"sol-exit-[a-z0-9-]+", args.release):
        p.error("use a fresh sol-exit-* release id")
    dest = KIT.parent / args.release
    head = fleet()
    if (
        head["build_git_revision"]
        != (KIT / "evidence/source-revision").read_text().strip()[:8]
    ):
        raise ValueError("Live predecessor has changed. Use its signed staging kit.")
    rev = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    dest.mkdir(mode=0o700)
    for name in ("evidence", "bin"):
        (dest / name).mkdir()
    binary = dest / "bin/postfiat-node"
    shutil.copy2(args.binary, binary)
    with binary.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    evidence = dest / "evidence"
    (evidence / "source-revision").write_text(rev + "\n")
    (evidence / "binary.sha256").write_text(f"{digest}  {binary}\n")
    save(
        evidence / "replay-live.json",
        dict(
            validators={
                f"validator-{i}": {"status": v}
                for i, v in enumerate(six(lambda i: rpc(i, "status")))
            }
        ),
    )
    env = (
        (KIT / "env")
        .read_text()
        .replace(OLD, args.release)
        .replace("sol-release6-4-20261119", OLD)
    )
    (dest / "env").write_text(env)
    for name in ("stage.sh", "roll-rest.sh", "deploy_host.sh", "restore-watchdog.sh"):
        script = (KIT / name).read_text().replace(str(KIT) + "/env", str(dest) + "/env")
        (dest / name).write_text(script)
    shutil.copy2(dest / "restore-watchdog.sh", evidence / "restore-watchdog.sh")
    print(dest)


if __name__ == "__main__":
    main()
