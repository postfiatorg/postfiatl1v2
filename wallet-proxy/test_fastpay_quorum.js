// Quorum test for broadcastFastpayMutation: an owned-apply must succeed when a
// BFT quorum of validators (5 of 6) apply it, even if one validator fails. The
// previous implementation required ALL validators and failed valid transfers.
//
// We spin up 6 local TCP RPC stubs: 5 return ok:true for both `status` and the
// broadcast method, 1 returns ok:false. We set RPC_FLEET to point at them,
// require the proxy, call broadcastFastpayMutation({ method: 'owned_apply' }),
// and assert ok:true with applied_count === 5.

const assert = require('assert');
const net = require('net');

const VALIDATOR_COUNT = 6;
const PORTS = [];
const SEEN_REQUESTS = [];

function makeStubServer(validatorId, applySucceeds) {
  return net.createServer((socket) => {
    let buffer = '';
    socket.on('data', (chunk) => {
      buffer += chunk.toString('utf8');
      let idx;
      while ((idx = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, idx);
        buffer = buffer.slice(idx + 1);
        const request = JSON.parse(line);
        SEEN_REQUESTS.push({ validatorId, request });
        // Status always succeeds so the validator counts as converged. The
        // broadcast certificate-apply methods succeed only
        // if applySucceeds — modeling a validator that is up but rejects/fails
        // the apply (the user's real-world failure mode).
        const ok = (request.method === 'status' || request.method === 'server_info') ? true : applySucceeds;
        socket.write(JSON.stringify({
          version: 'postfiat-local-rpc-v1',
          id: request.id,
          ok,
          result: ok ? {
            block_height: 100,
            block_tip_hash: 'tip',
            state_root: 'root',
            validator_id: validatorId,
            validator_count: VALIDATOR_COUNT,
            chain_id: 'postfiat-wan-devnet',
            summary: `applied on ${validatorId}`,
            created_objects: [],
          } : null,
          error: ok ? null : { code: 'validator_apply_failed', message: 'stub apply failure' },
          events: [],
        }) + '\n');
      }
    });
  });
}

async function main() {
  // Boot 6 stub servers; validator 5 will fail.
  const servers = [];
  for (let i = 0; i < VALIDATOR_COUNT; i++) {
    const srv = makeStubServer(`validator-${i}`, i !== 5);
    await new Promise((resolve) => srv.listen(0, '127.0.0.1', resolve));
    PORTS.push(srv.address().port);
    servers.push(srv);
  }

  // Set RPC_FLEET env BEFORE requiring the proxy module.
  const fleetStr = Array.from({ length: VALIDATOR_COUNT }, (_, i) =>
    `validator-${i}=127.0.0.1:${PORTS[i]}`,
  ).join(',');
  process.env.RPC_FLEET = fleetStr;
  // Disable keep-alive so each request is a fresh one-shot connection (simpler
  // stub interaction; the bug under test is in the quorum logic, not the
  // connection reuse).
  process.env.ENABLE_UPSTREAM_KEEPALIVE = 'false';
  // Disable primary-success requirement for this test (default is false anyway).
  process.env.FASTPAY_REQUIRE_PRIMARY_SUCCESS = 'false';
  process.env.FASTPAY_CERTIFICATE_FINALITY_ENABLED = 'false';
  // Ensure the broadcast required count is the quorum (the fix under test).
  delete process.env.FASTPAY_BROADCAST_REQUIRED_COUNT;

  const { broadcastFastpayMutation, bftQuorumThreshold } = require('./server');

  const quorum = bftQuorumThreshold(VALIDATOR_COUNT);
  assert.strictEqual(quorum, 5, 'BFT quorum for 6 validators is 5');

  const request = {
    version: 'postfiat-local-rpc-v1',
    id: 'test-owned-apply',
    method: 'owned_apply',
    params: { cert_json: '{"order":{"inputs":[],"outputs":[],"fee":0,"nonce":1,"memos":[]},"owner_pubkey_hex":"x","owner_signature_hex":"y","votes":[]}' },
  };

  const response = await broadcastFastpayMutation(request);

  // The fix: success at quorum (5), not all 6.
  assert.strictEqual(response.ok, true, `expected ok:true at quorum ${quorum}, got ok:${response.ok} error=${JSON.stringify(response.error)}`);
  assert.strictEqual(response.result.applied_count, 5, `expected 5 applied, got ${response.result.applied_count}`);
  assert.strictEqual(response.result.required_count, quorum, `expected required_count=${quorum}, got ${response.result.required_count}`);
  assert.strictEqual(response.result.fleet_count, VALIDATOR_COUNT);

  console.log('PASS broadcastFastpayMutation succeeds at BFT quorum (5/6) with one validator down');

  // Now force the old behavior via env override and confirm it FAILS at 5/6
  // (proves the override works and that the old all-6 requirement was the bug).
  process.env.FASTPAY_BROADCAST_REQUIRED_COUNT = String(VALIDATOR_COUNT);
  // Re-require won't re-evaluate; the override is read per-call inside
  // broadcastFastpayMutation, so the next call uses it.
  const responseAll = await broadcastFastpayMutation(request);
  assert.strictEqual(responseAll.ok, false, 'with FASTPAY_BROADCAST_REQUIRED_COUNT=6 and one validator down, expected ok:false');
  assert.ok(/failed before 6\/6 validator completion/.test(responseAll.error.message), `expected 6/6 failure message, got: ${responseAll.error.message}`);
  console.log('PASS broadcastFastpayMutation fails when forced to require all 6 (env override restores old behavior)');

  delete process.env.FASTPAY_BROADCAST_REQUIRED_COUNT;
  const v3Request = {
    version: 'postfiat-local-rpc-v1',
    id: 'test-owned-apply-v3',
    method: 'owned_apply_v3',
    params: { cert_json: request.params.cert_json },
  };
  const v3Response = await broadcastFastpayMutation(v3Request);
  assert.strictEqual(v3Response.ok, true, 'v3 apply must succeed at quorum');
  const v3Requests = SEEN_REQUESTS.filter(({ request: seen }) => seen.method === 'owned_apply_v3');
  assert.ok(v3Requests.length >= quorum, `expected at least ${quorum} v3 applies, got ${v3Requests.length}`);
  for (const { validatorId, request: seen } of v3Requests) {
    assert.strictEqual(
      seen.params.validator_id,
      validatorId,
      'v3 apply must bind each request to the validator receiving it',
    );
  }
  assert.strictEqual(
    new Set(v3Requests.map(({ validatorId }) => validatorId)).size,
    v3Requests.length,
    'v3 apply must not duplicate a validator target',
  );
  console.log('PASS v3 apply injects the exact per-target validator identity and resolves at quorum');

  for (const srv of servers) srv.close();
  console.log('fastpay quorum tests passed');
}

main().catch((err) => {
  console.error('fastpay quorum test FAILED:', err);
  process.exit(1);
});
