const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const {
  FastpayCertificateOutbox,
  MAX_COMPLETED_CERTIFICATES,
  SCHEMA,
} = require('./fastpay-certificate-outbox');

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-fastpay-outbox-unit-'));
const file = path.join(dir, 'outbox.json');
const outbox = new FastpayCertificateOutbox(file);
const request = { method: 'owned_apply', params: { cert_json: '{}' } };
outbox.enqueue({
  certificate_id: 'cert-1',
  method: 'owned_apply',
  request,
  created_at_ms: 1,
});
const signedAck = { schema: 'postfiat-fastpay-apply-ack-v1', validator_id: 'validator-0' };
outbox.markApplied('cert-1', 'validator-0', signedAck);
outbox.markApplied('cert-1', 'validator-0', signedAck);

const recovered = new FastpayCertificateOutbox(file);
assert.strictEqual(recovered.pending().length, 1);
assert.deepStrictEqual(recovered.pending()[0].applied_validators, ['validator-0']);
assert.deepStrictEqual(recovered.pending()[0].apply_acknowledgements, [signedAck]);
assert.strictEqual(recovered.terminal({ certificate_id: 'cert-1', method: 'owned_apply', request }), null);
assert.strictEqual(recovered.complete('cert-1'), false, 'a pre-terminal crash cannot complete');
const terminalResult = {
  schema: 'postfiat-fastpay-certificate-finality-v1',
  method: 'owned_apply',
  certificate_id: 'cert-1',
  certificate_final: true,
  apply_acknowledgements: [signedAck],
};
recovered.markTerminal('cert-1', terminalResult);
assert.strictEqual(recovered.complete('cert-1'), true);
const restarted = new FastpayCertificateOutbox(file);
assert.deepStrictEqual(restarted.pending(), []);
assert.strictEqual(restarted.completed().length, 1);
assert.deepStrictEqual(restarted.terminal({
  certificate_id: 'cert-1',
  method: 'owned_apply',
  request,
}), terminalResult, 'a lost response must replay after proxy restart');
assert.throws(() => restarted.terminal({
  certificate_id: 'cert-1',
  method: 'owned_apply',
  request: { method: 'owned_apply', params: { cert_json: '{"conflict":true}' } },
}), /conflicts with its durable record/);

const stored = JSON.parse(fs.readFileSync(file, 'utf8'));
assert.strictEqual(stored.schema, SCHEMA);
stored.completed[0].terminal_result.certificate_final = false;
fs.writeFileSync(file, `${JSON.stringify(stored)}\n`);
assert.throws(
  () => new FastpayCertificateOutbox(file),
  /terminal (?:result is not bound|digest mismatch)/,
  'a tampered completed response must fail closed',
);

const boundedFile = path.join(dir, 'bounded.json');
let now = 10_000;
const bounded = new FastpayCertificateOutbox(boundedFile, {
  now: () => now,
  maxCompletedCertificates: 2,
  completedTtlMs: 1_000,
});
for (let index = 0; index < 3; index += 1) {
  const certificateId = `bounded-${index}`;
  const request = { method: 'owned_apply', params: { cert_json: `{"index":${index}}` } };
  bounded.enqueue({ certificate_id: certificateId, method: request.method, request, created_at_ms: now });
  bounded.markTerminal(certificateId, {
    schema: 'postfiat-fastpay-certificate-finality-v1',
    certificate_id: certificateId,
    method: 'owned_apply',
    certificate_final: true,
  });
  bounded.complete(certificateId);
  now += 1;
}
assert.strictEqual(bounded.completed().length, 2);
assert.strictEqual(bounded.terminal({
  certificate_id: 'bounded-0',
  method: 'owned_apply',
  request: { method: 'owned_apply', params: { cert_json: '{"index":0}' } },
}), null, 'count compaction must evict the oldest completed response');
now += 2_000;
bounded.compact();
assert.deepStrictEqual(bounded.completed(), [], 'age compaction must remain bounded');
assert.ok(MAX_COMPLETED_CERTIFICATES >= 2);

const legacyFile = path.join(dir, 'legacy.json');
fs.writeFileSync(legacyFile, `${JSON.stringify({
  schema: 'postfiat-fastpay-certificate-outbox-v1',
  records: [{
    schema: 'postfiat-fastpay-certificate-outbox-record-v1',
    certificate_id: 'legacy-cert',
    method: 'owned_apply',
    request: { method: 'owned_apply', params: { cert_json: '{"legacy":true}' } },
    created_at_ms: 5,
    applied_validators: ['validator-0'],
    apply_acknowledgements: [signedAck],
  }],
})}\n`, { mode: 0o600 });
const migrated = new FastpayCertificateOutbox(legacyFile);
assert.strictEqual(migrated.pending().length, 1);
assert.strictEqual(JSON.parse(fs.readFileSync(legacyFile, 'utf8')).schema, SCHEMA);
fs.rmSync(dir, { recursive: true, force: true });
console.log('PASS FastPay certificate outbox is bounded, durable, conflict-safe, and replayable');
