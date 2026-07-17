import {
  LEGACY_A651_ETHEREUM_TOKEN,
  LEGACY_A651_UNISWAP_POOL_ID,
  LEGACY_A651_UNISWAP_ROUTE,
  PFTL_UNISWAP_BETA_ROUTE,
} from './navswap-actions.js';

export { PFTL_UNISWAP_BETA_ROUTE };

export const PFTL_UNISWAP_BETA_TRUST_CLASS = 'CONTROLLED';
export const PFTL_UNISWAP_OPTIMISTIC_TRUST_CLASS = 'OPTIMISTIC';

const BETA_STAGES = new Set(['explicit_beta', 'internal_beta', 'controlled_beta']);
const OPTIMISTIC_PUBLIC_BETA_STAGES = new Set([
  'public_beta',
  'optimistic_public_beta',
  'capped_public_beta',
]);
const REQUIRED_OPTIMISTIC_DISPLAY_FIELDS = [
  'canonicalNav',
  'uniswapMarketPrice',
  'proofFreshness',
  'bridgeVerifierMode',
  'challengeResolutionMode',
  'packetStatus',
  'refundDeadlineUnixSeconds',
  'routeTrustLabel',
];
const OBJECTIVE_CHALLENGE_RESOLUTION_MODES = new Set([
  'onchain_objective',
  'direct_or_succinct_pftl_finality',
]);
const ARBITRATED_CHALLENGE_RESOLUTION_MODES = new Set([
  'owner_arbitrated',
  'governance_arbitrated',
]);

export function evaluatePftlUniswapBetaRoute({
  routeCapability = null,
  amountAtoms = null,
} = {}) {
  const capability = routeCapability && typeof routeCapability === 'object' ? routeCapability : {};
  const errors = [];
  const trustClass = stringField(capability, ['route_trust_class', 'trust_class', 'trustClass']).toUpperCase();
  const releaseStage = stringField(capability, ['release_stage', 'route_stage', 'route_mode', 'mode']).toLowerCase();
  const explicitBeta = booleanField(capability, ['explicit_beta', 'beta_enabled', 'internal_beta']) === true
    || BETA_STAGES.has(releaseStage);
  const publicRouting = booleanField(capability, ['public_routing_enabled', 'public_routing', 'live_public_routing']);
  const paused = booleanField(capability, ['paused', 'route_paused']) === true
    || ['paused', 'disabled_paused'].includes(stringField(capability, ['status', 'route_status']).toLowerCase());

  const routeSupplyCapAtoms = positiveBigIntField(capability, [
    'route_supply_cap_atoms',
    'route_cap_atoms',
    'supply_cap_atoms',
  ]);
  const capRemainingAtoms = nonNegativeBigIntField(capability, [
    'supply_cap_remaining_atoms',
    'route_cap_remaining_atoms',
    'cap_remaining_atoms',
  ]);
  const packetNotionalCapAtoms = positiveBigIntField(capability, [
    'packet_notional_cap_atoms',
    'per_packet_cap_atoms',
    'max_packet_notional_atoms',
  ]);
  const requestedAtoms = optionalBigInt(amountAtoms);

  if (capability.enabled !== true) errors.push('route is not enabled for explicit beta use');
  if (capability.can_quote !== true) errors.push('route cannot quote');
  if (capability.can_run !== true) errors.push('route cannot run');
  if (!explicitBeta) errors.push('route is not marked explicit/internal beta');
  if (trustClass !== PFTL_UNISWAP_BETA_TRUST_CLASS) errors.push('route trust class must be CONTROLLED');
  if (publicRouting === true) errors.push('route must not be marked public');
  if (paused) errors.push('route is paused');
  if (routeSupplyCapAtoms === null) errors.push('route supply cap is missing');
  if (capRemainingAtoms === null) errors.push('remaining route cap is missing');
  if (packetNotionalCapAtoms === null) errors.push('packet notional cap is missing');
  if (capRemainingAtoms === 0n) errors.push('remaining route cap is zero');
  if (requestedAtoms !== null && packetNotionalCapAtoms !== null && requestedAtoms > packetNotionalCapAtoms) {
    errors.push('requested amount exceeds packet cap');
  }
  if (requestedAtoms !== null && capRemainingAtoms !== null && requestedAtoms > capRemainingAtoms) {
    errors.push('requested amount exceeds remaining route cap');
  }

  const legacyField = firstLegacyField(capability);
  if (legacyField) errors.push(`${legacyField} selects legacy a651`);
  if (booleanField(capability, ['legacy_pool_enabled', 'legacy_pool_fallback', 'uses_legacy_pool']) === true) {
    errors.push('legacy pool fallback is enabled');
  }
  if (stringField(capability, ['fallback_route', 'fallbackRoute']) === LEGACY_A651_UNISWAP_ROUTE) {
    errors.push('legacy a651 fallback route is configured');
  }

  return {
    ok: errors.length === 0,
    route: PFTL_UNISWAP_BETA_ROUTE,
    status: errors.length === 0 ? 'controlled_beta_ready' : 'controlled_beta_blocked',
    message: errors[0] || 'Controlled beta route accepted: capped, unpaused, and legacy fallback disabled',
    blockingReasons: errors,
    trustClass,
    explicitBeta,
    publicRouting: publicRouting === true,
    paused,
    routeSupplyCapAtoms: routeSupplyCapAtoms === null ? null : routeSupplyCapAtoms.toString(),
    capRemainingAtoms: capRemainingAtoms === null ? null : capRemainingAtoms.toString(),
    packetNotionalCapAtoms: packetNotionalCapAtoms === null ? null : packetNotionalCapAtoms.toString(),
    walletCopy: {
      label: 'CONTROLLED beta',
      route: 'pfUSDC -> primary mint -> bridge -> wrapped venue token',
      warning: 'Operator-controlled beta route. Public routing is disabled.',
    },
  };
}

export function evaluatePftlUniswapOptimisticRoute({
  routeCapability = null,
  amountAtoms = null,
} = {}) {
  const capability = routeCapability && typeof routeCapability === 'object' ? routeCapability : {};
  const errors = [];
  const trustClass = stringField(capability, ['route_trust_class', 'trust_class', 'trustClass']).toUpperCase();
  const releaseStage = stringField(capability, ['release_stage', 'route_stage', 'route_mode', 'mode']).toLowerCase();
  const publicBeta = booleanField(capability, ['optimistic_public_beta', 'public_beta_enabled']) === true
    || OPTIMISTIC_PUBLIC_BETA_STAGES.has(releaseStage);
  const publicRouting = booleanField(capability, ['public_routing_enabled', 'public_routing', 'live_public_routing']);
  const paused = booleanField(capability, ['paused', 'route_paused']) === true
    || ['paused', 'disabled_paused'].includes(stringField(capability, ['status', 'route_status']).toLowerCase());

  const routeSupplyCapAtoms = positiveBigIntField(capability, [
    'route_supply_cap_atoms',
    'route_cap_atoms',
    'supply_cap_atoms',
  ]);
  const capRemainingAtoms = nonNegativeBigIntField(capability, [
    'supply_cap_remaining_atoms',
    'route_cap_remaining_atoms',
    'cap_remaining_atoms',
  ]);
  const packetNotionalCapAtoms = positiveBigIntField(capability, [
    'packet_notional_cap_atoms',
    'per_packet_cap_atoms',
    'max_packet_notional_atoms',
  ]);
  const posterBondWei = positiveBigIntField(capability, ['poster_bond_wei', 'claim_bond_wei']);
  const challengerBondWei = positiveBigIntField(capability, ['challenger_bond_wei', 'challenge_bond_wei']);
  const challengeGasMarginWei = positiveBigIntField(capability, ['challenge_gas_cost_with_margin_wei']);
  const challengeWindowSeconds = positiveBigIntField(capability, ['challenge_window_seconds']);
  const challengeResolutionWindowSeconds = positiveBigIntField(capability, [
    'challenge_resolution_window_seconds',
    'resolution_window_seconds',
  ]);
  const challengeResolutionMode = normalizedResolutionMode(capability);
  const watcherLivenessSlo = stringField(capability, ['watcher_liveness_slo', 'watcher_slo']);
  const bindingDigest = stringField(capability, [
    'optimistic_launch_binding_digest',
    'launch_binding_digest',
    'binding_digest',
  ]).toLowerCase();
  const failClosedConditions = arrayField(capability, ['fail_closed_conditions', 'fail_closed', 'fail_closed_requirements']);
  const requestedAtoms = optionalBigInt(amountAtoms);
  const routeClaim = stringField(capability, ['route_claim', 'claim', 'wallet_claim']).toUpperCase();
  const preSignDisplay = optimisticPreSignDisplay({
    capability,
    trustClass,
    routeSupplyCapAtoms,
    capRemainingAtoms,
    packetNotionalCapAtoms,
    posterBondWei,
    challengerBondWei,
    challengeWindowSeconds,
    challengeResolutionMode,
    watcherLivenessSlo,
    failClosedConditions,
  });

  if (capability.enabled !== true) errors.push('route is not enabled for optimistic public beta use');
  if (capability.can_quote !== true) errors.push('route cannot quote');
  if (capability.can_run !== true) errors.push('route cannot run');
  if (!publicBeta) errors.push('route is not marked optimistic public beta');
  if (trustClass !== PFTL_UNISWAP_OPTIMISTIC_TRUST_CLASS) errors.push('route trust class must be OPTIMISTIC');
  if (routeClaim === 'TRUSTLESS' || routeClaim === 'TRUSTLESS_FINALITY') {
    errors.push('OPTIMISTIC route must not claim trustless finality');
  }
  if (publicRouting !== true) errors.push('public routing flag must be true for optimistic public beta');
  if (paused) errors.push('route is paused');
  if (routeSupplyCapAtoms === null) errors.push('route supply cap is missing');
  if (capRemainingAtoms === null) errors.push('remaining route cap is missing');
  if (packetNotionalCapAtoms === null) errors.push('packet notional cap is missing');
  if (capRemainingAtoms === 0n) errors.push('remaining route cap is zero');
  if (posterBondWei === null) errors.push('poster bond is missing');
  if (challengerBondWei === null) errors.push('challenger bond is missing');
  if (challengeGasMarginWei === null) errors.push('challenge gas margin is missing');
  if (challengeWindowSeconds === null) errors.push('challenge window is missing');
  if (challengeResolutionWindowSeconds === null) errors.push('challenge resolution window is missing');
  if (!challengeResolutionMode) {
    errors.push('challenge resolution mode is missing');
  } else if (
    !OBJECTIVE_CHALLENGE_RESOLUTION_MODES.has(challengeResolutionMode)
      && !ARBITRATED_CHALLENGE_RESOLUTION_MODES.has(challengeResolutionMode)
  ) {
    errors.push('challenge resolution mode must be objective or disclosed as arbitrated');
  }
  if (!watcherLivenessSlo) errors.push('watcher liveness SLO is missing');
  if (!/^[0-9a-f]{96}$/.test(bindingDigest)) errors.push('optimistic launch binding digest is missing');
  if (failClosedConditions.length === 0) errors.push('fail-closed conditions are missing');
  for (const field of REQUIRED_OPTIMISTIC_DISPLAY_FIELDS) {
    if (!preSignDisplay[field]) errors.push(`pre-sign display missing ${field}`);
  }
  const displayTrustLabel = String(preSignDisplay.routeTrustLabel || '').toUpperCase();
  if (displayTrustLabel && (displayTrustLabel.includes('TRUSTLESS') || !displayTrustLabel.includes('OPTIMISTIC'))) {
    errors.push('pre-sign route trust label must disclose OPTIMISTIC and must not claim trustless finality');
  }
  if (
    displayTrustLabel
      && ARBITRATED_CHALLENGE_RESOLUTION_MODES.has(challengeResolutionMode)
      && !displayTrustLabel.includes('ARBITRATED')
  ) {
    errors.push('pre-sign route trust label must disclose ARBITRATED challenge resolution');
  }
  const displayVerifierMode = String(preSignDisplay.bridgeVerifierMode || '').toUpperCase();
  if (displayVerifierMode && displayVerifierMode !== PFTL_UNISWAP_OPTIMISTIC_TRUST_CLASS) {
    errors.push('pre-sign bridge verifier mode must match OPTIMISTIC');
  }
  if (requestedAtoms !== null && packetNotionalCapAtoms !== null && requestedAtoms > packetNotionalCapAtoms) {
    errors.push('requested amount exceeds packet cap');
  }
  if (requestedAtoms !== null && capRemainingAtoms !== null && requestedAtoms > capRemainingAtoms) {
    errors.push('requested amount exceeds remaining route cap');
  }

  const legacyField = firstLegacyField(capability);
  if (legacyField) errors.push(`${legacyField} selects legacy a651`);
  if (booleanField(capability, ['legacy_pool_enabled', 'legacy_pool_fallback', 'uses_legacy_pool']) === true) {
    errors.push('legacy pool fallback is enabled');
  }
  if (stringField(capability, ['fallback_route', 'fallbackRoute']) === LEGACY_A651_UNISWAP_ROUTE) {
    errors.push('legacy a651 fallback route is configured');
  }

  return {
    ok: errors.length === 0,
    route: PFTL_UNISWAP_BETA_ROUTE,
    status: errors.length === 0 ? 'optimistic_public_beta_ready' : 'optimistic_public_beta_blocked',
    message: errors[0] || 'Optimistic public beta route accepted: capped, bonded, watched, and fail-closed',
    blockingReasons: errors,
    trustClass,
    publicBeta,
    publicRouting: publicRouting === true,
    paused,
    routeSupplyCapAtoms: routeSupplyCapAtoms === null ? null : routeSupplyCapAtoms.toString(),
    capRemainingAtoms: capRemainingAtoms === null ? null : capRemainingAtoms.toString(),
    packetNotionalCapAtoms: packetNotionalCapAtoms === null ? null : packetNotionalCapAtoms.toString(),
    posterBondWei: posterBondWei === null ? null : posterBondWei.toString(),
    challengerBondWei: challengerBondWei === null ? null : challengerBondWei.toString(),
    challengeGasMarginWei: challengeGasMarginWei === null ? null : challengeGasMarginWei.toString(),
    challengeWindowSeconds: challengeWindowSeconds === null ? null : challengeWindowSeconds.toString(),
    challengeResolutionWindowSeconds: challengeResolutionWindowSeconds === null
      ? null
      : challengeResolutionWindowSeconds.toString(),
    challengeResolutionMode: challengeResolutionMode || null,
    watcherLivenessSlo,
    optimisticLaunchBindingDigest: bindingDigest || null,
    failClosedConditions,
    preSignDisplay,
    walletCopy: {
      label: ARBITRATED_CHALLENGE_RESOLUTION_MODES.has(challengeResolutionMode)
        ? 'OPTIMISTIC owner-arbitrated public beta'
        : 'OPTIMISTIC public beta',
      route: 'pfUSDC -> primary mint -> bridge -> wrapped venue token',
      warning: ARBITRATED_CHALLENGE_RESOLUTION_MODES.has(challengeResolutionMode)
        ? 'Optimistic route. Owner/governance resolver arbitrates challenges until direct PFTL finality verification exists.'
        : 'Optimistic route. Watchers, challenge window, bonds, route caps, and fail-closed rules apply.',
      requiredBeforeSigning: [
        'route cap',
        'packet cap',
        'poster bond',
        'challenger bond',
        'challenge window',
        'challenge resolution mode',
        'watcher liveness SLO',
        'fail-closed conditions',
      ],
    },
  };
}

function optimisticPreSignDisplay({
  capability,
  trustClass,
  routeSupplyCapAtoms,
  capRemainingAtoms,
  packetNotionalCapAtoms,
  posterBondWei,
  challengerBondWei,
  challengeWindowSeconds,
  challengeResolutionMode,
  watcherLivenessSlo,
  failClosedConditions,
}) {
  const refundDeadline = nonNegativeBigIntField(capability, [
    'refund_deadline_unix_seconds',
    'refund_deadline_seconds',
    'refund_deadline',
  ]);
  const bridgeVerifierMode = stringField(capability, [
    'bridge_verifier_mode',
    'verifier_mode',
    'bridgeVerifierMode',
  ]) || trustClass;
  const routeTrustLabel = stringField(capability, [
    'route_trust_label',
    'trust_label',
    'wallet_trust_label',
  ]);

  return {
    canonicalNav: stringField(capability, [
      'canonical_nav',
      'canonical_nav_price',
      'primary_nav_price',
      'nav_price',
    ]),
    uniswapMarketPrice: stringField(capability, [
      'uniswap_market_price',
      'amm_market_price',
      'market_price',
    ]),
    proofFreshness: stringField(capability, [
      'proof_freshness',
      'nav_proof_freshness',
      'receipt_proof_freshness',
      'proof_status',
    ]),
    bridgeVerifierMode,
    challengeResolutionMode,
    packetStatus: stringField(capability, [
      'packet_status',
      'bridge_packet_status',
      'packet_state',
    ]),
    refundDeadlineUnixSeconds: refundDeadline === null ? null : refundDeadline.toString(),
    routeTrustLabel,
    routeCapAtoms: routeSupplyCapAtoms === null ? null : routeSupplyCapAtoms.toString(),
    capRemainingAtoms: capRemainingAtoms === null ? null : capRemainingAtoms.toString(),
    packetNotionalCapAtoms: packetNotionalCapAtoms === null ? null : packetNotionalCapAtoms.toString(),
    posterBondWei: posterBondWei === null ? null : posterBondWei.toString(),
    challengerBondWei: challengerBondWei === null ? null : challengerBondWei.toString(),
    challengeWindowSeconds: challengeWindowSeconds === null ? null : challengeWindowSeconds.toString(),
    watcherLivenessSlo,
    failClosedConditions,
  };
}

function normalizedResolutionMode(capability) {
  return stringField(capability, [
    'challenge_resolution_mode',
    'resolver_mode',
    'challenge_resolver_mode',
  ]).toLowerCase().replace(/-/g, '_');
}

function stringField(object, keys) {
  for (const key of keys) {
    const value = object?.[key];
    if (value !== undefined && value !== null && value !== '') return String(value).trim();
  }
  return '';
}

function arrayField(object, keys) {
  for (const key of keys) {
    const value = object?.[key];
    if (Array.isArray(value)) return value.map((item) => String(item)).filter(Boolean);
    if (typeof value === 'string' && value.trim()) return [value.trim()];
  }
  return [];
}

function booleanField(object, keys) {
  for (const key of keys) {
    const value = object?.[key];
    if (value === true || value === false) return value;
    if (typeof value === 'string') {
      const lower = value.trim().toLowerCase();
      if (lower === 'true') return true;
      if (lower === 'false') return false;
    }
  }
  return null;
}

function integerText(value) {
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value) || value < 0) return null;
    return String(value);
  }
  const text = String(value ?? '').trim();
  return /^[0-9]+$/.test(text) ? text : null;
}

function optionalBigInt(value) {
  const text = integerText(value);
  if (text === null) return null;
  return BigInt(text);
}

function positiveBigIntField(object, keys) {
  for (const key of keys) {
    const value = optionalBigInt(object?.[key]);
    if (value !== null) return value > 0n ? value : null;
  }
  return null;
}

function nonNegativeBigIntField(object, keys) {
  for (const key of keys) {
    const value = optionalBigInt(object?.[key]);
    if (value !== null) return value;
  }
  return null;
}

function firstLegacyField(capability) {
  const legacyValues = new Set([
    LEGACY_A651_ETHEREUM_TOKEN.toLowerCase(),
    LEGACY_A651_UNISWAP_POOL_ID.toLowerCase(),
  ]);
  for (const key of [
    'wrapped_navcoin_token',
    'destination_token',
    'venue_token',
    'uniswap_pool_id',
    'uniswap_pool_id_or_path',
    'pool_id',
  ]) {
    const value = stringField(capability, [key]).toLowerCase();
    if (value && legacyValues.has(value)) return key;
  }
  return null;
}
