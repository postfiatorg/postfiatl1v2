'use strict';

const LOOPBACK_HOSTS = new Set(['127.0.0.1', 'localhost', '::1']);
const HEX96_RE = /^[0-9a-f]{96}$/;
const POSTFIAT_ADDRESS_RE = /^pf[0-9a-f]{40}$/;
const BOUNDED_ID_RE = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/;
const IDEMPOTENCY_KEY_RE = /^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/;
const BOUNDED_REFERENCE_RE = /^[A-Za-z0-9_.:-]{1,256}$/;
const INTENT_SCHEMA = 'postfiat.pftl_swap.intent.v1';
const SIGNED_INTENT_SCHEMA = 'postfiat.pftl_swap.signed_intent.v1';
const ML_DSA_65_PUBLIC_KEY_HEX_LENGTH = 3_904;
const ML_DSA_65_SIGNATURE_HEX_LENGTH = 6_618;
const SIGNED_INTENT_FIELDS = new Set(['schema', 'intent', 'algorithm_id', 'public_key_hex', 'signature_hex']);
const INTENT_FIELDS = new Set([
    'schema', 'chain_id', 'genesis_hash', 'protocol_version', 'principal', 'controlled_wallet_id',
    'route_id', 'direction', 'output_mode', 'input_reference', 'input_amount_atoms',
    'minimum_output_amount_atoms', 'maximum_fee_atoms', 'quote_id', 'pricing_nav_epoch',
    'policy_hash', 'expiry_height', 'idempotency_key',
]);

function configFromEnv(env = process.env) {
    const raw = String(env.PFTL_PRIVATE_SWAP_URL || '').trim();
    const controlledWalletId = String(env.PFTL_PRIVATE_SWAP_CONTROLLED_WALLET_ID || '').trim().toLowerCase();
    const routeId = String(env.PFTL_PRIVATE_SWAP_ROUTE_ID || '').trim();
    if (!raw) return { configured: false, missing: ['PFTL_PRIVATE_SWAP_URL'] };
    let url;
    try { url = new URL(raw); } catch (_) {
        return { configured: false, missing: ['valid PFTL_PRIVATE_SWAP_URL'] };
    }
    if (url.protocol !== 'http:' || !LOOPBACK_HOSTS.has(url.hostname)
        || url.username || url.password || url.search || url.hash) {
        return { configured: false, missing: ['loopback HTTP PFTL_PRIVATE_SWAP_URL'] };
    }
    if (!POSTFIAT_ADDRESS_RE.test(controlledWalletId)) {
        return { configured: false, missing: ['PFTL_PRIVATE_SWAP_CONTROLLED_WALLET_ID'] };
    }
    if (!BOUNDED_ID_RE.test(routeId)) {
        return { configured: false, missing: ['PFTL_PRIVATE_SWAP_ROUTE_ID'] };
    }
    const suppliedTimeout = Number(env.PFTL_PRIVATE_SWAP_TIMEOUT_MS || 310_000);
    if (!Number.isSafeInteger(suppliedTimeout) || suppliedTimeout < 1_000 || suppliedTimeout > 600_000) {
        return { configured: false, missing: ['bounded PFTL_PRIVATE_SWAP_TIMEOUT_MS'] };
    }
    return {
        configured: true,
        baseUrl: url,
        controlledWalletId,
        routeId,
        timeoutMs: suppliedTimeout,
    };
}

function boundedPositiveInteger(value, label, maximum = Number.MAX_SAFE_INTEGER) {
    const text = String(value ?? '').trim();
    if (!/^[1-9][0-9]*$/.test(text)) throw Object.assign(
        new Error(`${label} must be a positive integer`), { code: 'pftl_private_swap_invalid_request' });
    const parsed = Number(text);
    if (!Number.isSafeInteger(parsed) || parsed > maximum) throw Object.assign(
        new Error(`${label} exceeds the configured safe bound`), { code: 'pftl_private_swap_invalid_request' });
    return parsed;
}

function validateQuoteRequest(body) {
    const direction = String(body?.direction || '');
    const outputMode = String(body?.output_mode || '');
    if (!['issue', 'redeem'].includes(direction) || !['private', 'transparent'].includes(outputMode)) {
        throw Object.assign(new Error('private-primary direction or output mode is invalid'),
            { code: 'pftl_private_swap_invalid_request' });
    }
    return {
        direction,
        nav_amount_atoms: boundedPositiveInteger(body?.nav_amount_atoms, 'nav_amount_atoms', 1_000_000),
        output_mode: outputMode,
    };
}

function validateSignedIntent(body, controlledWalletId, routeId) {
    const signed = body?.signed_intent;
    const intent = signed?.intent;
    const signedFieldsExact = signed && Object.keys(signed).every(field => SIGNED_INTENT_FIELDS.has(field))
        && Object.keys(signed).length === SIGNED_INTENT_FIELDS.size;
    const intentFieldsExact = intent && Object.keys(intent).every(field => INTENT_FIELDS.has(field))
        && Object.keys(intent).length === INTENT_FIELDS.size;
    if (signed?.schema !== SIGNED_INTENT_SCHEMA || signed?.algorithm_id !== 'ML-DSA-65'
        || !signedFieldsExact || !intentFieldsExact
        || !new RegExp(`^[0-9a-f]{${ML_DSA_65_PUBLIC_KEY_HEX_LENGTH}}$`).test(String(signed?.public_key_hex || ''))
        || !new RegExp(`^[0-9a-f]{${ML_DSA_65_SIGNATURE_HEX_LENGTH}}$`).test(String(signed?.signature_hex || ''))
        || intent?.schema !== INTENT_SCHEMA || intent?.principal !== controlledWalletId
        || intent?.controlled_wallet_id !== controlledWalletId
        || !BOUNDED_ID_RE.test(String(intent?.chain_id || ''))
        || intent?.route_id !== routeId || !BOUNDED_ID_RE.test(String(intent?.route_id || ''))
        || !['issue', 'redeem'].includes(intent?.direction)
        || !['private', 'transparent'].includes(intent?.output_mode)
        || !BOUNDED_REFERENCE_RE.test(String(intent?.input_reference || ''))
        || !HEX96_RE.test(String(intent?.genesis_hash || ''))
        || !HEX96_RE.test(String(intent?.quote_id || ''))
        || !HEX96_RE.test(String(intent?.policy_hash || ''))
        || !Number.isSafeInteger(intent?.protocol_version) || intent.protocol_version <= 0
        || intent.protocol_version > 0xffff_ffff
        || !Number.isSafeInteger(intent?.input_amount_atoms) || intent.input_amount_atoms <= 0
        || !Number.isSafeInteger(intent?.minimum_output_amount_atoms)
        || intent.minimum_output_amount_atoms <= 0
        || !Number.isSafeInteger(intent?.maximum_fee_atoms) || intent.maximum_fee_atoms <= 0
        || !Number.isSafeInteger(intent?.pricing_nav_epoch) || intent.pricing_nav_epoch <= 0
        || !Number.isSafeInteger(intent?.expiry_height) || intent.expiry_height <= 0
        || !IDEMPOTENCY_KEY_RE.test(String(intent?.idempotency_key || ''))) {
        throw Object.assign(new Error('signed private-primary intent failed the wallet proxy boundary'),
            { code: 'pftl_private_swap_invalid_signed_intent' });
    }
    for (const [field, value] of Object.entries(intent)) {
        if (/(seed|private|secret|password|backup|opening|spend_key)/i.test(field)) {
            throw Object.assign(new Error(`signed private-primary intent contains forbidden field ${field}`),
                { code: 'pftl_private_swap_private_material_rejected' });
        }
        if (typeof value === 'object' && value !== null) {
            throw Object.assign(new Error('signed private-primary intent must contain scalar public fields only'),
                { code: 'pftl_private_swap_invalid_signed_intent' });
        }
    }
    return { signed_intent: signed };
}

async function upstream(config, method, path, body = undefined) {
    if (!config.configured) return {
        ok: false,
        ready: false,
        code: 'pftl_private_swap_not_configured',
        message: `resident private-primary service is not configured: ${config.missing.join(', ')}`,
    };
    const target = new URL(path, config.baseUrl);
    let response;
    try {
        response = await fetch(target, {
            method,
            headers: body === undefined ? { Accept: 'application/json' }
                : { Accept: 'application/json', 'Content-Type': 'application/json' },
            body: body === undefined ? undefined : JSON.stringify(body),
            signal: AbortSignal.timeout(config.timeoutMs),
        });
    } catch (_) {
        return {
            ok: false,
            ready: false,
            code: 'pftl_private_swap_unreachable',
            message: 'resident private-primary service is unreachable on the configured loopback endpoint',
            http_status: 503,
        };
    }
    const payload = await response.json().catch(() => ({}));
    return { ...payload, http_status: response.status };
}

function create(_runtime, env = process.env) {
    const config = configFromEnv(env);
    return {
        pftlPrivateSwapConfig: () => ({
            configured: config.configured,
            controlled_wallet_id: config.controlledWalletId || null,
            route_id: config.routeId || null,
            missing: config.missing || [],
        }),
        pftlPrivateSwapReadiness: async () => upstream(config, 'GET', '/v1/ready'),
        pftlPrivateSwapQuote: async body => {
            const result = await upstream(config, 'POST', '/v1/quote', validateQuoteRequest(body));
            if (result?.quote && result.quote.route_id !== config.routeId) return {
                ok: false,
                code: 'pftl_private_swap_route_identity_mismatch',
                message: 'resident private-primary quote does not match the configured governed route',
                http_status: 502,
            };
            return result;
        },
        pftlPrivateSwapSubmit: async body => upstream(
            config, 'POST', '/v1/swap', validateSignedIntent(
                body, config.controlledWalletId, config.routeId)),
        pftlPrivateSwapStatus: async idempotencyKey => {
            if (!IDEMPOTENCY_KEY_RE.test(String(idempotencyKey || ''))) throw Object.assign(
                new Error('private-primary idempotency key is invalid'),
                { code: 'pftl_private_swap_invalid_idempotency_key' });
            return upstream(config, 'GET', `/v1/status?id=${encodeURIComponent(idempotencyKey)}`);
        },
    };
}

module.exports = {
    ML_DSA_65_PUBLIC_KEY_HEX_LENGTH,
    ML_DSA_65_SIGNATURE_HEX_LENGTH,
    configFromEnv,
    create,
    validateQuoteRequest,
    validateSignedIntent,
};
