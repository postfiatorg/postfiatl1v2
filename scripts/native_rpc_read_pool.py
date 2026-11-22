#!/usr/bin/env python3
"""Loopback-only read multiplexer. No cache, retry, key, or write dispatch.

Short-lived local observers share bounded persistent read connections per
validator. Expensive diagnostics, history, and owned-object scans use separate connections so they cannot
hold short state reads behind their response time. No responses are cached.
"""
import argparse
import json
import signal
import socket
import socketserver
import threading
from native_rpc_reader import ReadOnlyRpcReader, READ_METHODS


class ReadPoolServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True
    request_queue_size = 32

    def __init__(self, listen_port, upstream_port):
        if (type(listen_port) is not int or type(upstream_port) is not int or
                not 1024 <= listen_port <= 65535 or not 1024 <= upstream_port <= 65535 or
                listen_port == upstream_port):
            raise ValueError('Distinct unprivileged loopback ports required')
        self.reader = ReadOnlyRpcReader(upstream_port, 'shared-read-pool')
        self.diagnostics = ReadOnlyRpcReader(upstream_port, 'diagnostic-read-pool')
        self.history = ReadOnlyRpcReader(upstream_port, 'history-read-pool')
        self.owned = ReadOnlyRpcReader(upstream_port, 'owned-read-pool')
        self.slots = threading.BoundedSemaphore(32)
        super().__init__(('127.0.0.1', listen_port), Handler)

    def process_request(self, request, address):
        if not self.slots.acquire(blocking=False):
            request.close()
            return
        try:
            super().process_request(request, address)
        except BaseException:
            self.slots.release()
            raise

    def process_request_thread(self, request, address):
        try:
            super().process_request_thread(request, address)
        finally:
            self.slots.release()

    def server_close(self):
        super().server_close()
        self.reader.close()
        self.diagnostics.close()
        self.history.close()
        self.owned.close()


class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        self.connection.settimeout(25)
        try:
            for _ in range(63):
                raw = self.rfile.readline(1024 * 1024 + 1)
                if not raw:
                    return
                if len(raw) > 1024 * 1024 or not raw.endswith(b'\n'):
                    return
                request = json.loads(raw)
                if (request.get('version') != 'postfiat-local-rpc-v1' or
                        not isinstance(request.get('id'), str) or
                        not isinstance(request.get('params', {}), dict)):
                    return
                if request.get('method') not in READ_METHODS:
                    response = {'version': 'postfiat-local-rpc-v1', 'ok': False,
                                'error': {'code': 'read_only', 'message': 'Read-only endpoint'}}
                else:
                    reader = (self.server.diagnostics if request['method'] in ('server_info', 'metrics')
                              else self.server.history if request['method'] == 'account_tx'
                              else self.server.owned if request['method'] in ('owned_objects', 'owned_objects_query')
                              else self.server.reader)
                    response = reader.request(request['method'], request.get('params', {}), 20)
                response['id'] = request['id']
                self.wfile.write((json.dumps(response) + '\n').encode())
                self.wfile.flush()
        except (OSError, ValueError, KeyError, TypeError):
            # Closing preserves transport-failure semantics; the caller alone
            # decides whether a pure read can fail over. Never replay here.
            return


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mapping', action='append', required=True, help='listen_port:upstream_port')
    args = parser.parse_args()
    pairs = [tuple(map(int, row.split(':'))) for row in args.mapping]
    if len(pairs) > 6 or any(len(row) != 2 for row in pairs) or len({row[0] for row in pairs}) != len(pairs):
        raise ValueError('At most six distinct mappings required')
    servers = []
    stop = threading.Event()
    signal.signal(signal.SIGTERM, lambda *_: stop.set())
    signal.signal(signal.SIGINT, lambda *_: stop.set())
    try:
        for listen, upstream in pairs:
            server = ReadPoolServer(listen, upstream)
            servers.append(server)
            threading.Thread(target=server.serve_forever, daemon=True).start()
        stop.wait()
    finally:
        for server in servers:
            server.shutdown()
            server.server_close()


if __name__ == '__main__':
    main()
