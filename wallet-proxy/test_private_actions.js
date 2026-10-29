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

const NODE_RECORD_SCHEMA = 'postfiat.private_action_admission.record.v1';
const NODE_RECEIPT_SCHEMA = 'postfiat.private_action_receipt.v1';
const SUBMIT_PARAMS = ['chain_id', 'genesis_hash', 'protocol_version', 'pool_id', 'session_scope', 'intent_id', 'action_json', 'action_hash'];
const SECTIONS = ['commitments', 'nullifiers', 'accepted_anchors', 'outputs', 'assets'];
const assetRow = (row) => ({
    asset_id: row.asset_id.value, ingress_total: 1, egress_total: 0, live_total: 1, asset_info_bound: true,
    issuer: row.issuer.value, code: row.code.value, precision: row.precision.value,
    requires_authorization: row.requires_authorization.value, proof_profile_id: row.proof_profile_id.value,
});
function basePool() {
    const field = (n) => `${n.toString(16).padStart(2, '0')}${'00'.repeat(31)}`;
    return {
        commitments: [1, 2, 3, 4].map(field),
        nullifiers: [5, 6, 7, 8].map(field),
        accepted_anchors: [9, 10, 11, 12].map((n, i) => ({ root: field(n), output_count: i + 1 })),
        outputs: [1, 2, 3, 4].map((n, i) => ({ position: i, commitment: field(n), kind: 'asset_orchard', ciphertext: `${'4'}${n}`.repeat(290) })),
        assets: [assetRow(assets.A666), assetRow(assets.pfUSDC)],
        lines: [{ trustline_id: h96('11'), asset_id: assets.pfUSDC.asset_id.value, issuer: assets.pfUSDC.issuer.value,
            code: 'PFUSDC', precision: 6, requires_authorization: false, balance: 0, limit: 1000000, authorized: true,
            frozen: false, line_status: 'ok' }],
        pricing_claim_json: '{"nav_epoch":12}',
        completeness: { empty_pool_before_ingress: false, history_complete: true, outputs_complete: true, anchors_from_retained_prefixes: true, complete: true },
    };
}

function stub(validatorId) {
    const state = { submits: [], records: new Map(), submitError: null, receipts: new Map(), tip: { height: 1161 }, pool: basePool(), pageSize: {}, pageCalls: [], onPage: null, sockets: new Set() };
    const answer = (request) => {
        const p = request.params || {};
        // Shapes of crates/node/src/private_action_rpc.rs and private_action_admission.rs.
        if (request.method === 'private_action_submit') {
            state.submits.push(p);
            const extra = Object.keys(p).find((key) => !SUBMIT_PARAMS.includes(key));
            if (extra || SUBMIT_PARAMS.some((key) => p[key] === undefined)) {
                return { ok: false, error: { code: 'private_action_request_invalid', message: `unknown or missing param ${extra || ''}` } };
            }
            if (state.submitError) return { ok: false, error: { code: state.submitError, message: 'stubbed refusal' } };
            const existing = state.records.get(p.intent_id);
            // Like the node: the same bytes over an abandoned record reserve a fresh one.
            if (existing && existing.state !== 'abandoned') {
                return { ok: true, result: { schema: NODE_RECORD_SCHEMA, existing: true, record: existing } };
            }
            const domain = { chain_id: p.chain_id, genesis_hash: p.genesis_hash, protocol_version: p.protocol_version };
            const parsed = parsePublicAction(p.action_json);
            const batchId = batchIdFor(domain, parsed.canonical);
            const record = {
                schema: NODE_RECORD_SCHEMA, intent_id: p.intent_id,
                domain: { ...domain, pool_id: p.pool_id, session_scope: p.session_scope },
                action_hash: p.action_hash, action_kind: parsed.kind, action_json: p.action_json, batch_id: batchId,
                batch_json: `{"batch_id":"${batchId}","atomic":true,"actions":[${parsed.canonical}]}`,
                state: 'queued', received_height: state.tip.height, dispatch_count: 0, outcome: null,
            };
            state.records.set(p.intent_id, record);
            return { ok: true, result: { schema: NODE_RECORD_SCHEMA, existing: false, record } };
        }
        if (request.method === 'private_action_receipt') {
            const contract = state.receipts.get(p.intent_id);
            const record = state.records.get(p.intent_id) || null;
            if (!contract && !record) {
                return { ok: true, result: { schema: NODE_RECEIPT_SCHEMA, intent_id: p.intent_id, state: 'pending_unknown', record: null, contract_receipt: null } };
            }
            const receipt = contract || {
                schema: 'postfiat-private-actions-v1', intent_id: record.intent_id, state: record.state, action_hash: record.action_hash,
                batch_id: record.batch_id, transaction_id: null, inclusion_height: null, block_hash: null, state_root: null,
                accepted: null, code: record.state === 'abandoned' ? 'private_action_abandoned' : null, fee: null, finality: null,
                ...(record.state === 'abandoned' ? { abandon_reason: record.abandon_reason } : {}),
            };
            return { ok: true, result: { schema: NODE_RECEIPT_SCHEMA, intent_id: p.intent_id, state: receipt.state, record, contract_receipt: receipt } };
        }
        if (request.method === 'private_pool_snapshot') {
            const height = state.tip.height;
            return { ok: true, result: {
                schema: 'postfiat.private_pool_snapshot.v1', ...DOMAIN, address: p.address ?? null,
                snapshot_token: `tok-${p.pool_id}-${height}`, finalized_height: height,
                finalized_block_hash: h96('a4'), finalized_state_root: h96('f3'), pool_id: p.pool_id, pool_present: true,
                counts: Object.fromEntries(SECTIONS.map((section) => [section, state.pool[section].length])),
                page_limits: { items: 512, outputs: 64, output_ciphertext_bytes: 1048576, assets: 16 },
                base_asset_id: p.base_asset_id, quote_asset_id: p.quote_asset_id, inclusion_height: height + 1,
                pricing_status: 'ok', pricing_refusal: null, pricing_claim: null, pricing_claim_json: state.pool.pricing_claim_json,
                egress_status: 'ungoverned', policy_id: null, disclosure_hash: null, completeness: state.pool.completeness,
            } };
        }
        if (request.method === 'private_pool_page' || request.method === 'private_account_lines') {
            state.pageCalls.push(request.method === 'private_pool_page' ? p.section : 'lines');
            const current = `tok-asset-orchard-v1-${state.tip.height}`;
            const report = { snapshot_token: p.snapshot_token, status: 'ok', refusal: null };
            if (state.onPage) state.onPage(state.pageCalls.length);
            if (p.snapshot_token !== `tok-asset-orchard-v1-${state.tip.height}` || p.snapshot_token !== current) {
                return { ok: true, result: { ...report, status: 'refused', refusal: { code: 'snapshot_unavailable', reason: 'tip moved' } } };
            }
            if (request.method === 'private_account_lines') {
                return { ok: true, result: { ...report, schema: 'postfiat.private_account_lines.v1', address: p.address,
                    line_count: state.pool.lines.length, lines: state.pool.lines, positive_asset_count: 1,
                    account_assets_consistent: true, all_lines_usable: true } };
            }
            const rows = state.pool[p.section];
            const offset = p.cursor ? Number(p.cursor.split(':')[1]) : 0;
            const limit = state.pageSize[p.section] ?? (p.section === 'assets' ? 1 : 2);
            const end = Math.min(rows.length, offset + limit);
            return { ok: true, result: { ...report, schema: 'postfiat.private_pool_page.v1', section: p.section, offset, limit,
                total: rows.length, item_count: end - offset, items: rows.slice(offset, end),
                next_cursor: end < rows.length ? `${p.section}:${end}` : null } };
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

        // Snapshot: two pages per section join into complete, ordered lists pinned to one token.
        const sharedTip = { height: 1161 };
        a.state.tip = sharedTip;
        b.state.tip = sharedTip;
        const snap = await post(port, 'snapshot', { address: WALLET });
        assert.strictEqual(snap.statusCode, 200, JSON.stringify(snap.body));
        assert.deepStrictEqual(Object.keys(snap.body).sort(), Object.keys(schema.examples.snapshot.response).sort());
        assert.deepStrictEqual(snap.body.domain, DOMAIN);
        assert.deepStrictEqual(snap.body.snapshot_token, { height: 1161, block_hash: h96('a4'), state_root: h96('f3') });
        const pool = basePool();
        assert.deepStrictEqual(snap.body.commitments, pool.commitments);
        assert.deepStrictEqual(snap.body.nullifiers, pool.nullifiers);
        assert.deepStrictEqual(snap.body.accepted_anchors, pool.accepted_anchors.map((row) => row.root));
        assert.deepStrictEqual(snap.body.outputs, pool.outputs.map(({ commitment, ciphertext }) => ({ commitment, ciphertext })));
        assert.deepStrictEqual(snap.body.assets.map((row) => [row.code, row.precision, row.requires_authorization, row.proof_profile_id]), [
            ['A666', 6, true, assets.A666.proof_profile_id.value], ['PFUSDC', 6, false, assets.pfUSDC.proof_profile_id.value]]);
        assert.strictEqual(snap.body.pricing_claim_json, '{"nav_epoch":12}');
        assert.strictEqual(snap.body.policy_id, null);
        assert.deepStrictEqual(snap.body.complete, { commitments: true, nullifiers: true, accepted_anchors: true, outputs: true, assets: true });
        assert.deepStrictEqual(snap.body.cursors, { commitments: null, nullifiers: null, accepted_anchors: null, outputs: null });
        for (const section of SECTIONS) assert.strictEqual(a.state.pageCalls.filter((call) => call === section).length, 2, section);
        assert.ok(a.state.pageCalls.includes('lines'), 'account lines are read under the token');
        assert.strictEqual(b.state.pageCalls.length, 0, 'pages come from one source');

        // A frozen line for a listed asset, or incomplete node history, clears the matching flags.
        a.state.pool.lines = [{ ...pool.lines[0], frozen: true, line_status: 'frozen' }];
        a.state.pool.completeness = { ...pool.completeness, history_complete: false, complete: false };
        const flagged = await post(port, 'snapshot', { address: WALLET });
        assert.deepStrictEqual(flagged.body.complete, { commitments: false, nullifiers: false, accepted_anchors: true, outputs: true, assets: false });
        a.state.pool = basePool();

        // The tip moves between pages: one retry from a fresh token, then 503.
        a.state.pageCalls = [];
        a.state.onPage = (calls) => { if (calls === 1) sharedTip.height = 1162; };
        const refreshed = await post(port, 'snapshot', { address: WALLET });
        assert.strictEqual(refreshed.statusCode, 200, JSON.stringify(refreshed.body));
        assert.strictEqual(refreshed.body.snapshot_token.height, 1162);
        a.state.onPage = () => { sharedTip.height += 1; };
        const moving = await post(port, 'snapshot', { address: WALLET });
        assert.strictEqual(moving.statusCode, 503);
        assert.strictEqual(moving.body.code, 'private_snapshot_unavailable');
        a.state.onPage = null;

        // Sources at different tokens: a mixed tip is refused.
        b.state.tip = { height: sharedTip.height + 1 };
        const diverged = await post(port, 'snapshot', { address: WALLET });
        assert.strictEqual(diverged.statusCode, 503);
        assert.strictEqual(diverged.body.code, 'private_snapshot_sources_diverged');
        b.state.tip = sharedTip;

        // Ceilings: 4096 list items, 64 ciphertexts, and the 4 MiB response.
        const big = basePool();
        big.commitments = Array.from({ length: 4100 }, (_, n) => n.toString(16).padStart(64, '0'));
        big.outputs = Array.from({ length: 70 }, (_, n) => ({ position: n, commitment: n.toString(16).padStart(64, '0'), kind: 'asset_orchard', ciphertext: 'ab'.repeat(100) }));
        a.state.pool = big;
        b.state.pool = big;
        a.state.pageSize = { commitments: 512, outputs: 64 };
        const capped = await post(port, 'snapshot', { address: WALLET });
        assert.strictEqual(capped.statusCode, 200, JSON.stringify(capped.body).slice(0, 300));
        assert.strictEqual(capped.body.commitments.length, 4096);
        assert.strictEqual(capped.body.outputs.length, 64);
        assert.strictEqual(capped.body.complete.commitments, false);
        assert.strictEqual(capped.body.complete.outputs, false);
        big.outputs = Array.from({ length: 64 }, (_, n) => ({ position: n, commitment: n.toString(16).padStart(64, '0'), kind: 'asset_orchard', ciphertext: 'cd'.repeat(48 * 1024) }));
        const heavy = await post(port, 'snapshot', { address: WALLET });
        assert.strictEqual(heavy.statusCode, 200);
        assert.ok(Buffer.byteLength(JSON.stringify(heavy.body)) <= 4 * 1024 * 1024);
        assert.ok(heavy.body.outputs.length < 64 && heavy.body.outputs.length > 0);
        assert.strictEqual(heavy.body.complete.outputs, false);
        a.state.pool = basePool();
        b.state.pool = basePool();
        a.state.pageSize = {};

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
                assert.deepStrictEqual(Object.keys(sent[0]).sort(), [...SUBMIT_PARAMS].sort());
                assert.strictEqual(sent[0].session_scope, 'alpha');
                assert.strictEqual(sent[0].pool_id, 'asset-orchard-v1');
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
            schema: 'postfiat-private-actions-v1', state: 'rejected', intent_id: ingress.intent_id, action_hash: ingress.action_hash, batch_id: ingress.batch_id,
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

        // The node's refusal classes: request_invalid reserves nothing (queued, resent later);
        // a final refusal from every source makes the record rejected.
        const variant = (amountDelta) => {
            const value = JSON.parse(positive.asset_orchard_private_egress_v1.action_json);
            value.amount += amountDelta;
            return withIntent(JSON.stringify(value));
        };
        a.state.submitError = 'private_action_request_invalid';
        b.state.submitError = 'private_action_request_invalid';
        const invalid = variant(10);
        const held = await post(port, 'submit', invalid);
        assert.strictEqual(held.statusCode, 202);
        assert.strictEqual(held.body.state, 'queued');
        a.state.submitError = null;
        b.state.submitError = null;
        const resent = await post(port, 'submit', invalid);
        assert.strictEqual(resent.body.state, 'queued');
        assert.strictEqual(submitsFor(a, invalid.intent_id).length, 2, 'the reserved bytes are resent once nothing was reserved');
        await post(port, 'submit', invalid);
        assert.strictEqual(submitsFor(a, invalid.intent_id).length, 2, 'no resend once every source holds them');
        a.state.submitError = 'private_action_rejected';
        b.state.submitError = 'private_action_rejected';
        const refusedEverywhere = await post(port, 'submit', variant(11));
        assert.strictEqual(refusedEverywhere.body.state, 'rejected');
        a.state.submitError = 'private_action_rejected';
        b.state.submitError = 'shielded_atomic_batch_not_active';
        const notYet = await post(port, 'submit', variant(12));
        assert.strictEqual(notYet.body.state, 'queued', 'an execution code is not a final refusal');
        a.state.submitError = null;
        b.state.submitError = null;

        // The node's `unknown`: two unknown answers are unknown, not a mismatch.
        const unknownIntent = variant(13);
        await post(port, 'submit', unknownIntent);
        for (const s of [a, b]) s.state.records.get(unknownIntent.intent_id).state = 'unknown';
        const unknownReceipt = await post(port, 'receipt', unknownIntent);
        assert.strictEqual(unknownReceipt.body.state, 'unknown');
        assert.notStrictEqual(unknownReceipt.body.code, 'private_action_source_mismatch');

        // Abandoned by an operator on the nodes: reported as itself, never resent by the gateway
        // on its own; the wallet's explicit resubmit reaches every source, and the fresh node
        // records follow the chain.
        const lost = variant(15);
        await post(port, 'submit', lost);
        for (const s of [a, b]) {
            Object.assign(s.state.records.get(lost.intent_id), {
                state: 'abandoned', abandon_reason: 'abandoned by the operator at unix 1700000000: lost',
            });
        }
        const sentBefore = [a, b].map((s) => submitsFor(s, lost.intent_id).length);
        const abandonedReceipt = await post(port, 'receipt', lost);
        assert.strictEqual(abandonedReceipt.statusCode, 200, JSON.stringify(abandonedReceipt.body));
        assert.strictEqual(abandonedReceipt.body.state, 'abandoned');
        assert.strictEqual(abandonedReceipt.body.code, 'private_action_abandoned');
        assert.strictEqual(abandonedReceipt.body.abandon_reason, 'abandoned by the operator at unix 1700000000: lost');
        assert.deepStrictEqual(Object.keys(abandonedReceipt.body).sort(),
            Object.keys(schema.examples.receipt.abandoned_response).sort());
        assert.strictEqual((await post(port, 'receipt', lost)).body.state, 'abandoned');
        assert.deepStrictEqual([a, b].map((s) => submitsFor(s, lost.intent_id).length), sentBefore,
            'an abandoned intent is never resent on the gateway\'s own initiative');
        const resubmitted = await post(port, 'submit', lost);
        assert.strictEqual(resubmitted.statusCode, 202, JSON.stringify(resubmitted.body));
        assert.strictEqual(resubmitted.body.state, 'queued');
        for (const [index, s] of [a, b].entries()) {
            assert.strictEqual(submitsFor(s, lost.intent_id).length, sentBefore[index] + 1, s.validatorId);
            assert.strictEqual(submitsFor(s, lost.intent_id).at(-1).action_json, lost.action_json);
            assert.strictEqual(s.state.records.get(lost.intent_id).state, 'queued', 'a fresh node record');
        }
        assert.strictEqual((await post(port, 'submit', lost)).body.state, 'queued');
        assert.strictEqual(submitsFor(a, lost.intent_id).length, sentBefore[0] + 1, 'no further resend');
        const landed = {
            schema: 'postfiat-private-actions-v1', state: 'accepted', intent_id: lost.intent_id,
            action_hash: resubmitted.body.action_hash, batch_id: resubmitted.body.batch_id,
            transaction_id: h96('d8'), inclusion_height: 1180, block_hash: h96('e8'), state_root: h96('f8'),
            accepted: true, code: 'accepted', fee: 10,
            finality: { kind: 'validator_quorum_block_certificate_v1', height: 1180, block_hash: h96('e8'), state_root: h96('f8'), quorum: 5, validator_count: 6, certificate_json: '{}' },
        };
        a.state.receipts.set(lost.intent_id, landed);
        b.state.receipts.set(lost.intent_id, landed);
        const settled = await post(port, 'receipt', lost);
        assert.strictEqual(settled.body.state, 'accepted');
        assert.strictEqual(settled.body.abandon_reason, undefined);

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

        // Three sources: the record reaches every one; the proposer's acceptance is
        // unconfirmed until another source follows the chain, then final.
        const c = stub('validator-c');
        await new Promise((resolve) => c.server.listen(0, '127.0.0.1', resolve));
        const trio = [a, b, c].map((s) => `${s.validatorId}=127.0.0.1:${s.server.address().port}`).join(',');
        const three = createPrivateActions({
            env: { ...privateEnv, WALLET_PROXY_PRIVATE_ACTIONS_RPC_SOURCES: trio, WALLET_PROXY_PRIVATE_ACTIONS_STORE: fs.mkdtempSync(path.join(os.tmpdir(), 'pft-private-three-')) },
            rpcRequest: (source, request) => oneShot(source, request),
        });
        const chained = variant(14);
        const admitted = await three.submit(chained, 'alpha');
        assert.strictEqual(admitted.state, 'queued');
        for (const s of [a, b, c]) assert.strictEqual(submitsFor(s, chained.intent_id).length, 1, s.validatorId);
        const accepted = {
            schema: 'postfiat-private-actions-v1', state: 'accepted', intent_id: chained.intent_id, action_hash: admitted.action_hash,
            batch_id: admitted.batch_id, transaction_id: h96('d7'), inclusion_height: 1175, block_hash: h96('e7'), state_root: h96('f7'),
            accepted: true, code: 'accepted', fee: 10,
            finality: { kind: 'validator_quorum_block_certificate_v1', height: 1175, block_hash: h96('e7'), state_root: h96('f7'), quorum: 5, validator_count: 6, certificate_json: '{}' },
        };
        c.state.receipts.set(chained.intent_id, accepted);
        const proposerOnly = await three.receipt(chained, 'alpha');
        assert.strictEqual(proposerOnly.code, 'private_receipt_unconfirmed');
        a.state.records.get(chained.intent_id).state = 'unknown';
        assert.strictEqual((await three.receipt(chained, 'alpha')).code, 'private_receipt_unconfirmed', 'accepted plus unknown stays unconfirmed');
        a.state.receipts.set(chained.intent_id, accepted);
        const followed = await three.receipt(chained, 'alpha');
        assert.strictEqual(followed.state, 'accepted');
        assert.strictEqual(followed.inclusion_height, 1175);
        assert.deepStrictEqual(followed.finality, accepted.finality);
        c.server.close();
        for (const socket of c.state.sockets) socket.destroy();

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
