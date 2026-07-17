import assert from 'node:assert/strict';
import test from 'node:test';
import {
  fastSwapDemoApi, formatAtoms, formatUsdE8, navPresentation, receiptPresentation, shorten,
} from './fastswap-demo.js';

test('posts an arbitrary wallet address to the bounded faucet route', async () => {
  const originalFetch = globalThis.fetch;
  let observed;
  globalThis.fetch = async (url, options) => {
    observed = { url, options };
    return {
      ok: true,
      status: 200,
      json: async () => ({ ok: true, result: { receipt: { accepted: true, code: 'accepted' } } }),
    };
  };
  try {
    await fastSwapDemoApi.faucet('pfde0ba09f38b1748f8d77709715e1095a0ff74d0f');
  } finally {
    globalThis.fetch = originalFetch;
  }
  assert.equal(observed.url, '/api/fastswap-demo/faucet');
  assert.equal(observed.options.method, 'POST');
  assert.deepEqual(JSON.parse(observed.options.body), {
    address: 'pfde0ba09f38b1748f8d77709715e1095a0ff74d0f',
  });
});

test('checks an existing grant through a read-only faucet status route', async () => {
  const originalFetch = globalThis.fetch;
  let observed;
  globalThis.fetch = async (url, options) => {
    observed = { url, options };
    return {
      ok: true,
      status: 200,
      json: async () => ({ ok: true, result: { claimed: true } }),
    };
  };
  try {
    await fastSwapDemoApi.faucetStatus('pfde0ba09f38b1748f8d77709715e1095a0ff74d0f');
  } finally {
    globalThis.fetch = originalFetch;
  }
  assert.equal(observed.url, '/api/fastswap-demo/faucet-status');
  assert.equal(observed.options.method, 'POST');
});

test('renders dust units and certified NAV without floating point rounding', () => {
    assert.equal(formatAtoms(9, 8), '0.00000009');
    assert.equal(formatAtoms(1, 8), '0.00000001');
    assert.equal(formatAtoms(9, 0), '9');
    assert.equal(formatUsdE8(820102177), '$8.20102177');
  });

test('shows packet age separately from the protocol freshness verdict', () => {
    const view = navPresentation({
      active: true,
      reserve_packet_fresh: true,
      supply_packet_fresh: true,
      policy_paused: false,
      reserve_packet_age_blocks: 650,
      packet_expires_at: 2_000_000,
      policy_blocks_remaining: 98,
      usd_e8: 820102177,
    }, 1_900_000);
    assert.equal(view.protocolFresh, true);
    assert.equal(view.verdict, 'VERIFIED · USABLE');
    assert.equal(view.ageLabel, '650 blocks old');
    assert.equal(view.price, '$8.20102177');
  });

test('fails closed when any freshness input is false', () => {
    assert.equal(navPresentation({ active: true, reserve_packet_fresh: false, supply_packet_fresh: true })
      .protocolFresh, false);
  });

test('never presents rejected or unknown receipts as success', () => {
    assert.equal(receiptPresentation({ receipt: { accepted: true, code: 'fastswap_applied' } }).tone,
      'accepted');
    assert.equal(receiptPresentation({ receipt: { accepted: false, code: 'policy_expired' } }).tone,
      'rejected');
    assert.equal(receiptPresentation({ receipt: { accepted: true, code: 'unexpected' } }).tone,
      'unknown');
  });

test('shortens hashes without hiding that they are identifiers', () => {
    assert.equal(shorten('1234567890abcdefghijklmnop', 6, 4), '123456…mnop');
  });
