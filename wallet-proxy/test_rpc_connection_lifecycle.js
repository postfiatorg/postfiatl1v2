'use strict';

const assert = require('assert');
const fs = require('fs');
const path = require('path');
const { EventEmitter } = require('events');
const { create } = require('./rpc-routing');

class Socket extends EventEmitter {
    constructor() { super(); this.destroyed = false; this.readyState = 'opening'; this.writes = []; }
    setKeepAlive() {}
    write(value) { this.writes.push(JSON.parse(value)); }
    destroy() { this.destroyed = true; this.readyState = 'closed'; }
    connect() { this.readyState = 'open'; this.emit('connect'); }
    reply(value) { this.emit('data', Buffer.from(JSON.stringify(value) + '\n')); }
    end() {}
}

const settledWithin = (promise, ms) => Promise.race([
    promise.then((value) => ({ state: 'resolved', value }), (error) => ({ state: 'rejected', error })),
    new Promise((resolve) => setTimeout(() => resolve({ state: 'pending' }), ms)),
]);

async function check(name, run) {
    try {
        await run();
        console.log(`PASS ${name}`);
    } catch (error) {
        console.error(`FAIL ${name}: ${error.message.split('\n')[0]}`);
        process.exitCode = 1;
    }
}

function fixture(extra = {}) {
    const sockets = [];
    const routing = create({ TCP_TIMEOUT_MS: 1000, ...extra,
        net: { connect() { const socket = new Socket(); sockets.push(socket); return socket; } } });
    return { sockets, routing, connection: new routing.UpstreamRpcConnection('fixture', 1) };
}

async function main() {
    const sockets = [];
    const { UpstreamRpcConnection } = create({ TCP_TIMEOUT_MS: 1000,
        net: { connect() { const socket = new Socket(); sockets.push(socket); return socket; } } });
    const connection = new UpstreamRpcConnection('fixture', 1);
    const first = connection.requestLine({ id: 'first', method: 'private_pool_page' });
    const failed = assert.rejects(first, /fixture reset/);
    const old = sockets[0]; old.connect(); old.emit('error', new Error('fixture reset'));
    await failed;
    const second = connection.requestLine({ id: 'second', method: 'private_pool_page' });
    const replacement = sockets[1]; replacement.connect();
    old.emit('close'); old.emit('connect'); old.reply({ id: 'stale' });
    assert.strictEqual(connection.socket, replacement);
    replacement.reply({ id: 'second' });
    assert.deepStrictEqual(JSON.parse(await second), { id: 'second' });

    // Complete 64 frames, then submit a mutation. It must use a new socket
    // exactly once, and a late FIN from the retired socket must not reject it.
    for (let i = 1; i < 64; i += 1) {
        const read = connection.requestLine({ id: `read-${i}`, method: 'private_pool_page' });
        replacement.reply({ id: `read-${i}` }); await read;
    }
    assert.ok(replacement.destroyed);
    const mutation = connection.requestLine({ id: 'mutation', method: 'private_action_submit' });
    const third = sockets[2]; third.connect(); replacement.emit('close');
    assert.deepStrictEqual(third.writes.map(row => row.id), ['mutation']);
    third.reply({ id: 'mutation' }); await mutation;
    assert.strictEqual(sockets.flatMap(socket => socket.writes).filter(row => row.id === 'mutation').length, 1);
    connection.close();
    console.log('PASS stale socket events and 64-request retirement preserve replacement RPC connections');

    // The ceiling is the node's own constant.
    await check('the request ceiling and idle retirement follow rpc-serve', async () => {
        const rpcCli = fs.readFileSync(path.join(__dirname, '..', 'crates', 'node', 'src', 'rpc_cli.rs'), 'utf8');
        const nodeCeiling = Number(/const RPC_SERVE_MAX_REQUESTS_PER_CONNECTION: u64 = (\d+);/.exec(rpcCli)?.[1]);
        const { routing } = fixture();
        assert.strictEqual(routing.UPSTREAM_MAX_REQUESTS_PER_CONNECTION, nodeCeiling);
        const nodeIdleMs = Number(/const RPC_SERVE_CONNECTION_READ_TIMEOUT_MS: u64 = ([\d_]+);/.exec(rpcCli)?.[1].replace(/_/g, ''));
        assert.ok(routing.UPSTREAM_IDLE_RETIRE_MS > 0 && routing.UPSTREAM_IDLE_RETIRE_MS <= nodeIdleMs - 5000);
    });

    // A one-shot request refuses a response for another id.
    await check('a one-shot request refuses a mismatched response id', async () => {
        const { sockets, routing } = fixture();
        const oneShot = routing.rpcTcpRequestOneShotLine('fixture', 1, { id: 'mine', method: 'status' }, 1000);
        sockets[0].connect(); sockets[0].reply({ id: 'someone-else' });
        await assert.rejects(oneShot, (error) => error.transportClass === 'rpc_response_id_mismatch');
    });

    // A connect that never completes is bounded by the request timeout.
    await check('a hung connect fails within the request timeout', async () => {
        const { sockets, connection: hung } = fixture();
        const outcome = await settledWithin(hung.requestLine({ id: 'hung', method: 'status' }, 50), 500);
        assert.strictEqual(outcome.state, 'rejected');
        assert.strictEqual(outcome.error.transportClass, 'rpc_timeout');
        assert.ok(sockets[0].destroyed);
        assert.deepStrictEqual(sockets[0].writes, []);
        hung.close();
    });

    // A response whose id differs from the request is refused; the queue
    // continues on a new socket and nothing is written twice.
    await check('a mismatched response id is refused without replay', async () => {
        const { sockets, connection: c } = fixture();
        const mismatched = c.requestLine({ id: 'mine', method: 'status' });
        const next = c.requestLine({ id: 'next', method: 'private_action_submit' });
        sockets[0].connect();
        sockets[0].reply({ id: 'someone-else' });
        await assert.rejects(mismatched, (error) => error.transportClass === 'rpc_response_id_mismatch');
        assert.ok(sockets[0].destroyed);
        sockets[1].connect();
        sockets[1].reply({ id: 'next' });
        assert.deepStrictEqual(JSON.parse(await next), { id: 'next' });
        assert.deepStrictEqual(sockets.map((socket) => socket.writes.map((row) => row.id)), [['mine'], ['next']]);
        c.close();
    });

    // An idle socket is retired before the node's idle close can race a write.
    await check('an idle socket is retired before reuse', async () => {
        let now = 1_000_000;
        const { sockets, routing: idleRouting, connection: c } = fixture({ upstreamClockMs: () => now });
        const first = c.requestLine({ id: 'first', method: 'status' });
        sockets[0].connect(); sockets[0].reply({ id: 'first' }); await first;
        now += idleRouting.UPSTREAM_IDLE_RETIRE_MS - 1;
        const warm = c.requestLine({ id: 'warm', method: 'status' });
        assert.strictEqual(sockets.length, 1);
        sockets[0].reply({ id: 'warm' }); await warm;
        now += idleRouting.UPSTREAM_IDLE_RETIRE_MS;
        const idle = c.requestLine({ id: 'idle', method: 'status' });
        assert.strictEqual(sockets.length, 2);
        assert.ok(sockets[0].destroyed);
        assert.deepStrictEqual(sockets[0].writes.map((row) => row.id), ['first', 'warm']);
        sockets[1].connect(); sockets[1].reply({ id: 'idle' });
        assert.deepStrictEqual(JSON.parse(await idle), { id: 'idle' });
        sockets[0].emit('close');
        assert.strictEqual(c.socket, sockets[1]);
        c.close();
    });

    // A half-closed socket that is replaced is destroyed.
    await check('a replaced half-closed socket is destroyed', async () => {
        const { sockets, connection: c } = fixture();
        const first = c.requestLine({ id: 'first', method: 'status' });
        sockets[0].connect(); sockets[0].reply({ id: 'first' }); await first;
        sockets[0].readyState = 'writeOnly';
        const second = c.requestLine({ id: 'second', method: 'status' });
        assert.ok(sockets[0].destroyed);
        sockets[1].connect(); sockets[1].reply({ id: 'second' }); await second;
        c.close();
    });

    // Transport errors carry their class from the throw site.
    await check('transport errors are classed where they are thrown', async () => {
        const { sockets, connection: c } = fixture();
        const timedOut = c.requestLine({ id: 'slow', method: 'status' }, 20);
        sockets[0].connect();
        await assert.rejects(timedOut, (error) => error.transportClass === 'rpc_timeout');
        const closed = c.requestLine({ id: 'closed', method: 'status' });
        sockets[1].connect(); sockets[1].emit('close');
        await assert.rejects(closed, (error) => error.transportClass === 'rpc_connection_closed');
        c.close();
    });
}
main().catch(error => { console.error(error); process.exitCode = 1; });
