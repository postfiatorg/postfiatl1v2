'use strict';

const assert = require('assert');
const crypto = require('crypto');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const {
    CONFIG_SCHEMA, ROUTE_ID, SOURCE_CHAIN_ID, SOURCE_PROOF_KIND, STAGES, readiness, validateConfig,
} = require('./eth-fast-lane-driver');
const { PENDING_MARKER, RECORD_SCHEMA } = require('./ingress-permit-activate');

const TOOL = path.join(__dirname, 'ingress-permit-activate.js');
const DRIVER = path.join(__dirname, 'eth-fast-lane-driver.js');
const IDENTITY = {
    program_vkey: `0x${'6b'.repeat(32)}`,
    manifest_hash: '6c'.repeat(32),
    route_profile_hash: '6d'.repeat(48),
    asset_id: '6e'.repeat(48),
    vault_address: `0x${'11'.repeat(20)}`,
    vault_runtime_code_hash: `0x${'12'.repeat(32)}`,
    token_address: `0x${'13'.repeat(20)}`,
    token_runtime_code_hash: `0x${'14'.repeat(32)}`,
};
const fileSha256 = (file) => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const writeJson = (file, value, mode = 0o600) => {
    fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, { mode });
    fs.chmodSync(file, mode);
};

// Readiness stub: reads the pinned permit like a consumer would and counts its runs.
const READINESS_STUB = `'use strict';
const fs = require('fs');
const permit = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
fs.appendFileSync(process.argv[2] + '.readiness-runs', 'x');
if (permit.stub_readiness_fail) {
  process.stdout.write(JSON.stringify({ ok: false, retryable: false, code: 'stub_not_ready' }));
  process.exit(2);
}
process.stdout.write(JSON.stringify(Object.assign(${JSON.stringify(IDENTITY)}, {
  ok: true, ready: true, route_id: 'ethereum-mainnet-usdc-v1', source_chain_id: 1,
  source_proof_kind: 'sp1-ethereum-finality-v1', observer_attestor_enabled: false,
  prover_authenticated: true, prover_healthy: true, route_manifest_active: true,
  program_vkey_active: true, nav_cap_growth_enabled: true, vault_paused: false,
  vault_code_hash_matches: true, token_code_hash_matches: true,
  execution_rpc_sources_reachable: 2, beacon_finality_current: true,
})));
`;

// Stand-in for the permit consumer's validator (the real one lives on the operator host).
const VALIDATOR_STUB = `'use strict';
const fs = require('fs');
const fail = (why) => { process.stderr.write(why + '\\n'); process.exit(2); };
let p;
try { p = JSON.parse(fs.readFileSync(process.argv[2], 'utf8')); } catch (_) { fail('permit is not JSON'); }
if (p.schema !== 'test-ingress-authorization-v1') fail('wrong permit schema');
if (p.route_id !== 'ethereum-mainnet-usdc-v1' || p.source_chain_id !== 1) fail('wrong source or route identity');
if (!Array.isArray(p.wallets) || !p.wallets.every((w) => /^0x[0-9a-f]{40}$/.test(w))) fail('bad wallet scope');
const now = Date.now();
if (!(Date.parse(p.not_before) <= now && now < Date.parse(p.not_after))) fail('permit is outside its window');
if (!/^[1-9][0-9]*$/.test(String(p.max_amount_atoms))) fail('unbounded value authority');
process.stdout.write(JSON.stringify({ ok: true }));
`;

function permit(overrides = {}) {
    return {
        schema: 'test-ingress-authorization-v1',
        route_id: ROUTE_ID,
        source_chain_id: SOURCE_CHAIN_ID,
        wallets: [`0x${'a1'.repeat(20)}`],
        not_before: new Date(Date.now() - 3_600_000).toISOString(),
        not_after: new Date(Date.now() + 3_600_000).toISOString(),
        max_amount_atoms: '1000000',
        ...overrides,
    };
}

function fixture() {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-permit-activate-'));
    const configs = path.join(root, 'configs');
    fs.mkdirSync(configs, { mode: 0o700 });
    const f = {
        root,
        stub: path.join(root, 'readiness-stub.js'),
        validator: path.join(root, 'permit-validator.js'),
        permit: path.join(configs, 'ingress-authorization.json'),
        config: path.join(configs, 'ingress-driver.json'),
        routes: path.join(configs, 'routes.json'),
        candidate: path.join(root, 'candidate.json'),
        record: path.join(root, 'records', 'activation.json'),
    };
    fs.mkdirSync(path.dirname(f.record), { mode: 0o700 });
    fs.writeFileSync(f.stub, READINESS_STUB, { mode: 0o700 });
    fs.writeFileSync(f.validator, VALIDATOR_STUB, { mode: 0o700 });
    writeJson(f.permit, permit());
    const node = { program: process.execPath, program_sha256: fileSha256(process.execPath), timeout_ms: 5_000 };
    writeJson(f.config, {
        schema: CONFIG_SCHEMA, route_id: ROUTE_ID, source_chain_id: SOURCE_CHAIN_ID,
        source_proof_kind: SOURCE_PROOF_KIND, ...IDENTITY,
        pinned_files: [f.stub, f.validator, f.permit].map((file) => ({ path: file, sha256: fileSha256(file) })),
        readiness: { ...node, args: [f.stub, f.permit] },
        stages: STAGES.map((stage) => ({ ...node, stage, args: [f.stub, f.permit, stage] })),
    });
    writeJson(f.routes, [{ route_id: ROUTE_ID, pinned_files: [{ path: f.config, sha256: fileSha256(f.config) }] }]);
    writeJson(f.candidate, permit({ wallets: [`0x${'a1'.repeat(20)}`, `0x${'b2'.repeat(20)}`] }));
    return f;
}

function tool(f, mode, extra = [], env = {}) {
    const args = [TOOL, `--${mode}`];
    if (mode !== 'rollback') {
        args.push('--driver-config', f.config, '--candidate', f.candidate, '--routes-config', f.routes,
            '--validator', process.execPath, '--validator-arg', f.validator, '--validator-arg', '{permit_file}');
    }
    if (mode !== 'check') args.push('--record', f.record);
    const run = spawnSync(process.execPath, [...args, ...extra], {
        encoding: 'utf8', env: { ...process.env, ...env }, timeout: 30_000,
    });
    let output = null;
    try { output = JSON.parse(run.stdout); } catch (_) { /* crash case */ }
    return { ...run, output };
}

const snapshot = (f) => Object.fromEntries([f.permit, f.config, f.routes].map((file) => [file, fs.readFileSync(file)]));
function assertUnchanged(f, before) {
    for (const [file, bytes] of Object.entries(before)) assert.deepStrictEqual(fs.readFileSync(file), bytes, `${file} changed`);
    assert.strictEqual(fs.existsSync(path.join(path.dirname(f.config), PENDING_MARKER)), false);
}

async function main() {
    // Guard: the existing driver still fails closed on a pin mismatch, before running readiness.
    {
        const f = fixture();
        assert.strictEqual(validateConfig(f.config).ok, true);
        assert.strictEqual((await readiness(f.config, ROUTE_ID)).ready, true);
        const runs = fs.readFileSync(`${f.permit}.readiness-runs`, 'utf8').length;
        const swap = path.join(path.dirname(f.permit), 'swap.tmp');
        fs.copyFileSync(f.candidate, swap);
        fs.chmodSync(swap, 0o600);
        fs.renameSync(swap, f.permit);
        assert.throws(() => validateConfig(f.config), (error) => error.code === 'driver_config_invalid' && error.terminal === true);
        await assert.rejects(readiness(f.config, ROUTE_ID), (error) => error.code === 'driver_config_invalid' && error.terminal === true);
        const cli = spawnSync(process.execPath, [DRIVER, 'readiness', '--config', f.config, '--route', ROUTE_ID], { encoding: 'utf8' });
        assert.strictEqual(cli.status, 2);
        assert.match(cli.stderr, /terminal driver failure/);
        assert.strictEqual(fs.readFileSync(`${f.permit}.readiness-runs`, 'utf8').length, runs, 'readiness ran despite pin mismatch');
    }

    // --check validates and changes nothing.
    {
        const f = fixture();
        const before = snapshot(f);
        const run = tool(f, 'check');
        assert.strictEqual(run.status, 0, run.stdout);
        assert.strictEqual(run.output.changed, false);
        assert.strictEqual(run.output.candidate_permit_sha256, fileSha256(f.candidate));
        assert.strictEqual(run.output.previous_permit_sha256, before[f.permit] && fileSha256(f.permit));
        assert.deepStrictEqual(run.output.routes_config, { source: f.routes, pins_driver_config: true });
        assertUnchanged(f, before);
        assert.strictEqual(fs.existsSync(f.record), false);
    }

    // --activate installs permit, driver pin and routes pin together; readiness is fresh.
    {
        const f = fixture();
        const before = snapshot(f);
        const runs = fs.existsSync(`${f.permit}.readiness-runs`) ? 1 : 0;
        const run = tool(f, 'activate');
        assert.strictEqual(run.status, 0, run.stdout);
        assert.deepStrictEqual(fs.readFileSync(f.permit), fs.readFileSync(f.candidate));
        assert.strictEqual(fs.statSync(f.permit).mode & 0o7777, 0o600);
        assert.ok(fs.readFileSync(`${f.permit}.readiness-runs`, 'utf8').length > runs, 'readiness was not run');
        const config = JSON.parse(fs.readFileSync(f.config, 'utf8'));
        assert.strictEqual(config.pinned_files[2].sha256, fileSha256(f.candidate));
        const original = JSON.parse(before[f.config]);
        original.pinned_files[2].sha256 = fileSha256(f.candidate);
        assert.deepStrictEqual(config, original, 'only the permit pin may change');
        assert.strictEqual(JSON.parse(fs.readFileSync(f.routes, 'utf8'))[0].pinned_files[0].sha256, fileSha256(f.config));
        assert.strictEqual(validateConfig(f.config).ok, true);
        const record = JSON.parse(fs.readFileSync(f.record, 'utf8'));
        assert.strictEqual(record.schema, RECORD_SCHEMA);
        assert.strictEqual(record.state, 'activated');
        assert.strictEqual(record.readiness.ok, true);
        assert.strictEqual(record.readiness.result.ready, true);
        for (const key of ['started_at', 'installed_at', 'finished_at']) assert.ok(Date.parse(record[key]), key);
        assert.strictEqual(record.candidate_permit_sha256, fileSha256(f.candidate));
        assert.strictEqual(record.new_config_sha256, fileSha256(f.config));
        assert.notStrictEqual(record.previous_config_sha256, record.new_config_sha256);
        assert.strictEqual(record.files.length, 3);
        for (const file of record.files) {
            assert.deepStrictEqual(fs.readFileSync(file.backup), before[file.path]);
            assert.strictEqual(file.previous_sha256, crypto.createHash('sha256').update(before[file.path]).digest('hex'));
        }
        assert.strictEqual(fs.existsSync(path.join(path.dirname(f.config), PENDING_MARKER)), false);

        // --rollback restores an activated change byte for byte.
        const back = tool(f, 'rollback');
        assert.strictEqual(back.status, 0, back.stdout);
        assert.strictEqual(back.output.driver_config_pins_valid, true);
        assertUnchanged(f, before);
        assert.strictEqual(JSON.parse(fs.readFileSync(f.record, 'utf8')).state, 'rolled_back');
    }

    // Readiness failure after install restores every file byte for byte.
    {
        const f = fixture();
        writeJson(f.candidate, permit({ stub_readiness_fail: true }));
        const before = snapshot(f);
        const run = tool(f, 'activate');
        assert.strictEqual(run.status, 2, run.stdout);
        assert.strictEqual(run.output.code, 'activation_rolled_back');
        assertUnchanged(f, before);
        const record = JSON.parse(fs.readFileSync(f.record, 'utf8'));
        assert.strictEqual(record.state, 'rolled_back');
        assert.strictEqual(record.readiness.ok, false);
        assert.ok(record.restoration.every((item) => item.verified));
        assert.strictEqual(validateConfig(f.config).ok, true);
    }

    // Refusals: symlink, wrong mode, wrong owner, expired, not yet valid, unpinned validator.
    const refusals = [
        ['symlink', (f) => { const real = `${f.candidate}.real`; fs.renameSync(f.candidate, real); fs.symlinkSync(real, f.candidate); }, /symlink/],
        ['mode', (f) => fs.chmodSync(f.candidate, 0o644), /mode 0600/],
        ['owner', (f) => {
            if (process.getuid() === 0) fs.chownSync(f.candidate, 65534, 65534);
            else f.candidate = '/etc/passwd';
        }, /owned by the service user/],
        ['expired', (f) => writeJson(f.candidate, permit({ not_after: new Date(Date.now() - 1000).toISOString() })), /outside its window/],
        ['not yet valid', (f) => writeJson(f.candidate, permit({ not_before: new Date(Date.now() + 3_600_000).toISOString() })), /outside its window/],
        ['route identity', (f) => writeJson(f.candidate, permit({ source_chain_id: 8453 })), /source or route identity/],
        ['unpinned validator', (f) => { const copy = `${f.validator}.copy.js`; fs.copyFileSync(f.validator, copy); f.validator = copy; }, /not hash-pinned/],
    ];
    for (const [label, mutate, pattern] of refusals) {
        for (const mode of ['check', 'activate']) {
            const f = fixture();
            mutate(f);
            const before = snapshot(f);
            const run = tool(f, mode);
            assert.strictEqual(run.status, 1, `${label}/${mode}: ${run.stdout}`);
            assert.match(run.output.message, pattern, label);
            assertUnchanged(f, before);
            assert.strictEqual(fs.existsSync(f.record), false, label);
        }
    }

    // Death between the renames: later --check/--activate refuse until --rollback.
    {
        const f = fixture();
        const before = snapshot(f);
        const crash = tool(f, 'activate', [], { PFT_INGRESS_PERMIT_ACTIVATE_FAULT: 'after-permit-rename' });
        assert.strictEqual(crash.signal, 'SIGKILL');
        assert.deepStrictEqual(fs.readFileSync(f.permit), fs.readFileSync(f.candidate));
        assert.deepStrictEqual(fs.readFileSync(f.config), before[f.config]);
        assert.throws(() => validateConfig(f.config), (error) => error.code === 'driver_config_invalid');
        for (const mode of ['check', 'activate']) {
            const recordBefore = fs.readFileSync(f.record);
            const run = tool(f, mode, mode === 'activate' ? ['--record', `${f.record}.2`] : []);
            assert.strictEqual(run.status, 1, run.stdout);
            assert.strictEqual(run.output.code, 'activation_pending');
            assert.match(run.output.message, /permit=new, driver_config=previous, routes_config=previous/);
            assert.match(run.output.message, /--rollback --record/);
            assert.deepStrictEqual(fs.readFileSync(f.record), recordBefore);
        }
        const back = tool(f, 'rollback');
        assert.strictEqual(back.status, 0, back.stdout);
        assertUnchanged(f, before);
        assert.strictEqual(validateConfig(f.config).ok, true);
        assert.strictEqual(tool(f, 'check').status, 0);
    }

    // Unfinished jobs whose config snapshot pins the previous permit block activation.
    {
        const f = fixture();
        const jobDir = path.join(f.root, 'bridge-jobs', 'jobs', 'ab'.repeat(32));
        fs.mkdirSync(jobDir, { recursive: true });
        fs.copyFileSync(f.config, path.join(jobDir, 'driver-config.snapshot.json'));
        writeJson(path.join(jobDir, 'worker-state.json'), { status: 'proving' });
        const before = snapshot(f);
        const run = tool(f, 'activate', ['--job-root', path.join(f.root, 'bridge-jobs')]);
        assert.strictEqual(run.status, 1, run.stdout);
        assert.match(run.output.message, /1 unfinished bridge job/);
        assertUnchanged(f, before);
    }

    // A routes config that pins the driver config cannot be inline; activation needs a routes decision.
    {
        const f = fixture();
        const inline = fs.readFileSync(f.routes, 'utf8');
        const args = [TOOL, '--check', '--driver-config', f.config, '--candidate', f.candidate,
            '--validator', process.execPath, '--validator-arg', f.validator, '--validator-arg', '{permit_file}'];
        const run = spawnSync(process.execPath, args, { encoding: 'utf8', env: { ...process.env, TRUSTLESS_BRIDGE_ROUTES_JSON: inline } });
        assert.strictEqual(run.status, 1);
        assert.match(JSON.parse(run.stdout).message, /inline TRUSTLESS_BRIDGE_ROUTES_JSON/);
        const noRoutes = spawnSync(process.execPath, [...args.slice(0, 1), '--activate', ...args.slice(2), '--record', f.record],
            { encoding: 'utf8', env: { ...process.env, TRUSTLESS_BRIDGE_ROUTES_JSON: '', TRUSTLESS_BRIDGE_ROUTES_JSON_FILE: '' } });
        assert.strictEqual(noRoutes.status, 1);
        assert.match(JSON.parse(noRoutes.stdout).message, /--routes-config/);
    }

    process.stdout.write('ingress permit activation tests passed\n');
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
