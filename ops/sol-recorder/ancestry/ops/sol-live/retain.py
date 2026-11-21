"""Retain certified source slots with the source ledgers and restore the relay archive."""

import os
from pathlib import Path
import shutil
import tempfile
import threading
import time

FILES = ("validators", "certificate", "shreds", "state", "finalized-id", "events.json")


def mirror(source, target):
    target.mkdir(parents=True, exist_ok=True)
    for marker in source.glob("*/events.json"):
        slot = marker.parent
        if not slot.name.isdecimal():
            continue
        dst = target / slot.name
        if all((dst / name).is_file() for name in FILES):
            continue
        if not all((slot / name).is_file() for name in FILES):
            continue
        # The marker is last, so a reader can never consume a partial slot.
        dst.mkdir(exist_ok=True)
        for name in FILES:
            if (dst / name).is_file() and name != marker.name:
                continue
            with tempfile.NamedTemporaryFile(dir=dst, delete=False) as stage:
                with (slot / name).open("rb") as inp:
                    shutil.copyfileobj(inp, stage)
                stage.flush()
                os.fsync(stage.fileno())
            os.replace(stage.name, dst / name)
        with_dir = os.open(dst, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(with_dir)
        finally:
            os.close(with_dir)


def retain(state):
    archive = state / "archive"
    retained = state / "ledgers" / "certified-events"
    retained.mkdir(parents=True, exist_ok=True)
    while True:
        try:
            # Import slots written by earlier binaries and repair gaps in either copy.
            mirror(archive, retained)
            mirror(retained, archive)
            descendants = state / "ledgers" / "descendant-events"
            recovered = state / "archive-ancestry"
            mirror(descendants, recovered)
            mirror(recovered, descendants)
        except OSError as error:
            print(f"certified archive mirror: {error}", flush=True)
        time.sleep(0.2)


def start(state):
    threading.Thread(target=retain, args=(Path(state),), daemon=True).start()
