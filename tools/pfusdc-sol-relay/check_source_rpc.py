#!/usr/bin/env python3
"""Exercise finalized getTransaction using the actual signed message from the retained block."""

import argparse
import json
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

from jobs import Relay
from source import ALPHABET, authenticated_transaction, bind, journal, locate
from storage import encode, load


def b58(raw):
    value = int.from_bytes(raw, "big")
    digits = ""
    while value:
        value, index = divmod(value, 58)
        digits = ALPHABET[index] + digits
    return "1" * (len(raw) - len(raw.lstrip(b"\0"))) + digits


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", type=Path)
    cfg = load(parser.parse_args().config)
    root = Path(cfg["capture_root"])
    signature = (root / "signature").read_text().strip()
    slot = load(root / f"deposit-{signature}/source.json")["slot"]
    raw = json.loads(
        subprocess.check_output(
            [cfg["capture_binary"], "transaction", cfg["archive"], str(slot), signature]
        )
    )
    msg = {
        "accountKeys": raw["keys"],
        "recentBlockhash": raw["recentBlockhash"],
        "addressTableLookups": raw["lookups"],
        "instructions": [
            {
                "programIdIndex": ix["programIdIndex"],
                "accounts": ix["accounts"],
                "data": b58(bytes.fromhex(ix["data_hex"])),
            }
            for ix in raw["instructions"]
        ],
    }
    result = {
        "slot": slot,
        "transaction": {"message": msg, "signatures": [signature]},
        "meta": {"err": None},
    }
    requests = []

    class RPC(BaseHTTPRequestHandler):
        def do_POST(self):
            req = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            if req["method"] == "getGenesisHash":
                assert req["params"] == []
                value = b58(bytes.fromhex(cfg["solana_genesis_hash"]))
            else:
                assert req["method"] == "getTransaction"
                requests.append(req["params"])
                value = None if len(requests) == 1 else result
            self.send_response(200)
            self.end_headers()
            self.wfile.write(
                encode(
                    {
                        "id": 1,
                        "jsonrpc": "2.0",
                        "result": value,
                    }
                )
            )

        def log_message(self, *args):
            pass

    with HTTPServer(("127.0.0.1", 0), RPC) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            location = locate(
                dict(cfg, solana_rpc=f"http://127.0.0.1:{server.server_port}"),
                "deposit",
                signature,
            )
        finally:
            server.shutdown()
            thread.join()
    expected = [
        signature,
        {
            "commitment": "finalized",
            "encoding": "json",
            "maxSupportedTransactionVersion": 0,
        },
    ]
    assert requests == [expected, expected]
    authenticated_transaction(cfg, location, Path(cfg["archive"]))
    job, _ = Relay(cfg).create("deposit", signature)
    selected = bind(location, journal((job / "public.bin").read_bytes()))
    assert len(selected) == 1 and selected[0]["amount_atoms"] == "5000000"
    print(
        f"PASS finalized RPC replay; getTransaction calls=2 slot={slot} signed message matched captured block, amount_atoms=5000000"
    )


if __name__ == "__main__":
    main()
