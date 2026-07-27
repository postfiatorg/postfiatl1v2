import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  ETH_SEPOLIA_CHAIN_ID,
  ETH_SEPOLIA_USDC,
  ETH_FAST_LANE_ROUTE_ID,
  ETH_FAST_LANE_STAGES,
  LIFECYCLE_RELEASE_REQUIRED,
  assertEthFastLaneRoute,
  encodeApprove,
  encodeBalanceOf,
  formatAtoms,
  parseUsdcAtoms,
  initialLaneState,
  laneTransition,
  checkConservation,
  isValidProofRef,
} from './eth-fast-lane.js';

const VAULT = '0x' + 'ab'.repeat(20);
const RECIPIENT = '0x' + 'cd'.repeat(20);
const BURN_SOURCE = '0x' + 'ef'.repeat(20);

function baseRoute() {
  return {
    route_id: ETH_FAST_LANE_ROUTE_ID,
    source_chain_id: ETH_SEPOLIA_CHAIN_ID,
    token_address: ETH_SEPOLIA_USDC,
    vault_address: VAULT,
  };
}

test('route config rejects Arbitrum markers and wrong chain', () => {
  assert.doesNotThrow(() => assertEthFastLaneRoute(baseRoute()));
  assert.throws(() => assertEthFastLaneRoute({ ...baseRoute(), route_id: 'arbitrum-sepolia-usdc-v1' }), /Arbitrum/);
  assert.throws(() => assertEthFastLaneRoute({ ...baseRoute(), source_chain_id: 421614 }), /11155111/);
  assert.throws(() => assertEthFastLaneRoute({ ...baseRoute(), note: 'arbitrum fallback' }), /Arbitrum/);
  assert.throws(() => assertEthFastLaneRoute({ ...baseRoute(), token_address: VAULT }), /canonical/);
  const omitted = baseRoute();
  delete omitted.token_address;
  assert.throws(() => assertEthFastLaneRoute(omitted), /canonical/);
});

test('approve/balance calldata encoding is exact', () => {
  const approve = encodeApprove(VAULT, 1000000n);
  assert.ok(approve.startsWith('0x095ea7b3'));
  assert.ok(approve.includes('ab'.repeat(20)));
  assert.ok(approve.endsWith('f4240'.padStart(64, '0')));
  const balance = encodeBalanceOf(RECIPIENT);
  assert.ok(balance.startsWith('0x70a08231'));
  assert.throws(() => encodeApprove('0x1234', 1n), /address/);
  assert.throws(() => encodeApprove(VAULT, -1n), /nonnegative/);
});

test('exact atom display round-trips without float drift', () => {
  assert.equal(formatAtoms(1000000n), '1.000000');
  assert.equal(formatAtoms(1n), '0.000001');
  assert.equal(formatAtoms(123456789n), '123.456789');
  assert.equal(formatAtoms(357559n), '0.357559');
  assert.equal(parseUsdcAtoms('1.000000'), 1000000n);
  assert.equal(parseUsdcAtoms('0.357559'), 357559n);
  assert.equal(parseUsdcAtoms(formatAtoms(987654321n)), 987654321n);
  assert.throws(() => parseUsdcAtoms('0.0000001'), /<=6/);
  assert.throws(() => parseUsdcAtoms('abc'), /decimal/);
});

test('six-stage machine enforces order, spendable credit, provenance, different recipient', () => {
  assert.equal(LIFECYCLE_RELEASE_REQUIRED, false);
  let state = initialLaneState();
  assert.equal(state.credit_state, null);
  assert.equal(state.lifecycle_release, 'not_applicable');

  assert.throws(() => laneTransition(state, { stage: 'credit', credited_atoms: 100 }), /out of order/);

  state = laneTransition(state, { stage: 'deposit', balances_atoms: { vault: 1000000 } });
  state = laneTransition(state, { stage: 'credit', credited_atoms: 1000000 });
  assert.equal(state.credit_state, 'spendable');

  assert.throws(
    () => laneTransition(state, { stage: 'transparent_send', debit_provenance: 'inventory' }),
    /newly credited ingress atoms/,
  );
  state = laneTransition(state, {
    stage: 'transparent_send',
    debit_provenance: 'ingress_credit',
    balances_atoms: { a: 0, b: 400000 },
  });
  state = laneTransition(state, {
    stage: 'orchard_send',
    debit_provenance: 'ingress_credit',
    balances_atoms: { a: 0, c: 200000 },
  });
  state = laneTransition(state, { stage: 'burn', balances_atoms: { c: 0 } });
  assert.throws(
    () => laneTransition(state, { stage: 'withdrawal', recipient: BURN_SOURCE, burn_source: BURN_SOURCE }),
    /must differ/,
  );
  state = laneTransition(state, { stage: 'withdrawal', recipient: RECIPIENT, burn_source: BURN_SOURCE });
  assert.equal(state.stage, 'done');
  assert.deepEqual(state.completed, ETH_FAST_LANE_STAGES);
  assert.equal(state.provenance.length, 2);
});

test('stage events carrying Arbitrum markers fail closed', () => {
  let state = initialLaneState();
  assert.throws(() => laneTransition(state, { stage: 'deposit', via: 'arbitrum bridge' }), /Arbitrum/);
});

test('conservation identity V = S + D + B - R over exact atoms', () => {
  const ok = checkConservation({
    vault_atoms: 1000000n,
    supply_atoms: 1000010n,
    deposit_atoms: 0n,
    burn_atoms: 400000n,
    redeemed_atoms: 400010n,
  });
  assert.equal(ok.ok, true);
  const bad = checkConservation({
    vault_atoms: 999999n,
    supply_atoms: 1000010n,
    deposit_atoms: 0n,
    burn_atoms: 400000n,
    redeemed_atoms: 400010n,
  });
  assert.equal(bad.ok, false);
});

test('proof reference shape is enforced', () => {
  assert.equal(isValidProofRef('0x' + 'ab'.repeat(32)), true);
  assert.equal(isValidProofRef('ab'.repeat(32)), true);
  assert.equal(isValidProofRef('0x1234'), false);
  assert.equal(isValidProofRef(null), false);
});
