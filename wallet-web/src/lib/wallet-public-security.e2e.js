import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const WALLET_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const REPOSITORY_ROOT = resolve(WALLET_ROOT, '..');
const PROXY_ROOT = join(REPOSITORY_ROOT, 'wallet-proxy');
const DIST_ROOT = join(WALLET_ROOT, 'dist');
const TEST_TOKEN = 'wallet-public-browser-test-token-00000001';

async function listen(server) {
  await new Promise((resolveListen, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolveListen);
  });
  return server.address().port;
}

async function stopServer(server) {
  await new Promise((resolveClose) => server.close(resolveClose));
}

async function waitForHttp(url, child, output) {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(`wallet proxy exited early (${child.exitCode}): ${output.join('')}`);
    }
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch (_) {
      // Startup is asynchronous; retry only inside the fixed deadline.
    }
    await new Promise((resolveWait) => setTimeout(resolveWait, 50));
  }
  throw new Error(`wallet proxy did not become ready: ${output.join('')}`);
}

async function terminate(child) {
  if (child.exitCode !== null) return;
  child.kill('SIGTERM');
  await Promise.race([
    new Promise((resolveExit) => child.once('exit', resolveExit)),
    new Promise((resolveWait) => setTimeout(resolveWait, 3_000)),
  ]);
  if (child.exitCode === null) child.kill('SIGKILL');
}

async function productionDeploymentAssertions() {
  const [dockerfile, localCompose, publicCompose, proxyPackage] = await Promise.all([
    readFile(join(PROXY_ROOT, 'Dockerfile'), 'utf8'),
    readFile(join(REPOSITORY_ROOT, 'docker-compose.wallet.yml'), 'utf8'),
    readFile(join(REPOSITORY_ROOT, 'docker-compose.wallet-public.yml'), 'utf8'),
    readFile(join(PROXY_ROOT, 'package.json'), 'utf8'),
  ]);
  assert.match(dockerfile, /CMD \["node", "server\.js"\]/);
  assert.doesNotMatch(dockerfile, /\bvite\b|npm run dev|npm run preview/);
  for (const compose of [localCompose, publicCompose]) {
    assert.match(compose, /WALLET_STATIC_DIR: "\/wallet\/dist"/);
    assert.match(compose, /\.\/wallet-web\/dist:\/wallet\/dist:ro/);
    assert.match(compose, /ENABLE_NATIVE_WALLET_SIGNER: "false"/);
    assert.doesNotMatch(compose, /\b5173\b|npm run dev|npm run preview|\bvite\b/);
  }
  const proxyDependencies = JSON.parse(proxyPackage).dependencies || {};
  assert.equal(proxyDependencies.vite, undefined);

  const distFiles = await readdir(DIST_ROOT, { recursive: true });
  assert.equal(distFiles.some((entry) => entry.endsWith('.map')), false);
  assert.equal(distFiles.some((entry) => entry.includes('@vite') || entry.startsWith('src/')), false);
}

test('production wallet service enforces CSP, origin, navigation, cache, and disclosure boundaries', {
  timeout: 90_000,
}, async () => {
  await productionDeploymentAssertions();

  const probe = createServer();
  const proxyPort = await listen(probe);
  await stopServer(probe);
  const walletOrigin = `http://127.0.0.1:${proxyPort}`;
  const output = [];
  const child = spawn(process.execPath, ['server.js'], {
    cwd: PROXY_ROOT,
    env: {
      ...process.env,
      LISTEN_HOST: '127.0.0.1',
      LISTEN_PORT: String(proxyPort),
      ALLOWED_ORIGINS: walletOrigin,
      WALLET_PROXY_API_TOKEN: TEST_TOKEN,
      WALLET_STATIC_DIR: DIST_ROOT,
      FASTPAY_ROUTE_WARMUP_ENABLED: 'false',
      NAVSWAP_RUN_STORE_PATH: '',
      NAVSWAP_IDEMPOTENCY_STORE_PATH: '',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.stdout.on('data', (chunk) => output.push(chunk.toString('utf8')));
  child.stderr.on('data', (chunk) => output.push(chunk.toString('utf8')));

  const attackerRequests = [];
  const attacker = createServer((request, response) => {
    attackerRequests.push({ method: request.method, url: request.url });
    if (request.url === '/frame') {
      response.writeHead(200, { 'Content-Type': 'text/html' });
      response.end(`<!doctype html><iframe id="target" src="${walletOrigin}/"></iframe>`);
      return;
    }
    if (request.url === '/') {
      response.writeHead(200, { 'Content-Type': 'text/html' });
      response.end('<!doctype html><title>foreign origin</title>');
      return;
    }
    response.writeHead(204, {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Headers': 'authorization,content-type',
      'Access-Control-Allow-Methods': 'GET,POST,OPTIONS',
    });
    response.end();
  });
  const attackerPort = await listen(attacker);
  const attackerOrigin = `http://127.0.0.1:${attackerPort}`;

  let browser;
  try {
    await waitForHttp(`${walletOrigin}/healthz`, child, output);
    browser = await chromium.launch({ headless: true });
    const context = await browser.newContext();
    const page = await context.newPage();
    const rootResponse = await page.goto(`${walletOrigin}/`, { waitUntil: 'domcontentloaded' });
    assert.equal(rootResponse.status(), 200);

    const rootHeaders = rootResponse.headers();
    const csp = rootHeaders['content-security-policy'] || '';
    assert.match(csp, /default-src 'self'/);
    assert.match(csp, /base-uri 'none'/);
    assert.match(csp, /frame-ancestors 'none'/);
    assert.match(csp, /form-action 'self'/);
    assert.match(csp, /object-src 'none'/);
    assert.equal(rootHeaders['x-frame-options'], 'DENY');
    assert.equal(rootHeaders['x-content-type-options'], 'nosniff');
    assert.equal(rootHeaders['referrer-policy'], 'no-referrer');
    assert.equal(rootHeaders['cache-control'], 'no-store');

    const html = await rootResponse.text();
    const assetMatches = [...html.matchAll(/(?:src|href)="(\/assets\/[^"]+)"/g)]
      .map((match) => match[1]);
    assert.ok(assetMatches.length >= 2, 'production build must reference content-hashed assets');
    for (const assetPath of assetMatches) {
      assert.match(assetPath, /^\/assets\/[A-Za-z0-9_.-]+-[A-Za-z0-9_-]{8,}\.[A-Za-z0-9]+$/);
      const response = await context.request.get(`${walletOrigin}${assetPath}`);
      assert.equal(response.status(), 200);
      assert.equal(response.headers()['cache-control'], 'public, max-age=31536000, immutable');
      if (assetPath.endsWith('.js')) {
        assert.doesNotMatch(await response.text(), /sourceMappingURL|\/@vite\/client|react-refresh/);
      }
    }

    for (const forbiddenPath of [
      '/@vite/client',
      '/@fs/etc/passwd',
      '/src/main.jsx',
      '/node_modules/vite/dist/client/client.mjs',
      '/.env',
      `${assetMatches.find((entry) => entry.endsWith('.js'))}.map`,
    ]) {
      const response = await context.request.get(`${walletOrigin}${forbiddenPath}`);
      assert.equal(response.status(), 404, `${forbiddenPath} must fail closed`);
      assert.equal(response.headers()['cache-control'], 'no-store');
    }

    const sameOriginMutationStatus = await page.evaluate(async () => {
      const response = await fetch('/api/navswap/fund-pfusdc', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ wallet_address: 'pf-browser-security', amount_atoms: '1' }),
      });
      return response.status;
    });
    assert.equal(sameOriginMutationStatus, 401);

    const cspBehavior = await page.evaluate(async (foreignOrigin) => {
      const inline = document.createElement('script');
      inline.textContent = 'window.__postfiat_inline_script_ran = true';
      document.head.append(inline);
      await new Promise((resolveWait) => setTimeout(resolveWait, 50));

      const base = document.createElement('base');
      base.href = `${foreignOrigin}/stolen-base/`;
      document.head.append(base);
      const relative = document.createElement('a');
      relative.href = 'relative-path';

      let foreignConnectBlocked = false;
      try {
        await fetch(`${foreignOrigin}/csp-connect`);
      } catch (_) {
        foreignConnectBlocked = true;
      }
      return {
        inlineBlocked: window.__postfiat_inline_script_ran !== true,
        baseBlocked: relative.href.startsWith(location.origin),
        foreignConnectBlocked,
      };
    }, attackerOrigin);
    assert.deepEqual(cspBehavior, {
      inlineBlocked: true,
      baseBlocked: true,
      foreignConnectBlocked: true,
    });
    assert.equal(attackerRequests.some((entry) => entry.url === '/csp-connect'), false);

    const framePage = await context.newPage();
    await framePage.goto(`${attackerOrigin}/frame`, { waitUntil: 'load' });
    await framePage.waitForTimeout(250);
    assert.equal(
      framePage.frames().some((frame) => frame !== framePage.mainFrame()
        && frame.url().startsWith(walletOrigin)),
      false,
      'wallet must not render inside a foreign-origin frame',
    );

    const crossOriginPage = await context.newPage();
    await crossOriginPage.goto(`${attackerOrigin}/`, { waitUntil: 'load' });
    const crossOriginBlocked = await crossOriginPage.evaluate(async ({ origin, token }) => {
      try {
        await fetch(`${origin}/api/navswap/fund-pfusdc`, {
          method: 'POST',
          headers: {
            Authorization: `Bearer ${token}`,
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({ wallet_address: 'pf-browser-security', amount_atoms: '1' }),
        });
        return false;
      } catch (_) {
        return true;
      }
    }, { origin: walletOrigin, token: TEST_TOKEN });
    assert.equal(crossOriginBlocked, true);

    const acceptance = {
      schema: 'postfiat-p0-wallet-public-browser-acceptance-v1',
      accepted: true,
      service: 'wallet-proxy production static server',
      vite_runtime_reachable: false,
      source_maps_reachable: false,
      content_hashed_asset_count: assetMatches.length,
      checks: {
        csp_runtime: cspBehavior,
        foreign_frame_blocked: true,
        unauthenticated_mutation_status: sameOriginMutationStatus,
        foreign_origin_mutation_blocked: crossOriginBlocked,
        html_cache_control: rootHeaders['cache-control'],
        hashed_asset_cache_control: 'public, max-age=31536000, immutable',
      },
    };
    const evidenceDir = process.env.P0_WALLET_BROWSER_EVIDENCE_DIR;
    if (evidenceDir) {
      await mkdir(evidenceDir, { recursive: true });
      await writeFile(join(evidenceDir, 'ACCEPTANCE.json'), `${JSON.stringify(acceptance, null, 2)}\n`);
    }
  } finally {
    if (browser) await browser.close();
    await stopServer(attacker);
    await terminate(child);
  }
});
