'use strict';

const assert = require('assert');
const net = require('net');

async function main() {
  const connectionIds = new WeakMap();
  const requestConnections = [];
  let nextConnectionId = 0;
  const server = net.createServer(socket => {
    const connectionId = ++nextConnectionId;
    connectionIds.set(socket, connectionId);
    let buffer = '';
    socket.on('data', chunk => {
      buffer += chunk.toString('utf8');
      let newline;
      while ((newline = buffer.indexOf('\n')) >= 0) {
        const request = JSON.parse(buffer.slice(0, newline));
        buffer = buffer.slice(newline + 1);
        requestConnections.push({ id: request.id, connectionId });
        socket.write(`${JSON.stringify({
          version: 'postfiat-local-rpc-v1',
          id: request.id,
          ok: true,
          result: { accepted: true },
          error: null,
          events: [],
        })}\n`);
        if (request.method === 'owned_apply') {
          socket.end();
        }
      }
    });
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));

  process.env.RPC_FLEET = `validator-0=127.0.0.1:${server.address().port}`;
  process.env.FASTPAY_ROUTE_WARMUP_ENABLED = 'false';
  const { rpcTcpRequest, closeUpstreamRpcConnections, compactFastpayVoteRequest } = require('./server');
  const largeOrderJson = JSON.stringify({ owner_pubkey_hex: 'ab'.repeat(2000), owner_signature_hex: 'cd'.repeat(3300) });
  const compact = compactFastpayVoteRequest({
    version: 'postfiat-local-rpc-v1', id: 'compact', method: 'owned_sign',
    params: { validator_id: 'validator-0', order_json: largeOrderJson },
  });
  assert.strictEqual(compact.params.order_json, undefined);
  assert.ok(compact.params.order_json_gzip_base64.length < largeOrderJson.length);
  assert.strictEqual(
    require('zlib').gunzipSync(Buffer.from(compact.params.order_json_gzip_base64, 'base64')).toString('utf8'),
    largeOrderJson,
  );
  const request = (id, method, channel) => rpcTcpRequest(
    '127.0.0.1',
    server.address().port,
    { version: 'postfiat-local-rpc-v1', id, method, params: {} },
    1000,
    channel,
  );

  await request('vote-before', 'owned_sign', 'fastpay-vote');
  await request('apply', 'owned_apply', 'fastpay-apply');
  await request('vote-after', 'owned_sign', 'fastpay-vote');

  const byId = Object.fromEntries(requestConnections.map(row => [row.id, row.connectionId]));
  assert.strictEqual(byId['vote-before'], byId['vote-after'], 'vote lane must stay hot after apply closes');
  assert.notStrictEqual(byId.apply, byId['vote-before'], 'apply must use an isolated connection');

  closeUpstreamRpcConnections();
  await new Promise(resolve => server.close(resolve));
  console.log('PASS FastPay apply closure cannot evict the persistent vote session');
}

main().catch(error => {
  console.error(error);
  process.exit(1);
});
