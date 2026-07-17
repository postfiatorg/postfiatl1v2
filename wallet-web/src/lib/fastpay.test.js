import assert from 'node:assert/strict';
import test from 'node:test';

import {
  looksLikePublicKeyHex,
  resolveFastpayRecipientPublicKey,
} from './fastpay.js';

const OWN_ADDRESS = 'pf1111111111111111111111111111111111111111';
const OTHER_ADDRESS = 'pf2222222222222222222222222222222222222222';
const OWN_PUBLIC_KEY = 'a'.repeat(3904);
const OTHER_PUBLIC_KEY = 'b'.repeat(3904);

test('looksLikePublicKeyHex accepts long even hex public keys', () => {
  assert.equal(looksLikePublicKeyHex(OWN_PUBLIC_KEY), true);
});

test('looksLikePublicKeyHex rejects account addresses', () => {
  assert.equal(looksLikePublicKeyHex(OWN_ADDRESS), false);
});

test('resolveFastpayRecipientPublicKey accepts a pasted public key', async () => {
  const resolved = await resolveFastpayRecipientPublicKey({
    rpc: null,
    recipient: OTHER_PUBLIC_KEY.toUpperCase(),
    ownAddress: OWN_ADDRESS,
    ownPublicKeyHex: OWN_PUBLIC_KEY,
  });

  assert.equal(resolved, OTHER_PUBLIC_KEY);
});

test('resolveFastpayRecipientPublicKey maps the wallet address to its local public key', async () => {
  const resolved = await resolveFastpayRecipientPublicKey({
    rpc: null,
    recipient: OWN_ADDRESS,
    ownAddress: OWN_ADDRESS,
    ownPublicKeyHex: OWN_PUBLIC_KEY,
  });

  assert.equal(resolved, OWN_PUBLIC_KEY);
});

test('resolveFastpayRecipientPublicKey reads a published public key from account RPC', async () => {
  const rpc = {
    async account(address) {
      assert.equal(address, OTHER_ADDRESS);
      return {
        ok: true,
        result: {
          address,
          balance: 0,
          sequence: 0,
          public_key_hex: OTHER_PUBLIC_KEY,
        },
      };
    },
  };

  const resolved = await resolveFastpayRecipientPublicKey({
    rpc,
    recipient: OTHER_ADDRESS,
    ownAddress: OWN_ADDRESS,
    ownPublicKeyHex: OWN_PUBLIC_KEY,
  });

  assert.equal(resolved, OTHER_PUBLIC_KEY);
});

test('resolveFastpayRecipientPublicKey rejects unpublished account public keys', async () => {
  const rpc = {
    async account(address) {
      return {
        ok: true,
        result: { address, balance: 0, sequence: 0, public_key_hex: null },
      };
    },
  };

  await assert.rejects(
    () => resolveFastpayRecipientPublicKey({
      rpc,
      recipient: OTHER_ADDRESS,
      ownAddress: OWN_ADDRESS,
      ownPublicKeyHex: OWN_PUBLIC_KEY,
    }),
    /has not published a public key/,
  );
});
