'use strict';

const assert = require('node:assert');
const { EventEmitter } = require('node:events');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { create, jobIdFor } = require('./pnok-fix-jobs');

const BASE = '01'.repeat(48);
const QUOTE = '02'.repeat(48);
const OPERATOR = `pf${'11'.repeat(20)}`;
const BOB = `pf${'22'.repeat(20)}`;
const BASE_NOTE = 'aa'.repeat(32);
const QUOTE_NOTE = 'bb'.repeat(32);

const config = {
    schema: 'postfiat-pnok-private-fix-wallet-config-v1',
    enabled: true,
    base_asset_id: BASE,
    quote_asset_id: QUOTE,
    base_symbol: 'pfUSDC',
    quote_symbol: 'pNOK',
    base_precision: 6,
    quote_precision: 0,
    base_atoms: '20000000',
    quote_atoms: '210',
    fix_operator: OPERATOR,
    demo_wallet: BOB,
    trust_label: 'controlled sandbox checkpoint',
    execution_label: 'private on PFTL',
    resident_service_url: 'http://127.0.0.1:18799',
    python_bin: process.execPath,
    driver_script: '/controlled/pnok-private-fix-demo.py',
    facility_key_file: '/controlled/facility.wallet-key.json',
    cwd: process.cwd(),
    worker_timeout_ms: 60_000,
    max_retries: 3,
    retry_delay_ms: 100,
};

function request(id = 'browser-run-01') {
    return {
        client_request_id: id,
        base_asset_id: BASE,
        quote_asset_id: QUOTE,
        base_atoms: '20000000',
    };
}

(async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-pnok-fix-jobs-'));
    let layout = 'acquire';
    let nextPid = 800_000;
    const alive = new Set();
    const spawns = [];
    const notes = () => layout === 'acquire'
        ? [
            { id: BASE_NOTE, wallet_address: BOB, asset_id: BASE, amount_atoms: 20_000_000, state: 'spendable' },
            { id: QUOTE_NOTE, wallet_address: OPERATOR, asset_id: QUOTE, amount_atoms: 210, state: 'spendable' },
        ]
        : [
            { id: BASE_NOTE, wallet_address: OPERATOR, asset_id: BASE, amount_atoms: 20_000_000, state: 'spendable' },
            { id: QUOTE_NOTE, wallet_address: BOB, asset_id: QUOTE, amount_atoms: 210, state: 'spendable' },
        ];
    const fetchImpl = async (url) => ({
        ok: true,
        json: async () => String(url).endsWith('/readiness')
            ? { ok: true, ready: true }
            : { ok: true, notes: notes() },
    });
    const spawnImpl = (bin, args) => {
        const child = new EventEmitter();
        child.pid = ++nextPid;
        child.unref = () => {};
        child.kill = () => { alive.delete(child.pid); return true; };
        alive.add(child.pid);
        spawns.push({ bin, args, child });
        return child;
    };
    const subject = create({}, {
        root,
        config,
        fetch: fetchImpl,
        spawn: spawnImpl,
        processAlive: (pid) => alive.has(pid),
        setInterval: () => ({ unref() {} }),
        clearInterval: () => {},
    });
    try {
        const readiness = await subject.pnokFixWalletReadiness();
        assert.strictEqual(readiness.ready, true);
        assert.strictEqual(readiness.acquire_inventory_ready, true);
        assert.strictEqual(readiness.restore_inventory_ready, false);
        assert.strictEqual(readiness.execution_privacy, 'private on PFTL');
        assert.strictEqual(readiness.source_boundary, 'controlled sandbox checkpoint');

        const [first, replay] = await Promise.all([
            subject.submitPnokFixWalletJob(request()),
            subject.submitPnokFixWalletJob(request()),
        ]);
        assert.strictEqual(first.job_id, jobIdFor('acquire', 'browser-run-01'));
        assert.deepStrictEqual([first.idempotent_replay, replay.idempotent_replay].sort(), [false, true]);
        assert.strictEqual(spawns.length, 1, 'idempotent browser retry must launch one worker');
        assert.ok(spawns[0].args.includes(BASE_NOTE));
        assert.ok(spawns[0].args.includes(QUOTE_NOTE));
        assert.ok(spawns[0].args.includes('--verify-replay'));
        const publicEncoding = JSON.stringify(first);
        assert.ok(!publicEncoding.includes(BASE_NOTE));
        assert.ok(!publicEncoding.includes(QUOTE_NOTE));
        assert.ok(!publicEncoding.includes(OPERATOR));
        assert.ok(!publicEncoding.includes(BOB));

        await assert.rejects(
            subject.submitPnokFixWalletJob({ ...request(), base_atoms: '1' }),
            (error) => error.code === 'invalid_pnok_fix_wallet_request',
        );

        const intentDir = spawns[0].args[spawns[0].args.indexOf('--intent-dir') + 1];
        fs.mkdirSync(path.join(intentDir, 'public'), { recursive: true });
        fs.writeFileSync(path.join(intentDir, 'public/status.json'), JSON.stringify({
            schema: 'postfiat-pnok-private-fix-demo-public-status-v1',
            stage: 'complete',
            fix_packet_hash: '33'.repeat(48),
            reservation_id: '44'.repeat(48),
            nullifier_occurrence_counts: [1, 1],
            output_occurrence_counts: [1, 1],
            replay_rejected_without_effect: true,
            supply_unchanged: true,
        }));
        alive.delete(spawns[0].child.pid);
        spawns[0].child.emit('exit', 0, null);
        await new Promise((resolve) => setImmediate(resolve));
        const accepted = subject.pnokFixWalletJobStatus(first.job_id);
        assert.strictEqual(accepted.status, 'accepted');
        assert.strictEqual(accepted.execution_stage, 'complete');
        assert.strictEqual(accepted.replay_rejected_without_effect, true);

        layout = 'restore';
        const restore = await subject.submitPnokFixRestoreJob(request('restore-run-01'));
        assert.strictEqual(restore.direction, 'restore');
        assert.strictEqual(spawns.length, 2);
        assert.ok(spawns[1].args.includes('--no-verify-replay'));
        assert.strictEqual(
            spawns[1].args[spawns[1].args.indexOf('--liquidity-wallet-address') + 1],
            BOB,
        );
    } finally {
        subject.closePnokFixWalletJobs();
        fs.rmSync(root, { recursive: true, force: true });
    }

    const disabled = create({}, { env: {} });
    const disabledReadiness = await disabled.pnokFixWalletReadiness();
    assert.strictEqual(disabledReadiness.ready, false);
    await assert.rejects(
        disabled.submitPnokFixWalletJob(request()),
        (error) => error.code === 'pnok_fix_wallet_not_configured',
    );
})().catch((error) => {
    process.stderr.write(`${error.stack || error}\n`);
    process.exitCode = 1;
});
