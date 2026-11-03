'use strict';

// W-1 S3.3 against the real node binary: two `rpc-serve
// --allow-private-action-admission` validators and the real proxy. On the
// default path a submission reaches the validator-owned admission
// (`private_action_submit`) and the legacy routes answer 410 even with a
// validator key file configured; only with the switch on do they run.
// Skipped when no postfiat-node binary is present (POSTFIAT_NODE_BIN or target/).

const assert = require('assert');
const fs = require('fs');
const http = require('http');
const net = require('net');
const os = require('os');
const path = require('path');
const { execFileSync, spawn } = require('child_process');

const { LEGACY_HTTP, LEGACY_ROUTES, RETIRED_CODE, VALIDATOR_OWNED } = require('./legacy-shielded-certification');
const { intentIdFor } = require('./private-actions');

const REPO = path.join(__dirname, '..');
const NODE_BIN = [process.env.POSTFIAT_NODE_BIN, path.join(REPO, 'target', 'release', 'postfiat-node'),
    path.join(REPO, 'target', 'debug', 'postfiat-node')].find((file) => file && fs.existsSync(file));
const vectors = JSON.parse(fs.readFileSync(path.join(REPO, 'crates', 'node', 'tests', 'fixtures', 'private-actions-v1', 'vectors.json'), 'utf8'));
const WALLET = 'pf0123456789abcdef0123456789abcdef01234567';
const TOKEN = 'legacy-switch-binary-token-32-bytes-min-01';
const ORIGIN = 'https://wallet.example.test';

function freePort() {
    return new Promise((resolve) => {
        const server = net.createServer().listen(0, '127.0.0.1', () => {
            const { port } = server.address();
            server.close(() => resolve(port));
        });
    });
}

function request(port, method, pathname, body) {
    return new Promise((resolve, reject) => {
        const payload = body === undefined ? '' : JSON.stringify(body);
        const req = http.request({
            host: '127.0.0.1', port, path: pathname, method,
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

// One newline-framed call to a node's rpc-serve.
function nodeRpc(port, method, params) {
    return new Promise((resolve, reject) => {
        const socket = net.connect(port, '127.0.0.1');
        let buffer = '';
        socket.setEncoding('utf8');
        socket.on('connect', () => socket.write(`${JSON.stringify({ version: 'postfiat-local-rpc-v1', id: method, method, params })}\n`));
        socket.on('data', (chunk) => {
            buffer += chunk;
            const index = buffer.indexOf('\n');
            if (index >= 0) { socket.end(); resolve(JSON.parse(buffer.slice(0, index))); }
        });
        socket.on('error', reject);
    });
}

async function waitFor(check, label, timeoutMs = 20_000) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
        if (await check()) return;
        await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error(`timed out waiting for ${label}`);
}

const listening = (port) => new Promise((resolve) => {
    const probe = net.connect(port, '127.0.0.1');
    probe.on('connect', () => { probe.end(); resolve(true); });
    probe.on('error', () => resolve(false));
});

async function main() {
    if (!NODE_BIN) {
        console.log('SKIP legacy shielded certification node-binary test: no postfiat-node binary');
        return;
    }
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-legacy-switch-binary-'));
    const children = [];
    let admitted = '';
    try {
        // Two validators of one chain, each serving private_action_submit behind its switch.
        const rpcPorts = [await freePort(), await freePort()];
        let domain;
        for (const index of [0, 1]) {
            const init = JSON.parse(execFileSync(NODE_BIN, ['init', '--data-dir', path.join(root, `v${index}`),
                '--chain-id', 'postfiat-local', '--node-id', `validator-${index}`, '--validators', '2'], { encoding: 'utf8' }));
            domain = { chain_id: init.chain_id, genesis_hash: init.genesis_hash, protocol_version: init.protocol_version };
        }
        execFileSync(NODE_BIN, ['topology', '--validators', '2', '--base-port', String(await freePort()),
            '--output', path.join(root, 'topology.json')], { stdio: 'ignore' });
        for (const index of [0, 1]) {
            const dir = path.join(root, `v${index}`);
            const keys = JSON.parse(fs.readFileSync(path.join(dir, 'validator_keys.json'), 'utf8').split('\npftmac1:')[0]);
            const record = keys.validators.find((entry) => entry.node_id === `validator-${index}`);
            fs.writeFileSync(path.join(root, `v${index}.key.json`), `${JSON.stringify({ validators: [record] })}\n`, { mode: 0o600 });
            const child = spawn(NODE_BIN, ['rpc-serve', '--unsafe-devnet-json-storage', '--data-dir', dir,
                '--spool-dir', path.join(root, `spool${index}`), '--ready-file', path.join(root, `ready${index}.json`),
                '--port', String(rpcPorts[index]), '--allow-private-action-admission',
                '--finality-topology', path.join(root, 'topology.json'), '--finality-key-file', path.join(root, `v${index}.key.json`),
                '--finality-artifact-root', path.join(root, `finality${index}`), '--keep-alive'], { stdio: 'ignore' });
            children.push(child);
            await waitFor(() => fs.existsSync(path.join(root, `ready${index}.json`)), `validator-${index} rpc-serve`);
        }

        // The real proxy; a validator key file and data directory are configured for the
        // legacy ingress in both runs, so only the switch decides whether they are used.
        const proxyPort = await freePort();
        const baseEnv = {
            PATH: process.env.PATH, HOME: root, LISTEN_HOST: '127.0.0.1', LISTEN_PORT: String(proxyPort),
            NAVSWAP_IDEMPOTENCY_STORE_PATH: 'off', NAVSWAP_RUN_STORE_PATH: 'off', ENABLE_UPSTREAM_KEEPALIVE: 'false',
            WALLET_PROXY_API_TOKEN: TOKEN, ALLOWED_ORIGINS: ORIGIN,
            NAVSWAP_ENABLE_SHIELDED_INGRESS: 'true',
            NAVSWAP_SHIELDED_INGRESS_KEY_FILE: path.join(root, 'v0.key.json'),
            NAVSWAP_SHIELDED_INGRESS_DATA_DIR: path.join(root, 'v0'),
            NAVSWAP_SHIELDED_INGRESS_TOPOLOGY: path.join(root, 'topology.json'),
            WALLET_PROXY_PRIVATE_ACTIONS_CHAIN_ID: domain.chain_id,
            WALLET_PROXY_PRIVATE_ACTIONS_GENESIS_HASH: domain.genesis_hash,
            WALLET_PROXY_PRIVATE_ACTIONS_RPC_SOURCES: rpcPorts.map((port, index) => `validator-${index}=127.0.0.1:${port}`).join(','),
            WALLET_PROXY_PRIVATE_ACTIONS_STORE: path.join(root, 'gateway-store'),
        };
        const startProxy = async (env) => {
            const child = spawn(process.execPath, ['server.js'], { cwd: __dirname, env, stdio: ['ignore', 'pipe', 'pipe'] });
            let output = '';
            child.stdout.on('data', (chunk) => { output += chunk; });
            child.stderr.on('data', (chunk) => { output += chunk; });
            children.push(child);
            await waitFor(() => listening(proxyPort), 'the proxy');
            return { child, output: () => output };
        };
        const stopProxy = (proxy) => new Promise((resolve) => { proxy.child.once('exit', resolve); proxy.child.kill('SIGKILL'); });

        // Default (switch unset): the validator-owned path.
        const defaultProxy = await startProxy(baseEnv);
        assert.match(defaultProxy.output(), /Legacy shielded certification: off; submissions take the validator-owned admission/);
        const status = await request(proxyPort, 'GET', '/api/shielded-nav-swap/status');
        assert.strictEqual(status.body.certification_path.active, VALIDATOR_OWNED);
        assert.deepStrictEqual(status.body.ingress.missing, ['WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION=on'],
            'the configured validator key file is not taken on the default path');
        for (const route of LEGACY_ROUTES) {
            const refused = await request(proxyPort, 'POST', route, {});
            assert.deepStrictEqual([refused.statusCode, refused.body.code], [410, RETIRED_CODE], route);
        }
        // The swap vector carries no chain domain of its own, so it binds to this chain's.
        const vector = vectors.positive.find((entry) => entry.kind === 'shielded_swap_v1');
        const intentId = intentIdFor(domain, vector.action_json);
        const submitted = await request(proxyPort, 'POST', '/api/private/v1/submit',
            { address: WALLET, intent_id: intentId, action_json: vector.action_json });
        assert.ok([200, 202].includes(submitted.statusCode), JSON.stringify(submitted.body));
        // Both validators' admission answered: a held record, or the node's own refusal of this
        // vector on a fresh chain (for example shielded_atomic_batch_not_active), never a
        // transport error. No HTTP-side certification ran.
        const record = JSON.parse(fs.readFileSync(path.join(root, 'gateway-store', 'intents', `${intentId}.json`), 'utf8'));
        const answers = record.last_dispatch.sources;
        assert.deepStrictEqual(answers.map((answer) => answer.source), ['validator-0', 'validator-1'], JSON.stringify(record));
        assert.ok(answers.every((answer) => answer.ok === true || (/^[a-z][a-z0-9_]+$/.test(answer.code) && !/^rpc_/.test(answer.code))), JSON.stringify(answers));
        admitted = answers.map((answer) => `${answer.source}: ${answer.ok ? 'held' : answer.code}`).join(', ');
        assert.ok(!fs.existsSync(path.join(root, '.local', 'share', 'postfiat', 'wallet-proxy', 'shielded-ingress')),
            'no legacy ingress artifact directory');
        await stopProxy(defaultProxy);

        // Switch on: the legacy route runs its execution again (no 410) and readiness names it.
        const legacyProxy = await startProxy({ ...baseEnv, WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION: 'on' });
        const legacyStatus = await request(proxyPort, 'GET', '/api/shielded-nav-swap/status');
        assert.strictEqual(legacyStatus.body.certification_path.active, LEGACY_HTTP);
        const legacy = await request(proxyPort, 'POST', '/api/shielded-nav-swap/ingress', {});
        assert.notStrictEqual(legacy.statusCode, 410, JSON.stringify(legacy.body));
        assert.notStrictEqual(legacy.body.code, RETIRED_CODE);
        await stopProxy(legacyProxy);
        console.log(`legacy shielded certification node-binary test passed (${path.relative(REPO, NODE_BIN)}; ${admitted})`);
    } finally {
        for (const child of children) if (child.exitCode === null) child.kill('SIGKILL');
        fs.rmSync(root, { recursive: true, force: true });
    }
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
