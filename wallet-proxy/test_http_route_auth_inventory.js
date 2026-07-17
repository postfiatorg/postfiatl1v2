'use strict';

// Every HTTP POST route must be explicitly classified. Unknown/new POST paths
// fail closed behind proxy authentication.

const assert = require('assert');
const fs = require('fs');
const path = require('path');

process.env.WALLET_PROXY_API_TOKEN = 'test-only-wallet-proxy-token-32-bytes-minimum';
const { httpRequestRequiresAuth } = require('./server');

const publicReadOnlyPosts = new Set([
  '/api/shielded-nav-swap/quote',
  '/api/shielded-nav-swap/preflight',
  '/api/navswap/planner-inputs',
  '/api/navswap/quotes',
  '/api/navswap/readiness',
  '/api/navswap/actions/prepare',
  '/api/navswap/actions/prepare-batch',
]);

const source = fs.readFileSync(
  path.join(__dirname, 'navswap-persistence-http.js'),
  'utf8',
);
const postRoutes = [...source.matchAll(/req\.method === 'POST' && url\.pathname === '([^']+)'/g)]
  .map((match) => match[1]);

assert(postRoutes.length > 0, 'HTTP POST route inventory unexpectedly empty');
assert.strictEqual(new Set(postRoutes).size, postRoutes.length, 'duplicate HTTP POST handler');
for (const route of postRoutes) {
  assert.strictEqual(
    httpRequestRequiresAuth('POST', route),
    !publicReadOnlyPosts.has(route),
    `incorrect HTTP POST authorization classification for ${route}`,
  );
}
for (const route of publicReadOnlyPosts) {
  assert(postRoutes.includes(route), `stale public read-only POST exception ${route}`);
}
assert.strictEqual(httpRequestRequiresAuth('POST', '/api/future-unclassified-write'), true);
assert.strictEqual(httpRequestRequiresAuth('GET', '/api/future-unclassified-write'), false);

console.log(`HTTP POST authorization inventory passed (${postRoutes.length} routes)`);
