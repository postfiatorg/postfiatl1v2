'use strict';

// owned_fence_status admission: the route takes a connection permit before
// its fan-out, identical in-flight reads for one lock share one fan-out, and
// distinct fan-outs in flight are bounded per caller and per gateway.
const assert = require('assert');
const WebSocket = require('ws');
const { defaultRpcResult, startProxyFixture } = require('./proxy-test-fixture');

const LOCK_A = '22'.repeat(48);
const LOCK_B = '33'.repeat(48);
const LOCK_C = '44'.repeat(48);

async function withGateway(env, run) {
  let calls = 0;
  const held = [];
  const fixture = await startProxyFixture({
    fleetSize: 3,
    env,
    rpcHandler(request) {
      if (request.method !== 'owned_fence_status') return defaultRpcResult(request);
      calls += 1;
      // Validators answer only when the test releases them.
      return new Promise((resolve) => held.push(() => resolve({
        ok: true,
        result: { lock_id: request.params.lock_id, validator_id: request.id, signature_hex: 'ab' },
      })));
    },
  });
  const sockets = [];
  const connect = async () => {
    const ws = new WebSocket(fixture.url);
    sockets.push(ws);
    await new Promise((resolve, reject) => { ws.once('open', resolve); ws.once('error', reject); });
    return (id, lockId) => new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`no reply to ${id}`)), 5_000);
      const onMessage = (data) => {
        const message = JSON.parse(data.toString());
        if (message.id === id) { clearTimeout(timer); ws.off('message', onMessage); resolve(message); }
      };
      ws.on('message', onMessage);
      ws.send(JSON.stringify({ version: 'postfiat-local-rpc-v1', id, method: 'owned_fence_status', params: { lock_id: lockId } }));
    });
  };
  const waitForCalls = async (count) => {
    const deadline = Date.now() + 5_000;
    while (calls < count) {
      if (Date.now() > deadline) throw new Error(`expected ${count} validator calls, saw ${calls}`);
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
  };
  const release = () => { while (held.length) held.shift()(); };
  try {
    await run({ connect, waitForCalls, release, calls: () => calls });
  } finally {
    release();
    sockets.forEach((ws) => ws.close());
    await fixture.close();
  }
}

async function main() {
  // The permit: with one upstream request per connection, a second read
  // while the first has no answer is refused, not fanned out.
  await withGateway({ MAX_TCP_PER_WS: '1' }, async ({ connect, waitForCalls, release, calls }) => {
    const ask = await connect();
    const first = ask('permit-1', LOCK_A);
    await waitForCalls(3);
    const second = await ask('permit-2', LOCK_B);
    assert.strictEqual(second.error.code, 'proxy_rate_limited');
    release();
    assert.strictEqual((await first).ok, true);
    assert.strictEqual(calls(), 3);
    // Released in `finally`: the next read is admitted.
    const third = ask('permit-3', LOCK_B);
    await waitForCalls(6);
    release();
    assert.strictEqual((await third).ok, true);
  });
  console.log('PASS owned_fence_status takes a connection permit before its fan-out');

  // Coalescing: two callers reading one lock at once share one fan-out.
  await withGateway({}, async ({ connect, waitForCalls, release, calls }) => {
    const [askOne, askTwo] = [await connect(), await connect()];
    const one = askOne('same-1', LOCK_A);
    await waitForCalls(3);
    const two = askTwo('same-2', LOCK_A);
    await new Promise((resolve) => setTimeout(resolve, 100));
    release();
    const [first, second] = await Promise.all([one, two]);
    assert.strictEqual(calls(), 3, 'one fan-out for both reads');
    assert.deepStrictEqual(first.result, second.result);
    assert.deepStrictEqual([first.id, second.id], ['same-1', 'same-2']);
  });
  console.log('PASS identical in-flight owned_fence_status reads share one fan-out');

  // Per-caller bound: a second distinct lock from one connection is busy.
  await withGateway({ FASTPAY_FENCE_STATUS_MAX_FANOUTS_PER_CALLER: '1' }, async ({ connect, waitForCalls, release }) => {
    const ask = await connect();
    const first = ask('caller-1', LOCK_A);
    await waitForCalls(3);
    const busy = await ask('caller-2', LOCK_B);
    assert.strictEqual(busy.error.code, 'proxy_fence_status_busy');
    assert.match(busy.error.message, /caller/);
    assert(busy.error.retry_after_ms > 0);
    // Joining the read already in flight is not a new fan-out.
    const joined = ask('caller-3', LOCK_A);
    release();
    assert.strictEqual((await first).ok, true);
    assert.strictEqual((await joined).ok, true);
  });
  console.log('PASS owned_fence_status fan-outs are bounded per caller with a typed busy answer');

  // Global bound: across connections.
  await withGateway({ FASTPAY_FENCE_STATUS_MAX_FANOUTS: '2' }, async ({ connect, waitForCalls, release }) => {
    const [askOne, askTwo, askThree] = [await connect(), await connect(), await connect()];
    const one = askOne('global-1', LOCK_A);
    const two = askTwo('global-2', LOCK_B);
    await waitForCalls(6);
    const busy = await askThree('global-3', LOCK_C);
    assert.strictEqual(busy.error.code, 'proxy_fence_status_busy');
    assert.match(busy.error.message, /gateway/);
    release();
    assert.strictEqual((await one).ok, true);
    assert.strictEqual((await two).ok, true);
  });
  console.log('PASS owned_fence_status fan-outs are bounded per gateway with a typed busy answer');
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
