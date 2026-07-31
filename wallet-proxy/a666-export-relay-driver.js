#!/usr/bin/env node
'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { execFile, spawn } = require('node:child_process');
const { promisify } = require('node:util');

const execFileAsync = promisify(execFile);
const CONFIG_SCHEMA = 'postfiat-a666-export-relay-driver-config-v1';
const JOB_SCHEMA = 'postfiat-a666-export-relay-job-v1';
const STATE_SCHEMA = 'postfiat-a666-export-relay-state-v1';
const ROUTE_ID = 'pftl-a666-ethereum-wA666-usdc-v1';
const ROUTE_DIGEST = '12ed00ca87e29554ce4b978da1710fffc0830767e84e62f08df257f727db953efdd89bcf6ea99f5634d6e5ea8aca2933';
const CONTROLLER = '0x9a0262c0572fb4db08765408eb225e207f40c3d9';
const TOKEN = '0xee4c92edb03efdd9b519339edc19ad70c69a9be5';
const VERIFIER = '0xb79ff97ecc11574a8a78d0b5a9d7c8c2a94bf96a';
const PROGRAM_VKEY = '0x004e44aca326861252ee5ff7863b1174635b727759b75d46b28bb28d4a7b34f9';
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
        throw new Error('A666 export driver config is not an owner-controlled regular file');
    }
    const config = JSON.parse(fs.readFileSync(absolute, 'utf8'));
    if (config?.schema !== CONFIG_SCHEMA || config.route_id !== ROUTE_ID
        || config.route_config_digest !== ROUTE_DIGEST
        || String(config.controller || '').toLowerCase() !== CONTROLLER
        || String(config.wrapped_token || '').toLowerCase() !== TOKEN
        || String(config.verifier || '').toLowerCase() !== VERIFIER
        || String(config.program_vkey || '').toLowerCase() !== PROGRAM_VKEY) {
        throw new Error('A666 export driver config identity mismatch');
    }
    const required = ['proof_script', 'accept_script', 'checkpoint_script', 'destination_script'];
    for (const field of required) {
        config[field] = secureFile(config[field], config[`${field}_sha256`], field);
    }
    config.repo = path.resolve(config.repo);
    config.node_binary = String(config.node_binary);
    config.ethereum_rpc = String(config.ethereum_rpc);
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

function state(job, status, stageIndex, fields = {}) {
    atomicWrite(path.join(path.dirname(job.file), 'worker-state.json'), {
        schema: STATE_SCHEMA,
        job_id: job.value.job_id,
        status,
        stage_index: stageIndex,
        retryable: !['accepted', 'failed'].includes(status),
        route_id: ROUTE_ID,
        packet_hash: job.value.request.packet_hash,
        source_height: job.value.source_height,
        updated_at_unix: Math.floor(Date.now() / 1000),
        ...fields,
    });
}

function loadJob(file) {
    const absolute = path.resolve(file);
    const value = JSON.parse(fs.readFileSync(absolute, 'utf8'));
    const request = value?.request || {};
    if (value?.schema !== JOB_SCHEMA || !/^0x[0-9a-f]{64}$/.test(String(value.job_id || ''))
        || request.route_id !== ROUTE_ID || request.route_config_digest !== ROUTE_DIGEST
        || !HASH48_RE.test(String(request.packet_hash || ''))
        || !HASH32_RE.test(String(request.packet_digest || ''))
        || !EVM_RE.test(String(request.ethereum_recipient || ''))
        || !/^[1-9][0-9]*$/.test(String(request.amount_atoms || ''))
        || !['AwaitingSourceDebit', 'SourceDebited'].includes(value.source_status_at_creation)
        || (value.source_status_at_creation === 'AwaitingSourceDebit'
            && !Number.isSafeInteger(Number(request.deadline_seconds)))
        || (value.source_status_at_creation === 'SourceDebited'
            && (!Number.isSafeInteger(Number(value.source_height)) || Number(value.source_height) <= 0))) {
        throw new Error('invalid durable A666 export relay job');
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

async function packetStatus(config, packetHash) {
    const command = [
        config.node_binary, 'navcoin-bridge-packet',
        '--data-dir', '/var/lib/postfiat/validator-2', '--route-id', ROUTE_ID,
        '--packet-hash', packetHash,
    ].join(' ');
    const [statusRaw, digestRaw] = await Promise.all([
        ssh(config, config.validator2_host, command),
        ssh(config, config.validator2_host, [
            'jq -er',
            `--arg route '${ROUTE_ID}'`,
            `--arg packet '${packetHash}'`,
            `'[.pftl_uniswap_routes[] | select(.route_id==$route)`,
            '| .export_packets[$packet].ethereum_packet_digest] ',
            '| if length==1 and (.[0]|type)=="string" then .[0] else error("packet digest lookup failed") end\'',
            '/var/lib/postfiat/validator-2/ledger.json',
        ].join(' ')),
    ]);
    return { ...JSON.parse(statusRaw.stdout),
        ethereum_packet_digest: String(digestRaw.stdout).trim().toLowerCase() };
}

async function packetConsumed(config, packetDigest) {
    const { stdout } = await cast(config, [
        'call', CONTROLLER, 'consumedPacket(bytes32)(bool)', `0x${packetDigest}`,
    ]);
    return String(stdout).trim() === 'true';
}

async function inspect(config, request) {
    const report = await packetStatus(config, request.packet_hash);
    const packet = report?.packet || {};
    if (report?.route_id !== ROUTE_ID || report?.route_config_digest !== ROUTE_DIGEST
        || report.packet_hash !== request.packet_hash || packet.packet_hash !== request.packet_hash
        || report.ethereum_packet_digest !== request.packet_digest
        || String(packet.ethereum_recipient || '').toLowerCase() !== request.ethereum_recipient
        || String(packet.amount_atoms) !== request.amount_atoms
        || (request.deadline_seconds != null
            && Number(packet.destination_deadline_seconds) !== Number(request.deadline_seconds))
        || !Number.isSafeInteger(Number(packet.source_height))) {
        throw Object.assign(new Error('PFTL export packet does not match the relay request'), {
            code: 'a666_export_packet_binding_mismatch', terminal: true,
        });
    }
    const consumed = await packetConsumed(config, request.packet_digest);
    if (packet.status === 'DestinationConsumed' && !consumed) {
        throw Object.assign(new Error('PFTL reports destination consume but Ethereum packet is not consumed'), {
            code: 'a666_export_cross_chain_mismatch', terminal: true,
        });
    }
    if (!['SourceDebited', 'DestinationConsumed'].includes(packet.status)) {
        throw Object.assign(new Error(`A666 export packet is not relayable: ${packet.status}`), {
            code: 'a666_export_packet_not_relayable', terminal: true,
        });
    }
    return {
        ok: true,
        route_id: ROUTE_ID,
        route_config_digest: ROUTE_DIGEST,
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
    const supplyCommand = `${config.node_binary} navcoin-bridge-supply-status --data-dir /var/lib/postfiat/validator-2 --route-id ${ROUTE_ID}`;
    const [supplyRaw, remotePins, chainRaw, vkeyRaw, pausedRaw, symbolRaw, agentRaw] = await Promise.all([
        ssh(config, config.validator2_host, supplyCommand),
        ssh(config, config.a100_host,
            `sha256sum ${config.a100_prover} ${config.a100_elf}`,
            config.a100_port),
        cast(config, ['chain-id']),
        cast(config, ['call', VERIFIER, 'programVKey()(bytes32)']),
        cast(config, ['call', CONTROLLER, 'mintPaused()(bool)']),
        cast(config, ['call', TOKEN, 'symbol()(string)']),
        run(config.stakehub_python, ['-c', [
            'import json',
            'from stakehub.agentd import call',
            'r=call({"op":"status"},timeout=5)',
            `targets={${JSON.stringify(VERIFIER)},${JSON.stringify(CONTROLLER)}}`,
            'wl={str(x).lower() for x in (r or {}).get("policy",{}).get("whitelist",[])}',
            'print(json.dumps({"ok":bool(r and r.get("ok")),"unlocked":bool(r and r.get("unlocked")),"targets":targets.issubset(wl)}))',
        ].join(';')], { timeout: 10_000, env: { ...process.env, PYTHONPATH: config.stakehub_repo } }),
    ]);
    const supply = JSON.parse(supplyRaw.stdout);
    const pins = new Map(String(remotePins.stdout).trim().split('\n').map((line) => {
        const [digest, file] = line.trim().split(/\s+/, 2); return [file, digest];
    }));
    const agent = JSON.parse(agentRaw.stdout);
    const gates = {
        route_identity: supply.route_id === ROUTE_ID && supply.route_config_digest === ROUTE_DIGEST,
        pftl_invariant: supply.invariant_holds === true,
        pftl_live: supply.paused === false && supply.live_value_enabled === true,
        ethereum_chain: String(chainRaw.stdout).trim() === '1',
        verifier_vkey: String(vkeyRaw.stdout).trim().toLowerCase() === PROGRAM_VKEY,
        mint_unpaused: String(pausedRaw.stdout).trim() === 'false',
        token_identity: String(symbolRaw.stdout).trim().replaceAll('"', '') === 'wA666',
        prover_binary: pins.get(config.a100_prover) === config.a100_prover_sha256,
        prover_program: pins.get(config.a100_elf) === config.a100_elf_sha256,
        signer: Boolean(agent.ok && agent.unlocked && agent.targets),
    };
    const ready = Object.values(gates).every(Boolean);
    if (!ready) {
        const failed = Object.entries(gates).filter(([, value]) => !value).map(([key]) => key);
        throw new Error(`A666 export relay readiness gates are incomplete: ${failed.join(',')}`);
    }
    return {
        ok: true, ready: true, schema: 'postfiat-a666-export-relay-readiness-v1',
        route_id: ROUTE_ID, route_config_digest: ROUTE_DIGEST,
        wrapped_token: TOKEN, controller: CONTROLLER, verifier: VERIFIER,
        program_vkey: PROGRAM_VKEY, pftl_invariant_holds: true,
        prover_authenticated: true, prover_healthy: true, signer_unlocked: true,
        max_concurrent_jobs: 1,
    };
}

function proofStage(phaseDir) {
    if (fs.existsSync(path.join(phaseDir, 'completion.json'))) return ['accepted', 8];
    if (fs.existsSync(path.join(phaseDir, 'destination-consume', 'proof', 'checkpoint.json'))) return ['acknowledging_pftl', 7];
    if (fs.existsSync(path.join(phaseDir, 'ethereum', 'mint-state.json'))) return ['waiting_for_ethereum_finality', 6];
    if (fs.existsSync(path.join(phaseDir, 'export-proof', 'proof-cuda', 'proof-report.json'))) return ['submitting_ethereum_mint', 5];
    if (fs.existsSync(path.join(phaseDir, 'export-proof', 'receipt-witness.json'))) return ['proving_receipt', 4];
    if (fs.existsSync(path.join(phaseDir, 'a666', 'export-packet-before-proof.json'))) return ['waiting_for_pftl_checkpoint', 3];
    return ['discovering_packet', 2];
}

async function spawnProof(config, job, phaseDir) {
    const request = job.value.request;
    const args = [config.proof_script, '--resume', '--packet-hash', request.packet_hash,
        '--phase-dir', phaseDir, '--workflow-id', `a666-relay-${job.value.job_id.slice(2, 18)}`,
        '--expected-recipient', request.ethereum_recipient,
        '--expected-amount-atoms', request.amount_atoms];
    await new Promise((resolve, reject) => {
        const child = spawn('bash', args, { cwd: config.repo, stdio: 'inherit', env: process.env });
        child.once('error', reject);
        child.once('exit', (code, signal) => code === 0 ? resolve() : reject(Object.assign(
            new Error(`proof harness exited ${code ?? signal}`), { code: 'a666_export_proof_failed' },
        )));
    });
}

async function runJob(config, jobFile) {
    const job = loadJob(jobFile);
    const request = job.value.request;
    if (job.value.source_status_at_creation === 'AwaitingSourceDebit'
        && Number(request.deadline_seconds) <= Math.floor(Date.now() / 1000)) {
        throw Object.assign(new Error('pre-armed A666 export request expired before source debit'), {
            code: 'a666_export_request_expired', terminal: true,
        });
    }
    const initial = await inspect(config, request);
    if (job.value.source_status_at_creation === 'SourceDebited'
        && Number(initial.source_height) !== Number(job.value.source_height)) {
        throw Object.assign(new Error('persisted A666 source height changed'), {
            code: 'a666_export_source_height_changed', terminal: true,
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
        state(job, 'accepted', 8, { retryable: false, receipt_id: request.packet_hash,
            message: 'wA666 mint and PFTL destination acknowledgement are finalized.' });
        return;
    }
    const phaseDir = path.join(path.dirname(job.file), 'proof');
    state(job, 'discovering_packet', 2, { message: 'Verifying the finalized PFTL export packet.' });
    const monitor = setInterval(() => {
        const [status, index] = proofStage(phaseDir);
        if (status !== 'accepted') state(job, status, index, { message: 'Trustless export relay is running.' });
    }, 5_000);
    monitor.unref?.();
    try { await spawnProof(config, job, phaseDir); } finally { clearInterval(monitor); }
    const completion = JSON.parse(fs.readFileSync(path.join(phaseDir, 'completion.json'), 'utf8'));
    if (completion?.verdict !== 'PASS' || completion.packet_hash !== request.packet_hash
        || String(completion.recipient || '').toLowerCase() !== request.ethereum_recipient
        || String(completion.amount_atoms) !== request.amount_atoms
        || !/^0x[0-9a-f]{64}$/.test(String(completion.mint_tx || '').toLowerCase())) {
        throw Object.assign(new Error('A666 export completion binding mismatch'), {
            code: 'a666_export_completion_mismatch', terminal: true,
        });
    }
    const final = await inspect(config, request);
    if (final.packet_status !== 'DestinationConsumed' || !final.ethereum_packet_consumed) {
        throw new Error('A666 export completed locally but cross-chain terminal state is not visible');
    }
    state(job, 'accepted', 8, { retryable: false, receipt_id: request.packet_hash,
        ethereum_tx_hash: String(completion.mint_tx).toLowerCase(),
        message: 'wA666 mint and PFTL destination acknowledgement are finalized.' });
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
        const job = loadJob(option(argv, '--job-file'));
        try {
            await runJob(config, job.file);
        } catch (error) {
            const terminal = terminalError(error);
            state(job, terminal ? 'failed' : 'retry_wait', terminal ? 9 : 1, {
                retryable: !terminal,
                code: String(error.code || (terminal ? 'a666_export_failed' : 'a666_export_retryable')).slice(0, 64),
                message: terminal
                    ? 'A666 export failed a binding or safety gate.'
                    : 'A666 export relay will resume from durable evidence.',
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

module.exports = { terminalError };
