"""The settlement loop's only way into this host: the forced command of its SSH key line in the
reviewer user's authorized_keys (install-exit-review.sh --settler-key).

It reads one JSON request on standard input and accepts exactly two:
  {"request": "review", "amendment": {...}, "proposal_slot": N, "expires_at_height": N}
      hands in a drafted paid or refund decision for this host's own review;
  {"request": "result", "decision_sha256": "<64 hex>"}
      returns this host's result for it: its signature, its refusal with the reason, or pending.
It runs no command, names no file and moves no file: the decision's hash names the inbox entry.
The node user's signing step (exit_review.py sign-door) signs a handed-in decision only from this
host's own fresh review, exactly as `exit_review.py sign` does.
"""

import hashlib
import io
import json
import os
import re
import select
import sys
import time
from pathlib import Path

INBOX = Path("/var/lib/postfiat-exit-review/inbox")
OUTBOX = Path("/var/lib/postfiat-exit-review/outbox")
LIMIT = 65536
# A request must arrive whole within this many seconds; an unfinished one is refused.
READ_SECONDS = 30
# The inbox is bounded: at most this many decisions wait at once, and an entry is removed after
# RETAIN_SECONDS. A settlement's signatures expire after 30 blocks, far inside that.
MAX_ENTRIES = 64
RETAIN_SECONDS = 24 * 3600
REFUSED = 2
KIND = re.compile(r"solana_exit_(r8_paid|refund):[0-9a-f:]+")
SHA = re.compile(r"[0-9a-f]{64}")


class Refused(ValueError):
    pass


def require(ok, message):
    if not ok:
        raise Refused(message)


def write(path, body):
    # A name of its own per request: two requests for one decision never share a half-written file.
    tmp = path.with_name(f".{path.stem}.{os.getpid()}.{time.monotonic_ns()}.tmp")
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o640)
    try:
        with os.fdopen(fd, "w") as stream:
            json.dump(body, stream, sort_keys=True)
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except FileNotFoundError:
            pass
        raise


def make_room(inbox, name, now=None):
    """Remove entries and leftovers older than RETAIN_SECONDS, then refuse a new decision when
    MAX_ENTRIES already wait. Handing in a waiting decision again replaces it and is always allowed."""
    now = time.time() if now is None else now
    waiting = 0
    for entry in os.scandir(inbox):
        try:
            if not entry.is_file(follow_symlinks=False):
                continue
            if now - entry.stat(follow_symlinks=False).st_mtime > RETAIN_SECONDS:
                os.unlink(entry.path)
            elif entry.name.endswith(".json") and entry.name != name:
                waiting += 1
        except FileNotFoundError:
            continue
    require(waiting < MAX_ENTRIES, f"{MAX_ENTRIES} decisions already wait on this host; try again later")


def read(path):
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    except FileNotFoundError:
        return None
    with os.fdopen(fd, "rb") as stream:
        return json.loads(stream.read(LIMIT))


def read_request(stdin):
    """At most LIMIT + 1 bytes of the request, within READ_SECONDS in total."""
    try:
        fd = stdin.fileno()
    except (AttributeError, OSError, io.UnsupportedOperation):
        return stdin.read(LIMIT + 1)
    deadline, chunks, size = time.monotonic() + READ_SECONDS, [], 0
    while size <= LIMIT:
        remaining = deadline - time.monotonic()
        require(remaining > 0 and select.select([fd], [], [], remaining)[0], "the request did not arrive in time")
        chunk = os.read(fd, LIMIT + 1 - size)
        if not chunk:
            break
        chunks.append(chunk)
        size += len(chunk)
    return b"".join(chunks)


def handle(request, inbox, outbox):
    require(isinstance(request, dict), "not a request")
    if request.get("request") == "review":
        require(set(request) == {"request", "amendment", "proposal_slot", "expires_at_height"},
                "a review request has exactly an amendment, a proposal slot and an expiry height")
        amendment = request["amendment"]
        require(isinstance(amendment, dict) and isinstance(amendment.get("kind"), str)
                and KIND.fullmatch(amendment["kind"]) and type(amendment.get("value")) is int
                and amendment["value"] == 1 and amendment.get("paused", False) is False,
                "not a plain Solana exit paid or refund decision")
        slot, expires = request["proposal_slot"], request["expires_at_height"]
        require(type(slot) is int and type(expires) is int and 0 < slot < expires,
                "proposal slot and expiry height must be integers, the expiry after the slot")
        sha = hashlib.sha256(amendment["kind"].encode()).hexdigest()
        make_room(inbox, f"{sha}.json")
        write(inbox / f"{sha}.json", dict(amendment=amendment, proposal_slot=slot, expires_at_height=expires))
        return dict(decision_sha256=sha, state="handed in")
    if request.get("request") == "result":
        require(set(request) == {"request", "decision_sha256"}, "a result request names only the decision's hash")
        sha = request["decision_sha256"]
        require(isinstance(sha, str) and SHA.fullmatch(sha), "the decision's hash is 64 lowercase hex digits")
        result = read(outbox / f"{sha}.json")
        if result is not None:
            return result
        return dict(decision_sha256=sha, state="pending" if (inbox / f"{sha}.json").exists() else "unknown")
    raise Refused("only the review and result requests are accepted")


def main(stdin=None, environ=None, inbox=INBOX, outbox=OUTBOX):
    stdin = stdin or sys.stdin.buffer
    environ = os.environ if environ is None else environ
    try:
        require(not environ.get("SSH_ORIGINAL_COMMAND"), "this key runs no command; send a request on stdin")
        raw = read_request(stdin)
        require(len(raw) <= LIMIT, "request too large")
        try:
            request = json.loads(raw)
        except ValueError:
            raise Refused("the request is not JSON") from None
        print(json.dumps(handle(request, Path(inbox), Path(outbox)), sort_keys=True))
        return 0
    except Refused as error:
        print(f"refused: {error}", file=sys.stderr)
        return REFUSED


if __name__ == "__main__":
    sys.exit(main())
