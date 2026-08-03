import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildPftlPrivateIntent,
  completePftlPrivateIntent,
  createPftlPrivateQuote,
  loadPftlPrivateRecoveries,
  parsePrivateNavcoinAmountAtoms,
  savePftlPrivateRecoveries,
  signPftlPrivateIntent,
} from './pftl-private-primary.js';

const WALLET = `pf${'11'.repeat(20)}`;
const quote = {
  schema: 'postfiat.pftl_swap.quote.v1',
  quote_id: '22'.repeat(48),
  chain_id: 'postfiat-wan-devnet-2',
  genesis_hash: '33'.repeat(48),
  protocol_version: 1,
  route_id: 'pftl-navcoin-primary-v1',
  direction: 'issue',
  output_mode: 'private',
  nav_amount_atoms: 1_000_000,
  input_amount_atoms: 1_005_000,
  output_amount_atoms: 1_000_000,
  maximum_fee_atoms: 100,
  pricing_nav_epoch: 8,
  policy_hash: '44'.repeat(48),
  expiry_height: 900,
};

test('builds the exact bounded intent and signs only inside WASM', () => {
  const intent = buildPftlPrivateIntent({
    quote,
    walletAddress: WALLET,
    controlledWalletId: WALLET,
    inputReference: 'transparent-pfusdc',
    idempotencyKey: 'navcoin-browser-issue-01',
  });
  let backupSeen = '';
  const signed = signPftlPrivateIntent({
    wasm: {
      wallet_sign_pftl_swap_intent(backup, json) {
        backupSeen = backup;
        return {
          schema: 'postfiat.pftl_swap.signed_intent.v1',
          intent: JSON.parse(json),
          algorithm_id: 'ML-DSA-65',
          public_key_hex: '55',
          signature_hex: '66',
        };
      },
    },
    backupJson: 'wallet-local-backup',
    intent,
  });
  assert.equal(backupSeen, 'wallet-local-backup');
  assert.equal(signed.intent.principal, WALLET);
  assert.equal(signed.intent.minimum_output_amount_atoms, quote.output_amount_atoms);
  assert.equal(JSON.stringify(signed).includes('wallet-local-backup'), false);
  assert.throws(() => buildPftlPrivateIntent({
    quote,
    walletAddress: `pf${'77'.repeat(20)}`,
    controlledWalletId: WALLET,
    inputReference: 'transparent-pfusdc',
    idempotencyKey: 'wrong-wallet',
  }), /not the controlled/);
});

test('authenticated quote request is exact and bounded', async () => {
  let request;
  const result = await createPftlPrivateQuote({
    direction: 'issue',
    navAmountAtoms: 1_000_000,
    outputMode: 'private',
    expectedRouteId: quote.route_id,
    proxyAuthToken: 'session-token',
    fetchImpl: async (path, options) => {
      request = { path, options };
      return { ok: true, status: 200, json: async () => ({ ok: true, quote }) };
    },
  });
  assert.equal(result.quote_id, quote.quote_id);
  assert.equal(request.path, '/api/pftl-private-swap/quotes');
  assert.equal(request.options.headers.Authorization, 'Bearer session-token');
  assert.deepEqual(JSON.parse(request.options.body), {
    direction: 'issue', nav_amount_atoms: 1_000_000, output_mode: 'private',
  });
});

test('pending execution polls, then same-intent replays committed output reference', async () => {
  const intent = buildPftlPrivateIntent({
    quote,
    walletAddress: WALLET,
    controlledWalletId: WALLET,
    inputReference: 'transparent-pfusdc',
    idempotencyKey: 'navcoin-browser-issue-recovery',
  });
  const signedIntent = {
    schema: 'postfiat.pftl_swap.signed_intent.v1', intent,
    algorithm_id: 'ML-DSA-65', public_key_hex: '55', signature_hex: '66',
  };
  const calls = [];
  const signals = [];
  const fetchImpl = async (path, options) => {
    calls.push(path);
    signals.push(options.signal);
    if (calls.length === 1) return response(202, { ok: true, swap: { state: 'PUBLISHED' } });
    if (calls.length === 2) return response(200, { ok: true, swap: { state: 'COMMITTED' } });
    return response(200, { ok: true, swap: { state: 'COMMITTED' }, output_note_refs: ['77'.repeat(32)] });
  };
  const controller = new AbortController();
  const completed = await completePftlPrivateIntent(signedIntent, {
    fetchImpl, pollIntervalMs: 1, timeoutMs: 1_000, signal: controller.signal,
  });
  assert.deepEqual(calls, [
    '/api/pftl-private-swap/jobs',
    '/api/pftl-private-swap/jobs/navcoin-browser-issue-recovery',
    '/api/pftl-private-swap/jobs',
  ]);
  assert.ok(signals.every(signal => signal === controller.signal));
  assert.equal(completed.output_note_refs.length, 1);
});

test('NAVCoin display amounts convert exactly to governed bounded atoms', () => {
  assert.equal(parsePrivateNavcoinAmountAtoms('1'), 1_000_000);
  assert.equal(parsePrivateNavcoinAmountAtoms('0.000001'), 1);
  assert.throws(() => parsePrivateNavcoinAmountAtoms('1.000001'), /Controlled/);
  assert.throws(() => parsePrivateNavcoinAmountAtoms('0.0000001'), /6 decimal/);
});

test('browser recovery persists only whitelisted public signed lineage', () => {
  const intent = buildPftlPrivateIntent({
    quote,
    walletAddress: WALLET,
    controlledWalletId: WALLET,
    inputReference: 'transparent-pfusdc',
    idempotencyKey: 'navcoin-browser-public-recovery',
  });
  const entries = new Map();
  const storage = {
    getItem: key => entries.get(key) || null,
    setItem: (key, value) => entries.set(key, value),
  };
  savePftlPrivateRecoveries(storage, WALLET, [{
    quote,
    signed_intent: {
      schema: 'postfiat.pftl_swap.signed_intent.v1', intent,
      algorithm_id: 'ML-DSA-65', public_key_hex: '55', signature_hex: '66',
    },
    response: {
      ok: true,
      swap: { state: 'COMMITTED', private_key_hex: 'must-not-persist' },
      output_note_refs: ['77'.repeat(32)],
      note_opening: 'must-not-persist',
    },
    status: 'COMMITTED',
    created_at_unix_ms: 1,
    backup_json: 'must-not-persist',
    private_key_hex: 'must-not-persist',
  }]);
  const serialized = [...entries.values()][0];
  assert.equal(serialized.includes('must-not-persist'), false);
  assert.equal(loadPftlPrivateRecoveries(storage, WALLET)[0].status, 'COMMITTED');
});

function response(status, payload) {
  return { ok: status >= 200 && status < 300, status, json: async () => payload };
}
