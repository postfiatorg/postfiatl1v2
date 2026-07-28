#!/usr/bin/env python3
"""Expose one HTTPS JSON-RPC endpoint as a loopback-only HTTP endpoint."""

from __future__ import annotations

import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import requests


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--listen-port", type=int, required=True)
    parser.add_argument("--upstream", required=True)
    args = parser.parse_args()

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self) -> None:
            length = int(self.headers.get("Content-Length", "0"))
            body = self.rfile.read(length)
            try:
                response = requests.post(
                    args.upstream,
                    data=body,
                    headers={
                        "Content-Type": "application/json",
                        "User-Agent": "postfiat-loopback-jsonrpc-proxy/1",
                    },
                    timeout=120,
                )
                payload = response.content
                self.send_response(response.status_code)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)
            except Exception as error:
                payload = str(error).encode()
                self.send_response(502)
                self.send_header("Content-Type", "text/plain")
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

        def log_message(self, *_args: object) -> None:
            return

    server = ThreadingHTTPServer(("127.0.0.1", args.listen_port), Handler)
    server.serve_forever()


if __name__ == "__main__":
    main()
