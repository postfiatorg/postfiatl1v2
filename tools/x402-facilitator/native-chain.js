'use strict';

// Native Post Fiat L1v2 chain signing, hashing, and FastPay v3 certificate
// verification helpers matching Rust (`crates/types`, `crates/execution`)
// byte-for-byte.

const crypto = require('node:crypto');
const mp = require('../../wallet-proxy/message-protocol');

const TX_SIGNATURE_CONTEXT = Buffer.from('postfiat-l1-v2/tx/v1', 'utf8');
const OWNED_TRANSFER_CONTEXT_V3 = Buffer.from('postfiat-l1-v2/owned-transfer/v3', 'utf8');
const FASTPAY_APPLY_ACK_CONTEXT_V1 = Buffer.from('postfiat-l1-v2/fastpay-apply-ack/v1', 'utf8');

const ADDRESS_NAMESPACE = 'pf';
const PAYMENT_V2_TRANSACTION_KIND = 'payment_v2';
const ML_DSA_65_ALGORITHM = 'ML-DSA-65';
const PAYMENT_V2_TX_ID_DOMAIN = 'postfiat.payment_v2.tx_id.v1';

const OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3 = 'postfiat-owned-certificate-domain-v3';
const OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2 = 'postfiat-owned-certificate-domain-v2';
const FASTPAY_ORDER_RECOVERY_SCHEMA_V1 = 'postfiat-fastpay-order-recovery-v1';
const FASTPAY_APPLY_ACK_SCHEMA_V1 = 'postfiat-fastpay-apply-ack-v1';
const FASTPAY_LOCK_ID_DOMAIN_V1 = 'postfiat.fastpay.lock-id.v1';
const FASTPAY_TRANSFER_ORDER_DOMAIN_V3 = 'postfiat.fastpay.transfer-order.v3';
const FASTPAY_CERTIFICATE_DIGEST_DOMAIN_V1 = 'postfiat.fastpay.certificate.v1';

function hashDomainBytes(domain, bytes) {
  return mp.sha3_384(Buffer.concat([Buffer.from(String(domain), 'utf8'), Buffer.from([0]), Buffer.from(bytes)]));
}

function hashDomainHex(domain, bytes) {
  return hashDomainBytes(domain, bytes).toString('hex');
}

function u64le(value) {
  const out = Buffer.alloc(8);
  out.writeBigUInt64LE(BigInt(value));
  return out;
}

function u32le(value) {
  const out = Buffer.alloc(4);
  out.writeUInt32LE(Number(value));
  return out;
}

function lenText(value = '') {
  const bytes = Buffer.from(String(value), 'utf8');
  return Buffer.concat([u64le(bytes.length), bytes]);
}

function hexByteLen(hexStr) {
  const s = String(hexStr || '');
  if (s.length % 2 !== 0 || !/^[0-9a-f]*$/i.test(s)) {
    throw new Error('invalid hex memo field');
  }
  return s.length / 2;
}

// ---------------------------------------------------------------------------
// Native Consensus SignedPaymentV2 (matches crates/types + crates/execution)
// ---------------------------------------------------------------------------

function buildUnsignedPaymentV2({
  chainId,
  genesisHash,
  protocolVersion = 1,
  from,
  to,
  amount,
  fee,
  sequence,
  memos = [],
}) {
  return {
    chain_id: String(chainId),
    genesis_hash: String(genesisHash).toLowerCase(),
    protocol_version: Number(protocolVersion),
    address_namespace: ADDRESS_NAMESPACE,
    transaction_kind: PAYMENT_V2_TRANSACTION_KIND,
    signature_algorithm_id: ML_DSA_65_ALGORITHM,
    from: String(from),
    to: String(to),
    amount: Number(amount),
    fee: Number(fee),
    sequence: Number(sequence),
    memos: (Array.isArray(memos) ? memos : []).map((m) => ({
      memo_type: String(m.memo_type || ''),
      memo_format: String(m.memo_format || ''),
      memo_data: String(m.memo_data || ''),
    })),
  };
}

function paymentV2SigningBytes(unsigned) {
  const memos = Array.isArray(unsigned?.memos) ? unsigned.memos : [];
  const lines = [
    'postfiat.payment.v2',
    `chain_id=${unsigned.chain_id}`,
    `genesis_hash=${unsigned.genesis_hash}`,
    `protocol_version=${unsigned.protocol_version}`,
    `address_namespace=${unsigned.address_namespace}`,
    `transaction_kind=${unsigned.transaction_kind}`,
    `signature_algorithm_id=${unsigned.signature_algorithm_id}`,
    `from=${unsigned.from}`,
    `to=${unsigned.to}`,
    `amount=${unsigned.amount}`,
    `fee=${unsigned.fee}`,
    `sequence=${unsigned.sequence}`,
    `memo_count=${memos.length}`,
  ];
  for (let i = 0; i < memos.length; i += 1) {
    const m = memos[i];
    const mt = String(m.memo_type || '');
    const mf = String(m.memo_format || '');
    const md = String(m.memo_data || '');
    lines.push(
      `memo[${i}].type_bytes=${hexByteLen(mt)}`,
      `memo[${i}].type=${mt}`,
      `memo[${i}].format_bytes=${hexByteLen(mf)}`,
      `memo[${i}].format=${mf}`,
      `memo[${i}].data_bytes=${hexByteLen(md)}`,
      `memo[${i}].data=${md}`
    );
  }
  return Buffer.from(`${lines.join('\n')}\n`, 'utf8');
}

function paymentV2TxIdPreimageBytes(signed) {
  const suffix = `algorithm=${signed.algorithm_id}\npublic_key=${signed.public_key_hex}\nsignature=${signed.signature_hex}\n`;
  return Buffer.concat([paymentV2SigningBytes(signed.unsigned), Buffer.from(suffix, 'utf8')]);
}

function computeNativePaymentV2TxId(signed) {
  return hashDomainBytes(PAYMENT_V2_TX_ID_DOMAIN, paymentV2TxIdPreimageBytes(signed));
}

function isNativeSignedPaymentV2(obj) {
  return (
    Boolean(obj) &&
    typeof obj === 'object' &&
    Boolean(obj.unsigned) &&
    typeof obj.unsigned === 'object' &&
    obj.unsigned.transaction_kind === PAYMENT_V2_TRANSACTION_KIND &&
    typeof obj.public_key_hex === 'string' &&
    typeof obj.signature_hex === 'string'
  );
}

async function signNativePaymentV2(unsigned, { publicKey, secretKey }) {
  const { mlDsa65 } = await mp.loadPqModules();
  const signingBytes = paymentV2SigningBytes(unsigned);
  const sig = Buffer.from(mlDsa65.sign(signingBytes, Uint8Array.from(secretKey), { context: TX_SIGNATURE_CONTEXT }));
  const signedPaymentV2 = {
    unsigned,
    algorithm_id: ML_DSA_65_ALGORITHM,
    public_key_hex: Buffer.from(publicKey).toString('hex'),
    signature_hex: sig.toString('hex'),
  };
  const txId = computeNativePaymentV2TxId(signedPaymentV2);
  return {
    signedPaymentV2,
    txId,
    txIdHex: txId.toString('hex'),
    signedPayload: Buffer.from(JSON.stringify(signedPaymentV2), 'utf8'),
    signature: sig,
  };
}

async function verifyNativePaymentV2(signed) {
  if (!isNativeSignedPaymentV2(signed)) {
    return { ok: false, reason: 'invalid_native_payment_v2_shape' };
  }
  if (signed.algorithm_id !== ML_DSA_65_ALGORITHM || signed.unsigned.signature_algorithm_id !== ML_DSA_65_ALGORITHM) {
    return { ok: false, reason: 'unsupported_signature_algorithm' };
  }
  const pubkeyBuf = Buffer.from(String(signed.public_key_hex || ''), 'hex');
  const sigBuf = Buffer.from(String(signed.signature_hex || ''), 'hex');
  if (pubkeyBuf.length !== mp.ML_DSA_65_PUBKEY_BYTES || sigBuf.length !== mp.ML_DSA_65_SIGNATURE_BYTES) {
    return { ok: false, reason: 'invalid_payer_signature_format' };
  }
  const derivedFrom = mp.deriveAccountAddress(pubkeyBuf);
  if (signed.unsigned.from !== derivedFrom) {
    return { ok: false, reason: 'payer_address_pubkey_mismatch' };
  }
  const { mlDsa65 } = await mp.loadPqModules();
  let signingBytes;
  try {
    signingBytes = paymentV2SigningBytes(signed.unsigned);
  } catch (err) {
    return { ok: false, reason: `invalid_payment_v2_fields:${err.message}` };
  }
  if (!mlDsa65.verify(sigBuf, signingBytes, pubkeyBuf, { context: TX_SIGNATURE_CONTEXT })) {
    return { ok: false, reason: 'invalid_payer_signature' };
  }
  const txId = computeNativePaymentV2TxId(signed);
  return {
    ok: true,
    txId,
    txIdHex: txId.toString('hex'),
    pubkeyBuf,
    unsigned: signed.unsigned,
  };
}

// ---------------------------------------------------------------------------
// Native FastPay OwnedTransferOrderV3, OwnedTransferCertificateV3 & ApplyAckV1
// ---------------------------------------------------------------------------

function fastpayTransferLockPreimageV1(order) {
  const domain = order.domain || {};
  const recovery = order.recovery || {};
  const inputs = Array.isArray(order.inputs) ? order.inputs : [];
  const outputs = Array.isArray(order.outputs) ? order.outputs : [];
  const memos = Array.isArray(order.memos) ? order.memos : [];
  return Buffer.concat([
    Buffer.from('postfiat.fastpay.transfer-lock-preimage.v1\0', 'utf8'),
    lenText(domain.schema),
    lenText(domain.chain_id),
    lenText(domain.genesis_hash),
    u32le(domain.protocol_version),
    lenText(domain.registry_id),
    u64le(recovery.committee_epoch),
    u64le(recovery.valid_from_height),
    u64le(recovery.expires_at_height),
    u64le(recovery.recovery_closes_at_height),
    u64le(inputs.length),
    ...inputs.flatMap((input) => [lenText(input.id), u64le(input.version)]),
    u64le(outputs.length),
    ...outputs.flatMap((output) => [lenText(output.owner_pubkey_hex), u64le(output.value), lenText(output.asset)]),
    u64le(order.fee),
    u64le(order.nonce),
    u64le(memos.length),
    ...memos.flatMap((memo) => [lenText(memo.memo_type), lenText(memo.memo_format), lenText(memo.memo_data)]),
  ]);
}

function fastpayTransferLockIdV1(order) {
  return hashDomainHex(FASTPAY_LOCK_ID_DOMAIN_V1, fastpayTransferLockPreimageV1(order));
}

function ownedTransferV3SigningBytes(order) {
  const preimage = fastpayTransferLockPreimageV1(order);
  return Buffer.concat([
    Buffer.from('postfiat.owned-transfer.v3\0', 'utf8'),
    u64le(preimage.length),
    preimage,
    lenText(order.recovery?.lock_id),
  ]);
}

function fastpayTransferOrderDigestV3(order) {
  return hashDomainBytes(FASTPAY_TRANSFER_ORDER_DOMAIN_V3, ownedTransferV3SigningBytes(order));
}

function isNativeSignedOwnedTransferOrderV3(obj) {
  return (
    Boolean(obj) &&
    typeof obj === 'object' &&
    Boolean(obj.order) &&
    typeof obj.order === 'object' &&
    Boolean(obj.order.domain) &&
    Boolean(obj.order.recovery) &&
    Array.isArray(obj.order.inputs) &&
    Array.isArray(obj.order.outputs) &&
    typeof obj.owner_pubkey_hex === 'string' &&
    typeof obj.owner_signature_hex === 'string'
  );
}

function isNativeOwnedTransferCertificateV3(obj) {
  return isNativeSignedOwnedTransferOrderV3(obj) && Array.isArray(obj.votes);
}

async function signNativeOwnedTransferOrderV3(orderInput, { publicKey, secretKey }) {
  const order = {
    ...orderInput,
    recovery: {
      schema: FASTPAY_ORDER_RECOVERY_SCHEMA_V1,
      ...orderInput.recovery,
    },
  };
  order.recovery.lock_id = fastpayTransferLockIdV1(order);
  const { mlDsa65 } = await mp.loadPqModules();
  const signingBytes = ownedTransferV3SigningBytes(order);
  const sig = Buffer.from(
    mlDsa65.sign(signingBytes, Uint8Array.from(secretKey), { context: OWNED_TRANSFER_CONTEXT_V3 })
  );
  const signedOrder = {
    order,
    owner_pubkey_hex: Buffer.from(publicKey).toString('hex'),
    owner_signature_hex: sig.toString('hex'),
  };
  const orderHash = fastpayTransferOrderDigestV3(order);
  return {
    signedOrder,
    orderHash,
    orderHashHex: orderHash.toString('hex'),
    lockId: order.recovery.lock_id,
    signedPayload: Buffer.from(JSON.stringify(signedOrder), 'utf8'),
    signature: sig,
  };
}

async function verifyNativeOwnedTransferOrderV3(signedOrder) {
  if (!isNativeSignedOwnedTransferOrderV3(signedOrder)) {
    return { ok: false, reason: 'invalid_native_fastpay_order_shape' };
  }
  const { order } = signedOrder;
  const schema = String(order.domain?.schema || '');
  if (schema !== OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3 && schema !== OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2) {
    return { ok: false, reason: 'invalid_fastpay_domain_schema' };
  }
  const expectedLockId = fastpayTransferLockIdV1(order);
  if (String(order.recovery?.lock_id || '').toLowerCase() !== expectedLockId) {
    return { ok: false, reason: 'fastpay_lock_id_mismatch' };
  }
  const pubkeyBuf = Buffer.from(String(signedOrder.owner_pubkey_hex || ''), 'hex');
  const sigBuf = Buffer.from(String(signedOrder.owner_signature_hex || ''), 'hex');
  if (pubkeyBuf.length !== mp.ML_DSA_65_PUBKEY_BYTES || sigBuf.length !== mp.ML_DSA_65_SIGNATURE_BYTES) {
    return { ok: false, reason: 'invalid_payer_signature_format' };
  }
  const { mlDsa65 } = await mp.loadPqModules();
  const signingBytes = ownedTransferV3SigningBytes(order);
  if (!mlDsa65.verify(sigBuf, signingBytes, pubkeyBuf, { context: OWNED_TRANSFER_CONTEXT_V3 })) {
    return { ok: false, reason: 'invalid_payer_signature' };
  }
  const orderHash = fastpayTransferOrderDigestV3(order);
  return {
    ok: true,
    orderHash,
    orderHashHex: orderHash.toString('hex'),
    lockId: expectedLockId,
    signingBytes,
    pubkeyBuf,
    payerAccount: mp.deriveAccountAddress(pubkeyBuf),
  };
}

async function signNativeOwnedTransferVoteV3(order, { validatorId, secretKey }) {
  const { mlDsa65 } = await mp.loadPqModules();
  const signingBytes = ownedTransferV3SigningBytes(order);
  const sig = Buffer.from(
    mlDsa65.sign(signingBytes, Uint8Array.from(secretKey), { context: OWNED_TRANSFER_CONTEXT_V3 })
  );
  return {
    validator_id: String(validatorId),
    signature_hex: sig.toString('hex'),
  };
}

function fastpayTransferCertificateDigestV3(cert) {
  const votes = [...(Array.isArray(cert.votes) ? cert.votes : [])].sort((a, b) =>
    String(a.validator_id).localeCompare(String(b.validator_id))
  );
  const seen = new Set();
  const voteParts = [];
  for (const vote of votes) {
    const id = String(vote.validator_id || '');
    if (!id || seen.has(id)) {
      throw new Error('duplicate or empty validator_id in FastPay v3 certificate');
    }
    seen.add(id);
    voteParts.push(lenText(id), lenText(String(vote.signature_hex || '')));
  }
  const body = Buffer.concat([
    ownedTransferV3SigningBytes(cert.order),
    lenText(cert.owner_pubkey_hex),
    lenText(cert.owner_signature_hex),
    u64le(votes.length),
    ...voteParts,
  ]);
  return hashDomainBytes(FASTPAY_CERTIFICATE_DIGEST_DOMAIN_V1, body);
}

function domainsMatch(actual, expected) {
  if (!expected) return true;
  if (!actual || typeof actual !== 'object') return false;
  if (expected.schema && actual.schema !== expected.schema) return false;
  if (expected.chain_id && actual.chain_id !== expected.chain_id) return false;
  if (expected.genesis_hash && String(actual.genesis_hash || '').toLowerCase() !== String(expected.genesis_hash).toLowerCase()) return false;
  if (expected.protocol_version !== undefined && Number(actual.protocol_version) !== Number(expected.protocol_version)) return false;
  if (expected.registry_id && String(actual.registry_id || '').toLowerCase() !== String(expected.registry_id).toLowerCase()) return false;
  return true;
}

async function verifyNativeOwnedTransferCertificateV3(cert, { committee, expectedDomain = null, expectedOrderHash = null }) {
  if (!isNativeOwnedTransferCertificateV3(cert)) {
    return { ok: false, reason: 'invalid_native_certificate_shape' };
  }
  if (!committee || !(committee.validators instanceof Map)) {
    return { ok: false, reason: 'missing_committee_for_epoch' };
  }
  if (!domainsMatch(cert.order.domain, expectedDomain)) {
    return { ok: false, reason: 'certificate_domain_mismatch' };
  }
  if (Number(cert.order.recovery?.committee_epoch) !== Number(committee.epoch)) {
    return { ok: false, reason: 'certificate_committee_epoch_mismatch' };
  }
  const ownerCheck = await verifyNativeOwnedTransferOrderV3(cert);
  if (!ownerCheck.ok) {
    return ownerCheck;
  }
  if (expectedOrderHash) {
    const expectedBuf = Buffer.isBuffer(expectedOrderHash) ? expectedOrderHash : Buffer.from(String(expectedOrderHash), 'hex');
    if (expectedBuf.length !== 48 || !crypto.timingSafeEqual(expectedBuf, ownerCheck.orderHash)) {
      return { ok: false, reason: 'certificate_order_hash_mismatch' };
    }
  }

  const { mlDsa65 } = await mp.loadPqModules();
  const seen = new Set();
  let valid = 0;
  for (const vote of cert.votes) {
    const id = String(vote?.validator_id || '');
    if (!id || seen.has(id)) {
      return { ok: false, reason: 'duplicate_or_empty_validator_vote' };
    }
    seen.add(id);
    const pk = committee.validators.get(id);
    if (!pk) {
      return { ok: false, reason: `unknown_committee_validator:${id}` };
    }
    const sigBuf = Buffer.from(String(vote?.signature_hex || ''), 'hex');
    if (sigBuf.length !== mp.ML_DSA_65_SIGNATURE_BYTES) continue;
    if (mlDsa65.verify(sigBuf, ownerCheck.signingBytes, pk, { context: OWNED_TRANSFER_CONTEXT_V3 })) {
      valid += 1;
    }
  }
  if (valid < committee.threshold) {
    return {
      ok: false,
      reason: `insufficient_quorum_signatures:${valid}/${committee.threshold}`,
    };
  }
  return {
    ok: true,
    epoch: committee.epoch,
    quorumCount: valid,
    orderHash: ownerCheck.orderHash,
    certificateDigest: fastpayTransferCertificateDigestV3(cert),
  };
}

function fastpayApplyAckSigningBytesV1(ack) {
  const domain = ack.domain || {};
  return Buffer.concat([
    Buffer.from('postfiat.fastpay.apply-ack.v1\0', 'utf8'),
    lenText(ack.schema),
    lenText(domain.schema),
    lenText(domain.chain_id),
    lenText(domain.genesis_hash),
    lenText(domain.registry_id),
    lenText(ack.lock_id),
    lenText(ack.order_digest),
    lenText(ack.certificate_digest),
    lenText(ack.terminal_state_digest),
    lenText(ack.validator_id),
    u32le(domain.protocol_version),
    u64le(ack.committee_epoch),
  ]);
}

async function signNativeFastPayApplyAckV1(ackWithoutSig, secretKey) {
  const { mlDsa65 } = await mp.loadPqModules();
  const ack = {
    schema: FASTPAY_APPLY_ACK_SCHEMA_V1,
    ...ackWithoutSig,
  };
  const bytes = fastpayApplyAckSigningBytesV1(ack);
  const sig = Buffer.from(
    mlDsa65.sign(bytes, Uint8Array.from(secretKey), { context: FASTPAY_APPLY_ACK_CONTEXT_V1 })
  );
  return {
    ...ack,
    signature_hex: sig.toString('hex'),
  };
}

async function verifyNativeFastPayApplyAckV1(ack, validatorPubkeyBuf) {
  if (!ack || typeof ack !== 'object' || ack.schema !== FASTPAY_APPLY_ACK_SCHEMA_V1) {
    return false;
  }
  const sigBuf = Buffer.from(String(ack.signature_hex || ''), 'hex');
  if (sigBuf.length !== mp.ML_DSA_65_SIGNATURE_BYTES || !Buffer.isBuffer(validatorPubkeyBuf)) {
    return false;
  }
  const { mlDsa65 } = await mp.loadPqModules();
  const bytes = fastpayApplyAckSigningBytesV1(ack);
  return mlDsa65.verify(sigBuf, bytes, validatorPubkeyBuf, { context: FASTPAY_APPLY_ACK_CONTEXT_V1 });
}

module.exports = {
  TX_SIGNATURE_CONTEXT,
  OWNED_TRANSFER_CONTEXT_V3,
  FASTPAY_APPLY_ACK_CONTEXT_V1,
  OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3,
  OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2,
  FASTPAY_ORDER_RECOVERY_SCHEMA_V1,
  FASTPAY_APPLY_ACK_SCHEMA_V1,
  PAYMENT_V2_TX_ID_DOMAIN,
  FASTPAY_LOCK_ID_DOMAIN_V1,
  FASTPAY_TRANSFER_ORDER_DOMAIN_V3,
  FASTPAY_CERTIFICATE_DIGEST_DOMAIN_V1,
  hashDomainBytes,
  hashDomainHex,
  buildUnsignedPaymentV2,
  paymentV2SigningBytes,
  paymentV2TxIdPreimageBytes,
  computeNativePaymentV2TxId,
  isNativeSignedPaymentV2,
  signNativePaymentV2,
  verifyNativePaymentV2,
  fastpayTransferLockPreimageV1,
  fastpayTransferLockIdV1,
  ownedTransferV3SigningBytes,
  fastpayTransferOrderDigestV3,
  isNativeSignedOwnedTransferOrderV3,
  isNativeOwnedTransferCertificateV3,
  signNativeOwnedTransferOrderV3,
  verifyNativeOwnedTransferOrderV3,
  signNativeOwnedTransferVoteV3,
  fastpayTransferCertificateDigestV3,
  domainsMatch,
  verifyNativeOwnedTransferCertificateV3,
  fastpayApplyAckSigningBytesV1,
  signNativeFastPayApplyAckV1,
  verifyNativeFastPayApplyAckV1,
};
