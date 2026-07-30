export const A666_ROUTE_SCHEMA = 'postfiat-pftl-uniswap-supply-status-v2';
export const A666_OUTBOUND_TRUST_CLASS = 'TRUSTLESS_FINALITY';
export const A666_RETURN_TRUST_CLASS = 'BFT_CHECKPOINT';
export const A666_ISSUE_MULTIPLIER_BPS = 10_050;
export const A666_REDEEM_MULTIPLIER_BPS = 9_995;
export const A666_MAX_EXPORT_PACKETS_PER_ORDER = 4n;
export const A666_PRIMARY_ROUTE_ID = 'pftl-a666-ethereum-wA666-usdc-v1';
export const A666_NATIVE_ASSET_ID = '521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c';
export const A666_SETTLEMENT_ASSET_ID = '02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b';
export const A666_ROUTE_CONFIG_DIGEST = '12ed00ca87e29554ce4b978da1710fffc0830767e84e62f08df257f727db953efdd89bcf6ea99f5634d6e5ea8aca2933';
export const A666_HANDOFF_CONTROLLER = '0x9a0262c0572fb4db08765408eb225e207f40c3d9';
export const A666_WRAPPED_TOKEN = '0xee4c92edb03efdd9b519339edc19ad70c69a9be5';
export const A666_ATOMS_PER_UNIT = 1_000_000n;
export const A666_NAV_SCALE = 100_000_000n;
export const A666_BPS_SCALE = 10_000n;

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

export function parseA666Units(value) {
  const text = String(value ?? '').trim();
  if (!/^(?:0|[1-9][0-9]*)(?:\.[0-9]{0,6})?$/.test(text)) return null;
  const [whole, fraction = ''] = text.split('.');
  const atoms = (BigInt(whole) * A666_ATOMS_PER_UNIT)
    + BigInt(fraction.padEnd(6, '0') || '0');
  return atoms > 0n ? atoms : null;
}

export function formatA666Units(value, maximumFractionDigits = 6) {
  const atoms = nonNegativeBigInt(value);
  if (atoms === null) return '—';
  const whole = atoms / A666_ATOMS_PER_UNIT;
  const fraction = (atoms % A666_ATOMS_PER_UNIT).toString().padStart(6, '0');
  const shown = fraction.slice(0, Math.max(0, Math.min(6, maximumFractionDigits))).replace(/0+$/, '');
  return `${whole.toLocaleString('en-US')}${shown ? `.${shown}` : ''}`;
}

export function formatA666Nav(navUsdE8) {
  const nav = nonNegativeBigInt(navUsdE8);
  if (nav === null) return '—';
  const whole = nav / A666_NAV_SCALE;
  const fraction = (nav % A666_NAV_SCALE).toString().padStart(8, '0').replace(/0+$/, '');
  return `$${whole.toLocaleString('en-US')}${fraction ? `.${fraction}` : ''}`;
}

export function deriveA666IssueQuote(amountAtoms, navUsdE8, multiplierBps = A666_ISSUE_MULTIPLIER_BPS) {
  const amount = positiveBigInt(amountAtoms);
  const nav = positiveBigInt(navUsdE8);
  const multiplier = positiveBigInt(multiplierBps);
  if (!amount || !nav || !multiplier) return null;
  const baseReserveAtoms = divideRoundUp(amount * nav, A666_NAV_SCALE);
  const settlementAtoms = divideRoundUp(baseReserveAtoms * multiplier, A666_BPS_SCALE);
  return {
    amountAtoms: amount.toString(),
    baseReserveAtoms: baseReserveAtoms.toString(),
    settlementAtoms: settlementAtoms.toString(),
    spreadAtoms: (settlementAtoms - baseReserveAtoms).toString(),
  };
}

export function deriveA666RedeemQuote(amountAtoms, navUsdE8, multiplierBps = A666_REDEEM_MULTIPLIER_BPS) {
  const amount = positiveBigInt(amountAtoms);
  const nav = positiveBigInt(navUsdE8);
  const multiplier = positiveBigInt(multiplierBps);
  if (!amount || !nav || !multiplier) return null;
  const baseReserveAtoms = divideRoundUp(amount * nav, A666_NAV_SCALE);
  const settlementAtoms = (baseReserveAtoms * multiplier) / A666_BPS_SCALE;
  return {
    amountAtoms: amount.toString(),
    baseReserveAtoms: baseReserveAtoms.toString(),
    settlementAtoms: settlementAtoms.toString(),
    spreadAtoms: (baseReserveAtoms - settlementAtoms).toString(),
  };
}

export function evaluateA666ResidentMarket({
  supplyStatus = null,
  navStatus = null,
  chainStatus = null,
  direction = 'issue',
  amountAtoms = null,
  pfusdcBalanceAtoms = null,
  a666BalanceAtoms = null,
} = {}) {
  const status = objectValue(supplyStatus);
  const nav = objectValue(navStatus);
  const chain = objectValue(chainStatus);
  const errors = [];
  const amount = positiveBigInt(amountAtoms);
  const currentHeight = nonNegativeBigInt(chain.block_height);
  const validFrom = nonNegativeBigInt(status.policy_valid_from_height);
  const expiresAt = positiveBigInt(status.policy_expires_at_height);

  requireEqual(errors, status.schema, A666_ROUTE_SCHEMA, 'supply-status schema');
  requireEqual(errors, Number(status.route_schema_version), 2, 'route schema version');
  requirePin(errors, 'route id', status.route_id, A666_PRIMARY_ROUTE_ID);
  requirePin(errors, 'route config digest', status.route_config_digest, A666_ROUTE_CONFIG_DIGEST);
  requirePin(errors, 'native A666 asset', status.native_nav_asset_id, A666_NATIVE_ASSET_ID);
  requirePin(errors, 'pfUSDC settlement asset', status.settlement_asset_id, A666_SETTLEMENT_ASSET_ID);
  requirePin(errors, 'handoff controller', status.handoff_controller, A666_HANDOFF_CONTROLLER);
  requirePin(errors, 'wrapped token', status.wrapped_navcoin_token, A666_WRAPPED_TOKEN);
  requireEqual(errors, Number(status.ethereum_chain_id), 1, 'Ethereum chain');
  requireEqual(errors, Number(status.issue_multiplier_bps), A666_ISSUE_MULTIPLIER_BPS, 'issue multiplier');
  requireEqual(errors, Number(status.redeem_multiplier_bps), A666_REDEEM_MULTIPLIER_BPS, 'redeem multiplier');
  if (status.live_value_enabled !== true) errors.push('primary market live value is disabled');
  if (status.paused !== false) errors.push('primary market is paused');
  if (status.invariant_holds !== true) errors.push('route supply invariant is not satisfied');
  if (!HASH48_RE.test(String(status.policy_hash || ''))) errors.push('policy hash is missing or malformed');
  if (!HASH48_RE.test(String(status.pricing_reserve_packet_hash || ''))) {
    errors.push('pricing reserve packet hash is missing or malformed');
  }

  if (nav.schema !== 'postfiat-vault-bridge-status-v1') errors.push('live NAV status schema is invalid');
  requirePin(errors, 'NAV asset', nav.asset_id, A666_NATIVE_ASSET_ID);
  requireEqual(errors, nav.valuation_unit, 'USD_1E8', 'NAV valuation unit');
  if (positiveBigInt(nav.nav_per_unit) === null) errors.push('live NAV price is missing');
  if (positiveBigInt(nav.finalized_epoch) !== positiveBigInt(status.pricing_nav_epoch)) {
    errors.push('live NAV epoch does not match the route policy');
  }
  if (String(nav.finalized_reserve_packet_hash || '') !== String(status.pricing_reserve_packet_hash || '')) {
    errors.push('live NAV reserve packet does not match the route policy');
  }

  if (currentHeight === null) errors.push('chain height is unavailable');
  if (currentHeight !== null && validFrom !== null && currentHeight < validFrom) errors.push('market policy is not active yet');
  if (currentHeight !== null && expiresAt !== null && currentHeight >= expiresAt) errors.push('market policy has expired');
  if (amount === null) errors.push('enter a positive A666 amount');

  const minimum = positiveBigInt(status.min_order_atoms);
  const maximum = positiveBigInt(status.max_order_atoms);
  if (amount && minimum && amount < minimum) errors.push('amount is below the market minimum');
  if (amount && maximum && amount > maximum) errors.push('amount exceeds the market maximum');

  const issueQuote = deriveA666IssueQuote(amount, nav.nav_per_unit, status.issue_multiplier_bps);
  const redeemQuote = deriveA666RedeemQuote(amount, nav.nav_per_unit, status.redeem_multiplier_bps);
  if (direction === 'issue') {
    const available = nonNegativeBigInt(status.available_issue_atoms);
    const capacity = nonNegativeBigInt(status.issue_capacity_remaining_atoms);
    const supplyCap = nonNegativeBigInt(status.supply_cap_remaining_atoms);
    if (amount && (available === null || amount > available)) errors.push('available issue inventory is insufficient');
    if (amount && (capacity === null || amount > capacity)) errors.push('issue capacity is insufficient');
    if (amount && (supplyCap === null || amount > supplyCap)) errors.push('route supply capacity is insufficient');
    const balance = nonNegativeBigInt(pfusdcBalanceAtoms);
    if (issueQuote && (balance === null || balance < BigInt(issueQuote.settlementAtoms))) {
      errors.push('wallet pfUSDC balance is insufficient');
    }
  } else if (direction === 'redeem') {
    const available = nonNegativeBigInt(status.available_redeem_atoms);
    const capacity = nonNegativeBigInt(status.redeem_capacity_remaining_atoms);
    const balance = nonNegativeBigInt(a666BalanceAtoms);
    if (amount && (available === null || amount > available)) errors.push('available redemption inventory is insufficient');
    if (amount && (capacity === null || amount > capacity)) errors.push('redemption capacity is insufficient');
    if (amount && (balance === null || amount > balance)) errors.push('wallet A666 balance is insufficient');
  } else {
    errors.push('market direction is invalid');
  }

  return {
    ok: errors.length === 0,
    blockingReasons: errors,
    quote: direction === 'redeem' ? redeemQuote : issueQuote,
  };
}

export function randomLowerHex(bytes) {
  if (!Number.isInteger(bytes) || bytes <= 0) throw new Error('random byte count must be a positive integer');
  const values = new Uint8Array(bytes);
  globalThis.crypto.getRandomValues(values);
  return Array.from(values, value => value.toString(16).padStart(2, '0')).join('');
}

export function buildA666IssueOperations({
  walletAddress,
  ethereumRecipient,
  supplyStatus,
  chainHeight,
  amountAtoms,
  settlementAtoms,
  reservationId = randomLowerHex(48),
  subscriptionNonce = randomLowerHex(32),
} = {}) {
  const status = objectValue(supplyStatus);
  if (!/^pf[0-9a-f]{40}$/.test(String(walletAddress || ''))) throw new Error('wallet address is malformed');
  if (!EVM_RE.test(String(ethereumRecipient || ''))) throw new Error('Ethereum recipient must be a lowercase address');
  const height = nonNegativeBigInt(chainHeight);
  const policyExpiry = positiveBigInt(status.policy_expires_at_height);
  if (height === null || policyExpiry === null) throw new Error('route height or policy expiry is unavailable');
  const reservationExpiry = height + 100n < policyExpiry ? height + 100n : policyExpiry - 1n;
  if (reservationExpiry <= height) throw new Error('route policy expires too soon to reserve this order');
  const amount = positiveBigInt(amountAtoms);
  const settlement = positiveBigInt(settlementAtoms);
  if (!amount || !settlement) throw new Error('issue amounts are invalid');
  if (!HASH48_RE.test(reservationId)) throw new Error('reservation id is malformed');
  if (!/^[0-9a-f]{64}$/.test(subscriptionNonce)) throw new Error('subscription nonce is malformed');
  const common = {
    route_id: status.route_id,
    reservation_id: reservationId,
  };
  return {
    reservationId,
    reserve: {
      operation: 'pftl_uniswap_order_reserve',
      subscriber: walletAddress,
      ...common,
      ethereum_recipient: ethereumRecipient,
      route_epoch: Number(status.route_epoch),
      policy_epoch: Number(status.policy_epoch),
      policy_hash: status.policy_hash,
      mint_amount_atoms: Number(amount),
      max_settlement_value_atoms: Number(settlement),
      expires_at_height: Number(reservationExpiry),
    },
    subscribe: {
      operation: 'pftl_uniswap_primary_subscribe_v2',
      subscriber: walletAddress,
      ...common,
      subscription_nonce: subscriptionNonce,
      settlement_asset_id: status.settlement_asset_id,
      settlement_value_atoms: Number(settlement),
      pricing_nav_epoch: Number(status.pricing_nav_epoch),
      pricing_reserve_packet_hash: status.pricing_reserve_packet_hash,
    },
    release: {
      operation: 'pftl_uniswap_order_release',
      releaser: walletAddress,
      ...common,
    },
  };
}

export function buildA666RedeemOperation({
  walletAddress,
  supplyStatus,
  chainHeight,
  amountAtoms,
  minimumSettlementAtoms,
  redemptionNonce = randomLowerHex(32),
} = {}) {
  const status = objectValue(supplyStatus);
  if (!/^pf[0-9a-f]{40}$/.test(String(walletAddress || ''))) throw new Error('wallet address is malformed');
  const height = nonNegativeBigInt(chainHeight);
  const policyExpiry = positiveBigInt(status.policy_expires_at_height);
  if (height === null || policyExpiry === null) throw new Error('route height or policy expiry is unavailable');
  const expiry = height + 100n < policyExpiry ? height + 100n : policyExpiry - 1n;
  if (expiry <= height) throw new Error('route policy expires too soon to redeem');
  const amount = positiveBigInt(amountAtoms);
  const settlement = positiveBigInt(minimumSettlementAtoms);
  if (!amount || !settlement) throw new Error('redemption amounts are invalid');
  if (!/^[0-9a-f]{64}$/.test(redemptionNonce)) throw new Error('redemption nonce is malformed');
  return {
    operation: 'pftl_uniswap_primary_redeem',
    owner: walletAddress,
    settlement_recipient: walletAddress,
    route_id: status.route_id,
    redemption_nonce: redemptionNonce,
    nav_amount_atoms: Number(amount),
    min_settlement_value_atoms: Number(settlement),
    route_epoch: Number(status.route_epoch),
    policy_epoch: Number(status.policy_epoch),
    policy_hash: status.policy_hash,
    pricing_nav_epoch: Number(status.pricing_nav_epoch),
    pricing_reserve_packet_hash: status.pricing_reserve_packet_hash,
    expires_at_height: Number(expiry),
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
