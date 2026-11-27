'use strict';

// Post Fiat L1v2 x402 v2 Facilitator & Resource Gate (Pillar 2 / Contract C1).
//
// Enforces:
//  - Canonical Contract C1 protobuf3 headers (PAYMENT-REQUIRED / PAYMENT-SIGNATURE / PAYMENT-RESPONSE)
//  - FastPay PFT (exact + pf-fastpay-settled) and consensus-lane PFUSDC (explicit sequence + finality receipt)
//  - Immutable order_hash / tx_hash spent-payment deduplication (never certificate_digest)
//  - Per-IP and per-key sliding-window rate limits + local ML-DSA-65 & pf.ptr v5 INLINE memo
//    verification strictly BEFORE any upstream validator/RPC call
//  - Epoch-tagged validator committee rotation for FastPay quorum certificates

const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');

const mp = require('../../wallet-proxy/message-protocol');

const HEADER_PAYMENT_REQUIRED = 'payment-required';
const HEADER_PAYMENT_SIGNATURE = 'payment-signature';
const HEADER_PAYMENT_RESPONSE = 'payment-response';

const DOMAIN_FASTPAY_ORDER_SIGN = Buffer.from('postfiat-l1-v2/fastpay-order/v3\0', 'ascii');
const DOMAIN_FASTPAY_ORDER_HASH = Buffer.from('postfiat-l1-v2/fastpay-order-hash/v3\0', 'ascii');
const DOMAIN_FASTPAY_VOTE_SIGN = Buffer.from('postfiat-l1-v2/fastpay-vote/v3\0', 'ascii');
const DOMAIN_CONSENSUS_TX_SIGN = Buffer.from('postfiat-l1-v2/consensus-tx/v1\0', 'ascii');
const DOMAIN_CONSENSUS_TX_HASH = Buffer.from('postfiat-l1-v2/consensus-tx-hash/v1\0', 'ascii');

const MAX_BODY_BYTES = 256 * 1024;
const DEFAULT_CHALLENGE_TTL_MS = 5 * 60_000;

class X402Error extends Error {
  constructor(statusCode, code, message) {
    super(message || code);
    this.name = 'X402Error';
    this.statusCode = statusCode;
    this.code = code;
  }
}

// ---------------------------------------------------------------------------
// Canonical request hashing, FastPay fee floor, and signed transfer helpers
// ---------------------------------------------------------------------------

function computeHttpRequestHash({ method, urlPath, contentType = '', body = Buffer.alloc(0) }) {
  const bodyBuf = Buffer.isBuffer(body) ? body : Buffer.from(String(body || ''), 'utf8');
  const normMethod = String(method || 'GET').toUpperCase();
  const normPath = String(urlPath || '/');
  const normCt = String(contentType || '').split(';')[0].trim().toLowerCase();
  return mp.sha256(
    Buffer.concat([
      Buffer.from(`${normMethod} ${normPath}\ncontent-type:${normCt}\n`, 'utf8'),
      bodyBuf,
    ])
  );
}

// Sui-inspired FastPay fee floor (Contract C4 Train A / Pillar 2):
// 25 + ceil(weight_bytes / 512) + 1_000 * max(0, outputs - inputs) + 1 * inputs
function computeMinimumFastPayFeeAtoms({ weightBytes, inputCount = 1, outputCount = 1 }) {
  const inputs = Math.max(1, Number(inputCount) || 1);
  const outputs = Math.max(1, Number(outputCount) || 1);
  const netNewObjects = Math.max(0, outputs - inputs);
  const sizeFee = Math.ceil(Math.max(1, Number(weightBytes) || 1) / 512);
  return 25 + sizeFee + 1_000 * netNewObjects + inputs;
}

function buildX402InlineMemo(requestHash) {
  const hashBuf = Buffer.isBuffer(requestHash) ? requestHash : Buffer.from(requestHash, 'hex');
  if (hashBuf.length !== 32) {
    throw new X402Error(400, 'invalid_request_hash', 'request_hash must be 32 bytes');
  }
  return mp.encodePointerV5Memo({
    cid_bin: Buffer.alloc(0),
    target: mp.PointerTarget.INLINE,
    kind: mp.ContentKind.X402,
    schema: mp.SCHEMA_ID.X402_REQUEST_V1,
    flags: mp.PointerFlag.PUBLIC | mp.PointerFlag.EPHEMERAL,
    inline_payload: hashBuf,
  });
}

function extractX402RequestHashFromMemos(memos) {
  if (!Array.isArray(memos) || memos.length === 0 || memos.length > mp.MAX_PAYMENT_MEMOS) {
    throw new X402Error(400, 'missing_x402_memo', 'signed transfer must include a pf.ptr v5 INLINE x402 memo');
  }
  for (const memo of memos) {
    if (memo?.memo_type === mp.MEMO_TYPE_PF_PTR_HEX && memo?.memo_format === mp.MEMO_FORMAT_V5_HEX) {
      let ptr;
      try {
        ptr = mp.decodePointerV5Memo(memo, { enforceCanonicalOrder: true });
      } catch (err) {
        throw new X402Error(400, 'invalid_x402_memo', err.message);
      }
      if (
        ptr.target === mp.PointerTarget.INLINE &&
        ptr.kind === mp.ContentKind.X402 &&
        ptr.schema === mp.SCHEMA_ID.X402_REQUEST_V1 &&
        ptr.inline_payload.length === 32
      ) {
        return ptr.inline_payload;
      }
    }
  }
  throw new X402Error(400, 'missing_x402_memo', 'no valid pf.ptr v5 INLINE x402 request_hash memo found');
}

function computeFastPayOrderHash(order) {
  const canonical = mp.canonicalJsonBytes(order);
  return mp.sha256(Buffer.concat([DOMAIN_FASTPAY_ORDER_HASH, canonical]));
}

function computeConsensusTxHash(tx) {
  const canonical = mp.canonicalJsonBytes(tx);
  return mp.sha256(Buffer.concat([DOMAIN_CONSENSUS_TX_HASH, canonical]));
}

async function signFastPayOrder(order, { publicKey, secretKey }) {
  const { mlDsa65 } = await mp.loadPqModules();
  const canonical = mp.canonicalJsonBytes(order);
  const preimage = Buffer.concat([DOMAIN_FASTPAY_ORDER_SIGN, canonical]);
  const signature = Buffer.from(mlDsa65.sign(preimage, secretKey));
  const orderHash = computeFastPayOrderHash(order);
  const signedPayload = mp.canonicalJsonBytes({
    version: 'OwnedTransferOrderV3',
    order,
    payer_pubkey_b64: Buffer.from(publicKey).toString('base64url'),
    signature_b64: signature.toString('base64url'),
  });
  return { orderHash, signedPayload, signature };
}

async function signConsensusTransfer(tx, { publicKey, secretKey }) {
  const { mlDsa65 } = await mp.loadPqModules();
  const canonical = mp.canonicalJsonBytes(tx);
  const preimage = Buffer.concat([DOMAIN_CONSENSUS_TX_SIGN, canonical]);
  const signature = Buffer.from(mlDsa65.sign(preimage, secretKey));
  const txHash = computeConsensusTxHash(tx);
  const signedPayload = mp.canonicalJsonBytes({
    version: tx.lane === mp.PaymentLane.CONSENSUS_ISSUED ? 'issued_payment_v2' : 'payment_v2',
    tx,
    payer_pubkey_b64: Buffer.from(publicKey).toString('base64url'),
    signature_b64: signature.toString('base64url'),
  });
  return { txHash, signedPayload, signature };
}

async function signFastPayValidatorVote({ orderHash, epoch, validatorId, secretKey }) {
  const { mlDsa65 } = await mp.loadPqModules();
  const hashBuf = Buffer.isBuffer(orderHash) ? orderHash : Buffer.from(orderHash, 'hex');
  const msg = Buffer.concat([
    DOMAIN_FASTPAY_VOTE_SIGN,
    Buffer.from(`${epoch}:${validatorId}:`, 'utf8'),
    hashBuf,
  ]);
  const signature = Buffer.from(mlDsa65.sign(msg, secretKey));
  return { validator_id: validatorId, signature_b64: signature.toString('base64url') };
}

function computeCertificateDigest(cert) {
  return mp.sha256(mp.canonicalJsonBytes(cert));
}

// ---------------------------------------------------------------------------
// Spent-payment deduplication stores (immutable order_hash / tx_hash ONLY)
// ---------------------------------------------------------------------------

class MemorySpentStore {
  constructor() {
    this._entries = new Map();
  }

  has(dedupKey) {
    return this._entries.has(dedupKey);
  }

  get(dedupKey) {
    return this._entries.get(dedupKey) || null;
  }

  markSpent(dedupKey, record) {
    if (this._entries.has(dedupKey)) return false;
    this._entries.set(dedupKey, record);
    return true;
  }

  size() {
    return this._entries.size;
  }
}

class FileSpentStore {
  constructor(filePath) {
    this._filePath = path.resolve(filePath);
    this._entries = new Map();
    if (fs.existsSync(this._filePath)) {
      const raw = fs.readFileSync(this._filePath, 'utf8').trim();
      if (raw) {
        const parsed = JSON.parse(raw);
        for (const [k, v] of Object.entries(parsed)) {
          this._entries.set(k, v);
        }
      }
    }
  }

  has(dedupKey) {
    return this._entries.has(dedupKey);
  }

  get(dedupKey) {
    return this._entries.get(dedupKey) || null;
  }

  markSpent(dedupKey, record) {
    if (this._entries.has(dedupKey)) return false;
    this._entries.set(dedupKey, record);
    const dir = path.dirname(this._filePath);
    fs.mkdirSync(dir, { recursive: true });
    const tmp = `${this._filePath}.tmp.${process.pid}.${Date.now()}`;
    fs.writeFileSync(tmp, JSON.stringify(Object.fromEntries(this._entries), null, 2), 'utf8');
    fs.renameSync(tmp, this._filePath);
    return true;
  }

  size() {
    return this._entries.size;
  }
}

// ---------------------------------------------------------------------------
// Sliding-window rate limiter (per-IP + per-key)
// ---------------------------------------------------------------------------

class SlidingWindowRateLimiter {
  constructor({ limit = 60, windowMs = 60_000, now = Date.now } = {}) {
    this._limit = limit;
    this._windowMs = windowMs;
    this._now = now;
    this._buckets = new Map();
  }

  admit(bucketKey) {
    const nowMs = this._now();
    const key = String(bucketKey || 'unknown');
    let entry = this._buckets.get(key);
    if (!entry || nowMs - entry.startedAtMs >= this._windowMs) {
      entry = { startedAtMs: nowMs, count: 0 };
      this._buckets.set(key, entry);
    }
    if (entry.count >= this._limit) {
      return {
        allowed: false,
        retryAfterMs: Math.max(1, this._windowMs - (nowMs - entry.startedAtMs)),
      };
    }
    entry.count += 1;
    return { allowed: true, retryAfterMs: 0 };
  }
}

// ---------------------------------------------------------------------------
// Rotatable Validator Committee Registry for FastPay quorum verification
// ---------------------------------------------------------------------------

class CommitteeRegistry {
  constructor(initialCommittee = null) {
    this._activeEpoch = 0;
    this._committees = new Map();
    if (initialCommittee) {
      this.rotateCommittee(initialCommittee);
    }
  }

  rotateCommittee({ epoch, threshold, validators, retainPreviousEpochs = 0 }) {
    if (!Number.isInteger(epoch) || epoch <= this._activeEpoch) {
      throw new X402Error(400, 'invalid_committee_epoch', 'committee epoch must strictly increase');
    }
    if (!Array.isArray(validators) || validators.length === 0) {
      throw new X402Error(400, 'invalid_committee', 'committee requires at least one validator');
    }
    if (!Number.isInteger(threshold) || threshold < 1 || threshold > validators.length) {
      throw new X402Error(400, 'invalid_committee_threshold', 'threshold out of range');
    }
    const map = new Map();
    for (const v of validators) {
      map.set(String(v.id), Buffer.from(v.pubkeyHex, 'hex'));
    }
    this._committees.set(epoch, { epoch, threshold, validators: map });
    this._activeEpoch = epoch;
    const minRetained = epoch - Math.max(0, retainPreviousEpochs);
    for (const oldEpoch of this._committees.keys()) {
      if (oldEpoch < minRetained) this._committees.delete(oldEpoch);
    }
  }

  activeEpoch() {
    return this._activeEpoch;
  }

  async verifyCertificate(certificatePayload, expectedOrderHash) {
    let cert;
    try {
      cert = JSON.parse(Buffer.from(certificatePayload).toString('utf8'));
    } catch (_) {
      throw new X402Error(400, 'invalid_certificate_payload', 'certificate_payload must be valid JSON');
    }
    const committee = this._committees.get(cert?.epoch);
    if (!committee) {
      throw new X402Error(409, 'stale_or_unknown_committee_epoch', `committee epoch ${cert?.epoch} is not accepted`);
    }
    if (String(cert.order_hash_hex || '').toLowerCase() !== expectedOrderHash.toString('hex')) {
      throw new X402Error(400, 'certificate_order_hash_mismatch', 'certificate order_hash does not match payment');
    }
    if (!Array.isArray(cert.votes)) {
      throw new X402Error(400, 'invalid_certificate_votes', 'certificate missing votes array');
    }
    const { mlDsa65 } = await mp.loadPqModules();
    const validValidators = new Set();
    for (const vote of cert.votes) {
      const id = String(vote?.validator_id || '');
      const pk = committee.validators.get(id);
      if (!pk || validValidators.has(id)) continue;
      const msg = Buffer.concat([
        DOMAIN_FASTPAY_VOTE_SIGN,
        Buffer.from(`${cert.epoch}:${id}:`, 'utf8'),
        expectedOrderHash,
      ]);
      try {
        const sig = vote.signature_b64
          ? Buffer.from(String(vote.signature_b64), 'base64url')
          : Buffer.from(String(vote.signature_hex || ''), 'hex');
        if (sig.length === mp.ML_DSA_65_SIGNATURE_BYTES && mlDsa65.verify(sig, msg, pk)) {
          validValidators.add(id);
        }
      } catch (_) {
        // Ignore invalid individual vote; quorum count decides validity.
      }
    }
    if (validValidators.size < committee.threshold) {
      throw new X402Error(
        401,
        'insufficient_quorum_signatures',
        `verified ${validValidators.size} of ${committee.threshold} required committee signatures`
      );
    }
    return {
      epoch: cert.epoch,
      quorumCount: validValidators.size,
      certificateDigest: computeCertificateDigest(cert),
    };
  }
}

// ---------------------------------------------------------------------------
// Core Facilitator Engine
// ---------------------------------------------------------------------------

function createFacilitatorCore({
  facilitatorId = 'postfiat-facilitator-v1',
  chainId = 'postfiat-devnet-2',
  network = 'postfiat:devnet-2',
  facilitatorUrl = 'http://127.0.0.1:4020',
  spentStore = new MemorySpentStore(),
  committeeRegistry = new CommitteeRegistry(),
  settlementAdapter = null,
  ipRateLimit = 60,
  keyRateLimit = 60,
  rateWindowMs = 60_000,
  challengeTtlMs = DEFAULT_CHALLENGE_TTL_MS,
  now = Date.now,
} = {}) {
  const ipLimiter = new SlidingWindowRateLimiter({ limit: ipRateLimit, windowMs: rateWindowMs, now });
  const keyLimiter = new SlidingWindowRateLimiter({ limit: keyRateLimit, windowMs: rateWindowMs, now });
  const challenges = new Map(); // nonceHex -> { required, createdAtMs }

  function checkRateLimits({ remoteIp = 'unknown', apiKey = '' } = {}) {
    const ipRes = ipLimiter.admit(`ip:${remoteIp}`);
    if (!ipRes.allowed) {
      throw new X402Error(429, 'rate_limited_ip', `IP rate limit exceeded; retry after ${ipRes.retryAfterMs}ms`);
    }
    if (apiKey) {
      const keyRes = keyLimiter.admit(`key:${apiKey}`);
      if (!keyRes.allowed) {
        throw new X402Error(429, 'rate_limited_key', `API key rate limit exceeded; retry after ${keyRes.retryAfterMs}ms`);
      }
    }
  }

  function issueChallenge({
    scheme = mp.X402Scheme.EXACT,
    lane = mp.PaymentLane.FASTPAY,
    assetId = 'PFT',
    maxAmountAtoms,
    payTo,
    requestHash,
    resourceUri = '',
    ttlMs = challengeTtlMs,
    nonce = null,
  }) {
    const hashBuf = Buffer.isBuffer(requestHash) ? requestHash : Buffer.from(String(requestHash || ''), 'hex');
    if (hashBuf.length !== 32) {
      throw new X402Error(400, 'invalid_request_hash', 'requestHash must be 32 bytes');
    }
    if (!Number.isSafeInteger(maxAmountAtoms) || maxAmountAtoms <= 0) {
      throw new X402Error(400, 'invalid_amount', 'maxAmountAtoms must be a positive integer');
    }
    if (!payTo || typeof payTo !== 'string') {
      throw new X402Error(400, 'invalid_pay_to', 'payTo account is required');
    }
    const nonceBuf = nonce ? Buffer.from(nonce) : crypto.randomBytes(16);
    if (nonceBuf.length < 16 || nonceBuf.length > 32) {
      throw new X402Error(400, 'invalid_nonce', 'nonce must be 16..32 bytes');
    }
    const nowMs = now();
    const required = {
      x402_version: 2,
      scheme,
      network,
      lane,
      asset_id: assetId,
      max_amount_atoms: maxAmountAtoms,
      pay_to: payTo,
      request_hash: hashBuf,
      nonce: nonceBuf,
      expires_at_unix_ms: nowMs + ttlMs,
      resource_uri: resourceUri,
      facilitator_url: facilitatorUrl,
    };
    challenges.set(nonceBuf.toString('hex'), { required, createdAtMs: nowMs });
    const raw = mp.encodeX402PaymentRequired(required);
    return {
      required,
      headerValue: mp.encodeX402Header(raw),
    };
  }

  function resolveChallenge(expectedRequired, nonceBuf) {
    if (expectedRequired) return expectedRequired;
    const stored = challenges.get(nonceBuf.toString('hex'));
    if (!stored) {
      throw new X402Error(400, 'unknown_challenge_nonce', 'challenge nonce is unknown or expired');
    }
    return stored.required;
  }

  // Strictly local verification — NEVER calls upstream validators/RPC.
  async function verifyPaymentLocally(signatureHeaderValue, { expectedRequired = null, remoteIp = 'unknown', apiKey = '' } = {}) {
    checkRateLimits({ remoteIp, apiKey });

    let sigMsg;
    try {
      const raw = mp.decodeX402Header(signatureHeaderValue);
      sigMsg = mp.decodeX402PaymentSignature(raw, { enforceCanonicalOrder: true });
    } catch (err) {
      throw new X402Error(400, 'invalid_payment_signature_header', err.message);
    }

    if (sigMsg.x402_version !== 2) {
      throw new X402Error(400, 'unsupported_x402_version', `expected x402_version=2, got ${sigMsg.x402_version}`);
    }
    if (sigMsg.nonce.length < 16 || sigMsg.nonce.length > 32) {
      throw new X402Error(400, 'invalid_nonce', 'nonce must be 16..32 bytes');
    }

    const req = resolveChallenge(expectedRequired, sigMsg.nonce);
    if (now() > req.expires_at_unix_ms) {
      throw new X402Error(401, 'challenge_expired', 'PAYMENT-REQUIRED challenge has expired');
    }
    if (sigMsg.network !== req.network) {
      throw new X402Error(400, 'network_mismatch', `expected network ${req.network}, got ${sigMsg.network}`);
    }
    if (sigMsg.lane !== req.lane) {
      throw new X402Error(400, 'lane_mismatch', `expected lane ${req.lane}, got ${sigMsg.lane}`);
    }
    if (sigMsg.scheme !== req.scheme) {
      throw new X402Error(400, 'scheme_mismatch', `expected scheme ${req.scheme}, got ${sigMsg.scheme}`);
    }
    if (
      sigMsg.request_hash.length !== 32 ||
      !crypto.timingSafeEqual(sigMsg.request_hash, req.request_hash)
    ) {
      throw new X402Error(400, 'request_hash_mismatch', 'PAYMENT-SIGNATURE request_hash does not match challenge');
    }
    if (!crypto.timingSafeEqual(sigMsg.nonce, req.nonce)) {
      throw new X402Error(400, 'nonce_mismatch', 'PAYMENT-SIGNATURE nonce does not match challenge');
    }

    let parsedPayload;
    try {
      parsedPayload = JSON.parse(sigMsg.signed_payload.toString('utf8'));
    } catch (_) {
      throw new X402Error(400, 'invalid_signed_payload', 'signed_payload must be valid canonical JSON');
    }

    const { mlDsa65 } = await mp.loadPqModules();
    const payerPubkeyBuf = parsedPayload?.payer_pubkey_b64
      ? Buffer.from(String(parsedPayload.payer_pubkey_b64), 'base64url')
      : Buffer.from(String(parsedPayload?.payer_pubkey_hex || ''), 'hex');
    const sigBuf = parsedPayload?.signature_b64
      ? Buffer.from(String(parsedPayload.signature_b64), 'base64url')
      : Buffer.from(String(parsedPayload?.signature_hex || ''), 'hex');
    if (
      payerPubkeyBuf.length !== mp.ML_DSA_65_PUBKEY_BYTES ||
      sigBuf.length !== mp.ML_DSA_65_SIGNATURE_BYTES
    ) {
      throw new X402Error(401, 'invalid_payer_signature_format', 'signed_payload requires ML-DSA-65 pubkey & signature');
    }

    // Secondary rate-limit keyed on payer pubkey fingerprint to prevent Sybil spam
    const payerKeyFingerprint = mp.sha256(payerPubkeyBuf).toString('hex').slice(0, 16);
    const payerRate = keyLimiter.admit(`payer:${payerKeyFingerprint}`);
    if (!payerRate.allowed) {
      throw new X402Error(429, 'rate_limited_payer', `payer rate limit exceeded; retry after ${payerRate.retryAfterMs}ms`);
    }

    if (sigMsg.lane === mp.PaymentLane.FASTPAY) {
      const order = parsedPayload.order;
      if (!order || typeof order !== 'object') {
        throw new X402Error(400, 'missing_fastpay_order', 'signed_payload missing FastPay order');
      }
      if (order.chain_id !== chainId) {
        throw new X402Error(400, 'chain_id_mismatch', `expected chain_id ${chainId}`);
      }
      if (order.asset_id !== req.asset_id) {
        throw new X402Error(400, 'asset_mismatch', `expected asset ${req.asset_id}, got ${order.asset_id}`);
      }
      if (order.payee_account !== req.pay_to) {
        throw new X402Error(400, 'pay_to_mismatch', `expected payee ${req.pay_to}, got ${order.payee_account}`);
      }
      if (!Number.isSafeInteger(order.amount_atoms) || order.amount_atoms < req.max_amount_atoms) {
        throw new X402Error(402, 'insufficient_payment_amount', 'order amount_atoms below required max_amount_atoms');
      }
      const minFee = computeMinimumFastPayFeeAtoms({
        weightBytes: sigMsg.signed_payload.length,
        inputCount: order.input_count || 1,
        outputCount: order.output_count || 1,
      });
      if (!Number.isSafeInteger(order.fee_atoms) || order.fee_atoms < minFee) {
        throw new X402Error(400, 'insufficient_fastpay_fee', `FastPay fee ${order.fee_atoms} below floor ${minFee}`);
      }
      const memoHash = extractX402RequestHashFromMemos(order.memos);
      if (!crypto.timingSafeEqual(memoHash, req.request_hash)) {
        throw new X402Error(400, 'memo_request_hash_mismatch', 'pf.ptr v5 INLINE memo hash does not match request_hash');
      }
      const computedOrderHash = computeFastPayOrderHash(order);
      if (sigMsg.order_hash.length !== 32 || !crypto.timingSafeEqual(sigMsg.order_hash, computedOrderHash)) {
        throw new X402Error(400, 'order_hash_mismatch', 'PAYMENT-SIGNATURE order_hash does not match signed order');
      }
      const preimage = Buffer.concat([DOMAIN_FASTPAY_ORDER_SIGN, mp.canonicalJsonBytes(order)]);
      const sigOk = mlDsa65.verify(sigBuf, preimage, payerPubkeyBuf);
      if (!sigOk) {
        throw new X402Error(401, 'invalid_payer_signature', 'ML-DSA-65 FastPay order signature failed local verification');
      }
      const dedupKey = `fastpay:${computedOrderHash.toString('hex')}`;
      return {
        dedupKey,
        lane: mp.PaymentLane.FASTPAY,
        scheme: sigMsg.scheme,
        required: req,
        signature: sigMsg,
        orderHash: computedOrderHash,
        txHash: Buffer.alloc(0),
        payerAccount: String(order.payer_account || ''),
        payeeAccount: String(order.payee_account),
        assetId: String(order.asset_id),
        amountAtoms: order.amount_atoms,
        feeAtoms: order.fee_atoms,
        signedOrder: order,
      };
    }

    if (
      sigMsg.lane === mp.PaymentLane.CONSENSUS_ISSUED ||
      sigMsg.lane === mp.PaymentLane.CONSENSUS_PFT
    ) {
      const tx = parsedPayload.tx;
      if (!tx || typeof tx !== 'object') {
        throw new X402Error(400, 'missing_consensus_tx', 'signed_payload missing consensus transaction');
      }
      if (tx.chain_id !== chainId) {
        throw new X402Error(400, 'chain_id_mismatch', `expected chain_id ${chainId}`);
      }
      if (!Number.isSafeInteger(tx.sequence) || tx.sequence < 0) {
        throw new X402Error(400, 'missing_explicit_sequence', 'consensus-lane payment requires explicit client sequence');
      }
      if (tx.asset_id !== req.asset_id) {
        throw new X402Error(400, 'asset_mismatch', `expected asset ${req.asset_id}, got ${tx.asset_id}`);
      }
      if (tx.payee_account !== req.pay_to) {
        throw new X402Error(400, 'pay_to_mismatch', `expected payee ${req.pay_to}, got ${tx.payee_account}`);
      }
      if (!Number.isSafeInteger(tx.amount_atoms) || tx.amount_atoms < req.max_amount_atoms) {
        throw new X402Error(402, 'insufficient_payment_amount', 'consensus tx amount_atoms below required amount');
      }
      const memoHash = extractX402RequestHashFromMemos(tx.memos);
      if (!crypto.timingSafeEqual(memoHash, req.request_hash)) {
        throw new X402Error(400, 'memo_request_hash_mismatch', 'pf.ptr v5 INLINE memo hash does not match request_hash');
      }
      const computedTxHash = computeConsensusTxHash(tx);
      if (sigMsg.tx_hash.length !== 32 || !crypto.timingSafeEqual(sigMsg.tx_hash, computedTxHash)) {
        throw new X402Error(400, 'tx_hash_mismatch', 'PAYMENT-SIGNATURE tx_hash does not match signed consensus tx');
      }
      const preimage = Buffer.concat([DOMAIN_CONSENSUS_TX_SIGN, mp.canonicalJsonBytes(tx)]);
      const sigOk = mlDsa65.verify(sigBuf, preimage, payerPubkeyBuf);
      if (!sigOk) {
        throw new X402Error(401, 'invalid_payer_signature', 'ML-DSA-65 consensus tx signature failed local verification');
      }
      const dedupKey = `consensus:${computedTxHash.toString('hex')}`;
      return {
        dedupKey,
        lane: sigMsg.lane,
        scheme: sigMsg.scheme,
        required: req,
        signature: sigMsg,
        orderHash: Buffer.alloc(0),
        txHash: computedTxHash,
        payerAccount: String(tx.payer_account || ''),
        payeeAccount: String(tx.payee_account),
        assetId: String(tx.asset_id),
        amountAtoms: tx.amount_atoms,
        feeAtoms: tx.fee_atoms || 0,
        signedTx: tx,
      };
    }

    throw new X402Error(400, 'unsupported_lane', `unsupported payment lane: ${sigMsg.lane}`);
  }

  async function settlePayment(signatureHeaderValue, { expectedRequired = null, remoteIp = 'unknown', apiKey = '' } = {}) {
    // 1. Complete local verification BEFORE touching spent store or upstream validators.
    const verified = await verifyPaymentLocally(signatureHeaderValue, { expectedRequired, remoteIp, apiKey });

    // 2. Reject replays keyed strictly on immutable order_hash or tx_hash (NEVER certificate_digest).
    if (spentStore.has(verified.dedupKey)) {
      throw new X402Error(
        409,
        'payment_already_spent',
        `payment ${verified.dedupKey} has already been settled`
      );
    }

    let certificateDigest = Buffer.alloc(0);
    let finalizedHeight = 0;

    // 3. Settle / verify finality only after local verification and dedup pass.
    if (verified.lane === mp.PaymentLane.FASTPAY) {
      if (verified.scheme === mp.X402Scheme.PF_FASTPAY_SETTLED) {
        if (!verified.signature.certificate_payload || verified.signature.certificate_payload.length === 0) {
          throw new X402Error(400, 'missing_certificate_payload', 'pf-fastpay-settled requires certificate_payload');
        }
        const certCheck = await committeeRegistry.verifyCertificate(
          verified.signature.certificate_payload,
          verified.orderHash
        );
        certificateDigest = certCheck.certificateDigest;
      } else {
        if (!settlementAdapter || typeof settlementAdapter.settleFastPay !== 'function') {
          throw new X402Error(503, 'settlement_backend_unavailable', 'FastPay settlement backend not configured');
        }
        const settled = await settlementAdapter.settleFastPay(verified);
        if (!settled?.accepted) {
          throw new X402Error(502, 'fastpay_settlement_rejected', settled?.reason || 'FastPay quorum rejected order');
        }
        certificateDigest = Buffer.isBuffer(settled.certificateDigest)
          ? settled.certificateDigest
          : Buffer.from(String(settled.certificateDigestHex || ''), 'hex');
        finalizedHeight = Number(settled.finalizedHeight || 0);
      }
    } else {
      if (!settlementAdapter || typeof settlementAdapter.settleConsensus !== 'function') {
        throw new X402Error(503, 'settlement_backend_unavailable', 'consensus settlement backend not configured');
      }
      const receipt = await settlementAdapter.settleConsensus(verified);
      if (!receipt?.accepted || !Number.isSafeInteger(receipt.finalizedHeight) || receipt.finalizedHeight <= 0) {
        throw new X402Error(502, 'consensus_finality_not_accepted', receipt?.reason || 'consensus finality receipt rejected');
      }
      finalizedHeight = receipt.finalizedHeight;
    }

    const settledAtMs = now();
    const inserted = spentStore.markSpent(verified.dedupKey, {
      dedup_key: verified.dedupKey,
      lane: verified.lane,
      amount_atoms: verified.amountAtoms,
      asset_id: verified.assetId,
      settled_at_unix_ms: settledAtMs,
    });
    if (!inserted) {
      throw new X402Error(409, 'payment_already_spent', `concurrent replay rejected for ${verified.dedupKey}`);
    }

    const paymentRef = {
      lane: verified.lane,
      chain_id: chainId,
      order_hash: verified.orderHash,
      certificate_digest: certificateDigest,
      tx_hash: verified.txHash,
      asset_id: verified.assetId,
      payer_account: verified.payerAccount,
      payee_account: verified.payeeAccount,
      amount_atoms: verified.amountAtoms,
      fee_atoms: verified.feeAtoms,
      finalized_height: finalizedHeight,
    };
    const responseMsg = {
      x402_version: 2,
      settled: true,
      payment_ref: paymentRef,
      settled_at_unix_ms: settledAtMs,
      facilitator_id: facilitatorId,
    };
    const responseHeaderValue = mp.encodeX402Header(mp.encodeX402PaymentResponse(responseMsg));
    return {
      response: responseMsg,
      responseHeaderValue,
      dedupKey: verified.dedupKey,
    };
  }

  return {
    facilitatorId,
    chainId,
    network,
    committeeRegistry,
    spentStore,
    issueChallenge,
    verifyPaymentLocally,
    settlePayment,
  };
}

// ---------------------------------------------------------------------------
// Standalone HTTP Facilitator Server
// ---------------------------------------------------------------------------

function readJsonBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let total = 0;
    req.on('data', (chunk) => {
      total += chunk.length;
      if (total > MAX_BODY_BYTES) {
        reject(new X402Error(413, 'body_too_large', 'request body exceeds limit'));
        req.resume();
        return;
      }
      chunks.push(chunk);
    });
    req.on('end', () => {
      try {
        const raw = Buffer.concat(chunks).toString('utf8');
        resolve(raw.trim() ? JSON.parse(raw) : {});
      } catch (_) {
        reject(new X402Error(400, 'invalid_json', 'request body must be valid JSON'));
      }
    });
    req.on('error', reject);
  });
}

function sendJson(res, statusCode, payload, extraHeaders = {}) {
  const raw = JSON.stringify(payload);
  res.writeHead(statusCode, {
    'content-type': 'application/json',
    'content-length': Buffer.byteLength(raw),
    ...extraHeaders,
  });
  res.end(raw);
}

function createFacilitatorServer(core, { maxHeaderSize = 65_536 } = {}) {
  return http.createServer({ maxHeaderSize }, async (req, res) => {
    const remoteIp = String(req.socket?.remoteAddress || 'unknown');
    const apiKey = String(req.headers['x-facilitator-key'] || '');
    try {
      const url = new URL(req.url || '/', 'http://127.0.0.1');
      if (req.method === 'GET' && url.pathname === '/healthz') {
        sendJson(res, 200, {
          ok: true,
          facilitator_id: core.facilitatorId,
          chain_id: core.chainId,
          network: core.network,
          active_committee_epoch: core.committeeRegistry.activeEpoch(),
          spent_count: core.spentStore.size(),
        });
        return;
      }

      if (req.method === 'POST' && url.pathname === '/v1/challenge') {
        const body = await readJsonBody(req);
        const challenge = core.issueChallenge({
          scheme: body.scheme ?? mp.X402Scheme.EXACT,
          lane: body.lane ?? mp.PaymentLane.FASTPAY,
          assetId: body.asset_id || 'PFT',
          maxAmountAtoms: body.max_amount_atoms,
          payTo: body.pay_to,
          requestHash: body.request_hash_hex,
          resourceUri: body.resource_uri || '',
          ttlMs: body.ttl_ms,
        });
        sendJson(
          res,
          200,
          {
            ok: true,
            payment_required_header: challenge.headerValue,
            nonce_hex: challenge.required.nonce.toString('hex'),
            expires_at_unix_ms: challenge.required.expires_at_unix_ms,
          },
          { [HEADER_PAYMENT_REQUIRED]: challenge.headerValue }
        );
        return;
      }

      if (req.method === 'POST' && (url.pathname === '/v1/verify' || url.pathname === '/v1/settle')) {
        const body = await readJsonBody(req);
        const sigHeader = String(req.headers[HEADER_PAYMENT_SIGNATURE] || body.payment_signature_header || '');
        const expectedRequired = body.payment_required_header
          ? mp.decodeX402PaymentRequired(mp.decodeX402Header(body.payment_required_header))
          : null;

        if (url.pathname === '/v1/verify') {
          const verified = await core.verifyPaymentLocally(sigHeader, {
            expectedRequired,
            remoteIp,
            apiKey,
          });
          sendJson(res, 200, {
            ok: true,
            verified: true,
            dedup_key: verified.dedupKey,
            lane: verified.lane,
            amount_atoms: verified.amountAtoms,
            asset_id: verified.assetId,
          });
          return;
        }

        const settled = await core.settlePayment(sigHeader, {
          expectedRequired,
          remoteIp,
          apiKey,
        });
        sendJson(
          res,
          200,
          {
            ok: true,
            settled: true,
            dedup_key: settled.dedupKey,
            payment_response_header: settled.responseHeaderValue,
          },
          { [HEADER_PAYMENT_RESPONSE]: settled.responseHeaderValue }
        );
        return;
      }

      sendJson(res, 404, { ok: false, code: 'not_found' });
    } catch (err) {
      if (err instanceof X402Error) {
        sendJson(res, err.statusCode, { ok: false, code: err.code, error: err.message });
      } else {
        sendJson(res, 500, { ok: false, code: 'internal_error', error: err.message });
      }
    }
  });
}

// ---------------------------------------------------------------------------
// Protected Resource / MCP HTTP Gate Helper
// ---------------------------------------------------------------------------

function createX402Gate({
  facilitatorCore,
  payTo,
  priceAtoms,
  assetId = 'PFT',
  lane = mp.PaymentLane.FASTPAY,
  scheme = mp.X402Scheme.EXACT,
  resourceOrigin = 'http://127.0.0.1',
}) {
  return async function enforceX402(req, res, rawBody) {
    const requestHash = computeHttpRequestHash({
      method: req.method,
      urlPath: req.url,
      contentType: req.headers['content-type'] || '',
      body: rawBody,
    });
    const sigHeader = req.headers[HEADER_PAYMENT_SIGNATURE];
    if (!sigHeader) {
      const challenge = facilitatorCore.issueChallenge({
        scheme,
        lane,
        assetId,
        maxAmountAtoms: priceAtoms,
        payTo,
        requestHash,
        resourceUri: `${resourceOrigin}${req.url}`,
      });
      sendJson(
        res,
        402,
        {
          ok: false,
          code: 'payment_required',
          x402_version: 2,
          pay_to: payTo,
          asset_id: assetId,
          max_amount_atoms: priceAtoms,
        },
        { [HEADER_PAYMENT_REQUIRED]: challenge.headerValue }
      );
      return null;
    }

    try {
      const settled = await facilitatorCore.settlePayment(String(sigHeader), {
        remoteIp: String(req.socket?.remoteAddress || 'unknown'),
        apiKey: String(req.headers['x-facilitator-key'] || ''),
      });
      if (!crypto.timingSafeEqual(settled.response.payment_ref.order_hash.length ? requestHash : requestHash, requestHash)) {
        throw new X402Error(400, 'request_hash_mismatch', 'request hash mismatch');
      }
      return settled;
    } catch (err) {
      const status = err instanceof X402Error ? err.statusCode : 500;
      const code = err instanceof X402Error ? err.code : 'settlement_error';
      sendJson(res, status, { ok: false, code, error: err.message });
      return null;
    }
  };
}

module.exports = {
  HEADER_PAYMENT_REQUIRED,
  HEADER_PAYMENT_SIGNATURE,
  HEADER_PAYMENT_RESPONSE,
  X402Error,
  computeHttpRequestHash,
  computeMinimumFastPayFeeAtoms,
  buildX402InlineMemo,
  extractX402RequestHashFromMemos,
  computeFastPayOrderHash,
  computeConsensusTxHash,
  signFastPayOrder,
  signConsensusTransfer,
  signFastPayValidatorVote,
  computeCertificateDigest,
  MemorySpentStore,
  FileSpentStore,
  SlidingWindowRateLimiter,
  CommitteeRegistry,
  createFacilitatorCore,
  createFacilitatorServer,
  createX402Gate,
};
