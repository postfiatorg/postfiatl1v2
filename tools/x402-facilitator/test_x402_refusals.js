'use strict';

// What the x402 gate, its stores, the paying client and the daemon must refuse.
// Every case builds its own facilitator, so the cases do not share state.

const assert = require('node:assert/strict');
const { spawn, spawnSync } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');

const mp = require('../../wallet-proxy/message-protocol');
const {
  HEADER_PAYMENT_REQUIRED,
  HEADER_PAYMENT_SIGNATURE,
  CommitteeRegistry,
  FileSpentStore,
  MemorySpentStore,
  SlidingWindowRateLimiter,
  X402Error,
  buildX402InlineMemo,
  computeHttpRequestHash,
  createFacilitatorCore,
  createFacilitatorServer,
  createX402Gate,
  signConsensusTransfer,
  signFastPayOrder,
} = require('./facilitator');
const { X402HttpClient, X402McpClient } = require('./mcp-client');
const { createFacilitatorFromEnv } = require('./server');

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
const CHALLENGE_TTL_MS = 5 * 60_000;
const FAR_FUTURE_TTL_MS = 10 ** 12;
const FACILITATOR_PATH = path.join(__dirname, 'facilitator.js');
const DAEMON_PATH = path.join(__dirname, 'server.js');
const DAEMON_START_TIMEOUT_MS = 5_000;
const RACING_PROCESSES = 6;
const KEYS_EVERY_PROCESS_WANTS = 40;
const RACE_START_DELAY_MS = 500;

// Run with `node -e`: opens the shared file, waits for the common start time, then tries to mark
// the same keys as the other processes and one key of its own. Prints the shared keys it won.
const RACING_PROCESS_SCRIPT = `
  const [facilitatorPath, file, startAtMs, sharedKeys, name] = process.argv.slice(1);
  const { FileSpentStore } = require(facilitatorPath);
  const store = new FileSpentStore(file);
  while (Date.now() < Number(startAtMs)) {}
  const won = [];
  for (let i = 0; i < Number(sharedKeys); i += 1) {
    if (store.markSpent('fastpay:shared-' + i, { by: name })) won.push(i);
  }
  store.markSpent('fastpay:own-' + name, { by: name });
  process.stdout.write(JSON.stringify(won));
`;

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

function withTempDir(run) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-x402-refusals-'));
  const cleanUp = () => fs.rmSync(dir, { recursive: true, force: true });
  return Promise.resolve()
    .then(() => run(dir))
    .finally(cleanUp);
}

function runNode(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, args, { stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.on('error', reject);
    child.on('close', (code) => {
      if (code === 0) resolve(stdout);
      else reject(new Error(`node exited with ${code}: ${stderr}`));
    });
  });
}

// One facilitator core, its public HTTP server, and a resource server with one gate per path.
// `beforeServe` runs with the settlement result right before the resource is sent.
async function startStack({ settlementAdapter = null, coreOptions = {}, gateOptions = {}, beforeServe = () => {} } = {}) {
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
    gates[urlPath] = createX402Gate({ facilitatorCore: core, ...terms, ...gateOptions });
  }
  const servedBodies = [];
  const resourceServer = http.createServer({ maxHeaderSize: 65_536 }, async (req, res) => {
    const rawBody = await readRawBody(req);
    const settled = await gates[req.url](req, res, rawBody);
    if (!settled) return;
    beforeServe(settled);
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

// ---------------------------------------------------------------------------
// Used payments: across a restart, across processes, on disk before serving
// ---------------------------------------------------------------------------

// A second store on the same file must see the first one's entry and must not mark the same
// payment again, and no entry of either store may be lost.
function usedPaymentIsSeenByEveryStoreOnTheFile() {
  return withTempDir((dir) => {
    const file = path.join(dir, 'spent.jsonl');
    const first = new FileSpentStore(file);
    const second = new FileSpentStore(file);

    assert.equal(first.markSpent('fastpay:X', { n: 1 }), true);
    assert.equal(second.has('fastpay:X'), true);
    assert.equal(second.markSpent('fastpay:X', { n: 2 }), false);
    assert.deepEqual(second.get('fastpay:X'), { n: 1 });

    assert.equal(first.markSpent('fastpay:Y', { n: 3 }), true);
    assert.equal(second.markSpent('fastpay:Z', { n: 4 }), true);
    const afterRestart = new FileSpentStore(file);
    assert.equal(afterRestart.size(), 3);
    for (const key of ['fastpay:X', 'fastpay:Y', 'fastpay:Z']) {
      assert.equal(afterRestart.has(key), true, key);
    }
  });
}

function usedPaymentIsMarkedByOneProcessOnly() {
  return withTempDir(async (dir) => {
    const file = path.join(dir, 'spent.jsonl');
    const openedBeforeTheRace = new FileSpentStore(file);
    const names = Array.from({ length: RACING_PROCESSES }, (_, i) => `p${i}`);
    const startAtMs = String(Date.now() + RACE_START_DELAY_MS);
    const outputs = await Promise.all(
      names.map((name) =>
        runNode(['-e', RACING_PROCESS_SCRIPT, FACILITATOR_PATH, file, startAtMs, String(KEYS_EVERY_PROCESS_WANTS), name])
      )
    );
    const wonByProcess = outputs.map((stdout) => JSON.parse(stdout));

    for (let key = 0; key < KEYS_EVERY_PROCESS_WANTS; key += 1) {
      const winners = wonByProcess.filter((won) => won.includes(key)).length;
      assert.equal(winners, 1, `fastpay:shared-${key} was marked by ${winners} processes`);
      assert.equal(openedBeforeTheRace.markSpent(`fastpay:shared-${key}`, {}), false);
    }
    for (const name of names) {
      assert.equal(openedBeforeTheRace.has(`fastpay:own-${name}`), true, `the entry of ${name} is lost`);
    }
    assert.equal(openedBeforeTheRace.size(), KEYS_EVERY_PROCESS_WANTS + RACING_PROCESSES);
  });
}

function fileStoreSkipsACutOffLineAndStopsOnAnUnreadableOne() {
  return withTempDir((dir) => {
    const file = path.join(dir, 'spent.jsonl');
    const store = new FileSpentStore(file);
    assert.equal(store.markSpent('fastpay:X', {}), true);

    // What a write leaves behind when the disk fills up half way: its writer refused the payment.
    fs.appendFileSync(file, '\n{"dedup_key":"fastpay:cut-off","wri');
    const afterCutOff = new FileSpentStore(file);
    assert.equal(afterCutOff.markSpent('fastpay:Y', {}), true);
    assert.equal(store.has('fastpay:Y'), true);
    assert.equal(store.has('fastpay:cut-off'), false);
    assert.equal(store.size(), 2);

    fs.appendFileSync(file, '\n{"dedup_key":7}\n');
    assert.throws(() => store.has('fastpay:X'), /not a used-payment record/);
    assert.throws(() => store.markSpent('fastpay:W', {}), /not a used-payment record/);
    assert.throws(() => new FileSpentStore(file), /not a used-payment record/);
  });
}

function usedPaymentIsOnDiskBeforeTheResourceIsServed({ payerKeys }) {
  return withTempDir(async (dir) => {
    const file = path.join(dir, 'spent.jsonl');
    const realFsyncSync = fs.fsyncSync;
    let syncs = 0;
    fs.fsyncSync = (fd) => {
      syncs += 1;
      return realFsyncSync(fd);
    };
    const seenWhenServed = {};
    const spentStore = new FileSpentStore(file);
    const syncsBeforeThePayment = syncs;
    const stack = await startStack({
      coreOptions: { spentStore },
      beforeServe: (settled) => {
        seenWhenServed.syncs = syncs;
        seenWhenServed.recordOnDisk = fs.readFileSync(file, 'utf8').includes(settled.dedupKey);
      },
    });
    try {
      const body = '{"prompt":"durable"}';
      const required = await challengeAt(stack, '/cheap', body);
      const payment = await payFor(required, requestHashFor('/cheap', body), payerKeys);
      const res = await presentAt(stack, '/cheap', body, payment);
      assert.equal(res.statusCode, 200, JSON.stringify(res.body));
      assert.equal(seenWhenServed.recordOnDisk, true, 'the record must be in the file before the resource is served');
      assert.ok(seenWhenServed.syncs > syncsBeforeThePayment, 'the record must be synced to disk before the resource is served');
    } finally {
      fs.fsyncSync = realFsyncSync;
      await stack.close();
    }
  });
}

async function gateRefusesWhenTheUsedPaymentRecordCannotBeWritten({ payerKeys }) {
  const internalDetail = '/var/lib/x402/spent.jsonl';
  const spentStore = {
    failing: true,
    has: () => false,
    size: () => 0,
    markSpent() {
      if (this.failing) throw new Error(`ENOSPC: no space left on device, write ${internalDetail}`);
      return true;
    },
  };
  const stack = await startStack({ coreOptions: { spentStore } });
  try {
    const body = '{"prompt":"disk full"}';
    const required = await challengeAt(stack, '/cheap', body);
    const payment = await payFor(required, requestHashFor('/cheap', body), payerKeys);

    const refused = await presentAt(stack, '/cheap', body, payment);
    assertRefused(refused, 503, 'spent_store_unavailable');
    assert.equal(JSON.stringify(refused.body).includes(internalDetail), false);
    assert.equal(stack.servedBodies.length, 0);

    spentStore.failing = false;
    const served = await presentAt(stack, '/cheap', body, payment);
    assert.equal(served.statusCode, 200, JSON.stringify(served.body));
  } finally {
    await stack.close();
  }
}

// The same signed order under the same challenge terms after a restart, at the facilitator.
// A gate refuses it earlier already: its challenges do not outlive the process.
function usedPaymentStaysUsedAfterARestart({ payerKeys }) {
  return withTempDir(async (dir) => {
    const file = path.join(dir, 'spent.jsonl');
    const requestHash = requestHashFor('/cheap', '{"prompt":"restart"}');
    const beforeRestart = await startStack({ coreOptions: { spentStore: new FileSpentStore(file) } });
    let required;
    let payment;
    try {
      ({ required } = beforeRestart.core.issueChallenge({ ...CHEAP_TERMS, maxAmountAtoms: CHEAP_TERMS.priceAtoms, requestHash }));
      payment = await payFor(required, requestHash, payerKeys);
      const settled = await beforeRestart.core.settlePayment(payment, { expectedRequired: required });
      assert.equal(settled.response.settled, true);
    } finally {
      await beforeRestart.close();
    }

    const afterRestart = await startStack({ coreOptions: { spentStore: new FileSpentStore(file) } });
    try {
      await assert.rejects(
        () => afterRestart.core.settlePayment(payment, { expectedRequired: required }),
        (err) => err instanceof X402Error && err.statusCode === 409 && err.code === 'payment_already_spent'
      );
      assert.equal(afterRestart.backendCalls.fastpay, 0);
    } finally {
      await afterRestart.close();
    }
  });
}

function daemonEnv(extra) {
  return { PATH: process.env.PATH, X402_LISTEN_PORT: '0', ...extra };
}

// The daemon must not run on a store that forgets every used payment on restart.
function daemonRefusesToStartWithoutADurableStore() {
  return withTempDir(async (dir) => {
    const refused = spawnSync(process.execPath, [DAEMON_PATH], {
      env: daemonEnv({}),
      encoding: 'utf8',
      timeout: DAEMON_START_TIMEOUT_MS,
    });
    assert.equal(refused.status, 1, 'the daemon must exit instead of running on an in-memory store');
    assert.match(refused.stderr, /X402_SPENT_STORE_PATH/);

    const started = await new Promise((resolve, reject) => {
      const child = spawn(process.execPath, [DAEMON_PATH], {
        env: daemonEnv({ X402_SPENT_STORE_PATH: path.join(dir, 'spent.jsonl') }),
        stdio: ['ignore', 'pipe', 'pipe'],
      });
      const giveUp = setTimeout(() => {
        child.kill();
        reject(new Error('the daemon did not start with a durable store'));
      }, DAEMON_START_TIMEOUT_MS);
      child.stdout.on('data', (chunk) => {
        clearTimeout(giveUp);
        child.kill();
        resolve(String(chunk));
      });
      child.on('error', reject);
    });
    assert.match(started, /x402_facilitator_started/);
  });
}

// ---------------------------------------------------------------------------
// The paying client: a total budget, and the terms the caller agreed to
// ---------------------------------------------------------------------------

// A server that asks for `terms` on every call, takes the signed payment and never serves.
async function startTakingServer(terms) {
  const taken = [];
  const server = http.createServer({ maxHeaderSize: 65_536 }, async (req, res) => {
    const rawBody = await readRawBody(req);
    const paymentHeader = req.headers[HEADER_PAYMENT_SIGNATURE];
    if (paymentHeader) {
      const payment = mp.decodeX402PaymentSignature(mp.decodeX402Header(String(paymentHeader)));
      const { order } = JSON.parse(payment.signed_payload.toString('utf8'));
      taken.push({ to: order.payee_account, atoms: order.amount_atoms + order.fee_atoms });
      res.writeHead(500, { 'content-type': JSON_CONTENT_TYPE });
      res.end('{"error":"try again later"}');
      return;
    }
    const required = {
      x402_version: 2,
      scheme: mp.X402Scheme.EXACT,
      network: NETWORK,
      lane: mp.PaymentLane.FASTPAY,
      asset_id: terms.assetId,
      max_amount_atoms: terms.priceAtoms,
      pay_to: terms.payTo,
      request_hash: computeHttpRequestHash({
        method: req.method,
        urlPath: req.url,
        contentType: req.headers['content-type'],
        body: rawBody,
      }),
      nonce: crypto.randomBytes(16),
      expires_at_unix_ms: Date.now() + 60_000,
    };
    res.writeHead(402, {
      'content-type': JSON_CONTENT_TYPE,
      [HEADER_PAYMENT_REQUIRED]: mp.encodeX402Header(mp.encodeX402PaymentRequired(required)),
    });
    res.end('{}');
  });
  const port = await listen(server);
  return { taken, endpointUrl: `http://127.0.0.1:${port}/mcp`, close: () => closeServer(server) };
}

function payingClient(payerKeys, maxBudgetAtoms) {
  return new X402HttpClient({
    chainId: CHAIN_ID,
    payerAccount: PAYER,
    payerPublicKey: payerKeys.publicKey,
    payerSecretKey: payerKeys.secretKey,
    maxBudgetAtoms,
    allowedAssets: ['PFT', PFUSDC_ASSET_ID],
  });
}

async function errorCodesOfCalls(mcpClient, calls, options) {
  const codes = [];
  for (let i = 0; i < calls; i += 1) {
    try {
      await mcpClient.callTool('cheap_lookup', { q: i }, options);
      codes.push('served');
    } catch (err) {
      codes.push(err.code);
    }
  }
  return codes;
}

// A server that asks on every call and never serves must not get more than the budget in total.
async function clientBudgetIsATotalAcrossCalls({ payerKeys }) {
  const budgetAtoms = 500_000;
  const askedPerCall = 200_000;
  const server = await startTakingServer({ priceAtoms: askedPerCall, assetId: 'PFT', payTo: OTHER_ACCOUNT });
  try {
    const mcpClient = new X402McpClient({ endpointUrl: server.endpointUrl, httpClient: payingClient(payerKeys, budgetAtoms) });
    const codes = await errorCodesOfCalls(mcpClient, 5);

    assert.deepEqual(codes, ['mcp_call_failed', 'mcp_call_failed', 'budget_exceeded', 'budget_exceeded', 'budget_exceeded']);
    assert.equal(server.taken.length, 2);
    const signedAtoms = server.taken.reduce((sum, payment) => sum + payment.atoms, 0);
    assert.ok(signedAtoms <= budgetAtoms, `signed ${signedAtoms} atoms with a budget of ${budgetAtoms}`);

    // The fee is part of what leaves the account, so a budget equal to the price is not enough.
    const exactPriceClient = new X402McpClient({ endpointUrl: server.endpointUrl, httpClient: payingClient(payerKeys, askedPerCall) });
    assert.deepEqual(await errorCodesOfCalls(exactPriceClient, 1), ['budget_exceeded']);
    assert.equal(server.taken.length, 2);
  } finally {
    await server.close();
  }
  assert.throws(() => payingClient(payerKeys, Number.NaN), /maxBudgetAtoms/);
}

async function clientRefusesTermsTheCallerDidNotAgreeTo({ payerKeys }) {
  const agreed = { amountAtoms: 50_000, assetId: 'PFT', payTo: PAYEE };
  const asked = { priceAtoms: agreed.amountAtoms, assetId: agreed.assetId, payTo: agreed.payTo };
  const otherTerms = {
    price: { ...asked, priceAtoms: agreed.amountAtoms + 1 },
    asset: { ...asked, assetId: PFUSDC_ASSET_ID },
    recipient: { ...asked, payTo: OTHER_ACCOUNT },
  };
  for (const [name, terms] of Object.entries(otherTerms)) {
    const server = await startTakingServer(terms);
    try {
      const mcpClient = new X402McpClient({ endpointUrl: server.endpointUrl, httpClient: payingClient(payerKeys, 1_000_000) });
      const codes = await errorCodesOfCalls(mcpClient, 1, { expectedPayment: agreed });
      assert.deepEqual(codes, ['payment_terms_mismatch'], `another ${name} than the caller agreed to`);
      assert.equal(server.taken.length, 0, `no payment may be signed for another ${name}`);
    } finally {
      await server.close();
    }
  }

  const server = await startTakingServer(asked);
  try {
    const mcpClient = new X402McpClient({ endpointUrl: server.endpointUrl, httpClient: payingClient(payerKeys, 1_000_000) });
    const incomplete = { amountAtoms: agreed.amountAtoms, assetId: agreed.assetId };
    assert.deepEqual(await errorCodesOfCalls(mcpClient, 1, { expectedPayment: incomplete }), ['invalid_expected_payment']);
    assert.equal(server.taken.length, 0);

    assert.deepEqual(await errorCodesOfCalls(mcpClient, 1, { expectedPayment: agreed }), ['mcp_call_failed']);
    assert.deepEqual(server.taken.map((payment) => payment.to), [PAYEE]);
  } finally {
    await server.close();
  }
}

// ---------------------------------------------------------------------------
// Limits: not usable against a payer, and bounded
// ---------------------------------------------------------------------------

function junkPaymentNaming(payerPublicKey, required, requestHash) {
  const signedPayload = Buffer.from(
    JSON.stringify({
      order: { chain_id: 'x' },
      payer_pubkey_b64: Buffer.from(payerPublicKey).toString('base64url'),
      signature_b64: Buffer.alloc(mp.ML_DSA_65_SIGNATURE_BYTES).toString('base64url'),
    })
  );
  return wrapPayment(required, requestHash, { order_hash: Buffer.alloc(32), signed_payload: signedPayload });
}

// Unsigned payloads that name a payer's key, sent from several addresses, must not get that
// payer's valid payment refused with 429.
async function junkAndOldPaymentsDoNotCountAgainstAPayer({ payerKeys }) {
  const payerLimit = 2;
  const stack = await startStack({ coreOptions: { keyRateLimit: payerLimit } });
  try {
    const requestHash = requestHashFor('/cheap', '{"prompt":"limits"}');
    const freshPayment = async () => {
      const { required } = stack.core.issueChallenge({ ...CHEAP_TERMS, maxAmountAtoms: CHEAP_TERMS.priceAtoms, requestHash });
      return { required, payment: await payFor(required, requestHash, payerKeys) };
    };
    const rejectionOf = (run) => run().then(() => 'settled', (err) => err.code);

    for (let i = 0; i < payerLimit + 3; i += 1) {
      const { required } = stack.core.issueChallenge({ ...CHEAP_TERMS, maxAmountAtoms: CHEAP_TERMS.priceAtoms, requestHash });
      const junk = junkPaymentNaming(payerKeys.publicKey, required, requestHash);
      const remoteIp = `10.0.0.${i}`;
      assert.equal(await rejectionOf(() => stack.core.verifyPaymentLocally(junk, { remoteIp })), 'chain_id_mismatch');
      assert.equal(await rejectionOf(() => stack.core.settlePayment(junk, { remoteIp })), 'chain_id_mismatch');
    }

    const first = await freshPayment();
    assert.equal(await rejectionOf(() => stack.core.settlePayment(first.payment)), 'settled');

    // The payer's own payment, already used, sent again by somebody who saw it.
    for (let i = 0; i < payerLimit + 3; i += 1) {
      const replay = () => stack.core.settlePayment(first.payment, { expectedRequired: first.required, remoteIp: `10.0.1.${i}` });
      assert.equal(await rejectionOf(replay), 'payment_already_spent');
    }

    const second = await freshPayment();
    assert.equal(await rejectionOf(() => stack.core.settlePayment(second.payment)), 'settled');

    // The limit still holds for what the payer really pays.
    const third = await freshPayment();
    assert.equal(await rejectionOf(() => stack.core.settlePayment(third.payment)), 'rate_limited_payer');
    assert.equal(stack.backendCalls.fastpay, payerLimit);
  } finally {
    await stack.close();
  }
}

async function openChallengesHaveABoundAndAnExpiry({ payerKeys }) {
  const maxOpenChallenges = 3;
  const stack = await startStack({ coreOptions: { maxOpenChallenges }, gateOptions: { maxOpenChallenges } });
  try {
    const body = '{"prompt":"bounded"}';
    const requestHash = requestHashFor('/cheap', body);
    const issue = (extra = {}) =>
      stack.core.issueChallenge({ ...CHEAP_TERMS, maxAmountAtoms: CHEAP_TERMS.priceAtoms, requestHash, ...extra });
    const isFull = (err) => err instanceof X402Error && err.statusCode === 503 && err.code === 'challenge_store_full';

    // The public store: full after three, free again once they expired, and no challenge may
    // ask to live longer than the limit.
    for (let i = 0; i < maxOpenChallenges; i += 1) issue();
    assert.throws(() => issue(), isFull);
    stack.clock.nowMs += CHALLENGE_TTL_MS + 1;
    for (let i = 0; i < maxOpenChallenges; i += 1) issue();
    assert.throws(() => issue(), isFull);
    assert.throws(() => issue({ ttlMs: FAR_FUTURE_TTL_MS }), (err) => err instanceof X402Error && err.code === 'invalid_ttl');
    const overHttp = await post(stack.facilitatorPort, '/v1/challenge', {
      body: JSON.stringify({ max_amount_atoms: 1, pay_to: PAYEE, request_hash_hex: requestHash.toString('hex'), ttl_ms: FAR_FUTURE_TTL_MS }),
    });
    assertRefused(overHttp, 400, 'invalid_ttl');

    // A gate's store: a full store refuses new challenges and keeps the ones it gave out.
    const firstChallenge = await challengeAt(stack, '/cheap', body);
    for (let i = 1; i < maxOpenChallenges; i += 1) await challengeAt(stack, '/cheap', body);
    assertRefused(await post(stack.resourcePort, '/cheap', { body }), 503, 'challenge_store_full');
    const payment = await payFor(firstChallenge, requestHash, payerKeys);
    const paid = await presentAt(stack, '/cheap', body, payment);
    assert.equal(paid.statusCode, 200, JSON.stringify(paid.body));

    stack.clock.nowMs += CHALLENGE_TTL_MS + 1;
    for (let i = 0; i < maxOpenChallenges; i += 1) await challengeAt(stack, '/cheap', body);
    assertRefused(await post(stack.resourcePort, '/cheap', { body }), 503, 'challenge_store_full');
  } finally {
    await stack.close();
  }
}

function limiterCountersHaveABoundAndAnExpiry() {
  const windowMs = 1_000;
  const maxBuckets = 3;
  let nowMs = START_CLOCK_MS;
  const limiter = new SlidingWindowRateLimiter({ limit: 1, windowMs, maxBuckets, now: () => nowMs });

  assert.equal(limiter.admit('a').allowed, true);
  assert.equal(limiter.admit('a').allowed, false, 'one request per window');
  for (const key of ['b', 'c', 'd']) {
    nowMs += 1;
    assert.equal(limiter.admit(key).allowed, true, 'a new key is never locked out by a full table');
  }
  assert.equal(limiter.admit('a').allowed, true, 'a full table forgets its oldest counter');
  assert.equal(limiter.admit('d').allowed, false, 'and keeps the newer ones');
  assert.equal(limiter.size(), maxBuckets);

  nowMs += windowMs;
  assert.equal(limiter.admit('d').allowed, true);
  assert.equal(limiter.size(), 1, 'counters of a finished window are gone');
}

// Made-up API keys fill a table of their own. They must not push a payer's counter out of the
// payer table, which would reset that payer's count.
async function madeUpApiKeysDoNotResetAPayersCount({ payerKeys }) {
  const maxRateLimitBuckets = 4;
  const stack = await startStack({ coreOptions: { maxRateLimitBuckets, keyRateLimit: 1 } });
  try {
    const requestHash = requestHashFor('/cheap', '{"prompt":"api keys"}');
    const settleFresh = async (options) => {
      const { required } = stack.core.issueChallenge({ ...CHEAP_TERMS, maxAmountAtoms: CHEAP_TERMS.priceAtoms, requestHash });
      const payment = await payFor(required, requestHash, payerKeys);
      return stack.core.settlePayment(payment, options).then(() => 'settled', (err) => err.code);
    };

    assert.equal(await settleFresh(), 'settled');
    for (let i = 0; i < maxRateLimitBuckets * 2; i += 1) {
      const junk = () => stack.core.verifyPaymentLocally('junk', { apiKey: `made-up-${i}` });
      await assert.rejects(junk, (err) => err.code === 'invalid_payment_signature_header');
    }
    assert.equal(await settleFresh(), 'rate_limited_payer', 'the payer is still counted after the flood of keys');
  } finally {
    await stack.close();
  }
}

async function unpaidRequestsAreLimitedPerAddress({ payerKeys }) {
  const ipRateLimit = 3;
  const stack = await startStack({ coreOptions: { ipRateLimit } });
  try {
    const body = '{"prompt":"unpaid"}';
    const firstChallenge = await challengeAt(stack, '/cheap', body);
    for (let i = 1; i < ipRateLimit; i += 1) await challengeAt(stack, '/cheap', body);
    assertRefused(await post(stack.resourcePort, '/cheap', { body }), 429, 'rate_limited_ip');

    // Asking and paying are counted apart, so the challenges already given out can be paid.
    const payment = await payFor(firstChallenge, requestHashFor('/cheap', body), payerKeys);
    const paid = await presentAt(stack, '/cheap', body, payment);
    assert.equal(paid.statusCode, 200, JSON.stringify(paid.body));
  } finally {
    await stack.close();
  }
}

// ---------------------------------------------------------------------------
// The daemon: a prototype until it has a settlement adapter on the chain
// ---------------------------------------------------------------------------

function daemonRefusesProductionModeWithoutAnAdapter() {
  return withTempDir((dir) => {
    const spentStorePath = path.join(dir, 'spent.jsonl');
    const production = { X402_MODE: 'production', X402_SPENT_STORE_PATH: spentStorePath };
    const needsAdapter = /settlement adapter/;

    const fromCli = spawnSync(process.execPath, [DAEMON_PATH], {
      env: daemonEnv(production),
      encoding: 'utf8',
      timeout: DAEMON_START_TIMEOUT_MS,
    });
    assert.equal(fromCli.status, 1, 'the shipped daemon has no adapter and must not start in production mode');
    assert.match(fromCli.stderr, needsAdapter);

    assert.throws(() => createFacilitatorFromEnv(production), needsAdapter);
    const halfAdapter = { settleFastPay: async () => ({ accepted: true }) };
    assert.throws(() => createFacilitatorFromEnv(production, { settlementAdapter: halfAdapter }), needsAdapter);
    assert.throws(() => createFacilitatorFromEnv({ ...production, X402_MODE: 'live' }), /X402_MODE/);

    const settlementAdapter = { ...halfAdapter, settleConsensus: async () => ({ accepted: true, finalizedHeight: 1 }) };
    assert.equal(createFacilitatorFromEnv(production, { settlementAdapter }).mode, 'production');
    assert.equal(createFacilitatorFromEnv({ X402_SPENT_STORE_PATH: spentStorePath }).mode, 'prototype');
  });
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
  ['a used payment is seen by every store on the file', usedPaymentIsSeenByEveryStoreOnTheFile],
  ['a used payment is marked by one process only', usedPaymentIsMarkedByOneProcessOnly],
  ['the file store skips a cut-off line and stops on an unreadable one', fileStoreSkipsACutOffLineAndStopsOnAnUnreadableOne],
  ['a used payment is on disk before the resource is served', usedPaymentIsOnDiskBeforeTheResourceIsServed],
  ['a gate refuses when the used-payment record cannot be written', gateRefusesWhenTheUsedPaymentRecordCannotBeWritten],
  ['a used payment stays used after a restart', usedPaymentStaysUsedAfterARestart],
  ['the daemon refuses to start without a durable store', daemonRefusesToStartWithoutADurableStore],
  ['the client budget is a total across calls', clientBudgetIsATotalAcrossCalls],
  ['the client refuses terms the caller did not agree to', clientRefusesTermsTheCallerDidNotAgreeTo],
  ['junk and old payments do not count against a payer', junkAndOldPaymentsDoNotCountAgainstAPayer],
  ['open challenges have a bound and an expiry', openChallengesHaveABoundAndAnExpiry],
  ['limiter counters have a bound and an expiry', limiterCountersHaveABoundAndAnExpiry],
  ['made-up API keys do not reset the count of a payer', madeUpApiKeysDoNotResetAPayersCount],
  ['unpaid requests are limited per address', unpaidRequestsAreLimitedPerAddress],
  ['the daemon refuses production mode without an adapter', daemonRefusesProductionModeWithoutAnAdapter],
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
