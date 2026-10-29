#!/usr/bin/env node
'use strict';

// Switches the ingress deposit permit and every pin that depends on it in one
// journaled step. Pin checks in eth-fast-lane-driver.js stay unchanged: this tool
// only rewrites the expected hashes, then proves a fresh driver readiness.

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');
const driver = require('./eth-fast-lane-driver');

const RECORD_SCHEMA = 'postfiat-ingress-permit-activation-record-v1';
const PERMIT_BASENAME = 'ingress-authorization.json';
const PENDING_MARKER = '.ingress-permit-activation.pending.json';
// Test-only fault injection: kill this process right after the permit rename.
const FAULT_ENV = 'PFT_INGRESS_PERMIT_ACTIVATE_FAULT';
const TERMINAL_JOB_STATES = new Set(['accepted', 'failed']);

const sha256 = (bytes) => crypto.createHash('sha256').update(bytes).digest('hex');
const fileHash = (file) => sha256(fs.readFileSync(file));
const nowIso = () => new Date().toISOString();

function refuse(message, code = 'activation_refused') {
    throw Object.assign(new Error(message), { code });
}

function fsyncDir(directory) {
    const fd = fs.openSync(directory, 'r');
    try { fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
}

// Temp file in the target directory + rename; keeps the given mode and owner.
function replaceFile(target, bytes, { mode, uid, gid }) {
    const directory = path.dirname(target);
    const temporary = path.join(directory,
        `.${path.basename(target)}.${process.pid}.${crypto.randomBytes(6).toString('hex')}.tmp`);
    const fd = fs.openSync(temporary, 'wx', 0o600);
    try {
        fs.writeFileSync(fd, bytes);
        fs.fchmodSync(fd, mode);
        const stat = fs.fstatSync(fd);
        if (stat.uid !== uid || stat.gid !== gid) fs.fchownSync(fd, uid, gid);
        fs.fsyncSync(fd);
    } catch (error) {
        fs.closeSync(fd);
        fs.rmSync(temporary, { force: true });
        throw error;
    }
    fs.closeSync(fd);
    fs.renameSync(temporary, target);
    fsyncDir(directory);
}

function writeJsonDurable(file, value) {
    const exists = fs.existsSync(file);
    const stat = exists ? fs.statSync(file) : null;
    replaceFile(file, Buffer.from(`${JSON.stringify(value, null, 2)}\n`), {
        mode: 0o600,
        uid: stat ? stat.uid : process.getuid(),
        gid: stat ? stat.gid : process.getgid(),
    });
}

function readCandidate(candidate) {
    const absolute = path.resolve(String(candidate || ''));
    const stat = fs.lstatSync(absolute);
    if (stat.isSymbolicLink()) refuse(`candidate permit ${absolute} must not be a symlink`, 'candidate_invalid');
    if (!stat.isFile()) refuse(`candidate permit ${absolute} must be a regular file`, 'candidate_invalid');
    if (stat.uid !== process.getuid()) {
        refuse(`candidate permit ${absolute} must be owned by the service user`, 'candidate_invalid');
    }
    if ((stat.mode & 0o7777) !== 0o600) {
        refuse(`candidate permit ${absolute} must have mode 0600 (found ${(stat.mode & 0o7777).toString(8)})`,
            'candidate_invalid');
    }
    const fd = fs.openSync(absolute, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW);
    try {
        const opened = fs.fstatSync(fd);
        if (opened.ino !== stat.ino || opened.dev !== stat.dev) refuse('candidate permit changed while reading', 'candidate_invalid');
        return { absolute, bytes: fs.readFileSync(fd) };
    } finally { fs.closeSync(fd); }
}

function replaceHashOnce(bytes, oldHash, newHash, label) {
    const text = bytes.toString('utf8');
    const count = text.split(oldHash).length - 1;
    if (count !== 1) refuse(`${label} must contain the hash ${oldHash} exactly once (found ${count})`);
    return Buffer.from(text.replace(oldHash, newHash), 'utf8');
}

function findPermitPin(config, permitOption) {
    const pins = config.pinned_files.map((pin, index) => ({ index, path: path.resolve(String(pin.path)) }));
    const matches = permitOption
        ? pins.filter((pin) => pin.path === path.resolve(permitOption))
        : pins.filter((pin) => path.basename(pin.path) === PERMIT_BASENAME);
    if (matches.length !== 1) {
        refuse(`driver config must pin exactly one permit ${permitOption || PERMIT_BASENAME} (found ${matches.length}); use --permit <path>`);
    }
    return matches[0];
}

// The permit's content rules belong to its consumer. The validator must be a
// hash-pinned artifact of the current driver config so arbitrary code is never trusted.
function runValidator(config, options, candidatePath) {
    if (!options.validator) refuse('--validator <program> is required: the permit consumer\'s own validator');
    const pinned = new Map(config.pinned_files.map((pin) => [path.resolve(String(pin.path)), String(pin.sha256)]));
    for (const row of [config.readiness, ...config.stages]) pinned.set(path.resolve(row.program), String(row.program_sha256));
    const program = path.resolve(options.validator);
    const values = { permit_file: candidatePath, route_id: driver.ROUTE_ID, source_chain_id: String(driver.SOURCE_CHAIN_ID) };
    const args = (options.validatorArgs.length ? options.validatorArgs : ['{permit_file}'])
        .map((arg) => arg.replace(/\{([a-z_]+)\}/g, (_, key) => {
            if (!(key in values)) refuse(`unknown validator placeholder: ${key}`);
            return values[key];
        }));
    for (const artifact of [program, ...args.filter((arg) => path.isAbsolute(arg) && arg !== candidatePath && fs.existsSync(arg))]) {
        if (!pinned.has(artifact) || fileHash(artifact) !== pinned.get(artifact)) {
            refuse(`validator artifact ${artifact} is not hash-pinned by the driver config`);
        }
    }
    const run = spawnSync(program, args, { encoding: 'utf8', timeout: 30_000, maxBuffer: 1024 * 1024 });
    let output = null;
    try { output = JSON.parse(String(run.stdout || '').trim()); } catch (_) { /* exit status decides */ }
    if (run.error || run.status !== 0 || output?.ok === false) {
        const detail = String(run.stderr || run.error?.message || '').trim().split('\n')[0].slice(0, 200);
        refuse(`permit validator rejected the candidate: ${detail || `exit ${run.status}`}`, 'candidate_invalid');
    }
    return { program, args, exit_status: run.status, output };
}

// trustless-bridge-jobs.js reads routes from TRUSTLESS_BRIDGE_ROUTES_JSON(_FILE) and
// checks each route's pinned_files at proxy start, which may include this driver config.
function routesPlan(options, configPath, oldConfigHash, newConfigHash) {
    const env = options.env;
    const file = options.routesConfig || env.TRUSTLESS_BRIDGE_ROUTES_JSON_FILE;
    const inline = String(env.TRUSTLESS_BRIDGE_ROUTES_JSON || '').trim();
    const pinsConfig = (routes) => routes.some((route) => (route.pinned_files || [])
        .some((pin) => path.resolve(String(pin?.path || '')) === configPath));
    if (!file && inline) {
        if (pinsConfig(JSON.parse(inline))) {
            refuse('the driver config is pinned by the inline TRUSTLESS_BRIDGE_ROUTES_JSON route config, '
                + 'which this tool cannot rewrite; move it to a TRUSTLESS_BRIDGE_ROUTES_JSON_FILE');
        }
        return { source: 'TRUSTLESS_BRIDGE_ROUTES_JSON', pins_driver_config: false };
    }
    if (!file) {
        if (options.mode === 'activate' && !options.noRoutesConfig) {
            refuse('pass --routes-config <path> (the proxy\'s TRUSTLESS_BRIDGE_ROUTES_JSON_FILE) or --no-routes-config');
        }
        return null;
    }
    const routesPath = path.resolve(file);
    const bytes = fs.readFileSync(routesPath);
    const routes = JSON.parse(bytes.toString('utf8'));
    if (!Array.isArray(routes)) refuse(`routes config ${routesPath} must be a JSON array`);
    if (!pinsConfig(routes)) return { source: routesPath, pins_driver_config: false };
    for (const route of routes) {
        for (const pin of route.pinned_files || []) {
            if (path.resolve(String(pin?.path || '')) === configPath && pin.sha256 !== oldConfigHash) {
                refuse(`routes config ${routesPath} pins the driver config at ${pin.sha256}, not the installed ${oldConfigHash}`);
            }
        }
    }
    return {
        source: routesPath,
        pins_driver_config: true,
        file: { role: 'routes_config', path: routesPath, previous: bytes, next: replaceHashOnce(bytes, oldConfigHash, newConfigHash, `routes config ${routesPath}`) },
    };
}

// eth-fast-lane-driver.js runJob snapshots the driver config per job and re-checks the
// snapshot's pins on every run, so a live job pinned to the old permit would stall.
function inflightJobs(jobRoot, permitPath, oldPermitHash) {
    if (!jobRoot) return null;
    const jobsDir = path.join(path.resolve(jobRoot), 'jobs');
    if (!fs.existsSync(jobsDir)) return [];
    const affected = [];
    for (const name of fs.readdirSync(jobsDir)) {
        const snapshot = path.join(jobsDir, name, 'driver-config.snapshot.json');
        if (!fs.existsSync(snapshot)) continue;
        let status = null;
        try { status = JSON.parse(fs.readFileSync(path.join(jobsDir, name, 'worker-state.json'), 'utf8')).status; } catch (_) { /* non-terminal */ }
        if (TERMINAL_JOB_STATES.has(status)) continue;
        const pins = JSON.parse(fs.readFileSync(snapshot, 'utf8')).pinned_files || [];
        if (pins.some((pin) => path.resolve(String(pin.path)) === permitPath && pin.sha256 === oldPermitHash)) {
            affected.push({ job_id: `0x${name}`, status: status || 'unknown' });
        }
    }
    return affected;
}

function pendingMarkerPath(configPath) {
    return path.join(path.dirname(configPath), PENDING_MARKER);
}

function assertNoPending(configPath) {
    const marker = pendingMarkerPath(configPath);
    if (!fs.existsSync(marker)) return;
    let recordPath = '<unreadable>';
    let states = [];
    try {
        recordPath = JSON.parse(fs.readFileSync(marker, 'utf8')).record;
        const record = JSON.parse(fs.readFileSync(recordPath, 'utf8'));
        states = record.files.map((file) => {
            const current = fs.existsSync(file.path) ? fileHash(file.path) : null;
            const state = current === file.previous_sha256 ? 'previous'
                : current === file.new_sha256 ? 'new' : 'unknown';
            return `${file.role}=${state}`;
        });
    } catch (_) { /* report what is known */ }
    refuse(`an activation did not finish (${states.join(', ') || 'state unknown'}); `
        + `run --rollback --record ${recordPath} first`, 'activation_pending');
}

function plan(options) {
    const configPath = path.resolve(String(options.driverConfig || ''));
    assertNoPending(configPath);
    try { driver.validateConfig(configPath); } catch (error) {
        refuse(`the installed driver config fails its own pin checks (${error.message}); fix that first`, 'driver_config_invalid');
    }
    const configBytes = fs.readFileSync(configPath);
    const config = JSON.parse(configBytes.toString('utf8'));
    const pin = findPermitPin(config, options.permit);
    const permitBytes = fs.readFileSync(pin.path);
    const candidate = readCandidate(options.candidate);
    if (candidate.absolute === pin.path) refuse('the candidate must be a separate file, not the installed permit');
    const previousPermitHash = sha256(permitBytes);
    const candidateHash = sha256(candidate.bytes);
    if (candidateHash === previousPermitHash) refuse('the candidate is identical to the installed permit');
    const validator = runValidator(config, options, candidate.absolute);
    const newConfigBytes = replaceHashOnce(configBytes, previousPermitHash, candidateHash, 'driver config');
    const expected = JSON.parse(configBytes.toString('utf8'));
    expected.pinned_files[pin.index].sha256 = candidateHash;
    if (JSON.stringify(expected) !== JSON.stringify(JSON.parse(newConfigBytes.toString('utf8')))) {
        refuse('rewriting the permit pin would change more than that pin');
    }
    const routes = routesPlan(options, configPath, sha256(configBytes), sha256(newConfigBytes));
    const jobs = inflightJobs(options.jobRoot || options.env.TRUSTLESS_BRIDGE_JOB_ROOT, pin.path, previousPermitHash);
    const files = [
        { role: 'permit', path: pin.path, previous: permitBytes, next: candidate.bytes },
        { role: 'driver_config', path: configPath, previous: configBytes, next: newConfigBytes },
    ];
    if (routes?.file) files.push(routes.file);
    return {
        configPath,
        candidate: candidate.absolute,
        files,
        summary: {
            driver_config: configPath,
            permit_path: pin.path,
            candidate: candidate.absolute,
            previous_permit_sha256: previousPermitHash,
            candidate_permit_sha256: candidateHash,
            previous_config_sha256: sha256(configBytes),
            new_config_sha256: sha256(newConfigBytes),
            routes_config: routes ? { source: routes.source, pins_driver_config: routes.pins_driver_config } : 'not checked',
            inflight_jobs_on_previous_permit: jobs ?? 'not checked',
            validator: { program: validator.program, args: validator.args, exit_status: validator.exit_status },
        },
    };
}

function restore(record, recordPath) {
    const results = [];
    for (const file of [...record.files].reverse()) {
        const backup = fs.readFileSync(file.backup);
        if (sha256(backup) !== file.previous_sha256) refuse(`backup ${file.backup} does not match its recorded hash`, 'restore_failed');
        const current = fs.existsSync(file.path) ? fileHash(file.path) : null;
        if (current !== file.previous_sha256) {
            if (current !== null && current !== file.new_sha256) {
                refuse(`${file.path} changed outside this activation (sha256 ${current}); not overwriting`, 'restore_failed');
            }
            replaceFile(file.path, backup, file);
        }
        const restored = fileHash(file.path);
        if (restored !== file.previous_sha256) refuse(`${file.path} did not restore to ${file.previous_sha256}`, 'restore_failed');
        results.push({ role: file.role, path: file.path, sha256: restored, verified: true });
    }
    const marker = pendingMarkerPath(record.driver_config);
    try {
        if (JSON.parse(fs.readFileSync(marker, 'utf8')).record === recordPath) fs.rmSync(marker);
    } catch (_) { /* no marker for this record */ }
    return results;
}

async function activate(options) {
    const planned = plan({ ...options, mode: 'activate' });
    if (Array.isArray(planned.summary.inflight_jobs_on_previous_permit)
        && planned.summary.inflight_jobs_on_previous_permit.length && !options.allowInflightJobs) {
        refuse(`${planned.summary.inflight_jobs_on_previous_permit.length} unfinished bridge job(s) pin the previous permit `
            + 'and would stall; wait for them or pass --allow-inflight-jobs');
    }
    const recordPath = path.resolve(String(options.record || ''));
    if (fs.existsSync(recordPath)) refuse(`record ${recordPath} already exists; use a new record path`);
    const backupDir = `${recordPath}.backups`;
    fs.mkdirSync(backupDir, { mode: 0o700 });
    const record = {
        schema: RECORD_SCHEMA,
        state: 'prepared',
        started_at: nowIso(),
        ...planned.summary,
        backup_dir: backupDir,
        files: planned.files.map((file, index) => {
            const stat = fs.statSync(file.path);
            const backup = path.join(backupDir, `${index}-${file.role}-${path.basename(file.path)}`);
            replaceFile(backup, file.previous, { mode: 0o600, uid: process.getuid(), gid: process.getgid() });
            return {
                role: file.role, path: file.path, backup,
                mode: stat.mode & 0o7777, uid: stat.uid, gid: stat.gid,
                previous_sha256: sha256(file.previous), new_sha256: sha256(file.next),
            };
        }),
    };
    writeJsonDurable(recordPath, record);
    writeJsonDurable(pendingMarkerPath(planned.configPath), { record: recordPath, started_at: record.started_at });
    try {
        planned.files.forEach((file, index) => {
            replaceFile(file.path, file.next, record.files[index]);
            if (index === 0 && options.env[FAULT_ENV] === 'after-permit-rename') process.kill(process.pid, 'SIGKILL');
        });
        record.state = 'installed';
        record.installed_at = nowIso();
        writeJsonDurable(recordPath, record);
        // Fresh: the driver's own readiness path re-runs loadConfig and the probe; no cache.
        try {
            record.readiness = { ok: true, checked_at: nowIso(), result: await driver.readiness(planned.configPath, driver.ROUTE_ID) };
        } catch (error) {
            record.readiness = { ok: false, checked_at: nowIso(), code: String(error.code || 'readiness_failed'), message: error.message };
            throw error;
        }
        record.state = 'activated';
        record.finished_at = nowIso();
        writeJsonDurable(recordPath, record);
        fs.rmSync(pendingMarkerPath(planned.configPath));
        return record;
    } catch (error) {
        record.failure = { code: String(error.code || 'activation_failed'), message: error.message };
        try {
            record.restoration = restore(record, recordPath);
            record.state = 'rolled_back';
        } catch (restoreError) {
            record.state = 'restore_failed';
            record.restore_error = restoreError.message;
        }
        record.finished_at = nowIso();
        writeJsonDurable(recordPath, record);
        throw Object.assign(new Error(`activation failed and was ${record.state === 'rolled_back'
            ? 'rolled back (verified by hash)' : 'NOT fully restored'}: ${error.message}`), {
            code: record.state === 'rolled_back' ? 'activation_rolled_back' : 'restore_failed',
        });
    }
}

function rollback(recordPath) {
    const absolute = path.resolve(String(recordPath || ''));
    const record = JSON.parse(fs.readFileSync(absolute, 'utf8'));
    if (record.schema !== RECORD_SCHEMA || !Array.isArray(record.files)) refuse(`${absolute} is not an activation record`);
    record.restoration = restore(record, absolute);
    record.state = 'rolled_back';
    record.rolled_back_at = nowIso();
    writeJsonDurable(absolute, record);
    let configValid = true;
    try { driver.validateConfig(record.driver_config); } catch (_) { configValid = false; }
    return { ok: true, mode: 'rollback', record: absolute, restoration: record.restoration, driver_config_pins_valid: configValid };
}

function parseArgs(argv) {
    const options = { validatorArgs: [], env: process.env };
    const takes = {
        '--driver-config': 'driverConfig', '--candidate': 'candidate', '--record': 'record',
        '--permit': 'permit', '--validator': 'validator', '--routes-config': 'routesConfig', '--job-root': 'jobRoot',
    };
    const flags = { '--no-routes-config': 'noRoutesConfig', '--allow-inflight-jobs': 'allowInflightJobs' };
    const modes = [];
    for (let index = 0; index < argv.length; index += 1) {
        const arg = argv[index];
        if (['--check', '--activate', '--rollback'].includes(arg)) modes.push(arg.slice(2));
        else if (arg === '--validator-arg') options.validatorArgs.push(String(argv[++index] ?? ''));
        else if (takes[arg] && argv[index + 1] !== undefined) options[takes[arg]] = argv[++index];
        else if (flags[arg]) options[flags[arg]] = true;
        else refuse(`unknown or incomplete argument: ${arg}`, 'usage');
    }
    if (modes.length !== 1) refuse('choose exactly one of --check, --activate, --rollback', 'usage');
    options.mode = modes[0];
    const required = { check: ['driverConfig', 'candidate'], activate: ['driverConfig', 'candidate', 'record'], rollback: ['record'] };
    for (const key of required[options.mode]) if (!options[key]) refuse(`--${options.mode} requires ${key}`, 'usage');
    return options;
}

const EXIT = { usage: 64, activation_rolled_back: 2, restore_failed: 3 };

async function cli(argv = process.argv.slice(2)) {
    try {
        const options = parseArgs(argv);
        let output;
        if (options.mode === 'check') output = { ok: true, mode: 'check', changed: false, ...plan(options).summary };
        else if (options.mode === 'activate') {
            const record = await activate(options);
            output = { ok: true, mode: 'activate', record: path.resolve(options.record), state: record.state, readiness: record.readiness };
        } else output = rollback(options.record);
        process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
        return 0;
    } catch (error) {
        process.stdout.write(`${JSON.stringify({ ok: false, code: error.code || 'error', message: error.message })}\n`);
        return EXIT[error.code] || 1;
    }
}

if (require.main === module) cli().then((code) => { process.exitCode = code; });

module.exports = { PENDING_MARKER, RECORD_SCHEMA, activate, cli, plan, rollback };
