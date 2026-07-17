import assert from 'node:assert/strict';
import test from 'node:test';

import {
  evaluatePftlUniswapBetaRoute,
  evaluatePftlUniswapOptimisticRoute,
  PFTL_UNISWAP_BETA_ROUTE,
} from './pftl-uniswap-route.js';

const legacyPftlA651 = 'dcddbf56e7e15f7893d0038e8e0e6089d5a41418dead75353aabb8c016cf626beeb93bc802929f29883c078d910f59d5';
const legacyEthereumA651 = '0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e';
const legacyPoolId = '0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84';

function betaCapability(overrides = {}) {
  return {
    label: 'PFTL-Uniswap controlled beta',
    enabled: true,
    can_quote: true,
    can_run: true,
    status: 'beta_ready',
    route_trust_class: 'CONTROLLED',
    release_stage: 'explicit_beta',
    public_routing_enabled: false,
    paused: false,
    route_supply_cap_atoms: '10000000',
    supply_cap_remaining_atoms: '9999958',
    packet_notional_cap_atoms: '200000000',
    native_nav_asset_id: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    wrapped_navcoin_token: '0xd969897adeb947a22e9621db2db186e6ea11140f',
    uniswap_pool_id: '0x5c7ea7b5e0091029297604a5908e13ee671b937917c96bc62e940796a269443d',
    fallback_route: null,
    legacy_pool_fallback: false,
    ...overrides,
  };
}

function optimisticCapability(overrides = {}) {
  return {
    label: 'PFTL-Uniswap optimistic public beta',
    enabled: true,
    can_quote: true,
    can_run: true,
    status: 'optimistic_ready',
    route_trust_class: 'OPTIMISTIC',
    release_stage: 'optimistic_public_beta',
    optimistic_public_beta: true,
    public_routing_enabled: true,
    paused: false,
    route_supply_cap_atoms: '10000000',
    supply_cap_remaining_atoms: '9999958',
    packet_notional_cap_atoms: '200000000',
    poster_bond_wei: '1000000000000000000',
    challenger_bond_wei: '1000000000000000000',
    challenge_gas_cost_with_margin_wei: '414367324620564',
    challenge_window_seconds: '1668',
    challenge_resolution_window_seconds: '900',
    watcher_liveness_slo: 'detect_posted_claims_within_60s_classify_within_300s',
    canonical_nav_price: '6.996659',
    uniswap_market_price: '7.041102',
    proof_freshness: 'fresh:42s',
    bridge_verifier_mode: 'OPTIMISTIC',
    challenge_resolution_mode: 'owner_arbitrated',
    packet_status: 'pending_challenge_window',
    refund_deadline_unix_seconds: '1782936000',
    route_trust_label: 'OPTIMISTIC owner-arbitrated public beta',
    optimistic_launch_binding_digest:
      'b21f3cffd0a2b981e39e1b19883d31689fb0c77c135fa593bf20bbb501e2732bacf9393e307688dc5c9aa2daa4854980',
    fail_closed_conditions: [
      'pause if watcher liveness fails',
      'pause if verifier parameters differ from binding digest',
    ],
    native_nav_asset_id: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    wrapped_navcoin_token: '0xd969897adeb947a22e9621db2db186e6ea11140f',
    uniswap_pool_id: '0x5c7ea7b5e0091029297604a5908e13ee671b937917c96bc62e940796a269443d',
    fallback_route: null,
    legacy_pool_fallback: false,
    ...overrides,
  };
}

function assertNoTrustlessDisplay(value) {
  assert.doesNotMatch(JSON.stringify(value), /trustless/i);
}

test('evaluatePftlUniswapBetaRoute accepts controlled capped explicit beta route', () => {
  const result = evaluatePftlUniswapBetaRoute({
    routeCapability: betaCapability(),
    amountAtoms: '1000000',
  });

  assert.equal(result.ok, true);
  assert.equal(result.route, PFTL_UNISWAP_BETA_ROUTE);
  assert.equal(result.status, 'controlled_beta_ready');
  assert.equal(result.trustClass, 'CONTROLLED');
  assert.equal(result.routeSupplyCapAtoms, '10000000');
  assert.equal(result.packetNotionalCapAtoms, '200000000');
  assert.equal(result.walletCopy.label, 'CONTROLLED beta');
  assertNoTrustlessDisplay(result.walletCopy);
});

test('evaluatePftlUniswapBetaRoute accepts native a651 with bridge-aware wrapped token and pool', () => {
  const result = evaluatePftlUniswapBetaRoute({
    routeCapability: betaCapability({ native_nav_asset_id: legacyPftlA651 }),
    amountAtoms: '1000000',
  });

  assert.equal(result.ok, true);
  assert.equal(result.route, PFTL_UNISWAP_BETA_ROUTE);
  assert.equal(result.trustClass, 'CONTROLLED');
});

test('evaluatePftlUniswapBetaRoute rejects missing explicit beta state', () => {
  const result = evaluatePftlUniswapBetaRoute({
    routeCapability: betaCapability({ release_stage: 'public', explicit_beta: false }),
  });

  assert.equal(result.ok, false);
  assert.match(result.blockingReasons.join('\n'), /explicit\/internal beta/);
});

test('evaluatePftlUniswapBetaRoute rejects non-controlled trust labels', () => {
  const result = evaluatePftlUniswapBetaRoute({
    routeCapability: betaCapability({ route_trust_class: 'TRUSTLESS_FINALITY' }),
  });

  assert.equal(result.ok, false);
  assert.match(result.blockingReasons.join('\n'), /CONTROLLED/);
});

test('evaluatePftlUniswapBetaRoute rejects public routing labels for beta', () => {
  const result = evaluatePftlUniswapBetaRoute({
    routeCapability: betaCapability({ public_routing_enabled: true }),
  });

  assert.equal(result.ok, false);
  assert.match(result.blockingReasons.join('\n'), /public/);
});

test('evaluatePftlUniswapBetaRoute rejects missing caps', () => {
  const result = evaluatePftlUniswapBetaRoute({
    routeCapability: betaCapability({
      route_supply_cap_atoms: '',
      supply_cap_remaining_atoms: null,
      packet_notional_cap_atoms: undefined,
    }),
  });

  assert.equal(result.ok, false);
  assert.match(result.blockingReasons.join('\n'), /route supply cap/);
  assert.match(result.blockingReasons.join('\n'), /remaining route cap/);
  assert.match(result.blockingReasons.join('\n'), /packet notional cap/);
});

test('evaluatePftlUniswapBetaRoute rejects paused routes', () => {
  const result = evaluatePftlUniswapBetaRoute({
    routeCapability: betaCapability({ paused: true }),
  });

  assert.equal(result.ok, false);
  assert.match(result.blockingReasons.join('\n'), /paused/);
});

test('evaluatePftlUniswapBetaRoute rejects packet cap excess', () => {
  const result = evaluatePftlUniswapBetaRoute({
    routeCapability: betaCapability({ packet_notional_cap_atoms: '1000' }),
    amountAtoms: '1001',
  });

  assert.equal(result.ok, false);
  assert.match(result.blockingReasons.join('\n'), /packet cap/);
});

test('evaluatePftlUniswapBetaRoute rejects route cap excess', () => {
  const result = evaluatePftlUniswapBetaRoute({
    routeCapability: betaCapability({ supply_cap_remaining_atoms: '1000' }),
    amountAtoms: '1001',
  });

  assert.equal(result.ok, false);
  assert.match(result.blockingReasons.join('\n'), /remaining route cap/);
});

test('evaluatePftlUniswapBetaRoute rejects legacy fallback route and flags', () => {
  const result = evaluatePftlUniswapBetaRoute({
    routeCapability: betaCapability({
      fallback_route: 'legacy_a651_uniswap',
      legacy_pool_fallback: true,
    }),
  });

  assert.equal(result.ok, false);
  assert.match(result.blockingReasons.join('\n'), /legacy pool fallback/);
  assert.match(result.blockingReasons.join('\n'), /legacy a651 fallback route/);
});

test('evaluatePftlUniswapBetaRoute rejects legacy Ethereum a651 token and pool fields', () => {
  for (const [field, value] of [
    ['wrapped_navcoin_token', legacyEthereumA651],
    ['uniswap_pool_id', legacyPoolId],
  ]) {
    const result = evaluatePftlUniswapBetaRoute({
      routeCapability: betaCapability({ [field]: value }),
    });

    assert.equal(result.ok, false, field);
    assert.match(result.blockingReasons.join('\n'), new RegExp(`${field} selects legacy a651`));
  }
});

test('evaluatePftlUniswapOptimisticRoute accepts capped public beta with visible challenge terms', () => {
  const result = evaluatePftlUniswapOptimisticRoute({
    routeCapability: optimisticCapability(),
    amountAtoms: '1000000',
  });

  assert.equal(result.ok, true);
  assert.equal(result.route, PFTL_UNISWAP_BETA_ROUTE);
  assert.equal(result.status, 'optimistic_public_beta_ready');
  assert.equal(result.trustClass, 'OPTIMISTIC');
  assert.equal(result.publicRouting, true);
  assert.equal(result.packetNotionalCapAtoms, '200000000');
  assert.equal(result.posterBondWei, '1000000000000000000');
  assert.equal(result.challengerBondWei, '1000000000000000000');
  assert.equal(result.challengeWindowSeconds, '1668');
  assert.equal(result.challengeResolutionWindowSeconds, '900');
  assert.equal(
    result.optimisticLaunchBindingDigest,
    'b21f3cffd0a2b981e39e1b19883d31689fb0c77c135fa593bf20bbb501e2732bacf9393e307688dc5c9aa2daa4854980',
  );
  assert.equal(result.challengeResolutionMode, 'owner_arbitrated');
  assert.equal(result.walletCopy.label, 'OPTIMISTIC owner-arbitrated public beta');
  assertNoTrustlessDisplay(result.walletCopy);
  assert.ok(result.walletCopy.requiredBeforeSigning.includes('challenge window'));
  assert.ok(result.walletCopy.requiredBeforeSigning.includes('challenge resolution mode'));
  assert.ok(result.walletCopy.requiredBeforeSigning.includes('fail-closed conditions'));
  assert.deepEqual(
    {
      canonicalNav: result.preSignDisplay.canonicalNav,
      uniswapMarketPrice: result.preSignDisplay.uniswapMarketPrice,
      proofFreshness: result.preSignDisplay.proofFreshness,
      bridgeVerifierMode: result.preSignDisplay.bridgeVerifierMode,
      challengeResolutionMode: result.preSignDisplay.challengeResolutionMode,
      packetStatus: result.preSignDisplay.packetStatus,
      refundDeadlineUnixSeconds: result.preSignDisplay.refundDeadlineUnixSeconds,
      routeTrustLabel: result.preSignDisplay.routeTrustLabel,
    },
    {
      canonicalNav: '6.996659',
      uniswapMarketPrice: '7.041102',
      proofFreshness: 'fresh:42s',
      bridgeVerifierMode: 'OPTIMISTIC',
      challengeResolutionMode: 'owner_arbitrated',
      packetStatus: 'pending_challenge_window',
      refundDeadlineUnixSeconds: '1782936000',
      routeTrustLabel: 'OPTIMISTIC owner-arbitrated public beta',
    },
  );
  assert.equal(result.preSignDisplay.routeCapAtoms, '10000000');
  assert.equal(result.preSignDisplay.packetNotionalCapAtoms, '200000000');
  assert.equal(result.preSignDisplay.posterBondWei, '1000000000000000000');
  assert.equal(result.preSignDisplay.challengerBondWei, '1000000000000000000');
  assert.equal(result.preSignDisplay.challengeWindowSeconds, '1668');
  assert.equal(result.preSignDisplay.watcherLivenessSlo, 'detect_posted_claims_within_60s_classify_within_300s');
  assert.equal(result.preSignDisplay.failClosedConditions.length, 2);
});

test('evaluatePftlUniswapOptimisticRoute rejects trustless copy on optimistic route', () => {
  const routeClaimResult = evaluatePftlUniswapOptimisticRoute({
    routeCapability: optimisticCapability({ route_claim: 'TRUSTLESS_FINALITY' }),
  });

  assert.equal(routeClaimResult.ok, false);
  assert.match(routeClaimResult.blockingReasons.join('\n'), /must not claim trustless finality/);

  const visibleLabelResult = evaluatePftlUniswapOptimisticRoute({
    routeCapability: optimisticCapability({
      route_trust_label: 'TRUSTLESS_FINALITY',
      bridge_verifier_mode: 'TRUSTLESS_FINALITY',
    }),
  });

  assert.equal(visibleLabelResult.ok, false);
  assert.match(visibleLabelResult.blockingReasons.join('\n'), /route trust label must disclose OPTIMISTIC/);
  assert.match(visibleLabelResult.blockingReasons.join('\n'), /bridge verifier mode must match OPTIMISTIC/);

  const arbitratedLabelResult = evaluatePftlUniswapOptimisticRoute({
    routeCapability: optimisticCapability({
      route_trust_label: 'OPTIMISTIC public beta',
    }),
  });

  assert.equal(arbitratedLabelResult.ok, false);
  assert.match(arbitratedLabelResult.blockingReasons.join('\n'), /must disclose ARBITRATED/);
});

test('evaluatePftlUniswapOptimisticRoute rejects missing challenge terms and watcher SLO', () => {
  const result = evaluatePftlUniswapOptimisticRoute({
    routeCapability: optimisticCapability({
      poster_bond_wei: '',
      challenger_bond_wei: null,
      challenge_gas_cost_with_margin_wei: undefined,
      challenge_window_seconds: '',
      challenge_resolution_window_seconds: '',
      challenge_resolution_mode: '',
      watcher_liveness_slo: '',
      optimistic_launch_binding_digest: '',
      fail_closed_conditions: [],
    }),
  });

  const reasons = result.blockingReasons.join('\n');
  assert.equal(result.ok, false);
  assert.match(reasons, /poster bond/);
  assert.match(reasons, /challenger bond/);
  assert.match(reasons, /challenge gas margin/);
  assert.match(reasons, /challenge window/);
  assert.match(reasons, /challenge resolution window/);
  assert.match(reasons, /challenge resolution mode/);
  assert.match(reasons, /watcher liveness SLO/);
  assert.match(reasons, /binding digest/);
  assert.match(reasons, /fail-closed conditions/);
});

test('evaluatePftlUniswapOptimisticRoute rejects missing Gate 6 pre-sign display fields', () => {
  const result = evaluatePftlUniswapOptimisticRoute({
    routeCapability: optimisticCapability({
      canonical_nav_price: '',
      uniswap_market_price: '',
      proof_freshness: '',
      bridge_verifier_mode: '',
      challenge_resolution_mode: '',
      packet_status: '',
      refund_deadline_unix_seconds: '',
      route_trust_label: '',
    }),
  });

  const reasons = result.blockingReasons.join('\n');
  assert.equal(result.ok, false);
  assert.match(reasons, /pre-sign display missing canonicalNav/);
  assert.match(reasons, /pre-sign display missing uniswapMarketPrice/);
  assert.match(reasons, /pre-sign display missing proofFreshness/);
  assert.match(reasons, /pre-sign display missing packetStatus/);
  assert.match(reasons, /pre-sign display missing challengeResolutionMode/);
  assert.match(reasons, /pre-sign display missing refundDeadlineUnixSeconds/);
  assert.match(reasons, /pre-sign display missing routeTrustLabel/);
});

test('evaluatePftlUniswapOptimisticRoute rejects uncapped or over-cap public beta', () => {
  const missingCaps = evaluatePftlUniswapOptimisticRoute({
    routeCapability: optimisticCapability({
      route_supply_cap_atoms: '',
      supply_cap_remaining_atoms: null,
      packet_notional_cap_atoms: undefined,
    }),
  });
  assert.equal(missingCaps.ok, false);
  assert.match(missingCaps.blockingReasons.join('\n'), /route supply cap/);
  assert.match(missingCaps.blockingReasons.join('\n'), /remaining route cap/);
  assert.match(missingCaps.blockingReasons.join('\n'), /packet notional cap/);

  const overPacketCap = evaluatePftlUniswapOptimisticRoute({
    routeCapability: optimisticCapability({ packet_notional_cap_atoms: '1000' }),
    amountAtoms: '1001',
  });
  assert.equal(overPacketCap.ok, false);
  assert.match(overPacketCap.blockingReasons.join('\n'), /packet cap/);
});

test('evaluatePftlUniswapOptimisticRoute rejects disabled public routing and legacy fallback', () => {
  const result = evaluatePftlUniswapOptimisticRoute({
    routeCapability: optimisticCapability({
      public_routing_enabled: false,
      fallback_route: 'legacy_a651_uniswap',
      legacy_pool_fallback: true,
    }),
  });

  assert.equal(result.ok, false);
  assert.match(result.blockingReasons.join('\n'), /public routing/);
  assert.match(result.blockingReasons.join('\n'), /legacy pool fallback/);
  assert.match(result.blockingReasons.join('\n'), /legacy a651 fallback route/);
});
