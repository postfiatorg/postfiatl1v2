export const A666_ROUNDTRIP_SCHEMA = 'stakehub-a666-wallet-roundtrip-v1';
export const A666_ROUNDTRIP_ROUTE = 'pftl-a666-ethereum-wA666-usdc-v1';
export const A666_ROUNDTRIP_AMOUNT = '10.000000';
export const A666_ROUNDTRIP_CONFIRMATION = 'RUN A666 ROUND TRIP';

async function authenticatedJson(url, proxyAuthToken, options = {}) {
  if (!proxyAuthToken) {
    throw new Error('Wallet proxy authorization is missing. Open More once to authorize this wallet session.');
  }
  const response = await fetch(url, {
    cache: 'no-store',
    ...options,
    headers: {
      Accept: 'application/json',
      Authorization: `Bearer ${proxyAuthToken}`,
      ...(options.body ? { 'Content-Type': 'application/json' } : {}),
      ...(options.headers || {}),
    },
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok || payload?.ok !== true) {
    const error = new Error(payload.message || payload.error || `A666 round-trip service failed with HTTP ${response.status}`);
    error.httpStatus = response.status;
    error.payload = payload;
    throw error;
  }
  if (payload.schema !== A666_ROUNDTRIP_SCHEMA
      || payload.route !== A666_ROUNDTRIP_ROUTE
      || payload.amount !== A666_ROUNDTRIP_AMOUNT
      || Number(payload.amount_atoms) !== 10_000_000) {
    throw new Error('A666 round-trip service identity mismatch');
  }
  return payload;
}

export function loadA666RoundtripStatus(proxyAuthToken, { signal } = {}) {
  return authenticatedJson('/api/a666-roundtrip/status', proxyAuthToken, { method: 'GET', signal });
}

export function startA666Roundtrip(proxyAuthToken, { signal } = {}) {
  return authenticatedJson('/api/a666-roundtrip/start', proxyAuthToken, {
    method: 'POST',
    signal,
    body: JSON.stringify({
      amount: A666_ROUNDTRIP_AMOUNT,
      confirmation: A666_ROUNDTRIP_CONFIRMATION,
    }),
  });
}
