import assert from 'node:assert/strict';
import test from 'node:test';

import { relayVaultDeposit } from './bridge-relay.js';

test('vault relay carries the session proxy token outside the request body', async () => {
  const previousFetch = globalThis.fetch;
  let captured;
  globalThis.fetch = async (url, options) => {
    captured = { url, options };
    return new Response(JSON.stringify({ ok: true, submitted: [] }), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });
  };
  try {
    await relayVaultDeposit({
      depositTxHash: '0x' + '11'.repeat(32),
      depositId: '0x' + '22'.repeat(32),
      pftlRecipient: 'pf-test-recipient',
      depositor: '0x3333333333333333333333333333333333333333',
      amountAtoms: '1000000',
      idempotencyKey: 'vault-relay:test',
      routeProfileHash: '44'.repeat(48),
      routeEpoch: 5,
      routeBinding: '0x' + '55'.repeat(32),
      proxyAuthToken: 'session-only-token',
    });
    assert.equal(captured.url, '/api/bridge/relay');
    assert.equal(captured.options.headers.Authorization, 'Bearer session-only-token');
    assert.equal(captured.options.headers['Idempotency-Key'], 'vault-relay:test');
    assert.doesNotMatch(captured.options.body, /session-only-token/);
  } finally {
    if (previousFetch === undefined) delete globalThis.fetch;
    else globalThis.fetch = previousFetch;
  }
});
