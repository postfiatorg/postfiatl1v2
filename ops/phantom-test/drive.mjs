import { readFileSync } from 'node:fs';
import { launch, dump, ID } from './lib.mjs';
const amount = process.argv[2] || '0.02', recipient = 'pf5c266a1bdb18320e140961411a8c4d8383eaafb2';
const ctx = await launch();
await new Promise(r => setTimeout(r, 4000));
for (const p of ctx.pages()) if (p.url().includes('onboarding')) await p.close();
const page = await ctx.newPage(); page.setDefaultTimeout(15000);
const seen = new Set();
ctx.on('response', async r => {
  const u = r.url();
  if (u.includes('/api/l1/bridge/') && r.status() >= 400) { let b=''; try { b=(await r.text()).slice(0,500);}catch{} console.log('API', r.status(), u.split('/api/l1/bridge')[1], b); return; }
  if (!/simulat|rpc|sendTransaction|solana-mainnet|transaction/i.test(u) || seen.has(u.split('?')[0] + r.status())) return; seen.add(u.split('?')[0] + r.status());
  if (false || u.startsWith('chrome-extension') || u.includes('127.0.0.1')) return;
  let body = ''; try { body = (await r.text()).slice(0, 700); } catch {}
  let req = ''; try { req = (r.request().postData() || '').slice(0, 300); } catch {}
  console.log('NET', r.status(), u.slice(0, 140), '| req', req, '| resp', body);
});
const errors = []; page.on('pageerror', e => errors.push(e.message)); page.on('console', m => { if (m.type() === 'error') errors.push(m.text().slice(0, 200)); });
const popup = async (action, tag) => {
  const waiting = ctx.waitForEvent('page', { timeout: 20000 }).catch(() => null);
  await action();
  const p = await waiting;
  if (p) { await p.waitForLoadState('domcontentloaded').catch(() => {}); await p.waitForTimeout(3500); await dump(p, tag); }
  else console.log(`[${tag}] no Phantom window opened`);
  return p;
};
try {
  await page.goto(`http://127.0.0.1:5199/phantom-live?recipient=${recipient}`);
  await page.waitForTimeout(4000); await dump(page, 'd1');
  console.log('phantom injected', await page.evaluate(() => !!window.phantom?.solana?.isPhantom));
  const again = page.getByRole('button', { name: 'Start another Solana deposit' });
  if (await again.count()) { await again.click(); await page.waitForTimeout(1500); }
  let p = await popup(() => page.getByRole('button', { name: /Connect Phantom|Reconnect/ }).click(), 'p-connect');
  if (p) {
    const pw = p.locator('input[type=password]');
    if (await pw.count()) { await pw.fill(readFileSync('/tmp/phantom-test/profile-password', 'utf8').trimEnd()); await p.getByRole('button', { name: /Unlock/ }).click(); await p.waitForTimeout(3000); await dump(p, 'p-unlocked'); }
    const ok = p.getByRole('button', { name: /^(Connect|Approve|Confirm)$/ });
    if (await ok.count()) await ok.first().click();
  }
  await page.waitForTimeout(4000); await dump(page, 'd2');
  if (process.argv[3] === 'send') {
    for (let i = 0; i < 30; i++) {
      const again = page.getByRole('button', { name: 'Start another Solana deposit' });
      if (await again.count()) { await again.click(); await page.waitForTimeout(1500); break; }
      if (await page.getByRole('button', { name: 'Send with Phantom' }).count()) break;
      await page.waitForTimeout(3000);
    }
    await page.getByLabel('Solana USDC amount').fill(amount).catch(async () => { await page.locator('input[inputmode=decimal]').first().fill(amount); });
    await page.waitForTimeout(800);
    const t0 = Date.now();
    p = await popup(() => page.getByRole('button', { name: 'Send with Phantom' }).click(), 'p-send');
    if (p) {
      const pw = p.locator('input[type=password]');
      if (await pw.count()) { await pw.fill(readFileSync('/tmp/phantom-test/profile-password', 'utf8').trimEnd()); await p.getByRole('button', { name: /Unlock/ }).click(); await p.waitForTimeout(4000); await dump(p, 'p-send-unlocked'); }
      if (process.env.APPROVE_DELAY_MS) { console.log('holding the Phantom approval for', process.env.APPROVE_DELAY_MS, 'ms'); await p.waitForTimeout(Number(process.env.APPROVE_DELAY_MS)); }
      const ok = p.getByRole('button', { name: /^(Confirm|Approve|Send|Confirm \(unsafe\))$/ });
      if (await ok.count()) { console.log('Phantom button:', await ok.first().innerText()); await ok.first().click(); await p.waitForTimeout(1500); const sure = p.getByRole('button', { name: /Yes, confirm|Confirm anyway|Proceed|Continue/i }); if (!p.isClosed()) {
        const understand = p.getByText('I understand that I could lose all of my funds by proceeding.');
        if (await understand.count()) { await understand.click().catch(() => {}); await p.waitForTimeout(700); }
        const yes = p.getByText('Yes, confirm (unsafe)');
        if (await yes.count()) { await yes.first().click({ force: true }).catch(e => console.log('yes click', e.message.slice(0, 80))); console.log('pressed Yes, confirm (unsafe)'); await p.waitForTimeout(1500).catch(() => {}); }
      } console.log('approved in Phantom after', Date.now() - t0, 'ms'); }
      else console.log('no approve button in Phantom');
    }
    let last = '';
    for (let i = 0; i < 100; i++) {
      await page.waitForTimeout(2000);
      const text = (await page.locator('body').innerText()).replace(/\n+/g, ' | ');
      const line = (text.match(/Deposit progress.{0,520}/)?.[0] || text.slice(-260)).replace(/[0-9.]+ s \| /, '');
      if (line !== last) { console.log(`t+${Math.round((Date.now() - t0) / 1000)}s`, line.slice(0, 420)); last = line; }
      if (/Deposit credited|could not finish this deposit|was not sent|Deposit not recorded/.test(text)) break;
    }
    await dump(page, 'd-final');
  }
} catch (e) { console.log('ERR', e.message.slice(0, 300)); await dump(page, 'derr'); }
console.log('page errors', JSON.stringify(errors).slice(0, 600));
await ctx.close();
