import assert from 'node:assert/strict';
import test from 'node:test';

import {
  DEFAULT_NAVCOIN_MARKET,
  NAVCOIN_MARKETS,
  displayAssetSymbol,
  navcoinMarketByKey,
  navcoinMarketForAsset,
  navcoinMarketForSettlementAsset,
} from './navcoin-markets.js';

test('governed NAVCoin registry has unique route, native asset, and wrapped token identities', () => {
  assert.ok(NAVCOIN_MARKETS.length > 0);
  for (const field of ['key', 'routeId', 'navAssetId', 'wrappedToken']) {
    const values = NAVCOIN_MARKETS.map(market => market[field].toLowerCase());
    assert.equal(new Set(values).size, values.length, `${field} identities must be unique`);
  }
});

test('NAVCoin display and navigation resolve through registry metadata', () => {
  const market = DEFAULT_NAVCOIN_MARKET;
  assert.equal(navcoinMarketByKey(market.key), market);
  assert.equal(navcoinMarketForAsset(market.navAssetId), market);
  assert.equal(navcoinMarketForSettlementAsset(market.settlementAssetId), market);
  assert.equal(displayAssetSymbol(market.navAssetId, 'fallback'), market.symbol);
  assert.equal(displayAssetSymbol(market.settlementAssetId, 'fallback'), market.settlementSymbol);
  assert.equal(displayAssetSymbol('unknown', 'fallback'), 'fallback');
});
