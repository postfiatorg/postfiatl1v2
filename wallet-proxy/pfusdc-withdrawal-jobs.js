'use strict';

const crypto = require('crypto');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawn } = require('child_process');

const PFTL_RE = /^pf[0-9a-f]{40}$/;
const EVM_RE = /^0x[0-9a-f]{40}$/;
const HASH_RE = /^[0-9a-f]{64}$/;
const PFTL_TX_RE = /^[0-9a-f]{96}$/;
const ASSET_RE = /^[0-9a-f]{96}$/;
const TERMINAL = new Set(['accepted', 'failed']);
const JOB_STATUS = new Set(['queued', 'running', ...TERMINAL]);
const REQUEST_FIELDS = new Set(['burn_tx_id', 'owner', 'ethereum_recipient', 'amount_atoms', 'asset_id']);

function atomicWrite(file, value) {
    const temporary = `${file}.${process.pid}.${crypto.randomBytes(6).toString('hex')}.tmp`;
    const fd = fs.openSync(temporary, 'wx', 0o600);
    try {
        fs.writeFileSync(fd, `${JSON.stringify(value, null, 2)}\n`);
        fs.fsyncSync(fd);
    } finally { fs.closeSync(fd); }
    fs.renameSync(temporary, file);
}

function safeFile(value, label, executable = false) {
    const file = path.resolve(String(value || ''));
    const stat = fs.lstatSync(file);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.uid !== process.getuid() || (stat.mode & 0o022) !== 0
        || (executable && (stat.mode & 0o100) === 0)) {
        throw new Error(`${label} must be an owner-controlled regular file`);
    }
    return file;
}

function normalizeRequest(body, assetId) {
    if (!body || typeof body !== 'object' || Array.isArray(body)
        || Object.keys(body).some(key => !REQUEST_FIELDS.has(key))) {
        throw Object.assign(new Error('Invalid pfUSDC withdrawal relay request.'), {
            code: 'pfusdc_withdrawal_request_invalid',
        });
    }
    const request = {
        burn_tx_id: String(body?.burn_tx_id || '').trim().toLowerCase().replace(/^0x/, ''),
        owner: String(body?.owner || '').trim().toLowerCase(),
        ethereum_recipient: String(body?.ethereum_recipient || '').trim().toLowerCase(),
        amount_atoms: String(body?.amount_atoms || '').trim(),
        asset_id: String(body?.asset_id || '').trim().toLowerCase(),
    };
    let amount = 0n;
    try { amount = BigInt(request.amount_atoms); } catch (_) { /* rejected below */ }
    if (!PFTL_TX_RE.test(request.burn_tx_id) || !PFTL_RE.test(request.owner)
        || !EVM_RE.test(request.ethereum_recipient) || request.asset_id !== assetId
        || amount <= 0n || amount > ((1n << 64n) - 1n)) {
        throw Object.assign(new Error('Invalid pfUSDC withdrawal relay request.'), {
            code: 'pfusdc_withdrawal_request_invalid',
        });
    }
    return request;
}

function jobId(request) {
    return crypto.createHash('sha256').update([
        request.asset_id, request.burn_tx_id, request.owner,
        request.ethereum_recipient, request.amount_atoms,
    ].join('\0')).digest('hex');
}

function proofQualification(fileValue, config) {
    if (!fileValue) return { ready: false, code: 'pfusdc_withdrawal_capacity_unqualified' };
    try {
        const file = safeFile(fileValue, 'pfUSDC withdrawal qualification report');
        const report = JSON.parse(fs.readFileSync(file, 'utf8'));
        const expectedVkey = String(config?.pfusdc_egress_program_vkey || '').toLowerCase();
        if (report?.schema !== 'postfiat.pfusdc.egress_proof_report.v1'
            || report?.proof_mode !== 'groth16'
            || !/^0x[0-9a-f]{64}$/.test(expectedVkey)
            || String(report?.program_vkey || '').toLowerCase() !== expectedVkey
            || !Number.isSafeInteger(report?.proof_bytes) || report.proof_bytes <= 0 || report.proof_bytes > 4096
            || !Number.isSafeInteger(report?.public_values_bytes) || report.public_values_bytes <= 0
            || report.public_values_bytes > 4096) {
            return { ready: false, code: 'pfusdc_withdrawal_capacity_unqualified' };
        }
        return { ready: true, report: file };
    } catch (_) { return { ready: false, code: 'pfusdc_withdrawal_capacity_unqualified' }; }
}

function create(runtime = {}, options = {}) {
    const env = options.env || process.env;
    const configured = String(env.PFUSDC_WITHDRAWAL_ENABLED || '') === 'true';
    const assetId = String(env.PFUSDC_ASSET_ID || '').trim().toLowerCase();
    const root = path.resolve(env.PFUSDC_WITHDRAWAL_JOB_ROOT
        || path.join(os.homedir(), '.postfiat', 'wallet-proxy-8080', 'pfusdc-withdrawals-v1'));
    if (!configured) return {
        pfusdcWithdrawalReadiness: async () => ({ ok: false, ready: false, code: 'pfusdc_withdrawal_not_configured', message: 'pfUSDC withdrawals are temporarily unavailable.' }),
        submitPfusdcWithdrawalJob: async () => { throw Object.assign(new Error('pfUSDC withdrawals are not configured.'), { code: 'pfusdc_withdrawal_not_configured' }); },
        retryPfusdcWithdrawalJob: async () => { throw Object.assign(new Error('pfUSDC withdrawals are not configured.'), { code: 'pfusdc_withdrawal_not_configured' }); },
        pfusdcWithdrawalJobStatus: () => null,
        pfusdcWithdrawalJobsForOwner: () => [],
        closePfusdcWithdrawalJobs: () => {},
    };
    if (!ASSET_RE.test(assetId)) throw new Error('PFUSDC_ASSET_ID is malformed');
    const script = safeFile(env.PFUSDC_WITHDRAWAL_SCRIPT, 'pfUSDC withdrawal script', true);
    const configFile = safeFile(env.PFUSDC_WITHDRAWAL_CONFIG_FILE, 'pfUSDC withdrawal config');
    const config = JSON.parse(fs.readFileSync(configFile, 'utf8'));
    const localProver = safeFile(env.PFUSDC_WITHDRAWAL_LOCAL_PROVER_BIN, 'pfUSDC local prover', true);
    const egressElf = safeFile(env.PFUSDC_WITHDRAWAL_EGRESS_ELF, 'pfUSDC egress ELF');
    const qualification = proofQualification(env.PFUSDC_WITHDRAWAL_QUALIFICATION_REPORT, config);
    const jobsRoot = path.join(root, 'jobs');
    fs.mkdirSync(jobsRoot, { recursive: true, mode: 0o700 });
    let active = null;
    let closed = false;

    const directoryFor = id => path.join(jobsRoot, id);
    const fileFor = id => path.join(directoryFor(id), 'job.json');
    const read = id => {
        if (!HASH_RE.test(String(id || '')) || !fs.existsSync(fileFor(id))) return null;
        try {
            const job = JSON.parse(fs.readFileSync(fileFor(id), 'utf8'));
            const request = normalizeRequest(job?.request, assetId);
            if (job?.schema !== 'postfiat-pfusdc-wallet-withdrawal-job-v1'
                || job?.job_id !== id || jobId(request) !== id || !JOB_STATUS.has(job?.status)
                || !Number.isSafeInteger(job?.created_at_unix) || job.created_at_unix <= 0
                || !Number.isSafeInteger(job?.updated_at_unix) || job.updated_at_unix <= 0
                || !Number.isSafeInteger(Number(job?.attempts || 0)) || Number(job?.attempts || 0) < 0) return null;
            return { ...job, request };
        } catch (_) { return null; }
    };
    const write = job => atomicWrite(fileFor(job.job_id), job);
    const phaseEvidence = (job, name) => path.join(
        directoryFor(job.job_id), 'pfusdc-egress', name,
    );
    const hasEthereumPayout = job => fs.existsSync(phaseEvidence(job, 'withdrawal-result.json'));
    const hasAcceptance = job => fs.existsSync(phaseEvidence(job, 'summary.json'));
    const publicJob = job => ({
        schema: job.schema, job_id: job.job_id, status: job.status,
        stage: job.stage, code: job.code || null, message: job.message || '',
        request: job.request, created_at_unix: job.created_at_unix,
        updated_at_unix: job.updated_at_unix, ethereum_tx_hash: job.ethereum_tx_hash || null,
    });

    function pump() {
        if (closed || active) return;
        const jobs = fs.readdirSync(jobsRoot).filter(HASH_RE.test.bind(HASH_RE))
            .map(read).filter(Boolean).sort((a, b) => a.created_at_unix - b.created_at_unix);
        const blockingFailure = jobs.find(job => job.status === 'failed' && !hasAcceptance(job));
        // Ethereum verifier checkpoints are monotonic. Never advance to a later
        // burn while an earlier accepted burn is failed or incomplete. The
        // authenticated retry action resumes that exact durable job; if payment
        // already happened, payout evidence prevents Ethereum from paying twice.
        if (blockingFailure) return;
        const queued = jobs.find(job => !TERMINAL.has(job.status));
        if (!queued) return;
        const phaseDir = directoryFor(queued.job_id);
        queued.status = 'running';
        queued.stage = 'Verifying the accepted PFTL burn and preparing Ethereum USDC (usually 20–40 minutes)';
        queued.updated_at_unix = Math.floor(Date.now() / 1000);
        write(queued);
        const args = [script, '--phase-dir', phaseDir, '--workflow-id', `wallet-withdraw-${queued.job_id.slice(0, 12)}`,
            '--amount-atoms', queued.request.amount_atoms, '--existing-burn-tx-id', queued.request.burn_tx_id,
            '--owner', queued.request.owner, '--recipient', queued.request.ethereum_recipient,
            '--prover-backend', 'cpu', '--resume'];
        const log = fs.openSync(path.join(phaseDir, 'worker.log'), 'a', 0o600);
        const child = spawn('/bin/bash', args, {
            cwd: path.dirname(path.dirname(script)), stdio: ['ignore', log, log],
            env: {
                ...process.env,
                A666_PFTL_RELEASE_ID: config.release_id,
                A666_PROPOSER_HOSTS_FILE: config.hosts_file,
                A666_LOCAL_NODE_BIN: config.local_node,
                A666_PFTL_TOPOLOGY_PATH: config.remote_topology,
                A666_PFUSDC_VERIFIER_ADDRESS: config.pfusdc_verifier_address,
                A666_PFUSDC_EGRESS_PROGRAM_VKEY: config.pfusdc_egress_program_vkey,
                A666_PFUSDC_DEPLOYMENT_MANIFEST: config.pfusdc_manifest,
                A666_PFUSDC_DEPLOYMENT_MANIFEST_SHA256: config.pfusdc_manifest_sha256,
                A666_CONTRACT_ARTIFACT_ROOT: config.contract_artifact_root,
                A666_PFUSDC_EGRESS_LOCAL_PROVER_BIN: localProver,
                A666_PFUSDC_EGRESS_ELF: egressElf,
                A666_PFUSDC_EGRESS_PROVER_BACKEND: 'cpu',
                A666_PFUSDC_EGRESS_PROVER_SHA256: '',
            },
        });
        fs.closeSync(log);
        active = child;
        child.once('exit', code => {
            active = null;
            const current = read(queued.job_id);
            const summaryFile = path.join(phaseDir, 'pfusdc-egress', 'summary.json');
            if (current && code === 0 && fs.existsSync(summaryFile)) {
                let summary = null;
                try { summary = JSON.parse(fs.readFileSync(summaryFile, 'utf8')); } catch (_) { /* rejected below */ }
                const txHash = String(summary?.ethereum_withdrawal?.tx || '').replace(/^0x/, '').toLowerCase();
                if (summary?.schema === 'postfiat.a666.pfusdc_proof_egress_acceptance.v1'
                    && summary.verdict === 'PASS' && summary.ethereum_withdrawal?.replay_rejected === true
                    && HASH_RE.test(txHash)
                    && String(summary.ethereum_withdrawal?.amount_atoms) === current.request.amount_atoms) {
                    current.status = 'accepted';
                    current.stage = 'Ethereum USDC received';
                    current.ethereum_tx_hash = txHash;
                    current.message = 'Ethereum USDC was released and PFTL accounting was settled.';
                } else {
                    current.status = 'failed'; current.code = 'pfusdc_withdrawal_result_invalid';
                    current.message = 'Withdrawal completion evidence failed validation.';
                }
            } else if (current) {
                current.attempts = Number(current.attempts || 0) + 1;
                current.status = current.attempts >= 3 ? 'failed' : 'queued';
                current.stage = current.status === 'queued'
                    ? 'Retrying safely from the last completed step'
                    : (hasEthereumPayout(current)
                        ? 'Ethereum USDC received; accounting retry required'
                        : 'Needs support');
                current.code = current.status === 'failed' ? 'pfusdc_withdrawal_worker_failed' : null;
                current.message = current.status === 'failed'
                    ? (hasEthereumPayout(current)
                        ? 'Ethereum USDC was released. Retry closes PFTL accounting without paying twice.'
                        : 'The withdrawal relay could not complete after three recoverable attempts.')
                    : '';
            }
            if (current) {
                current.updated_at_unix = Math.floor(Date.now() / 1000);
                write(current);
                setTimeout(pump, current.status === 'queued' ? 5_000 : 0).unref?.();
            } else setTimeout(pump, 0).unref?.();
        });
    }

    for (const id of fs.readdirSync(jobsRoot).filter(name => HASH_RE.test(name))) {
        const job = read(id);
        if (job && !TERMINAL.has(job.status)) {
            job.status = 'queued'; job.stage = 'Resuming safely'; write(job);
        }
    }
    const timer = setInterval(pump, 5_000); timer.unref?.(); pump();
    return {
        pfusdcWithdrawalReadiness: async () => ({ ok: true, ready: qualification.ready, schema: 'postfiat-pfusdc-wallet-withdrawal-readiness-v1', asset_id: assetId, ...(qualification.ready ? { proof_backend: 'local_cpu' } : { code: qualification.code, message: 'Withdrawals are temporarily paused while payout capacity is checked.' }) }),
        submitPfusdcWithdrawalJob: async body => {
            if (!qualification.ready) throw Object.assign(new Error('Withdrawals are temporarily paused while payout capacity is checked.'), { code: qualification.code });
            const request = normalizeRequest(body, assetId);
            const id = jobId(request); const existing = read(id);
            if (existing) return { ...publicJob(existing), idempotent_replay: true };
            fs.mkdirSync(directoryFor(id), { recursive: true, mode: 0o700 });
            const now = Math.floor(Date.now() / 1000);
            const job = { schema: 'postfiat-pfusdc-wallet-withdrawal-job-v1', job_id: id, status: 'queued', stage: 'Withdrawal burn confirmed', request, attempts: 0, created_at_unix: now, updated_at_unix: now };
            write(job); pump(); return { ...publicJob(job), idempotent_replay: false };
        },
        retryPfusdcWithdrawalJob: async id => {
            if (!qualification.ready) throw Object.assign(new Error('Withdrawals are temporarily paused while payout capacity is checked.'), { code: qualification.code });
            const job = read(String(id || '').toLowerCase());
            if (!job) return null;
            if (job.status !== 'failed') return { ...publicJob(job), idempotent_replay: true };
            job.status = 'queued'; job.stage = 'Resuming safely from the accepted PFTL burn';
            job.attempts = 0; job.code = null; job.message = '';
            job.updated_at_unix = Math.floor(Date.now() / 1000);
            write(job); pump();
            return { ...publicJob(job), idempotent_replay: false };
        },
        pfusdcWithdrawalJobStatus: id => { const job = read(String(id || '').toLowerCase()); return job ? publicJob(job) : null; },
        pfusdcWithdrawalJobsForOwner: (owner, limit = 20) => fs.readdirSync(jobsRoot).filter(name => HASH_RE.test(name)).map(read)
            .filter(job => job?.request?.owner === String(owner || '').toLowerCase())
            .sort((a, b) => b.created_at_unix - a.created_at_unix).slice(0, Math.min(100, Math.max(1, Number(limit) || 20))).map(publicJob),
        closePfusdcWithdrawalJobs: () => { closed = true; clearInterval(timer); active?.kill('SIGTERM'); },
    };
}

module.exports = { create, normalizeRequest, jobId, proofQualification };
