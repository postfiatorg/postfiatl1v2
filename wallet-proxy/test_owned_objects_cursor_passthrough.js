'use strict';

// The gateway forwards the owned_objects cursor and owner-key flag unchanged
// and returns next_cursor, height and state_root as the validator sent them.
const assert = require('assert');
const WebSocket = require('ws');
const { defaultRpcResult, startProxyFixture } = require('./proxy-test-fixture');

async function main() {
  const upstream = [];
  const fixture = await startProxyFixture({
    rpcHandler(request) {
      if (request.method !== 'owned_objects') return defaultRpcResult(request);
      upstream.push(request.params);
      return {
        ok: true,
        result: {
          schema: 'postfiat-owned-objects-v1',
          objects: [{ id: 'aa'.repeat(32), version: 1, value: 5, asset: 'PFT' }],
          cursor: request.params.cursor ?? null,
          next_cursor: 'ooc1.next',
          height: 41,
          state_root: 'cc'.repeat(48),
          owner_keys_omitted: request.params.omit_owner_keys === true,
        },
      };
    },
  });
  const ws = new WebSocket(fixture.url);
  try {
    await new Promise((resolve, reject) => { ws.once('open', resolve); ws.once('error', reject); });
    const params = {
      owner_public_key_hex: 'ab'.repeat(1952),
      limit: 128,
      cursor: 'ooc1.previous',
      omit_owner_keys: true,
    };
    ws.send(JSON.stringify({ version: 'postfiat-local-rpc-v1', id: 'oo-1', method: 'owned_objects', params }));
    const reply = await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('no owned_objects reply')), 5_000);
      ws.on('message', (data) => {
        const message = JSON.parse(data.toString());
        if (message.id === 'oo-1') { clearTimeout(timer); resolve(message); }
      });
    });
    assert.strictEqual(reply.ok, true, JSON.stringify(reply.error));
    assert.deepStrictEqual(upstream, [params]);
    assert.strictEqual(reply.result.cursor, 'ooc1.previous');
    assert.strictEqual(reply.result.next_cursor, 'ooc1.next');
    assert.strictEqual(reply.result.height, 41);
    assert.strictEqual(reply.result.owner_keys_omitted, true);
    console.log('PASS owned_objects cursor and flag pass through the gateway');
  } finally {
    ws.close();
    await fixture.close();
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
