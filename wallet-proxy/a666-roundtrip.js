'use strict';

const http = require('node:http');

const STATUS_PATH = '/api/a666-roundtrip/status';
const START_PATH = '/api/a666-roundtrip/start';
const AMOUNT = '10.000000';
const CONFIRMATION = 'RUN A666 ROUND TRIP';
const MAX_RESPONSE_BYTES = 1024 * 1024;

function loopbackUpstream(raw = process.env.A666_ROUNDTRIP_UPSTREAM || 'http://127.0.0.1:8787') {
    const upstream = new URL(String(raw));
    if (upstream.protocol !== 'http:'
        || !['127.0.0.1', 'localhost', '::1'].includes(upstream.hostname)
        || (upstream.pathname !== '/' && upstream.pathname !== '')) {
        throw Object.assign(new Error('A666 round-trip upstream must be loopback HTTP'), {
            code: 'a666_roundtrip_invalid_upstream',
        });
    }
    return upstream;
}

function parseUpstreamJson(raw, statusCode) {
    let payload;
    try { payload = JSON.parse(raw); } catch (_) {
        throw Object.assign(new Error('A666 round-trip service returned invalid JSON'), {
            code: 'a666_roundtrip_invalid_response',
        });
    }
    return { statusCode, payload };
}

function requestJson(upstream, pathname, { method = 'GET', body = null, timeoutMs = 30_000 } = {}) {
    const encoded = body === null ? null : Buffer.from(JSON.stringify(body), 'utf8');
    return new Promise((resolve, reject) => {
        const request = http.request({
            protocol: upstream.protocol,
            hostname: upstream.hostname,
            port: upstream.port || 80,
            method,
            path: pathname,
            headers: {
                Accept: 'application/json',
                ...(encoded ? {
                    'Content-Type': 'application/json',
                    'Content-Length': encoded.length,
                } : {}),
            },
        }, (response) => {
            const chunks = [];
            let bytes = 0;
            response.on('data', (chunk) => {
                bytes += chunk.length;
                if (bytes > MAX_RESPONSE_BYTES) {
                    request.destroy(Object.assign(new Error('A666 round-trip response is too large'), {
                        code: 'a666_roundtrip_response_too_large',
                    }));
                    return;
                }
                chunks.push(chunk);
            });
            response.on('end', () => {
                try {
                    resolve(parseUpstreamJson(Buffer.concat(chunks).toString('utf8'), response.statusCode || 502));
                } catch (error) { reject(error); }
            });
        });
        request.setTimeout(timeoutMs, () => request.destroy(Object.assign(
            new Error('A666 round-trip service timed out'),
            { code: 'a666_roundtrip_timeout' },
        )));
        request.on('error', (error) => {
            if (!error.code || ['ECONNREFUSED', 'ECONNRESET'].includes(error.code)) {
                error.code = 'a666_roundtrip_unavailable';
            }
            reject(error);
        });
        if (encoded) request.write(encoded);
        request.end();
    });
}

function create(_runtime = {}, options = {}) {
    const upstream = loopbackUpstream(options.upstream);
    const requestImpl = options.requestJson || requestJson;

    async function a666RoundtripStatus() {
        return requestImpl(upstream, STATUS_PATH, { method: 'GET', timeoutMs: 30_000 });
    }

    async function a666RoundtripStart(body) {
        if (body?.amount !== AMOUNT || body?.confirmation !== CONFIRMATION
            || Object.keys(body || {}).some((key) => !['amount', 'confirmation'].includes(key))) {
            throw Object.assign(new Error('A666 round trip requires the fixed 10.000000 USDC action'), {
                code: 'a666_roundtrip_invalid_request',
            });
        }
        return requestImpl(upstream, START_PATH, {
            method: 'POST',
            body: { amount: AMOUNT, confirmation: CONFIRMATION },
            timeoutMs: 60_000,
        });
    }

    return { a666RoundtripStart, a666RoundtripStatus };
}

module.exports = {
    AMOUNT,
    CONFIRMATION,
    START_PATH,
    STATUS_PATH,
    create,
    loopbackUpstream,
    parseUpstreamJson,
    requestJson,
};
