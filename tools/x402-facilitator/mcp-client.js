'use strict';

// Thin Post Fiat x402 HTTP & MCP (JSON-RPC 2.0) Payment Wrapper (Pillar 2).
//
// Intercepts HTTP 402 Payment Required responses, validates budget/policy caps,
// signs the prototype transfer object of facilitator.js (a JSON format of that file,
// not the chain's OwnedTransferOrderV3 or issued_payment_v2) with the request_hash
// anchored in a pf.ptr v5 INLINE memo, retries with PAYMENT-SIGNATURE, and checks
// the returned PAYMENT-RESPONSE against the payment it signed.

const crypto = require('node:crypto');
const http = require('node:http');
const https = require('node:https');

const mp = require('../../wallet-proxy/message-protocol');
const nativeChain = require('./native-chain');
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

const CONSENSUS_TRANSFER_FEE_ATOMS = 25;

// What a caller states with a call: the price, the asset and the recipient it agreed to pay.
function requireCompleteExpectedPayment({ amountAtoms, assetId, payTo }) {
  const complete =
    Number.isSafeInteger(amountAtoms) &&
    amountAtoms > 0 &&
    typeof assetId === 'string' &&
    assetId.length > 0 &&
    typeof payTo === 'string' &&
    payTo.length > 0;
  if (!complete) {
    throw new X402Error(400, 'invalid_expected_payment', 'expectedPayment needs amountAtoms, assetId and payTo');
  }
}

function requireTermsTheCallerAgreedTo(required, expectedPayment) {
  const asked = { amountAtoms: required.max_amount_atoms, assetId: required.asset_id, payTo: required.pay_to };
  for (const [term, askedValue] of Object.entries(asked)) {
    if (askedValue !== expectedPayment[term]) {
      throw new X402Error(
        403,
        'payment_terms_mismatch',
        `server asks ${term} ${askedValue}, the caller agreed to ${expectedPayment[term]}`
      );
    }
  }
}

class X402HttpClient {
  // maxBudgetAtoms is the most this client object signs away in its lifetime, fees included.
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
    nativePaymentV2 = null,
    nativeFastPay = null,
    now = Date.now,
  }) {
    if (!payerAccount || !payerPublicKey || !payerSecretKey) {
      throw new Error('X402HttpClient requires payerAccount, payerPublicKey, and payerSecretKey');
    }
    if (!Number.isSafeInteger(maxBudgetAtoms) || maxBudgetAtoms < 0) {
      throw new Error('X402HttpClient maxBudgetAtoms must be a non-negative integer');
    }
    this._chainId = chainId;
    this._payerAccount = payerAccount;
    this._payerPublicKey = payerPublicKey;
    this._payerSecretKey = payerSecretKey;
    this._maxBudgetAtoms = maxBudgetAtoms;
    this._signedAtoms = 0;
    this._allowedNetworks = new Set(allowedNetworks);
    this._allowedAssets = new Set(allowedAssets);
    this._nextConsensusSequence = nextConsensusSequence;
    this._directFastPaySettleFn = directFastPaySettleFn;
    this._nativePaymentV2 = nativePaymentV2;
    this._nativeFastPay = nativeFastPay;
    this._now = now;
  }

  // Counts a payment against the budget before it is signed. Nothing is given back afterwards:
  // whoever receives a signed payment can settle it, whatever they answer.
  _reserveBudget(amountAtoms, feeAtoms) {
    const costAtoms = amountAtoms + feeAtoms;
    if (!Number.isSafeInteger(costAtoms) || costAtoms <= 0 || this._signedAtoms + costAtoms > this._maxBudgetAtoms) {
      throw new X402Error(
        403,
        'budget_exceeded',
        `${amountAtoms} atoms plus ${feeAtoms} fee do not fit the client budget: ${this._signedAtoms} of ${this._maxBudgetAtoms} atoms are signed away`
      );
    }
    this._signedAtoms += costAtoms;
  }

  async _buildPaymentSignatureHeader(required, localRequestHash, expectedPayment = null) {
    if (required.x402_version !== 2) {
      throw new X402Error(400, 'unsupported_x402_version', `unsupported x402 version ${required.x402_version}`);
    }
    if (expectedPayment) requireTermsTheCallerAgreedTo(required, expectedPayment);
    if (this._now() > required.expires_at_unix_ms) {
      throw new X402Error(400, 'challenge_already_expired', 'server issued an already-expired 402 challenge');
    }
    if (!this._allowedNetworks.has(required.network)) {
      throw new X402Error(403, 'network_not_allowed', `network ${required.network} is not in client allowlist`);
    }
    if (!this._allowedAssets.has(required.asset_id)) {
      throw new X402Error(403, 'asset_not_allowed', `asset ${required.asset_id} is not in client allowlist`);
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
      if (this._nativeFastPay) {
        const cfg =
          typeof this._nativeFastPay === 'function'
            ? await this._nativeFastPay({ required, localRequestHash, memo: x402Memo })
            : this._nativeFastPay;
        const payeePubkeyHex =
          typeof cfg.payeePubkeyForAccount === 'function'
            ? await cfg.payeePubkeyForAccount(required.pay_to)
            : cfg.payeePubkeyHex;
        if (!payeePubkeyHex || !/^[0-9a-f]+$/i.test(payeePubkeyHex)) {
          throw new X402Error(400, 'missing_payee_pubkey', 'native FastPay requires payee ML-DSA-65 pubkey hex');
        }
        const payerPubkeyHex = Buffer.from(this._payerPublicKey).toString('hex');
        const recovery =
          typeof cfg.recoveryWindow === 'function'
            ? await cfg.recoveryWindow({ required })
            : cfg.recovery;
        const selected =
          typeof cfg.selectInputs === 'function'
            ? await cfg.selectInputs({ required, minAtoms: required.max_amount_atoms })
            : cfg.inputs;
        const inputList = Array.isArray(selected?.inputs) ? selected.inputs : Array.isArray(selected) ? selected : [];
        const inputValueTotal = Number(selected?.totalAtoms ?? cfg.inputTotalAtoms ?? 0);
        const nonceNum = Number(
          selected?.nonce ?? (typeof cfg.nextNonce === 'function' ? await cfg.nextNonce() : cfg.nonce ?? 1)
        );
        const buildDraft = (withChange) => {
          const outputs = [
            { owner_pubkey_hex: String(payeePubkeyHex).toLowerCase(), value: required.max_amount_atoms, asset: required.asset_id },
          ];
          if (withChange) {
            outputs.push({ owner_pubkey_hex: payerPubkeyHex, value: 0, asset: required.asset_id });
          }
          const draft = {
            domain: cfg.domain,
            recovery: {
              schema: nativeChain.FASTPAY_ORDER_RECOVERY_SCHEMA_V1,
              committee_epoch: Number(recovery.committee_epoch),
              lock_id: '00'.repeat(48),
              valid_from_height: Number(recovery.valid_from_height),
              expires_at_height: Number(recovery.expires_at_height),
              recovery_closes_at_height: Number(recovery.recovery_closes_at_height),
            },
            inputs: inputList.map((i) => ({ id: String(i.id), version: Number(i.version) })),
            outputs,
            fee: 0,
            nonce: nonceNum,
            memos: [x402Memo],
          };
          const minFee = computeMinimumFastPayFeeAtoms({
            weightBytes: nativeChain.ownedTransferV3SigningBytes(draft).length,
            inputCount: draft.inputs.length,
            outputCount: draft.outputs.length,
          });
          return { draft, minFee };
        };
        let { draft: nativeOrder, minFee } = buildDraft(true);
        const changeAtoms = inputValueTotal - required.max_amount_atoms - minFee;
        if (changeAtoms > 0) {
          nativeOrder.fee = minFee;
          nativeOrder.outputs[1].value = changeAtoms;
        } else {
          ({ draft: nativeOrder, minFee } = buildDraft(false));
          const exactFee = inputValueTotal - required.max_amount_atoms;
          if (exactFee < minFee) {
            throw new X402Error(402, 'insufficient_fastpay_input', 'FastPay input cannot cover amount + minimum fee');
          }
          nativeOrder.fee = exactFee;
        }
        this._reserveBudget(required.max_amount_atoms, nativeOrder.fee);
        const signed = await nativeChain.signNativeOwnedTransferOrderV3(nativeOrder, {
          publicKey: this._payerPublicKey,
          secretKey: this._payerSecretKey,
        });
        orderHash = signed.orderHash;
        signedPayload = signed.signedPayload;

        if (required.scheme === mp.X402Scheme.PF_FASTPAY_SETTLED) {
          if (typeof this._directFastPaySettleFn !== 'function') {
            throw new X402Error(400, 'missing_direct_settle_fn', 'pf-fastpay-settled requires directFastPaySettleFn');
          }
          certificatePayload = await this._directFastPaySettleFn({
            orderHash,
            signedPayload,
            order: signed.signedOrder.order,
            signedOrder: signed.signedOrder,
          });
        }
      } else {
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
        this._reserveBudget(baseOrder.amount_atoms, baseOrder.fee_atoms);
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
      }
    } else if (
      required.lane === mp.PaymentLane.CONSENSUS_ISSUED ||
      required.lane === mp.PaymentLane.CONSENSUS_PFT
    ) {
      if (required.lane === mp.PaymentLane.CONSENSUS_PFT && this._nativePaymentV2) {
        const cfg =
          typeof this._nativePaymentV2 === 'function'
            ? await this._nativePaymentV2({ required, localRequestHash, memo: x402Memo })
            : this._nativePaymentV2;
        const feeAtoms = Number(cfg.feeAtoms ?? CONSENSUS_TRANSFER_FEE_ATOMS);
        this._reserveBudget(required.max_amount_atoms, feeAtoms);
        const unsigned = nativeChain.buildUnsignedPaymentV2({
          chainId: this._chainId,
          genesisHash: cfg.genesisHash,
          protocolVersion: Number(cfg.protocolVersion ?? 1),
          from: this._payerAccount,
          to: required.pay_to,
          amount: required.max_amount_atoms,
          fee: feeAtoms,
          sequence: await this._nextConsensusSequence(),
          memos: [x402Memo],
        });
        const signed = await nativeChain.signNativePaymentV2(unsigned, {
          publicKey: this._payerPublicKey,
          secretKey: this._payerSecretKey,
        });
        txHash = signed.txId;
        signedPayload = signed.signedPayload;
      } else {
        this._reserveBudget(required.max_amount_atoms, CONSENSUS_TRANSFER_FEE_ATOMS);
        const tx = {
          chain_id: this._chainId,
          lane: required.lane,
          payer_account: this._payerAccount,
          payee_account: required.pay_to,
          asset_id: required.asset_id,
          amount_atoms: required.max_amount_atoms,
          fee_atoms: CONSENSUS_TRANSFER_FEE_ATOMS,
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
      }
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

  // With expectedPayment ({ amountAtoms, assetId, payTo }) the client pays only a challenge that
  // asks exactly that. Without it the server's challenge sets them, within the budget and the
  // asset and network lists.
  async request(targetUrl, { method = 'POST', headers = {}, body = '', expectedPayment = null } = {}) {
    if (expectedPayment) requireCompleteExpectedPayment(expectedPayment);
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
      localRequestHash,
      expectedPayment
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
      if (expectedOrderHash.length === mp.CHAIN_HASH_BYTES) {
        if (!crypto.timingSafeEqual(paymentReceipt.payment_ref.order_hash, expectedOrderHash)) {
          throw new X402Error(502, 'receipt_order_hash_mismatch', 'PAYMENT-RESPONSE order_hash mismatch');
        }
      } else if (expectedTxHash.length === mp.CHAIN_HASH_BYTES) {
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

  async callTool(toolName, args = {}, { expectedPayment = null } = {}) {
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
      expectedPayment,
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
