import assert from 'node:assert/strict';
import test from 'node:test';

import { relayA666Export } from './a666-export-relay.js';

test('A666 export is authenticated, idempotent, and carries no custody material', async () => {
  const previousFetch = globalThis.fetch;
  const calls = [];
  let postCalls = 0;
  globalThis.fetch = async (url, options = {}) => {
    calls.push({ url, options });
    if (url === '/api/a666/export-jobs') {
      postCalls += 1;
      if (postCalls === 1) throw new TypeError('proxy restarting');
      return new Response(JSON.stringify({ ok: true, job_id: `0x${'11'.repeat(32)}`, status: 'queued' }), {
        status: 202, headers: { 'content-type': 'application/json' },
      });
    }
    return new Response(JSON.stringify({
      ok: true, status: 'accepted', receipt_id: '22'.repeat(48),
      ethereum_tx_hash: `0x${'33'.repeat(32)}`,
    }), { status: 200, headers: { 'content-type': 'application/json' } });
  };
  try {
    const result = await relayA666Export({
      routeId: 'pftl-a666-ethereum-wA666-usdc-v1',
      routeConfigDigest: '44'.repeat(48),
      packetHash: '55'.repeat(48),
      packetDigest: '66'.repeat(32),
      ethereumRecipient: `0x${'77'.repeat(20)}`,
      amountAtoms: '1000000',
      deadlineSeconds: Math.floor(Date.now() / 1000) + 7200,
      proxyAuthToken: 'session-secret',
    });
    assert.equal(result.status, 'accepted');
    assert.equal(postCalls, 2, 'a lost POST response must be retried against the durable job ID');
    const posts = calls.filter(call => call.url === '/api/a666/export-jobs');
    assert.ok(posts.every(call => call.options.headers.Authorization === 'Bearer session-secret'));
    assert.ok(posts.every(call => !call.options.body.includes('session-secret')));
    assert.match(calls.at(-1).url, /^\/api\/a666\/export-jobs\//);
  } finally {
    if (previousFetch === undefined) delete globalThis.fetch;
    else globalThis.fetch = previousFetch;
  }
});
