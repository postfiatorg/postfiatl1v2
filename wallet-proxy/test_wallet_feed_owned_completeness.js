'use strict';

// The wallet feed reads one owned_objects page per validator per tick and
// marks whether that page is the whole inventory: a truncated page is
// published with owned_complete false, so a wallet never promotes it to its
// selection inventory (it pages with its own bounded all-pages read).
const assert = require('assert');
const net = require('net');

function validator(result) {
  const calls = [];
  const server = net.createServer((socket) => {
    socket.on('data', (chunk) => {
      const request = JSON.parse(chunk.toString('utf8').split('\n')[0]);
      calls.push(request);
      socket.write(`${JSON.stringify({ id: request.id, ok: true, result })}\n`);
    });
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve({ server, calls })));
}

const page = (overrides = {}) => ({
  schema: 'postfiat-owned-objects-v1',
  objects: [{ id: 'aa'.repeat(32), version: 1, value: 1, asset: 'PFT' }],
  object_count: 1,
  total_value: 101,
  truncated: true,
  cursor: null,
  next_cursor: `ooc2.tag.-.${'cc'.repeat(32)}.505654.${'aa'.repeat(32)}.1`,
  view_digest: 'cc'.repeat(32),
  height: 40,
  state_root: 'bb'.repeat(48),
  ...overrides,
});

async function snapshotFor(result) {
  const { server, calls } = await validator(result);
  const routing = require('./rpc-routing').create({
    net,
    RPC_FLEET: [{ validatorId: 'v1', host: '127.0.0.1', port: server.address().port }],
    FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT: 2048,
    WALLET_SUBSCRIPTION_READ_TIMEOUT_MS: 2000,
    TCP_TIMEOUT_MS: 2000,
    ENABLE_UPSTREAM_KEEPALIVE: false,
  });
  try {
    const snapshot = await routing.fetchWalletSnapshot({ owner_public_key_hex: 'ab'.repeat(32) });
    return { snapshot, calls };
  } finally {
    server.close();
  }
}

(async () => {
  const partial = await snapshotFor(page());
  assert.strictEqual(partial.calls.length, 1, 'one owned_objects query per validator per tick');
  assert.strictEqual(partial.snapshot.owned.total_value, 101);
  assert.strictEqual(partial.snapshot.owned_complete, false);

  const whole = await snapshotFor(page({ truncated: false, next_cursor: null }));
  assert.strictEqual(whole.snapshot.owned_complete, true);
  console.log('PASS the wallet feed marks a truncated owned_objects page as partial');
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
