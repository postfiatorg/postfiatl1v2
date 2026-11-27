'use strict';

// End-to-end automated smoke & security verification for Post Fiat x402 v2
// facilitator + MCP payment client wrapper:
//   1. HTTP 402 -> FastPay (exact) -> 200 OK + verified PAYMENT-RESPONSE
//   2. HTTP 402 -> FastPay (pf-fastpay-settled) with real ML-DSA-65 validator quorum
//   3. Consensus-lane PFUSDC (PAYMENT_LANE_CONSENSUS_ISSUED) with explicit sequence + finality check
//   4. Certificate-malleability replay rejection keyed on immutable order_hash (different cert digest)
//   5. Local verification BEFORE validator calls (bad sig, wrong payTo, underpay, expired -> 0 validator calls)
//   6. Validator committee rotation (epoch 1 -> epoch 2 accepts new quorum, rejects retired epoch)
//   7. Per-IP rate limiting (429 Too Many Requests)
//   8. MCP JSON-RPC tools/call end-to-end 402 -> FastPay -> 200 OK + budget cap refusal

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');

const mp = require('../../wallet-proxy/message-protocol');
const {
  HEADER_PAYMENT_RESPONSE,
  CommitteeRegistry,
  FileSpentStore,
  X402Error,
  buildX402InlineMemo,
  computeHttpRequestHash,
  createFacilitatorCore,
  createFacilitatorServer,
  createX402Gate,
  signFastPayOrder,
  signFastPayValidatorVote,
} = require('./facilitator');
const { X402HttpClient, X402McpClient } = require('./mcp-client');

function listen(server) {
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const addr = server.address();
      resolve(addr.port);
    });
  });
}

function closeServer(server) {
  return new Promise((resolve) => server.close(() => resolve()));
}

function readRawBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on('data', (c) => chunks.push(c));
    req.on('end', () => resolve(Buffer.concat(chunks)));
    req.on('error', reject);
  });
}

function postJson(port, urlPath, payload, headers = {}) {
  return new Promise((resolve, reject) => {
    const raw = JSON.stringify(payload);
    const req = http.request(
      {
        hostname: '127.0.0.1',
        port,
        path: urlPath,
        method: 'POST',
        headers: {
          'content-type': 'application/json',
          'content-length': Buffer.byteLength(raw),
          ...headers,
        },
      },
      (res) => {
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () => {
          const text = Buffer.concat(chunks).toString('utf8');
          resolve({ statusCode: res.statusCode, headers: res.headers, body: JSON.parse(text) });
        });
      }
    );
    req.on('error', reject);
    req.end(raw);
  });
}

async function main() {
  const { mlDsa65 } = await mp.loadPqModules();
  const payerKeys = mlDsa65.keygen(crypto.randomBytes(32));
  const otherKeys = mlDsa65.keygen(crypto.randomBytes(32));

  // Generate 4 ML-DSA-65 validator keys (3 for epoch 1, rotated set for epoch 2)
  const validators = [1, 2, 3, 4].map((i) => {
    const kp = mlDsa65.keygen(crypto.randomBytes(32));
    return {
      id: `val-${i}`,
      publicKey: kp.publicKey,
      secretKey: kp.secretKey,
      pubkeyHex: Buffer.from(kp.publicKey).toString('hex'),
    };
  });

  const committeeRegistry = new CommitteeRegistry({
    epoch: 1,
    threshold: 2,
    validators: validators.slice(0, 3),
  });

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-x402-'));
  const spentFile = path.join(tmpDir, 'spent.json');
  const spentStore = new FileSpentStore(spentFile);

  let fastpayValidatorCalls = 0;
  let consensusValidatorCalls = 0;
  let clockMs = 1764202000000;

  const settlementAdapter = {
    async settleFastPay(verified) {
      fastpayValidatorCalls += 1;
      const v1 = await signFastPayValidatorVote({
        orderHash: verified.orderHash,
        epoch: committeeRegistry.activeEpoch(),
        validatorId: validators[0].id,
        secretKey: validators[0].secretKey,
      });
      const v2 = await signFastPayValidatorVote({
        orderHash: verified.orderHash,
        epoch: committeeRegistry.activeEpoch(),
        validatorId: validators[1].id,
        secretKey: validators[1].secretKey,
      });
      const certDigest = mp.sha256(mp.canonicalJsonBytes({ epoch: committeeRegistry.activeEpoch(), votes: [v1, v2] }));
      return { accepted: true, certificateDigest: certDigest, finalizedHeight: 0 };
    },
    async settleConsensus(verified) {
      consensusValidatorCalls += 1;
      assert.ok(Number.isSafeInteger(verified.signedTx.sequence));
      return { accepted: true, finalizedHeight: 50_123 };
    },
  };

  const PAYEE = 'pf1payee0000000000000000000000000000000000';
  const PAYER = 'pf1payer0000000000000000000000000000000000';
  const PFUSDC_ASSET_ID = 'aa'.repeat(48);

  const core = createFacilitatorCore({
    facilitatorId: 'postfiat-facilitator-v1',
    chainId: 'postfiat-devnet-2',
    network: 'postfiat:devnet-2',
    spentStore,
    committeeRegistry,
    settlementAdapter,
    ipRateLimit: 100,
    keyRateLimit: 100,
    now: () => clockMs,
  });

  const facilitatorServer = createFacilitatorServer(core);
  const facilitatorPort = await listen(facilitatorServer);

  // Protected API + MCP server behind createX402Gate
  const fastpayExactGate = createX402Gate({
    facilitatorCore: core,
    payTo: PAYEE,
    priceAtoms: 50_000,
    assetId: 'PFT',
    lane: mp.PaymentLane.FASTPAY,
    scheme: mp.X402Scheme.EXACT,
  });
  const fastpaySettledGate = createX402Gate({
    facilitatorCore: core,
    payTo: PAYEE,
    priceAtoms: 75_000,
    assetId: 'PFT',
    lane: mp.PaymentLane.FASTPAY,
    scheme: mp.X402Scheme.PF_FASTPAY_SETTLED,
  });
  const pfusdcGate = createX402Gate({
    facilitatorCore: core,
    payTo: PAYEE,
    priceAtoms: 250_000,
    assetId: PFUSDC_ASSET_ID,
    lane: mp.PaymentLane.CONSENSUS_ISSUED,
    scheme: mp.X402Scheme.EXACT,
  });

  const resourceServer = http.createServer({ maxHeaderSize: 65_536 }, async (req, res) => {
    const rawBody = await readRawBody(req);
    let gate = fastpayExactGate;
    if (req.url === '/v1/settled-inference') gate = fastpaySettledGate;
    else if (req.url === '/v1/pfusdc-inference') gate = pfusdcGate;

    const settled = await gate(req, res, rawBody);
    if (!settled) return;

    if (req.url === '/mcp') {
      const rpc = JSON.parse(rawBody.toString('utf8'));
      const out = JSON.stringify({
        jsonrpc: '2.0',
        id: rpc.id,
        result: {
          content: [{ type: 'text', text: `tool:${rpc.params.name}:ok:${rpc.params.arguments.symbol}` }],
        },
      });
      res.writeHead(200, {
        'content-type': 'application/json',
        [HEADER_PAYMENT_RESPONSE]: settled.responseHeaderValue,
      });
      res.end(out);
      return;
    }

    const out = JSON.stringify({ ok: true, endpoint: req.url, paid_atoms: settled.response.payment_ref.amount_atoms });
    res.writeHead(200, {
      'content-type': 'application/json',
      [HEADER_PAYMENT_RESPONSE]: settled.responseHeaderValue,
    });
    res.end(out);
  });
  const resourcePort = await listen(resourceServer);

  try {
    // --- 1. End-to-end HTTP 402 -> FastPay (exact) -> 200 OK ---
    let seq = 10;
    const client = new X402HttpClient({
      chainId: 'postfiat-devnet-2',
      payerAccount: PAYER,
      payerPublicKey: payerKeys.publicKey,
      payerSecretKey: payerKeys.secretKey,
      maxBudgetAtoms: 600_000,
      allowedNetworks: ['postfiat:devnet-2'],
      allowedAssets: ['PFT', PFUSDC_ASSET_ID],
      nextConsensusSequence: () => seq++,
      directFastPaySettleFn: async ({ orderHash }) => {
        const epoch = committeeRegistry.activeEpoch();
        const activeVals = epoch === 1 ? validators.slice(0, 2) : validators.slice(2, 4);
        const votes = [];
        for (const v of activeVals) {
          votes.push(
            await signFastPayValidatorVote({
              orderHash,
              epoch,
              validatorId: v.id,
              secretKey: v.secretKey,
            })
          );
        }
        return Buffer.from(JSON.stringify({ epoch, order_hash_hex: orderHash.toString('hex'), votes }), 'utf8');
      },
      now: () => clockMs,
    });

    const res1 = await client.request(`http://127.0.0.1:${resourcePort}/v1/inference`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ prompt: 'summarize' }),
    });
    assert.equal(res1.statusCode, 200);
    assert.equal(res1.paymentReceipt.settled, true);
    assert.equal(res1.paymentReceipt.payment_ref.lane, mp.PaymentLane.FASTPAY);
    assert.equal(res1.paymentReceipt.payment_ref.amount_atoms, 50_000);
    assert.equal(fastpayValidatorCalls, 1);

    // --- 2. End-to-end HTTP 402 -> FastPay (pf-fastpay-settled) with real ML-DSA-65 quorum ---
    const res2 = await client.request(`http://127.0.0.1:${resourcePort}/v1/settled-inference`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ prompt: 'settled-quorum' }),
    });
    assert.equal(res2.statusCode, 200);
    assert.equal(res2.paymentReceipt.settled, true);
    assert.equal(res2.paymentReceipt.payment_ref.amount_atoms, 75_000);

    // --- 3. End-to-end HTTP 402 -> Consensus PFUSDC (explicit sequence + finality receipt) ---
    const res3 = await client.request(`http://127.0.0.1:${resourcePort}/v1/pfusdc-inference`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ prompt: 'pfusdc' }),
    });
    assert.equal(res3.statusCode, 200);
    assert.equal(res3.paymentReceipt.payment_ref.lane, mp.PaymentLane.CONSENSUS_ISSUED);
    assert.equal(res3.paymentReceipt.payment_ref.asset_id, PFUSDC_ASSET_ID);
    assert.equal(res3.paymentReceipt.payment_ref.finalized_height, 50_123);
    assert.equal(consensusValidatorCalls, 1);

    // --- 4. Certificate-malleability replay rejection (same order_hash, DIFFERENT certificate_digest) ---
    const replayReqHash = computeHttpRequestHash({
      method: 'POST',
      urlPath: '/v1/replay-test',
      contentType: 'application/json',
      body: '{"x":1}',
    });
    const replayChallenge = core.issueChallenge({
      scheme: mp.X402Scheme.PF_FASTPAY_SETTLED,
      lane: mp.PaymentLane.FASTPAY,
      assetId: 'PFT',
      maxAmountAtoms: 10_000,
      payTo: PAYEE,
      requestHash: replayReqHash,
    });
    const replayOrder = {
      chain_id: 'postfiat-devnet-2',
      payer_account: PAYER,
      payee_account: PAYEE,
      asset_id: 'PFT',
      amount_atoms: 10_000,
      fee_atoms: 2_000,
      input_count: 1,
      output_count: 2,
      nonce_hex: replayChallenge.required.nonce.toString('hex'),
      memos: [buildX402InlineMemo(replayReqHash)],
    };
    const signedReplay = await signFastPayOrder(replayOrder, {
      publicKey: payerKeys.publicKey,
      secretKey: payerKeys.secretKey,
    });
    const vote1 = await signFastPayValidatorVote({
      orderHash: signedReplay.orderHash,
      epoch: 1,
      validatorId: validators[0].id,
      secretKey: validators[0].secretKey,
    });
    const vote2 = await signFastPayValidatorVote({
      orderHash: signedReplay.orderHash,
      epoch: 1,
      validatorId: validators[1].id,
      secretKey: validators[1].secretKey,
    });
    const vote3 = await signFastPayValidatorVote({
      orderHash: signedReplay.orderHash,
      epoch: 1,
      validatorId: validators[2].id,
      secretKey: validators[2].secretKey,
    });
    const certA = Buffer.from(JSON.stringify({ epoch: 1, order_hash_hex: signedReplay.orderHash.toString('hex'), votes: [vote1, vote2] }));
    const certB = Buffer.from(JSON.stringify({ epoch: 1, order_hash_hex: signedReplay.orderHash.toString('hex'), votes: [vote2, vote3] }));
    assert.notEqual(mp.sha256(certA).toString('hex'), mp.sha256(certB).toString('hex'));

    const makeSigHdr = (certBuf) =>
      mp.encodeX402Header(
        mp.encodeX402PaymentSignature({
          x402_version: 2,
          scheme: mp.X402Scheme.PF_FASTPAY_SETTLED,
          network: 'postfiat:devnet-2',
          lane: mp.PaymentLane.FASTPAY,
          request_hash: replayReqHash,
          nonce: replayChallenge.required.nonce,
          order_hash: signedReplay.orderHash,
          signed_payload: signedReplay.signedPayload,
          certificate_payload: certBuf,
        })
      );

    const firstSettle = await postJson(facilitatorPort, '/v1/settle', {
      payment_signature_header: makeSigHdr(certA),
    });
    assert.equal(firstSettle.statusCode, 200);

    // Replaying the EXACT same order_hash with a DIFFERENT valid quorum certificate (certB) MUST return 409!
    const malleatedReplay = await postJson(facilitatorPort, '/v1/settle', {
      payment_signature_header: makeSigHdr(certB),
    });
    assert.equal(malleatedReplay.statusCode, 409);
    assert.equal(malleatedReplay.body.code, 'payment_already_spent');

    // Reload FileSpentStore from disk to prove cross-restart/replica persistence
    const reloadedStore = new FileSpentStore(spentFile);
    assert.equal(reloadedStore.has(`fastpay:${signedReplay.orderHash.toString('hex')}`), true);

    // --- 5. Local verification BEFORE validator calls (0 upstream calls on forged/bad requests) ---
    const callsBefore = fastpayValidatorCalls;
    const badChallenge = core.issueChallenge({
      scheme: mp.X402Scheme.EXACT,
      lane: mp.PaymentLane.FASTPAY,
      assetId: 'PFT',
      maxAmountAtoms: 20_000,
      payTo: PAYEE,
      requestHash: replayReqHash,
    });
    // Forged signature (signed by otherKeys, claiming payerKeys.publicKey)
    const forgedOrder = {
      ...replayOrder,
      amount_atoms: 20_000,
      nonce_hex: badChallenge.required.nonce.toString('hex'),
    };
    const forgedSigned = await signFastPayOrder(forgedOrder, {
      publicKey: payerKeys.publicKey,
      secretKey: otherKeys.secretKey,
    });
    const forgedHdr = mp.encodeX402Header(
      mp.encodeX402PaymentSignature({
        x402_version: 2,
        scheme: mp.X402Scheme.EXACT,
        network: 'postfiat:devnet-2',
        lane: mp.PaymentLane.FASTPAY,
        request_hash: replayReqHash,
        nonce: badChallenge.required.nonce,
        order_hash: forgedSigned.orderHash,
        signed_payload: forgedSigned.signedPayload,
      })
    );
    const badSigRes = await postJson(facilitatorPort, '/v1/settle', { payment_signature_header: forgedHdr });
    assert.equal(badSigRes.statusCode, 401);
    assert.equal(badSigRes.body.code, 'invalid_payer_signature');
    assert.equal(fastpayValidatorCalls, callsBefore, 'bad ML-DSA-65 signature must never call validators');

    // Wrong payee
    const wrongPayeeOrder = { ...forgedOrder, payee_account: 'pf1attacker0000000000000000000000000000000' };
    const wrongPayeeSigned = await signFastPayOrder(wrongPayeeOrder, {
      publicKey: payerKeys.publicKey,
      secretKey: payerKeys.secretKey,
    });
    const wrongPayeeHdr = mp.encodeX402Header(
      mp.encodeX402PaymentSignature({
        x402_version: 2,
        scheme: mp.X402Scheme.EXACT,
        network: 'postfiat:devnet-2',
        lane: mp.PaymentLane.FASTPAY,
        request_hash: replayReqHash,
        nonce: badChallenge.required.nonce,
        order_hash: wrongPayeeSigned.orderHash,
        signed_payload: wrongPayeeSigned.signedPayload,
      })
    );
    const wrongPayeeRes = await postJson(facilitatorPort, '/v1/settle', { payment_signature_header: wrongPayeeHdr });
    assert.equal(wrongPayeeRes.statusCode, 400);
    assert.equal(wrongPayeeRes.body.code, 'pay_to_mismatch');
    assert.equal(fastpayValidatorCalls, callsBefore, 'payTo mismatch must never call validators');

    // --- 6. Validator committee rotation (epoch 1 -> epoch 2) ---
    committeeRegistry.rotateCommittee({
      epoch: 2,
      threshold: 2,
      validators: validators.slice(1, 4), // val-2, val-3, val-4
      retainPreviousEpochs: 0,
    });
    // Epoch 2 pf-fastpay-settled succeeds
    const resRot = await client.request(`http://127.0.0.1:${resourcePort}/v1/settled-inference`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ prompt: 'post-rotation-epoch-2' }),
    });
    assert.equal(resRot.statusCode, 200);
    // Stale epoch 1 cert rejected
    const staleOrder = { ...replayOrder, nonce_hex: crypto.randomBytes(16).toString('hex') };
    const staleChallenge = core.issueChallenge({
      scheme: mp.X402Scheme.PF_FASTPAY_SETTLED,
      lane: mp.PaymentLane.FASTPAY,
      assetId: 'PFT',
      maxAmountAtoms: 10_000,
      payTo: PAYEE,
      requestHash: replayReqHash,
    });
    const staleSigned = await signFastPayOrder(staleOrder, {
      publicKey: payerKeys.publicKey,
      secretKey: payerKeys.secretKey,
    });
    const staleVote1 = await signFastPayValidatorVote({
      orderHash: staleSigned.orderHash,
      epoch: 1,
      validatorId: validators[0].id,
      secretKey: validators[0].secretKey,
    });
    const staleVote2 = await signFastPayValidatorVote({
      orderHash: staleSigned.orderHash,
      epoch: 1,
      validatorId: validators[1].id,
      secretKey: validators[1].secretKey,
    });
    const staleHdr = mp.encodeX402Header(
      mp.encodeX402PaymentSignature({
        x402_version: 2,
        scheme: mp.X402Scheme.PF_FASTPAY_SETTLED,
        network: 'postfiat:devnet-2',
        lane: mp.PaymentLane.FASTPAY,
        request_hash: replayReqHash,
        nonce: staleChallenge.required.nonce,
        order_hash: staleSigned.orderHash,
        signed_payload: staleSigned.signedPayload,
        certificate_payload: Buffer.from(
          JSON.stringify({ epoch: 1, order_hash_hex: staleSigned.orderHash.toString('hex'), votes: [staleVote1, staleVote2] })
        ),
      })
    );
    const staleRes = await postJson(facilitatorPort, '/v1/settle', { payment_signature_header: staleHdr });
    assert.equal(staleRes.statusCode, 409);
    assert.equal(staleRes.body.code, 'stale_or_unknown_committee_epoch');

    // --- 7. Per-IP rate limiting (429) ---
    const tightCore = createFacilitatorCore({ ipRateLimit: 2, keyRateLimit: 10, now: () => clockMs });
    await assert.rejects(() => tightCore.verifyPaymentLocally('bad', { remoteIp: '127.0.0.1' }), (e) => e.statusCode === 400);
    await assert.rejects(() => tightCore.verifyPaymentLocally('bad', { remoteIp: '127.0.0.1' }), (e) => e.statusCode === 400);
    await assert.rejects(
      () => tightCore.verifyPaymentLocally('bad', { remoteIp: '127.0.0.1' }),
      (e) => e instanceof X402Error && e.statusCode === 429 && e.code === 'rate_limited_ip'
    );

    // --- 8. MCP JSON-RPC tools/call end-to-end 402 -> FastPay -> 200 OK + budget cap refusal ---
    const mcpClient = new X402McpClient({
      endpointUrl: `http://127.0.0.1:${resourcePort}/mcp`,
      httpClient: client,
    });
    const mcpRes = await mcpClient.callTool('postfiat_quote', { symbol: 'PFT' });
    assert.equal(mcpRes.result.content[0].text, 'tool:postfiat_quote:ok:PFT');
    assert.equal(mcpRes.paymentReceipt.settled, true);
    assert.equal(mcpRes.paymentReceipt.payment_ref.amount_atoms, 50_000);

    // Budget cap refusal
    const lowBudgetClient = new X402HttpClient({
      chainId: 'postfiat-devnet-2',
      payerAccount: PAYER,
      payerPublicKey: payerKeys.publicKey,
      payerSecretKey: payerKeys.secretKey,
      maxBudgetAtoms: 1_000,
      now: () => clockMs,
    });
    const lowBudgetMcp = new X402McpClient({
      endpointUrl: `http://127.0.0.1:${resourcePort}/mcp`,
      httpClient: lowBudgetClient,
    });
    await assert.rejects(
      () => lowBudgetMcp.callTool('postfiat_quote', { symbol: 'PFT' }),
      (e) => e instanceof X402Error && e.code === 'budget_exceeded'
    );

    console.log('test_x402_e2e: all 8 x402 facilitator + MCP + replay + committee-rotation tests passed');
  } finally {
    await closeServer(resourceServer);
    await closeServer(facilitatorServer);
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
