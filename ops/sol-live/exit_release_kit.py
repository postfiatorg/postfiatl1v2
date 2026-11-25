"""Stage the inputs for the orchestrator's main-r64-style rolling release."""

import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
from exit_setup import ROOT, fleet, rpc, save, six

# Kits live in ~/var/<release id>.
KIT_ROOT = Path.home() / "var"


def predecessors(environ=None):
    """The release now live (EXIT_OLD_RELEASE) and the one it replaced (EXIT_OLDER_RELEASE). Both
    must be named and differ: there is no default, so a stale name cannot be used silently."""
    environ = os.environ if environ is None else environ
    names = []
    for name in ("EXIT_OLD_RELEASE", "EXIT_OLDER_RELEASE"):
        value = environ.get(name, "")
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", value):
            raise ValueError(f"Set {name} to that release's id; there is no default.")
        names.append(value)
    if names[0] == names[1]:
        raise ValueError("EXIT_OLD_RELEASE and EXIT_OLDER_RELEASE must differ.")
    return tuple(names)


def predecessor_kit(environ=None):
    """The live release's kit, ~/var/<EXIT_OLD_RELEASE>."""
    return KIT_ROOT / predecessors(environ)[0]


def next_env(text, old, older, new):
    """The predecessor's env with the live release renamed to `new` and the replaced one to the
    live one. Both names must occur in it; a replace that finds nothing is refused."""
    for name in (old, older):
        if name not in text:
            raise ValueError(f"{name} does not occur in the predecessor's env.")
    return text.replace(old, new).replace(older, old)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--release", required=True)
    p.add_argument("--binary", type=Path, required=True)
    args = p.parse_args()
    if not re.fullmatch(r"sol-exit-[a-z0-9-]+", args.release):
        p.error("use a fresh sol-exit-* release id")
    OLD, OLDER = predecessors()
    KIT = KIT_ROOT / OLD
    dest = KIT_ROOT / args.release
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
    env = next_env((KIT / "env").read_text(), OLD, OLDER, args.release)
    (dest / "env").write_text(env)
    for name in ("stage.sh", "roll-rest.sh", "deploy_host.sh", "restore-watchdog.sh"):
        script = (KIT / name).read_text().replace(str(KIT) + "/env", str(dest) + "/env")
        if name == "deploy_host.sh":
            lines = script.splitlines(keepends=True)
            restarts = [
                i
                for i, line in enumerate(lines)
                if line.startswith('$SSH "') and "systemctl restart " in line
            ]
            if len(restarts) != 2:
                raise ValueError("Unexpected predecessor restart sequence.")
            # A kit copied from an exit release already carries the guard lines.
            for i in [] if "EXIT_ROLL_GUARD" in script else reversed(restarts):
                lines.insert(
                    i, 'python3 "${EXIT_ROLL_GUARD:?use exit_roll.py}" guard "$N"\n'
                )
            script = "".join(lines)
        (dest / name).write_text(script)
    shutil.copy2(dest / "restore-watchdog.sh", evidence / "restore-watchdog.sh")
    print(dest)


if __name__ == "__main__":
    main()
