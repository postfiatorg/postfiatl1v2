'use strict';

const assert = require('assert');
const { EventEmitter } = require('events');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { keccak256 } = require('./keccak256');
const { canonicalJobKey, create, normalizeRoute } = require('./trustless-bridge-jobs');
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
        driver_args: ['run-job', '--job-file', '{job_file}', '--work-dir', '{job_dir}'],
        readiness_bin: process.execPath,
        readiness_sha256: fileSha256(process.execPath),
        readiness_args: ['readiness', '--route', '{route_id}'],
        cwd: root,
    },
    {
        route_id: 'arbitrum-one-usdc-v1',
        source_chain_id: 42161,
        source_proof_kind: 'sp1-arbitrum-bonded-v1',
        program_vkey: `0x${'7b'.repeat(32)}`,
        manifest_hash: '7c'.repeat(32),
        route_profile_hash: '7d'.repeat(48),
        asset_id: '7e'.repeat(48),
        vault_address: `0x${'21'.repeat(20)}`,
        vault_runtime_code_hash: `0x${'22'.repeat(32)}`,
        token_address: `0x${'23'.repeat(20)}`,
        token_runtime_code_hash: `0x${'24'.repeat(32)}`,
        driver_bin: process.execPath,
        driver_sha256: fileSha256(process.execPath),
        driver_args: ['run-job', '--job-file', '{job_file}'],
        readiness_bin: process.execPath,
        readiness_sha256: fileSha256(process.execPath),
        readiness_args: ['readiness', '--route', '{route_id}'],
        cwd: root,
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
assert.throws(
    () => normalizeRoute({
        ...routeRows[0],
        program_vkey: '0x0077f479ed28535dbb5035f455a875334bae7d5a1eaa7c22c6f070a404eab31f',
    }),
    /Sepolia P0 verifier identity cannot authorize/,
);
assert.throws(
    () => normalizeRoute({
        ...routeRows[0],
        manifest_hash: 'dc409b424e7627b936d81a16d2fc8f4c17e21a108d654be6b992e552d7b0c6d3',
    }),
    /Sepolia P0 verifier identity cannot authorize/,
);
assert.throws(
    () => normalizeRoute({ ...routeRows[0], max_amount_atoms: '5000000' }),
    /configures retired max_amount_atoms/,
    'operators must not be able to reintroduce a route-specific business cap',
);

function readinessPayload(routeId) {
    const route = routeRows.find((row) => row.route_id === routeId);
    return {
        ok: true,
        ready: true,
        route_id: route.route_id,
        source_chain_id: route.source_chain_id,
        source_proof_kind: route.source_proof_kind,
        program_vkey: route.program_vkey,
        manifest_hash: route.manifest_hash,
        route_profile_hash: route.route_profile_hash,
        asset_id: route.asset_id,
        vault_address: route.vault_address,
        vault_runtime_code_hash: route.vault_runtime_code_hash,
        token_address: route.token_address,
        token_runtime_code_hash: route.token_runtime_code_hash,
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

async function assertFailedReadinessRefreshesBeforeHealthyCacheExpiry() {
    const caseRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-bridge-ready-recover-'));
    let clock = 2_000_000;
    let calls = 0;
    const subject = create({}, {
        root: caseRoot,
        routes: [routeRows[0]],
        now: () => clock,
        execFileAsync: async () => {
            calls += 1;
            if (calls === 1) throw new Error('temporary preflight failure');
            return { stdout: JSON.stringify(readinessPayload('ethereum-mainnet-usdc-v1')) };
        },
        spawn: spawnMock,
        readinessRefreshMs: 60_000,
        readinessMaxAgeMs: 120_000,
        readinessFailureMaxAgeMs: 1_000,
        watchdogMs: 60_000,
    });
    try {
        const failed = await subject.refreshTrustlessBridgeReadiness(
            'ethereum-mainnet-usdc-v1',
        );
        assert.strictEqual(failed.ready, false);
        clock += 1_001;
        const warming = await subject.trustlessBridgeReadiness(
            'ethereum-mainnet-usdc-v1',
        );
        assert.strictEqual(warming.code, 'trustless_readiness_warming');
        await new Promise((resolve) => setImmediate(resolve));
        const recovered = await subject.trustlessBridgeReadiness(
            'ethereum-mainnet-usdc-v1',
        );
        assert.strictEqual(recovered.ready, true);
        assert.strictEqual(calls, 2);
    } finally {
        subject.closeTrustlessBridgeJobs();
        fs.rmSync(caseRoot, { recursive: true, force: true });
    }
}

async function assertSixValidatorIngressPreflight() {
    const caseRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-bridge-claim-preflight-'));
    const fleet = Array.from({ length: 6 }, (_, index) => ({
        validatorId: `validator-${index}`,
        host: '127.0.0.1',
        port: 41000 + index,
    }));
    const request = {
        route_id: routeRows[0].route_id,
        pftl_recipient: `pf${'31'.repeat(20)}`,
        depositor: `0x${'32'.repeat(20)}`,
        amount_atoms: '15000000',
    };
    const report = {
        schema: 'postfiat.pfusdc_ingress_preflight.v1',
        ready: true,
        code: 'ready',
        explanation: 'exact claim passes',
        route_id: routeRows[0].route_id,
        asset_id: routeRows[0].asset_id,
        route_profile_hash: routeRows[0].route_profile_hash,
        pftl_recipient: request.pftl_recipient,
        ethereum_depositor: request.depositor,
        amount_atoms: 15000000,
        state_root: '33'.repeat(48),
        quote_digest: '34'.repeat(32),
        quote_height: 900,
        expires_at_height: 908,
        orchard_aware_claim_active: true,
        source_series_active: true,
    };
    const subject = create({}, {
        root: caseRoot,
        routes: [routeRows[0]],
        execFileAsync,
        spawn: spawnMock,
        rpcFleet: fleet,
        rpcRequester: async () => ({ ok: true, result: report }),
        requireIngressPreflight: true,
        readinessRefreshMs: 60_000,
        readinessMaxAgeMs: 120_000,
        watchdogMs: 60_000,
    });
    try {
        await subject.refreshTrustlessBridgeReadiness(routeRows[0].route_id);
        const result = await subject.pfusdcIngressPreflight(request);
        assert.strictEqual(result.ready, true);
        assert.strictEqual(result.validator_count, 6);
        assert.strictEqual(result.quote_digest, report.quote_digest);
        assert.deepStrictEqual(result.validator_ids, fleet.map((row) => row.validatorId));
    } finally {
        subject.closeTrustlessBridgeJobs();
        fs.rmSync(caseRoot, { recursive: true, force: true });
    }
}

async function assertConcurrentSubmissionSafety() {
    const caseRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-bridge-submit-race-'));
    const localSpawns = [];
    let pid = 800_000;
    const subject = create({}, {
        root: caseRoot,
        routes: [routeRows[0]],
        now: () => nowMs,
        execFileAsync,
        spawn: (bin, args) => {
            const child = new EventEmitter();
            child.pid = pid += 1;
            child.unref = () => {};
            localSpawns.push({ bin, args, child });
            return child;
        },
        readinessRefreshMs: 60_000,
        readinessMaxAgeMs: 120_000,
        watchdogMs: 60_000,
    });
    try {
        await subject.refreshTrustlessBridgeReadiness('ethereum-mainnet-usdc-v1');
        const base = {
            route_id: 'ethereum-mainnet-usdc-v1',
            source_chain_id: 1,
            deposit_tx_hash: `0x${'91'.repeat(32)}`,
            deposit_id: `0x${'92'.repeat(32)}`,
            pftl_recipient: `pf${'93'.repeat(20)}`,
            depositor: `0x${'94'.repeat(20)}`,
            amount_atoms: '2000000',
        };
        const same = await Promise.all([
            subject.submitTrustlessBridgeJob({ ...base, idempotency_key: 'concurrent-same-a' }),
            subject.submitTrustlessBridgeJob({ ...base, idempotency_key: 'concurrent-same-b' }),
        ]);
        assert.strictEqual(same[0].job_id, same[1].job_id);
        assert.deepStrictEqual(
            same.map((row) => row.idempotent_replay).sort(),
            [false, true],
        );
        assert.strictEqual(localSpawns.length, 1, 'concurrent replay must spawn one worker');

        const conflictBase = {
            ...base,
            deposit_tx_hash: `0x${'95'.repeat(32)}`,
            deposit_id: `0x${'96'.repeat(32)}`,
        };
        const conflict = await Promise.allSettled([
            subject.submitTrustlessBridgeJob({
                ...conflictBase, amount_atoms: '1000000', idempotency_key: 'concurrent-conflict-a',
            }),
            subject.submitTrustlessBridgeJob({
                ...conflictBase, amount_atoms: '2000000', idempotency_key: 'concurrent-conflict-b',
            }),
        ]);
        assert.strictEqual(conflict.filter((row) => row.status === 'fulfilled').length, 1);
        const rejected = conflict.find((row) => row.status === 'rejected');
        assert.strictEqual(rejected.reason.code, 'bridge_job_binding_conflict');
        assert.strictEqual(localSpawns.length, 2, 'conflicting concurrent bindings must still spawn once');
    } finally {
        subject.closeTrustlessBridgeJobs();
        fs.rmSync(caseRoot, { recursive: true, force: true });
    }
}

async function runTwoProxySubmissionRace(requestA, requestB) {
    const caseRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-bridge-two-proxy-race-'));
    const localSpawns = [];
    let releaseCreation;
    let markCreationEntered;
    const creationEntered = new Promise((resolve) => { markCreationEntered = resolve; });
    const creationGate = new Promise((resolve) => { releaseCreation = resolve; });
    const common = {
        root: caseRoot,
        routes: [routeRows[0]],
        execFileAsync,
        spawn: (bin, args) => {
            const child = new EventEmitter();
            child.pid = process.pid;
            child.unref = () => {};
            localSpawns.push({ bin, args, child });
            return child;
        },
        readinessRefreshMs: 60_000,
        readinessMaxAgeMs: 120_000,
        watchdogMs: 60_000,
        submissionLockTimeoutMs: 2_000,
        submissionLockPollMs: 10,
        submissionLockStaleMs: 4_000,
    };
    const first = create({}, {
        ...common,
        beforeCreateJob: async () => {
            markCreationEntered();
            await creationGate;
        },
    });
    const second = create({}, common);
    try {
        await Promise.all([
            first.refreshTrustlessBridgeReadiness('ethereum-mainnet-usdc-v1'),
            second.refreshTrustlessBridgeReadiness('ethereum-mainnet-usdc-v1'),
        ]);
        const firstSubmission = first.submitTrustlessBridgeJob(requestA);
        await creationEntered;
        const secondSubmission = second.submitTrustlessBridgeJob(requestB);
        await new Promise((resolve) => setTimeout(resolve, 30));
        releaseCreation();
        const results = await Promise.allSettled([firstSubmission, secondSubmission]);
        return { results, spawn_count: localSpawns.length };
    } finally {
        releaseCreation();
        first.closeTrustlessBridgeJobs();
        second.closeTrustlessBridgeJobs();
        fs.rmSync(caseRoot, { recursive: true, force: true });
    }
}

async function assertCrossProcessSubmissionSafety() {
    const base = {
        route_id: 'ethereum-mainnet-usdc-v1',
        source_chain_id: 1,
        deposit_tx_hash: `0x${'a1'.repeat(32)}`,
        deposit_id: `0x${'a2'.repeat(32)}`,
        pftl_recipient: `pf${'a3'.repeat(20)}`,
        depositor: `0x${'a4'.repeat(20)}`,
        amount_atoms: '2000000',
    };
    const replay = await runTwoProxySubmissionRace(
        { ...base, idempotency_key: 'two-proxy-replay-a' },
        { ...base, idempotency_key: 'two-proxy-replay-b' },
    );
    assert.strictEqual(replay.results.filter((row) => row.status === 'fulfilled').length, 2);
    assert.deepStrictEqual(
        replay.results.map((row) => row.value.idempotent_replay).sort(),
        [false, true],
    );
    assert.strictEqual(replay.spawn_count, 1, 'overlapping proxies must spawn one worker');

    const conflictBase = {
        ...base,
        deposit_tx_hash: `0x${'a5'.repeat(32)}`,
        deposit_id: `0x${'a6'.repeat(32)}`,
    };
    const conflict = await runTwoProxySubmissionRace(
        { ...conflictBase, amount_atoms: '1000000', idempotency_key: 'two-proxy-conflict-a' },
        { ...conflictBase, amount_atoms: '2000000', idempotency_key: 'two-proxy-conflict-b' },
    );
    assert.strictEqual(conflict.results.filter((row) => row.status === 'fulfilled').length, 1);
    assert.strictEqual(
        conflict.results.find((row) => row.status === 'rejected').reason.code,
        'bridge_job_binding_conflict',
    );
    assert.strictEqual(conflict.spawn_count, 1, 'conflicting overlapping proxies must spawn once');
}

async function assertOrphanWorkerSupervision() {
    const caseRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-bridge-orphan-worker-'));
    const route = { ...routeRows[0], worker_timeout_ms: 60_000 };
    let clock = 10_000;
    let alive = true;
    let processToken = '424242';
    let nextWorkerPid = 900_000;
    const spawns = [];
    const kills = [];
    const options = {
        root: caseRoot,
        routes: [route],
        now: () => clock,
        wallNow: () => clock,
        processAlive: (pid) => alive && Number.isInteger(pid) && pid > 1,
        processStartToken: () => processToken,
        killProcess: (pid, signal) => { kills.push({ pid, signal }); },
        execFileAsync,
        spawn: (bin, args) => {
            const child = new EventEmitter();
            child.pid = nextWorkerPid += 1;
            child.unref = () => {};
            spawns.push({ bin, args, child });
            return child;
        },
        readinessRefreshMs: 60_000,
        readinessMaxAgeMs: 120_000,
        retryBaseMs: 1_000,
        retryMaxMs: 8_000,
        watchdogMs: 60_000,
    };
    const request = {
        route_id: route.route_id,
        source_chain_id: route.source_chain_id,
        deposit_tx_hash: `0x${'c1'.repeat(32)}`,
        deposit_id: `0x${'c2'.repeat(32)}`,
        pftl_recipient: `pf${'c3'.repeat(20)}`,
        depositor: `0x${'c4'.repeat(20)}`,
        amount_atoms: '2000000',
        idempotency_key: 'orphan-worker-test',
    };
    const first = create({}, options);
    try {
        await first.refreshTrustlessBridgeReadiness(route.route_id);
        const job = await first.submitTrustlessBridgeJob(request);
        assert.strictEqual(spawns.length, 1);
        first.closeTrustlessBridgeJobs();

        clock += 60_001;
        const restarted = create({}, options);
        try {
            assert.deepStrictEqual(kills.map((row) => row.signal), ['SIGTERM']);
            clock += 5_001;
            restarted.trustlessBridgeJobStatus(job.job_id);
            assert.deepStrictEqual(kills.map((row) => row.signal), ['SIGTERM', 'SIGKILL']);

            alive = false;
            const deferred = restarted.trustlessBridgeJobStatus(job.job_id);
            assert.strictEqual(deferred.last_worker_exit.signal, 'orphan-worker-timeout');
            assert.strictEqual(spawns.length, 1);
            clock += 1_000;
            restarted.trustlessBridgeJobStatus(job.job_id);
            assert.strictEqual(spawns.length, 2, 'timed-out orphan must resume after bounded backoff');

            restarted.closeTrustlessBridgeJobs();
            alive = true;
            processToken = 'recycled-pid';
            const recovered = create({}, options);
            try {
                const reused = recovered.trustlessBridgeJobStatus(job.job_id);
                assert.strictEqual(reused.last_worker_exit.signal, 'worker-pid-reused');
                assert.strictEqual(kills.length, 2, 'a recycled PID must never be signalled');
                clock += 2_000;
                recovered.trustlessBridgeJobStatus(job.job_id);
                assert.strictEqual(spawns.length, 3, 'PID reuse recovery must resume after backoff');
            } finally {
                recovered.closeTrustlessBridgeJobs();
            }
        } finally {
            restarted.closeTrustlessBridgeJobs();
        }
    } finally {
        first.closeTrustlessBridgeJobs();
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
    workerLogMaxBytes: 1_024,
    workerLogRetention: 2,
    workerStateQuarantineRetention: 2,
});

async function main() {
    await assertSixValidatorIngressPreflight();
    await assertFailedReadinessRefreshesBeforeHealthyCacheExpiry();
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
    assert.strictEqual(spawns[0].bin, process.execPath);
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
    const workerLog = path.join(root, 'jobs', expectedJobId.slice(2), 'worker.log');
    fs.writeFileSync(workerLog, Buffer.alloc(2_048, 0x61), { mode: 0o600 });
    nowMs += 1_000;
    bridge.trustlessBridgeJobStatus(expectedJobId);
    assert.strictEqual(spawns.length, 2, 'banked job must resume after bounded backoff');
    assert.strictEqual(fs.statSync(`${workerLog}.1`).size, 2_048);
    assert.strictEqual(fs.statSync(workerLog).size, 0);

    const stateFile = path.join(root, 'jobs', expectedJobId.slice(2), 'worker-state.json');
    const acceptedState = {
        schema: 'postfiat-trustless-bridge-worker-state-v2',
        status: 'accepted',
        route_id: routeRows[0].route_id,
        source_chain_id: routeRows[0].source_chain_id,
        source_proof_kind: routeRows[0].source_proof_kind,
        program_vkey: routeRows[0].program_vkey,
        manifest_hash: routeRows[0].manifest_hash,
        route_profile_hash: routeRows[0].route_profile_hash,
        asset_id: routeRows[0].asset_id,
        observer_attestor_enabled: false,
        updated_at_unix: Math.floor(nowMs / 1000),
        retryable: false,
        receipt_code: 'ACCEPTED',
        receipt_id: `0x${'b1'.repeat(32)}`,
        terminal_checkpoint_sha256: 'b2'.repeat(32),
    };
    let quarantined;
    for (let index = 0; index < 3; index += 1) {
        fs.writeFileSync(stateFile, `${JSON.stringify({
            ...acceptedState,
            observer_attestor_enabled: true,
            receipt_id: `0x${String(index + 1).padStart(2, '0').repeat(32)}`,
        })}\n`, { mode: 0o600 });
        quarantined = bridge.trustlessBridgeJobStatus(expectedJobId);
        assert.notStrictEqual(quarantined.status, 'accepted');
    }
    assert.strictEqual(
        fs.readdirSync(path.dirname(stateFile)).filter((name) => name.startsWith('worker-state.invalid.')).length,
        2,
        'forged worker-state quarantine retention must remain bounded',
    );
    fs.writeFileSync(stateFile, `${JSON.stringify({
        ...acceptedState,
        job_id: `0x${'ff'.repeat(32)}`,
        request: { pftl_recipient: `pf${'ff'.repeat(20)}` },
    })}\n`, { mode: 0o600 });
    spawns[1].child.emit('exit', 0, null);
    const accepted = bridge.trustlessBridgeJobStatus(expectedJobId);
    assert.strictEqual(accepted.status, 'accepted');
    assert.strictEqual(accepted.receipt_code, 'ACCEPTED');
    assert.strictEqual(accepted.job_id, expectedJobId, 'worker state cannot override durable job identity');
    assert.strictEqual(accepted.request.pftl_recipient, request.pftl_recipient);
    assert.strictEqual(spawns.length, 2);

    const arbRequest = {
        ...request,
        route_id: 'arbitrum-one-usdc-v1',
        source_chain_id: 42161,
        idempotency_key: 'arb-deposit-1',
    };
    const arb = await bridge.submitTrustlessBridgeJob(arbRequest);
    assert.notStrictEqual(arb.job_id, expectedJobId, 'route and chain must domain-separate job IDs');
    assert.strictEqual(spawns.at(-1).bin, process.execPath);

    const protocolMaximumRequest = {
        ...request,
        deposit_tx_hash: `0x${'91'.repeat(32)}`,
        deposit_id: `0x${'92'.repeat(32)}`,
        amount_atoms: '18446744073709551615',
        idempotency_key: 'protocol-u64-maximum-deposit',
    };
    const protocolMaximum = await bridge.submitTrustlessBridgeJob(protocolMaximumRequest);
    assert.match(protocolMaximum.job_id, /^0x[0-9a-f]{64}$/);

    await assert.rejects(
        () => bridge.submitTrustlessBridgeJob({
            ...protocolMaximumRequest,
            deposit_tx_hash: `0x${'93'.repeat(32)}`,
            deposit_id: `0x${'94'.repeat(32)}`,
            amount_atoms: '18446744073709551616',
            idempotency_key: 'above-protocol-u64-maximum',
        }),
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
        ['wrong-program-vkey', (row) => ({ ...row, program_vkey: `0x${'8b'.repeat(32)}` })],
        ['wrong-manifest-hash', (row) => ({ ...row, manifest_hash: '8c'.repeat(32) })],
        ['wrong-route-profile', (row) => ({ ...row, route_profile_hash: '8d'.repeat(48) })],
        ['wrong-asset', (row) => ({ ...row, asset_id: '8e'.repeat(48) })],
        ['wrong-vault', (row) => ({ ...row, vault_address: `0x${'31'.repeat(20)}` })],
        ['wrong-vault-code', (row) => ({ ...row, vault_runtime_code_hash: `0x${'32'.repeat(32)}` })],
        ['wrong-token', (row) => ({ ...row, token_address: `0x${'33'.repeat(20)}` })],
        ['wrong-token-code', (row) => ({ ...row, token_runtime_code_hash: `0x${'34'.repeat(32)}` })],
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
    await assertConcurrentSubmissionSafety();
    await assertCrossProcessSubmissionSafety();
    await assertOrphanWorkerSupervision();

    fs.rmSync(root, { recursive: true, force: true });
    console.log('trustless bridge P2 route/readiness/idempotency/resume/backoff tests passed');
}

main().catch((error) => {
    bridge.closeTrustlessBridgeJobs();
    fs.rmSync(root, { recursive: true, force: true });
    console.error(error);
    process.exitCode = 1;
});
