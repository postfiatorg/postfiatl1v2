import assert from 'node:assert/strict';
import test from 'node:test';

import { navDiscountPremiumBps, parseNavcoinSecondaryMarketData } from './navcoin-secondary-market.js';

const market = {
  routeId: 'pftl-a666-ethereum-wA666-usdc-v1',
  ethereumChainId: 1,
  wrappedToken: '0xee4c92edb03efdd9b519339edc19ad70c69a9be5',
};

function payload(overrides = {}) {
  return {
    ok: true,
    schema: 'postfiat-navcoin-secondary-market-data-v1',
    route_id: market.routeId,
    ethereum_chain_id: 1,
    base_token: market.wrappedToken,
    quote_token: '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48',
    pool_id: `0x${'12'.repeat(32)}`,
    spot_usdc_atoms_per_wa666: '768164',
    liquidity: '512125',
    ethereum_block_number: '25718670',
    observed_at_unix: 1786566000,
    ...overrides,
  };
}

test('secondary market data binds the live Ethereum pool to the governed NAV route', () => {
  const parsed = parseNavcoinSecondaryMarketData(payload(), market);
  assert.equal(parsed.spotUsdcAtoms, '768164');
  assert.equal(parsed.ethereumBlockNumber, '25718670');
  assert.equal(navDiscountPremiumBps(parsed.spotUsdcAtoms, '902480'), -1488n);
});

test('secondary market data fails closed on route or token substitution', () => {
  assert.throws(() => parseNavcoinSecondaryMarketData(payload({ route_id: 'wrong' }), market), /identity/);
  assert.throws(() => parseNavcoinSecondaryMarketData(payload({ base_token: `0x${'34'.repeat(20)}` }), market), /token identity/);
});
