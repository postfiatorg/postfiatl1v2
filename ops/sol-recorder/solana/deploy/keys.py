"""Task K signing keys. Only public addresses and seal paths go to stdout."""
import fcntl
import json
import os
from pathlib import Path
import subprocess

from solders.keypair import Keypair

DIRECTORY = Path.home() / ".config/postfiat/sol-bridge"


def load(path):
    return Keypair.from_json(Path(path).read_text())


def persistent(name, seal=False, *, directory=DIRECTORY):
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    path = directory / (name + ".json")
    sealed = path.with_suffix(".json.cold.gpg")
    with (directory / ".keys.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if not path.exists():
            if sealed.exists():
                raise RuntimeError(f"restore existing {sealed} before continuing")
            tmp = path.with_suffix(".new")
            with os.fdopen(os.open(tmp, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600), "w") as out:
                out.write(Keypair().to_json())
                out.flush()
                os.fsync(out.fileno())
            tmp.rename(path)
        if path.is_symlink() or path.stat().st_mode & 0o077:
            raise RuntimeError(f"expected owner-only regular key file at {path}")
        key = load(path)
        if seal:
            if not sealed.exists():
                subprocess.run([Path.home()/".local/bin/pf-seal", "cold", path], check=True)
            restored = subprocess.check_output([Path.home()/".local/bin/pf-unseal", sealed])
            if Keypair.from_bytes(bytes(json.loads(restored))).pubkey() != key.pubkey():
                raise RuntimeError("sealed key mismatch")
            print(f"{name}={key.pubkey()} mode={path.stat().st_mode & 0o777:o} cold_restore=verified")
        return key


if __name__ == "__main__":
    for program in ("vault", "recorder"):
        persistent(program, seal=True)
