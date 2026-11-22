import { launch, dump, ID } from './lib.mjs';
import { readFileSync } from 'node:fs';
const B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
function b58(bytes) { let n = 0n; for (const b of bytes) n = n * 256n + BigInt(b); let s = ''; while (n > 0n) { s = B58[Number(n % 58n)] + s; n /= 58n; } for (const b of bytes) { if (b) break; s = '1' + s; } return s; }
const secret = b58(JSON.parse(readFileSync('/tmp/phantom-test/test-wallet.json', 'utf8')));
const ctx = await launch();
await new Promise(r => setTimeout(r, 5000));
const page = ctx.pages().find(p => p.url().includes('onboarding')) || await ctx.newPage();
if (!page.url().includes('onboarding')) await page.goto(`chrome-extension://${ID}/onboarding.html`);
page.setDefaultTimeout(8000);
const step = process.argv[2] || 'all';
try {
  await page.getByRole('button', { name: 'I Already Have a Wallet' }).click();
  await page.getByRole('button', { name: /Import Private Key/ }).click();
  await page.waitForTimeout(1500); await dump(page, 'o3');
  const inputs = page.locator('input,textarea');
  const n = await inputs.count();
  for (let i = 0; i < n; i++) {
    const el = inputs.nth(i), ph = (await el.getAttribute('placeholder')) || '', name = (await el.getAttribute('name')) || '';
    if (/name/i.test(ph + name)) await el.fill('Bridge test');
    else if (/private|key/i.test(ph + name)) await el.fill(secret);
  }
  await page.getByRole('button', { name: 'Import', exact: true }).click();
  await page.waitForTimeout(2500); await dump(page, 'o5');
  const pw = page.locator('input[type=password]');
  if (await pw.count()) {
    for (let i = 0; i < await pw.count(); i++) await pw.nth(i).fill(readFileSync('/tmp/phantom-test/profile-password', 'utf8').trimEnd());
    const box = page.locator('input[type=checkbox]');
    if (await box.count()) await box.first().check({ force: true }).catch(() => {});
    await page.waitForTimeout(500); await dump(page, 'o6');
    await page.getByRole('button', { name: /Continue|Next|Save/ }).first().click();
    await page.waitForTimeout(3000); await dump(page, 'o7');
  }
  for (let i = 0; i < 7; i++) {
    const user = page.locator('input[placeholder=username]');
    if (await user.count()) { await user.fill('pfb' + Math.random().toString(36).slice(2, 12)); await page.waitForTimeout(2500); }
    const next = page.getByRole('button', { name: /Continue|Next|Get Started|Finish|Skip|Not now/i }).first();
    if (!(await next.count())) break;
    await next.click().catch(() => {}); await page.waitForTimeout(2500); await dump(page, 'o8-' + i);
  }
} catch (e) { console.log('ERR', e.message.slice(0, 300)); await dump(page, 'oerr'); }
await ctx.close();
