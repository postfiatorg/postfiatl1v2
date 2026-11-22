#!/usr/bin/env python3
"""Submit one witness and save the verified proof and its admission public values."""
import argparse
import json
import os
from pathlib import Path
import socket
import tempfile
import time


def prove(address, kind, mode, witness, output):
    start = time.monotonic()
    data = witness.read_bytes()
    with socket.socket(socket.AF_UNIX) as sock:
        # A stalled tunnel or server must fail the request so the relay retries; without a
        # limit the deposit waits until someone restarts the relay. A proof takes about a minute.
        sock.settimeout(300)
        sock.connect(str(address))
        sock.sendall(f"{kind} {mode} {len(data)}\n".encode())
        sock.sendall(data)
        sock.shutdown(socket.SHUT_WR)
        with sock.makefile("rb") as reader:
            header = json.loads(reader.readline(1 << 20))
            if "error" in header:
                raise RuntimeError(header["error"])
            size = header["proof_bytes"]
            raw = reader.read(size)
            if len(raw) != size or reader.read(1):
                raise ValueError("incomplete or oversized proof reply")
    header["seconds"]["client_round_trip"] = time.monotonic() - start
    # Publish proof and journal together after receiving the entire response.
    with tempfile.NamedTemporaryFile("w", dir=output.parent, delete=False) as tmp:
        json.dump({**header, "proof_hex": raw.hex()}, tmp)
    os.replace(tmp.name, output)
    print(json.dumps({k: v for k, v in header.items() if k != "public_values"}), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("socket", type=Path)
    parser.add_argument("guest", choices=["deposit", "parallel", "key_table", "handoff"])
    parser.add_argument("mode", choices=["groth16", "compressed"])
    parser.add_argument("witness", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    prove(args.socket, args.guest, args.mode, args.witness, args.output)
