'use strict';

const assert = require('node:assert/strict');
const http = require('node:http');

const {
    AMOUNT,
    CONFIRMATION,
    create,
    loopbackUpstream,
} = require('./a666-roundtrip');

assert.throws(() => loopbackUpstream('https://127.0.0.1:8787'), /loopback HTTP/);
assert.throws(() => loopbackUpstream('http://example.com:8787'), /loopback HTTP/);
assert.equal(loopbackUpstream('http://localhost:8787').hostname, 'localhost');

(async () => {
    const calls = [];
    const service = create({}, {
        upstream: 'http://127.0.0.1:8787',
        requestJson: async (_upstream, pathname, options) => {
            calls.push({ pathname, options });
            return { statusCode: 200, payload: { ok: true } };
        },
    });

    await service.a666RoundtripStatus();
    await service.a666RoundtripStart({ amount: AMOUNT, confirmation: CONFIRMATION });
    assert.deepEqual(calls.map((call) => [call.pathname, call.options.method]), [
        ['/api/a666-roundtrip/status', 'GET'],
        ['/api/a666-roundtrip/start', 'POST'],
    ]);
    assert.deepEqual(calls[1].options.body, { amount: AMOUNT, confirmation: CONFIRMATION });
    await assert.rejects(
        service.a666RoundtripStart({ amount: '10000000', confirmation: CONFIRMATION }),
        /fixed 10\.000000 USDC/,
    );
    await assert.rejects(
        service.a666RoundtripStart({ amount: AMOUNT, confirmation: CONFIRMATION, private_key: 'forbidden' }),
        /fixed 10\.000000 USDC/,
    );

    const requests = [];
    const server = http.createServer((req, res) => {
        const chunks = [];
        req.on('data', (chunk) => chunks.push(chunk));
        req.on('end', () => {
            requests.push({
                authorization: req.headers.authorization,
                body: Buffer.concat(chunks).toString('utf8'),
                method: req.method,
                url: req.url,
            });
            res.writeHead(200, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify({ ok: true }));
        });
    });
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    try {
        const live = create({}, { upstream: `http://127.0.0.1:${server.address().port}` });
        await live.a666RoundtripStatus();
        await live.a666RoundtripStart({ amount: AMOUNT, confirmation: CONFIRMATION });
        assert.equal(requests[0].authorization, undefined, 'browser proxy bearer token must not reach StakeHub');
        assert.deepEqual(requests.map(({ method, url }) => [method, url]), [
            ['GET', '/api/a666-roundtrip/status'],
            ['POST', '/api/a666-roundtrip/start'],
        ]);
        assert.deepEqual(JSON.parse(requests[1].body), { amount: AMOUNT, confirmation: CONFIRMATION });
    } finally {
        await new Promise((resolve) => server.close(resolve));
    }

    console.log('A666 round-trip proxy adapter passed');
})().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
