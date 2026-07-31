import assert from 'node:assert/strict';
import test from 'node:test';

import {
  assertBridgeReadinessMatchesRoute,
  loadBridgeReadiness,
  relayVaultDeposit,
  waitForBridgeReadiness,
} from './bridge-relay.js';

test('vault relay carries the session proxy token outside the request body', async () => {
  const previousFetch = globalThis.fetch;
  const captured = [];
  globalThis.fetch = async (url, options = {}) => {
    captured.push({ url, options });
    const payload = url === '/api/bridge/jobs'
      ? { ok: true, job_id: '0x' + '55'.repeat(32), status: 'queued' }
      : { ok: true, status: 'accepted', receipt_code: 'ACCEPTED', receipt_id: '0x' + '66'.repeat(32) };
    return new Response(JSON.stringify(payload), {
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
    assert.equal(captured[0].url, '/api/bridge/jobs');
    assert.equal(captured[0].options.headers.Authorization, 'Bearer session-only-token');
    assert.equal(captured[0].options.headers['Idempotency-Key'], 'vault-relay:test');
    assert.doesNotMatch(captured[0].options.body, /session-only-token/);
    assert.match(captured[1].url, /^\/api\/bridge\/jobs\//);
    assert.equal(captured[1].options.method, undefined);
  } finally {
    if (previousFetch === undefined) delete globalThis.fetch;
    else globalThis.fetch = previousFetch;
  }
});

test('bridge readiness is loaded from the Ethereum mainnet route', async () => {
  const previousFetch = globalThis.fetch;
  let requested;
  globalThis.fetch = async (url) => {
    requested = url;
    return new Response(JSON.stringify({ ok: true, ready: true }), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });
  };
  try {
    await loadBridgeReadiness();
    assert.equal(requested, '/api/bridge/readiness?route=ethereum-mainnet-usdc-v1');
  } finally {
    if (previousFetch === undefined) delete globalThis.fetch;
    else globalThis.fetch = previousFetch;
  }
});

test('bridge readiness must match every governed route identity', () => {
  const profile = {
    route_id: 'ethereum-mainnet-usdc-v1',
    source_chain_id: 1,
    asset_id: '11'.repeat(48),
    verifier_program_vkey: '0x' + '22'.repeat(32),
  };
  const route = {
    profile,
    profileHash: '33'.repeat(48),
    vaultAddress: '0x' + '44'.repeat(20),
    vaultRuntimeCodeHash: '0x' + '55'.repeat(32),
    tokenAddress: '0x' + '66'.repeat(20),
    tokenRuntimeCodeHash: '0x' + '77'.repeat(32),
  };
  const readiness = {
    ok: true,
    ready: true,
    route_id: profile.route_id,
    source_chain_id: profile.source_chain_id,
    source_proof_kind: 'sp1-ethereum-finality-v1',
    route_profile_hash: route.profileHash,
    asset_id: profile.asset_id,
    vault_address: route.vaultAddress,
    vault_runtime_code_hash: route.vaultRuntimeCodeHash,
    token_address: route.tokenAddress,
    token_runtime_code_hash: route.tokenRuntimeCodeHash,
    program_vkey: profile.verifier_program_vkey,
    observer_attestor_enabled: false,
    prover_authenticated: true,
    prover_healthy: true,
  };
  assert.equal(assertBridgeReadinessMatchesRoute(readiness, route), readiness);
  assert.throws(
    () => assertBridgeReadinessMatchesRoute({ ...readiness, route_profile_hash: '88'.repeat(48) }, route),
    /not ready/,
  );
});

test('bridge readiness retries transient warming but never retries identity mismatch', async () => {
  const previousFetch = globalThis.fetch;
  const profile = {
    route_id: 'ethereum-mainnet-usdc-v1',
    source_chain_id: 1,
    asset_id: '11'.repeat(48),
    verifier_program_vkey: '0x' + '22'.repeat(32),
  };
  const route = {
    profile,
    profileHash: '33'.repeat(48),
    vaultAddress: '0x' + '44'.repeat(20),
    vaultRuntimeCodeHash: '0x' + '55'.repeat(32),
    tokenAddress: '0x' + '66'.repeat(20),
    tokenRuntimeCodeHash: '0x' + '77'.repeat(32),
  };
  const ready = {
    ok: true,
    ready: true,
    route_id: profile.route_id,
    source_chain_id: profile.source_chain_id,
    source_proof_kind: 'sp1-ethereum-finality-v1',
    route_profile_hash: route.profileHash,
    asset_id: profile.asset_id,
    vault_address: route.vaultAddress,
    vault_runtime_code_hash: route.vaultRuntimeCodeHash,
    token_address: route.tokenAddress,
    token_runtime_code_hash: route.tokenRuntimeCodeHash,
    program_vkey: profile.verifier_program_vkey,
    observer_attestor_enabled: false,
    prover_authenticated: true,
    prover_healthy: true,
  };
  let calls = 0;
  globalThis.fetch = async () => {
    calls += 1;
    const payload = calls < 3
      ? { ok: false, ready: false, message: 'preflight warming' }
      : ready;
    return new Response(JSON.stringify(payload), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });
  };
  try {
    assert.deepEqual(
      await waitForBridgeReadiness(route, { attempts: 3, retryMs: 1 }),
      ready,
    );
    assert.equal(calls, 3);

    calls = 0;
    globalThis.fetch = async () => {
      calls += 1;
      return new Response(JSON.stringify({
        ...ready,
        route_profile_hash: '88'.repeat(48),
      }), { status: 200, headers: { 'content-type': 'application/json' } });
    };
    await assert.rejects(
      () => waitForBridgeReadiness(route, { attempts: 3, retryMs: 1 }),
      /not ready/,
    );
    assert.equal(calls, 1);
  } finally {
    if (previousFetch === undefined) delete globalThis.fetch;
    else globalThis.fetch = previousFetch;
  }
});
