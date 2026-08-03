const READINESS_SCHEMA = 'postfiat.wallet.pftl_private_swap_readiness.v1';
const UPSTREAM_READINESS_SCHEMA = 'postfiat.pftl_swap.readiness.v1';
const QUOTE_SCHEMA = 'postfiat.pftl_swap.quote.v1';
const INTENT_SCHEMA = 'postfiat.pftl_swap.intent.v1';
const SIGNED_INTENT_SCHEMA = 'postfiat.pftl_swap.signed_intent.v1';
const RECOVERY_SCHEMA = 'postfiat.wallet.pftl_private_swap_recovery.v1';
const TERMINAL_STATES = new Set(['COMMITTED', 'REJECTED']);
const RETRYABLE_STATES = new Set(['FAILED_PREPUBLISH', 'INTERRUPTED_PREPUBLISH']);
const INTENT_FIELDS = [
  'schema', 'chain_id', 'genesis_hash', 'protocol_version', 'principal', 'controlled_wallet_id',
  'route_id', 'direction', 'output_mode', 'input_reference', 'input_amount_atoms',
  'minimum_output_amount_atoms', 'maximum_fee_atoms', 'quote_id', 'pricing_nav_epoch',
  'policy_hash', 'expiry_height', 'idempotency_key',
];
const QUOTE_FIELDS = [
  'schema', 'quote_id', 'chain_id', 'genesis_hash', 'protocol_version', 'route_id', 'direction',
  'output_mode', 'nav_amount_atoms', 'input_asset_id', 'input_amount_atoms', 'output_asset_id',
  'output_amount_atoms', 'base_settlement_atoms', 'spread_atoms', 'maximum_fee_atoms',
  'route_epoch', 'policy_epoch', 'policy_hash', 'pricing_nav_epoch',
  'pricing_reserve_packet_hash', 'quote_height', 'quote_block_id', 'state_root', 'orchard_root',
  'route_state_hash', 'expiry_height', 'created_at_unix_ms',
];
const SWAP_FIELDS = [
  'swap_id', 'idempotency_key', 'quote_id', 'direction', 'input_amount_atoms',
  'minimum_output_amount_atoms', 'state', 'batch_hash', 'committed_height', 'certificate_ref',
];

function selectFields(value, fields) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return Object.fromEntries(fields.filter(field => Object.hasOwn(value, field)).map(field => [field, value[field]]));
}

function authHeaders(proxyAuthToken, json = false) {
  return {
    Accept: 'application/json',
    ...(json ? { 'Content-Type': 'application/json' } : {}),
    ...(proxyAuthToken ? { Authorization: `Bearer ${proxyAuthToken}` } : {}),
  };
}

async function requestJson(path, {
  method = 'GET', body, proxyAuthToken = '', fetchImpl = fetch, signal = undefined,
} = {}) {
  const response = await fetchImpl(path, {
    method,
    headers: authHeaders(proxyAuthToken, body !== undefined),
    body: body === undefined ? undefined : JSON.stringify(body),
    cache: 'no-store',
    signal,
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok || payload?.ok !== true) {
    const message = payload?.message
      || payload?.upstream?.message
      || `Private NAVCoin service failed with HTTP ${response.status}`;
    const error = new Error(message);
    error.httpStatus = response.status;
    error.payload = payload;
    throw error;
  }
  return payload;
}

function safePositiveInteger(value, label) {
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number <= 0) {
    throw new Error(`${label} is not a positive safe integer`);
  }
  return number;
}

function boundedString(value, label, maximum = 128) {
  const text = String(value || '');
  if (!text || text.length > maximum) throw new Error(`${label} is missing or exceeds its bound`);
  return text;
}

export async function loadPftlPrivateReadiness({ expectedRouteId = '', ...options } = {}) {
  const expectedRoute = boundedString(expectedRouteId, 'governed route id');
  const payload = await requestJson('/api/pftl-private-swap/readiness', options);
  if (payload.schema !== READINESS_SCHEMA
    || payload.configured !== true
    || payload.upstream?.schema !== UPSTREAM_READINESS_SCHEMA
    || payload.upstream?.ready !== true
    || payload.upstream?.local_only !== true
    || payload.controlled_wallet_id !== payload.upstream.controlled_wallet_id
    || payload.route_id !== expectedRoute
    || payload.upstream.route_id !== expectedRoute) {
    throw new Error('Private NAVCoin readiness boundary mismatch');
  }
  return payload;
}

export async function createPftlPrivateQuote({
  direction,
  navAmountAtoms,
  outputMode,
  expectedRouteId,
  proxyAuthToken = '',
  fetchImpl = fetch,
} = {}) {
  if (!['issue', 'redeem'].includes(direction) || !['private', 'transparent'].includes(outputMode)) {
    throw new Error('Private NAVCoin direction or output mode is invalid');
  }
  const expectedRoute = boundedString(expectedRouteId, 'governed route id');
  const payload = await requestJson('/api/pftl-private-swap/quotes', {
    method: 'POST',
    body: {
      direction,
      nav_amount_atoms: safePositiveInteger(navAmountAtoms, 'nav_amount_atoms'),
      output_mode: outputMode,
    },
    proxyAuthToken,
    fetchImpl,
  });
  const quote = payload.quote;
  if (quote?.schema !== QUOTE_SCHEMA
    || quote.direction !== direction
    || quote.output_mode !== outputMode
    || quote.route_id !== expectedRoute
    || safePositiveInteger(quote.nav_amount_atoms, 'quote nav_amount_atoms') !== Number(navAmountAtoms)) {
    throw new Error('Private NAVCoin quote does not match the requested lifecycle');
  }
  return quote;
}

export function buildPftlPrivateIntent({ quote, walletAddress, controlledWalletId, inputReference, idempotencyKey } = {}) {
  if (!quote || quote.schema !== QUOTE_SCHEMA) throw new Error('A verified Private NAVCoin quote is required');
  const principal = boundedString(walletAddress, 'wallet address', 42).toLowerCase();
  const controlled = boundedString(controlledWalletId, 'controlled wallet id', 42).toLowerCase();
  if (principal !== controlled) throw new Error('This wallet is not the controlled Private NAVCoin wallet');
  return {
    schema: INTENT_SCHEMA,
    chain_id: boundedString(quote.chain_id, 'chain id'),
    genesis_hash: boundedString(quote.genesis_hash, 'genesis hash', 96),
    protocol_version: safePositiveInteger(quote.protocol_version, 'protocol version'),
    principal,
    controlled_wallet_id: controlled,
    route_id: boundedString(quote.route_id, 'route id'),
    direction: quote.direction,
    output_mode: quote.output_mode,
    input_reference: boundedString(inputReference, 'input reference', 256),
    input_amount_atoms: safePositiveInteger(quote.input_amount_atoms, 'input amount'),
    minimum_output_amount_atoms: safePositiveInteger(quote.output_amount_atoms, 'minimum output amount'),
    maximum_fee_atoms: safePositiveInteger(quote.maximum_fee_atoms, 'maximum fee'),
    quote_id: boundedString(quote.quote_id, 'quote id', 96),
    pricing_nav_epoch: safePositiveInteger(quote.pricing_nav_epoch, 'pricing NAV epoch'),
    policy_hash: boundedString(quote.policy_hash, 'policy hash', 96),
    expiry_height: safePositiveInteger(quote.expiry_height, 'expiry height'),
    idempotency_key: boundedString(idempotencyKey, 'idempotency key'),
  };
}

export function signPftlPrivateIntent({ wasm, backupJson, intent } = {}) {
  if (!wasm?.wallet_sign_pftl_swap_intent) throw new Error('Wallet WASM does not support Private NAVCoin signing');
  if (!backupJson) throw new Error('The unlocked wallet backup is unavailable');
  const signed = wasm.wallet_sign_pftl_swap_intent(backupJson, JSON.stringify(intent));
  if (signed?.schema !== SIGNED_INTENT_SCHEMA
    || signed?.intent?.idempotency_key !== intent.idempotency_key
    || signed?.intent?.quote_id !== intent.quote_id) {
    throw new Error('Wallet returned a mismatched Private NAVCoin signature');
  }
  return signed;
}

export async function submitPftlPrivateIntent(signedIntent, options = {}) {
  if (signedIntent?.schema !== SIGNED_INTENT_SCHEMA) throw new Error('A signed Private NAVCoin intent is required');
  return requestJson('/api/pftl-private-swap/jobs', {
    method: 'POST',
    body: { signed_intent: signedIntent },
    ...options,
  });
}

export async function loadPftlPrivateStatus(idempotencyKey, options = {}) {
  const key = boundedString(idempotencyKey, 'idempotency key');
  return requestJson(`/api/pftl-private-swap/jobs/${encodeURIComponent(key)}`, options);
}

function abortableDelay(milliseconds, signal) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(resolve, milliseconds);
    signal?.addEventListener('abort', () => {
      clearTimeout(timer);
      reject(new DOMException('Private NAVCoin polling aborted', 'AbortError'));
    }, { once: true });
  });
}

export async function completePftlPrivateIntent(
  signedIntent,
  {
    proxyAuthToken = '',
    fetchImpl = fetch,
    pollIntervalMs = 2_000,
    timeoutMs = 30 * 60 * 1000,
    onStatus = null,
    signal = null,
  } = {},
) {
  const options = { proxyAuthToken, fetchImpl, signal };
  let response = await submitPftlPrivateIntent(signedIntent, options);
  onStatus?.(response);
  let state = response?.swap?.state;
  const deadline = Date.now() + timeoutMs;
  while (!TERMINAL_STATES.has(state) && Date.now() < deadline) {
    if (RETRYABLE_STATES.has(state)) {
      response = await submitPftlPrivateIntent(signedIntent, options);
      onStatus?.(response);
      state = response?.swap?.state;
      continue;
    }
    if (signal?.aborted) throw new DOMException('Private NAVCoin polling aborted', 'AbortError');
    await abortableDelay(pollIntervalMs, signal);
    response = await loadPftlPrivateStatus(signedIntent.intent.idempotency_key, options);
    onStatus?.(response);
    state = response?.swap?.state;
  }
  if (state === 'REJECTED') {
    throw new Error(`Private NAVCoin execution stopped safely in ${state}`);
  }
  if (state !== 'COMMITTED') {
    throw new Error('Private NAVCoin execution remains durable and can be resumed by its idempotency key');
  }
  // Status is deliberately public and omits note references. A same-intent replay
  // returns the committed output reference without creating another swap.
  const committed = await submitPftlPrivateIntent(signedIntent, options);
  if (committed?.swap?.state !== 'COMMITTED') throw new Error('Committed Private NAVCoin replay was not stable');
  return committed;
}

export function createPftlPrivateIdempotencyKey(direction, cryptoApi = globalThis.crypto) {
  if (!['issue', 'redeem'].includes(direction)) throw new Error('Private NAVCoin direction is invalid');
  const bytes = new Uint8Array(12);
  cryptoApi.getRandomValues(bytes);
  return `navcoin-browser-${direction}-${[...bytes].map(value => value.toString(16).padStart(2, '0')).join('')}`;
}

export function parsePrivateNavcoinAmountAtoms(value, { decimals = 6, maximumAtoms = 1_000_000 } = {}) {
  if (!Number.isInteger(decimals) || decimals < 0 || decimals > 18
    || !Number.isSafeInteger(maximumAtoms) || maximumAtoms <= 0) {
    throw new Error('Private NAVCoin amount policy is invalid');
  }
  const text = String(value || '').trim();
  const pattern = new RegExp(`^(?:0|[1-9][0-9]*)(?:\\.[0-9]{0,${decimals}})?$`);
  if (!pattern.test(text)) {
    throw new Error(`NAVCoin amount must have at most ${decimals} decimal places`);
  }
  const [whole, fraction = ''] = text.split('.');
  const scale = 10n ** BigInt(decimals);
  const atoms = BigInt(whole) * scale + BigInt((fraction + '0'.repeat(decimals)).slice(0, decimals) || '0');
  if (atoms <= 0n || atoms > BigInt(maximumAtoms)) {
    throw new Error('Controlled Private NAVCoin size is outside the resident service limit');
  }
  return Number(atoms);
}

export function pftlPrivateRecoveryKey(walletAddress) {
  return `postfiat.navcoin_private_primary.${String(walletAddress || '').toLowerCase()}.v1`;
}

function publicRecoveryRecord(record, walletAddress) {
  const suppliedSignedIntent = record?.signed_intent;
  const intent = selectFields(suppliedSignedIntent?.intent, INTENT_FIELDS);
  const signedIntent = suppliedSignedIntent && intent ? {
    schema: suppliedSignedIntent.schema,
    intent,
    algorithm_id: suppliedSignedIntent.algorithm_id,
    public_key_hex: suppliedSignedIntent.public_key_hex,
    signature_hex: suppliedSignedIntent.signature_hex,
  } : null;
  if (signedIntent?.schema !== SIGNED_INTENT_SCHEMA
    || signedIntent?.intent?.principal !== walletAddress
    || signedIntent?.intent?.controlled_wallet_id !== walletAddress) {
    throw new Error('Private NAVCoin recovery is not bound to this wallet');
  }
  const suppliedResponse = record.response;
  const response = suppliedResponse && typeof suppliedResponse === 'object' ? {
    ok: suppliedResponse.ok === true,
    replayed: suppliedResponse.replayed === true,
    swap: selectFields(suppliedResponse.swap, SWAP_FIELDS),
    output_note_refs: Array.isArray(suppliedResponse.output_note_refs)
      ? suppliedResponse.output_note_refs.filter(reference => /^[0-9a-f]{64}$/.test(String(reference))).slice(0, 1)
      : [],
  } : null;
  return {
    idempotency_key: signedIntent.intent.idempotency_key,
    direction: signedIntent.intent.direction,
    output_mode: signedIntent.intent.output_mode,
    quote: selectFields(record.quote, QUOTE_FIELDS),
    signed_intent: signedIntent,
    response,
    source_issue_idempotency_key: record.source_issue_idempotency_key || null,
    status: String(record.status || 'SIGNED'),
    created_at_unix_ms: safePositiveInteger(record.created_at_unix_ms || Date.now(), 'recovery creation time'),
  };
}

export function loadPftlPrivateRecoveries(storage, walletAddress) {
  const wallet = String(walletAddress || '').toLowerCase();
  if (!storage || !wallet) return [];
  try {
    const value = JSON.parse(storage.getItem(pftlPrivateRecoveryKey(wallet)) || 'null');
    if (value?.schema !== RECOVERY_SCHEMA || value.wallet_address !== wallet || !Array.isArray(value.records)) return [];
    return value.records.map(record => publicRecoveryRecord(record, wallet));
  } catch (_) {
    return [];
  }
}

export function savePftlPrivateRecoveries(storage, walletAddress, records) {
  const wallet = String(walletAddress || '').toLowerCase();
  if (!storage || !wallet) throw new Error('Private NAVCoin recovery storage or wallet is unavailable');
  const publicRecords = (Array.isArray(records) ? records : [])
    .slice(-32)
    .map(record => publicRecoveryRecord(record, wallet));
  storage.setItem(pftlPrivateRecoveryKey(wallet), JSON.stringify({
    schema: RECOVERY_SCHEMA,
    wallet_address: wallet,
    records: publicRecords,
  }));
  return publicRecords;
}

export { READINESS_SCHEMA, RECOVERY_SCHEMA, TERMINAL_STATES };
