'use strict';

const assert = require('node:assert');
const { EventEmitter } = require('node:events');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { create, canonicalJobId } = require('./a666-return-jobs');
const { terminalError } = require('./a666-return-relay-driver');

const ROUTE_ID = 'pftl-a666-ethereum-wA666-usdc-v1';
const ROUTE_DIGEST = '12ed00ca87e29554ce4b978da1710fffc0830767e84e62f08df257f727db953efdd89bcf6ea99f5634d6e5ea8aca2933';
const CONTROLLER = '0x9a0262c0572fb4db08765408eb225e207f40c3d9';
const TOKEN = '0xee4c92edb03efdd9b519339edc19ad70c69a9be5';
const ASSET = '521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c';
const TX = `0x${'11'.repeat(32)}`;

const config = {
    schema: 'postfiat-a666-return-relay-config-v1', enabled: true,
    route_id: ROUTE_ID, route_config_digest: ROUTE_DIGEST,
    controller: CONTROLLER, wrapped_token: TOKEN,
    driver_bin: process.execPath, driver_args: ['run-job', '--job-file', '{job_file}'],
    readiness_args: ['readiness'], cwd: process.cwd(), max_amount_atoms: '250000000000',
    worker_timeout_ms: 60_000,
};

function request(amount = '5000000') {
    return {
        route_id: ROUTE_ID, route_config_digest: ROUTE_DIGEST, transaction_hash: TX,
        ethereum_sender: `0x${'22'.repeat(20)}`, pftl_recipient: `pf${'33'.repeat(20)}`,
        native_nav_asset_id: ASSET, amount_atoms: amount, return_nonce: '44'.repeat(32),
    };
}

(async () => {
    assert.strictEqual(terminalError(new Error('temporary Ethereum RPC failure')), false);
    assert.strictEqual(terminalError(Object.assign(new Error('binding'), { terminal: true })), true);
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-a666-return-jobs-'));
    let nextPid = 700_000;
    const alive = new Set();
    const spawns = [];
    const options = {
        root, config, watchdogMs: 60_000,
        execFileAsync: async () => ({ stdout: JSON.stringify({
            ok: true, ready: true, route_id: ROUTE_ID, route_config_digest: ROUTE_DIGEST,
            controller: CONTROLLER,
        }) }),
        processAlive: pid => alive.has(pid),
        processStartToken: pid => alive.has(pid) ? `start-${pid}` : null,
        spawn: (bin, args) => {
            const child = new EventEmitter();
            child.pid = ++nextPid;
            child.unref = () => {};
            child.kill = signal => { alive.delete(child.pid); child.killedWith = signal; return true; };
            alive.add(child.pid);
            spawns.push({ bin, args, child });
            return child;
        },
    };
    const subject = create({}, options);
    try {
        assert.strictEqual((await subject.a666ReturnRelayReadiness()).ready, true);
        const jobs = await Promise.all([
            subject.submitA666ReturnRelayJob(request()),
            subject.submitA666ReturnRelayJob(request()),
        ]);
        assert.strictEqual(jobs[0].job_id, canonicalJobId(TX));
        assert.deepStrictEqual(jobs.map(row => row.idempotent_replay).sort(), [false, true]);
        assert.strictEqual(spawns.length, 1, 'idempotent replay must launch one return worker');
        await assert.rejects(subject.submitA666ReturnRelayJob(request('6000000')),
            error => error.code === 'a666_return_job_binding_conflict');
        subject.closeA666ReturnRelayJobs();
        assert.strictEqual(spawns[0].child.killedWith, 'SIGTERM');
    } finally {
        subject.closeA666ReturnRelayJobs();
        fs.rmSync(root, { recursive: true, force: true });
    }
    const disabled = create({}, { env: {} });
    assert.strictEqual((await disabled.a666ReturnRelayReadiness()).ready, false);
    await assert.rejects(disabled.submitA666ReturnRelayJob(request()),
        error => error.code === 'a666_return_relay_not_configured');
})().catch((error) => {
    process.stderr.write(`${error.stack || error}\n`);
    process.exitCode = 1;
});
