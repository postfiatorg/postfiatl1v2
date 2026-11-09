'use strict';

// The four anonymous private reads share one upstream call and one successful
// answer for two seconds per method, params and endpoint; each caller keeps
// its own request id, and refusals and transport errors are never kept.

const assert = require('assert');
const net = require('net');
const WebSocket = require('ws');
const {
    PUBLIC_READ_CACHE_METHODS, PUBLIC_READ_CACHE_TTL_MS, createPublicReadCache,
} = require('./public-read-cache');

function websocketCall(port, id, method, params) {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(`ws://127.0.0.1:${port}`);
        const timer = setTimeout(() => { ws.terminate(); reject(new Error(`${id} timed out`)); }, 5_000);
        ws.on('open', () => ws.send(JSON.stringify({ version: 'postfiat-local-rpc-v1', id, method, params })));
        ws.on('message', (message) => {
            clearTimeout(timer);
            ws.close();
            resolve(JSON.parse(message.toString('utf8')));
        });
        ws.on('error', reject);
    });
}

async function unit() {
    assert.deepStrictEqual([...PUBLIC_READ_CACHE_METHODS].sort(), [
        'asset_orchard_private_egress_binding', 'private_pool_page', 'private_pool_snapshot', 'private_readiness',
    ]);
    assert.strictEqual(PUBLIC_READ_CACHE_TTL_MS, 2_000);
    let now = 0;
    let loads = 0;
    const cache = createPublicReadCache({ nowMs: () => now, maxEntries: 4 });
    const ok = (tag) => async () => { loads += 1; return { id: 'upstream', ok: true, result: { tag, loads } }; };

    // Concurrent identical reads share one load; key order in params does not matter.
    const [a, b] = await Promise.all([
        cache.read('private_pool_page', { section: 'outputs', snapshot_token: 't' }, 'a', ok('x'), 'v0'),
        cache.read('private_pool_page', { snapshot_token: 't', section: 'outputs' }, 'b', ok('x'), 'v0'),
    ]);
    assert.strictEqual(loads, 1);
    assert.deepStrictEqual([a.id, b.id], ['a', 'b']);
    assert.deepStrictEqual(a.result, b.result);

    // Kept until the TTL, then loaded again.
    now = PUBLIC_READ_CACHE_TTL_MS - 1;
    await cache.read('private_pool_page', { section: 'outputs', snapshot_token: 't' }, 'c', ok('x'), 'v0');
    assert.strictEqual(loads, 1);
    now = PUBLIC_READ_CACHE_TTL_MS + 1;
    await cache.read('private_pool_page', { section: 'outputs', snapshot_token: 't' }, 'd', ok('x'), 'v0');
    assert.strictEqual(loads, 2);

    // Different params, method or endpoint never share an answer.
    await cache.read('private_pool_page', { section: 'commitments', snapshot_token: 't' }, 'e', ok('y'), 'v0');
    await cache.read('private_pool_page', { section: 'outputs', snapshot_token: 't' }, 'f', ok('z'), 'v1');
    await cache.read('private_readiness', {}, 'g', ok('r'), 'v0');
    assert.strictEqual(loads, 5);

    // Refusals and transport errors are not kept.
    let refusals = 0;
    const refuse = async () => { refusals += 1; return { id: 'upstream', ok: false, error: { code: 'rpc_busy' } }; };
    assert.strictEqual((await cache.read('private_pool_snapshot', { pool_id: 'p' }, 'h', refuse)).ok, false);
    await cache.read('private_pool_snapshot', { pool_id: 'p' }, 'i', refuse);
    assert.strictEqual(refusals, 2);
    let failures = 0;
    const fail = async () => { failures += 1; throw new Error('transport'); };
    await assert.rejects(cache.read('private_pool_snapshot', { pool_id: 'q' }, 'j', fail), /transport/);
    await assert.rejects(cache.read('private_pool_snapshot', { pool_id: 'q' }, 'k', fail), /transport/);
    assert.strictEqual(failures, 2);

    // Bounded, and refuses methods outside the list.
    assert.ok(cache.size() <= 4);
    await assert.rejects(cache.read('private_account_lines', {}, 'l', ok('n')), /not a cached public read/);
}

async function endToEnd() {
    const upstreamIds = [];
    const upstream = net.createServer((socket) => {
        socket.setEncoding('utf8');
        let buffer = '';
        socket.on('data', (chunk) => {
            buffer += chunk;
            let idx;
            while ((idx = buffer.indexOf('\n')) >= 0) {
                const request = JSON.parse(buffer.slice(0, idx));
                buffer = buffer.slice(idx + 1);
                upstreamIds.push(`${request.method}:${request.id}`);
                socket.write(JSON.stringify({
                    version: 'postfiat-local-rpc-v1', id: request.id, ok: true,
                    result: { method: request.method, served: upstreamIds.length }, error: null, events: [],
                }) + '\n');
            }
        });
    });
    await new Promise((resolve) => upstream.listen(0, '127.0.0.1', resolve));
    Object.assign(process.env, {
        RPC_HOST: '127.0.0.1',
        RPC_PORT: String(upstream.address().port),
        ENABLE_PROPOSER_ROUTING: 'false',
        WALLET_PROXY_API_TOKEN: 'public-read-cache-token-32-bytes-minimum',
        NAVSWAP_IDEMPOTENCY_STORE_PATH: 'off',
        NAVSWAP_RUN_STORE_PATH: 'off',
    });
    delete process.env.LISTEN_HOST;
    const { server, closeUpstreamRpcConnections } = require('./server');
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const port = server.address().port;
    try {
        const params = { snapshot_token: 'pps1:fixture', section: 'outputs', limit: 64 };
        const answers = await Promise.all(['p1', 'p2', 'p3'].map((id) => websocketCall(port, id, 'private_pool_page', params)));
        assert.deepStrictEqual(answers.map((answer) => answer.id), ['p1', 'p2', 'p3']);
        assert.ok(answers.every((answer) => answer.ok && answer.result.served === 1));
        assert.deepStrictEqual(upstreamIds, ['private_pool_page:p1']);
        // Uncached public reads still go upstream every time.
        await websocketCall(port, 's1', 'status', {});
        await websocketCall(port, 's2', 'status', {});
        assert.deepStrictEqual(upstreamIds.slice(1), ['status:s1', 'status:s2']);
    } finally {
        server.close();
        closeUpstreamRpcConnections?.();
        upstream.close();
    }
}

async function main() {
    await unit();
    console.log('PASS public read cache: shared loads, TTL, scope, no refusal kept, bounded');
    await endToEnd();
    console.log('PASS public read cache: one upstream call for concurrent gateway reads, each with its own id');
}

main().then(() => process.exit(0), (error) => { console.error(error); process.exit(1); });
