#!/usr/bin/env python3
"""Replace a stale snapshot pair in the observer's ledger before the node starts.

The observer writes no snapshots of its own, so after a stop it would restart from the pair it
was installed with and never catch up. This fetches a current full and incremental snapshot
from one cluster node when the local pair is too far behind. It does nothing when the pair is
recent. The validator checks the snapshot's hash when it loads it.

usage: fresh-snapshot.py LEDGER [--max-behind SLOTS]
"""
import argparse
import concurrent.futures
import json
import os
import pathlib
import re
import sys
import time
import urllib.error
import urllib.request

API = "https://api.mainnet-beta.solana.com"
FULL = re.compile(r"^snapshot-(\d+)-[1-9A-HJ-NP-Za-km-z]+\.tar\.zst$")
INCREMENTAL = re.compile(r"^incremental-snapshot-(\d+)-(\d+)-[1-9A-HJ-NP-Za-km-z]+\.tar\.zst$")
CHUNK = 256 * 1024 * 1024
WORKERS = 48


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


def rpc(method):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method}).encode()
    request = urllib.request.Request(API, data=body, headers={"content-type": "application/json"})
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)["result"]


def current_name(base, kind, pattern, timeout):
    """The file this node redirects to for its newest snapshot of this kind, or None."""
    opener = urllib.request.build_opener(NoRedirect)
    try:
        opener.open(urllib.request.Request(f"{base}/{kind}.tar.bz2", method="HEAD"), timeout=timeout)
    except urllib.error.HTTPError as error:
        name = (error.headers.get("location") or "").rsplit("/", 1)[-1]
        if pattern.match(name):
            return name
    except Exception:
        pass
    return None


def ranged_size(base, name, timeout=10):
    """The file's size on this node, or None when it does not serve byte ranges."""
    try:
        request = urllib.request.Request(f"{base}/{name}", headers={"Range": "bytes=0-0"})
        with urllib.request.urlopen(request, timeout=timeout) as response:
            if response.status == 206:
                return int(response.headers["Content-Range"].split("/")[1])
    except Exception:
        pass
    return None


def pair(base, timeout=4):
    """A node's newest matching (full, incremental) names, or None."""
    full = current_name(base, "snapshot", FULL, timeout)
    incremental = current_name(base, "incremental-snapshot", INCREMENTAL, timeout)
    if full and incremental and INCREMENTAL.match(incremental).group(1) == FULL.match(full).group(1):
        return full, incremental
    return None


def rate(base, name):
    """MB/s over a short ranged read of this file."""
    try:
        request = urllib.request.Request(f"{base}/{name}", headers={"Range": "bytes=0-67108863"})
        started, got = time.time(), 0
        with urllib.request.urlopen(request, timeout=4) as response:
            if response.status != 206:
                return 0
            while time.time() - started < 5:
                block = response.read(1 << 20)
                if not block:
                    break
                got += len(block)
        return got / 1e6 / (time.time() - started)
    except Exception:
        return 0


def candidates(tip, max_behind):
    """Cluster nodes whose newest pair is recent, fastest first: (base, full, incremental)."""
    peers = [f"http://{node['rpc']}" for node in rpc("getClusterNodes") if node.get("rpc")]

    def look(base):
        names = pair(base)
        if not names or tip - int(INCREMENTAL.match(names[1]).group(2)) > max_behind:
            return None
        speed = rate(base, names[0])
        return (speed, base, *names) if speed >= 5 else None

    with concurrent.futures.ThreadPoolExecutor(64) as pool:
        found = sorted(filter(None, pool.map(look, peers)), reverse=True)
    for speed, base, full, incremental in found[:5]:
        print(f"source {base}: {speed:.0f} MB/s, {incremental}", flush=True)
    return [row[1:] for row in found[:5]]


def ranged(base, name, target):
    size = ranged_size(base, name)
    if size is None:
        raise OSError("no byte ranges")
    part = target.with_name(target.name + ".part")
    descriptor = os.open(part, os.O_CREAT | os.O_RDWR | os.O_TRUNC, 0o644)
    os.ftruncate(descriptor, size)

    def piece(index):
        low, high = index * CHUNK, min(size, (index + 1) * CHUNK) - 1
        for attempt in range(8):
            try:
                request = urllib.request.Request(f"{base}/{name}", headers={"Range": f"bytes={low}-{high}"})
                with urllib.request.urlopen(request, timeout=120) as response:
                    if response.status != 206:
                        raise OSError("range refused")
                    offset = low
                    while block := response.read(1 << 22):
                        os.pwrite(descriptor, block, offset)
                        offset += len(block)
                if offset != high + 1:
                    raise OSError("short read")
                return
            except Exception:
                if attempt == 7:
                    raise
                time.sleep(2)

    try:
        with concurrent.futures.ThreadPoolExecutor(WORKERS) as pool:
            list(pool.map(piece, range((size + CHUNK - 1) // CHUNK)))
    finally:
        os.close(descriptor)
    os.rename(part, target)
    return size


def whole(base, name, target):
    """One connection, for the public endpoint, which serves no byte ranges."""
    part = target.with_name(target.name + ".part")
    with urllib.request.urlopen(f"{base}/{name}", timeout=120) as response, open(part, "wb") as out:
        size = int(response.headers["Content-Length"])
        while block := response.read(1 << 22):
            out.write(block)
    if part.stat().st_size != size:
        raise OSError("short read")
    os.rename(part, target)
    return size


def fetch(base, name, ledger):
    if (ledger / name).exists():
        print(f"already have {name}", flush=True)
        return
    started = time.time()
    size = (whole if base == API else ranged)(base, name, ledger / name)
    print(f"fetched {name} ({size / 1e9:.1f} GB) from {base} in {time.time() - started:.0f} s", flush=True)


def local_slot(ledger):
    """Newest slot the local pair can start from."""
    fulls = sorted(int(m.group(1)) for m in map(FULL.match, os.listdir(ledger)) if m)
    if not fulls:
        return None
    tips = [int(m.group(2)) for m in map(INCREMENTAL.match, os.listdir(ledger)) if m and int(m.group(1)) == fulls[-1]]
    return max(tips, default=fulls[-1])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("ledger", type=pathlib.Path)
    parser.add_argument("--max-behind", type=int, default=5000, help="slots the local pair may trail the cluster")
    args = parser.parse_args()
    ledger = args.ledger.resolve()

    tip = rpc("getSlot")
    slot = local_slot(ledger)
    if slot is not None and tip - slot <= args.max_behind:
        print(f"local snapshot at {slot} is {tip - slot} slots behind; keeping it", flush=True)
        return 0
    print("no local snapshot; fetching" if slot is None else f"local snapshot at {slot} is {tip - slot} slots behind; fetching", flush=True)

    # The public endpoint is the last resort: it is rate-limited and takes one connection.
    sources = candidates(tip, args.max_behind)
    public = pair(API, timeout=30)
    if public:
        sources.append((API, *public))
    for base, full, incremental in sources:
        try:
            fetch(base, full, ledger)
            fetch(base, incremental, ledger)
        except Exception as error:
            print(f"{base} failed: {error}", flush=True)
            continue
        for name in os.listdir(ledger):
            if (FULL.match(name) or INCREMENTAL.match(name) or name.endswith(".tar.zst.part")) and name not in (full, incremental):
                os.unlink(ledger / name)
                print(f"removed {name}", flush=True)
        return 0
    print("no source delivered a snapshot pair", flush=True)
    return 1


if __name__ == "__main__":
    sys.exit(main())
