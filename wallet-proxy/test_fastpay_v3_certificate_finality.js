'use strict';

const assert = require('assert');
const fs = require('fs');
const net = require('net');
const os = require('os');
const path = require('path');

const VALIDATOR_COUNT = 6;

async function main() {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-fastpay-v3-outbox-'));
  const outboxPath = path.join(tempDir, 'outbox.json');
  const servers = [];
  const ports = [];
  const seenValidatorIds = new Set();
  const applyAttempts = new Map();

  for (let i = 0; i < VALIDATOR_COUNT; i += 1) {
    const validatorId = `validator-${i}`;
    const server = net.createServer(socket => {
      let buffer = '';
      socket.on('data', chunk => {
        buffer += chunk.toString('utf8');
        let newline;
        while ((newline = buffer.indexOf('\n')) >= 0) {
          const request = JSON.parse(buffer.slice(0, newline));
          buffer = buffer.slice(newline + 1);
          if (request.method === 'owned_apply_v3') {
            seenValidatorIds.add(request.params.validator_id);
            applyAttempts.set(validatorId, (applyAttempts.get(validatorId) || 0) + 1);
          }
          const delay = request.method === 'owned_apply_v3' ? (i === 5 ? 180 : 5) : 0;
          const failFirstReplication = request.method === 'owned_apply_v3'
            && i === 5
            && applyAttempts.get(validatorId) === 1;
          setTimeout(() => socket.write(`${JSON.stringify({
            version: 'postfiat-local-rpc-v1',
            id: request.id,
            ok: !failFirstReplication,
            result: request.method === 'status' ? {
              block_height: 100,
              block_tip_hash: 'tip',
              state_root: 'root',
              validator_id: validatorId,
              validator_count: VALIDATOR_COUNT,
              chain_id: 'postfiat-wan-devnet',
            } : failFirstReplication ? null : {
              schema: 'postfiat-fastpay-apply-ack-v1',
              validator_id: validatorId,
              lock_id: 'lock',
              certificate_digest: 'certificate',
              terminal_state_digest: 'terminal',
            },
            error: failFirstReplication ? {
              code: 'owned_apply_v3_failed',
              message: 'temporary replication failure',
            } : null,
            events: [],
          })}\n`), delay);
        }
      });
    });
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    servers.push(server);
    ports.push(server.address().port);
  }

  process.env.RPC_FLEET = ports.map((port, i) => `validator-${i}=127.0.0.1:${port}`).join(',');
  process.env.ENABLE_UPSTREAM_KEEPALIVE = 'false';
  process.env.FASTPAY_CERTIFICATE_FINALITY_ENABLED = 'true';
  process.env.FASTPAY_CERTIFICATE_RETRY_MS = '250';
  process.env.FASTPAY_CERTIFICATE_OUTBOX_PATH = outboxPath;
  const {
    broadcastFastpayMutation,
    closeUpstreamRpcConnections,
    startFastpayCertificateRecovery,
  } = require('./server');
  startFastpayCertificateRecovery();
  const cert = {
    order: {
      recovery: { lock_id: 'lock' },
      inputs: [{ id: 'a', version: 1 }],
      outputs: [{ owner_pubkey_hex: 'b', value: 1, asset: 'PFT' }],
      fee: 0,
      nonce: 1,
      memos: [],
    },
    owner_pubkey_hex: 'owner',
    owner_signature_hex: 'signature',
    votes: Array.from({ length: 5 }, (_, i) => ({
      validator_id: `validator-${i}`,
      signature_hex: `sig-${i}`,
    })),
  };
  const started = Date.now();
  const response = await broadcastFastpayMutation({
    version: 'postfiat-local-rpc-v1',
    id: 'v3-certificate-finality',
    method: 'owned_apply_v3',
    params: { cert_json: JSON.stringify(cert) },
  });
  const duration = Date.now() - started;

  assert.strictEqual(response.ok, true);
  assert.strictEqual(response.result.certificate_final, true);
  assert.strictEqual(response.result.apply_acknowledgements.length, 5);
  assert.deepStrictEqual(
    response.result.apply_acknowledgements.map(ack => ack.validator_id).sort(),
    Array.from({ length: 5 }, (_, i) => `validator-${i}`),
    'compact finality must carry a distinct signed apply-ack quorum',
  );
  assert.ok(duration < 150, `v3 critical path waited beyond apply quorum: ${duration}ms`);

  await new Promise(resolve => setTimeout(resolve, 750));
  assert.deepStrictEqual(
    [...seenValidatorIds].sort(),
    Array.from({ length: VALIDATOR_COUNT }, (_, i) => `validator-${i}`),
    'each v3 apply must carry the target validator identity',
  );
  const terminal = JSON.parse(fs.readFileSync(outboxPath, 'utf8'));
  assert.deepStrictEqual(terminal.records, [], 'exact-six v3 replication must drain the outbox');
  assert.strictEqual(terminal.completed.length, 1, 'exact-six must retain a bounded replay tombstone');
  assert.strictEqual(applyAttempts.get('validator-5'), 2, 'failed v3 replication must retry from the durable outbox');

  const attemptsBeforeReplay = new Map(applyAttempts);
  const replay = await broadcastFastpayMutation({
    version: 'postfiat-local-rpc-v1',
    id: 'v3-certificate-finality-replay-after-lost-response',
    method: 'owned_apply_v3',
    params: { cert_json: JSON.stringify(cert) },
  });
  assert.strictEqual(replay.ok, true);
  assert.deepStrictEqual(
    replay.result,
    response.result,
    'a lost terminal response must replay exactly from the completed record',
  );
  assert.deepStrictEqual(
    [...applyAttempts.entries()],
    [...attemptsBeforeReplay.entries()],
    'completed replay must not contact validators or apply money twice',
  );

  closeUpstreamRpcConnections();
  for (const server of servers) server.close();
  fs.rmSync(tempDir, { recursive: true, force: true });
  console.log('PASS FastPay v3 returns at signed apply-ack quorum and durably replicates exact-six');
}

main().catch(error => {
  console.error(error);
  process.exit(1);
});
