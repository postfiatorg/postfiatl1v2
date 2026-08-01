#!/usr/bin/env node
'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const net = require('node:net');
const path = require('node:path');
const { execFile, spawn } = require('node:child_process');
const { promisify } = require('node:util');

const execFileAsync = promisify(execFile);
const CONFIG_SCHEMA = 'postfiat-navcoin-export-relay-driver-config-v1';
const JOB_SCHEMA = 'postfiat-navcoin-export-relay-job-v1';
const STATE_SCHEMA = 'postfiat-navcoin-export-relay-state-v1';
const ROUTE_ID_RE = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const HASH48_RE = /^[0-9a-f]{96}$/;
const HASH32_RE = /^[0-9a-f]{64}$/;
const EVM_RE = /^0x[0-9a-f]{40}$/;

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
        throw new Error('NAVCoin export driver config is not an owner-controlled regular file');
    }
    const config = JSON.parse(fs.readFileSync(absolute, 'utf8'));
    if (config?.schema !== CONFIG_SCHEMA
        || !ROUTE_ID_RE.test(String(config.route_id || ''))
        || !/^[0-9a-f]{96}$/.test(String(config.route_config_digest || ''))
        || !EVM_RE.test(String(config.controller || '').toLowerCase())
        || !EVM_RE.test(String(config.wrapped_token || '').toLowerCase())
        || !EVM_RE.test(String(config.verifier || '').toLowerCase())
        || !/^0x[0-9a-f]{64}$/.test(String(config.program_vkey || '').toLowerCase())
        || !/^[A-Za-z0-9._-]{1,32}$/.test(String(config.wrapped_token_symbol || ''))
        || !/^[A-Za-z0-9._-]{1,32}$/.test(String(config.native_asset_code || ''))
        || !/^\/[A-Za-z0-9/_.-]{1,255}$/.test(String(config.validator_data_dir || ''))
        || String(config.validator_data_dir).includes('..')
        || !/^[A-Za-z0-9/_.-]{1,255}$/.test(String(config.source_packet_marker || ''))
        || String(config.source_packet_marker).startsWith('/')
        || String(config.source_packet_marker).split('/').includes('..')) {
        throw new Error('NAVCoin export driver config identity mismatch');
    }
    const required = ['proof_script', 'accept_script', 'checkpoint_script', 'destination_script'];
    for (const field of required) {
        config[field] = secureFile(config[field], config[`${field}_sha256`], field);
    }
    config.repo = path.resolve(config.repo);
    config.node_binary = String(config.node_binary);
    config.ethereum_rpc = String(config.ethereum_rpc);
    config.signer_socket = path.resolve(String(config.signer_socket || ''));
    if (!/^[0-9a-f]{64}$/.test(String(config.signer_policy_hash || ''))
        || !EVM_RE.test(String(config.signer_address || '').toLowerCase())) {
        throw new Error('NAVCoin export driver signer identity is malformed');
    }
    config.route_config_digest = config.route_config_digest.toLowerCase();
    config.controller = config.controller.toLowerCase();
    config.wrapped_token = config.wrapped_token.toLowerCase();
    config.verifier = config.verifier.toLowerCase();
    config.program_vkey = config.program_vkey.toLowerCase();
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
        packet_hash: job.value.request.packet_hash,
        source_height: job.value.source_height,
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
        || !HASH48_RE.test(String(request.packet_hash || ''))
        || !HASH32_RE.test(String(request.packet_digest || ''))
        || !EVM_RE.test(String(request.ethereum_recipient || ''))
        || !/^[1-9][0-9]*$/.test(String(request.amount_atoms || ''))
        || !['AwaitingSourceDebit', 'SourceDebited'].includes(value.source_status_at_creation)
        || (value.source_status_at_creation === 'AwaitingSourceDebit'
            && !Number.isSafeInteger(Number(request.deadline_seconds)))
        || (value.source_status_at_creation === 'SourceDebited'
            && (!Number.isSafeInteger(Number(value.source_height)) || Number(value.source_height) <= 0))) {
        throw new Error('invalid durable NAVCoin export relay job');
    }
    return { file: absolute, value };
}

async function run(file, args, options = {}) {
    return execFileAsync(file, args, {
        timeout: options.timeout || 60_000,
        maxBuffer: options.maxBuffer || 4 * 1024 * 1024,
        cwd: options.cwd,
        env: options.env,
    });
}

async function ssh(config, host, command, port = null, timeout = 60_000) {
    const args = ['-o', 'BatchMode=yes', '-o', 'ConnectTimeout=15'];
    if (port) args.push('-p', String(port));
    args.push(`root@${host}`, command);
    return run('ssh', args, { timeout });
}

async function cast(config, args, timeout = 60_000) {
    return run(config.cast_binary, [...args, '--rpc-url', config.ethereum_rpc], { timeout });
}

async function signerStatus(config) {
    const request = `${JSON.stringify({
        schema: 'postfiat.constrained_signer.request.v1',
        op: 'status',
    })}\n`;
    return new Promise((resolve, reject) => {
        const client = net.createConnection(config.signer_socket);
        let response = '';
        const timer = setTimeout(() => {
            client.destroy();
            reject(new Error('constrained signer status timed out'));
        }, 5_000);
        client.setEncoding('utf8');
        client.on('connect', () => client.end(request));
        client.on('data', (chunk) => {
            response += chunk;
            if (Buffer.byteLength(response, 'utf8') > 256 * 1024) {
                client.destroy(new Error('constrained signer response exceeds 256 KiB'));
            }
        });
        client.on('error', (error) => {
            clearTimeout(timer);
            reject(error);
        });
        client.on('end', () => {
            clearTimeout(timer);
            try {
                const parsed = JSON.parse(response.trim());
                if (parsed?.schema !== 'postfiat.constrained_signer.response.v1') {
                    throw new Error('constrained signer response schema mismatch');
                }
                resolve(parsed);
            } catch (error) { reject(error); }
        });
    });
}

async function packetStatus(config, packetHash) {
    const command = [
        config.node_binary, 'navcoin-bridge-packet',
        '--data-dir', config.validator_data_dir, '--route-id', config.route_id,
        '--packet-hash', packetHash,
    ].join(' ');
    const [statusRaw, digestRaw] = await Promise.all([
        ssh(config, config.validator2_host, command),
        ssh(config, config.validator2_host, [
            'jq -er',
            `--arg route '${config.route_id}'`,
            `--arg packet '${packetHash}'`,
            `'[.pftl_uniswap_routes[] | select(.route_id==$route)`,
            '| .export_packets[$packet].ethereum_packet_digest] ',
            '| if length==1 and (.[0]|type)=="string" then .[0] else error("packet digest lookup failed") end\'',
            `${config.validator_data_dir}/ledger.json`,
        ].join(' ')),
    ]);
    return { ...JSON.parse(statusRaw.stdout),
        ethereum_packet_digest: String(digestRaw.stdout).trim().toLowerCase() };
}

async function packetConsumed(config, packetDigest) {
    const { stdout } = await cast(config, [
        'call', config.controller, 'consumedPacket(bytes32)(bool)', `0x${packetDigest}`,
    ]);
    return String(stdout).trim() === 'true';
}

async function inspect(config, request) {
    const report = await packetStatus(config, request.packet_hash);
    const packet = report?.packet || {};
    if (report?.route_id !== config.route_id
        || report?.route_config_digest !== config.route_config_digest
        || report.packet_hash !== request.packet_hash || packet.packet_hash !== request.packet_hash
        || report.ethereum_packet_digest !== request.packet_digest
        || String(packet.ethereum_recipient || '').toLowerCase() !== request.ethereum_recipient
        || String(packet.amount_atoms) !== request.amount_atoms
        || (request.deadline_seconds != null
            && Number(packet.destination_deadline_seconds) !== Number(request.deadline_seconds))
        || !Number.isSafeInteger(Number(packet.source_height))) {
        throw Object.assign(new Error('PFTL export packet does not match the relay request'), {
            code: 'navcoin_export_packet_binding_mismatch', terminal: true,
        });
    }
    const consumed = await packetConsumed(config, request.packet_digest);
    if (packet.status === 'DestinationConsumed' && !consumed) {
        throw Object.assign(new Error('PFTL reports destination consume but Ethereum packet is not consumed'), {
            code: 'navcoin_export_cross_chain_mismatch', terminal: true,
        });
    }
    if (!['SourceDebited', 'DestinationConsumed'].includes(packet.status)) {
        throw Object.assign(new Error(`NAVCoin export packet is not relayable: ${packet.status}`), {
            code: 'navcoin_export_packet_not_relayable', terminal: true,
        });
    }
    return {
        ok: true,
        route_id: config.route_id,
        route_config_digest: config.route_config_digest,
        packet_hash: request.packet_hash,
        packet_digest: request.packet_digest,
        ethereum_recipient: request.ethereum_recipient,
        amount_atoms: request.amount_atoms,
        source_height: Number(packet.source_height),
        packet_status: packet.status,
        ethereum_packet_consumed: consumed,
    };
}

async function readiness(config) {
    const supplyCommand = `${config.node_binary} navcoin-bridge-supply-status --data-dir ${config.validator_data_dir} --route-id ${config.route_id}`;
    const [supplyRaw, remotePins, chainRaw, vkeyRaw, pausedRaw, symbolRaw, signer] = await Promise.all([
        ssh(config, config.validator2_host, supplyCommand),
        ssh(config, config.a100_host,
            `sha256sum ${config.a100_prover} ${config.a100_elf}`,
            config.a100_port),
        cast(config, ['chain-id']),
        cast(config, ['call', config.verifier, 'programVKey()(bytes32)']),
        cast(config, ['call', config.controller, 'mintPaused()(bool)']),
        cast(config, ['call', config.wrapped_token, 'symbol()(string)']),
        signerStatus(config),
    ]);
    const supply = JSON.parse(supplyRaw.stdout);
    const pins = new Map(String(remotePins.stdout).trim().split('\n').map((line) => {
        const [digest, file] = line.trim().split(/\s+/, 2); return [file, digest];
    }));
    const gates = {
        route_identity: supply.route_id === config.route_id
            && supply.route_config_digest === config.route_config_digest,
        pftl_invariant: supply.invariant_holds === true,
        pftl_live: supply.paused === false && supply.live_value_enabled === true,
        ethereum_chain: String(chainRaw.stdout).trim() === '1',
        verifier_vkey: String(vkeyRaw.stdout).trim().toLowerCase() === config.program_vkey,
        mint_unpaused: String(pausedRaw.stdout).trim() === 'false',
        token_identity: String(symbolRaw.stdout).trim().replaceAll('"', '')
            === config.wrapped_token_symbol,
        prover_binary: pins.get(config.a100_prover) === config.a100_prover_sha256,
        prover_program: pins.get(config.a100_elf) === config.a100_elf_sha256,
        signer: Boolean(signer.ok && signer.ready && !signer.locked
            && signer.policy_hash === config.signer_policy_hash
            && String(signer.address || '').toLowerCase() === String(config.signer_address).toLowerCase()
            && Array.isArray(signer.chains) && signer.chains.includes(1)
            && Array.isArray(signer.routes) && signer.routes.includes(config.route_id)),
    };
    const ready = Object.values(gates).every(Boolean);
    if (!ready) {
        const failed = Object.entries(gates).filter(([, value]) => !value).map(([key]) => key);
        throw new Error(`NAVCoin export relay readiness gates are incomplete: ${failed.join(',')}`);
    }
    return {
        ok: true, ready: true, schema: 'postfiat-navcoin-export-relay-readiness-v1',
        route_id: config.route_id, route_config_digest: config.route_config_digest,
        wrapped_token: config.wrapped_token, controller: config.controller,
        verifier: config.verifier, program_vkey: config.program_vkey,
        native_asset_code: config.native_asset_code,
        wrapped_token_symbol: config.wrapped_token_symbol, pftl_invariant_holds: true,
        prover_authenticated: true, prover_healthy: true, signer_unlocked: true,
        max_concurrent_jobs: 1,
    };
}

function proofStage(config, phaseDir) {
    if (fs.existsSync(path.join(phaseDir, 'completion.json'))) return ['accepted', 8];
    if (fs.existsSync(path.join(phaseDir, 'destination-consume', 'proof', 'checkpoint.json'))) return ['acknowledging_pftl', 7];
    if (fs.existsSync(path.join(phaseDir, 'ethereum', 'mint-state.json'))) return ['waiting_for_ethereum_finality', 6];
    if (fs.existsSync(path.join(phaseDir, 'export-proof', 'proof-cuda', 'proof-report.json'))) return ['submitting_ethereum_mint', 5];
    if (fs.existsSync(path.join(phaseDir, 'export-proof', 'receipt-witness.json'))) return ['proving_receipt', 4];
    if (fs.existsSync(path.join(phaseDir, config.source_packet_marker))) return ['waiting_for_pftl_checkpoint', 3];
    return ['discovering_packet', 2];
}

async function spawnProof(config, job, phaseDir) {
    const request = job.value.request;
    const args = [config.proof_script, '--resume', '--packet-hash', request.packet_hash,
        '--phase-dir', phaseDir, '--workflow-id', `navcoin-relay-${job.value.job_id.slice(2, 18)}`,
        '--expected-recipient', request.ethereum_recipient,
        '--expected-amount-atoms', request.amount_atoms];
    await new Promise((resolve, reject) => {
        const child = spawn('bash', args, { cwd: config.repo, stdio: 'inherit', env: process.env });
        child.once('error', reject);
        child.once('exit', (code, signal) => code === 0 ? resolve() : reject(Object.assign(
            new Error(`proof harness exited ${code ?? signal}`), { code: 'navcoin_export_proof_failed' },
        )));
    });
}

async function runJob(config, jobFile) {
    const job = loadJob(jobFile, config);
    const request = job.value.request;
    if (job.value.source_status_at_creation === 'AwaitingSourceDebit'
        && Number(request.deadline_seconds) <= Math.floor(Date.now() / 1000)) {
        throw Object.assign(new Error('pre-armed NAVCoin export request expired before source debit'), {
            code: 'navcoin_export_request_expired', terminal: true,
        });
    }
    const initial = await inspect(config, request);
    if (job.value.source_status_at_creation === 'SourceDebited'
        && Number(initial.source_height) !== Number(job.value.source_height)) {
        throw Object.assign(new Error('persisted NAVCoin source height changed'), {
            code: 'navcoin_export_source_height_changed', terminal: true,
        });
    }
    if (job.value.source_status_at_creation === 'AwaitingSourceDebit') {
        job.value = {
            ...job.value,
            source_height: Number(initial.source_height),
            source_status_at_creation: 'SourceDebited',
            updated_at_unix: Math.floor(Date.now() / 1000),
        };
        atomicWrite(job.file, job.value);
    }
    if (job.value.source_status_at_creation === 'SourceDebited'
        && initial.packet_status === 'DestinationConsumed' && initial.ethereum_packet_consumed) {
        state(config, job, 'accepted', 8, { retryable: false, receipt_id: request.packet_hash,
            message: `${config.wrapped_token_symbol} mint and PFTL destination acknowledgement are finalized.` });
        return;
    }
    const phaseDir = path.join(path.dirname(job.file), 'proof');
    state(config, job, 'discovering_packet', 2, { message: 'Verifying the finalized PFTL export packet.' });
    const monitor = setInterval(() => {
        const [status, index] = proofStage(config, phaseDir);
        if (status !== 'accepted') state(config, job, status, index,
            { message: 'Proof-bound export relay is running.' });
    }, 5_000);
    monitor.unref?.();
    try { await spawnProof(config, job, phaseDir); } finally { clearInterval(monitor); }
    const completion = JSON.parse(fs.readFileSync(path.join(phaseDir, 'completion.json'), 'utf8'));
    if (completion?.verdict !== 'PASS' || completion.packet_hash !== request.packet_hash
        || String(completion.recipient || '').toLowerCase() !== request.ethereum_recipient
        || String(completion.amount_atoms) !== request.amount_atoms
        || !/^0x[0-9a-f]{64}$/.test(String(completion.mint_tx || '').toLowerCase())) {
        throw Object.assign(new Error('NAVCoin export completion binding mismatch'), {
            code: 'navcoin_export_completion_mismatch', terminal: true,
        });
    }
    const final = await inspect(config, request);
    if (final.packet_status !== 'DestinationConsumed' || !final.ethereum_packet_consumed) {
        throw new Error('NAVCoin export completed locally but cross-chain terminal state is not visible');
    }
    state(config, job, 'accepted', 8, { retryable: false, receipt_id: request.packet_hash,
        ethereum_tx_hash: String(completion.mint_tx).toLowerCase(),
        message: `${config.wrapped_token_symbol} mint and PFTL destination acknowledgement are finalized.` });
}

function terminalError(error) {
    // Only errors deliberately classified at the safety-check site may stop a
    // durable job. Child-process stderr can contain a full CLI usage page; it
    // is not a stable error protocol and must never turn a transient
    // pre-armed "packet unknown" response into a terminal loss of service.
    return error?.terminal === true;
}

async function main() {
    const argv = process.argv.slice(2);
    const command = argv[0];
    const config = loadConfig(option(argv, '--config'));
    if (command === 'readiness') {
        process.stdout.write(`${JSON.stringify(await readiness(config), null, 2)}\n`);
        return;
    }
    if (command === 'inspect') {
        const request = {
            packet_hash: option(argv, '--packet-hash').toLowerCase(),
            packet_digest: option(argv, '--packet-digest').toLowerCase().replace(/^0x/, ''),
            ethereum_recipient: option(argv, '--ethereum-recipient').toLowerCase(),
            amount_atoms: option(argv, '--amount-atoms'),
        };
        const result = await inspect(config, request);
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
        return;
    }
    if (command === 'run-job') {
        const job = loadJob(option(argv, '--job-file'), config);
        try {
            await runJob(config, job.file);
        } catch (error) {
            const terminal = terminalError(error);
            state(config, job, terminal ? 'failed' : 'retry_wait', terminal ? 9 : 1, {
                retryable: !terminal,
                code: String(error.code || (terminal ? 'navcoin_export_failed' : 'navcoin_export_retryable')).slice(0, 64),
                message: terminal
                    ? 'NAVCoin export failed a binding or safety gate.'
                    : 'NAVCoin export relay will resume from durable evidence.',
            });
            throw error;
        }
        return;
    }
    throw new Error('expected readiness, inspect, or run-job');
}

if (require.main === module) {
    main().catch((error) => {
        process.stderr.write(`${error.stack || error.message}\n`);
        process.exitCode = 1;
    });
}

module.exports = { loadConfig, terminalError };
