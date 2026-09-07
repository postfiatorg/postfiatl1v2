import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { mkdtemp, readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import net from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { createRequire } from 'node:module';

import { chromium } from 'playwright';

// Reuse the ws build already installed for wallet-proxy (no new dependency).
const requireFromHere = createRequire(import.meta.url);
const WebSocket = requireFromHere('../../../wallet-proxy/node_modules/ws');

const WALLET_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const REPOSITORY_ROOT = resolve(WALLET_ROOT, '..');
const PROXY_ROOT = join(REPOSITORY_ROOT, 'wallet-proxy');
const DIST_ROOT = join(WALLET_ROOT, 'dist');

// The production build pins CHAIN_ID in src/lib/utils.js (chainEnv default
// 'postfiat-wan-devnet-2'); the real browser signer refuses any quote whose
// chain_id differs from the wallet backup, so the mock must speak that chain.
const CHAIN_ID = 'postfiat-wan-devnet-2';
const GENESIS_HASH = 'f6'.repeat(48);
const PFUSDC_ASSET_ID = '02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b';
const A666_ASSET_ID = '521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c';
const PFUSDC_ISSUER = `pf${'22'.repeat(20)}`;
const A666_ISSUER = `pf${'33'.repeat(20)}`;
const TRUST_TX_ID = 'tx-trust-set-finalized-1';

async function listen(server) {
  await new Promise((resolveListen, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolveListen);
  });
  const address = server.address();
  if (!address || typeof address === 'string') throw new Error('local server did not receive a TCP port');
  return address.port;
}

async function closeServer(server) {
  server.closeAllConnections?.();
  await new Promise((resolveClose, rejectClose) => {
    server.close(error => (error ? rejectClose(error) : resolveClose()));
  });
}

async function reservePort() {
  const server = createServer();
  const port = await listen(server);
  await closeServer(server);
  return port;
}

async function waitForHttp(url, child, output) {
  const deadline = Date.now() + 20_000;
  let lastError = null;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(`wallet proxy exited early (${child.exitCode}): ${output.join('')}`);
    }
    try {
      const response = await fetch(url, { cache: 'no-store' });
      if (response.ok) return;
      lastError = new Error(`received HTTP ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise(resolveWait => setTimeout(resolveWait, 50));
  }
  throw new Error(`wallet proxy did not become ready: ${lastError?.message || 'no response'}`);
}

async function terminate(child) {
  if (child.exitCode === null) {
    child.kill('SIGTERM');
    await Promise.race([
      new Promise(resolveExit => child.once('exit', resolveExit)),
      new Promise(resolveWait => setTimeout(resolveWait, 3_000)),
    ]);
    if (child.exitCode === null) child.kill('SIGKILL');
  }
}

function assetInfoResult(assetId, issuer, code) {
  return {
    schema: 'postfiat-asset-info-v1',
    chain_id: CHAIN_ID,
    genesis_hash: GENESIS_HASH,
    protocol_version: 1,
    asset_id: assetId,
    found: true,
    asset: {
      asset_id: assetId,
      issuer,
      code,
      version: 1,
      precision: 6,
      display_name: `Local ${code}`,
      max_supply: null,
      requires_authorization: false,
      freeze_enabled: false,
      clawback_enabled: false,
      outstanding_supply: 0,
      trustline_count: 0,
      holder_count: 0,
    },
  };
}

function rpcResult(method, params, captures) {
  captures.methods.push(method);
  if (method === 'status') {
    return {
      chain_id: CHAIN_ID,
      genesis_hash: GENESIS_HASH,
      protocol_version: 1,
      block_height: 901,
      mempool_pending: 0,
      validator_count: 1,
      last_run_unix: Math.floor(Date.now() / 1000),
    };
  }
  if (method === 'server_info') return { rpc: { read_only: false, owned_lane_enabled: false } };
  if (method === 'navcoin_bridge_routes') {
    return { schema: 'postfiat-pftl-uniswap-routes-status-v2', route_count: 0, routes: [] };
  }
  if (method === 'account') return { balance: 0, sequence: 0, public_key_hex: null };
  if (method === 'account_assets') return { assets: [] };
  if (method === 'owned_objects') return { objects: [] };
  if (method === 'account_tx') return [];
  if (method === 'asset_info') {
    if (params?.asset_id === PFUSDC_ASSET_ID) return assetInfoResult(PFUSDC_ASSET_ID, PFUSDC_ISSUER, 'pfUSDC');
    if (params?.asset_id === A666_ASSET_ID) return assetInfoResult(A666_ASSET_ID, A666_ISSUER, 'A666');
    return {
      schema: 'postfiat-asset-info-v1',
      chain_id: CHAIN_ID,
      genesis_hash: GENESIS_HASH,
      protocol_version: 1,
      asset_id: String(params?.asset_id || ''),
      found: false,
      asset: null,
    };
  }
  if (method === 'asset_fee_quote') {
    const operation = JSON.parse(String(params?.operation_json || '{}'));
    captures.quotes.push({ source: params?.source, operation });
    const reserveSufficient = operation.asset_id !== A666_ASSET_ID;
    return {
      chain_id: CHAIN_ID,
      genesis_hash: GENESIS_HASH,
      protocol_version: 1,
      source: params?.source,
      minimum_fee: 1,
      sequence: 1,
      operation,
      sender_meets_reserve_after_fee: reserveSufficient,
    };
  }
  if (method === 'mempool_submit_signed_asset_transaction_finality') {
    captures.submissions.push(String(params?.signed_asset_transaction_json || ''));
    return { tx_id: TRUST_TX_ID, finality: { confirmed: true, tx_id: TRUST_TX_ID } };
  }
  return {};
}

async function startRpcFixture(captures) {
  const server = net.createServer(socket => {
    let pending = '';
    socket.on('error', error => {
      if (error.code !== 'ECONNRESET') throw error;
    });
    socket.on('data', chunk => {
      pending += chunk.toString('utf8');
      let delimiter = pending.indexOf('\n');
      while (delimiter >= 0) {
        const line = pending.slice(0, delimiter);
        pending = pending.slice(delimiter + 1);
        const request = JSON.parse(line);
        socket.write(`${JSON.stringify({
          version: request.version,
          id: request.id,
          ok: true,
          result: rpcResult(request.method, request.params, captures),
          error: null,
          events: [],
        })}\n`);
        delimiter = pending.indexOf('\n');
      }
    });
  });
  const port = await listen(server);
  return { server, port };
}

function startProxy({ port, rpcPort, residentPort, token }) {
  const output = [];
  const child = spawn(process.execPath, ['server.js'], {
    cwd: PROXY_ROOT,
    env: {
      ...process.env,
      LISTEN_HOST: '127.0.0.1',
      LISTEN_PORT: String(port),
      ALLOWED_ORIGINS: `http://127.0.0.1:${port}`,
      WALLET_PROXY_API_TOKEN: token,
      WALLET_PROXY_LOCAL_SESSION_PRINCIPAL: 'default',
      WALLET_STATIC_DIR: DIST_ROOT,
      FASTPAY_ROUTE_WARMUP_ENABLED: 'false',
      ENABLE_UPSTREAM_KEEPALIVE: 'false',
      ENABLE_PROPOSER_ROUTING: 'false',
      RPC_HOST: '127.0.0.1',
      RPC_PORT: String(rpcPort),
      RPC_FLEET: `validator-0=127.0.0.1:${rpcPort}`,
      NAVSWAP_RUN_STORE_PATH: 'off',
      NAVSWAP_IDEMPOTENCY_STORE_PATH: 'off',
      PFTL_PRIVATE_SWAP_URL: `http://127.0.0.1:${residentPort}`,
      PFTL_PRIVATE_SWAP_TIMEOUT_MS: '5000',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.stdout.on('data', chunk => output.push(chunk.toString('utf8')));
  child.stderr.on('data', chunk => output.push(chunk.toString('utf8')));
  return { child, output };
}

async function walletAddressFromIndexedDb(page) {
  return page.evaluate(() => new Promise((resolveAddress, rejectAddress) => {
    const request = indexedDB.open('postfiat-wallet');
    request.onerror = () => rejectAddress(request.error || new Error('wallet IndexedDB open failed'));
    request.onsuccess = () => {
      const database = request.result;
      const transaction = database.transaction('vaults', 'readonly');
      const record = transaction.objectStore('vaults').get('default');
      record.onerror = () => {
        database.close();
        rejectAddress(record.error || new Error('wallet vault record read failed'));
      };
      record.onsuccess = () => {
        const address = record.result?.metadata?.address || '';
        database.close();
        resolveAddress(address);
      };
    };
  }));
}

async function createWallet(page, passphrase) {
  await page.getByRole('button', { name: 'Create a new wallet', exact: true }).click();
  await page.locator('.pf-seed-display').waitFor();
  const seed = (await page.locator('.pf-seed-display').textContent() || '').trim();
  assert.match(seed, /^[0-9a-f]{64}$/, 'production wallet generated a local seed');
  await page.locator('.pf-checkbox input').check();
  await page.locator('input[placeholder="Encryption passphrase (min 10 chars)"]').fill(passphrase);
  await page.locator('input[placeholder="Confirm passphrase"]').fill(passphrase);
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).last().click();
  await page.locator('.pf-shell').waitFor();
  const address = await walletAddressFromIndexedDb(page);
  assert.match(address, /^pf[0-9a-f]{40}$/, 'production wallet persisted its browser-controlled address');
  return { seed, address };
}

function assertNoSensitiveMaterial(label, content, values) {
  for (const value of values) {
    assert.equal(String(content).includes(value), false, `${label} contains generated custody material`);
  }
}

async function assertSequencedQuoteThroughProxy(port, operation) {
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  try {
    await new Promise((resolveOpen, rejectOpen) => {
      ws.once('open', resolveOpen);
      ws.once('error', rejectOpen);
    });
    const request = {
      version: 'postfiat-local-rpc-v1',
      id: 'fixture-self-check',
      method: 'asset_fee_quote',
      params: { source: operation.account, operation_json: JSON.stringify(operation) },
    };
    const response = await new Promise((resolveMsg, rejectMsg) => {
      const timer = setTimeout(() => rejectMsg(new Error('proxied asset_fee_quote self-check timed out')), 10_000);
      ws.once('message', data => {
        clearTimeout(timer);
        try {
          resolveMsg(JSON.parse(data.toString('utf8')));
        } catch (parseError) {
          rejectMsg(parseError);
        }
      });
      ws.once('error', rejectMsg);
      ws.send(JSON.stringify(request));
    });
    assert.equal(response.ok, true, 'proxied asset_fee_quote self-check must succeed');
    const result = response.result || {};
    for (const field of ['chain_id', 'genesis_hash', 'protocol_version', 'minimum_fee', 'sequence', 'source']) {
      assert.ok(result[field] !== null && result[field] !== undefined,
        `proxied asset_fee_quote must expose non-null ${field}`);
    }
    assert.deepEqual(result.operation, operation, 'proxied asset_fee_quote must echo the exact operation');
  } finally {
    ws.close();
  }
}

test('fresh self-custody wallet adds pfUSDC with a holder-signed trust_set', {
  timeout: 120_000,
}, async () => {
  await readFile(join(DIST_ROOT, 'index.html'), 'utf8');

  const tempRoot = await mkdtemp(join(tmpdir(), 'postfiat-public-route-smoke-'));
  const profileDir = join(tempRoot, 'chromium-profile');
  const token = randomBytes(32).toString('hex');
  const passphrase = randomBytes(24).toString('hex');
  const captures = { methods: [], quotes: [], submissions: [] };
  let rpcFixture;
  let proxy;
  let context;
  let seed = '';

  try {
    rpcFixture = await startRpcFixture(captures);
    const residentPort = await reservePort();
    const proxyPort = await reservePort();
    const origin = `http://127.0.0.1:${proxyPort}`;
    proxy = startProxy({ port: proxyPort, rpcPort: rpcFixture.port, residentPort, token });
    await waitForHttp(`${origin}/healthz`, proxy.child, proxy.output);

    // Fixture self-assertion through the proxy before any browser launch: the
    // sequenced quote contract must reach the wallet intact, and this fails
    // here instead of inside the browser flow if the proxy contract drifts.
    await assertSequencedQuoteThroughProxy(proxyPort, {
      operation: 'trust_set',
      account: `pf${'44'.repeat(20)}`,
      issuer: PFUSDC_ISSUER,
      asset_id: PFUSDC_ASSET_ID,
      limit: 1,
      authorized: false,
      frozen: false,
      reserve_paid: 10,
    });
    captures.methods.length = 0;
    captures.quotes.length = 0;
    captures.submissions.length = 0;

    context = await chromium.launchPersistentContext(profileDir, { headless: true });
    const page = await context.newPage();
    page.setDefaultTimeout(15_000);
    page.setDefaultNavigationTimeout(15_000);
    const rootResponse = await page.goto(`${origin}/`, { waitUntil: 'domcontentloaded' });
    assert.equal(rootResponse?.status(), 200, 'production wallet build is served by wallet-proxy');

    const created = await createWallet(page, passphrase);
    seed = created.seed;
    const address = created.address;

    // Prove the unlock path on the same fresh wallet before the send surface.
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.getByText('Unlock this wallet', { exact: true }).waitFor();
    await page.locator('input[placeholder="Passphrase"]').fill(passphrase);
    await page.getByRole('button', { name: 'Unlock', exact: true }).click();
    await page.locator('.pf-shell').waitFor();

    await page.locator('.pf-sidebar .pf-nav').filter({ hasText: 'Send' }).click();
    await page.getByText('Advanced: add an asset by ID', { exact: true }).click();
    const idInput = page.locator('[data-testid="add-asset-id"]');
    const limitInput = page.locator('[data-testid="add-asset-limit"]');
    const addButton = page.locator('[data-testid="add-asset"]');
    await idInput.waitFor();
    assert.equal(await addButton.count(), 1, 'exactly one add-asset control renders');
    assert.equal(await addButton.isDisabled(), true, 'add-asset starts disabled with empty inputs');

    await idInput.fill(PFUSDC_ASSET_ID);
    await limitInput.fill('1000000');
    assert.equal(await addButton.isDisabled(), false, 'valid id and limit enable the control');
    await addButton.click();
    try {
      await page.locator('.pf-success').filter({ hasText: 'Trustline accepted' }).waitFor();
    } catch (waitError) {
      const uiErrors = await page.locator('.pf-error').allTextContents();
      throw new Error(`trustline success not shown (ui errors: ${uiErrors.join(' | ') || 'none'}; quotes: ${captures.quotes.length}; submissions: ${captures.submissions.length})`);
    }

    assert.equal(captures.quotes.length, 1, 'exactly one asset fee quote reached the RPC boundary');
    assert.equal(captures.submissions.length, 1, 'exactly one signed submission reached the RPC boundary');
    const quoted = captures.quotes[0];
    assert.equal(quoted.source, address, 'quote source is the browser wallet address');
    assert.deepEqual(quoted.operation, {
      operation: 'trust_set',
      account: address,
      issuer: PFUSDC_ISSUER,
      asset_id: PFUSDC_ASSET_ID,
      limit: 1000000,
      authorized: false,
      frozen: false,
      reserve_paid: 10,
    }, 'holder-only trust_set operation at the RPC boundary');
    assert.deepEqual(Object.keys(quoted.operation).sort(), [
      'account', 'asset_id', 'authorized', 'frozen', 'issuer', 'limit', 'operation', 'reserve_paid',
    ], 'operation carries no extra or custody fields');
    const signedPayload = captures.submissions[0];
    assert.match(signedPayload, /"trust_set"/, 'signed payload carries the trust_set operation');
    assert.match(signedPayload, new RegExp(PFUSDC_ASSET_ID), 'signed payload carries the staged asset id');
    assert.equal(captures.methods.some(method => /authorize|freeze|clawback|issuer_sign/i.test(method)), false,
      'no issuer authorize/freeze/sign RPC method left the browser flow');
    assertNoSensitiveMaterial('captured RPC quote boundary', JSON.stringify(captures.quotes), [seed, passphrase]);
    assertNoSensitiveMaterial('captured RPC submission boundary', signedPayload, [seed, passphrase]);

    // Negative subcase: reserve-insufficient quote refuses visibly with no submission.
    await idInput.fill(A666_ASSET_ID);
    await limitInput.fill('5');
    assert.equal(await addButton.isDisabled(), false);
    await addButton.click();
    await page.locator('.pf-error').filter({ hasText: 'Insufficient balance' }).waitFor();
    assert.equal(await page.locator('.pf-success').count(), 0, 'no finalized success is shown for the refused operation');
    assert.equal(captures.quotes.length, 2, 'reserve-insufficient quote still reached the boundary');
    assert.equal(captures.submissions.length, 1, 'reserve-insufficient operation was never submitted');
  } finally {
    if (context) await context.close();
    if (proxy) await terminate(proxy.child);
    if (rpcFixture) await closeServer(rpcFixture.server);
  }
});
