'use strict';

// Real-boundary closure regression for P0-PROXY-AUTH-01. The supported edge
// must authenticate distinct principals, scope durable idempotency by that
// principal, bound mutation pressure, reject oversized bodies, and refuse a
// hostile WebSocket upgrade before opening a connection.

const assert = require('assert');
const fs = require('fs');
const http = require('http');
const path = require('path');
const WebSocket = require('ws');

const ALPHA_TOKEN = 'alpha-test-token-32-bytes-minimum-0001';
const BETA_TOKEN = 'beta-test-token-32-bytes-minimum-00002';
const WALLET_ORIGIN = 'https://wallet.example.test';

delete process.env.WALLET_PROXY_API_TOKEN;
process.env.WALLET_PROXY_API_TOKENS_JSON = JSON.stringify({
  alpha: ALPHA_TOKEN,
  beta: BETA_TOKEN,
});
process.env.ALLOWED_ORIGINS = WALLET_ORIGIN;
process.env.WALLET_PROXY_MUTATION_RATE_LIMIT = '3';
process.env.WALLET_PROXY_MUTATION_RATE_WINDOW_MS = '60000';
process.env.WALLET_PROXY_MUTATION_CONCURRENCY = '1';
process.env.WALLET_PROXY_MAX_HTTP_BODY_BYTES = '256';
process.env.NAVSWAP_IDEMPOTENCY_STORE_PATH = 'off';
process.env.NAVSWAP_RUN_STORE_PATH = 'off';

const {
  acquireMutationAdmission,
  clearMutationAdmissionForTest,
  server,
} = require('./server');

function postRaw(port, pathname, rawBody, token, origin = WALLET_ORIGIN) {
  return new Promise((resolve, reject) => {
    const req = http.request({
      host: '127.0.0.1',
      port,
      path: pathname,
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        'content-length': Buffer.byteLength(rawBody),
        authorization: `Bearer ${token}`,
        origin,
      },
    }, (res) => {
      let raw = '';
      res.setEncoding('utf8');
      res.on('data', (chunk) => { raw += chunk; });
      res.on('end', () => {
        let body = null;
        try { body = raw ? JSON.parse(raw) : null; } catch (_) { /* asserted below */ }
        resolve({ statusCode: res.statusCode, headers: res.headers, body, raw });
      });
    });
    req.on('error', reject);
    req.end(rawBody);
  });
}

function postJson(port, pathname, body, token, origin) {
  return postRaw(port, pathname, JSON.stringify(body), token, origin);
}

function hostileWebSocketUpgrade(port) {
  return new Promise((resolve, reject) => {
    let opened = false;
    const ws = new WebSocket(`ws://127.0.0.1:${port}`, {
      origin: 'https://attacker.example',
    });
    const timer = setTimeout(() => {
      ws.terminate();
      reject(new Error('hostile WebSocket upgrade timed out'));
    }, 5_000);
    ws.on('open', () => { opened = true; });
    ws.on('unexpected-response', (_request, response) => {
      clearTimeout(timer);
      response.resume();
      resolve({ opened, statusCode: response.statusCode });
    });
    ws.on('close', (code) => {
      clearTimeout(timer);
      if (opened) resolve({ opened, closeCode: code, statusCode: null });
    });
    ws.on('error', (error) => {
      if (!opened && error.message.includes('Unexpected server response')) return;
      clearTimeout(timer);
      reject(error);
    });
  });
}

async function main() {
  if (typeof clearMutationAdmissionForTest === 'function') clearMutationAdmissionForTest();
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  try {
    const sharedBody = {
      idempotency_key: 'shared-principal-key-0001',
      recipient: 'pf-test',
      amount_atoms: '1',
    };
    const alpha = await postJson(
      port,
      '/api/navswap/devnet-fund-pfusdc',
      sharedBody,
      ALPHA_TOKEN,
    );
    const beta = await postJson(
      port,
      '/api/navswap/devnet-fund-pfusdc',
      sharedBody,
      BETA_TOKEN,
    );
    assert.notStrictEqual(alpha.statusCode, 401);
    assert.notStrictEqual(beta.statusCode, 401);
    assert.strictEqual(alpha.body.idempotency.replayed, false);
    assert.strictEqual(beta.body.idempotency.replayed, false);
    assert.strictEqual(alpha.body.idempotency.principal_id, 'alpha');
    assert.strictEqual(beta.body.idempotency.principal_id, 'beta');

    // Alpha has one admitted mutation above. Two more fit; the fourth is
    // rejected before route-specific work.
    for (const suffix of ['0002', '0003']) {
      const response = await postJson(port, '/api/navswap/devnet-fund-pfusdc', {
        ...sharedBody,
        idempotency_key: `alpha-rate-key-${suffix}`,
      }, ALPHA_TOKEN);
      assert.notStrictEqual(response.statusCode, 429);
    }
    const rateLimited = await postJson(port, '/api/navswap/devnet-fund-pfusdc', {
      ...sharedBody,
      idempotency_key: 'alpha-rate-key-0004',
    }, ALPHA_TOKEN);
    assert.strictEqual(rateLimited.statusCode, 429);
    assert.strictEqual(rateLimited.body.code, 'proxy_mutation_rate_limited');

    const oversized = await postRaw(
      port,
      '/api/navswap/devnet-fund-pfusdc',
      JSON.stringify({ padding: 'x'.repeat(512) }),
      BETA_TOKEN,
    );
    assert.strictEqual(oversized.statusCode, 413);
    assert.strictEqual(oversized.body.code, 'request_body_too_large');

    assert.strictEqual(typeof clearMutationAdmissionForTest, 'function');
    assert.strictEqual(typeof acquireMutationAdmission, 'function');
    clearMutationAdmissionForTest();
    const held = acquireMutationAdmission('alpha', 1000);
    assert.strictEqual(held.ok, true);
    const blocked = acquireMutationAdmission('beta', 1000);
    assert.strictEqual(blocked.ok, false);
    assert.strictEqual(blocked.code, 'proxy_mutation_concurrency_limited');
    held.release();
    const afterRelease = acquireMutationAdmission('beta', 1000);
    assert.strictEqual(afterRelease.ok, true);
    afterRelease.release();

    const hostileUpgrade = await hostileWebSocketUpgrade(port);
    assert.strictEqual(hostileUpgrade.opened, false);
    assert.strictEqual(hostileUpgrade.statusCode, 403);

    const compose = fs.readFileSync(
      path.resolve(__dirname, '..', 'docker-compose.wallet-public.yml'),
      'utf8',
    );
    const caddy = fs.readFileSync(
      path.resolve(__dirname, '..', 'wallet-caddy', 'Caddyfile.production'),
      'utf8',
    );
    const dockerfile = fs.readFileSync(
      path.resolve(__dirname, 'Dockerfile'),
      'utf8',
    );
    assert.match(compose, /127\.0\.0\.1:8080/);
    assert.doesNotMatch(compose, /(?:^|\s)-\s*["']?8080:8080/);
    assert.match(compose, /WALLET_PROXY_API_TOKENS_FILE/);
    assert.match(compose, /wallet_proxy_tokens/);
    assert.match(compose, /WALLET_PUBLIC_ORIGIN/);
    assert.match(compose, /WALLET_EDGE_UID:\?set to the TLS key owner uid/);
    assert.match(compose, /WALLET_EDGE_GID:\?set to the TLS key owner gid/);
    assert.match(compose, /XDG_DATA_HOME: "\/tmp\/caddy\/data"/);
    assert.doesNotMatch(compose, /wallet-caddy-(?:data|config)/);
    assert.match(caddy, /tls \{\$WALLET_TLS_CERT\} \{\$WALLET_TLS_KEY\}/);
    assert.match(caddy, /request_body[\s\S]*max_size 16MB/);
    assert.match(caddy, /reverse_proxy wallet-proxy:8080/);
    assert.match(dockerfile, /^FROM node:20-trixie-slim@sha256:[0-9a-f]{64}$/m);
    assert.match(dockerfile, /^USER node$/m);

    console.log('authenticated TLS wallet edge regression passed');
  } finally {
    if (typeof clearMutationAdmissionForTest === 'function') clearMutationAdmissionForTest();
    await new Promise((resolve) => server.close(resolve));
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
