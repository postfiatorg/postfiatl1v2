import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

import { chromium } from 'playwright';

const walletUrl = process.env.WALLET_WEB_URL || 'http://127.0.0.1:8080';
const evidenceDir = path.resolve(process.env.PNOK_FIX_UX_EVIDENCE_DIR
  || '../deployments/pnok-private-fix-20260801/browser-run-01');
const passphrase = crypto.randomBytes(24).toString('base64url');
const recoveryKey = 'postfiat.pnok_private_fix.active_job.v1';

fs.mkdirSync(evidenceDir, { recursive: true, mode: 0o700 });

function atomicJson(file, value) {
  const temporary = `${file}.${process.pid}.${crypto.randomBytes(6).toString('hex')}.tmp`;
  fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
  fs.renameSync(temporary, file);
}

function assertAccepted(status) {
  if (status?.ok !== true
    || status?.schema !== 'postfiat-pnok-private-fix-wallet-job-status-v1'
    || status?.direction !== 'acquire'
    || status?.status !== 'accepted'
    || status?.execution_stage !== 'complete'
    || status?.execution_privacy !== 'private on PFTL'
    || status?.source_boundary !== 'controlled sandbox checkpoint'
    || status?.base_atoms !== '20000000'
    || status?.quote_atoms !== '210'
    || status?.fee_atoms !== '0'
    || status?.price_impact_bps !== 0
    || status?.supply_unchanged !== true
    || status?.replay_rejected_without_effect !== true
    || JSON.stringify(status?.nullifier_occurrence_counts) !== '[1,1]'
    || JSON.stringify(status?.output_occurrence_counts) !== '[1,1]') {
    throw new Error(`browser FIX result failed acceptance checks: ${JSON.stringify(status)}`);
  }
}

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  ignoreHTTPSErrors: true,
  viewport: { width: 1440, height: 1100 },
});
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

  const sidebar = page.locator('.pf-sidebar');
  await sidebar.getByRole('button', { name: /Private FX/ }).waitFor({ state: 'visible' });
  await sidebar.getByRole('button', { name: /Private FX/ }).click();

  await page.getByRole('heading', { name: 'pfUSDC → pNOK', exact: true }).waitFor({ state: 'visible' });
  await page.getByText('FIX VERIFIED', { exact: true }).waitFor({ state: 'visible' });
  await page.getByText('private on PFTL', { exact: true }).waitFor({ state: 'visible' });
  await page.getByText('controlled sandbox checkpoint', { exact: true }).waitFor({ state: 'visible' });
  await page.getByText('20.000000 pfUSDC', { exact: true }).waitFor({ state: 'visible' });
  await page.getByText('210 pNOK', { exact: true }).waitFor({ state: 'visible' });
  await page.getByText('10.500000 pNOK/pfUSDC', { exact: true }).waitFor({ state: 'visible' });
  await page.getByText('Controlled demo only.', { exact: false }).waitFor({ state: 'visible' });
  await page.screenshot({ path: path.join(evidenceDir, '01-fix-verified.png'), fullPage: true });

  const execute = page.getByRole('button', { name: 'Privately swap 20.000000 pfUSDC', exact: true });
  await execute.waitFor({ state: 'visible' });
  if (await execute.isDisabled()) throw new Error('browser private FIX action is disabled');
  await execute.click();

  await page.getByText(/^Durable job /).waitFor({ state: 'visible', timeout: 60_000 });
  const recoveryBefore = await page.evaluate((key) => JSON.parse(localStorage.getItem(key) || 'null'), recoveryKey);
  if (!/^0x[0-9a-f]{64}$/.test(String(recoveryBefore?.job_id || ''))) {
    throw new Error('browser did not durably persist the private FIX job ID');
  }

  await sidebar.getByRole('button', { name: /Wallet/ }).click();
  await page.getByText('Total balance', { exact: true }).waitFor({ state: 'visible' });
  await sidebar.getByRole('button', { name: /Private FX/ }).click();
  await page.getByText(/^Durable job /).waitFor({ state: 'visible' });

  await page.reload({ waitUntil: 'networkidle' });
  await page.getByPlaceholder('Passphrase').fill(passphrase);
  await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  await page.locator('.pf-sidebar').getByRole('button', { name: /Private FX/ }).click();
  await page.getByText(/^Durable job /).waitFor({ state: 'visible' });
  await page.screenshot({ path: path.join(evidenceDir, '02-recovered-after-reload.png'), fullPage: true });

  await page.getByText('Private swap complete', { exact: true }).first()
    .waitFor({ state: 'visible', timeout: 45 * 60 * 1000 });

  const recoveryAfter = await page.evaluate((key) => JSON.parse(localStorage.getItem(key) || 'null'), recoveryKey);
  if (recoveryAfter?.job_id !== recoveryBefore.job_id) {
    throw new Error('browser recovery changed the durable private FIX job ID');
  }
  const status = await page.evaluate(async (jobId) => {
    const response = await fetch(`/api/pnok-fix/jobs/${encodeURIComponent(jobId)}`, { cache: 'no-store' });
    return response.json();
  }, recoveryAfter.job_id);
  assertAccepted(status);
  if (browserErrors.length) throw new Error(`browser errors: ${browserErrors.join(' | ')}`);

  await page.screenshot({ path: path.join(evidenceDir, '03-private-swap-complete.png'), fullPage: true });
  const report = {
    ok: true,
    schema: 'postfiat-pnok-private-fix-browser-ux-evidence-v1',
    wallet_url: walletUrl,
    recovery: {
      navigated_away_and_back: true,
      reloaded_and_unlocked: true,
      same_durable_job_id: true,
    },
    status,
    browser_errors: browserErrors,
    completed_at: new Date().toISOString(),
  };
  atomicJson(path.join(evidenceDir, 'report.json'), report);
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
} finally {
  await browser.close();
}
