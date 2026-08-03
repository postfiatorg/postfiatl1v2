'use strict';

const assert = require('assert');
const {
    configFromEnv,
    create,
    ML_DSA_65_PUBLIC_KEY_HEX_LENGTH,
    ML_DSA_65_SIGNATURE_HEX_LENGTH,
    validateQuoteRequest,
    validateSignedIntent,
} = require('./pftl-private-swap');

const WALLET = 'pf' + '11'.repeat(20);
const ROUTE = 'pftl-a666-ethereum-wA666-usdc-v1';

function signedIntent(overrides = {}) {
    return {
        signed_intent: {
            schema: 'postfiat.pftl_swap.signed_intent.v1',
            algorithm_id: 'ML-DSA-65',
            public_key_hex: '22'.repeat(ML_DSA_65_PUBLIC_KEY_HEX_LENGTH / 2),
            signature_hex: '33'.repeat(ML_DSA_65_SIGNATURE_HEX_LENGTH / 2),
            intent: {
                schema: 'postfiat.pftl_swap.intent.v1',
                chain_id: 'postfiat-wan-devnet-2',
                genesis_hash: '44'.repeat(48),
                protocol_version: 1,
                principal: WALLET,
                controlled_wallet_id: WALLET,
                route_id: ROUTE,
                direction: 'issue',
                output_mode: 'private',
                input_reference: 'transparent-pfusdc',
                input_amount_atoms: 1005000,
                minimum_output_amount_atoms: 1000000,
                maximum_fee_atoms: 100,
                quote_id: '55'.repeat(48),
                pricing_nav_epoch: 8,
                policy_hash: '66'.repeat(48),
                expiry_height: 790,
                idempotency_key: 'browser-private-issue-01',
                ...overrides,
            },
        },
    };
}

assert.strictEqual(configFromEnv({}).configured, false);
assert.strictEqual(configFromEnv({
    PFTL_PRIVATE_SWAP_URL: 'https://example.com',
    PFTL_PRIVATE_SWAP_CONTROLLED_WALLET_ID: WALLET,
}).configured, false);
assert.strictEqual(configFromEnv({
    PFTL_PRIVATE_SWAP_URL: 'http://127.0.0.1:39798',
    PFTL_PRIVATE_SWAP_CONTROLLED_WALLET_ID: WALLET,
    PFTL_PRIVATE_SWAP_ROUTE_ID: ROUTE,
}).configured, true);
assert.strictEqual(configFromEnv({
    PFTL_PRIVATE_SWAP_URL: 'http://127.0.0.1:39798',
    PFTL_PRIVATE_SWAP_CONTROLLED_WALLET_ID: WALLET,
    PFTL_PRIVATE_SWAP_ROUTE_ID: ROUTE,
    PFTL_PRIVATE_SWAP_TIMEOUT_MS: 'not-a-number',
}).configured, false);

assert.deepStrictEqual(validateQuoteRequest({
    direction: 'issue', nav_amount_atoms: '1000000', output_mode: 'private',
}), { direction: 'issue', nav_amount_atoms: 1000000, output_mode: 'private' });
assert.throws(() => validateQuoteRequest({
    direction: 'issue', nav_amount_atoms: '1000001', output_mode: 'private',
}), /safe bound/);

assert.deepStrictEqual(validateSignedIntent(signedIntent(), WALLET, ROUTE), signedIntent());
assert.throws(() => validateSignedIntent(signedIntent({ principal: 'pf' + '77'.repeat(20) }), WALLET, ROUTE),
    /wallet proxy boundary/);
assert.throws(() => validateSignedIntent(signedIntent({ private_key_hex: '88' }), WALLET, ROUTE),
    /wallet proxy boundary/);
assert.throws(() => validateSignedIntent(signedIntent({ input_reference: { note: 'hidden' } }), WALLET, ROUTE),
    /wallet proxy boundary/);
assert.throws(() => validateSignedIntent(signedIntent({ input_reference: 'private/note' }), WALLET, ROUTE),
    /wallet proxy boundary/);
assert.throws(() => validateSignedIntent(signedIntent({ idempotency_key: 'browser.private.issue' }), WALLET, ROUTE),
    /wallet proxy boundary/);
assert.throws(() => validateSignedIntent(signedIntent({ pricing_nav_epoch: 1.5 }), WALLET, ROUTE),
    /wallet proxy boundary/);
assert.throws(() => validateSignedIntent(signedIntent({ route_id: 'pftl-substituted-route' }), WALLET, ROUTE),
    /wallet proxy boundary/);
const extraEnvelope = signedIntent();
extraEnvelope.signed_intent.backup = 'forbidden';
assert.throws(() => validateSignedIntent(extraEnvelope, WALLET, ROUTE), /wallet proxy boundary/);
const shortSignature = signedIntent();
shortSignature.signed_intent.signature_hex = '33';
assert.throws(() => validateSignedIntent(shortSignature, WALLET, ROUTE), /wallet proxy boundary/);

(async () => {
    const originalFetch = global.fetch;
    global.fetch = async () => { throw new Error('loopback refused'); };
    try {
        const adapter = create(null, {
            PFTL_PRIVATE_SWAP_URL: 'http://127.0.0.1:39798',
            PFTL_PRIVATE_SWAP_CONTROLLED_WALLET_ID: WALLET,
            PFTL_PRIVATE_SWAP_ROUTE_ID: ROUTE,
        });
        const unreachable = await adapter.pftlPrivateSwapReadiness();
        assert.strictEqual(unreachable.ok, false);
        assert.strictEqual(unreachable.code, 'pftl_private_swap_unreachable');
        assert.strictEqual(unreachable.http_status, 503);
    } finally {
        global.fetch = originalFetch;
    }
    console.log('PFTL private-primary wallet boundary regression passed');
})().catch(error => {
    console.error(error);
    process.exitCode = 1;
});
