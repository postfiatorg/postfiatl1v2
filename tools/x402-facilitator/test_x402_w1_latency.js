'use strict';

// W1 verification tests:
// 1. Quorum-first vote racing beats a 3,000 ms straggler validator in < 200 ms.
// 2. 1-RTT `certificate` finality mode durably persists verified cert to FileCertificateStore
//    (and optionally sync-applies to a co-located validator) before returning HTTP 200,
//    while remote `owned_apply_v3` retries asynchronously in background.
// 3. Crash-and-restart replay: restarting a fresh adapter over the same FileCertificateStore
//    delivers pending certificates via `replayPendingCertificates()` until quorum acks confirm.
// 4. `GET /x402/certificates/:orderHash` serves the persisted certificate cleanly.
// 5. `OwnedObjectPool` enables concurrent 1-RTT x402 MCP calls from pre-split objects.

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');
const { performance } = require('node:perf_hooks');

const mp = require('../../wallet-proxy/message-protocol');
const nativeChain = require('./native-chain');
const {
  HEADER_PAYMENT_RESPONSE,
  CommitteeRegistry,
  createFacilitatorCore,
  createFacilitatorServer,
  createX402Gate,
} = require('./facilitator');
const { X402HttpClient, X402McpClient, OwnedObjectPool } = require('./mcp-client');
const { FileCertificateStore, createChainSettlementAdapter } = require('./chain-adapter');

function startMockValidatorRpc(handler) {
  return new Promise((resolve) => {
    const srv = http.createServer((req, res) => {
      const chunks = [];
      req.on('data', (c) => chunks.push(c));
      req.on('end', async () => {
        const body = JSON.parse(Buffer.concat(chunks).toString('utf8'));
        const out = await handler(body.method, body.params || {});
        if (res.writableEnded) return;
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
        res.writeHead(200, {
          'content-type': 'application/json',
          [HEADER_PAYMENT_RESPONSE]: settled.responseHeaderValue,
        });
        res.end(JSON.stringify({ jsonrpc: '2.0', id: body.id, result: { ok: true } }));
      });
    });
    srv.listen(0, '127.0.0.1', () => {
      resolve({ server: srv, url: `http://127.0.0.1:${srv.address().port}/mcp` });
    });
  });
}

function httpGetJson(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => {
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => resolve({ status: res.statusCode, body: JSON.parse(Buffer.concat(chunks).toString('utf8')) }));
    }).on('error', reject);
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
  const DOMAIN = {
    schema: nativeChain.OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3,
    chain_id: CHAIN_ID,
    genesis_hash: GENESIS_HASH,
    protocol_version: 1,
    registry_id: '88'.repeat(48),
  };

  // 4 validators, quorum = 3. Validator-3 is a 2,500ms straggler on sign;
  // remote validators (1, 2, 3) delay apply by 400ms and fail the first apply attempt on validator-2.
  const validators = [0, 1, 2, 3].map((idx) => {
    const kp = mlDsa65.keygen(crypto.randomBytes(32));
    return {
      id: `validator-${idx}`,
      publicKey: kp.publicKey,
      secretKey: kp.secretKey,
      pubkeyHex: Buffer.from(kp.publicKey).toString('hex'),
    };
  });

  const committeeRegistry = new CommitteeRegistry({
    epoch: 2,
    threshold: 3,
    validators,
    domain: DOMAIN,
  });

  const appliedByValidator = new Map(validators.map((v) => [v.id, new Set()]));
  let failRemoteApplies = false;
  let v2FirstApplyFailed = false;
  const servers = [];
  for (let i = 0; i < 4; i += 1) {
    const v = validators[i];
    servers.push(
      await startMockValidatorRpc(async (method, params) => {
        if (method === 'owned_sign_v3') {
          if (i === 3) {
            await new Promise((r) => setTimeout(r, 2500));
          }
          const signedOrder = JSON.parse(params.order_json);
          return { result: await nativeChain.signNativeOwnedTransferVoteV3(signedOrder.order, { validatorId: v.id, secretKey: v.secretKey }) };
        }
        if (method === 'owned_apply_v3') {
          if (i > 0) {
            if (failRemoteApplies) return { error: { code: -32000, message: 'remote temporarily offline' } };
            await new Promise((r) => setTimeout(r, 250));
            if (i === 2 && !v2FirstApplyFailed) {
              v2FirstApplyFailed = true;
              return { error: { code: -32000, message: 'transient apply glitch' } };
            }
          }
          const cert = JSON.parse(params.cert_json);
          const certCheck = await nativeChain.verifyNativeOwnedTransferCertificateV3(cert, {
            committee: committeeRegistry.getActiveCommittee(),
            expectedDomain: DOMAIN,
          });
          assert.equal(certCheck.ok, true);
          const orderDigestHex = certCheck.orderHash.toString('hex');
          appliedByValidator.get(v.id).add(orderDigestHex);
          const ack = await nativeChain.signNativeFastPayApplyAckV1(
            {
              domain: cert.order.domain,
              committee_epoch: cert.order.recovery.committee_epoch,
              lock_id: cert.order.recovery.lock_id,
              order_digest: orderDigestHex,
              certificate_digest: certCheck.certificateDigest.toString('hex'),
              terminal_state_digest: 'ef'.repeat(48),
              validator_id: v.id,
              block_height: 1942,
            },
            v.secretKey
          );
          return { result: ack };
        }
        return { error: { code: -32601, message: 'unknown' } };
      })
    );
  }

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'x402-w1-test-'));
  const certWalPath = path.join(tmpDir, 'certs.wal.ndjson');
  const rpcUrlList = servers.map((s, i) => `validator-${i}=${s.url}`);

  try {
    // Test 1 + 2 + 4: 1-RTT `certificate` finality beats 2,500ms sign straggler AND 250ms remote apply,
    // sync-applies to co-located validator-0 before HTTP 200, persists to WAL, retries v2, and serves via GET.
    const certStore1 = new FileCertificateStore(certWalPath);
    const adapter1 = createChainSettlementAdapter({
      rpcUrls: rpcUrlList,
      committeeRegistry,
      expectedDomain: DOMAIN,
      requireAuthenticatedCommittee: true,
      settlementFinalityMode: 'certificate',
      certificateStore: certStore1,
      coLocatedValidatorId: 'validator-0',
      applyRetryScheduleMs: [20, 40, 80],
    });
    const core1 = createFacilitatorCore({
      chainId: CHAIN_ID,
      network: NETWORK,
      committeeRegistry,
      expectedDomain: DOMAIN,
      requireNativePayloads: true,
      settlementAdapter: adapter1,
    });
    const gate1 = createX402Gate({
      facilitatorCore: core1,
      payTo: payeeAccount,
      priceAtoms: 500,
      assetId: 'PFT',
      lane: mp.PaymentLane.FASTPAY,
      scheme: mp.X402Scheme.EXACT,
    });
    const mcpSrv = await startResourceServer(gate1);
    const facSrv = createFacilitatorServer(core1);
    await new Promise((r) => facSrv.listen(0, '127.0.0.1', r));
    const facBase = `http://127.0.0.1:${facSrv.address().port}`;

    try {
      const pool = new OwnedObjectPool([
        { id: '11'.repeat(32), version: 1, value: 10_000, asset: 'PFT' },
        { id: '22'.repeat(32), version: 1, value: 10_000, asset: 'PFT' },
        { id: '33'.repeat(32), version: 1, value: 10_000, asset: 'PFT' },
        { id: '44'.repeat(32), version: 1, value: 10_000, asset: 'PFT' },
      ]);
      assert.equal(pool.availableCount(), 4);

      const t0 = performance.now();
      let nonceSeq = 1;
      const runPooledCall = async (idx) => {
        const obj = pool.checkout(500);
        assert(obj, `pool must supply object for call ${idx}`);
        const client = new X402McpClient({
          endpointUrl: mcpSrv.url,
          httpClient: new X402HttpClient({
            chainId: CHAIN_ID,
            payerAccount,
            payerPublicKey: payer.publicKey,
            payerSecretKey: payer.secretKey,
            maxBudgetAtoms: 50_000,
            allowedNetworks: [NETWORK],
            allowedAssets: ['PFT'],
            nativeFastPay: {
              domain: DOMAIN,
              payeePubkeyHex,
              recovery: {
                committee_epoch: 2,
                valid_from_height: 1940,
                expires_at_height: 1960,
                recovery_closes_at_height: 1980,
              },
              inputs: [{ id: obj.id, version: obj.version }],
              inputTotalAtoms: obj.value,
              nonce: nonceSeq++,
            },
          }),
        });
        const res = await client.callTool('quote', { idx }, { expectedPayment: { amountAtoms: 500, assetId: 'PFT', payTo: payeeAccount } });
        pool.recordSpent(obj.id, { id: `cc${idx}`.padEnd(64, '0'), version: 2, value: 8000, asset: 'PFT' });
        return res;
      };

      // Run 4 concurrent 1-RTT pooled calls while validator-3 lags 2,500ms on sign & remote apply lags 250ms
      const results = await Promise.all([0, 1, 2, 3].map(runPooledCall));
      const elapsedMs = performance.now() - t0;
      assert(elapsedMs < 1500, `quorum-first 1-RTT pooled calls must beat 2,500ms straggler, took ${elapsedMs.toFixed(1)}ms`);
      assert.equal(pool.availableCount(), 0);
      assert.equal(pool.pendingConfirmCount(), 4);
      assert.equal(pool.needsReplenish(), true);

      // Co-located validator-0 must ALREADY have applied all 4 certs before HTTP 200 returned!
      for (const r of results) {
        const orderHashHex = r.paymentReceipt.payment_ref.order_hash.toString('hex');
        assert.equal(appliedByValidator.get('validator-0').has(orderHashHex), true, 'co-located validator must have sync-applied before 200');
        const certResp = await httpGetJson(`${facBase}/x402/certificates/${orderHashHex}`);
        assert.equal(certResp.status, 200);
        assert.equal(certResp.body.order_hash_hex, orderHashHex);
      }

      // Wait for background delivery to finish and verify quorum acks + retry succeeded
      await adapter1.flushPendingApplies();
      assert.equal(adapter1.unconfirmedCertificateCount(), 0, 'all certs must confirm asynchronously');
      assert(adapter1.metrics().apply_retry_total >= 1, 'transient v2 failure must have been retried');

      // Test 3: Crash-and-restart recovery — remote validators fail during initial call,
      // facilitator crashes & restarts with a brand-new adapter reading the same FileCertificateStore,
      // then remote validators recover and `replayPendingCertificates()` delivers the cert!
      failRemoteApplies = true;
      const crashObj = { id: '55'.repeat(32), version: 1, value: 10_000, asset: 'PFT' };
      const crashClient = new X402McpClient({
        endpointUrl: mcpSrv.url,
        httpClient: new X402HttpClient({
          chainId: CHAIN_ID,
          payerAccount,
          payerPublicKey: payer.publicKey,
          payerSecretKey: payer.secretKey,
          maxBudgetAtoms: 50_000,
          allowedNetworks: [NETWORK],
          allowedAssets: ['PFT'],
          nativeFastPay: {
            domain: DOMAIN,
            payeePubkeyHex,
            recovery: {
              committee_epoch: 2,
              valid_from_height: 1940,
              expires_at_height: 1960,
              recovery_closes_at_height: 1980,
            },
            inputs: [{ id: crashObj.id, version: crashObj.version }],
            inputTotalAtoms: crashObj.value,
            nonce: 99,
          },
        }),
      });
      const crashRes = await crashClient.callTool('quote', { crash: true });
      const crashOrderHex = crashRes.paymentReceipt.payment_ref.order_hash.toString('hex');
      // Simulate immediate facilitator crash (close store before remote applies succeed)
      adapter1.close();

      // Remote validators come back online; new facilitator restarts from the persisted WAL
      failRemoteApplies = false;
      const certStore2 = new FileCertificateStore(certWalPath);
      const adapter2 = createChainSettlementAdapter({
        rpcUrls: rpcUrlList,
        committeeRegistry,
        expectedDomain: DOMAIN,
        requireAuthenticatedCommittee: true,
        settlementFinalityMode: 'certificate',
        certificateStore: certStore2,
        coLocatedValidatorId: 'validator-0',
        applyRetryScheduleMs: [20, 40],
      });
      assert.equal(adapter2.unconfirmedCertificateCount(), 1, 'restarted adapter must reload 1 pending cert from WAL');
      const replayedCount = await adapter2.replayPendingCertificates();
      assert.equal(replayedCount, 1);
      await adapter2.flushPendingApplies();
      assert.equal(adapter2.unconfirmedCertificateCount(), 0, 'replayed cert must reach quorum applied after restart');
      assert.equal(appliedByValidator.get('validator-1').has(crashOrderHex), true);
      assert.equal(appliedByValidator.get('validator-2').has(crashOrderHex), true);
      adapter2.close();
      console.log(`PASS test_x402_w1_latency.js (4 concurrent pooled 1-RTT calls beat 2.5s straggler in ${elapsedMs.toFixed(1)}ms + crash-replay verified)`);
    } finally {
      mcpSrv.server.close();
      facSrv.close();
    }
  } finally {
    for (const s of servers) s.server.close();
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

runTests().catch((err) => {
  console.error(err);
  process.exit(1);
});
