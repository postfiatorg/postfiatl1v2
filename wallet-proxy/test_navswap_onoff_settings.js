'use strict';
// These cases cover the legacy HTTP-side certification, kept behind its switch for one release (W-1 S3.3).
process.env.WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION = 'on';

const assert = require('node:assert/strict');
const childProcess = require('node:child_process');
const { EventEmitter } = require('node:events');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

// The certifier loop start is exercised through its exported path; the child process is
// recorded instead of started.
const spawnedEnvs = [];
childProcess.spawn = (_bin, _args, options) => {
    spawnedEnvs.push(options.env);
    const child = new EventEmitter();
    child.stdout = new EventEmitter();
    child.stderr = new EventEmitter();
    return child;
};

const SETTINGS = [
    'NAVSWAP_ENABLE_UNISWAP_BETA_ROUTE',
    'NAVSWAP_ENABLE_UNISWAP_BETA_RUNS',
    'NAVSWAP_UNISWAP_ROUTE_PAUSED',
    'NAVSWAP_UNISWAP_PUBLIC_ROUTING_ENABLED',
    'NAVSWAP_ENABLE_SHIELDED_INGRESS',
    'NAVSWAP_ENABLE_SHIELDED_SWAPS',
    'NAVSWAP_SHIELDED_CERTIFIER_LOOP',
    'NAVSWAP_SHIELDED_SWAP_CERTIFIER_LOOP',
    'NAVSWAP_ENABLE_SHIELDED_EGRESS',
    'NAVSWAP_ENABLE_SHIELDED_QUOTES',
    'NAVSWAP_ENABLE_SHIELDED_NAVSWAP_QUOTES',
    'NAVSWAP_SHIELDED_ROUND_PREWARM',
    'POSTFIAT_SHIELDED_ROUND_PREWARM',
];
for (const name of [...SETTINGS, 'NAVSWAP_SHIELDED_LIQUIDITY_COMMITMENT', 'NAVSWAP_SHIELDED_POOL_NOTE_COMMITMENT']) {
    delete process.env[name];
}

const { parseBooleanEnv, parseBooleanValue } = require('./boolean-env');
const server = require('./server');

const ON = ['true', 'TRUE', 'True', '1', ' true ', ' 1 ', '\ttrue\n'];
const OFF = ['false', 'FALSE', '0', ' false ', ' 0 '];
const UNSET = [undefined, '', ' ', '\t'];
const REFUSED = ['yes', 'YES', 'enabled', 'on', 'off', 'no', 'garbage', 'true1', '01', 't'];

function withEnv(values, fn) {
    const saved = {};
    for (const [name, value] of Object.entries(values)) {
        saved[name] = Object.hasOwn(process.env, name) ? process.env[name] : undefined;
        if (value === undefined) delete process.env[name];
        else process.env[name] = value;
    }
    try {
        return fn();
    } finally {
        for (const [name, value] of Object.entries(saved)) {
            if (value === undefined) delete process.env[name];
            else process.env[name] = value;
        }
    }
}

function outcome(fn) {
    try {
        return { value: fn() };
    } catch (error) {
        return { error: error.message };
    }
}

const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'navswap-onoff-settings-'));
function certifierLoopPrewarm() {
    const before = spawnedEnvs.length;
    const loopRoot = path.join(tmpDir, 'loop');
    const state = server.startShieldedCertifierLoop({
        node_bin: path.join(tmpDir, 'postfiat-node'),
        data_dir: tmpDir,
        topology: path.join(tmpDir, 'topology.json'),
        key_file: path.join(tmpDir, 'key.json'),
        proposal_key_file: null,
        timeout_ms: 1000,
        certifier_loop: {
            enabled: true,
            start_height: '7',
            poll_ms: 250,
            batch_dir: path.join(loopRoot, 'batches'),
            artifact_root: path.join(loopRoot, 'artifacts'),
            processed_dir: path.join(loopRoot, 'processed'),
            ready_file: path.join(loopRoot, 'ready.json'),
            report_file: path.join(loopRoot, 'report.json'),
        },
    });
    assert.equal(spawnedEnvs.length, before + 1, 'the certifier loop starts one child');
    state.child.emit('exit', 0, null);
    return spawnedEnvs[spawnedEnvs.length - 1].POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER === '1';
}
const roundEnvPrewarm = () => server.shieldedCertifiedRoundEnv().POSTFIAT_PREWARM_SHIELDED_VERIFIER === '1';
const quotesEnabled = () => !server.navswapCapabilities().routes.shielded_navswap.reason
    .includes('NAVSWAP_ENABLE_SHIELDED_QUOTES=true');

// Each read, reached through an exported path, with the setting it reads and its default.
const READS = [
    { name: 'NAVSWAP_ENABLE_UNISWAP_BETA_ROUTE', fallback: false, read: () => server.navswapBridgeConfig().explicit_beta },
    {
        name: 'NAVSWAP_ENABLE_UNISWAP_BETA_RUNS',
        fallback: false,
        context: { NAVSWAP_ENABLE_UNISWAP_BETA_ROUTE: 'true' },
        read: () => server.navswapBridgeConfig().beta_runs_enabled,
    },
    { name: 'NAVSWAP_UNISWAP_ROUTE_PAUSED', fallback: false, read: () => server.navswapBridgeConfig().paused },
    {
        name: 'NAVSWAP_UNISWAP_PUBLIC_ROUTING_ENABLED',
        fallback: false,
        read: () => server.navswapBridgeConfig().public_routing_enabled,
    },
    { name: 'NAVSWAP_ENABLE_SHIELDED_INGRESS', fallback: false, read: () => server.shieldedNavswapIngressConfig().enabled },
    { name: 'NAVSWAP_ENABLE_SHIELDED_SWAPS', fallback: false, read: () => server.shieldedNavswapSwapConfig().enabled },
    {
        name: 'NAVSWAP_SHIELDED_CERTIFIER_LOOP',
        fallback: false,
        read: () => server.shieldedNavswapSwapConfig().certifier_loop.enabled,
    },
    {
        name: 'NAVSWAP_SHIELDED_SWAP_CERTIFIER_LOOP',
        fallback: false,
        read: () => server.shieldedNavswapSwapConfig().certifier_loop.enabled,
    },
    { name: 'NAVSWAP_ENABLE_SHIELDED_EGRESS', fallback: false, read: () => server.shieldedNavswapEgressConfig().enabled },
    { name: 'NAVSWAP_ENABLE_SHIELDED_SWAPS', fallback: false, read: () => server.shieldedNavswapEgressConfig().enabled },
    { name: 'NAVSWAP_ENABLE_SHIELDED_QUOTES', fallback: true, read: quotesEnabled },
    { name: 'NAVSWAP_ENABLE_SHIELDED_NAVSWAP_QUOTES', fallback: true, read: quotesEnabled },
    { name: 'NAVSWAP_SHIELDED_ROUND_PREWARM', fallback: false, read: roundEnvPrewarm },
    { name: 'POSTFIAT_SHIELDED_ROUND_PREWARM', fallback: false, read: roundEnvPrewarm },
    { name: 'NAVSWAP_SHIELDED_ROUND_PREWARM', fallback: false, read: certifierLoopPrewarm },
    { name: 'POSTFIAT_SHIELDED_ROUND_PREWARM', fallback: false, read: certifierLoopPrewarm },
];

try {
    // (e) the value-taking parser and the env form that delegates to it.
    for (const fallback of [false, true]) {
        for (const raw of UNSET) assert.equal(parseBooleanValue('FLAG', raw, fallback), fallback);
    }
    for (const raw of ON) assert.equal(parseBooleanValue('FLAG', raw, false), true);
    for (const raw of OFF) assert.equal(parseBooleanValue('FLAG', raw, true), false);
    for (const raw of REFUSED) {
        assert.throws(() => parseBooleanValue('FLAG', raw, false), { message: 'FLAG must be true or false' });
    }
    for (const raw of [...UNSET, ...ON, ...OFF, ...REFUSED]) {
        assert.deepEqual(
            withEnv({ NAVSWAP_TEST_FLAG: raw }, () => outcome(() => parseBooleanEnv('NAVSWAP_TEST_FLAG', true))),
            outcome(() => parseBooleanValue('NAVSWAP_TEST_FLAG', raw, true)),
        );
    }

    // (a) accepted spellings and the default, and (b) refused spellings, for every read.
    for (const { name, fallback, read, context = {} } of READS) {
        const at = (raw) => withEnv({ ...context, [name]: raw }, read);
        for (const raw of ON) assert.equal(at(raw), true, `${name}=${JSON.stringify(raw)}`);
        for (const raw of OFF) assert.equal(at(raw), false, `${name}=${JSON.stringify(raw)}`);
        for (const raw of UNSET) assert.equal(at(raw), fallback, `${name}=${JSON.stringify(raw)}`);
        for (const raw of REFUSED) {
            assert.throws(() => at(raw), { message: `${name} must be true or false` }, `${name}=${JSON.stringify(raw)}`);
        }
    }

    // BETA_RUNS is read only when BETA_ROUTE is on.
    for (const route of [undefined, 'false', '0']) {
        assert.equal(withEnv({
            NAVSWAP_ENABLE_UNISWAP_BETA_ROUTE: route,
            NAVSWAP_ENABLE_UNISWAP_BETA_RUNS: 'garbage',
        }, () => server.navswapBridgeConfig().beta_runs_enabled), false);
    }

    // (c) aliases: the first name with a non-empty trimmed value supplies the value.
    const ALIASES = [
        { names: ['NAVSWAP_SHIELDED_CERTIFIER_LOOP', 'NAVSWAP_SHIELDED_SWAP_CERTIFIER_LOOP'], fallback: false,
            read: () => server.shieldedNavswapSwapConfig().certifier_loop.enabled },
        { names: ['NAVSWAP_ENABLE_SHIELDED_EGRESS', 'NAVSWAP_ENABLE_SHIELDED_SWAPS'], fallback: false,
            read: () => server.shieldedNavswapEgressConfig().enabled },
        { names: ['NAVSWAP_ENABLE_SHIELDED_QUOTES', 'NAVSWAP_ENABLE_SHIELDED_NAVSWAP_QUOTES'], fallback: true,
            read: quotesEnabled },
        { names: ['NAVSWAP_SHIELDED_ROUND_PREWARM', 'POSTFIAT_SHIELDED_ROUND_PREWARM'], fallback: false,
            read: roundEnvPrewarm },
        { names: ['NAVSWAP_SHIELDED_ROUND_PREWARM', 'POSTFIAT_SHIELDED_ROUND_PREWARM'], fallback: false,
            read: certifierLoopPrewarm },
    ];
    for (const { names: [first, second], fallback, read } of ALIASES) {
        const at = (a, b) => withEnv({ [first]: a, [second]: b }, read);
        assert.equal(at(undefined, undefined), fallback, `${first}/${second} unset`);
        assert.equal(at(' ', undefined), fallback, `${first} whitespace, ${second} unset`);
        assert.equal(at('true', 'false'), true, `${first} wins`);
        assert.equal(at('false', 'true'), false, `${first} wins`);
        assert.equal(at('true', 'garbage'), true, `${second} is not read when ${first} is set`);
        for (const blank of ['', ' ', '\t']) {
            assert.equal(at(blank, 'true'), true, `${first}=${JSON.stringify(blank)} falls through`);
            assert.equal(at(blank, '0'), false, `${first}=${JSON.stringify(blank)} falls through`);
        }
        assert.throws(() => at('garbage', 'true'), { message: `${first} must be true or false` });
        assert.throws(() => at(' ', 'yes'), { message: `${second} must be true or false` });
    }
    // EGRESS falls back to SWAPS; QUOTES is on when neither alias is set.
    assert.equal(withEnv({ NAVSWAP_ENABLE_SHIELDED_SWAPS: 'true' }, () => server.shieldedNavswapEgressConfig().enabled), true);
    assert.equal(withEnv({ NAVSWAP_ENABLE_SHIELDED_QUOTES: ' ' }, quotesEnabled), true);

    // (d) the two ROUND_PREWARM reads agree on every spelling, and the round env reads the
    // object it is given.
    for (const raw of [...UNSET, ...ON, ...OFF, ...REFUSED]) {
        for (const name of ['NAVSWAP_SHIELDED_ROUND_PREWARM', 'POSTFIAT_SHIELDED_ROUND_PREWARM']) {
            const values = { [name]: raw };
            const roundEnv = withEnv(values, () => outcome(roundEnvPrewarm));
            assert.deepEqual(withEnv(values, () => outcome(certifierLoopPrewarm)), roundEnv, `${name}=${JSON.stringify(raw)}`);
            const given = raw === undefined ? {} : values;
            assert.deepEqual(
                outcome(() => server.shieldedCertifiedRoundEnv(given).POSTFIAT_PREWARM_SHIELDED_VERIFIER === '1'),
                roundEnv,
                `${name}=${JSON.stringify(raw)} passed as an env object`,
            );
        }
    }
} finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
    if (typeof server.closeUpstreamRpcConnections === 'function') server.closeUpstreamRpcConnections();
}

console.log('PASS NavSwap on/off settings parse as strict booleans');
