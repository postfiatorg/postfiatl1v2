import { execFile } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { promisify } from 'node:util';

import { chromium } from 'playwright';

const execFileAsync = promisify(execFile);

const walletUrl = process.env.WALLET_WEB_URL || 'https://127.0.0.1:5173';
const ethereumRpc = process.env.ETHEREUM_RPC_URL || 'https://ethereum-rpc.publicnode.com';
const keystore = process.env.E2E_ETH_KEYSTORE;
const passwordFile = process.env.E2E_ETH_PASSWORD_FILE;
const tokenFile = process.env.WALLET_PROXY_API_TOKENS_FILE;
const evidenceDir = process.env.E2E_EVIDENCE_DIR;
const amountAtoms = 1_000_000n;
const canonicalUsdc = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
const governedVault = '0xaaa78fda7062efce769e95cd72fc55e507bc8183';
const approveSelector = '0x095ea7b3';
const depositSelector = '0x2391b457';

if (!keystore || !passwordFile || !tokenFile || !evidenceDir) {
  throw new Error(
    'E2E_ETH_KEYSTORE, E2E_ETH_PASSWORD_FILE, WALLET_PROXY_API_TOKENS_FILE, '
    + 'and E2E_EVIDENCE_DIR are required',
  );
}

await mkdir(evidenceDir, { recursive: true, mode: 0o700 });

const tokenMap = JSON.parse(await readFile(tokenFile, 'utf8'));
const proxyToken = String(tokenMap['local-demo'] || '');
if (proxyToken.length < 32) throw new Error('local-demo proxy token is unavailable');

const { stdout: addressStdout } = await execFileAsync('cast', [
  'wallet',
  'address',
  '--keystore',
  keystore,
  '--password-file',
  passwordFile,
]);
const ethereumAddress = addressStdout.trim();
if (!/^0x[0-9a-f]{40}$/i.test(ethereumAddress)) {
  throw new Error('test Ethereum address is invalid');
}

let rpcId = 0;
const sentTransactions = [];
const browserErrors = [];

async function ethereumRequest(method, params = []) {
  const response = await fetch(ethereumRpc, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: `ux-${++rpcId}`,
      method,
      params,
    }),
  });
  const payload = await response.json();
  if (payload.error) {
    const failure = new Error(payload.error.message || `Ethereum RPC ${method} failed`);
    failure.code = payload.error.code;
    throw failure;
  }
  return payload.result;
}

function assertBoundedBrowserTransaction(tx) {
  const from = String(tx?.from || '').toLowerCase();
  const to = String(tx?.to || '').toLowerCase();
  const data = String(tx?.data || '').toLowerCase();
  const value = BigInt(tx?.value || '0x0');
  if (from !== ethereumAddress.toLowerCase()) throw new Error('browser changed transaction sender');
  if (value !== 0n) throw new Error('browser transaction unexpectedly transfers native ETH');

  if (to === canonicalUsdc) {
    if (!data.startsWith(approveSelector) || data.length !== 138) {
      throw new Error('browser proposed a non-approve USDC transaction');
    }
    const spender = `0x${data.slice(34, 74)}`;
    const amount = BigInt(`0x${data.slice(74, 138)}`);
    if (spender !== governedVault || amount !== amountAtoms) {
      throw new Error('browser approval is not the exact 1 USDC governed-vault allowance');
    }
    return 'approve';
  }

  if (to === governedVault) {
    if (!data.startsWith(depositSelector)) {
      throw new Error('browser proposed a non-deposit governed-vault transaction');
    }
    const amount = BigInt(`0x${data.slice(10, 74)}`);
    if (amount !== amountAtoms) {
      throw new Error('browser deposit is not exactly 1 USDC');
    }
    return 'deposit';
  }

  throw new Error(`browser proposed an unapproved transaction destination ${to}`);
}

async function sendBrowserTransaction(tx) {
  const kind = assertBoundedBrowserTransaction(tx);
  const { stdout } = await execFileAsync('cast', [
    'send',
    tx.to,
    '--data',
    tx.data,
    '--value',
    tx.value || '0x0',
    '--rpc-url',
    ethereumRpc,
    '--keystore',
    keystore,
    '--password-file',
    passwordFile,
    '--async',
    '--json',
  ], { maxBuffer: 1024 * 1024 });
  const match = stdout.match(/0x[0-9a-f]{64}/i);
  if (!match) throw new Error(`cast did not return a transaction hash for ${kind}`);
  const txHash = match[0].toLowerCase();
  sentTransactions.push({ kind, tx_hash: txHash });
  return txHash;
}

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  ignoreHTTPSErrors: true,
  viewport: { width: 1440, height: 1100 },
  colorScheme: 'dark',
});

await context.exposeBinding('pftlE2eEthereumRequest', async (_source, request) => {
  const method = String(request?.method || '');
  const params = Array.isArray(request?.params) ? request.params : [];
  if (method === 'eth_requestAccounts' || method === 'eth_accounts') return [ethereumAddress];
  if (method === 'eth_chainId') return '0x1';
  if (method === 'wallet_switchEthereumChain') return null;
  if (method === 'eth_sendTransaction') return sendBrowserTransaction(params[0]);
  return ethereumRequest(method, params);
});

await context.addInitScript(() => {
  const listeners = new Map();
  window.ethereum = {
    isMetaMask: true,
    request: ({ method, params = [] }) => window.pftlE2eEthereumRequest({ method, params }),
    on(event, handler) {
      if (!listeners.has(event)) listeners.set(event, new Set());
      listeners.get(event).add(handler);
    },
    removeListener(event, handler) {
      listeners.get(event)?.delete(handler);
    },
  };
});

const page = await context.newPage();
page.setDefaultTimeout(120_000);
page.on('pageerror', (error) => browserErrors.push(`pageerror: ${error.message}`));
page.on('console', (message) => {
  if (message.type() === 'error' && !message.text().includes('frame-ancestors')) {
    browserErrors.push(message.text());
  }
});

const startedAt = Date.now();
let pftlAddress = '';
try {
  await page.goto(walletUrl, { waitUntil: 'networkidle' });

  await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();
  pftlAddress = await page.locator('text=/^pf[0-9a-f]{40}$/').first().textContent();
  await page.getByRole('checkbox').check();
  await page.getByPlaceholder('Encryption passphrase (min 10 chars)').fill('live-ux-acceptance-2026');
  await page.getByPlaceholder('Confirm passphrase').fill('live-ux-acceptance-2026');
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();
  await page.getByText(/height [1-9]\d*/).first().waitFor({ state: 'visible' });

  await page.locator('.pf-sidebar .pf-nav', { hasText: 'More' }).click();
  await page.locator('input[type="password"]').fill(proxyToken);
  await page.getByRole('button', { name: 'Save settings', exact: true }).click();
  await page.getByText('Settings saved', { exact: true }).waitFor({ state: 'visible' });

  await page.locator('.pf-sidebar .pf-nav', { hasText: 'Bridge' }).click();
  const connectButton = page.getByRole('button', { name: 'Connect MetaMask', exact: true });
  if (await connectButton.count() > 0 && await connectButton.isVisible()) {
    await connectButton.click();
  }
  await page.getByRole('button', { name: 'Approve Ethereum USDC', exact: true })
    .waitFor({ state: 'visible' });
  await page.getByText('1 USDC', { exact: true }).first().waitFor({ state: 'visible' });
  await page.locator('input[placeholder="0.00"]').fill('1');

  await page.screenshot({ path: `${evidenceDir}/01-funded-connected.png`, fullPage: true });
  await page.getByRole('button', { name: 'Approve Ethereum USDC', exact: true })
    .click({ timeout: 240_000 });
  await page.getByRole('button', { name: /Deposit and relay/ }).waitFor({ state: 'visible' });
  await page.screenshot({ path: `${evidenceDir}/02-approved.png`, fullPage: true });

  await page.getByRole('button', { name: /Deposit and relay/ }).click();
  await page.getByText('pfUSDC received on PFTL', { exact: true }).waitFor({
    state: 'visible',
    timeout: 2_400_000,
  });
  await page.screenshot({ path: `${evidenceDir}/03-pfusdc-received.png`, fullPage: true });

  const body = await page.locator('body').innerText();
  if (sentTransactions.map((item) => item.kind).join(',') !== 'approve,deposit') {
    throw new Error('browser did not submit exactly one approval and one deposit');
  }
  if (!body.includes('1 pfUSDC')) {
    throw new Error('wallet did not render the resulting 1 pfUSDC balance');
  }
  if (browserErrors.length) {
    throw new Error(`browser console errors: ${browserErrors.join(' | ')}`);
  }

  const result = {
    ok: true,
    schema: 'postfiat.wallet.live_eth_pfusdc_ux_acceptance.v1',
    wallet_url: walletUrl,
    ethereum_address: ethereumAddress,
    pftl_address: pftlAddress,
    amount_atoms: amountAtoms.toString(),
    transactions: sentTransactions,
    elapsed_ms: Date.now() - startedAt,
    terminal_copy: 'pfUSDC received on PFTL',
  };
  await writeFile(`${evidenceDir}/result.json`, `${JSON.stringify(result, null, 2)}\n`, {
    mode: 0o600,
  });
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
} catch (error) {
  await page.screenshot({ path: `${evidenceDir}/FAILURE.png`, fullPage: true }).catch(() => {});
  const failure = {
    ok: false,
    error: error.message,
    ethereum_address: ethereumAddress,
    pftl_address: pftlAddress,
    transactions: sentTransactions,
    browser_errors: browserErrors,
    elapsed_ms: Date.now() - startedAt,
  };
  await writeFile(`${evidenceDir}/failure.json`, `${JSON.stringify(failure, null, 2)}\n`, {
    mode: 0o600,
  });
  throw error;
} finally {
  await browser.close();
}
