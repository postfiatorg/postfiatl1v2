"""OS-enforced singleton for the value-moving coordinator worker."""

from __future__ import annotations

import fcntl
import os
from pathlib import Path
from typing import IO

from .policy import RealValuePolicyError


class CoordinatorAlreadyRunning(RealValuePolicyError):
    """Another process owns the value-worker lease."""


class CoordinatorProcessLock:
    """A crash releases this kernel lock; no stale time-based lease is trusted."""

    def __init__(self, path: str | Path) -> None:
        self.path = Path(path)
        self._handle: IO[str] | None = None

    def acquire(self) -> None:
        if self._handle is not None:
            return
        self.path.parent.mkdir(parents=True, exist_ok=True)
        handle = self.path.open("a+", encoding="ascii")
        os.chmod(self.path, 0o600)
        try:
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            handle.close()
            raise CoordinatorAlreadyRunning(
                "another coordinator process holds the value-worker lock"
            ) from error
        handle.seek(0)
        handle.truncate()
        handle.write(f"{os.getpid()}\n")
        handle.flush()
        os.fsync(handle.fileno())
        self._handle = handle

    def close(self) -> None:
        handle = self._handle
        if handle is None:
            return
        try:
            fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
        finally:
            handle.close()
            self._handle = None

    def __enter__(self) -> "CoordinatorProcessLock":
        self.acquire()
        return self

    def __exit__(self, *_exc: object) -> None:
        self.close()
