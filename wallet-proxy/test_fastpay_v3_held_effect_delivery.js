'use strict';

// A validator that is registered but is not a signer of the FastPay committee
// answers owned_apply_v3 with an unsigned held-effect receipt. The proxy used to
// read that as a failed delivery and sent the same certificate to it again every
// retry interval, for ever. The delivery is done after one accepted receipt; the
// receipt never counts toward the signed quorum; a receipt for another lock or
// from another validator is not accepted.

const assert = require('assert');
const fs = require('fs');
const net = require('net');
const os = require('os');
const path = require('path');

const VALIDATOR_COUNT = 6;

async function main() {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-fastpay-held-'));
  const outboxPath = path.join(tempDir, 'outbox.json');
  const servers = [];
  const ports = [];
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
          let result;
          if (request.method === 'status') {
            result = {
              block_height: 100, block_tip_hash: 'tip', state_root: 'root',
              validator_id: validatorId, validator_count: VALIDATOR_COUNT, chain_id: 'postfiat-wan-devnet',
            };
          } else {
            const lock = JSON.parse(request.params.cert_json).order.recovery.lock_id;
            const key = `${validatorId}/${lock}`;
            applyAttempts.set(key, (applyAttempts.get(key) || 0) + 1);
            result = i === 5 ? {
              schema: 'postfiat-fastpay-held-effect-v1',
              validator_id: validatorId,
              // For the lock "wrong-lock" the holder names another lock: not this certificate's receipt.
              lock_id: lock === 'wrong-lock' ? 'another-lock' : lock,
              certificate_digest: 'certificate',
              signed: false,
            } : {
              schema: 'postfiat-fastpay-apply-ack-v1',
              validator_id: validatorId,
              lock_id: lock,
              certificate_digest: 'certificate',
              terminal_state_digest: 'terminal',
            };
          }
          socket.write(`${JSON.stringify({
            version: 'postfiat-local-rpc-v1', id: request.id, ok: true, result, error: null, events: [],
          })}\n`);
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
  const certificate = lock => JSON.stringify({
    order: {
      recovery: { lock_id: lock },
      inputs: [{ id: lock, version: 1 }],
      outputs: [{ owner_pubkey_hex: 'b', value: 1, asset: 'PFT' }],
      fee: 0, nonce: 1, memos: [],
    },
    owner_pubkey_hex: 'owner',
    owner_signature_hex: 'signature',
    votes: Array.from({ length: 5 }, (_, i) => ({ validator_id: `validator-${i}`, signature_hex: `sig-${i}` })),
  });
  const send = lock => broadcastFastpayMutation({
    version: 'postfiat-local-rpc-v1', id: `held-${lock}`, method: 'owned_apply_v3',
    params: { cert_json: certificate(lock) },
  });

  const response = await send('lock');
  assert.strictEqual(response.ok, true);
  assert.strictEqual(response.result.certificate_final, true);
  assert.deepStrictEqual(
    response.result.apply_acknowledgements.map(ack => ack.validator_id).sort(),
    Array.from({ length: 5 }, (_, i) => `validator-${i}`),
    'the unsigned receipt of the holder must not appear among the signed acknowledgements',
  );
  // Six retry intervals: before the fix the holder was sent the certificate again on each.
  await new Promise(resolve => setTimeout(resolve, 1600));
  assert.strictEqual(applyAttempts.get('validator-5/lock'), 1,
    'a holder that answered with its held-effect receipt must not be sent the certificate again');
  let outbox = JSON.parse(fs.readFileSync(outboxPath, 'utf8'));
  assert.deepStrictEqual(outbox.records, [], 'five signed acknowledgements and one held receipt drain the outbox');
  assert.strictEqual(outbox.completed.length, 1);

  const second = await send('wrong-lock');
  assert.strictEqual(second.ok, true, 'the signed quorum of five still finalizes the certificate');
  await new Promise(resolve => setTimeout(resolve, 900));
  outbox = JSON.parse(fs.readFileSync(outboxPath, 'utf8'));
  assert.strictEqual(outbox.records.length, 1, 'a receipt naming another lock leaves the delivery pending');
  assert.ok(!outbox.records[0].applied_validators.includes('validator-5'));
  assert.ok(applyAttempts.get('validator-5/wrong-lock') >= 2, 'and the delivery is tried again');

  closeUpstreamRpcConnections();
  for (const server of servers) server.close();
  fs.rmSync(tempDir, { recursive: true, force: true });
  console.log('fastpay v3 held-effect delivery test passed');
}

main().then(() => process.exit(0), error => { console.error(error); process.exit(1); });
