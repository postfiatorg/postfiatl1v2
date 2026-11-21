import { launch, dump } from './lib.mjs';
const ctx = await launch(); await new Promise(r => setTimeout(r, 3000));
const page = await ctx.newPage();
await page.goto('http://127.0.0.1:5199/phantom-live?recipient=pf5c266a1bdb18320e140961411a8c4d8383eaafb2');
await page.waitForTimeout(9000);
console.log((await page.locator('body').innerText()).replace(/\n+/g, ' | ').slice(250, 1100));
await page.screenshot({ path: '/tmp/phantom-test/credited.png', fullPage: true });
await ctx.close();
