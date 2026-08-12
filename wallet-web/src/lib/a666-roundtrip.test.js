import assert from 'node:assert/strict';
import test from 'node:test';

import {
  A666_ROUNDTRIP_AMOUNT,
  A666_ROUNDTRIP_CONFIRMATION,
  loadA666RoundtripStatus,
  startA666Roundtrip,
} from './a666-roundtrip.js';

const statusPayload = {
  schema: 'stakehub-a666-wallet-roundtrip-v1',
  ok: true,
  route: 'pftl-a666-ethereum-wA666-usdc-v1',
  amount: '10.000000',
  amount_atoms: 10_000_000,
};

test('wallet status uses the existing bearer session', async (t) => {
  const originalFetch = globalThis.fetch;
  t.after(() => { globalThis.fetch = originalFetch; });
  globalThis.fetch = async (url, options) => {
    assert.equal(url, '/api/a666-roundtrip/status');
    assert.equal(options.method, 'GET');
    assert.equal(options.headers.Authorization, 'Bearer wallet-session-token');
    return { ok: true, status: 200, json: async () => statusPayload };
  };
  assert.deepEqual(await loadA666RoundtripStatus('wallet-session-token'), statusPayload);
});

test('wallet start constructs the fixed action without pasted JSON', async (t) => {
  const originalFetch = globalThis.fetch;
  t.after(() => { globalThis.fetch = originalFetch; });
  globalThis.fetch = async (url, options) => {
    assert.equal(url, '/api/a666-roundtrip/start');
    assert.equal(options.method, 'POST');
    assert.deepEqual(JSON.parse(options.body), {
      amount: A666_ROUNDTRIP_AMOUNT,
      confirmation: A666_ROUNDTRIP_CONFIRMATION,
    });
    return { ok: true, status: 200, json: async () => statusPayload };
  };
  await startA666Roundtrip('wallet-session-token');
});

test('wallet fails closed on missing auth and route identity mismatch', async (t) => {
  await assert.rejects(loadA666RoundtripStatus(''), /authorization is missing/);
  const originalFetch = globalThis.fetch;
  t.after(() => { globalThis.fetch = originalFetch; });
  globalThis.fetch = async () => ({
    ok: true,
    status: 200,
    json: async () => ({ ...statusPayload, route: 'wrong-route' }),
  });
  await assert.rejects(loadA666RoundtripStatus('wallet-session-token'), /identity mismatch/);
});
