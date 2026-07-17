'use strict';

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');

process.env.WALLET_PROXY_API_TOKEN = 'test-only-wallet-proxy-token-32-bytes-minimum';

const { serveWalletStatic } = require('./server');

function responseCapture() {
    const headers = new Map();
    return {
        headers,
        statusCode: 0,
        body: null,
        setHeader(name, value) { headers.set(name.toLowerCase(), value); },
        end(body = Buffer.alloc(0)) { this.body = Buffer.from(body); },
    };
}

async function main() {
    const staticDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-wallet-static-'));
    const outsideDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-wallet-outside-'));
    fs.mkdirSync(path.join(staticDir, 'assets'));
    fs.mkdirSync(path.join(staticDir, '@vite'));
    fs.mkdirSync(path.join(staticDir, 'src'));
    fs.writeFileSync(path.join(staticDir, 'index.html'), '<!doctype html><title>wallet</title>');
    fs.writeFileSync(path.join(staticDir, 'assets', 'wallet-C0FFEE12.js'), 'export {};');
    fs.writeFileSync(path.join(staticDir, 'assets', 'wallet.js'), 'export {};');
    fs.writeFileSync(path.join(staticDir, 'assets', 'wallet.js.map'), '{}');
    fs.writeFileSync(path.join(staticDir, '@vite', 'client'), 'throw new Error("dev only");');
    fs.writeFileSync(path.join(staticDir, 'src', 'main.jsx'), 'throw new Error("source only");');
    fs.writeFileSync(path.join(staticDir, '.env'), 'SECRET=not-for-the-browser');
    fs.writeFileSync(path.join(outsideDir, 'escaped-C0FFEE12.js'), 'throw new Error("escaped");');
    fs.symlinkSync(
        path.join(outsideDir, 'escaped-C0FFEE12.js'),
        path.join(staticDir, 'assets', 'escaped-C0FFEE12.js'),
    );

    const htmlResponse = responseCapture();
    assert.strictEqual(await serveWalletStatic(
        { method: 'GET' },
        htmlResponse,
        new URL('http://localhost/'),
        staticDir,
    ), true);
    assert.strictEqual(htmlResponse.statusCode, 200);
    assert.match(htmlResponse.headers.get('content-security-policy'), /frame-ancestors 'none'/);
    assert.strictEqual(htmlResponse.headers.get('x-frame-options'), 'DENY');
    assert.strictEqual(htmlResponse.headers.get('x-content-type-options'), 'nosniff');
    assert.strictEqual(htmlResponse.headers.get('cache-control'), 'no-store');

    const assetResponse = responseCapture();
    assert.strictEqual(await serveWalletStatic(
        { method: 'GET' },
        assetResponse,
        new URL('http://localhost/assets/wallet-C0FFEE12.js'),
        staticDir,
    ), true);
    assert.strictEqual(assetResponse.headers.get('cache-control'), 'public, max-age=31536000, immutable');

    const unhashedAssetResponse = responseCapture();
    assert.strictEqual(await serveWalletStatic(
        { method: 'GET' },
        unhashedAssetResponse,
        new URL('http://localhost/assets/wallet.js'),
        staticDir,
    ), true);
    assert.strictEqual(unhashedAssetResponse.headers.get('cache-control'), 'no-store');

    for (const forbiddenPath of [
        '/assets/wallet.js.map',
        '/@vite/client',
        '/src/main.jsx',
        '/.env',
    ]) {
        const forbiddenResponse = responseCapture();
        assert.strictEqual(
            await serveWalletStatic(
                { method: 'GET' },
                forbiddenResponse,
                new URL(`http://localhost${forbiddenPath}`),
                staticDir,
            ),
            false,
            `${forbiddenPath} must never be served by the production wallet`,
        );
    }

    const symlinkResponse = responseCapture();
    assert.strictEqual(await serveWalletStatic(
        { method: 'GET' },
        symlinkResponse,
        new URL('http://localhost/assets/escaped-C0FFEE12.js'),
        staticDir,
    ), false, 'production static serving must not follow links outside the build root');

    const traversalResponse = responseCapture();
    assert.strictEqual(await serveWalletStatic(
        { method: 'GET' },
        traversalResponse,
        { pathname: '/../outside' },
        staticDir,
    ), false);

    const apiResponse = responseCapture();
    assert.strictEqual(await serveWalletStatic(
        { method: 'GET' },
        apiResponse,
        new URL('http://localhost/api/navswap/status'),
        staticDir,
    ), false);

    fs.rmSync(staticDir, { force: true, recursive: true });
    fs.rmSync(outsideDir, { force: true, recursive: true });
    console.log('P0-WALLET-02 hardened static serving regression passed');
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
