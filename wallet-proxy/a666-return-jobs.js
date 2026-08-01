'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFile, spawn } = require('node:child_process');
const { promisify } = require('node:util');

const JOB_SCHEMA = 'postfiat-a666-return-relay-job-v1';
const STATE_SCHEMA = 'postfiat-a666-return-relay-state-v1';
const CONFIG_SCHEMA = 'postfiat-a666-return-relay-config-v1';
const ROUTE_ID = 'pftl-a666-ethereum-wA666-usdc-v1';
const ROUTE_DIGEST = '12ed00ca87e29554ce4b978da1710fffc0830767e84e62f08df257f727db953efdd89bcf6ea99f5634d6e5ea8aca2933';
const NATIVE_ASSET = '521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c';
const CONTROLLER = '0x9a0262c0572fb4db08765408eb225e207f40c3d9';
const WRAPPED_TOKEN = '0xee4c92edb03efdd9b519339edc19ad70c69a9be5';
const HASH32_RE = /^(?:0x)?[0-9a-f]{64}$/;
const EVM_RE = /^0x[0-9a-f]{40}$/;
const PFTL_RE = /^pf[0-9a-f]{40}$/;
const JOB_ID_RE = /^0x[0-9a-f]{64}$/;
const TERMINAL = new Set(['accepted', 'failed']);
const STAGES = new Set([
    'queued', 'waiting_for_ethereum_receipt', 'waiting_for_ethereum_finality',
    'proving_ethereum_receipt', 'submitting_pftl_import', 'retry_wait', 'accepted', 'failed',
]);

function stableJson(value) {
    if (Array.isArray(value)) return `[${value.map(stableJson).join(',')}]`;
    if (value && typeof value === 'object') {
        return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`).join(',')}}`;
    }
    return JSON.stringify(value);
}

function atomicWrite(file, value) {
    fs.mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 });
    const temporary = `${file}.${process.pid}.${crypto.randomBytes(8).toString('hex')}.tmp`;
    const fd = fs.openSync(temporary, 'wx', 0o600);
    try {
        fs.writeFileSync(fd, `${JSON.stringify(value, null, 2)}\n`);
        fs.fsyncSync(fd);
    } finally { fs.closeSync(fd); }
    fs.renameSync(temporary, file);
    const directory = fs.openSync(path.dirname(file), 'r');
    try { fs.fsyncSync(directory); } finally { fs.closeSync(directory); }
}

function sha256File(file) {
    return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function securePinnedFile(file, digest, label) {
    const absolute = path.resolve(String(file || ''));
    const stat = fs.lstatSync(absolute);
    if (!stat.isFile() || stat.isSymbolicLink() || (stat.mode & 0o022) !== 0
        || !/^[0-9a-f]{64}$/.test(String(digest || '')) || sha256File(absolute) !== digest) {
        throw new Error(`${label} failed secure hash pin validation`);
    }
    return absolute;
}

function loadConfig(env, supplied) {
    const raw = supplied || (() => {
        const file = path.resolve(String(env.A666_RETURN_RELAY_CONFIG_FILE || ''));
        const stat = fs.lstatSync(file);
        if (!stat.isFile() || stat.isSymbolicLink() || stat.uid !== process.getuid()
            || (stat.mode & 0o022) !== 0 || stat.size > 64 * 1024) {
            throw new Error('A666 return relay config must be an owner-controlled regular file');
        }
        return JSON.parse(fs.readFileSync(file, 'utf8'));
    })();
    if (raw?.schema !== CONFIG_SCHEMA || raw.enabled !== true || raw.route_id !== ROUTE_ID
        || raw.route_config_digest !== ROUTE_DIGEST
        || String(raw.controller || '').toLowerCase() !== CONTROLLER
        || String(raw.wrapped_token || '').toLowerCase() !== WRAPPED_TOKEN
        || !Array.isArray(raw.driver_args) || !Array.isArray(raw.readiness_args)) {
        throw new Error('invalid A666 return relay config');
    }
    const driverBin = supplied ? path.resolve(raw.driver_bin)
        : securePinnedFile(raw.driver_bin, raw.driver_sha256, 'A666 return relay driver');
    const maxAmount = BigInt(String(raw.max_amount_atoms || '0'));
    if (maxAmount <= 0n) throw new Error('invalid A666 return relay amount cap');
    return {
        ...raw,
        driver_bin: driverBin,
        cwd: path.resolve(raw.cwd),
        max_amount_atoms: maxAmount,
        worker_timeout_ms: Number(raw.worker_timeout_ms || 4 * 60 * 60 * 1000),
        readiness_timeout_ms: Number(raw.readiness_timeout_ms || 120_000),
    };
}

function canonicalJobId(transactionHash) {
    const digest = crypto.createHash('sha256')
        .update('postfiat.a666.return-relay.job.v1\0')
        .update(Buffer.from(transactionHash.slice(2), 'hex')).digest('hex');
    return `0x${digest}`;
}

function normalizeRequest(body, config) {
    const request = {
        route_id: String(body?.route_id || '').trim(),
        route_config_digest: String(body?.route_config_digest || '').trim().toLowerCase(),
        transaction_hash: String(body?.transaction_hash || '').trim().toLowerCase(),
        ethereum_sender: String(body?.ethereum_sender || '').trim().toLowerCase(),
        pftl_recipient: String(body?.pftl_recipient || '').trim().toLowerCase(),
        native_nav_asset_id: String(body?.native_nav_asset_id || '').trim().toLowerCase(),
        amount_atoms: String(body?.amount_atoms || '').trim(),
        return_nonce: String(body?.return_nonce || '').trim().toLowerCase().replace(/^0x/, ''),
    };
    let amount = 0n;
    try { amount = BigInt(request.amount_atoms); } catch (_) { /* rejected below */ }
    if (request.route_id !== ROUTE_ID || request.route_config_digest !== ROUTE_DIGEST
        || !/^0x[0-9a-f]{64}$/.test(request.transaction_hash)
        || !EVM_RE.test(request.ethereum_sender) || !PFTL_RE.test(request.pftl_recipient)
        || request.native_nav_asset_id !== NATIVE_ASSET || amount <= 0n
        || amount > config.max_amount_atoms || !HASH32_RE.test(request.return_nonce)
        || /^0+$/.test(request.return_nonce)) {
        throw Object.assign(new Error('invalid A666 return relay request'), {
            code: 'invalid_a666_return_relay_request',
        });
    }
    return request;
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
    } catch (_) { return null; }
}

function expandArgs(args, values) {
    return args.map((argument) => String(argument).replace(/\{([a-z_]+)\}/g, (_, key) => {
        if (!(key in values)) throw new Error(`unknown A666 return relay placeholder: ${key}`);
        return values[key];
    }));
}

function create(runtime = {}, options = {}) {
    const env = options.env || process.env;
    if (!options.config && !env.A666_RETURN_RELAY_CONFIG_FILE) {
        const unavailable = async () => ({ ok: false, ready: false, route_id: ROUTE_ID,
            code: 'a666_return_relay_not_configured',
            message: 'A666 return relay is not configured on this wallet host.' });
        return {
            a666ReturnRelayReadiness: unavailable,
            submitA666ReturnRelayJob: async () => { throw Object.assign(
                new Error('A666 return relay is not configured.'), { code: 'a666_return_relay_not_configured' }); },
            a666ReturnRelayJobStatus: () => null,
            closeA666ReturnRelayJobs: () => {},
            canonicalA666ReturnRelayJobId: canonicalJobId,
            _pumpA666ReturnJobsForTest: () => {},
        };
    }
    const config = loadConfig(env, options.config);
    const now = options.now || Date.now;
    const wallNow = options.wallNow || Date.now;
    const execFileAsync = options.execFileAsync || runtime.execFileAsync || promisify(execFile);
    const spawnImpl = options.spawn || runtime.spawn || spawn;
    const isProcessAlive = options.processAlive || processAlive;
    const getProcessStartToken = options.processStartToken || processStartToken;
    const setIntervalImpl = options.setInterval || setInterval;
    const clearIntervalImpl = options.clearInterval || clearInterval;
    const root = path.resolve(options.root || env.A666_RETURN_RELAY_JOB_ROOT
        || path.join(os.homedir(), '.postfiat', 'wallet-proxy-8080', 'a666-return-jobs-v1'));
    const jobsRoot = path.join(root, 'jobs');
    fs.mkdirSync(jobsRoot, { recursive: true, mode: 0o700 });
    const workers = new Map();
    const submissions = new Map();
    let readiness = null;
    let readinessCheckedAt = 0;
    let pumping = false;
    const retryBaseMs = Number(options.retryBaseMs || env.A666_RETURN_RELAY_RETRY_BASE_MS || 5_000);
    const retryMaxMs = Number(options.retryMaxMs || env.A666_RETURN_RELAY_RETRY_MAX_MS || 300_000);
    const watchdogMs = Number(options.watchdogMs || 5_000);
    const directoryFor = (jobId) => path.join(jobsRoot, jobId.slice(2));
    const jobFileFor = (jobId) => path.join(directoryFor(jobId), 'job.json');
    const stateFileFor = (jobId) => path.join(directoryFor(jobId), 'worker-state.json');

    function validateState(job, state) {
        const common = state?.schema === STATE_SCHEMA && STAGES.has(state.status)
            && state.job_id === job.job_id && state.transaction_hash === job.request.transaction_hash
            && state.route_id === ROUTE_ID && Number.isSafeInteger(Number(state.updated_at_unix));
        const terminal = state?.status === 'accepted'
            ? state.retryable === false && /^[0-9a-f]{64}$/.test(String(state.return_burn_id || ''))
            : state?.status === 'failed'
                ? state.retryable === false && /^[a-z0-9_]{1,64}$/.test(String(state.code || ''))
                : state?.retryable === true;
        if (!common || !terminal) throw new Error('invalid A666 return worker state');
        const fields = ['status', 'stage_index', 'retryable', 'code', 'message', 'updated_at_unix',
            'transaction_hash', 'route_id', 'return_burn_id', 'ethereum_block_number',
            'checkpoint_finalized_height', 'pftl_height', 'pftl_tx_id'];
        return Object.fromEntries(Object.entries(state).filter(([key]) => fields.includes(key)));
    }

    function readJob(jobId) {
        const file = jobFileFor(jobId);
        if (!fs.existsSync(file)) return null;
        const job = JSON.parse(fs.readFileSync(file, 'utf8'));
        if (job?.schema !== JOB_SCHEMA || job.job_id !== jobId) throw new Error('invalid A666 return job');
        const stateFile = stateFileFor(jobId);
        return fs.existsSync(stateFile)
            ? { ...job, ...validateState(job, JSON.parse(fs.readFileSync(stateFile, 'utf8'))) } : job;
    }

    function backoff(retries) {
        return Math.min(retryMaxMs, retryBaseMs * (2 ** Math.min(16, Math.max(0, retries - 1))));
    }

    function reconcileExit(jobId, code, signal) {
        workers.delete(jobId);
        const file = jobFileFor(jobId);
        if (!fs.existsSync(file)) return;
        const original = JSON.parse(fs.readFileSync(file, 'utf8'));
        let current = null;
        try { current = readJob(jobId); } catch (_) { /* retry durable request */ }
        if (current && TERMINAL.has(current.status)) {
            atomicWrite(file, { ...original, worker_pid: null, worker_process_start_token: null,
                updated_at_unix: Math.floor(now() / 1000) });
        } else {
            const retryCount = Number(original.retry_count || 0) + 1;
            atomicWrite(file, { ...original, worker_pid: null, worker_process_start_token: null,
                retry_count: retryCount, next_retry_at_ms: now() + backoff(retryCount),
                last_worker_exit: { code: Number.isInteger(code) ? code : null, signal: signal || null },
                updated_at_unix: Math.floor(now() / 1000) });
        }
        setImmediate(pump);
    }

    function activeProcessJob() {
        for (const name of fs.readdirSync(jobsRoot).filter((value) => /^[0-9a-f]{64}$/.test(value))) {
            const job = readJob(`0x${name}`);
            if (!job || TERMINAL.has(job.status)) continue;
            if (isProcessAlive(job.worker_pid)
                && getProcessStartToken(job.worker_pid) === job.worker_process_start_token) return job;
            if (job.worker_pid) atomicWrite(jobFileFor(job.job_id), { ...job, worker_pid: null,
                worker_process_start_token: null, updated_at_unix: Math.floor(now() / 1000) });
        }
        return null;
    }

    function queuedJobs() {
        return fs.readdirSync(jobsRoot).filter((value) => /^[0-9a-f]{64}$/.test(value))
            .map((value) => readJob(`0x${value}`))
            .filter((job) => job && !TERMINAL.has(job.status) && !job.worker_pid
                && Number(job.next_retry_at_ms || 0) <= now())
            .sort((left, right) => Number(left.created_at_unix) - Number(right.created_at_unix)
                || left.job_id.localeCompare(right.job_id));
    }

    function pump() {
        if (pumping) return;
        pumping = true;
        try {
            if (workers.size > 0 || activeProcessJob()) return;
            const job = queuedJobs()[0];
            if (!job) return;
            const values = { job_file: jobFileFor(job.job_id), job_dir: directoryFor(job.job_id) };
            const logFd = fs.openSync(path.join(directoryFor(job.job_id), 'worker.log'), 'a', 0o600);
            const child = spawnImpl(config.driver_bin, expandArgs(config.driver_args, values), {
                cwd: config.cwd, stdio: ['ignore', logFd, logFd], env,
            });
            fs.closeSync(logFd);
            atomicWrite(jobFileFor(job.job_id), { ...JSON.parse(fs.readFileSync(jobFileFor(job.job_id), 'utf8')),
                worker_pid: child.pid, worker_process_start_token: getProcessStartToken(child.pid),
                worker_started_at_ms: wallNow(), updated_at_unix: Math.floor(now() / 1000) });
            const timeout = setTimeout(() => child.kill('SIGTERM'), config.worker_timeout_ms);
            timeout.unref?.();
            workers.set(job.job_id, { child, timeout });
            let settled = false;
            const finish = (code, signal) => {
                if (settled) return;
                settled = true;
                clearTimeout(timeout);
                reconcileExit(job.job_id, code, signal);
            };
            child.once('error', () => finish(null, 'spawn-error'));
            child.once('exit', finish);
            child.unref?.();
        } finally { pumping = false; }
    }

    async function refreshReadiness() {
        try {
            const { stdout } = await execFileAsync(config.driver_bin, config.readiness_args, {
                cwd: config.cwd, timeout: config.readiness_timeout_ms, maxBuffer: 1024 * 1024,
            });
            const result = JSON.parse(String(stdout).trim());
            if (result?.ok !== true || result?.ready !== true || result.route_id !== ROUTE_ID
                || result.route_config_digest !== ROUTE_DIGEST
                || String(result.controller || '').toLowerCase() !== CONTROLLER) {
                throw new Error('A666 return readiness identity mismatch');
            }
            readiness = result;
        } catch (error) {
            readiness = { ok: false, ready: false, route_id: ROUTE_ID,
                code: 'a666_return_relay_unavailable', message: 'A666 return relay is unavailable.',
                diagnostic_code: String(error.code || error.name || 'readiness_failed').slice(0, 64) };
        }
        readinessCheckedAt = now();
        return { ...readiness, checked_at_unix: Math.floor(readinessCheckedAt / 1000) };
    }

    async function relayReadiness() {
        if (!readiness || now() - readinessCheckedAt > 30_000) return refreshReadiness();
        return { ...readiness, checked_at_unix: Math.floor(readinessCheckedAt / 1000) };
    }

    async function submitUnlocked(body) {
        const request = normalizeRequest(body, config);
        const jobId = canonicalJobId(request.transaction_hash);
        const file = jobFileFor(jobId);
        const fingerprint = crypto.createHash('sha256').update(stableJson(request)).digest('hex');
        if (fs.existsSync(file)) {
            const existing = JSON.parse(fs.readFileSync(file, 'utf8'));
            if (existing.request_fingerprint !== fingerprint) throw Object.assign(
                new Error('A666 return job binding conflict'), { code: 'a666_return_job_binding_conflict' });
            pump();
            return { ...readJob(jobId), idempotent_replay: true };
        }
        const ready = await relayReadiness();
        if (ready.ready !== true) throw Object.assign(new Error(ready.message), { code: ready.code });
        const directory = directoryFor(jobId);
        fs.mkdirSync(directory, { recursive: true, mode: 0o700 });
        if (fs.existsSync(file)) return submitUnlocked(body);
        const timestamp = Math.floor(now() / 1000);
        atomicWrite(file, { schema: JOB_SCHEMA, ok: true, job_id: jobId, status: 'queued', request,
            request_fingerprint: fingerprint, retry_count: 0, next_retry_at_ms: 0, worker_pid: null,
            created_at_unix: timestamp, updated_at_unix: timestamp });
        atomicWrite(stateFileFor(jobId), { schema: STATE_SCHEMA, job_id: jobId, status: 'queued',
            stage_index: 1, retryable: true, route_id: ROUTE_ID,
            transaction_hash: request.transaction_hash, updated_at_unix: timestamp });
        pump();
        return { ...readJob(jobId), idempotent_replay: false };
    }

    async function submit(body) {
        const key = String(body?.transaction_hash || '').trim().toLowerCase();
        while (submissions.has(key)) await submissions.get(key);
        let release;
        const lock = new Promise((resolve) => { release = resolve; });
        submissions.set(key, lock);
        try { return await submitUnlocked(body); } finally { submissions.delete(key); release(); }
    }

    function status(jobId) {
        const normalized = String(jobId || '').trim().toLowerCase();
        if (!JOB_ID_RE.test(normalized)) return null;
        pump();
        return readJob(normalized);
    }

    const watchdog = setIntervalImpl(pump, watchdogMs);
    watchdog.unref?.();
    refreshReadiness().catch(() => {});
    pump();
    function close() {
        clearIntervalImpl(watchdog);
        for (const worker of workers.values()) {
            clearTimeout(worker.timeout);
            worker.child.kill('SIGTERM');
        }
        workers.clear();
    }
    return {
        a666ReturnRelayReadiness: relayReadiness,
        submitA666ReturnRelayJob: submit,
        a666ReturnRelayJobStatus: status,
        closeA666ReturnRelayJobs: close,
        canonicalA666ReturnRelayJobId: canonicalJobId,
        _pumpA666ReturnJobsForTest: pump,
    };
}

module.exports = { CONFIG_SCHEMA, JOB_SCHEMA, STATE_SCHEMA, canonicalJobId, create };
