'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');

const mp = require('../../wallet-proxy/message-protocol');
const {
  buildX402InlineMemo,
  computeHttpRequestHash,
  createFacilitatorCore,
  signConsensusTransfer,
  signFastPayOrder,
} = require('./facilitator');
const { createChainSettlementAdapter } = require('./chain-adapter');
const { createFacilitatorFromEnv } = require('./server');

function startMockValidatorRpc(handler) {
  return new Promise((resolve) => {
    const srv = http.createServer((req, res) => {
      const chunks = [];
      req.on('data', (c) => chunks.push(c));
      req.on('end', () => {
        const body = JSON.parse(Buffer.concat(chunks).toString('utf8'));
        assert.equal(body.version, 'postfiat-local-rpc-v1');
        const out = handler(body.method, body.params || {});
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(JSON.stringify({ version: body.version, id: body.id, ok: !out.error, ...out }));
      });
    });
    srv.listen(0, '127.0.0.1', () => {
      const addr = srv.address();
      resolve({ server: srv, url: `http://127.0.0.1:${addr.port}` });
    });
  });
}

async function runTests() {
  const { mlDsa65 } = await mp.loadPqModules();
  const payer = mlDsa65.keygen(crypto.randomBytes(32));
  const payeeAccount = 'pf721bb3b98bceeeebfb835d97dac6a7b4a72c797a';
  const payerAccount = mp.deriveAccountAddress(payer.publicKey);
  const requestHash = computeHttpRequestHash({
    method: 'GET',
    urlPath: '/api/quote',
    contentType: '',
    body: Buffer.alloc(0),
  });

  // 1. FastPay quorum settlement over real JSON-RPC (`owned_sign_v3` x4 w/ 1 lagging -> `owned_apply_v3`)
  const signCalls = [];
  const applyCalls = [];
  const validators = [];
  for (let i = 0; i < 4; i += 1) {
    validators.push(
      await startMockValidatorRpc((method, params) => {
        if (method === 'owned_sign_v3') {
          signCalls.push(params.validator_id);
          if (i === 3) return { error: { code: -32000, message: 'validator lagging' } };
          return { result: { vote: { validator_id: params.validator_id, sig: `sig-${i}` } } };
        }
        if (method === 'owned_apply_v3') {
          applyCalls.push(i);
          return { result: { accepted: true, certificate_digest: 'ab'.repeat(48), block_height: 1932 } };
        }
        return { error: { code: -32601, message: 'unknown method' } };
      })
    );
  }

  try {
    const adapter = createChainSettlementAdapter({
      rpcUrls: validators.map((v, i) => `val-${i}=${v.url}`),
    });
    assert.equal(adapter.quorumThreshold, 3);

    const core = createFacilitatorCore({
      chainId: 'postfiat-wan-devnet-2',
      network: 'postfiat:wan-devnet-2',
      settlementAdapter: adapter,
    });

    const challenge = core.issueChallenge({
      scheme: mp.X402Scheme.EXACT,
      lane: mp.PaymentLane.FASTPAY,
      assetId: 'PFT',
      maxAmountAtoms: 5000,
      payTo: payeeAccount,
      requestHash,
      resourceUri: '/api/quote',
    });
    const order = {
      chain_id: 'postfiat-wan-devnet-2',
      payer_account: payerAccount,
      payee_account: payeeAccount,
      asset_id: 'PFT',
      amount_atoms: 5000,
      fee_atoms: 60,
      input_count: 1,
      output_count: 1,
      nonce_hex: challenge.required.nonce.toString('hex'),
      memos: [buildX402InlineMemo(requestHash)],
    };
    const signed = await signFastPayOrder(order, payer);
    const sigHeader = mp.encodeX402Header(
      mp.encodeX402PaymentSignature({
        x402_version: 2,
        scheme: mp.X402Scheme.EXACT,
        network: 'postfiat:wan-devnet-2',
        lane: mp.PaymentLane.FASTPAY,
        request_hash: requestHash,
        nonce: challenge.required.nonce,
        order_hash: signed.orderHash,
        signed_payload: signed.signedPayload,
      })
    );
    const settled = await core.settlePayment(sigHeader, { expectedRequired: challenge.required });
    assert.equal(settled.response.settled, true);
    assert.equal(settled.response.payment_ref.finalized_height, 1932);
    assert.equal(signCalls.length, 4);
    assert.equal(applyCalls.length, 4);

    // 2. Consensus lane submission + `tx` finality check (with memo request_hash verification)
    const consensusVal = await startMockValidatorRpc((method, params) => {
      if (method === 'mempool_submit_signed_payment_v2') {
        return { result: { tx_id: 'cd'.repeat(48) } };
      }
      if (method === 'tx') {
        assert.equal(params.tx_id, 'cd'.repeat(48));
        return {
          result: {
            tx_id: 'cd'.repeat(48),
            chain_id: 'postfiat-wan-devnet-2',
            confirmed: true,
            receipt: { tx_id: params.tx_id, accepted: true },
            block: { header: { height: 1933 } },
            transaction_rows: [{ tx_id: params.tx_id, transaction_kind: 'payment_v2',
              block_height: 1933, from: payerAccount, to: payeeAccount, amount: 5000,
              memos: [buildX402InlineMemo(requestHash)] }],
          },
        };
      }
      return { error: { code: -32601, message: 'unknown' } };
    });

    try {
      const consensusAdapter = createChainSettlementAdapter({
        rpcUrls: [consensusVal.url],
        pollIntervalMs: 10,
        finalityTimeoutMs: 2000,
      });
      const consCore = createFacilitatorCore({
        chainId: 'postfiat-wan-devnet-2',
        network: 'postfiat:wan-devnet-2',
        settlementAdapter: consensusAdapter,
      });
      const cChallenge = consCore.issueChallenge({
        scheme: mp.X402Scheme.EXACT,
        lane: mp.PaymentLane.CONSENSUS_PFT,
        assetId: 'PFT',
        maxAmountAtoms: 5000,
        payTo: payeeAccount,
        requestHash,
        resourceUri: '/api/quote',
      });
      const txObj = {
        chain_id: 'postfiat-wan-devnet-2',
        lane: mp.PaymentLane.CONSENSUS_PFT,
        payer_account: payerAccount,
        payee_account: payeeAccount,
        asset_id: 'PFT',
        amount_atoms: 5000,
        fee_atoms: 22,
        sequence: 1,
        nonce_hex: cChallenge.required.nonce.toString('hex'),
        memos: [buildX402InlineMemo(requestHash)],
        signed_payment_v2_json: '{"signed":"tx"}',
      };
      const signedCons = await signConsensusTransfer(txObj, payer);
      const cSigHeader = mp.encodeX402Header(
        mp.encodeX402PaymentSignature({
          x402_version: 2,
          scheme: mp.X402Scheme.EXACT,
          network: 'postfiat:wan-devnet-2',
          lane: mp.PaymentLane.CONSENSUS_PFT,
          request_hash: requestHash,
          nonce: cChallenge.required.nonce,
          tx_hash: signedCons.txHash,
          signed_payload: signedCons.signedPayload,
        })
      );
      const consSettled = await consCore.settlePayment(cSigHeader, { expectedRequired: cChallenge.required });
      assert.equal(consSettled.response.settled, true);
      assert.equal(consSettled.response.payment_ref.finalized_height, 1933);
      assert.equal(consSettled.response.payment_ref.tx_hash.toString('hex'), 'cd'.repeat(48));

      // Configured endpoints alone cannot supply a trusted committee.
      const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'x402-prod-env-'));
      try {
        assert.throws(() => createFacilitatorFromEnv({
          X402_MODE: 'production',
          X402_SPENT_STORE_PATH: path.join(tmpDir, 'spent.ndjson'),
          X402_RPC_URLS: validators.map((v) => v.url).join(','),
        }), /injected settlement adapter/);
      } finally {
        fs.rmSync(tmpDir, { recursive: true, force: true });
      }
    } finally {
      consensusVal.server.close();
    }
  } finally {
    validators.forEach((v) => v.server.close());
  }

  console.log('test_x402_chain_adapter: prototype adapter contract tests passed');
}

runTests().catch((err) => {
  console.error(err);
  process.exit(1);
});
