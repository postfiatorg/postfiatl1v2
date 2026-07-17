import { chromium } from 'playwright';


const walletWebUrl = process.env.WALLET_WEB_URL;
const swapServerUrl = process.env.SWAP_SERVER_URL;
const runCount = Number.parseInt(process.env.RUN_COUNT || '2', 10);

if (!walletWebUrl || !swapServerUrl) {
  throw new Error('WALLET_WEB_URL and SWAP_SERVER_URL are required');
}
if (!Number.isSafeInteger(runCount) || runCount < 1 || runCount > 10) {
  throw new Error('RUN_COUNT must be an integer between 1 and 10');
}

const browser = await chromium.launch({ headless: true });
const outcomes = [];
const progress = (attempt, stage) => process.stderr.write(`attempt ${attempt}: ${stage}\n`);
try {
  for (let attempt = 1; attempt <= runCount; attempt += 1) {
    const context = await browser.newContext();
    const page = await context.newPage();
    page.setDefaultTimeout(300_000);

    progress(attempt, 'opening fresh browser profile');
    await page.goto(walletWebUrl, { waitUntil: 'domcontentloaded' });
    progress(attempt, 'generating wallet');
    await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();
    const address = await page.locator('text=/^pf[0-9a-f]{40}$/').first().textContent();
    const passphrase = `acceptance-${crypto.randomUUID()}`;
    await page.getByRole('checkbox').check();
    await page.getByPlaceholder('Encryption passphrase (min 10 chars)').fill(passphrase);
    await page.getByPlaceholder('Confirm passphrase').fill(passphrase);
    await page.getByRole('button', { name: 'Create Wallet', exact: true }).click();

    progress(attempt, 'configuring certified backend');
    await page.locator('.pf-sidebar .pf-nav', { hasText: 'More' }).click();
    const swapField = page.getByText('Swap server', { exact: true }).locator('..').locator('input');
    await swapField.fill(swapServerUrl);
    await page.getByRole('button', { name: 'Save settings', exact: true }).click();
    await page.locator('.pf-sidebar .pf-nav', { hasText: 'Swap' }).click();

    progress(attempt, 'starting shipped UX workflow');
    const startedAt = Date.now();
    await page.getByRole('button', { name: 'Run verified private swap', exact: true }).click();
    const workflow = page.getByRole('region', { name: 'Certified private swap workflow' });
    await workflow.getByText('verified', { exact: true }).waitFor({ state: 'visible' });
    const verifiedSteps = await workflow.locator('.pf-swap-step.done').count();
    if (verifiedSteps !== 7) throw new Error(`expected 7 verified steps, got ${verifiedSteps}`);

    progress(attempt, 'seven steps verified');
    const details = await workflow.locator('.pfs-detail-list > div').allTextContents();
    outcomes.push({
      attempt,
      address,
      elapsed_s: Number(((Date.now() - startedAt) / 1000).toFixed(2)),
      verified_steps: verifiedSteps,
      details,
    });
    await context.close();
  }
} finally {
  await browser.close();
}

if (new Set(outcomes.map(outcome => outcome.address)).size !== outcomes.length) {
  throw new Error('acceptance runs did not use distinct fresh browser wallets');
}
process.stdout.write(`${JSON.stringify({ ok: true, outcomes }, null, 2)}\n`);
