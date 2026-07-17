import { chromium } from 'playwright';
import { mkdir } from 'node:fs/promises';
import process from 'node:process';

const baseUrl = process.env.FASTSWAP_DEMO_URL || 'http://127.0.0.1:5180';
const evidenceDir = process.env.FASTSWAP_DEMO_SCREENSHOTS;
if (!evidenceDir) throw new Error('FASTSWAP_DEMO_SCREENSHOTS is required');
await mkdir(evidenceDir, { recursive: true });

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({
  viewport: { width: 1440, height: 1100 },
  colorScheme: 'dark',
});
const page = await context.newPage();
page.on('pageerror', (error) => console.error(`browser page error: ${error.message}`));

try {
  await page.goto(baseUrl, { waitUntil: 'networkidle', timeout: 60_000 });
  await page.getByRole('button', { name: 'Create Wallet' }).click();
  await page.getByLabel('I have saved my seed in a secure location').check();
  await page.getByPlaceholder('Encryption passphrase (min 10 chars)').fill('fastswap-demo-only-2026');
  await page.getByPlaceholder('Confirm passphrase').fill('fastswap-demo-only-2026');
  await page.getByRole('button', { name: 'Create Wallet' }).click();
  await page.getByRole('button', { name: 'Buy a651' }).first().click();
  await page.getByTestId('fastswap-demo').waitFor({ state: 'visible', timeout: 30_000 });
  await page.getByText('VERIFIED · USABLE').waitFor({ state: 'visible', timeout: 90_000 });
  await page.getByRole('button', { name: 'REVIEW & CONFIRM LIVE SWAP' }).waitFor({ state: 'visible' });
  await page.screenshot({ path: `${evidenceDir}/01-verified-nav-review.png`, fullPage: true });

  await page.getByRole('button', { name: 'REVIEW & CONFIRM LIVE SWAP' }).click();
  await page.getByRole('dialog').waitFor({ state: 'visible' });
  await page.getByRole('button', { name: 'YES, EXECUTE LIVE SWAP' }).click();
  await page.getByTestId('fastswap-terminal').waitFor({ state: 'visible', timeout: 90_000 });
  const terminalText = await page.getByTestId('fastswap-terminal').innerText();
  if (!terminalText.includes('ACCEPTED') || !terminalText.includes('Applied on all 6 validators')) {
    throw new Error(`GUI did not render a proven accepted terminal state: ${terminalText}`);
  }
  await page.screenshot({ path: `${evidenceDir}/02-accepted-fastswap-receipt.png`, fullPage: true });
  console.log(JSON.stringify({
    browser: 'chromium',
    pre_submit_screenshot: `${evidenceDir}/01-verified-nav-review.png`,
    accepted_screenshot: `${evidenceDir}/02-accepted-fastswap-receipt.png`,
    gui_terminal: 'accepted',
  }));
} catch (error) {
  await page.screenshot({ path: `${evidenceDir}/FAILURE.png`, fullPage: true }).catch(() => {});
  throw error;
} finally {
  await browser.close();
}
