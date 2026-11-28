'use strict';

// Live JSON-RPC settlement adapter for Post Fiat L1v2 x402 v2 facilitator.
//
// Connects `tools/x402-facilitator` to real postfiatl1v2 validator JSON-RPC endpoints:
//  - FastPay lane: fans out `owned_sign_v3` across configured validators, cryptographically
//    authenticates >= 2f+1 distinct active-committee ML-DSA-65 votes against the pinned
//    CommitteeRegistry + domain + epoch, assembles the native `OwnedTransferCertificateV3`,
//    calls `owned_apply_v3` with `(validator_id, cert_json)`, and verifies signed
//    `FastPayApplyAckV1` acknowledgements (and optional indexed `tx` lookup by `order_hash`).
//  - Consensus lane: submits native signed `payment_v2` (`mempool_submit_signed_payment_v2`),
//    verifies exact 48-byte `tx_id` identity agreement between client, facilitator, node,
//    and archive, polls `tx` until finality, and checks recipient, amount, and memo hash.

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { performance } = require('node:perf_hooks');
const mp = require('../../wallet-proxy/message-protocol');
const nativeChain = require('./native-chain');
const { FastPayRpcV2Client } = require('./fastpay-rpc-v2');
const { extractX402RequestHashFromMemos } = require('./facilitator');

const DEFAULT_RPC_TIMEOUT_MS = 10_000;
const DEFAULT_POLL_INTERVAL_MS = 500;
const DEFAULT_FINALITY_TIMEOUT_MS = 30_000;
const DEFAULT_APPLY_RETRY_SCHEDULE_MS = [250, 500, 1_000, 2_000, 5_000];
const FINALITY_MODE_CERTIFICATE = 'certificate';
const FINALITY_MODE_APPLIED_QUORUM = 'applied_quorum';
const CERT_FILE_MODE = 0o600;
const LINE_FEED = 0x0a;

function syncDirectory(dirPath) {
  let fd;
  try {
    fd = fs.openSync(dirPath, 'r');
    fs.fsyncSync(fd);
  } catch (_) {
    // Non-fatal on filesystems that do not support directory fsync.
  } finally {
    if (fd !== undefined) {
      try {
        fs.closeSync(fd);
      } catch (_) {}
    }
  }
}

class MemoryCertificateStore {
  constructor() {
    this._records = new Map(); // orderHashHex -> record
  }

  putPending(record) {
    const key = String(record.order_hash_hex).toLowerCase();
    const existing = this._records.get(key);
    if (existing) return existing;
    const stored = {
      ...record,
      order_hash_hex: key,
      confirmed_validators: Array.isArray(record.confirmed_validators) ? [...record.confirmed_validators] : [],
      status: record.status || 'pending',
    };
    this._records.set(key, stored);
    return stored;
  }

  updateAck(orderHashHex, validatorId, { finalizedHeight = 0, requiredAcks = 1 } = {}) {
    const key = String(orderHashHex).toLowerCase();
    const rec = this._records.get(key);
    if (!rec) return null;
    if (!rec.confirmed_validators.includes(validatorId)) {
      rec.confirmed_validators.push(validatorId);
    }
    if (Number.isSafeInteger(finalizedHeight) && finalizedHeight > (rec.finalized_height || 0)) {
      rec.finalized_height = finalizedHeight;
    }
    if (rec.confirmed_validators.length >= requiredAcks) {
      rec.status = 'applied';
    }
    return rec;
  }

  get(orderHashHex) {
    return this._records.get(String(orderHashHex).toLowerCase()) || null;
  }

  listPending() {
    return [...this._records.values()].filter((r) => r.status !== 'applied');
  }

  close() {}
}

// Durable append-only WAL for verified FastPay v3 certificates (W1).
// Every put/ack update is written in one writeSync + fsyncSync before returning,
// with leading/trailing newlines so torn tail writes are ignored cleanly on reload.
class FileCertificateStore {
  constructor(filePath) {
    this._filePath = path.resolve(filePath);
    this._records = new Map();
    const dir = path.dirname(this._filePath);
    fs.mkdirSync(dir, { recursive: true });
    this._fd = fs.openSync(this._filePath, 'a+', CERT_FILE_MODE);
    syncDirectory(dir);
    this._reload();
  }

  _append(event) {
    const line = Buffer.from(`\n${JSON.stringify(event)}\n`, 'utf8');
    const written = fs.writeSync(this._fd, line);
    if (written !== line.length) {
      throw new Error(`certificate WAL ${this._filePath} partial write`);
    }
    fs.fsyncSync(this._fd);
  }

  _reload() {
    const { size } = fs.fstatSync(this._fd);
    if (size === 0) return;
    const raw = Buffer.alloc(size);
    let offset = 0;
    while (offset < size) {
      const n = fs.readSync(this._fd, raw, offset, size - offset, offset);
      if (n === 0) break;
      offset += n;
    }
    const completeEnd = raw.subarray(0, offset).lastIndexOf(LINE_FEED) + 1;
    for (const line of raw.subarray(0, completeEnd).toString('utf8').split('\n')) {
      if (!line.startsWith('{') || !line.endsWith('}')) continue;
      let ev;
      try {
        ev = JSON.parse(line);
      } catch (_) {
        continue;
      }
      if (ev.op === 'put' && typeof ev.record?.order_hash_hex === 'string') {
        const key = ev.record.order_hash_hex.toLowerCase();
        if (!this._records.has(key)) {
          this._records.set(key, {
            ...ev.record,
            order_hash_hex: key,
            confirmed_validators: Array.isArray(ev.record.confirmed_validators) ? [...ev.record.confirmed_validators] : [],
            status: ev.record.status || 'pending',
          });
        }
      } else if (ev.op === 'ack' && typeof ev.order_hash_hex === 'string' && typeof ev.validator_id === 'string') {
        const rec = this._records.get(ev.order_hash_hex.toLowerCase());
        if (rec) {
          if (!rec.confirmed_validators.includes(ev.validator_id)) {
            rec.confirmed_validators.push(ev.validator_id);
          }
          if (Number.isSafeInteger(ev.finalized_height) && ev.finalized_height > (rec.finalized_height || 0)) {
            rec.finalized_height = ev.finalized_height;
          }
          if (ev.applied === true) rec.status = 'applied';
        }
      }
    }
  }

  putPending(record) {
    const key = String(record.order_hash_hex).toLowerCase();
    const existing = this._records.get(key);
    if (existing) return existing;
    const stored = {
      ...record,
      order_hash_hex: key,
      confirmed_validators: Array.isArray(record.confirmed_validators) ? [...record.confirmed_validators] : [],
      status: record.status || 'pending',
    };
    this._append({ op: 'put', record: stored });
    this._records.set(key, stored);
    return stored;
  }

  updateAck(orderHashHex, validatorId, { finalizedHeight = 0, requiredAcks = 1 } = {}) {
    const key = String(orderHashHex).toLowerCase();
    const rec = this._records.get(key);
    if (!rec) return null;
    if (!rec.confirmed_validators.includes(validatorId)) {
      rec.confirmed_validators.push(validatorId);
    }
    if (Number.isSafeInteger(finalizedHeight) && finalizedHeight > (rec.finalized_height || 0)) {
      rec.finalized_height = finalizedHeight;
    }
    const applied = rec.confirmed_validators.length >= requiredAcks;
    if (applied) rec.status = 'applied';
    this._append({
      op: 'ack',
      order_hash_hex: key,
      validator_id: validatorId,
      finalized_height: rec.finalized_height || 0,
      applied,
    });
    return rec;
  }

  get(orderHashHex) {
    return this._records.get(String(orderHashHex).toLowerCase()) || null;
  }

  listPending() {
    return [...this._records.values()].filter((r) => r.status !== 'applied');
  }

  close() {
    if (this._fd !== null && this._fd !== undefined) {
      try {
        fs.closeSync(this._fd);
      } catch (_) {}
      this._fd = null;
    }
  }
}

function parseValidatorEndpoints(rpcUrls, validatorIds = []) {
  const list = Array.isArray(rpcUrls)
    ? rpcUrls
    : String(rpcUrls || '')
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);
  if (list.length === 0) {
    throw new Error('at least one postfiatl1v2 JSON-RPC URL is required');
  }
  return list.map((entry, idx) => {
    if (typeof entry === 'object' && entry !== null) {
      return {
        url: String(entry.url || '').trim(),
        validatorId: String(entry.validatorId || entry.validator_id || `validator-${idx}`),
      };
    }
    const eq = entry.indexOf('=');
    if (eq > 0 && !entry.slice(0, eq).includes('://')) {
      return {
        validatorId: entry.slice(0, eq).trim(),
        url: entry.slice(eq + 1).trim(),
      };
    }
    return {
      validatorId: String(validatorIds[idx] || `validator-${idx}`),
      url: entry,
    };
  });
}

async function defaultJsonRpcCall(url, method, params, { timeoutMs = DEFAULT_RPC_TIMEOUT_MS, fetchFn = globalThis.fetch } = {}) {
  if (typeof fetchFn !== 'function') {
    throw new Error('global fetch is unavailable; inject fetchFn');
  }
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const id = crypto.randomUUID();
  const version = 'postfiat-local-rpc-v1';
  try {
    const res = await fetchFn(url, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        version,
        id,
        method,
        params,
      }),
      signal: controller.signal,
    });
    if (!res.ok) {
      throw new Error(`RPC HTTP ${res.status} from ${url} (${method})`);
    }
    const payload = await res.json();
    if (payload?.version !== version || payload.id !== id) throw new Error('RPC response identity mismatch');
    if (payload.ok !== true || payload.error) {
      const msg = payload.error?.message || 'request refused';
      throw new Error(`RPC error on ${method}: ${msg}`);
    }
    if (!payload || !Object.prototype.hasOwnProperty.call(payload, 'result')) {
      throw new Error(`RPC ${method} response missing result`);
    }
    return payload.result;
  } finally {
    clearTimeout(timer);
  }
}

function defaultQuorumThreshold(count) {
  return Math.floor((2 * count) / 3) + 1;
}

function verifyOnChainTxMatchesPayment(txResult, verified, txId) {
  if (txResult?.confirmed !== true || txResult.receipt?.accepted !== true) {
    return { ok: false, reason: 'onchain_tx_not_accepted' };
  }
  const expectedChainId = verified.signedTx?.chain_id || verified.nativeSignedPaymentV2?.unsigned?.chain_id;
  if (
    !/^[0-9a-f]{96}$/.test(txId) ||
    txResult.tx_id !== txId ||
    txResult.receipt.tx_id !== txId ||
    txResult.chain_id !== expectedChainId
  ) {
    return { ok: false, reason: 'onchain_tx_identity_mismatch' };
  }
  const blockHeight = txResult.block?.header?.height;
  if (!Number.isSafeInteger(blockHeight) || blockHeight <= 0) {
    return { ok: false, reason: 'onchain_tx_not_finalized' };
  }
  const rows = txResult.transaction_rows;
  const row = Array.isArray(rows) && rows.length === 1 ? rows[0] : null;
  if (
    verified.lane !== mp.PaymentLane.CONSENSUS_PFT ||
    verified.assetId !== 'PFT' ||
    row?.transaction_kind !== 'payment_v2' ||
    row.tx_id !== txId ||
    row.block_height !== blockHeight ||
    row.from !== verified.payerAccount ||
    row.to !== verified.payeeAccount ||
    !Number.isSafeInteger(row.amount) ||
    row.amount !== verified.amountAtoms
  ) {
    return { ok: false, reason: 'onchain_payment_row_mismatch' };
  }
  const memos = row.memos;
  let memoHash;
  try {
    memoHash = extractX402RequestHashFromMemos(memos);
  } catch (err) {
    return { ok: false, reason: `onchain_memo_invalid:${err.message}` };
  }
  if (!Buffer.isBuffer(verified.requestHash) || !crypto.timingSafeEqual(memoHash, verified.requestHash)) {
    return { ok: false, reason: 'onchain_memo_request_hash_mismatch' };
  }
  return { ok: true, finalizedHeight: blockHeight };
}

function createChainSettlementAdapter({
  rpcUrls,
  fastpayRpcV2Endpoints = null, // optional { 'validator-0': { host, port } } or CSV "validator-0=host:port,..."
  fastpayRpcV2TlsOptions = {},
  validatorIds = [],
  quorumThreshold = null,
  committeeRegistry = null,
  expectedDomain = null,
  requireAuthenticatedCommittee = false,
  verifyIndexedOrder = false,
  settlementFinalityMode = null, // 'certificate' | 'applied_quorum'
  certificateStore = null,
  coLocatedValidatorId = null,
  coLocatedValidatorSyncApply = true,
  applyRetryScheduleMs = DEFAULT_APPLY_RETRY_SCHEDULE_MS,
  maxApplyRetryRounds = 8,
  rpcTimeoutMs = DEFAULT_RPC_TIMEOUT_MS,
  pollIntervalMs = DEFAULT_POLL_INTERVAL_MS,
  finalityTimeoutMs = DEFAULT_FINALITY_TIMEOUT_MS,
  fetchFn = globalThis.fetch,
  sleepFn = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  onAlert = null,
} = {}) {
  const endpoints = parseValidatorEndpoints(rpcUrls, validatorIds);
  const authenticatedCommittee = Boolean(committeeRegistry && typeof committeeRegistry.getActiveCommittee === 'function' && expectedDomain);
  if (requireAuthenticatedCommittee && !authenticatedCommittee) {
    throw new Error('authenticated FastPay adapter requires both committeeRegistry and expectedDomain');
  }
  const activeCommittee = authenticatedCommittee ? committeeRegistry.getActiveCommittee() : null;
  if (requireAuthenticatedCommittee && !activeCommittee) {
    throw new Error('committeeRegistry has no active committee epoch configured');
  }
  const quorum = quorumThreshold ?? (activeCommittee ? activeCommittee.threshold : defaultQuorumThreshold(endpoints.length));
  if (!Number.isSafeInteger(quorum) || quorum <= 0 || quorum > endpoints.length) {
    throw new Error(`quorumThreshold ${quorum} must be within 1..${endpoints.length}`);
  }

  const resolvedFinalityMode =
    settlementFinalityMode ||
    (verifyIndexedOrder ? FINALITY_MODE_APPLIED_QUORUM : certificateStore ? FINALITY_MODE_CERTIFICATE : FINALITY_MODE_APPLIED_QUORUM);
  if (resolvedFinalityMode !== FINALITY_MODE_CERTIFICATE && resolvedFinalityMode !== FINALITY_MODE_APPLIED_QUORUM) {
    throw new Error(
      `settlementFinalityMode must be "${FINALITY_MODE_CERTIFICATE}" or "${FINALITY_MODE_APPLIED_QUORUM}", got "${resolvedFinalityMode}"`
    );
  }
  const certStore = certificateStore || new MemoryCertificateStore();
  const endpointById = new Map(endpoints.map((ep) => [ep.validatorId, ep]));
  const v2Clients = new Map();
  if (fastpayRpcV2Endpoints) {
    const entries =
      typeof fastpayRpcV2Endpoints === 'string'
        ? fastpayRpcV2Endpoints
            .split(',')
            .map((s) => s.trim())
            .filter(Boolean)
            .map((item) => {
              const eq = item.indexOf('=');
              const vid = eq > 0 ? item.slice(0, eq).trim() : '';
              const addr = eq > 0 ? item.slice(eq + 1).trim() : item;
              const colon = addr.lastIndexOf(':');
              return [vid, { host: addr.slice(0, colon), port: Number(addr.slice(colon + 1)) }];
            })
        : Object.entries(fastpayRpcV2Endpoints);
    for (const [vid, cfg] of entries) {
      if (vid && cfg && cfg.host && cfg.port) {
        v2Clients.set(
          vid,
          new FastPayRpcV2Client({
            host: cfg.host,
            port: cfg.port,
            tlsOptions: fastpayRpcV2TlsOptions,
            timeoutMs: rpcTimeoutMs,
          })
        );
      }
    }
  }
  const inFlightBackgroundApplies = new Set();
  const metricsState = {
    voteQuorumSamplesMs: [],
    applyQuorumSamplesMs: [],
    applyRetryTotal: 0,
  };

  const rpc = (url, method, params) =>
    defaultJsonRpcCall(url, method, params, { timeoutMs: rpcTimeoutMs, fetchFn });

  async function applyCertificateToEndpoint(ep, { certJson, certObj = null, committee, orderHashHex, lockId, expectedCertDigestHex }) {
    const v2Client = v2Clients.get(ep.validatorId);
    const ack = v2Client
      ? (await v2Client.ownedApplyV3FastOrFallback(ep.validatorId, certObj || JSON.parse(certJson))).ack
      : await rpc(ep.url, 'owned_apply_v3', {
          validator_id: ep.validatorId,
          cert_json: certJson,
        });
    const pk = committee.validators.get(ep.validatorId);
    if (!pk || !ack) return { ok: false, reason: 'missing_ack_or_key' };
    if (
      ack.signed === false &&
      ack.schema === 'postfiat-fastpay-held-effect-v1' &&
      ack.validator_id === ep.validatorId &&
      String(ack.lock_id || '').toLowerCase() === lockId &&
      String(ack.certificate_digest || '').toLowerCase() === expectedCertDigestHex
    ) {
      return { ok: false, held: true, validatorId: ep.validatorId };
    }
    if (
      ack.validator_id !== ep.validatorId ||
      String(ack.order_digest || '').toLowerCase() !== orderHashHex ||
      String(ack.lock_id || '').toLowerCase() !== lockId ||
      String(ack.certificate_digest || '').toLowerCase() !== expectedCertDigestHex ||
      Number(ack.committee_epoch) !== Number(committee.epoch) ||
      !nativeChain.domainsMatch(ack.domain, expectedDomain)
    ) {
      return { ok: false, reason: 'apply_ack_binding_mismatch' };
    }
    const ackSigOk = await nativeChain.verifyNativeFastPayApplyAckV1(ack, pk);
    if (!ackSigOk) return { ok: false, reason: 'apply_ack_signature_invalid' };
    const finalizedHeight = Number(ack.block_height || ack.finalized_height || ack.height || 0);
    return {
      ok: true,
      validatorId: ep.validatorId,
      finalizedHeight: Number.isSafeInteger(finalizedHeight) && finalizedHeight > 0 ? finalizedHeight : 0,
    };
  }

  function scheduleBackgroundApply(record, { committee, requiredAcks }) {
    const orderHashHex = record.order_hash_hex;
    const task = (async () => {
      const tStart = performance.now();
      const confirmed = new Set(record.confirmed_validators || []);
      const heldOnly = new Set();
      let attempt = 0;
      let postQuorumRetries = 0;
      while (confirmed.size + heldOnly.size < endpoints.length && attempt <= maxApplyRetryRounds) {
        if (confirmed.size >= requiredAcks) {
          if (postQuorumRetries >= 1) break;
          postQuorumRetries += 1;
        }
        const remaining = endpoints.filter(
          (ep) => !confirmed.has(ep.validatorId) && !heldOnly.has(ep.validatorId)
        );
        if (remaining.length === 0) break;
        if (attempt > 0) {
          metricsState.applyRetryTotal += 1;
          const delayMs = applyRetryScheduleMs[Math.min(attempt - 1, applyRetryScheduleMs.length - 1)] || 250;
          await sleepFn(delayMs);
        }
        await Promise.allSettled(
          remaining.map(async (ep) => {
            try {
              const res = await applyCertificateToEndpoint(ep, {
                certJson: record.cert_json,
                committee,
                orderHashHex,
                lockId: record.lock_id,
                expectedCertDigestHex: record.certificate_digest_hex,
              });
              if (res.ok) {
                confirmed.add(ep.validatorId);
                certStore.updateAck(orderHashHex, ep.validatorId, {
                  finalizedHeight: res.finalizedHeight,
                  requiredAcks,
                });
              } else if (res.held) {
                heldOnly.add(ep.validatorId);
              }
            } catch (_) {
              // Retried in the next backoff round.
            }
          })
        );
        attempt += 1;
      }
      if (confirmed.size >= requiredAcks) {
        metricsState.applyQuorumSamplesMs.push(Number((performance.now() - tStart).toFixed(2)));
      } else if (typeof onAlert === 'function') {
        onAlert({
          event: 'fastpay_unconfirmed_cert_expiring',
          order_hash_hex: orderHashHex,
          lock_id: record.lock_id,
          confirmed_count: confirmed.size,
          required_acks: requiredAcks,
        });
      }
    })();
    inFlightBackgroundApplies.add(task);
    task.finally(() => inFlightBackgroundApplies.delete(task));
    return task;
  }

  async function settleFastPayAuthenticated(verified) {
    const committee = committeeRegistry.getActiveCommittee();
    if (!committee) {
      return { accepted: false, reason: 'missing_active_committee' };
    }
    const signedOrder = verified?.nativeSignedOrderV3 || verified?.signedOrder;
    if (!nativeChain.isNativeSignedOwnedTransferOrderV3(signedOrder)) {
      return { accepted: false, reason: 'native_owned_transfer_order_v3_required' };
    }
    const { order } = signedOrder;
    if (!nativeChain.domainsMatch(order.domain, expectedDomain)) {
      return { accepted: false, reason: 'fastpay_order_domain_mismatch' };
    }
    if (Number(order.recovery?.committee_epoch) !== Number(committee.epoch)) {
      return { accepted: false, reason: 'fastpay_order_committee_epoch_mismatch' };
    }
    const ownerCheck = await nativeChain.verifyNativeOwnedTransferOrderV3(signedOrder);
    if (!ownerCheck.ok) {
      return { accepted: false, reason: `invalid_fastpay_order:${ownerCheck.reason}` };
    }
    if (Buffer.isBuffer(verified.orderHash) && !crypto.timingSafeEqual(verified.orderHash, ownerCheck.orderHash)) {
      return { accepted: false, reason: 'fastpay_order_hash_mismatch' };
    }

    const orderJson = JSON.stringify(signedOrder);
    const { mlDsa65 } = await mp.loadPqModules();
    const requiredVotes = Math.max(quorum, committee.threshold);

    // 1. Quorum-first `owned_sign_v3` fan-out: authenticate each vote immediately on arrival
    //    and resolve as soon as `requiredVotes` distinct active-committee votes verify.
    const tVoteStart = performance.now();
    const verifiedVotes = [];
    const seenValidators = new Set();
    const errors = [];

    await new Promise((resolve) => {
      let settledCount = 0;
      let done = false;
      const finish = () => {
        if (!done) {
          done = true;
          resolve();
        }
      };
      if (endpoints.length < requiredVotes) {
        finish();
        return;
      }
      for (const ep of endpoints) {
        const v2Client = v2Clients.get(ep.validatorId);
        const signPromise = v2Client
          ? v2Client.ownedSignV3(ep.validatorId, signedOrder).then((r) => r.vote)
          : rpc(ep.url, 'owned_sign_v3', {
              order_json: orderJson,
              validator_id: ep.validatorId,
            });
        signPromise
          .then((voteResult) => {
            if (done) return;
            const rawVote = voteResult?.vote || voteResult;
            if (!rawVote || rawVote.error || rawVote.rejected === true) {
              errors.push(`${ep.validatorId}:${rawVote?.error || 'rejected'}`);
              return;
            }
            const voteValidatorId = String(rawVote.validator_id || '');
            if (!voteValidatorId || voteValidatorId !== ep.validatorId) {
              errors.push(`${ep.validatorId}:validator_id_mismatch`);
              return;
            }
            if (seenValidators.has(voteValidatorId)) {
              errors.push(`${voteValidatorId}:duplicate_vote`);
              return;
            }
            const pk = committee.validators.get(voteValidatorId);
            if (!pk) {
              errors.push(`${voteValidatorId}:not_in_active_committee`);
              return;
            }
            const sigHex = String(rawVote.signature_hex || '').toLowerCase();
            const sigBuf = Buffer.from(sigHex, 'hex');
            if (sigBuf.length !== mp.ML_DSA_65_SIGNATURE_BYTES) {
              errors.push(`${voteValidatorId}:invalid_vote_signature_length`);
              return;
            }
            if (!mlDsa65.verify(sigBuf, ownerCheck.signingBytes, pk, { context: nativeChain.OWNED_TRANSFER_CONTEXT_V3 })) {
              errors.push(`${voteValidatorId}:invalid_vote_signature`);
              return;
            }
            seenValidators.add(voteValidatorId);
            verifiedVotes.push({ validator_id: voteValidatorId, signature_hex: sigHex });
            if (verifiedVotes.length >= requiredVotes) {
              finish();
            }
          })
          .catch((err) => {
            if (!done) errors.push(String(err?.message || err || 'rpc_failed'));
          })
          .finally(() => {
            settledCount += 1;
            const maxPossible = verifiedVotes.length + (endpoints.length - settledCount);
            if (settledCount >= endpoints.length || maxPossible < requiredVotes) {
              finish();
            }
          });
      }
    });

    if (verifiedVotes.length < requiredVotes) {
      return {
        accepted: false,
        reason: `fastpay_authenticated_quorum_not_reached:${verifiedVotes.length}/${requiredVotes}${errors.length ? ` (${errors.join(' | ')})` : ''}`,
      };
    }
    metricsState.voteQuorumSamplesMs.push(Number((performance.now() - tVoteStart).toFixed(2)));

    // 2. Assemble canonical native `OwnedTransferCertificateV3`.
    //    The payer ML-DSA-65 signature, domain, epoch, and every one of the `requiredVotes`
    //    distinct active-committee ML-DSA-65 vote signatures were already verified above;
    //    compute the canonical SHA3-384 certificate digest directly without re-verifying 6× ML-DSA-65.
    const cert = {
      order: signedOrder.order,
      owner_pubkey_hex: signedOrder.owner_pubkey_hex,
      owner_signature_hex: signedOrder.owner_signature_hex,
      votes: verifiedVotes.slice(0, requiredVotes),
    };
    const expectedCertDigestHex = nativeChain.fastpayTransferCertificateDigestV3(cert).toString('hex');
    const certJson = JSON.stringify(cert);

    // 3a. 1-RTT certificate finality mode (W1): durably persist verified certificate to local
    //     WAL (plus optional synchronous apply to co-located validator) before returning,
    //     and fan out remote `owned_apply_v3` asynchronously with bounded retry.
    if (resolvedFinalityMode === FINALITY_MODE_CERTIFICATE) {
      const stored = certStore.putPending({
        order_hash_hex: ownerCheck.orderHashHex,
        certificate_digest_hex: expectedCertDigestHex,
        lock_id: ownerCheck.lockId,
        committee_epoch: Number(committee.epoch),
        expires_at_height: Number(order.recovery?.expires_at_height || 0),
        recovery_closes_at_height: Number(order.recovery?.recovery_closes_at_height || 0),
        cert_json: certJson,
      });

      let finalizedHeight = stored.finalized_height || 0;
      let appliedValidators = stored.confirmed_validators.length;

      if (coLocatedValidatorId && coLocatedValidatorSyncApply) {
        const localEp = endpointById.get(String(coLocatedValidatorId));
        if (!localEp) {
          return { accepted: false, reason: `unknown_colocated_validator:${coLocatedValidatorId}` };
        }
        try {
          const localRes = await applyCertificateToEndpoint(localEp, {
            certJson,
            committee,
            orderHashHex: ownerCheck.orderHashHex,
            lockId: ownerCheck.lockId,
            expectedCertDigestHex,
          });
          if (!localRes.ok) {
            return { accepted: false, reason: `colocated_apply_failed:${localRes.reason}` };
          }
          const updated = certStore.updateAck(ownerCheck.orderHashHex, localEp.validatorId, {
            finalizedHeight: localRes.finalizedHeight,
            requiredAcks: requiredVotes,
          });
          finalizedHeight = Math.max(finalizedHeight, localRes.finalizedHeight);
          appliedValidators = updated ? updated.confirmed_validators.length : 1;
        } catch (err) {
          return { accepted: false, reason: `colocated_apply_error:${err?.message || err}` };
        }
      }

      if (appliedValidators < requiredVotes) {
        scheduleBackgroundApply(certStore.get(ownerCheck.orderHashHex) || stored, {
          committee,
          requiredAcks: requiredVotes,
        });
      }

      return {
        accepted: true,
        finalityMode: FINALITY_MODE_CERTIFICATE,
        orderHashHex: ownerCheck.orderHashHex,
        certificateDigestHex: expectedCertDigestHex,
        finalizedHeight,
        quorumVotes: verifiedVotes.length,
        appliedValidators,
      };
    }

    // 3b. Strict 2-RTT `applied_quorum` mode: quorum-first `owned_apply_v3` fan-out
    //     verifying signed `FastPayApplyAckV1` acknowledgements.
    const tApplyStart = performance.now();
    const authenticatedAcks = new Set();
    let finalizedHeight = 0;

    await new Promise((resolve) => {
      let settledCount = 0;
      let done = false;
      const finish = () => {
        if (!done) {
          done = true;
          resolve();
        }
      };
      for (const ep of endpoints) {
        applyCertificateToEndpoint(ep, {
          certJson,
          committee,
          orderHashHex: ownerCheck.orderHashHex,
          lockId: ownerCheck.lockId,
          expectedCertDigestHex,
        })
          .then((res) => {
            if (!res.ok) return;
            authenticatedAcks.add(ep.validatorId);
            if (res.finalizedHeight > finalizedHeight) finalizedHeight = res.finalizedHeight;
            if (authenticatedAcks.size >= requiredVotes) finish();
          })
          .catch(() => {})
          .finally(() => {
            settledCount += 1;
            if (settledCount >= endpoints.length) finish();
          });
      }
    });

    if (authenticatedAcks.size === 0) {
      return { accepted: false, reason: 'fastpay_apply_no_authenticated_ack' };
    }
    metricsState.applyQuorumSamplesMs.push(Number((performance.now() - tApplyStart).toFixed(2)));

    // 4. Optional indexed order verification via `tx` lookup by immutable 48-byte `order_hash`.
    if (verifyIndexedOrder) {
      const txRes = await rpc(endpoints[0].url, 'tx', { tx_id: ownerCheck.orderHashHex });
      if (
        txRes?.confirmed !== true ||
        String(txRes.tx_id || '').toLowerCase() !== ownerCheck.orderHashHex
      ) {
        return { accepted: false, reason: 'fastpay_indexed_order_mismatch' };
      }
      const indexedHeight = Number(txRes.block?.header?.height || txRes.block_height || 0);
      if (Number.isSafeInteger(indexedHeight) && indexedHeight > finalizedHeight) {
        finalizedHeight = indexedHeight;
      }
    }

    return {
      accepted: true,
      finalityMode: FINALITY_MODE_APPLIED_QUORUM,
      orderHashHex: ownerCheck.orderHashHex,
      certificateDigestHex: expectedCertDigestHex,
      finalizedHeight,
      quorumVotes: verifiedVotes.length,
      appliedValidators: authenticatedAcks.size,
    };
  }

  async function settleFastPay(verified) {
    if (authenticatedCommittee || requireAuthenticatedCommittee) {
      return settleFastPayAuthenticated(verified);
    }

    // Legacy prototype path (explicitly refused in production by server.js).
    const rawOrder =
      verified?.signedOrder?.owned_transfer_order_v3 ||
      verified?.signedOrder?.chain_order ||
      verified?.signedOrder;
    if (!rawOrder || typeof rawOrder !== 'object') {
      return { accepted: false, reason: 'missing_fastpay_order_payload' };
    }
    const orderJson = typeof rawOrder === 'string' ? rawOrder : JSON.stringify(rawOrder);

    const votes = [];
    const errors = [];
    const results = await Promise.allSettled(
      endpoints.map(async (ep) => {
        const vote = await rpc(ep.url, 'owned_sign_v3', {
          order_json: orderJson,
          validator_id: ep.validatorId,
        });
        return { endpoint: ep, vote };
      })
    );

    for (const outcome of results) {
      if (outcome.status === 'fulfilled' && outcome.value?.vote) {
        const v = outcome.value.vote;
        if (v.error || v.rejected === true) {
          errors.push(`${outcome.value.endpoint.validatorId}:${v.error || 'rejected'}`);
        } else {
          votes.push(v.vote || v);
        }
      } else if (outcome.status === 'rejected') {
        errors.push(String(outcome.reason?.message || outcome.reason));
      }
    }

    if (votes.length < quorum) {
      return {
        accepted: false,
        reason: `fastpay_quorum_not_reached:${votes.length}/${quorum}${errors.length ? ` (${errors[0]})` : ''}`,
      };
    }

    const cert = {
      order: typeof rawOrder === 'string' ? JSON.parse(rawOrder) : rawOrder,
      votes: votes.slice(0, quorum),
    };
    const certJson = JSON.stringify(cert);
    const applyResults = await Promise.allSettled(
      endpoints.map((ep) => rpc(ep.url, 'owned_apply_v3', { validator_id: ep.validatorId, cert_json: certJson }))
    );

    let acceptedCount = 0;
    let certificateDigestHex = '';
    let finalizedHeight = 0;
    for (const outcome of applyResults) {
      if (outcome.status !== 'fulfilled' || !outcome.value) continue;
      const ack = outcome.value;
      if (ack.accepted === true || ack.applied === true || ack.status === 'applied') {
        acceptedCount += 1;
        if (!certificateDigestHex) {
          const digest = String(ack.certificate_digest || ack.certificateDigest || '').toLowerCase();
          if (/^[0-9a-f]{96}$/.test(digest)) certificateDigestHex = digest;
        }
        const h = Number(ack.block_height || ack.finalized_height || ack.height || 0);
        if (Number.isSafeInteger(h) && h > finalizedHeight) finalizedHeight = h;
      }
    }

    if (acceptedCount === 0) {
      return { accepted: false, reason: 'fastpay_apply_rejected_by_all_validators' };
    }
    if (!certificateDigestHex) {
      return { accepted: false, reason: 'missing_chain_certificate_digest' };
    }
    return {
      accepted: true,
      certificateDigestHex,
      finalizedHeight,
      quorumVotes: votes.length,
      appliedValidators: acceptedCount,
    };
  }

  async function settleConsensus(verified) {
    const primary = endpoints[0];
    const signedTx = verified?.signedTx || {};
    if (verified.lane !== mp.PaymentLane.CONSENSUS_PFT) {
      return { accepted: false, reason: 'unsupported_settlement_lane' };
    }
    const expectedNativeTxId = verified.nativeConsensus ? verified.txHash.toString('hex') : '';
    let txId = expectedNativeTxId || String(signedTx.tx_id || signedTx.onchain_tx_id || '').trim().toLowerCase();

    if (verified.nativeSignedPaymentV2 || signedTx.signed_payment_v2_json || signedTx.signed_payment_v2) {
      const payload = verified.nativeSignedPaymentV2 || signedTx.signed_payment_v2;
      const signedPaymentV2Json =
        typeof signedTx.signed_payment_v2_json === 'string'
          ? signedTx.signed_payment_v2_json
          : JSON.stringify(payload);
      const submitRes = await rpc(primary.url, 'mempool_submit_signed_payment_v2', {
        signed_payment_v2_json: signedPaymentV2Json,
      });
      const returnedTxId = String(submitRes?.tx_id || '').trim().toLowerCase();
      if (expectedNativeTxId && returnedTxId && returnedTxId !== expectedNativeTxId) {
        return { accepted: false, reason: 'node_tx_id_disagrees_with_native_payment_v2' };
      }
      txId = returnedTxId || txId;
    } else if (signedTx.signed_asset_tx_json || signedTx.signed_asset_tx) {
      const signedAssetTxJson =
        typeof signedTx.signed_asset_tx_json === 'string'
          ? signedTx.signed_asset_tx_json
          : JSON.stringify(signedTx.signed_asset_tx);
      const submitRes = await rpc(primary.url, 'mempool_submit_signed_asset_transaction', {
        signed_asset_transaction_json: signedAssetTxJson,
      });
      txId = String(submitRes?.tx_id || txId).trim().toLowerCase();
    }

    if (!/^[0-9a-f]{96}$/.test(txId)) {
      return { accepted: false, reason: 'missing_onchain_tx_id_or_signed_tx_payload' };
    }

    const deadline = Date.now() + finalityTimeoutMs;
    let lastReason = 'finality_timeout';
    while (true) {
      try {
        const txRes = await rpc(primary.url, 'tx', { tx_id: txId });
        if (txRes?.confirmed === true) {
          const check = verifyOnChainTxMatchesPayment(txRes, verified, txId);
          if (!check.ok) {
            return { accepted: false, reason: check.reason };
          }
          return {
            accepted: true,
            finalizedHeight: check.finalizedHeight,
            txId,
          };
        }
        lastReason = 'tx_pending_finality';
      } catch (err) {
        lastReason = String(err?.message || err);
      }
      if (Date.now() >= deadline) {
        return { accepted: false, reason: `consensus_finality_timeout:${lastReason}` };
      }
      await sleepFn(pollIntervalMs);
    }
  }

  async function replayPendingCertificates() {
    if (!authenticatedCommittee) return 0;
    const pending = certStore.listPending();
    let replayed = 0;
    for (const rec of pending) {
      const committee =
        committeeRegistry.getCommittee(rec.committee_epoch) || committeeRegistry.getActiveCommittee();
      if (!committee) continue;
      const requiredAcks = Math.max(quorum, committee.threshold);
      scheduleBackgroundApply(rec, { committee, requiredAcks });
      replayed += 1;
    }
    return replayed;
  }

  async function flushPendingApplies() {
    while (inFlightBackgroundApplies.size > 0) {
      await Promise.allSettled([...inFlightBackgroundApplies]);
    }
  }

  function getStoredCertificate(orderHashHex) {
    return certStore.get(orderHashHex);
  }

  function unconfirmedCertificateCount() {
    return certStore.listPending().length;
  }

  function metrics() {
    return {
      finality_mode: resolvedFinalityMode,
      vote_quorum_ms: [...metricsState.voteQuorumSamplesMs],
      apply_quorum_ms: [...metricsState.applyQuorumSamplesMs],
      unconfirmed_certs: certStore.listPending().length,
      apply_retry_total: metricsState.applyRetryTotal,
    };
  }

  return {
    endpoints,
    quorumThreshold: quorum,
    authenticatedCommittee,
    settlementFinalityMode: resolvedFinalityMode,
    settleFastPay,
    settleConsensus,
    getStoredCertificate,
    unconfirmedCertificateCount,
    replayPendingCertificates,
    flushPendingApplies,
    metrics,
    close: () => {
      for (const client of v2Clients.values()) client.close();
      certStore.close();
    },
  };
}

module.exports = {
  FINALITY_MODE_CERTIFICATE,
  FINALITY_MODE_APPLIED_QUORUM,
  MemoryCertificateStore,
  FileCertificateStore,
  createChainSettlementAdapter,
  parseValidatorEndpoints,
  verifyOnChainTxMatchesPayment,
};
