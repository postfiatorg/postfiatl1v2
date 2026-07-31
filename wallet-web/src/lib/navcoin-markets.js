import {
  A666_NATIVE_ASSET_ID,
  A666_PRIMARY_ROUTE_ID,
  A666_SETTLEMENT_ASSET_ID,
  A666_WRAPPED_TOKEN,
} from './a666-primary-route.js';
import { ETH_MAINNET_CHAIN_ID, ETH_MAINNET_USDC } from './utils.js';

// The wallet renders governed NAVCoin routes from this registry. A route entry is
// deployment configuration, not a product-specific UI branch. Adding another
// NAVCoin should add a reviewed descriptor (and its transaction adapter) here;
// shared wallet surfaces must not special-case its symbol.
export const NAVCOIN_MARKETS = Object.freeze([
  Object.freeze({
    key: A666_PRIMARY_ROUTE_ID,
    routeId: A666_PRIMARY_ROUTE_ID,
    navAssetId: A666_NATIVE_ASSET_ID,
    settlementAssetId: A666_SETTLEMENT_ASSET_ID,
    wrappedToken: A666_WRAPPED_TOKEN,
    symbol: 'A666',
    name: 'A666 NAVCoin fund share',
    wrappedSymbol: 'wA666',
    settlementSymbol: 'pfUSDC',
    settlementSourceChainId: ETH_MAINNET_CHAIN_ID,
    settlementTokenAddress: ETH_MAINNET_USDC,
    decimals: 6,
    transactionAdapter: 'a666-mainnet-v2',
  }),
]);

export const DEFAULT_NAVCOIN_MARKET = NAVCOIN_MARKETS[0];

function normalized(value) {
  return String(value || '').toLowerCase();
}

export function navcoinMarketByKey(key) {
  return NAVCOIN_MARKETS.find(market => market.key === key) || DEFAULT_NAVCOIN_MARKET;
}

export function navcoinMarketForAsset(assetId) {
  const target = normalized(assetId);
  return NAVCOIN_MARKETS.find(market => normalized(market.navAssetId) === target) || null;
}

export function navcoinMarketForSettlementAsset(assetId) {
  const target = normalized(assetId);
  return NAVCOIN_MARKETS.find(market => normalized(market.settlementAssetId) === target) || null;
}

export function displayAssetSymbol(assetId, fallback) {
  const market = navcoinMarketForAsset(assetId);
  if (market) return market.symbol;
  const settlementMarket = navcoinMarketForSettlementAsset(assetId);
  if (settlementMarket) return settlementMarket.settlementSymbol;
  return fallback;
}
