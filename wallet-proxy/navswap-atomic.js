'use strict';

const crypto = require('crypto');

const ATOMIC_SETTLEMENT_MODE = 'atomic_swap_v1';
const ATOMIC_PROXY_ROUTABLE_TRANSPORT = Symbol.for(
    'postfiat.wallet-proxy.atomic-routable-transport.v1',
);
const ATOMIC_QUOTE_METHOD = 'atomic_swap_fee_quote';
const ATOMIC_FINALITY_METHOD = 'mempool_submit_signed_atomic_swap_transaction_finality';
const ATOMIC_RAW_SUBMIT_METHOD = 'mempool_submit_signed_atomic_swap_transaction';
const ATOMIC_QUOTE_SCHEMA = 'postfiat-navswap-atomic-quote-v1';
const ATOMIC_RUN_SCHEMA = 'postfiat-navswap-atomic-run-v1';
const ATOMIC_RPC_QUOTE_SCHEMA = 'postfiat-atomic-swap-fee-quote-v1';
const ATOMIC_RPC_FINALITY_SCHEMA = 'postfiat-rpc-mempool-submit-signed-atomic-swap-finality-v1';
const MAX_SIGNED_ATOMIC_SWAP_JSON_BYTES = 64 * 1024;

const ATOMIC_QUOTE_BODY_KEYS = new Set([
    'route',
    'settlement_mode',
    'request_id',
    'rfq_hash',
    'market_envelope_hash',
    'nav_epoch',
    'expires_at_height',
    'swap_nonce',
    'leg_0_owner',
    'leg_0_recipient',
    'leg_0_issuer',
    'leg_0_asset_id',
    'leg_0_amount',
    'leg_1_owner',
    'leg_1_recipient',
    'leg_1_issuer',
    'leg_1_asset_id',
    'leg_1_amount',
]);

const ATOMIC_QUOTE_PARAM_KEYS = new Set(
    [...ATOMIC_QUOTE_BODY_KEYS].filter((key) => !['route', 'settlement_mode', 'request_id'].includes(key)),
);

const ATOMIC_RUN_BODY_KEYS = new Set([
    'route',
    'settlement_mode',
    'idempotency_key',
    'request_id',
    'expected_tx_id',
    'signed_atomic_swap_transaction_json',
    'quote_binding',
    'proxy_readiness_timeout_ms',
]);

const ATOMIC_QUOTE_BINDING_KEYS = new Set([
    'parent_height',
    'parent_hash',
    'parent_state_root',
]);

const ATOMIC_FINALITY_PARAM_KEYS = new Set([
    'signed_atomic_swap_transaction_json',
    'proxy_required_current_height',
    'proxy_required_state_root',
    'proxy_required_parent_hash',
    'proxy_readiness_timeout_ms',
]);

const SIGNED_ATOMIC_SWAP_KEYS = new Set([
    'unsigned',
    'authorization_0',
    'authorization_1',
]);

const UNSIGNED_ATOMIC_SWAP_KEYS = new Set([
    'chain_id',
    'genesis_hash',
    'protocol_version',
    'address_namespace',
    'signature_algorithm_id',
    'rfq_hash',
    'market_envelope_hash',
    'nav_epoch',
    'expires_at_height',
    'swap_nonce',
    'leg_0',
    'leg_1',
]);

const ATOMIC_SWAP_LEG_KEYS = new Set([
    'owner',
    'recipient',
    'issuer',
    'asset_id',
    'amount',
    'sequence',
    'fee',
]);

const ATOMIC_SWAP_AUTHORIZATION_KEYS = new Set([
    'owner',
    'algorithm_id',
    'public_key_hex',
    'signature_hex',
]);

const ATOMIC_PRIVATE_KEY_PATTERNS = [
    /(^|_)backup(_json|_file|_path)?$/,
    /(^|_)decrypted_backup$/,
    /(^|_)key_(file|path)$/,
    /(^|_)(owner|buyer|seller|wallet|signer|issuer|subscriber)_key_(file|path|json)$/,
    /(^|_)manifest(_file|_path|_json)?$/,
    /(^|_)wallet_manifest$/,
    /(^|_)mnemonic$/,
    /(^|_)passphrase$/,
    /(^|_)private_key(_hex|_json)?$/,
    /(^|_)secret_key(_hex|_json)?$/,
    /(^|_)seed(_phrase|_hex)?$/,
    /(^|_)master_seed(_hex)?$/,
];

function atomicError(code, message, details = {}) {
    const error = new Error(message);
    error.code = code;
    Object.assign(error, details);
    return error;
}

function markAtomicProxyRoutableTransport(transport) {
    if (typeof transport !== 'function') {
        throw new TypeError('atomic proxy transport must be a function');
    }
    if (transport[ATOMIC_PROXY_ROUTABLE_TRANSPORT] !== true) {
        Object.defineProperty(transport, ATOMIC_PROXY_ROUTABLE_TRANSPORT, {
            value: true,
            configurable: false,
            enumerable: false,
            writable: false,
        });
    }
    return transport;
}

function isAtomicProxyRoutableTransport(transport) {
    return (
        typeof transport === 'function'
        && transport[ATOMIC_PROXY_ROUTABLE_TRANSPORT] === true
    );
}

function normalizedKey(key) {
    return String(key || '')
        .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
        .replace(/[^A-Za-z0-9]+/g, '_')
        .replace(/^_+|_+$/g, '')
        .toLowerCase();
}

function findAtomicPrivateMaterial(value, path = '$', seen = new WeakSet()) {
    if (!value || typeof value !== 'object') return [];
    if (seen.has(value)) return [];
    seen.add(value);
    if (Array.isArray(value)) {
        return value.flatMap((item, index) => findAtomicPrivateMaterial(item, `${path}[${index}]`, seen));
    }
    const hits = [];
    for (const [key, child] of Object.entries(value)) {
        const childPath = `${path}.${key}`;
        const normalized = normalizedKey(key);
        if (ATOMIC_PRIVATE_KEY_PATTERNS.some((pattern) => pattern.test(normalized))) {
            hits.push(childPath);
        }
        hits.push(...findAtomicPrivateMaterial(child, childPath, seen));
    }
    return hits;
}

function assertNoAtomicPrivateMaterial(value) {
    const hits = findAtomicPrivateMaterial(value);
    if (hits.length > 0) {
        throw atomicError(
            'atomic_navswap_private_material_rejected',
            `atomic NAVSwap request contains forbidden private wallet material at ${hits[0]}`,
        );
    }
}

function assertPlainObject(value, field) {
    if (!value || typeof value !== 'object' || Array.isArray(value)) {
        throw atomicError('atomic_navswap_invalid_request', `${field} must be an object`);
    }
    return value;
}

function assertOnlyKeys(value, allowed, field) {
    assertPlainObject(value, field);
    const unknown = Object.keys(value).filter((key) => !allowed.has(key));
    if (unknown.length > 0) {
        throw atomicError(
            'atomic_navswap_unknown_field',
            `${field} contains unsupported field ${unknown[0]}`,
        );
    }
}

function requiredString(value, field, maxLength = 4096) {
    if (typeof value !== 'string' || !value.trim()) {
        throw atomicError('atomic_navswap_invalid_request', `${field} must be a non-empty string`);
    }
    if (value.length > maxLength) {
        throw atomicError('atomic_navswap_invalid_request', `${field} exceeds ${maxLength} bytes`);
    }
    return value;
}

function requestId(value) {
    const text = requiredString(value, 'request_id', 128);
    if (!/^[A-Za-z0-9._:-]+$/.test(text)) {
        throw atomicError(
            'atomic_navswap_invalid_request_id',
            'request_id may contain only letters, digits, dot, underscore, colon, and hyphen',
        );
    }
    return text;
}

function lowerHex(value, length, field) {
    const text = requiredString(value, field, length);
    if (!new RegExp(`^[0-9a-f]{${length}}$`).test(text)) {
        throw atomicError(
            'atomic_navswap_invalid_request',
            `${field} must be ${length} lowercase hexadecimal characters`,
        );
    }
    return text;
}

function pftlAddress(value, field) {
    const text = requiredString(value, field, 42);
    if (!/^pf[0-9a-f]{40}$/.test(text)) {
        throw atomicError(
            'atomic_navswap_invalid_request',
            `${field} must be a lowercase PostFiat account address`,
        );
    }
    return text;
}

function safeU64(value, field, { nonzero = false } = {}) {
    const text = typeof value === 'number' ? String(value) : String(value ?? '').trim();
    if (!/^(0|[1-9][0-9]*)$/.test(text)) {
        throw atomicError('atomic_navswap_invalid_request', `${field} must be an unsigned integer`);
    }
    const parsed = Number(text);
    if (!Number.isSafeInteger(parsed) || parsed < 0 || (nonzero && parsed === 0)) {
        throw atomicError(
            'atomic_navswap_invalid_request',
            `${field} must be a ${nonzero ? 'positive ' : ''}wallet-safe unsigned integer`,
        );
    }
    return parsed;
}

function optionalPositiveU64(value, field) {
    if (value === undefined || value === null) return undefined;
    return safeU64(value, field, { nonzero: true });
}

function assertAtomicRoute(body) {
    if (body.route !== 'transparent_navswap') {
        throw atomicError(
            'atomic_navswap_route_invalid',
            'atomic_swap_v1 is available only through transparent_navswap',
        );
    }
    if (body.settlement_mode !== ATOMIC_SETTLEMENT_MODE) {
        throw atomicError(
            'atomic_navswap_settlement_mode_invalid',
            `settlement_mode must be ${ATOMIC_SETTLEMENT_MODE}`,
        );
    }
}

function isAtomicSettlementMode(body = {}) {
    return body?.settlement_mode === ATOMIC_SETTLEMENT_MODE;
}

function configuredAtomicAssetId(runtime, key) {
    try {
        return lowerHex(runtime?.[key], 96, `configured ${key}`);
    } catch (_) {
        throw atomicError(
            'atomic_navswap_configuration_invalid',
            `configured ${key} must be a 48-byte lowercase hex asset id`,
        );
    }
}

function stableAtomicJson(value) {
    if (Array.isArray(value)) return `[${value.map(stableAtomicJson).join(',')}]`;
    if (value && typeof value === 'object') {
        return `{${Object.keys(value).sort().map((key) => (
            `${JSON.stringify(key)}:${stableAtomicJson(value[key])}`
        )).join(',')}}`;
    }
    return JSON.stringify(value);
}

function atomicValueHash(value) {
    return crypto.createHash('sha256')
        .update(stableAtomicJson(value), 'utf8')
        .digest('hex');
}

function configuredAtomicRpcFleet(runtime) {
    const fleet = runtime?.RPC_FLEET;
    if (!Array.isArray(fleet) || fleet.length !== 6) {
        throw atomicError(
            'atomic_navswap_configuration_invalid',
            'atomic NAVSwap proxy requires exactly six configured RPC endpoints',
        );
    }
    const rpcFleet = fleet.map((endpoint) => ({
        node_id: requiredString(endpoint?.validatorId, 'RPC validator id', 64),
        host: requiredString(endpoint?.host, 'RPC host', 512),
        port: safeU64(endpoint?.port, 'RPC port', { nonzero: true }),
    })).sort((left, right) => left.node_id.localeCompare(right.node_id));
    const expectedNodeIds = Array.from({ length: 6 }, (_, index) => `validator-${index}`);
    if (
        rpcFleet.some((endpoint, index) => endpoint.node_id !== expectedNodeIds[index])
        || new Set(rpcFleet.map((endpoint) => `${endpoint.host}:${endpoint.port}`)).size !== 6
        || rpcFleet.some((endpoint) => endpoint.port > 65535)
    ) {
        throw atomicError(
            'atomic_navswap_configuration_invalid',
            'atomic NAVSwap proxy RPC endpoint authority is ambiguous',
        );
    }
    return rpcFleet;
}

function atomicProxyConfiguration(runtime, unsigned) {
    const configuration = {
        schema: 'postfiat-navswap-atomic-proxy-configuration-v1',
        chain_id: requiredString(unsigned?.chain_id, 'atomic chain_id', 128),
        genesis_hash: lowerHex(unsigned?.genesis_hash, 96, 'atomic genesis_hash'),
        protocol_version: safeU64(unsigned?.protocol_version, 'atomic protocol_version', { nonzero: true }),
        assets: {
            a651: configuredAtomicAssetId(runtime, 'A651_ASSET_ID'),
            pfusdc: configuredAtomicAssetId(runtime, 'PFUSDC_ASSET_ID'),
        },
        rpc_fleet: configuredAtomicRpcFleet(runtime),
    };
    return {
        configuration,
        configuration_hash: crypto.createHash('sha256')
            .update(stableAtomicJson(configuration), 'utf8')
            .digest('hex'),
    };
}

function assertConfiguredAtomicPair(runtime, leg0AssetId, leg1AssetId) {
    const pfusdcAssetId = configuredAtomicAssetId(runtime, 'PFUSDC_ASSET_ID');
    const a651AssetId = configuredAtomicAssetId(runtime, 'A651_ASSET_ID');
    if (pfusdcAssetId === a651AssetId) {
        throw atomicError(
            'atomic_navswap_configuration_invalid',
            'configured pfUSDC and a651 asset ids must differ',
        );
    }
    const observed0 = lowerHex(leg0AssetId, 96, 'leg_0_asset_id');
    const observed1 = lowerHex(leg1AssetId, 96, 'leg_1_asset_id');
    const configuredPair = (
        (observed0 === pfusdcAssetId && observed1 === a651AssetId)
        || (observed0 === a651AssetId && observed1 === pfusdcAssetId)
    );
    if (!configuredPair) {
        throw atomicError(
            'atomic_navswap_pair_not_supported',
            'atomic NAVSwap supports only the configured pfUSDC and a651 asset pair',
        );
    }
}

function normalizedAtomicQuoteParams(source, runtime = null) {
    assertPlainObject(source, 'atomic quote params');
    assertNoAtomicPrivateMaterial(source);
    assertOnlyKeys(source, ATOMIC_QUOTE_PARAM_KEYS, 'atomic quote params');
    const params = {
        rfq_hash: lowerHex(source.rfq_hash, 96, 'rfq_hash'),
        market_envelope_hash: lowerHex(source.market_envelope_hash, 96, 'market_envelope_hash'),
        nav_epoch: safeU64(source.nav_epoch, 'nav_epoch'),
        expires_at_height: safeU64(source.expires_at_height, 'expires_at_height', { nonzero: true }),
        swap_nonce: lowerHex(source.swap_nonce, 96, 'swap_nonce'),
        leg_0_owner: pftlAddress(source.leg_0_owner, 'leg_0_owner'),
        leg_0_recipient: pftlAddress(source.leg_0_recipient, 'leg_0_recipient'),
        leg_0_issuer: pftlAddress(source.leg_0_issuer, 'leg_0_issuer'),
        leg_0_asset_id: lowerHex(source.leg_0_asset_id, 96, 'leg_0_asset_id'),
        leg_0_amount: safeU64(source.leg_0_amount, 'leg_0_amount', { nonzero: true }),
        leg_1_owner: pftlAddress(source.leg_1_owner, 'leg_1_owner'),
        leg_1_recipient: pftlAddress(source.leg_1_recipient, 'leg_1_recipient'),
        leg_1_issuer: pftlAddress(source.leg_1_issuer, 'leg_1_issuer'),
        leg_1_asset_id: lowerHex(source.leg_1_asset_id, 96, 'leg_1_asset_id'),
        leg_1_amount: safeU64(source.leg_1_amount, 'leg_1_amount', { nonzero: true }),
    };
    if (params.leg_0_owner === params.leg_1_owner) {
        throw atomicError('atomic_navswap_invalid_request', 'atomic swap owners must differ');
    }
    if (
        params.leg_0_owner !== params.leg_1_recipient
        || params.leg_1_owner !== params.leg_0_recipient
    ) {
        throw atomicError('atomic_navswap_invalid_request', 'atomic swap legs must be reciprocal');
    }
    if (params.leg_0_asset_id === params.leg_1_asset_id) {
        throw atomicError('atomic_navswap_invalid_request', 'atomic swap assets must differ');
    }
    if (runtime) {
        assertConfiguredAtomicPair(runtime, params.leg_0_asset_id, params.leg_1_asset_id);
    }
    const order0 = `${params.leg_0_asset_id}:${params.leg_0_owner}`;
    const order1 = `${params.leg_1_asset_id}:${params.leg_1_owner}`;
    if (order0 >= order1) {
        throw atomicError(
            'atomic_navswap_invalid_request',
            'atomic swap legs must use canonical (asset_id, owner) ordering',
        );
    }
    return params;
}

function parseAtomicSignedTransaction(raw) {
    const text = requiredString(
        raw,
        'signed_atomic_swap_transaction_json',
        MAX_SIGNED_ATOMIC_SWAP_JSON_BYTES,
    );
    if (Buffer.byteLength(text, 'utf8') > MAX_SIGNED_ATOMIC_SWAP_JSON_BYTES) {
        throw atomicError(
            'atomic_navswap_invalid_request',
            `signed_atomic_swap_transaction_json exceeds ${MAX_SIGNED_ATOMIC_SWAP_JSON_BYTES} bytes`,
        );
    }
    let signed;
    try {
        signed = JSON.parse(text);
    } catch (_) {
        throw atomicError(
            'atomic_navswap_invalid_request',
            'signed_atomic_swap_transaction_json must contain valid JSON',
        );
    }
    assertNoAtomicPrivateMaterial(signed);
    assertOnlyKeys(signed, SIGNED_ATOMIC_SWAP_KEYS, 'signed atomic swap transaction');
    assertOnlyKeys(signed.unsigned, UNSIGNED_ATOMIC_SWAP_KEYS, 'signed atomic swap transaction.unsigned');
    assertOnlyKeys(signed.unsigned.leg_0, ATOMIC_SWAP_LEG_KEYS, 'signed atomic swap transaction.unsigned.leg_0');
    assertOnlyKeys(signed.unsigned.leg_1, ATOMIC_SWAP_LEG_KEYS, 'signed atomic swap transaction.unsigned.leg_1');
    assertOnlyKeys(
        signed.authorization_0,
        ATOMIC_SWAP_AUTHORIZATION_KEYS,
        'signed atomic swap transaction.authorization_0',
    );
    assertOnlyKeys(
        signed.authorization_1,
        ATOMIC_SWAP_AUTHORIZATION_KEYS,
        'signed atomic swap transaction.authorization_1',
    );
    return { text, signed };
}

function normalizedAtomicFinalityParams(source, runtime = null) {
    assertPlainObject(source, 'atomic finality params');
    assertNoAtomicPrivateMaterial(source);
    assertOnlyKeys(source, ATOMIC_FINALITY_PARAM_KEYS, 'atomic finality params');
    const { text, signed } = parseAtomicSignedTransaction(source.signed_atomic_swap_transaction_json);
    if (runtime) {
        assertConfiguredAtomicPair(
            runtime,
            signed.unsigned.leg_0.asset_id,
            signed.unsigned.leg_1.asset_id,
        );
    }
    const params = {
        signed_atomic_swap_transaction_json: text,
        proxy_required_current_height: safeU64(
            source.proxy_required_current_height,
            'proxy_required_current_height',
        ),
        proxy_required_state_root: lowerHex(
            source.proxy_required_state_root,
            96,
            'proxy_required_state_root',
        ),
        proxy_required_parent_hash: lowerHex(
            source.proxy_required_parent_hash,
            96,
            'proxy_required_parent_hash',
        ),
    };
    const timeout = optionalPositiveU64(source.proxy_readiness_timeout_ms, 'proxy_readiness_timeout_ms');
    if (timeout !== undefined) params.proxy_readiness_timeout_ms = timeout;
    return params;
}

function atomicRpcProxyError(request = {}, runtime = null) {
    if (request.method === ATOMIC_RAW_SUBMIT_METHOD) {
        return {
            code: 'proxy_atomic_swap_raw_submit_disabled',
            message: 'raw atomic swap submission is disabled; use certified finality submission once',
        };
    }
    if (![ATOMIC_QUOTE_METHOD, ATOMIC_FINALITY_METHOD].includes(request.method)) return null;
    try {
        requestId(request.id);
        if (runtime) configuredAtomicRpcFleet(runtime);
        if (request.method === ATOMIC_QUOTE_METHOD) {
            normalizedAtomicQuoteParams(request.params, runtime);
        } else {
            normalizedAtomicFinalityParams(request.params, runtime);
        }
        return null;
    } catch (error) {
        return {
            code: error.code || 'atomic_navswap_invalid_request',
            message: error.message || 'atomic NAVSwap RPC request is invalid',
        };
    }
}

function atomicResponseState(response) {
    if (response?.ok === true) return 'finalized';
    const code = String(response?.error?.code || '');
    if (code === 'rpc_finality_parent_stale') return 'terminal_stale';
    return 'failed';
}

function publicFailure(schema, body, error, extra = {}) {
    return {
        ok: false,
        schema,
        route: body?.route || 'transparent_navswap',
        settlement_mode: body?.settlement_mode || ATOMIC_SETTLEMENT_MODE,
        code: error.code || 'atomic_navswap_failed',
        message: error.message || 'atomic NAVSwap request failed',
        ...extra,
    };
}

function quoteResponseBinding(response, params, requestIdValue) {
    if (!response || response.id !== requestIdValue || response.ok !== true) {
        throw atomicError(
            'atomic_navswap_quote_response_mismatch',
            'atomic swap quote response does not match its request id or was not successful',
        );
    }
    const result = assertPlainObject(response.result, 'atomic quote response result');
    if (result.schema !== ATOMIC_RPC_QUOTE_SCHEMA || result.transaction_kind !== 'atomic_swap') {
        throw atomicError(
            'atomic_navswap_quote_response_mismatch',
            'atomic swap quote response has the wrong schema or transaction kind',
        );
    }
    const unsigned = assertPlainObject(result.unsigned_transaction, 'atomic quote unsigned_transaction');
    const requestBindings = {
        rfq_hash: params.rfq_hash,
        market_envelope_hash: params.market_envelope_hash,
        nav_epoch: params.nav_epoch,
        expires_at_height: params.expires_at_height,
        swap_nonce: params.swap_nonce,
        leg_0_owner: params.leg_0_owner,
        leg_0_recipient: params.leg_0_recipient,
        leg_0_issuer: params.leg_0_issuer,
        leg_0_asset_id: params.leg_0_asset_id,
        leg_0_amount: params.leg_0_amount,
        leg_1_owner: params.leg_1_owner,
        leg_1_recipient: params.leg_1_recipient,
        leg_1_issuer: params.leg_1_issuer,
        leg_1_asset_id: params.leg_1_asset_id,
        leg_1_amount: params.leg_1_amount,
    };
    const observedBindings = {
        rfq_hash: unsigned.rfq_hash,
        market_envelope_hash: unsigned.market_envelope_hash,
        nav_epoch: unsigned.nav_epoch,
        expires_at_height: unsigned.expires_at_height,
        swap_nonce: unsigned.swap_nonce,
        leg_0_owner: unsigned.leg_0?.owner,
        leg_0_recipient: unsigned.leg_0?.recipient,
        leg_0_issuer: unsigned.leg_0?.issuer,
        leg_0_asset_id: unsigned.leg_0?.asset_id,
        leg_0_amount: unsigned.leg_0?.amount,
        leg_1_owner: unsigned.leg_1?.owner,
        leg_1_recipient: unsigned.leg_1?.recipient,
        leg_1_issuer: unsigned.leg_1?.issuer,
        leg_1_asset_id: unsigned.leg_1?.asset_id,
        leg_1_amount: unsigned.leg_1?.amount,
    };
    for (const [field, expected] of Object.entries(requestBindings)) {
        if (observedBindings[field] !== expected) {
            throw atomicError(
                'atomic_navswap_quote_response_mismatch',
                `atomic swap quote response field ${field} does not match its request`,
            );
        }
    }
    const parentHeight = safeU64(result.parent_height, 'quote result parent_height');
    const quoteHeight = safeU64(result.quote_height, 'quote result quote_height', { nonzero: true });
    if (parentHeight + 1 !== quoteHeight) {
        throw atomicError(
            'atomic_navswap_quote_response_mismatch',
            'atomic swap quote height does not immediately follow its parent height',
        );
    }
    return {
        parent_height: parentHeight,
        parent_hash: lowerHex(result.parent_hash, 96, 'quote result parent_hash'),
        parent_state_root: lowerHex(
            result.parent_state_root,
            96,
            'quote result parent_state_root',
        ),
    };
}

function finalityResponseBinding(response, signed, quoteBinding, requestIdValue, expectedTxId) {
    if (!response || response.id !== requestIdValue || response.ok !== true) {
        throw atomicError(
            'atomic_navswap_finality_response_mismatch',
            'atomic swap finality response does not match its request id or was not successful',
        );
    }
    const result = assertPlainObject(response.result, 'atomic finality response result');
    if (result.schema !== ATOMIC_RPC_FINALITY_SCHEMA) {
        throw atomicError(
            'atomic_navswap_finality_response_mismatch',
            'atomic swap finality response has the wrong schema',
        );
    }
    const txId = lowerHex(result.tx_id, 96, 'atomic finality tx_id');
    if (txId !== expectedTxId) {
        throw atomicError(
            'atomic_navswap_finality_response_mismatch',
            'atomic swap finality response does not match the client-derived transaction id',
        );
    }
    const finality = assertPlainObject(result.finality, 'atomic finality proof');
    const unsigned = assertPlainObject(signed.unsigned, 'signed atomic swap transaction.unsigned');
    if (
        finality.confirmed !== true
        || finality.tx_id !== txId
        || finality.chain_id !== unsigned.chain_id
        || finality.genesis_hash !== unsigned.genesis_hash
        || finality.protocol_version !== unsigned.protocol_version
    ) {
        throw atomicError(
            'atomic_navswap_finality_response_mismatch',
            'atomic swap finality proof is not bound to the submitted transaction domain',
        );
    }
    const block = assertPlainObject(finality.block, 'atomic finality block');
    const header = assertPlainObject(block.header, 'atomic finality block header');
    const expectedHeight = quoteBinding.parent_height + 1;
    if (
        !Number.isSafeInteger(expectedHeight)
        || header.height !== expectedHeight
        || header.parent_hash !== quoteBinding.parent_hash
        || expectedHeight > safeU64(unsigned.expires_at_height, 'unsigned.expires_at_height')
    ) {
        throw atomicError(
            'atomic_navswap_finality_response_mismatch',
            'atomic swap finality block does not match the exact quoted parent',
        );
    }
    lowerHex(header.block_hash, 96, 'atomic finality block_hash');
    lowerHex(header.state_root, 96, 'atomic finality state_root');
    lowerHex(header.certificate_id, 96, 'atomic finality certificate_id');
    assertPlainObject(header.certificate, 'atomic finality certificate');

    const receipt = assertPlainObject(finality.receipt, 'atomic finality receipt');
    if (
        receipt.accepted !== true
        || receipt.code !== 'accepted'
        || receipt.tx_id !== txId
    ) {
        throw atomicError(
            'atomic_navswap_finality_response_mismatch',
            'atomic swap finality receipt does not carry the exact accepted terminal code',
        );
    }
    if (!Array.isArray(receipt.atomic_swap_legs) || receipt.atomic_swap_legs.length !== 2) {
        throw atomicError(
            'atomic_navswap_finality_response_mismatch',
            'atomic swap finality receipt must contain exactly two leg receipts',
        );
    }
    for (let index = 0; index < 2; index += 1) {
        const expected = assertPlainObject(unsigned[`leg_${index}`], `unsigned.leg_${index}`);
        const observed = assertPlainObject(
            receipt.atomic_swap_legs[index],
            `atomic finality receipt leg_${index}`,
        );
        const sequence = safeU64(expected.sequence, `unsigned.leg_${index}.sequence`, { nonzero: true });
        if (
            observed.owner !== expected.owner
            || observed.recipient !== expected.recipient
            || observed.asset_id !== expected.asset_id
            || safeU64(observed.amount, `receipt.leg_${index}.amount`, { nonzero: true })
                !== safeU64(expected.amount, `unsigned.leg_${index}.amount`, { nonzero: true })
            || safeU64(observed.fee_charged, `receipt.leg_${index}.fee_charged`, { nonzero: true })
                !== safeU64(expected.fee, `unsigned.leg_${index}.fee`, { nonzero: true })
            || safeU64(observed.pre_sequence, `receipt.leg_${index}.pre_sequence`) !== sequence - 1
            || safeU64(observed.post_sequence, `receipt.leg_${index}.post_sequence`, { nonzero: true })
                !== sequence
        ) {
            throw atomicError(
                'atomic_navswap_finality_response_mismatch',
                `atomic swap finality receipt leg_${index} differs from the submitted signed leg`,
            );
        }
    }
    const receiptIndex = safeU64(finality.receipt_index, 'atomic finality receipt_index');
    if (
        !Array.isArray(block.receipt_ids)
        || block.receipt_ids[receiptIndex] !== txId
    ) {
        throw atomicError(
            'atomic_navswap_finality_response_mismatch',
            'atomic swap finality block does not index the submitted transaction receipt',
        );
    }
    return { tx_id: txId, finality };
}

function create(runtime) {
    markAtomicProxyRoutableTransport(runtime.rpcTcpRequest);
    const legacyExecuteNavswapQuote = runtime.executeNavswapQuote;
    const legacyExecuteTransparentNavswapRun = runtime.executeTransparentNavswapRun;
    const legacyExecuteNavswapCapabilities = runtime.executeNavswapCapabilities;

    function atomicRpcProxyErrorForRuntime(request = {}) {
        return atomicRpcProxyError(request, runtime);
    }

    async function forwardAtomicRequest(request, rpcRequest = runtime.rpcTcpRequest) {
        let endpoint = { host: runtime.RPC_HOST, port: runtime.RPC_PORT };
        let proxyRoute = null;
        if (
            isAtomicProxyRoutableTransport(rpcRequest)
            && typeof runtime.resolveRpcTarget === 'function'
        ) {
            const selected = await runtime.resolveRpcTarget(request.method);
            endpoint = selected.endpoint;
            proxyRoute = selected.route || null;
        }
        const outbound = runtime.requestWithProxyReadiness(request, proxyRoute);
        const response = await rpcRequest(
            endpoint.host,
            endpoint.port,
            outbound,
            runtime.TCP_TIMEOUT_MS,
        );
        return { outbound, response, proxyRoute, endpoint };
    }

    async function executeAtomicNavswapQuote(body = {}, rpcRequest = runtime.rpcTcpRequest) {
        try {
            assertAtomicRoute(body);
            assertNoAtomicPrivateMaterial(body);
            assertOnlyKeys(body, ATOMIC_QUOTE_BODY_KEYS, 'atomic NAVSwap quote body');
            const id = requestId(body.request_id);
            const paramsSource = Object.fromEntries(
                Object.entries(body).filter(([key]) => ATOMIC_QUOTE_PARAM_KEYS.has(key)),
            );
            const params = normalizedAtomicQuoteParams(paramsSource, runtime);
            configuredAtomicRpcFleet(runtime);
            const request = {
                version: 'postfiat-local-rpc-v1',
                id,
                method: ATOMIC_QUOTE_METHOD,
                params,
            };
            const { outbound, response, proxyRoute } = await forwardAtomicRequest(request, rpcRequest);
            if (response?.ok !== true) {
                return {
                    ok: false,
                    schema: ATOMIC_QUOTE_SCHEMA,
                    route: body.route,
                    settlement_mode: ATOMIC_SETTLEMENT_MODE,
                    code: response?.error?.code || 'atomic_navswap_quote_failed',
                    message: response?.error?.message || 'atomic swap fee quote failed',
                    rpc_request: outbound,
                    rpc_response: response || null,
                    proxy_route: proxyRoute,
                };
            }
            const quoteBinding = quoteResponseBinding(response, params, id);
            const proxyConfiguration = atomicProxyConfiguration(
                runtime,
                response.result.unsigned_transaction,
            );
            return {
                ok: true,
                schema: ATOMIC_QUOTE_SCHEMA,
                route: body.route,
                settlement_mode: ATOMIC_SETTLEMENT_MODE,
                rpc_request: outbound,
                rpc_response: response,
                quote_binding: quoteBinding,
                proxy_route: proxyRoute,
                proxy_configuration: proxyConfiguration.configuration,
                proxy_configuration_hash: proxyConfiguration.configuration_hash,
                custody_boundary: 'dual_wallet_local_signing_only',
                next_endpoint: '/api/navswap/runs',
            };
        } catch (error) {
            return publicFailure(ATOMIC_QUOTE_SCHEMA, body, error);
        }
    }

    async function executeAtomicNavswapRun(body = {}, rpcRequest = runtime.rpcTcpRequest) {
        let outbound = null;
        let quoteBinding = null;
        let routeCachePrimed = false;
        try {
            assertAtomicRoute(body);
            assertNoAtomicPrivateMaterial(body);
            assertOnlyKeys(body, ATOMIC_RUN_BODY_KEYS, 'atomic NAVSwap run body');
            requiredString(body.idempotency_key, 'idempotency_key', 256);
            const id = requestId(body.request_id);
            const expectedTxId = lowerHex(body.expected_tx_id, 96, 'expected_tx_id');
            const { text, signed } = parseAtomicSignedTransaction(body.signed_atomic_swap_transaction_json);
            assertConfiguredAtomicPair(
                runtime,
                signed.unsigned.leg_0.asset_id,
                signed.unsigned.leg_1.asset_id,
            );
            const proxyConfiguration = atomicProxyConfiguration(runtime, signed.unsigned);
            assertOnlyKeys(body.quote_binding, ATOMIC_QUOTE_BINDING_KEYS, 'quote_binding');
            quoteBinding = {
                parent_height: safeU64(body.quote_binding.parent_height, 'quote_binding.parent_height'),
                parent_hash: lowerHex(body.quote_binding.parent_hash, 96, 'quote_binding.parent_hash'),
                parent_state_root: lowerHex(
                    body.quote_binding.parent_state_root,
                    96,
                    'quote_binding.parent_state_root',
                ),
            };
            const params = {
                signed_atomic_swap_transaction_json: text,
                proxy_required_current_height: quoteBinding.parent_height,
                proxy_required_state_root: quoteBinding.parent_state_root,
                proxy_required_parent_hash: quoteBinding.parent_hash,
            };
            const timeout = optionalPositiveU64(
                body.proxy_readiness_timeout_ms,
                'proxy_readiness_timeout_ms',
            );
            if (timeout !== undefined) params.proxy_readiness_timeout_ms = timeout;
            const request = {
                version: 'postfiat-local-rpc-v1',
                id,
                method: ATOMIC_FINALITY_METHOD,
                params,
            };
            // The finality endpoint is deliberately single-shot. Once the
            // request leaves this process, a transport failure is an unknown
            // submission outcome and must never trigger an automatic replay.
            outbound = request;
            const forwarded = await forwardAtomicRequest(request, rpcRequest);
            outbound = forwarded.outbound;
            const response = forwarded.response;
            if (!response || response.id !== id) {
                throw atomicError(
                    'atomic_navswap_finality_response_mismatch',
                    'atomic swap finality response does not match its request id',
                );
            }
            if (response.ok === true) {
                finalityResponseBinding(response, signed, quoteBinding, id, expectedTxId);
                if (forwarded.proxyRoute) {
                    try {
                        const responseLine = JSON.stringify(response);
                        if (typeof runtime.clearFastpayFleetStatusCache === 'function') {
                            runtime.clearFastpayFleetStatusCache();
                        }
                        if (typeof runtime.rememberFinalizedReadEndpoint === 'function') {
                            runtime.rememberFinalizedReadEndpoint(responseLine, {
                                endpoint: forwarded.endpoint,
                                route: forwarded.proxyRoute,
                            });
                        }
                        if (typeof runtime.primeNextProposerRouteCacheFromResponse === 'function') {
                            routeCachePrimed = Boolean(
                                runtime.primeNextProposerRouteCacheFromResponse(
                                    responseLine,
                                    forwarded.proxyRoute,
                                    { warmReadiness: true },
                                ),
                            );
                        }
                    } catch (_cacheError) {
                        routeCachePrimed = false;
                        if (typeof runtime.invalidateProposerRouteCache === 'function') {
                            runtime.invalidateProposerRouteCache();
                        }
                    }
                }
            }
            const state = atomicResponseState(response);
            return {
                ok: response.ok === true,
                schema: ATOMIC_RUN_SCHEMA,
                route: body.route,
                settlement_mode: ATOMIC_SETTLEMENT_MODE,
                state,
                code: response.ok === true ? null : (response.error?.code || 'atomic_navswap_finality_failed'),
                message: response.ok === true
                    ? 'Atomic NAVSwap finalized in one certified round.'
                    : (response.error?.message || 'Atomic NAVSwap finality submission did not complete.'),
                mutation_policy: 'single_shot_no_retry',
                rpc_request_hash: atomicValueHash(outbound),
                rpc_response: response,
                quote_binding: quoteBinding,
                proxy_route: forwarded.proxyRoute,
                next_proposer_route_cache_primed: routeCachePrimed,
                proxy_configuration: proxyConfiguration.configuration,
                proxy_configuration_hash: proxyConfiguration.configuration_hash,
                custody_boundary: 'dual_wallet_local_signing_only',
            };
        } catch (error) {
            return publicFailure(ATOMIC_RUN_SCHEMA, body, error, {
                state: outbound ? 'submitted_unknown' : 'not_submitted',
                mutation_policy: 'single_shot_no_retry',
                rpc_request: outbound,
                rpc_response: null,
                quote_binding: quoteBinding,
                custody_boundary: 'dual_wallet_local_signing_only',
            });
        }
    }

    async function executeNavswapQuoteWithAtomic(body = {}, ...args) {
        if (isAtomicSettlementMode(body)) return executeAtomicNavswapQuote(body, ...args);
        return legacyExecuteNavswapQuote(body, ...args);
    }

    async function executeTransparentNavswapRunWithAtomic(body = {}, ...args) {
        if (isAtomicSettlementMode(body)) return executeAtomicNavswapRun(body, ...args);
        return legacyExecuteTransparentNavswapRun(body, ...args);
    }

    async function executeNavswapCapabilitiesWithAtomic(...args) {
        const capabilities = await legacyExecuteNavswapCapabilities(...args);
        const transparent = capabilities?.routes?.transparent_navswap;
        if (!transparent || typeof transparent !== 'object') return capabilities;
        return {
            ...capabilities,
            routes: {
                ...capabilities.routes,
                transparent_navswap: {
                    ...transparent,
                    settlement_modes: Array.from(new Set([
                        ...(Array.isArray(transparent.settlement_modes) ? transparent.settlement_modes : []),
                        ATOMIC_SETTLEMENT_MODE,
                    ])),
                    atomic_swap_v1: {
                        enabled: true,
                        quote_endpoint: '/api/navswap/quotes',
                        finality_endpoint: '/api/navswap/runs',
                        quote_method: ATOMIC_QUOTE_METHOD,
                        submit_method: ATOMIC_FINALITY_METHOD,
                        raw_submit_enabled: false,
                        mutation_policy: 'single_shot_no_retry',
                        custody_boundary: 'dual_wallet_local_signing_only',
                    },
                },
            },
        };
    }

    return {
        ATOMIC_FINALITY_METHOD,
        ATOMIC_QUOTE_METHOD,
        ATOMIC_RAW_SUBMIT_METHOD,
        ATOMIC_SETTLEMENT_MODE,
        assertNoAtomicPrivateMaterial,
        atomicRpcProxyError: atomicRpcProxyErrorForRuntime,
        executeAtomicNavswapQuote,
        executeAtomicNavswapRun,
        executeNavswapCapabilitiesWithAtomic,
        executeNavswapQuoteWithAtomic,
        executeTransparentNavswapRunWithAtomic,
        findAtomicPrivateMaterial,
        isAtomicSettlementMode,
        markAtomicProxyRoutableTransport,
        normalizedAtomicFinalityParams,
        normalizedAtomicQuoteParams,
        parseAtomicSignedTransaction,
    };
}

module.exports = {
    ATOMIC_FINALITY_METHOD,
    ATOMIC_QUOTE_METHOD,
    ATOMIC_RAW_SUBMIT_METHOD,
    ATOMIC_SETTLEMENT_MODE,
    assertNoAtomicPrivateMaterial,
    atomicRpcProxyError,
    create,
    findAtomicPrivateMaterial,
    isAtomicSettlementMode,
    markAtomicProxyRoutableTransport,
    normalizedAtomicFinalityParams,
    normalizedAtomicQuoteParams,
    parseAtomicSignedTransaction,
};
