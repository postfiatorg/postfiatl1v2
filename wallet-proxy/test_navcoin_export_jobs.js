'use strict';

const assert = require('node:assert');
const { EventEmitter } = require('node:events');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { create, canonicalJobId } = require('./navcoin-export-jobs');
const { terminalError } = require('./navcoin-export-relay-driver');
const { parseNavcoinRelayPath } = require('./navswap-persistence-http');

const ROUTE_ID = 'pftl-qnav-ethereum-wqNAV-qusd-v1';
const OTHER_ROUTE_ID = 'pftl-rnav-ethereum-wrNAV-rusd-v1';
const ROUTE_DIGEST = 'aa'.repeat(48);
const TOKEN = `0x${'bb'.repeat(20)}`;
const RECIPIENT = `0x${'22'.repeat(20)}`;
const PACKET = '33'.repeat(48);
const DIGEST = '44'.repeat(32);

const config = {
    schema: 'postfiat-navcoin-export-relay-config-v1',
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
    assert.throws(
        () => create({}, { config: { ...config, worker_timeout_ms: Number.MAX_SAFE_INTEGER } }),
        /worker timeout.*supported range/,
    );
    assert.throws(
        () => create({}, { config: { ...config, driver_args: new Array(65).fill('x') } }),
        /driver arguments.*bound/,
    );
    assert.deepStrictEqual(
        parseNavcoinRelayPath(`/api/navcoin/${ROUTE_ID}/export-readiness`),
        { routeId: ROUTE_ID, resource: 'export-readiness', jobId: null },
        'public relay routing must preserve case-sensitive governed route IDs',
    );
    assert.strictEqual(
        parseNavcoinRelayPath('/api/navcoin/-invalid/export-readiness'),
        null,
        'public relay routing must reject a non-alphanumeric route prefix',
    );
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

    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-navcoin-export-jobs-'));
    const test = harness(root);
    const subject = create({}, test.options);
    try {
        const readiness = await subject.navcoinExportRelayReadiness(ROUTE_ID);
        assert.strictEqual(readiness.ready, true);
        assert.strictEqual((await subject.navcoinExportRelayReadiness(OTHER_ROUTE_ID)).ready, false);

        const concurrent = await Promise.all([
            subject.submitNavcoinExportRelayJob(request()),
            subject.submitNavcoinExportRelayJob(request()),
        ]);
        assert.strictEqual(concurrent[0].job_id, canonicalJobId(ROUTE_ID, PACKET));
        assert.notStrictEqual(canonicalJobId(ROUTE_ID, PACKET), canonicalJobId(OTHER_ROUTE_ID, PACKET));
        assert.deepStrictEqual(concurrent.map(row => row.idempotent_replay).sort(), [false, true]);
        assert.strictEqual(test.spawns.length, 1, 'concurrent replay must launch one worker');

        await assert.rejects(
            subject.submitNavcoinExportRelayJob(request(PACKET, '2000000')),
            error => error.code === 'navcoin_export_job_binding_conflict',
        );

        const secondPacket = '55'.repeat(48);
        const second = await subject.submitNavcoinExportRelayJob(request(secondPacket));
        assert.strictEqual(second.source_height, null);
        assert.strictEqual(test.spawns.length, 1, 'global serialization must keep the later job queued');

        assert.strictEqual(subject.navcoinExportRelayJobStatus(OTHER_ROUTE_ID, concurrent[0].job_id), null);
        subject.closeNavcoinExportRelayJobs();
        assert.strictEqual(test.spawns[0].child.killedWith, 'SIGTERM');
        const restarted = create({}, test.options);
        try {
            await new Promise(resolve => setImmediate(resolve));
            assert.strictEqual(test.spawns.length, 2, 'restart must resume the earliest durable job');
            const resumedArgs = test.spawns[1].args.join(' ');
            assert.ok(
                resumedArgs.includes(canonicalJobId(ROUTE_ID, PACKET).slice(2))
                || resumedArgs.includes(canonicalJobId(ROUTE_ID, secondPacket).slice(2)),
                'restart must resume one durable pre-armed job',
            );
        } finally { restarted.closeNavcoinExportRelayJobs(); }
    } finally {
        subject.closeNavcoinExportRelayJobs();
        fs.rmSync(root, { recursive: true, force: true });
    }

    const disabled = create({}, { env: {} });
    const disabledReadiness = await disabled.navcoinExportRelayReadiness(ROUTE_ID);
    assert.strictEqual(disabledReadiness.ready, false);
    await assert.rejects(disabled.submitNavcoinExportRelayJob(request()),
        error => error.code === 'navcoin_export_relay_not_configured');

    const multiRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-navcoin-export-routes-'));
    const secondDigest = 'ee'.repeat(48);
    const secondToken = `0x${'ff'.repeat(20)}`;
    const multi = create({}, {
        ...harness(multiRoot).options,
        root: multiRoot,
        config: undefined,
        configs: [config, { ...config, route_id: OTHER_ROUTE_ID,
            route_config_digest: secondDigest, wrapped_token: secondToken,
            readiness_args: ['readiness', '--route-id', OTHER_ROUTE_ID] }],
        execFileAsync: async (_bin, args) => {
            const second = args.includes(OTHER_ROUTE_ID);
            return { stdout: JSON.stringify({ ok: true, ready: true,
                route_id: second ? OTHER_ROUTE_ID : ROUTE_ID,
                route_config_digest: second ? secondDigest : ROUTE_DIGEST,
                wrapped_token: second ? secondToken : TOKEN }) };
        },
    });
    try {
        assert.strictEqual((await multi.navcoinExportRelayReadiness(ROUTE_ID)).ready, true);
        assert.strictEqual((await multi.navcoinExportRelayReadiness(OTHER_ROUTE_ID)).ready, true);
    } finally {
        multi.closeNavcoinExportRelayJobs();
        fs.rmSync(multiRoot, { recursive: true, force: true });
    }
})().catch(error => {
    process.stderr.write(`${error.stack || error}\n`);
    process.exitCode = 1;
});
