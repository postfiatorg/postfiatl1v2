'use strict';

const assert = require('assert');
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { CONFIG_SCHEMA, STAGES } = require('./eth-fast-lane-driver');
const TEST_PROGRAM_VKEY = `0x${'6b'.repeat(32)}`;
const TEST_MANIFEST_HASH = '6c'.repeat(32);
const TEST_ROUTE_PROFILE_HASH = '6d'.repeat(48);
const TEST_ASSET_ID = '6e'.repeat(48);
const TEST_VAULT = `0x${'11'.repeat(20)}`;
const TEST_VAULT_CODE_HASH = `0x${'12'.repeat(32)}`;
const TEST_TOKEN = `0x${'13'.repeat(20)}`;
const TEST_TOKEN_CODE_HASH = `0x${'14'.repeat(32)}`;

function fileSha256(file) {
    return require('crypto').createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-bridge-http-v2-'));
const adapter = path.join(root, 'eth-fast-lane-driver.js');
fs.copyFileSync(path.join(__dirname, 'eth-fast-lane-driver.js'), adapter);
fs.chmodSync(adapter, 0o700);
const stageDriver = path.join(root, 'stage-driver.js');
fs.writeFileSync(stageDriver, `
'use strict';
const fs = require('fs');
const args = process.argv.slice(2);
if (args[0] === 'readiness') {
  process.stdout.write(JSON.stringify({
    ok: true, ready: true, route_id: 'ethereum-mainnet-usdc-v1', source_chain_id: 1,
    source_proof_kind: 'sp1-ethereum-finality-v1',
    program_vkey: '${TEST_PROGRAM_VKEY}', manifest_hash: '${TEST_MANIFEST_HASH}',
    route_profile_hash: '${TEST_ROUTE_PROFILE_HASH}', asset_id: '${TEST_ASSET_ID}',
    vault_address: '${TEST_VAULT}', vault_runtime_code_hash: '${TEST_VAULT_CODE_HASH}',
    token_address: '${TEST_TOKEN}', token_runtime_code_hash: '${TEST_TOKEN_CODE_HASH}',
    observer_attestor_enabled: false, prover_authenticated: true,
    prover_healthy: true, route_manifest_active: true,
    program_vkey_active: true, nav_cap_growth_enabled: true,
    vault_paused: false, vault_code_hash_matches: true,
    token_code_hash_matches: true, execution_rpc_sources_reachable: 2,
    beacon_finality_current: true
  }));
  process.exit(0);
}
const stage = args[0];
const file = args[args.indexOf('--job-file') + 1];
const r = JSON.parse(fs.readFileSync(file, 'utf8')).request;
const out = {
  ok: true, stage, route_id: r.route_id, source_chain_id: r.source_chain_id,
  source_proof_kind: 'sp1-ethereum-finality-v1', deposit_tx_hash: r.deposit_tx_hash,
  program_vkey: '${TEST_PROGRAM_VKEY}', manifest_hash: '${TEST_MANIFEST_HASH}',
  route_profile_hash: '${TEST_ROUTE_PROFILE_HASH}', asset_id: '${TEST_ASSET_ID}',
  vault_address: '${TEST_VAULT}', vault_runtime_code_hash: '${TEST_VAULT_CODE_HASH}',
  token_address: '${TEST_TOKEN}', token_runtime_code_hash: '${TEST_TOKEN_CODE_HASH}',
  deposit_id: r.deposit_id, pftl_recipient: r.pftl_recipient,
  depositor: r.depositor, amount_atoms: r.amount_atoms
};
if (stage === 'confirming_deposit') out.deposit_confirmed = true;
if (stage === 'waiting_for_ethereum_finality') {
  out.ethereum_finalized = true; out.finalized_block_hash = '0x' + '55'.repeat(32);
  out.finalized_block_number = 123456;
}
if (stage === 'capturing_state_proof') {
  out.witness_sha256 = '66'.repeat(32); out.evidence_root = '0x' + '67'.repeat(48);
  out.nullifier = '0x' + '68'.repeat(32);
}
if (stage === 'proving') {
  out.proof_sha256 = '69'.repeat(32); out.public_values_sha256 = '6a'.repeat(32);
  out.program_vkey = '0x' + '6b'.repeat(32);
}
if (stage === 'verifying') out.proof_verified = true;
if (stage === 'claiming') {
  out.receipt_code = 'ACCEPTED'; out.receipt_id = '77'.repeat(48);
  out.tx_id = '0x' + '78'.repeat(32);
}
process.stdout.write(JSON.stringify(out));
`, { mode: 0o700 });
const driverConfig = path.join(root, 'driver-config.json');
fs.writeFileSync(driverConfig, `${JSON.stringify({
    schema: CONFIG_SCHEMA,
    route_id: 'ethereum-mainnet-usdc-v1',
    source_chain_id: 1,
    source_proof_kind: 'sp1-ethereum-finality-v1',
    program_vkey: TEST_PROGRAM_VKEY,
    manifest_hash: TEST_MANIFEST_HASH,
    route_profile_hash: TEST_ROUTE_PROFILE_HASH,
    asset_id: TEST_ASSET_ID,
    vault_address: TEST_VAULT,
    vault_runtime_code_hash: TEST_VAULT_CODE_HASH,
    token_address: TEST_TOKEN,
    token_runtime_code_hash: TEST_TOKEN_CODE_HASH,
    pinned_files: [{ path: stageDriver, sha256: fileSha256(stageDriver) }],
    readiness: {
        program: process.execPath,
        program_sha256: fileSha256(process.execPath),
        args: [stageDriver, 'readiness'],
        timeout_ms: 5_000,
    },
    stages: STAGES.map((stage) => ({
        stage,
        program: process.execPath,
        program_sha256: fileSha256(process.execPath),
        args: [stageDriver, stage, '--job-file', '{job_file}'],
        timeout_ms: 5_000,
    })),
}, null, 2)}\n`, { mode: 0o600 });

process.env.WALLET_PROXY_API_TOKEN = 'test-only-wallet-proxy-token-32-bytes-minimum';
process.env.TRUSTLESS_BRIDGE_JOB_ROOT = root;
process.env.TRUSTLESS_BRIDGE_READINESS_REFRESH_MS = '60000';
process.env.TRUSTLESS_BRIDGE_READINESS_MAX_AGE_MS = '120000';
// This fixture exercises the durable HTTP/job adapter in isolation. Exact
// six-validator terminal-claim admission has its own deterministic unit test.
process.env.TRUSTLESS_BRIDGE_REQUIRE_INGRESS_PREFLIGHT = 'false';
process.env.TRUSTLESS_BRIDGE_ROUTES_JSON = JSON.stringify([{
    route_id: 'ethereum-mainnet-usdc-v1',
    source_chain_id: 1,
    source_proof_kind: 'sp1-ethereum-finality-v1',
    program_vkey: TEST_PROGRAM_VKEY,
    manifest_hash: TEST_MANIFEST_HASH,
    route_profile_hash: TEST_ROUTE_PROFILE_HASH,
    asset_id: TEST_ASSET_ID,
    vault_address: TEST_VAULT,
    vault_runtime_code_hash: TEST_VAULT_CODE_HASH,
    token_address: TEST_TOKEN,
    token_runtime_code_hash: TEST_TOKEN_CODE_HASH,
    driver_bin: process.execPath,
    driver_sha256: fileSha256(process.execPath),
    driver_args: [adapter, 'run-job', '--config', driverConfig, '--job-file', '{job_file}'],
    readiness_bin: process.execPath,
    readiness_sha256: fileSha256(process.execPath),
    pinned_files: [{ path: adapter, sha256: fileSha256(adapter) }],
    readiness_args: [adapter, 'readiness', '--config', driverConfig, '--route', '{route_id}'],
    cwd: root,
}]);

const { server } = require('./server');

function request(port, method, requestPath, body, authenticated = false) {
    return new Promise((resolve, reject) => {
        const encoded = body === undefined ? '' : JSON.stringify(body);
        const req = http.request({
            host: '127.0.0.1',
            port,
            path: requestPath,
            method,
            headers: {
                ...(encoded ? {
                    'content-type': 'application/json',
                    'content-length': Buffer.byteLength(encoded),
                } : {}),
                ...(authenticated ? {
                    authorization: `Bearer ${process.env.WALLET_PROXY_API_TOKEN}`,
                    origin: 'http://localhost:5173',
                } : {}),
            },
        }, (res) => {
            let raw = '';
            res.setEncoding('utf8');
            res.on('data', (chunk) => { raw += chunk; });
            res.on('end', () => resolve({ status: res.statusCode, body: JSON.parse(raw) }));
        });
        req.on('error', reject);
        req.end(encoded);
    });
}

async function waitForReady(port) {
    let lastResponse = null;
    for (let attempt = 0; attempt < 250; attempt += 1) {
        const response = await request(
            port,
            'GET',
            '/api/bridge/readiness?route=ethereum-mainnet-usdc-v1',
        );
        lastResponse = response;
        if (response.body.ready === true) return response;
        await new Promise((resolve) => setTimeout(resolve, 20));
    }
    throw new Error(`route readiness did not prewarm: ${JSON.stringify(lastResponse?.body || {})}`);
}

async function main() {
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const port = server.address().port;
    try {
        const ready = await waitForReady(port);
        assert.strictEqual(ready.status, 200);
        assert.strictEqual(ready.body.source_proof_kind, 'sp1-ethereum-finality-v1');
        assert.strictEqual(ready.body.observer_attestor_enabled, false);

        const body = {
            route_id: 'ethereum-mainnet-usdc-v1',
            source_chain_id: 1,
            deposit_tx_hash: `0x${'11'.repeat(32)}`,
            deposit_id: `0x${'22'.repeat(32)}`,
            pftl_recipient: `pf${'33'.repeat(20)}`,
            depositor: `0x${'44'.repeat(20)}`,
            amount_atoms: '2000000',
            idempotency_key: 'http-e2e-deposit-1',
        };
        const unauthenticated = await request(port, 'POST', '/api/bridge/jobs', body);
        assert.strictEqual(unauthenticated.status, 403);

        const unauthenticatedList = await request(
            port,
            'GET',
            `/api/bridge/jobs?recipient=${encodeURIComponent(body.pftl_recipient)}`,
        );
        assert.strictEqual(unauthenticatedList.status, 403);

        const created = await request(port, 'POST', '/api/bridge/jobs', body, true);
        assert.strictEqual(created.status, 202);
        assert.match(created.body.job_id, /^0x[0-9a-f]{64}$/);
        let status;
        for (let attempt = 0; attempt < 250; attempt += 1) {
            status = await request(port, 'GET', `/api/bridge/jobs/${created.body.job_id}`);
            if (status.body.status === 'accepted') break;
            await new Promise((resolve) => setTimeout(resolve, 20));
        }
        assert.strictEqual(status.status, 200);
        assert.strictEqual(status.body.status, 'accepted');
        assert.strictEqual(status.body.receipt_code, 'ACCEPTED');
        assert.match(status.body.terminal_checkpoint_sha256, /^[0-9a-f]{64}$/);
        const checkpointDir = path.join(
            root, 'jobs', created.body.job_id.slice(2), 'checkpoints',
        );
        assert.strictEqual(fs.readdirSync(checkpointDir).length, STAGES.length);

        const listed = await request(
            port,
            'GET',
            `/api/bridge/jobs?recipient=${encodeURIComponent(body.pftl_recipient)}`,
            undefined,
            true,
        );
        assert.strictEqual(listed.status, 200);
        assert.strictEqual(listed.body.schema, 'postfiat-trustless-bridge-job-list-v1');
        assert.strictEqual(listed.body.jobs.length, 1);
        assert.strictEqual(listed.body.jobs[0].job_id, created.body.job_id);

        const replay = await request(port, 'POST', '/api/bridge/jobs', {
            ...body,
            idempotency_key: 'http-e2e-deposit-replay',
        }, true);
        assert.strictEqual(replay.status, 202);
        assert.strictEqual(replay.body.job_id, created.body.job_id);
        assert.strictEqual(replay.body.idempotent_replay, true);
    } finally {
        await new Promise((resolve) => server.close(resolve));
        fs.rmSync(root, { recursive: true, force: true });
    }
    console.log('trustless bridge P2 HTTP route/readiness/job/resume test passed');
}

main().catch((error) => {
    console.error(error);
    fs.rmSync(root, { recursive: true, force: true });
    process.exitCode = 1;
});
