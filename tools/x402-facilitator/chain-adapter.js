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
const mp = require('../../wallet-proxy/message-protocol');
const nativeChain = require('./native-chain');
const { extractX402RequestHashFromMemos } = require('./facilitator');

const DEFAULT_RPC_TIMEOUT_MS = 10_000;
const DEFAULT_POLL_INTERVAL_MS = 500;
const DEFAULT_FINALITY_TIMEOUT_MS = 30_000;

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
  validatorIds = [],
  quorumThreshold = null,
  committeeRegistry = null,
  expectedDomain = null,
  requireAuthenticatedCommittee = false,
  verifyIndexedOrder = false,
  rpcTimeoutMs = DEFAULT_RPC_TIMEOUT_MS,
  pollIntervalMs = DEFAULT_POLL_INTERVAL_MS,
  finalityTimeoutMs = DEFAULT_FINALITY_TIMEOUT_MS,
  fetchFn = globalThis.fetch,
  sleepFn = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
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

  const rpc = (url, method, params) =>
    defaultJsonRpcCall(url, method, params, { timeoutMs: rpcTimeoutMs, fetchFn });

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
    const verifiedVotes = [];
    const seenValidators = new Set();
    const errors = [];

    // 1. Fan out `owned_sign_v3` and cryptographically authenticate every committee vote.
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
      if (outcome.status !== 'fulfilled' || !outcome.value?.vote) {
        errors.push(String(outcome.reason?.message || outcome.reason || 'rpc_failed'));
        continue;
      }
      const ep = outcome.value.endpoint;
      const rawVote = outcome.value.vote.vote || outcome.value.vote;
      if (rawVote.error || rawVote.rejected === true) {
        errors.push(`${ep.validatorId}:${rawVote.error || 'rejected'}`);
        continue;
      }
      const voteValidatorId = String(rawVote.validator_id || '');
      if (!voteValidatorId || voteValidatorId !== ep.validatorId) {
        errors.push(`${ep.validatorId}:validator_id_mismatch`);
        continue;
      }
      if (seenValidators.has(voteValidatorId)) {
        errors.push(`${voteValidatorId}:duplicate_vote`);
        continue;
      }
      const pk = committee.validators.get(voteValidatorId);
      if (!pk) {
        errors.push(`${voteValidatorId}:not_in_active_committee`);
        continue;
      }
      const sigHex = String(rawVote.signature_hex || '').toLowerCase();
      const sigBuf = Buffer.from(sigHex, 'hex');
      if (sigBuf.length !== mp.ML_DSA_65_SIGNATURE_BYTES) {
        errors.push(`${voteValidatorId}:invalid_vote_signature_length`);
        continue;
      }
      if (!mlDsa65.verify(sigBuf, ownerCheck.signingBytes, pk, { context: nativeChain.OWNED_TRANSFER_CONTEXT_V3 })) {
        errors.push(`${voteValidatorId}:invalid_vote_signature`);
        continue;
      }
      seenValidators.add(voteValidatorId);
      verifiedVotes.push({ validator_id: voteValidatorId, signature_hex: sigHex });
    }

    const requiredVotes = Math.max(quorum, committee.threshold);
    if (verifiedVotes.length < requiredVotes) {
      return {
        accepted: false,
        reason: `fastpay_authenticated_quorum_not_reached:${verifiedVotes.length}/${requiredVotes}${errors.length ? ` (${errors[0]})` : ''}`,
      };
    }

    // 2. Assemble canonical native `OwnedTransferCertificateV3` and verify locally.
    const cert = {
      order: signedOrder.order,
      owner_pubkey_hex: signedOrder.owner_pubkey_hex,
      owner_signature_hex: signedOrder.owner_signature_hex,
      votes: verifiedVotes.slice(0, requiredVotes),
    };
    const certCheck = await nativeChain.verifyNativeOwnedTransferCertificateV3(cert, {
      committee,
      expectedDomain,
      expectedOrderHash: ownerCheck.orderHash,
    });
    if (!certCheck.ok) {
      return { accepted: false, reason: `assembled_certificate_invalid:${certCheck.reason}` };
    }
    const expectedCertDigestHex = certCheck.certificateDigest.toString('hex');
    const certJson = JSON.stringify(cert);

    // 3. Broadcast `owned_apply_v3` with `(validator_id, cert_json)` and authenticate `FastPayApplyAckV1`.
    const applyResults = await Promise.allSettled(
      endpoints.map(async (ep) => {
        const ack = await rpc(ep.url, 'owned_apply_v3', {
          validator_id: ep.validatorId,
          cert_json: certJson,
        });
        return { endpoint: ep, ack };
      })
    );

    const authenticatedAcks = new Set();
    let finalizedHeight = 0;
    for (const outcome of applyResults) {
      if (outcome.status !== 'fulfilled' || !outcome.value?.ack) continue;
      const ep = outcome.value.endpoint;
      const ack = outcome.value.ack;
      const pk = committee.validators.get(ep.validatorId);
      if (!pk) continue;
      if (
        ack.validator_id !== ep.validatorId ||
        String(ack.order_digest || '').toLowerCase() !== ownerCheck.orderHashHex ||
        String(ack.lock_id || '').toLowerCase() !== ownerCheck.lockId ||
        String(ack.certificate_digest || '').toLowerCase() !== expectedCertDigestHex ||
        Number(ack.committee_epoch) !== Number(committee.epoch) ||
        !nativeChain.domainsMatch(ack.domain, expectedDomain)
      ) {
        continue;
      }
      const ackSigOk = await nativeChain.verifyNativeFastPayApplyAckV1(ack, pk);
      if (!ackSigOk) continue;
      authenticatedAcks.add(ep.validatorId);
      const h = Number(ack.block_height || ack.finalized_height || ack.height || 0);
      if (Number.isSafeInteger(h) && h > finalizedHeight) finalizedHeight = h;
    }

    if (authenticatedAcks.size === 0) {
      return { accepted: false, reason: 'fastpay_apply_no_authenticated_ack' };
    }

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

  return {
    endpoints,
    quorumThreshold: quorum,
    authenticatedCommittee,
    settleFastPay,
    settleConsensus,
  };
}

module.exports = {
  createChainSettlementAdapter,
  parseValidatorEndpoints,
  verifyOnChainTxMatchesPayment,
};
