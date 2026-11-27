'use strict';

// What the x402 gate, its stores, the paying client and the daemon must refuse.
// Every case builds its own facilitator, so the cases do not share state.

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const http = require('node:http');

const mp = require('../../wallet-proxy/message-protocol');
const {
  HEADER_PAYMENT_REQUIRED,
  HEADER_PAYMENT_SIGNATURE,
  CommitteeRegistry,
  MemorySpentStore,
  X402Error,
  buildX402InlineMemo,
  computeHttpRequestHash,
  createFacilitatorCore,
  createFacilitatorServer,
  createX402Gate,
  signConsensusTransfer,
  signFastPayOrder,
} = require('./facilitator');

const CHAIN_ID = 'postfiat-devnet-2';
const NETWORK = 'postfiat:devnet-2';
const PAYEE = 'pf1payee0000000000000000000000000000000000';
const PAYER = 'pf1payer0000000000000000000000000000000000';
const OTHER_ACCOUNT = 'pf1other0000000000000000000000000000000000';
const PFUSDC_ASSET_ID = 'aa'.repeat(48);
const JSON_CONTENT_TYPE = 'application/json';
const START_CLOCK_MS = 1764202000000;
const FASTPAY_FEE_ATOMS = 2_000;
const CONSENSUS_FEE_ATOMS = 25;
const SLOW_SETTLEMENT_MS = 150;

const CHEAP_TERMS = Object.freeze({
  payTo: PAYEE,
  priceAtoms: 50_000,
  assetId: 'PFT',
  lane: mp.PaymentLane.FASTPAY,
  scheme: mp.X402Scheme.EXACT,
});
const EXPENSIVE_TERMS = Object.freeze({
  payTo: PAYEE,
  priceAtoms: 250_000,
  assetId: PFUSDC_ASSET_ID,
  lane: mp.PaymentLane.CONSENSUS_ISSUED,
  scheme: mp.X402Scheme.EXACT,
});
const GATE_TERMS_BY_PATH = Object.freeze({ '/cheap': CHEAP_TERMS, '/expensive': EXPENSIVE_TERMS });

function listen(server) {
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => resolve(server.address().port));
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

function post(port, urlPath, { headers = {}, body = '' } = {}) {
  return new Promise((resolve, reject) => {
    const raw = Buffer.from(body, 'utf8');
    const req = http.request(
      {
        hostname: '127.0.0.1',
        port,
        path: urlPath,
        method: 'POST',
        headers: { 'content-type': JSON_CONTENT_TYPE, 'content-length': raw.length, ...headers },
      },
      (res) => {
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () => {
          resolve({
            statusCode: res.statusCode,
            headers: res.headers,
            body: JSON.parse(Buffer.concat(chunks).toString('utf8')),
          });
        });
      }
    );
    req.on('error', reject);
    req.end(raw);
  });
}

function requestHashFor(urlPath, body) {
  return computeHttpRequestHash({ method: 'POST', urlPath, contentType: JSON_CONTENT_TYPE, body });
}

// One facilitator core, its public HTTP server, and a resource server with one gate per path.
async function startStack({ settlementAdapter = null, coreOptions = {} } = {}) {
  const clock = { nowMs: START_CLOCK_MS };
  const backendCalls = { fastpay: 0, consensus: 0 };
  const adapter = settlementAdapter || {
    async settleFastPay() {
      backendCalls.fastpay += 1;
      return { accepted: true, certificateDigest: Buffer.alloc(32, 1), finalizedHeight: 0 };
    },
    async settleConsensus() {
      backendCalls.consensus += 1;
      return { accepted: true, finalizedHeight: 50_123 };
    },
  };
  const core = createFacilitatorCore({
    chainId: CHAIN_ID,
    network: NETWORK,
    spentStore: new MemorySpentStore(),
    committeeRegistry: new CommitteeRegistry(),
    settlementAdapter: adapter,
    ipRateLimit: 1_000,
    keyRateLimit: 1_000,
    now: () => clock.nowMs,
    ...coreOptions,
  });
  const gates = {};
  for (const [urlPath, terms] of Object.entries(GATE_TERMS_BY_PATH)) {
    gates[urlPath] = createX402Gate({ facilitatorCore: core, ...terms });
  }
  const servedBodies = [];
  const resourceServer = http.createServer({ maxHeaderSize: 65_536 }, async (req, res) => {
    const rawBody = await readRawBody(req);
    const settled = await gates[req.url](req, res, rawBody);
    if (!settled) return;
    servedBodies.push(rawBody.toString('utf8'));
    res.writeHead(200, { 'content-type': JSON_CONTENT_TYPE });
    res.end(JSON.stringify({ ok: true, served: req.url }));
  });
  const facilitatorServer = createFacilitatorServer(core);
  const resourcePort = await listen(resourceServer);
  const facilitatorPort = await listen(facilitatorServer);
  return {
    core,
    clock,
    backendCalls,
    servedBodies,
    resourcePort,
    facilitatorPort,
    async close() {
      await closeServer(resourceServer);
      await closeServer(facilitatorServer);
    },
  };
}

async function challengeAt(stack, urlPath, body) {
  const res = await post(stack.resourcePort, urlPath, { body });
  assert.equal(res.statusCode, 402);
  return mp.decodeX402PaymentRequired(mp.decodeX402Header(String(res.headers[HEADER_PAYMENT_REQUIRED])));
}

function presentAt(stack, urlPath, body, paymentHeader) {
  return post(stack.resourcePort, urlPath, { body, headers: { [HEADER_PAYMENT_SIGNATURE]: paymentHeader } });
}

function wrapPayment(required, requestHash, fields) {
  return mp.encodeX402Header(
    mp.encodeX402PaymentSignature({
      x402_version: 2,
      scheme: required.scheme,
      network: required.network,
      lane: required.lane,
      request_hash: requestHash,
      nonce: required.nonce,
      ...fields,
    })
  );
}

// A payment for `required`, signed by `payerKeys`. `changes` overrides fields of the signed transfer.
async function payFor(required, requestHash, payerKeys, changes = {}) {
  const transfer = {
    chain_id: CHAIN_ID,
    payer_account: PAYER,
    payee_account: required.pay_to,
    asset_id: required.asset_id,
    amount_atoms: required.max_amount_atoms,
    nonce_hex: required.nonce.toString('hex'),
    memos: [buildX402InlineMemo(requestHash)],
  };
  if (required.lane === mp.PaymentLane.FASTPAY) {
    const order = { ...transfer, fee_atoms: FASTPAY_FEE_ATOMS, input_count: 1, output_count: 2, ...changes };
    const signed = await signFastPayOrder(order, payerKeys);
    return wrapPayment(required, requestHash, { order_hash: signed.orderHash, signed_payload: signed.signedPayload });
  }
  const tx = { ...transfer, lane: required.lane, fee_atoms: CONSENSUS_FEE_ATOMS, sequence: 1, ...changes };
  const signed = await signConsensusTransfer(tx, payerKeys);
  return wrapPayment(required, requestHash, { tx_hash: signed.txHash, signed_payload: signed.signedPayload });
}

function assertRefused(res, statusCode, code) {
  assert.equal(res.statusCode, statusCode, JSON.stringify(res.body));
  assert.equal(res.body.code, code);
}

function assertNothingSettled(stack) {
  assert.equal(stack.servedBodies.length, 0, 'no resource may be served');
  assert.deepEqual(stack.backendCalls, { fastpay: 0, consensus: 0 }, 'the settlement backend may not be called');
  assert.equal(stack.core.spentStore.size(), 0, 'no payment may be marked used');
}

// ---------------------------------------------------------------------------
// The gate: its own terms, its own challenges, the request in front of it
// ---------------------------------------------------------------------------

async function gateServesItsOwnChallengeOnce({ payerKeys }) {
  const stack = await startStack();
  try {
    const body = '{"prompt":"control"}';
    const required = await challengeAt(stack, '/cheap', body);
    const payment = await payFor(required, requestHashFor('/cheap', body), payerKeys);

    const first = await presentAt(stack, '/cheap', body, payment);
    assert.equal(first.statusCode, 200, JSON.stringify(first.body));
    assert.deepEqual(stack.servedBodies, [body]);

    // The challenge is gone once it was used, so the same header finds nothing to answer.
    const second = await presentAt(stack, '/cheap', body, payment);
    assertRefused(second, 400, 'unknown_challenge_nonce');
    assert.equal(stack.servedBodies.length, 1);
    assert.equal(stack.backendCalls.fastpay, 1);
  } finally {
    await stack.close();
  }
}

// A payment of 50,000 for the cheap gate must not open the gate priced 250,000.
async function gateRefusesPaymentMadeForAnotherGate({ payerKeys }) {
  const stack = await startStack();
  try {
    const body = '{"prompt":"cross-gate"}';
    const required = await challengeAt(stack, '/cheap', body);
    const payment = await payFor(required, requestHashFor('/cheap', body), payerKeys);

    const res = await presentAt(stack, '/expensive', body, payment);
    assertRefused(res, 400, 'unknown_challenge_nonce');
    assertNothingSettled(stack);
  } finally {
    await stack.close();
  }
}

// A payment bound to request body A must not serve request body B.
async function gateRefusesPaymentMadeForAnotherRequest({ payerKeys }) {
  const stack = await startStack();
  try {
    const paidBody = '{"prompt":"A"}';
    const otherBody = '{"prompt":"a completely different and longer request B"}';
    const required = await challengeAt(stack, '/cheap', paidBody);
    const payment = await payFor(required, requestHashFor('/cheap', paidBody), payerKeys);

    const res = await presentAt(stack, '/cheap', otherBody, payment);
    assertRefused(res, 400, 'request_hash_mismatch');
    assertNothingSettled(stack);
  } finally {
    await stack.close();
  }
}

// A 1-atom challenge payable to the caller, taken from the open POST /v1/challenge, must not
// open the gate priced 250,000.
async function gateRefusesChallengeFromThePublicEndpoint({ payerKeys }) {
  const stack = await startStack();
  try {
    const body = '{"prompt":"free ride"}';
    const requestHash = requestHashFor('/expensive', body);
    const issued = await post(stack.facilitatorPort, '/v1/challenge', {
      body: JSON.stringify({ max_amount_atoms: 1, pay_to: OTHER_ACCOUNT, request_hash_hex: requestHash.toString('hex') }),
    });
    assert.equal(issued.statusCode, 200);
    const required = mp.decodeX402PaymentRequired(mp.decodeX402Header(issued.body.payment_required_header));
    const payment = await payFor(required, requestHash, payerKeys, { payer_account: OTHER_ACCOUNT });

    const res = await presentAt(stack, '/expensive', body, payment);
    assertRefused(res, 400, 'unknown_challenge_nonce');
    assertNothingSettled(stack);
  } finally {
    await stack.close();
  }
}

// The terms are compared even when the challenge is handed in directly, so the refusals above
// do not rest only on where a challenge is stored.
async function settlementRefusesAnyDifferenceFromTheGateTerms({ payerKeys }) {
  const stack = await startStack();
  try {
    const requestHash = requestHashFor('/cheap', '{"prompt":"terms"}');
    const gateTerms = { ...CHEAP_TERMS, requestHash };
    const differences = {
      price: { priceAtoms: CHEAP_TERMS.priceAtoms + 1 },
      asset: { assetId: PFUSDC_ASSET_ID },
      recipient: { payTo: OTHER_ACCOUNT },
      lane: { lane: mp.PaymentLane.CONSENSUS_PFT },
      scheme: { scheme: mp.X402Scheme.PF_FASTPAY_SETTLED },
    };
    for (const [name, change] of Object.entries(differences)) {
      const { required } = stack.core.issueChallenge({ ...CHEAP_TERMS, maxAmountAtoms: CHEAP_TERMS.priceAtoms, requestHash });
      const payment = await payFor(required, requestHash, payerKeys);
      await assert.rejects(
        () => stack.core.settlePayment(payment, { expectedRequired: required, expectedTerms: { ...gateTerms, ...change } }),
        (err) => err instanceof X402Error && err.statusCode === 400 && err.code === 'gate_terms_mismatch',
        `a gate with another ${name} must refuse`
      );
    }
    const otherRequest = requestHashFor('/cheap', '{"prompt":"another request"}');
    const { required } = stack.core.issueChallenge({ ...CHEAP_TERMS, maxAmountAtoms: CHEAP_TERMS.priceAtoms, requestHash });
    const payment = await payFor(required, requestHash, payerKeys);
    await assert.rejects(
      () => stack.core.settlePayment(payment, { expectedRequired: required, expectedTerms: { ...gateTerms, requestHash: otherRequest } }),
      (err) => err instanceof X402Error && err.code === 'request_hash_mismatch'
    );
    assertNothingSettled(stack);

    const settled = await stack.core.settlePayment(payment, { expectedRequired: required, expectedTerms: gateTerms });
    assert.equal(settled.response.settled, true);
    assert.equal(Buffer.compare(settled.requestHash, requestHash), 0);
  } finally {
    await stack.close();
  }
}

// The scheme is named "exact": a gate takes its price, not more.
async function gateRefusesAnotherAmountThanItsPrice({ payerKeys }) {
  const stack = await startStack();
  try {
    const body = '{"prompt":"overpay"}';
    const required = await challengeAt(stack, '/cheap', body);
    const requestHash = requestHashFor('/cheap', body);

    const overpaid = await payFor(required, requestHash, payerKeys, { amount_atoms: CHEAP_TERMS.priceAtoms * 10 });
    assertRefused(await presentAt(stack, '/cheap', body, overpaid), 400, 'payment_amount_mismatch');
    const underpaid = await payFor(required, requestHash, payerKeys, { amount_atoms: CHEAP_TERMS.priceAtoms - 1 });
    assertRefused(await presentAt(stack, '/cheap', body, underpaid), 402, 'insufficient_payment_amount');
    assertNothingSettled(stack);
  } finally {
    await stack.close();
  }
}

// Nothing in the PAYMENT-SIGNATURE wrapper is signed, so an order has to name its challenge itself.
async function gateRefusesOrderSignedForAnotherChallenge({ payerKeys }) {
  const stack = await startStack();
  try {
    const body = '{"prompt":"rewrap"}';
    const requestHash = requestHashFor('/cheap', body);
    const firstChallenge = await challengeAt(stack, '/cheap', body);
    const secondChallenge = await challengeAt(stack, '/cheap', body);

    const orderForFirst = { nonce_hex: firstChallenge.nonce.toString('hex') };
    const rewrapped = await payFor(secondChallenge, requestHash, payerKeys, orderForFirst);
    assertRefused(await presentAt(stack, '/cheap', body, rewrapped), 400, 'order_nonce_mismatch');
    assertNothingSettled(stack);
  } finally {
    await stack.close();
  }
}

async function gateChecksConsensusLanePaymentsTheSameWay({ payerKeys }) {
  const stack = await startStack();
  try {
    const body = '{"prompt":"consensus lane"}';
    const requestHash = requestHashFor('/expensive', body);
    const firstChallenge = await challengeAt(stack, '/expensive', body);
    const secondChallenge = await challengeAt(stack, '/expensive', body);

    const overpaid = await payFor(firstChallenge, requestHash, payerKeys, { amount_atoms: EXPENSIVE_TERMS.priceAtoms + 1 });
    assertRefused(await presentAt(stack, '/expensive', body, overpaid), 400, 'payment_amount_mismatch');
    const txForFirst = { nonce_hex: firstChallenge.nonce.toString('hex') };
    const rewrapped = await payFor(secondChallenge, requestHash, payerKeys, txForFirst);
    assertRefused(await presentAt(stack, '/expensive', body, rewrapped), 400, 'order_nonce_mismatch');
    assertNothingSettled(stack);

    const payment = await payFor(firstChallenge, requestHash, payerKeys);
    const res = await presentAt(stack, '/expensive', body, payment);
    assert.equal(res.statusCode, 200, JSON.stringify(res.body));
    assert.deepEqual(stack.backendCalls, { fastpay: 0, consensus: 1 });
  } finally {
    await stack.close();
  }
}

async function gateSettlesOnePaymentPerChallenge({ payerKeys }) {
  let backendCalls = 0;
  let failNextSettlement = true;
  const stack = await startStack({
    settlementAdapter: {
      async settleFastPay() {
        backendCalls += 1;
        await new Promise((resolve) => setTimeout(resolve, SLOW_SETTLEMENT_MS));
        if (failNextSettlement) {
          failNextSettlement = false;
          return { accepted: false, reason: 'backend not ready' };
        }
        return { accepted: true, certificateDigest: Buffer.alloc(32, 2) };
      },
    },
  });
  try {
    const body = '{"prompt":"race"}';
    const required = await challengeAt(stack, '/cheap', body);
    const payment = await payFor(required, requestHashFor('/cheap', body), payerKeys);

    // A settlement that fails gives the challenge back, so the payer can present the payment again.
    assertRefused(await presentAt(stack, '/cheap', body, payment), 502, 'fastpay_settlement_rejected');
    assert.equal(backendCalls, 1);

    const [a, b] = await Promise.all([presentAt(stack, '/cheap', body, payment), presentAt(stack, '/cheap', body, payment)]);
    assert.deepEqual([a.statusCode, b.statusCode].sort(), [200, 409]);
    assert.equal([a, b].find((res) => res.statusCode === 409).body.code, 'challenge_already_used');
    assert.equal(backendCalls, 2, 'the second of two simultaneous payments must not reach the backend');
    assert.equal(stack.servedBodies.length, 1);
  } finally {
    await stack.close();
  }
}

// The last check in the gate compares what the settled payment was signed for with the request
// the gate is about to serve.
async function gateComparesTheSettledRequestHashWithItsOwn() {
  const settledForAnotherRequest = {
    issueChallenge() {
      throw new Error('not used in this case');
    },
    async settlePayment() {
      return {
        response: { settled: true, payment_ref: { order_hash: Buffer.alloc(32, 7) } },
        responseHeaderValue: '',
        dedupKey: 'fastpay:00',
        requestHash: requestHashFor('/cheap', '{"prompt":"what was paid for"}'),
      };
    },
  };
  const gate = createX402Gate({ facilitatorCore: settledForAnotherRequest, ...CHEAP_TERMS });
  let served = false;
  const server = http.createServer(async (req, res) => {
    const settled = await gate(req, res, await readRawBody(req));
    if (!settled) return;
    served = true;
    res.writeHead(200, { 'content-type': JSON_CONTENT_TYPE });
    res.end('{}');
  });
  const port = await listen(server);
  try {
    const res = await post(port, '/cheap', { body: '{"prompt":"what is served"}', headers: { [HEADER_PAYMENT_SIGNATURE]: 'AA' } });
    assertRefused(res, 400, 'request_hash_mismatch');
    assert.equal(served, false);
  } finally {
    await closeServer(server);
  }
}

const CASES = [
  ['a gate serves a payment for its own challenge once', gateServesItsOwnChallengeOnce],
  ['a gate refuses a payment made for another gate', gateRefusesPaymentMadeForAnotherGate],
  ['a gate refuses a payment made for another request', gateRefusesPaymentMadeForAnotherRequest],
  ['a gate refuses a challenge from the public endpoint', gateRefusesChallengeFromThePublicEndpoint],
  ['settlement refuses any difference from the gate terms', settlementRefusesAnyDifferenceFromTheGateTerms],
  ['a gate refuses another amount than its price', gateRefusesAnotherAmountThanItsPrice],
  ['a gate refuses an order signed for another challenge', gateRefusesOrderSignedForAnotherChallenge],
  ['a gate checks consensus-lane payments the same way', gateChecksConsensusLanePaymentsTheSameWay],
  ['a gate settles one payment per challenge', gateSettlesOnePaymentPerChallenge],
  ['a gate compares the settled request hash with its own', gateComparesTheSettledRequestHashWithItsOwn],
];

async function main() {
  const { mlDsa65 } = await mp.loadPqModules();
  const context = { payerKeys: mlDsa65.keygen(crypto.randomBytes(32)) };
  const failures = [];
  for (const [name, run] of CASES) {
    try {
      await run(context);
      console.log(`ok - ${name}`);
    } catch (err) {
      failures.push(name);
      console.log(`not ok - ${name}\n  ${String(err.message).split('\n')[0]}`);
    }
  }
  if (failures.length > 0) {
    throw new Error(`test_x402_refusals: ${failures.length} of ${CASES.length} cases failed`);
  }
  console.log(`test_x402_refusals: all ${CASES.length} cases passed`);
}

main().catch((err) => {
  console.error(err.message);
  process.exit(1);
});
