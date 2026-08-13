'use strict';

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');
const { execFile } = require('child_process');
const { promisify } = require('util');

const execFileAsyncDefault = promisify(execFile);
const CONFIG_SCHEMA = 'postfiat-eth-fast-lane-driver-config-v1';
const JOB_SCHEMA = 'postfiat-trustless-bridge-job-v2';
const STATE_SCHEMA = 'postfiat-trustless-bridge-worker-state-v2';
const CHECKPOINT_SCHEMA = 'postfiat-eth-fast-lane-stage-checkpoint-v1';
const ROUTE_ID = 'ethereum-mainnet-usdc-v1';
const SOURCE_CHAIN_ID = 1;
const SOURCE_PROOF_KIND = 'sp1-ethereum-finality-v1';
const SEPOLIA_P0_PROGRAM_VKEY = '0x0077f479ed28535dbb5035f455a875334bae7d5a1eaa7c22c6f070a404eab31f';
const SEPOLIA_P0_MANIFEST_HASH = 'dc409b424e7627b936d81a16d2fc8f4c17e21a108d654be6b992e552d7b0c6d3';
const HASH_RE = /^(?:0x)?[0-9a-f]{64}$/;
const HASH48_RE = /^(?:0x)?[0-9a-f]{96}$/;
const RECEIPT_ID_RE = /^(?:0x)?(?:[0-9a-f]{64}|[0-9a-f]{96})$/;
const STAGES = [
    'confirming_deposit',
    'waiting_for_ethereum_finality',
    'capturing_state_proof',
    'proving',
    'verifying',
    'claiming',
];
const RESULT_FIELDS = new Set([
    'ok', 'stage', 'route_id', 'source_chain_id', 'source_proof_kind',
    'route_profile_hash', 'asset_id', 'vault_address', 'vault_runtime_code_hash',
    'token_address', 'token_runtime_code_hash',
    'deposit_tx_hash', 'deposit_id', 'pftl_recipient', 'depositor', 'amount_atoms',
    'deposit_confirmed', 'ethereum_finalized', 'finalized_block_hash',
    'finalized_block_number', 'witness_sha256', 'evidence_root', 'nullifier',
    'proof_sha256', 'public_values_sha256', 'program_vkey', 'manifest_hash',
    'proof_verified', 'receipt_code', 'receipt_id', 'tx_id',
]);

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

function atomicWrite(file, value) {
    fs.mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 });
    const temporary = `${file}.${process.pid}.${crypto.randomBytes(6).toString('hex')}.tmp`;
    const fd = fs.openSync(temporary, 'wx', 0o600);
    try {
        fs.writeFileSync(fd, `${JSON.stringify(value, null, 2)}\n`);
        fs.fsyncSync(fd);
    } finally {
        fs.closeSync(fd);
    }
    fs.renameSync(temporary, file);
    const directoryFd = fs.openSync(path.dirname(file), 'r');
    try { fs.fsyncSync(directoryFd); } finally { fs.closeSync(directoryFd); }
}

function secureRegularFile(file, label, requireOwner = false) {
    const absolute = path.resolve(String(file || ''));
    const stat = fs.lstatSync(absolute);
    if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`${label} must be a regular file`);
    if ((stat.mode & 0o022) !== 0) throw new Error(`${label} must not be group/world writable`);
    if (requireOwner && stat.uid !== process.getuid()) throw new Error(`${label} must be owned by the service user`);
    return absolute;
}

function assertFileHash(file, expected, label, cache = null) {
    let actual = cache?.get(file);
    if (!actual) {
        actual = sha256(fs.readFileSync(file));
        cache?.set(file, actual);
    }
    if (!/^[0-9a-f]{64}$/.test(String(expected || ''))
        || actual !== expected) {
        throw new Error(`${label} SHA-256 pin mismatch`);
    }
}

function positiveInteger(value, label, minimum = 1) {
    const parsed = Number.parseInt(String(value ?? ''), 10);
    if (!Number.isSafeInteger(parsed) || parsed < minimum) throw new Error(`${label} must be a positive integer`);
    return parsed;
}

function loadConfig(file) {
    const configFile = secureRegularFile(file, 'driver config', true);
    const config = JSON.parse(fs.readFileSync(configFile, 'utf8'));
    if (config?.schema !== CONFIG_SCHEMA
        || config.route_id !== ROUTE_ID
        || Number(config.source_chain_id) !== SOURCE_CHAIN_ID
        || config.source_proof_kind !== SOURCE_PROOF_KIND
        || !/^0x[0-9a-f]{64}$/.test(String(config.program_vkey || ''))
        || !/^[0-9a-f]{64}$/.test(String(config.manifest_hash || ''))
        || config.program_vkey === SEPOLIA_P0_PROGRAM_VKEY
        || config.manifest_hash === SEPOLIA_P0_MANIFEST_HASH
        || !/^[0-9a-f]{96}$/.test(String(config.route_profile_hash || ''))
        || !/^[0-9a-f]{96}$/.test(String(config.asset_id || ''))
        || !/^0x[0-9a-f]{40}$/.test(String(config.vault_address || ''))
        || !/^0x[0-9a-f]{64}$/.test(String(config.vault_runtime_code_hash || ''))
        || !/^0x[0-9a-f]{40}$/.test(String(config.token_address || ''))
        || !/^0x[0-9a-f]{64}$/.test(String(config.token_runtime_code_hash || ''))
        || !config.readiness || !Array.isArray(config.stages)
        || config.stages.length !== STAGES.length) {
        throw new Error('invalid Ethereum fast-lane driver config');
    }
    const rows = [config.readiness, ...config.stages];
    const fileHashCache = new Map();
    if (!Array.isArray(config.pinned_files)) throw new Error('driver pinned_files must be an array');
    const pins = new Map();
    for (const pin of config.pinned_files) {
        const pinnedPath = secureRegularFile(pin?.path, 'driver pinned artifact');
        const pinnedHash = String(pin?.sha256 || '').toLowerCase();
        assertFileHash(pinnedPath, pinnedHash, 'driver pinned artifact', fileHashCache);
        pins.set(pinnedPath, pinnedHash);
    }
    rows.forEach((row, index) => {
        if (!row || typeof row !== 'object' || typeof row.program !== 'string'
            || !Array.isArray(row.args) || !row.args.every((arg) => typeof arg === 'string')) {
            throw new Error('invalid driver command config');
        }
        if (index > 0 && row.stage !== STAGES[index - 1]) throw new Error('driver stages are out of order');
        row.program = secureRegularFile(row.program, 'driver command');
        row.program_sha256 = String(row.program_sha256 || '').toLowerCase();
        assertFileHash(row.program, row.program_sha256, 'driver command', fileHashCache);
        for (const arg of row.args) {
            if (path.isAbsolute(arg) && fs.existsSync(arg)) {
                const absoluteArg = path.resolve(arg);
                if (!pins.has(absoluteArg) && absoluteArg !== row.program) {
                    throw new Error(`absolute command artifact is not hash-pinned: ${absoluteArg}`);
                }
            }
        }
        row.timeout_ms = positiveInteger(row.timeout_ms || 60_000, 'driver timeout', 100);
    });
    return { config, configFile, configSha256: sha256(fs.readFileSync(configFile)) };
}

function economicRequest(request) {
    const {
        route_id, source_chain_id, deposit_tx_hash, deposit_id,
        pftl_recipient, depositor, amount_atoms,
    } = request;
    return {
        route_id, source_chain_id, deposit_tx_hash, deposit_id,
        pftl_recipient, depositor, amount_atoms,
    };
}

function loadJob(file) {
    const jobFile = secureRegularFile(file, 'bridge job');
    const job = JSON.parse(fs.readFileSync(jobFile, 'utf8'));
    const request = job?.request;
    if (job?.schema !== JOB_SCHEMA || !request
        || request.route_id !== ROUTE_ID
        || Number(request.source_chain_id) !== SOURCE_CHAIN_ID
        || !HASH_RE.test(request.deposit_tx_hash)
        || !HASH_RE.test(request.deposit_id)
        || !/^pf[0-9a-f]{40}$/.test(request.pftl_recipient)
        || !/^0x[0-9a-f]{40}$/.test(request.depositor)
        || !/^[1-9][0-9]*$/.test(request.amount_atoms)) {
        throw new Error('invalid durable Ethereum bridge job');
    }
    const fingerprint = sha256(stableJson(economicRequest(request)));
    if (job.request_fingerprint !== fingerprint) throw new Error('bridge job fingerprint mismatch');
    return { job, jobFile, request, jobDir: path.dirname(jobFile), fingerprint };
}

function expandArgs(args, values) {
    return args.map((arg) => String(arg).replace(/\{([a-z_]+)\}/g, (_, key) => {
        if (!(key in values)) throw new Error(`unknown driver argument placeholder: ${key}`);
        return values[key];
    }));
}

function commandValues(context) {
    const { jobFile, jobDir, request, configFile } = context;
    return {
        job_file: jobFile,
        job_dir: jobDir,
        config_file: configFile,
        route_id: request.route_id,
        source_chain_id: String(request.source_chain_id),
        deposit_tx_hash: request.deposit_tx_hash,
        deposit_id: request.deposit_id,
        pftl_recipient: request.pftl_recipient,
        depositor: request.depositor,
        amount_atoms: request.amount_atoms,
        program_vkey: context.config.program_vkey,
        manifest_hash: context.config.manifest_hash,
        route_profile_hash: context.config.route_profile_hash,
        asset_id: context.config.asset_id,
        vault_address: context.config.vault_address,
        vault_runtime_code_hash: context.config.vault_runtime_code_hash,
        token_address: context.config.token_address,
        token_runtime_code_hash: context.config.token_runtime_code_hash,
    };
}

function publicResult(result) {
    return Object.fromEntries(Object.entries(result).filter(([key]) => RESULT_FIELDS.has(key)));
}

function terminalError(message, code = 'driver_result_binding_failed') {
    return Object.assign(new Error(message), { code, terminal: true });
}

function assertCommonBinding(result, stage, request, config) {
    if (result?.ok !== true || result.stage !== stage
        || result.route_id !== ROUTE_ID
        || Number(result.source_chain_id) !== SOURCE_CHAIN_ID
        || result.source_proof_kind !== SOURCE_PROOF_KIND
        || result.program_vkey !== config.program_vkey
        || result.manifest_hash !== config.manifest_hash
        || result.route_profile_hash !== config.route_profile_hash
        || result.asset_id !== config.asset_id
        || result.vault_address !== config.vault_address
        || result.vault_runtime_code_hash !== config.vault_runtime_code_hash
        || result.token_address !== config.token_address
        || result.token_runtime_code_hash !== config.token_runtime_code_hash
        || result.deposit_tx_hash !== request.deposit_tx_hash
        || result.deposit_id !== request.deposit_id
        || result.pftl_recipient !== request.pftl_recipient
        || result.depositor !== request.depositor
        || String(result.amount_atoms) !== request.amount_atoms) {
        throw terminalError(`driver result binding failed at ${stage}`);
    }
    if (stage === 'confirming_deposit' && result.deposit_confirmed !== true) throw terminalError('deposit not confirmed');
    if (stage === 'waiting_for_ethereum_finality' && (result.ethereum_finalized !== true
        || !HASH_RE.test(String(result.finalized_block_hash || ''))
        || !Number.isSafeInteger(Number(result.finalized_block_number)))) throw terminalError('Ethereum finality not proven');
    if (stage === 'capturing_state_proof' && (
        !HASH_RE.test(String(result.witness_sha256 || ''))
        || !HASH48_RE.test(String(result.evidence_root || ''))
        || !HASH_RE.test(String(result.nullifier || ''))
    )) throw terminalError('state-proof capture artifacts missing');
    if (stage === 'proving' && [result.proof_sha256, result.public_values_sha256, result.program_vkey]
        .some((value) => !HASH_RE.test(String(value || '')))) throw terminalError('SP1 proof artifacts missing');
    if (stage === 'verifying' && result.proof_verified !== true) throw terminalError('proof verification failed');
    if (stage === 'claiming' && (result.receipt_code !== 'ACCEPTED'
        || !RECEIPT_ID_RE.test(String(result.receipt_id || result.tx_id || '')))) throw terminalError('claim was not accepted');
}

function checkpointHash(checkpoint) {
    const { checkpoint_sha256: _ignored, ...body } = checkpoint;
    return sha256(stableJson(body));
}

function checkpointFile(jobDir, index, stage) {
    return path.join(jobDir, 'checkpoints', `${String(index).padStart(2, '0')}-${stage}.json`);
}

function loadCheckpoint(file, expected) {
    const checkpoint = JSON.parse(fs.readFileSync(file, 'utf8'));
    if (checkpoint?.schema !== CHECKPOINT_SCHEMA
        || checkpoint.stage !== expected.stage
        || checkpoint.stage_index !== expected.index
        || checkpoint.request_fingerprint !== expected.fingerprint
        || checkpoint.config_sha256 !== expected.configSha256
        || checkpoint.prior_checkpoint_sha256 !== expected.prior
        || checkpoint.checkpoint_sha256 !== checkpointHash(checkpoint)) {
        throw Object.assign(new Error(`durable checkpoint is invalid at ${expected.stage}`), {
            code: 'bridge_checkpoint_invalid', terminal: true,
        });
    }
    assertCommonBinding(checkpoint.result, expected.stage, expected.request, expected.config);
    return checkpoint;
}

function writeWorkerState(context, status, extra = {}) {
    atomicWrite(path.join(context.jobDir, 'worker-state.json'), {
        schema: STATE_SCHEMA,
        status,
        route_id: ROUTE_ID,
        source_chain_id: SOURCE_CHAIN_ID,
        source_proof_kind: SOURCE_PROOF_KIND,
        program_vkey: context.config.program_vkey,
        manifest_hash: context.config.manifest_hash,
        route_profile_hash: context.config.route_profile_hash,
        asset_id: context.config.asset_id,
        observer_attestor_enabled: false,
        updated_at_unix: Math.floor(Date.now() / 1000),
        ...extra,
    });
}

function parseCommandOutput(raw, stage) {
    if (Buffer.byteLength(String(raw || ''), 'utf8') > 1024 * 1024) throw new Error(`${stage} output is too large`);
    try { return JSON.parse(String(raw || '').trim()); } catch (_) {
        throw new Error(`${stage} did not return JSON`);
    }
}

async function executeCommand(command, values, execFileAsync) {
    const args = expandArgs(command.args, values);
    let stdout;
    try {
        ({ stdout } = await execFileAsync(command.program, args, {
            cwd: path.dirname(values.config_file),
            timeout: command.timeout_ms,
            maxBuffer: 1024 * 1024,
            env: process.env,
        }));
    } catch (error) {
        let result = null;
        try { result = parseCommandOutput(error.stdout, command.stage || 'readiness'); } catch (_) { /* generic retry */ }
        if (result?.ok === false) {
            throw Object.assign(new Error('driver stage did not complete'), {
                code: String(result.code || 'driver_stage_failed').slice(0, 64),
                terminal: result.retryable === false,
            });
        }
        throw Object.assign(new Error('driver command unavailable'), {
            code: String(error.code || 'driver_command_unavailable').slice(0, 64),
            terminal: false,
        });
    }
    try {
        return parseCommandOutput(stdout, command.stage || 'readiness');
    } catch (error) {
        throw Object.assign(error, { code: 'driver_output_invalid', terminal: true });
    }
}

async function readiness(configPath, routeId, dependencies = {}) {
    let loaded;
    try { loaded = loadConfig(configPath); } catch (error) {
        throw Object.assign(error, { code: 'driver_config_invalid', terminal: true });
    }
    if (routeId !== ROUTE_ID) throw terminalError('unsupported Ethereum fast-lane route', 'unsupported_bridge_route');
    const values = {
        config_file: loaded.configFile,
        route_id: ROUTE_ID,
        source_chain_id: String(SOURCE_CHAIN_ID),
    };
    const result = await executeCommand(
        loaded.config.readiness,
        values,
        dependencies.execFileAsync || execFileAsyncDefault,
    );
    if (result?.ok !== true || result?.ready !== true
        || result.route_id !== ROUTE_ID
        || Number(result.source_chain_id) !== SOURCE_CHAIN_ID
        || result.source_proof_kind !== SOURCE_PROOF_KIND
        || result.program_vkey !== loaded.config.program_vkey
        || result.manifest_hash !== loaded.config.manifest_hash
        || result.route_profile_hash !== loaded.config.route_profile_hash
        || result.asset_id !== loaded.config.asset_id
        || result.vault_address !== loaded.config.vault_address
        || result.vault_runtime_code_hash !== loaded.config.vault_runtime_code_hash
        || result.token_address !== loaded.config.token_address
        || result.token_runtime_code_hash !== loaded.config.token_runtime_code_hash
        || result.observer_attestor_enabled !== false
        || result.prover_authenticated !== true
        || result.prover_healthy !== true
        || result.route_manifest_active !== true
        || result.program_vkey_active !== true
        || result.nav_cap_growth_enabled !== true
        || result.vault_paused !== false
        || result.vault_code_hash_matches !== true
        || result.token_code_hash_matches !== true
        || Number(result.execution_rpc_sources_reachable) < 2
        || result.beacon_finality_current !== true) {
        throw terminalError('Ethereum readiness binding failed', 'driver_readiness_binding_failed');
    }
    return result;
}

function validateConfig(configPath) {
    let loaded;
    try { loaded = loadConfig(configPath); } catch (error) {
        throw Object.assign(error, { code: 'driver_config_invalid', terminal: true });
    }
    return {
        ok: true,
        schema: 'postfiat-eth-fast-lane-driver-config-validation-v1',
        route_id: ROUTE_ID,
        source_chain_id: SOURCE_CHAIN_ID,
        source_proof_kind: SOURCE_PROOF_KIND,
        route_profile_hash: loaded.config.route_profile_hash,
        asset_id: loaded.config.asset_id,
        program_vkey: loaded.config.program_vkey,
        manifest_hash: loaded.config.manifest_hash,
        vault_address: loaded.config.vault_address,
        vault_runtime_code_hash: loaded.config.vault_runtime_code_hash,
        token_address: loaded.config.token_address,
        token_runtime_code_hash: loaded.config.token_runtime_code_hash,
        config_sha256: loaded.configSha256,
        pinned_artifact_count: loaded.config.pinned_files.length,
        stages: [...STAGES],
        requires_live_readiness: true,
        activation_ready: false,
        observer_attestor_enabled: false,
    };
}

async function runJob(jobPath, configPath, dependencies = {}) {
    let job;
    try {
        job = loadJob(jobPath);
    } catch (error) {
        throw Object.assign(error, { code: 'driver_input_invalid', terminal: true });
    }
    const configSnapshot = path.join(job.jobDir, 'driver-config.snapshot.json');
    let loaded;
    try {
        if (!fs.existsSync(configSnapshot)) {
            const source = loadConfig(configPath);
            atomicWrite(configSnapshot, source.config);
        }
        loaded = loadConfig(configSnapshot);
    } catch (error) {
        throw Object.assign(error, { code: 'driver_config_invalid', terminal: true });
    }
    const context = { ...loaded, ...job };
    const values = commandValues(context);
    const execFileAsync = dependencies.execFileAsync || execFileAsyncDefault;
    let prior = '0'.repeat(64);
    let claimResult = null;

    try {
        for (let index = 0; index < STAGES.length; index += 1) {
            const stage = STAGES[index];
            const file = checkpointFile(context.jobDir, index, stage);
            let checkpoint;
            if (fs.existsSync(file)) {
                checkpoint = loadCheckpoint(file, {
                    stage, index, prior, request: context.request, config: loaded.config,
                    fingerprint: context.fingerprint, configSha256: context.configSha256,
                });
            } else {
                writeWorkerState(context, stage, { stage_index: index, retryable: true });
                const result = await executeCommand(loaded.config.stages[index], values, execFileAsync);
                assertCommonBinding(result, stage, context.request, loaded.config);
                checkpoint = {
                    schema: CHECKPOINT_SCHEMA,
                    stage,
                    stage_index: index,
                    request_fingerprint: context.fingerprint,
                    config_sha256: context.configSha256,
                    prior_checkpoint_sha256: prior,
                    result: publicResult(result),
                };
                checkpoint.checkpoint_sha256 = checkpointHash(checkpoint);
                atomicWrite(file, checkpoint);
            }
            prior = checkpoint.checkpoint_sha256;
            if (stage === 'claiming') claimResult = checkpoint.result;
        }
        writeWorkerState(context, 'accepted', {
            retryable: false,
            receipt_code: claimResult.receipt_code,
            receipt_id: claimResult.receipt_id || null,
            tx_id: claimResult.tx_id || null,
            terminal_checkpoint_sha256: prior,
        });
        return { ok: true, status: 'accepted', receipt_code: claimResult.receipt_code };
    } catch (error) {
        const terminal = error.terminal === true;
        writeWorkerState(context, terminal ? 'failed' : (readCurrentStage(context.jobDir) || 'queued'), {
            retryable: !terminal,
            code: String(error.code || (terminal ? 'bridge_job_failed' : 'bridge_job_retryable')).slice(0, 64),
            message: terminal
                ? 'Trustless Ethereum bridge job failed a binding or checkpoint gate.'
                : 'Trustless Ethereum bridge job will retry from its last durable checkpoint.',
        });
        throw Object.assign(error, { terminal });
    }
}

function readCurrentStage(jobDir) {
    try {
        const state = JSON.parse(fs.readFileSync(path.join(jobDir, 'worker-state.json'), 'utf8'));
        return STAGES.includes(state.status) ? state.status : null;
    } catch (_) { return null; }
}

function option(args, name) {
    const index = args.indexOf(name);
    if (index < 0 || !args[index + 1]) throw new Error(`missing ${name}`);
    return args[index + 1];
}

async function cli(argv = process.argv.slice(2)) {
    const command = argv[0];
    if (command === 'validate-config') {
        process.stdout.write(`${JSON.stringify(validateConfig(option(argv, '--config')))}\n`);
        return 0;
    }
    if (command === 'readiness') {
        const result = await readiness(option(argv, '--config'), option(argv, '--route'));
        process.stdout.write(`${JSON.stringify(result)}\n`);
        return 0;
    }
    if (command === 'run-job') {
        await runJob(option(argv, '--job-file'), option(argv, '--config'));
        return 0;
    }
    throw Object.assign(new Error('expected validate-config, readiness, or run-job'), { terminal: true });
}

if (require.main === module) {
    cli().then((code) => { process.exitCode = code; }).catch((error) => {
        process.stderr.write(`${error.terminal === true ? 'terminal' : 'retryable'} driver failure\n`);
        process.exitCode = error.terminal === true ? 2 : 75;
    });
}

module.exports = {
    CHECKPOINT_SCHEMA,
    CONFIG_SCHEMA,
    ROUTE_ID,
    SOURCE_CHAIN_ID,
    SOURCE_PROOF_KIND,
    STAGES,
    readiness,
    runJob,
    validateConfig,
};
