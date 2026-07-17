const assert = require('assert');
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');

const {
  buildNavswapNavProofResponse,
  clearNavswapIdempotencyForTest,
  executeNavswapCapabilities,
  executeNavswapIdempotentRequest,
  executeNavswapQuote,
  executeNavswapRun,
  loadNavswapIdempotencyStore,
  navswapIdempotencyStorePath,
  navswapRunPublic,
} = require('./server');
const { withEnvAsync } = require('./test_navswap_env');

async function testStakehubTransparentQuoteRequiresFreshProof() {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (url.pathname === '/api/navcoin') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        token: { supply: 4000, nav_per_unit: 4.75 },
        proof: {
          proof_status: 'stale',
          stale: true,
          reserve_packet_hash: 'stale-packet',
        },
        pftl: { chain_id: 'stakehub-demo-chain', current_height: 100300 },
      }));
      return;
    }
    if (url.pathname === '/api/navcoin/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ available: true, market_operations_status: 'active' }));
      return;
    }
    res.writeHead(404, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: 'not found' }));
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const port = server.address().port;
    await withEnvAsync({
      NAVSWAP_STAKEHUB_BASE_URL: `http://127.0.0.1:${port}`,
      NAVSWAP_STAKEHUB_READ_TIMEOUT_MS: '5000',
    }, async () => {
      const quote = await executeNavswapQuote({
        route: 'stakehub_transparent_roundtrip',
        from_asset: 'pfUSDC',
        to_asset: 'a651',
        amount: '1',
      });
      assert.strictEqual(quote.ok, false);
      assert.strictEqual(quote.code, 'stakehub_nav_proof_not_fresh');
      assert.strictEqual(quote.nav_proof.stale, true);
    });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

async function testStakehubTransparentRunSurfacesStakehubError() {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (req.method === 'GET' && url.pathname === '/api/navcoin') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        token: { supply: 4000, nav_per_unit: 4.75 },
        proof: {
          proof_status: 'fresh',
          stale: false,
          reserve_packet_hash: 'packet-hash',
          envelope_epoch: 3,
        },
        pftl: { current_height: 100300 },
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/navcoin/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ available: true, market_operations_status: 'active' }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/balances') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        address: 'pfoperator',
        pfusdc: { balance_atoms: 60003476 },
        a651: { balance_atoms: 3 },
        errors: [],
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ ok: null, status: 'idle' }));
      return;
    }
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({
      ok: false,
      status: 'failed',
      stage: 'vault_supply',
      error: 'validator-1 unavailable',
    }));
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const port = server.address().port;
    await withEnvAsync({
      NAVSWAP_STAKEHUB_BASE_URL: `http://127.0.0.1:${port}`,
      NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS: 'true',
      NAVSWAP_STAKEHUB_READ_TIMEOUT_MS: '5000',
    }, async () => {
      const run = await executeNavswapRun({
        route: 'stakehub_transparent_roundtrip',
        from_asset: 'pfUSDC',
        to_asset: 'a651',
        amount: '1',
      });
      assert.strictEqual(run.ok, false);
      assert.strictEqual(run.status, 'failed');
      assert.strictEqual(run.code, 'stakehub_transparent_run_failed');
      assert.match(run.message, /validator-1 unavailable/);
      assert.strictEqual(navswapRunPublic(run.run_id).result.stage, 'vault_supply');
    });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

async function testStakehubTransparentQuoteRequiresPreflightBalances() {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (url.pathname === '/api/navcoin') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        token: { supply: 4000, nav_per_unit: 4.75 },
        proof: {
          proof_status: 'fresh',
          stale: false,
          reserve_packet_hash: 'packet-hash',
          envelope_epoch: 3,
        },
        pftl: { current_height: 100300 },
      }));
      return;
    }
    if (url.pathname === '/api/navcoin/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ available: true, market_operations_status: 'active' }));
      return;
    }
    if (url.pathname === '/api/shielded-nav-swap/balances') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ errors: ['account assets unavailable'] }));
      return;
    }
    if (url.pathname === '/api/shielded-nav-swap/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ ok: null, status: 'idle' }));
      return;
    }
    res.writeHead(404, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: 'not found' }));
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const port = server.address().port;
    await withEnvAsync({
      NAVSWAP_STAKEHUB_BASE_URL: `http://127.0.0.1:${port}`,
      NAVSWAP_STAKEHUB_READ_TIMEOUT_MS: '5000',
    }, async () => {
      const quote = await executeNavswapQuote({
        route: 'stakehub_transparent_roundtrip',
        from_asset: 'pfUSDC',
        to_asset: 'a651',
        amount: '1',
      });
      assert.strictEqual(quote.ok, false);
      assert.strictEqual(quote.code, 'stakehub_transparent_balances_unavailable');
      assert.match(quote.message, /account assets unavailable/);
    });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

async function testStakehubTransparentQuoteBlocksFinalityRecovery() {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (url.pathname === '/api/navcoin') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        token: { supply: 4000, nav_per_unit: 4.75 },
        proof: {
          proof_status: 'fresh',
          stale: false,
          reserve_packet_hash: 'packet-hash',
          envelope_epoch: 3,
        },
        pftl: { current_height: 472 },
      }));
      return;
    }
    if (url.pathname === '/api/navcoin/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ available: true, market_operations_status: 'active' }));
      return;
    }
    if (url.pathname === '/api/shielded-nav-swap/balances') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        address: 'pfoperator',
        pfusdc: { balance_atoms: 60003476 },
        a651: { balance_atoms: 3 },
        errors: [],
      }));
      return;
    }
    if (url.pathname === '/api/shielded-nav-swap/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        ok: false,
        status: 'idle',
        transparent_roundtrip: {
          ok: false,
          status: 'needs_timeout_certificate',
          finality_recovery_required: true,
          current_height: 472,
          next_height: 473,
          message: 'PFTL height 473 view 0 needs a timeout certificate.',
        },
      }));
      return;
    }
    res.writeHead(404, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: 'not found' }));
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const port = server.address().port;
    await withEnvAsync({
      NAVSWAP_STAKEHUB_BASE_URL: `http://127.0.0.1:${port}`,
      NAVSWAP_STAKEHUB_READ_TIMEOUT_MS: '5000',
    }, async () => {
      const quote = await executeNavswapQuote({
        route: 'stakehub_transparent_roundtrip',
        from_asset: 'pfUSDC',
        to_asset: 'a651',
        amount: '1',
      });
      assert.strictEqual(quote.ok, false);
      assert.strictEqual(quote.code, 'stakehub_transparent_finality_recovery_required');
      assert.match(quote.message, /timeout certificate/);
      assert.strictEqual(
        quote.stakehub_preflight.swap_status.transparent_roundtrip.next_height,
        473,
      );
    });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

async function testStakehubTransparentCapabilitiesBlockFinalityRecovery() {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (url.pathname === '/api/shielded-nav-swap/balances') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        address: 'pfoperator',
        pfusdc: { balance_atoms: 60003476 },
        a651: { balance_atoms: 3 },
        errors: [],
      }));
      return;
    }
    if (url.pathname === '/api/shielded-nav-swap/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        ok: false,
        status: 'idle',
        transparent_roundtrip: {
          ok: false,
          status: 'needs_timeout_certificate',
          finality_recovery_required: true,
          message: 'PFTL height 473 view 0 needs a timeout certificate.',
        },
      }));
      return;
    }
    res.writeHead(404, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: 'not found' }));
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const port = server.address().port;
    await withEnvAsync({
      NAVSWAP_STAKEHUB_BASE_URL: `http://127.0.0.1:${port}`,
      NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS: 'true',
      NAVSWAP_STAKEHUB_READ_TIMEOUT_MS: '5000',
    }, async () => {
      const caps = await executeNavswapCapabilities(new Date('2026-06-29T00:00:00.000Z'));
      const route = caps.routes.stakehub_transparent_roundtrip;
      assert.strictEqual(route.enabled, false);
      assert.strictEqual(route.can_quote, false);
      assert.strictEqual(route.can_run, false);
      assert.strictEqual(route.status, 'stakehub_transparent_finality_recovery_required');
      assert.match(route.reason, /timeout certificate/);
    });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

async function testStakehubTransparentQuoteBlocksTransportRecovery() {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (url.pathname === '/api/navcoin') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        token: { supply: 4000, nav_per_unit: 4.75 },
        proof: {
          proof_status: 'fresh',
          stale: false,
          reserve_packet_hash: 'packet-hash',
          envelope_epoch: 3,
        },
        pftl: { current_height: 100300 },
      }));
      return;
    }
    if (url.pathname === '/api/navcoin/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ available: true, market_operations_status: 'active' }));
      return;
    }
    if (url.pathname === '/api/shielded-nav-swap/balances') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        address: 'pfoperator',
        pfusdc: { balance_atoms: 60003476 },
        a651: { balance_atoms: 3 },
        errors: [],
      }));
      return;
    }
    if (url.pathname === '/api/shielded-nav-swap/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        ok: false,
        status: 'idle',
        transparent_roundtrip: {
          ok: false,
          status: 'transport_recovery_required',
          finality_recovery_required: false,
          transport_recovery_required: true,
          message: 'Latest transparent run has incomplete certified transport artifacts.',
          latest_incomplete_run: {
            run_dir: '/tmp/stakehub-transparent-test',
            summary_exists: false,
          },
        },
      }));
      return;
    }
    res.writeHead(404, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: 'not found' }));
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const port = server.address().port;
    await withEnvAsync({
      NAVSWAP_STAKEHUB_BASE_URL: `http://127.0.0.1:${port}`,
      NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS: 'true',
      NAVSWAP_STAKEHUB_READ_TIMEOUT_MS: '5000',
    }, async () => {
      const quote = await executeNavswapQuote({
        route: 'stakehub_transparent_roundtrip',
        from_asset: 'pfUSDC',
        to_asset: 'a651',
        amount: '1',
      });
      assert.strictEqual(quote.ok, false);
      assert.strictEqual(quote.code, 'stakehub_transparent_transport_recovery_required');
      assert.match(quote.message, /incomplete certified transport/);
      assert.strictEqual(
        quote.stakehub_preflight.swap_status.transparent_roundtrip.transport_recovery_required,
        true,
      );

      const caps = await executeNavswapCapabilities(new Date('2026-06-29T00:00:00.000Z'));
      const route = caps.routes.stakehub_transparent_roundtrip;
      assert.strictEqual(route.enabled, false);
      assert.strictEqual(route.can_quote, false);
      assert.strictEqual(route.can_run, false);
      assert.strictEqual(route.status, 'stakehub_transparent_transport_recovery_required');
    });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

async function testNavProofFallbackAndStakehubPassthrough() {
  await withEnvAsync({
    NAVSWAP_STAKEHUB_BASE_URL: undefined,
    NAVSWAP_STAKEHUB_URL: undefined,
  }, async () => {
    const stub = await buildNavswapNavProofResponse(new URLSearchParams('asset_id=a651&phase=current'));
    assert.strictEqual(stub.ok, true);
    assert.strictEqual(stub.proof_available, false);
    assert.match(stub.message, /NAVSWAP_STAKEHUB_BASE_URL/);
  });

  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (url.pathname === '/api/navcoin') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        token: { supply: 4000, nav_per_unit: 4.75 },
        proof: {
          proof_status: 'fresh',
          stale: false,
          freshness_deadline_height: 100338,
          nav_per_unit: 4.75,
          reserve_packet_hash: 'packet-hash',
          envelope_epoch: 3,
          source_receipt_hashes: ['receipt-1'],
        },
        pftl: { chain_id: 'stakehub-demo-chain', current_height: 100300 },
      }));
      return;
    }
    if (url.pathname === '/api/navcoin/status') {
      assert.match(url.searchParams.get('asset_id'), /^[0-9a-f]{96}$/);
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        available: true,
        market_operations_status: 'active',
        accepted_policy_hash: 'policy-hash',
      }));
      return;
    }
    res.writeHead(404, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: 'not found' }));
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const port = server.address().port;
    await withEnvAsync({
      NAVSWAP_STAKEHUB_BASE_URL: `http://127.0.0.1:${port}`,
      NAVSWAP_STAKEHUB_READ_TIMEOUT_MS: '5000',
    }, async () => {
      const proof = await buildNavswapNavProofResponse(new URLSearchParams('asset_id=a651&phase=current'));
      assert.strictEqual(proof.ok, true);
      assert.strictEqual(proof.proof_available, true);
      assert.strictEqual(proof.source, 'stakehub:/api/navcoin');
      assert.strictEqual(proof.chain_id, 'stakehub-demo-chain');
      assert.strictEqual(proof.current_pftl_height, 100300);
      assert.strictEqual(proof.nav_epoch, 3);
      assert.strictEqual(proof.reserve_packet_hash, 'packet-hash');
      assert.strictEqual(proof.freshness_deadline_height, 100338);
      assert.strictEqual(proof.nav_per_unit, 4.75);
      assert.strictEqual(proof.supply, 4000);
      assert.strictEqual(proof.proof_status, 'fresh');
      assert.deepStrictEqual(proof.source_receipt_hashes, ['receipt-1']);
    });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

async function testNavswapIdempotencyReplaysSameRequestAndRejectsConflict() {
  clearNavswapIdempotencyForTest();
  try {
    let calls = 0;
    const first = await executeNavswapIdempotentRequest({
      method: 'POST',
      pathname: '/api/navswap/runs',
      body: {
        idempotency_key: 'navswap-test-key-1',
        route: 'transparent_navswap',
        amount: '1',
      },
    }, async () => {
      calls += 1;
      return { ok: true, schema: 'postfiat-navswap-run-v1', run_id: 'run-1' };
    });
    const replay = await executeNavswapIdempotentRequest({
      method: 'POST',
      pathname: '/api/navswap/runs',
      body: {
        idempotency_key: 'navswap-test-key-1',
        route: 'transparent_navswap',
        amount: '1',
      },
    }, async () => {
      calls += 1;
      return { ok: true, schema: 'postfiat-navswap-run-v1', run_id: 'run-2' };
    });
    const conflict = await executeNavswapIdempotentRequest({
      method: 'POST',
      pathname: '/api/navswap/runs',
      body: {
        idempotency_key: 'navswap-test-key-1',
        route: 'transparent_navswap',
        amount: '2',
      },
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
  await withEnvAsync({
    NAVSWAP_IDEMPOTENCY_STORE_PATH: storePath,
  }, async () => {
    clearNavswapIdempotencyForTest();
    try {
      assert.strictEqual(navswapIdempotencyStorePath(), storePath);
      let calls = 0;
      const first = await executeNavswapIdempotentRequest({
        method: 'POST',
        pathname: '/api/navswap/runs',
        body: {
          idempotency_key: 'navswap-durable-key-1',
          route: 'transparent_navswap',
          amount: '1',
        },
      }, async () => {
        calls += 1;
        return { ok: true, schema: 'postfiat-navswap-run-v1', run_id: 'durable-run-1' };
      });
      assert.strictEqual(first.run_id, 'durable-run-1');
      assert.strictEqual(first.idempotency.replayed, false);
      assert(fs.existsSync(storePath));

      clearNavswapIdempotencyForTest();
      const loaded = loadNavswapIdempotencyStore();
      assert.strictEqual(loaded.enabled, true);
      assert.strictEqual(loaded.loaded_count, 1);

      const replay = await executeNavswapIdempotentRequest({
        method: 'POST',
        pathname: '/api/navswap/runs',
        body: {
          idempotency_key: 'navswap-durable-key-1',
          route: 'transparent_navswap',
          amount: '1',
        },
      }, async () => {
        calls += 1;
        return { ok: true, schema: 'postfiat-navswap-run-v1', run_id: 'durable-run-duplicate' };
      });
      assert.strictEqual(calls, 1);
      assert.strictEqual(replay.run_id, 'durable-run-1');
      assert.strictEqual(replay.idempotency.replayed, true);

      const conflict = await executeNavswapIdempotentRequest({
        method: 'POST',
        pathname: '/api/navswap/runs',
        body: {
          idempotency_key: 'navswap-durable-key-1',
          route: 'transparent_navswap',
          amount: '2',
        },
      }, async () => {
        calls += 1;
        return { ok: true, schema: 'postfiat-navswap-run-v1', run_id: 'durable-run-conflict' };
      });
      assert.strictEqual(calls, 1);
      assert.strictEqual(conflict.ok, false);
      assert.strictEqual(conflict.code, 'navswap_idempotency_key_reused');
    } finally {
      clearNavswapIdempotencyForTest();
    }
  });
  fs.rmSync(tmpDir, { recursive: true, force: true });
}

async function runNavswapPolicyPersistenceTests() {
  await testFleetScriptsRequireConfiguredEndpoints();
  await testStakehubTransparentQuoteRequiresFreshProof();
  await testStakehubTransparentRunSurfacesStakehubError();
  await testStakehubTransparentQuoteRequiresPreflightBalances();
  await testStakehubTransparentQuoteBlocksFinalityRecovery();
  await testStakehubTransparentCapabilitiesBlockFinalityRecovery();
  await testStakehubTransparentQuoteBlocksTransportRecovery();
  await testNavProofFallbackAndStakehubPassthrough();
  await testNavswapIdempotencyReplaysSameRequestAndRejectsConflict();
  await testNavswapIdempotencySharesConcurrentRequest();
  await testNavswapIdempotencyStoreReplaysAfterReload();
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
    '../scripts/wallet-shielded-swap-step7-e2e.mjs',
    '../scripts/wallet-shielded-ingress-timeout-gate.mjs',
  ]) {
    const source = fs.readFileSync(path.join(__dirname, relativePath), 'utf8');
    assert(!source.includes(retiredHost), `${relativePath} contains retired validator-0`);
    assert(source.includes('VALIDATOR_HOSTS'), `${relativePath} must use configured fleet hosts`);
  }
}

module.exports = { runNavswapPolicyPersistenceTests };
