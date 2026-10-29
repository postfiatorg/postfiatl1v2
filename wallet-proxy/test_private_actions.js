'use strict';

// W-1 segment S2: the gateway's private action routes against two stub
// validators implementing private_action_submit / private_action_receipt.

const assert = require('assert');
const fs = require('fs');
const http = require('http');
const net = require('net');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const FIXTURES = path.join(__dirname, '..', 'crates', 'node', 'tests', 'fixtures', 'private-actions-v1');
const vectors = JSON.parse(fs.readFileSync(path.join(FIXTURES, 'vectors.json'), 'utf8'));
const assets = JSON.parse(fs.readFileSync(path.join(FIXTURES, 'assets.json'), 'utf8')).assets;
const schema = JSON.parse(fs.readFileSync(path.join(FIXTURES, 'schema.json'), 'utf8'));
const { batchIdFor, createPrivateActions, intentIdFor, parsePublicAction } = require('./private-actions');

const DOMAIN = vectors.domain;
const WALLET = 'pf0123456789abcdef0123456789abcdef01234567';
const ALPHA = 'alpha-private-actions-token-32-bytes-01';
const BETA = 'beta-private-actions-token-32-bytes-0002';
const ORIGIN = 'https://wallet.example.test';
const A666 = assets.A666.asset_id.value;
const PFUSDC = assets.pfUSDC.asset_id.value;
const positive = Object.fromEntries(vectors.positive.map((vector) => [vector.kind, vector]));
const negative = Object.fromEntries(vectors.negative.map((vector) => [vector.name, vector]));
const h96 = (byte) => byte.repeat(48);

function stub(validatorId) {
    const state = { submits: [], receipts: new Map(), height: 1161, sockets: new Set() };
    const answer = (request) => {
        const p = request.params || {};
        if (request.method === 'private_action_submit') {
            state.submits.push(p);
            const parsed = parsePublicAction(p.action_json);
            return { ok: true, result: { intent_id: p.intent_id, action_hash: p.action_hash, batch_id: batchIdFor(p.domain, parsed.canonical), state: 'queued' } };
        }
        if (request.method === 'private_action_receipt') {
            return { ok: true, result: state.receipts.get(p.intent_id) || { state: 'pending_unknown', intent_id: p.intent_id } };
        }
        if (request.method === 'server_info') {
            return { ok: true, result: { ...DOMAIN, storage: { finalized_height: state.height, finalized_block_hash: h96('a4'), finalized_state_root: h96('f3') } } };
        }
        if (request.method === 'asset_info') {
            const row = p.asset_id === A666 ? assets.A666 : assets.pfUSDC;
            return { ok: true, result: { found: true, asset: Object.fromEntries(['asset_id', 'issuer', 'code', 'precision', 'requires_authorization'].map((key) => [key, row[key].value])) } };
        }
        if (request.method === 'nav_reserve_proof_status') {
            const row = p.asset_id === A666 ? assets.A666 : assets.pfUSDC;
            return { ok: true, result: { active_profile: { profile_id: row.proof_profile_id.value } } };
        }
        return { ok: false, error: { code: 'unknown_method' } };
    };
    const server = net.createServer((socket) => {
        state.sockets.add(socket);
        socket.on('close', () => state.sockets.delete(socket));
        let buffer = '';
        socket.on('data', (chunk) => {
            buffer += chunk.toString('utf8');
            let index;
            while ((index = buffer.indexOf('\n')) >= 0) {
                const line = buffer.slice(0, index);
                buffer = buffer.slice(index + 1);
                const request = JSON.parse(line);
                socket.write(`${JSON.stringify({ version: request.version, id: request.id, ...answer(request) })}\n`);
            }
        });
        socket.on('error', () => {});
    });
    return { validatorId, state, server };
}

function oneShot(source, request) {
    return new Promise((resolve, reject) => {
        const socket = net.connect(source.port, source.host);
        let buffer = '';
        socket.on('connect', () => socket.write(`${JSON.stringify(request)}\n`));
        socket.on('data', (chunk) => {
            buffer += chunk;
            const index = buffer.indexOf('\n');
            if (index >= 0) { socket.end(); resolve(JSON.parse(buffer.slice(0, index))); }
        });
        socket.on('error', reject);
    });
}

function post(port, route, body, { token = ALPHA, origin = ORIGIN, raw = null } = {}) {
    return new Promise((resolve, reject) => {
        const payload = raw ?? JSON.stringify(body);
        const req = http.request({
            host: '127.0.0.1', port, path: `/api/private/v1/${route}`, method: 'POST',
            headers: {
                'content-type': 'application/json', 'content-length': Buffer.byteLength(payload),
                ...(token ? { authorization: `Bearer ${token}` } : {}), ...(origin ? { origin } : {}),
            },
        }, (res) => {
            let text = '';
            res.setEncoding('utf8');
            res.on('data', (chunk) => { text += chunk; });
            res.on('end', () => resolve({ statusCode: res.statusCode, headers: res.headers, body: text ? JSON.parse(text) : null }));
        });
        req.on('error', reject);
        req.end(payload);
    });
}

const body = (vector, overrides = {}) => ({ address: WALLET, intent_id: vector.intent_id, action_json: vector.action_json, ...overrides });
const withIntent = (actionJson, domain = DOMAIN) => ({ address: WALLET, intent_id: intentIdFor(domain, actionJson), action_json: actionJson });
const submitsFor = (s, intentId) => s.state.submits.filter((p) => p.intent_id === intentId);

async function main() {
    const a = stub('validator-a');
    const b = stub('validator-b');
    await Promise.all([a, b].map((s) => new Promise((resolve) => s.server.listen(0, '127.0.0.1', resolve))));
    const sources = [a, b].map((s) => ({ validatorId: s.validatorId, host: '127.0.0.1', port: s.server.address().port }));
    const storeDir = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-private-actions-'));
    const privateEnv = {
        WALLET_PROXY_PRIVATE_ACTIONS_CHAIN_ID: DOMAIN.chain_id,
        WALLET_PROXY_PRIVATE_ACTIONS_GENESIS_HASH: DOMAIN.genesis_hash,
        WALLET_PROXY_PRIVATE_ACTIONS_RPC_SOURCES: sources.map((s) => `${s.validatorId}=${s.host}:${s.port}`).join(','),
        WALLET_PROXY_PRIVATE_ACTIONS_BASE_ASSET_ID: A666,
        WALLET_PROXY_PRIVATE_ACTIONS_QUOTE_ASSET_ID: PFUSDC,
        WALLET_PROXY_PRIVATE_ACTIONS_STORE: storeDir,
    };
    delete process.env.WALLET_PROXY_API_TOKEN;
    delete process.env.LISTEN_HOST;
    Object.assign(process.env, privateEnv, {
        WALLET_PROXY_API_TOKENS_JSON: JSON.stringify({ alpha: ALPHA, beta: BETA }),
        ALLOWED_ORIGINS: ORIGIN,
        WALLET_PROXY_MUTATION_RATE_LIMIT: '1000',
        NAVSWAP_IDEMPOTENCY_STORE_PATH: 'off',
        NAVSWAP_RUN_STORE_PATH: 'off',
    });
    const { server } = require('./server');
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const port = server.address().port;
    try {
        // Unauthenticated → 401.
        const anonymous = await post(port, 'submit', body(positive.asset_orchard_ingress_v2), { token: '' });
        assert.strictEqual(anonymous.statusCode, 401);
        assert.strictEqual(anonymous.body.code, 'proxy_auth_required');
        assert.strictEqual((await post(port, 'snapshot', { address: WALLET }, { token: '' })).statusCode, 401);

        // Snapshot: the contract shape, one finalized token, honest completeness flags.
        const snap = await post(port, 'snapshot', { address: WALLET });
        assert.strictEqual(snap.statusCode, 200, JSON.stringify(snap.body));
        assert.deepStrictEqual(Object.keys(snap.body).sort(), Object.keys(schema.examples.snapshot.response).sort());
        assert.deepStrictEqual(snap.body.domain, DOMAIN);
        assert.deepStrictEqual(snap.body.snapshot_token, { height: 1161, block_hash: h96('a4'), state_root: h96('f3') });
        assert.deepStrictEqual(snap.body.complete, { commitments: false, nullifiers: false, accepted_anchors: false, outputs: false, assets: true });
        assert.deepStrictEqual(snap.body.assets.map((row) => [row.code, row.precision, row.requires_authorization, row.proof_profile_id]), [
            ['A666', 6, true, assets.A666.proof_profile_id.value], ['PFUSDC', 6, false, assets.pfUSDC.proof_profile_id.value]]);
        b.state.height = 1162;
        const diverged = await post(port, 'snapshot', { address: WALLET });
        assert.strictEqual(diverged.statusCode, 503);
        assert.strictEqual(diverged.body.code, 'private_snapshot_sources_diverged');
        b.state.height = 1161;

        // Each S1 vector is admitted as queued with its pinned bindings, sent once to each source.
        for (const vector of vectors.positive) {
            const response = await post(port, 'submit', body(vector));
            assert.strictEqual(response.statusCode, 202, `${vector.kind}: ${JSON.stringify(response.body)}`);
            assert.deepStrictEqual(Object.keys(response.body).sort(), Object.keys(schema.examples.submit.response).sort());
            assert.deepStrictEqual(response.body, {
                schema: 'postfiat-private-actions-v1', intent_id: vector.intent_id, state: 'queued',
                batch_id: vector.batch_id, action_hash: vector.action_hash,
            });
            for (const s of [a, b]) {
                const sent = submitsFor(s, vector.intent_id);
                assert.strictEqual(sent.length, 1);
                assert.strictEqual(sent[0].action_json, vector.action_json);
                assert.deepStrictEqual(sent[0].domain, DOMAIN);
            }
            const stored = JSON.parse(fs.readFileSync(path.join(storeDir, 'intents', `${vector.intent_id}.json`), 'utf8'));
            assert.strictEqual(stored.batch_json, vector.batch_json);
        }
        const ingress = positive.asset_orchard_ingress_v2;

        // The same body twice: the same intent, no second dispatch.
        const again = await post(port, 'submit', body(ingress));
        assert.strictEqual(again.statusCode, 202);
        assert.strictEqual(again.body.intent_id, ingress.intent_id);
        assert.strictEqual(submitsFor(a, ingress.intent_id).length, 1);
        assert.strictEqual(submitsFor(b, ingress.intent_id).length, 1);

        const refused = async (requestBody, status, code, options) => {
            const response = await post(port, 'submit', requestBody, options);
            assert.strictEqual(response.statusCode, status, `${code}: ${JSON.stringify(response.body)}`);
            assert.strictEqual(response.body.code, code);
        };
        // Different bytes under an existing intent; a second intent replaying the same action.
        const changed = negative.changed_text_under_existing_intent;
        await refused(body(ingress, { action_json: changed.action_json }), 400, 'private_intent_mismatch');
        const reformatted = negative.reformatted_text_same_action;
        await refused({ address: WALLET, intent_id: reformatted.recomputed_intent_id, action_json: reformatted.action_json }, 409, 'private_action_duplicate');
        // Legacy kinds and unknown fields at either level.
        for (const name of ['legacy_asset_orchard_ingress_v1', 'legacy_asset_orchard_egress_v1', 'swap_json_unknown_field', 'action_unknown_field']) {
            const statusCode = 400;
            await refused(withIntent(negative[name].action_json), statusCode, {
                legacy_asset_orchard_ingress_v1: 'private_action_legacy_refused',
                legacy_asset_orchard_egress_v1: 'private_action_legacy_refused',
                swap_json_unknown_field: 'private_swap_json_fields_refused',
                action_unknown_field: 'private_action_unknown_fields',
            }[name]);
        }
        // Oversized: the request and the action text.
        await refused(null, 413, 'request_body_too_large', { raw: JSON.stringify({ padding: 'x'.repeat(2 * 1024 * 1024) }) });
        await refused(withIntent(`{"kind":"shielded_swap_v1","swap_json":${JSON.stringify('x'.repeat(1024 * 1024))}}`), 413, 'private_action_too_large');
        // Mismatched domain: the intent id under another genesis, or a burn signed for another chain.
        await refused(withIntent(ingress.action_json, { ...DOMAIN, genesis_hash: h96('ab') }), 400, 'private_intent_mismatch');
        await refused(withIntent(ingress.action_json.replace('"chain_id":"postfiat-wan-devnet-2"', '"chain_id":"postfiat-other"')), 403, 'private_action_domain_mismatch');
        await refused({ ...withIntent(ingress.action_json), address: `pf${'9'.repeat(40)}` }, 403, 'private_action_address_mismatch');
        // Another session cannot read or resubmit this intent.
        await refused(body(ingress), 403, 'private_session_mismatch', { token: BETA });
        assert.strictEqual(submitsFor(a, ingress.intent_id).length, 1);

        // Receipts: unknown intent stays pending; a final rejection carries its code and fee.
        const egress = JSON.parse(positive.asset_orchard_private_egress_v1.action_json);
        egress.amount += 1;
        const unseen = withIntent(JSON.stringify(egress));
        const pending = await post(port, 'receipt', unseen);
        assert.strictEqual(pending.statusCode, 200);
        assert.deepStrictEqual(Object.keys(pending.body).sort(), Object.keys(schema.examples.receipt.response).sort());
        assert.strictEqual(pending.body.state, 'unknown');
        assert.strictEqual(pending.body.accepted, null);
        assert.strictEqual(pending.body.finality, null);
        const queued = await post(port, 'receipt', body(ingress));
        assert.strictEqual(queued.body.state, 'queued');
        const rejection = {
            state: 'rejected', intent_id: ingress.intent_id, action_hash: ingress.action_hash, batch_id: ingress.batch_id,
            transaction_id: h96('d4'), inclusion_height: 1170, block_hash: h96('e5'), state_root: h96('f6'),
            accepted: false, code: 'asset_orchard_proof_invalid', fee: 10,
            finality: { kind: 'validator_quorum_block_certificate_v1', height: 1170, block_hash: h96('e5'), state_root: h96('f6'), quorum: 5, validator_count: 6, certificate_json: '{}' },
        };
        a.state.receipts.set(ingress.intent_id, rejection);
        const unconfirmed = await post(port, 'receipt', body(ingress));
        assert.strictEqual(unconfirmed.body.state, 'queued');
        assert.strictEqual(unconfirmed.body.code, 'private_receipt_unconfirmed');
        b.state.receipts.set(ingress.intent_id, rejection);
        const final = await post(port, 'receipt', body(ingress));
        assert.strictEqual(final.statusCode, 200);
        assert.strictEqual(final.body.state, 'rejected');
        assert.strictEqual(final.body.accepted, false);
        assert.strictEqual(final.body.code, 'asset_orchard_proof_invalid');
        assert.strictEqual(final.body.fee, 10);
        assert.strictEqual(final.body.inclusion_height, 1170);
        assert.deepStrictEqual(final.body.finality, rejection.finality);
        b.state.receipts.set(ingress.intent_id, { ...rejection, fee: 11 });
        const split = await post(port, 'receipt', body(ingress));
        assert.strictEqual(split.body.state, 'unknown');
        assert.strictEqual(split.body.code, 'private_action_source_mismatch');
        assert.strictEqual(submitsFor(a, ingress.intent_id).length, 1, 'receipt never resubmits');

        // Crash between the reservation write and the dispatch.
        const crashStore = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-private-crash-'));
        const swap = positive.shielded_swap_v1;
        const crashEnv = { ...privateEnv, WALLET_PROXY_PRIVATE_ACTIONS_STORE: crashStore };
        const child = spawnSync(process.execPath, ['-e', `
            const { createPrivateActions } = require(${JSON.stringify(path.join(__dirname, 'private-actions.js'))});
            const actions = createPrivateActions({ env: ${JSON.stringify({ ...crashEnv, PFT_PRIVATE_ACTIONS_FAULT: 'after-reservation' })}, rpcRequest: () => { throw new Error('must not dispatch'); } });
            actions.submit(${JSON.stringify(body(swap))}, 'alpha').then(() => process.exit(3), () => process.exit(4));
        `], { encoding: 'utf8', timeout: 30_000 });
        assert.strictEqual(child.signal, 'SIGKILL', child.stderr);
        const restarted = createPrivateActions({ env: crashEnv, rpcRequest: (source, request) => oneShot(source, request) });
        const reserved = restarted.store.read(swap.intent_id);
        assert.strictEqual(reserved.state, 'queued');
        assert.strictEqual(reserved.acknowledged, false);
        assert.strictEqual(reserved.batch_id, swap.batch_id);
        assert.strictEqual((await restarted.receipt(body(swap), 'alpha')).state, 'queued');
        const before = submitsFor(a, swap.intent_id).length;
        const retried = await restarted.submit(body(swap), 'alpha');
        assert.strictEqual(retried.batch_id, swap.batch_id);
        assert.strictEqual(submitsFor(a, swap.intent_id).length, before + 1, 'the reserved batch is resent once');
        assert.strictEqual(submitsFor(a, swap.intent_id).at(-1).action_json, swap.action_json);
        await restarted.submit(body(swap), 'alpha');
        assert.strictEqual(submitsFor(a, swap.intent_id).length, before + 1, 'no further dispatch once acknowledged');

        // Disabled without a configured domain.
        const disabled = createPrivateActions({ env: {} });
        assert.strictEqual(disabled.enabled, false);

        console.log('private action routes regression passed');
    } finally {
        await new Promise((resolve) => server.close(resolve));
        for (const s of [a, b]) {
            s.server.close();
            for (const socket of s.state.sockets) socket.destroy();
        }
    }
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
