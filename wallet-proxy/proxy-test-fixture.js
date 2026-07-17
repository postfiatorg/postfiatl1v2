'use strict';

const fs = require('fs');
const net = require('net');
const os = require('os');
const path = require('path');
const { spawn } = require('child_process');

function listen(server) {
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      server.removeListener('error', reject);
      resolve(server.address().port);
    });
  });
}

async function reservePort() {
  const reservation = net.createServer();
  const port = await listen(reservation);
  await new Promise(resolve => reservation.close(resolve));
  return port;
}

function defaultRpcResult(request) {
  const fundedAddress = `pf${'1'.repeat(40)}`;
  const results = {
    status: {
      block_height: 470,
      block_tip_hash: 'fixture-tip',
      state_root: 'fixture-root',
      chain_id: 'postfiat-wan-devnet',
      validator_id: 'validator-0',
      validator_count: 1,
    },
    fee: { account_reserve: 1, minimum_fee: 1, burned_fee_total: 0 },
    validators: {
      chain_id: 'postfiat-wan-devnet',
      validator_count: 1,
      validators: ['validator-0'],
    },
    blocks: [{ transactions: [{ from: fundedAddress, to: fundedAddress }] }],
    account_tx: { rows: [] },
  };
  if (request.method === 'account') {
    if (request.params?.address === fundedAddress) {
      return { ok: true, result: { address: fundedAddress, balance: 5000, sequence: 1 } };
    }
    return {
      ok: false,
      error: { code: 'account_not_found', message: 'account not found in deterministic fixture' },
    };
  }
  if (Object.hasOwn(results, request.method)) {
    return { ok: true, result: results[request.method] };
  }
  return {
    ok: false,
    error: { code: 'fixture_method_unknown', message: `unsupported fixture method ${request.method}` },
  };
}

async function startProxyFixture(options = {}) {
  const rpcHandler = typeof options.rpcHandler === 'function'
    ? options.rpcHandler : defaultRpcResult;
  const rpc = net.createServer(socket => {
    let buffer = '';
    socket.on('data', chunk => {
      buffer += chunk.toString('utf8');
      let newline;
      while ((newline = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, newline);
        buffer = buffer.slice(newline + 1);
        let request;
        try {
          request = JSON.parse(line);
        } catch (_) {
          socket.destroy();
          return;
        }
        const outcome = rpcHandler(request);
        socket.write(`${JSON.stringify({
          version: 'postfiat-local-rpc-v1',
          id: request.id,
          ok: outcome.ok === true,
          result: outcome.result || null,
          error: outcome.error || null,
          events: [],
        })}\n`);
      }
    });
  });
  const rpcPort = await listen(rpc);
  const proxyPort = await reservePort();
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-proxy-fixture-'));
  let stdout = '';
  let stderr = '';
  const proxy = spawn(process.execPath, [path.join(__dirname, 'server.js')], {
    cwd: path.resolve(__dirname, '..'),
    env: {
      ...process.env,
      ENABLE_UPSTREAM_KEEPALIVE: 'false',
      FASTPAY_CERTIFICATE_OUTBOX_PATH: path.join(tempDir, 'fastpay-outbox.json'),
      LISTEN_HOST: '127.0.0.1',
      LISTEN_PORT: String(proxyPort),
      RPC_FLEET: `validator-0=127.0.0.1:${rpcPort}`,
      RPC_HOST: '127.0.0.1',
      RPC_PORT: String(rpcPort),
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  proxy.stdout.on('data', chunk => { stdout = `${stdout}${chunk}`.slice(-8192); });
  proxy.stderr.on('data', chunk => { stderr = `${stderr}${chunk}`.slice(-8192); });

  const deadline = Date.now() + 5_000;
  while (Date.now() < deadline) {
    const connected = await new Promise(resolve => {
      const socket = net.connect(proxyPort, '127.0.0.1');
      socket.once('connect', () => { socket.destroy(); resolve(true); });
      socket.once('error', () => resolve(false));
    });
    if (connected) {
      let closed = false;
      return {
        port: proxyPort,
        url: `ws://127.0.0.1:${proxyPort}`,
        close: async () => {
          if (closed) return;
          closed = true;
          proxy.kill('SIGTERM');
          if (proxy.exitCode === null) await new Promise(resolve => proxy.once('exit', resolve));
          await new Promise(resolve => rpc.close(resolve));
          fs.rmSync(tempDir, { recursive: true, force: true });
        },
      };
    }
    if (proxy.exitCode !== null) break;
    await new Promise(resolve => setTimeout(resolve, 25));
  }
  proxy.kill('SIGTERM');
  await new Promise(resolve => rpc.close(resolve));
  fs.rmSync(tempDir, { recursive: true, force: true });
  throw new Error(`proxy fixture failed to start: ${stderr || stdout || `exit ${proxy.exitCode}`}`);
}

module.exports = { defaultRpcResult, startProxyFixture };
