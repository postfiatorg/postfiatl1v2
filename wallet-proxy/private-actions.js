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
const { rpcTransportErrorClass } = require('./rpc-transport-errors');

const SCHEMA = 'postfiat-private-actions-v1';
const RECORD_SCHEMA = 'postfiat-private-action-record-v1';
const ROUTE_PREFIX = '/api/private/v1/';
const INTENT_DOMAIN = 'tasknode.private-intent.v1';
const ACTION_DOMAIN = 'postfiat.private-actions.action.v1';
const BATCH_DOMAIN = 'postfiat.shielded_action_batch.atomic.v2';
const PQ_POOL = 'PFT-PQ-SHIELDED-POOL-V1';
// Profile 1's validator-governed development series (postfiat_types::PQ_REHEARSAL_ISSUE).
const PQ_REHEARSAL_ISSUE = 'bbf2fb96437f2e87ff3b470f19aa2455ae226ab810b06b64ab327b0daa71a2d1ad14638a107b7bbd3b0cf3a6f4d56269';
const MAX_ACTION_BYTES = 1024 * 1024;
const MAX_REQUEST_BYTES = 2 * 1024 * 1024;
const MAX_RESPONSE_BYTES = 4 * 1024 * 1024;
// The wallet's snapshot ceilings (contract limits): list items, ciphertexts, ciphertext bytes, assets.
const MAX_SNAPSHOT_LIST = 4096;
const MAX_SNAPSHOT_OUTPUTS = 64;
const MAX_SNAPSHOT_CIPHERTEXT_BYTES = 2 * 1024 * 1024;
const MAX_SNAPSHOT_ASSETS = 16;
const STATES = new Set(['queued', 'dispatched', 'accepted', 'rejected', 'unknown']);
const NODE_STATES = new Set(['queued', 'dispatched', 'accepted', 'rejected', 'unknown', 'abandoned']);
const NODE_RECORD_SCHEMA = 'postfiat.private_action_admission.record.v1';
const NODE_RECEIPT_SCHEMA = 'postfiat.private_action_receipt.v1';
// Codes of the node's private_action_submit (crates/node/src/private_action_rpc.rs and
// private_action_admission.rs), in two classes.
// Final: the node examined this exact action and will never admit it. When every source
// answers with the same one, the record is `rejected`.
//   private_action_rejected       re-verification against finalized state failed
//   private_action_wrong_domain   the domain names another chain, genesis, protocol or pool
//   private_action_hash_mismatch  action_hash is not the canonical hash of the decoded action
const PRIVATE_SPEND_SCOPE = 'pq-private-spend';
const FINAL_REFUSALS = new Set(['private_action_rejected', 'private_action_wrong_domain', 'private_action_hash_mismatch']);
// Not reserved: nothing was written on that node, so the reserved bytes are resent later.
//   private_action_request_invalid  malformed params, scope or sizes
//   private_intent_mismatch         the node's intent recomputation disagrees with ours
//   private_intent_conflict         other bytes are reserved there under this intent
//   private_action_duplicate        the action or batch is reserved under another intent or on chain
//   execution codes from re-verification (e.g. shielded_atomic_batch_not_active): may pass later
//   private_action_failed, rpc_internal  node-side I/O or internal errors
//   private_action_not_proposer, private_action_dispatch_interrupted, private_action_result_unknown:
//                                   round-service codes on records, not submit errors
//   anything else, e.g. the methods absent while the operator switch is off
const FAULT_ENV = 'PFT_PRIVATE_ACTIONS_FAULT'; // test-only: 'after-reservation'
const HEX96 = /^[0-9a-f]{96}$/;
const ADDRESS_RE = /^pf[0-9a-f]{40}$/;

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
    pq_payment_v1: [['kind', 'const:pq_payment_v1'], ['body', 'string']],
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
function parsePublicAction(actionJson, domain) {
    let value;
    try { value = JSON.parse(actionJson); } catch (_) { throw refusal(400, 'private_action_malformed'); }
    const kind = value?.kind;
    if (LEGACY_KINDS.has(kind)) throw refusal(400, 'private_action_legacy_refused');
    if (OTHER_KINDS.has(kind)) throw refusal(400, 'private_action_kind_refused');
    if (!KIND_SPECS[kind]) throw refusal(400, 'private_action_malformed');
    const canonical = serializeObject(value, KIND_SPECS[kind], 'action');
    if (kind === 'pq_payment_v1') {
        if (value.body.length > Math.ceil(149130 / 3) * 4) throw refusal(413, 'private_action_too_large');
        const wire = Buffer.from(value.body, 'base64');
        if (wire.length < 72 || wire.length > 149130 || wire.toString('base64') !== value.body
            || wire.subarray(0, 5).toString('hex') !== '5046505101' || wire[71] > 1) throw refusal(400, 'private_action_malformed');
        const prefix = 72 + (wire[71] ? 20 + 8 + 1952 : 0);
        const inner = prefix + (wire[71] ? 3309 : 0);
        if (wire.length - inner > 140476 || wire.length <= inner + 3648 || !wire.subarray(inner, inner + 8).equals(Buffer.from('5046534830320000', 'hex'))
            || wire.readUInt32LE(inner + 3644) !== wire.length - inner - 3648) throw refusal(400, 'private_action_malformed');
        const mode = wire[6], issued = wire.subarray(7, 55).some(Boolean);
        const publicFee = wire.readBigUInt64LE(63);
        const mint = wire.readBigUInt64LE(inner + 56), exit = wire.readBigUInt64LE(inner + 64);
        const fee = wire.readBigUInt64LE(inner + 104);
        if (mode > 2 || wire[5] !== (issued && mode !== 1 ? 2 : 1) || Boolean(wire[71]) !== (mode === 1)
            || (publicFee > 0n) !== (issued && mode === 1) || (mint > 0n) !== (mode === 1) || (exit > 0n) !== (mode === 2)
            || (issued && mode === 1 && fee !== 0n) || exit + fee > 0xffffffffffffffffn
            || wire.subarray(inner + 72, inner + 84).some(Boolean)
            || wire.subarray(inner + 84, inner + 104).some(Boolean) !== (mode === 2)
            || (wire[71] && (!wire.subarray(72, 92).some(Boolean) || wire.readBigUInt64LE(92) === 0n))) {
            throw refusal(400, 'private_action_malformed');
        }
        // The configured domain and pool are checked separately. Only the
        // signature, proof length and proof can differ on an idempotent retry.
        const statement = crypto.createHash('sha384').update(wire.subarray(0, prefix))
            .update(wire.subarray(inner, inner + 3644)).digest('hex');
        let statementId;
        if (domain) {
            const chain = Buffer.from(domain.chain_id, 'utf8');
            const length = Buffer.alloc(8), version = Buffer.alloc(4);
            length.writeBigUInt64LE(BigInt(chain.length));
            version.writeUInt32LE(domain.protocol_version);
            const raw = crypto.createHash('sha384').update('PFT-SH-TXCTX-V2')
                .update(wire.subarray(inner, inner + 3644)).digest();
            statementId = crypto.createHash('sha384').update('PFT-SH-L1-CONTEXT-V1')
                .update(length).update(chain).update(Buffer.from(domain.genesis_hash, 'hex'))
                .update(version).update(PQ_POOL).update(wire.subarray(0, prefix)).update(raw).digest('hex');
        }
        return { kind, value, canonical, poolId: PQ_POOL, statement, statementId, assetId: wire.subarray(7, 55).toString('hex'),
            payer: wire[71] ? 'pf' + wire.subarray(72, 92).toString('hex') : null };
    }
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
        : fleet.slice();
    // A record waits on each node until that node proposes, so submissions go to every
    // source; at least two are needed to reconcile.
    if (sources.length < 2 || new Set(sources.map((source) => source.validatorId)).size !== sources.length) {
        throw new Error('private actions need at least two distinct validator RPC sources');
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
    for (const sub of ['intents', 'by-action', 'by-batch', 'by-statement', 'capabilities']) fs.mkdirSync(path.join(dir, sub), { recursive: true, mode: 0o700 });
    const fsyncDir = (directory) => {
        const fd = fs.openSync(directory, 'r');
        try { fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
    };
    function atomicWrite(file, value, createOnly = false) {
        const temporary = `${file}.${process.pid}.${crypto.randomBytes(6).toString('hex')}.tmp`;
        const fd = fs.openSync(temporary, 'wx', 0o600);
        try { fs.writeFileSync(fd, `${JSON.stringify(value)}\n`); fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
        if (createOnly) {
            try { fs.linkSync(temporary, file); }
            catch (err) { if (err.code !== 'EEXIST') throw err; }
            finally { fs.unlinkSync(temporary); }
        } else { fs.renameSync(temporary, file); }
        fsyncDir(path.dirname(file));
    }
    const recordFile = (intentId) => path.join(dir, 'intents', `${intentId}.json`);
    // Legacy payerless records have no wallet-held credential. Keep their
    // history, remove old owner labels, and refuse access through these routes.
    const legacy = new Set();
    for (const file of fs.readdirSync(path.join(dir, 'intents'))) {
        if (!/^[0-9a-f]{96}\.json$/.test(file)) continue;
        const target = path.join(dir, 'intents', file);
        const record = JSON.parse(fs.readFileSync(target, 'utf8'));
        if (record.kind !== 'pq_payment_v1' || record.capability_hash) continue;
        const action = JSON.parse(record.action_json);
        if (Buffer.from(action.body, 'base64')[71] !== 0) continue;
        legacy.add(record.intent_id);
        if (Object.hasOwn(record, 'principal') || record.address || Object.hasOwn(record, 'session_scope')) {
            delete record.principal;
            delete record.session_scope;
            record.address = null;
            atomicWrite(target, record);
        }
    }
    const owners = path.join(dir, 'by-statement');
    if (legacy.size) {
        for (const file of fs.readdirSync(owners).filter(name => HEX96.test(name))) {
            const target = path.join(owners, file);
            if (legacy.has(JSON.parse(fs.readFileSync(target, 'utf8')).intent_id)) fs.unlinkSync(target);
        }
        fsyncDir(owners);
    }
    return {
        bindCapability(intent, hash) {
            atomicWrite(path.join(dir, 'capabilities', intent), { capability_hash:hash }, true);
            this.checkCapability(intent, hash);
        },
        checkCapability(intent, hash) {
            let saved;
            try { saved = JSON.parse(fs.readFileSync(path.join(dir, 'capabilities', intent), 'utf8')); }
            catch (err) { if (err.code === 'ENOENT') throw refusal(403, 'private_capability_missing'); throw err; }
            if (saved.capability_hash !== hash) throw refusal(403, 'private_capability_invalid');
        },
        read(intentId) {
            try { return JSON.parse(fs.readFileSync(recordFile(intentId), 'utf8')); } catch (error) {
                if (error.code === 'ENOENT') return null;
                throw error;
            }
        },
        pqOwner(key) {
            try { return JSON.parse(fs.readFileSync(path.join(dir, 'by-statement', key), 'utf8')).intent_id; }
            catch (error) { if (error.code === 'ENOENT') return null; throw error; }
        },
        rememberPqOwner(key, intentId) { atomicWrite(path.join(dir, 'by-statement', key), { intent_id: intentId }); },
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

function createPrivateActions({ env = process.env, fleet = [], rpcRequest, prefix = ROUTE_PREFIX } = {}) {
    const config = loadConfig(env, fleet);
    if (!config.enabled) return { enabled: false, handleHttp: async (req, res, url) => disabledRoute(res, url, prefix) };
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
            const code = String(response?.error?.code || 'rpc_error').slice(0, 64);
            return { source: source.validatorId, ok: false, final: FINAL_REFUSALS.has(code), code };
        } catch (error) {
            // Transport errors reserve nothing that a resend could duplicate: the node returns
            // the existing record for an intent it already holds.
            // Only a class from the closed list reaches the caller: never an
            // upstream address, response body, exception message or raw system code.
            return { source: source.validatorId, ok: false, final: false, code: rpcTransportErrorClass(error) };
        }
    }
    const allSources = (method, params, sources = config.sources) => Promise.all(sources.map((source) => callSource(source, method, params)));
    const bindingsMatch = (result, record) => result?.intent_id === record.intent_id
        && result.action_hash === record.action_hash && result.batch_id === record.batch_id;

    function validateRequest(body, principal) {
        const fields = ['address', 'intent_id', 'action_json'];
        if (!exactKeys(body, fields) && !exactKeys(body, [...fields, 'capability'])) throw refusal(400, 'private_request_invalid');
        const { address, intent_id: intentId, action_json: actionJson } = body;
        if (!HEX96.test(String(intentId)) || typeof actionJson !== 'string') {
            throw refusal(400, 'private_request_invalid');
        }
        if (Buffer.byteLength(actionJson, 'utf8') > MAX_ACTION_BYTES) throw refusal(413, 'private_action_too_large');
        const action = parsePublicAction(actionJson);
        const payerless = action.kind === 'pq_payment_v1' && !action.payer;
        if (payerless && address !== null) throw refusal(403, 'private_action_address_mismatch');
        if (payerless ? typeof body.capability !== 'string' || !/^[0-9a-f]{64}$/.test(body.capability)
                : Object.hasOwn(body, 'capability')) throw refusal(403, 'private_capability_invalid');
        const capabilityHash = payerless ? sha3Hex('postfiat.gateway.capability.v1', body.capability) : undefined;
        if (!payerless && !ADDRESS_RE.test(String(address))) {
            throw refusal(400, 'private_request_invalid');
        }
        if (intentIdFor(config.domain, actionJson) !== intentId) throw refusal(400, 'private_intent_mismatch');
        if (action.kind === 'asset_orchard_ingress_v2') {
            const unsigned = action.value.burn_transaction.unsigned;
            if (unsigned.chain_id !== config.domain.chain_id || unsigned.genesis_hash !== config.domain.genesis_hash
                || unsigned.protocol_version !== config.domain.protocol_version) {
                throw refusal(403, 'private_action_domain_mismatch');
            }
            if (unsigned.source !== address || unsigned.owner !== address) throw refusal(403, 'private_action_address_mismatch');
        }
        if (action.payer && action.payer !== address) throw refusal(403, 'private_action_address_mismatch');
        if ((action.kind === 'pq_payment_v1') !== (config.poolId === PQ_POOL)) throw refusal(403, 'private_action_pool_mismatch');
        if (action.poolId !== config.poolId) throw refusal(403, 'private_action_pool_mismatch');
        if (prefix !== ROUTE_PREFIX && action.kind === 'pq_payment_v1'
            && action.assetId !== '00'.repeat(48) && action.assetId !== PQ_REHEARSAL_ISSUE) {
            throw refusal(403, 'private_action_asset_refused');
        }
        const allowed = [config.baseAssetId, config.quoteAssetId].filter(Boolean);
        const assetIds = action.kind === 'shielded_swap_v1' ? [] : [action.assetId || action.value.asset_id];
        if (allowed.length && assetIds.some((id) => !allowed.includes(id))) throw refusal(403, 'private_action_asset_refused');
        return {
            address, principal: payerless ? undefined : principal, capabilityHash,
            scope: payerless ? PRIVATE_SPEND_SCOPE + ':' + capabilityHash : principal,
            intentId, actionJson, kind: action.kind,
            canonical: action.canonical, statement: action.statement,
            actionHash: sha3Hex(ACTION_DOMAIN, action.canonical),
            batchId: batchIdFor(config.domain, action.canonical),
        };
    }

    function checkOwner(record, request) {
        if (record.action_json !== request.actionJson) throw refusal(409, 'private_intent_conflict');
        if (request.capabilityHash) {
            if (!record.capability_hash) throw refusal(403, 'private_legacy_record');
            if (record.capability_hash !== request.capabilityHash) throw refusal(403, 'private_capability_invalid');
            return;
        }
        if (record.principal !== request.principal || record.address !== request.address) throw refusal(403, 'private_session_mismatch');
    }

    const submitResponse = (record) => ({
        ...(prefix !== ROUTE_PREFIX && record.kind === 'pq_payment_v1'
            ? { statement_id: parsePublicAction(record.action_json, config.domain).statementId } : {}),
        schema: SCHEMA, intent_id: record.intent_id, state: record.state, batch_id: record.batch_id, action_hash: record.action_hash,
    });

    const nodeDomain = (record) => ({
        chain_id: record.domain.chain_id, genesis_hash: record.domain.genesis_hash,
        protocol_version: record.domain.protocol_version, pool_id: record.pool_id, session_scope: record.session_scope ?? record.principal,
    });

    function nodeRecordMatches(report, record) {
        const node = report?.record;
        return report?.schema === NODE_RECORD_SCHEMA && node?.schema === NODE_RECORD_SCHEMA
            && bindingsMatch(node, record) && node.action_json === record.action_json
            // Field by field: rpc-serve returns objects with sorted keys.
            && Object.entries(nodeDomain(record)).every(([key, value]) => node.domain?.[key] === value)
            && Object.keys(node.domain || {}).length === 5 && NODE_STATES.has(node.state);
    }

    // One gateway state from the per-source views.
    function aggregate(views) {
        const entries = config.sources.map((source) => views[source.validatorId]);
        if (entries.some((entry) => entry?.mismatch)) return { state: 'unknown', code: 'private_action_source_mismatch' };
        const reserved = entries.filter((entry) => entry?.acknowledged);
        if (reserved.length === 0) {
            if (entries.every((entry) => entry?.final && entry.code === entries[0].code)) return { state: 'rejected', code: entries[0].code };
            return { state: 'queued', code: entries.find((entry) => entry?.code)?.code || null };
        }
        const states = new Set(reserved.map((entry) => entry.state));
        if (entries.some((entry) => entry?.final) || (states.has('accepted') && states.has('rejected'))) {
            return { state: 'unknown', code: 'private_action_source_mismatch' };
        }
        if (states.size === 1 && reserved.length === entries.length) return { state: [...states][0], code: null };
        if (states.has('unknown')) return { state: 'unknown', code: null };
        if (states.has('accepted') || states.has('rejected') || states.has('dispatched')) return { state: 'dispatched', code: null };
        return { state: 'queued', code: null };
    }

    const unreservedSources = (record) => config.sources
        .filter((source) => !(record.acknowledged_by || []).includes(source.validatorId));

    // Sends the reserved bytes, never the request's, to every source that does not hold them yet.
    async function dispatch(intentId) {
        if (inflight.has(intentId)) return inflight.get(intentId);
        const run = (async () => {
            const record = store.read(intentId);
            const targets = unreservedSources(record);
            const results = await allSources('private_action_submit', {
                ...nodeDomain(record), intent_id: record.intent_id, action_json: record.action_json, action_hash: record.action_hash,
            }, targets);
            return serialized(() => {
                const current = store.read(intentId);
                const views = { ...(current.sources || {}) };
                for (const result of results) {
                    if (result.ok) {
                        views[result.source] = nodeRecordMatches(result.result, current)
                            ? { acknowledged: true, state: result.result.record.state, existing: result.result.existing === true }
                            : { mismatch: true };
                    } else {
                        views[result.source] = { final: result.final, code: result.code };
                    }
                }
                const { state, code } = aggregate(views);
                const acknowledgedBy = Object.keys(views).filter((id) => views[id].acknowledged);
                const updated = {
                    ...current,
                    state,
                    code,
                    sources: views,
                    acknowledged_by: acknowledgedBy,
                    acknowledged: acknowledgedBy.length > 0,
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

    const pqOwnerKey = (request) => sha3Hex('postfiat.gateway.pq-receipt.v1', JSON.stringify([
        config.domain, config.poolId, request.address, request.principal, request.statement,
    ]));

    function authorize(body) {
        if (config.poolId !== PQ_POOL || !exactKeys(body, ['intent_id', 'capability_hash'])
            || !HEX96.test(body.intent_id) || !HEX96.test(body.capability_hash))
            throw refusal(400, 'private_request_invalid');
        const prior = store.read(body.intent_id);
        if (prior && !prior.capability_hash) throw refusal(403, 'private_legacy_record');
        if (prior && prior.capability_hash !== body.capability_hash) throw refusal(403, 'private_capability_invalid');
        store.bindCapability(body.intent_id, body.capability_hash);
        return {schema:SCHEMA, intent_id:body.intent_id, capability_hash:body.capability_hash};
    }

    async function submit(body, principal) {
        let request = validateRequest(body, principal);
        // Bind this exact intent before a submit or a concurrent retry can reach a node.
        const previous = store.read(request.intentId);
        if (previous) checkOwner(previous, request);
        if (request.capabilityHash && !previous) store.checkCapability(request.intentId, request.capabilityHash);
        let authoritative;
        if (request.kind === 'pq_payment_v1') {
            // Let one authoritative admission transaction select the retry's
            // original intent before forwarding those exact bytes to peers.
            const result = await callSource(config.sources[0], 'private_action_submit', {
                ...config.domain, pool_id: PQ_POOL, session_scope: request.scope, intent_id: request.intentId,
                action_hash: request.actionHash, action_json: request.actionJson,
            });
            if (!result.ok) {
                // A retry can hit a changed fee after a lost acknowledgement. Only
                // report a refusal hint when every configured peer reports a miss.
                if (/^pq_[a-z0-9_]+$/.test(result.code) || result.final
                    || ['private_action_queue_full', 'private_action_stale_tip'].includes(result.code)) {
                    const receipts = await allSources('private_action_receipt', {intent_id: request.intentId});
                    if (receipts.every(r => r.ok && r.result?.schema === NODE_RECEIPT_SCHEMA
                        && r.result.intent_id === request.intentId && r.result.state === 'pending_unknown')) {
                        throw Object.assign(refusal(409, result.code), { unreserved: {
                            reserved: false, intent_id: request.intentId, action_hash: request.actionHash,
                        } });
                    }
                }
                throw refusal(503, result.code);
            }
            const owner = result.result?.record;
            if (result.result?.schema !== NODE_RECORD_SCHEMA || owner?.schema !== NODE_RECORD_SCHEMA || !NODE_STATES.has(owner.state)
                || owner.domain?.session_scope !== request.scope || owner.domain.pool_id !== PQ_POOL
                || !Object.entries(config.domain).every(([k, v]) => owner.domain[k] === v)) throw refusal(502, 'private_action_source_mismatch');
            if (request.capabilityHash && (owner.intent_id !== request.intentId || owner.action_json !== request.actionJson))
                throw refusal(409, 'private_intent_conflict');
            const original = validateRequest({ ...body, intent_id: owner.intent_id, action_json: owner.action_json }, principal);
            if (original.statement !== request.statement || owner.action_hash !== original.actionHash || owner.batch_id !== original.batchId)
                throw refusal(502, 'private_action_source_mismatch');
            request = original;
            authoritative = result.result;
        }
        const reserved = await serialized(() => {
            const existing = store.read(request.intentId);
            if (existing) {
                checkOwner(existing, request);
                if (existing.state === 'abandoned') {
                    // The wallet's explicit resubmit of the same bytes after an operator
                    // abandonment: the record goes back to queued and is sent again to the
                    // sources that abandoned it. Never done on the gateway's own initiative.
                    const resubmitted = {
                        ...existing,
                        state: 'queued',
                        code: null,
                        abandoned_history: [...(existing.abandoned_history || []),
                            { reason: existing.abandon_reason ?? null, observed_at_ms: existing.updated_at_ms }],
                        abandon_reason: undefined,
                        updated_at_ms: Date.now(),
                    };
                    store.write(resubmitted);
                    return { record: resubmitted, fresh: false };
                }
                return { record: existing, fresh: false };
            }
            const claims = request.kind === 'pq_payment_v1' ? [] : [['by-action', request.actionHash], ['by-batch', request.batchId]];
            for (const [index, key] of claims) {
                if (store.claim(index, key, request.intentId) !== request.intentId) throw refusal(409, 'private_action_duplicate');
            }
            const record = {
                schema: RECORD_SCHEMA, intent_id: request.intentId, domain: config.domain, address: request.address,
                principal: request.principal, capability_hash: request.capabilityHash, session_scope: request.scope, pool_id: config.poolId, kind: request.kind, action_json: request.actionJson,
                action_hash: request.actionHash, batch_id: request.batchId,
                batch_json: `{"batch_id":${JSON.stringify(request.batchId)},"atomic":true,"actions":[${request.canonical}]}`,
                state: 'queued', code: null, acknowledged: false, created_at_ms: Date.now(), updated_at_ms: Date.now(),
            };
            if (authoritative) {
                if (!nodeRecordMatches(authoritative, record)) throw refusal(502, 'private_action_source_mismatch');
                const first = config.sources[0].validatorId;
                record.sources = { [first]: { acknowledged: true, state: authoritative.record.state,
                    existing: authoritative.existing === true } };
                record.acknowledged_by = [first];
                record.acknowledged = true;
                Object.assign(record, aggregate(record.sources));
            }
            store.write(record);
            return { record, fresh: true };
        });
        if (request.kind === 'pq_payment_v1' && !request.capabilityHash) store.rememberPqOwner(pqOwnerKey(request), request.intentId);
        if (env[FAULT_ENV] === 'after-reservation') process.kill(process.pid, 'SIGKILL');
        // The identical reserved bytes go only to sources that do not hold them yet; a refused,
        // mismatched or fully reserved record is returned as it stands.
        const current = reserved.record;
        if (current.state === 'rejected' || current.code === 'private_action_source_mismatch'
            || unreservedSources(current).length === 0) {
            return submitResponse(current);
        }
        return submitResponse(await dispatch(request.intentId));
    }

    function finality(value) {
        if (!exactKeys(value, FINALITY_FIELDS) || value.kind !== 'validator_quorum_block_certificate_v1'
            || !HEX96.test(String(value.block_hash)) || !HEX96.test(String(value.state_root))
            || typeof value.certificate_json !== 'string') return null;
        return value;
    }

    async function receipt(body, principal) {
        let request = validateRequest(body, principal);
        if (request.capabilityHash && !store.read(request.intentId)) throw refusal(404, 'private_receipt_missing');
        if (request.kind === 'pq_payment_v1' && !request.capabilityHash) {
            const intentId = store.pqOwner(pqOwnerKey(request));
            if (intentId && intentId !== request.intentId) {
                const original = store.read(intentId);
                if (!original || original.principal !== request.principal || original.address !== request.address)
                    throw refusal(403, 'private_session_mismatch');
                const bound = validateRequest({ address: request.address, intent_id: intentId, action_json: original.action_json }, principal);
                if (bound.statement !== request.statement) throw refusal(502, 'private_action_source_mismatch');
                request = bound;
            }
        }
        const record = store.read(request.intentId);
        if (record) checkOwner(record, request);
        const expected = { intent_id: request.intentId, action_hash: request.actionHash, batch_id: request.batchId };
        const pending = (state, code = null) => ({
            schema: SCHEMA, ...expected, state, transaction_id: null, inclusion_height: null, block_hash: null,
            state_root: null, accepted: null, code, fee: null, finality: null,
        });
        const localState = record ? record.state : 'unknown';
        const results = await allSources('private_action_receipt', { intent_id: request.intentId });
        const views = [];
        const abandonedBy = [];
        for (const result of results) {
            if (!result.ok) continue;
            const report = result.result;
            if (report?.schema !== NODE_RECEIPT_SCHEMA || report.intent_id !== request.intentId) {
                return pending('unknown', 'private_action_source_mismatch');
            }
            if (report.state === 'pending_unknown') continue;
            if (request.kind === 'pq_payment_v1' && (report.record?.domain?.session_scope !== request.scope
                || report.record.domain.pool_id !== config.poolId || report.record.action_json !== request.actionJson
                || !Object.entries(config.domain).every(([k, v]) => report.record.domain[k] === v)))
                throw refusal(403, 'private_session_mismatch');
            const contract = report.contract_receipt;
            if (contract?.schema !== SCHEMA || !bindingsMatch(contract, expected) || contract.state !== report.state
                || !NODE_STATES.has(contract.state)) {
                return pending('unknown', 'private_action_source_mismatch');
            }
            views.push(contract);
            if (contract.state === 'abandoned') abandonedBy.push(result.source);
        }
        const outcome = (c) => JSON.stringify([c.state, c.transaction_id, c.inclusion_height, c.block_hash, c.state_root,
            c.accepted, c.code, c.fee, c.finality]);
        const finals = views.filter((c) => c.state === 'accepted' || c.state === 'rejected');
        if (finals.some((c) => outcome(c) !== outcome(finals[0]))) return pending('unknown', 'private_action_source_mismatch');
        if (finals.length === 1) return pending(localState, 'private_receipt_unconfirmed');
        if (finals.length >= 2) {
            const first = finals[0];
            const included = first.inclusion_height !== null && first.inclusion_height !== undefined;
            const evidence = included ? finality(first.finality) : null;
            // A rejection before any round has no inclusion and no state effect; anything
            // included must carry finality for exactly that block.
            const consistent = first.accepted === (first.state === 'accepted') && (included
                ? evidence && evidence.height === first.inclusion_height && evidence.block_hash === first.block_hash
                    && evidence.state_root === first.state_root
                : first.state === 'rejected' && first.finality === null);
            if (!consistent) return pending('unknown', 'private_action_source_mismatch');
            return {
                schema: SCHEMA, ...expected, state: first.state, transaction_id: first.transaction_id ?? null,
                inclusion_height: first.inclusion_height ?? null, block_hash: first.block_hash ?? null,
                state_root: first.state_root ?? null, accepted: first.accepted, code: first.code ?? null,
                fee: first.fee, finality: evidence,
            };
        }
        // Abandoned by an operator on a node: terminal until the wallet resubmits the same
        // bytes (a resubmit cannot double spend: it is the same batch). The gateway notes it
        // so that resubmit reaches those sources, and never resends on its own.
        if (abandonedBy.length) {
            const reason = views.find((c) => c.state === 'abandoned').abandon_reason ?? null;
            if (record) {
                await serialized(() => {
                    const current = store.read(request.intentId);
                    if (['accepted', 'rejected'].includes(current.state)) return;
                    const acknowledgedBy = (current.acknowledged_by || []).filter((id) => !abandonedBy.includes(id));
                    const sources = { ...(current.sources || {}) };
                    for (const id of abandonedBy) sources[id] = { abandoned: true, code: 'private_action_abandoned' };
                    store.write({
                        ...current, state: 'abandoned', code: 'private_action_abandoned', abandon_reason: reason,
                        sources, acknowledged_by: acknowledgedBy, acknowledged: acknowledgedBy.length > 0,
                        updated_at_ms: Date.now(),
                    });
                });
            }
            return { ...pending('abandoned', 'private_action_abandoned'), abandon_reason: reason };
        }
        if (views.some((c) => c.state === 'unknown')) return pending('unknown', views.find((c) => c.state === 'unknown').code ?? null);
        if (views.some((c) => c.state === 'dispatched')) return pending('dispatched', null);
        return pending(views.length ? 'queued' : localState, record?.code || null);
    }

    async function readSource(source, method, params) {
        const result = await callSource(source, method, params);
        if (!result.ok) throw refusal(503, 'private_snapshot_unavailable', `${method} failed on ${result.source}: ${result.code}`);
        return result.result;
    }

    // One finalized snapshot from both sources; a mixed tip is refused.
    async function pinSnapshot(address) {
        const params = config.poolId === PQ_POOL ? { pool_id: PQ_POOL }
            : { pool_id: config.poolId, address, base_asset_id: config.baseAssetId, quote_asset_id: config.quoteAssetId };
        const reports = await Promise.all(config.sources.map((source) => readSource(source, 'private_pool_snapshot', params)));
        for (const report of reports) {
            if (report?.chain_id !== config.domain.chain_id || report.genesis_hash !== config.domain.genesis_hash
                || report.protocol_version !== config.domain.protocol_version) {
                throw refusal(503, 'private_snapshot_domain_mismatch');
            }
        }
        const pinned = (report) => JSON.stringify([report.snapshot_token, report.finalized_height,
            report.finalized_block_hash, report.finalized_state_root, report.counts, report.inclusion_height, report.pq_issued_fee]);
        if (!reports[0].snapshot_token || reports.some((report) => pinned(report) !== pinned(reports[0]))) {
            throw refusal(503, 'private_snapshot_sources_diverged');
        }
        return reports[0];
    }

    class SnapshotMoved extends Error {}

    // The node's stale-token refusals (110997f1): the tip moved past the token
    // (`snapshot_unavailable`), the read needs the current tip
    // (`snapshot_not_current`), or the token left the retained points
    // (`snapshot_expired`). Each one restarts the scan from a fresh token.
    const STALE_TOKEN_CODES = new Set(['snapshot_unavailable', 'snapshot_not_current', 'snapshot_expired']);

    // Pages one section from the first source under a client ceiling; every page names the token.
    async function pageSection(report, section, accept) {
        const items = [];
        let cursor = null;
        let total = 0;
        let reachedEnd = false;
        for (;;) {
            const params = { snapshot_token: report.snapshot_token, section };
            if (cursor) params.cursor = cursor;
            const page = await readSource(config.sources[0], 'private_pool_page', params);
            if (page?.status !== 'ok') {
                if (STALE_TOKEN_CODES.has(page?.refusal?.code)) throw new SnapshotMoved();
                throw refusal(503, 'private_snapshot_page_refused', String(page?.refusal?.code || 'page_refused'));
            }
            if (page.snapshot_token !== report.snapshot_token || page.section !== section || page.offset !== items.length) {
                throw refusal(503, 'private_snapshot_page_mismatch');
            }
            total = page.total;
            let stopped = false;
            for (const item of page.items || []) {
                if (!accept(item, items)) { stopped = true; break; }
                items.push(item);
            }
            if (stopped) return { items, total, reachedEnd: false, cursor: null, truncated: true };
            if (!page.next_cursor) { reachedEnd = true; break; }
            cursor = page.next_cursor;
        }
        return { items, total, reachedEnd: reachedEnd && items.length === total, cursor: null, truncated: false };
    }

    async function assembleSnapshot(address) {
        const report = await pinSnapshot(address);
        const node = report.completeness || {};
        // Account lines are served only for the current tip, so they are read
        // first; pages, assets and receipts are served from retained points.
        const lines = await readSource(config.sources[0], 'private_account_lines', { snapshot_token: report.snapshot_token, address });
        if (lines?.status !== 'ok') {
            if (STALE_TOKEN_CODES.has(lines?.refusal?.code)) throw new SnapshotMoved();
            throw refusal(503, 'private_snapshot_lines_refused', String(lines?.refusal?.code || 'lines_refused'));
        }
        const listCeiling = (limit) => (item, items) => items.length < limit;
        const commitments = await pageSection(report, 'commitments', listCeiling(MAX_SNAPSHOT_LIST));
        const nullifiers = await pageSection(report, 'nullifiers', listCeiling(MAX_SNAPSHOT_LIST));
        const anchors = await pageSection(report, 'accepted_anchors', listCeiling(MAX_SNAPSHOT_LIST));
        const assets = await pageSection(report, 'assets', listCeiling(MAX_SNAPSHOT_ASSETS));
        const assetRows = assets.items.filter((row) => row.asset_info_bound && row.issuer && row.code
            && Number.isInteger(row.precision) && typeof row.requires_authorization === 'boolean')
            .map((row) => ({
                asset_id: row.asset_id, issuer: row.issuer, code: row.code, precision: row.precision,
                requires_authorization: row.requires_authorization, proof_profile_id: row.proof_profile_id ?? null,
            }));
        // A line for a listed asset must agree with its asset row and be usable.
        const linesAgree = (lines.lines || []).every((line) => {
            const row = assetRows.find((candidate) => candidate.asset_id === line.asset_id);
            return !row || (line.line_status === 'ok' && line.issuer === row.issuer && line.code === row.code
                && line.precision === row.precision && line.requires_authorization === row.requires_authorization);
        });
        const response = {
            schema: SCHEMA, domain: config.domain, address,
            snapshot_token: { height: report.finalized_height, block_hash: report.finalized_block_hash, state_root: report.finalized_state_root },
            commitments: commitments.items,
            nullifiers: nullifiers.items,
            accepted_anchors: anchors.items.map((row) => row.root),
            outputs: [],
            assets: assetRows,
            base_asset_id: report.base_asset_id || config.baseAssetId,
            quote_asset_id: report.quote_asset_id || config.quoteAssetId,
            pricing_claim_json: report.pricing_claim_json ?? null,
            policy_id: report.policy_id ?? null,
            disclosure_hash: report.disclosure_hash ?? null,
            complete: {
                commitments: commitments.reachedEnd && node.history_complete === true,
                nullifiers: nullifiers.reachedEnd && node.history_complete === true,
                accepted_anchors: anchors.reachedEnd && node.anchors_from_retained_prefixes === true,
                outputs: false,
                assets: assets.reachedEnd && assetRows.length === assets.items.length
                    && lines.account_assets_consistent === true && linesAgree,
            },
            cursors: { commitments: null, nullifiers: null, accepted_anchors: null, outputs: null },
        };
        // Outputs last: they fill what the 4 MiB response, 64-ciphertext and 2 MiB ceilings leave.
        let budget = MAX_RESPONSE_BYTES - Buffer.byteLength(JSON.stringify(response)) - 1024;
        let ciphertextBytes = 0;
        let missingCiphertext = false;
        const outputs = await pageSection(report, 'outputs', (row, items) => {
            if (typeof row.ciphertext !== 'string') { missingCiphertext = true; return true; }
            const size = Buffer.byteLength(JSON.stringify({ commitment: row.commitment, ciphertext: row.ciphertext })) + 1;
            if (items.length >= MAX_SNAPSHOT_OUTPUTS || ciphertextBytes + row.ciphertext.length / 2 > MAX_SNAPSHOT_CIPHERTEXT_BYTES
                || size > budget) return false;
            ciphertextBytes += row.ciphertext.length / 2;
            budget -= size;
            return true;
        });
        response.outputs = outputs.items.filter((row) => typeof row.ciphertext === 'string')
            .map((row) => ({ commitment: row.commitment, ciphertext: row.ciphertext }));
        response.complete.outputs = outputs.reachedEnd && !missingCiphertext && node.outputs_complete === true;
        return response;
    }

    async function pqPage(body) {
        if (config.poolId !== PQ_POOL || !exactKeys(body, ['snapshot_token', 'section', 'cursor', 'limit'])
            || !['pq_pairs', 'pq_assets'].includes(body.section)
            || typeof body.snapshot_token !== 'string' || body.snapshot_token.length > 320
            || !body.snapshot_token.startsWith(`pps1:${PQ_POOL}:`)
            || !(body.cursor === null || (typeof body.cursor === 'string' && body.cursor.length <= 128))
            || !Number.isInteger(body.limit) || body.limit < 1 || body.limit > 32) {
            throw refusal(400, 'private_request_invalid');
        }
        const params = { ...body };
        if (params.cursor === null) delete params.cursor;
        return readSource(config.sources[0], 'private_pool_page', params);
    }

    async function snapshot(body) {
        if (config.poolId === PQ_POOL) {
            if (!exactKeys(body, [])) throw refusal(400, 'private_request_invalid');
            const report = await pinSnapshot();
            if (report.pool_id !== PQ_POOL) throw refusal(503, 'private_snapshot_domain_mismatch');
            return report;
        }
        if (!exactKeys(body, ['address']) || !ADDRESS_RE.test(String(body.address))) throw refusal(400, 'private_request_invalid');
        if (!config.baseAssetId || !config.quoteAssetId) throw refusal(503, 'private_snapshot_not_configured');
        // A stale token (any STALE_TOKEN_CODES refusal): one fresh retry, then 503.
        for (let attempt = 0; attempt < 2; attempt += 1) {
            try {
                return await assembleSnapshot(body.address);
            } catch (error) {
                if (!(error instanceof SnapshotMoved)) throw error;
            }
        }
        throw refusal(503, 'private_snapshot_unavailable', 'the finalized tip moved twice while paging');
    }

    async function handleHttp(req, res, url, auth) {
        if (!url.pathname.startsWith(prefix)) return false;
        const route = url.pathname.slice(prefix.length);
        if (!['snapshot', 'submit', 'receipt'].includes(route) && !(['page','authorize'].includes(route) && config.poolId === PQ_POOL)) return send(res, 404, error('private_route_unknown'));
        if (req.method !== 'POST') return send(res, 405, error('method_not_allowed'));
        if (!auth.originAllowed(req, true)) return send(res, 403, error('origin_not_allowed'));
        const authentication = auth.authenticate(req);
        if (!authentication?.principal) {
            return send(res, authentication?.failure?.statusCode || 401, error(authentication?.failure?.code || 'proxy_auth_required'));
        }
        let admission = null;
        if (route === 'submit' || route === 'authorize') {
            admission = auth.admitMutation(authentication.principal);
            if (!admission.ok) return send(res, 429, { ...error(admission.code), retry_after_ms: admission.retry_after_ms });
        }
        try {
            const body = JSON.parse((await readBody(req, config.poolId === PQ_POOL && route === 'submit' ? MAX_ACTION_BYTES : MAX_REQUEST_BYTES)).toString('utf8'));
            if (route === 'authorize') return send(res, 200, authorize(body));
            if (route === 'page') return send(res, 200, await pqPage(body));
            if (route === 'snapshot') return send(res, 200, await snapshot(body));
            if (route === 'receipt') return send(res, 200, await receipt(body, authentication.principal));
            return send(res, 202, await submit(body, authentication.principal));
        } catch (caught) {
            if (caught instanceof SyntaxError) return send(res, 400, error('private_request_invalid'));
            if (caught.statusCode) return send(res, caught.statusCode, { ...error(caught.code, caught.message), ...caught.unreserved });
            return send(res, 500, error('private_actions_internal_error'));
        } finally {
            admission?.release?.();
        }
    }

    return { enabled: true, config, authorize, submit, receipt, snapshot, handleHttp, store };
}

function readBody(req, limit) {
    return new Promise((resolve, reject) => {
        if (Number(req.headers['content-length'] || 0) > limit) {
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
            if (size > limit) {
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

function disabledRoute(res, url, prefix) {
    if (!url.pathname.startsWith(prefix)) return false;
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
