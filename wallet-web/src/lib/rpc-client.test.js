import assert from 'node:assert/strict';
import test from 'node:test';

import {
  RpcClient,
  humanRpcErrorMessage,
  parseAccountResult,
  parseOwnedObjectsResult,
  pollOwnedObjectsTotal,
} from './rpc-client.js';
import {
  clearCustodyMaterialRegistry,
  registerCustodyMaterial,
} from './custody-boundary.js';

test('parseAccountResult accepts flat account responses', () => {
  const account = parseAccountResult({
    ok: true,
    result: {
      address: 'pf1234',
      balance: 123000000,
      sequence: 7,
    },
  });

  assert.equal(account.balance, 123000000);
  assert.equal(account.sequence, 7);
});

test('parseAccountResult accepts nested account responses', () => {
  const account = parseAccountResult({
    ok: true,
    result: {
      account: {
        address: 'pf1234',
        balance: 42,
        sequence: 2,
      },
    },
  });

  assert.equal(account.balance, 42);
  assert.equal(account.sequence, 2);
});

test('parseAccountResult preserves genuine zero balances', () => {
  const account = parseAccountResult({
    ok: true,
    result: {
      address: 'pfzero',
      balance: 0,
      sequence: 0,
    },
  });

  assert.equal(account.balance, 0);
  assert.equal(account.sequence, 0);
});

test('parseAccountResult rejects RPC errors instead of returning zero', () => {
  assert.throws(
    () => parseAccountResult({
      ok: false,
      result: null,
      error: { code: 'connection_error', message: 'connection closed' },
    }),
    /connection closed/,
  );
});

test('parseAccountResult rejects missing balance instead of returning zero', () => {
  assert.throws(
    () => parseAccountResult({
      ok: true,
      result: { address: 'pfmissing', sequence: 1 },
    }),
    /missing balance/,
  );
});

test('parseOwnedObjectsResult preserves total_value and objects', () => {
  const snapshot = parseOwnedObjectsResult({
    ok: true,
    result: {
      total_value: 50,
      objects: [{ id: 'a', value: 20 }, { id: 'b', value: 30 }],
    },
  });

  assert.equal(snapshot.totalValue, 50);
  assert.equal(snapshot.objects.length, 2);
});

test('parseOwnedObjectsResult computes total from objects when total_value is absent', () => {
  const snapshot = parseOwnedObjectsResult({
    ok: true,
    result: {
      objects: [{ id: 'a', value: 20 }, { id: 'b', value: 30 }],
    },
  });

  assert.equal(snapshot.totalValue, 50n);
});

test('humanRpcErrorMessage translates owned-object transport failures', () => {
  assert.equal(
    humanRpcErrorMessage(new Error('RPC send failed: owned_objects')),
    'FastPay object lookup is unavailable from this RPC endpoint. Check wallet network status and retry.',
  );
});

test('humanRpcErrorMessage translates finality submit failures', () => {
  assert.equal(
    humanRpcErrorMessage(new Error('wallet RPC connection dropped while sending mempool_submit_signed_payment_v2_finality')),
    'Finality submit is unavailable from this RPC endpoint. Use a finality-enabled wallet endpoint and retry.',
  );
});

test('RpcClient attaches the session-only proxy token to mutation requests', async () => {
  const previousWebSocket = globalThis.WebSocket;
  globalThis.WebSocket = { OPEN: 1, CLOSING: 2, CLOSED: 3 };
  try {
    const rpc = new RpcClient('ws://127.0.0.1:8080/rpc', 'session-token');
    rpc.connect = async () => {};
    rpc.ws = {
      readyState: 1,
      send(raw) {
        const request = JSON.parse(raw);
        assert.equal(request.proxy_auth_token, 'session-token');
        queueMicrotask(() => {
          rpc.pending.get(request.id).resolve({
            version: request.version,
            id: request.id,
            ok: true,
            result: {},
            error: null,
            events: [],
          });
        });
      },
    };
    const response = await rpc.call('owned_apply', { cert_json: '{}' });
    assert.equal(response.ok, true);
  } finally {
    globalThis.WebSocket = previousWebSocket;
  }
});

test('RpcClient rejects registered custody material before opening a socket', async () => {
  const seed = 'a7'.repeat(32);
  registerCustodyMaterial({ seed });
  try {
    const rpc = new RpcClient('ws://127.0.0.1:8080/rpc');
    let connected = false;
    rpc.connect = async () => { connected = true; };
    await assert.rejects(
      () => rpc.call('mempool_submit_signed_transfer', { metadata: seed }),
      /registered-secret-value/,
    );
    assert.equal(connected, false);
  } finally {
    clearCustodyMaterialRegistry();
  }
});

test('RpcClient exposes no unsigned account-to-owned mutation', () => {
  const client = new RpcClient('ws://127.0.0.1:18793');
  assert.equal(client.wrapOwned, undefined);
  assert.equal(client.unwrapOwned, undefined);
});

test('RpcClient binds FastPay v3 mutations and recovery reads to exact RPC methods', async () => {
  const client = new RpcClient('ws://127.0.0.1:18793');
  const calls = [];
  client.call = async (method, params = {}, timeoutMs) => {
    calls.push({ method, params, timeoutMs });
    return { ok: true, result: {} };
  };
  await client.ownedRecoveryCapabilities();
  await client.ownedSignV3('signed-order', 'validator-2');
  await client.ownedApplyV3('certificate');
  await client.ownedUnwrapSignV3('signed-unwrap', 'validator-3');
  await client.ownedUnwrapApplyV3('unwrap-certificate');
  await client.ownedCertificate({ lock_id: 'a'.repeat(96) });
  await client.ownedRecoveryStatus('b'.repeat(96));
  assert.deepEqual(calls.map(call => call.method), [
    'owned_recovery_capabilities',
    'owned_sign_v3',
    'owned_apply_v3',
    'owned_unwrap_sign_v3',
    'owned_unwrap_apply_v3',
    'owned_certificate',
    'owned_recovery_status',
  ]);
  assert.deepEqual(calls[1].params, { order_json: 'signed-order', validator_id: 'validator-2' });
  assert.deepEqual(calls[2].params, { cert_json: 'certificate' });
  assert.deepEqual(calls[5].params, { lock_id: 'a'.repeat(96) });
});

test('serverCapabilities fails closed when the FastPay owned lane is not advertised', async () => {
  const rpc = new RpcClient('ws://127.0.0.1:8080/rpc');
  rpc.call = async method => method === 'server_info'
    ? { ok: true, result: { rpc: { read_only: false } } }
    : { ok: true, result: { block_height: 1 } };
  const disabled = await rpc.serverCapabilities();
  assert.equal(disabled.owned_lane_enabled, false);

  rpc.call = async method => method === 'server_info'
    ? {
        ok: true,
        result: {
          rpc: {
            read_only: false,
            owned_lane_enabled: true,
            owned_certificate_domain: {
              schema: 'postfiat-owned-certificate-domain-v2',
              chain_id: 'postfiat-test',
              genesis_hash: 'a'.repeat(96),
              protocol_version: 1,
              registry_id: 'b'.repeat(96),
            },
          },
        },
      }
    : { ok: true, result: { block_height: 1 } };
  const enabled = await rpc.serverCapabilities();
  assert.equal(enabled.owned_lane_enabled, true);
  assert.equal(enabled.owned_certificate_domain.chain_id, 'postfiat-test');
});

test('submitSignedAssetTransaction uses canonical RPC parameter name', async () => {
  const calls = [];
  const rpc = new RpcClient('ws://127.0.0.1:8080/rpc');
  rpc.call = async (method, params, timeoutMs) => {
    calls.push([method, params, timeoutMs]);
    return { ok: true, result: { tx_id: 'asset-tx' } };
  };

  const result = await rpc.submitSignedAssetTransaction('signed-asset-json');

  assert.equal(result.result.tx_id, 'asset-tx');
  assert.deepEqual(calls, [[
    'mempool_submit_signed_asset_transaction',
    { signed_asset_transaction_json: 'signed-asset-json' },
    30000,
  ]]);
});

test('submitSignedAssetTransactionFinality uses certified asset finality RPC method', async () => {
  const calls = [];
  const rpc = new RpcClient('ws://127.0.0.1:8080/rpc');
  rpc.call = async (method, params, timeoutMs) => {
    calls.push([method, params, timeoutMs]);
    return { ok: true, result: { tx_id: 'asset-tx', round_ok: true } };
  };

  const result = await rpc.submitSignedAssetTransactionFinality('signed-asset-json');

  assert.equal(result.result.tx_id, 'asset-tx');
  assert.deepEqual(calls, [[
    'mempool_submit_signed_asset_transaction_finality',
    { signed_asset_transaction_json: 'signed-asset-json' },
    30000,
  ]]);
});

test('submitSignedEscrowTransactionFinality uses certified escrow finality RPC method', async () => {
  const calls = [];
  const rpc = new RpcClient('ws://127.0.0.1:8080/rpc');
  rpc.call = async (method, params, timeoutMs) => {
    calls.push([method, params, timeoutMs]);
    return { ok: true, result: { tx_id: 'escrow-tx', round_ok: true } };
  };

  const result = await rpc.submitSignedEscrowTransactionFinality('signed-escrow-json');

  assert.equal(result.result.tx_id, 'escrow-tx');
  assert.deepEqual(calls, [[
    'mempool_submit_signed_escrow_transaction_finality',
    { signed_escrow_transaction_json: 'signed-escrow-json' },
    30000,
  ]]);
});

test('pollOwnedObjectsTotal polls until the requested total is visible', async () => {
  const totals = [100, 100, 150];
  const snapshots = [];
  const rpc = {
    async ownedObjects() {
      const total = totals.shift() ?? 150;
      return { ok: true, result: { total_value: total, objects: [] } };
    },
  };

  const result = await pollOwnedObjectsTotal(rpc, 'pubkey', {
    minTotal: 150,
    intervalMs: 1,
    timeoutMs: 100,
    onSnapshot: snapshot => snapshots.push(snapshot.totalValue),
  });

  assert.equal(result.ok, true);
  assert.deepEqual(snapshots, [100, 100, 150]);
});

test('pollOwnedObjectsTotal returns a timeout result when the total never catches up', async () => {
  const rpc = {
    async ownedObjects() {
      return { ok: true, result: { total_value: 100, objects: [] } };
    },
  };

  const result = await pollOwnedObjectsTotal(rpc, 'pubkey', {
    minTotal: 200,
    intervalMs: 1,
    timeoutMs: 5,
  });

  assert.equal(result.ok, false);
  assert.equal(result.snapshot.totalValue, 100);
});
