const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');

const {
    certifyShieldedBatchViaWarmLoop,
    shieldedCertifiedRoundEnv,
    shieldedCertifierLoopBatchFile,
    validateShieldedCertifierLoopReportForBatch,
} = require('./server');

const defaults = shieldedCertifiedRoundEnv({});
assert.strictEqual(defaults.POSTFIAT_PREWARM_SHIELDED_VERIFIER, '0');
assert.strictEqual(defaults.POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER, '0');
assert.strictEqual(defaults.POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER, '0');

const staleInherited = shieldedCertifiedRoundEnv({
    POSTFIAT_PREWARM_SHIELDED_VERIFIER: '1',
    POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER: '1',
    POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER: '1',
});
assert.strictEqual(staleInherited.POSTFIAT_PREWARM_SHIELDED_VERIFIER, '0');
assert.strictEqual(staleInherited.POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER, '0');
assert.strictEqual(staleInherited.POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER, '0');

const optIn = shieldedCertifiedRoundEnv({
    NAVSWAP_SHIELDED_ROUND_PREWARM: 'true',
});
assert.strictEqual(optIn.POSTFIAT_PREWARM_SHIELDED_VERIFIER, '1');
assert.strictEqual(optIn.POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER, '1');
assert.strictEqual(optIn.POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER, '1');

assert.strictEqual(
    shieldedCertifierLoopBatchFile({ batch_dir: '/tmp/postfiat-loop' }, '20260703-000000000Z'),
    '/tmp/postfiat-loop/20260703-000000000Z.batch.json',
);

function makeBatch(batchId, actionId) {
    return {
        schema: 'postfiat-shielded-action-batch-v1',
        batch_id: batchId,
        actions: [{ kind: 'shielded_swap_v1', action_id: actionId, payload: { swap_json: '{}' } }],
    };
}

function makeRound(batchId, actionId) {
    return {
        round_ok: true,
        local_accepted_count: '1',
        local_rejected_count: '0',
        local_hot_finality: [{
            tx_id: actionId,
            receipt: { tx_id: actionId, accepted: true },
            block: {
                header: { batch_id: batchId },
                receipt_ids: [actionId],
            },
        }],
    };
}

validateShieldedCertifierLoopReportForBatch(
    makeBatch('batch-a', 'action-a'),
    makeRound('batch-a', 'action-a'),
    { rounds: [makeRound('batch-a', 'action-a')] },
);
assert.throws(
    () => validateShieldedCertifierLoopReportForBatch(
        makeBatch('batch-a', 'action-a'),
        makeRound('batch-b', 'action-a'),
        { rounds: [makeRound('batch-b', 'action-a')] },
    ),
    /batch id does not match/,
);
assert.throws(
    () => validateShieldedCertifierLoopReportForBatch(
        makeBatch('batch-a', 'action-a'),
        makeRound('batch-a', 'action-b'),
        { rounds: [makeRound('batch-a', 'action-b')] },
    ),
    /missing receipt/,
);

async function testCertifierLoopRearmsAfterOneRound() {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-certifier-loop-test-'));
    const nodeBin = path.join(tmpDir, 'fake-postfiat-node.js');
    const dataDir = path.join(tmpDir, 'data');
    const topology = path.join(tmpDir, 'topology.json');
    const keyFile = path.join(tmpDir, 'validator-key.json');
    const batchDir = path.join(tmpDir, 'batches');
    const artifactRoot = path.join(tmpDir, 'artifacts');
    fs.mkdirSync(dataDir, { recursive: true });
    fs.writeFileSync(topology, '{}\n');
    fs.writeFileSync(keyFile, '{}\n');
    fs.writeFileSync(nodeBin, `#!/usr/bin/env node
const fs = require('fs');
const path = require('path');
const args = process.argv.slice(2);
if (args[0] === 'status') {
  process.stdout.write(JSON.stringify({ block_height: 41 }) + '\\n');
  process.exit(0);
}
if (args[0] !== 'transport-peer-certified-batch-loop') {
  process.stderr.write('unexpected command ' + args[0] + '\\n');
  process.exit(9);
}
const batchDir = args[args.indexOf('--batch-dir') + 1];
const deadline = Date.now() + 5000;
let file = null;
while (Date.now() < deadline) {
  const files = fs.readdirSync(batchDir).filter((name) => name.endsWith('.batch.json')).sort();
  if (files.length > 0) { file = path.join(batchDir, files[0]); break; }
}
if (!file) {
  process.stderr.write('no batch file\\n');
  process.exit(10);
}
const batch = JSON.parse(fs.readFileSync(file, 'utf8'));
const actionId = batch.actions[0].action_id;
process.stdout.write(JSON.stringify({
  rounds: [{
    round_ok: true,
    local_accepted_count: '1',
    local_rejected_count: '0',
    local_hot_finality: [{
      tx_id: actionId,
      receipt: { tx_id: actionId, accepted: true },
      block: { header: { batch_id: batch.batch_id }, receipt_ids: [actionId] },
    }],
  }],
}) + '\\n');
`, { mode: 0o700 });
    fs.chmodSync(nodeBin, 0o700);
    const config = {
        certifier_loop: {
            enabled: true,
            batch_dir: batchDir,
            artifact_root: artifactRoot,
            processed_dir: path.join(tmpDir, 'processed'),
            ready_file: path.join(tmpDir, 'ready.json'),
            report_file: path.join(tmpDir, 'loop-report.json'),
            poll_ms: 10,
        },
        node_bin: nodeBin,
        data_dir: dataDir,
        topology,
        key_file: keyFile,
        timeout_ms: 5000,
    };
    try {
        const firstBatch = path.join(tmpDir, 'first-batch.json');
        fs.writeFileSync(firstBatch, JSON.stringify(makeBatch('batch-1', 'action-1')));
        const first = await certifyShieldedBatchViaWarmLoop(config, firstBatch, 'first');
        assert.strictEqual(first.round.local_hot_finality[0].tx_id, 'action-1');
        fs.unlinkSync(first.batch_file);

        const secondBatch = path.join(tmpDir, 'second-batch.json');
        fs.writeFileSync(secondBatch, JSON.stringify(makeBatch('batch-2', 'action-2')));
        const second = await certifyShieldedBatchViaWarmLoop(config, secondBatch, 'second');
        assert.strictEqual(second.round.local_hot_finality[0].tx_id, 'action-2');
    } finally {
        fs.rmSync(tmpDir, { recursive: true, force: true });
    }
}

testCertifierLoopRearmsAfterOneRound()
    .then(() => {
        console.log('PASS shielded certified round prewarm env defaults');
    })
    .catch((error) => {
        console.error(error);
        process.exitCode = 1;
    });
