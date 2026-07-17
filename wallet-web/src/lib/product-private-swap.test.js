import assert from 'node:assert/strict';
import test from 'node:test';

import {
  executeProductPrivateSwap,
  normalizeProductPrivateSwapResult,
  productPrivateSwapRunId,
  PRODUCT_PRIVATE_SWAP_STEPS,
} from './product-private-swap.js';


test('product private swap creates a bounded fresh run id', () => {
  const runId = productPrivateSwapRunId(`pf${'a'.repeat(40)}`, 123456, 0.5);
  assert.match(runId, /^ux-[a-z0-9]+-[0-9a-f]{8}$/);
  assert.ok(runId.length <= 64);
});


test('product private swap calls the certified backend with no private material', async () => {
  const calls = [];
  const response = {
    ok: true,
    complete: true,
    run_dir: '/runs/ux-test',
    wallet_ux: { run_id: 'ux-test' },
    steps: Object.fromEntries(PRODUCT_PRIVATE_SWAP_STEPS.map((name, index) => [name, {
      state: 'verified', artifact_hash: `hash-${index}`,
      expected_height: name === 'final_verify' ? 42 : null,
      expected_state_root: name === 'final_verify' ? 'root-42' : null,
    }])),
  };
  const swapServer = {
    async runPrivateSwapWorkflow(body) {
      calls.push(body);
      return response;
    },
  };

  const result = await executeProductPrivateSwap({
    swapServer,
    walletAddress: `pf${'b'.repeat(40)}`,
    runId: 'ux-test',
  });

  assert.deepEqual(calls[0], {
    action: 'execute', fresh_wallet: true,
    initiating_wallet_address: `pf${'b'.repeat(40)}`,
    no_money: true, run_id: 'ux-test',
  });
  assert.equal(result.complete, true);
  assert.equal(result.finalHeight, 42);
  assert.equal(result.steps.every(step => step.state === 'verified'), true);
  assert.equal('backupJson' in calls[0], false);
  assert.equal('private_key' in calls[0], false);
});


test('product private swap normalization fails closed on absent steps', () => {
  const result = normalizeProductPrivateSwapResult({ ok: false });
  assert.equal(result.ok, false);
  assert.equal(result.steps.length, PRODUCT_PRIVATE_SWAP_STEPS.length);
  assert.equal(result.steps.every(step => step.state === 'not_started'), true);
});
