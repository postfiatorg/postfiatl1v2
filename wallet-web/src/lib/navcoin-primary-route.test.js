import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildNavcoinIssueExportDraft,
  buildNavcoinIssueOperations,
  buildNavcoinRedeemOperation,
  deriveNavcoinIssueQuote,
  deriveNavcoinRedeemQuote,
  evaluateNavcoinResidentMarket,
  finalizeNavcoinIssueExportOperations,
  formatNavcoinNav,
  formatNavcoinUnits,
  parseNavcoinUnits,
} from './navcoin-primary-route.js';

const NAV_ASSET_ID = '22'.repeat(48);
const SETTLEMENT_ASSET_ID = '33'.repeat(48);
const ROUTE_CONFIG_DIGEST = '11'.repeat(48);
const ROUTE_ID = 'pftl-qnav-ethereum-wqNAV-qusd-v1';

const pins = {
  route_id: ROUTE_ID,
  route_config_digest: ROUTE_CONFIG_DIGEST,
  native_nav_asset_id: NAV_ASSET_ID,
  settlement_asset_id: SETTLEMENT_ASSET_ID,
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
    route_trust_class: 'TRUSTLESS_FINALITY',
    policy_hash: '88'.repeat(48),
    policy_epoch: 2,
    issue_multiplier_bps: 10050,
    redeem_multiplier_bps: 9995,
    min_order_atoms: '1',
    max_order_atoms: '1000000000000',
    available_issue_atoms: '1000000000000',
    pricing_nav_epoch: '9',
    pricing_reserve_packet_hash: '99'.repeat(48),
    ...overrides,
  };
}

const market = {
  routeId: ROUTE_ID,
  routeConfigDigest: ROUTE_CONFIG_DIGEST,
  navAssetId: NAV_ASSET_ID,
  settlementAssetId: SETTLEMENT_ASSET_ID,
  handoffController: `0x${'44'.repeat(20)}`,
  wrappedToken: `0x${'55'.repeat(20)}`,
  ethereumChainId: 1,
  symbol: 'qNAV',
  settlementSymbol: 'qUSD',
  decimals: 6,
  settlementDecimals: 6,
  routeTrustClass: 'TRUSTLESS_FINALITY',
};

function residentStatus(overrides = {}) {
  return status({
    route_id: ROUTE_ID,
    route_config_digest: ROUTE_CONFIG_DIGEST,
    native_nav_asset_id: NAV_ASSET_ID,
    settlement_asset_id: SETTLEMENT_ASSET_ID,
    handoff_controller: market.handoffController,
    wrapped_navcoin_token: market.wrappedToken,
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
    asset_id: NAV_ASSET_ID,
    valuation_unit: 'USD_1E8',
    nav_per_unit: '90115750',
    finalized_epoch: '9',
    finalized_reserve_packet_hash: '99'.repeat(48),
    ...overrides,
  };
}

test('parses, formats, and prices registered-precision NAVCoin amounts without floating point', () => {
  assert.equal(parseNavcoinUnits('100.000001', 6), 100000001n);
  assert.equal(parseNavcoinUnits('1.0000001', 6), null);
  assert.equal(formatNavcoinUnits('100000001', 6), '100.000001');
  assert.equal(formatNavcoinNav('90115750'), '$0.9011575');

  assert.deepEqual(deriveNavcoinIssueQuote('100000000', '90115750', '10050', 6, 6), {
    amountAtoms: '100000000',
    baseReserveAtoms: '90115750',
    settlementAtoms: '90566329',
    spreadAtoms: '450579',
  });
  assert.deepEqual(deriveNavcoinRedeemQuote('100000000', '90115750', '9995', 6, 6), {
    amountAtoms: '100000000',
    baseReserveAtoms: '90115750',
    settlementAtoms: '90070692',
    spreadAtoms: '45058',
  });
});

test('prices different registered native and settlement precisions exactly', () => {
  assert.deepEqual(deriveNavcoinIssueQuote('100000000', '200000000', '10000', 8, 6), {
    amountAtoms: '100000000',
    baseReserveAtoms: '2000000',
    settlementAtoms: '2000000',
    spreadAtoms: '0',
  });
  assert.equal(parseNavcoinUnits('1.000000001', 8), null);
});

test('resident market readiness binds registered route, live NAV, and balances', () => {
  const result = evaluateNavcoinResidentMarket({
    market,
    supplyStatus: residentStatus(),
    navStatus: navStatus(),
    chainStatus: { block_height: 556 },
    direction: 'issue',
    amountAtoms: '1000000',
    settlementBalanceAtoms: '1000000',
    navcoinBalanceAtoms: '0',
  });
  assert.equal(result.ok, true, result.blockingReasons.join('\n'));

  const stale = evaluateNavcoinResidentMarket({
    market,
    supplyStatus: residentStatus(),
    navStatus: navStatus({ finalized_epoch: '8' }),
    chainStatus: { block_height: 556 },
    direction: 'issue',
    amountAtoms: '1000000',
    settlementBalanceAtoms: '1000000',
  });
  assert.equal(stale.ok, false);
  assert.match(stale.blockingReasons.join('\n'), /NAV epoch/);

  const missingRegistry = evaluateNavcoinResidentMarket({
    supplyStatus: residentStatus(),
    navStatus: navStatus(),
    chainStatus: { block_height: 556 },
    direction: 'issue',
    amountAtoms: '1000000',
    settlementBalanceAtoms: '1000000',
  });
  assert.equal(missingRegistry.ok, false);
  assert.match(missingRegistry.blockingReasons.join('\n'), /market metadata/);

  const substitutedRoute = evaluateNavcoinResidentMarket({
    market,
    supplyStatus: residentStatus({ route_id: 'pftl-other-ethereum-wrapper-settlement-v1' }),
    navStatus: navStatus(),
    chainStatus: { block_height: 556 },
    direction: 'issue',
    amountAtoms: '1000000',
    settlementBalanceAtoms: '1000000',
  });
  assert.equal(substitutedRoute.ok, false);
  assert.match(substitutedRoute.blockingReasons.join('\n'), /route id/);
});

test('builds the exact resident issue and redeem operation sequence', () => {
  const wallet = `pf${'ab'.repeat(20)}`;
  const route = residentStatus();
  const issue = buildNavcoinIssueOperations({
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

  const draft = buildNavcoinIssueExportDraft({
    walletAddress: wallet,
    ethereumRecipient: `0x${'cd'.repeat(20)}`,
    supplyStatus: route,
    chainHeight: 556,
    amountAtoms: '1000000',
    settlementAtoms: '906000',
    reservationId: '12'.repeat(48),
    subscriptionNonce: '34'.repeat(32),
    packetHash: '56'.repeat(48),
    exportNonce: '78'.repeat(32),
    destinationDeadlineSeconds: 1_924_992_000,
    refundDelayBlocks: 100,
  });
  const prepared = finalizeNavcoinIssueExportOperations(draft, {
    schema: 'postfiat-wallet-pftl-uniswap-mint-packet-v1',
    packet: { ...draft.mintPacket, policy_hash_commitment: '9a'.repeat(32) },
    packet_digest: 'bc'.repeat(32),
  });
  assert.equal(prepared.export.operation, 'pftl_uniswap_export_debit');
  assert.equal(prepared.export.reservation_id, '12'.repeat(48));
  assert.equal(prepared.export.ethereum_packet_digest, 'bc'.repeat(32));
  assert.equal(prepared.export.ethereum_packet_schema_version, 2);

  const redeem = buildNavcoinRedeemOperation({
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

  assert.throws(() => buildNavcoinIssueOperations({
    walletAddress: wallet,
    ethereumRecipient: `0x${'cd'.repeat(20)}`,
    supplyStatus: route,
    chainHeight: 556,
    amountAtoms: '9007199254740992',
    settlementAtoms: '906000',
    reservationId: '12'.repeat(48),
    subscriptionNonce: '34'.repeat(32),
  }), /browser signing integer limit/);
});
