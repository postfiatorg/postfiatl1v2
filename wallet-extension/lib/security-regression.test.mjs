import assert from 'node:assert/strict';
import test from 'node:test';
import { webcrypto } from 'node:crypto';

import {
  KeyStore,
  MIN_PASSPHRASE_LENGTH,
  PBKDF2_ITERATIONS,
} from './keystore.js';
import { assertTransferQuoteMatchesIntent } from './tx-builder.js';

if (!globalThis.crypto) globalThis.crypto = webcrypto;
if (!globalThis.btoa) {
  globalThis.btoa = value => Buffer.from(value, 'binary').toString('base64');
  globalThis.atob = value => Buffer.from(value, 'base64').toString('binary');
}

test('extension transfer signing rejects an RPC-selected recipient or amount', () => {
  const quote = {
    from: 'pf-from',
    to: 'pf-recipient',
    amount: 25,
  };
  assert.doesNotThrow(() => {
    assertTransferQuoteMatchesIntent(quote, 'pf-from', 'pf-recipient', 25);
  });
  assert.throws(
    () => assertTransferQuoteMatchesIntent(
      { ...quote, to: 'pf-attacker', amount: 10_000 },
      'pf-from',
      'pf-recipient',
      25,
    ),
    /does not match the reviewed sender, recipient, and amount/,
  );
});

test('extension creates only strengthened vaults and retains legacy read compatibility', async () => {
  const store = new KeyStore();
  await assert.rejects(
    () => store.encrypt('00'.repeat(32), 'x'.repeat(MIN_PASSPHRASE_LENGTH - 1)),
    /at least 10 characters/,
  );

  const passphrase = 'correct horse battery staple';
  const seed = '12'.repeat(32);
  const current = await store.encrypt(seed, passphrase);
  assert.equal(current.iterations, PBKDF2_ITERATIONS);
  assert.equal(await store.decrypt(current, passphrase), seed);

  const enc = new TextEncoder();
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const keyMaterial = await crypto.subtle.importKey(
    'raw', enc.encode('old-pass'), 'PBKDF2', false, ['deriveKey'],
  );
  const legacyKey = await crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt, iterations: 100000, hash: 'SHA-256' },
    keyMaterial,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt'],
  );
  const ciphertext = new Uint8Array(await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv },
    legacyKey,
    enc.encode(seed),
  ));
  const legacy = {
    salt: btoa(String.fromCharCode(...salt)),
    iv: btoa(String.fromCharCode(...iv)),
    ciphertext: btoa(String.fromCharCode(...ciphertext)),
  };
  assert.equal(await store.decrypt(legacy, 'old-pass'), seed);
  await assert.rejects(
    () => store.decrypt({ ...legacy, iterations: 1 }, 'old-pass'),
    /Unsupported encrypted vault KDF parameters/,
  );
});
