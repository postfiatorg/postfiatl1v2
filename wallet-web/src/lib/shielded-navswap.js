export const SHIELDED_NAVSWAP_ROUTE = 'shielded_navswap';
export const ASSET_ORCHARD_SWAP_ACTION_SCHEMA = 'postfiat-asset-orchard-swap-action-v1';
export const ASSET_ORCHARD_POOL_ID = 'asset-orchard-v1';
export const ASSET_ORCHARD_SWAP_CIRCUIT_K = 15;
export const ASSET_ORCHARD_INGRESS_FILE_SCHEMA = 'postfiat-asset-orchard-ingress-file-v2';
export const ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA = 'postfiat-asset-orchard-private-egress-file-v1';
export const ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA = 'postfiat-asset-orchard-private-egress-action-v1';
export const SHIELDED_NAVSWAP_EGRESS_POLICY_ID = 'wallet_private_egress_public_exit_v1';
const MAX_SERIALIZED_JSON_INSPECTION_BYTES = 1_048_576;

export const SHIELDED_NAVSWAP_NOTE_STATES = Object.freeze([
  'pending',
  'spendable',
  'locked_for_swap',
  'locked_for_egress',
  'spent',
  'egressed',
  'unknown',
]);

export const SHIELDED_NAVSWAP_P9_STATUS = Object.freeze({
  step: 'P9',
  status: 'explicit_public_exit_required',
  copy: 'Private swap outputs stay private by default. Public exit requires a separate acknowledgement and receipt before bridge-out is available.',
});

export const SHIELDED_PRIVATE_MATERIAL_KEYS = Object.freeze([
  'backup',
  'backup_json',
  'decrypted_backup',
  'key_file',
  'note_file',
  'note_files',
  'note_opening',
  'note_openings',
  'opening',
  'passphrase',
  'private_key',
  'secret_key',
  'seed',
  'seed_phrase',
  'spend_authority',
  'spend_authorization_key',
  'spend_key',
  'spending_key',
]);

const PRIVATE_KEY_PATTERNS = [
  /(^|_)backup(_json)?$/,
  /(^|_)decrypted_backup$/,
  /(^|_)key_file$/,
  /(^|_)mnemonic$/,
  /(^|_)note_file(s)?$/,
  /(^|_)note_opening(s)?$/,
  /(^|_)passphrase$/,
  /(^|_)private_key$/,
  /(^|_)secret_key$/,
  /(^|_)seed(_phrase|_hex)?$/,
  /(^|_)spend(_|$)/,
  /(^|_)spending_key$/,
  /^(diversifier|g_d|pk_d|rho|psi|rcm|nk|rivk|rseed|spend_auth_signing_key|full_viewing_key(_hex)?)$/,
];

const CLEAR_ASSET_ORCHARD_ACTION_KEYS = new Set([
  'amount',
  'amount_atoms',
  'asset_id',
  'asset_tag',
  'asset_tag_hi',
  'asset_tag_lo',
  'diversifier',
  'full_viewing_key',
  'full_viewing_key_hex',
  'g_d',
  'input_note',
  'input_notes',
  'note',
  'note_opening',
  'note_openings',
  'nk',
  'output_note',
  'output_notes',
  'pk_d',
  'psi',
  'rcm',
  'rho',
  'rivk',
  'rseed',
  'spend_auth_signing_key',
  'spend_key',
  'spending_key',
]);

function normalizeKey(key) {
  return String(key || '')
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/[^A-Za-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toLowerCase();
}

function isPlainObject(value) {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function stringField(obj, names, fallback = '') {
  for (const name of names) {
    const value = obj?.[name];
    if (value !== undefined && value !== null && String(value).trim()) {
      return String(value).trim();
    }
  }
  return fallback;
}

function boolField(obj, names, fallback = false) {
  for (const name of names) {
    const value = obj?.[name];
    if (value === true || value === false) return value;
    if (typeof value === 'string') {
      const normalized = value.trim().toLowerCase();
      if (normalized === 'true') return true;
      if (normalized === 'false') return false;
    }
  }
  return fallback;
}

function integerField(obj, names) {
  for (const name of names) {
    const value = obj?.[name];
    if (value === undefined || value === null || value === '') continue;
    const parsed = Number.parseInt(String(value), 10);
    if (Number.isSafeInteger(parsed) && parsed >= 0) return parsed;
    return null;
  }
  return null;
}

function arrayField(obj, names) {
  for (const name of names) {
    const value = obj?.[name];
    if (Array.isArray(value)) return value;
  }
  return [];
}

function stableJson(value) {
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(stableJson).join(',')}]`;
  return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${stableJson(value[key])}`).join(',')}}`;
}

function forbiddenPrivateKey(key) {
  const normalized = normalizeKey(key);
  if (SHIELDED_PRIVATE_MATERIAL_KEYS.includes(normalized)) return true;
  return PRIVATE_KEY_PATTERNS.some(pattern => pattern.test(normalized));
}

export function findShieldedPrivateMaterial(value, path = '$', seen = new WeakSet(), depth = 0) {
  const hits = [];
  if (depth > 32) return [{ path, key: 'inspection_depth_exceeded' }];
  if (typeof value === 'string') {
    const trimmed = value.trim();
    if (['{', '['].includes(trimmed[0])) {
      if (trimmed.length > MAX_SERIALIZED_JSON_INSPECTION_BYTES) {
        return [{ path, key: 'serialized_json_inspection_limit_exceeded' }];
      }
      try {
        return findShieldedPrivateMaterial(JSON.parse(trimmed), `${path}<json>`, seen, depth + 1);
      } catch (_) {
        return hits;
      }
    }
    return hits;
  }
  if (!value || typeof value !== 'object') return hits;
  if (seen.has(value)) return hits;
  seen.add(value);

  if (Array.isArray(value)) {
    value.forEach((item, index) => {
      hits.push(...findShieldedPrivateMaterial(item, `${path}[${index}]`, seen, depth + 1));
    });
    return hits;
  }

  for (const [key, child] of Object.entries(value)) {
    const childPath = `${path}.${key}`;
    const normalized = normalizeKey(key);
    const publicSpendAuthorization = [
      'spend_authorization_signature',
      'spend_authorization_signatures',
    ].includes(normalized);
    if (!publicSpendAuthorization && forbiddenPrivateKey(key)) {
      hits.push({ path: childPath, key });
    }
    hits.push(...findShieldedPrivateMaterial(child, childPath, seen, depth + 1));
  }
  return hits;
}

export function assertNoShieldedPrivateMaterial(value, label = 'shielded NAVSwap request') {
  const hits = findShieldedPrivateMaterial(value);
  if (hits.length) {
    const first = hits[0];
    throw new Error(`${label} contains forbidden private wallet material at ${first.path}`);
  }
}

export function isShieldedNavswapRequest(path, body = null) {
  if (String(path || '').startsWith('/api/shielded-nav-swap/')) return true;
  return body?.route === SHIELDED_NAVSWAP_ROUTE || body?.route_id === SHIELDED_NAVSWAP_ROUTE;
}

export function normalizeShieldedAssetRegistry(capability = {}) {
  const rawAssets = arrayField(capability, [
    'asset_registry',
    'assetRegistry',
    'assets',
    'supported_assets',
    'supportedAssets',
  ]);
  return rawAssets.map((asset, index) => {
    const row = isPlainObject(asset) ? asset : {};
    const symbol = stringField(row, ['symbol', 'ticker', 'code', 'asset']).toLowerCase();
    const assetId = stringField(row, ['asset_id', 'assetId', 'id']);
    const precision = integerField(row, ['precision', 'decimals', 'scale']);
    const issuer = stringField(row, ['issuer', 'issuer_address', 'issuerAddress']);
    const navSource = stringField(row, ['nav_source', 'navSource', 'nav_epoch_source', 'price_source']);
    const policyHash = stringField(row, ['policy_hash', 'policyHash', 'nav_policy_hash', 'navPolicyHash']);
    const missing = [];
    if (!symbol) missing.push('symbol');
    if (!assetId) missing.push('asset_id');
    if (precision === null) missing.push('precision');
    if (!issuer) missing.push('issuer');
    if (!navSource) missing.push('nav_source');
    if (!policyHash) missing.push('policy_hash');
    return {
      index,
      symbol,
      asset_id: assetId,
      precision,
      issuer,
      nav_source: navSource,
      policy_hash: policyHash,
      supported: boolField(row, ['supported', 'tradeable', 'enabled'], false),
      display_only: boolField(row, ['display_only', 'displayOnly'], true),
      missing,
      ok: missing.length === 0,
    };
  });
}

function registryLookup(registry) {
  const byKey = new Map();
  for (const asset of registry) {
    if (asset.symbol) byKey.set(asset.symbol.toLowerCase(), asset);
    if (asset.asset_id) byKey.set(asset.asset_id.toLowerCase(), asset);
  }
  return byKey;
}

export function normalizeShieldedSupportedPairs(capability = {}, registry = normalizeShieldedAssetRegistry(capability)) {
  const lookup = registryLookup(registry);
  return arrayField(capability, ['supported_pairs', 'supportedPairs', 'pairs']).map((pair, index) => {
    const row = isPlainObject(pair) ? pair : {};
    const fromKey = stringField(row, ['from_asset', 'fromAsset', 'from', 'base']).toLowerCase();
    const toKey = stringField(row, ['to_asset', 'toAsset', 'to', 'quote']).toLowerCase();
    const fromAsset = lookup.get(fromKey) || null;
    const toAsset = lookup.get(toKey) || null;
    const errors = [];
    if (!fromAsset) errors.push('from asset is not in registry');
    if (!toAsset) errors.push('to asset is not in registry');
    if (fromAsset && !fromAsset.ok) errors.push(`${fromAsset.symbol || fromKey} registry entry is incomplete`);
    if (toAsset && !toAsset.ok) errors.push(`${toAsset.symbol || toKey} registry entry is incomplete`);
    if (fromAsset && fromAsset.supported !== true) errors.push(`${fromAsset.symbol} is display-only`);
    if (toAsset && toAsset.supported !== true) errors.push(`${toAsset.symbol} is display-only`);
    if (boolField(row, ['enabled', 'supported'], false) !== true) errors.push('pair is not enabled by adapter');
    return {
      index,
      from: fromAsset?.symbol || fromKey,
      to: toAsset?.symbol || toKey,
      from_asset_id: fromAsset?.asset_id || '',
      to_asset_id: toAsset?.asset_id || '',
      liquidity_mode: stringField(row, ['liquidity_mode', 'liquidityMode']),
      ok: errors.length === 0,
      errors,
    };
  });
}

const SHIELDED_NAVSWAP_LIQUIDITY_MODES = new Set([
  'bilateral_rfq',
  'operator_inventory',
  'pool_managed_note',
  'issuer_reserve_source',
]);

export function normalizeShieldedNavswapQuote(rawQuote = {}, nowMs = Date.now()) {
  const quote = isPlainObject(rawQuote) ? rawQuote : {};
  const liquidity = isPlainObject(quote.liquidity) ? quote.liquidity : {};
  const mode = stringField(liquidity, ['mode', 'liquidity_mode'], stringField(quote, ['liquidity_mode', 'liquidityMode']));
  const commitment = stringField(liquidity, ['commitment', 'liquidity_commitment'], stringField(quote, ['liquidity_commitment', 'liquidityCommitment']));
  const commitmentStatus = stringField(liquidity, ['commitment_status', 'commitmentStatus'], stringField(quote, ['liquidity_commitment_status', 'liquidityCommitmentStatus']));
  const policyHash = stringField(quote, ['policy_hash', 'policyHash']);
  const quoteBindingHash = stringField(quote, ['quote_binding_hash', 'quoteBindingHash']);
  const generatedAtMs = integerField(quote, ['quote_generated_at_ms', 'quoteGeneratedAtMs']);
  const expiresAtMs = integerField(quote, ['quote_expires_at_ms', 'quoteExpiresAtMs']);
  const missing = [];
  if (quote.schema && quote.schema !== 'postfiat-shielded-navswap-quote-v1') missing.push('schema');
  if (quote.ok !== true) missing.push('ok');
  if (!SHIELDED_NAVSWAP_LIQUIDITY_MODES.has(mode)) missing.push('liquidity_mode');
  if (!/^[0-9a-f]{64}$/.test(commitment) && !/^[0-9a-f]{96}$/.test(commitment)) missing.push('liquidity_commitment');
  if (commitmentStatus !== 'live') missing.push('liquidity_commitment_live');
  if (!/^[0-9a-f]{64}$/.test(policyHash)) missing.push('policy_hash');
  if (!/^[0-9a-f]{64}$/.test(quoteBindingHash)) missing.push('quote_binding_hash');
  if (!stringField(quote, ['output_amount_atoms', 'expected_output', 'outputAmountAtoms'])) missing.push('output_amount_atoms');
  if (expiresAtMs === null) missing.push('quote_expires_at_ms');
  const expired = expiresAtMs !== null && expiresAtMs <= nowMs;
  if (expired) missing.push('quote_not_expired');
  return {
    ok: quote.ok === true,
    ready: quote.ok === true && missing.length === 0,
    schema: quote.schema || '',
    status: stringField(quote, ['status'], quote.ok === true ? 'quote_ready_submit_disabled' : 'quote_unavailable'),
    message: stringField(quote, ['message']),
    from_asset: stringField(quote, ['from_asset', 'fromAsset']),
    to_asset: stringField(quote, ['to_asset', 'toAsset']),
    input_amount_atoms: stringField(quote, ['input_amount_atoms', 'amount_atoms', 'inputAmountAtoms', 'amountAtoms']),
    output_amount_atoms: stringField(quote, ['output_amount_atoms', 'expected_output', 'outputAmountAtoms']),
    minimum_output_atoms: stringField(quote, ['minimum_output_atoms', 'minimumOutputAtoms']),
    price_model: stringField(quote, ['price_model', 'priceModel']),
    quote_generated_at_ms: generatedAtMs,
    quote_expires_at_ms: expiresAtMs,
    expires_in_ms: expiresAtMs === null ? null : Math.max(0, expiresAtMs - nowMs),
    expired,
    liquidity: {
      mode,
      mode_label: stringField(liquidity, ['mode_label', 'modeLabel'], mode),
      source_class: stringField(liquidity, ['source_class', 'sourceClass'], mode),
      trust_class: stringField(liquidity, ['trust_class', 'trustClass'], 'CONTROLLED'),
      counterparty: stringField(liquidity, ['counterparty']),
      commitment,
      commitment_status: commitmentStatus,
      copy: stringField(liquidity, ['copy']),
    },
    policy_hash: policyHash,
    quote_binding_hash: quoteBindingHash,
    failure_mode: stringField(quote, ['failure_mode', 'failureMode']),
    next_gate: stringField(quote, ['next_gate', 'nextGate'], 'Step 7 private swap submit'),
    can_prove: quote.can_prove === true,
    can_run: quote.can_run === true,
    submit_enabled: quote.submit_enabled === true,
    missing,
    raw: quote,
  };
}

export function normalizeLocalProverReadiness(payload = {}) {
  const readiness = isPlainObject(payload) ? payload : {};
  const circuitId = stringField(readiness, ['circuit_id', 'circuitId']);
  const poolId = stringField(readiness, ['pool_id', 'poolId'], ASSET_ORCHARD_POOL_ID);
  const k = integerField(readiness, ['k', 'circuit_k']);
  const paramsHash = stringField(readiness, ['params_hash', 'paramsHash']);
  const vkHash = stringField(readiness, ['vk_hash', 'vkHash']);
  const provingKeyHash = stringField(readiness, ['proving_key_hash', 'provingKeyHash', 'pk_hash']);
  const missing = [];
  if (!poolId) missing.push('pool_id');
  if (!circuitId) missing.push('circuit_id');
  if (k !== ASSET_ORCHARD_SWAP_CIRCUIT_K) missing.push('k=15');
  if (!paramsHash) missing.push('params_hash');
  if (!vkHash) missing.push('vk_hash');
  if (boolField(readiness, ['local_only', 'localOnly'], false) !== true) missing.push('local_only');
  return {
    local_only: boolField(readiness, ['local_only', 'localOnly'], false),
    ready: boolField(readiness, ['ready'], false) && missing.length === 0,
    status: stringField(readiness, ['status'], missing.length ? 'not_ready' : 'ready'),
    pool_id: poolId,
    circuit_id: circuitId,
    k,
    params_hash: paramsHash,
    vk_hash: vkHash,
    proving_key_hash: provingKeyHash,
    missing,
  };
}

export function shieldedPrivateEgressDisclosureFields({
  walletAddress,
  to,
  assetId,
  amountAtoms,
  noteCommitment = '',
  policyId = SHIELDED_NAVSWAP_EGRESS_POLICY_ID,
} = {}) {
  return {
    schema: 'postfiat-shielded-navswap-private-egress-disclosure-v1',
    route: SHIELDED_NAVSWAP_ROUTE,
    action: 'private_egress_public_exit',
    wallet_address: String(walletAddress || '').trim(),
    destination: String(to || '').trim(),
    asset_id: String(assetId || '').trim().toLowerCase(),
    amount_atoms: String(amountAtoms || ''),
    note_commitment: String(noteCommitment || '').trim().toLowerCase(),
    policy_id: String(policyId || '').trim(),
    visible_after_submit: ['destination', 'asset_id', 'amount_atoms', 'receipt_timing'],
    stays_private: ['note_opening', 'spend_authority', 'wallet_local_note_file'],
  };
}

export async function shieldedPrivateEgressDisclosureHash(disclosure, { cryptoImpl = globalThis.crypto } = {}) {
  const cryptoApi = getCrypto(cryptoImpl);
  const bytes = new TextEncoder().encode(stableJson(disclosure));
  const digest = new Uint8Array(await cryptoApi.subtle.digest('SHA-256', bytes));
  return Array.from(digest, byte => byte.toString(16).padStart(2, '0')).join('');
}

export class LocalAssetOrchardProverClient {
  constructor({
    baseUrl = 'http://127.0.0.1:8789',
    fetchImpl = globalThis.fetch,
  } = {}) {
    assertLocalProverUrl(baseUrl);
    if (typeof fetchImpl !== 'function') throw new Error('Local prover fetch implementation is required');
    this.baseUrl = String(baseUrl).replace(/\/+$/, '');
    this.fetchImpl = fetchImpl;
  }

  async readiness() {
    const payload = await this._request('GET', '/asset-orchard/readiness');
    return normalizeLocalProverReadiness(payload);
  }

  async buildSwapAction(body, expected = {}) {
    assertNoShieldedPrivateMaterial(body, 'local Asset-Orchard prover request');
    const payload = await this._request('POST', '/asset-orchard/swap-actions', body);
    const actionJson = typeof payload?.action_json === 'string'
      ? payload.action_json
      : payload?.action
        ? JSON.stringify(payload.action)
        : JSON.stringify(payload);
    const action = payload?.action || JSON.parse(actionJson);
    const verification = verifyAssetOrchardSwapActionJson(action, expected);
    return {
      ok: true,
      swap_id: payload?.swap_id || null,
      action,
      action_json: actionJson,
      action_json_bytes: payload?.action_json_bytes || actionJson.length,
      verification,
      vault_update: payload?.vault_update || null,
      readiness: payload?.readiness ? normalizeLocalProverReadiness(payload.readiness) : null,
    };
  }

  async listNotes() {
    const payload = await this._request('GET', '/asset-orchard/notes');
    return Array.isArray(payload?.notes) ? payload.notes : [];
  }

  async finalizeSwap(body) {
    assertNoShieldedPrivateMaterial(body, 'local Asset-Orchard swap finalize request');
    return this._request('POST', '/asset-orchard/swap-finalize', body);
  }

  async buildPrivateEgressAction(body, expected = {}) {
    assertNoShieldedPrivateMaterial(body, 'local Asset-Orchard private egress request');
    const payload = await this._request('POST', '/asset-orchard/private-egress-actions', body);
    const egressJson = typeof payload?.egress_json === 'string'
      ? payload.egress_json
      : payload?.egress
        ? JSON.stringify(payload.egress)
        : JSON.stringify(payload);
    const verification = verifyAssetOrchardPrivateEgressJson(egressJson, expected);
    return {
      ok: true,
      egress_id: payload?.egress_id || null,
      egress_json: egressJson,
      egress_json_bytes: payload?.egress_json_bytes || egressJson.length,
      verification,
      vault_update: payload?.vault_update || null,
      readiness: payload?.readiness ? normalizeLocalProverReadiness(payload.readiness) : null,
    };
  }

  async finalizePrivateEgress(body) {
    assertNoShieldedPrivateMaterial(body, 'local Asset-Orchard private egress finalize request');
    return this._request('POST', '/asset-orchard/private-egress-finalize', body);
  }

  async buildIngressNote(body) {
    assertNoShieldedPrivateMaterial(body, 'local Asset-Orchard ingress note request');
    const payload = await this._request('POST', '/asset-orchard/ingress-notes', body);
    if (!payload?.wallet_note) {
      throw new Error('Local prover did not return a wallet note');
    }
    return payload;
  }

  async _request(method, path, body = undefined) {
    const options = {
      method,
      headers: { Accept: 'application/json' },
    };
    if (body !== undefined) {
      options.headers['Content-Type'] = 'application/json';
      options.body = JSON.stringify(body);
    }
    const resp = await this.fetchImpl(`${this.baseUrl}${path}`, options);
    const payload = await resp.json();
    if (!resp.ok || payload?.ok === false) {
      throw new Error(payload?.message || payload?.error || `Local prover request failed: ${resp.status}`);
    }
    return payload;
  }
}

export function randomAssetOrchardNoteSeedHex({ cryptoImpl = globalThis.crypto } = {}) {
  const cryptoApi = getCrypto(cryptoImpl);
  const bytes = cryptoApi.getRandomValues(new Uint8Array(32));
  return Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
}

export function buildAssetOrchardIngressPayload({
  signedBurnTransaction,
  assetId,
  amountAtoms,
  walletNote,
  encryptedOutput,
}) {
  if (!signedBurnTransaction || typeof signedBurnTransaction !== 'object') {
    throw new Error('Signed burn transaction is required for Asset-Orchard ingress');
  }
  const outputCommitment = String(walletNote?.output_commitment || '');
  if (!/^[0-9a-f]{64}$/.test(outputCommitment)) {
    throw new Error('Asset-Orchard wallet note output commitment is invalid');
  }
  const amount = Number.parseInt(String(amountAtoms), 10);
  if (!Number.isSafeInteger(amount) || amount <= 0) {
    throw new Error('Asset-Orchard ingress amount must be a positive safe integer atom amount');
  }
  if (typeof encryptedOutput !== 'string' || encryptedOutput.length === 0) {
    throw new Error('Asset-Orchard ingress encrypted output is required from the local prover');
  }
  if (
    encryptedOutput.length % 2 !== 0
    || !/^[0-9a-f]+$/.test(encryptedOutput)
    || !encryptedOutput.startsWith('5046414f454e4331')
  ) {
    throw new Error('Asset-Orchard ingress encrypted output must be lowercase PFAOENC1 ciphertext hex');
  }
  return {
    burn_transaction: signedBurnTransaction,
    pool_id: ASSET_ORCHARD_POOL_ID,
    asset_id: String(assetId || '').toLowerCase(),
    amount,
    output_commitment: outputCommitment,
    encrypted_output: encryptedOutput,
  };
}

function assertLocalProverUrl(baseUrl) {
  let parsed;
  try {
    parsed = new URL(baseUrl);
  } catch (_) {
    throw new Error('Local prover URL is invalid');
  }
  const host = parsed.hostname.toLowerCase();
  const local = host === 'localhost' || host === '127.0.0.1' || host === '::1' || host === '[::1]';
  if (!local) throw new Error('Asset-Orchard prover must be local-only');
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    throw new Error('Asset-Orchard prover URL must be HTTP(S)');
  }
}

export function normalizeShieldedNavswapCapability(rawCapability = {}) {
  const capability = isPlainObject(rawCapability) ? rawCapability : {};
  const registry = normalizeShieldedAssetRegistry(capability);
  const supportedPairs = normalizeShieldedSupportedPairs(capability, registry);
  const canIngress = boolField(capability, ['can_ingress', 'canIngress'], false);
  const quote = normalizeShieldedNavswapQuote(capability.quote || {});
  const adapterCanQuote = capability.can_quote === true && supportedPairs.some(pair => pair.ok);
  const proverReadiness = normalizeLocalProverReadiness(
    capability.local_prover || capability.localProver || capability.prover_readiness || {},
  );
  const disabledReasons = [];
  if (capability.enabled !== true) {
    disabledReasons.push(stringField(capability, ['disabled_reason', 'reason'], 'adapter has not enabled shielded NAVSwap'));
  }
  if (!adapterCanQuote && canIngress !== true) {
    disabledReasons.push('adapter cannot quote shielded NAVSwap');
  }
  const adapterCanRun = capability.can_run === true;
  const egress = isPlainObject(capability.egress) ? capability.egress : null;
  const canEgress = capability.can_egress === true || egress?.enabled === true;
  if (boolField(capability, ['requires_local_prover', 'requiresLocalProver'], true) !== true) {
    disabledReasons.push('adapter did not declare the local prover boundary');
  }
  if (boolField(capability, ['requires_note_scan', 'requiresNoteScan'], true) !== true) {
    disabledReasons.push('adapter did not declare local note scanning');
  }

  return {
    route: SHIELDED_NAVSWAP_ROUTE,
    enabled: capability.enabled === true,
    can_quote: adapterCanQuote,
    can_run: adapterCanRun,
    adapter_can_quote: capability.can_quote === true,
    adapter_can_run: adapterCanRun,
    can_ingress: canIngress,
    can_egress: canEgress,
    bridge_out_requires_public_exit_receipt: boolField(capability, ['bridge_out_requires_public_exit_receipt', 'bridgeOutRequiresPublicExitReceipt'], true),
    status: adapterCanQuote
      ? stringField(capability, ['status'], 'step6_quote_ready')
      : canIngress
      ? stringField(capability, ['status'], 'step5_ingress_ready')
      : 'preflight_only',
    custody_boundary: stringField(capability, ['custody_boundary', 'custodyBoundary'], 'wallet_local_note_keys_only'),
    requires_local_prover: boolField(capability, ['requires_local_prover', 'requiresLocalProver'], true),
    requires_note_scan: boolField(capability, ['requires_note_scan', 'requiresNoteScan'], true),
    supported_pairs: supportedPairs,
    liquidity_mode: stringField(capability, ['liquidity_mode', 'liquidityMode'], quote.liquidity.mode || 'adapter_reported'),
    privacy_label: stringField(capability, ['privacy_label', 'privacyLabel'], 'Private, wallet-local custody'),
    disabled_reason: disabledReasons[0] || (adapterCanRun ? '' : 'private submit/run remain disabled until the Step 7 review gate'),
    reason: disabledReasons[0] || stringField(
      capability,
      ['reason'],
      adapterCanRun ? 'Private submit is available for the controlled Step 7 route.' : 'Private quote preview is available; private submit/run remain disabled.',
    ),
    asset_registry: registry,
    local_prover: proverReadiness,
    ingress: capability.ingress || null,
    swap: capability.swap || null,
    egress,
    quote,
    p9_status: capability.p9_status || SHIELDED_NAVSWAP_P9_STATUS,
    privacy: {
      label: stringField(capability.privacy || {}, ['label'], 'Private'),
      disclosure_label: stringField(
        capability.privacy || {},
        ['disclosure_label', 'disclosureLabel'],
        'Wallet address and route preflight only; note openings and spend authority stay local.',
      ),
    },
  };
}

export function normalizeShieldedCapabilitiesEnvelope(envelope = {}) {
  const caps = isPlainObject(envelope) ? { ...envelope } : {};
  const routes = isPlainObject(caps.routes) ? { ...caps.routes } : {};
  routes[SHIELDED_NAVSWAP_ROUTE] = normalizeShieldedNavswapCapability(routes[SHIELDED_NAVSWAP_ROUTE] || {});
  return {
    ...caps,
    routes,
  };
}

export function reconcileShieldedNotes(notes = [], statuses = []) {
  const statusMap = new Map();
  for (const status of statuses || []) {
    const key = status?.note_id || status?.note_ref || status?.id || status?.commitment;
    if (key) statusMap.set(String(key), status);
  }
  return (notes || []).map(note => {
    const key = note?.note_id || note?.note_ref || note?.id || note?.commitment;
    const status = key ? statusMap.get(String(key)) : null;
    if (!status) return { ...note };
    if (status.nullified === true || status.spent === true || status.state === 'spent') {
      return { ...note, state: 'spent', nullifier: status.nullifier || note.nullifier || null };
    }
    if (status.egressed === true || status.state === 'egressed') return { ...note, state: 'egressed' };
    if (status.confirmed === true && note.state === 'pending') return { ...note, state: 'spendable' };
    return { ...note, state: status.state || note.state || 'unknown' };
  });
}

export function spendableShieldedNotes(notes = []) {
  return (notes || []).filter(note => note?.state === 'spendable' && note?.nullified !== true);
}

function getCrypto(cryptoImpl = globalThis.crypto) {
  if (!cryptoImpl?.subtle || typeof cryptoImpl.getRandomValues !== 'function') {
    throw new Error('WebCrypto is required for the shielded note vault');
  }
  return cryptoImpl;
}

function bytesToBase64(bytes) {
  if (typeof Buffer !== 'undefined') return Buffer.from(bytes).toString('base64');
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function base64ToBytes(value) {
  if (typeof Buffer !== 'undefined') return new Uint8Array(Buffer.from(value, 'base64'));
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

export async function deriveShieldedNoteVaultKey(passphrase, { salt = null, cryptoImpl = globalThis.crypto } = {}) {
  if (!String(passphrase || '')) throw new Error('Shielded note vault passphrase is required');
  const cryptoApi = getCrypto(cryptoImpl);
  const saltBytes = salt ? base64ToBytes(salt) : cryptoApi.getRandomValues(new Uint8Array(16));
  const material = await cryptoApi.subtle.importKey(
    'raw',
    new TextEncoder().encode(String(passphrase)),
    'PBKDF2',
    false,
    ['deriveKey'],
  );
  const key = await cryptoApi.subtle.deriveKey(
    { name: 'PBKDF2', salt: saltBytes, iterations: 150000, hash: 'SHA-256' },
    material,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  );
  return { key, salt: bytesToBase64(saltBytes) };
}

export async function sealShieldedNoteVault(snapshot, { key, salt, cryptoImpl = globalThis.crypto } = {}) {
  const cryptoApi = getCrypto(cryptoImpl);
  if (!key) throw new Error('Shielded note vault key is required');
  const iv = cryptoApi.getRandomValues(new Uint8Array(12));
  const plaintext = new TextEncoder().encode(JSON.stringify({
    schema: 'postfiat-shielded-note-vault-plaintext-v1',
    saved_at_ms: Date.now(),
    ...snapshot,
  }));
  const ciphertext = new Uint8Array(await cryptoApi.subtle.encrypt({ name: 'AES-GCM', iv }, key, plaintext));
  return {
    schema: 'postfiat-shielded-note-vault-v1',
    cipher: 'AES-256-GCM',
    kdf: 'PBKDF2-SHA256',
    salt,
    iv: bytesToBase64(iv),
    ciphertext: bytesToBase64(ciphertext),
  };
}

export async function openShieldedNoteVault(envelope, { key, cryptoImpl = globalThis.crypto } = {}) {
  const cryptoApi = getCrypto(cryptoImpl);
  if (!key) throw new Error('Shielded note vault key is required');
  if (envelope?.schema !== 'postfiat-shielded-note-vault-v1') {
    throw new Error('Unsupported shielded note vault schema');
  }
  const plaintext = await cryptoApi.subtle.decrypt(
    { name: 'AES-GCM', iv: base64ToBytes(envelope.iv) },
    key,
    base64ToBytes(envelope.ciphertext),
  );
  return JSON.parse(new TextDecoder().decode(plaintext));
}

export class ShieldedNoteVault {
  constructor({ storage, namespace = 'postfiat:shielded-note-vault', key, cryptoImpl = globalThis.crypto }) {
    if (!storage || typeof storage.getItem !== 'function' || typeof storage.setItem !== 'function') {
      throw new Error('Shielded note vault storage must implement getItem/setItem');
    }
    this.storage = storage;
    this.namespace = namespace;
    this.key = key;
    this.cryptoImpl = cryptoImpl;
  }

  async load() {
    const encoded = this.storage.getItem(this.namespace);
    if (!encoded) return { schema: 'postfiat-shielded-note-vault-plaintext-v1', notes: [], keys: {}, p9_status: SHIELDED_NAVSWAP_P9_STATUS };
    return openShieldedNoteVault(JSON.parse(encoded), { key: this.key, cryptoImpl: this.cryptoImpl });
  }

  async save(snapshot) {
    const notes = Array.isArray(snapshot?.notes) ? snapshot.notes : [];
    for (const note of notes) {
      if (!SHIELDED_NAVSWAP_NOTE_STATES.includes(note?.state || 'unknown')) {
        throw new Error(`Unsupported shielded note state: ${note?.state}`);
      }
    }
    const envelope = await sealShieldedNoteVault({
      notes,
      keys: snapshot?.keys || {},
      p9_status: snapshot?.p9_status || SHIELDED_NAVSWAP_P9_STATUS,
    }, { key: this.key, salt: snapshot?.salt || null, cryptoImpl: this.cryptoImpl });
    this.storage.setItem(this.namespace, JSON.stringify(envelope));
    return envelope;
  }
}

function findCleartextActionKeys(value, path = '$', seen = new WeakSet()) {
  const hits = [];
  if (!value || typeof value !== 'object') return hits;
  if (seen.has(value)) return hits;
  seen.add(value);
  if (Array.isArray(value)) {
    value.forEach((item, index) => hits.push(...findCleartextActionKeys(item, `${path}[${index}]`, seen)));
    return hits;
  }
  for (const [key, child] of Object.entries(value)) {
    const normalized = normalizeKey(key);
    const publicSpendAuthSignatures = normalized === 'spend_authorization_signatures';
    if (
      CLEAR_ASSET_ORCHARD_ACTION_KEYS.has(normalized)
      || (!publicSpendAuthSignatures && forbiddenPrivateKey(key))
    ) {
      hits.push({ path: `${path}.${key}`, key });
    }
    hits.push(...findCleartextActionKeys(child, `${path}.${key}`, seen));
  }
  return hits;
}

export function verifyAssetOrchardSwapActionJson(actionJson, expected = {}) {
  const action = typeof actionJson === 'string' ? JSON.parse(actionJson) : actionJson;
  if (!isPlainObject(action)) throw new Error('Asset-Orchard swap action must be an object');
  const schema = action.schema || action.action_schema;
  if (schema !== ASSET_ORCHARD_SWAP_ACTION_SCHEMA) {
    throw new Error('Asset-Orchard swap action schema mismatch');
  }
  if (expected.chain_id && action.chain_id !== expected.chain_id) throw new Error('Asset-Orchard swap action chain_id mismatch');
  if (expected.genesis_hash && action.genesis_hash !== expected.genesis_hash) throw new Error('Asset-Orchard swap action genesis_hash mismatch');
  const poolId = action.pool_id || action.poolId;
  if (poolId !== (expected.pool_id || ASSET_ORCHARD_POOL_ID)) throw new Error('Asset-Orchard swap action pool_id mismatch');
  if (expected.anchor && action.anchor !== expected.anchor) throw new Error('Asset-Orchard swap action anchor mismatch');
  if (expected.circuit_id && action.circuit_id !== expected.circuit_id) throw new Error('Asset-Orchard swap action circuit_id mismatch');

  const cleartextHits = findCleartextActionKeys(action);
  if (cleartextHits.length) {
    throw new Error(`Asset-Orchard swap action contains forbidden cleartext at ${cleartextHits[0].path}`);
  }

  const nullifiers = action.nullifiers || action.input_nullifiers || [];
  const outputCommitments = action.output_commitments || [];
  const accountingInputs = action.accounting_inputs || [];
  const accountingOutputs = action.accounting_outputs || [];
  if (expected.nullifier_count !== undefined && nullifiers.length !== expected.nullifier_count) {
    throw new Error('Asset-Orchard swap action nullifier count mismatch');
  }
  if (expected.output_count !== undefined && outputCommitments.length !== expected.output_count) {
    throw new Error('Asset-Orchard swap action output count mismatch');
  }
  if (expected.accounting_input_count !== undefined && accountingInputs.length !== expected.accounting_input_count) {
    throw new Error('Asset-Orchard swap action accounting input count mismatch');
  }
  if (expected.accounting_output_count !== undefined && accountingOutputs.length !== expected.accounting_output_count) {
    throw new Error('Asset-Orchard swap action accounting output count mismatch');
  }
  return {
    ok: true,
    schema,
    pool_id: poolId,
    anchor: action.anchor || null,
    nullifier_count: nullifiers.length,
    output_count: outputCommitments.length,
    accounting_input_count: accountingInputs.length,
    accounting_output_count: accountingOutputs.length,
  };
}

const PRIVATE_EGRESS_ALLOWED_SPEND_KEYS = new Set(['spend_authorization_signature']);
const PRIVATE_EGRESS_FORBIDDEN_CLEAR_KEYS = new Set([
  'input_note',
  'input_notes',
  'note',
  'note_file',
  'note_files',
  'note_opening',
  'note_openings',
  'opening',
  'rho',
  'rseed',
  'rcm',
  'seed',
  'seed_hex',
  'spend_key',
  'spending_key',
  'wallet_note',
]);

function findPrivateEgressForbiddenCleartext(value, path = '$', seen = new WeakSet()) {
  const hits = [];
  if (!value || typeof value !== 'object') return hits;
  if (seen.has(value)) return hits;
  seen.add(value);
  if (Array.isArray(value)) {
    value.forEach((item, index) => hits.push(...findPrivateEgressForbiddenCleartext(item, `${path}[${index}]`, seen)));
    return hits;
  }
  for (const [key, child] of Object.entries(value)) {
    const normalized = normalizeKey(key);
    const childPath = `${path}.${key}`;
    if (
      PRIVATE_EGRESS_FORBIDDEN_CLEAR_KEYS.has(normalized)
      || (forbiddenPrivateKey(key) && !PRIVATE_EGRESS_ALLOWED_SPEND_KEYS.has(normalized))
    ) {
      hits.push({ path: childPath, key });
    }
    hits.push(...findPrivateEgressForbiddenCleartext(child, childPath, seen));
  }
  return hits;
}

export function verifyAssetOrchardPrivateEgressJson(egressJson, expected = {}) {
  const file = typeof egressJson === 'string' ? JSON.parse(egressJson) : egressJson;
  if (!isPlainObject(file)) throw new Error('Asset-Orchard private egress file must be an object');
  if (file.schema !== ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA) {
    throw new Error('Asset-Orchard private egress file schema mismatch');
  }
  const payload = file.payload;
  if (!isPlainObject(payload)) throw new Error('Asset-Orchard private egress payload is required');
  if (payload.schema !== ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA) {
    throw new Error('Asset-Orchard private egress action schema mismatch');
  }
  if (payload.pool_id !== (expected.pool_id || ASSET_ORCHARD_POOL_ID)) {
    throw new Error('Asset-Orchard private egress pool_id mismatch');
  }
  if (expected.to && payload.to !== expected.to) throw new Error('Asset-Orchard private egress destination mismatch');
  if (expected.asset_id && payload.asset_id !== String(expected.asset_id).toLowerCase()) {
    throw new Error('Asset-Orchard private egress asset_id mismatch');
  }
  if (expected.amount_atoms !== undefined && String(payload.amount) !== String(expected.amount_atoms)) {
    throw new Error('Asset-Orchard private egress amount mismatch');
  }
  if (expected.policy_id && payload.policy_id !== expected.policy_id) {
    throw new Error('Asset-Orchard private egress policy_id mismatch');
  }
  if (expected.disclosure_hash && payload.disclosure_hash !== expected.disclosure_hash) {
    throw new Error('Asset-Orchard private egress disclosure_hash mismatch');
  }
  if (Number(payload.fee || 0) !== 0) throw new Error('Asset-Orchard private egress fee must be zero');
  for (const field of ['anchor', 'nullifier']) {
    if (!/^[0-9a-f]{64}$/.test(String(payload[field] || ''))) {
      throw new Error(`Asset-Orchard private egress ${field} is invalid`);
    }
  }
  if (!/^[0-9a-f]{128}$/.test(String(payload.exit_binding_hash || ''))) {
    throw new Error('Asset-Orchard private egress exit_binding_hash is invalid');
  }
  const proof = String(payload.proof || '');
  if (!/^[0-9a-f]+$/.test(proof) || proof.length % 2 !== 0) {
    throw new Error('Asset-Orchard private egress proof is invalid');
  }
  const cleartextHits = findPrivateEgressForbiddenCleartext(file);
  if (cleartextHits.length) {
    throw new Error(`Asset-Orchard private egress contains forbidden private material at ${cleartextHits[0].path}`);
  }
  return {
    schema: file.schema,
    action_schema: payload.schema,
    pool_id: payload.pool_id,
    to: payload.to,
    asset_id: payload.asset_id,
    amount_atoms: String(payload.amount),
    fee: String(payload.fee || 0),
    policy_id: payload.policy_id,
    disclosure_hash: payload.disclosure_hash,
    anchor: payload.anchor,
    nullifier: payload.nullifier,
    exit_binding_hash: payload.exit_binding_hash,
    proof_bytes: Math.floor(proof.length / 2),
  };
}
