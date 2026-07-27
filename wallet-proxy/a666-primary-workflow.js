'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const SCHEMA = 'postfiat-a666-primary-workflow-v1';
const STATES = Object.freeze([
    'QUOTED',
    'USDC_DEPOSIT_SUBMITTED',
    'USDC_DEPOSIT_FINALIZED',
    'PFUSDC_PROOF_RUNNING',
    'PFUSDC_CREDITED',
    'PRIMARY_SUBSCRIPTION_FINALIZED',
    'A666_EXPORT_FINALIZED',
    'PFTL_FINALITY_PROOF_RUNNING',
    'WA666_CLAIM_SUBMITTED',
    'COMPLETED',
    'RECOVERY_REQUIRED',
    'REFUNDED',
    'FAILED_TERMINAL',
]);
const TERMINAL = new Set(['COMPLETED', 'REFUNDED', 'FAILED_TERMINAL']);
const ALLOWED = new Map([
    ['QUOTED', new Set(['USDC_DEPOSIT_SUBMITTED', 'FAILED_TERMINAL'])],
    ['USDC_DEPOSIT_SUBMITTED', new Set(['USDC_DEPOSIT_FINALIZED', 'RECOVERY_REQUIRED'])],
    ['USDC_DEPOSIT_FINALIZED', new Set(['PFUSDC_PROOF_RUNNING', 'RECOVERY_REQUIRED'])],
    ['PFUSDC_PROOF_RUNNING', new Set(['PFUSDC_CREDITED', 'RECOVERY_REQUIRED'])],
    ['PFUSDC_CREDITED', new Set(['PRIMARY_SUBSCRIPTION_FINALIZED', 'RECOVERY_REQUIRED'])],
    ['PRIMARY_SUBSCRIPTION_FINALIZED', new Set(['A666_EXPORT_FINALIZED', 'RECOVERY_REQUIRED'])],
    ['A666_EXPORT_FINALIZED', new Set(['PFTL_FINALITY_PROOF_RUNNING', 'RECOVERY_REQUIRED'])],
    ['PFTL_FINALITY_PROOF_RUNNING', new Set(['WA666_CLAIM_SUBMITTED', 'RECOVERY_REQUIRED'])],
    ['WA666_CLAIM_SUBMITTED', new Set(['COMPLETED', 'RECOVERY_REQUIRED'])],
    ['RECOVERY_REQUIRED', new Set([
        'USDC_DEPOSIT_SUBMITTED',
        'PFUSDC_PROOF_RUNNING',
        'PFTL_FINALITY_PROOF_RUNNING',
        'WA666_CLAIM_SUBMITTED',
        'REFUNDED',
        'FAILED_TERMINAL',
    ])],
]);
const SUBMISSION_STATES = new Set([
    'USDC_DEPOSIT_SUBMITTED',
    'PFUSDC_PROOF_RUNNING',
    'PFTL_FINALITY_PROOF_RUNNING',
    'WA666_CLAIM_SUBMITTED',
]);
const FORBIDDEN_KEYS = /(?:private|secret|mnemonic|seed|signing).*?(?:key|phrase)?$/i;

function createWorkflow(root, intent, now = Date.now) {
    validateIntent(intent);
    rejectSecrets(intent);
    const workflowId = workflowIdForIntent(intent);
    const file = workflowFile(root, workflowId);
    if (fs.existsSync(file)) {
        const existing = loadWorkflow(root, workflowId);
        if (stableJson(existing.intent) !== stableJson(intent)) {
            throw new Error('workflow ID collision with a different immutable intent');
        }
        return existing;
    }
    const timestamp = now();
    const workflow = {
        schema: SCHEMA,
        workflow_id: workflowId,
        state: 'QUOTED',
        intent,
        created_at_unix_ms: timestamp,
        updated_at_unix_ms: timestamp,
        transitions: [],
    };
    atomicWrite(file, workflow);
    return workflow;
}

function transitionWorkflow(root, workflowId, nextState, evidence, now = Date.now) {
    if (!STATES.includes(nextState)) throw new Error(`unknown a666 workflow state: ${nextState}`);
    rejectSecrets(evidence);
    const workflow = loadWorkflow(root, workflowId);
    if (workflow.state === nextState) {
        const prior = workflow.transitions.at(-1);
        if (!prior || prior.to !== nextState || stableJson(prior.evidence) !== stableJson(evidence)) {
            throw new Error('idempotent transition replay changed its evidence');
        }
        return workflow;
    }
    if (TERMINAL.has(workflow.state)) throw new Error(`terminal workflow cannot leave ${workflow.state}`);
    if (!ALLOWED.get(workflow.state)?.has(nextState)) {
        throw new Error(`invalid a666 workflow transition ${workflow.state} -> ${nextState}`);
    }
    if (!evidence || typeof evidence !== 'object' || Array.isArray(evidence)) {
        throw new Error('workflow transition evidence must be an object');
    }
    if (SUBMISSION_STATES.has(nextState) && !nonEmptyString(evidence.submission_id)) {
        throw new Error(`${nextState} must persist submission_id before polling`);
    }
    const idempotencyKey = transitionKey(workflowId, workflow.state, nextState);
    const timestamp = now();
    workflow.transitions.push({
        from: workflow.state,
        to: nextState,
        idempotency_key: idempotencyKey,
        recorded_at_unix_ms: timestamp,
        evidence,
    });
    workflow.state = nextState;
    workflow.updated_at_unix_ms = timestamp;
    atomicWrite(workflowFile(root, workflowId), workflow);
    return workflow;
}

function loadWorkflow(root, workflowId) {
    if (!/^[0-9a-f]{64}$/.test(String(workflowId || ''))) throw new Error('malformed workflow ID');
    const workflow = JSON.parse(fs.readFileSync(workflowFile(root, workflowId), 'utf8'));
    if (workflow.schema !== SCHEMA || workflow.workflow_id !== workflowId || !STATES.includes(workflow.state)) {
        throw new Error('invalid persisted a666 workflow');
    }
    rejectSecrets(workflow);
    return workflow;
}

function nextAction(state) {
    const actions = {
        QUOTED: 'submit the pinned canonical-USDC deposit',
        USDC_DEPOSIT_SUBMITTED: 'poll the persisted deposit transaction to finality',
        USDC_DEPOSIT_FINALIZED: 'start the pfUSDC ingress proof once',
        PFUSDC_PROOF_RUNNING: 'poll the persisted pfUSDC proof job',
        PFUSDC_CREDITED: 'submit the reserved primary subscription',
        PRIMARY_SUBSCRIPTION_FINALIZED: 'submit entitlement-bound a666 export packets',
        A666_EXPORT_FINALIZED: 'start the PFTL receipt-finality proof once',
        PFTL_FINALITY_PROOF_RUNNING: 'poll the persisted PFTL proof job',
        WA666_CLAIM_SUBMITTED: 'poll the persisted Ethereum claim transaction',
        RECOVERY_REQUIRED: 'reconcile external status before selecting a retry or refund',
        COMPLETED: 'none',
        REFUNDED: 'none',
        FAILED_TERMINAL: 'none',
    };
    if (!(state in actions)) throw new Error(`unknown a666 workflow state: ${state}`);
    return actions[state];
}

function workflowIdForIntent(intent) {
    return crypto.createHash('sha256')
        .update('postfiat.a666.primary.workflow.v1\0')
        .update(stableJson(intent))
        .digest('hex');
}

function transitionKey(workflowId, from, to) {
    return crypto.createHash('sha256')
        .update('postfiat.a666.primary.transition.v1\0')
        .update(workflowId)
        .update('\0')
        .update(from)
        .update('\0')
        .update(to)
        .digest('hex');
}

function workflowFile(root, workflowId) {
    return path.join(path.resolve(root), `${workflowId}.json`);
}

function atomicWrite(file, value) {
    fs.mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 });
    const temporary = `${file}.${process.pid}.${crypto.randomBytes(8).toString('hex')}.tmp`;
    const fd = fs.openSync(temporary, 'wx', 0o600);
    try {
        fs.writeFileSync(fd, `${JSON.stringify(value, null, 2)}\n`);
        fs.fsyncSync(fd);
    } finally {
        fs.closeSync(fd);
    }
    fs.renameSync(temporary, file);
    const directory = fs.openSync(path.dirname(file), 'r');
    try { fs.fsyncSync(directory); } finally { fs.closeSync(directory); }
}

function validateIntent(intent) {
    if (!intent || typeof intent !== 'object' || Array.isArray(intent)) throw new Error('intent must be an object');
    for (const field of [
        'quote_id',
        'route_id',
        'route_config_digest',
        'bob_ethereum_recipient',
        'usdc_input_atoms',
        'wa666_output_atoms',
    ]) {
        if (!nonEmptyString(intent[field])) throw new Error(`intent is missing ${field}`);
    }
    if (!/^0x[0-9a-f]{40}$/.test(intent.bob_ethereum_recipient)) {
        throw new Error('Bob Ethereum recipient must be a lowercase address');
    }
    if (!/^[0-9]+$/.test(intent.usdc_input_atoms) || !/^[0-9]+$/.test(intent.wa666_output_atoms)) {
        throw new Error('intent amounts must be unsigned integer strings');
    }
}

function rejectSecrets(value, pathParts = []) {
    if (!value || typeof value !== 'object') return;
    for (const [key, child] of Object.entries(value)) {
        const nextPath = [...pathParts, key];
        if (FORBIDDEN_KEYS.test(key)) throw new Error(`workflow must not persist secret field ${nextPath.join('.')}`);
        rejectSecrets(child, nextPath);
    }
}

function stableJson(value) {
    if (Array.isArray(value)) return `[${value.map(stableJson).join(',')}]`;
    if (value && typeof value === 'object') {
        return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`).join(',')}}`;
    }
    return JSON.stringify(value);
}

function nonEmptyString(value) {
    return typeof value === 'string' && value.trim().length > 0;
}

module.exports = {
    SCHEMA,
    STATES,
    createWorkflow,
    loadWorkflow,
    nextAction,
    transitionWorkflow,
    transitionKey,
    workflowIdForIntent,
};
