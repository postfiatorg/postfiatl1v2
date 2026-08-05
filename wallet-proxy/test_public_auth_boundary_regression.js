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
process.env.WALLET_PROXY_LOCAL_SESSION_PRINCIPAL = 'default';

const {
  DEFAULT_RPC_FLEET,
  LISTEN_HOST,
  RPC_FLEET,
  RPC_HOST,
  rpcRequestRequiresAuth,
  server,
  websocketOriginAllowed,
} = require('./server');

function postJson(port, pathname, body, token = '', origin = '', extraHeaders = {}) {
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
        ...extraHeaders,
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

function getLocalSession(port, headers = {}) {
  return new Promise((resolve, reject) => {
    const req = http.request({
      host: '127.0.0.1',
      port,
      path: '/api/bridge/local-session',
      method: 'GET',
      headers,
    }, (res) => {
      let raw = '';
      res.setEncoding('utf8');
      res.on('data', (chunk) => { raw += chunk; });
      res.on('end', () => resolve({
        statusCode: res.statusCode,
        headers: res.headers,
        body: JSON.parse(raw),
      }));
    });
    req.on('error', reject);
    req.end();
  });
}

function getJson(port, pathname, token = '', headers = {}) {
  return new Promise((resolve, reject) => {
    const req = http.request({
      host: '127.0.0.1',
      port,
      path: pathname,
      method: 'GET',
      headers: {
        ...(token ? { authorization: `Bearer ${token}` } : {}),
        ...headers,
      },
    }, (res) => {
      let raw = '';
      res.setEncoding('utf8');
      res.on('data', (chunk) => { raw += chunk; });
      res.on('end', () => resolve({ statusCode: res.statusCode, body: JSON.parse(raw) }));
    });
    req.on('error', reject);
    req.end();
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

function callBrowserUnauthenticatedRpc(port, method, params, origin, host) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}`, {
      headers: { origin, ...(host ? { host } : {}) },
    });
    const timer = setTimeout(() => {
      ws.terminate();
      reject(new Error(`${method} response timed out`));
    }, 5_000);
    ws.on('open', () => {
      ws.send(JSON.stringify({
        version: 'postfiat-local-rpc-v1',
        id: `browser-unauthenticated-${method}`,
        method,
        params,
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
  assert.strictEqual(websocketOriginAllowed({
    headers: {
      origin: 'https://wallet-tunnel.example:5173',
      host: 'wallet-tunnel.example:5173',
    },
  }), true, 'exact browser-facing origin should be accepted');
  assert.strictEqual(websocketOriginAllowed({
    headers: {
      origin: 'https://attacker.example',
      host: 'wallet-tunnel.example:5173',
    },
  }), false, 'cross-origin websocket should remain forbidden');

  assert.strictEqual(LISTEN_HOST, '127.0.0.1');
  assert.strictEqual(RPC_HOST, '127.0.0.1');
  assert.strictEqual(RPC_FLEET.length, 6);
  assert(RPC_FLEET.every((endpoint) => endpoint.host === '127.0.0.1'));
  assert(!/64\.176\.220\.75|95\.179\.184\.122|66\.42\.48\.39|149\.28\.63\.106|95\.179\.179\.206|45\.32\.110\.170/.test(DEFAULT_RPC_FLEET));
  for (const method of [
    'owned_recovery_capabilities',
    'owned_certificate',
    'owned_recovery_status',
    'navcoin_bridge_supply_status',
    'nav_reserve_proof_status',
    'vault_bridge_status',
    'fx_fix_list',
    'fx_fix_info',
    'fx_fix_reservation_info',
    'asset_orchard_action_status',
    'fx_fix_quote',
  ]) {
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

  const a666Startup = fs.readFileSync(
    path.resolve(__dirname, '..', 'scripts', 'start-a666-wallet-local.sh'),
    'utf8',
  );
  assert.match(a666Startup, /RPC_PORT=39650/);
  for (let index = 0; index < 6; index += 1) {
    assert.match(
      a666Startup,
      new RegExp(`validator-${index}=127\\.0\\.0\\.1:3965${index}`),
    );
  }
  assert(!/127\.0\.0\.1:3865[0-5]/.test(a666Startup));

  const unsafeStartup = spawnSync(process.execPath, ['-e', "require('./wallet-proxy/server')"], {
    cwd: require('path').resolve(__dirname, '..'),
    encoding: 'utf8',
    env: {
      ...process.env,
      LISTEN_HOST: '0.0.0.0',
      ALLOWED_ORIGINS: '',
      WALLET_PROXY_API_TOKEN: '',
      WALLET_PROXY_LOCAL_SESSION_PRINCIPAL: '',
    },
  });
  assert.notStrictEqual(unsafeStartup.status, 0);
  assert.match(unsafeStartup.stderr, /non-loopback LISTEN_HOST requires/);

  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  try {
    const nonBrowserLocalSession = await getLocalSession(port);
    assert.strictEqual(nonBrowserLocalSession.statusCode, 403);

    const localSession = await getLocalSession(port, {
      'sec-fetch-site': 'same-origin',
    });
    assert.strictEqual(localSession.statusCode, 200);
    assert.strictEqual(localSession.headers['cache-control'], 'no-store');
    assert.strictEqual(localSession.body.schema, 'postfiat-local-wallet-session-v1');
    assert.strictEqual(localSession.body.principal, 'default');
    assert.strictEqual(localSession.body.token, process.env.WALLET_PROXY_API_TOKEN);

    const sameOriginJobDiscovery = await getJson(
      port,
      `/api/bridge/jobs?recipient=pf${'ab'.repeat(20)}&limit=20`,
      process.env.WALLET_PROXY_API_TOKEN,
      { 'sec-fetch-site': 'same-origin' },
    );
    assert.strictEqual(sameOriginJobDiscovery.statusCode, 200);
    assert.strictEqual(sameOriginJobDiscovery.body.ok, true);

    const forgedCrossSiteJobDiscovery = await getJson(
      port,
      `/api/bridge/jobs?recipient=pf${'ab'.repeat(20)}&limit=20`,
      process.env.WALLET_PROXY_API_TOKEN,
      { 'sec-fetch-site': 'cross-site' },
    );
    assert.strictEqual(forgedCrossSiteJobDiscovery.statusCode, 403);

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

    const tunneledSameOrigin = await postJson(
      port,
      '/api/bridge/relay',
      {},
      process.env.WALLET_PROXY_API_TOKEN,
      'https://wallet-tunnel.example:5173',
      { host: 'wallet-tunnel.example:5173' },
    );
    assert.notStrictEqual(tunneledSameOrigin.statusCode, 403);

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

    // A666 R4 step-3 vector (proxy allowlist fix, read-only diagnosis B):
    // a browser-origin unauthenticated nav_reserve_proof_status read is a
    // public status read like its navcoin_bridge_* siblings and must pass
    // the auth gate to backend forwarding. Live diagnosis: all six
    // validators serve the method (handler ledger-backed, provider-neutral;
    // crates/node/src/lifecycle_queries.rs:1886). In this harness the test
    // fleet is unreachable, so a transport/forwarding failure is expected;
    // an auth or origin refusal is the regression.
    const a666R4NavAssetId = '521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c';
    const step3Vector = await callBrowserUnauthenticatedRpc(
      port,
      'nav_reserve_proof_status',
      { asset_id: a666R4NavAssetId },
      'http://127.0.0.1:31021',
      '127.0.0.1:31021',
    );
    assert.strictEqual(rpcRequestRequiresAuth('nav_reserve_proof_status'), false,
      'nav_reserve_proof_status is a public read');
    assert.notStrictEqual(step3Vector.error?.code, 'proxy_auth_required',
      'public status read must not be auth-refused');
    assert.notStrictEqual(step3Vector.error?.code, 'proxy_origin_required',
      'public status read must not be origin-refused');
    assert.notStrictEqual(step3Vector.error?.code, 'proxy_internal_method',
      'public status read must not be internal-classified');
    // Anti-deletion guard: this vector must stay registered.
    const ownSource = fs.readFileSync(__filename, 'utf8');
    assert(ownSource.includes("'nav_reserve_proof_status'"),
      'step-3 public-read vector must stay registered');
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }

  console.log('P0-PROXY-AUTH-01/P0-CUSTODY-01 regression passed');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
