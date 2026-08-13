'use strict';

const ROUTE_ID = 'pftl-a666-ethereum-wA666-usdc-v1';
const ETHEREUM_CHAIN_ID = 1;
const STATE_VIEW = '0x7fFE42C4a5DEeA5b0feC41C94C136Cf115597227';
const POOL_ID = '0xc5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98';
const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
const WA666 = '0xee4c92edb03efdd9b519339edc19ad70c69a9be5';
const GET_SLOT0_SELECTOR = 'c815641c';
const GET_LIQUIDITY_SELECTOR = 'fa6793d5';
const Q96 = 1n << 96n;
const PRICE_SCALE = 1_000_000n;
const MAX_RESPONSE_BYTES = 64 * 1024;

function ethereumRpcUrl(raw = process.env.NAVCOIN_MARKET_DATA_RPC_URL
    || process.env.ETHEREUM_MAINNET_RPC_URL
    || 'https://ethereum-rpc.publicnode.com') {
    const url = new URL(String(raw));
    const loopback = ['127.0.0.1', 'localhost', '::1'].includes(url.hostname);
    if (url.protocol !== 'https:' && !(url.protocol === 'http:' && loopback)) {
        throw new Error('NAVCoin market-data RPC must use HTTPS or loopback HTTP');
    }
    return url.toString();
}

async function jsonRpc(method, params, options = {}) {
    const fetchImpl = options.fetchImpl || fetch;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), options.timeoutMs || 10_000);
    try {
        const response = await fetchImpl(options.rpcUrl || ethereumRpcUrl(), {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
            body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
            signal: controller.signal,
        });
        if (!response.ok) throw new Error(`Ethereum RPC returned HTTP ${response.status}`);
        const raw = await response.text();
        if (Buffer.byteLength(raw, 'utf8') > MAX_RESPONSE_BYTES) {
            throw new Error('Ethereum RPC response exceeded the market-data limit');
        }
        const payload = JSON.parse(raw);
        if (payload?.id !== 1 || typeof payload?.result !== 'string' || payload.error) {
            throw new Error('Ethereum RPC returned an invalid market-data response');
        }
        return payload.result;
    } finally {
        clearTimeout(timer);
    }
}

function uintWord(result, index, label) {
    const hex = String(result || '').toLowerCase();
    if (!/^0x[0-9a-f]+$/.test(hex)) throw new Error(`${label} response is not hexadecimal`);
    const payload = hex.slice(2);
    const start = index * 64;
    if (payload.length < start + 64) throw new Error(`${label} response is truncated`);
    return BigInt(`0x${payload.slice(start, start + 64)}`);
}

function signedInt24(word) {
    const masked = Number(word & 0xffffffn);
    return masked >= 0x800000 ? masked - 0x1000000 : masked;
}

function callData(selector) {
    return `0x${selector}${POOL_ID.slice(2)}`;
}

function parsePoolState(slot0Result, liquidityResult, blockResult) {
    const sqrtPriceX96 = uintWord(slot0Result, 0, 'Uniswap slot0');
    const tick = signedInt24(uintWord(slot0Result, 1, 'Uniswap slot0'));
    const protocolFee = uintWord(slot0Result, 2, 'Uniswap slot0');
    const lpFee = uintWord(slot0Result, 3, 'Uniswap slot0');
    const liquidity = uintWord(liquidityResult, 0, 'Uniswap liquidity');
    if (sqrtPriceX96 === 0n || liquidity === 0n) throw new Error('Uniswap pool is not initialized');
    const denominator = sqrtPriceX96 * sqrtPriceX96;
    const usdcAtomsPerWa666 = (Q96 * Q96 * PRICE_SCALE) / denominator;
    const blockNumber = BigInt(blockResult);
    return {
        sqrt_price_x96: sqrtPriceX96.toString(),
        tick,
        protocol_fee: protocolFee.toString(),
        lp_fee: lpFee.toString(),
        liquidity: liquidity.toString(),
        spot_usdc_atoms_per_wa666: usdcAtomsPerWa666.toString(),
        ethereum_block_number: blockNumber.toString(),
    };
}

function create(_runtime = {}, options = {}) {
    const rpcCall = options.rpcCall || ((method, params) => jsonRpc(method, params, options));
    const cacheMs = options.cacheMs ?? 10_000;
    let cached = null;

    async function navcoinMarketData(routeId) {
        if (routeId !== ROUTE_ID) {
            throw Object.assign(new Error('NAVCoin market-data route is unsupported'), {
                code: 'navcoin_market_data_route_unsupported',
            });
        }
        if (cached && Date.now() - cached.cachedAt < cacheMs) return cached.value;
        const [slot0, liquidity, block] = await Promise.all([
            rpcCall('eth_call', [{ to: STATE_VIEW, data: callData(GET_SLOT0_SELECTOR) }, 'latest']),
            rpcCall('eth_call', [{ to: STATE_VIEW, data: callData(GET_LIQUIDITY_SELECTOR) }, 'latest']),
            rpcCall('eth_blockNumber', []),
        ]);
        const value = {
            ok: true,
            schema: 'postfiat-navcoin-secondary-market-data-v1',
            route_id: ROUTE_ID,
            ethereum_chain_id: ETHEREUM_CHAIN_ID,
            base_token: WA666,
            quote_token: USDC,
            state_view: STATE_VIEW.toLowerCase(),
            pool_id: POOL_ID,
            ...parsePoolState(slot0, liquidity, block),
            observed_at_unix: Math.floor(Date.now() / 1000),
        };
        cached = { cachedAt: Date.now(), value };
        return value;
    }

    return { navcoinMarketData };
}

module.exports = {
    ETHEREUM_CHAIN_ID,
    POOL_ID,
    ROUTE_ID,
    STATE_VIEW,
    USDC,
    WA666,
    create,
    ethereumRpcUrl,
    parsePoolState,
};
