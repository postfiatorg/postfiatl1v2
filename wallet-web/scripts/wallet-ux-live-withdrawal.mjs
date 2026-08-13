import { chromium } from 'playwright';
import { access, mkdir, readFile, writeFile } from 'node:fs/promises';

const [origin, profileDir, passphraseFile, evidenceDir, ethereumAccount, expectedPftlAddress, recoveryBackupFile] = process.argv.slice(2);
if (!origin || !profileDir || !passphraseFile || !evidenceDir
  || !/^0x[0-9a-fA-F]{40}$/.test(ethereumAccount || '')
  || !/^pf[0-9a-f]{40}$/.test(expectedPftlAddress || '')) {
  throw new Error('usage: wallet-ux-live-withdrawal <origin> <profile> <passphrase-file> <evidence-dir> <ethereum-account> <expected-pftl-address> [recovery-backup-file]');
}
const marker = `${evidenceDir}/live-withdrawal-0.1-usdc.json`;
try {
  await access(marker);
  throw new Error(`refusing to repeat completed live withdrawal: ${marker}`);
} catch (error) {
  if (error.code !== 'ENOENT') throw error;
}
const passphrase = (await readFile(passphraseFile, 'utf8')).trim();
if (!passphrase) throw new Error('wallet passphrase file is empty');
let recoverySeed = '';
if (recoveryBackupFile) {
  const recovery = JSON.parse(await readFile(recoveryBackupFile, 'utf8'));
  recoverySeed = String(recovery?.master_seed_hex || '').toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(recoverySeed)) throw new Error('recovery backup is missing a valid master seed');
}
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
const [ethereumWei, usdcBefore, wa666Atoms] = await Promise.all([
  ethereum('eth_getBalance', [ethereumAccount, 'latest']),
  ethereum('eth_call', [{ to: USDC, data: balanceOfData(ethereumAccount) }, 'latest']),
  ethereum('eth_call', [{ to: WA666, data: balanceOfData(ethereumAccount) }, 'latest']),
]);

const context = await chromium.launchPersistentContext(profileDir, {
  headless: true,
  viewport: { width: 1440, height: 1100 },
});
try {
  await context.addInitScript(({ account, ethereumWei, usdcBefore, wa666Atoms }) => {
    const balances = {
      '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48': usdcBefore,
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
        throw new Error(`live withdrawal provider refuses unsupported method ${method}`);
      },
    };
  }, { account: ethereumAccount.toLowerCase(), ethereumWei, usdcBefore, wa666Atoms });

  const page = context.pages()[0] || await context.newPage();
  page.setDefaultTimeout(60_000);
  await page.goto(origin, { waitUntil: 'domcontentloaded' });
  const unlockHeading = page.getByText('Unlock this wallet', { exact: true });
  const restoreSeedButton = page.getByRole('button', { name: 'Restore from recovery seed', exact: true });
  await Promise.race([unlockHeading.waitFor(), restoreSeedButton.waitFor()]);
  if (await unlockHeading.isVisible().catch(() => false)) {
    await page.getByText(expectedPftlAddress, { exact: true }).waitFor();
    await page.locator('input[placeholder="Passphrase"]').fill(passphrase);
    await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  } else if (await restoreSeedButton.isVisible().catch(() => false)) {
    if (!recoverySeed) throw new Error('this browser profile needs the recovery backup');
    await restoreSeedButton.click();
    await page.getByPlaceholder('64-character recovery seed').fill(recoverySeed);
    await page.getByRole('button', { name: 'Continue and preview address', exact: true }).click();
    await page.getByText(expectedPftlAddress, { exact: true }).waitFor();
    await page.getByPlaceholder('Encryption passphrase (min 10 chars)').fill(passphrase);
    await page.getByPlaceholder('Confirm passphrase').fill(passphrase);
    await page.getByRole('button', { name: 'Restore this wallet', exact: true }).click();
  }
  await page.locator('.pf-shell').waitFor();
  await page.locator('button').filter({ hasText: /^Bridge$/ }).first().click();
  await page.getByRole('button', { name: 'Withdraw to Ethereum', exact: true }).click();
  // Job discovery and burn-before-job recovery are asynchronous. Never let a
  // quickly rendered reserve balance race this check into a duplicate burn.
  await page.waitForTimeout(8_000);
  const retry = page.getByRole('button', { name: 'Retry payout', exact: true });
  const running = page.getByText('Withdrawal in progress', { exact: true }).first();
  const ready = page.getByText(/USDC can be withdrawn now/);
  if (await retry.isVisible().catch(() => false)) {
    await retry.click();
  } else if (!(await running.isVisible().catch(() => false))) {
    await ready.waitFor();
    const amount = page.getByLabel('Amount to withdraw');
    await amount.fill('0.1');
    await page.getByRole('button', { name: 'Review withdrawal', exact: true }).click();
    await page.getByRole('button', { name: 'Confirm and withdraw', exact: true }).waitFor();
    await page.screenshot({ path: `${evidenceDir}/22-live-withdrawal-review.png`, fullPage: true });
    await page.getByRole('button', { name: 'Confirm and withdraw', exact: true }).click();
  }
  page.setDefaultTimeout(3_600_000);
  await page.getByText('USDC received on Ethereum', { exact: true }).waitFor();
  await page.screenshot({ path: `${evidenceDir}/23-live-withdrawal-complete.png`, fullPage: true });
  const usdcAfter = await ethereum('eth_call', [{ to: USDC, data: balanceOfData(ethereumAccount) }, 'latest']);
  const usdcDeltaAtoms = BigInt(usdcAfter) - BigInt(usdcBefore);
  if (usdcDeltaAtoms !== 100_000n) {
    throw new Error(`live withdrawal Ethereum USDC delta was ${usdcDeltaAtoms}, expected 100000 atoms`);
  }
  const result = {
    schema: 'postfiat.wallet_ux_live_withdrawal.v1',
    verdict: 'PASS',
    amount_usdc: '0.100000',
    ethereum_usdc_before_atoms: BigInt(usdcBefore).toString(),
    ethereum_usdc_after_atoms: BigInt(usdcAfter).toString(),
    ethereum_usdc_delta_atoms: usdcDeltaAtoms.toString(),
    pftl_signatures: 1,
    ethereum_recipient: ethereumAccount.toLowerCase(),
    completed_at: new Date().toISOString(),
  };
  await writeFile(marker, `${JSON.stringify(result, null, 2)}\n`, { encoding: 'utf8', mode: 0o600, flag: 'wx' });
  console.log(JSON.stringify({ ok: true, amount_usdc: result.amount_usdc, pftl_signatures: 1 }));
} finally {
  await context.close();
}
