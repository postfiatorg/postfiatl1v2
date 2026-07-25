const SIGNATURE_DOMAIN = new TextEncoder().encode(
  'postfiat.lightning_submarine_quote.v1\u0000',
);
const KEY_ID_DOMAIN = new TextEncoder().encode(
  'postfiat.lightning_submarine_quote.key.v1\u0000',
);
const HEX_32 = /^[0-9a-f]{64}$/;
const B64URL = /^[A-Za-z0-9_-]+$/;

function bytesToHex(value) {
  return Array.from(value, byte => byte.toString(16).padStart(2, '0')).join('');
}

function decodeBase64Url(value, expectedBytes, label) {
  if (
    typeof value !== 'string'
    || !B64URL.test(value)
    || value.includes('=')
  ) {
    throw new Error(`${label} is not canonical unpadded base64url`);
  }
  const padding = '='.repeat((4 - (value.length % 4)) % 4);
  let decoded;
  try {
    const binary = globalThis.atob(`${value}${padding}`.replace(/-/g, '+').replace(/_/g, '/'));
    decoded = Uint8Array.from(binary, character => character.charCodeAt(0));
  } catch (_) {
    throw new Error(`${label} is not valid base64url`);
  }
  if (decoded.length !== expectedBytes) {
    throw new Error(`${label} has the wrong length`);
  }
  return decoded;
}

function canonicalize(value) {
  if (value === null || typeof value === 'boolean' || typeof value === 'string') return value;
  if (Number.isSafeInteger(value)) return value;
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value && typeof value === 'object') {
    const result = {};
    for (const key of Object.keys(value).sort()) {
      result[key] = canonicalize(value[key]);
    }
    return result;
  }
  throw new Error('signed quote contains a non-canonical JSON value');
}

export function canonicalQuoteBytes(quote) {
  const encoded = new TextEncoder().encode(JSON.stringify(canonicalize(quote)));
  if (encoded.length === 0 || encoded.length > 64 * 1024) {
    throw new Error('signed quote canonical bytes are empty or oversized');
  }
  return encoded;
}

function concatBytes(...values) {
  const length = values.reduce((total, value) => total + value.length, 0);
  const result = new Uint8Array(length);
  let offset = 0;
  for (const value of values) {
    result.set(value, offset);
    offset += value.length;
  }
  return result;
}

export async function verifySignedQuoteEnvelope(
  envelope,
  expectedPublicKeyHex,
  subtle = globalThis.crypto?.subtle,
) {
  if (!subtle?.digest || !subtle?.importKey || !subtle?.verify) {
    throw new Error('WebCrypto Ed25519 verification is unavailable');
  }
  if (
    !envelope
    || typeof envelope !== 'object'
    || Array.isArray(envelope)
    || Object.keys(envelope).sort().join(',') !== 'algorithm,key_id,public_key,quote,signature'
    || envelope.algorithm !== 'Ed25519'
  ) {
    throw new Error('signed quote envelope is malformed');
  }
  if (typeof expectedPublicKeyHex !== 'string' || !HEX_32.test(expectedPublicKeyHex)) {
    throw new Error('coordinator quote signer key pin is invalid');
  }
  const publicKey = decodeBase64Url(envelope.public_key, 32, 'quote public_key');
  if (bytesToHex(publicKey) !== expectedPublicKeyHex) {
    throw new Error('signed quote public key does not match coordinator status pin');
  }
  const expectedKeyId = bytesToHex(new Uint8Array(await subtle.digest(
    'SHA-256',
    concatBytes(KEY_ID_DOMAIN, publicKey),
  )));
  if (envelope.key_id !== expectedKeyId) {
    throw new Error('signed quote key_id is invalid');
  }
  const signature = decodeBase64Url(envelope.signature, 64, 'quote signature');
  const quoteBytes = canonicalQuoteBytes(envelope.quote);
  const length = new Uint8Array(4);
  new DataView(length.buffer).setUint32(0, quoteBytes.length, false);
  const message = concatBytes(SIGNATURE_DOMAIN, length, quoteBytes);
  const key = await subtle.importKey(
    'raw',
    publicKey,
    { name: 'Ed25519' },
    false,
    ['verify'],
  );
  if (!await subtle.verify({ name: 'Ed25519' }, key, signature, message)) {
    throw new Error('signed quote Ed25519 signature is invalid');
  }
  return envelope.quote;
}
