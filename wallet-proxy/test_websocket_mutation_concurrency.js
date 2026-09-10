'use strict';

const assert = require('assert');
const net = require('net');
const WebSocket = require('ws');

const TOKEN = 'websocket-concurrency-token-32-bytes-minimum';
const ORIGIN = 'https://wallet.example.test';

function websocketCall(port, id) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}`, { origin: ORIGIN });
    const timer = setTimeout(() => {
      ws.terminate();
      reject(new Error(`${id} response timed out`));
    }, 5_000);
    ws.on('open', () => {
      ws.send(JSON.stringify({
        version: 'postfiat-local-rpc-v1',
        id,
        method: 'mempool_submit_signed_transfer',
        params: { signed_transfer_json: '{}' },
        proxy_auth_token: TOKEN,
      }));
    });
    ws.on('message', (message) => {
      clearTimeout(timer);
      ws.close();
      resolve(JSON.parse(message.toString('utf8')));
    });
    ws.on('error', reject);
  });
}

async function main() {
  let firstUpstreamSocket = null;
  let markFirstReceived;
  const firstReceived = new Promise((resolve) => { markFirstReceived = resolve; });
  const upstream = net.createServer((socket) => {
    socket.setEncoding('utf8');
    socket.once('data', (line) => {
      const request = JSON.parse(line.trim());
      if (request.id === 'first') {
        firstUpstreamSocket = socket;
        markFirstReceived();
        return;
      }
      socket.end(JSON.stringify({
        version: 'postfiat-local-rpc-v1',
        id: request.id,
        ok: true,
        result: {},
        error: null,
        events: [],
      }) + '\n');
    });
  });
  await new Promise((resolve) => upstream.listen(0, '127.0.0.1', resolve));

  process.env.RPC_HOST = '127.0.0.1';
  process.env.RPC_PORT = String(upstream.address().port);
  process.env.ENABLE_PROPOSER_ROUTING = 'false';
  process.env.ALLOWED_ORIGINS = ORIGIN;
  process.env.WALLET_PROXY_API_TOKEN = TOKEN;
  process.env.WALLET_PROXY_MUTATION_CONCURRENCY = '1';
  process.env.WALLET_PROXY_MUTATION_RATE_LIMIT = '10';

  const { server } = require('./server');
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const proxyPort = server.address().port;

  try {
    const first = websocketCall(proxyPort, 'first');
    await firstReceived;

    const blocked = await websocketCall(proxyPort, 'second');
    assert.strictEqual(blocked.ok, false);
    assert.strictEqual(blocked.error.code, 'proxy_mutation_concurrency_limited');

    firstUpstreamSocket.end(JSON.stringify({
      version: 'postfiat-local-rpc-v1',
      id: 'first',
      ok: true,
      result: {},
      error: null,
      events: [],
    }) + '\n');
    const completed = await first;
    assert.strictEqual(completed.ok, true);

    // The upstream responds automatically for every request except `first`,
    // so this proves the shared slot was released after the first response.
    const third = await websocketCall(proxyPort, 'third');
    assert.strictEqual(third.ok, true);

    console.log('WebSocket mutation concurrency lifecycle regression passed');
  } finally {
    firstUpstreamSocket?.destroy();
    await new Promise((resolve) => server.close(resolve));
    await new Promise((resolve) => upstream.close(resolve));
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
