'use strict';

const crypto = require('crypto');

// Anonymous callers may page the private pool through the gateway. Each of
// these reads makes the validator load its full finalized state, so identical
// requests share one upstream call and one successful answer for a short time.
const PUBLIC_READ_CACHE_METHODS = new Set([
    'private_readiness',
    'private_pool_snapshot',
    'private_pool_page',
    'asset_orchard_private_egress_binding',
]);
const PUBLIC_READ_CACHE_TTL_MS = 2_000;
const PUBLIC_READ_CACHE_MAX_ENTRIES = 256;

function canonicalJson(value) {
    if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
    if (value && typeof value === 'object') {
        return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
    }
    return JSON.stringify(value === undefined ? null : value);
}

function createPublicReadCache({
    ttlMs = PUBLIC_READ_CACHE_TTL_MS,
    maxEntries = PUBLIC_READ_CACHE_MAX_ENTRIES,
    nowMs = Date.now,
} = {}) {
    const entries = new Map();

    function evict(now) {
        for (const [key, entry] of entries) {
            if (entry.response && entry.expiresAtMs <= now) entries.delete(key);
        }
        while (entries.size >= maxEntries) entries.delete(entries.keys().next().value);
    }

    // `load` returns the parsed upstream response. Only `ok: true` answers are
    // kept; a refusal or transport error is shared by concurrent callers only.
    // The caller's own request id is put back on every answer.
    // `scope` names the upstream endpoint, so answers from two validators never mix.
    async function read(method, params, id, load, scope = '') {
        if (!PUBLIC_READ_CACHE_METHODS.has(method)) throw new Error(`${method} is not a cached public read`);
        const key = crypto.createHash('sha256').update(`${scope}\n${method}\n${canonicalJson(params ?? {})}`).digest('hex');
        const now = nowMs();
        let entry = entries.get(key);
        if (entry?.response && entry.expiresAtMs <= now) {
            entries.delete(key);
            entry = null;
        }
        if (!entry) {
            evict(now);
            entry = { response: null, expiresAtMs: 0, pending: null };
            entry.pending = Promise.resolve().then(load).then((response) => {
                if (response?.ok === true && entries.get(key) === entry) {
                    entry.response = response;
                    entry.expiresAtMs = nowMs() + ttlMs;
                    entry.pending = null;
                } else if (entries.get(key) === entry) {
                    entries.delete(key);
                }
                return response;
            }, (error) => {
                if (entries.get(key) === entry) entries.delete(key);
                throw error;
            });
            entries.set(key, entry);
        }
        const response = entry.response || await entry.pending;
        return { ...response, id };
    }

    return { read, size: () => entries.size };
}

module.exports = {
    PUBLIC_READ_CACHE_MAX_ENTRIES,
    PUBLIC_READ_CACHE_METHODS,
    PUBLIC_READ_CACHE_TTL_MS,
    canonicalJson,
    createPublicReadCache,
};
