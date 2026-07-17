export const NAVSWAP_WALLET_ACTION_SCHEMA = 'postfiat-navswap-wallet-action-request-v1';

export const TRANSPARENT_NAVSWAP_ROUTE = 'transparent_navswap';
export const PFTL_UNISWAP_BETA_ROUTE = 'uniswap_atomic_handoff';
export const LEGACY_A651_UNISWAP_ROUTE = 'legacy_a651_uniswap';
export const LEGACY_A651_PFTL_ASSET_ID = 'dcddbf56e7e15f7893d0038e8e0e6089d5a41418dead75353aabb8c016cf626beeb93bc802929f29883c078d910f59d5';
export const LEGACY_A651_ETHEREUM_TOKEN = '0x1e55eda7ce0788e8b624456c4d401a33bd83b62e';
export const LEGACY_A651_UNISWAP_POOL_ID = '0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84';

const TRANSPARENT_ROUTE = TRANSPARENT_NAVSWAP_ROUTE;
const ROUTE_TRUST_CLASSES = new Set(['CONTROLLED', 'OPTIMISTIC', 'TRUSTLESS_FINALITY', 'DISABLED']);
const WALLET_ACTION_ROUTES = new Set([TRANSPARENT_NAVSWAP_ROUTE, PFTL_UNISWAP_BETA_ROUTE]);
const PRIMARY_MINT_ROUTE_FAMILIES = new Set([
  'primary_pftl_mint',
  'composite_primary_mint_to_ethereum_venue',
]);
const OPTIMISTIC_PUBLIC_BETA_STAGES = new Set([
  'public_beta',
  'optimistic_public_beta',
  'capped_public_beta',
]);
const OBJECTIVE_CHALLENGE_RESOLUTION_MODES = new Set([
  'ONCHAIN_OBJECTIVE',
  'DIRECT_OR_SUCCINCT_PFTL_FINALITY',
]);
const ARBITRATED_CHALLENGE_RESOLUTION_MODES = new Set([
  'OWNER_ARBITRATED',
  'GOVERNANCE_ARBITRATED',
]);
const USER_NAVSWAP_ACTIONS = new Set([
  'vault_bridge_nav_subscription_allocate',
  'nav_redeem_at_nav',
  'pftl_uniswap_primary_subscribe',
  'pftl_uniswap_export_debit',
]);
const FORBIDDEN_REQUEST_KEYS = new Set([
  'key_file',
  'owner_key_file',
  'issuer_key_file',
  'subscriber_key_file',
  'private_key',
  'seed',
  'backup',
  'passphrase',
]);

function assertObject(value, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value;
}

function nonEmptyString(value, label) {
  if (typeof value !== 'string' || !value.trim()) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value.trim();
}

function optionalString(value, label) {
  if (value === undefined || value === null || value === '') return null;
  return nonEmptyString(value, label);
}

function integerText(value, label) {
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value) || value <= 0) {
      throw new Error(`${label} must be a positive integer`);
    }
    return String(value);
  }
  const text = String(value ?? '').trim();
  if (!/^[1-9][0-9]*$/.test(text)) {
    throw new Error(`${label} must be a positive integer`);
  }
  return text;
}

function optionalPositiveMillis(value, label) {
  if (value === undefined || value === null || value === '') return null;
  const text = integerText(value, label);
  const parsed = Number.parseInt(text, 10);
  if (!Number.isSafeInteger(parsed)) {
    throw new Error(`${label} exceeds the wallet adapter safe integer range`);
  }
  return parsed;
}

function explicitFalse(value) {
  return value === false || String(value).trim().toLowerCase() === 'false';
}

function explicitTrue(value) {
  return value === true || String(value).trim().toLowerCase() === 'true';
}

function staleStatus(value) {
  if (value === undefined || value === null || value === '') return false;
  return new Set(['stale', 'expired', 'inactive', 'unavailable', 'disabled']).has(
    String(value).trim().toLowerCase(),
  );
}

function assertFreshIntent(intent) {
  if (explicitFalse(intent.reserve_packet_fresh)) {
    throw new Error('NAVSwap quote reserve packet is stale');
  }
  if (explicitFalse(intent.supply_packet_fresh)) {
    throw new Error('NAVSwap quote supply packet is stale');
  }
  if (staleStatus(intent.proof_status) || staleStatus(intent.market_ops_status)) {
    throw new Error('NAVSwap quote freshness proof is stale');
  }
  const generatedAtMs = optionalPositiveMillis(intent.quote_generated_at_ms, 'NAVSwap quote_generated_at_ms');
  const expiresAtMs = optionalPositiveMillis(intent.quote_expires_at_ms, 'NAVSwap quote_expires_at_ms');
  if (generatedAtMs !== null && expiresAtMs !== null && expiresAtMs <= generatedAtMs) {
    throw new Error('NAVSwap quote expiration must be after quote generation');
  }
  if (expiresAtMs !== null && Date.now() > expiresAtMs) {
    throw new Error('NAVSwap quote has expired');
  }
}

function assertSame(actual, expected, label) {
  if (expected === undefined || expected === null || expected === '') return;
  if (actual !== expected) {
    throw new Error(`${label} does not match the approved NAVSwap request`);
  }
}

function assertOneOf(actual, allowed, label) {
  if (!allowed.has(actual)) {
    throw new Error(`${label} is not supported by this wallet route`);
  }
}

function lowerOptionalString(value) {
  if (value === undefined || value === null || value === '') return null;
  return String(value).trim().toLowerCase();
}

function assertNotLegacyA651(value, label) {
  const text = lowerOptionalString(value);
  if (!text) return;
  if (
    text === LEGACY_A651_ETHEREUM_TOKEN
    || text === LEGACY_A651_UNISWAP_POOL_ID
  ) {
    throw new Error(`${label} selects legacy a651; legacy a651 is historical secondary liquidity, not the PFTL-Uniswap bridge token`);
  }
}

function assertNoLegacyA651CompositeHandoff(intent, operation) {
  const routeFamily = lowerOptionalString(intent.route_family || intent.purchase_kind);
  if (routeFamily !== 'composite_primary_mint_to_ethereum_venue') return;

  assertNotLegacyA651(intent.wrapped_navcoin_token, 'NAV subscription wrapped_navcoin_token');
  assertNotLegacyA651(intent.destination_token, 'NAV subscription destination_token');
  assertNotLegacyA651(intent.venue_token, 'NAV subscription venue_token');
  assertNotLegacyA651(intent.uniswap_pool_id, 'NAV subscription uniswap_pool_id');
  assertNotLegacyA651(intent.uniswap_pool_id_or_path, 'NAV subscription uniswap_pool_id_or_path');
}

function assertPftlUniswapCompositeHandoff(intent, operation) {
  const routeFamily = lowerOptionalString(intent.route_family || intent.purchase_kind);
  if (routeFamily !== 'composite_primary_mint_to_ethereum_venue') {
    throw new Error('PFTL-Uniswap beta route requires composite_primary_mint_to_ethereum_venue intent');
  }

  const trustClass = nonEmptyString(intent.route_trust_class, 'NAV subscription route_trust_class').toUpperCase();
  if (trustClass !== 'CONTROLLED' && trustClass !== 'OPTIMISTIC') {
    throw new Error('PFTL-Uniswap beta route must be labeled CONTROLLED or OPTIMISTIC');
  }

  if (explicitTrue(intent.route_paused)) {
    throw new Error('PFTL-Uniswap beta route is paused');
  }

  integerText(
    intent.route_supply_cap_atoms || intent.route_cap_atoms,
    'PFTL-Uniswap beta route_supply_cap_atoms',
  );
  const capRemainingAtoms = integerText(
    intent.supply_cap_remaining_atoms || intent.route_cap_remaining_atoms || intent.cap_remaining_atoms,
    'PFTL-Uniswap beta supply_cap_remaining_atoms',
  );
  const packetCapAtoms = integerText(
    intent.packet_notional_cap_atoms || intent.per_packet_cap_atoms,
    'PFTL-Uniswap beta packet_notional_cap_atoms',
  );
  const routeAmountAtoms = BigInt(integerText(
    intent.export_amount_atoms
      || intent.mint_amount_atoms
      || operation.amount_atoms
      || operation.settlement_amount_atoms
      || operation.settlement_value_atoms,
    'PFTL-Uniswap beta route amount atoms',
  ));
  if (routeAmountAtoms > BigInt(packetCapAtoms)) {
    throw new Error('PFTL-Uniswap beta route amount exceeds packet cap');
  }
  if (routeAmountAtoms > BigInt(capRemainingAtoms)) {
    throw new Error('PFTL-Uniswap beta route amount exceeds remaining cap');
  }

  if (trustClass === 'CONTROLLED') {
    if (!explicitFalse(intent.public_routing_enabled)) {
      throw new Error('PFTL-Uniswap controlled beta route cannot be marked as public routing');
    }
  } else {
    assertOptimisticPublicBetaHandoff(intent);
  }
}

function assertIntegerAtMost(actual, max, label) {
  if (max === undefined || max === null || max === '') return;
  if (BigInt(integerText(actual, label)) > BigInt(integerText(max, `${label} max`))) {
    throw new Error(`${label} exceeds the approved NAVSwap request`);
  }
}

function assertIntegerSame(actual, expected, label) {
  if (expected === undefined || expected === null || expected === '') return;
  if (BigInt(integerText(actual, label)) !== BigInt(integerText(expected, `${label} approved`))) {
    throw new Error(`${label} does not match the approved NAVSwap request`);
  }
}

function assertHex96(value, label) {
  const text = nonEmptyString(value, label);
  if (!/^[0-9a-f]{96}$/i.test(text)) {
    throw new Error(`${label} must be 96 hex characters`);
  }
  return text.toLowerCase();
}

function assertHex64(value, label) {
  const text = nonEmptyString(value, label);
  if (!/^[0-9a-f]{64}$/i.test(text)) {
    throw new Error(`${label} must be 64 hex characters`);
  }
  return text.toLowerCase();
}

function assertNonEmptyList(value, label) {
  if (Array.isArray(value)) {
    if (value.some(item => typeof item === 'string' && item.trim())) return;
  } else if (typeof value === 'string' && value.trim()) {
    return;
  }
  throw new Error(`${label} must be a non-empty list`);
}

function assertOptimisticPublicBetaHandoff(intent) {
  const releaseStage = lowerOptionalString(intent.release_stage || intent.route_stage || intent.route_mode || intent.mode);
  if (!explicitTrue(intent.optimistic_public_beta) && !OPTIMISTIC_PUBLIC_BETA_STAGES.has(releaseStage)) {
    throw new Error('PFTL-Uniswap optimistic route must be marked optimistic public beta');
  }
  if (!explicitTrue(intent.public_routing_enabled)) {
    throw new Error('PFTL-Uniswap optimistic route must explicitly enable public routing');
  }

  integerText(intent.poster_bond_wei || intent.claim_bond_wei, 'PFTL-Uniswap optimistic poster_bond_wei');
  integerText(intent.challenger_bond_wei || intent.challenge_bond_wei, 'PFTL-Uniswap optimistic challenger_bond_wei');
  integerText(intent.challenge_gas_cost_with_margin_wei, 'PFTL-Uniswap optimistic challenge_gas_cost_with_margin_wei');
  integerText(intent.challenge_window_seconds, 'PFTL-Uniswap optimistic challenge_window_seconds');
  integerText(
    intent.challenge_resolution_window_seconds || intent.resolution_window_seconds,
    'PFTL-Uniswap optimistic challenge_resolution_window_seconds',
  );
  nonEmptyString(intent.watcher_liveness_slo || intent.watcher_slo, 'PFTL-Uniswap optimistic watcher_liveness_slo');
  assertHex96(
    intent.optimistic_launch_binding_digest || intent.launch_binding_digest || intent.binding_digest,
    'PFTL-Uniswap optimistic launch binding digest',
  );
  assertNonEmptyList(
    intent.fail_closed_conditions || intent.fail_closed || intent.fail_closed_requirements,
    'PFTL-Uniswap optimistic fail_closed_conditions',
  );
  const challengeResolutionMode = nonEmptyString(
    intent.challenge_resolution_mode || intent.resolver_mode || intent.challenge_resolver_mode,
    'PFTL-Uniswap optimistic challenge_resolution_mode',
  ).toUpperCase().replace(/-/g, '_');
  if (
    !OBJECTIVE_CHALLENGE_RESOLUTION_MODES.has(challengeResolutionMode)
      && !ARBITRATED_CHALLENGE_RESOLUTION_MODES.has(challengeResolutionMode)
  ) {
    throw new Error('PFTL-Uniswap optimistic challenge_resolution_mode must be objective or disclosed as arbitrated');
  }

  const routeTrustLabel = nonEmptyString(
    intent.route_trust_label || intent.trust_label || intent.wallet_trust_label,
    'PFTL-Uniswap optimistic route_trust_label',
  ).toUpperCase();
  if (routeTrustLabel.includes('TRUSTLESS') || !routeTrustLabel.includes('OPTIMISTIC')) {
    throw new Error('PFTL-Uniswap optimistic route trust label must disclose OPTIMISTIC and must not claim TRUSTLESS_FINALITY');
  }
  if (ARBITRATED_CHALLENGE_RESOLUTION_MODES.has(challengeResolutionMode) && !routeTrustLabel.includes('ARBITRATED')) {
    throw new Error('PFTL-Uniswap optimistic route trust label must disclose ARBITRATED challenge resolution');
  }
  const verifierMode = nonEmptyString(
    intent.bridge_verifier_mode || intent.verifier_mode,
    'PFTL-Uniswap optimistic bridge_verifier_mode',
  ).toUpperCase();
  if (verifierMode !== 'OPTIMISTIC') {
    throw new Error('PFTL-Uniswap optimistic bridge verifier mode must match OPTIMISTIC');
  }

  nonEmptyString(
    intent.canonical_nav || intent.canonical_nav_price || intent.primary_nav_price || intent.nav_price,
    'PFTL-Uniswap optimistic canonical NAV display',
  );
  nonEmptyString(
    intent.uniswap_market_price || intent.amm_market_price || intent.market_price,
    'PFTL-Uniswap optimistic Uniswap market price display',
  );
  nonEmptyString(
    intent.proof_freshness || intent.nav_proof_freshness || intent.receipt_proof_freshness || intent.proof_status,
    'PFTL-Uniswap optimistic proof freshness display',
  );
  nonEmptyString(
    intent.packet_status || intent.bridge_packet_status || intent.packet_state,
    'PFTL-Uniswap optimistic packet status display',
  );
  integerText(
    intent.refund_deadline_unix_seconds || intent.refund_deadline_seconds || intent.refund_deadline,
    'PFTL-Uniswap optimistic refund deadline display',
  );
}

function assertPrimaryMintEconomics(intent) {
  const routeFamily = nonEmptyString(
    intent.route_family || intent.purchase_kind,
    'NAV subscription route_family',
  );
  assertOneOf(routeFamily, PRIMARY_MINT_ROUTE_FAMILIES, 'NAV subscription route_family');

  const trustClass = nonEmptyString(intent.route_trust_class, 'NAV subscription route_trust_class').toUpperCase();
  assertOneOf(trustClass, ROUTE_TRUST_CLASSES, 'NAV subscription route_trust_class');
  if (trustClass === 'TRUSTLESS_FINALITY' && intent.finality_verifier !== 'direct_or_succinct_pftl_finality') {
    throw new Error('NAV subscription cannot claim TRUSTLESS_FINALITY without direct_or_succinct_pftl_finality');
  }

  assertSame(
    nonEmptyString(intent.supply_effect, 'NAV subscription supply_effect'),
    'mints_new_native_navcoin_supply',
    'NAV subscription supply_effect',
  );
  assertSame(
    nonEmptyString(intent.pricing_source, 'NAV subscription pricing_source'),
    'finalized_pre_inflow_nav_snapshot',
    'NAV subscription pricing_source',
  );
  assertSame(
    nonEmptyString(intent.settlement_reserve_effect, 'NAV subscription settlement_reserve_effect'),
    'added_after_primary_fill',
    'NAV subscription settlement_reserve_effect',
  );
  assertSame(
    nonEmptyString(intent.uniswap_supply_effect, 'NAV subscription uniswap_supply_effect'),
    'not_uniswap_supply',
    'NAV subscription uniswap_supply_effect',
  );
  integerText(intent.mint_amount_atoms, 'NAV subscription mint_amount_atoms');
  integerText(intent.pricing_nav_epoch || intent.nav_epoch, 'NAV subscription pricing_nav_epoch');
  integerText(intent.primary_nav_price_atoms || intent.nav_per_unit, 'NAV subscription primary_nav_price_atoms');
  assertHex96(
    intent.pricing_reserve_packet_hash || intent.reserve_packet_hash || intent.nav_reserve_packet_hash,
    'NAV subscription pricing_reserve_packet_hash',
  );

  if (routeFamily === 'composite_primary_mint_to_ethereum_venue') {
    assertSame(
      nonEmptyString(intent.bridge_packet_effect, 'NAV subscription bridge_packet_effect'),
      'minted_navcoin_exported_or_claimed',
      'NAV subscription bridge_packet_effect',
    );
    assertSame(
      nonEmptyString(intent.ethereum_supply_effect, 'NAV subscription ethereum_supply_effect'),
      'mints_wrapped_venue_token_from_pftl_packet',
      'NAV subscription ethereum_supply_effect',
    );
  }
}

function findForbiddenRequestKey(value, path = []) {
  if (!value || typeof value !== 'object') return null;
  for (const [key, child] of Object.entries(value)) {
    const nextPath = [...path, key];
    if (FORBIDDEN_REQUEST_KEYS.has(key)) return nextPath.join('.');
    const nested = findForbiddenRequestKey(child, nextPath);
    if (nested) return nested;
  }
  return null;
}

function navswapIntent(request, explicitIntent = {}) {
  const embedded = request.user_intent || request.intent || {};
  return { ...embedded, ...explicitIntent };
}

function routeFromRequest(request) {
  return String(request.route || request.route_id || '').trim();
}

function requestWalletAddress(request, intent) {
  return request.wallet_address
    || request.source
    || request.owner
    || intent.wallet_address
    || intent.owner
    || null;
}

function expectedAsset(intent, ...keys) {
  for (const key of keys) {
    const value = optionalString(intent[key], key);
    if (value) return value;
  }
  return null;
}

function verifyNavSubscriptionAllocate(operation, walletAddress, intent, route) {
  assertPrimaryMintEconomics(intent);
  assertNoLegacyA651CompositeHandoff(intent, operation);
  if (route === PFTL_UNISWAP_BETA_ROUTE) {
    assertPftlUniswapCompositeHandoff(intent, operation);
  }
  assertSame(operation.operator, intent.operator || intent.issuer, 'NAV subscription operator');
  assertSame(operation.consume_supply_owner, walletAddress, 'NAV subscription settlement owner');
  assertSame(operation.nav_recipient, walletAddress, 'NAV subscription recipient');
  assertSame(
    operation.nav_asset_id,
    expectedAsset(intent, 'to_asset_id', 'nav_asset_id', 'target_nav_asset_id'),
    'NAV subscription asset_id',
  );
  assertSame(
    operation.settlement_asset_id,
    expectedAsset(intent, 'from_asset_id', 'settlement_asset_id'),
    'NAV subscription settlement_asset_id',
  );
  integerText(operation.settlement_amount_atoms, 'NAV subscription settlement_amount_atoms');
  assertIntegerAtMost(
    operation.settlement_amount_atoms,
    intent.max_settlement_amount_atoms || intent.settlement_amount_atoms,
    'NAV subscription settlement_amount_atoms',
  );
  const operationSubscriptionId = optionalString(
    operation.subscription_id,
    'NAV subscription subscription_id',
  );
  const intentSubscriptionId = optionalString(
    intent.subscription_id || intent.client_order_id || intent.order_id,
    'NAV subscription intent subscription_id',
  );
  if (operationSubscriptionId || intentSubscriptionId) {
    assertSame(
      operationSubscriptionId,
      intentSubscriptionId,
      'NAV subscription subscription_id',
    );
  }
  nonEmptyString(operation.settlement_bucket_id, 'NAV subscription settlement_bucket_id');
  nonEmptyString(operation.settlement_receipt_id, 'NAV subscription settlement_receipt_id');
  nonEmptyString(operation.consume_supply_allocation_id, 'NAV subscription consume_supply_allocation_id');
  return {
    stage: 'nav_subscription_allocate',
    source: walletAddress,
    operation,
  };
}

function verifyNavRedeemAtNav(operation, walletAddress, intent) {
  assertSame(operation.owner, walletAddress, 'NAV redeem owner');
  assertSame(operation.issuer, intent.operator || intent.issuer, 'NAV redeem issuer');
  assertSame(
    operation.asset_id,
    expectedAsset(intent, 'from_asset_id', 'nav_asset_id'),
    'NAV redeem asset_id',
  );
  assertIntegerSame(operation.epoch, intent.nav_epoch, 'NAV redeem epoch');
  assertSame(
    operation.reserve_packet_hash,
    intent.reserve_packet_hash || intent.nav_reserve_packet_hash,
    'NAV redeem reserve_packet_hash',
  );
  integerText(operation.amount, 'NAV redeem amount');
  assertIntegerAtMost(
    operation.amount,
    intent.max_redeem_amount_atoms || intent.amount_atoms,
    'NAV redeem amount',
  );
  return {
    stage: 'nav_redeem_at_nav',
    source: walletAddress,
    operation,
  };
}

function verifyPftlUniswapPrimarySubscribe(operation, walletAddress, intent) {
  assertPrimaryMintEconomics(intent);
  assertNoLegacyA651CompositeHandoff(intent, operation);
  assertPftlUniswapCompositeHandoff(intent, operation);
  assertSame(operation.subscriber, walletAddress, 'PFTL-Uniswap primary subscriber');
  assertSame(operation.route_id, intent.route_id, 'PFTL-Uniswap route_id');
  assertSame(
    operation.settlement_asset_id,
    expectedAsset(intent, 'from_asset_id', 'settlement_asset_id'),
    'PFTL-Uniswap settlement_asset_id',
  );
  assertHex64(operation.subscription_nonce, 'PFTL-Uniswap subscription_nonce');
  assertSame(operation.subscription_nonce, intent.subscription_nonce, 'PFTL-Uniswap subscription_nonce');
  integerText(operation.settlement_value_atoms, 'PFTL-Uniswap settlement_value_atoms');
  assertIntegerAtMost(
    operation.settlement_value_atoms,
    intent.max_settlement_amount_atoms || intent.settlement_value_atoms,
    'PFTL-Uniswap settlement_value_atoms',
  );
  assertIntegerSame(
    operation.nav_price_settlement_atoms_per_nav_atom,
    intent.nav_price_settlement_atoms_per_nav_atom || intent.primary_nav_price_atoms || intent.nav_per_unit,
    'PFTL-Uniswap NAV price',
  );
  assertIntegerSame(
    operation.pricing_nav_epoch,
    intent.pricing_nav_epoch || intent.nav_epoch,
    'PFTL-Uniswap NAV epoch',
  );
  assertSame(
    operation.pricing_reserve_packet_hash,
    intent.pricing_reserve_packet_hash || intent.reserve_packet_hash || intent.nav_reserve_packet_hash,
    'PFTL-Uniswap reserve packet hash',
  );
  assertHex96(intent.route_config_digest, 'PFTL-Uniswap route_config_digest');
  if (intent.launch_config_digest) assertHex96(intent.launch_config_digest, 'PFTL-Uniswap launch_config_digest');
  return {
    stage: 'pftl_uniswap_primary_subscribe',
    source: walletAddress,
    operation,
  };
}

function verifyPftlUniswapExportDebit(operation, walletAddress, intent) {
  assertPrimaryMintEconomics(intent);
  assertNoLegacyA651CompositeHandoff(intent, operation);
  assertPftlUniswapCompositeHandoff(intent, operation);
  assertSame(operation.owner, walletAddress, 'PFTL-Uniswap export owner');
  assertSame(operation.route_id, intent.route_id, 'PFTL-Uniswap export route_id');
  assertHex96(operation.packet_hash, 'PFTL-Uniswap packet_hash');
  assertSame(operation.packet_hash, intent.packet_hash, 'PFTL-Uniswap packet_hash');
  assertHex64(operation.export_nonce, 'PFTL-Uniswap export_nonce');
  assertSame(operation.export_nonce, intent.export_nonce, 'PFTL-Uniswap export_nonce');
  assertSame(operation.ethereum_recipient, intent.ethereum_recipient, 'PFTL-Uniswap ethereum_recipient');
  integerText(operation.amount_atoms, 'PFTL-Uniswap export amount_atoms');
  assertIntegerAtMost(
    operation.amount_atoms,
    intent.export_amount_atoms || intent.mint_amount_atoms || intent.amount_atoms,
    'PFTL-Uniswap export amount_atoms',
  );
  assertIntegerSame(
    operation.destination_deadline_seconds,
    intent.destination_deadline_seconds,
    'PFTL-Uniswap destination deadline',
  );
  assertIntegerSame(
    operation.refund_delay_blocks,
    intent.refund_delay_blocks,
    'PFTL-Uniswap refund delay',
  );
  assertHex96(intent.route_config_digest, 'PFTL-Uniswap route_config_digest');
  if (intent.launch_config_digest) assertHex96(intent.launch_config_digest, 'PFTL-Uniswap launch_config_digest');
  return {
    stage: 'pftl_uniswap_export_debit',
    source: walletAddress,
    operation,
  };
}

export function verifyNavswapPreparedAssetAction(request, walletAddress, explicitIntent = {}) {
  const payload = assertObject(request, 'NAVSwap action request');
  const forbidden = findForbiddenRequestKey(payload);
  if (forbidden) {
    throw new Error(`NAVSwap action request must not include custody key material (${forbidden})`);
  }
  if (payload.schema !== NAVSWAP_WALLET_ACTION_SCHEMA) {
    throw new Error(`Unsupported NAVSwap action schema: ${payload.schema || 'missing'}`);
  }
  const route = routeFromRequest(payload);
  if (route === LEGACY_A651_UNISWAP_ROUTE) {
    throw new Error('Legacy a651 Uniswap route is inspection-only and cannot be used as the PFTL-Uniswap bridge handoff');
  }
  if (!WALLET_ACTION_ROUTES.has(route)) {
    throw new Error('NAVSwap action route must be transparent_navswap or uniswap_atomic_handoff');
  }

  const intent = navswapIntent(payload, explicitIntent);
  assertFreshIntent(intent);
  const approvedWallet = nonEmptyString(walletAddress, 'wallet address');
  const requestWallet = requestWalletAddress(payload, intent);
  if (requestWallet) assertSame(requestWallet, approvedWallet, 'NAVSwap action wallet address');

  const source = nonEmptyString(payload.source, 'NAVSwap action source');
  assertSame(source, approvedWallet, 'NAVSwap action source');

  const operation = assertObject(payload.operation, 'NAVSwap action operation');
  const operationKind = nonEmptyString(operation.operation, 'NAVSwap action operation kind');
  if (!USER_NAVSWAP_ACTIONS.has(operationKind)) {
    throw new Error(`NAVSwap action ${operationKind} is not wallet-owned or is not supported`);
  }
  if (payload.stage) {
    const expectedStage = operationKind === 'vault_bridge_nav_subscription_allocate'
      ? 'nav_subscription_allocate'
      : operationKind;
    assertSame(payload.stage, expectedStage, 'NAVSwap action stage');
  }

  let verified;
  if (operationKind === 'vault_bridge_nav_subscription_allocate') {
    verified = verifyNavSubscriptionAllocate(operation, approvedWallet, intent, route);
  } else if (operationKind === 'pftl_uniswap_primary_subscribe') {
    if (route !== PFTL_UNISWAP_BETA_ROUTE) {
      throw new Error('PFTL-Uniswap primary subscribe action is only supported on uniswap_atomic_handoff');
    }
    verified = verifyPftlUniswapPrimarySubscribe(operation, approvedWallet, intent);
  } else if (operationKind === 'pftl_uniswap_export_debit') {
    if (route !== PFTL_UNISWAP_BETA_ROUTE) {
      throw new Error('PFTL-Uniswap export debit action is only supported on uniswap_atomic_handoff');
    }
    verified = verifyPftlUniswapExportDebit(operation, approvedWallet, intent);
  } else {
    if (route === PFTL_UNISWAP_BETA_ROUTE) {
      throw new Error('PFTL-Uniswap beta route only supports wallet-owned primary subscribe and export debit actions');
    }
    verified = verifyNavRedeemAtNav(operation, approvedWallet, intent);
  }

  return {
    ok: true,
    schema: NAVSWAP_WALLET_ACTION_SCHEMA,
    action_id: payload.action_id || null,
    route,
    stage: verified.stage,
    source: verified.source,
    operation: verified.operation,
    fields: { operation: verified.operation },
    intent,
  };
}

export async function submitNavswapPreparedAssetAction({
  request,
  walletAddress,
  backupJson,
  txBuilder,
  intent = {},
}) {
  if (!backupJson) throw new Error('Wallet must be unlocked to sign this NAVSwap action');
  if (!txBuilder || typeof txBuilder.sendAssetTransfer !== 'function') {
    throw new Error('Transaction builder is not available for NAVSwap action signing');
  }
  const verified = verifyNavswapPreparedAssetAction(request, walletAddress, intent);
  const result = await txBuilder.sendAssetTransfer(
    backupJson,
    verified.source,
    verified.fields,
  );
  return {
    ...result,
    navswap_action: {
      schema: NAVSWAP_WALLET_ACTION_SCHEMA,
      action_id: verified.action_id,
      route: verified.route,
      stage: verified.stage,
      source: verified.source,
      operation: verified.operation,
    },
  };
}

function normalizePreparedActionList(requests) {
  if (Array.isArray(requests)) return requests;
  if (requests?.actions && Array.isArray(requests.actions)) return requests.actions;
  if (requests?.prepared_actions && Array.isArray(requests.prepared_actions)) return requests.prepared_actions;
  if (requests?.action) return [requests.action];
  throw new Error('NAVSwap prepared action batch must contain an actions array');
}

export async function submitNavswapPreparedAssetActions({
  requests,
  walletAddress,
  backupJson,
  txBuilder,
  intent = {},
  onProgress,
}) {
  if (!backupJson) throw new Error('Wallet must be unlocked to sign NAVSwap actions');
  if (!txBuilder || typeof txBuilder.sendAssetTransfer !== 'function') {
    throw new Error('Transaction builder is not available for NAVSwap action signing');
  }
  const actions = normalizePreparedActionList(requests);
  if (actions.length === 0) {
    throw new Error('NAVSwap prepared action batch must not be empty');
  }

  const verified = actions.map((request, index) => ({
    index,
    request,
    verified: verifyNavswapPreparedAssetAction(request, walletAddress, intent),
  }));

  const submissions = [];
  for (const item of verified) {
    onProgress?.({ index: item.index, stage: item.verified.stage, status: 'signing' });
    try {
      const result = await txBuilder.sendAssetTransfer(
        backupJson,
        item.verified.source,
        item.verified.fields,
      );
      const submission = {
        ...result,
        navswap_action: {
          schema: NAVSWAP_WALLET_ACTION_SCHEMA,
          action_id: item.verified.action_id,
          route: item.verified.route,
          stage: item.verified.stage,
          source: item.verified.source,
          operation: item.verified.operation,
        },
      };
      submissions.push(submission);
      onProgress?.({
        index: item.index,
        stage: item.verified.stage,
        status: 'submitted',
        txId: submission.txId,
        receipt: submission.receipt,
      });
    } catch (caught) {
      const error = caught instanceof Error ? caught : new Error(String(caught));
      error.partial_results = submissions;
      error.failed_action = {
        index: item.index,
        stage: item.verified.stage,
        action_id: item.verified.action_id,
      };
      onProgress?.({
        index: item.index,
        stage: item.verified.stage,
        status: 'failed',
        message: error.message,
      });
      throw error;
    }
  }

  return {
    ok: true,
    count: submissions.length,
    actions: submissions.map(result => result.navswap_action),
    submissions,
  };
}
