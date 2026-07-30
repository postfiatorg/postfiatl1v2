import assert from 'node:assert/strict';
import test from 'node:test';

import {
  A666_NATIVE_ASSET_ID,
  A666_PRIMARY_ROUTE_ID,
  A666_ROUTE_CONFIG_DIGEST,
  A666_SETTLEMENT_ASSET_ID,
  buildA666IssueOperations,
  buildA666RedeemOperation,
  deriveA666IssueQuote,
  deriveA666RedeemQuote,
  evaluateA666PrimaryAcquisition,
  evaluateA666ResidentMarket,
  formatA666Nav,
  formatA666Units,
  parseA666Units,
} from './a666-primary-route.js';

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

function residentStatus(overrides = {}) {
  return status({
    route_id: A666_PRIMARY_ROUTE_ID,
    route_config_digest: A666_ROUTE_CONFIG_DIGEST,
    native_nav_asset_id: A666_NATIVE_ASSET_ID,
    settlement_asset_id: A666_SETTLEMENT_ASSET_ID,
    handoff_controller: '0x9a0262c0572fb4db08765408eb225e207f40c3d9',
    wrapped_navcoin_token: '0xee4c92edb03efdd9b519339edc19ad70c69a9be5',
    route_epoch: 5,
    policy_epoch: 5,
    policy_valid_from_height: 555,
    policy_expires_at_height: 10000,
    issue_capacity_remaining_atoms: '2000000000000',
    redeem_capacity_remaining_atoms: '2000000000000',
    supply_cap_remaining_atoms: '1900000000000',
    available_redeem_atoms: '99000000',
    ...overrides,
  });
}

function navStatus(overrides = {}) {
  return {
    schema: 'postfiat-vault-bridge-status-v1',
    asset_id: A666_NATIVE_ASSET_ID,
    valuation_unit: 'USD_1E8',
    nav_per_unit: '90115750',
    finalized_epoch: '9',
    finalized_reserve_packet_hash: '99'.repeat(48),
    ...overrides,
  };
}

test('parses, formats, and prices six-decimal A666 amounts without floating point', () => {
  assert.equal(parseA666Units('100.000001'), 100000001n);
  assert.equal(parseA666Units('1.0000001'), null);
  assert.equal(formatA666Units('100000001'), '100.000001');
  assert.equal(formatA666Nav('90115750'), '$0.9011575');

  assert.deepEqual(deriveA666IssueQuote('100000000', '90115750'), {
    amountAtoms: '100000000',
    baseReserveAtoms: '90115750',
    settlementAtoms: '90566329',
    spreadAtoms: '450579',
  });
  assert.deepEqual(deriveA666RedeemQuote('100000000', '90115750'), {
    amountAtoms: '100000000',
    baseReserveAtoms: '90115750',
    settlementAtoms: '90070692',
    spreadAtoms: '45058',
  });
});

test('resident market readiness binds live NAV and checks wallet balances', () => {
  const result = evaluateA666ResidentMarket({
    supplyStatus: residentStatus(),
    navStatus: navStatus(),
    chainStatus: { block_height: 556 },
    direction: 'issue',
    amountAtoms: '1000000',
    pfusdcBalanceAtoms: '1000000',
    a666BalanceAtoms: '0',
  });
  assert.equal(result.ok, true, result.blockingReasons.join('\n'));

  const stale = evaluateA666ResidentMarket({
    supplyStatus: residentStatus(),
    navStatus: navStatus({ finalized_epoch: '8' }),
    chainStatus: { block_height: 556 },
    direction: 'issue',
    amountAtoms: '1000000',
    pfusdcBalanceAtoms: '1000000',
  });
  assert.equal(stale.ok, false);
  assert.match(stale.blockingReasons.join('\n'), /NAV epoch/);
});

test('builds the exact resident issue and redeem operation sequence', () => {
  const wallet = `pf${'ab'.repeat(20)}`;
  const route = residentStatus();
  const issue = buildA666IssueOperations({
    walletAddress: wallet,
    ethereumRecipient: `0x${'cd'.repeat(20)}`,
    supplyStatus: route,
    chainHeight: 556,
    amountAtoms: '1000000',
    settlementAtoms: '906000',
    reservationId: '12'.repeat(48),
    subscriptionNonce: '34'.repeat(32),
  });
  assert.equal(issue.reserve.operation, 'pftl_uniswap_order_reserve');
  assert.equal(issue.reserve.expires_at_height, 656);
  assert.equal(issue.subscribe.operation, 'pftl_uniswap_primary_subscribe_v2');
  assert.equal(issue.release.operation, 'pftl_uniswap_order_release');

  const redeem = buildA666RedeemOperation({
    walletAddress: wallet,
    supplyStatus: route,
    chainHeight: 556,
    amountAtoms: '1000000',
    minimumSettlementAtoms: '900000',
    redemptionNonce: '56'.repeat(32),
  });
  assert.equal(redeem.operation, 'pftl_uniswap_primary_redeem');
  assert.equal(redeem.expires_at_height, 656);
  assert.equal(redeem.settlement_recipient, wallet);
});
