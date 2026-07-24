'use strict';

const crypto = require('crypto');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFile, spawn } = require('child_process');
const { promisify } = require('util');
const { keccak256 } = require('./keccak256');

const TX_RE = /^0x[0-9a-f]{64}$/;
const BYTES32_RE = /^(?:0x)?[0-9a-f]{64}$/;
const PFT_RE = /^pf[0-9a-f]{40}$/;
const EVM_RE = /^0x[0-9a-f]{40}$/;
const ROUTE_RE = /^[a-z0-9][a-z0-9-]{2,63}$/;
const TERMINAL_STAGES = new Set(['accepted', 'failed']);
const ETH_STAGES = new Set([
    'queued', 'confirming_deposit', 'waiting_for_ethereum_finality',
    'capturing_state_proof', 'proving', 'verifying', 'growing_backed_cap',
    'claiming', 'accepted', 'failed',
]);
const ARBITRUM_STAGES = new Set([
    'queued', 'confirming_deposit', 'waiting_for_arbitrum_finality',
    'capturing_state_proof', 'proving', 'verifying', 'growing_backed_cap',
    'claiming', 'accepted', 'failed',
]);
const DEFAULT_MAX_AMOUNT_ATOMS = 5_000_000n;
const ROUTE_PROFILES = new Map([
    ['ethereum-mainnet-usdc-v1', {
        source_chain_id: 1,
        source_proof_kind: 'sp1-ethereum-finality-v1',
    }],
    ['arbitrum-one-usdc-v1', {
        source_chain_id: 42161,
        source_proof_kind: 'sp1-arbitrum-bonded-v1',
    }],
]);

function stableJson(value) {
    if (Array.isArray(value)) return `[${value.map(stableJson).join(',')}]`;
    if (value && typeof value === 'object') {
        return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`).join(',')}}`;
    }
    return JSON.stringify(value);
}

function atomicWrite(file, value) {
    const temporary = `${file}.${process.pid}.${crypto.randomBytes(6).toString('hex')}.tmp`;
    const data = `${JSON.stringify(value, null, 2)}\n`;
    const fd = fs.openSync(temporary, 'wx', 0o600);
    try {
        fs.writeFileSync(fd, data);
        fs.fsyncSync(fd);
    } finally {
        fs.closeSync(fd);
    }
    fs.renameSync(temporary, file);
    const directoryFd = fs.openSync(path.dirname(file), 'r');
    try { fs.fsyncSync(directoryFd); } finally { fs.closeSync(directoryFd); }
}

function processAlive(pid) {
    if (!Number.isInteger(pid) || pid <= 1) return false;
    try { process.kill(pid, 0); return true; } catch (_) { return false; }
}

function positiveInteger(value, fallback, minimum = 1) {
    const parsed = Number.parseInt(String(value ?? ''), 10);
    return Number.isSafeInteger(parsed) && parsed >= minimum ? parsed : fallback;
}

function readJsonFile(file) {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function expandArgs(args, values) {
    return args.map((arg) => String(arg).replace(/\{([a-z_]+)\}/g, (_, key) => {
        if (!(key in values)) throw new Error(`unknown bridge driver placeholder: ${key}`);
        return values[key];
    }));
}

function routeConfigFromEnvironment(env) {
    let encoded = String(env.TRUSTLESS_BRIDGE_ROUTES_JSON || '').trim();
    const file = String(env.TRUSTLESS_BRIDGE_ROUTES_JSON_FILE || '').trim();
    if (encoded && file) throw new Error('configure one trustless bridge route source');
    if (file) encoded = fs.readFileSync(file, 'utf8');
    if (!encoded) return [];
    const parsed = JSON.parse(encoded);
    if (!Array.isArray(parsed)) throw new Error('trustless bridge routes must be a JSON array');
    return parsed;
}

function normalizeRoute(raw) {
    const routeId = String(raw?.route_id || '').trim().toLowerCase();
    const sourceChainId = positiveInteger(raw?.source_chain_id, 0);
    const proofKind = String(raw?.source_proof_kind || '').trim();
    const driverBin = String(raw?.driver_bin || '').trim();
    const readinessBin = String(raw?.readiness_bin || driverBin).trim();
    const driverArgs = raw?.driver_args;
    const readinessArgs = raw?.readiness_args;
    if (!ROUTE_RE.test(routeId) || sourceChainId === 0 || !proofKind
        || !driverBin || !readinessBin || !Array.isArray(driverArgs)
        || !driverArgs.every((arg) => typeof arg === 'string')
        || !Array.isArray(readinessArgs)
        || !readinessArgs.every((arg) => typeof arg === 'string')) {
        throw new Error(`invalid trustless bridge route config: ${routeId || '<missing>'}`);
    }
    const profile = ROUTE_PROFILES.get(routeId);
    if (!profile
        || profile.source_chain_id !== sourceChainId
        || profile.source_proof_kind !== proofKind) {
        throw new Error(`unsupported trustless route/chain/proof binding: ${routeId}`);
    }
    const ethereum = proofKind === 'sp1-ethereum-finality-v1';
    const maxAmountAtoms = BigInt(String(raw.max_amount_atoms || DEFAULT_MAX_AMOUNT_ATOMS));
    if (maxAmountAtoms <= 0n) throw new Error(`invalid route amount cap: ${routeId}`);
    return {
        route_id: routeId,
        source_chain_id: sourceChainId,
        source_proof_kind: proofKind,
        driver_bin: driverBin,
        driver_args: driverArgs,
        readiness_bin: readinessBin,
        readiness_args: readinessArgs,
        cwd: path.resolve(String(raw.cwd || process.cwd())),
        max_amount_atoms: maxAmountAtoms,
        stages: ethereum ? ETH_STAGES : ARBITRUM_STAGES,
        readiness_timeout_ms: positiveInteger(raw.readiness_timeout_ms, 60_000, 1_000),
        worker_timeout_ms: positiveInteger(raw.worker_timeout_ms, 6 * 60 * 60 * 1000, 60_000),
    };
}

function canonicalJobKey(request) {
    const preimage = Buffer.concat([
        Buffer.from(request.route_id, 'utf8'), Buffer.from([0]),
        Buffer.from(String(request.source_chain_id), 'ascii'), Buffer.from([0]),
        Buffer.from(request.deposit_tx_hash.slice(2), 'hex'),
        Buffer.from(request.deposit_id.replace(/^0x/, ''), 'hex'),
    ]);
    return `0x${keccak256(preimage).toString('hex')}`;
}

function create(runtime = {}, options = {}) {
    const env = options.env || process.env;
    const now = options.now || Date.now;
    const execFileAsync = options.execFileAsync || runtime.execFileAsync || promisify(execFile);
    const spawnImpl = options.spawn || runtime.spawn || spawn;
    const setIntervalImpl = options.setInterval || setInterval;
    const clearIntervalImpl = options.clearInterval || clearInterval;
    const setTimeoutImpl = options.setTimeout || setTimeout;
    const clearTimeoutImpl = options.clearTimeout || clearTimeout;
    const root = path.resolve(options.root || env.TRUSTLESS_BRIDGE_JOB_ROOT
        || path.join(os.homedir(), '.postfiat', 'wallet-proxy-8080', 'bridge-jobs-v2'));
    const routeRows = options.routes || routeConfigFromEnvironment(env);
    const routes = new Map(routeRows.map((row) => {
        const normalized = normalizeRoute(row);
        return [normalized.route_id, normalized];
    }));
    if (routes.size !== routeRows.length) throw new Error('duplicate trustless bridge route_id');
    fs.mkdirSync(root, { recursive: true, mode: 0o700 });
    const jobsRoot = path.join(root, 'jobs');
    fs.mkdirSync(jobsRoot, { recursive: true, mode: 0o700 });
    const cacheFile = path.join(root, 'readiness-cache.json');
    const refreshMs = positiveInteger(options.readinessRefreshMs
        ?? env.TRUSTLESS_BRIDGE_READINESS_REFRESH_MS, 15_000, 5_000);
    const maxAgeMs = positiveInteger(options.readinessMaxAgeMs
        ?? env.TRUSTLESS_BRIDGE_READINESS_MAX_AGE_MS, 30_000, refreshMs);
    const retryBaseMs = positiveInteger(options.retryBaseMs
        ?? env.TRUSTLESS_BRIDGE_RETRY_BASE_MS, 5_000, 100);
    const retryMaxMs = positiveInteger(options.retryMaxMs
        ?? env.TRUSTLESS_BRIDGE_RETRY_MAX_MS, 300_000, retryBaseMs);
    const watchdogMs = positiveInteger(options.watchdogMs, 30_000, 100);
    const readiness = new Map();
    const readinessInflight = new Map();
    const workers = new Map();

    try {
        const persisted = readJsonFile(cacheFile);
        if (persisted?.schema === 'postfiat-trustless-bridge-readiness-cache-v2') {
            for (const [routeId, row] of Object.entries(persisted.routes || {})) {
                if (routes.has(routeId) && Number.isFinite(Number(row.checked_at_ms))) {
                    readiness.set(routeId, row);
                }
            }
        }
    } catch (_) { /* first start or invalid cache */ }

    function jobDirectory(jobId) { return path.join(jobsRoot, jobId.slice(2)); }
    function jobFile(jobId) { return path.join(jobDirectory(jobId), 'job.json'); }
    function workerStateFile(jobId) { return path.join(jobDirectory(jobId), 'worker-state.json'); }
    function readJob(jobId) {
        const file = jobFile(jobId);
        if (!fs.existsSync(file)) return null;
        const job = readJsonFile(file);
        const stateFile = workerStateFile(jobId);
        if (!fs.existsSync(stateFile)) return job;
        const state = readJsonFile(stateFile);
        const route = routes.get(job.request.route_id);
        if (!route || !route.stages.has(state.status)) return job;
        return { ...job, ...state, request: job.request };
    }

    function persistReadiness() {
        atomicWrite(cacheFile, {
            schema: 'postfiat-trustless-bridge-readiness-cache-v2',
            routes: Object.fromEntries(readiness),
        });
    }

    function cachedReadiness(routeId) {
        const cached = readiness.get(routeId);
        if (!cached) return null;
        return {
            ...cached.result,
            route_id: routeId,
            readiness_cache: 'warm',
            readiness_checked_at_unix: Math.floor(cached.checked_at_ms / 1000),
            readiness_age_ms: Math.max(0, now() - cached.checked_at_ms),
            readiness_refresh_interval_ms: refreshMs,
            readiness_max_age_ms: maxAgeMs,
        };
    }

    function validateReadiness(route, result) {
        const common = result?.ok === true && result?.ready === true
            && result?.route_id === route.route_id
            && Number(result?.source_chain_id) === route.source_chain_id
            && result?.source_proof_kind === route.source_proof_kind
            && result?.observer_attestor_enabled === false
            && result?.prover_authenticated === true
            && result?.prover_healthy === true
            && result?.route_manifest_active === true
            && result?.program_vkey_active === true
            && result?.nav_cap_growth_enabled === true
            && result?.vault_paused === false
            && result?.vault_code_hash_matches === true
            && result?.token_code_hash_matches === true;
        if (!common) throw new Error('route readiness did not satisfy trustless ingress gates');
        if (route.source_proof_kind === 'sp1-ethereum-finality-v1') {
            if (Number(result.execution_rpc_sources_reachable) < 2
                || result.beacon_finality_current !== true) {
                throw new Error('Ethereum finality readiness is incomplete');
            }
        }
        return result;
    }

    async function refreshRouteReadiness(routeId) {
        const route = routes.get(routeId);
        if (!route) throw Object.assign(new Error('unsupported bridge route'), { code: 'unsupported_bridge_route' });
        if (readinessInflight.has(routeId)) return readinessInflight.get(routeId);
        const refresh = (async () => {
            let result;
            try {
                const values = { route_id: route.route_id, source_chain_id: String(route.source_chain_id) };
                const { stdout } = await execFileAsync(
                    route.readiness_bin,
                    expandArgs(route.readiness_args, values),
                    { cwd: route.cwd, timeout: route.readiness_timeout_ms, maxBuffer: 1024 * 1024 },
                );
                result = validateReadiness(route, JSON.parse(String(stdout).trim()));
            } catch (error) {
                result = {
                    ok: false,
                    ready: false,
                    route_id: route.route_id,
                    source_chain_id: route.source_chain_id,
                    source_proof_kind: route.source_proof_kind,
                    trust_class: 'DISABLED',
                    code: 'trustless_ingress_unavailable',
                    message: `Trustless ${route.route_id} preflight is unavailable. Retry shortly.`,
                    diagnostic_code: String(error.code || error.name || 'readiness_failed').slice(0, 64),
                    observer_attestor_enabled: false,
                };
            }
            readiness.set(routeId, { checked_at_ms: now(), result });
            persistReadiness();
            return cachedReadiness(routeId);
        })();
        readinessInflight.set(routeId, refresh);
        try { return await refresh; } finally { readinessInflight.delete(routeId); }
    }

    async function routeReadiness(routeId) {
        const normalized = String(routeId || '').trim().toLowerCase();
        if (!routes.has(normalized)) {
            return {
                ok: false, ready: false, route_id: normalized,
                code: 'unsupported_bridge_route', observer_attestor_enabled: false,
            };
        }
        const cached = cachedReadiness(normalized);
        if (cached && cached.readiness_age_ms <= maxAgeMs) return cached;
        refreshRouteReadiness(normalized).catch(() => {});
        return {
            ok: false, ready: false, route_id: normalized,
            source_chain_id: routes.get(normalized).source_chain_id,
            source_proof_kind: routes.get(normalized).source_proof_kind,
            code: 'trustless_readiness_warming', observer_attestor_enabled: false,
            readiness_cache: 'warming', readiness_age_ms: cached?.readiness_age_ms ?? null,
            readiness_refresh_interval_ms: refreshMs, readiness_max_age_ms: maxAgeMs,
        };
    }

    function normalizeRequest(body) {
        const routeId = String(body?.route_id || '').trim().toLowerCase();
        const route = routes.get(routeId);
        const sourceChainId = positiveInteger(body?.source_chain_id, 0);
        const depositTxHash = String(body?.deposit_tx_hash || '').trim().toLowerCase();
        const depositId = String(body?.deposit_id || '').trim().toLowerCase();
        const pftlRecipient = String(body?.pftl_recipient || '').trim().toLowerCase();
        const depositor = String(body?.depositor || '').trim().toLowerCase();
        const idempotencyKey = String(body?.idempotency_key || '').trim();
        let amountAtoms;
        try { amountAtoms = BigInt(String(body?.amount_atoms || '')); } catch (_) { amountAtoms = 0n; }
        if (!route || sourceChainId !== route.source_chain_id
            || !TX_RE.test(depositTxHash) || !BYTES32_RE.test(depositId)
            || !PFT_RE.test(pftlRecipient) || !EVM_RE.test(depositor)
            || amountAtoms <= 0n || amountAtoms > route.max_amount_atoms
            || !/^[A-Za-z0-9._:-]{8,128}$/.test(idempotencyKey)) {
            throw Object.assign(new Error('invalid trustless bridge job request'), {
                code: 'invalid_trustless_bridge_job',
            });
        }
        return {
            route_id: routeId,
            source_chain_id: sourceChainId,
            deposit_tx_hash: depositTxHash,
            deposit_id: `0x${depositId.replace(/^0x/, '')}`,
            pftl_recipient: pftlRecipient,
            depositor,
            amount_atoms: amountAtoms.toString(),
            idempotency_key: idempotencyKey,
        };
    }

    function backoffMs(retryCount) {
        return Math.min(retryMaxMs, retryBaseMs * (2 ** Math.min(16, Math.max(0, retryCount - 1))));
    }

    function reconcileWorkerExit(jobId, code, signal) {
        const worker = workers.get(jobId);
        if (worker?.timeout) clearTimeoutImpl(worker.timeout);
        if (worker?.killTimeout) clearTimeoutImpl(worker.killTimeout);
        if (worker?.forcedExitTimeout) clearTimeoutImpl(worker.forcedExitTimeout);
        workers.delete(jobId);
        const file = jobFile(jobId);
        if (!fs.existsSync(file)) return;
        const job = readJsonFile(file);
        const stateFile = workerStateFile(jobId);
        if (fs.existsSync(stateFile)) {
            const state = readJsonFile(stateFile);
            if (TERMINAL_STAGES.has(state.status)) {
                atomicWrite(file, { ...job, worker_pid: null, updated_at_unix: Math.floor(now() / 1000) });
                return;
            }
        }
        const retryCount = positiveInteger(job.retry_count, 0, 0) + 1;
        atomicWrite(file, {
            ...job,
            worker_pid: null,
            retry_count: retryCount,
            next_retry_at_ms: now() + backoffMs(retryCount),
            last_worker_exit: { code: Number.isInteger(code) ? code : null, signal: signal || null },
            updated_at_unix: Math.floor(now() / 1000),
        });
    }

    function spawnWorker(jobId) {
        const file = jobFile(jobId);
        const job = readJob(jobId);
        if (!job || TERMINAL_STAGES.has(job.status)) return job;
        if (workers.has(jobId) || processAlive(job.worker_pid)) return job;
        if (Number(job.next_retry_at_ms || 0) > now()) return job;
        const route = routes.get(job.request.route_id);
        if (!route) return job;
        const values = {
            job_file: file,
            job_dir: jobDirectory(jobId),
            route_id: route.route_id,
            source_chain_id: String(route.source_chain_id),
        };
        const logFd = fs.openSync(path.join(jobDirectory(jobId), 'worker.log'), 'a', 0o600);
        const child = spawnImpl(route.driver_bin, expandArgs(route.driver_args, values), {
            cwd: route.cwd,
            stdio: ['ignore', logFd, logFd],
            env: { ...env, PYTHONUNBUFFERED: '1' },
        });
        fs.closeSync(logFd);
        let settled = false;
        const finish = (code, signal) => {
            if (settled) return;
            settled = true;
            reconcileWorkerExit(jobId, code, signal);
        };
        const worker = { child, timeout: null, killTimeout: null, forcedExitTimeout: null };
        const timeout = setTimeoutImpl(() => {
            try { child.kill?.('SIGTERM'); } catch (_) { /* escalation below */ }
            worker.killTimeout = setTimeoutImpl(() => {
                try { child.kill?.('SIGKILL'); } catch (_) { /* forced retry below */ }
                worker.forcedExitTimeout = setTimeoutImpl(
                    () => finish(null, 'worker-timeout'),
                    1_000,
                );
                worker.forcedExitTimeout.unref?.();
            }, 5_000);
            worker.killTimeout.unref?.();
        }, route.worker_timeout_ms);
        timeout.unref?.();
        worker.timeout = timeout;
        workers.set(jobId, worker);
        atomicWrite(file, {
            ...readJsonFile(file),
            status: job.status === 'created' ? 'queued' : job.status,
            worker_pid: child.pid,
            worker_started_at_ms: now(),
            updated_at_unix: Math.floor(now() / 1000),
        });
        child.once('error', () => finish(null, 'spawn-error'));
        child.once('exit', finish);
        child.unref?.();
        return readJob(jobId);
    }

    async function submit(body) {
        const request = normalizeRequest(body);
        const ready = await routeReadiness(request.route_id);
        if (ready.ready !== true) {
            throw Object.assign(new Error(ready.message || 'Trustless bridge path is unavailable'), {
                code: 'trustless_ingress_unavailable',
            });
        }
        const jobId = canonicalJobKey(request);
        const directory = jobDirectory(jobId);
        const file = jobFile(jobId);
        const { idempotency_key: idempotencyKey, ...economicRequest } = request;
        const fingerprint = crypto.createHash('sha256').update(stableJson(economicRequest)).digest('hex');
        fs.mkdirSync(directory, { recursive: true, mode: 0o700 });
        if (fs.existsSync(file)) {
            const existing = readJsonFile(file);
            if (existing.request_fingerprint !== fingerprint) {
                throw Object.assign(new Error('bridge job binding conflict'), {
                    code: 'bridge_job_binding_conflict',
                });
            }
            return { ...spawnWorker(jobId), idempotent_replay: true, idempotency_key: idempotencyKey };
        }
        const timestamp = Math.floor(now() / 1000);
        atomicWrite(file, {
            schema: 'postfiat-trustless-bridge-job-v2',
            ok: true,
            job_id: jobId,
            status: 'created',
            request,
            request_fingerprint: fingerprint,
            retry_count: 0,
            next_retry_at_ms: 0,
            created_at_unix: timestamp,
            updated_at_unix: timestamp,
            observer_attestor_enabled: false,
        });
        return spawnWorker(jobId);
    }

    function status(jobId) {
        const normalized = String(jobId || '').trim().toLowerCase();
        if (!TX_RE.test(normalized)) return null;
        const job = readJob(normalized);
        if (!job) return null;
        return spawnWorker(normalized);
    }

    function resumeJobs() {
        for (const name of fs.readdirSync(jobsRoot).filter((entry) => /^[0-9a-f]{64}$/.test(entry))) {
            try { spawnWorker(`0x${name}`); } catch (_) { /* bounded watchdog retries */ }
        }
    }

    for (const routeId of routes.keys()) refreshRouteReadiness(routeId).catch(() => {});
    resumeJobs();
    const readinessTimer = setIntervalImpl(() => {
        for (const routeId of routes.keys()) refreshRouteReadiness(routeId).catch(() => {});
    }, refreshMs);
    readinessTimer.unref?.();
    const watchdogTimer = setIntervalImpl(resumeJobs, watchdogMs);
    watchdogTimer.unref?.();

    function close() {
        clearIntervalImpl(readinessTimer);
        clearIntervalImpl(watchdogTimer);
    }

    return {
        trustlessBridgeReadiness: routeReadiness,
        refreshTrustlessBridgeReadiness: refreshRouteReadiness,
        submitTrustlessBridgeJob: submit,
        trustlessBridgeJobStatus: status,
        closeTrustlessBridgeJobs: close,
        canonicalTrustlessBridgeJobKey: canonicalJobKey,
        _reconcileTrustlessBridgeWorkerExitForTest: reconcileWorkerExit,
    };
}

module.exports = { canonicalJobKey, create, normalizeRoute };
