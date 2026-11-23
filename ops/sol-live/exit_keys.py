"""Open an exit signer only after checking the opened descriptor."""

import os
import stat
from pathlib import Path

from solders.keypair import Keypair


def load(path):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        info = os.fstat(fd)
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_uid != os.geteuid()
        ):
            raise ValueError("Signer must be an owned regular file with mode 0600.")
        with os.fdopen(fd, "r", closefd=False) as stream:
            return Keypair.from_json(stream.read())
    finally:
        os.close(fd)


def create_key(path):
    # Publish a complete, durable key without overwriting any existing signer.
    import tempfile

    path = Path(path)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False, mode="w") as tmp:
        try:
            tmp.write(Keypair().to_json())
            tmp.flush()
            os.fsync(tmp.fileno())
            os.link(tmp.name, path)
        finally:
            os.unlink(tmp.name)
    fd = os.open(path.parent, os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)
    return load(path)
