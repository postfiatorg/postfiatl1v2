'use strict';

const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
    createWorkflow,
    loadWorkflow,
    nextAction,
    transitionWorkflow,
} = require('./a666-primary-workflow');

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'a666-primary-workflow-'));
let clock = 1_000;
const now = () => ++clock;
const intent = {
    quote_id: 'quote-1',
    route_id: 'pftl-uniswap-a666-v2',
    route_config_digest: '11'.repeat(48),
    bob_ethereum_recipient: `0x${'22'.repeat(20)}`,
    usdc_input_atoms: '1005000000000',
    wa666_output_atoms: '1000000000000',
};

try {
    const created = createWorkflow(root, intent, now);
    assert.strictEqual(created.state, 'QUOTED');
    assert.strictEqual(createWorkflow(root, intent, now).workflow_id, created.workflow_id);
    assert.strictEqual(nextAction('QUOTED'), 'submit the pinned canonical-USDC deposit');

    const submitted = transitionWorkflow(
        root,
        created.workflow_id,
        'USDC_DEPOSIT_SUBMITTED',
        { submission_id: `0x${'33'.repeat(32)}` },
        now,
    );
    assert.strictEqual(submitted.state, 'USDC_DEPOSIT_SUBMITTED');
    assert.strictEqual(submitted.transitions.length, 1);

    const reloaded = loadWorkflow(root, created.workflow_id);
    assert.deepStrictEqual(reloaded, submitted, 'workflow must resume exactly after process restart');

    assert.throws(
        () => transitionWorkflow(root, created.workflow_id, 'USDC_DEPOSIT_SUBMITTED', {
            submission_id: `0x${'44'.repeat(32)}`,
        }, now),
        /changed its evidence/,
    );
    assert.throws(
        () => transitionWorkflow(root, created.workflow_id, 'PFUSDC_PROOF_RUNNING', {
            submission_id: 'proof-job-1',
        }, now),
        /invalid a666 workflow transition/,
    );

    transitionWorkflow(root, created.workflow_id, 'USDC_DEPOSIT_FINALIZED', {
        receipt_id: `0x${'55'.repeat(32)}`,
    }, now);
    const proof = transitionWorkflow(root, created.workflow_id, 'PFUSDC_PROOF_RUNNING', {
        submission_id: 'proof-job-1',
    }, now);
    const proofFileBeforeReplay = fs.readFileSync(
        path.join(root, `${created.workflow_id}.json`),
        'utf8',
    );
    const replayed = transitionWorkflow(root, created.workflow_id, 'PFUSDC_PROOF_RUNNING', {
        submission_id: 'proof-job-1',
    }, now);
    assert.deepStrictEqual(replayed, proof);
    assert.strictEqual(
        fs.readFileSync(path.join(root, `${created.workflow_id}.json`), 'utf8'),
        proofFileBeforeReplay,
    );
    assert.strictEqual(proof.state, 'PFUSDC_PROOF_RUNNING');

    assert.throws(
        () => createWorkflow(root, { ...intent, private_key: 'forbidden' }, now),
        /must not persist secret/,
    );
    assert.throws(
        () => transitionWorkflow(root, created.workflow_id, 'PFUSDC_CREDITED', {
            nested: { mnemonic_phrase: 'forbidden' },
        }, now),
        /must not persist secret/,
    );
} finally {
    fs.rmSync(root, { recursive: true, force: true });
}
