'use strict';

const assert = require('assert');
const { EventEmitter } = require('events');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { keccak256 } = require('./keccak256');
const { canonicalJobKey, create, normalizeRoute } = require('./trustless-bridge-jobs');

assert.strictEqual(
    keccak256(Buffer.alloc(0)).toString('hex'),
    'c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470',
    'job IDs must use Ethereum Keccak-256, not FIPS SHA3-256',
);
assert.strictEqual(
    canonicalJobKey({
        route_id: 'ethereum-mainnet-usdc-v1',
        source_chain_id: 1,
        deposit_tx_hash: `0x${'11'.repeat(32)}`,
        deposit_id: `0x${'22'.repeat(32)}`,
    }),
    '0x0f2da596c24a3b0821017404f4161a6a80b72c8ac3f822c30942ef4928463cd6',
    'durable route-indexed job key must remain stable',
);

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-bridge-jobs-v2-'));
let nowMs = 1_000_000;
let nextPid = 700_000;
const spawns = [];
const readinessCalls = [];
const routeRows = [
    {
        route_id: 'ethereum-mainnet-usdc-v1',
        source_chain_id: 1,
        source_proof_kind: 'sp1-ethereum-finality-v1',
        driver_bin: '/mock/eth-driver',
        driver_args: ['run-job', '--job-file', '{job_file}', '--work-dir', '{job_dir}'],
        readiness_bin: '/mock/eth-driver',
        readiness_args: ['readiness', '--route', '{route_id}'],
        cwd: root,
        max_amount_atoms: '5000000',
    },
    {
        route_id: 'arbitrum-one-usdc-v1',
        source_chain_id: 42161,
        source_proof_kind: 'sp1-arbitrum-bonded-v1',
        driver_bin: '/mock/arb-driver',
        driver_args: ['run-job', '--job-file', '{job_file}'],
        readiness_bin: '/mock/arb-driver',
        readiness_args: ['readiness', '--route', '{route_id}'],
        cwd: root,
        max_amount_atoms: '5000000',
    },
];

assert.throws(
    () => normalizeRoute({ ...routeRows[0], source_chain_id: 42161 }),
    /unsupported trustless route\/chain\/proof binding/,
);
assert.throws(
    () => normalizeRoute({ ...routeRows[0], source_proof_kind: 'sp1-arbitrum-bonded-v1' }),
    /unsupported trustless route\/chain\/proof binding/,
);
assert.throws(
    () => normalizeRoute({ ...routeRows[0], route_id: 'operator-invented-route' }),
    /unsupported trustless route\/chain\/proof binding/,
);

function readinessPayload(routeId) {
    const route = routeRows.find((row) => row.route_id === routeId);
    return {
        ok: true,
        ready: true,
        route_id: route.route_id,
        source_chain_id: route.source_chain_id,
        source_proof_kind: route.source_proof_kind,
        observer_attestor_enabled: false,
        prover_authenticated: true,
        prover_healthy: true,
        route_manifest_active: true,
        program_vkey_active: true,
        nav_cap_growth_enabled: true,
        vault_paused: false,
        vault_code_hash_matches: true,
        token_code_hash_matches: true,
        execution_rpc_sources_reachable: route.source_chain_id === 1 ? 2 : 1,
        beacon_finality_current: route.source_chain_id === 1,
    };
}

async function execFileAsync(_bin, args) {
    const routeId = args[args.indexOf('--route') + 1];
    readinessCalls.push(routeId);
    return { stdout: JSON.stringify(readinessPayload(routeId)) };
}

function spawnMock(bin, args) {
    const child = new EventEmitter();
    child.pid = nextPid += 1;
    child.unref = () => {};
    spawns.push({ bin, args, child });
    return child;
}

async function assertEthReadinessFails(label, mutate, execError = null) {
    const caseRoot = fs.mkdtempSync(path.join(os.tmpdir(), `pft-bridge-ready-${label}-`));
    const subject = create({}, {
        root: caseRoot,
        routes: [routeRows[0]],
        now: () => nowMs,
        execFileAsync: async () => {
            if (execError) throw execError;
            return { stdout: JSON.stringify(mutate(readinessPayload('ethereum-mainnet-usdc-v1'))) };
        },
        spawn: spawnMock,
        readinessRefreshMs: 60_000,
        readinessMaxAgeMs: 120_000,
        watchdogMs: 60_000,
    });
    try {
        const result = await subject.refreshTrustlessBridgeReadiness('ethereum-mainnet-usdc-v1');
        assert.strictEqual(result.ready, false, `${label} must fail readiness`);
        assert.strictEqual(result.code, 'trustless_ingress_unavailable');
        assert.strictEqual(result.observer_attestor_enabled, false);
        return result;
    } finally {
        subject.closeTrustlessBridgeJobs();
        fs.rmSync(caseRoot, { recursive: true, force: true });
    }
}

const bridge = create({}, {
    root,
    routes: routeRows,
    now: () => nowMs,
    execFileAsync,
    spawn: spawnMock,
    readinessRefreshMs: 60_000,
    readinessMaxAgeMs: 120_000,
    retryBaseMs: 1_000,
    retryMaxMs: 8_000,
    watchdogMs: 60_000,
});

async function main() {
    const ethReady = await bridge.refreshTrustlessBridgeReadiness('ethereum-mainnet-usdc-v1');
    const arbReady = await bridge.refreshTrustlessBridgeReadiness('arbitrum-one-usdc-v1');
    assert.strictEqual(ethReady.ready, true);
    assert.strictEqual(ethReady.execution_rpc_sources_reachable, 2);
    assert.strictEqual(arbReady.ready, true);
    assert.ok(readinessCalls.includes('ethereum-mainnet-usdc-v1'));
    assert.ok(readinessCalls.includes('arbitrum-one-usdc-v1'));

    const request = {
        route_id: 'ethereum-mainnet-usdc-v1',
        source_chain_id: 1,
        deposit_tx_hash: `0x${'11'.repeat(32)}`,
        deposit_id: `0x${'22'.repeat(32)}`,
        pftl_recipient: `pf${'33'.repeat(20)}`,
        depositor: `0x${'44'.repeat(20)}`,
        amount_atoms: '2000000',
        idempotency_key: 'eth-mainnet-deposit-1',
    };
    const expectedJobId = canonicalJobKey(request);
    const created = await bridge.submitTrustlessBridgeJob(request);
    assert.strictEqual(created.job_id, expectedJobId);
    assert.strictEqual(created.status, 'queued');
    assert.strictEqual(created.observer_attestor_enabled, false);
    assert.strictEqual(spawns.length, 1);
    assert.strictEqual(spawns[0].bin, '/mock/eth-driver');
    assert.ok(spawns[0].args.includes(path.join(root, 'jobs', expectedJobId.slice(2), 'job.json')));

    const replay = await bridge.submitTrustlessBridgeJob({
        ...request,
        idempotency_key: 'eth-mainnet-deposit-replay',
    });
    assert.strictEqual(replay.job_id, expectedJobId);
    assert.strictEqual(replay.idempotent_replay, true);
    assert.strictEqual(spawns.length, 1, 'idempotent replay must not duplicate a live worker');

    spawns[0].child.emit('exit', 75, null);
    const deferred = bridge.trustlessBridgeJobStatus(expectedJobId);
    assert.strictEqual(deferred.retry_count, 1);
    assert.strictEqual(deferred.next_retry_at_ms, nowMs + 1_000);
    assert.strictEqual(spawns.length, 1, 'bounded backoff must suppress an immediate retry');
    nowMs += 1_000;
    bridge.trustlessBridgeJobStatus(expectedJobId);
    assert.strictEqual(spawns.length, 2, 'banked job must resume after bounded backoff');

    const stateFile = path.join(root, 'jobs', expectedJobId.slice(2), 'worker-state.json');
    fs.writeFileSync(stateFile, `${JSON.stringify({
        schema: 'postfiat-trustless-bridge-worker-state-v2',
        status: 'accepted',
        receipt_code: 'ACCEPTED',
    })}\n`, { mode: 0o600 });
    spawns[1].child.emit('exit', 0, null);
    const accepted = bridge.trustlessBridgeJobStatus(expectedJobId);
    assert.strictEqual(accepted.status, 'accepted');
    assert.strictEqual(accepted.receipt_code, 'ACCEPTED');
    assert.strictEqual(spawns.length, 2);

    const arbRequest = {
        ...request,
        route_id: 'arbitrum-one-usdc-v1',
        source_chain_id: 42161,
        idempotency_key: 'arb-deposit-1',
    };
    const arb = await bridge.submitTrustlessBridgeJob(arbRequest);
    assert.notStrictEqual(arb.job_id, expectedJobId, 'route and chain must domain-separate job IDs');
    assert.strictEqual(spawns.at(-1).bin, '/mock/arb-driver');

    await assert.rejects(
        () => bridge.submitTrustlessBridgeJob({ ...request, amount_atoms: '5000001' }),
        (error) => error.code === 'invalid_trustless_bridge_job',
    );
    await assert.rejects(
        () => bridge.submitTrustlessBridgeJob({ ...request, source_chain_id: 42161 }),
        (error) => error.code === 'invalid_trustless_bridge_job',
    );

    bridge.closeTrustlessBridgeJobs();

    const resumedSpawns = [];
    const restarted = create({}, {
        root,
        routes: routeRows,
        now: () => nowMs,
        execFileAsync,
        spawn: (bin, args) => {
            const child = spawnMock(bin, args);
            resumedSpawns.push({ child, bin, args });
            return child;
        },
        readinessRefreshMs: 60_000,
        readinessMaxAgeMs: 120_000,
        retryBaseMs: 1_000,
        retryMaxMs: 8_000,
        watchdogMs: 60_000,
    });
    assert.strictEqual(restarted.trustlessBridgeJobStatus(expectedJobId).status, 'accepted');
    assert.ok(
        resumedSpawns.every(({ args }) => !args.includes(
            path.join(root, 'jobs', expectedJobId.slice(2), 'job.json'),
        )),
        'accepted jobs must remain terminal after restart',
    );
    restarted.closeTrustlessBridgeJobs();

    const mutations = [
        ['wrong-chain', (row) => ({ ...row, source_chain_id: 42161 })],
        ['wrong-proof-kind', (row) => ({ ...row, source_proof_kind: 'sp1-arbitrum-bonded-v1' })],
        ['observer-enabled', (row) => ({ ...row, observer_attestor_enabled: true })],
        ['prover-unauthenticated', (row) => ({ ...row, prover_authenticated: false })],
        ['prover-unhealthy', (row) => ({ ...row, prover_healthy: false })],
        ['manifest-inactive', (row) => ({ ...row, route_manifest_active: false })],
        ['vkey-inactive', (row) => ({ ...row, program_vkey_active: false })],
        ['cap-growth-disabled', (row) => ({ ...row, nav_cap_growth_enabled: false })],
        ['vault-paused', (row) => ({ ...row, vault_paused: true })],
        ['vault-code-mismatch', (row) => ({ ...row, vault_code_hash_matches: false })],
        ['token-code-mismatch', (row) => ({ ...row, token_code_hash_matches: false })],
        ['single-execution-rpc', (row) => ({ ...row, execution_rpc_sources_reachable: 1 })],
        ['stale-beacon-finality', (row) => ({ ...row, beacon_finality_current: false })],
    ];
    for (const [label, mutate] of mutations) await assertEthReadinessFails(label, mutate);
    const privateFailure = await assertEthReadinessFails(
        'private-error-redaction',
        (row) => row,
        Object.assign(new Error('/private/operator/path/prover.token failed'), { code: 'ECONNREFUSED' }),
    );
    assert.ok(!privateFailure.message.includes('/private/'));
    assert.strictEqual(privateFailure.diagnostic_code, 'ECONNREFUSED');

    fs.rmSync(root, { recursive: true, force: true });
    console.log('trustless bridge P2 route/readiness/idempotency/resume/backoff tests passed');
}

main().catch((error) => {
    bridge.closeTrustlessBridgeJobs();
    fs.rmSync(root, { recursive: true, force: true });
    console.error(error);
    process.exitCode = 1;
});
