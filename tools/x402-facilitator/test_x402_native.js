'use strict';

// End-to-end tests for native Post Fiat L1v2 x402 chain-byte signing,
// 48-byte SHA3-384 identity agreement (client == facilitator == node == archive),
// active-committee ML-DSA-65 quorum verification, and production fail-closed startup.

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');

const mp = require('../../wallet-proxy/message-protocol');
const nativeChain = require('./native-chain');
const {
  HEADER_PAYMENT_RESPONSE,
  CommitteeRegistry,
  createFacilitatorCore,
  createX402Gate,
} = require('./facilitator');
const { X402HttpClient, X402McpClient } = require('./mcp-client');
const { createChainSettlementAdapter } = require('./chain-adapter');
const { createFacilitatorFromEnv } = require('./server');

function startMockValidatorRpc(handler) {
  return new Promise((resolve) => {
    const srv = http.createServer((req, res) => {
      const chunks = [];
      req.on('data', (c) => chunks.push(c));
      req.on('end', async () => {
        const body = JSON.parse(Buffer.concat(chunks).toString('utf8'));
        assert.equal(body.version, 'postfiat-local-rpc-v1');
        const out = await handler(body.method, body.params || {});
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(JSON.stringify({ version: body.version, id: body.id, ok: !out.error, ...out }));
      });
    });
    srv.listen(0, '127.0.0.1', () => {
      resolve({ server: srv, url: `http://127.0.0.1:${srv.address().port}` });
    });
  });
}

function startResourceServer(enforceX402) {
  return new Promise((resolve) => {
    const srv = http.createServer({ maxHeaderSize: 65_536 }, (req, res) => {
      const chunks = [];
      req.on('data', (c) => chunks.push(c));
      req.on('end', async () => {
        const raw = Buffer.concat(chunks);
        const settled = await enforceX402(req, res, raw);
        if (!settled) return;
        const body = JSON.parse(raw.toString('utf8'));
        const out = JSON.stringify({
          jsonrpc: '2.0',
          id: body.id,
          result: { ok: true, tool: body.params?.name, paid_atoms: settled.response.payment_ref.amount_atoms },
        });
        res.writeHead(200, {
          'content-type': 'application/json',
          [HEADER_PAYMENT_RESPONSE]: settled.responseHeaderValue,
        });
        res.end(out);
      });
    });
    srv.listen(0, '127.0.0.1', () => {
      resolve({ server: srv, port: srv.address().port, url: `http://127.0.0.1:${srv.address().port}/mcp` });
    });
  });
}

async function runTests() {
  const { mlDsa65 } = await mp.loadPqModules();
  const payer = mlDsa65.keygen(crypto.randomBytes(32));
  const payee = mlDsa65.keygen(crypto.randomBytes(32));
  const payerAccount = mp.deriveAccountAddress(payer.publicKey);
  const payeeAccount = mp.deriveAccountAddress(payee.publicKey);
  const payeePubkeyHex = Buffer.from(payee.publicKey).toString('hex');

  const CHAIN_ID = 'postfiat-wan-devnet-2';
  const NETWORK = 'postfiat:wan-devnet-2';
  const GENESIS_HASH = 'ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9';
  const REGISTRY_ID = '77'.repeat(48);
  const DOMAIN = {
    schema: nativeChain.OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3,
    chain_id: CHAIN_ID,
    genesis_hash: GENESIS_HASH,
    protocol_version: 2,
    registry_id: REGISTRY_ID,
  };

  // Build 4 committee validators (threshold 3) + 1 outsider key.
  const committeeValidators = [0, 1, 2, 3].map((idx) => {
    const kp = mlDsa65.keygen(crypto.randomBytes(32));
    return {
      id: `validator-${idx}`,
      publicKey: kp.publicKey,
      secretKey: kp.secretKey,
      pubkeyHex: Buffer.from(kp.publicKey).toString('hex'),
    };
  });
  const outsider = mlDsa65.keygen(crypto.randomBytes(32));

  const committeeRegistry = new CommitteeRegistry({
    epoch: 9,
    threshold: 3,
    validators: committeeValidators,
    domain: DOMAIN,
  });

  // -------------------------------------------------------------------------
  // 1. Native Consensus payment_v2 end-to-end MCP 402 -> 200 round trip
  //    Client, facilitator, node, and archive ALL agree on the exact 48B tx_id.
  // -------------------------------------------------------------------------
  const archivedConsensusTxs = new Map();
  const consensusNode = await startMockValidatorRpc(async (method, params) => {
    if (method === 'mempool_submit_signed_payment_v2') {
      const signed = JSON.parse(params.signed_payment_v2_json);
      const check = await nativeChain.verifyNativePaymentV2(signed);
      assert.equal(check.ok, true, `node must verify native SignedPaymentV2: ${check.reason}`);
      archivedConsensusTxs.set(check.txIdHex, { signed, height: 1940 });
      return { result: { tx_id: check.txIdHex } };
    }
    if (method === 'tx') {
      const hit = archivedConsensusTxs.get(params.tx_id);
      if (!hit) return { result: { confirmed: false } };
      return {
        result: {
          tx_id: params.tx_id,
          chain_id: hit.signed.unsigned.chain_id,
          confirmed: true,
          receipt: { tx_id: params.tx_id, accepted: true },
          block: { header: { height: hit.height } },
          transaction_rows: [
            {
              tx_id: params.tx_id,
              transaction_kind: 'payment_v2',
              block_height: hit.height,
              from: hit.signed.unsigned.from,
              to: hit.signed.unsigned.to,
              amount: hit.signed.unsigned.amount,
              memos: hit.signed.unsigned.memos,
            },
          ],
        },
      };
    }
    return { error: { code: -32601, message: 'unknown' } };
  });

  try {
    const consAdapter = createChainSettlementAdapter({
      rpcUrls: [consensusNode.url],
      quorumThreshold: 1,
      pollIntervalMs: 10,
      finalityTimeoutMs: 2000,
    });
    const consCore = createFacilitatorCore({
      chainId: CHAIN_ID,
      network: NETWORK,
      committeeRegistry,
      expectedDomain: DOMAIN,
      requireNativePayloads: true,
      settlementAdapter: consAdapter,
    });
    const consGate = createX402Gate({
      facilitatorCore: consCore,
      payTo: payeeAccount,
      priceAtoms: 5000,
      assetId: 'PFT',
      lane: mp.PaymentLane.CONSENSUS_PFT,
      scheme: mp.X402Scheme.EXACT,
    });
    const mcpSrv = await startResourceServer(consGate);
    try {
      let seq = 10;
      const httpClient = new X402HttpClient({
        chainId: CHAIN_ID,
        payerAccount,
        payerPublicKey: payer.publicKey,
        payerSecretKey: payer.secretKey,
        maxBudgetAtoms: 100_000,
        allowedNetworks: [NETWORK],
        allowedAssets: ['PFT'],
        nextConsensusSequence: () => ++seq,
        nativePaymentV2: { genesisHash: GENESIS_HASH, protocolVersion: 2, feeAtoms: 22 },
      });
      const mcp = new X402McpClient({ endpointUrl: mcpSrv.url, httpClient });
      const res = await mcp.callTool(
        'quote',
        { pair: 'PFT/USDC' },
        { expectedPayment: { amountAtoms: 5000, assetId: 'PFT', payTo: payeeAccount } }
      );
      assert.equal(res.result.ok, true);
      assert.equal(res.paymentReceipt.settled, true);
      const receiptTxIdHex = res.paymentReceipt.payment_ref.tx_hash.toString('hex');
      assert.equal(receiptTxIdHex.length, 96);
      assert.equal(archivedConsensusTxs.has(receiptTxIdHex), true, 'archive tx_id must equal client/facilitator/response tx_id');
      assert.equal(res.paymentReceipt.payment_ref.finalized_height, 1940);
    } finally {
      mcpSrv.server.close();
    }
  } finally {
    consensusNode.server.close();
  }

  // -------------------------------------------------------------------------
  // 2. Native FastPay OwnedTransferOrderV3 + Active-Committee ML-DSA-65 Quorum
  //    + Authenticated FastPayApplyAckV1 + Indexed Order Lookup
  // -------------------------------------------------------------------------
  let forgeMode = 'none'; // 'none' | 'bad_vote_sig' | 'duplicate_id' | 'bad_ack_sig'
  const archivedFastPayOrders = new Map();
  const fpServers = [];
  for (let i = 0; i < 4; i += 1) {
    const v = committeeValidators[i];
    fpServers.push(
      await startMockValidatorRpc(async (method, params) => {
        if (method === 'owned_sign_v3') {
          if (i === 3) return { error: { code: -32000, message: 'validator lagging' } };
          const signedOrder = JSON.parse(params.order_json);
          const ownerOk = await nativeChain.verifyNativeOwnedTransferOrderV3(signedOrder);
          assert.equal(ownerOk.ok, true, 'validator must verify native owner signature');
          if (forgeMode === 'bad_vote_sig' && i === 0) {
            return { result: await nativeChain.signNativeOwnedTransferVoteV3(signedOrder.order, { validatorId: v.id, secretKey: outsider.secretKey }) };
          }
          if (forgeMode === 'duplicate_id' && i === 1) {
            return { result: await nativeChain.signNativeOwnedTransferVoteV3(signedOrder.order, { validatorId: 'validator-0', secretKey: committeeValidators[0].secretKey }) };
          }
          return { result: await nativeChain.signNativeOwnedTransferVoteV3(signedOrder.order, { validatorId: v.id, secretKey: v.secretKey }) };
        }
        if (method === 'owned_apply_v3') {
          assert.equal(params.validator_id, v.id, 'owned_apply_v3 must pass validator_id');
          const cert = JSON.parse(params.cert_json);
          const certCheck = await nativeChain.verifyNativeOwnedTransferCertificateV3(cert, {
            committee: committeeRegistry.getActiveCommittee(),
            expectedDomain: DOMAIN,
          });
          assert.equal(certCheck.ok, true, `validator must verify assembled OwnedTransferCertificateV3: ${certCheck.reason}`);
          const orderDigestHex = certCheck.orderHash.toString('hex');
          const certDigestHex = certCheck.certificateDigest.toString('hex');
          archivedFastPayOrders.set(orderDigestHex, { height: 1941, certDigestHex });
          const signingKey = forgeMode === 'bad_ack_sig' ? outsider.secretKey : v.secretKey;
          const ack = await nativeChain.signNativeFastPayApplyAckV1(
            {
              domain: cert.order.domain,
              committee_epoch: cert.order.recovery.committee_epoch,
              lock_id: cert.order.recovery.lock_id,
              order_digest: orderDigestHex,
              certificate_digest: certDigestHex,
              terminal_state_digest: 'ef'.repeat(48),
              validator_id: v.id,
              block_height: 1941,
            },
            signingKey
          );
          return { result: ack };
        }
        if (method === 'tx') {
          const hit = archivedFastPayOrders.get(params.tx_id);
          if (!hit) return { result: { confirmed: false } };
          return {
            result: {
              tx_id: params.tx_id,
              confirmed: true,
              block: { header: { height: hit.height } },
            },
          };
        }
        return { error: { code: -32601, message: 'unknown' } };
      })
    );
  }

  try {
    const rpcUrlList = fpServers.map((s, i) => `validator-${i}=${s.url}`);
    const fpAdapter = createChainSettlementAdapter({
      rpcUrls: rpcUrlList,
      committeeRegistry,
      expectedDomain: DOMAIN,
      requireAuthenticatedCommittee: true,
      verifyIndexedOrder: true,
    });
    assert.equal(fpAdapter.authenticatedCommittee, true);

    const fpCore = createFacilitatorCore({
      chainId: CHAIN_ID,
      network: NETWORK,
      committeeRegistry,
      expectedDomain: DOMAIN,
      requireNativePayloads: true,
      settlementAdapter: fpAdapter,
    });
    const fpGate = createX402Gate({
      facilitatorCore: fpCore,
      payTo: payeeAccount,
      priceAtoms: 7500,
      assetId: 'PFT',
      lane: mp.PaymentLane.FASTPAY,
      scheme: mp.X402Scheme.EXACT,
    });
    const fpMcpSrv = await startResourceServer(fpGate);

    const makeClient = (nonceVal) =>
      new X402McpClient({
        endpointUrl: fpMcpSrv.url,
        httpClient: new X402HttpClient({
          chainId: CHAIN_ID,
          payerAccount,
          payerPublicKey: payer.publicKey,
          payerSecretKey: payer.secretKey,
          maxBudgetAtoms: 500_000,
          allowedNetworks: [NETWORK],
          allowedAssets: ['PFT'],
          nativeFastPay: {
            domain: DOMAIN,
            payeePubkeyHex,
            recovery: {
              committee_epoch: 9,
              valid_from_height: 1900,
              expires_at_height: 2000,
              recovery_closes_at_height: 2100,
            },
            inputs: [{ id: 'aa'.repeat(32), version: 1 }],
            inputTotalAtoms: 50_000,
            nonce: nonceVal,
          },
        }),
      });

    try {
      // Happy path: 3/4 valid ML-DSA-65 committee votes + authenticated FastPayApplyAckV1 + indexed lookup
      const okRes = await makeClient(1).callTool(
        'fastpay_query',
        { q: 'state' },
        { expectedPayment: { amountAtoms: 7500, assetId: 'PFT', payTo: payeeAccount } }
      );
      assert.equal(okRes.result.ok, true);
      const orderHashHex = okRes.paymentReceipt.payment_ref.order_hash.toString('hex');
      const certDigestHex = okRes.paymentReceipt.payment_ref.certificate_digest.toString('hex');
      assert.equal(orderHashHex.length, 96);
      assert.equal(certDigestHex.length, 96);
      assert.equal(archivedFastPayOrders.get(orderHashHex)?.certDigestHex, certDigestHex);

      // Adversarial refusal 1: forged validator vote drops authenticated quorum below 3/3
      forgeMode = 'bad_vote_sig';
      await assert.rejects(
        () => makeClient(2).callTool('fastpay_query', { q: 'bad-sig' }),
        /fastpay_authenticated_quorum_not_reached/
      );

      // Adversarial refusal 2: duplicate / impersonated validator_id vote rejected
      forgeMode = 'duplicate_id';
      await assert.rejects(
        () => makeClient(3).callTool('fastpay_query', { q: 'dup-id' }),
        /fastpay_authenticated_quorum_not_reached/
      );

      // Adversarial refusal 3: forged FastPayApplyAckV1 signature rejected
      forgeMode = 'bad_ack_sig';
      await assert.rejects(
        () => makeClient(4).callTool('fastpay_query', { q: 'bad-ack' }),
        /fastpay_apply_no_authenticated_ack/
      );

      // Adversarial refusal 4: production server refuses URLs alone, but boots with pinned committee + domain!
      const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'x402-prod-native-'));
      try {
        assert.throws(
          () =>
            createFacilitatorFromEnv({
              X402_MODE: 'production',
              X402_SPENT_STORE_PATH: path.join(tmpDir, 'spent.ndjson'),
              X402_RPC_URLS: rpcUrlList.join(','),
            }),
          /X402_COMMITTEE_JSON/
        );
        const prod = createFacilitatorFromEnv({
          X402_MODE: 'production',
          X402_CHAIN_ID: CHAIN_ID,
          X402_NETWORK: NETWORK,
          X402_SPENT_STORE_PATH: path.join(tmpDir, 'spent-prod.ndjson'),
          X402_RPC_URLS: rpcUrlList.join(','),
          X402_DOMAIN_JSON: JSON.stringify(DOMAIN),
          X402_COMMITTEE_JSON: JSON.stringify({
            epoch: 9,
            threshold: 3,
            validators: committeeValidators.map((v) => ({ id: v.id, pubkeyHex: v.pubkeyHex })),
          }),
        });
        assert.equal(prod.mode, 'production');
        assert.equal(prod.core.committeeRegistry.activeEpoch(), 9);
      } finally {
        fs.rmSync(tmpDir, { recursive: true, force: true });
      }
    } finally {
      fpMcpSrv.server.close();
    }
  } finally {
    fpServers.forEach((s) => s.server.close());
  }

  console.log('test_x402_native: all native payment_v2, OwnedTransferOrderV3, committee-quorum, and refusal tests passed');
}

runTests().catch((err) => {
  console.error(err);
  process.exit(1);
});
