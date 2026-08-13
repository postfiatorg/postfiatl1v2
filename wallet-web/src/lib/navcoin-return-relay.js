import { assertNoCustodyMaterial } from './custody-boundary.js';

const TERMINAL = new Set(['accepted', 'failed']);
const BURN_SELECTOR = 'f34c595b';

function word(value) {
  const hex = typeof value === 'bigint' ? value.toString(16) : String(value).replace(/^0x/, '');
  if (!/^[0-9a-f]*$/i.test(hex) || hex.length > 64) throw new Error('NAVCoin return ABI value is invalid');
  return hex.padStart(64, '0').toLowerCase();
}

function dynamicBytes(bytes) {
  const hex = [...bytes].map(value => value.toString(16).padStart(2, '0')).join('');
  return `${word(BigInt(bytes.length))}${hex.padEnd(Math.ceil(hex.length / 64) * 64, '0')}`;
}

export function buildNavcoinReturnBurnCalldata({ amountAtoms, pftlRecipient, nativeNavAssetId, returnNonce }) {
  const amount = BigInt(String(amountAtoms || '0'));
  const recipient = String(pftlRecipient || '').trim().toLowerCase();
  const asset = String(nativeNavAssetId || '').trim().toLowerCase().replace(/^0x/, '');
  const nonce = String(returnNonce || '').trim().toLowerCase().replace(/^0x/, '');
  if (amount <= 0n || !/^pf[0-9a-f]{40}$/.test(recipient)
    || !/^[0-9a-f]{96}$/.test(asset) || !/^[0-9a-f]{64}$/.test(nonce) || /^0+$/.test(nonce)) {
    throw new Error('NAVCoin return burn fields are invalid');
  }
  const recipientTail = dynamicBytes(new TextEncoder().encode(recipient));
  const assetTail = dynamicBytes(Uint8Array.from(asset.match(/../g).map(value => Number.parseInt(value, 16))));
  const recipientOffset = 4n * 32n;
  const assetOffset = recipientOffset + BigInt(recipientTail.length / 2);
  return `0x${BURN_SELECTOR}${word(amount)}${word(recipientOffset)}${word(assetOffset)}${word(nonce)}${recipientTail}${assetTail}`;
}

export function createNavcoinReturnNonce(cryptoApi = globalThis.crypto) {
  if (!cryptoApi?.getRandomValues) throw new Error('Secure browser randomness is unavailable');
  const bytes = cryptoApi.getRandomValues(new Uint8Array(32));
  if (bytes.every(value => value === 0)) bytes[31] = 1;
  return [...bytes].map(value => value.toString(16).padStart(2, '0')).join('');
}

async function relayJson(url, options = {}) {
  const response = await fetch(url, { cache: 'no-store', ...options });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok || payload.ok !== true) {
    const error = new Error(payload.message || `NAVCoin return relay failed with HTTP ${response.status}`);
    error.payload = payload;
    error.httpStatus = response.status;
    throw error;
  }
  return payload;
}

function transient(error) {
  return !error?.httpStatus || error.httpStatus === 408 || error.httpStatus === 429 || error.httpStatus >= 500;
}

async function relayJsonWithRetry(url, options = {}, attempts = 4) {
  let lastError;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try { return await relayJson(url, options); } catch (error) {
      lastError = error;
      if (!transient(error) || attempt === attempts) throw error;
      await new Promise(resolve => setTimeout(resolve, Math.min(5_000, 500 * (2 ** (attempt - 1)))));
    }
  }
  throw lastError;
}

function routePath(routeId) {
  const route = String(routeId || '');
  if (!/^[a-z0-9._-]{1,64}$/.test(route)) throw new Error('NAVCoin route id is malformed');
  return `/api/navcoin/${encodeURIComponent(route)}`;
}

export async function loadNavcoinReturnReadiness(routeId) {
  return relayJson(`${routePath(routeId)}/return-readiness`);
}

export async function loadNavcoinReturnJobs(routeId, pftlRecipient, proxyAuthToken, limit = 20) {
  const recipient = String(pftlRecipient || '').trim().toLowerCase();
  if (!/^pf[0-9a-f]{40}$/.test(recipient)) throw new Error('PFTL recipient is malformed');
  return relayJson(`${routePath(routeId)}/return-jobs?pftl_recipient=${encodeURIComponent(recipient)}&limit=${Math.min(100, Math.max(1, Number(limit) || 20))}`, {
    headers: proxyAuthToken ? { Authorization: `Bearer ${proxyAuthToken}` } : {},
  });
}

export async function createNavcoinReturnJob({
  routeId, routeConfigDigest, transactionHash, ethereumSender, pftlRecipient,
  nativeNavAssetId, amountAtoms, returnNonce, proxyAuthToken,
}) {
  const body = {
    route_id: routeId,
    route_config_digest: routeConfigDigest,
    transaction_hash: transactionHash,
    ethereum_sender: ethereumSender,
    pftl_recipient: pftlRecipient,
    native_nav_asset_id: nativeNavAssetId,
    amount_atoms: String(amountAtoms || ''),
    return_nonce: returnNonce,
  };
  assertNoCustodyMaterial(body, 'NAVCoin return relay request');
  return relayJsonWithRetry(`${routePath(routeId)}/return-jobs`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(proxyAuthToken ? { Authorization: `Bearer ${proxyAuthToken}` } : {}),
    },
    body: JSON.stringify(body),
  });
}

export async function loadNavcoinReturnJob(routeId, jobId) {
  return relayJsonWithRetry(`${routePath(routeId)}/return-jobs/${encodeURIComponent(jobId)}`);
}

export async function waitForNavcoinReturnJob(
  routeId, jobId,
  { pollIntervalMs = 5_000, timeoutMs = 4 * 60 * 60 * 1000, onStatus = null } = {},
) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    let result;
    try { result = await loadNavcoinReturnJob(routeId, jobId); } catch (error) {
      if (!transient(error)) throw error;
      await new Promise(resolve => setTimeout(resolve, pollIntervalMs));
      continue;
    }
    onStatus?.(result);
    if (TERMINAL.has(result.status)) {
      if (result.status === 'accepted') return result;
      const error = new Error(result.message || 'The NAVCoin return failed a safety gate.');
      error.payload = result;
      throw error;
    }
    await new Promise(resolve => setTimeout(resolve, pollIntervalMs));
  }
  const error = new Error('The return remains safely queued and will continue without this browser.');
  error.code = 'navcoin_return_job_poll_timeout';
  throw error;
}
