import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { promisify } from 'node:util';

import { chromium } from 'playwright';

const execFileAsync = promisify(execFile);
const walletUrl = process.env.WALLET_WEB_URL || 'https://127.0.0.1:5173';
const ethereumRpc = process.env.ETHEREUM_RPC_URL || 'https://ethereum-rpc.publicnode.com';
const keystore = process.env.E2E_ETH_KEYSTORE;
const passwordFile = process.env.E2E_ETH_PASSWORD_FILE;
const pftlBackupFile = process.env.E2E_PFTL_BACKUP_FILE;
const evidenceDir = process.env.E2E_EVIDENCE_DIR;
const amountAtoms = 1_000_000n;
const wrappedA666 = '0xee4c92edb03efdd9b519339edc19ad70c69a9be5';
const restartProxyAfterPacket = process.env.E2E_RESTART_PROXY_AFTER_PACKET === 'true';

if (!keystore || !passwordFile || !pftlBackupFile || !evidenceDir) {
  throw new Error('E2E_ETH_KEYSTORE, E2E_ETH_PASSWORD_FILE, E2E_PFTL_BACKUP_FILE, and E2E_EVIDENCE_DIR are required');
}
await mkdir(evidenceDir, { recursive: true, mode: 0o700 });
const backup = JSON.parse(await readFile(pftlBackupFile, 'utf8'));
if (!/^[0-9a-f]{64}$/.test(String(backup.master_seed_hex || ''))) {
  throw new Error('PFTL wallet backup is missing a valid master seed');
}

const { stdout: addressStdout } = await execFileAsync('cast', [
  'wallet', 'address', '--keystore', keystore, '--password-file', passwordFile,
]);
const ethereumAddress = addressStdout.trim().toLowerCase();
if (!/^0x[0-9a-f]{40}$/.test(ethereumAddress)) throw new Error('Ethereum test account is invalid');

let rpcId = 0;
async function ethereumRequest(method, params = []) {
  const response = await fetch(ethereumRpc, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: `a666-ux-${++rpcId}`, method, params }),
    signal: AbortSignal.timeout(30_000),
  });
  const payload = await response.json();
  if (payload.error) throw new Error(payload.error.message || `Ethereum RPC ${method} failed`);
  return payload.result;
}

async function wrappedBalance() {
  const data = `0x70a08231${ethereumAddress.slice(2).padStart(64, '0')}`;
  return BigInt(await ethereumRequest('eth_call', [{ to: wrappedA666, data }, 'latest']));
}

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  ignoreHTTPSErrors: true,
  viewport: { width: 1440, height: 1200 },
  colorScheme: 'dark',
});
await context.exposeBinding('pftlA666EthereumRequest', async (_source, request) => {
  const method = String(request?.method || '');
  const params = Array.isArray(request?.params) ? request.params : [];
  if (method === 'eth_requestAccounts' || method === 'eth_accounts') return [ethereumAddress];
  if (method === 'eth_chainId') return '0x1';
  if (method === 'wallet_switchEthereumChain') return null;
  if (method === 'wallet_watchAsset') return true;
  if (method === 'eth_sendTransaction') throw new Error('A666 export UX must not request an Ethereum spend');
  return ethereumRequest(method, params);
});
await context.addInitScript(() => {
  const listeners = new Map();
  window.ethereum = {
    isMetaMask: true,
    request: ({ method, params = [] }) => window.pftlA666EthereumRequest({ method, params }),
    on(event, handler) {
      if (!listeners.has(event)) listeners.set(event, new Set());
      listeners.get(event).add(handler);
    },
    removeListener(event, handler) { listeners.get(event)?.delete(handler); },
  };
});

const page = await context.newPage();
page.setDefaultTimeout(120_000);
const browserErrors = [];
page.on('pageerror', error => browserErrors.push(`pageerror: ${error.message}`));
page.on('console', message => {
  if (message.type() === 'error' && !message.text().includes('frame-ancestors')) {
    browserErrors.push(`console: ${message.text()}`);
  }
});

const startedAt = Date.now();
let packetHash = '';
let pftlAddress = '';
const balanceBefore = await wrappedBalance();
try {
  await page.goto(walletUrl, { waitUntil: 'networkidle' });
  await page.getByRole('button', { name: 'Import Wallet', exact: true }).click();
  await page.getByPlaceholder(/64 hex chars/).fill(backup.master_seed_hex);
  await page.getByRole('button', { name: 'Validate Seed', exact: true }).click();
  pftlAddress = String(await page.locator('text=/^pf[0-9a-f]{40}$/').first().textContent()).trim();
  if (pftlAddress !== 'pfab9b9228942e5c529633a13aa271d5297bec6353') {
    throw new Error(`backup derived unexpected PFTL address ${pftlAddress}`);
  }
  const passphrase = 'a666-live-export-20260731';
  await page.getByPlaceholder('Encryption passphrase (min 10 chars)').fill(passphrase);
  await page.getByPlaceholder('Confirm passphrase').fill(passphrase);
  await page.getByRole('button', { name: 'Confirm Import', exact: true }).click();
  await page.getByText(/height [1-9]\d*/).first().waitFor({ state: 'visible' });

  await page.locator('.pf-sidebar .pf-nav', { hasText: 'NAV Markets' }).click();
  await page.getByText('Live route · invariant holds', { exact: true }).waitFor({ state: 'visible' });
  await page.getByRole('button', { name: 'Connect MetaMask', exact: true }).click();
  await page.locator('#a666-eth-recipient').waitFor({ state: 'visible' });
  const recipient = await page.locator('#a666-eth-recipient').inputValue();
  if (recipient !== ethereumAddress) throw new Error('wallet did not bind the connected MetaMask recipient');
  await page.locator('#navcoin-amount').fill('1');
  await page.getByRole('button', { name: 'Mint & export 1 A666', exact: true }).waitFor({ state: 'visible' });
  await page.screenshot({ path: `${evidenceDir}/01-ready-to-export.png`, fullPage: true });
  await page.getByRole('button', { name: 'Mint & export 1 A666', exact: true }).click();

  const market = page.getByTestId('navcoin-market');
  const marketHandle = await market.elementHandle();
  await Promise.race([
    page.waitForFunction(
      element => /^[0-9a-f]{96}$/.test(element?.getAttribute('data-export-packet-hash') || ''),
      marketHandle,
      { timeout: 600_000 },
    ),
    page.locator('.a666-trade-card .pf-error').waitFor({ state: 'visible', timeout: 600_000 }).then(async () => {
      throw new Error(`wallet export failed before packet finality: ${await page.locator('.a666-trade-card .pf-error').innerText()}`);
    }),
  ]);
  packetHash = String(await market.getAttribute('data-export-packet-hash'));
  await page.screenshot({ path: `${evidenceDir}/02-pftl-export-finalized.png`, fullPage: true });

  if (restartProxyAfterPacket) {
    await execFileAsync('systemctl', ['--user', 'restart', 'pft-wallet-proxy-8080.service'], {
      timeout: 30_000,
    });
  }

  await Promise.race([
    page.getByText('wA666 delivered to MetaMask', { exact: true }).waitFor({
      state: 'visible',
      timeout: 2_700_000,
    }),
    page.locator('.a666-trade-card .pf-error').waitFor({
      state: 'visible',
      timeout: 2_700_000,
    }).then(async () => {
      throw new Error(`wallet export relay failed: ${await page.locator('.a666-trade-card .pf-error').innerText()}`);
    }),
  ]);
  const balanceAfter = await wrappedBalance();
  if (balanceAfter - balanceBefore !== amountAtoms) {
    throw new Error(`wA666 balance delta was ${balanceAfter - balanceBefore}, expected ${amountAtoms}`);
  }
  await page.screenshot({ path: `${evidenceDir}/03-wa666-in-metamask.png`, fullPage: true });
  const materialErrors = browserErrors.filter(value => (
    !value.includes('Failed to load resource:')
    && !(restartProxyAfterPacket
      && value.includes('WebSocket connection')
      && value.includes('Unexpected response code: 502'))
  ));
  if (materialErrors.length) throw new Error(`browser errors: ${materialErrors.join(' | ')}`);

  const relayJobId = `0x${createHash('sha256')
    .update('postfiat.a666.export-relay.job.v1\0')
    .update(Buffer.from(packetHash, 'hex'))
    .digest('hex')}`;
  const relayJob = await page.evaluate(async jobId => {
    const response = await fetch(`/api/a666/export-jobs/${encodeURIComponent(jobId)}`, { cache: 'no-store' });
    return response.json();
  }, relayJobId);
  if (relayJob?.ok !== true || relayJob?.status !== 'accepted') {
    throw new Error(`durable relay did not reach accepted: ${JSON.stringify(relayJob)}`);
  }

  const result = {
    ok: true,
    schema: 'postfiat.wallet.live_a666_metamask_export_ux.v1',
    pftl_address: pftlAddress,
    ethereum_address: ethereumAddress,
    packet_hash: packetHash,
    amount_atoms: amountAtoms.toString(),
    wrapped_balance_before_atoms: balanceBefore.toString(),
    wrapped_balance_after_atoms: balanceAfter.toString(),
    relay_job_id: relayJobId,
    relay_status: relayJob.status,
    relay_retry_count: relayJob.retry_count,
    proxy_restart_injected: restartProxyAfterPacket,
    elapsed_ms: Date.now() - startedAt,
    terminal_copy: 'wA666 delivered to MetaMask',
  };
  await writeFile(`${evidenceDir}/result.json`, `${JSON.stringify(result, null, 2)}\n`, { mode: 0o600 });
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
} catch (error) {
  await page.screenshot({ path: `${evidenceDir}/FAILURE.png`, fullPage: true }).catch(() => {});
  const failure = {
    ok: false,
    error: error.message,
    pftl_address: pftlAddress,
    ethereum_address: ethereumAddress,
    packet_hash: packetHash,
    wrapped_balance_before_atoms: balanceBefore.toString(),
    wrapped_balance_now_atoms: (await wrappedBalance().catch(() => 0n)).toString(),
    browser_errors: browserErrors,
    elapsed_ms: Date.now() - startedAt,
  };
  await writeFile(`${evidenceDir}/failure.json`, `${JSON.stringify(failure, null, 2)}\n`, { mode: 0o600 });
  throw error;
} finally {
  await browser.close();
}
