import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

import { chromium } from 'playwright';

const walletUrl = process.env.WALLET_WEB_URL || 'http://127.0.0.1:8080';
const evidenceDir = path.resolve(process.env.PNOK_FIX_CAMPAIGN_EVIDENCE_DIR
  || '../deployments/pnok-private-fix-20260801/browser-qualification-10x');
const firstRunReport = path.resolve(process.env.PNOK_FIX_FIRST_RUN_REPORT
  || '../deployments/pnok-private-fix-20260801/browser-run-01/report.json');
const stateFile = path.join(evidenceDir, 'campaign-state.json');
const reportFile = path.join(evidenceDir, 'report.json');
const recoveryKey = 'postfiat.pnok_private_fix.active_job.v1';
const passphrase = 'pnok-private-fix-campaign-test-only';

fs.mkdirSync(evidenceDir, { recursive: true, mode: 0o700 });

function atomicJson(file, value) {
  const temporary = `${file}.${process.pid}.${crypto.randomBytes(6).toString('hex')}.tmp`;
  fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
  fs.renameSync(temporary, file);
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function accepted(status, direction) {
  return status?.ok === true
    && status?.schema === 'postfiat-pnok-private-fix-wallet-job-status-v1'
    && status?.direction === direction
    && status?.status === 'accepted'
    && status?.execution_stage === 'complete'
    && status?.execution_privacy === 'private on PFTL'
    && status?.source_boundary === 'controlled sandbox checkpoint'
    && status?.base_atoms === '20000000'
    && status?.quote_atoms === '210'
    && status?.fee_atoms === '0'
    && status?.price_impact_bps === 0
    && status?.supply_unchanged === true
    && JSON.stringify(status?.nullifier_occurrence_counts) === '[1,1]'
    && JSON.stringify(status?.output_occurrence_counts) === '[1,1]'
    && (direction !== 'acquire' || status?.replay_rejected_without_effect === true);
}

function requireAccepted(status, direction) {
  if (!accepted(status, direction)) {
    throw new Error(`${direction} failed acceptance checks: ${JSON.stringify(status)}`);
  }
}

function initialState() {
  const first = readJson(firstRunReport);
  requireAccepted(first.status, 'acquire');
  return {
    schema: 'postfiat-pnok-private-fix-browser-campaign-state-v1',
    wallet_url: walletUrl,
    target_runs: 10,
    runs: [{
      run_index: 1,
      acquire: first.status,
      recovery: first.recovery,
      source_report: firstRunReport,
    }],
    active: null,
    started_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  };
}

const campaign = fs.existsSync(stateFile) ? readJson(stateFile) : initialState();
atomicJson(stateFile, campaign);

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  ignoreHTTPSErrors: true,
  viewport: { width: 1440, height: 1100 },
});
const page = await context.newPage();
page.setDefaultTimeout(45_000);
const browserErrors = [];
page.on('pageerror', (error) => browserErrors.push(error.message));

function persist() {
  campaign.updated_at = new Date().toISOString();
  atomicJson(stateFile, campaign);
}

async function publicJob(jobId) {
  return page.evaluate(async (id) => {
    const response = await fetch(`/api/pnok-fix/jobs/${encodeURIComponent(id)}`, { cache: 'no-store' });
    return response.json();
  }, jobId);
}

async function waitForJob(jobId, direction, timeoutMs = 45 * 60 * 1000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const status = await publicJob(jobId);
    if (status?.status === 'accepted') {
      requireAccepted(status, direction);
      return status;
    }
    if (status?.status === 'failed') throw new Error(`${direction} job failed: ${JSON.stringify(status)}`);
    await page.waitForTimeout(2_000);
  }
  throw new Error(`${direction} job ${jobId} did not finish within the qualification timeout`);
}

async function createWallet() {
  await page.goto(walletUrl, { waitUntil: 'networkidle' });
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();
  await page.getByRole('checkbox', { name: /saved my seed/i }).check();
  await page.getByPlaceholder('Encryption passphrase (min 10 chars)').fill(passphrase);
  await page.getByPlaceholder('Confirm passphrase').fill(passphrase);
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();
  await page.locator('.pf-sidebar').getByRole('button', { name: /Private FX/ })
    .waitFor({ state: 'visible' });
}

async function readiness() {
  return page.evaluate(async () => (await fetch('/api/pnok-fix/readiness', { cache: 'no-store' })).json());
}

async function submitRestore(runIndex, ready) {
  return page.evaluate(async ({ index, expected }) => {
    const localSession = await (await fetch('/api/bridge/local-session', { cache: 'no-store' })).json();
    if (localSession?.ok !== true || typeof localSession.token !== 'string' || localSession.token.length < 32) {
      throw new Error('controlled localhost session is unavailable');
    }
    const response = await fetch('/api/pnok-fix/test-restore-jobs', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${localSession.token}`,
      },
      body: JSON.stringify({
        client_request_id: `pnok-browser-qualification-reset-${String(index).padStart(2, '0')}`,
        base_asset_id: expected.base_asset_id,
        quote_asset_id: expected.quote_asset_id,
        base_atoms: String(expected.base_atoms),
      }),
    });
    const payload = await response.json();
    if (!response.ok || payload?.ok !== true) throw new Error(payload?.message || `restore HTTP ${response.status}`);
    return payload;
  }, { index: runIndex, expected: ready });
}

async function showPrivateFix() {
  const sidebar = page.locator('.pf-sidebar');
  await sidebar.getByRole('button', { name: /Wallet/ }).click();
  await sidebar.getByRole('button', { name: /Private FX/ }).click();
  await page.getByRole('heading', { name: 'pfUSDC → pNOK', exact: true }).waitFor({ state: 'visible' });
}

async function qualifyAcquire(runIndex, active) {
  if (active.acquire_job_id) {
    await page.evaluate(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), {
      key: recoveryKey,
      value: {
        schema: 'postfiat-pnok-private-fix-browser-recovery-v1',
        client_request_id: active.acquire_client_request_id,
        base_asset_id: active.base_asset_id,
        quote_asset_id: active.quote_asset_id,
        base_atoms: '20000000',
        job_id: active.acquire_job_id,
      },
    });
  } else {
    await page.evaluate((key) => localStorage.removeItem(key), recoveryKey);
  }
  await showPrivateFix();

  if (!active.acquire_job_id) {
    await page.getByText('FIX VERIFIED', { exact: true }).waitFor({ state: 'visible' });
    await page.getByText('private on PFTL', { exact: true }).waitFor({ state: 'visible' });
    await page.getByText('controlled sandbox checkpoint', { exact: true }).waitFor({ state: 'visible' });
    await page.getByText('20.000000 pfUSDC', { exact: true }).waitFor({ state: 'visible' });
    await page.getByText('210 pNOK', { exact: true }).waitFor({ state: 'visible' });
    await page.screenshot({
      path: path.join(evidenceDir, `run-${String(runIndex).padStart(2, '0')}-01-fix-verified.png`),
      fullPage: true,
    });
    const button = page.getByRole('button', { name: 'Privately swap 20.000000 pfUSDC', exact: true });
    if (await button.isDisabled()) throw new Error(`run ${runIndex} acquisition button is disabled`);
    await button.click();
    await page.getByText(/^Durable job /).waitFor({ state: 'visible', timeout: 60_000 });
    const recovery = await page.evaluate((key) => JSON.parse(localStorage.getItem(key) || 'null'), recoveryKey);
    if (!/^0x[0-9a-f]{64}$/.test(String(recovery?.job_id || ''))) {
      throw new Error(`run ${runIndex} did not persist a durable acquisition job`);
    }
    Object.assign(active, {
      acquire_job_id: recovery.job_id,
      acquire_client_request_id: recovery.client_request_id,
      base_asset_id: recovery.base_asset_id,
      quote_asset_id: recovery.quote_asset_id,
    });
    persist();
  }

  await showPrivateFix();
  await page.reload({ waitUntil: 'networkidle' });
  await page.getByPlaceholder('Passphrase').fill(passphrase);
  await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  await page.locator('.pf-sidebar').getByRole('button', { name: /Private FX/ }).click();
  await page.getByText(/^Durable job /).waitFor({ state: 'visible' });

  const status = await waitForJob(active.acquire_job_id, 'acquire');
  await page.getByText('Private swap complete', { exact: true }).first()
    .waitFor({ state: 'visible', timeout: 60_000 });
  await page.screenshot({
    path: path.join(evidenceDir, `run-${String(runIndex).padStart(2, '0')}-02-complete.png`),
    fullPage: true,
  });
  return status;
}

try {
  await createWallet();
  for (let runIndex = 2; runIndex <= 10; runIndex += 1) {
    if (campaign.runs.some((run) => run.run_index === runIndex)) continue;
    if (!campaign.active || campaign.active.run_index !== runIndex) {
      campaign.active = { run_index: runIndex, started_at: new Date().toISOString() };
      persist();
    }
    const active = campaign.active;
    const ready = await readiness();
    if (!active.reset_job_id) {
      if (ready.restore_inventory_ready !== true) {
        throw new Error(`run ${runIndex} requires exact inverse inventory before its automated reset`);
      }
      const submitted = await submitRestore(runIndex, ready);
      active.reset_job_id = submitted.job_id;
      persist();
    }
    if (!active.reset) {
      active.reset = await waitForJob(active.reset_job_id, 'restore');
      persist();
    }
    const afterReset = await readiness();
    if (afterReset.acquire_inventory_ready !== true || afterReset.restore_inventory_ready !== false) {
      throw new Error(`run ${runIndex} automated reset did not restore exact acquisition inventory`);
    }
    active.acquire = await qualifyAcquire(runIndex, active);
    active.completed_at = new Date().toISOString();
    campaign.runs.push({ ...active });
    campaign.active = null;
    persist();
    process.stdout.write(`${JSON.stringify({ run_index: runIndex, ok: true, acquire_job_id: active.acquire_job_id })}\n`);
  }

  const consecutive = campaign.runs.length === 10
    && campaign.runs.every((run, index) => run.run_index === index + 1 && accepted(run.acquire, 'acquire'))
    && campaign.runs.slice(1).every((run) => accepted(run.reset, 'restore'));
  if (!consecutive || browserErrors.length) {
    throw new Error(`10-run campaign did not qualify: runs=${campaign.runs.length}, browser_errors=${browserErrors.length}`);
  }
  const finalReadiness = await readiness();
  if (finalReadiness.acquire_inventory_ready !== false || finalReadiness.restore_inventory_ready !== true) {
    throw new Error('final qualification inventory does not match the tenth acquisition output');
  }
  const report = {
    ok: true,
    schema: 'postfiat-pnok-private-fix-browser-qualification-v1',
    wallet_url: walletUrl,
    target_runs: 10,
    completed_runs: 10,
    consecutive_without_manual_state_repair: true,
    each_acquisition_browser_initiated: true,
    automated_inverse_private_swaps: 9,
    refresh_recovery_exercised_each_run: true,
    runs: campaign.runs,
    final_readiness: finalReadiness,
    browser_errors: browserErrors,
    started_at: campaign.started_at,
    completed_at: new Date().toISOString(),
  };
  atomicJson(reportFile, report);
  process.stdout.write(`${JSON.stringify({
    ok: true,
    schema: report.schema,
    completed_runs: report.completed_runs,
    automated_inverse_private_swaps: report.automated_inverse_private_swaps,
    browser_errors: report.browser_errors,
    report_file: reportFile,
  }, null, 2)}\n`);
} finally {
  await browser.close();
}
