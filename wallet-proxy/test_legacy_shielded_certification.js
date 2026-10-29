'use strict';

// W-1 S3.3: WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION retires the proxy's
// legacy shielded certification paths; the user-proven route works without them.

const assert = require('assert');
const fs = require('fs');
const http = require('http');
const net = require('net');
const os = require('os');
const path = require('path');
const { spawn } = require('child_process');

const {
    LEGACY_ROUTES, RETIRED_CODE, legacyShieldedCertificationEnabled,
} = require('./legacy-shielded-certification');
const { batchIdFor, parsePublicAction } = require('./private-actions');

const FIXTURES = path.join(__dirname, '..', 'crates', 'node', 'tests', 'fixtures', 'private-actions-v1');
const vectors = JSON.parse(fs.readFileSync(path.join(FIXTURES, 'vectors.json'), 'utf8'));
const DOMAIN = vectors.domain;
const WALLET = 'pf0123456789abcdef0123456789abcdef01234567';
const TOKEN = 'legacy-switch-test-token-32-bytes-min-001';
const ORIGIN = 'https://wallet.example.test';

// A minimal stand-in for the node's private_action_submit / private_action_receipt.
function stub(validatorId) {
    const records = new Map();
    const server = net.createServer((socket) => {
        let buffer = '';
        socket.on('data', (chunk) => {
            buffer += chunk;
            let index;
            while ((index = buffer.indexOf('\n')) >= 0) {
                const request = JSON.parse(buffer.slice(0, index));
                buffer = buffer.slice(index + 1);
                const p = request.params || {};
                let answer = { ok: false, error: { code: 'unknown_method' } };
                if (request.method === 'private_action_submit') {
                    const domain = { chain_id: p.chain_id, genesis_hash: p.genesis_hash, protocol_version: p.protocol_version };
                    const record = records.get(p.intent_id) || {
                        schema: 'postfiat.private_action_admission.record.v1', intent_id: p.intent_id,
                        domain: { ...domain, pool_id: p.pool_id, session_scope: p.session_scope },
                        action_hash: p.action_hash, action_kind: 'x', action_json: p.action_json,
                        batch_id: batchIdFor(domain, parsePublicAction(p.action_json).canonical),
                        batch_json: '{}', state: 'queued', received_height: 1, dispatch_count: 0, outcome: null,
                    };
                    records.set(p.intent_id, record);
                    answer = { ok: true, result: { schema: 'postfiat.private_action_admission.record.v1', existing: false, record } };
                }
                socket.write(`${JSON.stringify({ version: request.version, id: request.id, ...answer })}\n`);
            }
        });
        socket.on('error', () => {});
    });
    return { validatorId, server, records };
}

function post(port, pathname, body) {
    return new Promise((resolve, reject) => {
        const payload = JSON.stringify(body);
        const req = http.request({
            host: '127.0.0.1', port, path: pathname, method: 'POST',
            headers: {
                'content-type': 'application/json', 'content-length': Buffer.byteLength(payload),
                authorization: `Bearer ${TOKEN}`, origin: ORIGIN,
            },
        }, (res) => {
            let text = '';
            res.setEncoding('utf8');
            res.on('data', (chunk) => { text += chunk; });
            res.on('end', () => resolve({ statusCode: res.statusCode, body: text ? JSON.parse(text) : null }));
        });
        req.on('error', reject);
        req.end(payload);
    });
}

function freePort() {
    return new Promise((resolve) => {
        const server = net.createServer().listen(0, '127.0.0.1', () => {
            const { port } = server.address();
            server.close(() => resolve(port));
        });
    });
}

// Starts `node server.js` and resolves with its exit code, or, once it listens, with its
// output and the result of `whileListening(port)`.
async function startProxy(extraEnv, whileListening = null) {
    const home = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-legacy-switch-home-'));
    const port = await freePort();
    const child = spawn(process.execPath, ['server.js'], {
        cwd: __dirname,
        env: {
            PATH: process.env.PATH, HOME: home, LISTEN_HOST: '127.0.0.1', LISTEN_PORT: String(port),
            NAVSWAP_IDEMPOTENCY_STORE_PATH: 'off', NAVSWAP_RUN_STORE_PATH: 'off', ...extraEnv,
        },
        stdio: ['ignore', 'pipe', 'pipe'],
    });
    let output = '';
    child.stdout.on('data', (chunk) => { output += chunk; });
    child.stderr.on('data', (chunk) => { output += chunk; });
    return new Promise((resolve) => {
        const timer = setTimeout(() => { child.kill('SIGKILL'); resolve({ exitCode: null, output, listening: false }); }, 20_000);
        child.on('exit', (code) => { clearTimeout(timer); resolve({ exitCode: code, output, listening: false }); });
        const poll = setInterval(() => {
            const probe = net.connect(port, '127.0.0.1');
            probe.on('connect', async () => {
                probe.end();
                clearInterval(poll);
                clearTimeout(timer);
                child.removeAllListeners('exit');
                const observed = whileListening ? await whileListening(port) : null;
                child.kill('SIGKILL');
                resolve({ exitCode: null, output, listening: true, observed });
            });
            probe.on('error', () => {});
        }, 100);
        child.on('exit', () => clearInterval(poll));
    });
}

const LEGACY_READER = /(VALIDATOR_KEY_FILE|SHIELDED_INGRESS_KEY_FILE|PROPOSAL_KEY_FILE|SHIELDED_INGRESS_DATA_DIR|POSTFIAT_DATA_DIR|PFTL_DATA_DIR|DATA_DIR_TEMPLATE|validator_data_dir)/;

async function main() {
    // The switch value.
    assert.strictEqual(legacyShieldedCertificationEnabled({}), true, 'default on');
    assert.strictEqual(legacyShieldedCertificationEnabled({ WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION: 'off' }), false);
    assert.strictEqual(legacyShieldedCertificationEnabled({ WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION: 'ON' }), true);
    assert.throws(() => legacyShieldedCertificationEnabled({ WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION: 'maybe' }), /on or off/);

    // Repository scan: every wallet-proxy reader of a validator key file or data directory.
    const readers = {};
    for (const name of fs.readdirSync(__dirname).filter((file) => file.endsWith('.js') && !file.startsWith('test_'))) {
        if (LEGACY_READER.test(fs.readFileSync(path.join(__dirname, name), 'utf8'))) readers[name] = true;
    }
    assert.deepStrictEqual(Object.keys(readers).sort(), [
        // Legacy shielded certification, behind the switch: the ingress/swap/egress config
        // (validator key, proposal key, data directory, topology) ...
        'navswap-config-bridge.js',
        // ... and the laggard catch-up over SSH, called only from the three legacy executions.
        'navswap-shielded.js',
        // NAVCoin bridge relay drivers (validator_data_dir on their host): outside this switch.
        'navcoin-export-relay-driver.js',
        'navcoin-return-relay-driver.js',
    ].sort(), 'condition: wallet-proxy readers of a validator key file or data directory; expected: exactly this list');
    const bridge = fs.readFileSync(path.join(__dirname, 'navswap-config-bridge.js'), 'utf8');
    const ingressStart = bridge.indexOf('function shieldedNavswapIngressConfig()');
    assert.ok(bridge.indexOf('legacyShieldedCertificationEnabled()', ingressStart) < bridge.indexOf("presentEnv('NAVSWAP_SHIELDED_INGRESS_DATA_DIR')", ingressStart),
        'the switch is checked before the legacy config reads a key file or data directory');
    const shielded = fs.readFileSync(path.join(__dirname, 'navswap-shielded.js'), 'utf8');
    const catchUpCallers = [...shielded.matchAll(/await runShieldedLaggardCatchUp\(/g)].map((match) => {
        const before = shielded.slice(0, match.index);
        return before.slice(before.lastIndexOf('    async function ')).split('(')[0].replace('    async function ', '');
    });
    assert.deepStrictEqual(catchUpCallers, ['executeShieldedNavswapIngress', 'executeShieldedNavswapSwap', 'executeShieldedNavswapEgress']);
    assert.ok(!/KEY_FILE|DATA_DIR|validator_keys/.test(fs.readFileSync(path.join(__dirname, 'private-actions.js'), 'utf8')));

    // Real starts: off with nothing configured; default with nothing enabled; on, enabled and no key file.
    const off = await startProxy({ WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION: 'off' });
    assert.ok(off.listening, off.output);
    assert.match(off.output, /Legacy shielded certification: off/);
    const defaultStart = await startProxy({});
    assert.ok(defaultStart.listening, defaultStart.output);
    assert.match(defaultStart.output, /Legacy shielded certification: on \(routes present, not enabled\)/);
    // On, enabled and no key file: starts with one warning and answers 409 as before.
    const misconfigured = await startProxy({
        NAVSWAP_ENABLE_SHIELDED_INGRESS: 'true', NAVSWAP_SHIELDED_INGRESS_KEY_FILE: '/nonexistent/validator.key',
        NAVSWAP_SHIELDED_INGRESS_DATA_DIR: '/nonexistent/data', WALLET_PROXY_API_TOKEN: TOKEN, ALLOWED_ORIGINS: ORIGIN,
    }, (port) => post(port, '/api/shielded-nav-swap/ingress', {}));
    assert.ok(misconfigured.listening, misconfigured.output);
    assert.match(misconfigured.output,
        /Warning: legacy shielded certification is on and enabled, but the validator key file and the validator data directory is missing/);
    assert.strictEqual(misconfigured.output.split('\n').filter((line) => line.startsWith('Warning: legacy shielded')).length, 1);
    assert.strictEqual(misconfigured.observed.statusCode, 409, JSON.stringify(misconfigured.observed.body));
    // Only an invalid switch value stops startup.
    const invalid = await startProxy({ WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION: 'maybe' });
    assert.ok(!invalid.listening && invalid.exitCode !== 0 && invalid.exitCode !== null, invalid.output);
    assert.match(invalid.output, /WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION must be on or off/);
    const offIgnoresThem = await startProxy({
        WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION: 'off', NAVSWAP_ENABLE_SHIELDED_INGRESS: 'true',
        NAVSWAP_SHIELDED_INGRESS_KEY_FILE: '/nonexistent/validator.key', NAVSWAP_SHIELDED_INGRESS_DATA_DIR: '/nonexistent/data',
    });
    assert.ok(offIgnoresThem.listening, offIgnoresThem.output);

    // In-process with the switch off: legacy routes answer 410, nothing legacy is configured,
    // and the user-proven route admits a vector through two validator sources.
    const a = stub('validator-a');
    const b = stub('validator-b');
    await Promise.all([a, b].map((s) => new Promise((resolve) => s.server.listen(0, '127.0.0.1', resolve))));
    delete process.env.WALLET_PROXY_API_TOKENS_JSON;
    delete process.env.LISTEN_HOST;
    Object.assign(process.env, {
        WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION: 'off',
        NAVSWAP_ENABLE_SHIELDED_INGRESS: 'true',
        NAVSWAP_ENABLE_SHIELDED_SWAPS: 'true',
        NAVSWAP_SHIELDED_CERTIFIER_LOOP: 'true',
        NAVSWAP_SHIELDED_INGRESS_KEY_FILE: '/nonexistent/validator.key',
        NAVSWAP_SHIELDED_INGRESS_DATA_DIR: '/nonexistent/data',
        WALLET_PROXY_API_TOKEN: TOKEN,
        ALLOWED_ORIGINS: ORIGIN,
        NAVSWAP_IDEMPOTENCY_STORE_PATH: 'off',
        NAVSWAP_RUN_STORE_PATH: 'off',
        ENABLE_UPSTREAM_KEEPALIVE: 'false',
        WALLET_PROXY_PRIVATE_ACTIONS_CHAIN_ID: DOMAIN.chain_id,
        WALLET_PROXY_PRIVATE_ACTIONS_GENESIS_HASH: DOMAIN.genesis_hash,
        WALLET_PROXY_PRIVATE_ACTIONS_RPC_SOURCES: [a, b].map((s) => `${s.validatorId}=127.0.0.1:${s.server.address().port}`).join(','),
        WALLET_PROXY_PRIVATE_ACTIONS_STORE: fs.mkdtempSync(path.join(os.tmpdir(), 'pft-legacy-switch-store-')),
    });
    const proxy = require('./server');
    const ingress = proxy.shieldedNavswapIngressConfig();
    assert.deepStrictEqual([ingress.key_file, ingress.data_dir, ingress.topology, ingress.configured], [null, null, null, false]);
    const swap = proxy.shieldedNavswapSwapConfig();
    assert.strictEqual(swap.certifier_loop.enabled, false);
    await new Promise((resolve) => proxy.server.listen(0, '127.0.0.1', resolve));
    const port = proxy.server.address().port;
    try {
        for (const route of LEGACY_ROUTES) {
            const response = await post(port, route, {});
            assert.strictEqual(response.statusCode, 410, route);
            assert.strictEqual(response.body.code, RETIRED_CODE);
            assert.strictEqual(response.body.replacement, '/api/private/v1/');
        }
        const vector = vectors.positive.find((entry) => entry.kind === 'asset_orchard_ingress_v2');
        const body = { address: WALLET, intent_id: vector.intent_id, action_json: vector.action_json };
        const submitted = await post(port, '/api/private/v1/submit', body);
        assert.strictEqual(submitted.statusCode, 202, JSON.stringify(submitted.body));
        assert.strictEqual(submitted.body.state, 'queued');
        assert.ok(a.records.has(vector.intent_id) && b.records.has(vector.intent_id));
        console.log('legacy shielded certification switch regression passed');
    } finally {
        await new Promise((resolve) => proxy.server.close(resolve));
        for (const s of [a, b]) s.server.close();
    }
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
