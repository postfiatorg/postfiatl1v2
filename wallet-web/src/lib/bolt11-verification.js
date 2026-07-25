import bolt11 from '@atomiqlabs/bolt11';

const MAINNET_INVOICE = /^lnbc(?:[0-9]+[munp]?)?1[02-9ac-hj-np-z]+$/;
const HEX_32 = /^[0-9a-f]{64}$/;
const COMPRESSED_SECP256K1_KEY = /^(02|03)[0-9a-f]{64}$/;
const MAX_BOLT11_CHARS = 8192;

function exactTag(decoded, name) {
  const values = decoded.tags.filter(tag => tag?.tagName === name);
  if (values.length !== 1) {
    throw new Error(`BOLT11 invoice must contain exactly one ${name} tag`);
  }
  return values[0].data;
}

function canonicalInvoice(value) {
  if (
    typeof value !== 'string'
    || value.length === 0
    || value.length > MAX_BOLT11_CHARS
    || value.toLowerCase() !== value
    || !MAINNET_INVOICE.test(value)
  ) {
    throw new Error(
      'invoice must be a canonical lowercase Bitcoin-mainnet BOLT11 invoice',
    );
  }
  return value;
}

function decimalInteger(value, label) {
  const text = String(value ?? '');
  if (!/^(0|[1-9][0-9]*)$/.test(text)) {
    throw new Error(`${label} must be a canonical decimal integer`);
  }
  return text;
}

function exactInteger(value, label, minimum = 0) {
  if (!Number.isSafeInteger(value) || value < minimum) {
    throw new Error(`${label} must be a safe integer of at least ${minimum}`);
  }
  return value;
}

function hash32(value, label) {
  if (typeof value !== 'string' || !HEX_32.test(value)) {
    throw new Error(`${label} must be canonical lowercase 32-byte hex`);
  }
  return value;
}

function compressedKey(value, label) {
  if (typeof value !== 'string' || !COMPRESSED_SECP256K1_KEY.test(value)) {
    throw new Error(`${label} must be a compressed secp256k1 public key`);
  }
  return value;
}

/**
 * Decode and cryptographically bind a BOLT11 invoice in the browser.
 *
 * The decoder validates Bech32 and recovers the signing key from the compact
 * secp256k1 signature. We then bind every value-bearing field to the signed
 * coordinator quote. The coordinator's DecodePayReq check remains a second,
 * independent check; it is not trusted as the browser's decoder.
 */
export function verifyMainnetBolt11Invoice(invoiceValue, expected) {
  const invoice = canonicalInvoice(invoiceValue);
  let decoded;
  try {
    decoded = bolt11.decode(invoice);
  } catch (cause) {
    throw new Error(`BOLT11 decode/signature verification failed: ${cause.message}`);
  }
  if (
    decoded.complete !== true
    || decoded.paymentRequest !== invoice
    || decoded.network?.bech32 !== 'bc'
    || !decoded.prefix?.startsWith('lnbc')
    || !/^[0-9a-f]{128}$/.test(decoded.signature || '')
    || ![0, 1, 2, 3].includes(decoded.recoveryFlag)
  ) {
    throw new Error('BOLT11 invoice is not a complete signed Bitcoin-mainnet request');
  }

  const expectedHash = hash32(expected?.paymentHash, 'expected payment hash');
  const expectedAmount = decimalInteger(
    expected?.amountMsat,
    'expected invoice amount_msat',
  );
  const expectedPayee = compressedKey(expected?.payee, 'expected invoice payee');
  const expectedExpiry = exactInteger(
    expected?.expiryUnix,
    'expected invoice expiry',
    1,
  );
  const expectedCltv = exactInteger(
    expected?.minFinalCltvDelta,
    'expected minimum final CLTV delta',
    1,
  );

  const paymentHash = hash32(
    exactTag(decoded, 'payment_hash'),
    'decoded payment hash',
  );
  const paymentSecret = hash32(
    exactTag(decoded, 'payment_secret'),
    'decoded payment secret',
  );
  const expirySeconds = exactInteger(
    exactTag(decoded, 'expire_time'),
    'decoded expiry duration',
    1,
  );
  const minFinalCltvDelta = exactInteger(
    exactTag(decoded, 'min_final_cltv_expiry'),
    'decoded minimum final CLTV delta',
    1,
  );
  const features = exactTag(decoded, 'feature_bits');
  const payee = compressedKey(decoded.payeeNodeKey, 'recovered invoice payee');
  const amountMsat = decimalInteger(decoded.millisatoshis, 'decoded invoice amount_msat');
  const expiryUnix = exactInteger(
    decoded.timestamp + expirySeconds,
    'decoded invoice expiry',
    1,
  );

  if (
    features?.payment_secret?.supported !== true
    && features?.payment_secret?.required !== true
  ) {
    throw new Error('BOLT11 payment-secret feature is absent');
  }
  const extraBits = features?.extra_bits;
  if (!extraBits || extraBits.start_bit !== 20 || !Array.isArray(extraBits.bits)) {
    throw new Error('BOLT11 feature bits are malformed');
  }
  // BOLT feature bits 30/31 are AMP required/optional. The legacy decoder
  // exposes bits >=20 as a zero-indexed array, so AMP is positions 10 and 11.
  if (extraBits.bits[10] === true || extraBits.bits[11] === true) {
    throw new Error('AMP BOLT11 invoices are unsupported');
  }
  if (extraBits.has_required === true) {
    throw new Error('BOLT11 invoice requires an unsupported feature bit');
  }

  if (paymentHash !== expectedHash) {
    throw new Error('BOLT11 payment hash does not match the signed quote');
  }
  if (amountMsat !== expectedAmount) {
    throw new Error('BOLT11 amount does not match the signed quote');
  }
  if (payee !== expectedPayee) {
    throw new Error('BOLT11 recovered payee does not match the signed quote');
  }
  if (expiryUnix !== expectedExpiry || decoded.timeExpireDate !== expectedExpiry) {
    throw new Error('BOLT11 expiry does not match the signed quote');
  }
  if (minFinalCltvDelta !== expectedCltv) {
    throw new Error('BOLT11 final CLTV delta does not match the signed quote');
  }

  return Object.freeze({
    invoice,
    paymentHash,
    paymentSecret,
    amountMsat,
    payee,
    timestampUnix: decoded.timestamp,
    expirySeconds,
    expiryUnix,
    minFinalCltvDelta,
    signature: decoded.signature,
    recoveryFlag: decoded.recoveryFlag,
  });
}
