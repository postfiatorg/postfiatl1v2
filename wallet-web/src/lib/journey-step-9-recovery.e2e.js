import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { createServer } from 'node:http';
import { mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import net from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const WALLET_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const REPOSITORY_ROOT = resolve(WALLET_ROOT, '..');
const PROXY_ROOT = join(REPOSITORY_ROOT, 'wallet-proxy');
const DIST_ROOT = join(WALLET_ROOT, 'dist');

const ROUTE_ID = 'journey-step-9-private-primary';
const NAV_ASSET_ID = 'a1'.repeat(48);
const SETTLEMENT_ASSET_ID = 'b2'.repeat(48);
const ROUTE_DIGEST = 'c3'.repeat(48);
const RESERVE_PACKET_HASH = 'd4'.repeat(48);
const FINAL_BALANCE_TUPLE = Object.freeze([
  Object.freeze({ asset_id: SETTLEMENT_ASSET_ID, amount_atoms: '3988000' }),
  Object.freeze({ asset_id: NAV_ASSET_ID, amount_atoms: '1010000' }),
]);
const FINAL_RECEIPT_IDENTITY = 'journey-step-9-pending-final-receipt';

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function initialResidentStore() {
  return {
    schema: 'postfiat-journey-step-9-resident-job-store-v1',
    jobs: {},
  };
}

async function readStore(storePath) {
  return JSON.parse(await readFile(storePath, 'utf8'));
}

async function writeStore(storePath, value) {
  await writeFile(storePath, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}

async function readRequestBody(request) {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  return Buffer.concat(chunks).toString('utf8');
}

function sendJson(response, status, payload) {
  response.writeHead(status, {
    'content-type': 'application/json; charset=utf-8',
    'cache-control': 'no-store',
  });
  response.end(JSON.stringify(payload));
}

function quote() {
  return {
    schema: 'postfiat.pftl_swap.quote.v1',
    quote_id: 'e5'.repeat(48),
    chain_id: 'postfiat-wan-devnet-2',
    genesis_hash: 'f6'.repeat(48),
    protocol_version: 1,
    route_id: ROUTE_ID,
    direction: 'issue',
    output_mode: 'private',
    nav_amount_atoms: 1000000,
    input_asset_id: SETTLEMENT_ASSET_ID,
    input_amount_atoms: 1005000,
    output_asset_id: NAV_ASSET_ID,
    output_amount_atoms: 1000000,
    base_settlement_atoms: 1000000,
    spread_atoms: 5000,
    maximum_fee_atoms: 5000,
    route_epoch: 9,
    policy_epoch: 9,
    policy_hash: '12'.repeat(48),
    pricing_nav_epoch: 9,
    pricing_reserve_packet_hash: RESERVE_PACKET_HASH,
    quote_height: 900,
    quote_block_id: '34'.repeat(32),
    state_root: '56'.repeat(32),
    orchard_root: '78'.repeat(32),
    route_state_hash: '9a'.repeat(32),
    expiry_height: 1200,
    created_at_unix_ms: 1,
  };
}

function residentResult(job, replayed) {
  const committed = job.state === 'COMMITTED';
  return {
    ok: true,
    replayed,
    swap: {
      swap_id: `swap-${job.idempotency_key}`,
      idempotency_key: job.idempotency_key,
      quote_id: quote().quote_id,
      direction: 'issue',
      input_amount_atoms: 1005000,
      minimum_output_amount_atoms: 1000000,
      state: job.state,
      batch_hash: 'ab'.repeat(32),
      committed_height: committed ? 901 : null,
      certificate_ref: committed ? FINAL_RECEIPT_IDENTITY : null,
    },
    final_balance_tuple: committed ? clone(FINAL_BALANCE_TUPLE) : null,
    receipt: committed ? { receipt_identity: FINAL_RECEIPT_IDENTITY, finalized: true } : null,
    output_note_refs: committed ? ['cd'.repeat(32)] : [],
  };
}

async function startResident(storePath, identity, ingress) {
  const server = createServer(async (request, response) => {
    const url = new URL(request.url || '/', 'http://127.0.0.1');

    if (request.method === 'GET' && url.pathname === '/v1/ready') {
      sendJson(response, 200, {
        ok: true,
        schema: 'postfiat.pftl_swap.readiness.v1',
        ready: true,
        local_only: true,
        controlled_wallet_id: identity.walletAddress,
        route_id: ROUTE_ID,
        checks: { admission: { max_nav_amount_atoms: 1000000 } },
      });
      return;
    }

    if (request.method === 'POST' && url.pathname === '/v1/quote') {
      const bodyText = await readRequestBody(request);
      ingress.push({ method: request.method, path: url.pathname, body: bodyText });
      const body = JSON.parse(bodyText);
      if (body?.direction !== 'issue' || body?.output_mode !== 'private'
        || body?.nav_amount_atoms !== 1000000) {
        sendJson(response, 400, { ok: false, message: 'local rehearsal quote binding failed' });
        return;
      }
      sendJson(response, 200, { ok: true, quote: quote() });
      return;
    }

    if (request.method === 'POST' && url.pathname === '/v1/swap') {
      const bodyText = await readRequestBody(request);
      ingress.push({ method: request.method, path: url.pathname, body: bodyText });
      const body = JSON.parse(bodyText);
      const signed = body?.signed_intent;
      const idempotencyKey = signed?.intent?.idempotency_key;
      if (!idempotencyKey || signed?.intent?.principal !== identity.walletAddress
        || signed?.intent?.controlled_wallet_id !== identity.walletAddress
        || signed?.intent?.route_id !== ROUTE_ID) {
        sendJson(response, 400, { ok: false, message: 'local rehearsal signed intent binding failed' });
        return;
      }

      const store = await readStore(storePath);
      let job = store.jobs[idempotencyKey];
      if (!job) {
        job = {
          idempotency_key: idempotencyKey,
          state: 'PUBLISHED',
          submit_count: 1,
          commit_count: 0,
          final_balance_tuple: clone(FINAL_BALANCE_TUPLE),
          receipt_identity: FINAL_RECEIPT_IDENTITY,
        };
        store.jobs[idempotencyKey] = job;
        await writeStore(storePath, store);
        sendJson(response, 202, residentResult(job, false));
        return;
      }

      if (job.state === 'PUBLISHED') {
        job.state = 'COMMITTED';
        job.submit_count += 1;
        job.commit_count += 1;
        await writeStore(storePath, store);
        sendJson(response, 200, residentResult(job, false));
        return;
      }

      job.submit_count += 1;
      await writeStore(storePath, store);
      sendJson(response, 200, residentResult(job, true));
      return;
    }

    if (request.method === 'GET' && url.pathname === '/v1/status') {
      const idempotencyKey = url.searchParams.get('id') || '';
      ingress.push({ method: request.method, path: url.pathname, body: idempotencyKey });
      const job = (await readStore(storePath)).jobs[idempotencyKey];
      if (!job) {
        sendJson(response, 404, { ok: false, message: 'local rehearsal job was not found' });
        return;
      }
      sendJson(response, 200, residentResult(job, true));
      return;
    }

    sendJson(response, 404, { ok: false, message: 'local rehearsal resident path was not found' });
  });
  await listen(server);
  return server;
}

function rpcResult(method) {
  if (method === 'status') {
    return {
      chain_id: 'postfiat-local-rehearsal',
      genesis_hash: 'f6'.repeat(48),
      protocol_version: 1,
      block_height: 901,
      mempool_pending: 0,
      validator_count: 1,
      last_run_unix: Math.floor(Date.now() / 1000),
    };
  }
  if (method === 'server_info') return { rpc: { read_only: false, owned_lane_enabled: false } };
  if (method === 'navcoin_bridge_routes') {
    return {
      schema: 'postfiat-pftl-uniswap-routes-status-v2',
      route_count: 1,
      routes: [{
        route_family: 'primary_pftl_mint',
        route_id: ROUTE_ID,
        route_config_digest: ROUTE_DIGEST,
        native_nav_asset_id: NAV_ASSET_ID,
        settlement_asset_id: SETTLEMENT_ASSET_ID,
        wrapped_navcoin_token: `0x${'11'.repeat(20)}`,
        handoff_controller: `0x${'22'.repeat(20)}`,
        native_nav_asset_code: 'NAV',
        native_nav_asset_display_name: 'Local NAV',
        settlement_asset_code: 'pfUSDC',
        settlement_asset_display_name: 'Local pfUSDC',
        native_nav_asset_precision: 6,
        settlement_asset_precision: 6,
        ethereum_chain_id: 1,
        route_trust_class: 'CONTROLLED',
        live_value_enabled: true,
        route_live: true,
        paused: false,
      }],
    };
  }
  if (method === 'navcoin_bridge_supply_status') {
    return {
      route_id: ROUTE_ID,
      native_nav_asset_id: NAV_ASSET_ID,
      settlement_asset_id: SETTLEMENT_ASSET_ID,
      live_value_enabled: true,
      paused: false,
      invariant_holds: true,
      pricing_nav_epoch: 9,
      pricing_reserve_packet_hash: RESERVE_PACKET_HASH,
      settlement_reserve_atoms: '3988000',
      available_issue_atoms: '1010000',
      available_redeem_atoms: '1010000',
    };
  }
  if (method === 'vault_bridge_status') {
    return {
      asset_id: NAV_ASSET_ID,
      finalized_epoch: 9,
      finalized_reserve_packet_hash: RESERVE_PACKET_HASH,
      nav_per_unit: '1.000000',
    };
  }
  if (method === 'account') return { balance: 0, sequence: 0, public_key_hex: null };
  if (method === 'account_assets') return { assets: [] };
  if (method === 'owned_objects') return { objects: [] };
  if (method === 'account_tx') return [];
  return {};
}

async function startRpcFixture() {
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
          result: rpcResult(request.method),
          error: null,
          events: [],
        })}\n`);
        delimiter = pending.indexOf('\n');
      }
    });
  });
  await new Promise((resolveListen, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolveListen);
  });
  const address = server.address();
  if (!address || typeof address === 'string') throw new Error('local RPC fixture did not receive a TCP port');
  return { server, port: address.port };
}

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

function startProxy({ port, residentPort, rpcPort, token, controlledWallet }) {
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
      PFTL_PRIVATE_SWAP_CONTROLLED_WALLET_ID: controlledWallet,
      PFTL_PRIVATE_SWAP_ROUTE_ID: ROUTE_ID,
      PFTL_PRIVATE_SWAP_TIMEOUT_MS: '5000',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.stdout.on('data', chunk => output.push(chunk.toString('utf8')));
  child.stderr.on('data', chunk => output.push(chunk.toString('utf8')));
  return { child, output };
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
  if (child.exitCode !== null) return;
  const exited = new Promise(resolveExit => child.once('exit', resolveExit));
  if (!child.kill('SIGTERM')) throw new Error('wallet proxy did not accept SIGTERM');
  await Promise.race([
    exited,
    new Promise(resolveWait => setTimeout(resolveWait, 5_000)),
  ]);
  if (child.exitCode === null) {
    const killed = new Promise(resolveExit => child.once('exit', resolveExit));
    if (!child.kill('SIGKILL')) throw new Error('wallet proxy did not accept SIGKILL');
    await killed;
  }
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
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();
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

async function unlockAndOpenPrivatePrimary(page, passphrase) {
  await page.getByText('Wallet locked', { exact: true }).waitFor();
  await page.locator('input[placeholder="Passphrase"]').fill(passphrase);
  await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  await page.locator('.pf-shell').waitFor();
  await page.locator('.pf-sidebar .pf-nav').filter({ hasText: 'Process' }).click();
  await page.locator('#private-navcoin-primary').waitFor();
  await page.waitForFunction(() => {
    const button = [...document.querySelectorAll('#private-navcoin-primary button')]
      .find(candidate => /Sign and issue privately|Resume durable intent|Sign another issue/.test(candidate.textContent || ''));
    return Boolean(button && !button.disabled);
  });
}

async function waitForPendingJob(page, storePath, ingress) {
  const deadline = Date.now() + 5_000;
  let lastState = 'missing';
  while (Date.now() < deadline) {
    const store = await readStore(storePath);
    const [idempotencyKey] = Object.keys(store.jobs);
    const job = idempotencyKey ? store.jobs[idempotencyKey] : null;
    lastState = job?.state || 'missing';
    if (job?.state === 'PUBLISHED') return { idempotencyKey, job };
    await new Promise(resolveWait => setTimeout(resolveWait, 25));
  }
  const uiErrors = await page.locator('#private-navcoin-primary .pf-error').allTextContents();
  throw new Error(
    `production private-primary component did not persist a pending recovery record (last state: ${lastState}; ui: ${uiErrors.join(' | ') || 'none'}; resident calls: ${ingress.map(item => `${item.method} ${item.path}`).join(', ') || 'none'})`,
  );
}

async function scanTreeForValues(root, values) {
  const hits = [];
  async function visit(path) {
    const info = await stat(path);
    if (info.isDirectory()) {
      const entries = await readdir(path);
      for (const entry of entries) await visit(join(path, entry));
      return;
    }
    if (!info.isFile() || info.size > 16 * 1024 * 1024) return;
    const content = await readFile(path);
    if (values.some(value => content.includes(Buffer.from(value)))) hits.push(path);
  }
  await visit(root);
  return hits;
}

function assertNoSensitiveMaterial(label, content, values) {
  for (const value of values) {
    assert.equal(String(content).includes(value), false, `${label} contains generated custody material`);
  }
}

test('journey step 9 resumes the production private-primary recovery component after proxy restart and browser reload', {
  timeout: 120_000,
}, async () => {
  const routeRegistry = rpcResult('navcoin_bridge_routes');
  assert.ok(
    routeRegistry.routes.every(row => typeof row.route_live === 'boolean'),
    'public-browser route fixture must bind boolean route_live before browser launch',
  );
  await readFile(join(DIST_ROOT, 'index.html'), 'utf8');

  const tempRoot = await mkdtemp(join(tmpdir(), 'postfiat-journey-step-9-'));
  const profileDir = join(tempRoot, 'chromium-profile');
  const residentStorePath = join(tempRoot, 'resident-durable-jobs.json');
  const acceptancePath = join(tempRoot, 'journey-step-9-acceptance.json');
  const token = randomBytes(32).toString('hex');
  const passphrase = randomBytes(24).toString('hex');
  const identity = { walletAddress: `pf${'00'.repeat(20)}` };
  const ingress = [];
  const consoleLines = [];
  let rpcFixture;
  let resident;
  let context;
  let bootstrapProxy;
  let firstProxy;
  let secondProxy;
  let seed = '';

  try {
    await writeStore(residentStorePath, initialResidentStore());
    rpcFixture = await startRpcFixture();
    resident = await startResident(residentStorePath, identity, ingress);
    const residentAddress = resident.address();
    if (!residentAddress || typeof residentAddress === 'string') {
      throw new Error('resident rehearsal service did not receive a TCP port');
    }

    const proxyPort = await reservePort();
    const origin = `http://127.0.0.1:${proxyPort}`;
    bootstrapProxy = startProxy({
      port: proxyPort,
      residentPort: residentAddress.port,
      rpcPort: rpcFixture.port,
      token,
      controlledWallet: identity.walletAddress,
    });
    await waitForHttp(`${origin}/healthz`, bootstrapProxy.child, bootstrapProxy.output);

    context = await chromium.launchPersistentContext(profileDir, { headless: true });
    const page = await context.newPage();
    page.on('console', message => consoleLines.push(message.text()));
    page.setDefaultTimeout(15_000);
    page.setDefaultNavigationTimeout(15_000);
    const rootResponse = await page.goto(`${origin}/`, { waitUntil: 'domcontentloaded' });
    assert.equal(rootResponse?.status(), 200, 'production wallet build is served by wallet-proxy');

    const created = await createWallet(page, passphrase);
    seed = created.seed;
    identity.walletAddress = created.address.toLowerCase();

    await terminate(bootstrapProxy.child);
    firstProxy = startProxy({
      port: proxyPort,
      residentPort: residentAddress.port,
      rpcPort: rpcFixture.port,
      token,
      controlledWallet: identity.walletAddress,
    });
    await waitForHttp(`${origin}/healthz`, firstProxy.child, firstProxy.output);

    await page.reload({ waitUntil: 'domcontentloaded' });
    await unlockAndOpenPrivatePrimary(page, passphrase);
    const privatePrimary = page.locator('#private-navcoin-primary');
    await privatePrimary.getByRole('button', { name: /Sign and issue privately/ }).click();
    const pending = await waitForPendingJob(page, residentStorePath, ingress);
    const pendingId = pending.idempotencyKey;
    assert.match(pendingId || '', /^navcoin-browser-issue-[0-9a-f]{24}$/);
    assert.equal(pending.job.submit_count, 1);

    await terminate(firstProxy.child);
    assert.notEqual(firstProxy.child.exitCode, null, 'first actual recovery proxy process stopped');
    secondProxy = startProxy({
      port: proxyPort,
      residentPort: residentAddress.port,
      rpcPort: rpcFixture.port,
      token,
      controlledWallet: identity.walletAddress,
    });
    assert.notEqual(secondProxy.child.pid, firstProxy.child.pid, 'recovery uses a replacement proxy process');
    await waitForHttp(`${origin}/healthz`, secondProxy.child, secondProxy.output);

    await page.reload({ waitUntil: 'domcontentloaded' });
    await unlockAndOpenPrivatePrimary(page, passphrase);
    await privatePrimary.getByText('Private primary swap committed', { exact: true }).first().waitFor();

    const recoveredStore = await readStore(residentStorePath);
    const recoveredJob = recoveredStore.jobs[pendingId];
    assert.equal(recoveredJob.state, 'COMMITTED');
    assert.equal(
      recoveredJob.submit_count,
      3,
      'original + recovery + stable committed replay',
    );
    assert.equal(recoveredJob.commit_count, 1, 'production component resume commits once');
    assert.deepEqual(recoveredJob.final_balance_tuple, FINAL_BALANCE_TUPLE);
    assert.equal(recoveredJob.receipt_identity, FINAL_RECEIPT_IDENTITY);

    const recoveryKey = `postfiat.navcoin_private_primary.${identity.walletAddress}.v1`;
    const persistedRecovery = await page.evaluate(key => localStorage.getItem(key), recoveryKey);
    const parsedRecovery = JSON.parse(persistedRecovery || 'null');
    const persistedRecord = parsedRecovery?.records?.find(record => record.idempotency_key === pendingId);
    assert.equal(persistedRecord?.status, 'COMMITTED', 'production savePftlPrivateRecoveries persisted the resumed state');
    assert.equal(
      persistedRecord?.response?.swap?.certificate_ref,
      FINAL_RECEIPT_IDENTITY,
      'production recovery record preserves the finalized certificate identity',
    );
    assert.equal(
      persistedRecord?.response?.receipt?.receipt_identity,
      FINAL_RECEIPT_IDENTITY,
      'production recovery record preserves the finalized receipt identity',
    );
    assert.deepEqual(
      persistedRecord?.response?.final_balance_tuple,
      FINAL_BALANCE_TUPLE,
      'production recovery record preserves the redacted final balance tuple',
    );
    const forbiddenRecoveryField = /seed|mnemonic|private[_-]?key|owner[_-]?key|spend[_-]?auth/i;
    const visitRecoveryRecord = (value, path = 'recovery') => {
      if (Array.isArray(value)) {
        value.forEach((item, index) => visitRecoveryRecord(item, `${path}[${index}]`));
        return;
      }
      if (!value || typeof value !== 'object') return;
      for (const [key, item] of Object.entries(value)) {
        assert.equal(forbiddenRecoveryField.test(key), false, `forbidden recovery field: ${path}.${key}`);
        visitRecoveryRecord(item, `${path}.${key}`);
      }
    };
    visitRecoveryRecord(persistedRecord);
    assertNoSensitiveMaterial(
      'serialized production recovery record',
      JSON.stringify(persistedRecord),
      [seed, passphrase],
    );

    const submitCountBeforeFinalizedReload = recoveredJob.submit_count;
    await page.reload({ waitUntil: 'domcontentloaded' });
    await unlockAndOpenPrivatePrimary(page, passphrase);
    await privatePrimary.getByText('Private primary swap committed', { exact: true }).first().waitFor();
    const finalizedStore = await readStore(residentStorePath);
    assert.equal(
      finalizedStore.jobs[pendingId].submit_count,
      submitCountBeforeFinalizedReload,
      'production loadPftlPrivateRecoveries loads finalized state without resubmission',
    );
    assert.equal(finalizedStore.jobs[pendingId].commit_count, 1, 'finalized reload does not duplicate success');
    const sidebarLedger = page.locator('.pf-sidebar .pf-ledger');
    await sidebarLedger.waitFor();
    const sidebarLedgerText = await sidebarLedger.textContent() || '';
    assert.equal(
      sidebarLedgerText.includes('height 901'),
      true,
      'visible sidebar reconnects at chain height 901',
    );

      const downloadControl = page.getByRole('button', {
        name: 'Download public receipt',
        exact: true,
      });
      await downloadControl.waitFor({ state: 'visible' });
      const downloadEvent = page.waitForEvent('download');
      await downloadControl.click();
      const download = await downloadEvent;
      const publicReceiptPath = join(tempRoot, 'downloaded-public-receipt.json');
      await download.saveAs(publicReceiptPath);
      const publicReceiptBytes = await readFile(publicReceiptPath);
      const publicReceiptText = publicReceiptBytes.toString('utf8');
      const publicReceipt = JSON.parse(publicReceiptText);
      assert.match(
        String(publicReceipt?.schema || ''),
        /^postfiat[.-].*public[-_.]receipt.*v[0-9]+$/i,
        'download uses a versioned public-receipt schema',
      );
      const operationIdentity = publicReceipt?.idempotency_key
        ?? publicReceipt?.operation?.idempotency_key;
      assert.equal(operationIdentity, pendingId, 'download binds the completed operation identity');
      const operationStatus = publicReceipt?.status ?? publicReceipt?.operation?.status;
      assert.equal(operationStatus, 'COMMITTED', 'download identifies a completed operation');
      const receiptIdentity = publicReceipt?.receipt_identity
        ?? publicReceipt?.certificate_ref
        ?? publicReceipt?.receipt?.receipt_identity
        ?? publicReceipt?.receipt?.certificate_ref;
      assert.equal(
        receiptIdentity,
        persistedRecord?.response?.swap?.certificate_ref,
        'download binds the observed finalized receipt/certificate identity',
      );
      const downloadedBalanceTuple = publicReceipt?.final_balance_tuple
        ?? publicReceipt?.operation?.final_balance_tuple;
      assert.deepEqual(
        downloadedBalanceTuple,
        recoveredJob.final_balance_tuple,
        'download binds the observed final balance tuple',
      );
      const forbiddenField = /seed|private[_-]?key|owner[_-]?key|spend[_-]?auth|secret|mnemonic|backup|passphrase/i;
      const visitPublicReceipt = (value, path = 'receipt') => {
        if (Array.isArray(value)) {
          value.forEach((item, index) => visitPublicReceipt(item, `${path}[${index}]`));
          return;
        }
        if (!value || typeof value !== 'object') return;
        for (const [key, item] of Object.entries(value)) {
          assert.equal(forbiddenField.test(key), false, `forbidden public-receipt field: ${path}.${key}`);
          visitPublicReceipt(item, `${path}.${key}`);
        }
      };
      visitPublicReceipt(publicReceipt);
      assertNoSensitiveMaterial('downloaded public receipt', publicReceiptText, [seed, passphrase]);

    const browserStorage = await page.evaluate(() => JSON.stringify({
      local: Object.fromEntries(Object.keys(localStorage).map(key => [key, localStorage.getItem(key)])),
      session: Object.fromEntries(Object.keys(sessionStorage).map(key => [key, sessionStorage.getItem(key)])),
    }));
    const proxyOutput = [
      ...bootstrapProxy.output,
      ...firstProxy.output,
      ...secondProxy.output,
    ].join('');
    const sensitive = [seed, passphrase];
    assertNoSensitiveMaterial('proxy ingress', JSON.stringify(ingress), sensitive);
    assertNoSensitiveMaterial('proxy console', proxyOutput, sensitive);
    assertNoSensitiveMaterial('browser console', consoleLines.join('\n'), sensitive);
    assertNoSensitiveMaterial('browser storage', browserStorage, sensitive);
    assertNoSensitiveMaterial('durable job store', await readFile(residentStorePath, 'utf8'), sensitive);

    const observed = {
      production_component_recovery_path_exercised: persistedRecord?.status === 'COMMITTED',
      proxy_sigterm_restart_same_port: firstProxy.child.exitCode !== null
        && secondProxy.child.pid !== firstProxy.child.pid,
      durable_chromium_reload_reconnect: sidebarLedgerText.includes('height 901'),
      pending_recovered_once: recoveredJob.commit_count === 1 && recoveredJob.submit_count === 3,
      finalized_loaded_idempotently: finalizedStore.jobs[pendingId].submit_count === submitCountBeforeFinalizedReload,
      final_balance_tuple_matches_fixture: JSON.stringify(recoveredJob.final_balance_tuple) === JSON.stringify(FINAL_BALANCE_TUPLE),
      receipt_identity_matches_fixture: persistedRecord?.response?.swap?.certificate_ref === FINAL_RECEIPT_IDENTITY,
      custody_material_absent: true,
    };
    observed.custody_material_absent = sensitive.every(value => ![
      JSON.stringify(ingress),
      proxyOutput,
      consoleLines.join('\n'),
      browserStorage,
      JSON.stringify(observed),
    ].some(content => content.includes(value)));
    const acceptance = {
      schema: 'postfiat-journey-step-9-wallet-recovery-e2e-v2',
      accepted: Object.values(observed).every(Boolean),
      observed,
    };
    await writeFile(acceptancePath, `${JSON.stringify(acceptance, null, 2)}\n`, { mode: 0o600 });
    assert.equal(acceptance.accepted, true);
    assertNoSensitiveMaterial('produced test evidence', await readFile(acceptancePath, 'utf8'), sensitive);
  } finally {
    if (context) await context.close();
    if (secondProxy) await terminate(secondProxy.child);
    if (firstProxy) await terminate(firstProxy.child);
    if (bootstrapProxy) await terminate(bootstrapProxy.child);
    if (resident) await closeServer(resident);
    if (rpcFixture) await closeServer(rpcFixture.server);
    if (seed) {
      const profileHits = await scanTreeForValues(profileDir, [seed, passphrase]);
      assert.deepEqual(profileHits, [], 'durable Chromium profile excludes generated custody material');
    }
    await rm(tempRoot, { recursive: true, force: true });
  }
});
