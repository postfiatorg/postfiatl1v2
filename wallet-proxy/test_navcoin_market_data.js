'use strict';

const assert = require('node:assert/strict');
const {
    POOL_ID,
    ROUTE_ID,
    STATE_VIEW,
    create,
    ethereumRpcUrl,
    parsePoolState,
} = require('./navcoin-market-data');

function word(value) {
    return BigInt(value).toString(16).padStart(64, '0');
}

const sqrtPriceX96 = 90396703501139385985669265699n;
const slot0 = `0x${word(sqrtPriceX96)}${word(2637)}${word(500)}${word(3_000_000)}`;
const liquidity = `0x${word(512125)}`;
const parsed = parsePoolState(slot0, liquidity, '0x1886fae');
assert.equal(parsed.sqrt_price_x96, sqrtPriceX96.toString());
assert.equal(parsed.tick, 2637);
assert.equal(parsed.liquidity, '512125');
assert.equal(parsed.spot_usdc_atoms_per_wa666, '768164');

assert.throws(() => ethereumRpcUrl('http://example.com'), /HTTPS or loopback/);
assert.equal(ethereumRpcUrl('http://127.0.0.1:8545'), 'http://127.0.0.1:8545/');

const calls = [];
const market = create({}, {
    cacheMs: 60_000,
    rpcCall: async (method, params) => {
        calls.push({ method, params });
        if (method === 'eth_blockNumber') return '0x1886fae';
        if (params[0].to !== STATE_VIEW) throw new Error('wrong StateView');
        if (params[0].data.endsWith(POOL_ID.slice(2)) === false) throw new Error('wrong pool');
        return params[0].data.startsWith('0xc815641c') ? slot0 : liquidity;
    },
});

(async () => {
    const first = await market.navcoinMarketData(ROUTE_ID);
    const second = await market.navcoinMarketData(ROUTE_ID);
    assert.equal(first.schema, 'postfiat-navcoin-secondary-market-data-v1');
    assert.equal(first.spot_usdc_atoms_per_wa666, '768164');
    assert.equal(second, first);
    assert.equal(calls.length, 3, 'cached read must not repeat Ethereum calls');
    await assert.rejects(() => market.navcoinMarketData('wrong-route'), /unsupported/);
    console.log('NAVCoin secondary-market data regression passed');
})().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
