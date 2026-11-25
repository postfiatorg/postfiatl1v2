"""Build-stage-only rootfs assembly. This file is excluded from the runtime."""

import json
import os
from pathlib import Path
import shutil
import subprocess


root = Path("/rootfs")
root.mkdir()

# The collector has no interactive terminal, SQL database, UUID generator,
# local DBM store, or compressed-file input. Omit their native extensions and
# libraries instead of carrying unused parsers into the measured image.
UNUSED_EXTENSIONS = ("_tkinter", "_sqlite3", "_curses", "readline", "_uuid", "_dbm", "_gdbm", "_bz2", "_lzma")


def copy(source, target=None):
    source = Path(source)
    destination = root / str(target or source).lstrip("/")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    return destination


copy("/usr/local/bin/python3.13")
shutil.copytree("/usr/local/lib/python3.13", root / "usr/local/lib/python3.13", ignore=shutil.ignore_patterns(
    "site-packages", "__pycache__", "test", "tests", "idlelib", "tkinter", "sqlite3", "curses", "dbm",
    "turtledemo", "ensurepip", "venv", *(name + "*" for name in UNUSED_EXTENSIONS)))
packages = root / "usr/local/lib/python3.13/site-packages"
shutil.copytree("/build/packages", packages, ignore=shutil.ignore_patterns("__pycache__", "bin"))
shutil.copytree("/build/source/pftl_schwab_tee", packages / "pftl_schwab_tee")
copy("/build/libnsm.so", "/usr/local/lib/libnsm.so")
for name in ("aws-nitro-root.der", "aws-nitro-root.sha256", "build-inputs.json"):
    copy("/build/" + name, "/app/" + name)
copy("/etc/ssl/certs/ca-certificates.crt")
copy("/etc/os-release")
copy("/etc/debian_version")

# Copy only ELF dependencies; no shell, package manager, compiler or order SDK.
dependencies = set()
for binary in [Path("/usr/local/bin/python3.13"), Path("/build/libnsm.so"),
               *Path("/usr/local/lib/python3.13/lib-dynload").glob("*.so"),
               *Path("/build/packages").rglob("*.so")]:
    if binary.name.startswith(UNUSED_EXTENSIONS):
        continue
    result = subprocess.run(["ldd", str(binary)], check=False, capture_output=True, text=True)
    if result.returncode and "not a dynamic executable" not in result.stderr + result.stdout:
        raise RuntimeError("ELF dependency inspection failed")
    for line in result.stdout.splitlines():
        parts = line.strip().split()
        if "not found" in line:
            raise RuntimeError(f"missing ELF dependency for {binary}: {line.strip()}")
        candidate = parts[2] if len(parts) > 2 and parts[1] == "=>" else parts[0] if parts else ""
        if candidate.startswith("/"):
            dependencies.add(candidate)
for dependency in sorted(dependencies):
    copy(dependency)

# Retain metadata only for Debian packages whose files are actually shipped,
# so scanners can identify the OS libraries in this scratch-based runtime.
owners = {"ca-certificates", "base-files"}
for dependency in dependencies:
    for name in (dependency, str(Path(dependency).resolve())):
        found = subprocess.run(["dpkg-query", "-S", name], capture_output=True, text=True)
        if found.returncode == 0:
            owners.update(line.split(": ", 1)[0] for line in found.stdout.splitlines())
status = []
for owner in sorted(owners):
    found = subprocess.run(["dpkg-query", "-s", owner], capture_output=True, text=True, check=True)
    status.append(found.stdout.strip())
metadata = root / "var/lib/dpkg/status"
metadata.parent.mkdir(parents=True)
metadata.write_text("\n\n".join(status) + "\n")
(root / "tmp").mkdir(mode=0o1777)
epoch = int(os.environ["SOURCE_DATE_EPOCH"])
for path in sorted(root.rglob("*"), key=lambda value: len(value.parts), reverse=True):
    os.utime(path, (epoch, epoch), follow_symlinks=False)
os.utime(root, (epoch, epoch))
