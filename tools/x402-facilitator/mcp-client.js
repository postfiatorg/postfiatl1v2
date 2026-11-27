'use strict';

// Thin Post Fiat x402 HTTP & MCP (JSON-RPC 2.0) Payment Wrapper (Pillar 2).
//
// Intercepts HTTP 402 Payment Required responses, validates budget/policy caps,
// signs a FastPay OwnedTransferOrderV3 (or consensus-lane issued_payment_v2) with
// the request_hash anchored in a pf.ptr v5 INLINE memo, retries with
// PAYMENT-SIGNATURE, and verifies the returned PAYMENT-RESPONSE settlement receipt.

const crypto = require('node:crypto');
const http = require('node:http');
const https = require('node:https');

const mp = require('../../wallet-proxy/message-protocol');
const {
  HEADER_PAYMENT_REQUIRED,
  HEADER_PAYMENT_SIGNATURE,
  HEADER_PAYMENT_RESPONSE,
  X402Error,
  buildX402InlineMemo,
  computeHttpRequestHash,
  computeMinimumFastPayFeeAtoms,
  signConsensusTransfer,
  signFastPayOrder,
} = require('./facilitator');

function httpRequest(targetUrl, { method = 'POST', headers = {}, body = '' } = {}) {
  return new Promise((resolve, reject) => {
    const parsed = new URL(targetUrl);
    const transport = parsed.protocol === 'https:' ? https : http;
    const bodyBuf = Buffer.isBuffer(body) ? body : Buffer.from(String(body || ''), 'utf8');
    const req = transport.request(
      {
        hostname: parsed.hostname,
        port: parsed.port || (parsed.protocol === 'https:' ? 443 : 80),
        path: `${parsed.pathname}${parsed.search}`,
        method,
        headers: {
          'content-length': bodyBuf.length,
          ...headers,
        },
      },
      (res) => {
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () => {
          resolve({
            statusCode: res.statusCode,
            headers: res.headers,
            body: Buffer.concat(chunks),
          });
        });
      }
    );
    req.on('error', reject);
    if (bodyBuf.length > 0) req.write(bodyBuf);
    req.end();
  });
}

class X402HttpClient {
  constructor({
    chainId = 'postfiat-devnet-2',
    payerAccount,
    payerPublicKey,
    payerSecretKey,
    maxBudgetAtoms = 1_000_000,
    allowedNetworks = ['postfiat:devnet-2'],
    allowedAssets = ['PFT'],
    nextConsensusSequence = () => 1,
    directFastPaySettleFn = null,
    now = Date.now,
  }) {
    if (!payerAccount || !payerPublicKey || !payerSecretKey) {
      throw new Error('X402HttpClient requires payerAccount, payerPublicKey, and payerSecretKey');
    }
    this._chainId = chainId;
    this._payerAccount = payerAccount;
    this._payerPublicKey = payerPublicKey;
    this._payerSecretKey = payerSecretKey;
    this._maxBudgetAtoms = maxBudgetAtoms;
    this._allowedNetworks = new Set(allowedNetworks);
    this._allowedAssets = new Set(allowedAssets);
    this._nextConsensusSequence = nextConsensusSequence;
    this._directFastPaySettleFn = directFastPaySettleFn;
    this._now = now;
  }

  async _buildPaymentSignatureHeader(required, localRequestHash) {
    if (required.x402_version !== 2) {
      throw new X402Error(400, 'unsupported_x402_version', `unsupported x402 version ${required.x402_version}`);
    }
    if (this._now() > required.expires_at_unix_ms) {
      throw new X402Error(400, 'challenge_already_expired', 'server issued an already-expired 402 challenge');
    }
    if (!this._allowedNetworks.has(required.network)) {
      throw new X402Error(403, 'network_not_allowed', `network ${required.network} is not in client allowlist`);
    }
    if (!this._allowedAssets.has(required.asset_id)) {
      throw new X402Error(403, 'asset_not_allowed', `asset ${required.asset_id} is not in client allowlist`);
    }
    if (required.max_amount_atoms > this._maxBudgetAtoms) {
      throw new X402Error(
        403,
        'budget_exceeded',
        `requested ${required.max_amount_atoms} atoms exceeds client maxBudgetAtoms ${this._maxBudgetAtoms}`
      );
    }
    if (
      required.request_hash.length !== 32 ||
      !crypto.timingSafeEqual(required.request_hash, localRequestHash)
    ) {
      throw new X402Error(400, 'server_request_hash_mismatch', 'server PAYMENT-REQUIRED request_hash mismatch');
    }

    const x402Memo = buildX402InlineMemo(localRequestHash);
    let orderHash = Buffer.alloc(0);
    let txHash = Buffer.alloc(0);
    let signedPayload = Buffer.alloc(0);
    let certificatePayload = Buffer.alloc(0);

    if (required.lane === mp.PaymentLane.FASTPAY) {
      const baseOrder = {
        chain_id: this._chainId,
        payer_account: this._payerAccount,
        payee_account: required.pay_to,
        asset_id: required.asset_id,
        amount_atoms: required.max_amount_atoms,
        fee_atoms: 0,
        input_count: 1,
        output_count: 2,
        nonce_hex: required.nonce.toString('hex'),
        memos: [x402Memo],
      };
      // Compute fee floor conservatively using signed payload size estimate (order + ML-DSA-65 pk/sig hex)
      const estBytes = mp.canonicalJsonBytes(baseOrder).length + (mp.ML_DSA_65_PUBKEY_BYTES + mp.ML_DSA_65_SIGNATURE_BYTES) * 2 + 256;
      baseOrder.fee_atoms = computeMinimumFastPayFeeAtoms({
        weightBytes: estBytes,
        inputCount: baseOrder.input_count,
        outputCount: baseOrder.output_count,
      });
      const signed = await signFastPayOrder(baseOrder, {
        publicKey: this._payerPublicKey,
        secretKey: this._payerSecretKey,
      });
      orderHash = signed.orderHash;
      signedPayload = signed.signedPayload;

      if (required.scheme === mp.X402Scheme.PF_FASTPAY_SETTLED) {
        if (typeof this._directFastPaySettleFn !== 'function') {
          throw new X402Error(400, 'missing_direct_settle_fn', 'pf-fastpay-settled requires directFastPaySettleFn');
        }
        certificatePayload = await this._directFastPaySettleFn({ orderHash, signedPayload, order: baseOrder });
      }
    } else if (
      required.lane === mp.PaymentLane.CONSENSUS_ISSUED ||
      required.lane === mp.PaymentLane.CONSENSUS_PFT
    ) {
      const tx = {
        chain_id: this._chainId,
        lane: required.lane,
        payer_account: this._payerAccount,
        payee_account: required.pay_to,
        asset_id: required.asset_id,
        amount_atoms: required.max_amount_atoms,
        fee_atoms: 25,
        sequence: await this._nextConsensusSequence(),
        nonce_hex: required.nonce.toString('hex'),
        memos: [x402Memo],
      };
      const signed = await signConsensusTransfer(tx, {
        publicKey: this._payerPublicKey,
        secretKey: this._payerSecretKey,
      });
      txHash = signed.txHash;
      signedPayload = signed.signedPayload;
    } else {
      throw new X402Error(400, 'unsupported_lane', `unsupported lane ${required.lane}`);
    }

    const sigMsg = {
      x402_version: 2,
      scheme: required.scheme,
      network: required.network,
      lane: required.lane,
      request_hash: localRequestHash,
      nonce: required.nonce,
      order_hash: orderHash,
      tx_hash: txHash,
      signed_payload: signedPayload,
      certificate_payload: certificatePayload,
    };
    return {
      headerValue: mp.encodeX402Header(mp.encodeX402PaymentSignature(sigMsg)),
      expectedOrderHash: orderHash,
      expectedTxHash: txHash,
    };
  }

  async request(targetUrl, { method = 'POST', headers = {}, body = '' } = {}) {
    const parsed = new URL(targetUrl);
    const bodyBuf = Buffer.isBuffer(body) ? body : Buffer.from(String(body || ''), 'utf8');
    const firstRes = await httpRequest(targetUrl, { method, headers, body: bodyBuf });
    if (firstRes.statusCode !== 402) {
      return { ...firstRes, paymentReceipt: null };
    }

    const reqHeader = firstRes.headers[HEADER_PAYMENT_REQUIRED];
    if (!reqHeader) {
      throw new X402Error(502, 'missing_payment_required_header', '402 response lacked PAYMENT-REQUIRED header');
    }
    const required = mp.decodeX402PaymentRequired(mp.decodeX402Header(String(reqHeader)));
    const localRequestHash = computeHttpRequestHash({
      method,
      urlPath: `${parsed.pathname}${parsed.search}`,
      contentType: headers['content-type'] || headers['Content-Type'] || '',
      body: bodyBuf,
    });
    const { headerValue, expectedOrderHash, expectedTxHash } = await this._buildPaymentSignatureHeader(
      required,
      localRequestHash
    );

    const retryRes = await httpRequest(targetUrl, {
      method,
      headers: {
        ...headers,
        [HEADER_PAYMENT_SIGNATURE]: headerValue,
      },
      body: bodyBuf,
    });

    let paymentReceipt = null;
    const respHeader = retryRes.headers[HEADER_PAYMENT_RESPONSE];
    if (respHeader) {
      paymentReceipt = mp.decodeX402PaymentResponse(mp.decodeX402Header(String(respHeader)));
      if (!paymentReceipt.settled || !paymentReceipt.payment_ref) {
        throw new X402Error(502, 'unsettled_payment_response', 'server returned unsettled PAYMENT-RESPONSE');
      }
      if (expectedOrderHash.length === 32) {
        if (!crypto.timingSafeEqual(paymentReceipt.payment_ref.order_hash, expectedOrderHash)) {
          throw new X402Error(502, 'receipt_order_hash_mismatch', 'PAYMENT-RESPONSE order_hash mismatch');
        }
      } else if (expectedTxHash.length === 32) {
        if (!crypto.timingSafeEqual(paymentReceipt.payment_ref.tx_hash, expectedTxHash)) {
          throw new X402Error(502, 'receipt_tx_hash_mismatch', 'PAYMENT-RESPONSE tx_hash mismatch');
        }
      }
    }
    return { ...retryRes, paymentReceipt };
  }
}

// Thin MCP JSON-RPC 2.0 tool-call wrapper over X402HttpClient.
class X402McpClient {
  constructor({ endpointUrl, httpClient }) {
    this._endpointUrl = endpointUrl;
    this._httpClient = httpClient;
    this._nextId = 1;
  }

  async callTool(toolName, args = {}) {
    const id = this._nextId++;
    const payload = JSON.stringify({
      jsonrpc: '2.0',
      id,
      method: 'tools/call',
      params: { name: toolName, arguments: args },
    });
    const res = await this._httpClient.request(this._endpointUrl, {
      method: 'POST',
      headers: { 'content-type': 'application/json', accept: 'application/json' },
      body: payload,
    });
    if (res.statusCode !== 200) {
      throw new X402Error(res.statusCode, 'mcp_call_failed', res.body.toString('utf8'));
    }
    const rpc = JSON.parse(res.body.toString('utf8'));
    if (rpc.error) {
      throw new X402Error(500, 'mcp_rpc_error', rpc.error.message || JSON.stringify(rpc.error));
    }
    return {
      result: rpc.result,
      paymentReceipt: res.paymentReceipt,
    };
  }
}

module.exports = {
  X402HttpClient,
  X402McpClient,
};
