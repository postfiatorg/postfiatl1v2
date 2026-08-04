import assert from 'node:assert/strict';
import { createHash, randomBytes } from 'node:crypto';
import { readFile, stat, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

import {
  loadNavcoinExportReadiness,
  waitForNavcoinExportJob,
} from './navcoin-export-relay.js';
import {
  loadNavcoinReturnReadiness,
  waitForNavcoinReturnJob,
} from './navcoin-return-relay.js';
import {
  loadPftlPrivateReadiness,
  loadPftlPrivateRecoveries,
  pftlPrivateRecoveryKey,
} from './pftl-private-primary.js';

const CANDIDATE_REVISION = '39f7fae3191aa34c376ae1657650a9ec2444f421';
const POLL_INTERVAL_MS = 5_000;
const MAX_ASYNC_WAIT_MS = 12 * 60 * 60 * 1_000;
const FORBIDDEN_FIELD = /seed|mnemonic|private[_-]?key|owner[_-]?key|spend[_-]?auth/i;
const WALLET_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const PRODUCTION_COMPONENT_GRAPH = Object.freeze([
  join(WALLET_ROOT, 'src/App.jsx'),
  join(WALLET_ROOT, 'src/components/Swap.jsx'),
  join(WALLET_ROOT, 'src/components/NavcoinMarket.jsx'),
  join(WALLET_ROOT, 'src/components/NavcoinPrimaryMarket.jsx'),
  join(WALLET_ROOT, 'src/components/PftlPrivatePrimary.jsx'),
]);
const JOURNEY_LABELS = Object.freeze([
  'build identity',
  'browser-controlled connect/create',
  'six sources/proofs/NAV',
  'A666+pfUSDC balances',
  'transparent issue/redeem',
  'private issue/redeem',
  'export',
  'return',
  'restart+reload recovery',
  'public receipt download',
]);
const PRODUCTION_MODULES = Object.freeze({
  loadNavcoinExportReadiness,
  waitForNavcoinExportJob,
  loadNavcoinReturnReadiness,
  waitForNavcoinReturnJob,
  loadPftlPrivateReadiness,
  loadPftlPrivateRecoveries,
  pftlPrivateRecoveryKey,
});

function requiredPath(name) {
  const value = String(process.env[name] || '').trim();
  if (!value) throw new Error(`${name} is required`);
  return resolve(value);
}

function requiredLoopbackOrigin() {
  const raw = String(process.env.POSTFIAT_R4_WALLET_ORIGIN || '').trim();
  if (!raw) throw new Error('POSTFIAT_R4_WALLET_ORIGIN is required');
  const origin = new URL(raw);
  assert.equal(origin.protocol, 'http:', 'rehearsal wallet must use HTTP on loopback');
  assert.ok(['127.0.0.1', 'localhost', '::1'].includes(origin.hostname), 'wallet origin must be loopback');
  assert.ok(origin.port, 'wallet origin must include an explicit port');
  assert.equal(origin.username, '', 'wallet origin must not contain credentials');
  assert.equal(origin.password, '', 'wallet origin must not contain credentials');
  return origin;
}

function requiredAsyncBudget() {
  const value = Number(process.env.POSTFIAT_R4_ASYNC_MAX_WAIT_MS);
  assert.ok(Number.isSafeInteger(value) && value > 0, 'POSTFIAT_R4_ASYNC_MAX_WAIT_MS is required');
  assert.ok(value <= MAX_ASYNC_WAIT_MS, 'asynchronous artifact wait exceeds 12 hours');
  return value;
}

async function readJson(path, label) {
  const parsed = JSON.parse(await readFile(path, 'utf8'));
  assert.ok(parsed && typeof parsed === 'object' && !Array.isArray(parsed), `${label} must be a JSON object`);
  return parsed;
}

function endpointStrings(value, path = 'manifest', output = []) {
  if (Array.isArray(value)) {
    value.forEach((item, index) => endpointStrings(item, `${path}[${index}]`, output));
    return output;
  }
  if (!value || typeof value !== 'object') return output;
  for (const [key, item] of Object.entries(value)) {
    const nextPath = `${path}.${key}`;
    if (typeof item === 'string' && /(url|origin|endpoint|rpc|host)$/i.test(key)) {
      output.push({ path: nextPath, value: item });
    }
    endpointStrings(item, nextPath, output);
  }
  return output;
}

function assertLoopbackManifest(label, manifest) {
  const endpoints = endpointStrings(manifest);
  assert.ok(endpoints.length > 0, `${label} exposes no endpoint bindings`);
  for (const endpoint of endpoints) {
    if (endpoint.value === '127.0.0.1' || endpoint.value === 'localhost' || endpoint.value === '::1') continue;
    let parsed;
    try {
      parsed = new URL(endpoint.value);
    } catch {
      if (/^127\.0\.0\.1:\d+$/.test(endpoint.value)) continue;
      throw new Error(`${endpoint.path} is not an explicit loopback endpoint`);
    }
    assert.ok(['127.0.0.1', 'localhost', '::1'].includes(parsed.hostname), `${endpoint.path} is not loopback`);
    assert.equal(parsed.username, '', `${endpoint.path} contains credentials`);
    assert.equal(parsed.password, '', `${endpoint.path} contains credentials`);
  }
}

function observedStep(step, observations, predicates) {
  assert.equal(JOURNEY_LABELS[step - 1], observations.label, `step ${step} label drift`);
  const checks = Object.fromEntries(Object.entries(predicates).map(([name, predicate]) => [name, Boolean(predicate)]));
  return {
    step,
    label: observations.label,
    observations,
    checks,
    ok: Object.values(checks).length > 0 && Object.values(checks).every(Boolean),
  };
}

function scanForbiddenFields(value, path = 'artifact') {
  if (Array.isArray(value)) {
    value.forEach((item, index) => scanForbiddenFields(item, `${path}[${index}]`));
    return;
  }
  if (!value || typeof value !== 'object') return;
  for (const [key, item] of Object.entries(value)) {
    assert.equal(FORBIDDEN_FIELD.test(key), false, `forbidden custody field ${path}.${key}`);
    scanForbiddenFields(item, `${path}.${key}`);
  }
}

function assertCustodyValuesAbsent(label, value, custodyValues) {
  const serialized = typeof value === 'string' ? value : JSON.stringify(value);
  for (const custodyValue of custodyValues) {
    assert.equal(serialized.includes(custodyValue), false, `${label} contains browser custody material`);
  }
}

async function sha256File(path) {
  return createHash('sha256').update(await readFile(path)).digest('hex');
}

async function pollContentAddressedSlot(path, kind, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    try {
      last = await readJson(path, `${kind} artifact slot`);
      if (last.status === 'accepted') {
        assert.match(String(last.artifact_sha256 || ''), /^[0-9a-f]{64}$/);
        assert.match(String(last.receipt_identity || ''), /^[A-Za-z0-9._:-]{8,256}$/);
        assert.ok(last.observed_chain_state && Number.isSafeInteger(last.observed_chain_state.height));
        assert.match(String(last.observed_chain_state.tip || ''), /^[0-9a-f]{64,96}$/);
        assert.match(String(last.observed_chain_state.state_root || ''), /^[0-9a-f]{64,96}$/);
        assert.equal(last.observed_chain_state.finalized, true);
        const artifactPath = resolve(String(last.artifact_path || ''));
        assert.equal(await sha256File(artifactPath), last.artifact_sha256, `${kind} artifact hash mismatch`);
        return last;
      }
      if (last.status === 'failed') throw new Error(`${kind} relay failed: ${last.failure_code || 'unknown'}`);
    } catch (error) {
      if (error?.code !== 'ENOENT') throw error;
    }
    await new Promise(resolveWait => setTimeout(resolveWait, POLL_INTERVAL_MS));
  }
  const error = new Error(`${kind} content-addressed proof/relay slot timed out`);
  error.code = `a666_r4_${kind}_slot_timeout`;
  error.lastObserved = last;
  throw error;
}

async function verifyProductionGraph() {
  const [app, swap, marketAdapter, market, privatePrimary] = await Promise.all(
    PRODUCTION_COMPONENT_GRAPH.map(path => readFile(path, 'utf8')),
  );
  const checks = {
    app_renders_swap: app.includes("import Swap from './components/Swap.jsx'"),
    app_renders_market_adapter: app.includes("import NavcoinMarket from './components/NavcoinMarket.jsx'"),
    swap_renders_private_primary: swap.includes("import PftlPrivatePrimary from './PftlPrivatePrimary.jsx'"),
    adapter_renders_primary_market: marketAdapter.includes("import PftlUniswapPrimaryMarket from './NavcoinPrimaryMarket.jsx'"),
    primary_uses_export_relay: market.includes("from '../lib/navcoin-export-relay.js'"),
    primary_uses_return_relay: market.includes("from '../lib/navcoin-return-relay.js'"),
    private_uses_production_library: privatePrimary.includes("from '../lib/pftl-private-primary.js'"),
  };
  assert.ok(Object.values(checks).every(Boolean), 'production rendered component import graph changed');
  assert.ok(Object.values(PRODUCTION_MODULES).every(value => typeof value === 'function'));
  return checks;
}

async function createBrowserControlledWallet(page, passphrase) {
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();
  const seedDisplay = page.locator('.pf-seed-display');
  await seedDisplay.waitFor({ state: 'visible' });
  const seed = String(await seedDisplay.textContent() || '').trim();
  assert.match(seed, /^[0-9a-f]{64}$/);
  await page.locator('.pf-checkbox input').check();
  await page.locator('input[placeholder="Encryption passphrase (min 10 chars)"]').fill(passphrase);
  await page.locator('input[placeholder="Confirm passphrase"]').fill(passphrase);
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).last().click();
  await page.locator('.pf-shell').waitFor();
  const address = await page.evaluate(() => new Promise((resolveAddress, rejectAddress) => {
    const request = indexedDB.open('postfiat-wallet');
    request.onerror = () => rejectAddress(request.error);
    request.onsuccess = () => {
      const database = request.result;
      const transaction = database.transaction('vaults', 'readonly');
      const record = transaction.objectStore('vaults').get('default');
      record.onerror = () => rejectAddress(record.error);
      record.onsuccess = () => {
        const result = record.result?.metadata?.address || '';
        database.close();
        resolveAddress(result);
      };
    };
  }));
  assert.match(String(address), /^pf[0-9a-f]{40}$/);
  return { seed, address: String(address) };
}

async function unlock(page, passphrase) {
  const lock = page.getByText('Wallet locked', { exact: true });
  if (await lock.isVisible().catch(() => false)) {
    await page.locator('input[placeholder="Passphrase"]').fill(passphrase);
    await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  }
  await page.locator('.pf-shell').waitFor();
}

async function openPrimaryMarket(page) {
  await page.locator('.pf-sidebar .pf-nav').filter({ hasText: 'NAV Markets' }).click();
  await page.locator('[data-testid="navcoin-market"]').waitFor();
  return page.locator('[data-testid="navcoin-market"]');
}

async function waitForReserveProofFrame(frames) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const match = [...frames].reverse().find(value => {
      const packets = value?.result?.packets ?? value?.result?.result?.packets;
      return Array.isArray(packets);
    });
    if (match) return match;
    await new Promise(resolveWait => setTimeout(resolveWait, 100));
  }
  throw new Error('production wallet did not observe the reserve-proof RPC response');
}

async function marketBalances(market) {
  const rows = await market.locator('.navcoin-primary-balance').allTextContents();
  const observed = Object.fromEntries(rows.map(row => {
    const normalized = row.replace(/\s+/g, ' ').trim();
    const split = normalized.search(/[0-9]/);
    return [normalized.slice(0, split).trim(), normalized.slice(split).trim()];
  }));
  assert.ok(Object.keys(observed).some(key => /pfUSDC/i.test(key)));
  assert.ok(Object.keys(observed).some(key => /A666/i.test(key)));
  return observed;
}

async function completeTransparentRoundTrip(page, market) {
  const before = await marketBalances(market);
  await market.getByRole('button', { name: /Mint A666/ }).first().click();
  await market.locator('#navcoin-amount').fill('1');
  await market.getByRole('button', { name: 'Keep on PFTL', exact: true }).click();
  await market.getByRole('button', { name: /^Mint 1(?:\.0+)? A666$/ }).click();
  await market.getByText('A666 purchase complete', { exact: true }).waitFor();

  await market.getByRole('button', { name: 'Redeem', exact: true }).first().click();
  await market.getByRole('button', { name: 'From PFTL', exact: true }).click();
  await market.locator('#navcoin-amount').fill('1');
  await market.getByRole('button', { name: /^Redeem 1(?:\.0+)? A666$/ }).click();
  await market.getByText('A666 redemption complete', { exact: true }).waitFor();
  const after = await marketBalances(market);
  const progress = await market.locator('.navcoin-primary-progress-step.done').allTextContents();
  return { before, after, progress };
}

async function completePrivateRoundTrip(page) {
  await page.locator('.pf-sidebar .pf-nav').filter({ hasText: 'Process' }).click();
  const panel = page.locator('#private-navcoin-primary');
  await panel.waitFor();
  await panel.locator('select').first().selectOption('issue');
  await panel.locator('input[inputmode="decimal"]').fill('1');
  await panel.getByRole('button', { name: /Sign and issue privately/ }).click();
  await panel.getByText('Private primary swap committed', { exact: true }).first().waitFor();
  await panel.locator('select').first().selectOption('redeem');
  await panel.locator('select').nth(2).selectOption({ index: 1 });
  await panel.getByRole('button', { name: /Sign another redeem|Sign and redeem privately/ }).click();
  await panel.getByText('Private primary swap committed', { exact: true }).first().waitFor();
  return page.evaluate(() => {
    const key = Object.keys(localStorage).find(item => item.startsWith('postfiat.navcoin_private_primary.'));
    return { key, value: key ? JSON.parse(localStorage.getItem(key) || 'null') : null };
  });
}

async function requestProxyRestart(requestPath, pidPath) {
  const before = Number((await readFile(pidPath, 'utf8')).trim());
  assert.ok(Number.isSafeInteger(before) && before > 0);
  await writeFile(requestPath, 'restart\n', { flag: 'wx', mode: 0o600 });
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    const after = Number((await readFile(pidPath, 'utf8').catch(() => '')).trim());
    if (Number.isSafeInteger(after) && after > 0 && after !== before) return { before, after };
    await new Promise(resolveWait => setTimeout(resolveWait, 250));
  }
  throw new Error('production wallet proxy did not restart');
}

test('A666 R4 offline Ethereum rehearsal drives the production rendered wallet journey', {
  timeout: MAX_ASYNC_WAIT_MS + 10 * 60 * 1_000,
}, async () => {
  const origin = requiredLoopbackOrigin();
  const asyncBudgetMs = requiredAsyncBudget();
  const fireControlPath = requiredPath('POSTFIAT_R4_FIRE_CONTROL_MANIFEST');
  const setupPath = requiredPath('POSTFIAT_R4_SETUP_MANIFEST');
  const deploymentPath = requiredPath('POSTFIAT_R4_DEPLOYMENT_MANIFEST');
  const exportSlotPath = requiredPath('POSTFIAT_R4_EXPORT_ARTIFACT_SLOT');
  const returnSlotPath = requiredPath('POSTFIAT_R4_RETURN_ARTIFACT_SLOT');
  const proxyRestartPath = requiredPath('POSTFIAT_R4_PROXY_RESTART_REQUEST');
  const proxyPidPath = requiredPath('POSTFIAT_R4_PROXY_PID_FILE');
  const reportPath = requiredPath('POSTFIAT_R4_JOURNEY_REPORT');
  const runClassification = String(process.env.POSTFIAT_R4_RUN_CLASSIFICATION || 'construction');
  assert.equal(runClassification, 'construction', 'this increment cannot produce an official pass');

  const [fireControl, setup, deployment, productionGraph] = await Promise.all([
    readJson(fireControlPath, 'fire-control manifest'),
    readJson(setupPath, 'setup manifest'),
    readJson(deploymentPath, 'deployment manifest'),
    verifyProductionGraph(),
  ]);
  assert.equal(setup.candidate?.revision ?? setup.candidate_revision, CANDIDATE_REVISION);
  assert.equal(deployment.candidate_revision ?? deployment.candidate?.revision, CANDIDATE_REVISION);
  assert.equal(fireControl.candidate_revision ?? fireControl.bindings?.candidate_revision, CANDIDATE_REVISION);
  assertLoopbackManifest('setup manifest', setup);
  assertLoopbackManifest('deployment manifest', deployment);
  assert.equal(fireControl.ready_to_fire, true, 'fire-control must be computed GREEN before execution');

  const passphrase = randomBytes(24).toString('hex');
  let seed = '';
  const results = [];
  const context = await chromium.launchPersistentContext('', { headless: true, acceptDownloads: true });
  try {
    const page = await context.newPage();
    const rpcFrames = [];
    page.on('websocket', socket => socket.on('framereceived', event => {
      if (typeof event.payload !== 'string') return;
      try { rpcFrames.push(JSON.parse(event.payload)); } catch { /* non-JSON frames are irrelevant */ }
    }));
    page.setDefaultTimeout(30_000);
    const response = await page.goto(origin.href, { waitUntil: 'domcontentloaded' });

    const expectedVersion = String(setup.wallet?.package_version ?? setup.wallet_package_version ?? '');
    assert.match(expectedVersion, /^\d+\.\d+\.\d+$/, 'setup manifest must pin the wallet package version');
    const versionNode = page.locator('.pf-sidebar').getByText(`v${expectedVersion}`, { exact: true });
    await versionNode.waitFor({ state: 'visible' });
    const visibleVersion = String(await versionNode.textContent() || '').trim();
    results.push(observedStep(1, {
      label: 'build identity',
      response_status: response?.status(),
      visible_version: visibleVersion,
      expected_version: expectedVersion,
      candidate_revision: CANDIDATE_REVISION,
      production_import_graph: productionGraph,
    }, {
      served: response?.status() === 200,
      version_visible: visibleVersion.length > 0 && visibleVersion.includes(expectedVersion),
      candidate_bound: (setup.candidate?.revision ?? setup.candidate_revision) === CANDIDATE_REVISION,
      production_graph: Object.values(productionGraph).every(Boolean),
    }));

    const created = await createBrowserControlledWallet(page, passphrase);
    seed = created.seed;
    results.push(observedStep(2, {
      label: 'browser-controlled connect/create',
      wallet_address: created.address,
      shell_visible: await page.locator('.pf-shell').isVisible(),
    }, {
      browser_address: /^pf[0-9a-f]{40}$/.test(created.address),
      rendered_wallet: await page.locator('.pf-shell').isVisible(),
    }));

    const market = await openPrimaryMarket(page);
    const marketText = (await market.textContent() || '').replace(/\s+/g, ' ');
    const reserveResponse = await waitForReserveProofFrame(rpcFrames);
    const packets = reserveResponse.result?.packets ?? reserveResponse.result?.result?.packets ?? [];
    const sourceIdentities = packets.flatMap(packet => packet.source_identities ?? packet.sources ?? []);
    const proofIdentities = packets.map(packet => packet.packet_hash ?? packet.reserve_packet_hash).filter(Boolean);
    results.push(observedStep(3, {
      label: 'six sources/proofs/NAV',
      source_identities: sourceIdentities,
      proof_identities: proofIdentities,
      rendered_nav_summary: marketText.match(/Verified NAV[^\n]*/)?.[0] ?? '',
      reserve_response_id: reserveResponse.id ?? null,
    }, {
      six_sources: sourceIdentities.length === 6 && new Set(sourceIdentities).size === 6,
      both_proofs: proofIdentities.length >= 2 && new Set(proofIdentities).size >= 2,
      nav_rendered: /Verified NAV/.test(marketText),
      proof_rendered: /matches finalized PFTL reserve proof/.test(marketText),
    }));

    const initialBalances = await marketBalances(market);
    results.push(observedStep(4, {
      label: 'A666+pfUSDC balances',
      balances: initialBalances,
    }, {
      a666_visible: Object.keys(initialBalances).some(key => /A666/i.test(key)),
      pfusdc_visible: Object.keys(initialBalances).some(key => /pfUSDC/i.test(key)),
      finalized_marker: /YOUR BALANCES\s*finalized/i.test(marketText),
    }));

    const transparent = await completeTransparentRoundTrip(page, market);
    results.push(observedStep(5, {
      label: 'transparent issue/redeem',
      before_balances: transparent.before,
      after_balances: transparent.after,
      finalized_progress: transparent.progress,
    }, {
      issue_and_redeem_receipts: transparent.progress.length >= 2,
      round_trip_conserved: JSON.stringify(transparent.before) === JSON.stringify(transparent.after),
    }));

    const privateRoundTrip = await completePrivateRoundTrip(page);
    const privateRecords = privateRoundTrip.value?.records ?? [];
    const privateReceipts = privateRecords
      .map(record => record.response?.receipt?.receipt_identity ?? record.response?.swap?.certificate_ref)
      .filter(Boolean);
    results.push(observedStep(6, {
      label: 'private issue/redeem',
      recovery_key: privateRoundTrip.key,
      operation_identities: privateRecords.map(record => record.idempotency_key),
      receipt_identities: privateReceipts,
      final_balance_tuples: privateRecords.map(record => record.response?.final_balance_tuple).filter(Boolean),
    }, {
      issue_and_redeem_committed: privateRecords.filter(record => record.status === 'COMMITTED').length >= 2,
      distinct_receipts: privateReceipts.length >= 2 && new Set(privateReceipts).size >= 2,
      durable_recovery: typeof privateRoundTrip.key === 'string',
    }));

    await openPrimaryMarket(page);
    await market.getByRole('button', { name: /Mint A666/ }).first().click();
    await market.getByRole('button', { name: 'Deliver to MetaMask', exact: true }).click();
    await market.locator('#navcoin-amount').fill('1');
    await market.getByRole('button', { name: /^Mint & export 1(?:\.0+)? A666$/ }).click();
    const exportArtifact = await pollContentAddressedSlot(exportSlotPath, 'export', asyncBudgetMs);
    results.push(observedStep(7, {
      label: 'export',
      artifact_sha256: exportArtifact.artifact_sha256,
      receipt_identity: exportArtifact.receipt_identity,
      observed_chain_state: exportArtifact.observed_chain_state,
    }, {
      content_addressed: await sha256File(resolve(exportArtifact.artifact_path)) === exportArtifact.artifact_sha256,
      receipt_observed: Boolean(exportArtifact.receipt_identity),
      finalized_state_observed: exportArtifact.observed_chain_state.finalized === true,
    }));

    await market.getByRole('button', { name: 'Redeem', exact: true }).first().click();
    await market.getByRole('button', { name: 'From MetaMask', exact: true }).click();
    await market.locator('#navcoin-amount').fill('1');
    await market.getByRole('button', { name: /^Return & redeem 1(?:\.0+)? A666$/ }).click();
    const returnArtifact = await pollContentAddressedSlot(returnSlotPath, 'return', asyncBudgetMs);
    results.push(observedStep(8, {
      label: 'return',
      artifact_sha256: returnArtifact.artifact_sha256,
      receipt_identity: returnArtifact.receipt_identity,
      observed_chain_state: returnArtifact.observed_chain_state,
      conservation: returnArtifact.conservation,
    }, {
      content_addressed: await sha256File(resolve(returnArtifact.artifact_path)) === returnArtifact.artifact_sha256,
      receipt_observed: Boolean(returnArtifact.receipt_identity),
      finalized_state_observed: returnArtifact.observed_chain_state.finalized === true,
      exact_conservation: returnArtifact.conservation?.equal === true,
    }));

    const balancesBeforeRestart = await marketBalances(market);
    const receiptIdsBeforeRestart = [...new Set([...privateReceipts, exportArtifact.receipt_identity, returnArtifact.receipt_identity])];
    const restart = await requestProxyRestart(proxyRestartPath, proxyPidPath);
    await page.reload({ waitUntil: 'domcontentloaded' });
    await unlock(page, passphrase);
    const recoveredMarket = await openPrimaryMarket(page);
    const balancesAfterRestart = await marketBalances(recoveredMarket);
    const recovered = await page.evaluate(() => Object.fromEntries(
      Object.keys(localStorage)
        .filter(key => key.startsWith('postfiat.navcoin_private_primary.') || key.includes('navcoin'))
        .map(key => [key, localStorage.getItem(key)]),
    ));
    const recoveredText = JSON.stringify(recovered);
    results.push(observedStep(9, {
      label: 'restart+reload recovery',
      proxy_pid_before: restart.before,
      proxy_pid_after: restart.after,
      balances_before: balancesBeforeRestart,
      balances_after: balancesAfterRestart,
      receipt_identities: receiptIdsBeforeRestart,
    }, {
      actual_proxy_restart: restart.before !== restart.after,
      exact_balances: JSON.stringify(balancesBeforeRestart) === JSON.stringify(balancesAfterRestart),
      receipt_identities_recovered: receiptIdsBeforeRestart.every(identity => recoveredText.includes(identity)),
    }));

    await page.locator('.pf-sidebar .pf-nav').filter({ hasText: 'Process' }).click();
    const downloadControl = page.getByRole('button', { name: 'Download public receipt', exact: true });
    await downloadControl.waitFor({ state: 'visible' });
    const event = page.waitForEvent('download');
    await downloadControl.click();
    const download = await event;
    const downloadPath = await download.path();
    assert.ok(downloadPath);
    const receiptText = await readFile(downloadPath, 'utf8');
    const publicReceipt = JSON.parse(receiptText);
    scanForbiddenFields(publicReceipt);
    assertCustodyValuesAbsent('downloaded public receipt', receiptText, [seed, passphrase]);
    results.push(observedStep(10, {
      label: 'public receipt download',
      suggested_filename: download.suggestedFilename(),
      schema: publicReceipt.schema,
      receipt_identity: publicReceipt.receipt_identity ?? publicReceipt.certificate_ref,
      final_balance_tuple: publicReceipt.final_balance_tuple,
    }, {
      real_download: (await stat(downloadPath)).size > 0,
      versioned_schema: /^postfiat[.-].*public[-_.]receipt.*v[0-9]+$/i.test(String(publicReceipt.schema || '')),
      receipt_bound: receiptIdsBeforeRestart.includes(publicReceipt.receipt_identity ?? publicReceipt.certificate_ref),
      final_balances_present: Array.isArray(publicReceipt.final_balance_tuple),
    }));

    const report = {
      schema: 'postfiat.a666.r4.offline-ethereum-browser-journey.v1',
      candidate_revision: CANDIDATE_REVISION,
      run_classification: runClassification,
      steps: results,
      all_steps_observed: results.length === JOURNEY_LABELS.length && results.every(result => result.ok),
      construction_execution: runClassification === 'construction'
        && results.length === JOURNEY_LABELS.length
        && results.every(result => result.ok),
    };
    scanForbiddenFields(report);
    assertCustodyValuesAbsent('journey report', report, [seed, passphrase]);
    await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
    assert.equal(report.all_steps_observed, true);
  } finally {
    await context.close();
  }
});
