'use strict';

const assert = require('assert');
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-bridge-http-v2-'));
const driver = path.join(root, 'driver.js');
fs.writeFileSync(driver, `
'use strict';
const fs = require('fs');
const path = require('path');
const args = process.argv.slice(2);
if (args[0] === 'readiness') {
  const route = args[args.indexOf('--route') + 1];
  process.stdout.write(JSON.stringify({
    ok: true, ready: true, route_id: route, source_chain_id: 1,
    source_proof_kind: 'sp1-ethereum-finality-v1',
    observer_attestor_enabled: false, prover_authenticated: true,
    prover_healthy: true, route_manifest_active: true,
    program_vkey_active: true, nav_cap_growth_enabled: true,
    vault_paused: false, vault_code_hash_matches: true,
    token_code_hash_matches: true, execution_rpc_sources_reachable: 2,
    beacon_finality_current: true
  }));
  process.exit(0);
}
if (args[0] === 'run-job') {
  const file = args[args.indexOf('--job-file') + 1];
  fs.writeFileSync(path.join(path.dirname(file), 'worker-state.json'), JSON.stringify({
    schema: 'postfiat-trustless-bridge-worker-state-v2',
    status: 'accepted', receipt_code: 'ACCEPTED'
  }));
  process.exit(0);
}
process.exit(2);
`, { mode: 0o700 });

process.env.WALLET_PROXY_API_TOKEN = 'test-only-wallet-proxy-token-32-bytes-minimum';
process.env.TRUSTLESS_BRIDGE_JOB_ROOT = root;
process.env.TRUSTLESS_BRIDGE_READINESS_REFRESH_MS = '60000';
process.env.TRUSTLESS_BRIDGE_READINESS_MAX_AGE_MS = '120000';
process.env.TRUSTLESS_BRIDGE_ROUTES_JSON = JSON.stringify([{
    route_id: 'ethereum-mainnet-usdc-v1',
    source_chain_id: 1,
    source_proof_kind: 'sp1-ethereum-finality-v1',
    driver_bin: process.execPath,
    driver_args: [driver, 'run-job', '--job-file', '{job_file}'],
    readiness_bin: process.execPath,
    readiness_args: [driver, 'readiness', '--route', '{route_id}'],
    cwd: root,
    max_amount_atoms: '5000000',
}]);

const { server } = require('./server');

function request(port, method, requestPath, body, authenticated = false) {
    return new Promise((resolve, reject) => {
        const encoded = body === undefined ? '' : JSON.stringify(body);
        const req = http.request({
            host: '127.0.0.1',
            port,
            path: requestPath,
            method,
            headers: {
                ...(encoded ? {
                    'content-type': 'application/json',
                    'content-length': Buffer.byteLength(encoded),
                } : {}),
                ...(authenticated ? {
                    authorization: `Bearer ${process.env.WALLET_PROXY_API_TOKEN}`,
                    origin: 'http://localhost:5173',
                } : {}),
            },
        }, (res) => {
            let raw = '';
            res.setEncoding('utf8');
            res.on('data', (chunk) => { raw += chunk; });
            res.on('end', () => resolve({ status: res.statusCode, body: JSON.parse(raw) }));
        });
        req.on('error', reject);
        req.end(encoded);
    });
}

async function waitForReady(port) {
    for (let attempt = 0; attempt < 50; attempt += 1) {
        const response = await request(
            port,
            'GET',
            '/api/bridge/readiness?route=ethereum-mainnet-usdc-v1',
        );
        if (response.body.ready === true) return response;
        await new Promise((resolve) => setTimeout(resolve, 20));
    }
    throw new Error('route readiness did not prewarm');
}

async function main() {
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const port = server.address().port;
    try {
        const ready = await waitForReady(port);
        assert.strictEqual(ready.status, 200);
        assert.strictEqual(ready.body.source_proof_kind, 'sp1-ethereum-finality-v1');
        assert.strictEqual(ready.body.observer_attestor_enabled, false);

        const body = {
            route_id: 'ethereum-mainnet-usdc-v1',
            source_chain_id: 1,
            deposit_tx_hash: `0x${'11'.repeat(32)}`,
            deposit_id: `0x${'22'.repeat(32)}`,
            pftl_recipient: `pf${'33'.repeat(20)}`,
            depositor: `0x${'44'.repeat(20)}`,
            amount_atoms: '2000000',
            idempotency_key: 'http-e2e-deposit-1',
        };
        const unauthenticated = await request(port, 'POST', '/api/bridge/jobs', body);
        assert.strictEqual(unauthenticated.status, 403);

        const created = await request(port, 'POST', '/api/bridge/jobs', body, true);
        assert.strictEqual(created.status, 202);
        assert.match(created.body.job_id, /^0x[0-9a-f]{64}$/);
        let status;
        for (let attempt = 0; attempt < 50; attempt += 1) {
            status = await request(port, 'GET', `/api/bridge/jobs/${created.body.job_id}`);
            if (status.body.status === 'accepted') break;
            await new Promise((resolve) => setTimeout(resolve, 20));
        }
        assert.strictEqual(status.status, 200);
        assert.strictEqual(status.body.status, 'accepted');
        assert.strictEqual(status.body.receipt_code, 'ACCEPTED');

        const replay = await request(port, 'POST', '/api/bridge/jobs', {
            ...body,
            idempotency_key: 'http-e2e-deposit-replay',
        }, true);
        assert.strictEqual(replay.status, 202);
        assert.strictEqual(replay.body.job_id, created.body.job_id);
        assert.strictEqual(replay.body.idempotent_replay, true);
    } finally {
        await new Promise((resolve) => server.close(resolve));
        fs.rmSync(root, { recursive: true, force: true });
    }
    console.log('trustless bridge P2 HTTP route/readiness/job/resume test passed');
}

main().catch((error) => {
    console.error(error);
    fs.rmSync(root, { recursive: true, force: true });
    process.exitCode = 1;
});
