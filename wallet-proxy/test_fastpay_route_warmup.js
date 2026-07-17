'use strict';

const assert = require('assert');
const net = require('net');

const VALIDATOR_COUNT = 6;

async function main() {
  const servers = [];
  const ports = [];
  const statusCounts = new Map();

  for (let i = 0; i < VALIDATOR_COUNT; i += 1) {
    const validatorId = `validator-${i}`;
    const server = net.createServer(socket => {
      let buffer = '';
      socket.on('data', chunk => {
        buffer += chunk.toString('utf8');
        let newline;
        while ((newline = buffer.indexOf('\n')) >= 0) {
          const request = JSON.parse(buffer.slice(0, newline));
          buffer = buffer.slice(newline + 1);
          const count = (statusCounts.get(validatorId) || 0) + 1;
          statusCounts.set(validatorId, count);
          // The initial route-prime wave is immediate. A later health refresh is
          // deliberately slow so we can prove it does not block vote routing.
          const delayMs = count === 1 ? 0 : 150;
          setTimeout(() => socket.write(`${JSON.stringify({
            version: 'postfiat-local-rpc-v1',
            id: request.id,
            ok: true,
            result: {
              block_height: 100,
              block_tip_hash: 'tip',
              state_root: 'root',
              validator_id: validatorId,
              validator_count: VALIDATOR_COUNT,
              chain_id: 'postfiat-wan-devnet',
            },
            error: null,
            events: [],
          })}\n`), delayMs);
        }
      });
    });
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    servers.push(server);
    ports.push(server.address().port);
  }

  process.env.RPC_FLEET = ports.map(
    (port, i) => `validator-${i}=127.0.0.1:${port}`,
  ).join(',');
  process.env.FASTPAY_ROUTE_WARMUP_ENABLED = 'true';
  process.env.FASTPAY_FLEET_STATUS_CACHE_MS = '1000';
  process.env.FASTPAY_ROUTE_REFRESH_MS = '500';
  process.env.FASTPAY_CERTIFICATE_OUTBOX_PATH = `/tmp/postfiat-fastpay-route-warmup-${process.pid}.json`;

  const {
    chooseOwnedVoteEndpoint,
    closeUpstreamRpcConnections,
    collectFastpayFleetStatuses,
    startFastpayRouteWarmup,
  } = require('./server');

  const warmup = startFastpayRouteWarmup();
  const initial = await warmup.initial;
  assert.strictEqual(initial.converged_count, VALIDATOR_COUNT);

  const refresh = collectFastpayFleetStatuses(
    ports.map((port, i) => ({ validatorId: `validator-${i}`, host: '127.0.0.1', port })),
    { forceRefresh: true, channel: 'status' },
  );
  const started = Date.now();
  const selected = await chooseOwnedVoteEndpoint({
    params: { validator_id: 'validator-4' },
  });
  const routeMs = Date.now() - started;
  assert.strictEqual(selected.endpoint.validatorId, 'validator-4');
  assert.ok(routeMs < 75, `fresh cached route waited for background refresh: ${routeMs}ms`);
  await refresh;

  closeUpstreamRpcConnections();
  for (const server of servers) server.close();
  console.log('PASS FastPay primes vote sessions and health refresh never blocks a fresh cached vote route');
}

main().catch(error => {
  console.error(error);
  process.exit(1);
});
