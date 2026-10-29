'use strict';

// W-1 segment S2: the gateway's versioned private action routes. The gateway
// strictly decodes a user-proven action, recomputes its intent id, reserves it
// durably once, and relays the exact bytes to two validator sources through
// the node methods private_action_submit / private_action_receipt (S3). It
// holds no key, reads no validator data directory and never builds a proof.
// Contract: crates/node/tests/fixtures/private-actions-v1/.

const crypto = require('crypto');
const fs = require('fs');
const os = require('os');
const path = require('path');

const SCHEMA = 'postfiat-private-actions-v1';
const RECORD_SCHEMA = 'postfiat-private-action-record-v1';
const ROUTE_PREFIX = '/api/private/v1/';
const INTENT_DOMAIN = 'tasknode.private-intent.v1';
const ACTION_DOMAIN = 'postfiat.private-actions.action.v1';
const BATCH_DOMAIN = 'postfiat.shielded_action_batch.atomic.v2';
const MAX_ACTION_BYTES = 1024 * 1024;
const MAX_REQUEST_BYTES = 2 * 1024 * 1024;
const MAX_RESPONSE_BYTES = 4 * 1024 * 1024;
const STATES = new Set(['queued', 'dispatched', 'accepted', 'rejected', 'unknown']);
const NODE_STATES = new Set(['queued', 'dispatched', 'accepted', 'rejected']);
const FAULT_ENV = 'PFT_PRIVATE_ACTIONS_FAULT'; // test-only: 'after-reservation'
const HEX96 = /^[0-9a-f]{96}$/;
const ADDRESS_RE = /^pf[0-9a-f]{40}$/;
const UNDELIVERED = new Set(['ECONNREFUSED', 'ENOTFOUND', 'EHOSTUNREACH', 'ENETUNREACH', 'EAI_AGAIN']);

// Field order and types of the serde serialization of postfiat_types, so the
// re-serialized action is byte-identical to the node's canonical form.
const UNSIGNED_BURN = [
    ['chain_id', 'string'], ['genesis_hash', 'string'], ['protocol_version', 'u32'],
    ['address_namespace', 'string'], ['transaction_kind', 'string'],
    ['signature_algorithm_id', 'string'], ['source', 'string'], ['fee', 'u64'],
    ['sequence', 'u64'], ['operation', 'const:asset_burn'], ['owner', 'string'],
    ['issuer', 'string'], ['asset_id', 'string'], ['amount', 'u64'],
];
const SIGNED_BURN = [
    ['unsigned', UNSIGNED_BURN], ['algorithm_id', 'string'],
    ['public_key_hex', 'string'], ['signature_hex', 'string'],
];
const KIND_SPECS = {
    asset_orchard_ingress_v2: [
        ['kind', 'const:asset_orchard_ingress_v2'], ['burn_transaction', SIGNED_BURN],
        ['pool_id', 'string'], ['asset_id', 'string'], ['amount', 'u64'],
        ['output_commitment', 'string'], ['encrypted_output', 'string'],
    ],
    shielded_swap_v1: [['kind', 'const:shielded_swap_v1'], ['swap_json', 'string']],
    asset_orchard_private_egress_v1: [
        ['kind', 'const:asset_orchard_private_egress_v1'], ['version', 'u16'], ['schema', 'string'],
        ['pool_id', 'string'], ['to', 'string'], ['asset_id', 'string'], ['amount', 'u64'],
        ['fee', 'u64'], ['policy_id', 'string'], ['disclosure_hash', 'string'],
        ['proof_system_id', 'string'], ['circuit_id', 'string'], ['pool_domain', 'string'],
        ['anchor', 'string'], ['nullifier', 'string'], ['randomized_verification_key', 'string'],
        ['asset_tag_lo', 'hex128'], ['asset_tag_hi', 'hex128'], ['exit_binding_hash', 'string'],
        ['proof', 'string'], ['spend_authorization_signature', 'string'],
    ],
};
const LEGACY_KINDS = new Set(['asset_orchard_ingress_v1', 'asset_orchard_egress_v1']);
const OTHER_KINDS = new Set([
    'shield_mint', 'shield_spend', 'shield_migrate', 'orchard_action_v1', 'orchard_withdraw_v1',
    'orchard_deposit_v1', 'asset_orchard_private_primary_issue_v1',
    'asset_orchard_private_primary_redeem_v1',
]);
const SWAP_FIELDS = [
    'version', 'schema', 'pool_id', 'proof_system_id', 'circuit_id', 'pool_domain', 'anchor',
    'nullifiers', 'randomized_verification_keys', 'output_commitments', 'encrypted_outputs',
    'accounting_inputs', 'accounting_outputs', 'pricing_claim', 'swap_binding_hash', 'fee',
    'proof', 'spend_authorization_signatures',
];
const SWAP_RECORD_FIELDS = ['output_commitment', 'value_commitment'];
const PRICING_CLAIM_FIELDS = [
    'nav_epoch', 'reserve_packet_hash', 'ratio_numerator', 'ratio_denominator', 'mode', 'band_bps',
    'base_asset_tag_lo', 'base_asset_tag_hi', 'quote_asset_tag_lo', 'quote_asset_tag_hi',
];
const FINALITY_FIELDS = ['kind', 'height', 'block_hash', 'state_root', 'quorum', 'validator_count', 'certificate_json'];

const refusal = (statusCode, code, message = code) => Object.assign(new Error(message), { statusCode, code });

function sha3Hex(domain, text) {
    return crypto.createHash('sha3-384')
        .update(Buffer.from(domain, 'utf8')).update(Buffer.from([0])).update(Buffer.from(text, 'utf8'))
        .digest('hex');
}

const LONE_SURROGATE = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;

function serializeField(value, type, at) {
    if (Array.isArray(type)) return serializeObject(value, type, at);
    if (type.startsWith('const:')) {
        if (value !== type.slice(6)) throw refusal(400, 'private_action_malformed', `${at} must be ${type.slice(6)}`);
        return JSON.stringify(value);
    }
    if (type === 'string' || type === 'hex128') {
        if (typeof value !== 'string' || LONE_SURROGATE.test(value)
            || (type === 'hex128' && !/^[0-9a-f]{32}$/.test(value))) {
            throw refusal(400, 'private_action_malformed', `${at} is malformed`);
        }
        return JSON.stringify(value);
    }
    const max = { u16: 0xffff, u32: 0xffffffff, u64: Number.MAX_SAFE_INTEGER }[type];
    if (!Number.isSafeInteger(value) || value < 0 || value > max) {
        throw refusal(400, 'private_action_malformed', `${at} must be an integer in range`);
    }
    return String(value);
}

function serializeObject(value, spec, at) {
    if (!value || typeof value !== 'object' || Array.isArray(value)) {
        throw refusal(400, 'private_action_malformed', `${at} must be an object`);
    }
    const names = spec.map(([name]) => name);
    if (Object.keys(value).some((key) => !names.includes(key))) {
        throw refusal(400, 'private_action_unknown_fields', `${at} has a field the action does not define`);
    }
    if (names.some((name) => !Object.prototype.hasOwnProperty.call(value, name))) {
        throw refusal(400, 'private_action_malformed', `${at} is missing a field`);
    }
    return `{${spec.map(([name, type]) => `${JSON.stringify(name)}:${serializeField(value[name], type, `${at}.${name}`)}`).join(',')}}`;
}

function exactKeys(value, fields) {
    return value && typeof value === 'object' && !Array.isArray(value)
        && Object.keys(value).length === fields.length && fields.every((field) => Object.prototype.hasOwnProperty.call(value, field));
}

// Strict decoding of both JSON levels; returns the canonical action text.
function parsePublicAction(actionJson) {
    let value;
    try { value = JSON.parse(actionJson); } catch (_) { throw refusal(400, 'private_action_malformed'); }
    const kind = value?.kind;
    if (LEGACY_KINDS.has(kind)) throw refusal(400, 'private_action_legacy_refused');
    if (OTHER_KINDS.has(kind)) throw refusal(400, 'private_action_kind_refused');
    if (!KIND_SPECS[kind]) throw refusal(400, 'private_action_malformed');
    const canonical = serializeObject(value, KIND_SPECS[kind], 'action');
    let poolId = value.pool_id;
    if (kind === 'shielded_swap_v1') {
        let swap;
        try { swap = JSON.parse(value.swap_json); } catch (_) { throw refusal(400, 'private_swap_json_malformed'); }
        if (!swap || typeof swap !== 'object' || Array.isArray(swap)) throw refusal(400, 'private_swap_json_malformed');
        const records = [...(Array.isArray(swap.accounting_inputs) ? swap.accounting_inputs : [null]),
            ...(Array.isArray(swap.accounting_outputs) ? swap.accounting_outputs : [null])];
        if (!exactKeys(swap, SWAP_FIELDS) || !exactKeys(swap.pricing_claim, PRICING_CLAIM_FIELDS)
            || !records.every((record) => exactKeys(record, SWAP_RECORD_FIELDS))) {
            throw refusal(400, 'private_swap_json_fields_refused');
        }
        poolId = swap.pool_id;
    }
    return { kind, value, canonical, poolId };
}

function batchIdFor(domain, canonical) {
    const preimage = `[${JSON.stringify(domain.chain_id)},${JSON.stringify(domain.genesis_hash)},${domain.protocol_version},"shielded",[${canonical}]]`;
    return sha3Hex(BATCH_DOMAIN, preimage);
}

function intentIdFor(domain, actionJson) {
    const preimage = JSON.stringify([
        { chain_id: domain.chain_id, genesis_hash: domain.genesis_hash, protocol_version: domain.protocol_version },
        actionJson,
    ]);
    return sha3Hex(INTENT_DOMAIN, preimage);
}

function loadConfig(env, fleet = []) {
    const chainId = String(env.WALLET_PROXY_PRIVATE_ACTIONS_CHAIN_ID || '').trim();
    const genesisHash = String(env.WALLET_PROXY_PRIVATE_ACTIONS_GENESIS_HASH || '').trim().toLowerCase();
    if (!chainId && !genesisHash) return { enabled: false };
    const protocolVersion = Number(env.WALLET_PROXY_PRIVATE_ACTIONS_PROTOCOL_VERSION || 1);
    if (!/^[a-z0-9][a-z0-9._-]{0,63}$/.test(chainId) || !HEX96.test(genesisHash)
        || !Number.isSafeInteger(protocolVersion) || protocolVersion < 1) {
        throw new Error('private actions need WALLET_PROXY_PRIVATE_ACTIONS_CHAIN_ID and a 96-hex WALLET_PROXY_PRIVATE_ACTIONS_GENESIS_HASH');
    }
    const sourceText = String(env.WALLET_PROXY_PRIVATE_ACTIONS_RPC_SOURCES || '').trim();
    const sources = sourceText
        ? sourceText.split(',').map((part) => {
            const [validatorId, endpoint] = part.trim().split('=');
            const colon = String(endpoint || '').lastIndexOf(':');
            const port = Number(String(endpoint).slice(colon + 1));
            if (!validatorId || colon <= 0 || !Number.isInteger(port) || port <= 0 || port > 65535) {
                throw new Error(`invalid WALLET_PROXY_PRIVATE_ACTIONS_RPC_SOURCES entry: ${part}`);
            }
            return { validatorId, host: endpoint.slice(0, colon), port };
        })
        : fleet.slice(0, 2);
    if (sources.length !== 2 || sources[0].validatorId === sources[1].validatorId) {
        throw new Error('private actions need exactly two distinct validator RPC sources');
    }
    const assetIds = ['BASE', 'QUOTE'].map((name) => String(env[`WALLET_PROXY_PRIVATE_ACTIONS_${name}_ASSET_ID`] || '').trim().toLowerCase());
    if (assetIds.some((id) => id && !HEX96.test(id))) throw new Error('private action asset ids must be 96 hex');
    return {
        enabled: true,
        domain: { chain_id: chainId, genesis_hash: genesisHash, protocol_version: protocolVersion },
        poolId: String(env.WALLET_PROXY_PRIVATE_ACTIONS_POOL_ID || 'asset-orchard-v1').trim(),
        baseAssetId: assetIds[0] || null,
        quoteAssetId: assetIds[1] || null,
        sources,
        storeDir: path.resolve(env.WALLET_PROXY_PRIVATE_ACTIONS_STORE
            || path.join(os.homedir(), '.local', 'share', 'postfiat', 'wallet-proxy', 'private-actions')),
        timeoutMs: Number(env.WALLET_PROXY_PRIVATE_ACTIONS_RPC_TIMEOUT_MS || 15_000),
    };
}

// One record per intent, written like the bridge jobs: temp file, fsync, rename, fsync the directory.
function createStore(dir) {
    for (const sub of ['intents', 'by-action', 'by-batch']) fs.mkdirSync(path.join(dir, sub), { recursive: true, mode: 0o700 });
    const fsyncDir = (directory) => {
        const fd = fs.openSync(directory, 'r');
        try { fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
    };
    function atomicWrite(file, value) {
        const temporary = `${file}.${process.pid}.${crypto.randomBytes(6).toString('hex')}.tmp`;
        const fd = fs.openSync(temporary, 'wx', 0o600);
        try { fs.writeFileSync(fd, `${JSON.stringify(value)}\n`); fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
        fs.renameSync(temporary, file);
        fsyncDir(path.dirname(file));
    }
    const recordFile = (intentId) => path.join(dir, 'intents', `${intentId}.json`);
    return {
        read(intentId) {
            try { return JSON.parse(fs.readFileSync(recordFile(intentId), 'utf8')); } catch (error) {
                if (error.code === 'ENOENT') return null;
                throw error;
            }
        },
        write(record) { atomicWrite(recordFile(record.intent_id), record); },
        // Exclusive create: the first intent to claim a key owns it forever.
        claim(index, key, intentId) {
            const file = path.join(dir, index, `${key}.json`);
            try {
                const fd = fs.openSync(file, 'wx', 0o600);
                try { fs.writeFileSync(fd, `${JSON.stringify({ intent_id: intentId })}\n`); fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
                fsyncDir(path.dirname(file));
                return intentId;
            } catch (error) {
                if (error.code !== 'EEXIST') throw error;
                return JSON.parse(fs.readFileSync(file, 'utf8')).intent_id;
            }
        },
    };
}

function createPrivateActions({ env = process.env, fleet = [], rpcRequest } = {}) {
    const config = loadConfig(env, fleet);
    if (!config.enabled) return { enabled: false, handleHttp: async (req, res, url) => disabledRoute(res, url) };
    if (typeof rpcRequest !== 'function') throw new Error('private actions need an RPC client');
    const store = createStore(config.storeDir);
    let chain = Promise.resolve();
    const serialized = (operation) => {
        const run = chain.then(operation, operation);
        chain = run.catch(() => {});
        return run;
    };
    const inflight = new Map();

    async function callSource(source, method, params) {
        const request = { version: 'postfiat-local-rpc-v1', id: `private-${method}-${source.validatorId}-${Date.now()}`, method, params };
        try {
            const response = await rpcRequest(source, request, config.timeoutMs);
            if (response?.ok === true) return { source: source.validatorId, ok: true, result: response.result };
            // Only the node's own refusal of the action is final; other errors (an unknown
            // method, the operator switch off) mean nothing was reserved there.
            const code = String(response?.error?.code || 'rpc_error').slice(0, 64);
            const refused = code.startsWith('private_action_');
            return { source: source.validatorId, ok: false, refused, delivered: refused, code };
        } catch (error) {
            return { source: source.validatorId, ok: false, delivered: !UNDELIVERED.has(error.code), code: String(error.code || 'rpc_transport_error').slice(0, 64) };
        }
    }
    const bothSources = (method, params) => Promise.all(config.sources.map((source) => callSource(source, method, params)));
    const bindingsMatch = (result, record) => result?.intent_id === record.intent_id
        && result.action_hash === record.action_hash && result.batch_id === record.batch_id;

    function validateRequest(body, principal) {
        if (!exactKeys(body, ['address', 'intent_id', 'action_json'])) throw refusal(400, 'private_request_invalid');
        const { address, intent_id: intentId, action_json: actionJson } = body;
        if (!ADDRESS_RE.test(String(address)) || !HEX96.test(String(intentId)) || typeof actionJson !== 'string') {
            throw refusal(400, 'private_request_invalid');
        }
        if (Buffer.byteLength(actionJson, 'utf8') > MAX_ACTION_BYTES) throw refusal(413, 'private_action_too_large');
        const action = parsePublicAction(actionJson);
        if (intentIdFor(config.domain, actionJson) !== intentId) throw refusal(400, 'private_intent_mismatch');
        if (action.kind === 'asset_orchard_ingress_v2') {
            const unsigned = action.value.burn_transaction.unsigned;
            if (unsigned.chain_id !== config.domain.chain_id || unsigned.genesis_hash !== config.domain.genesis_hash
                || unsigned.protocol_version !== config.domain.protocol_version) {
                throw refusal(403, 'private_action_domain_mismatch');
            }
            if (unsigned.source !== address || unsigned.owner !== address) throw refusal(403, 'private_action_address_mismatch');
        }
        if (action.poolId !== config.poolId) throw refusal(403, 'private_action_pool_mismatch');
        const allowed = [config.baseAssetId, config.quoteAssetId].filter(Boolean);
        const assetIds = action.kind === 'shielded_swap_v1' ? [] : [action.value.asset_id];
        if (allowed.length && assetIds.some((id) => !allowed.includes(id))) throw refusal(403, 'private_action_asset_refused');
        return {
            address, principal, intentId, actionJson, kind: action.kind,
            canonical: action.canonical,
            actionHash: sha3Hex(ACTION_DOMAIN, action.canonical),
            batchId: batchIdFor(config.domain, action.canonical),
        };
    }

    function checkOwner(record, request) {
        if (record.action_json !== request.actionJson) throw refusal(409, 'private_intent_conflict');
        if (record.principal !== request.principal || record.address !== request.address) throw refusal(403, 'private_session_mismatch');
    }

    const submitResponse = (record) => ({
        schema: SCHEMA, intent_id: record.intent_id, state: record.state, batch_id: record.batch_id, action_hash: record.action_hash,
    });

    // Sends the reserved bytes, never the request's; reconciles the two sources.
    async function dispatch(intentId) {
        if (inflight.has(intentId)) return inflight.get(intentId);
        const run = (async () => {
            const record = store.read(intentId);
            const results = await bothSources('private_action_submit', {
                domain: config.domain, intent_id: record.intent_id, action_json: record.action_json, action_hash: record.action_hash,
            });
            return serialized(() => {
                const current = store.read(intentId);
                const answered = results.filter((result) => result.ok);
                const states = new Set(answered.map((result) => result.result?.state));
                let state = current.state;
                let code = null;
                if (answered.some((result) => !bindingsMatch(result.result, current) || !NODE_STATES.has(result.result?.state))
                    || states.size > 1 || (answered.length && results.some((result) => result.refused))) {
                    state = 'unknown';
                    code = 'private_action_source_mismatch';
                } else if (answered.length) {
                    state = [...states][0];
                } else if (results.every((result) => result.refused && result.code === results[0].code)) {
                    state = 'rejected';
                    code = results[0].code;
                } else if (results.some((result) => result.delivered)) {
                    state = 'unknown';
                    code = 'private_action_dispatch_unknown';
                } else {
                    code = 'private_action_sources_unreachable';
                }
                const updated = {
                    ...current,
                    state,
                    code,
                    acknowledged: current.acknowledged || answered.length > 0 || results.some((result) => result.delivered),
                    dispatch_attempts: (current.dispatch_attempts || 0) + 1,
                    last_dispatch: { at_ms: Date.now(), sources: results.map(({ source, ok, code: c }) => ({ source, ok, code: c || null })) },
                    updated_at_ms: Date.now(),
                };
                store.write(updated);
                return updated;
            });
        })();
        inflight.set(intentId, run);
        try { return await run; } finally { inflight.delete(intentId); }
    }

    async function submit(body, principal) {
        const request = validateRequest(body, principal);
        const reserved = await serialized(() => {
            const existing = store.read(request.intentId);
            if (existing) {
                checkOwner(existing, request);
                return { record: existing, fresh: false };
            }
            for (const [index, key] of [['by-action', request.actionHash], ['by-batch', request.batchId]]) {
                if (store.claim(index, key, request.intentId) !== request.intentId) throw refusal(409, 'private_action_duplicate');
            }
            const record = {
                schema: RECORD_SCHEMA, intent_id: request.intentId, domain: config.domain, address: request.address,
                principal: request.principal, pool_id: config.poolId, kind: request.kind, action_json: request.actionJson,
                action_hash: request.actionHash, batch_id: request.batchId,
                batch_json: `{"batch_id":${JSON.stringify(request.batchId)},"atomic":true,"actions":[${request.canonical}]}`,
                state: 'queued', code: null, acknowledged: false, created_at_ms: Date.now(), updated_at_ms: Date.now(),
            };
            store.write(record);
            return { record, fresh: true };
        });
        if (env[FAULT_ENV] === 'after-reservation') process.kill(process.pid, 'SIGKILL');
        // Only a reservation no validator has seen is (re)sent; anything else is returned as it stands.
        if (reserved.record.state !== 'queued' || reserved.record.acknowledged) return submitResponse(reserved.record);
        return submitResponse(await dispatch(request.intentId));
    }

    function finality(value) {
        if (!exactKeys(value, FINALITY_FIELDS) || value.kind !== 'validator_quorum_block_certificate_v1'
            || !HEX96.test(String(value.block_hash)) || !HEX96.test(String(value.state_root))
            || typeof value.certificate_json !== 'string') return null;
        return value;
    }

    async function receipt(body, principal) {
        const request = validateRequest(body, principal);
        const record = store.read(request.intentId);
        if (record) checkOwner(record, request);
        const expected = { intent_id: request.intentId, action_hash: request.actionHash, batch_id: request.batchId };
        const pending = (state, code = null) => ({
            schema: SCHEMA, ...expected, state, transaction_id: null, inclusion_height: null, block_hash: null,
            state_root: null, accepted: null, code, fee: null, finality: null,
        });
        const localState = record ? record.state : 'unknown';
        const results = await bothSources('private_action_receipt', { intent_id: request.intentId });
        const answers = results.filter((result) => result.ok && result.result?.state !== 'pending_unknown');
        if (answers.some((result) => !bindingsMatch(result.result, expected))) return pending('unknown', 'private_action_source_mismatch');
        const finals = answers.filter((result) => result.result.state === 'accepted' || result.result.state === 'rejected');
        if (finals.length === 0) return pending(localState, record?.code || null);
        if (finals.length === 1) return pending(localState, 'private_receipt_unconfirmed');
        const outcome = (result) => {
            const r = result.result;
            return JSON.stringify([r.state, r.transaction_id, r.inclusion_height, r.block_hash, r.state_root, r.accepted, r.code, r.fee]);
        };
        const evidence = finality(finals[0].result.finality);
        const first = finals[0].result;
        if (outcome(finals[0]) !== outcome(finals[1]) || !evidence || evidence.height !== first.inclusion_height
            || evidence.block_hash !== first.block_hash || evidence.state_root !== first.state_root
            || first.accepted !== (first.state === 'accepted')) {
            return pending('unknown', 'private_action_source_mismatch');
        }
        return {
            schema: SCHEMA, ...expected, state: first.state, transaction_id: first.transaction_id ?? null,
            inclusion_height: first.inclusion_height, block_hash: first.block_hash, state_root: first.state_root,
            accepted: first.accepted, code: first.code ?? null, fee: first.fee, finality: evidence,
        };
    }

    async function snapshot(body) {
        if (!exactKeys(body, ['address']) || !ADDRESS_RE.test(String(body.address))) throw refusal(400, 'private_request_invalid');
        if (!config.baseAssetId || !config.quoteAssetId) throw refusal(503, 'private_snapshot_not_configured');
        const info = await bothSources('server_info', {});
        const tip = (result) => result.ok && result.result?.chain_id === config.domain.chain_id
            && result.result?.genesis_hash === config.domain.genesis_hash
            ? JSON.stringify([result.result.storage?.finalized_height, result.result.storage?.finalized_block_hash, result.result.storage?.finalized_state_root])
            : null;
        if (!tip(info[0]) || tip(info[0]) !== tip(info[1])) throw refusal(503, 'private_snapshot_sources_diverged');
        const [height, blockHash, stateRoot] = JSON.parse(tip(info[0]));
        const assets = [];
        for (const assetId of [config.baseAssetId, config.quoteAssetId]) {
            const rows = await bothSources('asset_info', { asset_id: assetId });
            const profiles = await bothSources('nav_reserve_proof_status', { asset_id: assetId });
            const row = (result) => {
                const a = result.ok && result.result?.found === true ? result.result.asset : null;
                return a ? JSON.stringify([a.asset_id, a.issuer, a.code, a.precision, a.requires_authorization]) : null;
            };
            if (!row(rows[0]) || row(rows[0]) !== row(rows[1])) throw refusal(503, 'private_snapshot_sources_diverged');
            const profile = (result) => (result.ok ? result.result?.active_profile?.profile_id ?? null : null);
            const [id, issuer, code, precision, requiresAuthorization] = JSON.parse(row(rows[0]));
            assets.push({
                asset_id: id, issuer, code, precision, requires_authorization: requiresAuthorization,
                proof_profile_id: profile(profiles[0]) === profile(profiles[1]) ? profile(profiles[0]) : null,
            });
        }
        // Pool lists, the pricing claim and the egress binding come with the finalized reads (S4/S5);
        // until then they are empty or null and marked incomplete, never guessed.
        return {
            schema: SCHEMA, domain: config.domain, address: body.address,
            snapshot_token: { height, block_hash: blockHash, state_root: stateRoot },
            commitments: [], nullifiers: [], accepted_anchors: [], outputs: [], assets,
            base_asset_id: config.baseAssetId, quote_asset_id: config.quoteAssetId,
            pricing_claim_json: null, policy_id: null, disclosure_hash: null,
            complete: { commitments: false, nullifiers: false, accepted_anchors: false, outputs: false, assets: true },
            cursors: { commitments: null, nullifiers: null, accepted_anchors: null, outputs: null },
        };
    }

    async function handleHttp(req, res, url, auth) {
        if (!url.pathname.startsWith(ROUTE_PREFIX)) return false;
        const route = url.pathname.slice(ROUTE_PREFIX.length);
        if (!['snapshot', 'submit', 'receipt'].includes(route)) return send(res, 404, error('private_route_unknown'));
        if (req.method !== 'POST') return send(res, 405, error('method_not_allowed'));
        if (!auth.originAllowed(req, true)) return send(res, 403, error('origin_not_allowed'));
        const authentication = auth.authenticate(req);
        if (!authentication?.principal) {
            return send(res, authentication?.failure?.statusCode || 401, error(authentication?.failure?.code || 'proxy_auth_required'));
        }
        let admission = null;
        if (route === 'submit') {
            admission = auth.admitMutation(authentication.principal);
            if (!admission.ok) return send(res, 429, { ...error(admission.code), retry_after_ms: admission.retry_after_ms });
        }
        try {
            const body = JSON.parse((await readBody(req)).toString('utf8'));
            if (route === 'snapshot') return send(res, 200, await snapshot(body));
            if (route === 'receipt') return send(res, 200, await receipt(body, authentication.principal));
            return send(res, 202, await submit(body, authentication.principal));
        } catch (caught) {
            if (caught instanceof SyntaxError) return send(res, 400, error('private_request_invalid'));
            if (caught.statusCode) return send(res, caught.statusCode, error(caught.code, caught.message));
            return send(res, 500, error('private_actions_internal_error'));
        } finally {
            admission?.release?.();
        }
    }

    return { enabled: true, config, submit, receipt, snapshot, handleHttp, store };
}

function readBody(req) {
    return new Promise((resolve, reject) => {
        if (Number(req.headers['content-length'] || 0) > MAX_REQUEST_BYTES) {
            req.resume();
            reject(refusal(413, 'request_body_too_large'));
            return;
        }
        const chunks = [];
        let size = 0;
        let done = false;
        req.on('data', (chunk) => {
            if (done) return;
            size += chunk.length;
            if (size > MAX_REQUEST_BYTES) {
                done = true;
                req.resume();
                reject(refusal(413, 'request_body_too_large'));
                return;
            }
            chunks.push(chunk);
        });
        req.on('end', () => { if (!done) { done = true; resolve(Buffer.concat(chunks)); } });
        req.on('error', (failure) => { if (!done) { done = true; reject(failure); } });
    });
}

const error = (code, message = code) => ({ ok: false, schema: SCHEMA, code, message });

function send(res, statusCode, body) {
    let payload = JSON.stringify(body);
    if (Buffer.byteLength(payload) > MAX_RESPONSE_BYTES) {
        statusCode = 502;
        payload = JSON.stringify(error('private_response_too_large'));
    }
    res.writeHead(statusCode, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
    res.end(payload);
    return true;
}

function disabledRoute(res, url) {
    if (!url.pathname.startsWith(ROUTE_PREFIX)) return false;
    return send(res, 404, error('private_actions_disabled'));
}

module.exports = {
    ROUTE_PREFIX,
    SCHEMA,
    STATES,
    batchIdFor,
    createPrivateActions,
    intentIdFor,
    parsePublicAction,
    sha3Hex,
};
