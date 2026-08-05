import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

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
  route_live: true,
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

const ROUTE_LIVE_FILTER_VECTOR_NAME = 'governed NAVCoin markets consume only route_live registry rows';

const journeyRoute = {
  ...route,
  route_id: 'pftl-a666-r4-offline-rehearsal-v1',
  route_config_digest: 'b2'.repeat(48),
  route_live: true,
};

const inactiveEpochRoute = {
  ...route,
  route_id: 'pftl-a666-ethereum-wA666-usdc-v1',
  route_config_digest: 'a4'.repeat(48),
  wrapped_navcoin_token: `0x${'66'.repeat(20)}`,
  handoff_controller: `0x${'77'.repeat(20)}`,
  live_value_enabled: false,
  route_live: false,
};

test(ROUTE_LIVE_FILTER_VECTOR_NAME, () => {
  const markets = navcoinMarketsFromRoutes({
    schema: 'postfiat-pftl-uniswap-routes-status-v2',
    route_count: 2,
    routes: [inactiveEpochRoute, journeyRoute],
  });
  assert.equal(markets.length, 1);
  assert.equal(markets[0].routeId, journeyRoute.route_id);
});

test('governed NAVCoin markets reject duplicate live route identities', () => {
  assert.throws(() => navcoinMarketsFromRoutes({
    schema: 'postfiat-pftl-uniswap-routes-status-v2',
    route_count: 2,
    routes: [
      journeyRoute,
      {
        ...journeyRoute,
        route_id: 'pftl-a666-r4-second-live-v1',
        route_config_digest: 'c3'.repeat(48),
        wrapped_navcoin_token: `0x${'88'.repeat(20)}`,
        handoff_controller: `0x${'99'.repeat(20)}`,
      },
    ],
  }), /duplicate identity/);
});

test('governed NAVCoin markets fail closed on missing or non-boolean route_live', () => {
  const missing = { ...journeyRoute };
  delete missing.route_live;
  assert.throws(() => navcoinMarketsFromRoutes({
    schema: 'postfiat-pftl-uniswap-routes-status-v2', route_count: 1, routes: [missing],
  }), /route_live/);
  assert.throws(() => navcoinMarketsFromRoutes({
    schema: 'postfiat-pftl-uniswap-routes-status-v2', route_count: 1,
    routes: [{ ...journeyRoute, route_live: 'true' }],
  }), /route_live/);
});

test('route_live filter regression vector is structurally retained', () => {
  const source = readFileSync(fileURLToPath(import.meta.url), 'utf8');
  assert.match(source, /const ROUTE_LIVE_FILTER_VECTOR_NAME = 'governed NAVCoin markets consume only route_live registry rows'/);
  assert.match(source, /route_live: false/);
  assert.match(source, /route_live: true/);
  assert.match(source, /test\(ROUTE_LIVE_FILTER_VECTOR_NAME, \(\) =>/);
  assert.doesNotMatch(source, /test\(ROUTE_LIVE_FILTER_VECTOR_NAME, \{ todo:/);
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
