import { assertNoCustodyMaterial } from './custody-boundary.js';

const READINESS_SCHEMA = 'postfiat-pnok-private-fix-wallet-readiness-v1';
const JOB_SCHEMA = 'postfiat-pnok-private-fix-wallet-job-status-v1';
const TERMINAL = new Set(['accepted', 'failed']);

async function responseJson(url, options = {}) {
  const response = await fetch(url, { cache: 'no-store', ...options });
  const payload = await response.json().catch(() => ({}));
  if ((!response.ok && payload?.ok !== true) || payload?.ok !== true) {
    const error = new Error(payload.message || `pNOK FIX service failed with HTTP ${response.status}`);
    error.payload = payload;
    error.httpStatus = response.status;
    throw error;
  }
  return payload;
}

export async function loadPnokFixReadiness() {
  const payload = await responseJson('/api/pnok-fix/readiness');
  if (payload.schema !== READINESS_SCHEMA) throw new Error('pNOK FIX readiness schema mismatch');
  return payload;
}

export async function createPnokFixJob({
  clientRequestId,
  baseAssetId,
  quoteAssetId,
  baseAtoms,
  proxyAuthToken,
} = {}) {
  const body = {
    client_request_id: clientRequestId,
    base_asset_id: baseAssetId,
    quote_asset_id: quoteAssetId,
    base_atoms: String(baseAtoms || ''),
  };
  assertNoCustodyMaterial(body, 'pNOK private FIX request');
  const payload = await responseJson('/api/pnok-fix/jobs', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(proxyAuthToken ? { Authorization: `Bearer ${proxyAuthToken}` } : {}),
    },
    body: JSON.stringify(body),
  });
  if (payload.schema !== JOB_SCHEMA) throw new Error('pNOK FIX job schema mismatch');
  return payload;
}

export async function loadPnokFixJob(jobId) {
  const payload = await responseJson(`/api/pnok-fix/jobs/${encodeURIComponent(jobId)}`);
  if (payload.schema !== JOB_SCHEMA) throw new Error('pNOK FIX job schema mismatch');
  return payload;
}

export async function waitForPnokFixJob(
  jobId,
  { pollIntervalMs = 2_000, timeoutMs = 45 * 60 * 1000, onStatus = null, signal = null } = {},
) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (signal?.aborted) throw new DOMException('pNOK FIX polling aborted', 'AbortError');
    const result = await loadPnokFixJob(jobId);
    onStatus?.(result);
    if (TERMINAL.has(result.status)) return result;
    await new Promise((resolve, reject) => {
      const timer = setTimeout(resolve, pollIntervalMs);
      signal?.addEventListener('abort', () => {
        clearTimeout(timer);
        reject(new DOMException('pNOK FIX polling aborted', 'AbortError'));
      }, { once: true });
    });
  }
  throw new Error('The private FIX job is still safe and recoverable by its durable job ID.');
}

export function createPnokFixClientRequestId(cryptoApi = globalThis.crypto) {
  const bytes = new Uint8Array(12);
  cryptoApi.getRandomValues(bytes);
  return `pnok-wallet-${[...bytes].map((value) => value.toString(16).padStart(2, '0')).join('')}`;
}

export function formatAssetAtoms(value, precision) {
  let atoms;
  try { atoms = BigInt(String(value)); } catch (_) { return '—'; }
  if (!Number.isInteger(precision) || precision < 0 || precision > 18) return '—';
  if (precision === 0) return atoms.toString();
  const scale = 10n ** BigInt(precision);
  const whole = atoms / scale;
  const fractional = (atoms % scale).toString().padStart(precision, '0');
  return `${whole}.${fractional}`;
}

function unwrap(response, label) {
  if (response?.ok !== true || !response.result) {
    throw new Error(response?.error?.message || `${label} is unavailable`);
  }
  return response.result;
}

export function verifyPnokFixQuote(readiness, listResponse, quoteResponse) {
  if (readiness?.schema !== READINESS_SCHEMA
    || readiness.execution_privacy !== 'private on PFTL'
    || readiness.source_boundary !== 'controlled sandbox checkpoint') {
    throw new Error('pNOK FIX trust or privacy boundary mismatch');
  }
  const listing = unwrap(listResponse, 'FX FIX discovery');
  const fixes = Array.isArray(listing.fixes) ? listing.fixes : [];
  const matching = fixes.filter((row) => {
    const packet = row?.state?.packet;
    const maxFills = Number(packet?.max_fills);
    let exactCapacity = false;
    try {
      exactCapacity = Number.isSafeInteger(maxFills) && maxFills > 0
        && BigInt(String(packet?.capacity_base_atoms)) === BigInt(String(readiness.base_atoms)) * BigInt(maxFills)
        && BigInt(String(packet?.capacity_quote_atoms)) === BigInt(String(readiness.quote_atoms)) * BigInt(maxFills);
    } catch (_) { exactCapacity = false; }
    return row?.status === 'active'
      && packet?.base_asset_id === readiness.base_asset_id
      && packet?.quote_asset_id === readiness.quote_asset_id
      && packet?.source_label === 'pnok_demo_fix'
      && Number(packet?.ratio_numerator) === Number(readiness.ratio_numerator)
      && Number(packet?.ratio_denominator) === Number(readiness.ratio_denominator)
      && Number(packet?.band_bps) === 0
      && Number(packet?.fee_bps) === 0
      && String(packet?.minimum_base_atoms) === String(readiness.base_atoms)
      && exactCapacity
      && Number(row?.remaining_fill_slots) > 0;
  });
  if (matching.length !== 1) throw new Error('asset pair does not resolve to exactly one active demo FIX');
  const row = matching[0];
  const packet = row.state.packet;
  const quote = unwrap(quoteResponse, 'FX FIX quote');
  const exact = quote.fix_packet_hash === packet.packet_hash
    && quote.base_asset?.asset_id === readiness.base_asset_id
    && quote.quote_asset?.asset_id === readiness.quote_asset_id
    && String(quote.base_atoms) === String(readiness.base_atoms)
    && String(quote.quote_atoms) === String(readiness.quote_atoms)
    && quote.exact_division === true
    && Number(quote.fee_atoms) === 0
    && Number(quote.price_impact_bps) === 0
    && quote.source_label === 'pnok_demo_fix'
    && Number(packet.expires_at_height) > Number(quote.current_height);
  if (!exact) throw new Error('wallet recomputation does not match the finalized demo FIX quote');
  return { row, packet, quote };
}
