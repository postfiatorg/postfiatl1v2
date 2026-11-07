'use strict';

const assert = require('assert');
const { EventEmitter } = require('events');
const { create } = require('./rpc-routing');

class Socket extends EventEmitter {
    constructor() { super(); this.destroyed = false; this.readyState = 'opening'; this.writes = []; }
    setKeepAlive() {}
    write(value) { this.writes.push(JSON.parse(value)); }
    destroy() { this.destroyed = true; this.readyState = 'closed'; }
    connect() { this.readyState = 'open'; this.emit('connect'); }
    reply(value) { this.emit('data', Buffer.from(JSON.stringify(value) + '\n')); }
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
}
main().catch(error => { console.error(error); process.exitCode = 1; });
