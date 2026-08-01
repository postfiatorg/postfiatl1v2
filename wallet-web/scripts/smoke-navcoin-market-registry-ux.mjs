import { chromium } from 'playwright';

const walletUrl = process.env.WALLET_WEB_URL || 'https://127.0.0.1:5173';
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ ignoreHTTPSErrors: true, viewport: { width: 1440, height: 1100 } });
const page = await context.newPage();
page.setDefaultTimeout(30_000);
const browserErrors = [];
page.on('pageerror', error => browserErrors.push(error.message));

try {
  await page.goto(walletUrl, { waitUntil: 'networkidle' });
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();
  await page.getByRole('checkbox', { name: /saved my seed/i }).check();
  const passphrase = 'navcoin-registry-smoke-only';
  await page.getByPlaceholder('Encryption passphrase (min 10 chars)').fill(passphrase);
  await page.getByPlaceholder('Confirm passphrase').fill(passphrase);
  await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();

  const sidebar = page.locator('.pf-sidebar');
  await sidebar.getByRole('button', { name: /NAV Markets/ }).waitFor({ state: 'visible' });
  if (await sidebar.getByText('A666 Market', { exact: true }).count()) {
    throw new Error('product navigation still exposes an A666-specific market');
  }
  await sidebar.getByRole('button', { name: /NAV Markets/ }).click();

  const market = page.getByTestId('navcoin-market');
  await market.waitFor({ state: 'visible' });
  await market.getByText('NAVCOIN PRIMARY MARKET · PFTL', { exact: true }).waitFor({ state: 'visible' });
  const selector = market.locator('#navcoin-market-select');
  const configuredMarkets = await selector.locator('option').count();
  if (configuredMarkets < 1) throw new Error('NAVCoin market registry is empty');
  if (await selector.inputValue() === 'a666') throw new Error('market selection is keyed by a product symbol instead of a route identity');

  await market.getByRole('button', { name: 'Redeem', exact: true }).click();
  await market.getByRole('button', { name: 'From MetaMask', exact: true }).waitFor({ state: 'visible' });
  await market.getByRole('button', { name: 'From MetaMask', exact: true }).click();
  await market.getByText(/Return wA666 trustlessly to PFTL/).waitFor({ state: 'visible' });
  await market.getByRole('button', { name: /Return & redeem/ }).waitFor({ state: 'visible' });

  await sidebar.getByRole('button', { name: /NavCoins/ }).click();
  await page.getByText('Each NAVCoin has its own governed route', { exact: false }).waitFor({ state: 'visible' });
  if (browserErrors.length) throw new Error(`browser errors: ${browserErrors.join(' | ')}`);

  process.stdout.write(`${JSON.stringify({
    ok: true,
    schema: 'postfiat.wallet.navcoin_market_registry_ux.v1',
    wallet_url: walletUrl,
    configured_markets: configuredMarkets,
  }, null, 2)}\n`);
} finally {
  await browser.close();
}
