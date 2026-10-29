'use strict';

// Per-install native wallet sessions (wallet team dependency W-2). A native client
// proves an ML-DSA-65 installation key and receives a short-lived bearer token
// that carries exactly the browser relay's caps. Tokens are stateless HMACs.

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const OPEN_PATH = '/api/native-session/v1/open';
const OPEN_SCHEMA = 'postfiat-native-session-open-v1';
const SESSION_SCHEMA = 'postfiat-native-session-v1';
const ALLOWLIST_SCHEMA = 'postfiat-native-session-allowlist-v1';
// FIPS 204 ML-DSA context, named like crates/crypto_provider's signature contexts.
const SIGNATURE_CONTEXT = 'postfiat-l1-v2/native-session/v1';
const TOKEN_MAC_DOMAIN = 'postfiat-l1-v2/native-session-token/v1';
const INSTALL_ID_DOMAIN = 'postfiat-l1-v2/native-session-install/v1';
const TOKEN_PREFIX = 'pfns1';
const ALGORITHM_ID = 'ML-DSA-65';
const PUBLIC_KEY_HEX_LENGTH = 3_904;
const SIGNATURE_HEX_LENGTH = 6_618;
const MAX_OPEN_BODY_BYTES = 16 * 1024;
const MAX_TOKEN_BYTES = 1024;
const MAX_CLOCK_SKEW_MS = 5 * 60_000;
const OPEN_FIELDS = [
    'schema', 'algorithm_id', 'public_key_hex', 'client_nonce_hex', 'origin',
    'chain_id', 'genesis_hash_hex', 'issued_at_ms', 'signature_hex',
];
const CHAIN_ID_RE = /^[a-z0-9][a-z0-9._-]{0,63}$/;
const HEX96_RE = /^[0-9a-f]{96}$/;

let mlDsa65Promise = null;
function mlDsa65() {
    mlDsa65Promise ||= import('@noble/post-quantum/ml-dsa.js').then((module) => module.ml_dsa65);
    return mlDsa65Promise;
}

const b64url = (bytes) => Buffer.from(bytes).toString('base64url');
const sha256Hex = (...parts) => crypto.createHash('sha256').update(Buffer.concat(parts.map((part) => Buffer.from(part)))).digest('hex');

function installId(publicKeyHex) {
    return sha256Hex(INSTALL_ID_DOMAIN, Buffer.from([0]), Buffer.from(publicKeyHex, 'hex'));
}

// The exact bytes the client signs. Every field has a newline-free charset.
function openMessage(body) {
    return Buffer.from([
        OPEN_SCHEMA, body.origin, body.chain_id, body.genesis_hash_hex,
        body.public_key_hex, body.client_nonce_hex, String(body.issued_at_ms),
    ].join('\n'), 'utf8');
}

function normalizeOrigin(value) {
    const url = new URL(String(value));
    if (url.protocol !== 'https:' || url.username || url.password
        || url.pathname !== '/' || url.search || url.hash || String(value) !== url.origin) {
        throw new Error('WALLET_PROXY_NATIVE_SESSION_ORIGIN must be an exact https origin (scheme://host[:port])');
    }
    return url.origin;
}

function boundedInteger(raw, name, fallback, minimum, maximum) {
    if (raw === undefined || String(raw).trim() === '') return fallback;
    const value = /^\d+$/.test(String(raw).trim()) ? Number(String(raw).trim()) : Number.NaN;
    if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
        throw new Error(`${name} must be an integer between ${minimum} and ${maximum}`);
    }
    return value;
}

// Same checks as the other operator files: regular, not a symlink, owned by the
// service user, not group/world writable.
function loadAllowlist(file) {
    const absolute = path.resolve(file);
    const stat = fs.lstatSync(absolute);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.uid !== process.getuid()
        || (stat.mode & 0o022) !== 0) {
        throw new Error('WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE must be an owner-controlled regular file');
    }
    const parsed = JSON.parse(fs.readFileSync(absolute, 'utf8'));
    const keys = parsed?.install_public_keys_hex;
    if (parsed?.schema !== ALLOWLIST_SCHEMA || !Array.isArray(keys)
        || !keys.every((key) => new RegExp(`^[0-9a-f]{${PUBLIC_KEY_HEX_LENGTH}}$`).test(String(key)))) {
        throw new Error(`native session allowlist must be {"schema":"${ALLOWLIST_SCHEMA}","install_public_keys_hex":[...]}`);
    }
    return new Set(keys.map(installId));
}

function createNativeSessions({ env = process.env, caps = {}, now = Date.now } = {}) {
    const rawOrigin = String(env.WALLET_PROXY_NATIVE_SESSION_ORIGIN || '').trim();
    const enabled = rawOrigin !== '';
    let origin = null;
    let secret = null;
    let chainId = null;
    let genesisHash = null;
    let ttlMs = 0;
    let allowlist = null;
    const replayLimit = boundedInteger(env.WALLET_PROXY_NATIVE_SESSION_REPLAY_CACHE_LIMIT,
        'WALLET_PROXY_NATIVE_SESSION_REPLAY_CACHE_LIMIT', 100_000, 100, 10_000_000);
    if (enabled) {
        origin = normalizeOrigin(rawOrigin);
        secret = String(env.WALLET_PROXY_NATIVE_SESSION_SECRET || '');
        if (Buffer.byteLength(secret, 'utf8') < 32) {
            throw new Error('WALLET_PROXY_NATIVE_SESSION_SECRET must contain at least 32 bytes when native sessions are enabled');
        }
        chainId = String(env.WALLET_PROXY_NATIVE_SESSION_CHAIN_ID || '').trim();
        genesisHash = String(env.WALLET_PROXY_NATIVE_SESSION_GENESIS_HASH || '').trim().toLowerCase();
        if (!CHAIN_ID_RE.test(chainId) || !HEX96_RE.test(genesisHash)) {
            throw new Error('native sessions require WALLET_PROXY_NATIVE_SESSION_CHAIN_ID and a 96-hex WALLET_PROXY_NATIVE_SESSION_GENESIS_HASH');
        }
        ttlMs = boundedInteger(env.WALLET_PROXY_NATIVE_SESSION_TTL_MS,
            'WALLET_PROXY_NATIVE_SESSION_TTL_MS', 15 * 60_000, 60_000, 60 * 60_000);
        const allowlistFile = String(env.WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE || '').trim();
        if (allowlistFile) allowlist = loadAllowlist(allowlistFile);
    }
    const seenNonces = new Map();

    function mac(payloadPart) {
        return crypto.createHmac('sha256', secret).update(TOKEN_MAC_DOMAIN).update('\0').update(payloadPart).digest();
    }

    function mint(id, issuedAtMs) {
        const payload = {
            v: 1, iid: id, org: origin, cid: chainId, gen: genesisHash,
            iat: issuedAtMs, exp: issuedAtMs + ttlMs, jti: crypto.randomBytes(16).toString('hex'),
        };
        const payloadPart = b64url(JSON.stringify(payload));
        return { token: `${TOKEN_PREFIX}.${payloadPart}.${b64url(mac(payloadPart))}`, payload };
    }

    function isNativeToken(candidate) {
        return String(candidate || '').startsWith(`${TOKEN_PREFIX}.`);
    }

    // Returns { principal } or { failure: { statusCode, code } }.
    function authenticate(candidate, requestOrigin = '') {
        const fail = (statusCode, code) => ({ principal: null, failure: { statusCode, code } });
        if (!enabled) return fail(401, 'native_session_disabled');
        const token = String(candidate || '');
        const parts = token.split('.');
        if (Buffer.byteLength(token) > MAX_TOKEN_BYTES || parts.length !== 3 || parts[0] !== TOKEN_PREFIX) {
            return fail(401, 'native_session_invalid');
        }
        const presented = Buffer.from(parts[2], 'base64url');
        const expected = mac(parts[1]);
        if (presented.length !== expected.length || !crypto.timingSafeEqual(presented, expected)) {
            return fail(401, 'native_session_invalid');
        }
        let payload;
        try { payload = JSON.parse(Buffer.from(parts[1], 'base64url').toString('utf8')); } catch (_) {
            return fail(401, 'native_session_invalid');
        }
        if (payload?.v !== 1 || !/^[0-9a-f]{64}$/.test(String(payload.iid || ''))) return fail(401, 'native_session_invalid');
        if (payload.org !== origin || (requestOrigin && requestOrigin !== origin)) return fail(403, 'native_session_wrong_origin');
        if (payload.cid !== chainId || payload.gen !== genesisHash) return fail(403, 'native_session_wrong_network');
        if (!Number.isSafeInteger(payload.exp) || payload.exp <= now()) return fail(401, 'native_session_expired');
        if (allowlist && !allowlist.has(payload.iid)) return fail(403, 'native_session_install_not_allowed');
        return { principal: `native:${payload.iid}`, failure: null };
    }

    function rememberNonce(nonce, expiresAtMs) {
        const nowMs = now();
        if (seenNonces.has(nonce)) return 'replayed';
        if (seenNonces.size >= replayLimit) {
            for (const [key, expiry] of seenNonces) if (expiry <= nowMs) seenNonces.delete(key);
            // A full cache fails closed instead of evicting live nonces.
            if (seenNonces.size >= replayLimit) return 'full';
        }
        seenNonces.set(nonce, expiresAtMs);
        return 'fresh';
    }

    function readBody(req) {
        return new Promise((resolve, reject) => {
            const declared = Number(req.headers['content-length'] || 0);
            if (declared > MAX_OPEN_BODY_BYTES) {
                reject(Object.assign(new Error('too large'), { statusCode: 413, code: 'request_body_too_large' }));
                req.resume();
                return;
            }
            const chunks = [];
            let size = 0;
            let done = false;
            req.on('data', (chunk) => {
                if (done) return;
                size += chunk.length;
                if (size > MAX_OPEN_BODY_BYTES) {
                    done = true;
                    reject(Object.assign(new Error('too large'), { statusCode: 413, code: 'request_body_too_large' }));
                    req.resume();
                    return;
                }
                chunks.push(chunk);
            });
            req.on('end', () => { if (!done) { done = true; resolve(Buffer.concat(chunks)); } });
            req.on('error', (error) => { if (!done) { done = true; reject(error); } });
        });
    }

    async function open(req) {
        const fail = (statusCode, code) => ({ statusCode, body: { ok: false, schema: SESSION_SCHEMA, code } });
        if (!enabled) return fail(404, 'native_session_disabled');
        if (req.method !== 'POST') return fail(405, 'method_not_allowed');
        const headerOrigin = String(req.headers.origin || '').trim();
        if (headerOrigin && headerOrigin !== origin) return fail(403, 'native_session_wrong_origin');
        let body;
        try {
            body = JSON.parse((await readBody(req)).toString('utf8'));
        } catch (error) {
            return error.statusCode ? fail(error.statusCode, error.code) : fail(400, 'invalid_json');
        }
        // Exact field set: nothing custody-bearing or unexpected is accepted.
        const keys = body && typeof body === 'object' && !Array.isArray(body) ? Object.keys(body) : [];
        if (keys.length !== OPEN_FIELDS.length || !OPEN_FIELDS.every((field) => keys.includes(field))
            || body.schema !== OPEN_SCHEMA || body.algorithm_id !== ALGORITHM_ID
            || !new RegExp(`^[0-9a-f]{${PUBLIC_KEY_HEX_LENGTH}}$`).test(String(body.public_key_hex))
            || !new RegExp(`^[0-9a-f]{${SIGNATURE_HEX_LENGTH}}$`).test(String(body.signature_hex))
            || !/^[0-9a-f]{64}$/.test(String(body.client_nonce_hex))
            || typeof body.origin !== 'string' || typeof body.chain_id !== 'string'
            || !HEX96_RE.test(String(body.genesis_hash_hex))
            || !Number.isSafeInteger(body.issued_at_ms)) {
            return fail(400, 'native_session_request_invalid');
        }
        if (body.origin !== origin) return fail(403, 'native_session_wrong_origin');
        if (body.chain_id !== chainId || body.genesis_hash_hex !== genesisHash) return fail(403, 'native_session_wrong_network');
        const nowMs = now();
        if (Math.abs(nowMs - body.issued_at_ms) > MAX_CLOCK_SKEW_MS) return fail(401, 'native_session_stale');
        const verifier = await mlDsa65();
        let valid = false;
        try {
            valid = verifier.verify(
                Buffer.from(body.signature_hex, 'hex'), openMessage(body),
                Buffer.from(body.public_key_hex, 'hex'), { context: Buffer.from(SIGNATURE_CONTEXT, 'utf8') },
            );
        } catch (_) { valid = false; }
        if (!valid) return fail(401, 'native_session_bad_signature');
        const id = installId(body.public_key_hex);
        if (allowlist && !allowlist.has(id)) return fail(403, 'native_session_install_not_allowed');
        const nonce = rememberNonce(body.client_nonce_hex, body.issued_at_ms + MAX_CLOCK_SKEW_MS);
        if (nonce === 'replayed') return fail(409, 'native_session_nonce_replayed');
        if (nonce === 'full') return fail(503, 'native_session_busy');
        const { token, payload } = mint(id, nowMs);
        return {
            statusCode: 200,
            body: {
                ok: true,
                schema: SESSION_SCHEMA,
                principal: `native:${id}`,
                install_id: id,
                token_type: 'Bearer',
                token,
                expires_at_ms: payload.exp,
                origin,
                chain_id: chainId,
                genesis_hash_hex: genesisHash,
                caps: { same_as: 'browser_relay', ...caps },
            },
        };
    }

    // Native tokens never leave a trail in URLs; reject any request that tries.
    function queryCarriesToken(url) {
        for (const value of url.searchParams.values()) if (isNativeToken(value)) return true;
        return false;
    }

    return { enabled, authenticate, isNativeToken, open, queryCarriesToken };
}

module.exports = {
    ALGORITHM_ID,
    OPEN_PATH,
    OPEN_SCHEMA,
    SESSION_SCHEMA,
    SIGNATURE_CONTEXT,
    createNativeSessions,
    installId,
    mlDsa65,
    openMessage,
};
