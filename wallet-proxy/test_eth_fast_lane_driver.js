'use strict';

const assert = require('assert');
const crypto = require('crypto');
const fs = require('fs');
const os = require('os');
const path = require('path');
const {
    CONFIG_SCHEMA, ROUTE_ID, SOURCE_CHAIN_ID, SOURCE_PROOF_KIND, STAGES,
    migrateLegacyCapJob, readiness, runJob, validateConfig,
} = require('./eth-fast-lane-driver');
const TEST_PROGRAM_VKEY = `0x${'6b'.repeat(32)}`;
const TEST_MANIFEST_HASH = '6c'.repeat(32);
const TEST_ROUTE_PROFILE_HASH = '6d'.repeat(48);
const TEST_ASSET_ID = '6e'.repeat(48);
const TEST_VAULT = `0x${'11'.repeat(20)}`;
const TEST_VAULT_CODE_HASH = `0x${'12'.repeat(32)}`;
const TEST_TOKEN = `0x${'13'.repeat(20)}`;
const TEST_TOKEN_CODE_HASH = `0x${'14'.repeat(32)}`;

function stableJson(value) {
    if (Array.isArray(value)) return `[${value.map(stableJson).join(',')}]`;
    if (value && typeof value === 'object') {
        return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`).join(',')}}`;
    }
    return JSON.stringify(value);
}

function sha256(value) {
    return crypto.createHash('sha256').update(value).digest('hex');
}

function fileSha256(file) {
    return sha256(fs.readFileSync(file));
}

function writeJson(file, value, mode = 0o600) {
    fs.mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 });
    fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, { mode });
}

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-eth-fast-driver-'));
const mock = path.join(root, 'mock-stage.js');
fs.writeFileSync(mock, `
'use strict';
const fs = require('fs');
const path = require('path');
const mode = process.argv[2];
if (mode === 'readiness') {
  process.stdout.write(JSON.stringify({
    ok: true, ready: true,
    route_id: 'ethereum-mainnet-usdc-v1', source_chain_id: 1,
    source_proof_kind: 'sp1-ethereum-finality-v1', observer_attestor_enabled: false,
    program_vkey: '${TEST_PROGRAM_VKEY}', manifest_hash: '${TEST_MANIFEST_HASH}',
    route_profile_hash: '${TEST_ROUTE_PROFILE_HASH}', asset_id: '${TEST_ASSET_ID}',
    vault_address: '${TEST_VAULT}', vault_runtime_code_hash: '${TEST_VAULT_CODE_HASH}',
    token_address: '${TEST_TOKEN}', token_runtime_code_hash: '${TEST_TOKEN_CODE_HASH}',
    prover_authenticated: true, prover_healthy: true, route_manifest_active: true,
    program_vkey_active: true, nav_cap_growth_enabled: true, vault_paused: false,
    vault_code_hash_matches: true, token_code_hash_matches: true,
    execution_rpc_sources_reachable: 2, beacon_finality_current: true
  }));
  process.exit(0);
}
const stage = mode;
const jobFile = process.argv[process.argv.indexOf('--job-file') + 1];
const job = JSON.parse(fs.readFileSync(jobFile, 'utf8'));
const jobDir = path.dirname(jobFile);
const countsFile = path.join(jobDir, 'mock-counts.json');
let counts = {};
try { counts = JSON.parse(fs.readFileSync(countsFile, 'utf8')); } catch {}
counts[stage] = (counts[stage] || 0) + 1;
fs.writeFileSync(countsFile, JSON.stringify(counts));
if (stage === 'proving' && !fs.existsSync(path.join(jobDir, 'proving-failed-once'))) {
  fs.writeFileSync(path.join(jobDir, 'proving-failed-once'), '1');
  process.stdout.write(JSON.stringify({ok:false,retryable:true,code:'prover_temporarily_unavailable'}));
  process.exit(75);
}
const r = job.request;
const result = {
  ok: true, stage,
  route_id: r.route_id, source_chain_id: r.source_chain_id,
  source_proof_kind: 'sp1-ethereum-finality-v1',
  program_vkey: '${TEST_PROGRAM_VKEY}', manifest_hash: '${TEST_MANIFEST_HASH}',
  route_profile_hash: '${TEST_ROUTE_PROFILE_HASH}', asset_id: '${TEST_ASSET_ID}',
  vault_address: '${TEST_VAULT}', vault_runtime_code_hash: '${TEST_VAULT_CODE_HASH}',
  token_address: '${TEST_TOKEN}', token_runtime_code_hash: '${TEST_TOKEN_CODE_HASH}',
  deposit_tx_hash: r.deposit_tx_hash, deposit_id: r.deposit_id,
  pftl_recipient: r.pftl_recipient, depositor: r.depositor, amount_atoms: r.amount_atoms
};
if (stage === 'confirming_deposit') result.deposit_confirmed = true;
if (stage === 'waiting_for_ethereum_finality') {
  result.ethereum_finalized = true; result.finalized_block_hash = '0x' + '55'.repeat(32);
  result.finalized_block_number = 123456;
}
if (stage === 'capturing_state_proof') {
  result.witness_sha256 = '66'.repeat(32); result.evidence_root = '0x' + '67'.repeat(48);
  result.nullifier = '0x' + '68'.repeat(32);
}
if (stage === 'proving') {
  result.proof_sha256 = '69'.repeat(32); result.public_values_sha256 = '6a'.repeat(32);
  result.program_vkey = '0x' + '6b'.repeat(32);
}
if (stage === 'verifying') result.proof_verified = true;
if (stage === 'claiming') {
  result.receipt_code = 'ACCEPTED'; result.receipt_id = '77'.repeat(48);
  result.tx_id = '0x' + '78'.repeat(32);
}
if (process.env.MOCK_BAD_STAGE === stage) result.amount_atoms = '999';
process.stdout.write(JSON.stringify(result));
`, { mode: 0o700 });

function makeConfig(directory) {
    const config = {
        schema: CONFIG_SCHEMA,
        route_id: ROUTE_ID,
        source_chain_id: SOURCE_CHAIN_ID,
        source_proof_kind: SOURCE_PROOF_KIND,
        program_vkey: TEST_PROGRAM_VKEY,
        manifest_hash: TEST_MANIFEST_HASH,
        route_profile_hash: TEST_ROUTE_PROFILE_HASH,
        asset_id: TEST_ASSET_ID,
        vault_address: TEST_VAULT,
        vault_runtime_code_hash: TEST_VAULT_CODE_HASH,
        token_address: TEST_TOKEN,
        token_runtime_code_hash: TEST_TOKEN_CODE_HASH,
        pinned_files: [{ path: mock, sha256: fileSha256(mock) }],
        readiness: {
            program: process.execPath,
            program_sha256: fileSha256(process.execPath),
            args: [mock, 'readiness'],
            timeout_ms: 5_000,
        },
        stages: STAGES.map((stage) => ({
            stage,
            program: process.execPath,
            program_sha256: fileSha256(process.execPath),
            args: [mock, stage, '--job-file', '{job_file}'],
            timeout_ms: 5_000,
        })),
    };
    const file = path.join(directory, 'driver-config.json');
    writeJson(file, config);
    return file;
}

function makeJob(directory, suffix = '11') {
    const request = {
        route_id: ROUTE_ID,
        source_chain_id: SOURCE_CHAIN_ID,
        deposit_tx_hash: `0x${suffix.repeat(32)}`,
        deposit_id: `0x${'22'.repeat(32)}`,
        pftl_recipient: `pf${'33'.repeat(20)}`,
        depositor: `0x${'44'.repeat(20)}`,
        amount_atoms: '2000000',
        idempotency_key: `driver-test-${suffix}`,
    };
    const economic = { ...request };
    delete economic.idempotency_key;
    const file = path.join(directory, 'job.json');
    writeJson(file, {
        schema: 'postfiat-trustless-bridge-job-v2',
        job_id: `0x${'aa'.repeat(32)}`,
        status: 'queued',
        request,
        request_fingerprint: sha256(stableJson(economic)),
    });
    return file;
}

function resultFor(stage, request) {
    const result = {
        ok: true, stage,
        route_id: request.route_id, source_chain_id: request.source_chain_id,
        source_proof_kind: SOURCE_PROOF_KIND,
        program_vkey: TEST_PROGRAM_VKEY, manifest_hash: TEST_MANIFEST_HASH,
        route_profile_hash: TEST_ROUTE_PROFILE_HASH, asset_id: TEST_ASSET_ID,
        vault_address: TEST_VAULT, vault_runtime_code_hash: TEST_VAULT_CODE_HASH,
        token_address: TEST_TOKEN, token_runtime_code_hash: TEST_TOKEN_CODE_HASH,
        deposit_tx_hash: request.deposit_tx_hash, deposit_id: request.deposit_id,
        pftl_recipient: request.pftl_recipient, depositor: request.depositor,
        amount_atoms: request.amount_atoms,
    };
    if (stage === 'confirming_deposit') result.deposit_confirmed = true;
    if (stage === 'waiting_for_ethereum_finality') {
        result.ethereum_finalized = true;
        result.finalized_block_hash = `0x${'55'.repeat(32)}`;
        result.finalized_block_number = 123456;
    }
    if (stage === 'capturing_state_proof') {
        result.witness_sha256 = '66'.repeat(32);
        result.evidence_root = `0x${'67'.repeat(48)}`;
        result.nullifier = `0x${'68'.repeat(32)}`;
    }
    if (stage === 'proving') {
        result.proof_sha256 = '69'.repeat(32);
        result.public_values_sha256 = '6a'.repeat(32);
    }
    if (stage === 'verifying') result.proof_verified = true;
    return result;
}

function makeLegacyCapJob(directory, currentConfigFile) {
    const jobFile = makeJob(directory, '13');
    const job = JSON.parse(fs.readFileSync(jobFile, 'utf8'));
    const current = JSON.parse(fs.readFileSync(currentConfigFile, 'utf8'));
    const growing = {
        ...current.stages.find((row) => row.stage === 'verifying'),
        stage: 'growing_backed_cap',
    };
    growing.args = growing.args.map((arg) => (arg === 'verifying' ? 'growing_backed_cap' : arg));
    const legacy = {
        ...current,
        stages: [
            ...current.stages.slice(0, 5),
            growing,
            current.stages[5],
        ],
    };
    const snapshot = path.join(directory, 'driver-config.snapshot.json');
    writeJson(snapshot, legacy);
    const configSha256 = fileSha256(snapshot);
    let prior = '0'.repeat(64);
    const legacyStages = legacy.stages.slice(0, 6).map((row) => row.stage);
    legacyStages.forEach((stage, index) => {
        const checkpoint = {
            schema: 'postfiat-eth-fast-lane-stage-checkpoint-v1',
            stage,
            stage_index: index,
            request_fingerprint: job.request_fingerprint,
            config_sha256: configSha256,
            prior_checkpoint_sha256: prior,
            result: resultFor(stage, job.request),
        };
        checkpoint.checkpoint_sha256 = sha256(stableJson(checkpoint));
        prior = checkpoint.checkpoint_sha256;
        writeJson(path.join(directory, 'checkpoints', `${String(index).padStart(2, '0')}-${stage}.json`), checkpoint);
    });
    writeJson(path.join(directory, 'worker-state.json'), { status: 'claiming', retryable: true });
    return { jobFile, configSha256 };
}

async function main() {
    const config = makeConfig(root);
    const configValidation = validateConfig(config);
    assert.strictEqual(configValidation.ok, true);
    assert.strictEqual(configValidation.activation_ready, false);
    assert.strictEqual(configValidation.requires_live_readiness, true);
    assert.strictEqual(configValidation.route_profile_hash, TEST_ROUTE_PROFILE_HASH);
    const ready = await readiness(config, ROUTE_ID);
    assert.strictEqual(ready.ready, true);
    assert.strictEqual(ready.execution_rpc_sources_reachable, 2);
    await assert.rejects(
        () => readiness(config, 'arbitrum-one-usdc-v1'),
        (error) => error.terminal === true && error.code === 'unsupported_bridge_route',
    );
    const sepoliaReuseConfig = path.join(root, 'sepolia-reuse-config.json');
    const sepoliaReuse = JSON.parse(fs.readFileSync(config, 'utf8'));
    sepoliaReuse.program_vkey = '0x0077f479ed28535dbb5035f455a875334bae7d5a1eaa7c22c6f070a404eab31f';
    writeJson(sepoliaReuseConfig, sepoliaReuse);
    await assert.rejects(
        () => readiness(sepoliaReuseConfig, ROUTE_ID),
        (error) => error.terminal === true && error.code === 'driver_config_invalid',
    );

    const jobDir = path.join(root, 'job-a');
    const job = makeJob(jobDir);
    await assert.rejects(
        () => runJob(job, config),
        (error) => error.terminal === false && error.code === 'prover_temporarily_unavailable',
    );
    let state = JSON.parse(fs.readFileSync(path.join(jobDir, 'worker-state.json'), 'utf8'));
    assert.strictEqual(state.status, 'proving');
    assert.strictEqual(state.retryable, true);
    assert.strictEqual(fs.readdirSync(path.join(jobDir, 'checkpoints')).length, 3);

    writeJson(config, { schema: 'operator-config-rotated-after-banked-deposit' });
    const accepted = await runJob(job, config);
    assert.strictEqual(accepted.status, 'accepted');
    state = JSON.parse(fs.readFileSync(path.join(jobDir, 'worker-state.json'), 'utf8'));
    assert.strictEqual(state.status, 'accepted');
    assert.strictEqual(state.receipt_code, 'ACCEPTED');
    assert.match(state.terminal_checkpoint_sha256, /^[0-9a-f]{64}$/);
    const counts = JSON.parse(fs.readFileSync(path.join(jobDir, 'mock-counts.json'), 'utf8'));
    assert.deepStrictEqual(counts, {
        confirming_deposit: 1,
        waiting_for_ethereum_finality: 1,
        capturing_state_proof: 1,
        proving: 2,
        verifying: 1,
        claiming: 1,
    });

    await runJob(job, config);
    assert.deepStrictEqual(
        JSON.parse(fs.readFileSync(path.join(jobDir, 'mock-counts.json'), 'utf8')),
        counts,
        'accepted restart must reuse every banked checkpoint',
    );

    const checkpoint = path.join(jobDir, 'checkpoints', '01-waiting_for_ethereum_finality.json');
    const tampered = JSON.parse(fs.readFileSync(checkpoint, 'utf8'));
    tampered.result.finalized_block_number += 1;
    writeJson(checkpoint, tampered);
    await assert.rejects(
        () => runJob(job, config),
        (error) => error.terminal === true && error.code === 'bridge_checkpoint_invalid',
    );
    state = JSON.parse(fs.readFileSync(path.join(jobDir, 'worker-state.json'), 'utf8'));
    assert.strictEqual(state.status, 'failed');
    assert.strictEqual(state.retryable, false);

    makeConfig(root);
    const migrationJobDir = path.join(root, 'job-cap-migration');
    const migrationJob = makeLegacyCapJob(migrationJobDir, config);
    const migration = migrateLegacyCapJob(migrationJob.jobFile, config, {
        expectedLegacyConfigSha256: migrationJob.configSha256,
    });
    assert.strictEqual(migration.ethereum_transaction_replay_allowed, false);
    assert.strictEqual(migration.archived_checkpoints.length, 6);
    assert.deepStrictEqual(fs.readdirSync(path.join(migrationJobDir, 'checkpoints')), []);
    assert.ok(fs.existsSync(path.join(
        migrationJobDir, 'migrations', migration.migration_id, 'driver-config.snapshot.json',
    )));
    fs.writeFileSync(path.join(migrationJobDir, 'proving-failed-once'), '1');
    const migratedAccepted = await runJob(migrationJob.jobFile, config);
    assert.strictEqual(migratedAccepted.status, 'accepted');
    assert.strictEqual(
        JSON.parse(fs.readFileSync(path.join(migrationJobDir, 'worker-state.json'), 'utf8')).status,
        'accepted',
    );

    const badJobDir = path.join(root, 'job-b');
    const badJob = makeJob(badJobDir, '12');
    process.env.MOCK_BAD_STAGE = 'confirming_deposit';
    try {
        await assert.rejects(
            () => runJob(badJob, config),
            (error) => error.terminal === true && error.code === 'driver_result_binding_failed',
        );
    } finally {
        delete process.env.MOCK_BAD_STAGE;
    }
    state = JSON.parse(fs.readFileSync(path.join(badJobDir, 'worker-state.json'), 'utf8'));
    assert.strictEqual(state.status, 'failed');
    assert.strictEqual(state.code, 'driver_result_binding_failed');
    assert.ok(!fs.existsSync(path.join(badJobDir, 'checkpoints', '00-confirming_deposit.json')));

    fs.rmSync(root, { recursive: true, force: true });
    console.log('ETH fast-lane durable driver retry/resume/checkpoint/binding tests passed');
}

main().catch((error) => {
    delete process.env.MOCK_BAD_STAGE;
    fs.rmSync(root, { recursive: true, force: true });
    console.error(error);
    process.exitCode = 1;
});
