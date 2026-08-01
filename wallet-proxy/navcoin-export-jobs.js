'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFile, spawn } = require('node:child_process');
const { promisify } = require('node:util');

const JOB_SCHEMA = 'postfiat-navcoin-export-relay-job-v1';
const STATE_SCHEMA = 'postfiat-navcoin-export-relay-state-v1';
const CONFIG_SCHEMA = 'postfiat-navcoin-export-relay-config-v1';
const ROUTE_ID_RE = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const HASH48_RE = /^[0-9a-f]{96}$/;
const HASH32_RE = /^(?:0x)?[0-9a-f]{64}$/;
const EVM_RE = /^0x[0-9a-f]{40}$/;
const JOB_ID_RE = /^0x[0-9a-f]{64}$/;
const TERMINAL = new Set(['accepted', 'failed']);
const STAGES = new Set([
    'queued', 'awaiting_source_debit', 'discovering_packet', 'waiting_for_pftl_checkpoint', 'proving_receipt',
    'submitting_ethereum_mint', 'waiting_for_ethereum_finality',
    'acknowledging_pftl', 'retry_wait', 'accepted', 'failed',
]);

function boundedInteger(value, fallback, minimum, maximum, label) {
    const number = Number(value ?? fallback);
    if (!Number.isSafeInteger(number) || number < minimum || number > maximum) {
        throw new Error(`${label} is outside the supported range`);
    }
    return number;
}

function boundedArgs(value, label) {
    if (!Array.isArray(value) || value.length > 64
        || value.some((item) => typeof item !== 'string' || item.length === 0 || item.length > 4096)) {
        throw new Error(`${label} is malformed or exceeds its bound`);
    }
    return value;
}

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
    } finally {
        fs.closeSync(fd);
    }
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

function loadConfig(env, supplied, configuredFile = '') {
    const raw = supplied || (() => {
        const configured = String(configuredFile
            || env.NAVCOIN_EXPORT_RELAY_CONFIG_FILE || '').trim();
        if (!configured) throw new Error('NAVCoin export relay config is not configured');
        const file = path.resolve(configured);
        const stat = fs.lstatSync(file);
        if (!stat.isFile() || stat.isSymbolicLink() || stat.uid !== process.getuid()
            || (stat.mode & 0o022) !== 0 || stat.size > 64 * 1024) {
            throw new Error('NAVCoin export relay config must be an owner-controlled regular file');
        }
        return JSON.parse(fs.readFileSync(file, 'utf8'));
    })();
    if (raw?.schema !== CONFIG_SCHEMA || raw.enabled !== true
        || !ROUTE_ID_RE.test(String(raw.route_id || ''))
        || !HASH48_RE.test(String(raw.route_config_digest || ''))
        || !EVM_RE.test(String(raw.wrapped_token || '').toLowerCase())
        || !Array.isArray(raw.driver_args) || !Array.isArray(raw.inspect_args)
        || !Array.isArray(raw.readiness_args)) {
        throw new Error('invalid NAVCoin export relay config');
    }
    const driverBin = supplied
        ? path.resolve(raw.driver_bin)
        : securePinnedFile(raw.driver_bin, raw.driver_sha256, 'NAVCoin export relay driver');
    const maxAmount = BigInt(String(raw.max_amount_atoms || '0'));
    if (maxAmount <= 0n) throw new Error('invalid NAVCoin export relay amount cap');
    return {
        ...raw,
        route_config_digest: raw.route_config_digest.toLowerCase(),
        wrapped_token: raw.wrapped_token.toLowerCase(),
        driver_bin: driverBin,
        cwd: path.resolve(raw.cwd),
        max_amount_atoms: maxAmount,
        driver_args: boundedArgs(raw.driver_args, 'NAVCoin export driver arguments'),
        inspect_args: boundedArgs(raw.inspect_args, 'NAVCoin export inspection arguments'),
        readiness_args: boundedArgs(raw.readiness_args, 'NAVCoin export readiness arguments'),
        worker_timeout_ms: boundedInteger(raw.worker_timeout_ms, 4 * 60 * 60 * 1000, 1_000, 24 * 60 * 60 * 1_000, 'NAVCoin export worker timeout'),
        inspect_timeout_ms: boundedInteger(raw.inspect_timeout_ms, 60_000, 100, 10 * 60 * 1_000, 'NAVCoin export inspection timeout'),
        readiness_timeout_ms: boundedInteger(raw.readiness_timeout_ms, 90_000, 100, 10 * 60 * 1_000, 'NAVCoin export readiness timeout'),
    };
}

function expandArgs(args, values) {
    return args.map((argument) => String(argument).replace(/\{([a-z_]+)\}/g, (_, key) => {
        if (!(key in values)) throw new Error(`unknown NAVCoin relay placeholder: ${key}`);
        return values[key];
    }));
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

function canonicalJobId(routeId, packetHash) {
    if (!ROUTE_ID_RE.test(String(routeId || '')) || !HASH48_RE.test(String(packetHash || ''))) {
        throw new Error('invalid NAVCoin export job identity');
    }
    const route = Buffer.from(routeId, 'utf8');
    const routeLength = Buffer.alloc(2);
    routeLength.writeUInt16BE(route.length);
    const digest = crypto.createHash('sha256')
        .update('postfiat.navcoin.export-relay.job.v1\0')
        .update(routeLength)
        .update(route)
        .update(Buffer.from(packetHash, 'hex'))
        .digest('hex');
    return `0x${digest}`;
}

function normalizeRequest(body, config) {
    const request = {
        route_id: String(body?.route_id || '').trim(),
        route_config_digest: String(body?.route_config_digest || '').trim().toLowerCase(),
        packet_hash: String(body?.packet_hash || '').trim().toLowerCase(),
        packet_digest: String(body?.packet_digest || '').trim().toLowerCase().replace(/^0x/, ''),
        ethereum_recipient: String(body?.ethereum_recipient || '').trim().toLowerCase(),
        amount_atoms: String(body?.amount_atoms || '').trim(),
        deadline_seconds: Number(body?.deadline_seconds),
    };
    let amount = 0n;
    try { amount = BigInt(request.amount_atoms); } catch (_) { /* rejected below */ }
    if (request.route_id !== config.route_id
        || request.route_config_digest !== config.route_config_digest
        || !HASH48_RE.test(request.packet_hash) || !HASH32_RE.test(request.packet_digest)
        || !EVM_RE.test(request.ethereum_recipient) || amount <= 0n
        || amount > config.max_amount_atoms
        || !Number.isSafeInteger(request.deadline_seconds)
        || request.deadline_seconds <= Math.floor(Date.now() / 1000) + 3600
        || request.deadline_seconds > Math.floor(Date.now() / 1000) + 172800) {
        throw Object.assign(new Error('invalid NAVCoin export relay request'), {
            code: 'invalid_navcoin_export_relay_request',
        });
    }
    return request;
}

function validateInspection(request, inspection) {
    if (inspection?.ok !== true || inspection.route_id !== request.route_id
        || inspection.route_config_digest !== request.route_config_digest
        || inspection.packet_hash !== request.packet_hash
        || inspection.packet_digest !== request.packet_digest
        || String(inspection.ethereum_recipient || '').toLowerCase() !== request.ethereum_recipient
        || String(inspection.amount_atoms) !== request.amount_atoms
        || !Number.isSafeInteger(Number(inspection.source_height))
        || Number(inspection.source_height) <= 0
        || inspection.packet_status !== 'SourceDebited'
        || inspection.ethereum_packet_consumed !== false) {
        throw Object.assign(new Error('NAVCoin export packet inspection failed closed'), {
            code: 'navcoin_export_packet_binding_mismatch',
        });
    }
    return inspection;
}

function createRoute(runtime = {}, options = {}) {
    const env = options.env || process.env;
    const now = options.now || Date.now;
    const wallNow = options.wallNow || Date.now;
    const execFileAsync = options.execFileAsync || runtime.execFileAsync || promisify(execFile);
    const spawnImpl = options.spawn || runtime.spawn || spawn;
    const isProcessAlive = options.processAlive || processAlive;
    const getProcessStartToken = options.processStartToken || processStartToken;
    const setIntervalImpl = options.setInterval || setInterval;
    const clearIntervalImpl = options.clearInterval || clearInterval;
    const config = loadConfig(env, options.config);
    const root = path.resolve(options.root || env.NAVCOIN_EXPORT_RELAY_JOB_ROOT
        || path.join(os.homedir(), '.postfiat', 'wallet-proxy-8080', 'navcoin-export-jobs-v1'));
    const jobsRoot = path.join(root, 'jobs');
    fs.mkdirSync(jobsRoot, { recursive: true, mode: 0o700 });
    const workers = new Map();
    const submissions = new Map();
    let readiness = null;
    let readinessCheckedAt = 0;
    let pumping = false;
    const retryBaseMs = boundedInteger(options.retryBaseMs ?? env.NAVCOIN_EXPORT_RELAY_RETRY_BASE_MS, 5_000, 100, 60_000, 'NAVCoin export retry base');
    const retryMaxMs = boundedInteger(options.retryMaxMs ?? env.NAVCOIN_EXPORT_RELAY_RETRY_MAX_MS, 300_000, 1_000, 3_600_000, 'NAVCoin export retry maximum');
    const watchdogMs = boundedInteger(options.watchdogMs, 5_000, 250, 60_000, 'NAVCoin export watchdog interval');
    if (retryBaseMs > retryMaxMs) throw new Error('NAVCoin export retry base exceeds maximum');

    const directoryFor = (jobId) => path.join(jobsRoot, jobId.slice(2));
    const jobFileFor = (jobId) => path.join(directoryFor(jobId), 'job.json');
    const stateFileFor = (jobId) => path.join(directoryFor(jobId), 'worker-state.json');

    function validateState(job, state) {
        const common = state?.schema === STATE_SCHEMA && STAGES.has(state.status)
            && state.job_id === job.job_id && state.packet_hash === job.request.packet_hash
            && state.route_id === config.route_id
            && Number.isSafeInteger(Number(state.updated_at_unix));
        const terminal = state?.status === 'accepted'
            ? state.retryable === false && HASH48_RE.test(String(state.receipt_id || ''))
            : state?.status === 'failed'
                ? state.retryable === false && /^[a-z0-9_]{1,64}$/.test(String(state.code || ''))
                : state?.retryable === true;
        if (!common || !terminal) throw new Error('invalid NAVCoin export worker state');
        const publicFields = [
            'status', 'stage_index', 'retryable', 'code', 'message', 'updated_at_unix',
            'packet_hash', 'route_id', 'source_height', 'ethereum_tx_hash', 'receipt_id',
        ];
        return Object.fromEntries(Object.entries(state).filter(([key]) => publicFields.includes(key)));
    }

    function readJob(jobId) {
        const file = jobFileFor(jobId);
        if (!fs.existsSync(file)) return null;
        const job = JSON.parse(fs.readFileSync(file, 'utf8'));
        if (job?.schema !== JOB_SCHEMA || job.job_id !== jobId
            || job.request?.route_id !== config.route_id
            || job.request?.route_config_digest !== config.route_config_digest) {
            throw new Error('invalid NAVCoin export job');
        }
        const stateFile = stateFileFor(jobId);
        if (!fs.existsSync(stateFile)) return job;
        return { ...job, ...validateState(job, JSON.parse(fs.readFileSync(stateFile, 'utf8'))) };
    }

    function backoff(retries) {
        return Math.min(retryMaxMs, retryBaseMs * (2 ** Math.min(16, Math.max(0, retries - 1))));
    }

    function reconcileExit(jobId, code, signal) {
        workers.delete(jobId);
        const file = jobFileFor(jobId);
        if (!fs.existsSync(file)) return;
        const job = JSON.parse(fs.readFileSync(file, 'utf8'));
        let state = null;
        try { state = readJob(jobId); } catch (_) { /* retry from durable job */ }
        if (state && TERMINAL.has(state.status)) {
            atomicWrite(file, { ...job, worker_pid: null, worker_process_start_token: null,
                updated_at_unix: Math.floor(now() / 1000) });
        } else {
            const retryCount = Number(job.retry_count || 0) + 1;
            atomicWrite(file, { ...job, worker_pid: null, worker_process_start_token: null,
                retry_count: retryCount, next_retry_at_ms: now() + backoff(retryCount),
                last_worker_exit: { code: Number.isInteger(code) ? code : null, signal: signal || null },
                updated_at_unix: Math.floor(now() / 1000) });
        }
        setImmediate(pump);
    }

    function activeProcessJob() {
        for (const name of fs.readdirSync(jobsRoot).filter((value) => /^[0-9a-f]{64}$/.test(value))) {
            const jobId = `0x${name}`;
            const job = readJob(jobId);
            if (!job || TERMINAL.has(job.status)) continue;
            if (isProcessAlive(job.worker_pid)
                && getProcessStartToken(job.worker_pid) === job.worker_process_start_token) {
                return job;
            }
            if (job.worker_pid) {
                atomicWrite(jobFileFor(jobId), { ...job, worker_pid: null,
                    worker_process_start_token: null, updated_at_unix: Math.floor(now() / 1000) });
            }
        }
        return null;
    }

    function queuedJobs() {
        return fs.readdirSync(jobsRoot)
            .filter((value) => /^[0-9a-f]{64}$/.test(value))
            .map((value) => readJob(`0x${value}`))
            .filter((job) => job && !TERMINAL.has(job.status)
                && !job.worker_pid && Number(job.next_retry_at_ms || 0) <= now())
            .sort((left, right) => Number(left.source_height) - Number(right.source_height)
                || Number(left.created_at_unix) - Number(right.created_at_unix)
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
            const started = {
                ...JSON.parse(fs.readFileSync(jobFileFor(job.job_id), 'utf8')),
                worker_pid: child.pid,
                worker_process_start_token: getProcessStartToken(child.pid),
                worker_started_at_ms: wallNow(),
                updated_at_unix: Math.floor(now() / 1000),
            };
            atomicWrite(jobFileFor(job.job_id), started);
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
            if (result?.ok !== true || result?.ready !== true
                || result.route_id !== config.route_id
                || result.route_config_digest !== config.route_config_digest
                || String(result.wrapped_token || '').toLowerCase() !== config.wrapped_token) {
                throw new Error('NAVCoin export readiness identity mismatch');
            }
            readiness = result;
        } catch (error) {
            readiness = { ok: false, ready: false, route_id: config.route_id,
                code: 'navcoin_export_relay_unavailable', message: 'NAVCoin export relay is unavailable.',
                diagnostic_code: String(error.code || error.name || 'readiness_failed').slice(0, 64) };
        }
        readinessCheckedAt = now();
        return { ...readiness, checked_at_unix: Math.floor(readinessCheckedAt / 1000) };
    }

    async function relayReadiness(routeId = config.route_id) {
        if (routeId !== config.route_id) return {
            ok: false, ready: false, route_id: routeId,
            code: 'navcoin_export_route_not_configured',
            message: 'NAVCoin export relay is not configured for this route.',
        };
        if (!readiness || now() - readinessCheckedAt > 30_000) return refreshReadiness();
        return { ...readiness, checked_at_unix: Math.floor(readinessCheckedAt / 1000) };
    }

    async function submitUnlocked(body) {
        const request = normalizeRequest(body, config);
        const jobId = canonicalJobId(request.route_id, request.packet_hash);
        const directory = directoryFor(jobId);
        const file = jobFileFor(jobId);
        const fingerprint = crypto.createHash('sha256').update(stableJson(request)).digest('hex');
        if (fs.existsSync(file)) {
            const existing = JSON.parse(fs.readFileSync(file, 'utf8'));
            if (existing.request_fingerprint !== fingerprint) {
                throw Object.assign(new Error('NAVCoin export job binding conflict'), { code: 'navcoin_export_job_binding_conflict' });
            }
            pump();
            return { ...readJob(jobId), idempotent_replay: true };
        }
        const ready = await relayReadiness();
        if (ready.ready !== true) throw Object.assign(new Error(ready.message), { code: ready.code });
        fs.mkdirSync(directory, { recursive: true, mode: 0o700 });
        if (fs.existsSync(file)) {
            const existing = JSON.parse(fs.readFileSync(file, 'utf8'));
            if (existing.request_fingerprint !== fingerprint) {
                throw Object.assign(new Error('NAVCoin export job binding conflict'), { code: 'navcoin_export_job_binding_conflict' });
            }
            pump();
            return { ...readJob(jobId), idempotent_replay: true };
        }
        const timestamp = Math.floor(now() / 1000);
        atomicWrite(file, { schema: JOB_SCHEMA, ok: true, job_id: jobId, status: 'awaiting_source_debit',
            request, request_fingerprint: fingerprint, source_height: null,
            source_status_at_creation: 'AwaitingSourceDebit',
            retry_count: 0, next_retry_at_ms: 0, worker_pid: null,
            created_at_unix: timestamp, updated_at_unix: timestamp });
        atomicWrite(stateFileFor(jobId), { schema: STATE_SCHEMA, job_id: jobId,
            status: 'awaiting_source_debit', stage_index: 1, retryable: true,
            route_id: config.route_id,
            packet_hash: request.packet_hash, source_height: null,
            updated_at_unix: timestamp });
        pump();
        return { ...readJob(jobId), idempotent_replay: false };
    }

    async function submit(body) {
        const packetHash = String(body?.packet_hash || '').trim().toLowerCase();
        const key = HASH48_RE.test(packetHash) ? packetHash : crypto.randomUUID();
        while (submissions.has(key)) await submissions.get(key);
        let release;
        const lock = new Promise((resolve) => { release = resolve; });
        submissions.set(key, lock);
        try { return await submitUnlocked(body); } finally {
            submissions.delete(key);
            release();
        }
    }

    function status(routeId, jobId) {
        if (routeId !== config.route_id) return null;
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
        navcoinExportRelayReadiness: relayReadiness,
        submitNavcoinExportRelayJob: submit,
        navcoinExportRelayJobStatus: status,
        closeNavcoinExportRelayJobs: close,
        canonicalNavcoinExportRelayJobId: canonicalJobId,
        _pumpNavcoinExportJobsForTest: pump,
    };
}

function configuredFiles(env) {
    const single = String(env.NAVCOIN_EXPORT_RELAY_CONFIG_FILE || '').trim();
    const multiple = String(env.NAVCOIN_EXPORT_RELAY_CONFIG_FILES || '').trim();
    if (single && multiple) {
        throw new Error('configure either one NAVCoin export relay file or a file list, not both');
    }
    const files = (multiple ? multiple.split(',') : (single ? [single] : []))
        .map((value) => value.trim()).filter(Boolean);
    if (files.length > 64 || new Set(files).size !== files.length) {
        throw new Error('NAVCoin export relay config file list must contain at most 64 unique files');
    }
    return files;
}

function create(runtime = {}, options = {}) {
    const env = options.env || process.env;
    const supplied = Array.isArray(options.configs)
        ? options.configs
        : (options.config ? [options.config] : []);
    if (supplied.length > 64) throw new Error('at most 64 NAVCoin export routes may be configured');
    const configs = supplied.length > 0
        ? supplied.map((config) => loadConfig(env, config))
        : configuredFiles(env).map((file) => loadConfig(env, null, file));
    if (configs.length === 0) {
        const unavailable = async (routeId = '') => ({ ok: false, ready: false,
            route_id: String(routeId || ''), code: 'navcoin_export_relay_not_configured',
            message: 'NAVCoin export relay is not configured on this wallet host.' });
        return {
            navcoinExportRelayReadiness: unavailable,
            submitNavcoinExportRelayJob: async () => { throw Object.assign(
                new Error('NAVCoin export relay is not configured.'),
                { code: 'navcoin_export_relay_not_configured' }); },
            navcoinExportRelayJobStatus: () => null,
            closeNavcoinExportRelayJobs: () => {},
            canonicalNavcoinExportRelayJobId: canonicalJobId,
            _pumpNavcoinExportJobsForTest: () => {},
        };
    }
    const routes = new Map();
    for (const config of configs) {
        if (routes.has(config.route_id)) throw new Error('duplicate NAVCoin export route config');
        const routeRoot = configs.length === 1 ? options.root : path.join(
            path.resolve(options.root || env.NAVCOIN_EXPORT_RELAY_JOB_ROOT
                || path.join(os.homedir(), '.postfiat', 'wallet-proxy-8080', 'navcoin-export-jobs-v1')),
            crypto.createHash('sha256').update(config.route_id).digest('hex'),
        );
        routes.set(config.route_id, createRoute(runtime, { ...options, config, root: routeRoot }));
    }
    return {
        navcoinExportRelayReadiness: (routeId) => {
            const route = routes.get(routeId);
            return route ? route.navcoinExportRelayReadiness(routeId) : Promise.resolve({
                ok: false, ready: false, route_id: String(routeId || ''),
                code: 'navcoin_export_route_not_configured',
                message: 'NAVCoin export relay is not configured for this route.',
            });
        },
        submitNavcoinExportRelayJob: (body) => {
            const route = routes.get(String(body?.route_id || ''));
            if (!route) throw Object.assign(new Error('NAVCoin export route is not configured.'),
                { code: 'navcoin_export_route_not_configured' });
            return route.submitNavcoinExportRelayJob(body);
        },
        navcoinExportRelayJobStatus: (routeId, jobId) =>
            routes.get(routeId)?.navcoinExportRelayJobStatus(routeId, jobId) || null,
        closeNavcoinExportRelayJobs: () => {
            for (const route of routes.values()) route.closeNavcoinExportRelayJobs();
        },
        canonicalNavcoinExportRelayJobId: canonicalJobId,
        _pumpNavcoinExportJobsForTest: () => {
            for (const route of routes.values()) route._pumpNavcoinExportJobsForTest();
        },
    };
}

module.exports = { CONFIG_SCHEMA, JOB_SCHEMA, STATE_SCHEMA, canonicalJobId, create };
