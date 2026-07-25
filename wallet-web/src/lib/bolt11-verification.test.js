import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import test from 'node:test';
import bolt11 from '@atomiqlabs/bolt11';

import { verifyMainnetBolt11Invoice } from './bolt11-verification.js';

const PREIMAGE = '42'.repeat(32);
const PAYMENT_HASH = createHash('sha256')
  .update(Buffer.from(PREIMAGE, 'hex'))
  .digest('hex');
const TIMESTAMP = 2_000_000_000;
const EXPIRY_SECONDS = 900;
const AMOUNT_MSAT = '1000000';
const MIN_FINAL_CLTV = 144;
const PRIVATE_KEY = '00'.repeat(31) + '01';

function invoiceFixture({
  privateKey = PRIVATE_KEY,
  amountMsat = AMOUNT_MSAT,
  paymentHash = PAYMENT_HASH,
  expirySeconds = EXPIRY_SECONDS,
  minFinalCltv = MIN_FINAL_CLTV,
  extraFeatureBits = [],
} = {}) {
  const encoded = bolt11.encode({
    millisatoshis: amountMsat,
    timestamp: TIMESTAMP,
    tags: [
      { tagName: 'payment_hash', data: paymentHash },
      { tagName: 'payment_secret', data: 'cd'.repeat(32) },
      { tagName: 'description', data: 'PostFiat CONTROLLED NAVcoin test' },
      { tagName: 'expire_time', data: expirySeconds },
      { tagName: 'min_final_cltv_expiry', data: minFinalCltv },
      {
        tagName: 'feature_bits',
        data: {
          word_length: 7,
          var_onion_optin: { supported: true, required: false },
          payment_secret: { supported: true, required: false },
          basic_mpp: { supported: true, required: false },
          extra_bits: {
            start_bit: 20,
            bits: extraFeatureBits,
          },
        },
      },
    ],
  }, false);
  return bolt11.sign(encoded, privateKey);
}

function expected(fixture) {
  return {
    paymentHash: PAYMENT_HASH,
    amountMsat: AMOUNT_MSAT,
    payee: fixture.payeeNodeKey,
    expiryUnix: TIMESTAMP + EXPIRY_SECONDS,
    minFinalCltvDelta: MIN_FINAL_CLTV,
  };
}

test('browser independently verifies signed mainnet BOLT11 value fields', () => {
  const fixture = invoiceFixture();
  const verified = verifyMainnetBolt11Invoice(
    fixture.paymentRequest,
    expected(fixture),
  );
  assert.equal(verified.paymentHash, PAYMENT_HASH);
  assert.equal(verified.amountMsat, AMOUNT_MSAT);
  assert.equal(verified.payee, fixture.payeeNodeKey);
  assert.equal(verified.expiryUnix, TIMESTAMP + EXPIRY_SECONDS);
  assert.equal(verified.minFinalCltvDelta, MIN_FINAL_CLTV);
  assert.match(verified.signature, /^[0-9a-f]{128}$/);
});

test('browser rejects every value-bearing BOLT11 mismatch', () => {
  const fixture = invoiceFixture();
  const baseline = expected(fixture);
  const mismatches = [
    [{ ...baseline, paymentHash: 'ff'.repeat(32) }, /payment hash/],
    [{ ...baseline, amountMsat: '1001000' }, /amount/],
    [{ ...baseline, payee: `03${'11'.repeat(32)}` }, /recovered payee/],
    [{ ...baseline, expiryUnix: baseline.expiryUnix + 1 }, /expiry/],
    [{ ...baseline, minFinalCltvDelta: MIN_FINAL_CLTV + 1 }, /CLTV/],
  ];
  for (const [changed, pattern] of mismatches) {
    assert.throws(
      () => verifyMainnetBolt11Invoice(fixture.paymentRequest, changed),
      pattern,
    );
  }
});

test('browser rejects a differently signed invoice and AMP feature bits', () => {
  const fixture = invoiceFixture();
  const otherSigner = invoiceFixture({
    privateKey: '00'.repeat(31) + '02',
  });
  assert.throws(
    () => verifyMainnetBolt11Invoice(
      otherSigner.paymentRequest,
      expected(fixture),
    ),
    /recovered payee/,
  );

  const ampOptionalBits = new Array(12).fill(false);
  ampOptionalBits[11] = true;
  const amp = invoiceFixture({ extraFeatureBits: ampOptionalBits });
  assert.throws(
    () => verifyMainnetBolt11Invoice(amp.paymentRequest, expected(amp)),
    /AMP/,
  );
});

test('browser rejects mixed-case, testnet, and checksum-corrupt invoices', () => {
  const fixture = invoiceFixture();
  assert.throws(
    () => verifyMainnetBolt11Invoice(
      fixture.paymentRequest.toUpperCase(),
      expected(fixture),
    ),
    /canonical lowercase/,
  );
  const testnet = bolt11.sign(
    bolt11.encode({
      network: {
        bech32: 'tb',
        pubKeyHash: 0x6f,
        scriptHash: 0xc4,
        validWitnessVersions: [0, 1],
        wif: 0xef,
      },
      millisatoshis: AMOUNT_MSAT,
      timestamp: TIMESTAMP,
      tags: [
        { tagName: 'payment_hash', data: PAYMENT_HASH },
        { tagName: 'description', data: 'testnet' },
      ],
    }),
    PRIVATE_KEY,
  );
  assert.throws(
    () => verifyMainnetBolt11Invoice(testnet.paymentRequest, expected(fixture)),
    /Bitcoin-mainnet/,
  );
  const last = fixture.paymentRequest.at(-1);
  const corrupted = `${fixture.paymentRequest.slice(0, -1)}${last === 'q' ? 'p' : 'q'}`;
  assert.throws(
    () => verifyMainnetBolt11Invoice(corrupted, expected(fixture)),
    /decode\/signature verification/,
  );
});
