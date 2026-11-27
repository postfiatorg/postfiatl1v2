'use strict';

// Live JSON-RPC settlement adapter for Post Fiat L1v2 x402 v2 facilitator.
//
// Connects `tools/x402-facilitator` to real postfiatl1v2 validator JSON-RPC endpoints:
//  - FastPay lane: fans out `owned_sign_v3` across configured validators to collect a
//    Byzantine quorum of votes, assembles the transfer certificate, and calls `owned_apply_v3`.
//  - Consensus lane: submits signed `payment_v2` (`mempool_submit_signed_payment_v2`) or
//    issued asset transactions (`mempool_submit_signed_asset_transaction`), then polls `tx`
//    until on-chain finality (`accepted === true && block_height > 0`) and verifies the
//    on-chain recipient, amount, and pf.ptr v5 INLINE request_hash memo.

const crypto = require('node:crypto');
const mp = require('../../wallet-proxy/message-protocol');
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
    // Supports either `https://host:port` or `validator-id=https://host:port`.
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
  if (!/^[0-9a-f]{96}$/.test(txId) || txResult.tx_id !== txId || txResult.receipt.tx_id !== txId ||
      txResult.chain_id !== verified.signedTx.chain_id) {
    return { ok: false, reason: 'onchain_tx_identity_mismatch' };
  }
  const blockHeight = txResult.block?.header?.height;
  if (!Number.isSafeInteger(blockHeight) || blockHeight <= 0) {
    return { ok: false, reason: 'onchain_tx_not_finalized' };
  }
  const rows = txResult.transaction_rows;
  const row = Array.isArray(rows) && rows.length === 1 ? rows[0] : null;
  // Only payment_v2 archives contain signed memo bytes. Issued-asset rows cannot
  // attest to the x402 request hash under this version of the node RPC.
  if (verified.lane !== mp.PaymentLane.CONSENSUS_PFT || verified.assetId !== 'PFT' ||
      row?.transaction_kind !== 'payment_v2' || row.tx_id !== txId || row.block_height !== blockHeight ||
      row.from !== verified.payerAccount || row.to !== verified.payeeAccount ||
      !Number.isSafeInteger(row.amount) || row.amount !== verified.amountAtoms) {
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
  rpcTimeoutMs = DEFAULT_RPC_TIMEOUT_MS,
  pollIntervalMs = DEFAULT_POLL_INTERVAL_MS,
  finalityTimeoutMs = DEFAULT_FINALITY_TIMEOUT_MS,
  fetchFn = globalThis.fetch,
  sleepFn = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
} = {}) {
  const endpoints = parseValidatorEndpoints(rpcUrls, validatorIds);
  const quorum = quorumThreshold ?? defaultQuorumThreshold(endpoints.length);
  if (!Number.isSafeInteger(quorum) || quorum <= 0 || quorum > endpoints.length) {
    throw new Error(`quorumThreshold ${quorum} must be within 1..${endpoints.length}`);
  }

  const rpc = (url, method, params) =>
    defaultJsonRpcCall(url, method, params, { timeoutMs: rpcTimeoutMs, fetchFn });

  async function settleFastPay(verified) {
    const rawOrder =
      verified?.signedOrder?.owned_transfer_order_v3 ||
      verified?.signedOrder?.chain_order ||
      verified?.signedOrder;
    if (!rawOrder || typeof rawOrder !== 'object') {
      return { accepted: false, reason: 'missing_fastpay_order_payload' };
    }
    const orderJson = typeof rawOrder === 'string' ? rawOrder : JSON.stringify(rawOrder);

    // 1. Collect Byzantine quorum votes via `owned_sign_v3` across validators.
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

    // 2. Assemble certificate and broadcast `owned_apply_v3` to validators.
    const cert = {
      order: typeof rawOrder === 'string' ? JSON.parse(rawOrder) : rawOrder,
      votes: votes.slice(0, quorum),
    };
    const certJson = JSON.stringify(cert);
    const applyResults = await Promise.allSettled(
      endpoints.map((ep) => rpc(ep.url, 'owned_apply_v3', { cert_json: certJson }))
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
    if (verified.lane !== mp.PaymentLane.CONSENSUS_PFT) return { accepted: false, reason: 'unsupported_settlement_lane' };
    let txId = String(signedTx.tx_id || signedTx.onchain_tx_id || '').trim().toLowerCase();

    // 1. Submit signed transaction if raw chain payload is provided and not already finalized.
    if (signedTx.signed_payment_v2_json || signedTx.signed_payment_v2) {
      const signedPaymentV2Json =
        typeof signedTx.signed_payment_v2_json === 'string'
          ? signedTx.signed_payment_v2_json
          : JSON.stringify(signedTx.signed_payment_v2);
      const submitRes = await rpc(primary.url, 'mempool_submit_signed_payment_v2', {
        signed_payment_v2_json: signedPaymentV2Json,
      });
      txId = String(submitRes?.tx_id || txId).trim().toLowerCase();
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

    // 2. Poll `tx` until finalized on-chain and verify recipient, amount, and request_hash memo.
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
    settleFastPay,
    settleConsensus,
  };
}

module.exports = {
  createChainSettlementAdapter,
  parseValidatorEndpoints,
  verifyOnChainTxMatchesPayment,
};
