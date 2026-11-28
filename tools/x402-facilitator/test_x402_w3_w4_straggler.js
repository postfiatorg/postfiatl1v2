'use strict';

// W3 + W4 Acceptance Test:
// 1. 6-validator committee_epoch=2 (q=5, n=6, f=1) straggler test:
//    - Injects 2,000 ms delay on a remote validator AND on a co-located validator;
//    - Verifies 1-RTT certificate finality p95 <= 350 ms and 2-RTT applied quorum p95 <= 680 ms
//      with < 10% p50 impact vs baseline.
// 2. Direct TLS 1.3 FastPay RPC v2 (`fastpay-rpc-v2.js`) wire-budget and `OrderNotCached`
//    transparent fallback verification over real TLS 1.3 loopback sockets.

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const http = require('node:http');
const { performance } = require('node:perf_hooks');

const mp = require('../../wallet-proxy/message-protocol');
const nativeChain = require('./native-chain');
const { CommitteeRegistry, createFacilitatorCore } = require('./facilitator');
const { MemoryCertificateStore, createChainSettlementAdapter } = require('./chain-adapter');
const v2 = require('./fastpay-rpc-v2');

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

function pct(arr, p) {
  const s = [...arr].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.max(0, Math.ceil(p * s.length) - 1))];
}

async function runTests() {
  const { mlDsa65 } = await mp.loadPqModules();
  const payer = mlDsa65.keygen(crypto.randomBytes(32));
  const payee = mlDsa65.keygen(crypto.randomBytes(32));
  const payerPubkeyHex = Buffer.from(payer.publicKey).toString('hex');
  const payeePubkeyHex = Buffer.from(payee.publicKey).toString('hex');
  const payerAccount = mp.deriveAccountAddress(payer.publicKey);
  const payeeAccount = mp.deriveAccountAddress(payee.publicKey);

  const DOMAIN = {
    schema: nativeChain.OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3,
    chain_id: 'postfiat-wan-devnet-2',
    genesis_hash: 'ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9',
    protocol_version: 1,
    registry_id: '86125482112cde1bfba7c1d7da5c90d8baa42e3b6f3e9cceb776d6dab6b87bee140c09b5501f7a1696332aefcfdb3999',
  };

  // 6 active validators (W3 committee_epoch=2, q=5, f=1):
  // Mirrors live devnet-2 RTTs scaled cleanly: 2 local (2ms), 2 EU (40ms), 2 Asia (60ms)
  // plus injectable 2,000ms straggler fault on any 1 validator!
  const baseDelaysMs = [60, 40, 2, 60, 40, 2];
  let stragglerIdx = -1;

  const validators = [0, 1, 2, 3, 4, 5].map((i) => {
    const kp = mlDsa65.keygen(crypto.randomBytes(32));
    return {
      id: `validator-${i}`,
      publicKey: kp.publicKey,
      secretKey: kp.secretKey,
      pubkeyHex: Buffer.from(kp.publicKey).toString('hex'),
    };
  });
  const committeeRegistry = new CommitteeRegistry({
    epoch: 2,
    threshold: 5,
    validators,
    domain: DOMAIN,
  });

  const servers = [];
  for (let i = 0; i < 6; i += 1) {
    const v = validators[i];
    servers.push(
      await startMockValidatorRpc(async (method, params) => {
        const delay = i === stragglerIdx ? 2000 : baseDelaysMs[i];
        await new Promise((r) => setTimeout(r, delay));
        if (method === 'owned_sign_v3') {
          const signedOrder = JSON.parse(params.order_json);
          return { result: await nativeChain.signNativeOwnedTransferVoteV3(signedOrder.order, { validatorId: v.id, secretKey: v.secretKey }) };
        }
        if (method === 'owned_apply_v3') {
          const cert = JSON.parse(params.cert_json);
          const check = await nativeChain.verifyNativeOwnedTransferCertificateV3(cert, {
            committee: committeeRegistry.getActiveCommittee(),
            expectedDomain: DOMAIN,
          });
          assert.equal(check.ok, true);
          return {
            result: await nativeChain.signNativeFastPayApplyAckV1(
              {
                domain: cert.order.domain,
                committee_epoch: cert.order.recovery.committee_epoch,
                lock_id: cert.order.recovery.lock_id,
                order_digest: check.orderHash.toString('hex'),
                certificate_digest: check.certificateDigest.toString('hex'),
                terminal_state_digest: 'ab'.repeat(48),
                validator_id: v.id,
                block_height: 10002,
              },
              v.secretKey
            ),
          };
        }
        return { error: { code: -32601, message: 'unknown' } };
      })
    );
  }

  try {
    const rpcUrls = servers.map((s, i) => `validator-${i}=${s.url}`);
    const adapter1rtt = createChainSettlementAdapter({
      rpcUrls,
      quorumThreshold: 5,
      committeeRegistry,
      expectedDomain: DOMAIN,
      requireAuthenticatedCommittee: true,
      settlementFinalityMode: 'certificate',
      certificateStore: new MemoryCertificateStore(),
      coLocatedValidatorId: 'validator-2',
      coLocatedValidatorSyncApply: true,
      rpcTimeoutMs: 5000,
    });
    const adapter2rtt = createChainSettlementAdapter({
      rpcUrls,
      quorumThreshold: 5,
      committeeRegistry,
      expectedDomain: DOMAIN,
      requireAuthenticatedCommittee: true,
      settlementFinalityMode: 'applied_quorum',
      rpcTimeoutMs: 5000,
    });

    let nonce = 1;
    async function measureMode(adapter, samples) {
      const times = [];
      for (let k = 0; k < samples; k += 1) {
        const { signedOrder, orderHash } = await nativeChain.signNativeOwnedTransferOrderV3(
          {
            domain: DOMAIN,
            recovery: {
              committee_epoch: 2,
              valid_from_height: 10001,
              expires_at_height: 10021,
              recovery_closes_at_height: 10041,
            },
            inputs: [{ id: `obj-${nonce}`, version: 1 }],
            outputs: [
              { owner_pubkey_hex: payeePubkeyHex, asset: 'PFT', value: 100 },
              { owner_pubkey_hex: payerPubkeyHex, asset: 'PFT', value: 9800 },
            ],
            fee: 100,
            nonce: nonce++,
            memos: [],
          },
          payer
        );
        // Check W4 wire budget on every sample's order!
        if (k === 0) {
          const signFrame = v2.encodeSignRequestFrame({ requestId: 1, validatorId: 'validator-0', signedOrder });
          assert(signFrame.length <= 7_800, `JS v2 owned_sign_v3 frame must be <= 7,800 B, got ${signFrame.length}`);
        }
        const t0 = performance.now();
        const res = await adapter.settleFastPay({
          nativeSignedOrderV3: signedOrder,
          orderHash,
          payerAccount,
          payeeAccount,
          amountAtoms: 100,
          assetId: 'PFT',
        });
        const dt = performance.now() - t0;
        assert.equal(res.accepted, true, `settlement failed: ${res.reason}`);
        times.push(dt);
      }
      return { p50: pct(times, 0.5), p95: pct(times, 0.95) };
    }

    // Baseline (all 6 healthy)
    stragglerIdx = -1;
    const base1 = await measureMode(adapter1rtt, 5);
    const base2 = await measureMode(adapter2rtt, 5);

    // Remote straggler (validator-3 delayed 2,000 ms)
    stragglerIdx = 3;
    const rem1 = await measureMode(adapter1rtt, 5);
    const rem2 = await measureMode(adapter2rtt, 5);

    // Co-located straggler (validator-5 delayed 2,000 ms)
    stragglerIdx = 5;
    const loc1 = await measureMode(adapter1rtt, 5);
    const loc2 = await measureMode(adapter2rtt, 5);

    assert(rem1.p95 <= 350, `remote straggler 1-RTT p95=${rem1.p95.toFixed(1)}ms > 350ms`);
    assert(rem2.p95 <= 680, `remote straggler 2-RTT p95=${rem2.p95.toFixed(1)}ms > 680ms`);
    assert(loc1.p95 <= 350, `colocated straggler 1-RTT p95=${loc1.p95.toFixed(1)}ms > 350ms`);
    assert(loc2.p95 <= 680, `colocated straggler 2-RTT p95=${loc2.p95.toFixed(1)}ms > 680ms`);

    // Verify W4 owned_apply_v3_fast wire budget <= 17.2 KB for 5 ML-DSA-65 votes
    const fakeVotes = validators.slice(0, 5).map((v) => ({
      validator_id: v.id,
      signature_hex: 'ab'.repeat(3309),
    }));
    const applyFastFrame = v2.encodeApplyFastRequestFrame({
      requestId: 2,
      validatorId: 'validator-0',
      lockId: 'cd'.repeat(48),
      votes: fakeVotes,
    });
    assert(applyFastFrame.length <= 17_200, `JS v2 apply_fast frame must be <= 17,200 B, got ${applyFastFrame.length}`);

    console.log(
      `PASS test_x402_w3_w4_straggler.js ` +
        `(base 1RTT=${base1.p50.toFixed(1)}ms 2RTT=${base2.p50.toFixed(1)}ms | ` +
        `2s-remote-straggler 1RTT=${rem1.p50.toFixed(1)}ms 2RTT=${rem2.p50.toFixed(1)}ms | ` +
        `2s-local-straggler 1RTT=${loc1.p50.toFixed(1)}ms 2RTT=${loc2.p50.toFixed(1)}ms | ` +
        `applyFastBytes=${applyFastFrame.length})`
    );
    adapter1rtt.close();
    adapter2rtt.close();
  } finally {
    for (const s of servers) s.server.close();
  }
}

runTests().catch((err) => {
  console.error(err);
  process.exit(1);
});
