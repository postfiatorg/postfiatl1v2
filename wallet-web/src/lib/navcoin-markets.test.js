import assert from 'node:assert/strict';
import test from 'node:test';

import {
  displayAssetSymbol,
  navcoinMarketByKey,
  navcoinMarketForAsset,
  navcoinMarketForSettlementAsset,
  navcoinMarketsFromRoutes,
} from './navcoin-markets.js';

const route = {
  route_id: 'pftl-qnav-ethereum-wqNAV-qusd-v1',
  route_family: 'primary_pftl_mint',
  route_config_digest: '11'.repeat(48),
  route_trust_class: 'BFT_CHECKPOINT',
  live_value_enabled: true,
  paused: false,
  native_nav_asset_id: '22'.repeat(48),
  native_nav_asset_code: 'qNAV',
  native_nav_asset_display_name: 'Qualified NAVCoin',
  native_nav_asset_precision: 6,
  settlement_asset_id: '33'.repeat(48),
  settlement_asset_code: 'qUSD',
  settlement_asset_display_name: 'Qualified settlement asset',
  settlement_asset_precision: 6,
  wrapped_navcoin_token: `0x${'44'.repeat(20)}`,
  handoff_controller: `0x${'55'.repeat(20)}`,
  ethereum_chain_id: 1,
};

test('governed NAVCoin markets are derived from bounded chain registry rows', () => {
  const markets = navcoinMarketsFromRoutes({
    schema: 'postfiat-pftl-uniswap-routes-status-v2',
    route_count: 1,
    routes: [route],
  });
  const market = markets[0];
  assert.equal(market.symbol, 'qNAV');
  assert.equal(market.settlementSymbol, 'qUSD');
  assert.equal(market.transactionAdapter, 'pftl-uniswap-primary-v2');
  assert.equal(navcoinMarketByKey(markets, market.key), market);
  assert.equal(navcoinMarketForAsset(markets, market.navAssetId), market);
  assert.equal(navcoinMarketForSettlementAsset(markets, market.settlementAssetId), market);
  assert.equal(displayAssetSymbol(markets, market.navAssetId, 'fallback'), 'qNAV');
  assert.equal(displayAssetSymbol(markets, market.settlementAssetId, 'fallback'), 'qUSD');
  assert.equal(displayAssetSymbol(markets, 'unknown', 'fallback'), 'fallback');
});

test('governed NAVCoin registry rejects malformed and duplicate identities', () => {
  assert.throws(() => navcoinMarketsFromRoutes({
    schema: 'postfiat-pftl-uniswap-routes-status-v1', route_count: 1, routes: [route],
  }), /registry response is malformed/);
  assert.throws(() => navcoinMarketsFromRoutes({
    schema: 'postfiat-pftl-uniswap-routes-status-v2', route_count: 2, routes: [route, route],
  }), /duplicate identity/);
  assert.throws(() => navcoinMarketsFromRoutes({
    schema: 'postfiat-pftl-uniswap-routes-status-v2',
    route_count: 3,
    routes: [
      { ...route, route_id: 'route-a' },
      {
        ...route,
        route_id: 'route-b',
        native_nav_asset_id: '66'.repeat(48),
        wrapped_navcoin_token: `0x${'77'.repeat(20)}`,
      },
      {
        ...route,
        route_id: 'route-c',
        wrapped_navcoin_token: `0x${'88'.repeat(20)}`,
      },
    ],
  }), /duplicate identity/);
  assert.throws(() => navcoinMarketsFromRoutes({
    schema: 'postfiat-pftl-uniswap-routes-status-v2', route_count: '1', routes: [route],
  }), /registry response is malformed/);
  assert.throws(() => navcoinMarketsFromRoutes({
    schema: 'postfiat-pftl-uniswap-routes-status-v2', route_count: 1,
    routes: [{ ...route, native_nav_asset_precision: 19 }],
  }), /supported range/);
  assert.throws(() => navcoinMarketsFromRoutes({
    schema: 'postfiat-pftl-uniswap-routes-status-v2', route_count: 1,
    routes: [{ ...route, route_id: '-pftl-invalid-route' }],
  }), /route id/);
});
