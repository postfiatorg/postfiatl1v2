export const A666_ROUTE_SCHEMA = 'postfiat-pftl-uniswap-supply-status-v2';
export const A666_OUTBOUND_TRUST_CLASS = 'TRUSTLESS_FINALITY';
export const A666_RETURN_TRUST_CLASS = 'BFT_CHECKPOINT';
export const A666_ISSUE_MULTIPLIER_BPS = 10_050;
export const A666_REDEEM_MULTIPLIER_BPS = 9_995;
export const A666_MAX_EXPORT_PACKETS_PER_ORDER = 4n;

const HASH48_RE = /^[0-9a-f]{96}$/;
const EVM_RE = /^0x[0-9a-f]{40}$/;
const BYTES32_RE = /^0x[0-9a-f]{64}$/;

export function evaluateA666PrimaryAcquisition({
  supplyStatus = null,
  quote = null,
  amountAtoms = null,
  expected = null,
} = {}) {
  const status = objectValue(supplyStatus);
  const offer = objectValue(quote);
  const pins = objectValue(expected);
  const errors = [];
  const amount = positiveBigInt(amountAtoms);
  const packetCap = positiveBigInt(status.packet_notional_cap_atoms);
  const issueAvailable = nonNegativeBigInt(status.available_issue_atoms);
  const exportAvailable = nonNegativeBigInt(status.available_export_capacity_atoms);
  const minimumOrder = positiveBigInt(status.min_order_atoms);
  const maximumOrder = positiveBigInt(status.max_order_atoms);
  const packetCount = amount && packetCap ? divideRoundUp(amount, packetCap) : null;

  requireEqual(errors, status.schema, A666_ROUTE_SCHEMA, 'supply-status schema');
  requireEqual(errors, Number(status.route_schema_version), 2, 'route schema version');
  requireEqual(errors, Number(status.issue_multiplier_bps), A666_ISSUE_MULTIPLIER_BPS, 'issue multiplier');
  requireEqual(errors, Number(status.redeem_multiplier_bps), A666_REDEEM_MULTIPLIER_BPS, 'redeem multiplier');
  requireEqual(errors, status.outbound_verification_class, A666_OUTBOUND_TRUST_CLASS, 'outbound trust class');
  requireEqual(errors, status.return_verification_class, A666_RETURN_TRUST_CLASS, 'return trust class');
  requireEqual(errors, Number(status.ethereum_chain_id), 1, 'Ethereum chain');
  if (status.live_value_enabled !== true) errors.push('route live value is disabled');
  if (status.paused !== false) errors.push('route is paused');
  if (status.invariant_holds !== true) errors.push('route supply invariant is not satisfied');
  if (!HASH48_RE.test(String(status.policy_hash || ''))) errors.push('policy hash is missing or malformed');
  if (!HASH48_RE.test(String(status.pricing_reserve_packet_hash || ''))) {
    errors.push('pricing reserve packet hash is missing or malformed');
  }
  if (!EVM_RE.test(String(status.handoff_controller || ''))) errors.push('handoff controller is malformed');
  if (!EVM_RE.test(String(status.wrapped_navcoin_token || ''))) errors.push('wA666 token is malformed');

  if (amount === null) errors.push('requested a666 amount must be a positive integer');
  if (minimumOrder === null || maximumOrder === null) errors.push('order bounds are missing');
  if (amount && minimumOrder && amount < minimumOrder) errors.push('requested amount is below the minimum order');
  if (amount && maximumOrder && amount > maximumOrder) errors.push('requested amount exceeds the maximum order');
  if (issueAvailable === null || (amount && issueAvailable < amount)) {
    errors.push('remaining primary issue capacity is insufficient');
  }
  if (exportAvailable === null || (amount && exportAvailable < amount)) {
    errors.push('remaining Ethereum export capacity is insufficient');
  }
  if (packetCap === null) errors.push('packet cap is missing');
  if (packetCount && packetCount > A666_MAX_EXPORT_PACKETS_PER_ORDER) {
    errors.push('order requires more than four bound export packets');
  }

  requirePin(errors, 'route_id', status.route_id, pins.route_id);
  requirePin(errors, 'route_config_digest', status.route_config_digest, pins.route_config_digest);
  requirePin(errors, 'native_nav_asset_id', status.native_nav_asset_id, pins.native_nav_asset_id);
  requirePin(errors, 'settlement_asset_id', status.settlement_asset_id, pins.settlement_asset_id);
  requirePin(errors, 'handoff_controller', status.handoff_controller, pins.handoff_controller);
  requirePin(errors, 'wrapped_navcoin_token', status.wrapped_navcoin_token, pins.wrapped_navcoin_token);
  requirePin(errors, 'Uniswap pool', offer.uniswap_pool_id, pins.uniswap_pool_id);
  requirePin(errors, 'proof program vkey', offer.proof_program_vkey, pins.proof_program_vkey);

  const requiredQuoteFields = [
    'usdc_input_atoms',
    'max_usdc_input_atoms',
    'wa666_output_atoms',
    'min_wa666_output_atoms',
    'finalized_nav_atoms',
    'estimated_ethereum_gas_wei',
    'estimated_completion_seconds',
    'reservation_expires_at_height',
    'export_deadline_unix_seconds',
  ];
  for (const field of requiredQuoteFields) {
    if (positiveBigInt(offer[field]) === null) errors.push(`quote is missing ${field}`);
  }
  if (positiveBigInt(offer.pricing_nav_epoch) !== positiveBigInt(status.pricing_nav_epoch)) {
    errors.push('quote NAV epoch does not match the live route policy');
  }
  if (String(offer.pricing_reserve_packet_hash || '') !== String(status.pricing_reserve_packet_hash || '')) {
    errors.push('quote reserve packet does not match the live route policy');
  }
  if (!BYTES32_RE.test(String(offer.proof_program_vkey || ''))) {
    errors.push('quote proof program vkey is malformed');
  }

  return {
    ok: errors.length === 0,
    status: errors.length === 0 ? 'a666_primary_ready' : 'a666_primary_blocked',
    blockingReasons: errors,
    amountAtoms: amount?.toString() || null,
    packetCount: packetCount?.toString() || null,
    preSignDisplay: {
      action: 'Acquire a666 on Ethereum',
      usdcInputAtoms: textOrNull(offer.usdc_input_atoms),
      maxUsdcInputAtoms: textOrNull(offer.max_usdc_input_atoms),
      wa666OutputAtoms: textOrNull(offer.wa666_output_atoms),
      minWa666OutputAtoms: textOrNull(offer.min_wa666_output_atoms),
      finalizedNavAtoms: textOrNull(offer.finalized_nav_atoms),
      pricingNavEpoch: textOrNull(status.pricing_nav_epoch),
      issueMultiplierBps: String(A666_ISSUE_MULTIPLIER_BPS),
      availableIssueAtoms: issueAvailable?.toString() || null,
      availableExportAtoms: exportAvailable?.toString() || null,
      outboundTrustClass: status.outbound_verification_class || null,
      returnTrustClass: status.return_verification_class || null,
      estimatedEthereumGasWei: textOrNull(offer.estimated_ethereum_gas_wei),
      estimatedCompletionSeconds: textOrNull(offer.estimated_completion_seconds),
      reservationExpiresAtHeight: textOrNull(offer.reservation_expires_at_height),
      exportDeadlineUnixSeconds: textOrNull(offer.export_deadline_unix_seconds),
      routeId: status.route_id || null,
      wrappedToken: status.wrapped_navcoin_token || null,
      controller: status.handoff_controller || null,
      uniswapPoolId: offer.uniswap_pool_id || null,
    },
  };
}

function objectValue(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

function positiveBigInt(value) {
  const text = String(value ?? '').trim();
  return /^[0-9]+$/.test(text) && BigInt(text) > 0n ? BigInt(text) : null;
}

function nonNegativeBigInt(value) {
  const text = String(value ?? '').trim();
  return /^[0-9]+$/.test(text) ? BigInt(text) : null;
}

function divideRoundUp(value, divisor) {
  return (value + divisor - 1n) / divisor;
}

function requireEqual(errors, actual, expected, label) {
  if (actual !== expected) errors.push(`${label} does not match canonical a666 v2`);
}

function requirePin(errors, label, actual, expected) {
  if (!expected) {
    errors.push(`${label} production pin is missing`);
  } else if (String(actual || '').toLowerCase() !== String(expected).toLowerCase()) {
    errors.push(`${label} does not match the frozen production manifest`);
  }
}

function textOrNull(value) {
  const text = String(value ?? '').trim();
  return text || null;
}
