const assert = require('assert');
const fs = require('fs');
const net = require('net');
const os = require('os');
const path = require('path');

const VALIDATOR_COUNT = 6;

async function main() {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-fastpay-outbox-'));
  const outboxPath = path.join(tempDir, 'outbox.json');
  const servers = [];
  const ports = [];
  let durableBeforeApply = true;

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
          if (request.method === 'owned_apply') {
            try {
              const persisted = JSON.parse(fs.readFileSync(outboxPath, 'utf8'));
              durableBeforeApply = durableBeforeApply && persisted.records.length === 1;
            } catch (_) {
              durableBeforeApply = false;
            }
          }
          const delay = request.method === 'owned_apply' ? (i === 2 ? 5 : 180) : 0;
          setTimeout(() => socket.write(`${JSON.stringify({
            version: 'postfiat-local-rpc-v1',
            id: request.id,
            ok: true,
            result: request.method === 'status' ? {
              block_height: 100,
              block_tip_hash: 'tip',
              state_root: 'root',
              validator_id: validatorId,
              validator_count: VALIDATOR_COUNT,
              chain_id: 'postfiat-wan-devnet',
            } : {
              schema: 'postfiat-owned-apply-report-v1',
              summary: `applied on ${validatorId}`,
              created_objects: [],
            },
            error: null,
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
  process.env.FASTPAY_CERTIFICATE_OUTBOX_PATH = outboxPath;
  const { broadcastFastpayMutation, closeUpstreamRpcConnections } = require('./server');
  const cert = {
    order: { inputs: [{ id: 'a', version: 1 }], outputs: [{ owner_pubkey_hex: 'b', value: 1, asset: 'PFT' }], fee: 0, nonce: 1, memos: [] },
    owner_pubkey_hex: 'owner',
    owner_signature_hex: 'signature',
    votes: Array.from({ length: 5 }, (_, i) => ({ validator_id: `validator-${i}`, signature_hex: `sig-${i}` })),
  };
  const started = Date.now();
  const response = await broadcastFastpayMutation({
    version: 'postfiat-local-rpc-v1',
    id: 'certificate-finality',
    method: 'owned_apply',
    params: { cert_json: JSON.stringify(cert) },
  });
  const duration = Date.now() - started;
  assert.strictEqual(response.ok, true);
  assert.strictEqual(response.result.certificate_final, true);
  assert.strictEqual(response.result.certificate_quorum, 5);
  assert.strictEqual(response.result.certificate_vote_count, 5);
  assert.strictEqual(response.result.apply_ack_validator, 'validator-2');
  assert.strictEqual(response.result.applied_count, 1);
  assert.ok(duration < 150, `critical path waited for replication: ${duration}ms`);
  assert.strictEqual(durableBeforeApply, true, 'certificate must be durable before apply');

  await new Promise(resolve => setTimeout(resolve, 300));
  const terminal = JSON.parse(fs.readFileSync(outboxPath, 'utf8'));
  assert.deepStrictEqual(terminal.records, [], 'exact-six replication must drain the outbox');

  closeUpstreamRpcConnections();
  for (const server of servers) server.close();
  fs.rmSync(tempDir, { recursive: true, force: true });
  console.log('PASS FastPay returns at durable certificate + first apply ack and replicates exact-six asynchronously');
}

main().catch(error => {
  console.error(error);
  process.exit(1);
});
