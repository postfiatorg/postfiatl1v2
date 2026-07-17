import { assertNoCustodyMaterial } from './custody-boundary.js';

const API_ROOT = '/api/fastswap-demo';

async function request(path, options = {}) {
  if (options.body) assertNoCustodyMaterial(options.body, `wallet FastSwap demo ${path}`);
  const response = await fetch(`${API_ROOT}${path}`, {
    cache: 'no-store',
    headers: { 'content-type': 'application/json', ...(options.headers || {}) },
    ...options,
  });
  let payload;
  try {
    payload = await response.json();
  } catch (_) {
    throw new Error(`FastSwap service returned HTTP ${response.status} without JSON`);
  }
  if (!response.ok || payload?.ok !== true) {
    throw new Error(payload?.error || `FastSwap service returned HTTP ${response.status}`);
  }
  return payload.result;
}

export const fastSwapDemoApi = {
  status: () => request('/status'),
  quote: () => request('/quote'),
  faucetStatus: (address) => request('/faucet-status', {
    method: 'POST',
    body: JSON.stringify({ address }),
  }),
  faucet: (address) => request('/faucet', {
    method: 'POST',
    body: JSON.stringify({ address }),
  }),
  swap: (quoteId) => request('/swap', {
    method: 'POST',
    body: JSON.stringify({ quote_id: quoteId, confirm: true }),
  }),
};

export function formatAtoms(atoms, decimals = 8) {
  if (!Number.isSafeInteger(atoms) || atoms < 0 || !Number.isInteger(decimals) || decimals < 0) {
    return '—';
  }
  if (decimals === 0) return String(atoms);
  const digits = String(atoms).padStart(decimals + 1, '0');
  const whole = digits.slice(0, -decimals) || '0';
  const fraction = decimals ? digits.slice(-decimals).replace(/0+$/, '') : '';
  return fraction ? `${whole}.${fraction}` : whole;
}

export function formatUsdE8(value) {
  if (!Number.isSafeInteger(value) || value < 0) return '—';
  return `$${formatAtoms(value, 8)}`;
}

export function navPresentation(nav, nowSeconds = Math.floor(Date.now() / 1000)) {
  const protocolFresh = Boolean(
    nav?.active && nav?.reserve_packet_fresh && nav?.supply_packet_fresh && !nav?.policy_paused,
  );
  const secondsLeft = Number.isFinite(nav?.packet_expires_at)
    ? nav.packet_expires_at - nowSeconds
    : null;
  const expiresLabel = secondsLeft == null
    ? 'expiry unavailable'
    : secondsLeft <= 0
      ? 'expired'
      : `${Math.floor(secondsLeft / 86400)}d ${Math.floor((secondsLeft % 86400) / 3600)}h remaining`;
  return {
    protocolFresh,
    verdict: protocolFresh ? 'VERIFIED · USABLE' : 'NOT USABLE',
    price: formatUsdE8(nav?.usd_e8),
    ageLabel: Number.isSafeInteger(nav?.reserve_packet_age_blocks)
      ? `${nav.reserve_packet_age_blocks.toLocaleString()} blocks old`
      : 'age unavailable',
    expiresLabel,
    blocksRemaining: Number.isSafeInteger(nav?.policy_blocks_remaining)
      ? `${nav.policy_blocks_remaining.toLocaleString()} policy blocks remaining`
      : 'policy window unavailable',
  };
}

export function receiptPresentation(result) {
  if (result?.receipt?.accepted === true && result.receipt.code === 'fastswap_applied') {
    return { tone: 'accepted', label: 'ACCEPTED', message: 'Both assets moved. The swap is final.' };
  }
  if (result?.receipt?.accepted === false || result?.status === 'rejected') {
    return { tone: 'rejected', label: 'REJECTED', message: 'No success is shown. Inspect the receipt reason.' };
  }
  return { tone: 'unknown', label: 'UNKNOWN', message: 'Finality is not proven. Do not assume the swap happened.' };
}

export function shorten(value, left = 10, right = 8) {
  if (!value || value.length <= left + right + 1) return value || '—';
  return `${value.slice(0, left)}…${value.slice(-right)}`;
}
