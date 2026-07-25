import assert from 'node:assert/strict';
import { webcrypto } from 'node:crypto';
import test from 'node:test';

import {
  canonicalQuoteBytes,
  verifySignedQuoteEnvelope,
} from './lightning-quote-signature.js';

const encoder = new TextEncoder();
const DOMAIN = encoder.encode('postfiat.lightning_submarine_quote.v1\u0000');
const KEY_DOMAIN = encoder.encode('postfiat.lightning_submarine_quote.key.v1\u0000');

function join(...values) {
  const result = new Uint8Array(values.reduce((total, value) => total + value.length, 0));
  let offset = 0;
  for (const value of values) {
    result.set(value, offset);
    offset += value.length;
  }
  return result;
}

function b64url(value) {
  return Buffer.from(value).toString('base64url');
}

test('browser verifies the exact pinned Ed25519 quote envelope', async () => {
  const keys = await webcrypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']);
  const publicKey = new Uint8Array(await webcrypto.subtle.exportKey('raw', keys.publicKey));
  const quote = { z: ['ascii', 2], a: { y: true, x: 'value' } };
  const quoteBytes = canonicalQuoteBytes(quote);
  assert.equal(new TextDecoder().decode(quoteBytes), '{"a":{"x":"value","y":true},"z":["ascii",2]}');
  const length = new Uint8Array(4);
  new DataView(length.buffer).setUint32(0, quoteBytes.length, false);
  const signature = new Uint8Array(await webcrypto.subtle.sign(
    'Ed25519',
    keys.privateKey,
    join(DOMAIN, length, quoteBytes),
  ));
  const keyId = Buffer.from(await webcrypto.subtle.digest(
    'SHA-256',
    join(KEY_DOMAIN, publicKey),
  )).toString('hex');
  const envelope = {
    algorithm: 'Ed25519',
    key_id: keyId,
    public_key: b64url(publicKey),
    quote,
    signature: b64url(signature),
  };
  assert.deepEqual(
    await verifySignedQuoteEnvelope(
      envelope,
      Buffer.from(publicKey).toString('hex'),
      webcrypto.subtle,
    ),
    quote,
  );

  const tampered = structuredClone(envelope);
  tampered.quote.a.x = 'changed';
  await assert.rejects(
    () => verifySignedQuoteEnvelope(
      tampered,
      Buffer.from(publicKey).toString('hex'),
      webcrypto.subtle,
    ),
    /signature is invalid/,
  );
});
