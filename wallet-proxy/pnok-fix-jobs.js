'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawn } = require('node:child_process');

const CONFIG_SCHEMA = 'postfiat-pnok-private-fix-wallet-config-v1';
const JOB_SCHEMA = 'postfiat-pnok-private-fix-wallet-job-v1';
const PUBLIC_SCHEMA = 'postfiat-pnok-private-fix-wallet-job-status-v1';
const JOB_ID_RE = /^0x[0-9a-f]{64}$/;
const REQUEST_ID_RE = /^[a-z0-9][a-z0-9-]{0,63}$/;
const ASSET_ID_RE = /^[0-9a-f]{96}$/;
const ADDRESS_RE = /^pf[0-9a-f]{40}$/;
const TERMINAL = new Set(['accepted', 'failed']);
const RUNNABLE = new Set(['queued', 'retry_wait']);

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

function secureFile(file, digest, label, { executable = false, privateFile = false } = {}) {
    const absolute = path.resolve(String(file || ''));
    const stat = fs.lstatSync(absolute);
    const mode = stat.mode & 0o777;
    const ownerAllowed = stat.uid === process.getuid() || (executable && stat.uid === 0);
    if (!stat.isFile() || stat.isSymbolicLink() || !ownerAllowed
        || (mode & 0o022) !== 0 || (executable && (mode & 0o100) === 0)
        || (privateFile && mode !== 0o600)
        || (digest && (!/^[0-9a-f]{64}$/.test(String(digest)) || sha256File(absolute) !== digest))) {
        throw new Error(`${label} failed secure file validation`);
    }
    return absolute;
}

function loadConfig(env, supplied) {
    const raw = supplied || (() => {
        const file = path.resolve(String(env.PNOK_FIX_WALLET_CONFIG_FILE || ''));
        if (!file) throw new Error('pNOK FIX wallet config is not configured');
        const stat = fs.lstatSync(file);
        if (!stat.isFile() || stat.isSymbolicLink() || stat.uid !== process.getuid()
            || (stat.mode & 0o022) !== 0 || stat.size > 64 * 1024) {
            throw new Error('pNOK FIX wallet config must be an owner-controlled regular file');
        }
        return JSON.parse(fs.readFileSync(file, 'utf8'));
    })();
    const valid = raw?.schema === CONFIG_SCHEMA && raw.enabled === true
        && ASSET_ID_RE.test(String(raw.base_asset_id || ''))
        && ASSET_ID_RE.test(String(raw.quote_asset_id || ''))
        && ADDRESS_RE.test(String(raw.fix_operator || ''))
        && ADDRESS_RE.test(String(raw.demo_wallet || ''))
        && String(raw.base_atoms) === '20000000'
        && String(raw.quote_atoms) === '210'
        && raw.base_symbol === 'pfUSDC' && raw.quote_symbol === 'pNOK'
        && Number(raw.base_precision) === 6 && Number(raw.quote_precision) === 0
        && raw.trust_label === 'controlled sandbox checkpoint'
        && raw.execution_label === 'private on PFTL';
    if (!valid) throw new Error('invalid pNOK FIX wallet config');
    const service = new URL(String(raw.resident_service_url || ''));
    if (service.protocol !== 'http:' || !['127.0.0.1', 'localhost', '::1'].includes(service.hostname)) {
        throw new Error('pNOK FIX resident service must use loopback HTTP');
    }
    const driverScript = supplied
        ? path.resolve(raw.driver_script)
        : secureFile(raw.driver_script, raw.driver_sha256, 'pNOK FIX driver');
    const pythonBin = supplied
        ? path.resolve(raw.python_bin)
        : secureFile(raw.python_bin, raw.python_sha256, 'pNOK FIX Python', { executable: true });
    const facilityKey = supplied
        ? path.resolve(raw.facility_key_file)
        : secureFile(raw.facility_key_file, null, 'pNOK FIX facility key', { privateFile: true });
    const config = {
        ...raw,
        driver_script: driverScript,
        python_bin: pythonBin,
        facility_key_file: facilityKey,
        resident_service_url: service.toString().replace(/\/$/, ''),
        cwd: path.resolve(raw.cwd),
        worker_timeout_ms: Number(raw.worker_timeout_ms || 45 * 60 * 1000),
        max_retries: Number(raw.max_retries ?? 3),
        retry_delay_ms: Number(raw.retry_delay_ms || 5_000),
    };
    if (!Number.isSafeInteger(config.worker_timeout_ms) || config.worker_timeout_ms < 60_000
        || !Number.isSafeInteger(config.max_retries) || config.max_retries < 1 || config.max_retries > 10
        || !Number.isSafeInteger(config.retry_delay_ms) || config.retry_delay_ms < 100) {
        throw new Error('invalid pNOK FIX worker limits');
    }
    return config;
}

function processAlive(pid) {
    if (!Number.isInteger(pid) || pid <= 1) return false;
    try { process.kill(pid, 0); return true; } catch (_) { return false; }
}

function jobIdFor(direction, clientRequestId) {
    return `0x${crypto.createHash('sha256')
        .update('postfiat.pnok.private_fix.wallet_job.v1\0')
        .update(direction)
        .update('\0')
        .update(clientRequestId)
        .digest('hex')}`;
}

function normalizeRequest(body, config, direction) {
    const request = {
        direction,
        client_request_id: String(body?.client_request_id || '').trim().toLowerCase(),
        base_asset_id: String(body?.base_asset_id || '').trim().toLowerCase(),
        quote_asset_id: String(body?.quote_asset_id || '').trim().toLowerCase(),
        base_atoms: String(body?.base_atoms || '').trim(),
    };
    if (!['acquire', 'restore'].includes(direction)
        || !REQUEST_ID_RE.test(request.client_request_id)
        || request.base_asset_id !== config.base_asset_id
        || request.quote_asset_id !== config.quote_asset_id
        || request.base_atoms !== String(config.base_atoms)) {
        throw Object.assign(new Error('invalid pNOK private FIX wallet request'), {
            code: 'invalid_pnok_fix_wallet_request',
        });
    }
    return request;
}

async function jsonFetch(url, timeoutMs = 5_000, fetchImpl = fetch) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), timeoutMs);
    try {
        const response = await fetchImpl(url, { cache: 'no-store', signal: controller.signal });
        const body = await response.json();
        if (!response.ok || body?.ok !== true) throw new Error('resident service rejected request');
        return body;
    } finally {
        clearTimeout(timeout);
    }
}

function exactNote(notes, owner, assetId, amountAtoms) {
    const matches = notes.filter((note) => note?.wallet_address === owner
        && note?.asset_id === assetId && String(note?.amount_atoms) === String(amountAtoms)
        && note?.state === 'spendable' && /^[0-9a-f]{64}$/.test(String(note?.id || '')));
    if (matches.length !== 1) {
        throw Object.assign(new Error('exact private demo inventory is unavailable'), {
            code: 'pnok_fix_inventory_unavailable',
        });
    }
    return matches[0];
}

function create(runtime = {}, options = {}) {
    const env = options.env || process.env;
    const now = options.now || Date.now;
    const spawnImpl = options.spawn || runtime.spawn || spawn;
    const fetchImpl = options.fetch || fetch;
    const isProcessAlive = options.processAlive || processAlive;
    const setIntervalImpl = options.setInterval || setInterval;
    const clearIntervalImpl = options.clearInterval || clearInterval;
    if (!options.config && !env.PNOK_FIX_WALLET_CONFIG_FILE) {
        const unavailable = async () => ({
            ok: false, ready: false, schema: 'postfiat-pnok-private-fix-wallet-readiness-v1',
            code: 'pnok_fix_wallet_not_configured',
            message: 'The pNOK private FIX demo is not configured on this wallet host.',
        });
        return {
            pnokFixWalletReadiness: unavailable,
            submitPnokFixWalletJob: async () => { throw Object.assign(new Error('pNOK FIX wallet is not configured'), { code: 'pnok_fix_wallet_not_configured' }); },
            submitPnokFixRestoreJob: async () => { throw Object.assign(new Error('pNOK FIX wallet is not configured'), { code: 'pnok_fix_wallet_not_configured' }); },
            pnokFixWalletJobStatus: () => null,
            closePnokFixWalletJobs: () => {},
        };
    }
    const config = loadConfig(env, options.config);
    const root = path.resolve(options.root || env.PNOK_FIX_WALLET_JOB_ROOT
        || path.join(os.homedir(), '.postfiat', 'wallet-proxy-8080', 'pnok-fix-jobs-v1'));
    const jobsRoot = path.join(root, 'jobs');
    fs.mkdirSync(jobsRoot, { recursive: true, mode: 0o700 });
    const workers = new Map();
    const submissions = new Map();
    let pumping = false;
    let closed = false;

    const directoryFor = (jobId) => path.join(jobsRoot, jobId.slice(2));
    const jobFileFor = (jobId) => path.join(directoryFor(jobId), 'job.json');
    const intentDirFor = (jobId) => path.join(directoryFor(jobId), 'intent');

    function readJob(jobId) {
        const file = jobFileFor(jobId);
        if (!fs.existsSync(file)) return null;
        const job = JSON.parse(fs.readFileSync(file, 'utf8'));
        if (job?.schema !== JOB_SCHEMA || job.job_id !== jobId) throw new Error('invalid pNOK FIX job');
        if (job.status === 'running' && !isProcessAlive(job.worker_pid)) {
            job.status = 'retry_wait';
            job.worker_pid = null;
            job.next_retry_at_ms = now();
            job.updated_at_unix_ms = now();
            atomicWrite(file, job);
        }
        return job;
    }

    function driverPublicStatus(job) {
        const file = path.join(intentDirFor(job.job_id), 'public', 'status.json');
        if (!fs.existsSync(file)) return null;
        const status = JSON.parse(fs.readFileSync(file, 'utf8'));
        return status?.schema === 'postfiat-pnok-private-fix-demo-public-status-v1' ? status : null;
    }

    function publicJob(job, idempotentReplay = false) {
        const execution = driverPublicStatus(job);
        return {
            ok: true,
            schema: PUBLIC_SCHEMA,
            job_id: job.job_id,
            client_request_id: job.request.client_request_id,
            direction: job.request.direction,
            status: job.status,
            execution_stage: execution?.stage || null,
            fix_packet_hash: execution?.fix_packet_hash || null,
            reservation_id: execution?.reservation_id || null,
            nullifier_occurrence_counts: execution?.nullifier_occurrence_counts || null,
            output_occurrence_counts: execution?.output_occurrence_counts || null,
            replay_rejected_without_effect: execution?.replay_rejected_without_effect ?? null,
            supply_unchanged: execution?.supply_unchanged ?? null,
            execution_privacy: config.execution_label,
            source_boundary: config.trust_label,
            base_symbol: config.base_symbol,
            quote_symbol: config.quote_symbol,
            base_atoms: String(config.base_atoms),
            quote_atoms: String(config.quote_atoms),
            ratio_display: '10.500000 pNOK/pfUSDC',
            fee_atoms: '0',
            price_impact_bps: 0,
            retry_count: job.retry_count,
            code: job.code || null,
            message: job.message || null,
            created_at_unix_ms: job.created_at_unix_ms,
            updated_at_unix_ms: job.updated_at_unix_ms,
            idempotent_replay: idempotentReplay,
        };
    }

    function commandFor(job) {
        const acquire = job.request.direction === 'acquire';
        const walletAddress = acquire ? config.demo_wallet : config.fix_operator;
        const liquidityAddress = acquire ? config.fix_operator : config.demo_wallet;
        const intentId = `pnok-wallet-${job.job_id.slice(2, 34)}`;
        return [
            config.driver_script,
            'run',
            '--intent-dir', intentDirFor(job.job_id),
            '--intent-id', intentId,
            '--wallet-address', walletAddress,
            '--facility-operator', config.fix_operator,
            '--liquidity-wallet-address', liquidityAddress,
            '--facility-key-file', config.facility_key_file,
            '--quote-asset-id', config.quote_asset_id,
            '--wallet-note-commitment', job.private_inputs.wallet_note_commitment,
            '--liquidity-commitment', job.private_inputs.liquidity_commitment,
            '--service-url', config.resident_service_url,
            acquire ? '--verify-replay' : '--no-verify-replay',
        ];
    }

    function finish(jobId, code, signal) {
        const worker = workers.get(jobId);
        if (worker?.timeout) clearTimeout(worker.timeout);
        workers.delete(jobId);
        const job = readJob(jobId);
        if (!job || TERMINAL.has(job.status)) return;
        const execution = driverPublicStatus(job);
        const accepted = code === 0 && execution?.stage === 'complete'
            && execution?.supply_unchanged === true
            && Array.isArray(execution?.nullifier_occurrence_counts)
            && execution.nullifier_occurrence_counts.every((value) => value === 1)
            && Array.isArray(execution?.output_occurrence_counts)
            && execution.output_occurrence_counts.every((value) => value === 1)
            && (job.request.direction !== 'acquire' || execution.replay_rejected_without_effect === true);
        if (accepted) {
            Object.assign(job, { status: 'accepted', worker_pid: null, code: null,
                message: 'Private FIX swap finalized and the owned output was scanned.',
                updated_at_unix_ms: now() });
        } else if (job.retry_count < config.max_retries) {
            Object.assign(job, { status: 'retry_wait', worker_pid: null,
                next_retry_at_ms: now() + config.retry_delay_ms,
                code: 'pnok_fix_worker_retry',
                message: `Durable execution will resume after worker exit ${code ?? signal ?? 'unknown'}.`,
                updated_at_unix_ms: now() });
        } else {
            Object.assign(job, { status: 'failed', worker_pid: null,
                code: 'pnok_fix_worker_failed',
                message: 'Private FIX execution exhausted its bounded automatic retries.',
                updated_at_unix_ms: now() });
        }
        atomicWrite(jobFileFor(jobId), job);
        setImmediate(pump);
    }

    function pump() {
        if (closed || pumping || workers.size > 0) return;
        pumping = true;
        try {
            const candidates = fs.readdirSync(jobsRoot, { withFileTypes: true })
                .filter((entry) => entry.isDirectory() && /^[0-9a-f]{64}$/.test(entry.name))
                .map((entry) => readJob(`0x${entry.name}`))
                .filter((job) => job && RUNNABLE.has(job.status) && Number(job.next_retry_at_ms || 0) <= now())
                .sort((a, b) => a.created_at_unix_ms - b.created_at_unix_ms);
            const job = candidates[0];
            if (!job) return;
            job.retry_count += 1;
            job.status = 'running';
            job.updated_at_unix_ms = now();
            const output = fs.openSync(path.join(directoryFor(job.job_id), `worker-${job.retry_count}.stdout.log`), 'a', 0o600);
            const error = fs.openSync(path.join(directoryFor(job.job_id), `worker-${job.retry_count}.stderr.log`), 'a', 0o600);
            const child = spawnImpl(config.python_bin, commandFor(job), {
                cwd: config.cwd,
                stdio: ['ignore', output, error],
                env: { ...process.env, PYTHONUNBUFFERED: '1' },
            });
            fs.closeSync(output);
            fs.closeSync(error);
            job.worker_pid = child.pid;
            atomicWrite(jobFileFor(job.job_id), job);
            const timeout = setTimeout(() => child.kill('SIGTERM'), config.worker_timeout_ms);
            timeout.unref?.();
            workers.set(job.job_id, { child, timeout });
            let settled = false;
            const done = (code, signal) => {
                if (settled) return;
                settled = true;
                finish(job.job_id, code, signal);
            };
            child.once('error', () => done(null, 'spawn-error'));
            child.once('exit', done);
            child.unref?.();
        } finally {
            pumping = false;
        }
    }

    async function readiness() {
        try {
            const [ready, notesResponse] = await Promise.all([
                jsonFetch(`${config.resident_service_url}/asset-orchard/readiness`, 5_000, fetchImpl),
                jsonFetch(`${config.resident_service_url}/asset-orchard/notes`, 5_000, fetchImpl),
            ]);
            const notes = Array.isArray(notesResponse.notes) ? notesResponse.notes : [];
            let acquireReady = true;
            let restoreReady = true;
            try {
                exactNote(notes, config.demo_wallet, config.base_asset_id, config.base_atoms);
                exactNote(notes, config.fix_operator, config.quote_asset_id, config.quote_atoms);
            } catch (_) { acquireReady = false; }
            try {
                exactNote(notes, config.fix_operator, config.base_asset_id, config.base_atoms);
                exactNote(notes, config.demo_wallet, config.quote_asset_id, config.quote_atoms);
            } catch (_) { restoreReady = false; }
            return {
                ok: true,
                ready: ready.ready === true && acquireReady,
                schema: 'postfiat-pnok-private-fix-wallet-readiness-v1',
                resident_prover_ready: ready.ready === true,
                acquire_inventory_ready: acquireReady,
                restore_inventory_ready: restoreReady,
                base_asset_id: config.base_asset_id,
                quote_asset_id: config.quote_asset_id,
                base_symbol: config.base_symbol,
                quote_symbol: config.quote_symbol,
                base_precision: config.base_precision,
                quote_precision: config.quote_precision,
                base_atoms: String(config.base_atoms),
                quote_atoms: String(config.quote_atoms),
                ratio_numerator: 21,
                ratio_denominator: 2_000_000,
                fee_atoms: '0',
                price_impact_bps: 0,
                execution_privacy: config.execution_label,
                source_boundary: config.trust_label,
            };
        } catch (_) {
            return { ok: false, ready: false,
                schema: 'postfiat-pnok-private-fix-wallet-readiness-v1',
                code: 'pnok_fix_wallet_unavailable',
                message: 'The resident private FIX service is unavailable.' };
        }
    }

    async function submitUnlocked(body, direction) {
        const request = normalizeRequest(body, config, direction);
        const jobId = jobIdFor(direction, request.client_request_id);
        const file = jobFileFor(jobId);
        const fingerprint = crypto.createHash('sha256').update(stableJson(request)).digest('hex');
        if (fs.existsSync(file)) {
            const existing = readJob(jobId);
            if (existing.request_fingerprint !== fingerprint) {
                throw Object.assign(new Error('pNOK FIX job binding conflict'), { code: 'pnok_fix_job_binding_conflict' });
            }
            pump();
            return publicJob(existing, true);
        }
        const notesResponse = await jsonFetch(
            `${config.resident_service_url}/asset-orchard/notes`, 5_000, fetchImpl
        );
        const notes = Array.isArray(notesResponse.notes) ? notesResponse.notes : [];
        const acquire = direction === 'acquire';
        const walletInput = exactNote(notes,
            acquire ? config.demo_wallet : config.fix_operator,
            config.base_asset_id, config.base_atoms);
        const liquidityInput = exactNote(notes,
            acquire ? config.fix_operator : config.demo_wallet,
            config.quote_asset_id, config.quote_atoms);
        const timestamp = now();
        const job = {
            schema: JOB_SCHEMA,
            job_id: jobId,
            request,
            request_fingerprint: fingerprint,
            private_inputs: {
                wallet_note_commitment: walletInput.id,
                liquidity_commitment: liquidityInput.id,
            },
            status: 'queued',
            retry_count: 0,
            next_retry_at_ms: 0,
            worker_pid: null,
            code: null,
            message: 'Private FIX execution is durably queued.',
            created_at_unix_ms: timestamp,
            updated_at_unix_ms: timestamp,
        };
        fs.mkdirSync(directoryFor(jobId), { recursive: true, mode: 0o700 });
        if (!fs.existsSync(file)) atomicWrite(file, job);
        pump();
        return publicJob(readJob(jobId), false);
    }

    async function submit(body, direction) {
        const key = `${direction}:${String(body?.client_request_id || '')}`;
        while (submissions.has(key)) await submissions.get(key);
        let release;
        const lock = new Promise((resolve) => { release = resolve; });
        submissions.set(key, lock);
        try { return await submitUnlocked(body, direction); } finally {
            submissions.delete(key);
            release();
        }
    }

    function status(jobId) {
        const normalized = String(jobId || '').trim().toLowerCase();
        if (!JOB_ID_RE.test(normalized)) return null;
        const job = readJob(normalized);
        pump();
        return job ? publicJob(job) : null;
    }

    const watchdog = setIntervalImpl(pump, 2_000);
    watchdog.unref?.();
    pump();

    function close() {
        closed = true;
        clearIntervalImpl(watchdog);
        for (const worker of workers.values()) {
            clearTimeout(worker.timeout);
            worker.child.kill('SIGTERM');
        }
        workers.clear();
    }

    return {
        pnokFixWalletReadiness: readiness,
        submitPnokFixWalletJob: (body) => submit(body, 'acquire'),
        submitPnokFixRestoreJob: (body) => submit(body, 'restore'),
        pnokFixWalletJobStatus: status,
        closePnokFixWalletJobs: close,
        _pumpPnokFixWalletJobsForTest: pump,
    };
}

module.exports = { CONFIG_SCHEMA, JOB_SCHEMA, PUBLIC_SCHEMA, create, jobIdFor, normalizeRequest };
