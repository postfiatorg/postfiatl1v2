"""Bounded, serialized keep-alive sessions for loopback native read RPCs.

No automatic replay: callers retain their existing retry and readiness policy.
Connections rotate before the server's 64-frame cap and before its idle timeout.
"""
import itertools
import json
import socket
import threading
import time

READ_METHODS = frozenset({
    'status', 'metrics', 'server_info', 'account', 'account_assets', 'account_tx',
    'account_lines', 'account_offers', 'asset_info', 'asset_list', 'assets',
    'issuer_assets', 'offer_info', 'book_offers', 'owned_objects', 'owned_account',
    'owned_objects_query', 'owned_recovery_capabilities', 'owned_validators',
    'owned_rules', 'owned_capabilities', 'fee', 'asset_fee_quote',
    'transfer_fee_quote', 'atomic_swap_fee_quote', 'tx', 'blocks', 'receipts',
    'validators', 'navcoin_bridge_routes', 'navcoin_bridge_supply_status',
    'navcoin_bridge_export_status', 'navcoin_bridge_return_status',
    'navcoin_bridge_export_records', 'navcoin_bridge_return_records',
    'navcoin_bridge_packet', 'navcoin_bridge_claims', 'navcoin_bridge_packet_preflight',
    'navcoin_bridge_route_status', 'vault_bridge_status', 'vault_bridge_route',
    'vault_bridge_profiles', 'vault_bridge_profile', 'vault_bridge_profile_v2',
    'nav_reserve_proof_status', 'navcoin_markets', 'navcoin_market_registry',
    'nav_asset_status', 'nav_epoch', 'nav_profile', 'nav_proof_profile',
    'market_ops_status', 'bridge_route_info', 'pftl_uniswap_route_status',
    'pftl_uniswap_route',
})


class ReadOnlyRpcReader:
    def __init__(self, port, source, *, max_frames=63, idle_seconds=20):
        self.port = port
        self.source = source
        self.max_frames = max_frames
        self.idle_seconds = idle_seconds
        self.lock = threading.Lock()
        self.socket = None
        self.buffer = b''
        self.frames = 0
        self.last_used = 0
        self.ids = itertools.count(1)

    def _close(self):
        if self.socket is not None:
            self.socket.close()
        self.socket = None
        self.buffer = b''
        self.frames = 0

    def close(self):
        with self.lock:
            self._close()

    def request(self, method, params=None, timeout=10):
        if method not in READ_METHODS:
            raise ValueError('Only explicit read-only RPC methods are permitted')
        if timeout <= 0:
            raise TimeoutError('Read deadline expired')
        deadline = time.monotonic() + timeout
        if not self.lock.acquire(timeout=timeout):
            raise TimeoutError('Read queue deadline expired')
        try:
            def remaining():
                value = deadline - time.monotonic()
                if value <= 0:
                    raise TimeoutError('Read deadline expired')
                return value
            if (self.socket is not None and
                    (self.frames >= self.max_frames or
                     time.monotonic() - self.last_used >= self.idle_seconds)):
                self._close()
            if self.socket is None:
                self.socket = socket.create_connection(('127.0.0.1', self.port), remaining())
            request_id = self.source + '-' + str(next(self.ids))
            payload = {'version': 'postfiat-local-rpc-v1', 'id': request_id,
                       'method': method, 'params': params or {}}
            self.socket.settimeout(remaining())
            self.socket.sendall((json.dumps(payload) + '\n').encode())
            while b'\n' not in self.buffer:
                self.socket.settimeout(remaining())
                chunk = self.socket.recv(65536)
                if not chunk:
                    raise ConnectionError('Read connection closed before response')
                self.buffer += chunk
                if len(self.buffer) > 8 * 1024 * 1024:
                    raise ValueError('Read response exceeds frame bound')
            line, self.buffer = self.buffer.split(b'\n', 1)
            response = json.loads(line)
            if response.get('id') != request_id or self.buffer:
                raise ValueError('Read response identity or framing differs')
            self.frames += 1
            self.last_used = time.monotonic()
            if not isinstance(response.get('ok'), bool):
                raise ValueError('Native response lacks a boolean result status')
            return response
        except BaseException:
            self._close()
            raise
        finally:
            self.lock.release()

    def read(self, method, params=None, timeout=10):
        response = self.request(method, params, timeout)
        if response['ok'] is not True:
            code = response.get('error', {}).get('code', 'unknown')
            if code in ('rpc_server_error', 'rpc_child_timeout'):
                raise ConnectionError('Native read temporarily unavailable: ' + code)
            raise ValueError('Native read refused: ' + str(code))
        return response['result']


class ReadOnlyRpcPool:
    def __init__(self, source):
        self.source = source
        self.lock = threading.Lock()
        self.readers = {}

    def read(self, port, method, params=None, timeout=10):
        with self.lock:
            reader = self.readers.setdefault(port, ReadOnlyRpcReader(port, self.source))
        return reader.read(method, params, timeout)

    def close(self):
        with self.lock:
            for reader in self.readers.values():
                reader.close()
            self.readers.clear()
