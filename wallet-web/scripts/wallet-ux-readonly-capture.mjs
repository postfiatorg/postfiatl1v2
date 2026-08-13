import { chromium } from 'playwright';
import { mkdir, readFile } from 'node:fs/promises';

const [origin, profileDir, passphraseFile, evidenceDir, ethereumAccount] = process.argv.slice(2);
if (!origin || !profileDir || !passphraseFile || !evidenceDir
  || !/^0x[0-9a-fA-F]{40}$/.test(ethereumAccount || '')) {
  throw new Error('usage: wallet-ux-readonly-capture <origin> <profile> <passphrase-file> <evidence-dir> <ethereum-account>');
}
const passphrase = (await readFile(passphraseFile, 'utf8')).trim();
if (!passphrase) throw new Error('wallet passphrase file is empty');
await mkdir(evidenceDir, { recursive: true });

const ethereumRpc = process.env.WALLET_UX_ETHEREUM_RPC || 'https://ethereum-rpc.publicnode.com';
if (!/^https:\/\//.test(ethereumRpc)) throw new Error('Ethereum evidence RPC must use HTTPS');
const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
const WA666 = '0xee4c92edb03efdd9b519339edc19ad70c69a9be5';
let rpcId = 0;
async function ethereum(method, params) {
  const response = await fetch(ethereumRpc, {
    method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: ++rpcId, method, params }),
  });
  const body = await response.json();
  if (!response.ok || body.error || typeof body.result !== 'string') {
    throw new Error(`Ethereum evidence query failed for ${method}`);
  }
  return body.result;
}
const balanceOfData = account => `0x70a08231${account.toLowerCase().slice(2).padStart(64, '0')}`;
const [ethereumWei, usdcAtoms, wa666Atoms] = await Promise.all([
  ethereum('eth_getBalance', [ethereumAccount, 'latest']),
  ethereum('eth_call', [{ to: USDC, data: balanceOfData(ethereumAccount) }, 'latest']),
  ethereum('eth_call', [{ to: WA666, data: balanceOfData(ethereumAccount) }, 'latest']),
]);

const context = await chromium.launchPersistentContext(profileDir, {
  headless: true,
  viewport: { width: 1440, height: 1100 },
});
try {
  // Read-only EIP-1193 provider used to render the wallet's actual Ethereum
  // account state without authorizing a transaction or exposing a browser key.
  await context.addInitScript(({ account, ethereumWei, usdcAtoms, wa666Atoms }) => {
    const balances = {
      '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48': usdcAtoms,
      '0xee4c92edb03efdd9b519339edc19ad70c69a9be5': wa666Atoms,
    };
    window.ethereum = {
      isMetaMask: true,
      on() {},
      removeListener() {},
      request: async ({ method, params = [] }) => {
        if (method === 'eth_accounts' || method === 'eth_requestAccounts') return [account];
        if (method === 'eth_chainId') return '0x1';
        if (method === 'wallet_switchEthereumChain') return null;
        if (method === 'eth_getBalance') return ethereumWei;
        if (method === 'eth_call') return balances[String(params?.[0]?.to || '').toLowerCase()] || '0x0';
        throw new Error(`read-only capture provider does not support ${method}`);
      },
    };
  }, { account: ethereumAccount.toLowerCase(), ethereumWei, usdcAtoms, wa666Atoms });

  const page = context.pages()[0] || await context.newPage();
  page.setDefaultTimeout(30_000);
  await page.goto(origin, { waitUntil: 'domcontentloaded' });
  await Promise.race([
    page.getByText('Unlock this wallet', { exact: true }).waitFor(),
    page.locator('.pf-shell').waitFor(),
  ]);
  if (await page.getByText('Unlock this wallet', { exact: true }).isVisible().catch(() => false)) {
    await page.locator('input[placeholder="Passphrase"]').fill(passphrase);
    await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  }
  await page.locator('.pf-shell').waitFor();
  await page.waitForTimeout(12_000);

  const captures = [
    ['Home', '14-home-ethereum-holdings.png', 4_000],
    ['Assets', '18-assets-final.png', 3_000],
    ['Trade', '15-trade-live-price-vs-nav.png', 10_000],
    ['Activity', '17-unified-activity.png', 5_000],
    ['Settings', '19-settings-final.png', 3_000],
    ['Send', '20-send-final.png', 3_000],
  ];
  for (const [tab, filename, waitMs] of captures) {
    await page.locator('.pf-nav').filter({ hasText: tab }).click();
    await page.waitForTimeout(waitMs);
    if (tab === 'Trade') {
      await page.getByRole('button', { name: /Refreshing/ }).waitFor({ state: 'hidden', timeout: 30_000 }).catch(() => {});
    }
    if (tab === 'Activity') {
      await page.getByText('Loading wallet activity…', { exact: true }).waitFor({ state: 'hidden', timeout: 30_000 });
    }
    await page.screenshot({ path: `${evidenceDir}/${filename}`, fullPage: true });
  }

  await page.locator('.pf-nav').filter({ hasText: 'Bridge' }).click();
  await page.waitForTimeout(7_000);
  await page.getByRole('button', { name: 'Withdraw to Ethereum', exact: true }).click();
  await page.waitForTimeout(12_000);
  await page.screenshot({ path: `${evidenceDir}/16-bridge-withdraw.png`, fullPage: true });
  let screenshots = 7;
  if (await page.getByText('USDC received on Ethereum', { exact: true }).isVisible().catch(() => false)) {
    await page.screenshot({ path: `${evidenceDir}/23-live-withdrawal-complete.png`, fullPage: true });
    screenshots += 1;
  }
  const reviewButton = page.getByRole('button', { name: 'Review withdrawal', exact: true });
  if (await reviewButton.isEnabled().catch(() => false)) {
    await reviewButton.click();
    await page.getByText('PFTL network fee', { exact: true }).waitFor();
    await page.screenshot({ path: `${evidenceDir}/21-bridge-withdraw-review.png`, fullPage: true });
    screenshots += 1;
  }
  console.log(JSON.stringify({ ok: true, screenshots, transactions: 0 }));
} finally {
  await context.close();
}
