'use strict';

// Regression coverage for P0-PROXY-AUTH-01 and P0-CUSTODY-01.

const assert = require('assert');
const fs = require('fs');
const http = require('http');
const path = require('path');
const { spawnSync } = require('child_process');
const WebSocket = require('ws');

delete process.env.LISTEN_HOST;
delete process.env.ALLOWED_ORIGINS;
process.env.WALLET_PROXY_API_TOKEN = 'test-only-wallet-proxy-token-32-bytes-minimum';

const {
  DEFAULT_RPC_FLEET,
  LISTEN_HOST,
  RPC_FLEET,
  RPC_HOST,
  rpcRequestRequiresAuth,
  server,
} = require('./server');

function postJson(port, pathname, body, token = '', origin = '') {
  return new Promise((resolve, reject) => {
    const payload = JSON.stringify(body);
    const req = http.request({
      host: '127.0.0.1',
      port,
      path: pathname,
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        'content-length': Buffer.byteLength(payload),
        ...(token ? { authorization: `Bearer ${token}` } : {}),
        ...(origin ? { origin } : {}),
      },
    }, (res) => {
      let raw = '';
      res.setEncoding('utf8');
      res.on('data', (chunk) => { raw += chunk; });
      res.on('end', () => resolve({ statusCode: res.statusCode, body: JSON.parse(raw) }));
    });
    req.on('error', reject);
    req.end(payload);
  });
}

function callRemovedWalletSigner(port) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}`);
    const timer = setTimeout(() => {
      ws.terminate();
      reject(new Error('wallet signer response timed out'));
    }, 5_000);
    ws.on('open', () => {
      ws.send(JSON.stringify({
        version: 'postfiat-local-rpc-v1',
        id: 'unauthenticated-signer-reproduction',
        method: 'wallet_sign_owned_transfer',
        params: {},
      }));
    });
    ws.on('message', (message) => {
      clearTimeout(timer);
      const response = JSON.parse(message.toString('utf8'));
      ws.close();
      resolve(response);
    });
    ws.on('error', reject);
  });
}

function callAuthenticatedRpc(port, method) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}`, {
      headers: {
        authorization: `Bearer ${process.env.WALLET_PROXY_API_TOKEN}`,
        origin: 'http://localhost:5173',
      },
    });
    const timer = setTimeout(() => {
      ws.terminate();
      reject(new Error(`${method} response timed out`));
    }, 5_000);
    ws.on('open', () => {
      ws.send(JSON.stringify({
        version: 'postfiat-local-rpc-v1',
        id: `authenticated-${method}`,
        method,
        params: {},
        proxy_auth_token: process.env.WALLET_PROXY_API_TOKEN,
      }));
    });
    ws.on('message', (message) => {
      clearTimeout(timer);
      const response = JSON.parse(message.toString('utf8'));
      ws.close();
      resolve(response);
    });
    ws.on('error', reject);
  });
}

async function main() {
  assert.strictEqual(LISTEN_HOST, '127.0.0.1');
  assert.strictEqual(RPC_HOST, '127.0.0.1');
  assert.strictEqual(RPC_FLEET.length, 6);
  assert(RPC_FLEET.every((endpoint) => endpoint.host === '127.0.0.1'));
  assert(!/64\.176\.220\.75|95\.179\.184\.122|66\.42\.48\.39|149\.28\.63\.106|95\.179\.179\.206|45\.32\.110\.170/.test(DEFAULT_RPC_FLEET));
  for (const method of ['owned_recovery_capabilities', 'owned_certificate', 'owned_recovery_status']) {
    assert.strictEqual(rpcRequestRequiresAuth(method), false, `${method} is a public read`);
  }
  for (const method of ['owned_sign_v3', 'owned_apply_v3', 'owned_unwrap_sign_v3', 'owned_unwrap_apply_v3']) {
    assert.strictEqual(rpcRequestRequiresAuth(method), true, `${method} is an authenticated mutation`);
  }
  assert.strictEqual(rpcRequestRequiresAuth('consensus_v2_timeout_vote'), true);

  const publicCompose = fs.readFileSync(
    path.resolve(__dirname, '..', 'docker-compose.wallet.yml'),
    'utf8',
  );
  assert(!/\b(?:207\.148\.29\.78|95\.179\.184\.122|66\.42\.48\.39|149\.28\.63\.106|95\.179\.179\.206|45\.32\.110\.170)\b/.test(publicCompose));
  assert(!/issuer\.key\.json|holder\.key\.json|ENABLE_NATIVE_WALLET_SIGNER:\s*["']?true/.test(publicCompose));
  assert.match(publicCompose, /RPC_HOST:.*:\?set an explicit validator/);
  assert.match(publicCompose, /WALLET_PROXY_API_TOKEN:.*:\?set a random token/);

  const unsafeStartup = spawnSync(process.execPath, ['-e', "require('./wallet-proxy/server')"], {
    cwd: require('path').resolve(__dirname, '..'),
    encoding: 'utf8',
    env: {
      ...process.env,
      LISTEN_HOST: '0.0.0.0',
      ALLOWED_ORIGINS: '',
      WALLET_PROXY_API_TOKEN: '',
    },
  });
  assert.notStrictEqual(unsafeStartup.status, 0);
  assert.match(unsafeStartup.stderr, /non-loopback LISTEN_HOST requires/);

  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  try {
    const missingOrigin = await postJson(
      port,
      '/api/bridge/relay',
      {},
      process.env.WALLET_PROXY_API_TOKEN,
    );
    assert.strictEqual(missingOrigin.statusCode, 403);

    const foreignOrigin = await postJson(
      port,
      '/api/bridge/relay',
      {},
      process.env.WALLET_PROXY_API_TOKEN,
      'https://attacker.example',
    );
    assert.strictEqual(foreignOrigin.statusCode, 403);

    const mutation = await postJson(
      port,
      '/api/bridge/relay',
      {},
      '',
      'http://localhost:5173',
    );
    assert.strictEqual(mutation.statusCode, 401);
    assert.strictEqual(mutation.body.code, 'proxy_auth_required');

    const authorized = await postJson(
      port,
      '/api/bridge/relay',
      {},
      process.env.WALLET_PROXY_API_TOKEN,
      'http://localhost:5173',
    );
    assert.notStrictEqual(authorized.statusCode, 401);

    // The public proxy no longer exposes any seed-bearing signing method,
    // regardless of whether the caller presents the proxy mutation token.
    const signer = await callRemovedWalletSigner(port);
    assert.strictEqual(signer.error.code, 'proxy_method_removed');
    assert.match(signer.error.message, /sign locally/);

    // Timeout votes mutate durable consensus safety state and are available
    // only to the proxy's bounded view-recovery orchestrator, never browsers.
    const timeoutVote = await callAuthenticatedRpc(port, 'consensus_v2_timeout_vote');
    assert.strictEqual(timeoutVote.error.code, 'proxy_internal_method');
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }

  console.log('P0-PROXY-AUTH-01/P0-CUSTODY-01 regression passed');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
