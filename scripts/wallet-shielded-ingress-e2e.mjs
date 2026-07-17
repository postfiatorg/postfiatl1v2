#!/usr/bin/env node
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { homedir } from 'node:os';
import process from 'node:process';
import WebSocket from '../wallet-proxy/node_modules/ws/index.js';

import { RpcClient } from '../wallet-web/src/lib/rpc-client.js';
import { A651_ASSET_ID } from '../wallet-web/src/lib/utils.js';

globalThis.WebSocket = WebSocket;

const require = createRequire(new URL('../wallet-web/package.json', import.meta.url));
const { chromium } = require('playwright');

const APP_URL = process.env.ORCHARD_INGRESS_E2E_URL || 'http://127.0.0.1:5173/';
const RPC_URL = process.env.ORCHARD_INGRESS_E2E_RPC || 'ws://127.0.0.1:8080/rpc';
const RUNS = Number.parseInt(process.env.ORCHARD_INGRESS_E2E_RUNS || '2', 10);
const INGRESS_AMOUNT = process.env.ORCHARD_INGRESS_E2E_AMOUNT || '0.002';
const A651_FUND_AMOUNT = process.env.ORCHARD_INGRESS_E2E_A651_FUND_AMOUNT || '0.02';
const PFT_FUND_AMOUNT_ATOMS = process.env.ORCHARD_INGRESS_E2E_PFT_ATOMS || '100000';
const OUT_DIR = process.env.ORCHARD_INGRESS_E2E_OUT_DIR
  || `docs/evidence/wallet-private-swap-step5-live-ingress-${new Date().toISOString().replace(/[:.]/g, '')}`;
const SENSITIVE_DIR = process.env.ORCHARD_INGRESS_E2E_SENSITIVE_DIR
  || `/tmp/postfiat-orchard-ingress-sensitive-${Date.now()}`;
const LOCAL_VAULT_DIR = process.env.ASSET_ORCHARD_LOCAL_VAULT_DIR
  || join(process.env.XDG_DATA_HOME || join(homedir(), '.local/share'), 'postfiat/asset-orchard-local-vault');
const SYNC_RELAY_STATE = !['0', 'false', 'no'].includes(String(process.env.ORCHARD_INGRESS_E2E_SYNC_RELAY_STATE || 'true').toLowerCase());
const SYNC_RELAY_STATE_CMD = process.env.ORCHARD_INGRESS_E2E_SYNC_RELAY_STATE_CMD
  || 'scripts/wallet-shielded-ingress-sync-state';
const PFT_FUNDER = process.env.ORCHARD_INGRESS_E2E_FUNDER
  || 'pff3e396f771a8f490ca330e1720472d473bcfcb6d';
const PFT_FUNDER_KEY_FILE = process.env.ORCHARD_INGRESS_E2E_FUNDER_KEY_FILE
  || '/run/secrets/navswap-issuer.key.json';

mkdirSync(OUT_DIR, { recursive: true });
mkdirSync(SENSITIVE_DIR, { recursive: true, mode: 0o700 });

function assertOk(condition, message) {
  if (!condition) throw new Error(message);
}

function writeJson(file, value) {
  writeFileSync(file, `${JSON.stringify(value, (_key, val) => (
    typeof val === 'bigint' ? val.toString() : val
  ), 2)}\n`);
}

function syncRelayState(label) {
  if (!SYNC_RELAY_STATE) {
    return { skipped: true, reason: 'ORCHARD_INGRESS_E2E_SYNC_RELAY_STATE disabled' };
  }
  const reportFile = join(OUT_DIR, `${label}-relay-state-sync.json`);
  const proc = spawnSync(SYNC_RELAY_STATE_CMD, [], {
    cwd: process.cwd(),
    env: {
      ...process.env,
      REPORT_FILE: reportFile,
    },
    encoding: 'utf8',
    maxBuffer: 8 * 1024 * 1024,
  });
  if (proc.error) throw proc.error;
  assertOk(proc.status === 0, `relay state sync failed (${proc.status}): ${proc.stderr || proc.stdout}`);
  const report = JSON.parse(readFileSync(reportFile, 'utf8'));
  return {
    skipped: false,
    report_file: reportFile,
    local_before_height: report.local_before?.block_height || null,
    local_after_height: report.local_after?.block_height || null,
    majority_height: report.fleet?.majority?.height || null,
    majority_count: report.fleet?.majority?.count || null,
    source_validator: report.source_validator || null,
  };
}

function parseAmountAtoms(value, precision = 6) {
  const text = String(value || '').trim();
  assertOk(/^[0-9]+(?:\.[0-9]+)?$/.test(text), `invalid decimal amount ${value}`);
  const [whole, frac = ''] = text.split('.');
  assertOk(frac.length <= precision, `amount ${value} exceeds ${precision} decimals`);
  return BigInt(whole) * (10n ** BigInt(precision)) + BigInt(frac.padEnd(precision, '0') || '0');
}

function assetItems(result) {
  if (Array.isArray(result)) return result;
  if (Array.isArray(result?.assets)) return result.assets;
  return [];
}

function canonicalBalanceAtoms(result, assetId) {
  let total = 0n;
  for (const item of assetItems(result)) {
    const id = item.asset_id || item.id;
    if (id === assetId) total += BigInt(item.balance ?? item.amount ?? 0);
  }
  return total;
}

async function waitForAssetBalance(rpc, account, assetId, predicate, label, timeoutMs = 180_000) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() <= deadline) {
    const resp = await rpc.accountAssets(account);
    last = resp;
    const balance = canonicalBalanceAtoms(resp.result, assetId);
    if (resp.ok && predicate(balance)) return { resp, balance };
    await new Promise(resolve => setTimeout(resolve, 1500));
  }
  throw new Error(`asset ${assetId} balance wait failed for ${account}: ${label}; last=${JSON.stringify(last)}`);
}

async function waitForNativeBalance(rpc, account, minimum, timeoutMs = 120_000) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() <= deadline) {
    const resp = await rpc.account(account);
    last = resp;
    const balance = BigInt(resp.result?.balance || 0);
    if (resp.ok && balance >= BigInt(minimum)) return { resp, balance };
    await new Promise(resolve => setTimeout(resolve, 1500));
  }
  throw new Error(`PFT balance for ${account} did not reach ${minimum}; last=${JSON.stringify(last)}`);
}

function signWithProxy(command, quote) {
  const signer = [
    'tmp=$(mktemp)',
    'trap "rm -f $tmp" EXIT',
    'cat > "$tmp"',
    `/usr/local/bin/postfiat-node ${command} --key-file "${PFT_FUNDER_KEY_FILE}" --quote-file "$tmp"`,
  ].join('; ');
  const proc = spawnSync('docker', [
    'compose',
    '-f',
    'docker-compose.wallet.yml',
    'exec',
    '-T',
    'wallet-proxy',
    'sh',
    '-lc',
    signer,
  ], {
    cwd: process.cwd(),
    input: JSON.stringify(quote),
    encoding: 'utf8',
    maxBuffer: 4 * 1024 * 1024,
  });
  if (proc.error) throw proc.error;
  assertOk(proc.status === 0, `wallet-proxy signer failed (${proc.status}): ${proc.stderr || proc.stdout}`);
  return JSON.parse(proc.stdout);
}

async function fundPft(rpc, recipient) {
  const amount = Number(PFT_FUND_AMOUNT_ATOMS);
  assertOk(Number.isSafeInteger(amount) && amount > 0, 'PFT funding amount must be a safe positive integer');
  const quoteResp = await rpc.transferFeeQuote(PFT_FUNDER, recipient, amount);
  assertOk(quoteResp.ok, `PFT funding transfer_fee_quote failed: ${quoteResp.error?.message || 'unknown'}`);
  const signed = signWithProxy('wallet-sign-transfer', quoteResp.result);
  const submitResp = await rpc.submitSignedTransferFinality(JSON.stringify(signed));
  assertOk(submitResp.ok, `PFT funding submit failed: ${submitResp.error?.message || 'unknown'}`);
  const after = await waitForNativeBalance(rpc, recipient, PFT_FUND_AMOUNT_ATOMS);
  return {
    amount_atoms: PFT_FUND_AMOUNT_ATOMS,
    tx_id: submitResp.result?.tx_id || null,
    finality_height: submitResp.result?.finality?.block?.header?.height || null,
    balance_atoms: after.balance.toString(),
  };
}

async function fundA651(rpc, recipient, amountAtoms) {
  assertOk(amountAtoms > 0n && amountAtoms <= BigInt(Number.MAX_SAFE_INTEGER), 'a651 funding amount must be a safe positive atom count');
  const before = await rpc.accountAssets(recipient);
  const beforeBalance = canonicalBalanceAtoms(before.result, A651_ASSET_ID);
  const operation = {
    operation: 'issued_payment',
    from: PFT_FUNDER,
    to: recipient,
    issuer: PFT_FUNDER,
    asset_id: A651_ASSET_ID,
    amount: Number(amountAtoms),
  };
  const quoteResp = await rpc.assetFeeQuote(PFT_FUNDER, JSON.stringify(operation));
  assertOk(quoteResp.ok, `a651 funding asset_fee_quote failed: ${quoteResp.error?.message || 'unknown'}`);
  const signed = signWithProxy('wallet-sign-asset-transaction', quoteResp.result);
  const submitResp = await rpc.submitSignedAssetTransactionFinality(JSON.stringify(signed));
  assertOk(submitResp.ok, `a651 funding submit failed: ${submitResp.error?.message || 'unknown'}`);
  const after = await waitForAssetBalance(
    rpc,
    recipient,
    A651_ASSET_ID,
    balance => balance >= beforeBalance + amountAtoms,
    'funded a651 visible',
  );
  return {
    amount_atoms: amountAtoms.toString(),
    tx_id: submitResp.result?.tx_id || null,
    finality_height: submitResp.result?.finality?.block?.header?.height || null,
    before_balance_atoms: beforeBalance.toString(),
    balance_atoms: after.balance.toString(),
  };
}

async function installCapture(page) {
  await page.addInitScript(() => {
    window.__orchardIngressE2e = { responses: [] };
    const originalFetch = window.fetch.bind(window);
    window.fetch = async (...args) => {
      const response = await originalFetch(...args);
      const url = typeof args[0] === 'string' ? args[0] : args[0]?.url || '';
      if (url.includes('/api/shielded-nav-swap/') || url.includes('/api/navswap/') || url.includes('/asset-orchard/')) {
        response.clone().json().then((body) => {
          const redacted = structuredClone(body);
          if (url.includes('/asset-orchard/')) {
            if (redacted.wallet_note?.output_commitment) {
              redacted.wallet_note = { output_commitment: redacted.wallet_note.output_commitment };
            } else {
              delete redacted.wallet_note;
            }
          }
          window.__orchardIngressE2e.responses.push({
            at: new Date().toISOString(),
            url,
            status: response.status,
            ok: response.ok,
            body: redacted,
          });
        }).catch((error) => {
          window.__orchardIngressE2e.responses.push({
            at: new Date().toISOString(),
            url,
            status: response.status,
            ok: response.ok,
            error: String(error),
          });
        });
      }
      return response;
    };
  });
}

async function createWallet(page) {
  const passphrase = `orchard-ingress-e2e-${Date.now()}`;
  await page.goto(APP_URL, { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 30_000 }).catch(() => {});
  await page.screenshot({ path: join(OUT_DIR, '00-onboard.png'), fullPage: true });
  await page.getByRole('button', { name: /^Create Wallet$/ }).first().click({ timeout: 30_000 });
  await page.waitForSelector('.pf-seed-display', { timeout: 30_000 });
  const seed = (await page.locator('.pf-seed-display').innerText()).trim();
  assertOk(/^[0-9a-f]{64}$/i.test(seed), 'generated wallet seed was not displayed');
  const accountAddress = (await page.locator('text=/pf[a-f0-9]{40}/i').first().innerText()).trim();
  writeJson(join(SENSITIVE_DIR, 'wallet-sensitive.json'), {
    warning: 'Sensitive devnet wallet seed. Do not commit or publish.',
    accountAddress,
    seed,
    passphrase,
  });
  await page.locator('input[type="checkbox"]').first().check();
  const passphraseInputs = page.locator('input[type="password"]');
  await passphraseInputs.nth(0).fill(passphrase);
  await passphraseInputs.nth(1).fill(passphrase);
  await page.getByRole('button', { name: /^Create Wallet$/ }).last().click({ timeout: 30_000 });
  await page.waitForTimeout(1500);
  await page.screenshot({ path: join(OUT_DIR, '01-wallet-created.png'), fullPage: true });
  return { accountAddress };
}

async function clickEnabledButton(page, pattern, timeoutMs = 180_000) {
  const deadline = Date.now() + timeoutMs;
  let lastText = '';
  while (Date.now() <= deadline) {
    lastText = await page.locator('body').innerText().catch(() => '');
    const button = page.locator('button').filter({ hasText: pattern }).first();
    const visible = await button.isVisible({ timeout: 1000 }).catch(() => false);
    if (visible) {
      const disabled = await button.evaluate(node => Boolean(node.disabled)).catch(() => true);
      if (!disabled) {
        await button.click({ timeout: 30_000 });
        return true;
      }
    }
    await page.waitForTimeout(1000);
  }
  throw new Error(`timed out waiting for enabled button ${pattern}; last body:\n${lastText}`);
}

async function selectShieldedRoute(page) {
  await page.locator('button').filter({ hasText: 'Swap' }).first().click({ timeout: 15_000 });
  await page.waitForSelector('text=/Move between assets/i', { timeout: 45_000 });
  if (/Public ingress to private note/i.test(await page.locator('body').innerText())) {
    return;
  }
  await clickEnabledButton(page, /Shielded NAVSwap/i, 120_000);
  await page.waitForSelector('text=/Public ingress to private note/i', { timeout: 45_000 });
}

function latestCaptured(responses, needle) {
  return [...responses].reverse().find(entry => String(entry.url || '').includes(needle))?.body || null;
}

function receiptSummary(ingress) {
  const receipts = Array.isArray(ingress?.receipts) ? ingress.receipts : [];
  return receipts.map(receipt => ({
    accepted: receipt.accepted,
    code: receipt.code || null,
    message: receipt.message || null,
    tx_id: receipt.tx_id || receipt.transaction_id || receipt.transaction_hash || null,
    height: receipt.height || receipt.block_height || receipt.finality_height || null,
  }));
}

function vaultFileInfo(outputCommitment) {
  const path = join(LOCAL_VAULT_DIR, `${outputCommitment}.json`);
  const stat = statSync(path);
  return {
    path,
    exists: true,
    size_bytes: stat.size,
    mode_octal: (stat.mode & 0o777).toString(8).padStart(3, '0'),
  };
}

async function runIngress(page, rpc, wallet, runIndex, amountAtoms) {
  await selectShieldedRoute(page);
  const input = page.locator('input[aria-label^="Amount of"]').first();
  await input.waitFor({ timeout: 45_000 });
  await input.fill(INGRESS_AMOUNT);
  await page.screenshot({ path: join(OUT_DIR, `run-${runIndex}-before-submit.png`), fullPage: true });

  const beforeAssetsResp = await rpc.accountAssets(wallet.accountAddress);
  const beforeAccountResp = await rpc.account(wallet.accountAddress);
  const beforeA651 = canonicalBalanceAtoms(beforeAssetsResp.result, A651_ASSET_ID);
  assertOk(beforeA651 >= amountAtoms, `run ${runIndex} requires ${amountAtoms} a651 atoms, wallet has ${beforeA651}`);
  const capturedStart = await page.evaluate(() => window.__orchardIngressE2e?.responses?.length || 0);
  const vaultBefore = new Set(readdirSync(LOCAL_VAULT_DIR).filter(name => name.endsWith('.json')));
  const relayStateSync = syncRelayState(`run-${runIndex}`);

  await clickEnabledButton(page, /Create private note/i, 300_000);
  await page.waitForFunction(
    () => /Ingress certified|Ingress blocked/i.test(document.body.innerText || ''),
    null,
    { timeout: 600_000 },
  );
  await page.waitForTimeout(1500);
  const finalText = await page.locator('body').innerText();
  await page.screenshot({ path: join(OUT_DIR, `run-${runIndex}-after-submit.png`), fullPage: true });
  assertOk(!/Ingress blocked/i.test(finalText), `ingress run ${runIndex} failed in UX:\n${finalText}`);

  const captured = await page.evaluate((start) => (
    window.__orchardIngressE2e?.responses || []
  ).slice(start), capturedStart);
  const preflight = latestCaptured(captured, '/api/shielded-nav-swap/preflight');
  const localNote = latestCaptured(captured, '/asset-orchard/ingress-notes');
  const ingress = latestCaptured(captured, '/api/shielded-nav-swap/ingress');
  assertOk(preflight?.ok === true, `run ${runIndex} missing successful preflight`);
  assertOk(localNote?.ok === true, `run ${runIndex} missing successful local note build`);
  assertOk(ingress?.ok === true, `run ${runIndex} missing successful ingress relay`);
  const outputCommitment = ingress.output_commitment || localNote.wallet_note?.output_commitment;
  assertOk(/^[0-9a-f]{64}$/.test(String(outputCommitment || '')), `run ${runIndex} missing output commitment`);
  const vaultInfo = vaultFileInfo(outputCommitment);
  assertOk(vaultInfo.mode_octal === '600', `vault file mode must be 600, got ${vaultInfo.mode_octal}`);

  const after = await waitForAssetBalance(
    rpc,
    wallet.accountAddress,
    A651_ASSET_ID,
    balance => balance <= beforeA651 - amountAtoms,
    'a651 burn reflected after ingress',
  );
  const afterAccountResp = await rpc.account(wallet.accountAddress);
  const vaultAfter = new Set(readdirSync(LOCAL_VAULT_DIR).filter(name => name.endsWith('.json')));
  const newVaultFiles = [...vaultAfter].filter(name => !vaultBefore.has(name));

  const runEvidence = {
    schema: 'postfiat-wallet-private-swap-step5-live-ingress-run-v1',
    run_index: runIndex,
    captured_at: new Date().toISOString(),
    wallet_address: wallet.accountAddress,
    asset: {
      symbol: 'a651',
      asset_id: A651_ASSET_ID,
      amount_atoms: amountAtoms.toString(),
    },
    before: {
      account: beforeAccountResp,
      a651_balance_atoms: beforeA651.toString(),
    },
    after: {
      account: afterAccountResp,
      a651_balance_atoms: after.balance.toString(),
    },
    reconciliation: {
      expected_public_burn_atoms: amountAtoms.toString(),
      actual_public_delta_atoms: (beforeA651 - after.balance).toString(),
      public_balance_decreased_by_at_least_amount: after.balance <= beforeA651 - amountAtoms,
    },
    preflight: {
      ok: preflight.ok,
      status: preflight.status,
      amount_atoms: preflight.amount_atoms,
      operation_kind: preflight.operation?.asset_burn ? 'asset_burn' : preflight.operation?.operation || null,
      issuer: preflight.operation?.asset_burn?.issuer || preflight.operation?.issuer || null,
    },
    relay_state_sync: relayStateSync,
    local_note: {
      ok: localNote.ok,
      output_commitment: localNote.wallet_note?.output_commitment || null,
      vault_record: localNote.vault_record || null,
      wallet_note_redacted: true,
    },
    ingress: {
      ok: ingress.ok,
      status: ingress.status,
      message: ingress.message,
      output_commitment: outputCommitment,
      artifact_dir: ingress.artifact_dir || null,
      receipts: receiptSummary(ingress),
      report_round_ok: ingress.report?.round_ok ?? ingress.report?.transport?.round_ok ?? null,
      report_local_accepted_count: ingress.report?.local_accepted_count ?? ingress.report?.transport?.local_accepted_count ?? null,
      report_local_rejected_count: ingress.report?.local_rejected_count ?? ingress.report?.transport?.local_rejected_count ?? null,
    },
    vault: {
      file: vaultInfo,
      new_files_count: newVaultFiles.length,
      new_files: newVaultFiles,
    },
    screenshots: [
      `run-${runIndex}-before-submit.png`,
      `run-${runIndex}-after-submit.png`,
    ],
  };
  writeJson(join(OUT_DIR, `run-${runIndex}-evidence.json`), runEvidence);
  return runEvidence;
}

async function main() {
  assertOk(Number.isInteger(RUNS) && RUNS > 0, 'ORCHARD_INGRESS_E2E_RUNS must be positive');
  const ingressAtoms = parseAmountAtoms(INGRESS_AMOUNT);
  const fundAtoms = parseAmountAtoms(A651_FUND_AMOUNT);
  assertOk(ingressAtoms > 0n, 'ingress amount must be positive');
  assertOk(fundAtoms >= ingressAtoms * BigInt(RUNS), 'a651 fund amount must cover all ingress runs');

  const rpc = new RpcClient(RPC_URL);
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
  });
  const runEvidence = [];
  let wallet = null;
  let funding = null;
  try {
    const context = await browser.newContext({
      ignoreHTTPSErrors: true,
      viewport: { width: 1440, height: 1100 },
      permissions: ['local-network-access'],
    });
    await context.grantPermissions(['local-network-access'], { origin: new URL(APP_URL).origin });
    const page = await context.newPage();
    page.setDefaultTimeout(60_000);
    page.on('console', (message) => {
      const text = message.text();
      if (/orchard|shielded|ingress|navswap|wallet/i.test(text)) {
        console.log(`[browser:${message.type()}] ${text}`);
      }
    });
    await installCapture(page);
    wallet = await createWallet(page);
    funding = {
      pft: await fundPft(rpc, wallet.accountAddress),
      a651: await fundA651(rpc, wallet.accountAddress, fundAtoms),
    };
    for (let runIndex = 1; runIndex <= RUNS; runIndex += 1) {
      runEvidence.push(await runIngress(page, rpc, wallet, runIndex, ingressAtoms));
    }
    await context.close();
  } finally {
    await browser.close();
    rpc.close();
  }

  const report = {
    schema: 'postfiat-wallet-private-swap-step5-live-ingress-e2e-v1',
    captured_at: new Date().toISOString(),
    app_url: APP_URL,
    rpc_url: RPC_URL,
    route: 'shielded_navswap',
    local_service: 'http://127.0.0.1:8789/asset-orchard/ingress-notes',
    local_vault_dir: LOCAL_VAULT_DIR,
    wallet_address: wallet?.accountAddress || null,
    funding,
    ingress_amount_atoms: ingressAtoms.toString(),
    runs: runEvidence,
    sensitive_material_dir: SENSITIVE_DIR,
    sensitive_material_note: 'Wallet seed/passphrase are stored only in this local /tmp directory and are not copied into docs/evidence.',
    redaction: {
      wallet_note_body: 'redacted from evidence; only output_commitment and vault_record are recorded',
      private_key_material: 'not sent to proxy and not written under docs/evidence',
    },
  };
  writeJson(join(OUT_DIR, 'report.json'), report);
  console.log(JSON.stringify({
    ok: true,
    out_dir: OUT_DIR,
    wallet_address: wallet?.accountAddress || null,
    run_count: runEvidence.length,
    output_commitments: runEvidence.map(run => run.ingress.output_commitment),
  }, null, 2));
}

main().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
