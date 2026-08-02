#!/usr/bin/env python3
"""Expose one HTTPS JSON-RPC endpoint as a loopback-only HTTP endpoint."""

from __future__ import annotations

import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import time

import requests


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--listen-port", type=int, required=True)
    parser.add_argument("--upstream", required=True)
    parser.add_argument(
        "--attempts",
        type=int,
        default=1,
        help="maximum attempts for transient HTTP/RPC transport failures",
    )
    parser.add_argument(
        "--retry-delay-seconds",
        type=float,
        default=0.25,
        help="delay between transient-failure retries",
    )
    parser.add_argument(
        "--upstream-timeout-seconds",
        type=float,
        default=120,
        help="timeout for each individual upstream attempt",
    )
    args = parser.parse_args()
    if not 1 <= args.attempts <= 100:
        parser.error("--attempts must be between 1 and 100")
    if not 0 <= args.retry_delay_seconds <= 30:
        parser.error("--retry-delay-seconds must be between 0 and 30")
    if not 0.1 <= args.upstream_timeout_seconds <= 120:
        parser.error("--upstream-timeout-seconds must be between 0.1 and 120")

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self) -> None:
            length = int(self.headers.get("Content-Length", "0"))
            body = self.rfile.read(length)
            response: requests.Response | None = None
            last_error: Exception | None = None
            for attempt in range(args.attempts):
                try:
                    response = requests.post(
                        args.upstream,
                        data=body,
                        headers={
                            "Content-Type": "application/json",
                            "User-Agent": "postfiat-loopback-jsonrpc-proxy/1",
                        },
                        timeout=args.upstream_timeout_seconds,
                    )
                    if response.status_code != 429 and response.status_code < 500:
                        break
                except requests.RequestException as error:
                    last_error = error
                if attempt + 1 < args.attempts:
                    time.sleep(args.retry_delay_seconds)

            if response is not None:
                payload = response.content
                self.send_response(response.status_code)
                self.send_header("Content-Type", "application/json")
            else:
                payload = str(last_error or "upstream request failed").encode()
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
