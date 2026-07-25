import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import bolt11 from '@atomiqlabs/bolt11';
import { chromium } from 'playwright';
import { build } from 'vite';

const WALLET_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const PREIMAGE = '42'.repeat(32);
const PAYMENT_HASH = createHash('sha256')
  .update(Buffer.from(PREIMAGE, 'hex'))
  .digest('hex');
const TIMESTAMP = 2_000_000_000;

async function listen(server) {
  await new Promise((resolveListen, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolveListen);
  });
  return server.address().port;
}

async function close(server) {
  await new Promise(resolveClose => server.close(resolveClose));
}

test('production-bundled BOLT11 verifier runs inside Chromium', {
  timeout: 90_000,
}, async () => {
  const output = await mkdtemp(join(tmpdir(), 'pftl-bolt11-browser-'));
  let browser;
  let server;
  try {
    await build({
      root: WALLET_ROOT,
      configFile: join(WALLET_ROOT, 'vite.config.js'),
      logLevel: 'silent',
      build: {
        outDir: output,
        emptyOutDir: true,
        lib: {
          entry: join(WALLET_ROOT, 'src/lib/bolt11-verification.js'),
          formats: ['es'],
          fileName: () => 'verifier.js',
        },
      },
    });
    const bundle = await readFile(join(output, 'verifier.js'));
    assert.ok(bundle.length > 1_000);
    assert.doesNotMatch(bundle.toString('utf8'), /__vite-browser-external/);

    server = createServer(async (request, response) => {
      if (request.url === '/verifier.js') {
        response.writeHead(200, {
          'Content-Type': 'text/javascript',
          'Cache-Control': 'no-store',
        });
        response.end(bundle);
        return;
      }
      response.writeHead(200, {
        'Content-Type': 'text/html',
        'Cache-Control': 'no-store',
      });
      response.end('<!doctype html><title>BOLT11 verifier</title>');
    });
    const port = await listen(server);

    const encoded = bolt11.encode({
      millisatoshis: '1000000',
      timestamp: TIMESTAMP,
      tags: [
        { tagName: 'payment_hash', data: PAYMENT_HASH },
        { tagName: 'payment_secret', data: 'cd'.repeat(32) },
        { tagName: 'description', data: 'PostFiat CONTROLLED NAVcoin test' },
        { tagName: 'expire_time', data: 900 },
        { tagName: 'min_final_cltv_expiry', data: 144 },
        {
          tagName: 'feature_bits',
          data: {
            word_length: 4,
            var_onion_optin: { supported: true, required: false },
            payment_secret: { supported: true, required: false },
            basic_mpp: { supported: true, required: false },
          },
        },
      ],
    }, false);
    const fixture = bolt11.sign(encoded, '00'.repeat(31) + '01');
    const expected = {
      paymentHash: PAYMENT_HASH,
      amountMsat: '1000000',
      payee: fixture.payeeNodeKey,
      expiryUnix: TIMESTAMP + 900,
      minFinalCltvDelta: 144,
    };

    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${port}/`);
    const result = await page.evaluate(async ({ invoice, expectedFields }) => {
      const verifier = await import('/verifier.js');
      const verified = verifier.verifyMainnetBolt11Invoice(
        invoice,
        expectedFields,
      );
      let mismatch;
      try {
        verifier.verifyMainnetBolt11Invoice(invoice, {
          ...expectedFields,
          amountMsat: '1001000',
        });
        mismatch = 'accepted';
      } catch (error) {
        mismatch = error.message;
      }
      return {
        verified,
        mismatch,
        bufferType: typeof globalThis.Buffer,
      };
    }, {
      invoice: fixture.paymentRequest,
      expectedFields: expected,
    });
    assert.equal(result.verified.paymentHash, PAYMENT_HASH);
    assert.equal(result.verified.payee, fixture.payeeNodeKey);
    assert.match(result.mismatch, /amount/);
    assert.equal(
      result.bufferType,
      'undefined',
      'verifier must use its bundled Buffer and not mutate the page global',
    );
  } finally {
    if (browser) await browser.close();
    if (server) await close(server);
    await rm(output, { recursive: true, force: true });
  }
});
