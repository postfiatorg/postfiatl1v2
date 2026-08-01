import assert from 'node:assert/strict';
import test from 'node:test';

import { buildA666ReturnBurnCalldata, createA666ReturnJob } from './a666-return-relay.js';

test('return burn calldata exactly matches the deployed Solidity ABI', () => {
  const calldata = buildA666ReturnBurnCalldata({
    amountAtoms: '5000000',
    pftlRecipient: 'pf2ddefa436ad1ccc40b61e26409b334ace71b9fb2',
    nativeNavAssetId: '521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c',
    returnNonce: '11'.repeat(32),
  });
  assert.equal(calldata, '0xf34c595b00000000000000000000000000000000000000000000000000000000004c4b40000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000e01111111111111111111111111111111111111111111111111111111111111111000000000000000000000000000000000000000000000000000000000000002a706632646465666134333661643163636334306236316532363430396233333461636537316239666232000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000030521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c00000000000000000000000000000000');
});

test('return relay request is authenticated and carries no custody material', async () => {
  const previousFetch = globalThis.fetch;
  let request;
  globalThis.fetch = async (_url, options) => {
    request = options;
    return new Response(JSON.stringify({ ok: true, job_id: `0x${'22'.repeat(32)}`, status: 'queued' }), {
      status: 202, headers: { 'content-type': 'application/json' },
    });
  };
  try {
    await createA666ReturnJob({
      routeId: 'pftl-a666-ethereum-wA666-usdc-v1', routeConfigDigest: '33'.repeat(48),
      transactionHash: `0x${'44'.repeat(32)}`, ethereumSender: `0x${'55'.repeat(20)}`,
      pftlRecipient: `pf${'66'.repeat(20)}`, nativeNavAssetId: '77'.repeat(48),
      amountAtoms: '5000000', returnNonce: '88'.repeat(32), proxyAuthToken: 'session-secret',
    });
    assert.equal(request.headers.Authorization, 'Bearer session-secret');
    assert.ok(!request.body.includes('session-secret'));
  } finally {
    if (previousFetch === undefined) delete globalThis.fetch;
    else globalThis.fetch = previousFetch;
  }
});
