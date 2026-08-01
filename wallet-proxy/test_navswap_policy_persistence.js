const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');

const {
  clearNavswapIdempotencyForTest,
  executeNavswapIdempotentRequest,
  loadNavswapIdempotencyStore,
  navswapIdempotencyStorePath,
} = require('./server');
const { withEnvAsync } = require('./test_navswap_env');

async function testNavswapIdempotencyReplaysSameRequestAndRejectsConflict() {
  clearNavswapIdempotencyForTest();
  try {
    let calls = 0;
    const request = {
      method: 'POST',
      pathname: '/api/navswap/runs',
      body: {
        idempotency_key: 'navswap-test-key-1',
        route: 'transparent_navswap',
        amount: '1',
      },
    };
    const first = await executeNavswapIdempotentRequest(request, async () => {
      calls += 1;
      return { ok: true, schema: 'postfiat-navswap-run-v1', run_id: 'run-1' };
    });
    const replay = await executeNavswapIdempotentRequest(request, async () => {
      calls += 1;
      return { ok: true, schema: 'postfiat-navswap-run-v1', run_id: 'run-2' };
    });
    const conflict = await executeNavswapIdempotentRequest({
      ...request,
      body: { ...request.body, amount: '2' },
    }, async () => {
      calls += 1;
      return { ok: true, schema: 'postfiat-navswap-run-v1', run_id: 'run-3' };
    });

    assert.strictEqual(calls, 1);
    assert.strictEqual(first.run_id, 'run-1');
    assert.strictEqual(first.idempotency.replayed, false);
    assert.strictEqual(replay.run_id, 'run-1');
    assert.strictEqual(replay.idempotency.replayed, true);
    assert.strictEqual(conflict.ok, false);
    assert.strictEqual(conflict.code, 'navswap_idempotency_key_reused');
  } finally {
    clearNavswapIdempotencyForTest();
  }
}

async function testNavswapIdempotencySharesConcurrentRequest() {
  clearNavswapIdempotencyForTest();
  let release;
  const gate = new Promise((resolve) => { release = resolve; });
  try {
    let calls = 0;
    const request = {
      method: 'POST',
      pathname: '/api/navswap/devnet-fund-pfusdc',
      body: {
        idempotency_key: 'navswap-test-key-2',
        route: 'transparent_navswap',
        amount: '1',
      },
    };
    const firstPromise = executeNavswapIdempotentRequest(request, async () => {
      calls += 1;
      await gate;
      return { ok: true, schema: 'postfiat-navswap-devnet-funding-v1', tx_id: 'funding-tx' };
    });
    const replayPromise = executeNavswapIdempotentRequest(request, async () => {
      calls += 1;
      return { ok: true, schema: 'postfiat-navswap-devnet-funding-v1', tx_id: 'duplicate-tx' };
    });
    release();
    const [first, replay] = await Promise.all([firstPromise, replayPromise]);

    assert.strictEqual(calls, 1);
    assert.strictEqual(first.tx_id, 'funding-tx');
    assert.strictEqual(first.idempotency.replayed, false);
    assert.strictEqual(replay.tx_id, 'funding-tx');
    assert.strictEqual(replay.idempotency.replayed, true);
  } finally {
    clearNavswapIdempotencyForTest();
  }
}

async function testNavswapIdempotencyStoreReplaysAfterReload() {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-navswap-idempotency-'));
  const storePath = path.join(tmpDir, 'idempotency.jsonl');
  try {
    await withEnvAsync({ NAVSWAP_IDEMPOTENCY_STORE_PATH: storePath }, async () => {
      clearNavswapIdempotencyForTest();
      try {
        assert.strictEqual(navswapIdempotencyStorePath(), storePath);
        let calls = 0;
        const request = {
          method: 'POST',
          pathname: '/api/navswap/runs',
          body: {
            idempotency_key: 'navswap-durable-key-1',
            route: 'transparent_navswap',
            amount: '1',
          },
        };
        const first = await executeNavswapIdempotentRequest(request, async () => {
          calls += 1;
          return { ok: true, schema: 'postfiat-navswap-run-v1', run_id: 'durable-run-1' };
        });
        assert.strictEqual(first.idempotency.replayed, false);
        assert(fs.existsSync(storePath));

        clearNavswapIdempotencyForTest();
        assert.strictEqual(loadNavswapIdempotencyStore().loaded_count, 1);
        const replay = await executeNavswapIdempotentRequest(request, async () => {
          calls += 1;
          return { ok: true, schema: 'postfiat-navswap-run-v1', run_id: 'duplicate' };
        });
        assert.strictEqual(calls, 1);
        assert.strictEqual(replay.run_id, 'durable-run-1');
        assert.strictEqual(replay.idempotency.replayed, true);
      } finally {
        clearNavswapIdempotencyForTest();
      }
    });
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

async function testFleetScriptsRequireConfiguredEndpoints() {
  const retiredHost = '198.51.100.10';
  const { configuredFleetEndpoints } = await import('../scripts/lib/configured-fleet-endpoints.mjs');
  assert.throws(() => configuredFleetEndpoints({}), /VALIDATOR_HOSTS must be supplied/);
  assert.deepStrictEqual(configuredFleetEndpoints({
    VALIDATOR_HOSTS: '192.0.2.1,192.0.2.2',
    VALIDATOR_RPC_PORTS: '27650,27651',
  }), {
    hosts: ['192.0.2.1', '192.0.2.2'],
    ports: [27650, 27651],
  });

  for (const relativePath of [
    '../scripts/wallet-shielded-ingress-sync-state',
    '../scripts/wan-devnet-state-sync',
    '../scripts/wallet-shielded-ingress-timeout-gate.mjs',
  ]) {
    const source = fs.readFileSync(path.join(__dirname, relativePath), 'utf8');
    assert(!source.includes(retiredHost), `${relativePath} contains retired validator-0`);
    assert(source.includes('VALIDATOR_HOSTS'), `${relativePath} must use configured fleet hosts`);
  }
}

async function runNavswapPolicyPersistenceTests() {
  await testFleetScriptsRequireConfiguredEndpoints();
  await testNavswapIdempotencyReplaysSameRequestAndRejectsConflict();
  await testNavswapIdempotencySharesConcurrentRequest();
  await testNavswapIdempotencyStoreReplaysAfterReload();
}

module.exports = { runNavswapPolicyPersistenceTests };
