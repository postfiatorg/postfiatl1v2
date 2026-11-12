'use strict';

// owned_fence_status: the gateway asks every validator and returns each
// signed answer with its validator, unchanged; no majority, no shared cache.
const assert = require('assert');
const WebSocket = require('ws');
const { defaultRpcResult, startProxyFixture } = require('./proxy-test-fixture');

const LOCK = '22'.repeat(48);

async function main() {
  let calls = 0;
  const fixture = await startProxyFixture({
    fleetSize: 3,
    rpcHandler(request) {
      if (request.method !== 'owned_fence_status') return defaultRpcResult(request);
      calls += 1;
      assert.deepStrictEqual(request.params, { lock_id: LOCK });
      const validator = request.id.replace('proxy-fence-status-', '');
      if (validator === 'validator-2') {
        return { ok: false, error: { code: 'fastpay_fence_status_signer_unavailable', message: 'socket mode' } };
      }
      // Two validators at different tips: the gateway must not merge them.
      return {
        ok: true,
        result: { lock_id: LOCK, validator_id: validator, observed_height: validator === 'validator-0' ? 7 : 8, signature_hex: 'ab' },
      };
    },
  });
  const ws = new WebSocket(fixture.url);
  try {
    await new Promise((resolve, reject) => { ws.once('open', resolve); ws.once('error', reject); });
    const ask = (id) => new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('no owned_fence_status reply')), 5_000);
      const onMessage = (data) => {
        const message = JSON.parse(data.toString());
        if (message.id === id) { clearTimeout(timer); ws.off('message', onMessage); resolve(message); }
      };
      ws.on('message', onMessage);
      ws.send(JSON.stringify({ version: 'postfiat-local-rpc-v1', id, method: 'owned_fence_status', params: { lock_id: LOCK } }));
    });
    const reply = await ask('fs-1');
    assert.strictEqual(reply.ok, true, JSON.stringify(reply.error));
    assert.strictEqual(reply.result.fleet_count, 3);
    assert.strictEqual(reply.result.answered_count, 2);
    assert.strictEqual(reply.result.partial, true);
    assert.deepStrictEqual(reply.result.answers.map((a) => [a.validator_id, a.ok]), [
      ['validator-0', true], ['validator-1', true], ['validator-2', false],
    ]);
    assert.strictEqual(reply.result.answers[0].status.observed_height, 7);
    assert.strictEqual(reply.result.answers[1].status.observed_height, 8);
    assert.strictEqual(reply.result.answers[2].error.code, 'fastpay_fence_status_signer_unavailable');
    assert(!('result' in reply.result) && !('majority' in reply.result));
    await ask('fs-2');
    assert.strictEqual(calls, 6, 'a second read asks every validator again');
    console.log('PASS owned_fence_status returns every validator answer unmerged');
  } finally {
    ws.close();
    await fixture.close();
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
