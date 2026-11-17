'use strict';

// Per-install native wallet sessions (wallet team dependency W-2). A native client
// proves an ML-DSA-65 installation key and receives a short-lived bearer token
// that carries exactly the browser relay's caps. Tokens are stateless HMACs.

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const OPEN_PATH = '/api/native-session/v1/open';
const INFO_PATH = '/api/native-session/v1/info';
const INFO_SCHEMA = 'postfiat-native-session-info-v1';
const OPEN_SCHEMA = 'postfiat-native-session-open-v1';
const SESSION_SCHEMA = 'postfiat-native-session-v1';
const ALLOWLIST_SCHEMA = 'postfiat-native-session-allowlist-v1';
const DENYLIST_SCHEMA = 'postfiat-native-session-denylist-v1';
// The operator lists are read and hashed at most this often; a changed hash reloads.
const LIST_CHECK_INTERVAL_MS = 1_000;
// Each list is read and hashed on every check, so its size bounds that pause.
// 4 MiB holds about 1,070 allowlisted keys (3,904 hex each, four times the
// default live-install cap) or about 58,000 denylisted ids; reading and hashing
// it takes a few milliseconds.
const MAX_LIST_FILE_BYTES = 4 * 1024 * 1024;
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
const MAX_RATE_WINDOW_ADDRESSES = 100_000;
// Sixteen live installs per mutation-concurrency slot (WALLET_PROXY_MUTATION_CONCURRENCY
// defaults to 16). Past that the shared slots, not the per-install windows, decide
// throughput, so more installs only add Sybil budget and memory.
const DEFAULT_MAX_ACTIVE_INSTALLS = 256;
const OPEN_FIELDS = [
    'schema', 'algorithm_id', 'public_key_hex', 'client_nonce_hex', 'origin',
    'chain_id', 'genesis_hash_hex', 'issued_at_ms', 'signature_hex',
];
const CHAIN_ID_RE = /^[a-z0-9][a-z0-9._-]{0,63}$/;
const HEX96_RE = /^[0-9a-f]{96}$/;
const INSTALL_ID_RE = /^[0-9a-f]{64}$/;
const PUBLIC_KEY_RE = new RegExp(`^[0-9a-f]{${PUBLIC_KEY_HEX_LENGTH}}$`);
// List elements must be strings: String([id]) would pass the pattern but land as an array.
const isHexString = (value, pattern) => typeof value === 'string' && pattern.test(value);

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
// service user, not group/world writable, and no larger than MAX_LIST_FILE_BYTES.
// Checked on the opened descriptor, so the bytes returned are the file that passed.
function readOperatorFile(file, variable) {
    const refused = new Error(`${variable} must be an owner-controlled regular file`);
    let fd;
    try {
        fd = fs.openSync(path.resolve(file), fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
    } catch (error) {
        throw error.code === 'ELOOP' ? refused : error;
    }
    try {
        const stat = fs.fstatSync(fd);
        if (!stat.isFile() || stat.uid !== process.getuid() || (stat.mode & 0o022) !== 0) throw refused;
        const tooLarge = (size) => new Error(`${variable} is ${size} bytes, past the ${MAX_LIST_FILE_BYTES}-byte limit`);
        if (stat.size > MAX_LIST_FILE_BYTES) throw tooLarge(stat.size);
        const bytes = fs.readFileSync(fd);
        // A file that grew after the stat is refused too.
        if (bytes.length > MAX_LIST_FILE_BYTES) throw tooLarge(bytes.length);
        return bytes;
    } finally {
        fs.closeSync(fd);
    }
}

function parseOperatorJson(bytes, variable) {
    try {
        return JSON.parse(bytes.toString('utf8'));
    } catch (error) {
        // Never echo file contents into logs.
        throw error instanceof SyntaxError ? new Error(`${variable} is not valid JSON`) : error;
    }
}

const ALLOWLIST_VARIABLE = 'WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE';
const DENYLIST_VARIABLE = 'WALLET_PROXY_NATIVE_SESSION_DENYLIST_FILE';

function parseAllowlist(bytes) {
    const parsed = parseOperatorJson(bytes, ALLOWLIST_VARIABLE);
    const keys = parsed?.install_public_keys_hex;
    if (parsed?.schema !== ALLOWLIST_SCHEMA || !Array.isArray(keys)
        || !keys.every((key) => isHexString(key, PUBLIC_KEY_RE))) {
        throw new Error(`native session allowlist must be {"schema":"${ALLOWLIST_SCHEMA}","install_public_keys_hex":[...]}`);
    }
    return new Set(keys.map(installId));
}

function parseDenylist(bytes) {
    const parsed = parseOperatorJson(bytes, DENYLIST_VARIABLE);
    const ids = parsed?.install_ids_hex;
    if (parsed?.schema !== DENYLIST_SCHEMA || !Array.isArray(ids) || !ids.every((id) => isHexString(id, INSTALL_ID_RE))) {
        throw new Error(`native session denylist must be {"schema":"${DENYLIST_SCHEMA}","install_ids_hex":["<64 hex install id>",...]}`);
    }
    return new Set(ids);
}

// An operator list read and hashed on each poll; new contents are loaded. A file
// that cannot be read or parsed keeps the last good list and is retried on the next
// poll. Each distinct error is logged once.
function watchedList(file, variable, parse, label, logError) {
    const initial = readOperatorFile(file, variable);
    let current = parse(initial);
    let loadedHash = sha256Hex(initial);
    let lastError = null;
    return {
        get: () => current,
        // True when a new list was loaded.
        refresh() {
            try {
                const bytes = readOperatorFile(file, variable);
                const hash = sha256Hex(bytes);
                if (hash === loadedHash) {
                    lastError = null;
                    return false;
                }
                current = parse(bytes);
                loadedHash = hash;
                lastError = null;
                return true;
            } catch (error) {
                if (error.message !== lastError) {
                    lastError = error.message;
                    logError(`native session ${label} not reloaded, keeping the last good list: ${error.message}`);
                }
                return false;
            }
        },
    };
}

function createNativeSessions({
    env = process.env, caps = {}, now = Date.now,
    log = (line) => console.log(line), logError = (line) => console.error(line),
} = {}) {
    const rawOrigin = String(env.WALLET_PROXY_NATIVE_SESSION_ORIGIN || '').trim();
    const enabled = rawOrigin !== '';
    let origin = null;
    let secret = null;
    let chainId = null;
    let genesisHash = null;
    let ttlMs = 0;
    let allowlist = null;
    let denylist = null;
    const replayLimit = boundedInteger(env.WALLET_PROXY_NATIVE_SESSION_REPLAY_CACHE_LIMIT,
        'WALLET_PROXY_NATIVE_SESSION_REPLAY_CACHE_LIMIT', 100_000, 100, 10_000_000);
    // The open route is unauthenticated and verifies ML-DSA per call: bound it per source address.
    const openRateLimit = boundedInteger(env.WALLET_PROXY_NATIVE_SESSION_OPEN_RATE_LIMIT,
        'WALLET_PROXY_NATIVE_SESSION_OPEN_RATE_LIMIT', 10, 1, 100_000);
    const openRateWindowMs = boundedInteger(env.WALLET_PROXY_NATIVE_SESSION_OPEN_RATE_WINDOW_MS,
        'WALLET_PROXY_NATIVE_SESSION_OPEN_RATE_WINDOW_MS', 60_000, 1_000, 86_400_000);
    // Each install gets its own mutation window; this bounds how many hold one at a time.
    const maxActiveInstalls = boundedInteger(env.WALLET_PROXY_NATIVE_SESSION_MAX_ACTIVE_INSTALLS,
        'WALLET_PROXY_NATIVE_SESSION_MAX_ACTIVE_INSTALLS', DEFAULT_MAX_ACTIVE_INSTALLS, 1, 1_000_000);
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
        if (allowlistFile) allowlist = watchedList(allowlistFile, ALLOWLIST_VARIABLE, parseAllowlist, 'allowlist', logError);
        const denylistFile = String(env.WALLET_PROXY_NATIVE_SESSION_DENYLIST_FILE || '').trim();
        if (denylistFile) denylist = watchedList(denylistFile, DENYLIST_VARIABLE, parseDenylist, 'denylist', logError);
    }
    // Installs taken off the allowlist since startup: a reopen is a revocation.
    const delisted = new Set();
    let listsCheckedAtMs = -Infinity;
    const seenNonces = new Map();
    const activeInstalls = new Map(); // install id -> latest token expiry
    const openWindows = new Map();
    // Carried in every token's MAC'd payload: a token of an earlier process is
    // refused, so the in-memory live-install table is complete after a restart.
    const bootId = crypto.randomBytes(16).toString('hex');

    // Returns 0 when admitted, otherwise the milliseconds until the window reopens.
    function admitOpen(address) {
        const nowMs = now();
        let window = openWindows.get(address);
        if (!window || nowMs - window.startedAtMs >= openRateWindowMs) {
            if (!window && openWindows.size >= MAX_RATE_WINDOW_ADDRESSES) {
                for (const [key, entry] of openWindows) {
                    if (nowMs - entry.startedAtMs >= openRateWindowMs) openWindows.delete(key);
                }
                // A full table fails closed instead of forgetting live windows.
                if (openWindows.size >= MAX_RATE_WINDOW_ADDRESSES) return openRateWindowMs;
            }
            window = { startedAtMs: nowMs, count: 0 };
            openWindows.set(address, window);
        }
        if (window.count >= openRateLimit) return Math.max(1, openRateWindowMs - (nowMs - window.startedAtMs));
        window.count += 1;
        return 0;
    }

    const listRefusal = (id) => denylist?.get().has(id) || (allowlist && !allowlist.get().has(id));

    function refreshLists() {
        const nowMs = now();
        if (nowMs - listsCheckedAtMs < LIST_CHECK_INTERVAL_MS) return;
        listsCheckedAtMs = nowMs;
        const previous = allowlist?.get();
        const allowReloaded = allowlist?.refresh();
        if (allowReloaded) {
            const next = allowlist.get();
            for (const id of previous) if (!next.has(id)) delisted.add(id);
            for (const id of next) delisted.delete(id);
        }
        const denyReloaded = denylist?.refresh();
        // A revoked install frees its slot now, not when it next shows up.
        if (allowReloaded || denyReloaded) {
            for (const id of activeInstalls.keys()) if (listRefusal(id)) activeInstalls.delete(id);
        }
    }

    // Null when the install may hold a session, otherwise the refusal code. An
    // install that holds a token, or was listed before, is revoked rather than unknown.
    function installRefusal(id, holdsToken) {
        refreshLists();
        let refusal = null;
        if (denylist?.get().has(id)) refusal = 'native_session_install_revoked';
        else if (listRefusal(id)) {
            refusal = holdsToken || delisted.has(id) ? 'native_session_install_revoked' : 'native_session_install_not_allowed';
        }
        if (refusal) activeInstalls.delete(id); // a revoked install frees its slot
        return refusal;
    }

    // A live install may always renew; a new one needs a free slot.
    function installSlotFree(id, nowMs) {
        if (activeInstalls.get(id) > nowMs || activeInstalls.size < maxActiveInstalls) return true;
        for (const [key, expiry] of activeInstalls) if (expiry <= nowMs) activeInstalls.delete(key);
        return activeInstalls.size < maxActiveInstalls;
    }

    function noteActive(id, expiresAtMs) {
        if (!(activeInstalls.get(id) >= expiresAtMs)) activeInstalls.set(id, expiresAtMs);
    }

    function mac(payloadPart) {
        return crypto.createHmac('sha256', secret).update(TOKEN_MAC_DOMAIN).update('\0').update(payloadPart).digest();
    }

    function mint(id, issuedAtMs) {
        const payload = {
            v: 1, iid: id, org: origin, cid: chainId, gen: genesisHash, boot: bootId,
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
        if (payload.boot !== bootId) return fail(401, 'native_session_restarted');
        const refusal = installRefusal(payload.iid, true);
        if (refusal) return fail(403, refusal);
        // An install revoked and listed again lost its slot: it needs a free one back.
        if (!installSlotFree(payload.iid, now())) return fail(503, 'native_session_busy');
        noteActive(payload.iid, payload.exp);
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

    // One audit line per open: the install id's first 16 hex characters (only once
    // the signature proves the key), the source address and the outcome code.
    // Never the token, the signature or the secret.
    async function open(req) {
        // A disabled gateway has no session surface to audit.
        if (!enabled) return { statusCode: 404, body: { ok: false, schema: SESSION_SCHEMA, code: 'native_session_disabled' } };
        const audit = { id: null };
        const result = await openSession(req, audit);
        log(JSON.stringify({
            event: 'native_session_open',
            install_id_prefix: audit.id ? audit.id.slice(0, 16) : null,
            source: String(req.socket?.remoteAddress || 'unknown'),
            outcome: result.body.ok ? 'ok' : result.body.code,
        }));
        return result;
    }

    async function openSession(req, audit) {
        const fail = (statusCode, code) => ({ statusCode, body: { ok: false, schema: SESSION_SCHEMA, code } });
        if (req.method !== 'POST') return fail(405, 'method_not_allowed');
        // Before any body read or signature check. Behind a reverse proxy the
        // source address is the proxy's, so the window is shared by its clients.
        const retryAfterMs = admitOpen(String(req.socket?.remoteAddress || 'unknown'));
        if (retryAfterMs) {
            req.resume();
            return { statusCode: 429, body: { ok: false, schema: SESSION_SCHEMA, code: 'native_session_rate_limited', retry_after_ms: retryAfterMs } };
        }
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
        audit.id = id;
        const refusal = installRefusal(id, false);
        if (refusal) return fail(403, refusal);
        if (!installSlotFree(id, nowMs)) return fail(503, 'native_session_busy');
        const nonce = rememberNonce(body.client_nonce_hex, body.issued_at_ms + MAX_CLOCK_SKEW_MS);
        if (nonce === 'replayed') return fail(409, 'native_session_nonce_replayed');
        if (nonce === 'full') return fail(503, 'native_session_busy');
        const { token, payload } = mint(id, nowMs);
        noteActive(id, payload.exp);
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

    // Discovery for wallets: the settings a session is bound to. No secret, no list contents.
    function info(req) {
        const fail = (statusCode, code) => ({ statusCode, body: { ok: false, schema: INFO_SCHEMA, code } });
        if (!enabled) return fail(404, 'native_session_disabled');
        if (req.method !== 'GET' && req.method !== 'HEAD') return fail(405, 'method_not_allowed');
        return {
            statusCode: 200,
            body: {
                ok: true,
                schema: INFO_SCHEMA,
                enabled,
                origin,
                chain_id: chainId,
                genesis_hash_hex: genesisHash,
                ttl_ms: ttlMs,
                allowlist: allowlist !== null,
                max_active_installs: maxActiveInstalls,
            },
        };
    }

    // Native tokens never leave a trail in URLs; reject any request that tries.
    function queryCarriesToken(url) {
        for (const value of url.searchParams.values()) if (isNativeToken(value)) return true;
        return false;
    }

    return { enabled, authenticate, info, isNativeToken, open, queryCarriesToken };
}

module.exports = {
    ALGORITHM_ID,
    DEFAULT_MAX_ACTIVE_INSTALLS,
    INFO_PATH,
    MAX_LIST_FILE_BYTES,
    OPEN_PATH,
    OPEN_SCHEMA,
    SESSION_SCHEMA,
    SIGNATURE_CONTEXT,
    createNativeSessions,
    installId,
    mlDsa65,
    openMessage,
};
