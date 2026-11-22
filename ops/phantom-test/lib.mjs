export const { chromium } = await import('/home/postfiat/repos/postfiatl1v2-fastswap/wallet-web/node_modules/playwright/index.mjs');
export const ext = '/tmp/phantom-test/ext', ID = 'bfnaelmomeimhlpmgjnjophhpkkoljpa';
export const launch = () => chromium.launchPersistentContext('/tmp/phantom-test/profile', { headless: false,
  executablePath: process.env.HOME + '/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',
  args: ['--headless=new', '--no-sandbox', `--disable-extensions-except=${ext}`, `--load-extension=${ext}`], viewport: { width: 1280, height: 900 } });
export async function dump(page, tag) {
  await page.screenshot({ path: `/tmp/phantom-test/${tag}.png` }).catch(() => {});
  console.log(`[${tag}] url`, page.url().slice(0, 90));
  console.log(`[${tag}] buttons`, JSON.stringify(await page.locator('button').allInnerTexts().catch(() => [])).slice(0, 500));
  console.log(`[${tag}] inputs`, JSON.stringify(await page.locator('input,textarea').evaluateAll(els => els.map(e => [e.tagName, e.type, e.name, e.placeholder, e.getAttribute('data-testid')])).catch(() => [])).slice(0, 500));
  console.log(`[${tag}] text`, (await page.locator('body').innerText().catch(() => '')).replace(/\n+/g, ' | ').slice(0, 500));
}
