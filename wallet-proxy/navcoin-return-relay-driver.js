#!/usr/bin/env node
'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { execFile } = require('node:child_process');
const { promisify } = require('node:util');

const execFileAsync = promisify(execFile);
const CONFIG_SCHEMA = 'postfiat-navcoin-return-relay-driver-config-v1';
const JOB_SCHEMA = 'postfiat-navcoin-return-relay-job-v1';
const STATE_SCHEMA = 'postfiat-navcoin-return-relay-state-v1';
const ROUTE_ID_RE = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const RELEASE_BINARY_RE = /^\/opt\/postfiat\/releases\/([a-z0-9][a-z0-9.-]{0,127})\/postfiat-node$/;

function option(argv, name) {
    const index = argv.indexOf(name);
    if (index < 0 || !argv[index + 1]) throw new Error(`missing ${name}`);
    return argv[index + 1];
}

function sha256File(file) {
    return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function secureFile(file, digest, label) {
    const absolute = path.resolve(String(file || ''));
    const stat = fs.lstatSync(absolute);
    if (!stat.isFile() || stat.isSymbolicLink() || (stat.mode & 0o022) !== 0
        || !/^[0-9a-f]{64}$/.test(String(digest || '')) || sha256File(absolute) !== digest) {
        throw new Error(`${label} failed secure hash pin validation`);
    }
    return absolute;
}

function loadConfig(file) {
    const absolute = path.resolve(file);
    const stat = fs.lstatSync(absolute);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.uid !== process.getuid()
        || (stat.mode & 0o022) !== 0 || stat.size > 64 * 1024) {
        throw new Error('NAVCoin return driver config is not an owner-controlled regular file');
    }
    const config = JSON.parse(fs.readFileSync(absolute, 'utf8'));
    if (config?.schema !== CONFIG_SCHEMA
        || !ROUTE_ID_RE.test(String(config.route_id || ''))
        || !/^[0-9a-f]{96}$/.test(String(config.route_config_digest || ''))
        || !/^0x[0-9a-f]{40}$/.test(String(config.controller || '').toLowerCase())
        || !/^0x[0-9a-f]{40}$/.test(String(config.wrapped_token || '').toLowerCase())
        || !/^[0-9a-f]{96}$/.test(String(config.native_nav_asset_id || '').toLowerCase())
        || !/^[A-Za-z0-9._-]{1,32}$/.test(String(config.wrapped_token_symbol || ''))
        || !/^[A-Za-z0-9._-]{1,32}$/.test(String(config.native_asset_code || ''))
        || !/^[a-z0-9._-]{1,96}$/.test(String(config.burn_report_schema || ''))
        || !/^\/[A-Za-z0-9/_.-]{1,255}$/.test(String(config.validator_data_dir || ''))
        || String(config.validator_data_dir).includes('..')) {
        throw new Error('NAVCoin return driver config identity mismatch');
    }
    for (const field of ['burn_inspect_script', 'return_import_script']) {
        config[field] = secureFile(config[field], config[`${field}_sha256`], field);
    }
    config.repo = path.resolve(config.repo);
    config.python = String(config.python);
    config.node_binary = String(config.node_binary);
    const release = RELEASE_BINARY_RE.exec(config.node_binary);
    if (!release || !/^[0-9a-f]{64}$/.test(String(config.node_binary_sha256 || ''))) {
        throw new Error('NAVCoin return driver node binary is not an exact signed release');
    }
    config.release_id = release[1];
    config.ethereum_rpc = String(config.ethereum_rpc);
    config.route_config_digest = config.route_config_digest.toLowerCase();
    config.controller = config.controller.toLowerCase();
    config.wrapped_token = config.wrapped_token.toLowerCase();
    config.native_nav_asset_id = config.native_nav_asset_id.toLowerCase();
    return config;
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

function state(config, job, status, stageIndex, fields = {}) {
    atomicWrite(path.join(path.dirname(job.file), 'worker-state.json'), {
        schema: STATE_SCHEMA,
        job_id: job.value.job_id,
        status,
        stage_index: stageIndex,
        retryable: !['accepted', 'failed'].includes(status),
        route_id: config.route_id,
        transaction_hash: job.value.request.transaction_hash,
        updated_at_unix: Math.floor(Date.now() / 1000),
        ...fields,
    });
}

function loadJob(file, config) {
    const absolute = path.resolve(file);
    const value = JSON.parse(fs.readFileSync(absolute, 'utf8'));
    const request = value?.request || {};
    if (value?.schema !== JOB_SCHEMA || !/^0x[0-9a-f]{64}$/.test(String(value.job_id || ''))
        || request.route_id !== config.route_id
        || request.route_config_digest !== config.route_config_digest
        || !/^0x[0-9a-f]{64}$/.test(String(request.transaction_hash || ''))
        || !/^0x[0-9a-f]{40}$/.test(String(request.ethereum_sender || ''))
        || !/^pf[0-9a-f]{40}$/.test(String(request.pftl_recipient || ''))
        || request.native_nav_asset_id !== config.native_nav_asset_id
        || !/^[1-9][0-9]*$/.test(String(request.amount_atoms || ''))
        || !/^[0-9a-f]{64}$/.test(String(request.return_nonce || ''))
        || /^0+$/.test(request.return_nonce)) {
        throw new Error('invalid durable NAVCoin return relay job');
    }
    return { file: absolute, value };
}

async function run(file, args, options = {}) {
    return execFileAsync(file, args, {
        timeout: options.timeout || 120_000,
        maxBuffer: options.maxBuffer || 4 * 1024 * 1024,
        cwd: options.cwd,
        env: options.env,
    });
}

async function ssh(config, command, timeout = 120_000) {
    return run('ssh', ['-o', 'BatchMode=yes', '-o', 'ConnectTimeout=15',
        `root@${config.validator2_host}`, command], { timeout });
}

async function cast(config, args, timeout = 60_000) {
    return run(config.cast_binary, [...args, '--rpc-url', config.ethereum_rpc], { timeout });
}

async function supplyStatus(config) {
    const command = `${config.node_binary} navcoin-bridge-supply-status `
        + `--data-dir ${config.validator_data_dir} --route-id ${config.route_id}`;
    return JSON.parse((await ssh(config, command)).stdout);
}

async function importedReturn(config, burnId) {
    const program = [
        'jq -cer',
        `--arg route '${config.route_id}'`,
        `--arg burn '${burnId}'`,
        `'first(.. | objects | select(.route_id? == $route) | .return_imports[$burn]?) // null'`,
        `${config.validator_data_dir}/ledger.json`,
    ].join(' ');
    try {
        const value = JSON.parse((await ssh(config, program)).stdout);
        return value?.status === 'imported' ? value : null;
    } catch (_) { return null; }
}

async function readiness(config) {
    const [supply, nodePin, chainRaw, tokenRaw, assetRaw] = await Promise.all([
        supplyStatus(config),
        ssh(config, `sha256sum ${config.node_binary}`),
        cast(config, ['chain-id']),
        cast(config, ['call', config.controller, 'wrappedToken()(address)']),
        cast(config, ['call', config.controller, 'nativeNavAssetId()(bytes)']),
    ]);
    const nodeDigest = String(nodePin.stdout).trim().split(/\s+/, 1)[0];
    const gates = {
        route_identity: supply.route_id === config.route_id
            && supply.route_config_digest === config.route_config_digest,
        pftl_invariant: supply.invariant_holds === true,
        pftl_live: supply.paused === false && supply.live_value_enabled === true,
        pftl_node_binary: nodeDigest === config.node_binary_sha256,
        return_class: supply.return_verification_class === 'BFT_CHECKPOINT',
        ethereum_chain: String(chainRaw.stdout).trim() === '1',
        controller_token: String(tokenRaw.stdout).trim().toLowerCase() === config.wrapped_token,
        controller_asset: String(assetRaw.stdout).trim().toLowerCase()
            === `0x${config.native_nav_asset_id}`,
    };
    const ready = Object.values(gates).every(Boolean);
    if (!ready) throw new Error(`NAVCoin return readiness gates are incomplete: ${
        Object.entries(gates).filter(([, value]) => !value).map(([key]) => key).join(',')}`);
    return {
        ok: true, ready: true, schema: 'postfiat-navcoin-return-relay-readiness-v1',
        route_id: config.route_id, route_config_digest: config.route_config_digest,
        controller: config.controller, wrapped_token: config.wrapped_token,
        native_nav_asset_id: config.native_nav_asset_id,
        native_asset_code: config.native_asset_code,
        release_id: config.release_id,
        node_binary_sha256: config.node_binary_sha256,
        wrapped_token_symbol: config.wrapped_token_symbol,
        pftl_invariant_holds: true, max_concurrent_jobs: 1,
    };
}

async function inspectBurn(config, request) {
    const { stdout } = await run(config.python, [config.burn_inspect_script,
        '--transaction-hash', request.transaction_hash,
        '--ethereum-sender', request.ethereum_sender,
        '--pftl-recipient', request.pftl_recipient,
        '--amount-atoms', request.amount_atoms,
        '--return-nonce', request.return_nonce], { timeout: 180_000, cwd: config.repo });
    const report = JSON.parse(stdout);
    if (report?.schema !== config.burn_report_schema || report.phase !== 'burned'
        || String(report.transaction?.tx || '').toLowerCase() !== request.transaction_hash
        || String(report.ethereum_sender || '').toLowerCase() !== request.ethereum_sender
        || report.pftl_recipient !== request.pftl_recipient
        || report.native_nav_asset_id !== config.native_nav_asset_id
        || String(report.amount_atoms) !== request.amount_atoms
        || report.return_nonce !== request.return_nonce
        || String(report.controller || '').toLowerCase() !== config.controller
        || String(report.wrapped_token || '').toLowerCase() !== config.wrapped_token
        || !/^[0-9a-f]{64}$/.test(String(report.return_burn_id || ''))
        || !Number.isSafeInteger(Number(report.transaction?.block_number))
        || Number(report.transaction.block_number) <= 0
        || !Number.isSafeInteger(Number(report.event_log_index))) {
        throw Object.assign(new Error('Ethereum return burn does not match the wallet request'), {
            code: 'navcoin_return_burn_binding_mismatch', terminal: true,
        });
    }
    return report;
}

function findImportSummary(attemptDir) {
    const returnDir = path.join(attemptDir, 'return');
    if (!fs.existsSync(returnDir)) return null;
    const finality = fs.readdirSync(returnDir).filter((name) => /^import-finality-h\d+$/.test(name)).sort();
    if (finality.length !== 1) return null;
    const file = path.join(returnDir, finality[0], 'summary.json');
    return fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, 'utf8')) : null;
}

async function runJob(config, jobFile) {
    const job = loadJob(jobFile, config);
    const request = job.value.request;
    state(config, job, 'waiting_for_ethereum_receipt', 2, {
        message: 'Waiting for the MetaMask return burn receipt.' });
    const burn = await inspectBurn(config, request);
    const burnId = burn.return_burn_id;
    const burnFields = { return_burn_id: burnId,
        ethereum_block_number: Number(burn.transaction.block_number) };
    const alreadyImported = await importedReturn(config, burnId);
    if (alreadyImported) {
        state(config, job, 'accepted', 6, { ...burnFields, retryable: false,
            checkpoint_finalized_height: Number(alreadyImported.finalized_height),
            message: `${config.wrapped_token_symbol} return is finalized and native ${config.native_asset_code} is available on PFTL.` });
        return;
    }
    const status = await ssh(config,
        `${config.node_binary} status --data-dir ${config.validator_data_dir}`);
    const expectedHeight = Number(JSON.parse(status.stdout).block_height) + 1;
    if (!Number.isSafeInteger(expectedHeight) || expectedHeight <= 1) throw new Error('PFTL height is unavailable');
    const attemptDir = path.join(path.dirname(job.file), 'attempts',
        `${Date.now()}-${crypto.randomBytes(4).toString('hex')}`);
    atomicWrite(path.join(attemptDir, 'return', 'ethereum-burn', 'burn.json'), burn);
    state(config, job, 'waiting_for_ethereum_finality', 3, { ...burnFields,
        message: `Waiting for Ethereum finality before restoring native ${config.native_asset_code}.` });
    const workflow = `wallet-return-${job.value.job_id.slice(2, 18)}`;
    const running = run('bash', [config.return_import_script, '--phase-dir', attemptDir,
        '--workflow-id', workflow, '--expected-pftl-height', String(expectedHeight)], {
        timeout: config.import_timeout_ms || 60 * 60 * 1000, cwd: config.repo,
        maxBuffer: 8 * 1024 * 1024,
        env: { ...process.env, NAVCOIN_CAST_BIN: config.cast_binary,
            NAVCOIN_ETHEREUM_RPC: config.ethereum_rpc },
    });
    const monitor = setInterval(() => {
        const proof = path.join(attemptDir, 'return', 'proof');
        const importSummary = findImportSummary(attemptDir);
        if (importSummary) state(config, job, 'submitting_pftl_import', 5, { ...burnFields,
            message: 'Verifying the finalized PFTL return import.' });
        else if (fs.existsSync(path.join(proof, 'receipt-proof.json'))) {
            state(config, job, 'submitting_pftl_import', 5, { ...burnFields,
                message: 'Submitting the proof-bound return import to PFTL.' });
        } else if (fs.existsSync(path.join(proof, 'checkpoint.json'))) {
            state(config, job, 'proving_ethereum_receipt', 4, { ...burnFields,
                message: 'Building and certifying the Ethereum receipt proof.' });
        }
    }, 5_000);
    monitor.unref?.();
    try { await running; } finally { clearInterval(monitor); }
    const summary = JSON.parse(fs.readFileSync(path.join(attemptDir, 'return', 'summary.json'), 'utf8'));
    const importSummary = findImportSummary(attemptDir);
    if (summary?.verdict !== 'PASS' || summary.ethereum_burn_tx !== request.transaction_hash
        || summary.burn_event_hash !== burnId || importSummary?.accepted !== true
        || importSummary.confirmed !== true || !/^[0-9a-f]{96}$/.test(String(importSummary.tx_id || ''))) {
        throw Object.assign(new Error('NAVCoin return completion binding mismatch'), {
            code: 'navcoin_return_completion_mismatch', terminal: true,
        });
    }
    const final = await importedReturn(config, burnId);
    if (!final || final.pftl_recipient !== request.pftl_recipient
        || String(final.amount_atoms) !== request.amount_atoms
        || final.ethereum_sender !== request.ethereum_sender) {
        throw new Error('NAVCoin return completed locally but PFTL terminal state is not visible');
    }
    state(config, job, 'accepted', 6, { ...burnFields, retryable: false,
        checkpoint_finalized_height: Number(summary.checkpoint_finalized_height),
        pftl_height: Number(summary.pftl_height), pftl_tx_id: importSummary.tx_id,
        message: `${config.wrapped_token_symbol} return is finalized and native ${config.native_asset_code} is available on PFTL.` });
}

function terminalError(error) { return error?.terminal === true; }

async function main() {
    const argv = process.argv.slice(2);
    const command = argv[0];
    const config = loadConfig(option(argv, '--config'));
    if (command === 'readiness') {
        process.stdout.write(`${JSON.stringify(await readiness(config), null, 2)}\n`);
        return;
    }
    if (command === 'run-job') {
        const job = loadJob(option(argv, '--job-file'), config);
        try { await runJob(config, job.file); } catch (error) {
            const terminal = terminalError(error);
            state(config, job, terminal ? 'failed' : 'retry_wait', terminal ? 7 : 1, {
                retryable: !terminal,
                code: String(error.code || (terminal ? 'navcoin_return_failed' : 'navcoin_return_retryable')).slice(0, 64),
                message: terminal
                    ? 'NAVCoin return failed a binding or safety gate.'
                    : 'NAVCoin return relay will resume from the finalized MetaMask transaction.',
            });
            throw error;
        }
        return;
    }
    throw new Error('expected readiness or run-job');
}

if (require.main === module) {
    main().catch((error) => {
        process.stderr.write(`${error.stack || error.message}\n`);
        process.exitCode = 1;
    });
}

module.exports = { loadConfig, terminalError };
