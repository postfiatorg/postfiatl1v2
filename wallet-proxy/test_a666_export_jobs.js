'use strict';

const assert = require('node:assert');
const { EventEmitter } = require('node:events');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { create, canonicalJobId } = require('./a666-export-jobs');
const { terminalError } = require('./a666-export-relay-driver');

const ROUTE_ID = 'pftl-a666-ethereum-wA666-usdc-v1';
const ROUTE_DIGEST = '12ed00ca87e29554ce4b978da1710fffc0830767e84e62f08df257f727db953efdd89bcf6ea99f5634d6e5ea8aca2933';
const TOKEN = '0xee4c92edb03efdd9b519339edc19ad70c69a9be5';
const RECIPIENT = `0x${'22'.repeat(20)}`;
const PACKET = '33'.repeat(48);
const DIGEST = '44'.repeat(32);

const config = {
    schema: 'postfiat-a666-export-relay-config-v1',
    enabled: true,
    route_id: ROUTE_ID,
    route_config_digest: ROUTE_DIGEST,
    wrapped_token: TOKEN,
    driver_bin: process.execPath,
    driver_args: ['run-job', '--job-file', '{job_file}'],
    inspect_args: ['inspect', '--packet-hash', '{packet_hash}', '--packet-digest', '{packet_digest}',
        '--ethereum-recipient', '{ethereum_recipient}', '--amount-atoms', '{amount_atoms}'],
    readiness_args: ['readiness'],
    cwd: process.cwd(),
    max_amount_atoms: '250000000000',
    worker_timeout_ms: 60_000,
};

function request(packet = PACKET, amount = '1000000') {
    return {
        route_id: ROUTE_ID,
        route_config_digest: ROUTE_DIGEST,
        packet_hash: packet,
        packet_digest: DIGEST,
        ethereum_recipient: RECIPIENT,
        amount_atoms: amount,
        deadline_seconds: Math.floor(Date.now() / 1000) + 7200,
    };
}

function harness(root) {
    let nextPid = 900_000;
    const alive = new Set();
    const spawns = [];
    const execFileAsync = async (_bin, args) => {
        if (args[0] === 'readiness') return { stdout: JSON.stringify({
            ok: true, ready: true, route_id: ROUTE_ID, route_config_digest: ROUTE_DIGEST,
            wrapped_token: TOKEN,
        }) };
        const get = name => args[args.indexOf(name) + 1];
        return { stdout: JSON.stringify({
            ok: true,
            route_id: ROUTE_ID,
            route_config_digest: ROUTE_DIGEST,
            packet_hash: get('--packet-hash'),
            packet_digest: get('--packet-digest'),
            ethereum_recipient: get('--ethereum-recipient'),
            amount_atoms: get('--amount-atoms'),
            source_height: get('--packet-hash') === PACKET ? 700 : 701,
            packet_status: 'SourceDebited',
            ethereum_packet_consumed: false,
        }) };
    };
    const options = {
        root, config, execFileAsync, watchdogMs: 60_000,
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
    return { options, spawns, alive };
}

(async () => {
    assert.strictEqual(
        terminalError(new Error('packet unknown\nusage: --reject-malformed-input')),
        false,
        'unstructured child stderr must remain retryable',
    );
    assert.strictEqual(
        terminalError(Object.assign(new Error('binding mismatch'), { terminal: true })),
        true,
        'an explicitly classified safety failure must be terminal',
    );

    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-a666-export-jobs-'));
    const test = harness(root);
    const subject = create({}, test.options);
    try {
        const readiness = await subject.a666ExportRelayReadiness();
        assert.strictEqual(readiness.ready, true);

        const concurrent = await Promise.all([
            subject.submitA666ExportRelayJob(request()),
            subject.submitA666ExportRelayJob(request()),
        ]);
        assert.strictEqual(concurrent[0].job_id, canonicalJobId(PACKET));
        assert.deepStrictEqual(concurrent.map(row => row.idempotent_replay).sort(), [false, true]);
        assert.strictEqual(test.spawns.length, 1, 'concurrent replay must launch one worker');

        await assert.rejects(
            subject.submitA666ExportRelayJob(request(PACKET, '2000000')),
            error => error.code === 'a666_export_job_binding_conflict',
        );

        const secondPacket = '55'.repeat(48);
        const second = await subject.submitA666ExportRelayJob(request(secondPacket));
        assert.strictEqual(second.source_height, null);
        assert.strictEqual(test.spawns.length, 1, 'global serialization must keep the later job queued');

        subject.closeA666ExportRelayJobs();
        assert.strictEqual(test.spawns[0].child.killedWith, 'SIGTERM');
        const restarted = create({}, test.options);
        try {
            await new Promise(resolve => setImmediate(resolve));
            assert.strictEqual(test.spawns.length, 2, 'restart must resume the earliest durable job');
            const resumedArgs = test.spawns[1].args.join(' ');
            assert.ok(
                resumedArgs.includes(canonicalJobId(PACKET).slice(2))
                || resumedArgs.includes(canonicalJobId(secondPacket).slice(2)),
                'restart must resume one durable pre-armed job',
            );
        } finally { restarted.closeA666ExportRelayJobs(); }
    } finally {
        subject.closeA666ExportRelayJobs();
        fs.rmSync(root, { recursive: true, force: true });
    }

    const disabled = create({}, { env: {} });
    const disabledReadiness = await disabled.a666ExportRelayReadiness();
    assert.strictEqual(disabledReadiness.ready, false);
    await assert.rejects(disabled.submitA666ExportRelayJob(request()),
        error => error.code === 'a666_export_relay_not_configured');
})().catch(error => {
    process.stderr.write(`${error.stack || error}\n`);
    process.exitCode = 1;
});
