'use strict';

// owned_fence_status budget: each new fan-out spends a token of its source
// address and of the gateway; joining a fan-out in flight spends none.
const assert = require('assert');
const WebSocket = require('ws');
const { defaultRpcResult, startProxyFixture } = require('./proxy-test-fixture');

const lock = (index) => index.toString(16).padStart(96, '0');
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function withGateway(env, fleetSize, run) {
  let calls = 0;
  const held = [];
  const fixture = await startProxyFixture({
    fleetSize,
    env,
    rpcHandler(request) {
      if (request.method !== 'owned_fence_status') return defaultRpcResult(request);
      calls += 1;
      return new Promise((resolve) => held.push(() => resolve({
        ok: true,
        result: { lock_id: request.params.lock_id, validator_id: request.id, signature_hex: 'ab' },
      })));
    },
  });
  const sockets = [];
  let next = 0;
  const connect = async (localAddress = '127.0.0.1') => {
    const ws = new WebSocket(fixture.url, { localAddress });
    sockets.push(ws);
    await new Promise((resolve, reject) => { ws.once('open', resolve); ws.once('error', reject); });
    ws.on('error', () => {});
    return (lockId) => new Promise((resolve, reject) => {
      const id = `budget-${next++}`;
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
      await sleep(10);
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

const busy = (answer, scope) => {
  assert.strictEqual(answer.ok, false);
  assert.strictEqual(answer.error.code, 'proxy_fence_status_busy');
  assert.match(answer.error.message, scope);
  assert(answer.error.retry_after_ms > 0);
};

async function main() {
  // The reviewer's case: nine connections from one address, six validators.
  // The first wave fills the 16 fan-outs (96 calls); the second wave, right
  // after it completes, is refused by the source budget.
  await withGateway({
    FASTPAY_FENCE_STATUS_SOURCE_BURST: '16',
    FASTPAY_FENCE_STATUS_SOURCE_REFILL_MS: '600000',
  }, 6, async ({ connect, waitForCalls, release, calls }) => {
    const asks = [];
    for (let index = 0; index < 9; index += 1) asks.push(await connect());
    const wave = asks.slice(0, 8).flatMap((ask, index) => [ask(lock(2 * index + 1)), ask(lock(2 * index + 2))]);
    await waitForCalls(96);
    busy(await asks[8](lock(100)), /in flight \(gateway\)/);
    release();
    for (const answer of await Promise.all(wave)) assert.strictEqual(answer.ok, true);
    const second = await Promise.all(asks.map((ask, index) => ask(lock(200 + index))));
    second.forEach((answer) => busy(answer, /source budget/));
    assert.strictEqual(calls(), 96, 'the second wave reached no validator');
  });
  console.log('PASS the reviewer\'s nine-connection second wave is refused by the source budget');

  // Refill: one token, regained after the hinted wait.
  await withGateway({
    FASTPAY_FENCE_STATUS_SOURCE_BURST: '1',
    FASTPAY_FENCE_STATUS_SOURCE_REFILL_MS: '300',
  }, 3, async ({ connect, waitForCalls, release }) => {
    const ask = await connect();
    const first = ask(lock(1));
    await waitForCalls(3);
    release();
    assert.strictEqual((await first).ok, true);
    const refused = await ask(lock(2));
    busy(refused, /source budget/);
    assert(refused.error.retry_after_ms <= 300);
    await sleep(refused.error.retry_after_ms + 50);
    const again = ask(lock(2));
    await waitForCalls(6);
    release();
    assert.strictEqual((await again).ok, true);
  });
  console.log('PASS the source budget refills over time and hints when');

  // Two addresses: one spending its budget does not starve the other.
  await withGateway({
    FASTPAY_FENCE_STATUS_SOURCE_BURST: '2',
    FASTPAY_FENCE_STATUS_SOURCE_REFILL_MS: '600000',
  }, 3, async ({ connect, waitForCalls, release }) => {
    const [askA, askB] = [await connect('127.0.0.1'), await connect('127.0.0.2')];
    for (const index of [1, 2]) {
      const read = askA(lock(index));
      await waitForCalls(3 * index);
      release();
      assert.strictEqual((await read).ok, true);
    }
    busy(await askA(lock(3)), /source budget/);
    const other = askB(lock(3));
    await waitForCalls(9);
    release();
    assert.strictEqual((await other).ok, true);
  });
  console.log('PASS two source addresses keep separate budgets');

  // The gateway budget bounds all sources together.
  await withGateway({
    FASTPAY_FENCE_STATUS_GLOBAL_BURST: '1',
    FASTPAY_FENCE_STATUS_GLOBAL_REFILL_MS: '600000',
  }, 3, async ({ connect, waitForCalls, release }) => {
    const [askA, askB] = [await connect('127.0.0.1'), await connect('127.0.0.2')];
    const read = askA(lock(1));
    await waitForCalls(3);
    busy(await askB(lock(2)), /gateway budget/);
    release();
    assert.strictEqual((await read).ok, true);
  });
  console.log('PASS the gateway budget bounds every source together');

  // Joining a fan-out in flight costs nothing: with two tokens, one fan-out
  // and three joins leave one token for a second lock. (The gateway caches
  // no fence-status answer; a join is its only shared answer.)
  await withGateway({
    FASTPAY_FENCE_STATUS_SOURCE_BURST: '2',
    FASTPAY_FENCE_STATUS_SOURCE_REFILL_MS: '600000',
  }, 3, async ({ connect, waitForCalls, release, calls }) => {
    const asks = [await connect(), await connect(), await connect(), await connect()];
    const first = asks[0](lock(1));
    await waitForCalls(3);
    const joins = asks.slice(1).map((ask) => ask(lock(1)));
    await sleep(100);
    release();
    for (const answer of [await first, ...(await Promise.all(joins))]) assert.strictEqual(answer.ok, true);
    assert.strictEqual(calls(), 3);
    const second = asks[1](lock(2));
    await waitForCalls(6);
    release();
    assert.strictEqual((await second).ok, true);
    busy(await asks[2](lock(3)), /source budget/);
  });
  console.log('PASS a read joining a fan-out in flight spends no token');
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
