import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

import { chromium } from 'playwright';

const walletUrl = process.env.WALLET_WEB_URL || 'http://127.0.0.1:8080';
const repo = path.resolve('..');
const evidenceDir = path.resolve(process.env.PNOK_FIX_FAULT_EVIDENCE_DIR
  || '../deployments/pnok-private-fix-20260801/recovery-faults');
const recoveryKey = 'postfiat.pnok_private_fix.active_job.v1';
const passphrase = crypto.randomBytes(24).toString('base64url');
fs.mkdirSync(evidenceDir, { recursive: true, mode: 0o700 });

function atomicJson(file, value) {
  const temporary = `${file}.${process.pid}.${crypto.randomBytes(6).toString('hex')}.tmp`;
  fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
  fs.renameSync(temporary, file);
}

function restart(service) {
  execFileSync('systemctl', ['--user', 'restart', service], { stdio: 'ignore' });
}

function restartRemoteValidatorFive() {
  execFileSync('ssh', [
    '-o', 'BatchMode=yes',
    '-o', 'StrictHostKeyChecking=yes',
    'root@45.32.110.170',
    'systemctl', 'restart', 'postfiat-validator-5.service', 'postfiat-validator-5-rpc.service',
  ], { stdio: 'ignore' });
}

async function sleep(ms) {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

async function jsonFetch(url, options = {}) {
  const response = await fetch(url, { cache: 'no-store', ...options });
  const payload = await response.json();
  if (!response.ok || payload?.ok !== true) throw new Error(payload?.message || `HTTP ${response.status}`);
  return payload;
}

async function readinessFetch(url) {
  const response = await fetch(url, { cache: 'no-store' });
  const payload = await response.json();
  // The readiness endpoint deliberately returns 503 when only one trade
  // direction has inventory. That is a valid state during this campaign.
  if (payload?.ok !== true) throw new Error(payload?.message || `HTTP ${response.status}`);
  return payload;
}

async function eventually(read, predicate, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    try {
      last = await read();
      if (predicate(last)) return last;
    } catch (_) { /* deliberate service outages are retried */ }
    await sleep(1_000);
  }
  throw new Error(`${label} did not recover before timeout: ${JSON.stringify(last)}`);
}

function accepted(status, direction) {
  return status?.ok === true
    && status?.direction === direction
    && status?.status === 'accepted'
    && status?.execution_stage === 'complete'
    && status?.supply_unchanged === true
    && JSON.stringify(status?.nullifier_occurrence_counts) === '[1,1]'
    && JSON.stringify(status?.output_occurrence_counts) === '[1,1]'
    && (direction !== 'acquire' || status?.replay_rejected_without_effect === true);
}

async function waitJob(jobId, direction) {
  return eventually(
    () => jsonFetch(`${walletUrl}/api/pnok-fix/jobs/${encodeURIComponent(jobId)}`),
    (status) => {
      if (status?.status === 'failed') throw new Error(`${direction} failed: ${JSON.stringify(status)}`);
      return accepted(status, direction);
    },
    45 * 60 * 1000,
    `${direction} job`,
  );
}

// Epoch 3 is intentionally exhausted by the 10-run qualification. Epoch 4
// provides exactly one inverse reset plus one acquisition for fault recovery.
const epochFourDir = path.join(repo, 'deployments/pnok-private-fix-20260801/repeat-fix-epoch-4');
const epochFourStatusFile = path.join(epochFourDir, 'public/status.json');
const epochFourComplete = fs.existsSync(epochFourStatusFile)
  && JSON.parse(fs.readFileSync(epochFourStatusFile, 'utf8')).stage === 'complete';
if (!epochFourComplete) {
  execFileSync('python3', [
    path.join(repo, 'scripts/pnok-fix-successor.py'),
    '--output-dir', epochFourDir,
    '--max-fills', '2',
    '--validity-blocks', '2000',
  ], { cwd: repo, stdio: ['ignore', 'pipe', 'pipe'] });
}

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 1100 } });
const page = await context.newPage();
page.setDefaultTimeout(45_000);
const browserErrors = [];
page.on('pageerror', (error) => browserErrors.push(error.message));

try {
  await page.goto(walletUrl, { waitUntil: 'networkidle' });
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();
  await page.getByRole('checkbox', { name: /saved my seed/i }).check();
  await page.getByPlaceholder('Encryption passphrase (min 10 chars)').fill(passphrase);
  await page.getByPlaceholder('Confirm passphrase').fill(passphrase);
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();
  await page.locator('.pf-sidebar').getByRole('button', { name: /Private FX/ })
    .waitFor({ state: 'visible' });

  const ready = await readinessFetch(`${walletUrl}/api/pnok-fix/readiness`);
  if (ready.restore_inventory_ready !== true || ready.acquire_inventory_ready !== false) {
    throw new Error('fault campaign requires the tenth acquisition output inventory');
  }
  const localSession = await page.evaluate(async () => {
    const response = await fetch('/api/bridge/local-session', { cache: 'no-store' });
    return response.json();
  });
  if (typeof localSession.token !== 'string' || localSession.token.length < 32) {
    throw new Error('controlled localhost session token is unavailable');
  }
  const resetBody = {
    client_request_id: 'pnok-browser-fault-reset-01',
    base_asset_id: ready.base_asset_id,
    quote_asset_id: ready.quote_asset_id,
    base_atoms: String(ready.base_atoms),
  };
  const submitReset = () => page.evaluate(async ({ token, body }) => {
    const response = await fetch('/api/pnok-fix/test-restore-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify(body),
    });
    const payload = await response.json();
    if (!response.ok || payload?.ok !== true) {
      throw new Error(payload?.message || `HTTP ${response.status}`);
    }
    return payload;
  }, { token: localSession.token, body: resetBody });
  const resetFirst = await submitReset();
  const resetDuplicate = await submitReset();
  if (resetDuplicate.job_id !== resetFirst.job_id || resetDuplicate.idempotent_replay !== true) {
    throw new Error('duplicate durable reset request was not idempotent');
  }
  await eventually(
    () => jsonFetch(`${walletUrl}/api/pnok-fix/jobs/${encodeURIComponent(resetFirst.job_id)}`),
    (status) => ['action_built', 'reservation_finalized', 'batch_built'].includes(status.execution_stage),
    10 * 60 * 1000,
    'reset action build',
  );
  restartRemoteValidatorFive();
  const reset = await waitJob(resetFirst.job_id, 'restore');

  // Restart the resident proof service itself, wait for both pinned circuits
  // to be fully warm, then prove the browser acquisition without intervention.
  restart('postfiat-pftl-pnok-prover.service');
  await eventually(
    () => jsonFetch('http://127.0.0.1:18799/asset-orchard/readiness'),
    (status) => status.ready === true
      && status.prover_warm?.ready === true
      && status.prover_warm?.circuits?.swap?.ready === true
      && status.prover_warm?.circuits?.private_egress?.ready === true,
    15 * 60 * 1000,
    'resident prover restart and prewarm',
  );

  await page.reload({ waitUntil: 'networkidle' });
  await page.getByPlaceholder('Passphrase').fill(passphrase);
  await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  const sidebar = page.locator('.pf-sidebar');
  await sidebar.getByRole('button', { name: /Private FX/ }).click();
  await page.getByText('FIX VERIFIED', { exact: true }).waitFor({ state: 'visible' });
  await page.screenshot({ path: path.join(evidenceDir, '01-after-prover-restart.png'), fullPage: true });
  await page.getByRole('button', { name: 'Privately swap 20.000000 pfUSDC', exact: true }).click();
  await page.getByText(/^Durable job /).waitFor({ state: 'visible', timeout: 60_000 });
  const recovery = await page.evaluate((key) => JSON.parse(localStorage.getItem(key) || 'null'), recoveryKey);
  if (!/^0x[0-9a-f]{64}$/.test(String(recovery?.job_id || ''))) {
    throw new Error('fault acquisition did not persist a durable browser job');
  }
  await eventually(
    () => jsonFetch(`${walletUrl}/api/pnok-fix/jobs/${encodeURIComponent(recovery.job_id)}`),
    (status) => ['action_built', 'reservation_finalized', 'batch_built'].includes(status.execution_stage),
    10 * 60 * 1000,
    'acquisition action build',
  );
  restart('pft-wallet-proxy-8080.service');
  await eventually(
    () => readinessFetch(`${walletUrl}/api/pnok-fix/readiness`),
    (status) => status.resident_prover_ready === true,
    2 * 60 * 1000,
    'second wallet proxy restart',
  );

  await page.reload({ waitUntil: 'networkidle' });
  await page.getByPlaceholder('Passphrase').fill(passphrase);
  await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  await page.locator('.pf-sidebar').getByRole('button', { name: /Private FX/ }).click();
  await page.getByText(/^Durable job /).waitFor({ state: 'visible' });
  const acquire = await waitJob(recovery.job_id, 'acquire');
  if (acquire.retry_count < 2) throw new Error('acquisition worker did not exercise durable retry after proxy restart');
  await page.getByText('Private swap complete', { exact: true }).first()
    .waitFor({ state: 'visible', timeout: 60_000 });
  await page.screenshot({ path: path.join(evidenceDir, '02-recovered-private-swap.png'), fullPage: true });

  const report = {
    ok: true,
    schema: 'postfiat-pnok-private-fix-recovery-faults-v1',
    controlled_demo_only: true,
    duplicate_submit_idempotent: true,
    validator_restart_during_reset_recovered: true,
    resident_prover_restart_and_full_prewarm_recovered: true,
    wallet_proxy_restart_during_browser_acquisition_recovered: true,
    reset,
    acquire,
    browser_errors: browserErrors,
    completed_at: new Date().toISOString(),
  };
  if (browserErrors.length) throw new Error(`browser errors: ${browserErrors.join(' | ')}`);
  atomicJson(path.join(evidenceDir, 'report.json'), report);
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
} finally {
  await browser.close();
}
