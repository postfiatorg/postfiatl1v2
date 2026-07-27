import assert from 'node:assert/strict';
import test from 'node:test';

import { evaluateA666PrimaryAcquisition } from './a666-primary-route.js';

const pins = {
  route_id: 'pftl-uniswap-a666-v2',
  route_config_digest: '11'.repeat(48),
  native_nav_asset_id: '22'.repeat(48),
  settlement_asset_id: '33'.repeat(48),
  handoff_controller: `0x${'44'.repeat(20)}`,
  wrapped_navcoin_token: `0x${'55'.repeat(20)}`,
  uniswap_pool_id: `0x${'66'.repeat(32)}`,
  proof_program_vkey: `0x${'77'.repeat(32)}`,
};

function status(overrides = {}) {
  return {
    schema: 'postfiat-pftl-uniswap-supply-status-v2',
    route_schema_version: 2,
    route_id: pins.route_id,
    route_config_digest: pins.route_config_digest,
    native_nav_asset_id: pins.native_nav_asset_id,
    settlement_asset_id: pins.settlement_asset_id,
    handoff_controller: pins.handoff_controller,
    wrapped_navcoin_token: pins.wrapped_navcoin_token,
    ethereum_chain_id: 1,
    live_value_enabled: true,
    paused: false,
    invariant_holds: true,
    outbound_verification_class: 'TRUSTLESS_FINALITY',
    return_verification_class: 'BFT_CHECKPOINT',
    policy_hash: '88'.repeat(48),
    policy_epoch: 2,
    issue_multiplier_bps: 10050,
    redeem_multiplier_bps: 9995,
    min_order_atoms: '1',
    max_order_atoms: '1000000000000',
    packet_notional_cap_atoms: '250000000000',
    available_issue_atoms: '1000000000000',
    available_export_capacity_atoms: '2000000000000',
    pricing_nav_epoch: '9',
    pricing_reserve_packet_hash: '99'.repeat(48),
    ...overrides,
  };
}

function quote(overrides = {}) {
  return {
    usdc_input_atoms: '1005000000000',
    max_usdc_input_atoms: '1005000000000',
    wa666_output_atoms: '1000000000000',
    min_wa666_output_atoms: '1000000000000',
    finalized_nav_atoms: '1000000',
    pricing_nav_epoch: '9',
    pricing_reserve_packet_hash: '99'.repeat(48),
    estimated_ethereum_gas_wei: '5000000000000000',
    estimated_completion_seconds: '1200',
    reservation_expires_at_height: '2000',
    export_deadline_unix_seconds: '1924992000',
    uniswap_pool_id: pins.uniswap_pool_id,
    proof_program_vkey: pins.proof_program_vkey,
    ...overrides,
  };
}

test('accepts a one-million a666 primary acquisition as four proof-bound packets', () => {
  const result = evaluateA666PrimaryAcquisition({
    supplyStatus: status(),
    quote: quote(),
    amountAtoms: '1000000000000',
    expected: pins,
  });
  assert.equal(result.ok, true, result.blockingReasons.join('\n'));
  assert.equal(result.packetCount, '4');
  assert.equal(result.preSignDisplay.action, 'Acquire a666 on Ethereum');
  assert.equal(result.preSignDisplay.outboundTrustClass, 'TRUSTLESS_FINALITY');
});

test('fails closed for paused, legacy, over-cap, stale, or unpinned routes', () => {
  for (const candidate of [
    { supplyStatus: status({ paused: true }) },
    { supplyStatus: status({ route_schema_version: 1 }) },
    { supplyStatus: status({ outbound_verification_class: 'OPTIMISTIC' }) },
    { supplyStatus: status({ available_issue_atoms: '999' }) },
    { quote: quote({ pricing_nav_epoch: '8' }) },
    { expected: { ...pins, wrapped_navcoin_token: `0x${'aa'.repeat(20)}` } },
  ]) {
    const result = evaluateA666PrimaryAcquisition({
      supplyStatus: candidate.supplyStatus || status(),
      quote: candidate.quote || quote(),
      amountAtoms: '1000000000000',
      expected: candidate.expected || pins,
    });
    assert.equal(result.ok, false);
  }
});

test('rejects an order requiring more than four export packets', () => {
  const result = evaluateA666PrimaryAcquisition({
    supplyStatus: status({
      max_order_atoms: '1250000000000',
      available_issue_atoms: '1250000000000',
    }),
    quote: quote(),
    amountAtoms: '1250000000000',
    expected: pins,
  });
  assert.equal(result.ok, false);
  assert.match(result.blockingReasons.join('\n'), /more than four/);
});
