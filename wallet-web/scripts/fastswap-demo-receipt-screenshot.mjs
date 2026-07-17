import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import process from 'node:process';

const evidenceDir = process.env.FASTSWAP_DEMO_SCREENSHOTS;
if (!evidenceDir) throw new Error('FASTSWAP_DEMO_SCREENSHOTS is required');
await mkdir(evidenceDir, { recursive: true });
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 1100 }, colorScheme: 'dark' });
try {
  await page.goto('http://127.0.0.1:5180/?receipt=latest', { waitUntil: 'networkidle', timeout: 60_000 });
  await page.getByRole('button', { name: 'Create Wallet' }).click();
  await page.getByLabel('I have saved my seed in a secure location').check();
  await page.getByPlaceholder('Encryption passphrase (min 10 chars)').fill('fastswap-demo-only-2026');
  await page.getByPlaceholder('Confirm passphrase').fill('fastswap-demo-only-2026');
  await page.getByRole('button', { name: 'Create Wallet' }).click();
  await page.getByRole('button', { name: 'Buy a651' }).first().click();
  const terminal = page.getByTestId('fastswap-terminal');
  await terminal.waitFor({ state: 'visible', timeout: 90_000 });
  const text = await terminal.innerText();
  if (!text.includes('ACCEPTED') || !text.includes('Applied on all 6 validators')) {
    throw new Error(`latest terminal receipt is not accepted: ${text}`);
  }
  await page.addStyleTag({ content: `
    .pf-root,.pf-shell,.pf-main{height:auto!important;min-height:100vh!important}
    .fs-page{overflow:visible!important;min-height:auto!important}
  ` });
  await page.locator('.fs-page').evaluate((element) => { element.scrollTop = 0; });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${evidenceDir}/02-accepted-fastswap-receipt.png`, fullPage: true });
  console.log(JSON.stringify({ gui_terminal: 'accepted', screenshot: `${evidenceDir}/02-accepted-fastswap-receipt.png` }));
} finally {
  await browser.close();
}
