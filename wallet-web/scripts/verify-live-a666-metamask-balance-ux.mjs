import { execFile } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { promisify } from 'node:util';

import { chromium } from 'playwright';

const execFileAsync = promisify(execFile);
const walletUrl = process.env.WALLET_WEB_URL || 'https://127.0.0.1:5173';
const ethereumRpc = process.env.ETHEREUM_RPC_URL || 'https://ethereum-rpc.publicnode.com';
const keystore = process.env.E2E_ETH_KEYSTORE;
const passwordFile = process.env.E2E_ETH_PASSWORD_FILE;
const pftlBackupFile = process.env.E2E_PFTL_BACKUP_FILE;
const evidenceDir = process.env.E2E_EVIDENCE_DIR;
const minimumBalanceAtoms = BigInt(process.env.E2E_MINIMUM_WA666_ATOMS || '1000000');
const wrappedA666 = '0xee4c92edb03efdd9b519339edc19ad70c69a9be5';

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
    body: JSON.stringify({ jsonrpc: '2.0', id: `a666-ux-verify-${++rpcId}`, method, params }),
  });
  const payload = await response.json();
  if (payload.error) throw new Error(payload.error.message || `Ethereum RPC ${method} failed`);
  return payload.result;
}

async function wrappedBalance() {
  const data = `0x70a08231${ethereumAddress.slice(2).padStart(64, '0')}`;
  return BigInt(await ethereumRequest('eth_call', [{ to: wrappedA666, data }, 'latest']));
}

function formatA666(atoms) {
  const whole = atoms / 1_000_000n;
  const fraction = String(atoms % 1_000_000n).padStart(6, '0').replace(/0+$/, '');
  return fraction ? `${whole}.${fraction}` : String(whole);
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
  if (method === 'eth_sendTransaction') throw new Error('Balance verification must not request an Ethereum spend');
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
page.setDefaultTimeout(30_000);
const browserErrors = [];
page.on('pageerror', error => browserErrors.push(`pageerror: ${error.message}`));
page.on('console', message => {
  if (message.type() === 'error' && !message.text().includes('frame-ancestors')) {
    browserErrors.push(`console: ${message.text()}`);
  }
});

try {
  process.stderr.write('open wallet\n');
  await page.goto(walletUrl, { waitUntil: 'networkidle' });
  process.stderr.write('import wallet\n');
  await page.getByRole('button', { name: 'Import Wallet', exact: true }).click();
  await page.getByPlaceholder(/64 hex chars/).fill(backup.master_seed_hex);
  await page.getByRole('button', { name: 'Validate Seed', exact: true }).click();
  const pftlAddress = String(await page.locator('text=/^pf[0-9a-f]{40}$/').first().textContent()).trim();
  if (pftlAddress !== 'pfab9b9228942e5c529633a13aa271d5297bec6353') {
    throw new Error(`backup derived unexpected PFTL address ${pftlAddress}`);
  }
  const passphrase = 'a666-live-balance-verify-20260731';
  await page.getByPlaceholder('Encryption passphrase (min 10 chars)').fill(passphrase);
  await page.getByPlaceholder('Confirm passphrase').fill(passphrase);
  await page.getByRole('button', { name: 'Confirm Import', exact: true }).click();
  await page.getByText(/height [1-9]\d*/).first().waitFor({ state: 'visible' });

  process.stderr.write('open A666 market\n');
  await page.locator('.pf-sidebar .pf-nav', { hasText: 'NAV Markets' }).click();
  await page.getByText('Live route · invariant holds', { exact: true }).waitFor({ state: 'visible' });
  process.stderr.write('connect MetaMask\n');
  await page.getByRole('button', { name: 'Connect MetaMask', exact: true }).click();
  const balance = await wrappedBalance();
  if (balance < minimumBalanceAtoms) {
    throw new Error(`MetaMask wA666 balance ${balance} is below required ${minimumBalanceAtoms}`);
  }
  const displayedBalance = formatA666(balance);
  const balanceCard = page.locator('.a666-balance', { hasText: 'wA666 · MetaMask' });
  await balanceCard.getByText(displayedBalance, { exact: true }).waitFor({ state: 'visible' });
  const recipient = await page.locator('#a666-eth-recipient').inputValue();
  if (recipient !== ethereumAddress) throw new Error('wallet displayed the wrong MetaMask recipient');

  process.stderr.write('capture verified balance\n');
  await page.screenshot({ path: `${evidenceDir}/wa666-held-in-metamask.png`, fullPage: true });
  const materialErrors = browserErrors.filter(value => !value.includes('Failed to load resource:'));
  if (materialErrors.length) throw new Error(`browser errors: ${materialErrors.join(' | ')}`);
  const result = {
    ok: true,
    schema: 'postfiat.wallet.live_a666_metamask_balance_ux.v1',
    pftl_address: pftlAddress,
    ethereum_address: ethereumAddress,
    wrapped_token: wrappedA666,
    wrapped_balance_atoms: balance.toString(),
    displayed_balance: displayedBalance,
    ethereum_transaction_requested: false,
  };
  await writeFile(`${evidenceDir}/result.json`, `${JSON.stringify(result, null, 2)}\n`, { mode: 0o600 });
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
} catch (error) {
  await page.screenshot({ path: `${evidenceDir}/FAILURE.png`, fullPage: true }).catch(() => {});
  throw error;
} finally {
  await browser.close();
}
