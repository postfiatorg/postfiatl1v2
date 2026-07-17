#!/usr/bin/env node
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import process from 'node:process';
import WebSocket from '../wallet-proxy/node_modules/ws/index.js';

import { RpcClient } from '../wallet-web/src/lib/rpc-client.js';
import { PFUSDC_ASSET_ID } from '../wallet-web/src/lib/utils.js';

globalThis.WebSocket = WebSocket;

const require = createRequire(new URL('../wallet-web/package.json', import.meta.url));
const { chromium } = require('playwright');

const APP_URL = process.env.PFTL_WALLET_E2E_URL || 'http://127.0.0.1:5173/';
const API_BASE = APP_URL.replace(/\/+$/, '');
const RPC_URL = process.env.PFTL_WALLET_E2E_RPC || 'ws://127.0.0.1:8080/rpc';
const RUNS = Number.parseInt(process.env.PFTL_WALLET_E2E_RUNS || '2', 10);
const AMOUNT = process.env.PFTL_WALLET_E2E_AMOUNT || '0.25';
const FUNDING_AMOUNT = process.env.PFTL_WALLET_E2E_FUNDING_AMOUNT || String(Number(AMOUNT) * 2);
const PFT_FUND_AMOUNT_ATOMS = process.env.PFTL_WALLET_E2E_PFT_ATOMS || '100000';
const OUT_DIR = process.env.PFTL_WALLET_E2E_OUT_DIR
  || `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/wallet-controlled-beta-${Date.now()}`;
const SENSITIVE_DIR = process.env.PFTL_WALLET_E2E_SENSITIVE_DIR
  || `/tmp/postfiat-wallet-e2e-sensitive-${Date.now()}`;
const PFT_FUNDER = process.env.PFTL_WALLET_E2E_PFT_FUNDER
  || 'pff3e396f771a8f490ca330e1720472d473bcfcb6d';
const PFT_FUNDER_KEY_FILE = process.env.PFTL_WALLET_E2E_PFT_FUNDER_KEY_FILE
  || '/run/secrets/navswap-issuer.key.json';
const ROUTE = 'uniswap_atomic_handoff';
const ROUTE_ID = process.env.NAVSWAP_ROUTE_ID || 'pftl-a651-usdc-wallet-e2e-20260702-v1';
const PFUSDC_DISPLAY = 'pfUSDC';

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

function parseAmountAtoms(value) {
  const text = String(value || '').trim();
  assertOk(/^[0-9]+(?:\.[0-9]+)?$/.test(text), `invalid decimal amount ${value}`);
  const [whole, frac = ''] = text.split('.');
  const atoms = `${whole}${frac.padEnd(6, '0').slice(0, 6)}`.replace(/^0+(?=\d)/, '');
  return BigInt(atoms || '0');
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

async function safeRpc(rpc, method, params = {}, timeoutMs = 20_000) {
  try {
    return await rpc.call(method, params, timeoutMs);
  } catch (error) {
    return { ok: false, error: { message: error.message || String(error) } };
  }
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

async function waitForAssetBalance(rpc, account, assetId, minimum, timeoutMs = 120_000) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() <= deadline) {
    const resp = await rpc.accountAssets(account);
    last = resp;
    const balance = canonicalBalanceAtoms(resp.result, assetId);
    if (resp.ok && balance >= BigInt(minimum)) return { resp, balance };
    await new Promise(resolve => setTimeout(resolve, 1500));
  }
  throw new Error(`asset ${assetId} balance for ${account} did not reach ${minimum}; last=${JSON.stringify(last)}`);
}

async function fundPft(rpc, recipient) {
  const pftFundAmount = Number(PFT_FUND_AMOUNT_ATOMS);
  assertOk(Number.isSafeInteger(pftFundAmount) && pftFundAmount > 0, 'PFT funding amount must be a safe positive integer');
  const quoteResp = await rpc.transferFeeQuote(PFT_FUNDER, recipient, pftFundAmount);
  assertOk(quoteResp.ok, `PFT funding transfer_fee_quote failed: ${quoteResp.error?.message || 'unknown'}`);
  assertOk(quoteResp.result?.sender_meets_reserve_after_transfer !== false, 'PFT funding signer would fall below reserve');
  assertOk(quoteResp.result?.recipient_meets_reserve_after_transfer !== false, 'PFT funding recipient would still be below reserve');

  const signer = [
    'tmp=$(mktemp)',
    'trap "rm -f $tmp" EXIT',
    'cat > "$tmp"',
    `/usr/local/bin/postfiat-node wallet-sign-transfer --key-file "${PFT_FUNDER_KEY_FILE}" --quote-file "$tmp"`,
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
    input: JSON.stringify(quoteResp.result),
    encoding: 'utf8',
    maxBuffer: 4 * 1024 * 1024,
  });
  if (proc.error) throw proc.error;
  assertOk(proc.status === 0, `wallet-proxy PFT signer failed (${proc.status}): ${proc.stderr || proc.stdout}`);
  const signed = JSON.parse(proc.stdout);
  const submitResp = await rpc.submitSignedTransferFinality(JSON.stringify(signed));
  assertOk(submitResp.ok, `PFT funding submit failed: ${submitResp.error?.message || 'unknown'}`);
  const after = await waitForNativeBalance(rpc, recipient, PFT_FUND_AMOUNT_ATOMS);
  return {
    ok: true,
    amount_atoms: PFT_FUND_AMOUNT_ATOMS,
    tx_id: submitResp.result?.tx_id || null,
    finality_height: submitResp.result?.finality?.block?.header?.height || null,
    balance_atoms: after.balance.toString(),
  };
}

async function fundPfusdc(recipient) {
  const quoteRes = await fetch(`${API_BASE}/api/navswap/quotes`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      route: ROUTE,
      wallet_address: recipient,
      from_asset: PFUSDC_DISPLAY,
      to_asset: 'a651',
      amount: FUNDING_AMOUNT,
      auto_plan: true,
    }),
  });
  const quote = await quoteRes.json();
  assertOk(quoteRes.ok && quote.ok, `PFTL-Uniswap funding quote failed: ${quote.message || quoteRes.status}`);
  const amountAtoms = BigInt(String(quote.settlement_amount_atoms || '0'));
  assertOk(amountAtoms > 0n, 'PFTL-Uniswap funding quote did not return settlement_amount_atoms');
  assertOk(amountAtoms <= BigInt(Number.MAX_SAFE_INTEGER), 'pfUSDC funding amount exceeds JS safe integer range');

  const operation = {
    operation: 'issued_payment',
    from: PFT_FUNDER,
    to: recipient,
    issuer: PFT_FUNDER,
    asset_id: PFUSDC_ASSET_ID,
    amount: Number(amountAtoms),
  };
  return { quote, operation, amountAtoms };
}

async function signAndSubmitPfusdcFunding(rpc, recipient) {
  const prepared = await fundPfusdc(recipient);
  const quoteResp = await rpc.assetFeeQuote(PFT_FUNDER, JSON.stringify(prepared.operation));
  assertOk(quoteResp.ok, `pfUSDC funding asset_fee_quote failed: ${quoteResp.error?.message || 'unknown'}`);
  assertOk(quoteResp.result?.sender_meets_reserve_after_fee !== false, 'pfUSDC issuer would fall below PFT fee reserve');
  const signer = [
    'tmp=$(mktemp)',
    'trap "rm -f $tmp" EXIT',
    'cat > "$tmp"',
    `/usr/local/bin/postfiat-node wallet-sign-asset-transaction --key-file "${PFT_FUNDER_KEY_FILE}" --quote-file "$tmp"`,
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
    input: JSON.stringify(quoteResp.result),
    encoding: 'utf8',
    maxBuffer: 4 * 1024 * 1024,
  });
  if (proc.error) throw proc.error;
  assertOk(proc.status === 0, `wallet-proxy pfUSDC signer failed (${proc.status}): ${proc.stderr || proc.stdout}`);
  const signed = JSON.parse(proc.stdout);
  const submitResp = await rpc.submitSignedAssetTransactionFinality(JSON.stringify(signed));
  assertOk(submitResp.ok, `pfUSDC funding submit failed: ${submitResp.error?.message || 'unknown'}`);
  const after = await waitForAssetBalance(rpc, recipient, PFUSDC_ASSET_ID, prepared.amountAtoms);
  return {
    ok: true,
    amount_atoms: prepared.amountAtoms.toString(),
    tx_id: submitResp.result?.tx_id || null,
    finality_height: submitResp.result?.finality?.block?.header?.height || null,
    before_quote: prepared.quote,
    operation: prepared.operation,
    asset_fee_quote: quoteResp.result,
    balance_atoms: after.balance.toString(),
  };
}

async function installCapture(page) {
  await page.addInitScript(() => {
    window.__pftlE2e = { navswapResponses: [] };
    const originalFetch = window.fetch.bind(window);
    window.fetch = async (...args) => {
      const response = await originalFetch(...args);
      const url = typeof args[0] === 'string' ? args[0] : args[0]?.url || '';
      if (url.includes('/api/navswap/')) {
        response.clone().json().then((body) => {
          window.__pftlE2e.navswapResponses.push({
            at: new Date().toISOString(),
            url,
            status: response.status,
            ok: response.ok,
            body,
          });
        }).catch((error) => {
          window.__pftlE2e.navswapResponses.push({
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

async function createWallet(page, runIndex) {
  const passphrase = `pftl-uniswap-e2e-${Date.now()}-${runIndex}`;
  await page.goto(APP_URL, { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 30_000 }).catch(() => {});
  await page.screenshot({ path: join(OUT_DIR, `run-${runIndex}-00-onboard.png`), fullPage: true });
  await page.getByRole('button', { name: /^Create Wallet$/ }).first().click({ timeout: 30_000 });
  await page.waitForSelector('.pf-seed-display', { timeout: 30_000 });
  const seed = (await page.locator('.pf-seed-display').innerText()).trim();
  assertOk(/^[0-9a-f]{64}$/i.test(seed), 'generated wallet seed was not displayed');
  const accountAddress = (await page.locator('text=/pf[a-f0-9]{40}/i').first().innerText()).trim();
  writeJson(join(SENSITIVE_DIR, `run-${runIndex}-wallet-sensitive.json`), {
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
  await page.screenshot({ path: join(OUT_DIR, `run-${runIndex}-01-wallet-created.png`), fullPage: true });
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

async function readUiBalances(page) {
  return page.evaluate(() => {
    const balances = {};
    const rows = Array.from(document.querySelectorAll('.pfs-balance-list div'));
    for (const row of rows) {
      const parts = (row.innerText || '').split(/\n+/).map(part => part.trim()).filter(Boolean);
      if (parts.length >= 2) balances[parts[0]] = parts[1];
    }
    return balances;
  });
}

async function selectPftlRoute(page, runIndex) {
  await page.locator('button').filter({ hasText: 'Swap' }).first().click({ timeout: 15_000 });
  await page.waitForSelector('text=/Move between assets/i', { timeout: 45_000 });
  await clickEnabledButton(page, /PFTL-Uniswap beta/i, 120_000);
  await page.waitForSelector('text=/Mint \\+ export/i', { timeout: 45_000 });
  await page.screenshot({ path: join(OUT_DIR, `run-${runIndex}-02-pftl-route-selected.png`), fullPage: true });
}

function extractRunCompletion(responses) {
  const bodies = responses.map(entry => entry.body).filter(Boolean);
  const completed = bodies.find(body => body?.result?.receipt_verification)
    || bodies.find(body => body?.status === 'destination_consume_submitted')
    || bodies.find(body => body?.ok === true && body?.terminal === true);
  const quote = bodies.find(body => body?.route === ROUTE && body?.prepared_action_batch);
  const run = bodies.find(body => body?.route === ROUTE && body?.run_id);
  const verification = completed?.result?.receipt_verification || completed?.receipt_verification || null;
  return { quote, run, completed, verification };
}

function navswapRunTerminal(status) {
  if (!status) return false;
  if (status.terminal === true) return true;
  if (status.ok === true || status.ok === false) return true;
  return [
    'operator_mint_submitted',
    'operator_redeem_settle_submitted',
    'destination_consume_submitted',
    'complete',
    'failed',
    'transparent_complete',
  ].includes(status.status);
}

async function waitForCapturedRunId(page, startIndex, timeoutMs = 120_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() <= deadline) {
    const runId = await page.evaluate((start) => {
      const responses = (window.__pftlE2e?.navswapResponses || []).slice(start);
      for (let i = responses.length - 1; i >= 0; i -= 1) {
        const body = responses[i]?.body;
        if (body?.run_id) return body.run_id;
        if (body?.status?.run_id) return body.status.run_id;
      }
      return null;
    }, startIndex);
    if (runId) return runId;
    await page.waitForTimeout(1000);
  }
  throw new Error('timed out waiting for captured NAVSwap run id');
}

async function pollNavswapRun(runId, timeoutMs = 240_000) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() <= deadline) {
    const res = await fetch(`${API_BASE}/api/navswap/runs/${encodeURIComponent(runId)}`);
    last = await res.json();
    if (navswapRunTerminal(last)) return last;
    await new Promise(resolve => setTimeout(resolve, 1500));
  }
  throw new Error(`NAVSwap run ${runId} did not reach terminal status; last=${JSON.stringify(last)}`);
}

async function runWalletFlow(page, rpc, runIndex, wallet) {
  await selectPftlRoute(page, runIndex);
  const amountInput = page.locator('input[aria-label^="Amount of"]').first();
  await amountInput.waitFor({ timeout: 60_000 });
  await amountInput.fill(AMOUNT);
  await page.screenshot({ path: join(OUT_DIR, `run-${runIndex}-03-amount-entered.png`), fullPage: true });
  await clickEnabledButton(page, /Get quote|Refresh quote/i, 240_000);
  await page.waitForFunction(
    () => /Submit wallet source actions/i.test(document.body.innerText || ''),
    null,
    { timeout: 240_000 },
  );
  await page.screenshot({ path: join(OUT_DIR, `run-${runIndex}-04-quote-ready.png`), fullPage: true });
  const beforeResponses = await page.evaluate(() => window.__pftlE2e?.navswapResponses?.length || 0);
  const beforeAssetsResp = await rpc.accountAssets(wallet.accountAddress);
  const beforeAccountResp = await rpc.account(wallet.accountAddress);
  const beforeSupply = await safeRpc(rpc, 'navcoin_bridge_supply_status', { route_id: ROUTE_ID });
  const beforeRoutes = await safeRpc(rpc, 'navcoin_bridge_routes', {});
  await clickEnabledButton(page, /Submit wallet source actions/i, 300_000);
  await page.waitForFunction(
    () => /Swap complete|Swap needs attention|Source actions submitted/i.test(document.body.innerText || ''),
    null,
    { timeout: 900_000 },
  );
  const runId = await waitForCapturedRunId(page, beforeResponses);
  const terminalRun = await pollNavswapRun(runId);
  await page.waitForTimeout(2000);
  const finalText = await page.locator('body').innerText();
  await page.screenshot({ path: join(OUT_DIR, `run-${runIndex}-05-complete.png`), fullPage: true });
  assertOk(!/Swap needs attention/i.test(finalText), `wallet flow ${runIndex} failed in UX:\n${finalText}`);

  const capturedResponses = await page.evaluate((start) => (
    window.__pftlE2e?.navswapResponses || []
  ).slice(start), beforeResponses);
  const responses = [
    ...capturedResponses,
    {
      at: new Date().toISOString(),
      url: `${API_BASE}/api/navswap/runs/${runId}`,
      status: 200,
      ok: true,
      body: terminalRun,
    },
  ];
  const completion = extractRunCompletion(responses);
  assertOk(completion.verification?.checks?.wallet_primary_submitted, 'completion missing wallet primary submitted check');
  assertOk(completion.verification?.checks?.source_packet_matches_wallet_export, 'completion missing source packet verification check');
  assertOk(completion.verification?.checks?.operator_submit_accepted, 'completion missing operator submit accepted check');

  const packetHash = completion.verification.packet_hash;
  const packetStatus = packetHash
    ? await safeRpc(rpc, 'navcoin_bridge_packet', { route_id: ROUTE_ID, packet_hash: packetHash })
    : null;
  const afterAssetsResp = await rpc.accountAssets(wallet.accountAddress);
  const afterAccountResp = await rpc.account(wallet.accountAddress);
  const afterSupply = await safeRpc(rpc, 'navcoin_bridge_supply_status', { route_id: ROUTE_ID });
  const uiBalances = await readUiBalances(page);

  const beforePfusdc = canonicalBalanceAtoms(beforeAssetsResp.result, PFUSDC_ASSET_ID);
  const afterPfusdc = canonicalBalanceAtoms(afterAssetsResp.result, PFUSDC_ASSET_ID);
  assertOk(afterPfusdc < beforePfusdc, `pfUSDC balance did not decrease: before=${beforePfusdc} after=${afterPfusdc}`);

  return {
    run_index: runIndex,
    wallet_address: wallet.accountAddress,
    amount: AMOUNT,
    before: {
      account: beforeAccountResp,
      assets: beforeAssetsResp,
      pfusdc_atoms: beforePfusdc.toString(),
      supply: beforeSupply,
      routes: beforeRoutes,
    },
    after: {
      account: afterAccountResp,
      assets: afterAssetsResp,
      pfusdc_atoms: afterPfusdc.toString(),
      supply: afterSupply,
      packet_status: packetStatus,
      ui_balances: uiBalances,
      final_text: finalText,
    },
    navswap_responses: responses,
    completion,
  };
}

async function main() {
  assertOk(Number.isInteger(RUNS) && RUNS > 0, 'PFTL_WALLET_E2E_RUNS must be positive');
  assertOk(parseAmountAtoms(AMOUNT) > 0n, 'PFTL_WALLET_E2E_AMOUNT must be positive');
  assertOk(parseAmountAtoms(FUNDING_AMOUNT) >= parseAmountAtoms(AMOUNT), 'funding amount must cover swap amount');

  const rpc = new RpcClient(RPC_URL);
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
  });
  const runs = [];
  try {
    for (let runIndex = 1; runIndex <= RUNS; runIndex += 1) {
      const context = await browser.newContext({
        ignoreHTTPSErrors: true,
        viewport: { width: 1440, height: 1100 },
      });
      const page = await context.newPage();
      page.setDefaultTimeout(60_000);
      page.on('console', (message) => {
        const text = message.text();
        if (/navswap|pftl|uniswap|source|destination|wallet/i.test(text)) {
          console.log(`[browser:${message.type()}] ${text}`);
        }
      });
      await installCapture(page);
      const wallet = await createWallet(page, runIndex);
      const pftFunding = await fundPft(rpc, wallet.accountAddress);
      const pfusdcFunding = await signAndSubmitPfusdcFunding(rpc, wallet.accountAddress);
      const minPfusdc = BigInt(pfusdcFunding.amount_atoms || '0');
      await waitForAssetBalance(rpc, wallet.accountAddress, PFUSDC_ASSET_ID, minPfusdc);
      const flow = await runWalletFlow(page, rpc, runIndex, wallet);
      runs.push({
        wallet,
        funding: { pft: pftFunding, pfusdc: pfusdcFunding },
        flow,
      });
      await context.close();
    }
  } finally {
    await browser.close();
    rpc.close();
  }

  const report = {
    schema: 'postfiat-pftl-uniswap-wallet-controlled-beta-e2e-v1',
    captured_at: new Date().toISOString(),
    app_url: APP_URL,
    rpc_url: RPC_URL,
    route: ROUTE,
    route_id: ROUTE_ID,
    amount: AMOUNT,
    funding_amount: FUNDING_AMOUNT,
    runs: runs.map(run => ({
      wallet_address: run.wallet.accountAddress,
      funding: run.funding,
      flow: run.flow,
    })),
    sensitive_material_dir: SENSITIVE_DIR,
    sensitive_material_note: 'Wallet seeds are stored only in this local /tmp directory and are not copied into docs/evidence.',
    limitations: [
      'Wallet UX flow covers source primary subscribe, source export, consensus packet verification, and CONTROLLED operator-attested destination consume.',
      'Current wallet-proxy completion does not submit a real fork consumeMintAndSwap transaction.',
      'Flow E return-import, refund drill, and pause drill remain open for the directive.',
    ],
  };
  writeJson(join(OUT_DIR, 'report.json'), report);
  console.log(JSON.stringify({
    ok: true,
    out_dir: OUT_DIR,
    sensitive_dir: SENSITIVE_DIR,
    run_count: runs.length,
    wallets: runs.map(run => run.wallet.accountAddress),
    packet_hashes: runs.map(run => run.flow.completion.verification?.packet_hash || null),
  }, null, 2));
}

main().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
