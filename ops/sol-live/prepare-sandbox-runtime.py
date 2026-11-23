"""Record the dynamic libraries needed by this Ubuntu Python/SSH/prover runtime."""

import os
from pathlib import Path
import re
import subprocess
import sys
import sysconfig


def main(app):
    stdlib = Path(sysconfig.get_path("stdlib"))
    python = Path(sys.executable).resolve()
    inputs = [
        python,
        Path("/usr/bin/ssh"),
        app / "proof/bin/proof-host",
        *(stdlib / "lib-dynload").glob("*.so"),
        *(app / "venv").rglob("*.so*"),
    ]
    libraries = set()
    for path in inputs:
        result = subprocess.check_output(["ldd", str(path)], text=True)
        if "not found" in result:
            raise ValueError(f"Missing runtime dependency for {path.name}")
        for lib in re.findall(r"(?:=>\s+|^\s*)(/\S+)\s+\(", result, re.M):
            if not Path(lib).is_file() or not lib.startswith(
                ("/lib/", "/lib64/", "/usr/lib/")
            ):
                raise ValueError(f"Unexpected runtime library path for {path.name}")
            libraries.add(lib)
    (app / "runtime-files").write_text(
        "\n".join(sorted(libraries | {str(python), str(stdlib), "/usr/bin/ssh"})) + "\n"
    )
    (app / "passwd").write_text(
        f"worker:x:{os.getuid()}:{os.getgid()}::/home/worker:/bin/false\n"
    )
    (app / "nsswitch.conf").write_text(
        "passwd: files\ngroup: files\nhosts: files dns\n"
    )


if __name__ == "__main__":
    main(Path(sys.argv[1]).resolve())
