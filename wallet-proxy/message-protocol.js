'use strict';

// Post Fiat L1v2 Canonical Message Protocol v1 JavaScript SDK (Pillar 2 / Contract C1).
//
// Zero external protobuf runtime dependencies. Uses Node built-in `crypto` plus the
// already-locked `@noble/post-quantum` (ML-DSA-65 + IETF X-Wing `ml_kem768_x25519`)
// and `@noble/ciphers` (`xchacha20poly1305`) packages from wallet-proxy.

const crypto = require('node:crypto');

// L1 PaymentMemo consensus limits (crates/types/src/core_chain.rs)
const MAX_PAYMENT_MEMOS = 4;
const MAX_PAYMENT_MEMO_TYPE_BYTES = 64;
const MAX_PAYMENT_MEMO_FORMAT_BYTES = 64;
const MAX_PAYMENT_MEMO_DATA_BYTES = 256;
const MAX_PAYMENT_MEMO_TOTAL_BYTES = 512;

// Envelope & Manifest payload caps
const MAX_SINGLE_ENVELOPE_BYTES = 1_048_576; // 1 MiB
const MAX_MANIFEST_PLAINTEXT_BYTES = 16_777_216; // 16 MiB

// Normative post-quantum byte sizes
const ML_DSA_65_PUBKEY_BYTES = 1952;
const ML_DSA_65_SIGNATURE_BYTES = 3309;
const XWING_PUBKEY_BYTES = 1216; // 1184B ML-KEM-768 ek || 32B X25519 pk
const XWING_CIPHERTEXT_BYTES = 1120; // 1088B ML-KEM-768 ct || 32B X25519 epk
const AEAD_KEY_BYTES = 32;
const AEAD_NONCE_BYTES = 24;
const AEAD_TAG_BYTES = 16;
const WRAPPED_CEK_BYTES = AEAD_KEY_BYTES + AEAD_TAG_BYTES;
const CIDV1_RAW_PREFIX = Buffer.from([0x01, 0x55, 0x12, 0x20]);
const CIDV1_RAW_BYTES = 36;

// Canonical domain separation strings (Contract C1)
const DOMAIN_ENVELOPE_SIGN = Buffer.from('postfiat-l1-v2/message-envelope/v1', 'ascii');
const DOMAIN_MESSAGE_AAD = Buffer.from('postfiat-l1-v2/message-aad/v1', 'ascii');
const DOMAIN_XWING_WRAP_INFO = 'postfiat-l1-v2/xwing-wrap/v1';
const DOMAIN_MESSAGE_KEY_KDF = Buffer.from('postfiat-l1-v2/message-key/v1', 'ascii');
const DOMAIN_MESSAGE_KEY_BIND = Buffer.from('postfiat-l1-v2/message-key-bind/v1', 'ascii');
const DOMAIN_CEK_COMMIT = Buffer.from('postfiat-l1-v2/cek-commit/v1', 'ascii');
const DOMAIN_SEALED_RECIPIENT_TAG = Buffer.from('postfiat-l1-v2/sealed-recipient-tag/v1', 'ascii');

// Account address derivation of the chain (crates/crypto_provider/src/address.rs).
const ACCOUNT_ADDRESS_PREFIX = 'pf';
const ACCOUNT_ADDRESS_DERIVATION_DOMAIN = Buffer.from('postfiat.address.v1', 'ascii');
const ACCOUNT_ADDRESS_DIGEST_BYTES = 20;

const MEMO_TYPE_PF_PTR_HEX = Buffer.from('pf.ptr', 'ascii').toString('hex'); // "70662e707472"
const MEMO_FORMAT_V5_HEX = Buffer.from('v5', 'ascii').toString('hex'); // "7635"
const MEMO_FORMAT_V4_HEX = Buffer.from('v4', 'ascii').toString('hex'); // "7634"

const SCHEMA_ID = Object.freeze({
  CUSTOM: 1,
  TEXT_V1: 100,
  GOVERNANCE_PACKET_V1: 101,
  GOVERNANCE_SNAPSHOT_V1: 102,
  GOVERNANCE_TRANSCRIPT_V1: 103,
  GOVERNANCE_MINUTES_V1: 104,
  GOVERNANCE_RULING_V1: 105,
  REWARD_V1: 201,
  NFT_METADATA_V1: 400,
  NFT_CLAIM_V1: 401,
  X402_REQUEST_V1: 402,
  NFT_PROVENANCE_V1: 403,
});

const ContentKind = Object.freeze({
  UNSPECIFIED: 0,
  TASK: 1,
  TASK_UPDATE: 2,
  TASK_SUBMISSION: 3,
  CHAT: 4,
  CONTEXT: 5,
  REWARD: 6,
  POLICY: 7,
  IDENTITY: 8,
  ASSET: 9,
  DOCUMENT: 10,
  SYSTEM: 11,
  GOVERNANCE: 12,
  NFT: 13,
  X402: 14,
  TEST: 99,
});

const PointerTarget = Object.freeze({
  UNSPECIFIED: 0,
  CONTENT_BLOB: 1,
  MANIFEST: 2,
  INLINE: 3,
});

const PointerFlag = Object.freeze({
  UNSPECIFIED: 0,
  PRIVATE: 0x01,
  PUBLIC: 0x02,
  EPHEMERAL: 0x04,
  TOMBSTONE: 0x08,
  MULTIPART: 0x10,
  SEALED: 0x20,
});

const VISIBILITY_FLAG_MASK = PointerFlag.PRIVATE | PointerFlag.PUBLIC | PointerFlag.SEALED;
const ALLOWED_POINTER_FLAG_MASK =
  PointerFlag.PRIVATE |
  PointerFlag.PUBLIC |
  PointerFlag.EPHEMERAL |
  PointerFlag.TOMBSTONE |
  PointerFlag.MULTIPART |
  PointerFlag.SEALED;

const Visibility = Object.freeze({
  UNSPECIFIED: 0,
  PUBLIC: 1,
  PRIVATE: 2,
  SEALED: 3,
});

const CipherSuite = Object.freeze({
  UNSPECIFIED: 0,
  NONE: 1,
  XWING_XCHACHA20_POLY1305: 2,
  LEGACY_X25519_XCHACHA20_POLY1305: 3,
});

const SignatureAlgorithm = Object.freeze({
  UNSPECIFIED: 0,
  ML_DSA_65: 1,
});

const BodyEncoding = Object.freeze({
  UNSPECIFIED: 0,
  PROTOBUF: 1,
  CANONICAL_JSON: 2,
  RAW_UTF8: 3,
  RAW_BYTES: 4,
});

const PaymentLane = Object.freeze({
  UNSPECIFIED: 0,
  FASTPAY: 1,
  CONSENSUS_PFT: 2,
  CONSENSUS_ISSUED: 3,
});

const X402Scheme = Object.freeze({
  UNSPECIFIED: 0,
  EXACT: 1,
  PF_FASTPAY_SETTLED: 2,
});

class MessageProtocolError extends Error {
  constructor(message) {
    super(message);
    this.name = 'MessageProtocolError';
  }
}

// Lazy-load Noble ESM modules so CommonJS consumers require() synchronously.
let _pqModulesPromise = null;
function loadPqModules() {
  _pqModulesPromise ||= Promise.all([
    import('@noble/post-quantum/ml-dsa.js'),
    import('@noble/post-quantum/hybrid.js'),
    import('@noble/ciphers/chacha.js'),
  ]).then(([dsa, hybrid, chacha]) => ({
    mlDsa65: dsa.ml_dsa65,
    xwing: hybrid.ml_kem768_x25519,
    xchacha20poly1305: chacha.xchacha20poly1305,
  }));
  return _pqModulesPromise;
}

// ---------------------------------------------------------------------------
// Deterministic protobuf3 wire primitives
// ---------------------------------------------------------------------------

const MAX_U64 = (1n << 64n) - 1n;

function toBigUint(value, label = 'varint') {
  if (typeof value === 'bigint') {
    if (value < 0n || value > MAX_U64) throw new MessageProtocolError(`${label} out of uint64 range`);
    return value;
  }
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) {
    return BigInt(value);
  }
  if (typeof value === 'boolean') {
    return value ? 1n : 0n;
  }
  throw new MessageProtocolError(`invalid uint64 ${label}: ${String(value)}`);
}

function toSafeNumber(bigintVal, label = 'uint64') {
  if (bigintVal <= BigInt(Number.MAX_SAFE_INTEGER)) {
    return Number(bigintVal);
  }
  return bigintVal;
}

function encodeVarint(value) {
  let n = toBigUint(value);
  const out = [];
  while (n > 0x7fn) {
    out.push(Number((n & 0x7fn) | 0x80n));
    n >>= 7n;
  }
  out.push(Number(n));
  return Buffer.from(out);
}

function decodeVarint(buf, offset) {
  let result = 0n;
  let shift = 0n;
  const start = offset;
  while (true) {
    if (offset >= buf.length) throw new MessageProtocolError('truncated protobuf varint');
    const byte = buf[offset];
    offset += 1;
    result |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) {
      if (offset - start > 1 && byte === 0) {
        throw new MessageProtocolError('non-minimal protobuf varint');
      }
      if (result > MAX_U64) throw new MessageProtocolError('protobuf varint overflow');
      return { value: result, offset };
    }
    shift += 7n;
    if (shift > 63n) throw new MessageProtocolError('protobuf varint overflow');
  }
}

function asBuf(value) {
  if (!value) return Buffer.alloc(0);
  if (Buffer.isBuffer(value)) return value;
  if (value instanceof Uint8Array) return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  throw new MessageProtocolError('expected Buffer or Uint8Array');
}

function pbVarint(fieldNum, value) {
  if (value === undefined || value === null || value === 0 || value === 0n || value === false) {
    return Buffer.alloc(0);
  }
  const n = toBigUint(value, `field ${fieldNum}`);
  if (n === 0n) return Buffer.alloc(0);
  return Buffer.concat([encodeVarint((fieldNum << 3) | 0), encodeVarint(n)]);
}

function pbBool(fieldNum, value) {
  return value ? pbVarint(fieldNum, 1) : Buffer.alloc(0);
}

function pbBytes(fieldNum, value) {
  const buf = asBuf(value);
  if (buf.length === 0) return Buffer.alloc(0);
  return Buffer.concat([encodeVarint((fieldNum << 3) | 2), encodeVarint(buf.length), buf]);
}

function pbString(fieldNum, value) {
  if (!value) return Buffer.alloc(0);
  if (typeof value !== 'string') throw new MessageProtocolError(`field ${fieldNum} must be string`);
  return pbBytes(fieldNum, Buffer.from(value, 'utf8'));
}

function decodeWireFields(input, { enforceCanonicalOrder = false } = {}) {
  const buf = asBuf(input);
  const fields = [];
  let offset = 0;
  let lastField = 0;
  while (offset < buf.length) {
    const tagDec = decodeVarint(buf, offset);
    offset = tagDec.offset;
    const tag = Number(tagDec.value);
    const fieldNum = tag >>> 3;
    const wireType = tag & 0x07;
    if (fieldNum === 0) throw new MessageProtocolError('invalid protobuf field number 0');
    if (enforceCanonicalOrder && fieldNum < lastField) {
      throw new MessageProtocolError(`non-canonical protobuf field order: ${fieldNum} after ${lastField}`);
    }
    lastField = fieldNum;
    if (wireType === 0) {
      const valDec = decodeVarint(buf, offset);
      offset = valDec.offset;
      fields.push({ fieldNum, wireType, value: valDec.value });
    } else if (wireType === 2) {
      const lenDec = decodeVarint(buf, offset);
      offset = lenDec.offset;
      const len = Number(lenDec.value);
      if (offset + len > buf.length) throw new MessageProtocolError('truncated protobuf length-delimited field');
      fields.push({ fieldNum, wireType, value: buf.subarray(offset, offset + len) });
      offset += len;
    } else if (wireType === 1) {
      if (offset + 8 > buf.length) throw new MessageProtocolError('truncated protobuf 64-bit field');
      fields.push({ fieldNum, wireType, value: buf.subarray(offset, offset + 8) });
      offset += 8;
    } else if (wireType === 5) {
      if (offset + 4 > buf.length) throw new MessageProtocolError('truncated protobuf 32-bit field');
      fields.push({ fieldNum, wireType, value: buf.subarray(offset, offset + 4) });
      offset += 4;
    } else {
      throw new MessageProtocolError(`unsupported protobuf wire type ${wireType}`);
    }
  }
  return fields;
}

function expectWt(field, expected) {
  if (field.wireType !== expected) {
    throw new MessageProtocolError(
      `unexpected wire type ${field.wireType} for field ${field.fieldNum} (expected ${expected})`
    );
  }
}

// ---------------------------------------------------------------------------
// Base32lower (RFC 4648 without padding) + IPFS CIDv1 raw helpers
// ---------------------------------------------------------------------------

const BASE32_ALPHABET = 'abcdefghijklmnopqrstuvwxyz234567';
const BASE32_LOOKUP = new Int8Array(128).fill(-1);
for (let i = 0; i < BASE32_ALPHABET.length; i += 1) {
  BASE32_LOOKUP[BASE32_ALPHABET.charCodeAt(i)] = i;
}

function base32LowerEncode(bytes) {
  const buf = asBuf(bytes);
  let bits = 0;
  let value = 0;
  let out = '';
  for (let i = 0; i < buf.length; i += 1) {
    value = (value << 8) | buf[i];
    bits += 8;
    while (bits >= 5) {
      bits -= 5;
      out += BASE32_ALPHABET[(value >>> bits) & 0x1f];
    }
  }
  if (bits > 0) {
    out += BASE32_ALPHABET[(value << (5 - bits)) & 0x1f];
  }
  return out;
}

function base32LowerDecode(str) {
  if (typeof str !== 'string' || str.length === 0) {
    throw new MessageProtocolError('base32 string must be non-empty');
  }
  let bits = 0;
  let value = 0;
  const out = [];
  for (let i = 0; i < str.length; i += 1) {
    const code = str.charCodeAt(i);
    if (code >= 128 || BASE32_LOOKUP[code] === -1) {
      throw new MessageProtocolError(`invalid base32lower character at index ${i}`);
    }
    value = (value << 5) | BASE32_LOOKUP[code];
    bits += 5;
    if (bits >= 8) {
      bits -= 8;
      out.push((value >>> bits) & 0xff);
    }
  }
  if (bits >= 5 || (value & ((1 << bits) - 1)) !== 0) {
    throw new MessageProtocolError('non-canonical base32lower trailing bits');
  }
  return Buffer.from(out);
}

function sha256(data) {
  return crypto.createHash('sha256').update(asBuf(data)).digest();
}

function sha3_384(data) {
  return crypto.createHash('sha3-384').update(asBuf(data)).digest();
}

function hkdfSha256(ikm, salt, info, length = 32) {
  if (!Number.isInteger(length) || length <= 0 || length > 255 * 32) {
    throw new MessageProtocolError(`invalid HKDF length: ${length}`);
  }
  const ikmBuf = asBuf(ikm);
  const saltBuf = salt && asBuf(salt).length > 0 ? asBuf(salt) : Buffer.alloc(32);
  const infoBuf = typeof info === 'string' ? Buffer.from(info, 'utf8') : asBuf(info);
  return Buffer.from(crypto.hkdfSync('sha256', ikmBuf, saltBuf, infoBuf, length));
}

function cidBinToMultibase(cidBin) {
  const buf = asBuf(cidBin);
  if (buf.length !== CIDV1_RAW_BYTES || !buf.subarray(0, 4).equals(CIDV1_RAW_PREFIX)) {
    throw new MessageProtocolError('cid_bin must be 36 bytes starting with 0x01 0x55 0x12 0x20');
  }
  return `b${base32LowerEncode(buf)}`;
}

function multibaseToCidBin(multibaseCid) {
  if (typeof multibaseCid !== 'string' || !multibaseCid.startsWith('b')) {
    throw new MessageProtocolError("multibase_cid must start with base32lower prefix 'b'");
  }
  const cidBin = base32LowerDecode(multibaseCid.slice(1));
  if (cidBin.length !== CIDV1_RAW_BYTES || !cidBin.subarray(0, 4).equals(CIDV1_RAW_PREFIX)) {
    throw new MessageProtocolError('decoded CID is not a 36-byte CIDv1 raw sha2-256 block');
  }
  return cidBin;
}

function cidv1RawFromBytes(block) {
  const raw = asBuf(block);
  const cidBin = Buffer.concat([CIDV1_RAW_PREFIX, sha256(raw)]);
  return { cidBin, multibaseCid: cidBinToMultibase(cidBin) };
}

function verifyCidv1Raw(block, { expectedCidBin = null, expectedMultibaseCid = null } = {}) {
  const raw = asBuf(block);
  const { cidBin, multibaseCid } = cidv1RawFromBytes(raw);
  if (expectedCidBin !== null) {
    const expected = asBuf(expectedCidBin);
    if (expected.length !== CIDV1_RAW_BYTES || !crypto.timingSafeEqual(cidBin, expected)) {
      throw new MessageProtocolError('IPFS CIDv1 raw binary digest mismatch');
    }
  }
  if (expectedMultibaseCid !== null) {
    const decoded = multibaseToCidBin(expectedMultibaseCid);
    if (!crypto.timingSafeEqual(cidBin, decoded) || expectedMultibaseCid !== multibaseCid) {
      throw new MessageProtocolError('IPFS CIDv1 raw multibase mismatch');
    }
  }
  return { cid_bin: cidBin, byte_length: raw.length, multibase_cid: multibaseCid };
}

// RFC 8785 / JCS canonical JSON for plain JSON objects/arrays/primitives.
function canonicalJsonString(value) {
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map((item) => canonicalJsonString(item)).join(',')}]`;
  }
  const keys = Object.keys(value)
    .filter((key) => value[key] !== undefined)
    .sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${canonicalJsonString(value[k])}`).join(',')}}`;
}

function canonicalJsonBytes(value) {
  return Buffer.from(canonicalJsonString(value), 'utf8');
}

// ---------------------------------------------------------------------------
// Cryptographic domain-separated preimages & key derivation
// ---------------------------------------------------------------------------

function u32be(n) {
  const buf = Buffer.allocUnsafe(4);
  buf.writeUInt32BE(n, 0);
  return buf;
}

function u64be(n) {
  const buf = Buffer.allocUnsafe(8);
  buf.writeBigUInt64BE(toBigUint(n, 'u64'), 0);
  return buf;
}

function envelopeSigningPreimage(version, headerBytes, encryptionHeaderBytes, bodyBytes) {
  const h = asBuf(headerBytes);
  const eh = asBuf(encryptionHeaderBytes);
  const b = asBuf(bodyBytes);
  return Buffer.concat([
    DOMAIN_ENVELOPE_SIGN,
    Buffer.from([0]),
    u32be(version),
    u32be(h.length),
    h,
    u32be(eh.length),
    eh,
    u32be(b.length),
    b,
  ]);
}

function computeAadContext(chainId, headerBytes, chunkIndex = 0) {
  if (typeof chainId !== 'string' || chainId.length === 0) {
    throw new MessageProtocolError('chainId is required for AAD context');
  }
  return sha256(
    Buffer.concat([
      DOMAIN_MESSAGE_AAD,
      Buffer.from([0]),
      Buffer.from(chainId, 'utf8'),
      Buffer.from([0]),
      sha256(headerBytes),
      u32be(chunkIndex),
    ])
  );
}

function deriveMessageKemSeed(walletMasterEntropy, chainId) {
  const entropy = asBuf(walletMasterEntropy);
  if (entropy.length < 32) {
    throw new MessageProtocolError('walletMasterEntropy must be at least 32 bytes');
  }
  if (typeof chainId !== 'string' || chainId.length === 0) {
    throw new MessageProtocolError('chainId is required');
  }
  return hkdfSha256(entropy, Buffer.from(chainId, 'utf8'), DOMAIN_MESSAGE_KEY_KDF, 32);
}

async function deriveMessageKemKeypair(walletMasterEntropy, chainId) {
  const seed = deriveMessageKemSeed(walletMasterEntropy, chainId);
  const { xwing } = await loadPqModules();
  const { publicKey, secretKey } = xwing.keygen(seed);
  const publicKeyBuf = Buffer.from(publicKey);
  const secretKeyBuf = Buffer.from(secretKey);
  return {
    seed,
    publicKey: publicKeyBuf,
    secretKey: secretKeyBuf,
    keyId: sha256(publicKeyBuf),
  };
}

function computeRecipientKeyId(xwingPublicKey) {
  const pk = asBuf(xwingPublicKey);
  if (pk.length === 0) throw new MessageProtocolError('xwingPublicKey must be non-empty');
  return sha256(pk);
}

function deriveXWingWrapKek(sharedSecret, aadContext, recipientKeyId, kdfInfo = DOMAIN_XWING_WRAP_INFO) {
  const ss = asBuf(sharedSecret);
  const aad = asBuf(aadContext);
  const kid = asBuf(recipientKeyId);
  if (ss.length !== 32) throw new MessageProtocolError('X-Wing sharedSecret must be 32 bytes');
  if (aad.length !== 32) throw new MessageProtocolError('aadContext must be 32 bytes');
  if (kid.length !== 32) throw new MessageProtocolError('recipientKeyId must be 32 bytes');
  const info = Buffer.concat([Buffer.from(kdfInfo, 'utf8'), Buffer.from([0]), kid]);
  return hkdfSha256(ss, aad, info, 32);
}

// The canonical lowercase address of the account that an ML-DSA-65 public key controls. Same
// derivation as address_from_public_key in crates/crypto_provider/src/lib.rs.
function deriveAccountAddress(accountPubkey) {
  const pubkey = asBuf(accountPubkey);
  if (pubkey.length !== ML_DSA_65_PUBKEY_BYTES) {
    throw new MessageProtocolError(`account public key must be ${ML_DSA_65_PUBKEY_BYTES} bytes`);
  }
  const digest = sha3_384(Buffer.concat([ACCOUNT_ADDRESS_DERIVATION_DOMAIN, Buffer.from([0]), pubkey]));
  return `${ACCOUNT_ADDRESS_PREFIX}${digest.subarray(0, ACCOUNT_ADDRESS_DIGEST_BYTES).toString('hex')}`;
}

function computeKeyBindingPreimage({
  chain_id,
  account,
  xwing_public_key,
  key_id,
  created_at_unix_ms,
  key_epoch,
}) {
  const chainBuf = Buffer.from(String(chain_id || ''), 'utf8');
  const acctBuf = Buffer.from(String(account || ''), 'utf8');
  return Buffer.concat([
    DOMAIN_MESSAGE_KEY_BIND,
    Buffer.from([0]),
    u32be(chainBuf.length),
    chainBuf,
    u32be(acctBuf.length),
    acctBuf,
    asBuf(xwing_public_key),
    asBuf(key_id),
    u64be(created_at_unix_ms),
    u32be(key_epoch),
  ]);
}

async function xchacha20Poly1305Encrypt(key, nonce, plaintext, aad) {
  const keyBuf = asBuf(key);
  const nonceBuf = asBuf(nonce);
  if (keyBuf.length !== AEAD_KEY_BYTES) throw new MessageProtocolError('XChaCha20-Poly1305 key must be 32 bytes');
  if (nonceBuf.length !== AEAD_NONCE_BYTES) throw new MessageProtocolError('XChaCha20-Poly1305 nonce must be 24 bytes');
  const { xchacha20poly1305 } = await loadPqModules();
  const cipher = xchacha20poly1305(keyBuf, nonceBuf, aad ? asBuf(aad) : undefined);
  return Buffer.from(cipher.encrypt(asBuf(plaintext)));
}

async function xchacha20Poly1305Decrypt(key, nonce, ciphertextWithTag, aad) {
  const keyBuf = asBuf(key);
  const nonceBuf = asBuf(nonce);
  const ct = asBuf(ciphertextWithTag);
  if (keyBuf.length !== AEAD_KEY_BYTES) throw new MessageProtocolError('XChaCha20-Poly1305 key must be 32 bytes');
  if (nonceBuf.length !== AEAD_NONCE_BYTES) throw new MessageProtocolError('XChaCha20-Poly1305 nonce must be 24 bytes');
  if (ct.length < AEAD_TAG_BYTES) throw new MessageProtocolError('ciphertext shorter than 16-byte tag');
  const { xchacha20poly1305 } = await loadPqModules();
  const cipher = xchacha20poly1305(keyBuf, nonceBuf, aad ? asBuf(aad) : undefined);
  try {
    return Buffer.from(cipher.decrypt(ct));
  } catch (err) {
    throw new MessageProtocolError(`XChaCha20-Poly1305 authentication failed: ${err.message}`);
  }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

function validatePointerV5Flags(flags) {
  if (!Number.isInteger(flags) || flags < 0 || (flags & ~ALLOWED_POINTER_FLAG_MASK) !== 0) {
    throw new MessageProtocolError(`unsupported pointer flags: 0x${Number(flags).toString(16)}`);
  }
  const vis = flags & VISIBILITY_FLAG_MASK;
  if (vis !== PointerFlag.PRIVATE && vis !== PointerFlag.PUBLIC && vis !== PointerFlag.SEALED) {
    throw new MessageProtocolError(
      `pointer flags must set exactly one of PRIVATE (0x01), PUBLIC (0x02), SEALED (0x20); got 0x${flags.toString(16)}`
    );
  }
}

function validateL1PaymentMemo(memoTypeHex, memoFormatHex, memoDataHex) {
  const specs = [
    ['memo_type', memoTypeHex, MAX_PAYMENT_MEMO_TYPE_BYTES],
    ['memo_format', memoFormatHex, MAX_PAYMENT_MEMO_FORMAT_BYTES],
    ['memo_data', memoDataHex, MAX_PAYMENT_MEMO_DATA_BYTES],
  ];
  for (const [name, val, limit] of specs) {
    if (typeof val !== 'string' || !/^[0-9a-f]*$/.test(val) || val.length % 2 !== 0) {
      throw new MessageProtocolError(`${name} must be even-length lowercase hex`);
    }
    if (val.length / 2 > limit) {
      throw new MessageProtocolError(`${name} decoded length ${val.length / 2} exceeds limit ${limit}`);
    }
  }
  const total = (memoTypeHex.length + memoFormatHex.length + memoDataHex.length) / 2;
  if (total === 0 || total > MAX_PAYMENT_MEMO_TOTAL_BYTES) {
    throw new MessageProtocolError(`total memo byte length ${total} outside 1..${MAX_PAYMENT_MEMO_TOTAL_BYTES}`);
  }
}

// ---------------------------------------------------------------------------
// Message encoders & decoders (postfiat.message.v1)
// ---------------------------------------------------------------------------

function encodeIpfsCidRef(ref) {
  const cidBin = asBuf(ref.cid_bin);
  const multibase = ref.multibase_cid || '';
  if (cidBin.length > 0 && multibase && cidBinToMultibase(cidBin) !== multibase) {
    throw new MessageProtocolError('IpfsCidRef multibase_cid does not match cid_bin');
  }
  return Buffer.concat([
    pbBytes(1, cidBin),
    pbVarint(2, ref.byte_length || 0),
    pbString(3, multibase),
  ]);
}

function decodeIpfsCidRef(bytes, opts = {}) {
  const out = { cid_bin: Buffer.alloc(0), byte_length: 0, multibase_cid: '' };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 2); out.cid_bin = Buffer.from(f.value); }
    else if (f.fieldNum === 2) { expectWt(f, 0); out.byte_length = toSafeNumber(f.value); }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.multibase_cid = f.value.toString('utf8'); }
  }
  if (out.cid_bin.length > 0 && out.multibase_cid && cidBinToMultibase(out.cid_bin) !== out.multibase_cid) {
    throw new MessageProtocolError('IpfsCidRef multibase_cid does not match cid_bin');
  }
  return out;
}

function validatePointer(ptr) {
  validatePointerV5Flags(ptr.flags);
  const cidBin = asBuf(ptr.cid_bin);
  const inlinePayload = asBuf(ptr.inline_payload);
  if (ptr.target === PointerTarget.INLINE) {
    if (cidBin.length !== 0 || inlinePayload.length === 0) {
      throw new MessageProtocolError('POINTER_TARGET_INLINE requires non-empty inline_payload and empty cid_bin');
    }
  } else if (ptr.target === PointerTarget.CONTENT_BLOB || ptr.target === PointerTarget.MANIFEST) {
    if (cidBin.length !== CIDV1_RAW_BYTES || !cidBin.subarray(0, 4).equals(CIDV1_RAW_PREFIX)) {
      throw new MessageProtocolError('CONTENT_BLOB/MANIFEST pointer requires 36-byte CIDv1 raw cid_bin');
    }
    if (inlinePayload.length !== 0) {
      throw new MessageProtocolError('CONTENT_BLOB/MANIFEST pointer must leave inline_payload empty');
    }
  } else {
    throw new MessageProtocolError(`unsupported PointerTarget: ${ptr.target}`);
  }
}

function encodePointer(ptr) {
  validatePointer(ptr);
  const encoded = Buffer.concat([
    pbBytes(1, ptr.cid_bin),
    pbVarint(2, ptr.target || 0),
    pbVarint(3, ptr.kind || 0),
    pbVarint(4, ptr.schema || 0),
    pbString(5, ptr.task_id || ''),
    pbString(6, ptr.thread_id || ''),
    pbString(7, ptr.context_id || ''),
    pbVarint(8, ptr.flags || 0),
    pbBytes(9, ptr.inline_payload),
    pbString(10, ptr.schema_name || ''),
  ]);
  if (encoded.length > MAX_PAYMENT_MEMO_DATA_BYTES) {
    throw new MessageProtocolError(`Pointer v5 length ${encoded.length} exceeds ${MAX_PAYMENT_MEMO_DATA_BYTES}`);
  }
  return encoded;
}

function decodePointer(bytes, opts = {}) {
  const buf = asBuf(bytes);
  if (buf.length > MAX_PAYMENT_MEMO_DATA_BYTES) {
    throw new MessageProtocolError(`Pointer v5 length ${buf.length} exceeds ${MAX_PAYMENT_MEMO_DATA_BYTES}`);
  }
  const out = {
    cid_bin: Buffer.alloc(0),
    target: 0,
    kind: 0,
    schema: 0,
    task_id: '',
    thread_id: '',
    context_id: '',
    flags: 0,
    inline_payload: Buffer.alloc(0),
    schema_name: '',
  };
  for (const f of decodeWireFields(buf, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 2); out.cid_bin = Buffer.from(f.value); }
    else if (f.fieldNum === 2) { expectWt(f, 0); out.target = Number(f.value); }
    else if (f.fieldNum === 3) { expectWt(f, 0); out.kind = Number(f.value); }
    else if (f.fieldNum === 4) { expectWt(f, 0); out.schema = Number(f.value); }
    else if (f.fieldNum === 5) { expectWt(f, 2); out.task_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 6) { expectWt(f, 2); out.thread_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 7) { expectWt(f, 2); out.context_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 8) { expectWt(f, 0); out.flags = Number(f.value); }
    else if (f.fieldNum === 9) { expectWt(f, 2); out.inline_payload = Buffer.from(f.value); }
    else if (f.fieldNum === 10) { expectWt(f, 2); out.schema_name = f.value.toString('utf8'); }
  }
  validatePointer(out);
  return out;
}

function encodeLegacyPointerV4(ptr) {
  return Buffer.concat([
    pbString(1, ptr.cid || ''),
    pbVarint(2, ptr.target || 0),
    pbVarint(3, ptr.kind || 0),
    pbVarint(4, ptr.schema || 0),
    pbString(5, ptr.task_id || ''),
    pbString(6, ptr.thread_id || ''),
    pbString(7, ptr.context_id || ''),
    pbVarint(8, ptr.flags || 0),
  ]);
}

function decodeLegacyPointerV4(bytes) {
  const out = { cid: '', target: 0, kind: 0, schema: 0, task_id: '', thread_id: '', context_id: '', flags: 0 };
  for (const f of decodeWireFields(bytes)) {
    if (f.fieldNum === 1) { expectWt(f, 2); out.cid = f.value.toString('utf8'); }
    else if (f.fieldNum === 2) { expectWt(f, 0); out.target = Number(f.value); }
    else if (f.fieldNum === 3) { expectWt(f, 0); out.kind = Number(f.value); }
    else if (f.fieldNum === 4) { expectWt(f, 0); out.schema = Number(f.value); }
    else if (f.fieldNum === 5) { expectWt(f, 2); out.task_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 6) { expectWt(f, 2); out.thread_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 7) { expectWt(f, 2); out.context_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 8) { expectWt(f, 0); out.flags = Number(f.value); }
  }
  return out;
}

function encodeRecipientWrap(wrap) {
  return Buffer.concat([
    pbBytes(1, wrap.recipient_key_id),
    pbString(2, wrap.recipient_account || ''),
    pbBytes(3, wrap.xwing_ciphertext),
    pbBytes(4, wrap.wrap_nonce),
    pbBytes(5, wrap.wrapped_cek),
  ]);
}

function decodeRecipientWrap(bytes, opts = {}) {
  const out = {
    recipient_key_id: Buffer.alloc(0),
    recipient_account: '',
    xwing_ciphertext: Buffer.alloc(0),
    wrap_nonce: Buffer.alloc(0),
    wrapped_cek: Buffer.alloc(0),
  };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 2); out.recipient_key_id = Buffer.from(f.value); }
    else if (f.fieldNum === 2) { expectWt(f, 2); out.recipient_account = f.value.toString('utf8'); }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.xwing_ciphertext = Buffer.from(f.value); }
    else if (f.fieldNum === 4) { expectWt(f, 2); out.wrap_nonce = Buffer.from(f.value); }
    else if (f.fieldNum === 5) { expectWt(f, 2); out.wrapped_cek = Buffer.from(f.value); }
  }
  return out;
}

function encodeXWingEncryptionHeader(hdr) {
  const parts = [
    pbVarint(1, hdr.cipher_suite || 0),
    pbBytes(2, hdr.aead_nonce),
    pbBytes(3, hdr.aad_context),
    pbVarint(4, hdr.chunk_index || 0),
    pbString(5, hdr.kdf_info || ''),
  ];
  for (const r of hdr.recipients || []) {
    parts.push(pbBytes(6, encodeRecipientWrap(r)));
  }
  if (hdr.cek_commitment && hdr.cek_commitment.length > 0) {
    parts.push(pbBytes(7, hdr.cek_commitment));
  }
  return Buffer.concat(parts);
}

function decodeXWingEncryptionHeader(bytes, opts = {}) {
  const out = {
    cipher_suite: 0,
    aead_nonce: Buffer.alloc(0),
    aad_context: Buffer.alloc(0),
    chunk_index: 0,
    kdf_info: '',
    recipients: [],
    cek_commitment: Buffer.alloc(0),
  };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 0); out.cipher_suite = Number(f.value); }
    else if (f.fieldNum === 2) { expectWt(f, 2); out.aead_nonce = Buffer.from(f.value); }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.aad_context = Buffer.from(f.value); }
    else if (f.fieldNum === 4) { expectWt(f, 0); out.chunk_index = Number(f.value); }
    else if (f.fieldNum === 5) { expectWt(f, 2); out.kdf_info = f.value.toString('utf8'); }
    else if (f.fieldNum === 6) { expectWt(f, 2); out.recipients.push(decodeRecipientWrap(f.value, opts)); }
    else if (f.fieldNum === 7) { expectWt(f, 2); out.cek_commitment = Buffer.from(f.value); }
  }
  return out;
}

function encodeMessageKeyBinding(kb) {
  return Buffer.concat([
    pbString(1, kb.chain_id || ''),
    pbString(2, kb.account || ''),
    pbBytes(3, kb.xwing_public_key),
    pbBytes(4, kb.key_id),
    pbVarint(5, kb.created_at_unix_ms || 0),
    pbVarint(6, kb.key_epoch || 0),
    pbVarint(7, kb.signature_algorithm || 0),
    pbBytes(8, kb.account_pubkey),
    pbBytes(9, kb.signature),
  ]);
}

function decodeMessageKeyBinding(bytes, opts = {}) {
  const out = {
    chain_id: '',
    account: '',
    xwing_public_key: Buffer.alloc(0),
    key_id: Buffer.alloc(0),
    created_at_unix_ms: 0,
    key_epoch: 0,
    signature_algorithm: 0,
    account_pubkey: Buffer.alloc(0),
    signature: Buffer.alloc(0),
  };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 2); out.chain_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 2) { expectWt(f, 2); out.account = f.value.toString('utf8'); }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.xwing_public_key = Buffer.from(f.value); }
    else if (f.fieldNum === 4) { expectWt(f, 2); out.key_id = Buffer.from(f.value); }
    else if (f.fieldNum === 5) { expectWt(f, 0); out.created_at_unix_ms = toSafeNumber(f.value); }
    else if (f.fieldNum === 6) { expectWt(f, 0); out.key_epoch = Number(f.value); }
    else if (f.fieldNum === 7) { expectWt(f, 0); out.signature_algorithm = Number(f.value); }
    else if (f.fieldNum === 8) { expectWt(f, 2); out.account_pubkey = Buffer.from(f.value); }
    else if (f.fieldNum === 9) { expectWt(f, 2); out.signature = Buffer.from(f.value); }
  }
  return out;
}

function encodeFastPayPaymentRef(ref) {
  return Buffer.concat([
    pbVarint(1, ref.lane || 0),
    pbString(2, ref.chain_id || ''),
    pbBytes(3, ref.order_hash),
    pbBytes(4, ref.certificate_digest),
    pbBytes(5, ref.tx_hash),
    pbString(6, ref.asset_id || ''),
    pbString(7, ref.payer_account || ''),
    pbString(8, ref.payee_account || ''),
    pbVarint(9, ref.amount_atoms || 0),
    pbVarint(10, ref.fee_atoms || 0),
    pbVarint(11, ref.finalized_height || 0),
  ]);
}

function decodeFastPayPaymentRef(bytes, opts = {}) {
  const out = {
    lane: 0,
    chain_id: '',
    order_hash: Buffer.alloc(0),
    certificate_digest: Buffer.alloc(0),
    tx_hash: Buffer.alloc(0),
    asset_id: '',
    payer_account: '',
    payee_account: '',
    amount_atoms: 0,
    fee_atoms: 0,
    finalized_height: 0,
  };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 0); out.lane = Number(f.value); }
    else if (f.fieldNum === 2) { expectWt(f, 2); out.chain_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.order_hash = Buffer.from(f.value); }
    else if (f.fieldNum === 4) { expectWt(f, 2); out.certificate_digest = Buffer.from(f.value); }
    else if (f.fieldNum === 5) { expectWt(f, 2); out.tx_hash = Buffer.from(f.value); }
    else if (f.fieldNum === 6) { expectWt(f, 2); out.asset_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 7) { expectWt(f, 2); out.payer_account = f.value.toString('utf8'); }
    else if (f.fieldNum === 8) { expectWt(f, 2); out.payee_account = f.value.toString('utf8'); }
    else if (f.fieldNum === 9) { expectWt(f, 0); out.amount_atoms = toSafeNumber(f.value); }
    else if (f.fieldNum === 10) { expectWt(f, 0); out.fee_atoms = toSafeNumber(f.value); }
    else if (f.fieldNum === 11) { expectWt(f, 0); out.finalized_height = toSafeNumber(f.value); }
  }
  return out;
}

function encodeX402PaymentRequired(req) {
  return Buffer.concat([
    pbVarint(1, req.x402_version || 0),
    pbVarint(2, req.scheme || 0),
    pbString(3, req.network || ''),
    pbVarint(4, req.lane || 0),
    pbString(5, req.asset_id || ''),
    pbVarint(6, req.max_amount_atoms || 0),
    pbString(7, req.pay_to || ''),
    pbBytes(8, req.request_hash),
    pbBytes(9, req.nonce),
    pbVarint(10, req.expires_at_unix_ms || 0),
    pbString(11, req.resource_uri || ''),
    pbString(12, req.facilitator_url || ''),
  ]);
}

function decodeX402PaymentRequired(bytes, opts = {}) {
  const out = {
    x402_version: 0,
    scheme: 0,
    network: '',
    lane: 0,
    asset_id: '',
    max_amount_atoms: 0,
    pay_to: '',
    request_hash: Buffer.alloc(0),
    nonce: Buffer.alloc(0),
    expires_at_unix_ms: 0,
    resource_uri: '',
    facilitator_url: '',
  };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 0); out.x402_version = Number(f.value); }
    else if (f.fieldNum === 2) { expectWt(f, 0); out.scheme = Number(f.value); }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.network = f.value.toString('utf8'); }
    else if (f.fieldNum === 4) { expectWt(f, 0); out.lane = Number(f.value); }
    else if (f.fieldNum === 5) { expectWt(f, 2); out.asset_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 6) { expectWt(f, 0); out.max_amount_atoms = toSafeNumber(f.value); }
    else if (f.fieldNum === 7) { expectWt(f, 2); out.pay_to = f.value.toString('utf8'); }
    else if (f.fieldNum === 8) { expectWt(f, 2); out.request_hash = Buffer.from(f.value); }
    else if (f.fieldNum === 9) { expectWt(f, 2); out.nonce = Buffer.from(f.value); }
    else if (f.fieldNum === 10) { expectWt(f, 0); out.expires_at_unix_ms = toSafeNumber(f.value); }
    else if (f.fieldNum === 11) { expectWt(f, 2); out.resource_uri = f.value.toString('utf8'); }
    else if (f.fieldNum === 12) { expectWt(f, 2); out.facilitator_url = f.value.toString('utf8'); }
  }
  return out;
}

function encodeX402PaymentSignature(sig) {
  return Buffer.concat([
    pbVarint(1, sig.x402_version || 0),
    pbVarint(2, sig.scheme || 0),
    pbString(3, sig.network || ''),
    pbVarint(4, sig.lane || 0),
    pbBytes(5, sig.request_hash),
    pbBytes(6, sig.nonce),
    pbBytes(7, sig.order_hash),
    pbBytes(8, sig.tx_hash),
    pbBytes(9, sig.signed_payload),
    pbBytes(10, sig.certificate_payload),
  ]);
}

function decodeX402PaymentSignature(bytes, opts = {}) {
  const out = {
    x402_version: 0,
    scheme: 0,
    network: '',
    lane: 0,
    request_hash: Buffer.alloc(0),
    nonce: Buffer.alloc(0),
    order_hash: Buffer.alloc(0),
    tx_hash: Buffer.alloc(0),
    signed_payload: Buffer.alloc(0),
    certificate_payload: Buffer.alloc(0),
  };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 0); out.x402_version = Number(f.value); }
    else if (f.fieldNum === 2) { expectWt(f, 0); out.scheme = Number(f.value); }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.network = f.value.toString('utf8'); }
    else if (f.fieldNum === 4) { expectWt(f, 0); out.lane = Number(f.value); }
    else if (f.fieldNum === 5) { expectWt(f, 2); out.request_hash = Buffer.from(f.value); }
    else if (f.fieldNum === 6) { expectWt(f, 2); out.nonce = Buffer.from(f.value); }
    else if (f.fieldNum === 7) { expectWt(f, 2); out.order_hash = Buffer.from(f.value); }
    else if (f.fieldNum === 8) { expectWt(f, 2); out.tx_hash = Buffer.from(f.value); }
    else if (f.fieldNum === 9) { expectWt(f, 2); out.signed_payload = Buffer.from(f.value); }
    else if (f.fieldNum === 10) { expectWt(f, 2); out.certificate_payload = Buffer.from(f.value); }
  }
  return out;
}

function encodeX402PaymentResponse(resp) {
  return Buffer.concat([
    pbVarint(1, resp.x402_version || 0),
    pbBool(2, Boolean(resp.settled)),
    pbBytes(3, resp.payment_ref ? encodeFastPayPaymentRef(resp.payment_ref) : Buffer.alloc(0)),
    pbVarint(4, resp.settled_at_unix_ms || 0),
    pbString(5, resp.facilitator_id || ''),
  ]);
}

function decodeX402PaymentResponse(bytes, opts = {}) {
  const out = {
    x402_version: 0,
    settled: false,
    payment_ref: null,
    settled_at_unix_ms: 0,
    facilitator_id: '',
  };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 0); out.x402_version = Number(f.value); }
    else if (f.fieldNum === 2) { expectWt(f, 0); out.settled = f.value !== 0n; }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.payment_ref = decodeFastPayPaymentRef(f.value, opts); }
    else if (f.fieldNum === 4) { expectWt(f, 0); out.settled_at_unix_ms = toSafeNumber(f.value); }
    else if (f.fieldNum === 5) { expectWt(f, 2); out.facilitator_id = f.value.toString('utf8'); }
  }
  return out;
}

function encodeMessageHeader(hdr) {
  return Buffer.concat([
    pbVarint(1, hdr.protocol_version || 0),
    pbString(2, hdr.chain_id || ''),
    pbString(3, hdr.schema_name || ''),
    pbVarint(4, hdr.schema_id || 0),
    pbVarint(5, hdr.kind || 0),
    pbVarint(6, hdr.visibility || 0),
    pbVarint(7, hdr.body_encoding || 0),
    pbVarint(8, hdr.cipher_suite || 0),
    pbString(9, hdr.sender_account || ''),
    pbVarint(10, hdr.created_at_unix_ms || 0),
    pbString(11, hdr.task_id || ''),
    pbString(12, hdr.thread_id || ''),
    pbString(13, hdr.context_id || ''),
    pbBytes(14, hdr.reply_to_cid ? encodeIpfsCidRef(hdr.reply_to_cid) : Buffer.alloc(0)),
    pbBytes(15, hdr.payment_ref ? encodeFastPayPaymentRef(hdr.payment_ref) : Buffer.alloc(0)),
    pbBytes(16, hdr.x402_receipt ? encodeX402PaymentResponse(hdr.x402_receipt) : Buffer.alloc(0)),
  ]);
}

function decodeMessageHeader(bytes, opts = {}) {
  const out = {
    protocol_version: 0,
    chain_id: '',
    schema_name: '',
    schema_id: 0,
    kind: 0,
    visibility: 0,
    body_encoding: 0,
    cipher_suite: 0,
    sender_account: '',
    created_at_unix_ms: 0,
    task_id: '',
    thread_id: '',
    context_id: '',
    reply_to_cid: null,
    payment_ref: null,
    x402_receipt: null,
  };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 0); out.protocol_version = Number(f.value); }
    else if (f.fieldNum === 2) { expectWt(f, 2); out.chain_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.schema_name = f.value.toString('utf8'); }
    else if (f.fieldNum === 4) { expectWt(f, 0); out.schema_id = Number(f.value); }
    else if (f.fieldNum === 5) { expectWt(f, 0); out.kind = Number(f.value); }
    else if (f.fieldNum === 6) { expectWt(f, 0); out.visibility = Number(f.value); }
    else if (f.fieldNum === 7) { expectWt(f, 0); out.body_encoding = Number(f.value); }
    else if (f.fieldNum === 8) { expectWt(f, 0); out.cipher_suite = Number(f.value); }
    else if (f.fieldNum === 9) { expectWt(f, 2); out.sender_account = f.value.toString('utf8'); }
    else if (f.fieldNum === 10) { expectWt(f, 0); out.created_at_unix_ms = toSafeNumber(f.value); }
    else if (f.fieldNum === 11) { expectWt(f, 2); out.task_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 12) { expectWt(f, 2); out.thread_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 13) { expectWt(f, 2); out.context_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 14) { expectWt(f, 2); out.reply_to_cid = decodeIpfsCidRef(f.value, opts); }
    else if (f.fieldNum === 15) { expectWt(f, 2); out.payment_ref = decodeFastPayPaymentRef(f.value, opts); }
    else if (f.fieldNum === 16) { expectWt(f, 2); out.x402_receipt = decodeX402PaymentResponse(f.value, opts); }
  }
  return out;
}

function encodeEnvelope(env) {
  const encoded = Buffer.concat([
    pbVarint(1, env.version || 0),
    pbBytes(2, env.header_bytes),
    pbBytes(3, env.encryption_header_bytes),
    pbBytes(4, env.body_bytes),
    pbVarint(5, env.signature_algorithm || 0),
    pbBytes(6, env.sender_pubkey),
    pbBytes(7, env.signature),
  ]);
  if (encoded.length > MAX_SINGLE_ENVELOPE_BYTES) {
    throw new MessageProtocolError(`Envelope length ${encoded.length} exceeds ${MAX_SINGLE_ENVELOPE_BYTES}`);
  }
  return encoded;
}

function decodeEnvelope(bytes, opts = {}) {
  const buf = asBuf(bytes);
  if (buf.length > MAX_SINGLE_ENVELOPE_BYTES) {
    throw new MessageProtocolError(`Envelope length ${buf.length} exceeds ${MAX_SINGLE_ENVELOPE_BYTES}`);
  }
  const out = {
    version: 0,
    header_bytes: Buffer.alloc(0),
    encryption_header_bytes: Buffer.alloc(0),
    body_bytes: Buffer.alloc(0),
    signature_algorithm: 0,
    sender_pubkey: Buffer.alloc(0),
    signature: Buffer.alloc(0),
  };
  for (const f of decodeWireFields(buf, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 0); out.version = Number(f.value); }
    else if (f.fieldNum === 2) { expectWt(f, 2); out.header_bytes = Buffer.from(f.value); }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.encryption_header_bytes = Buffer.from(f.value); }
    else if (f.fieldNum === 4) { expectWt(f, 2); out.body_bytes = Buffer.from(f.value); }
    else if (f.fieldNum === 5) { expectWt(f, 0); out.signature_algorithm = Number(f.value); }
    else if (f.fieldNum === 6) { expectWt(f, 2); out.sender_pubkey = Buffer.from(f.value); }
    else if (f.fieldNum === 7) { expectWt(f, 2); out.signature = Buffer.from(f.value); }
  }
  return out;
}

function encodeManifestChunk(chunk) {
  return Buffer.concat([
    pbVarint(1, chunk.chunk_index || 0),
    pbBytes(2, chunk.cid ? encodeIpfsCidRef(chunk.cid) : Buffer.alloc(0)),
    pbBytes(3, chunk.aead_nonce),
  ]);
}

function decodeManifestChunk(bytes, opts = {}) {
  const out = { chunk_index: 0, cid: null, aead_nonce: Buffer.alloc(0) };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 0); out.chunk_index = Number(f.value); }
    else if (f.fieldNum === 2) { expectWt(f, 2); out.cid = decodeIpfsCidRef(f.value, opts); }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.aead_nonce = Buffer.from(f.value); }
  }
  return out;
}

function validateManifest(manifest) {
  if (manifest.total_plaintext_bytes > MAX_MANIFEST_PLAINTEXT_BYTES) {
    throw new MessageProtocolError(`Manifest total_plaintext_bytes ${manifest.total_plaintext_bytes} exceeds 16 MiB`);
  }
  if (!manifest.chunk_size_bytes || manifest.chunk_size_bytes > MAX_SINGLE_ENVELOPE_BYTES) {
    throw new MessageProtocolError(`Manifest chunk_size_bytes ${manifest.chunk_size_bytes} outside 1..1MiB`);
  }
  (manifest.chunks || []).forEach((chunk, idx) => {
    if (chunk.chunk_index !== idx) {
      throw new MessageProtocolError(`Manifest chunks must be strictly ascending 0..N-1; expected ${idx}, got ${chunk.chunk_index}`);
    }
  });
}

function encodeManifest(manifest) {
  validateManifest(manifest);
  const parts = [
    pbVarint(1, manifest.version || 0),
    pbVarint(2, manifest.total_plaintext_bytes || 0),
    pbVarint(3, manifest.chunk_size_bytes || 0),
    pbString(4, manifest.media_type || ''),
    pbString(5, manifest.inner_schema_name || ''),
    pbVarint(6, manifest.inner_body_encoding || 0),
    pbVarint(7, manifest.cipher_suite || 0),
  ];
  for (const chunk of manifest.chunks || []) {
    parts.push(pbBytes(8, encodeManifestChunk(chunk)));
  }
  return Buffer.concat(parts);
}

function decodeManifest(bytes, opts = {}) {
  const out = {
    version: 0,
    total_plaintext_bytes: 0,
    chunk_size_bytes: 0,
    media_type: '',
    inner_schema_name: '',
    inner_body_encoding: 0,
    cipher_suite: 0,
    chunks: [],
  };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 0); out.version = Number(f.value); }
    else if (f.fieldNum === 2) { expectWt(f, 0); out.total_plaintext_bytes = toSafeNumber(f.value); }
    else if (f.fieldNum === 3) { expectWt(f, 0); out.chunk_size_bytes = Number(f.value); }
    else if (f.fieldNum === 4) { expectWt(f, 2); out.media_type = f.value.toString('utf8'); }
    else if (f.fieldNum === 5) { expectWt(f, 2); out.inner_schema_name = f.value.toString('utf8'); }
    else if (f.fieldNum === 6) { expectWt(f, 0); out.inner_body_encoding = Number(f.value); }
    else if (f.fieldNum === 7) { expectWt(f, 0); out.cipher_suite = Number(f.value); }
    else if (f.fieldNum === 8) { expectWt(f, 2); out.chunks.push(decodeManifestChunk(f.value, opts)); }
  }
  validateManifest(out);
  return out;
}

function encodeTextMessage(msg) {
  const parts = [pbString(1, msg.text || ''), pbString(2, msg.format || '')];
  for (const att of msg.attachments || []) {
    parts.push(pbBytes(3, encodeIpfsCidRef(att)));
  }
  return Buffer.concat(parts);
}

function decodeTextMessage(bytes, opts = {}) {
  const out = { text: '', format: '', attachments: [] };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 2); out.text = f.value.toString('utf8'); }
    else if (f.fieldNum === 2) { expectWt(f, 2); out.format = f.value.toString('utf8'); }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.attachments.push(decodeIpfsCidRef(f.value, opts)); }
  }
  return out;
}

function encodeRewardNotice(notice) {
  return Buffer.concat([
    pbString(1, notice.reward_id || ''),
    pbString(2, notice.task_id || ''),
    pbString(3, notice.recipient_account || ''),
    pbVarint(4, notice.amount_atoms || 0),
    pbString(5, notice.asset_id || ''),
    pbString(6, notice.summary || ''),
    pbBytes(7, notice.payment_ref ? encodeFastPayPaymentRef(notice.payment_ref) : Buffer.alloc(0)),
  ]);
}

function decodeRewardNotice(bytes, opts = {}) {
  const out = {
    reward_id: '',
    task_id: '',
    recipient_account: '',
    amount_atoms: 0,
    asset_id: '',
    summary: '',
    payment_ref: null,
  };
  for (const f of decodeWireFields(bytes, opts)) {
    if (f.fieldNum === 1) { expectWt(f, 2); out.reward_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 2) { expectWt(f, 2); out.task_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 3) { expectWt(f, 2); out.recipient_account = f.value.toString('utf8'); }
    else if (f.fieldNum === 4) { expectWt(f, 0); out.amount_atoms = toSafeNumber(f.value); }
    else if (f.fieldNum === 5) { expectWt(f, 2); out.asset_id = f.value.toString('utf8'); }
    else if (f.fieldNum === 6) { expectWt(f, 2); out.summary = f.value.toString('utf8'); }
    else if (f.fieldNum === 7) { expectWt(f, 2); out.payment_ref = decodeFastPayPaymentRef(f.value, opts); }
  }
  return out;
}

// ---------------------------------------------------------------------------
// Memo & x402 HTTP header helpers
// ---------------------------------------------------------------------------

function encodePointerV5Memo(pointer) {
  const memoDataHex = encodePointer(pointer).toString('hex');
  validateL1PaymentMemo(MEMO_TYPE_PF_PTR_HEX, MEMO_FORMAT_V5_HEX, memoDataHex);
  return {
    memo_type: MEMO_TYPE_PF_PTR_HEX,
    memo_format: MEMO_FORMAT_V5_HEX,
    memo_data: memoDataHex,
  };
}

function decodePointerV5Memo(memo, opts = {}) {
  const memoType = String(memo?.memo_type || '');
  const memoFormat = String(memo?.memo_format || '');
  const memoData = String(memo?.memo_data || '');
  validateL1PaymentMemo(memoType, memoFormat, memoData);
  if (memoType !== MEMO_TYPE_PF_PTR_HEX || memoFormat !== MEMO_FORMAT_V5_HEX) {
    throw new MessageProtocolError(`not a pf.ptr v5 memo: type=${memoType} format=${memoFormat}`);
  }
  return decodePointer(Buffer.from(memoData, 'hex'), opts);
}

function decodeLegacyPointerV4Memo(memo) {
  const memoType = String(memo?.memo_type || '').toLowerCase();
  const memoFormat = String(memo?.memo_format || '').toLowerCase();
  const memoData = String(memo?.memo_data || '');
  if (memoType !== MEMO_TYPE_PF_PTR_HEX || memoFormat !== MEMO_FORMAT_V4_HEX) {
    throw new MessageProtocolError(`not a pf.ptr v4 memo: type=${memoType} format=${memoFormat}`);
  }
  if (!/^[0-9a-fA-F]*$/.test(memoData) || memoData.length % 2 !== 0) {
    throw new MessageProtocolError('invalid hex in legacy pf.ptr v4 memo_data');
  }
  return decodeLegacyPointerV4(Buffer.from(memoData, 'hex'));
}

function encodeX402Header(rawBytes) {
  return asBuf(rawBytes).toString('base64url');
}

function decodeX402Header(headerValue) {
  if (typeof headerValue !== 'string' || headerValue.trim().length === 0) {
    throw new MessageProtocolError('x402 header value must be non-empty');
  }
  const trimmed = headerValue.trim();
  if (!/^[A-Za-z0-9_-]+$/.test(trimmed)) {
    throw new MessageProtocolError('x402 header must be unpadded base64url');
  }
  return Buffer.from(trimmed, 'base64url');
}

// ---------------------------------------------------------------------------
// High-level PQ operations (real ML-DSA-65 + IETF X-Wing + XChaCha20-Poly1305)
// ---------------------------------------------------------------------------

async function signMessageKeyBinding({
  chain_id,
  account,
  xwing_public_key,
  created_at_unix_ms,
  key_epoch = 1,
  account_pubkey,
  account_secret_key,
}) {
  const xwingPk = asBuf(xwing_public_key);
  if (xwingPk.length !== XWING_PUBKEY_BYTES) {
    throw new MessageProtocolError(`xwing_public_key must be ${XWING_PUBKEY_BYTES} bytes`);
  }
  const key_id = computeRecipientKeyId(xwingPk);
  const { mlDsa65 } = await loadPqModules();
  const preimage = computeKeyBindingPreimage({
    chain_id,
    account,
    xwing_public_key: xwingPk,
    key_id,
    created_at_unix_ms,
    key_epoch,
  });
  const signature = Buffer.from(mlDsa65.sign(preimage, asBuf(account_secret_key)));
  const binding = {
    chain_id,
    account,
    xwing_public_key: xwingPk,
    key_id,
    created_at_unix_ms,
    key_epoch,
    signature_algorithm: SignatureAlgorithm.ML_DSA_65,
    account_pubkey: asBuf(account_pubkey),
    signature,
  };
  return { binding, bytes: encodeMessageKeyBinding(binding) };
}

function computeCekCommitment(cek, aadContext) {
  const cekBuf = asBuf(cek);
  const aadBuf = asBuf(aadContext);
  if (cekBuf.length !== AEAD_KEY_BYTES) {
    throw new MessageProtocolError(`cek must be ${AEAD_KEY_BYTES} bytes, got ${cekBuf.length}`);
  }
  if (aadBuf.length !== 32) {
    throw new MessageProtocolError(`aadContext must be 32 bytes, got ${aadBuf.length}`);
  }
  return sha256(Buffer.concat([DOMAIN_CEK_COMMIT, Buffer.from([0x00]), cekBuf, aadBuf]));
}

function computeSealedRecipientTag(xwingSharedSecret, aadContext) {
  const ssBuf = asBuf(xwingSharedSecret);
  const aadBuf = asBuf(aadContext);
  if (ssBuf.length !== 32) {
    throw new MessageProtocolError(`xwingSharedSecret must be 32 bytes, got ${ssBuf.length}`);
  }
  if (aadBuf.length !== 32) {
    throw new MessageProtocolError(`aadContext must be 32 bytes, got ${aadBuf.length}`);
  }
  return sha256(Buffer.concat([DOMAIN_SEALED_RECIPIENT_TAG, Buffer.from([0x00]), ssBuf, aadBuf]));
}

async function generateRotatableXWingKeypair() {
  const { xwing } = await loadPqModules();
  const seed = crypto.randomBytes(32);
  const kp = xwing.keygen(seed);
  const publicKey = Buffer.from(kp.publicKey);
  const secretKey = Buffer.from(kp.secretKey);
  return { seed, publicKey, secretKey, keyId: computeRecipientKeyId(publicKey) };
}

async function rotateMessageKeyBinding({
  chain_id,
  account,
  new_xwing_public_key,
  previous_key_epoch,
  new_key_epoch = null,
  created_at_unix_ms,
  account_pubkey,
  account_secret_key,
}) {
  const targetEpoch = new_key_epoch === null || new_key_epoch === undefined
    ? previous_key_epoch + 1
    : new_key_epoch;
  if (!Number.isInteger(targetEpoch) || targetEpoch < 1 || targetEpoch <= previous_key_epoch) {
    throw new MessageProtocolError(
      `rotated key_epoch (${targetEpoch}) must be >= 1 and strictly greater than previous_key_epoch (${previous_key_epoch})`
    );
  }
  return signMessageKeyBinding({
    chain_id,
    account,
    xwing_public_key: new_xwing_public_key,
    created_at_unix_ms,
    key_epoch: targetEpoch,
    account_pubkey,
    account_secret_key,
  });
}

async function verifyMessageKeyBinding(
  bindingOrBytes,
  { expectedChainId = null, expectedAccount = null, minKnownEpoch = 0 } = {}
) {
  const binding = Buffer.isBuffer(bindingOrBytes) || bindingOrBytes instanceof Uint8Array
    ? decodeMessageKeyBinding(bindingOrBytes)
    : bindingOrBytes;
  if (binding.signature_algorithm !== SignatureAlgorithm.ML_DSA_65) {
    throw new MessageProtocolError('MessageKeyBinding requires ML-DSA-65');
  }
  const expectedKeyId = computeRecipientKeyId(binding.xwing_public_key);
  if (binding.key_id.length !== 32 || !crypto.timingSafeEqual(binding.key_id, expectedKeyId)) {
    throw new MessageProtocolError('MessageKeyBinding key_id does not match SHA-256(xwing_public_key)');
  }
  // The signature below only proves that account_pubkey signed. Without this, anyone could
  // bind an encryption key of their own to somebody else's account.
  if (binding.account !== deriveAccountAddress(binding.account_pubkey)) {
    throw new MessageProtocolError('MessageKeyBinding account_pubkey is not the key of the named account');
  }
  if (expectedChainId !== null && expectedChainId !== undefined && binding.chain_id !== expectedChainId) {
    throw new MessageProtocolError(
      `MessageKeyBinding chain_id mismatch: got ${JSON.stringify(binding.chain_id)}, expected ${JSON.stringify(expectedChainId)}`
    );
  }
  if (expectedAccount !== null && expectedAccount !== undefined && binding.account !== expectedAccount) {
    throw new MessageProtocolError(
      `MessageKeyBinding account mismatch: got ${JSON.stringify(binding.account)}, expected ${JSON.stringify(expectedAccount)}`
    );
  }
  if (binding.key_epoch < minKnownEpoch) {
    throw new MessageProtocolError(
      `MessageKeyBinding key_epoch rollback rejected: got ${binding.key_epoch} < minKnownEpoch ${minKnownEpoch}`
    );
  }
  const { mlDsa65 } = await loadPqModules();
  const preimage = computeKeyBindingPreimage(binding);
  const valid = mlDsa65.verify(asBuf(binding.signature), preimage, asBuf(binding.account_pubkey));
  if (!valid) {
    throw new MessageProtocolError('MessageKeyBinding ML-DSA-65 signature invalid');
  }
  return binding;
}

async function sealEnvelope({
  header,
  bodyPlaintext,
  senderPubkey,
  senderSecretKey = null,
  signFn = null,
  recipients = [],
  chunkIndex = 0,
  pointerTarget = PointerTarget.CONTENT_BLOB,
  extraPointerFlags = 0,
  cek = null,
  aeadNonce = null,
  blindSealedRecipients = true,
  emitCekCommitment = true,
}) {
  if (header.protocol_version !== 1) {
    throw new MessageProtocolError('MessageHeader.protocol_version must be 1');
  }
  const { mlDsa65, xwing } = await loadPqModules();
  const headerBytes = encodeMessageHeader(header);
  const bodyBuf = asBuf(bodyPlaintext);
  let encryptionHeaderBytes = Buffer.alloc(0);
  let bodyBytes = bodyBuf;
  let visFlag = 0;

  if (header.visibility === Visibility.PUBLIC) {
    if (header.cipher_suite !== CipherSuite.NONE) {
      throw new MessageProtocolError('VISIBILITY_PUBLIC requires CIPHER_SUITE_NONE');
    }
    if (recipients.length > 0) {
      throw new MessageProtocolError('VISIBILITY_PUBLIC cannot include encrypted recipients');
    }
    visFlag = PointerFlag.PUBLIC;
  } else if (header.visibility === Visibility.PRIVATE || header.visibility === Visibility.SEALED) {
    if (header.cipher_suite !== CipherSuite.XWING_XCHACHA20_POLY1305) {
      throw new MessageProtocolError('PRIVATE/SEALED requires CIPHER_SUITE_XWING_XCHACHA20_POLY1305');
    }
    if (!Array.isArray(recipients) || recipients.length === 0) {
      throw new MessageProtocolError('PRIVATE/SEALED requires at least one recipient');
    }
    const contentKey = cek ? asBuf(cek) : crypto.randomBytes(AEAD_KEY_BYTES);
    const bodyNonce = aeadNonce ? asBuf(aeadNonce) : crypto.randomBytes(AEAD_NONCE_BYTES);
    const aadContext = computeAadContext(header.chain_id, headerBytes, chunkIndex);
    const cekCommitment = emitCekCommitment ? computeCekCommitment(contentKey, aadContext) : Buffer.alloc(0);
    bodyBytes = await xchacha20Poly1305Encrypt(contentKey, bodyNonce, bodyBuf, aadContext);

    const wraps = [];
    const seenIds = new Set();
    for (const recipient of recipients) {
      const xwingPk = asBuf(recipient.xwingPublicKey || recipient.xwing_public_key);
      const staticKeyId = computeRecipientKeyId(xwingPk);
      const keyHex = staticKeyId.toString('hex');
      if (seenIds.has(keyHex)) throw new MessageProtocolError('duplicate recipient_key_id');
      seenIds.add(keyHex);

      const encap = recipient.encapFn
        ? await recipient.encapFn(xwingPk)
        : xwing.encapsulate(xwingPk);
      const xwingCt = Buffer.from(encap.cipherText);
      const sharedSecret = Buffer.from(encap.sharedSecret);
      const wireTag =
        header.visibility === Visibility.SEALED && blindSealedRecipients
          ? computeSealedRecipientTag(sharedSecret, aadContext)
          : staticKeyId;
      const kek = deriveXWingWrapKek(sharedSecret, aadContext, wireTag);
      const wrapNonce = crypto.randomBytes(AEAD_NONCE_BYTES);
      const wrappedCek = await xchacha20Poly1305Encrypt(kek, wrapNonce, contentKey, aadContext);
      wraps.push({
        recipient_key_id: wireTag,
        recipient_account: header.visibility === Visibility.SEALED ? '' : String(recipient.account || ''),
        xwing_ciphertext: xwingCt,
        wrap_nonce: wrapNonce,
        wrapped_cek: wrappedCek,
      });
    }
    wraps.sort((a, b) => Buffer.compare(a.recipient_key_id, b.recipient_key_id));
    encryptionHeaderBytes = encodeXWingEncryptionHeader({
      cipher_suite: CipherSuite.XWING_XCHACHA20_POLY1305,
      aead_nonce: bodyNonce,
      aad_context: aadContext,
      chunk_index: chunkIndex,
      kdf_info: DOMAIN_XWING_WRAP_INFO,
      recipients: wraps,
      cek_commitment: cekCommitment,
    });
    visFlag = header.visibility === Visibility.SEALED ? PointerFlag.SEALED : PointerFlag.PRIVATE;
  } else {
    throw new MessageProtocolError(`invalid visibility: ${header.visibility}`);
  }

  const preimage = envelopeSigningPreimage(1, headerBytes, encryptionHeaderBytes, bodyBytes);
  const signature = signFn
    ? asBuf(await signFn(preimage))
    : Buffer.from(mlDsa65.sign(preimage, asBuf(senderSecretKey)));

  const envelope = {
    version: 1,
    header_bytes: headerBytes,
    encryption_header_bytes: encryptionHeaderBytes,
    body_bytes: bodyBytes,
    signature_algorithm: SignatureAlgorithm.ML_DSA_65,
    sender_pubkey: asBuf(senderPubkey),
    signature,
  };
  const envelopeBytes = encodeEnvelope(envelope);
  const { cidBin, multibaseCid } = cidv1RawFromBytes(envelopeBytes);

  let ptrFlags = visFlag | extraPointerFlags;
  if (pointerTarget === PointerTarget.MANIFEST) ptrFlags |= PointerFlag.MULTIPART;
  const pointer = {
    cid_bin: cidBin,
    target: pointerTarget,
    kind: header.kind,
    schema: header.schema_id,
    task_id: header.task_id || '',
    thread_id: header.thread_id || '',
    context_id: header.context_id || '',
    flags: ptrFlags,
    inline_payload: Buffer.alloc(0),
    schema_name: header.schema_id === SCHEMA_ID.CUSTOM ? (header.schema_name || '') : '',
  };
  const memo = encodePointerV5Memo(pointer);
  return { envelope, envelopeBytes, cidBin, multibaseCid, pointer, memo };
}

async function openEnvelope(
  envelopeBytes,
  {
    expectedCidBin = null,
    expectedMultibaseCid = null,
    expectedChainId = null,
    expectedTxSender = null,
    verifySenderAddress = true,
    verifySignatureFn = null,
    recipientKeyId = null,
    recipientXWingSecretKey = null,
    xwingDecapFn = null,
    enforceCanonicalOrder = false,
    requireCekCommitment = false,
  } = {}
) {
  const raw = asBuf(envelopeBytes);
  // Step 1: Verify CIDv1 raw over exact received bytes (zero re-serialization).
  const cidRef = verifyCidv1Raw(raw, { expectedCidBin, expectedMultibaseCid });

  // Step 2: Decode outer Envelope only and verify ML-DSA-65 signature over raw byte fields BEFORE decoding nested messages.
  const envelope = decodeEnvelope(raw, { enforceCanonicalOrder });
  if (envelope.version !== 1) throw new MessageProtocolError(`unsupported Envelope version: ${envelope.version}`);
  if (envelope.signature_algorithm !== SignatureAlgorithm.ML_DSA_65) {
    throw new MessageProtocolError(`unsupported signature_algorithm: ${envelope.signature_algorithm}`);
  }
  const preimage = envelopeSigningPreimage(
    envelope.version,
    envelope.header_bytes,
    envelope.encryption_header_bytes,
    envelope.body_bytes
  );
  const { mlDsa65, xwing } = await loadPqModules();
  const sigValid = verifySignatureFn
    ? Boolean(await verifySignatureFn(preimage, envelope.signature, envelope.sender_pubkey))
    : mlDsa65.verify(envelope.signature, preimage, envelope.sender_pubkey);
  if (!sigValid) {
    throw new MessageProtocolError('Envelope raw-byte signature verification failed');
  }

  // Step 3: Decode nested MessageHeader only after signature verification passes.
  const header = decodeMessageHeader(envelope.header_bytes, { enforceCanonicalOrder });
  if (header.protocol_version !== 1) {
    throw new MessageProtocolError(`unsupported MessageHeader.protocol_version: ${header.protocol_version}`);
  }

  // Step 4: Enforce sender-key derivation, chain_id, and anchoring tx.from bindings.
  if (verifySenderAddress) {
    const derivedSender = deriveAccountAddress(envelope.sender_pubkey);
    if (derivedSender !== header.sender_account) {
      throw new MessageProtocolError(
        `Envelope sender_pubkey address ${derivedSender} != header.sender_account ${header.sender_account}`
      );
    }
  }
  if (expectedChainId !== null && expectedChainId !== undefined && header.chain_id !== expectedChainId) {
    throw new MessageProtocolError(
      `Envelope chain_id mismatch: got ${JSON.stringify(header.chain_id)}, expected ${JSON.stringify(expectedChainId)}`
    );
  }
  if (expectedTxSender !== null && expectedTxSender !== undefined && header.sender_account !== expectedTxSender) {
    throw new MessageProtocolError(
      `Foreign re-anchor rejected: header.sender_account ${header.sender_account} != anchoring tx.from ${expectedTxSender}`
    );
  }

  if (header.visibility === Visibility.PUBLIC) {
    if (envelope.encryption_header_bytes.length !== 0) {
      throw new MessageProtocolError('VISIBILITY_PUBLIC envelope must have empty encryption_header_bytes');
    }
    return {
      envelope,
      header,
      encryptionHeader: null,
      plaintextBody: envelope.body_bytes,
      cidRef,
    };
  }

  if (envelope.encryption_header_bytes.length === 0) {
    throw new MessageProtocolError('PRIVATE/SEALED envelope missing encryption_header_bytes');
  }
  const encryptionHeader = decodeXWingEncryptionHeader(envelope.encryption_header_bytes, { enforceCanonicalOrder });
  if (header.visibility === Visibility.SEALED) {
    for (const wrap of encryptionHeader.recipients) {
      if (wrap.recipient_account) {
        throw new MessageProtocolError('VISIBILITY_SEALED must omit RecipientWrap.recipient_account');
      }
    }
  }

  const expectedAad = computeAadContext(header.chain_id, envelope.header_bytes, encryptionHeader.chunk_index);
  if (
    encryptionHeader.aad_context.length !== 32 ||
    !crypto.timingSafeEqual(encryptionHeader.aad_context, expectedAad)
  ) {
    throw new MessageProtocolError('XWingEncryptionHeader.aad_context mismatch');
  }

  if (!recipientXWingSecretKey && !xwingDecapFn) {
    throw new MessageProtocolError('recipientXWingSecretKey (or xwingDecapFn) is required');
  }
  const decapWrap = async (ct) =>
    xwingDecapFn
      ? asBuf(await xwingDecapFn(ct))
      : Buffer.from(xwing.decapsulate(ct, asBuf(recipientXWingSecretKey)));

  let wrap = null;
  let sharedSecret = null;
  if (recipientKeyId) {
    const targetId = asBuf(recipientKeyId);
    wrap = encryptionHeader.recipients.find(
      (w) => w.recipient_key_id.length === targetId.length && crypto.timingSafeEqual(w.recipient_key_id, targetId)
    ) || null;
  }
  if (wrap) {
    sharedSecret = await decapWrap(wrap.xwing_ciphertext);
  } else if (header.visibility === Visibility.SEALED) {
    // Blinded SEALED mode: trial-decapsulate wraps and match per-message unlinkable tag.
    for (const candidate of encryptionHeader.recipients) {
      if (candidate.recipient_key_id.length !== 32) continue;
      let ss;
      try {
        ss = await decapWrap(candidate.xwing_ciphertext);
      } catch {
        continue;
      }
      if (ss.length === 32) {
        const expectedTag = computeSealedRecipientTag(ss, expectedAad);
        if (crypto.timingSafeEqual(expectedTag, candidate.recipient_key_id)) {
          wrap = candidate;
          sharedSecret = ss;
          break;
        }
      }
    }
    if (!wrap || !sharedSecret) throw new MessageProtocolError('no matching RecipientWrap for recipientKeyId');
  } else {
    if (!recipientKeyId) {
      throw new MessageProtocolError('recipientKeyId and recipientXWingSecretKey (or xwingDecapFn) are required');
    }
    throw new MessageProtocolError('no matching RecipientWrap for recipientKeyId');
  }

  const kek = deriveXWingWrapKek(
    sharedSecret,
    expectedAad,
    wrap.recipient_key_id,
    encryptionHeader.kdf_info || DOMAIN_XWING_WRAP_INFO
  );
  const cek = await xchacha20Poly1305Decrypt(kek, wrap.wrap_nonce, wrap.wrapped_cek, expectedAad);
  if (encryptionHeader.cek_commitment && encryptionHeader.cek_commitment.length > 0) {
    const expectedCommit = computeCekCommitment(cek, expectedAad);
    if (
      encryptionHeader.cek_commitment.length !== 32 ||
      !crypto.timingSafeEqual(encryptionHeader.cek_commitment, expectedCommit)
    ) {
      throw new MessageProtocolError('XWingEncryptionHeader.cek_commitment mismatch');
    }
  } else if (requireCekCommitment) {
    throw new MessageProtocolError('XWingEncryptionHeader.cek_commitment missing');
  }
  const plaintextBody = await xchacha20Poly1305Decrypt(cek, encryptionHeader.aead_nonce, envelope.body_bytes, expectedAad);
  return {
    envelope,
    header,
    encryptionHeader,
    plaintextBody,
    cidRef,
  };
}

async function splitManifestChunks(
  payload,
  {
    chainId,
    headerBytes,
    mediaType = 'application/octet-stream',
    innerSchemaName = '',
    innerBodyEncoding = BodyEncoding.RAW_BYTES,
    cipherSuite = CipherSuite.XWING_XCHACHA20_POLY1305,
    cek = null,
    chunkSizeBytes = MAX_SINGLE_ENVELOPE_BYTES - AEAD_TAG_BYTES,
  }
) {
  const buf = asBuf(payload);
  if (buf.length > MAX_MANIFEST_PLAINTEXT_BYTES) {
    throw new MessageProtocolError(`payload size ${buf.length} exceeds 16 MiB Manifest limit`);
  }
  const encrypted = cipherSuite === CipherSuite.XWING_XCHACHA20_POLY1305;
  const maxChunk = encrypted ? MAX_SINGLE_ENVELOPE_BYTES - AEAD_TAG_BYTES : MAX_SINGLE_ENVELOPE_BYTES;
  if (!Number.isInteger(chunkSizeBytes) || chunkSizeBytes <= 0 || chunkSizeBytes > maxChunk) {
    throw new MessageProtocolError(`chunkSizeBytes ${chunkSizeBytes} exceeds per-block limit ${maxChunk}`);
  }
  const keyBuf = encrypted ? asBuf(cek) : null;
  if (encrypted && keyBuf.length !== AEAD_KEY_BYTES) {
    throw new MessageProtocolError('32-byte cek required for encrypted Manifest chunks');
  }

  const blocks = [];
  const chunks = [];
  let idx = 0;
  for (let offset = 0; offset < Math.max(buf.length, 1); offset += chunkSizeBytes) {
    const slice = buf.subarray(offset, offset + chunkSizeBytes);
    let block = slice;
    let nonce = Buffer.alloc(0);
    if (encrypted) {
      nonce = crypto.randomBytes(AEAD_NONCE_BYTES);
      const aad = computeAadContext(chainId, headerBytes, idx);
      block = await xchacha20Poly1305Encrypt(keyBuf, nonce, slice, aad);
    }
    const { cidBin, multibaseCid } = cidv1RawFromBytes(block);
    blocks.push(block);
    chunks.push({
      chunk_index: idx,
      cid: { cid_bin: cidBin, byte_length: block.length, multibase_cid: multibaseCid },
      aead_nonce: nonce,
    });
    idx += 1;
    if (buf.length === 0) break;
  }

  const manifest = {
    version: 1,
    total_plaintext_bytes: buf.length,
    chunk_size_bytes: chunkSizeBytes,
    media_type: mediaType,
    inner_schema_name: innerSchemaName,
    inner_body_encoding: innerBodyEncoding,
    cipher_suite: cipherSuite,
    chunks,
  };
  validateManifest(manifest);
  return { manifest, manifestBytes: encodeManifest(manifest), blocks };
}

async function assembleManifestChunks(manifest, blocks, { chainId, headerBytes, cek = null }) {
  validateManifest(manifest);
  if (!Array.isArray(blocks) || blocks.length !== manifest.chunks.length) {
    throw new MessageProtocolError('blocks length does not match manifest.chunks length');
  }
  const encrypted = manifest.cipher_suite === CipherSuite.XWING_XCHACHA20_POLY1305;
  const keyBuf = encrypted ? asBuf(cek) : null;
  if (encrypted && keyBuf.length !== AEAD_KEY_BYTES) {
    throw new MessageProtocolError('32-byte cek required to decrypt Manifest chunks');
  }
  const parts = [];
  for (let i = 0; i < manifest.chunks.length; i += 1) {
    const chunk = manifest.chunks[i];
    const block = asBuf(blocks[i]);
    verifyCidv1Raw(block, {
      expectedCidBin: chunk.cid.cid_bin,
      expectedMultibaseCid: chunk.cid.multibase_cid || null,
    });
    if (block.length !== chunk.cid.byte_length) {
      throw new MessageProtocolError(`chunk ${chunk.chunk_index} byte_length mismatch`);
    }
    if (encrypted) {
      const aad = computeAadContext(chainId, headerBytes, chunk.chunk_index);
      parts.push(await xchacha20Poly1305Decrypt(keyBuf, chunk.aead_nonce, block, aad));
    } else {
      parts.push(block);
    }
  }
  const out = Buffer.concat(parts);
  if (out.length !== manifest.total_plaintext_bytes) {
    throw new MessageProtocolError(`reassembled length ${out.length} != total_plaintext_bytes ${manifest.total_plaintext_bytes}`);
  }
  return out;
}

module.exports = {
  // Constants & Enums
  MAX_PAYMENT_MEMOS,
  MAX_PAYMENT_MEMO_TYPE_BYTES,
  MAX_PAYMENT_MEMO_FORMAT_BYTES,
  MAX_PAYMENT_MEMO_DATA_BYTES,
  MAX_PAYMENT_MEMO_TOTAL_BYTES,
  MAX_SINGLE_ENVELOPE_BYTES,
  MAX_MANIFEST_PLAINTEXT_BYTES,
  ML_DSA_65_PUBKEY_BYTES,
  ML_DSA_65_SIGNATURE_BYTES,
  XWING_PUBKEY_BYTES,
  XWING_CIPHERTEXT_BYTES,
  AEAD_KEY_BYTES,
  AEAD_NONCE_BYTES,
  AEAD_TAG_BYTES,
  WRAPPED_CEK_BYTES,
  CIDV1_RAW_PREFIX,
  CIDV1_RAW_BYTES,
  DOMAIN_ENVELOPE_SIGN,
  DOMAIN_MESSAGE_AAD,
  DOMAIN_XWING_WRAP_INFO,
  DOMAIN_MESSAGE_KEY_KDF,
  DOMAIN_MESSAGE_KEY_BIND,
  DOMAIN_CEK_COMMIT,
  DOMAIN_SEALED_RECIPIENT_TAG,
  MEMO_TYPE_PF_PTR_HEX,
  MEMO_FORMAT_V5_HEX,
  MEMO_FORMAT_V4_HEX,
  SCHEMA_ID,
  ContentKind,
  PointerTarget,
  PointerFlag,
  VISIBILITY_FLAG_MASK,
  ALLOWED_POINTER_FLAG_MASK,
  Visibility,
  CipherSuite,
  SignatureAlgorithm,
  BodyEncoding,
  PaymentLane,
  X402Scheme,
  MessageProtocolError,
  // Wire & crypto primitives
  loadPqModules,
  encodeVarint,
  decodeVarint,
  decodeWireFields,
  sha256,
  sha3_384,
  hkdfSha256,
  canonicalJsonString,
  canonicalJsonBytes,
  cidBinToMultibase,
  multibaseToCidBin,
  cidv1RawFromBytes,
  verifyCidv1Raw,
  envelopeSigningPreimage,
  computeAadContext,
  deriveMessageKemSeed,
  deriveMessageKemKeypair,
  computeRecipientKeyId,
  computeCekCommitment,
  computeSealedRecipientTag,
  deriveXWingWrapKek,
  generateRotatableXWingKeypair,
  deriveAccountAddress,
  computeKeyBindingPreimage,
  xchacha20Poly1305Encrypt,
  xchacha20Poly1305Decrypt,
  validatePointerV5Flags,
  validateL1PaymentMemo,
  // Protobuf message codecs
  encodeIpfsCidRef,
  decodeIpfsCidRef,
  encodePointer,
  decodePointer,
  encodeLegacyPointerV4,
  decodeLegacyPointerV4,
  encodeRecipientWrap,
  decodeRecipientWrap,
  encodeXWingEncryptionHeader,
  decodeXWingEncryptionHeader,
  encodeMessageKeyBinding,
  decodeMessageKeyBinding,
  encodeFastPayPaymentRef,
  decodeFastPayPaymentRef,
  encodeX402PaymentRequired,
  decodeX402PaymentRequired,
  encodeX402PaymentSignature,
  decodeX402PaymentSignature,
  encodeX402PaymentResponse,
  decodeX402PaymentResponse,
  encodeMessageHeader,
  decodeMessageHeader,
  encodeEnvelope,
  decodeEnvelope,
  encodeManifestChunk,
  decodeManifestChunk,
  encodeManifest,
  decodeManifest,
  encodeTextMessage,
  decodeTextMessage,
  encodeRewardNotice,
  decodeRewardNotice,
  // Memo & x402 headers
  encodePointerV5Memo,
  decodePointerV5Memo,
  decodeLegacyPointerV4Memo,
  encodeX402Header,
  decodeX402Header,
  // High-level PQ envelope & manifest API
  signMessageKeyBinding,
  rotateMessageKeyBinding,
  verifyMessageKeyBinding,
  sealEnvelope,
  openEnvelope,
  splitManifestChunks,
  assembleManifestChunks,
};
