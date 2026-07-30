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
const RECEIPT_ID_RE = /^(?:0x)?(?:[0-9a-f]{64}|[0-9a-f]{96})$/;
const PFT_RE = /^pf[0-9a-f]{40}$/;
const EVM_RE = /^0x[0-9a-f]{40}$/;
const ROUTE_RE = /^[a-z0-9][a-z0-9-]{2,63}$/;
const TERMINAL_STAGES = new Set(['accepted', 'failed']);
const WORKER_STATE_SCHEMA = 'postfiat-trustless-bridge-worker-state-v2';
const WORKER_STATE_PUBLIC_FIELDS = new Set([
    'status', 'route_id', 'source_chain_id', 'source_proof_kind',
    'program_vkey', 'manifest_hash', 'route_profile_hash', 'asset_id',
    'observer_attestor_enabled', 'updated_at_unix', 'stage_index', 'retryable',
    'code', 'message', 'receipt_code', 'receipt_id', 'tx_id',
    'terminal_checkpoint_sha256',
]);
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
const FILE_HASH_RE = /^[0-9a-f]{64}$/;
const PROGRAM_VKEY_RE = /^0x[0-9a-f]{64}$/;
const HASH48_RE = /^[0-9a-f]{96}$/;
const EVM_CODE_HASH_RE = /^0x[0-9a-f]{64}$/;
const SEPOLIA_P0_PROGRAM_VKEY = '0x0077f479ed28535dbb5035f455a875334bae7d5a1eaa7c22c6f070a404eab31f';
const SEPOLIA_P0_MANIFEST_HASH = 'dc409b424e7627b936d81a16d2fc8f4c17e21a108d654be6b992e552d7b0c6d3';
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

function processStartToken(pid) {
    if (!Number.isInteger(pid) || pid <= 1) return null;
    try {
        const stat = fs.readFileSync(`/proc/${pid}/stat`, 'utf8');
        const fields = stat.slice(stat.lastIndexOf(') ') + 2).trim().split(/\s+/);
        return /^\d+$/.test(fields[19] || '') ? fields[19] : null;
    } catch (_) {
        return null;
    }
}

function positiveInteger(value, fallback, minimum = 1) {
    const parsed = Number.parseInt(String(value ?? ''), 10);
    return Number.isSafeInteger(parsed) && parsed >= minimum ? parsed : fallback;
}

function readJsonFile(file) {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function sha256File(file) {
    return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function validatePinnedFile(file, expectedHash, label) {
    const absolute = path.resolve(String(file || ''));
    const stat = fs.lstatSync(absolute);
    if (!stat.isFile() || stat.isSymbolicLink() || (stat.mode & 0o022) !== 0
        || !FILE_HASH_RE.test(String(expectedHash || ''))
        || sha256File(absolute) !== expectedHash) {
        throw new Error(`${label} failed secure hash pin validation`);
    }
    return absolute;
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
    if (file) {
        const absolute = path.resolve(file);
        const stat = fs.lstatSync(absolute);
        if (!stat.isFile() || stat.isSymbolicLink() || stat.uid !== process.getuid()
            || (stat.mode & 0o022) !== 0) {
            throw new Error('trustless bridge route config must be an owner-controlled regular file');
        }
        encoded = fs.readFileSync(absolute, 'utf8');
    }
    if (!encoded) return [];
    const parsed = JSON.parse(encoded);
    if (!Array.isArray(parsed)) throw new Error('trustless bridge routes must be a JSON array');
    return parsed;
}

function normalizeRoute(raw) {
    const routeId = String(raw?.route_id || '').trim().toLowerCase();
    const sourceChainId = positiveInteger(raw?.source_chain_id, 0);
    const proofKind = String(raw?.source_proof_kind || '').trim();
    const programVkey = String(raw?.program_vkey || '').trim().toLowerCase();
    const manifestHash = String(raw?.manifest_hash || '').trim().toLowerCase();
    const routeProfileHash = String(raw?.route_profile_hash || '').trim().toLowerCase();
    const assetId = String(raw?.asset_id || '').trim().toLowerCase();
    const vaultAddress = String(raw?.vault_address || '').trim().toLowerCase();
    const vaultCodeHash = String(raw?.vault_runtime_code_hash || '').trim().toLowerCase();
    const tokenAddress = String(raw?.token_address || '').trim().toLowerCase();
    const tokenCodeHash = String(raw?.token_runtime_code_hash || '').trim().toLowerCase();
    const driverBinRaw = String(raw?.driver_bin || '').trim();
    const readinessBinRaw = String(raw?.readiness_bin || driverBinRaw).trim();
    const driverArgs = raw?.driver_args;
    const readinessArgs = raw?.readiness_args;
    if (!ROUTE_RE.test(routeId) || sourceChainId === 0 || !proofKind
        || !PROGRAM_VKEY_RE.test(programVkey) || !FILE_HASH_RE.test(manifestHash)
        || !HASH48_RE.test(routeProfileHash) || !HASH48_RE.test(assetId)
        || !EVM_RE.test(vaultAddress) || !EVM_CODE_HASH_RE.test(vaultCodeHash)
        || !EVM_RE.test(tokenAddress) || !EVM_CODE_HASH_RE.test(tokenCodeHash)
        || !driverBinRaw || !readinessBinRaw || !Array.isArray(driverArgs)
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
    if (routeId === 'ethereum-mainnet-usdc-v1'
        && (programVkey === SEPOLIA_P0_PROGRAM_VKEY || manifestHash === SEPOLIA_P0_MANIFEST_HASH)) {
        throw new Error('Sepolia P0 verifier identity cannot authorize the Ethereum mainnet route');
    }
    const ethereum = proofKind === 'sp1-ethereum-finality-v1';
    const maxAmountAtoms = BigInt(String(raw.max_amount_atoms || DEFAULT_MAX_AMOUNT_ATOMS));
    if (maxAmountAtoms <= 0n) throw new Error(`invalid route amount cap: ${routeId}`);
    const driverBin = validatePinnedFile(
        driverBinRaw, String(raw.driver_sha256 || '').toLowerCase(), 'bridge driver',
    );
    const readinessBin = validatePinnedFile(
        readinessBinRaw, String(raw.readiness_sha256 || raw.driver_sha256 || '').toLowerCase(),
        'bridge readiness driver',
    );
    const pinnedFiles = Array.isArray(raw.pinned_files) ? raw.pinned_files : [];
    for (const pin of pinnedFiles) {
        validatePinnedFile(pin?.path, String(pin?.sha256 || '').toLowerCase(), 'bridge route artifact');
    }
    return {
        route_id: routeId,
        source_chain_id: sourceChainId,
        source_proof_kind: proofKind,
        program_vkey: programVkey,
        manifest_hash: manifestHash,
        route_profile_hash: routeProfileHash,
        asset_id: assetId,
        vault_address: vaultAddress,
        vault_runtime_code_hash: vaultCodeHash,
        token_address: tokenAddress,
        token_runtime_code_hash: tokenCodeHash,
        driver_bin: driverBin,
        driver_args: driverArgs,
        readiness_bin: readinessBin,
        readiness_args: readinessArgs,
        driver_sha256: String(raw.driver_sha256).toLowerCase(),
        readiness_sha256: String(raw.readiness_sha256 || raw.driver_sha256).toLowerCase(),
        pinned_files: pinnedFiles,
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
    const wallNow = options.wallNow || Date.now;
    const beforeCreateJob = options.beforeCreateJob || (async () => {});
    const isProcessAlive = options.processAlive || processAlive;
    const getProcessStartToken = options.processStartToken || processStartToken;
    const killProcess = options.killProcess || ((pid, signal) => process.kill(pid, signal));
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
    const submissionLockTimeoutMs = positiveInteger(options.submissionLockTimeoutMs
        ?? env.TRUSTLESS_BRIDGE_SUBMISSION_LOCK_TIMEOUT_MS, 15_000, 100);
    const submissionLockPollMs = positiveInteger(options.submissionLockPollMs, 50, 10);
    const submissionLockStaleMs = positiveInteger(options.submissionLockStaleMs,
        Math.max(60_000, submissionLockTimeoutMs * 2), submissionLockTimeoutMs);
    const workerLogMaxBytes = positiveInteger(options.workerLogMaxBytes
        ?? env.TRUSTLESS_BRIDGE_WORKER_LOG_MAX_BYTES, 10 * 1024 * 1024, 1024);
    const workerLogRetention = positiveInteger(options.workerLogRetention, 3, 1);
    const workerStateQuarantineRetention = positiveInteger(
        options.workerStateQuarantineRetention, 8, 1,
    );
    const readiness = new Map();
    const readinessInflight = new Map();
    const workers = new Map();
    const submissions = new Map();

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
    function submissionLockFile(jobId) { return path.join(jobDirectory(jobId), 'submission.lock'); }
    function validateWorkerState(job, state) {
        const route = routes.get(job.request.route_id);
        const common = route
            && state?.schema === WORKER_STATE_SCHEMA
            && route.stages.has(state.status)
            && state.route_id === route.route_id
            && Number(state.source_chain_id) === route.source_chain_id
            && state.source_proof_kind === route.source_proof_kind
            && state.program_vkey === route.program_vkey
            && state.manifest_hash === route.manifest_hash
            && state.route_profile_hash === route.route_profile_hash
            && state.asset_id === route.asset_id
            && state.observer_attestor_enabled === false
            && Number.isSafeInteger(Number(state.updated_at_unix));
        let terminalValid = true;
        if (state?.status === 'accepted') {
            terminalValid = state.retryable === false
                && state.receipt_code === 'ACCEPTED'
                && RECEIPT_ID_RE.test(String(state.receipt_id || state.tx_id || ''))
                && FILE_HASH_RE.test(String(state.terminal_checkpoint_sha256 || ''));
        } else if (state?.status === 'failed') {
            terminalValid = state.retryable === false
                && /^[a-z0-9_]{1,64}$/.test(String(state.code || ''));
        } else {
            terminalValid = state?.retryable === true;
        }
        if (!common || !terminalValid) {
            throw Object.assign(new Error('durable bridge worker state failed validation'), {
                code: 'bridge_worker_state_invalid',
            });
        }
        return Object.fromEntries(
            Object.entries(state).filter(([key]) => WORKER_STATE_PUBLIC_FIELDS.has(key)),
        );
    }

    function quarantineWorkerState(jobId) {
        const stateFile = workerStateFile(jobId);
        if (!fs.existsSync(stateFile)) return null;
        const quarantine = path.join(
            jobDirectory(jobId),
            `worker-state.invalid.${wallNow()}.${crypto.randomBytes(4).toString('hex')}.json`,
        );
        fs.renameSync(stateFile, quarantine);
        const quarantines = fs.readdirSync(jobDirectory(jobId))
            .filter((name) => name.startsWith('worker-state.invalid.'))
            .sort()
            .reverse();
        for (const name of quarantines.slice(workerStateQuarantineRetention)) {
            try { fs.unlinkSync(path.join(jobDirectory(jobId), name)); } catch (_) { /* bounded best effort */ }
        }
        return quarantine;
    }

    function rotateWorkerLog(jobId) {
        const logFile = path.join(jobDirectory(jobId), 'worker.log');
        if (!fs.existsSync(logFile)) return logFile;
        const stat = fs.lstatSync(logFile);
        if (!stat.isFile() || stat.isSymbolicLink()) {
            throw new Error('bridge worker log must be a regular file');
        }
        if (stat.size < workerLogMaxBytes) return logFile;
        for (let index = workerLogRetention; index >= 1; index -= 1) {
            const destination = `${logFile}.${index}`;
            if (index === workerLogRetention && fs.existsSync(destination)) fs.unlinkSync(destination);
            const source = index === 1 ? logFile : `${logFile}.${index - 1}`;
            if (fs.existsSync(source)) fs.renameSync(source, destination);
        }
        return logFile;
    }

    function readJob(jobId) {
        const file = jobFile(jobId);
        if (!fs.existsSync(file)) return null;
        const job = readJsonFile(file);
        const stateFile = workerStateFile(jobId);
        if (!fs.existsSync(stateFile)) return job;
        const state = validateWorkerState(job, readJsonFile(stateFile));
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
            && result?.program_vkey === route.program_vkey
            && result?.manifest_hash === route.manifest_hash
            && result?.route_profile_hash === route.route_profile_hash
            && result?.asset_id === route.asset_id
            && result?.vault_address === route.vault_address
            && result?.vault_runtime_code_hash === route.vault_runtime_code_hash
            && result?.token_address === route.token_address
            && result?.token_runtime_code_hash === route.token_runtime_code_hash
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

    function wait(ms) {
        return new Promise((resolve) => setTimeoutImpl(resolve, ms));
    }

    function staleSubmissionLock(lockFile) {
        try {
            const stat = fs.lstatSync(lockFile);
            if (!stat.isFile() || stat.isSymbolicLink()) return false;
            const ageMs = Math.max(0, wallNow() - stat.mtimeMs);
            let row = null;
            try { row = readJsonFile(lockFile); } catch (_) { /* malformed lock ages out */ }
            if (row && Number.isInteger(row.pid) && isProcessAlive(row.pid)) return false;
            return (row && Number.isInteger(row.pid)) || ageMs >= submissionLockStaleMs;
        } catch (_) {
            return false;
        }
    }

    function tryAcquireSubmissionLock(jobId) {
        const directory = jobDirectory(jobId);
        fs.mkdirSync(directory, { recursive: true, mode: 0o700 });
        const lockFile = submissionLockFile(jobId);
        const token = crypto.randomBytes(16).toString('hex');
        try {
            const fd = fs.openSync(lockFile, 'wx', 0o600);
            try {
                fs.writeFileSync(fd, `${JSON.stringify({
                    schema: 'postfiat-trustless-bridge-submission-lock-v1',
                    token,
                    pid: process.pid,
                    created_at_ms: wallNow(),
                })}\n`);
                fs.fsyncSync(fd);
            } finally {
                fs.closeSync(fd);
            }
            return { lockFile, token };
        } catch (error) {
            if (error.code !== 'EEXIST') throw error;
            if (staleSubmissionLock(lockFile)) {
                try { fs.unlinkSync(lockFile); } catch (_) { /* another process won recovery */ }
            }
            return null;
        }
    }

    async function acquireSubmissionLock(jobId) {
        const deadline = wallNow() + submissionLockTimeoutMs;
        while (wallNow() < deadline) {
            const acquired = tryAcquireSubmissionLock(jobId);
            if (acquired) return acquired;
            await wait(submissionLockPollMs);
        }
        throw Object.assign(new Error('bridge job is busy; retry shortly'), {
            code: 'bridge_job_busy',
        });
    }

    function releaseSubmissionLock(lock) {
        try {
            const row = readJsonFile(lock.lockFile);
            if (row?.token === lock.token) fs.unlinkSync(lock.lockFile);
        } catch (_) { /* process exit or another owner already cleaned up */ }
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
            try {
                const state = validateWorkerState(job, readJsonFile(stateFile));
                if (TERMINAL_STAGES.has(state.status)) {
                    atomicWrite(file, { ...job, worker_pid: null, updated_at_unix: Math.floor(now() / 1000) });
                    return;
                }
            } catch (error) {
                if (error.code !== 'bridge_worker_state_invalid') throw error;
                quarantineWorkerState(jobId);
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

    function superviseOrphanedWorker(jobId, job, route) {
        const startedAt = Number(job.worker_started_at_ms)
            || (Number(job.updated_at_unix) * 1000);
        if (!Number.isFinite(startedAt) || wallNow() - startedAt <= route.worker_timeout_ms) {
            return job;
        }
        const requestedAt = Number(job.worker_termination_requested_at_ms || 0);
        if (requestedAt <= 0) {
            try { killProcess(job.worker_pid, 'SIGTERM'); } catch (_) { /* next watchdog verifies */ }
            const updated = {
                ...job,
                worker_termination_requested_at_ms: wallNow(),
                updated_at_unix: Math.floor(now() / 1000),
            };
            atomicWrite(jobFile(jobId), updated);
            return updated;
        }
        if (wallNow() - requestedAt >= 5_000) {
            try { killProcess(job.worker_pid, 'SIGKILL'); } catch (_) { /* next watchdog verifies */ }
        }
        return job;
    }

    function workerProcessState(job) {
        if (!isProcessAlive(job.worker_pid)) return 'dead';
        if (!job.worker_process_start_token) return 'unknown';
        const current = getProcessStartToken(job.worker_pid);
        return current === job.worker_process_start_token ? 'matching' : 'reused';
    }

    function spawnWorker(jobId) {
        const file = jobFile(jobId);
        let job;
        try {
            job = readJob(jobId);
        } catch (error) {
            if (error.code !== 'bridge_worker_state_invalid') throw error;
            quarantineWorkerState(jobId);
            job = fs.existsSync(file) ? readJsonFile(file) : null;
        }
        if (!job || TERMINAL_STAGES.has(job.status)) return job;
        if (workers.has(jobId)) return job;
        const route = routes.get(job.request.route_id);
        if (!route) return job;
        const processState = workerProcessState(job);
        if (processState === 'matching') return superviseOrphanedWorker(jobId, job, route);
        // A legacy worker without a process-start identity is never signalled: a
        // recycled PID could otherwise terminate an unrelated host process.
        if (processState === 'unknown') return job;
        if (processState === 'reused') {
            const retryCount = positiveInteger(job.retry_count, 0, 0) + 1;
            job = {
                ...job,
                worker_pid: null,
                worker_process_start_token: null,
                retry_count: retryCount,
                next_retry_at_ms: now() + backoffMs(retryCount),
                last_worker_exit: { code: null, signal: 'worker-pid-reused' },
                updated_at_unix: Math.floor(now() / 1000),
            };
            atomicWrite(file, job);
            return job;
        }
        if (Number(job.worker_termination_requested_at_ms || 0) > 0) {
            const retryCount = positiveInteger(job.retry_count, 0, 0) + 1;
            job = {
                ...job,
                worker_pid: null,
                worker_termination_requested_at_ms: null,
                retry_count: retryCount,
                next_retry_at_ms: now() + backoffMs(retryCount),
                last_worker_exit: { code: null, signal: 'orphan-worker-timeout' },
                updated_at_unix: Math.floor(now() / 1000),
            };
            atomicWrite(file, job);
            return job;
        }
        if (Number(job.next_retry_at_ms || 0) > now()) return job;
        const values = {
            job_file: file,
            job_dir: jobDirectory(jobId),
            route_id: route.route_id,
            source_chain_id: String(route.source_chain_id),
        };
        const logFd = fs.openSync(rotateWorkerLog(jobId), 'a', 0o600);
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
            worker_process_start_token: getProcessStartToken(child.pid),
            worker_started_at_ms: wallNow(),
            worker_termination_requested_at_ms: null,
            updated_at_unix: Math.floor(now() / 1000),
        });
        child.once('error', () => finish(null, 'spawn-error'));
        child.once('exit', finish);
        child.unref?.();
        return readJob(jobId);
    }

    async function createOrReplayJob(request, jobId) {
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
        await beforeCreateJob({ job_id: jobId, request: economicRequest });
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
        return { ...spawnWorker(jobId), idempotent_replay: false, idempotency_key: idempotencyKey };
    }

    async function withSubmissionLock(jobId, operation) {
        while (submissions.has(jobId)) {
            try { await submissions.get(jobId); } catch (_) { /* next caller may retry */ }
        }
        const running = Promise.resolve().then(operation);
        submissions.set(jobId, running);
        try {
            return await running;
        } finally {
            if (submissions.get(jobId) === running) submissions.delete(jobId);
        }
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
        return withSubmissionLock(jobId, async () => {
            const lock = await acquireSubmissionLock(jobId);
            try {
                return await createOrReplayJob(request, jobId);
            } finally {
                releaseSubmissionLock(lock);
            }
        });
    }

    function status(jobId) {
        const normalized = String(jobId || '').trim().toLowerCase();
        if (!TX_RE.test(normalized)) return null;
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
        for (const worker of workers.values()) {
            if (worker.timeout) clearTimeoutImpl(worker.timeout);
            if (worker.killTimeout) clearTimeoutImpl(worker.killTimeout);
            if (worker.forcedExitTimeout) clearTimeoutImpl(worker.forcedExitTimeout);
        }
        workers.clear();
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
