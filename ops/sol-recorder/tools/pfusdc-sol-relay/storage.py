"""Crash-safe files for a single POSIX volume shared by CLI and HTTP workers."""

import contextlib
import fcntl
import hashlib
import json
import os
import tempfile
from pathlib import Path

ROUTE = "solana-mainnet-usdc-v1"
DEPOSIT = "sp1-solana-alpenglow-finality-v1"


def encode(obj):
    return json.dumps(obj, separators=(",", ":"), ensure_ascii=True).encode()


def load(path):
    return json.loads(path.read_bytes())


def digest(data):
    return hashlib.sha256(data).hexdigest()


def sync_dir(directory):
    fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def mkdir(path):
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    sync_dir(path.parent)


def save(path, data):
    if not isinstance(data, bytes):
        data = encode(data)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as file:
        try:
            file.write(data)
            file.flush()
            os.fsync(file.fileno())
            os.replace(file.name, path)
            sync_dir(path.parent)
        finally:
            Path(file.name).unlink(missing_ok=True)


@contextlib.contextmanager
def locked(path, *, shared=False):
    with path.open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_SH if shared else fcntl.LOCK_EX)
        yield


class Invalid(ValueError):
    """A binding, signature or terminal receipt failed validation."""


class AwaitingSource(Exception):
    """Finality or the rooted archive has not arrived yet."""
