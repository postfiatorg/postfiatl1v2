const ROUTE_ID_RE = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const EVM_RE = /^0x[0-9a-f]{40}$/;
const HASH32_RE = /^0x[0-9a-f]{64}$/;

function positiveIntegerString(value, field) {
  const text = String(value || '');
  if (!/^[1-9][0-9]*$/.test(text)) throw new Error(`${field} is unavailable`);
  return text;
}

export function parseNavcoinSecondaryMarketData(payload, market) {
  if (payload?.ok !== true || payload?.schema !== 'postfiat-navcoin-secondary-market-data-v1') {
    throw new Error('Secondary-market price is unavailable');
  }
  if (!market || payload.route_id !== market.routeId || payload.ethereum_chain_id !== market.ethereumChainId) {
    throw new Error('Secondary-market route identity does not match the selected NAV asset');
  }
  if (!EVM_RE.test(payload.base_token) || payload.base_token !== market.wrappedToken
    || !EVM_RE.test(payload.quote_token)) {
    throw new Error('Secondary-market token identity is invalid');
  }
  if (!HASH32_RE.test(payload.pool_id)) throw new Error('Secondary-market pool identity is invalid');
  const observedAt = Number(payload.observed_at_unix);
  if (!Number.isSafeInteger(observedAt) || observedAt <= 0) {
    throw new Error('Secondary-market observation time is invalid');
  }
  return Object.freeze({
    routeId: payload.route_id,
    spotUsdcAtoms: positiveIntegerString(payload.spot_usdc_atoms_per_wa666, 'Secondary-market price'),
    liquidity: positiveIntegerString(payload.liquidity, 'Secondary-market liquidity'),
    ethereumBlockNumber: positiveIntegerString(payload.ethereum_block_number, 'Ethereum block'),
    observedAt,
  });
}

export async function loadNavcoinSecondaryMarketData(market, fetchImpl = fetch) {
  if (!market || !ROUTE_ID_RE.test(String(market.routeId || ''))) {
    throw new Error('Selected NAV asset has no secondary-market route');
  }
  const response = await fetchImpl(`/api/navcoin/${encodeURIComponent(market.routeId)}/market-data`, {
    method: 'GET',
    headers: { Accept: 'application/json' },
    cache: 'no-store',
  });
  if (!response.ok) throw new Error('Secondary-market price is temporarily unavailable');
  return parseNavcoinSecondaryMarketData(await response.json(), market);
}

export function navDiscountPremiumBps(spotAtoms, navAtoms) {
  const spot = BigInt(String(spotAtoms || 0));
  const nav = BigInt(String(navAtoms || 0));
  if (spot <= 0n || nav <= 0n) return null;
  return ((spot - nav) * 10_000n) / nav;
}
