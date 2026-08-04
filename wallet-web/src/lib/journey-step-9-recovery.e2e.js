import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { createServer } from 'node:http';
import { mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const WALLET_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const REPOSITORY_ROOT = resolve(WALLET_ROOT, '..');
const PROXY_ROOT = join(REPOSITORY_ROOT, 'wallet-proxy');
const DIST_ROOT = join(WALLET_ROOT, 'dist');

const CONTROLLED_WALLET = `pf${'a1'.repeat(20)}`;
const ROUTE_ID = 'journey-step-9-private-primary';
const RECOVERY_KEY = `postfiat.journey.step9.recovery.${CONTROLLED_WALLET}.v1`;
const CONNECTION_KEY = 'postfiat.journey.step9.connection.v1';
const PENDING_ID = 'journey-step-9-pending';
const FINALIZED_ID = 'journey-step-9-finalized';
const PUBLIC_KEY_HEX = 'b2'.repeat(1952);
const SIGNATURE_HEX = 'c3'.repeat(3309);

const FINAL_BALANCE_TUPLE = Object.freeze([
  Object.freeze({ asset_id: 'pfusdc-test-asset', amount_atoms: '3988000' }),
  Object.freeze({ asset_id: 'navcoin-test-asset', amount_atoms: '1010000' }),
]);

const FIXTURES = Object.freeze({
  pending: Object.freeze({
    idempotency_key: PENDING_ID,
    receipt_identity: 'journey-step-9-pending-final-receipt',
    final_balance_tuple: FINAL_BALANCE_TUPLE,
  }),
  finalized: Object.freeze({
    idempotency_key: FINALIZED_ID,
    receipt_identity: 'journey-step-9-finalized-receipt',
    final_balance_tuple: FINAL_BALANCE_TUPLE,
  }),
});

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function signedIntent(idempotencyKey) {
  return {
    schema: 'postfiat.pftl_swap.signed_intent.v1',
    algorithm_id: 'ML-DSA-65',
    public_key_hex: PUBLIC_KEY_HEX,
    signature_hex: SIGNATURE_HEX,
    intent: {
      schema: 'postfiat.pftl_swap.intent.v1',
      chain_id: 'postfiat-local-rehearsal',
      genesis_hash: 'd4'.repeat(48),
      protocol_version: 1,
      principal: CONTROLLED_WALLET,
      controlled_wallet_id: CONTROLLED_WALLET,
      route_id: ROUTE_ID,
      direction: 'issue',
      output_mode: 'private',
      input_reference: 'transparent-pfusdc',
      input_amount_atoms: 1005000,
      minimum_output_amount_atoms: 1000000,
      maximum_fee_atoms: 5000,
      quote_id: 'e5'.repeat(48),
      pricing_nav_epoch: 9,
      policy_hash: 'f6'.repeat(48),
      expiry_height: 1200,
      idempotency_key: idempotencyKey,
    },
  };
}

function recoveryRecord({ fixture, status, intent }) {
  return {
    idempotency_key: fixture.idempotency_key,
    status,
    signed_intent: intent,
    final_balance_tuple: status === 'COMMITTED' ? clone(fixture.final_balance_tuple) : null,
    receipt_identity: status === 'COMMITTED' ? fixture.receipt_identity : null,
  };
}

function initialResidentStore() {
  return {
    schema: 'postfiat-journey-step-9-resident-job-store-v1',
    jobs: {
      [FINALIZED_ID]: {
        idempotency_key: FINALIZED_ID,
        state: 'COMMITTED',
        submit_count: 0,
        commit_count: 1,
        receipt_identity: FIXTURES.finalized.receipt_identity,
        final_balance_tuple: clone(FIXTURES.finalized.final_balance_tuple),
      },
    },
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

function residentResult(job, replayed) {
  const committed = job.state === 'COMMITTED';
  return {
    ok: true,
    replayed,
    swap: {
      swap_id: `swap-${job.idempotency_key}`,
      idempotency_key: job.idempotency_key,
      quote_id: 'e5'.repeat(48),
      direction: 'issue',
      input_amount_atoms: 1005000,
      minimum_output_amount_atoms: 1000000,
      state: job.state,
      batch_hash: 'ab'.repeat(32),
      committed_height: committed ? 901 : null,
      certificate_ref: committed ? job.receipt_identity : null,
    },
    final_balance_tuple: committed ? clone(job.final_balance_tuple) : null,
    receipt: committed ? { receipt_identity: job.receipt_identity, finalized: true } : null,
  };
}

async function startResident(storePath, ingress) {
  const server = createServer(async (request, response) => {
    const url = new URL(request.url || '/', 'http://127.0.0.1');

    if (request.method === 'GET' && url.pathname === '/v1/ready') {
      sendJson(response, 200, {
        ok: true,
        schema: 'postfiat.pftl_swap.readiness.v1',
        ready: true,
        local_only: true,
        controlled_wallet_id: CONTROLLED_WALLET,
        route_id: ROUTE_ID,
      });
      return;
    }

    if (request.method === 'GET' && url.pathname === '/v1/status') {
      const idempotencyKey = url.searchParams.get('id') || '';
      const store = await readStore(storePath);
      const job = store.jobs[idempotencyKey];
      ingress.push({ method: request.method, path: url.pathname, body: idempotencyKey });
      if (!job) {
        sendJson(response, 404, { ok: false, message: 'local rehearsal job was not found' });
        return;
      }
      sendJson(response, 200, residentResult(job, true));
      return;
    }

    if (request.method === 'POST' && url.pathname === '/v1/swap') {
      const bodyText = await readRequestBody(request);
      ingress.push({ method: request.method, path: url.pathname, body: bodyText });
      const body = JSON.parse(bodyText);
      const idempotencyKey = body?.signed_intent?.intent?.idempotency_key;
      const principal = body?.signed_intent?.intent?.principal;
      const routeId = body?.signed_intent?.intent?.route_id;
      if (!idempotencyKey || principal !== CONTROLLED_WALLET || routeId !== ROUTE_ID) {
        sendJson(response, 400, { ok: false, message: 'local rehearsal intent binding failed' });
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
          receipt_identity: FIXTURES.pending.receipt_identity,
          final_balance_tuple: clone(FIXTURES.pending.final_balance_tuple),
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

    sendJson(response, 404, { ok: false, message: 'local rehearsal resident path was not found' });
  });

  await listen(server);
  return server;
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

function startProxy({ port, residentPort, token, runStorePath, idempotencyStorePath }) {
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
      RPC_PORT: '1',
      RPC_FLEET: 'validator-0=127.0.0.1:1',
      NAVSWAP_RUN_STORE_PATH: runStorePath,
      NAVSWAP_IDEMPOTENCY_STORE_PATH: idempotencyStorePath,
      PFTL_PRIVATE_SWAP_URL: `http://127.0.0.1:${residentPort}`,
      PFTL_PRIVATE_SWAP_CONTROLLED_WALLET_ID: CONTROLLED_WALLET,
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

async function initialPendingOperation(page, { pendingIntent, finalizedIntent, sensitive }) {
  return page.evaluate(async ({ recoveryKey, connectionKey, pending, finalized, sensitiveValues }) => {
    window.__journeyStep9Ephemeral = sensitiveValues;
    const health = await fetch('/healthz', { cache: 'no-store' });
    if (!health.ok) throw new Error('production wallet proxy health check failed before interruption');

    const sessionResponse = await fetch('/api/bridge/local-session', {
      headers: { Accept: 'application/json' },
      cache: 'no-store',
    });
    const session = await sessionResponse.json();
    if (!sessionResponse.ok || session?.ok !== true || typeof session?.token !== 'string') {
      throw new Error('production wallet did not establish its controlled local proxy session');
    }

    const response = await fetch('/api/pftl-private-swap/jobs', {
      method: 'POST',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        Authorization: `Bearer ${session.token}`,
      },
      body: JSON.stringify({ signed_intent: pending }),
      cache: 'no-store',
    });
    const result = await response.json();
    if (response.status !== 202 || result?.swap?.state !== 'PUBLISHED') {
      throw new Error('pending local rehearsal operation was not durably journaled');
    }

    localStorage.setItem(recoveryKey, JSON.stringify({
      schema: 'postfiat.journey-step-9.browser-recovery.v1',
      records: [
        {
          idempotency_key: pending.intent.idempotency_key,
          status: 'PUBLISHED',
          signed_intent: pending,
          final_balance_tuple: null,
          receipt_identity: null,
        },
        {
          idempotency_key: finalized.intent.idempotency_key,
          status: 'COMMITTED',
          signed_intent: finalized,
          final_balance_tuple: [
            { asset_id: 'pfusdc-test-asset', amount_atoms: '3988000' },
            { asset_id: 'navcoin-test-asset', amount_atoms: '1010000' },
          ],
          receipt_identity: 'journey-step-9-finalized-receipt',
        },
      ],
    }));
    sessionStorage.setItem(connectionKey, 'connected');
    return {
      state: result.swap.state,
      storage: {
        local: Object.fromEntries(Object.keys(localStorage).map(key => [key, localStorage.getItem(key)])),
        session: Object.fromEntries(Object.keys(sessionStorage).map(key => [key, sessionStorage.getItem(key)])),
      },
    };
  }, {
    recoveryKey: RECOVERY_KEY,
    connectionKey: CONNECTION_KEY,
    pending: pendingIntent,
    finalized: finalizedIntent,
    sensitiveValues: sensitive,
  });
}

async function recoverAfterReload(page) {
  return page.evaluate(async ({ recoveryKey, connectionKey, pendingId, finalizedId }) => {
    if (Object.hasOwn(window, '__journeyStep9Ephemeral')) {
      throw new Error('browser reload did not clear ephemeral operation state');
    }

    const stored = JSON.parse(localStorage.getItem(recoveryKey) || 'null');
    const pending = stored?.records?.find(record => record.idempotency_key === pendingId);
    const finalized = stored?.records?.find(record => record.idempotency_key === finalizedId);
    if (!pending || pending.status !== 'PUBLISHED' || !pending.signed_intent || !finalized) {
      throw new Error('browser durable recovery records did not survive reload');
    }

    const health = await fetch('/healthz', { cache: 'no-store' });
    if (!health.ok) throw new Error('wallet proxy did not reconnect after restart');
    sessionStorage.setItem(connectionKey, 'reconnected');

    const sessionResponse = await fetch('/api/bridge/local-session', {
      headers: { Accept: 'application/json' },
      cache: 'no-store',
    });
    const session = await sessionResponse.json();
    if (!sessionResponse.ok || session?.ok !== true || typeof session?.token !== 'string') {
      throw new Error('wallet reconnect did not restore its local proxy session');
    }

    const recoveredResponse = await fetch('/api/pftl-private-swap/jobs', {
      method: 'POST',
      headers: {
        Accept: 'application/json',
        'Content-Type': 'application/json',
        Authorization: `Bearer ${session.token}`,
      },
      body: JSON.stringify({ signed_intent: pending.signed_intent }),
      cache: 'no-store',
    });
    const recovered = await recoveredResponse.json();
    if (!recoveredResponse.ok || recovered?.swap?.state !== 'COMMITTED') {
      throw new Error('pending operation did not recover to a finalized state');
    }

    const finalizedResponse = await fetch(
      `/api/pftl-private-swap/jobs/${encodeURIComponent(finalized.idempotency_key)}`,
      {
        headers: { Accept: 'application/json', Authorization: `Bearer ${session.token}` },
        cache: 'no-store',
      },
    );
    const finalizedResult = await finalizedResponse.json();
    if (!finalizedResponse.ok || finalizedResult?.swap?.state !== 'COMMITTED') {
      throw new Error('already-finalized operation did not load idempotently after reload');
    }

    pending.status = 'COMMITTED';
    pending.final_balance_tuple = recovered.final_balance_tuple;
    pending.receipt_identity = recovered.receipt?.receipt_identity || null;
    stored.records = stored.records.map(record => record.idempotency_key === pendingId ? pending : record);
    localStorage.setItem(recoveryKey, JSON.stringify(stored));

    return {
      connection_status: sessionStorage.getItem(connectionKey),
      recovered,
      finalized: finalizedResult,
      storage: {
        local: Object.fromEntries(Object.keys(localStorage).map(key => [key, localStorage.getItem(key)])),
        session: Object.fromEntries(Object.keys(sessionStorage).map(key => [key, sessionStorage.getItem(key)])),
      },
    };
  }, {
    recoveryKey: RECOVERY_KEY,
    connectionKey: CONNECTION_KEY,
    pendingId: PENDING_ID,
    finalizedId: FINALIZED_ID,
  });
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

test('journey step 9 recovers pending and finalized production-wallet operations across proxy restart and browser reload', {
  timeout: 90_000,
}, async () => {
  await readFile(join(DIST_ROOT, 'index.html'), 'utf8');

  const tempRoot = await mkdtemp(join(tmpdir(), 'postfiat-journey-step-9-'));
  const profileDir = join(tempRoot, 'chromium-profile');
  const residentStorePath = join(tempRoot, 'resident-durable-jobs.json');
  const proxyRunStorePath = join(tempRoot, 'proxy-navswap-runs.jsonl');
  const proxyIdempotencyStorePath = join(tempRoot, 'proxy-navswap-idempotency.jsonl');
  const acceptancePath = join(tempRoot, 'journey-step-9-acceptance.json');
  const ingress = [];
    const consoleLines = [];
  const sensitive = [randomBytes(32).toString('hex'), randomBytes(32).toString('hex')];
  const token = randomBytes(32).toString('hex');
  const pendingIntent = signedIntent(PENDING_ID);
  const finalizedIntent = signedIntent(FINALIZED_ID);
  let resident;
  let context;
  let firstProxy;
  let secondProxy;

  try {
    await writeStore(residentStorePath, initialResidentStore());
    resident = await startResident(residentStorePath, ingress);
    const residentAddress = resident.address();
    if (!residentAddress || typeof residentAddress === 'string') {
      throw new Error('resident rehearsal service did not receive a TCP port');
    }

    const proxyPort = await reservePort();
    const origin = `http://127.0.0.1:${proxyPort}`;
    firstProxy = startProxy({
      port: proxyPort,
      residentPort: residentAddress.port,
      token,
      runStorePath: proxyRunStorePath,
      idempotencyStorePath: proxyIdempotencyStorePath,
    });
    await waitForHttp(`${origin}/healthz`, firstProxy.child, firstProxy.output);

    context = await chromium.launchPersistentContext(profileDir, { headless: true });
    const page = await context.newPage();
    page.on('console', message => consoleLines.push(message.text()));
    page.setDefaultTimeout(10_000);
    page.setDefaultNavigationTimeout(10_000);
    const rootResponse = await page.goto(`${origin}/`, { waitUntil: 'domcontentloaded' });
    assert.equal(rootResponse?.status(), 200, 'production wallet build is served by the actual wallet proxy');
    await page.waitForFunction(() => document.querySelector('#root')?.childElementCount > 0);

    const pendingBeforeRestart = await initialPendingOperation(page, {
      pendingIntent,
      finalizedIntent,
      sensitive,
    });
    assert.equal(pendingBeforeRestart.state, 'PUBLISHED');
    assert.ok(pendingBeforeRestart.storage.local[RECOVERY_KEY], 'pending recovery record is durable browser storage');
    assert.equal(
      JSON.stringify(pendingBeforeRestart.storage).includes(sensitive[0]) || JSON.stringify(pendingBeforeRestart.storage).includes(sensitive[1]),
      false,
      'browser storage excludes generated seed and owner private key',
    );

    const pendingStoreBeforeRestart = await readStore(residentStorePath);
    assert.equal(pendingStoreBeforeRestart.jobs[PENDING_ID].state, 'PUBLISHED');
    assert.equal(pendingStoreBeforeRestart.jobs[PENDING_ID].submit_count, 1);

    await terminate(firstProxy.child);
    assert.notEqual(firstProxy.child.exitCode, null, 'the first actual wallet-proxy process stopped');
    const disconnected = await page.evaluate(async () => {
      try {
        const response = await fetch('/healthz', { cache: 'no-store' });
        return !response.ok;
      } catch (error) {
        return error instanceof Error || typeof error === 'object';
      }
    });
    assert.equal(disconnected, true, 'browser observes the actual proxy interruption');

    secondProxy = startProxy({
      port: proxyPort,
      residentPort: residentAddress.port,
      token,
      runStorePath: proxyRunStorePath,
      idempotencyStorePath: proxyIdempotencyStorePath,
    });
    assert.notEqual(secondProxy.child.pid, firstProxy.child.pid, 'recovery uses a replacement proxy process');
    await waitForHttp(`${origin}/healthz`, secondProxy.child, secondProxy.output);

    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.waitForFunction(() => document.querySelector('#root')?.childElementCount > 0);
    const recovered = await recoverAfterReload(page);
    assert.equal(recovered.connection_status, 'reconnected', 'browser connection status returns after proxy restart');
    assert.deepEqual(recovered.recovered.final_balance_tuple, FIXTURES.pending.final_balance_tuple);
    assert.equal(recovered.recovered.receipt?.receipt_identity, FIXTURES.pending.receipt_identity);
    assert.deepEqual(recovered.finalized.final_balance_tuple, FIXTURES.finalized.final_balance_tuple);
    assert.equal(recovered.finalized.receipt?.receipt_identity, FIXTURES.finalized.receipt_identity);

    const recoveredStore = await readStore(residentStorePath);
    assert.equal(recoveredStore.jobs[PENDING_ID].state, 'COMMITTED');
    assert.equal(recoveredStore.jobs[PENDING_ID].submit_count, 2);
    assert.equal(recoveredStore.jobs[PENDING_ID].commit_count, 1, 'pending recovery finalizes exactly once');
    assert.equal(recoveredStore.jobs[FINALIZED_ID].submit_count, 0, 'finalized recovery loads without resubmission');
    assert.equal(recoveredStore.jobs[FINALIZED_ID].commit_count, 1, 'finalized recovery does not duplicate success');

    const postReloadStorage = JSON.stringify(recovered.storage);
    const proxyOutput = [...firstProxy.output, ...secondProxy.output].join('');
    const ingressBody = JSON.stringify(ingress);
    const residentStore = await readFile(residentStorePath, 'utf8');
    assertNoSensitiveMaterial('proxy ingress', ingressBody, sensitive);
    assertNoSensitiveMaterial('proxy console', proxyOutput, sensitive);
    assertNoSensitiveMaterial('browser console', consoleLines.join('\n'), sensitive);
    assertNoSensitiveMaterial('browser storage', postReloadStorage, sensitive);
    assertNoSensitiveMaterial('durable job store', residentStore, sensitive);

    const acceptance = {
      schema: 'postfiat-journey-step-9-wallet-recovery-e2e-v1',
      accepted: true,
      runtime: 'production wallet build plus wallet-proxy process and durable Chromium profile',
      pending_survives_proxy_restart_and_reload: true,
      proxy_restarted_same_port: true,
      browser_reconnected: true,
      pending_final_balance_tuple_matches_fixture: true,
      pending_receipt_identity_matches_fixture: true,
      finalized_loaded_without_resubmission: true,
      finalized_final_balance_tuple_matches_fixture: true,
      finalized_receipt_identity_matches_fixture: true,
      generated_custody_material_observed: false,
    };
    await mkdir(dirname(acceptancePath), { recursive: true });
    await writeFile(acceptancePath, `${JSON.stringify(acceptance, null, 2)}\n`, { mode: 0o600 });
    assertNoSensitiveMaterial('produced test evidence', await readFile(acceptancePath, 'utf8'), sensitive);
  } finally {
    if (context) await context.close();
    if (secondProxy) await terminate(secondProxy.child);
    if (firstProxy) await terminate(firstProxy.child);
    if (resident) await closeServer(resident);
    if (context) {
      const profileHits = await scanTreeForValues(profileDir, sensitive);
      assert.deepEqual(profileHits, [], 'durable Chromium profile excludes generated custody material');
    }
    await rm(tempRoot, { recursive: true, force: true });
  }
});
