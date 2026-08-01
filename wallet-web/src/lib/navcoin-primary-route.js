export const NAVCOIN_ROUTE_SCHEMA = 'postfiat-pftl-uniswap-supply-status-v2';
export const NAVCOIN_NAV_SCALE = 100_000_000n;
export const NAVCOIN_BPS_SCALE = 10_000n;
const JSON_SAFE_INTEGER_MAX = BigInt(Number.MAX_SAFE_INTEGER);

const HASH48_RE = /^[0-9a-f]{96}$/;
const EVM_RE = /^0x[0-9a-f]{40}$/;
export function parseNavcoinUnits(value, decimals = 6) {
  if (!Number.isSafeInteger(decimals) || decimals < 0 || decimals > 18) return null;
  const text = String(value ?? '').trim();
  const pattern = decimals === 0
    ? /^(?:0|[1-9][0-9]*)$/
    : new RegExp(`^(?:0|[1-9][0-9]*)(?:\\.[0-9]{0,${decimals}})?$`);
  if (!pattern.test(text)) return null;
  const [whole, fraction = ''] = text.split('.');
  const scale = 10n ** BigInt(decimals);
  const atoms = (BigInt(whole) * scale)
    + BigInt(fraction.padEnd(decimals, '0') || '0');
  return atoms > 0n ? atoms : null;
}

export function formatNavcoinUnits(value, decimals = 6, maximumFractionDigits = decimals) {
  if (!Number.isSafeInteger(decimals) || decimals < 0 || decimals > 18) return '—';
  const atoms = nonNegativeBigInt(value);
  if (atoms === null) return '—';
  const scale = 10n ** BigInt(decimals);
  const whole = atoms / scale;
  const fraction = (atoms % scale).toString().padStart(decimals, '0');
  const shown = fraction.slice(0, Math.max(0, Math.min(decimals, maximumFractionDigits))).replace(/0+$/, '');
  return `${whole.toLocaleString('en-US')}${shown ? `.${shown}` : ''}`;
}

export function formatNavcoinNav(navUsdE8) {
  const nav = nonNegativeBigInt(navUsdE8);
  if (nav === null) return '—';
  const whole = nav / NAVCOIN_NAV_SCALE;
  const fraction = (nav % NAVCOIN_NAV_SCALE).toString().padStart(8, '0').replace(/0+$/, '');
  return `$${whole.toLocaleString('en-US')}${fraction ? `.${fraction}` : ''}`;
}

export function deriveNavcoinIssueQuote(
  amountAtoms,
  navUsdE8,
  multiplierBps,
  nativeDecimals = 6,
  settlementDecimals = 6,
) {
  const amount = positiveBigInt(amountAtoms);
  const nav = positiveBigInt(navUsdE8);
  const multiplier = positiveBigInt(multiplierBps);
  const scales = assetScales(nativeDecimals, settlementDecimals);
  if (!amount || !nav || !multiplier || !scales) return null;
  const baseReserveAtoms = divideRoundUp(
    amount * nav * scales.settlement,
    scales.native * NAVCOIN_NAV_SCALE,
  );
  const settlementAtoms = divideRoundUp(baseReserveAtoms * multiplier, NAVCOIN_BPS_SCALE);
  return {
    amountAtoms: amount.toString(),
    baseReserveAtoms: baseReserveAtoms.toString(),
    settlementAtoms: settlementAtoms.toString(),
    spreadAtoms: (settlementAtoms - baseReserveAtoms).toString(),
  };
}

export function deriveNavcoinRedeemQuote(
  amountAtoms,
  navUsdE8,
  multiplierBps,
  nativeDecimals = 6,
  settlementDecimals = 6,
) {
  const amount = positiveBigInt(amountAtoms);
  const nav = positiveBigInt(navUsdE8);
  const multiplier = positiveBigInt(multiplierBps);
  const scales = assetScales(nativeDecimals, settlementDecimals);
  if (!amount || !nav || !multiplier || !scales) return null;
  const baseReserveAtoms = divideRoundUp(
    amount * nav * scales.settlement,
    scales.native * NAVCOIN_NAV_SCALE,
  );
  const settlementAtoms = (baseReserveAtoms * multiplier) / NAVCOIN_BPS_SCALE;
  return {
    amountAtoms: amount.toString(),
    baseReserveAtoms: baseReserveAtoms.toString(),
    settlementAtoms: settlementAtoms.toString(),
    spreadAtoms: (baseReserveAtoms - settlementAtoms).toString(),
  };
}

export function evaluateNavcoinResidentMarket({
  market = null,
  supplyStatus = null,
  navStatus = null,
  chainStatus = null,
  direction = 'issue',
  amountAtoms = null,
  settlementBalanceAtoms = null,
  navcoinBalanceAtoms = null,
} = {}) {
  const status = objectValue(supplyStatus);
  const nav = objectValue(navStatus);
  const chain = objectValue(chainStatus);
  const errors = [];
  const amount = positiveBigInt(amountAtoms);
  const currentHeight = nonNegativeBigInt(chain.block_height);
  const validFrom = nonNegativeBigInt(status.policy_valid_from_height);
  const expiresAt = positiveBigInt(status.policy_expires_at_height);
  const expected = objectValue(market);
  const navSymbol = expected.symbol || 'NAVCoin';
  const settlementSymbol = expected.settlementSymbol || 'settlement asset';

  if (!market) errors.push('governed NAVCoin market metadata is unavailable');
  requireEqual(errors, status.schema, NAVCOIN_ROUTE_SCHEMA, 'supply-status schema');
  requireEqual(errors, Number(status.route_schema_version), 2, 'route schema version');
  requirePin(errors, 'route id', status.route_id, expected.routeId);
  requirePin(errors, 'route config digest', status.route_config_digest, expected.routeConfigDigest);
  requirePin(errors, `native ${navSymbol} asset`, status.native_nav_asset_id, expected.navAssetId);
  requirePin(errors, `${settlementSymbol} settlement asset`, status.settlement_asset_id, expected.settlementAssetId);
  requirePin(errors, 'handoff controller', status.handoff_controller, expected.handoffController);
  requirePin(errors, 'wrapped token', status.wrapped_navcoin_token, expected.wrappedToken);
  requireEqual(errors, Number(status.ethereum_chain_id), Number(expected.ethereumChainId), 'Ethereum chain');
  requireEqual(errors, String(status.route_trust_class || ''), String(expected.routeTrustClass || ''), 'route trust class');
  if (positiveBigInt(status.issue_multiplier_bps) === null) errors.push('issue multiplier is missing');
  if (positiveBigInt(status.redeem_multiplier_bps) === null) errors.push('redeem multiplier is missing');
  if (status.live_value_enabled !== true) errors.push('primary market live value is disabled');
  if (status.paused !== false) errors.push('primary market is paused');
  if (status.invariant_holds !== true) errors.push('route supply invariant is not satisfied');
  if (!HASH48_RE.test(String(status.policy_hash || ''))) errors.push('policy hash is missing or malformed');
  if (!HASH48_RE.test(String(status.pricing_reserve_packet_hash || ''))) {
    errors.push('pricing reserve packet hash is missing or malformed');
  }

  if (nav.schema !== 'postfiat-vault-bridge-status-v1') errors.push('live NAV status schema is invalid');
  requirePin(errors, 'NAV asset', nav.asset_id, expected.navAssetId);
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
  if (amount === null) errors.push(`enter a positive ${navSymbol} amount`);

  const minimum = positiveBigInt(status.min_order_atoms);
  const maximum = positiveBigInt(status.max_order_atoms);
  if (amount && minimum && amount < minimum) errors.push('amount is below the market minimum');
  if (amount && maximum && amount > maximum) errors.push('amount exceeds the market maximum');

  const issueQuote = deriveNavcoinIssueQuote(
    amount,
    nav.nav_per_unit,
    status.issue_multiplier_bps,
    expected.decimals,
    expected.settlementDecimals,
  );
  const redeemQuote = deriveNavcoinRedeemQuote(
    amount,
    nav.nav_per_unit,
    status.redeem_multiplier_bps,
    expected.decimals,
    expected.settlementDecimals,
  );
  if (amount !== null && amount > JSON_SAFE_INTEGER_MAX) {
    errors.push('amount exceeds the browser signing integer limit');
  }
  const selectedQuote = direction === 'redeem' ? redeemQuote : issueQuote;
  if (selectedQuote
    && BigInt(selectedQuote.settlementAtoms) > JSON_SAFE_INTEGER_MAX) {
    errors.push('settlement amount exceeds the browser signing integer limit');
  }
  if (direction === 'issue') {
    const available = nonNegativeBigInt(status.available_issue_atoms);
    const capacity = nonNegativeBigInt(status.issue_capacity_remaining_atoms);
    const supplyCap = nonNegativeBigInt(status.supply_cap_remaining_atoms);
    if (amount && (available === null || amount > available)) errors.push('available issue inventory is insufficient');
    if (amount && (capacity === null || amount > capacity)) errors.push('issue capacity is insufficient');
    if (amount && (supplyCap === null || amount > supplyCap)) errors.push('route supply capacity is insufficient');
    const balance = nonNegativeBigInt(settlementBalanceAtoms);
    if (issueQuote && (balance === null || balance < BigInt(issueQuote.settlementAtoms))) {
      errors.push(`wallet ${settlementSymbol} balance is insufficient`);
    }
  } else if (direction === 'redeem') {
    const available = nonNegativeBigInt(status.available_redeem_atoms);
    const capacity = nonNegativeBigInt(status.redeem_capacity_remaining_atoms);
    const balance = nonNegativeBigInt(navcoinBalanceAtoms);
    if (amount && (available === null || amount > available)) errors.push('available redemption inventory is insufficient');
    if (amount && (capacity === null || amount > capacity)) errors.push('redemption capacity is insufficient');
    if (amount && (balance === null || amount > balance)) errors.push(`wallet ${navSymbol} balance is insufficient`);
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

export function buildNavcoinIssueOperations({
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
      route_epoch: safeJsonInteger(status.route_epoch, 'route epoch'),
      policy_epoch: safeJsonInteger(status.policy_epoch, 'policy epoch'),
      policy_hash: status.policy_hash,
      mint_amount_atoms: safeJsonInteger(amount, 'mint amount'),
      max_settlement_value_atoms: safeJsonInteger(settlement, 'maximum settlement amount'),
      expires_at_height: safeJsonInteger(reservationExpiry, 'reservation expiry'),
    },
    subscribe: {
      operation: 'pftl_uniswap_primary_subscribe_v2',
      subscriber: walletAddress,
      ...common,
      subscription_nonce: subscriptionNonce,
      settlement_asset_id: status.settlement_asset_id,
      settlement_value_atoms: safeJsonInteger(settlement, 'settlement amount'),
      pricing_nav_epoch: safeJsonInteger(status.pricing_nav_epoch, 'pricing NAV epoch'),
      pricing_reserve_packet_hash: status.pricing_reserve_packet_hash,
    },
    release: {
      operation: 'pftl_uniswap_order_release',
      releaser: walletAddress,
      ...common,
    },
  };
}

export function buildNavcoinIssueExportDraft({
  walletAddress,
  ethereumRecipient,
  supplyStatus,
  chainHeight,
  amountAtoms,
  settlementAtoms,
  reservationId = randomLowerHex(48),
  subscriptionNonce = randomLowerHex(32),
  packetHash = randomLowerHex(48),
  exportNonce = randomLowerHex(32),
  destinationDeadlineSeconds = Math.floor(Date.now() / 1000) + 86_400,
  refundDelayBlocks = 100,
} = {}) {
  const status = objectValue(supplyStatus);
  const issue = buildNavcoinIssueOperations({
    walletAddress,
    ethereumRecipient,
    supplyStatus: status,
    chainHeight,
    amountAtoms,
    settlementAtoms,
    reservationId,
    subscriptionNonce,
  });
  if (!HASH48_RE.test(packetHash)) throw new Error('export packet hash is malformed');
  if (!/^[0-9a-f]{64}$/.test(exportNonce)) throw new Error('export nonce is malformed');
  if (!Number.isSafeInteger(destinationDeadlineSeconds) || destinationDeadlineSeconds <= Math.floor(Date.now() / 1000) + 3600) {
    throw new Error('Ethereum destination deadline must leave at least one hour');
  }
  if (!Number.isSafeInteger(refundDelayBlocks) || refundDelayBlocks <= 0) {
    throw new Error('refund delay must be a positive block count');
  }
  const mintPacket = {
    route_config_digest: status.route_config_digest,
    source_packet_hash: packetHash,
    reservation_id: reservationId,
    source_receipt_hash: '00'.repeat(48),
    source_receipt_root: '00'.repeat(48),
    settlement_asset_id: status.settlement_asset_id,
    native_nav_asset_id: status.native_nav_asset_id,
    pricing_reserve_packet_hash: status.pricing_reserve_packet_hash,
    policy_hash_commitment: '',
    route_epoch: safeJsonInteger(status.route_epoch, 'route epoch'),
    pricing_nav_epoch: safeJsonInteger(status.pricing_nav_epoch, 'pricing NAV epoch'),
    deadline_seconds: destinationDeadlineSeconds,
    nonce: exportNonce,
    destination_chain_id: safeJsonInteger(status.ethereum_chain_id, 'destination chain id'),
    destination_controller: String(status.handoff_controller || '').toLowerCase(),
    wrapped_token: String(status.wrapped_navcoin_token || '').toLowerCase(),
    ethereum_recipient: ethereumRecipient,
    mint_amount_atoms: safeJsonInteger(amountAtoms, 'mint amount'),
    settlement_value_atoms: safeJsonInteger(settlementAtoms, 'settlement amount'),
  };
  return {
    ...issue,
    packetHash,
    exportNonce,
    policyHash: status.policy_hash,
    mintPacket,
    destinationDeadlineSeconds,
    refundDelayBlocks,
  };
}

export function finalizeNavcoinIssueExportOperations(draft, prepared) {
  const packet = objectValue(prepared?.packet);
  const digest = String(prepared?.packet_digest || '');
  if (prepared?.schema !== 'postfiat-wallet-pftl-uniswap-mint-packet-v1') {
    throw new Error('wallet mint-packet preparation returned an unexpected schema');
  }
  if (!/^[0-9a-f]{64}$/.test(digest)) throw new Error('wallet mint-packet digest is malformed');
  if (!/^[0-9a-f]{64}$/.test(String(packet.policy_hash_commitment || ''))) {
    throw new Error('wallet policy commitment is malformed');
  }
  for (const [field, expected] of Object.entries(draft.mintPacket)) {
    if (field === 'policy_hash_commitment') continue;
    if (String(packet[field]) !== String(expected)) {
      throw new Error(`wallet mint-packet ${field} changed during local preparation`);
    }
  }
  return {
    ...draft,
    mintPacket: packet,
    packetDigest: digest,
    export: {
      operation: 'pftl_uniswap_export_debit',
      owner: draft.reserve.subscriber,
      route_id: draft.reserve.route_id,
      packet_hash: draft.packetHash,
      export_nonce: draft.exportNonce,
      ethereum_recipient: draft.reserve.ethereum_recipient,
      amount_atoms: draft.reserve.mint_amount_atoms,
      reservation_id: draft.reservationId,
      settlement_value_atoms: draft.subscribe.settlement_value_atoms,
      destination_deadline_seconds: draft.destinationDeadlineSeconds,
      refund_delay_blocks: draft.refundDelayBlocks,
      ethereum_packet_digest: digest,
      ethereum_packet_schema_version: 2,
    },
  };
}

export function buildNavcoinRedeemOperation({
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
    nav_amount_atoms: safeJsonInteger(amount, 'NAVCoin redemption amount'),
    min_settlement_value_atoms: safeJsonInteger(settlement, 'minimum settlement amount'),
    route_epoch: safeJsonInteger(status.route_epoch, 'route epoch'),
    policy_epoch: safeJsonInteger(status.policy_epoch, 'policy epoch'),
    policy_hash: status.policy_hash,
    pricing_nav_epoch: safeJsonInteger(status.pricing_nav_epoch, 'pricing NAV epoch'),
    pricing_reserve_packet_hash: status.pricing_reserve_packet_hash,
    expires_at_height: safeJsonInteger(expiry, 'redemption expiry'),
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

function safeJsonInteger(value, field) {
  const parsed = nonNegativeBigInt(value);
  if (parsed === null || parsed > JSON_SAFE_INTEGER_MAX) {
    throw new Error(`${field} exceeds the browser signing integer limit`);
  }
  return Number(parsed);
}

function divideRoundUp(value, divisor) {
  return (value + divisor - 1n) / divisor;
}

function assetScales(nativeDecimals, settlementDecimals) {
  if (!Number.isSafeInteger(nativeDecimals) || nativeDecimals < 0 || nativeDecimals > 18
    || !Number.isSafeInteger(settlementDecimals) || settlementDecimals < 0 || settlementDecimals > 18) {
    return null;
  }
  return {
    native: 10n ** BigInt(nativeDecimals),
    settlement: 10n ** BigInt(settlementDecimals),
  };
}

function requireEqual(errors, actual, expected, label) {
  if (actual !== expected) errors.push(`${label} does not match the governed NAVCoin route`);
}

function requirePin(errors, label, actual, expected) {
  if (!expected) {
    errors.push(`${label} production pin is missing`);
  } else if (String(actual || '').toLowerCase() !== String(expected).toLowerCase()) {
    errors.push(`${label} does not match the frozen production manifest`);
  }
}
