const HASH48_RE = /^[0-9a-f]{96}$/;
const ROUTE_ID_RE = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const ASSET_CODE_RE = /^[A-Za-z][A-Za-z0-9._-]{0,15}$/;
const EVM_RE = /^0x[0-9a-f]{40}$/;

function exactString(value, field, pattern) {
  const text = String(value || '');
  if (!pattern.test(text)) throw new Error(`${field} is missing or malformed`);
  return text;
}

function boundedPrecision(value, field) {
  const precision = Number(value);
  if (!Number.isSafeInteger(precision) || precision < 0 || precision > 18) {
    throw new Error(`${field} is outside the supported range`);
  }
  return precision;
}

function boundedDisplayName(value, fallback) {
  const text = String(value || '').trim();
  if (!text) return fallback;
  if (text.length > 96 || /[\u0000-\u001f\u007f]/.test(text)) {
    throw new Error('NAVCoin display name is malformed');
  }
  return text;
}

export function navcoinMarketFromRoute(row) {
  if (!row || typeof row !== 'object' || Array.isArray(row)) {
    throw new Error('NAVCoin route row is malformed');
  }
  if (row.route_family !== 'primary_pftl_mint') {
    throw new Error('NAVCoin route family is unsupported');
  }
  const routeId = exactString(row.route_id, 'NAVCoin route id', ROUTE_ID_RE);
  const navAssetId = exactString(row.native_nav_asset_id, 'NAVCoin asset id', HASH48_RE);
  const settlementAssetId = exactString(row.settlement_asset_id, 'settlement asset id', HASH48_RE);
  const routeConfigDigest = exactString(row.route_config_digest, 'route configuration digest', HASH48_RE);
  const wrappedToken = exactString(String(row.wrapped_navcoin_token || '').toLowerCase(), 'wrapped token', EVM_RE);
  const handoffController = exactString(String(row.handoff_controller || '').toLowerCase(), 'handoff controller', EVM_RE);
  const symbol = exactString(row.native_nav_asset_code, 'NAVCoin symbol', ASSET_CODE_RE);
  const settlementSymbol = exactString(row.settlement_asset_code, 'settlement symbol', ASSET_CODE_RE);
  const decimals = boundedPrecision(row.native_nav_asset_precision, 'NAVCoin precision');
  const settlementDecimals = boundedPrecision(row.settlement_asset_precision, 'settlement precision');
  const ethereumChainId = Number(row.ethereum_chain_id);
  if (!Number.isSafeInteger(ethereumChainId) || ethereumChainId <= 0) {
    throw new Error('NAVCoin destination chain is malformed');
  }
  return Object.freeze({
    key: routeId,
    routeId,
    routeConfigDigest,
    navAssetId,
    settlementAssetId,
    wrappedToken,
    handoffController,
    symbol,
    name: boundedDisplayName(row.native_nav_asset_display_name, `${symbol} NAVCoin`),
    wrappedSymbol: `w${symbol}`.slice(0, 16),
    settlementSymbol,
    settlementName: boundedDisplayName(row.settlement_asset_display_name, `${settlementSymbol} settlement asset`),
    decimals,
    settlementDecimals,
    ethereumChainId,
    routeTrustClass: String(row.route_trust_class || ''),
    liveValueEnabled: row.live_value_enabled === true,
    paused: row.paused === true,
    transactionAdapter: 'pftl-uniswap-primary-v2',
  });
}

export function navcoinMarketsFromRoutes(report) {
  if (!report || report.schema !== 'postfiat-pftl-uniswap-routes-status-v2'
    || !Array.isArray(report.routes) || report.routes.length > 64
    || !Number.isSafeInteger(report.route_count)
    || report.route_count !== report.routes.length) {
    throw new Error('governed NAVCoin route registry response is malformed');
  }
  if (!report.routes.every(row => row && typeof row === 'object'
    && !Array.isArray(row) && typeof row.route_live === 'boolean')) {
    throw new Error('governed NAVCoin route registry route_live must be boolean');
  }
  const markets = report.routes
    .filter(row => row.route_live === true)
    .map(navcoinMarketFromRoute);
  markets.sort((left, right) => (left.routeId < right.routeId ? -1 : left.routeId > right.routeId ? 1 : 0));
  const routeIds = new Set();
  const navAssetIds = new Set();
  const wrappedTokens = new Set();
  for (const market of markets) {
    if (routeIds.has(market.routeId)
      || navAssetIds.has(market.navAssetId)
      || wrappedTokens.has(market.wrappedToken)) {
      throw new Error('governed NAVCoin route registry contains a duplicate identity');
    }
    routeIds.add(market.routeId);
    navAssetIds.add(market.navAssetId);
    wrappedTokens.add(market.wrappedToken);
  }
  return Object.freeze(markets);
}

export function navcoinMarketByKey(markets, key) {
  return markets.find(market => market.key === key) || markets[0] || null;
}

export function navcoinMarketForAsset(markets, assetId) {
  const target = String(assetId || '').toLowerCase();
  return markets.find(market => market.navAssetId === target) || null;
}

export function navcoinMarketForSettlementAsset(markets, assetId) {
  const target = String(assetId || '').toLowerCase();
  return markets.find(market => market.settlementAssetId === target) || null;
}

export function displayAssetSymbol(markets, assetId, fallback) {
  const market = navcoinMarketForAsset(markets, assetId);
  if (market) return market.symbol;
  const settlementMarket = navcoinMarketForSettlementAsset(markets, assetId);
  return settlementMarket?.settlementSymbol || fallback;
}
