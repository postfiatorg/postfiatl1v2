import assert from 'node:assert/strict';
import { createHash, webcrypto } from 'node:crypto';
import test from 'node:test';
import bolt11 from '@atomiqlabs/bolt11';

import {
  LIGHTNING_TO_PFTL,
  PFTL_TO_LIGHTNING,
  LightningNavcoinClient,
  assertFinalizedEscrowMatches,
  assertOfframpLockExecutable,
  assertSecretFreeCoordinatorValue,
  buildPayerFeeAcknowledgement,
  buildEscrowCancelOperation,
  buildEscrowCreateOperation,
  buildEscrowFinishOperation,
  safeToRevealInvoice,
  validateCoordinatorStatus,
  validateSwapSnapshot,
  verifyPhoenixPreimage,
} from './lightning-navcoin.js';
import { sha3_384DomainHex } from './evm.js';
import { canonicalQuoteBytes } from './lightning-quote-signature.js';
import { LIGHTNING_NAVCOIN_RELEASE_PINS } from './lightning-navcoin-release.js';

const PREIMAGE = 'ab'.repeat(32);
const PAYMENT_HASH = createHash('sha256').update(Buffer.from(PREIMAGE, 'hex')).digest('hex');
const OWNER = `pf${'11'.repeat(20)}`;
const RECIPIENT = `pf${'22'.repeat(20)}`;
const SWAP_ID = '33'.repeat(32);
const ESCROW_ID = '44'.repeat(48);
const GENESIS = LIGHTNING_NAVCOIN_RELEASE_PINS.pftlGenesisHash;
const ASSET_ID = LIGHTNING_NAVCOIN_RELEASE_PINS.pftlAssetId;
const STATE_ROOT = 'aa'.repeat(48);
const BLOCK_TIP = 'bb'.repeat(48);
const NOW = 2_000_000_000;
const INVOICE_PRIVATE_KEY = '00'.repeat(31) + '01';

const encodedInvoice = bolt11.encode({
  millisatoshis: '1000000',
  timestamp: NOW,
  tags: [
    { tagName: 'payment_hash', data: PAYMENT_HASH },
    { tagName: 'payment_secret', data: 'cd'.repeat(32) },
    { tagName: 'description', data: 'PostFiat CONTROLLED NAVcoin test' },
    { tagName: 'expire_time', data: 900 },
    { tagName: 'min_final_cltv_expiry', data: 144 },
    {
      tagName: 'feature_bits',
      data: {
        word_length: 4,
        var_onion_optin: { supported: true, required: false },
        payment_secret: { supported: true, required: false },
        basic_mpp: { supported: true, required: false },
      },
    },
  ],
}, false);
const signedInvoice = bolt11.sign(encodedInvoice, INVOICE_PRIVATE_KEY);
const INVOICE = signedInvoice.paymentRequest;
const INVOICE_PAYEE = signedInvoice.payeeNodeKey;
const TEST_RELEASE_PINS = Object.freeze({
  ...LIGHTNING_NAVCOIN_RELEASE_PINS,
  lndIdentityPubkeyHex: INVOICE_PAYEE,
});

function quote(direction = LIGHTNING_TO_PFTL) {
  return {
    schema: 'postfiat.lightning_submarine_quote.v1',
    swap_id: SWAP_ID,
    quote_expires_unix: NOW + 120,
    direction,
    payment_hash: PAYMENT_HASH,
    lightning_network: 'bitcoin',
    invoice: INVOICE,
    invoice_payee: INVOICE_PAYEE,
    invoice_amount_msat: '1000000',
    invoice_expiry_unix: NOW + 900,
    min_final_cltv_delta: 144,
    max_total_cltv_delta: 288,
    pftl_chain_id: LIGHTNING_NAVCOIN_RELEASE_PINS.pftlChainId,
    pftl_genesis_hash: GENESIS,
    pftl_asset_id: ASSET_ID,
    pftl_amount_atoms: '1000000',
    pftl_owner: direction === LIGHTNING_TO_PFTL ? OWNER : RECIPIENT,
    pftl_owner_sequence: 8,
    pftl_recipient: direction === LIGHTNING_TO_PFTL ? RECIPIENT : OWNER,
    expected_escrow_id: ESCROW_ID,
    condition: `a0258020${PAYMENT_HASH}810120`,
    finish_after: 0,
    cancel_after: 1200,
    latest_lightning_start_unix: NOW + 300,
    rate_numerator: '1',
    rate_denominator: '1',
    coordinator_fee_atoms: '0',
    nav_epoch: LIGHTNING_NAVCOIN_RELEASE_PINS.pftlNavEpoch,
    nav_reserve_packet_hash:
      LIGHTNING_NAVCOIN_RELEASE_PINS.pftlNavReservePacketHash,
    custody_class: 'NON_CUSTODIAL_HASHLOCK',
    atomicity_class: 'CONDITIONAL_HTLC',
    timeout_clock_class: 'OFFCHAIN_CROSS_LEDGER_POLICY',
    asset_control_class: 'CONTROLLED_ISSUED_ASSET',
  };
}

function pftl(direction = LIGHTNING_TO_PFTL) {
  const q = quote(direction);
  return {
    chain_id: q.pftl_chain_id,
    genesis_hash: q.pftl_genesis_hash,
    asset_id: q.pftl_asset_id,
    nav_epoch: q.nav_epoch,
    nav_reserve_packet_hash: q.nav_reserve_packet_hash,
    build_git_revision: LIGHTNING_NAVCOIN_RELEASE_PINS.pftlBuildGitRevision,
    state_root: STATE_ROOT,
    block_tip_hash: BLOCK_TIP,
    quorum: { observed: 6, required: 6, validator_count: 6, converged: true },
    receipt: { accepted: true, code: 'accepted', tx_id: '99'.repeat(48) },
    height: 42,
    wallet_balance_atoms: '5000000',
    escrow: {
      escrow_id: q.expected_escrow_id,
      owner: q.pftl_owner,
      recipient: q.pftl_recipient,
      asset_id: q.pftl_asset_id,
      amount: q.pftl_amount_atoms,
      condition: q.condition,
      finish_after: q.finish_after,
      cancel_after: q.cancel_after,
      state: 'open',
    },
  };
}

function independentPftlStatus(overrides = {}) {
  return {
    ok: true,
    result: {
      status: 'running',
      chain_id: LIGHTNING_NAVCOIN_RELEASE_PINS.pftlChainId,
      genesis_hash: LIGHTNING_NAVCOIN_RELEASE_PINS.pftlGenesisHash,
      build_git_revision:
        LIGHTNING_NAVCOIN_RELEASE_PINS.pftlBuildGitRevision,
      block_height: 42,
      block_tip_hash: BLOCK_TIP,
      state_root: STATE_ROOT,
      validator_count: 6,
      ...overrides,
    },
  };
}

function statusPayload(overrides = {}) {
  return {
    ok: true,
    result: {
      schema: 'postfiat.lightning_navcoin.status.v1',
      mode: 'ARMED',
      can_execute: true,
      hold_reasons: [],
      lightning_network: 'bitcoin',
      trust_class: 'CONTROLLED',
      atomicity_claim: 'non-custodial, conditionally atomic, COORDINATOR-TRUSTED timing',
      quote_signer_public_key_hex:
        LIGHTNING_NAVCOIN_RELEASE_PINS.quoteSignerPublicKeyHex,
      lnd: {
        network: 'mainnet',
        identity_pubkey: INVOICE_PAYEE,
        synced_to_chain: true,
      },
      pftl: pftl(),
      pftl_valuation_binding: {
        schema: 'postfiat.lightning_pftl_valuation_binding.v1',
        height: 42,
        block_tip_hash: BLOCK_TIP,
        state_root: STATE_ROOT,
        asset_id: ASSET_ID,
        nav_epoch: LIGHTNING_NAVCOIN_RELEASE_PINS.pftlNavEpoch,
        nav_per_unit_usd_e8: '1035074022',
        reserve_packet_hash:
          LIGHTNING_NAVCOIN_RELEASE_PINS.pftlNavReservePacketHash,
        valuation_unit: 'USD_PER_WHOLE_ASSET_UNIT',
        valuation_scale: 100000000,
        validator_count: 6,
        ledger_sha256: Array(6).fill('12'.repeat(32)),
        chain_tip_sha256: Array(6).fill('34'.repeat(32)),
        state_verification_sha256: Array(6).fill('56'.repeat(32)),
      },
      pftl_proof_assurance: {
        schema: 'postfiat.lightning_pftl_proof_assurance.v1',
        lifecycle: [
          'nav_reserve_submit',
          'nav_reserve_attest',
          'nav_epoch_finalize',
        ],
        profile: 'multi-fetch-quorum',
        attestation_count: 1,
        proof_bytes_stored_on_chain: true,
        consensus_native_groth16_verification: false,
      },
      pricing: { btc_usd_e8: '10000000000000' },
      limits: {
        per_run_usd_e8: '500000000',
        total_usd_e8: '2000000000',
        max_amount_msat: '10000000',
        max_fee_msat: '10000',
      },
      ...overrides,
    },
  };
}

function swapPayload(direction = LIGHTNING_TO_PFTL, overrides = {}) {
  return {
    ok: true,
    result: {
      swap_id: SWAP_ID,
      state: 'PFTL_LOCK_FINAL',
      can_execute: true,
      hold_reasons: [],
      quote: quote(direction),
      lightning: { state: 'OPEN' },
      pftl: pftl(direction),
      ...overrides,
    },
  };
}

function pendingSwapPayload(overrides = {}) {
  return {
    ok: true,
    result: {
      schema: 'postfiat.lightning_navcoin.swap.v1',
      swap_id: SWAP_ID,
      state: 'PFTL_LOCK_SUBMITTED',
      direction: LIGHTNING_TO_PFTL,
      payment_hash: PAYMENT_HASH,
      invoice_amount_msat: '1000000',
      wallet_address: RECIPIENT,
      pftl_amount_atoms: '1000000',
      can_execute: false,
      hold_reasons: ['nazgul_value_authorization_required'],
      pftl: {
        asset_id: ASSET_ID,
        nav_epoch: 2,
        nav_reserve_packet_hash: '88'.repeat(48),
      },
      ...overrides,
    },
  };
}

function response(payload, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: () => null },
    text: async () => JSON.stringify(payload),
  };
}

function memoryStorage() {
  const values = new Map();
  return {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, String(value)),
    removeItem: key => values.delete(key),
  };
}

async function signQuoteEnvelope(quoteValue) {
  const keys = await webcrypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']);
  const publicKey = new Uint8Array(
    await webcrypto.subtle.exportKey('raw', keys.publicKey),
  );
  const domain = new TextEncoder().encode(
    'postfiat.lightning_submarine_quote.v1\u0000',
  );
  const keyDomain = new TextEncoder().encode(
    'postfiat.lightning_submarine_quote.key.v1\u0000',
  );
  const quoteBytes = canonicalQuoteBytes(quoteValue);
  const length = new Uint8Array(4);
  new DataView(length.buffer).setUint32(0, quoteBytes.length, false);
  const join = (...values) => {
    const result = new Uint8Array(
      values.reduce((total, value) => total + value.length, 0),
    );
    let offset = 0;
    for (const value of values) {
      result.set(value, offset);
      offset += value.length;
    }
    return result;
  };
  const signature = new Uint8Array(await webcrypto.subtle.sign(
    'Ed25519',
    keys.privateKey,
    join(domain, length, quoteBytes),
  ));
  const keyId = Buffer.from(await webcrypto.subtle.digest(
    'SHA-256',
    join(keyDomain, publicKey),
  )).toString('hex');
  return {
    publicKeyHex: Buffer.from(publicKey).toString('hex'),
    envelope: {
      algorithm: 'Ed25519',
      key_id: keyId,
      public_key: Buffer.from(publicKey).toString('base64url'),
      quote: quoteValue,
      signature: Buffer.from(signature).toString('base64url'),
    },
  };
}

test('status validation is fail-closed for network, mode, and execution mismatch', () => {
  const status = validateCoordinatorStatus(statusPayload(), TEST_RELEASE_PINS);
  assert.equal(status.mode, 'ARMED');
  assert.equal(status.pftl.quorum.observed, 6);
  assert.equal(status.maxAmountMsat, '10000000');
  assert.equal(status.maxFeeMsat, '10000');
  assert.equal(status.valuationBinding.validatorCount, 6);
  assert.equal(status.proofAssurance.consensusNativeGroth16Verification, false);

  assert.throws(
    () => validateCoordinatorStatus(
      statusPayload({ lightning_network: 'regtest' }),
      TEST_RELEASE_PINS,
    ),
    /not bound to Bitcoin mainnet/,
  );
  assert.throws(
    () => validateCoordinatorStatus(
      statusPayload({ mode: 'HOLD', can_execute: true }),
      TEST_RELEASE_PINS,
    ),
    /cannot execute unless mode is ARMED/,
  );
  assert.throws(
    () => validateCoordinatorStatus(statusPayload({
      quote_signer_public_key_hex: 'aa'.repeat(32),
    }), TEST_RELEASE_PINS),
    /reviewed wallet release pin/,
  );
  assert.throws(
    () => validateCoordinatorStatus(statusPayload({
      pftl: { ...pftl(), asset_id: 'aa'.repeat(48) },
    }), TEST_RELEASE_PINS),
    /PFTL asset id.*reviewed wallet release pin/,
  );
  assert.throws(
    () => validateCoordinatorStatus(statusPayload()),
    /LND identity.*reviewed wallet release pin/,
    'the production release remains held until the interactive LND identity is pinned',
  );
  assert.throws(
    () => validateCoordinatorStatus(statusPayload({
      limits: {
        per_run_usd_e8: '500000000',
        total_usd_e8: '2000000000',
        max_amount_msat: '10000000',
      },
    }), TEST_RELEASE_PINS),
    /omits the real-value dust caps/,
  );
  assert.throws(
    () => validateCoordinatorStatus(statusPayload({
      pftl_proof_assurance: {
        schema: 'postfiat.lightning_pftl_proof_assurance.v1',
        lifecycle: [
          'nav_reserve_submit',
          'nav_reserve_attest',
          'nav_epoch_finalize',
        ],
        profile: 'multi-fetch-quorum',
        attestation_count: 1,
        proof_bytes_stored_on_chain: true,
        consensus_native_groth16_verification: true,
      },
    }), TEST_RELEASE_PINS),
    /proof-assurance boundary/,
  );
});

test('payer fee acknowledgement enforces the reserved fee and all-in USD caps', () => {
  const status = validateCoordinatorStatus(statusPayload(), TEST_RELEASE_PINS);
  const evidence = buildPayerFeeAcknowledgement(status, '1000000', '10', NOW);
  assert.deepEqual(evidence, {
    schema: 'postfiat.lightning_payer_fee_acknowledgement.v1',
    principal_msat: '1000000',
    displayed_fee_msat: '10000',
    coordinator_max_fee_msat: '10000',
    all_in_usd_e8: '101000000',
    per_run_usd_e8: '500000000',
    acknowledged_at_unix: NOW,
  });
  assert.throws(
    () => buildPayerFeeAcknowledgement(status, '1000000', '11', NOW),
    /reserved coordinator fee cap/,
  );
  const tinyRunCap = {
    ...status,
    perRunUsdE8: '10000000',
    maxFeeMsat: '10000',
  };
  assert.throws(
    () => buildPayerFeeAcknowledgement(tinyRunCap, '1000000', '1', NOW),
    /per-run USD cap/,
  );
});

test('swap validation binds payment hash, condition, invoice, escrow, and quorum', () => {
  const swap = validateSwapSnapshot(swapPayload());
  assert.equal(swap.quote.paymentHash, PAYMENT_HASH);
  assert.equal(swap.quote.escrowId, ESCROW_ID);
  assert.equal(swap.pftl.receipt.accepted, true);

  const malformed = swapPayload();
  malformed.result.quote.condition = `a0258020${'ff'.repeat(32)}810120`;
  assert.throws(() => validateSwapSnapshot(malformed), /does not canonically bind/);

  const wrongNetwork = swapPayload();
  wrongNetwork.result.quote.lightning_network = 'regtest';
  assert.throws(() => validateSwapSnapshot(wrongNetwork), /not bound to Bitcoin mainnet/);
});

test('coordinator traffic rejects custody material, preimages, and fulfillments recursively', () => {
  assert.throws(
    () => assertSecretFreeCoordinatorValue({ nested: { payment_preimage: PREIMAGE } }),
    /forbidden secret field/,
  );
  assert.throws(
    () => assertSecretFreeCoordinatorValue({ event: { fulfillment: `a0228020${PREIMAGE}` } }),
    /forbidden secret field/,
  );
  assert.throws(
    () => assertSecretFreeCoordinatorValue({ backup_json: '{"seed":"bad"}' }),
    /forbidden custody material/,
  );
  assert.doesNotThrow(() => assertSecretFreeCoordinatorValue({
    payment_hash: PAYMENT_HASH,
    condition: `a0258020${PAYMENT_HASH}810120`,
  }));
});

test('same-origin client sends CSRF/idempotency controls and reports only public PFTL tx ids', async () => {
  const calls = [];
  const fetchImpl = async (url, options) => {
    calls.push({ url, options });
    if (url.endsWith('/status')) return response(statusPayload());
    return response(pendingSwapPayload());
  };
  const client = new LightningNavcoinClient({
    fetchImpl,
    csrf: 'ab'.repeat(32),
    releasePins: TEST_RELEASE_PINS,
  });

  await client.status();
  await client.createQuote({
    direction: LIGHTNING_TO_PFTL,
    amountMsat: '1000000',
    walletAddress: RECIPIENT,
    clientRequestId: SWAP_ID,
  });
  await client.swap(SWAP_ID);
  await client.notifyPftlLock(SWAP_ID, '12'.repeat(48));
  await client.notifyPftlFinish(SWAP_ID, '34'.repeat(48));
  await client.notifyPftlCancel(SWAP_ID, '56'.repeat(48));

  assert.deepEqual(calls.map(call => call.url), [
    '/api/lightning-navcoin/v1/status',
    '/api/lightning-navcoin/v1/quotes',
    `/api/lightning-navcoin/v1/swaps/${SWAP_ID}`,
    `/api/lightning-navcoin/v1/swaps/${SWAP_ID}/pftl-lock`,
    `/api/lightning-navcoin/v1/swaps/${SWAP_ID}/pftl-finish`,
    `/api/lightning-navcoin/v1/swaps/${SWAP_ID}/pftl-cancel`,
  ]);
  assert.deepEqual(
    calls.map(call => call.options.method),
    ['GET', 'POST', 'GET', 'POST', 'POST', 'POST'],
  );
  assert.equal(calls[1].options.credentials, 'same-origin');
  assert.equal(calls[1].options.redirect, 'error');
  assert.equal(calls[1].options.headers['X-PostFiat-CSRF'], 'ab'.repeat(32));
  assert.equal(calls[1].options.headers.Authorization, undefined, 'backend bearer stays outside browser JavaScript');
  assert.deepEqual(JSON.parse(calls[1].options.body), {
    direction: LIGHTNING_TO_PFTL,
    amount_msat: '1000000',
    wallet_address: RECIPIENT,
    client_request_id: SWAP_ID,
  });
  assert.equal(calls[2].options.body, undefined, 'polling must remain secret-free GET');
  assert.deepEqual(JSON.parse(calls[3].options.body), { tx_id: '12'.repeat(48) });
  assert.deepEqual(JSON.parse(calls[4].options.body), { tx_id: '34'.repeat(48) });
  assert.deepEqual(JSON.parse(calls[5].options.body), { tx_id: '56'.repeat(48) });
});

test('dropped quote response reuses one durable request id and recovers the same swap', async () => {
  const storage = memoryStorage();
  let committed = null;
  let exposureCount = 0;
  let postCount = 0;
  let getCount = 0;
  const fetchImpl = async (url, options) => {
    if (options.method === 'POST' && url.endsWith('/quotes')) {
      postCount += 1;
      const body = JSON.parse(options.body);
      assert.match(body.client_request_id, /^[0-9a-f]{64}$/);
      assert.notEqual(
        storage.getItem('postfiat.lightning_navcoin.quote_request.v1'),
        null,
        'request id must be durable before POST',
      );
      if (committed === null) {
        exposureCount += 1;
        committed = pendingSwapPayload({
          swap_id: body.client_request_id,
        });
      } else {
        assert.equal(body.client_request_id, committed.result.swap_id);
      }
      throw new TypeError('simulated response loss after coordinator commit');
    }
    if (options.method === 'GET' && url.includes('/swaps/')) {
      getCount += 1;
      assert.ok(committed);
      assert.ok(url.endsWith(committed.result.swap_id));
      return response(committed);
    }
    throw new Error(`unexpected request ${options.method} ${url}`);
  };
  const client = new LightningNavcoinClient({
    fetchImpl,
    csrf: 'ab'.repeat(32),
    quoteRequestStorage: storage,
  });
  const input = {
    direction: LIGHTNING_TO_PFTL,
    amountMsat: '1000000',
    walletAddress: RECIPIENT,
  };
  const first = await client.createQuote(input);
  const retry = await client.createQuote(input);
  assert.equal(first.swapId, retry.swapId);
  assert.equal(exposureCount, 1);
  assert.equal(postCount, 2);
  assert.equal(getCount, 2);
  assert.equal(client.clearQuoteRequest(first.swapId), true);
  assert.equal(
    storage.getItem('postfiat.lightning_navcoin.quote_request.v1'),
    null,
  );
});

test('client rejects a secret-bearing poll response before rendering it', async () => {
  const client = new LightningNavcoinClient({
    csrf: 'ab'.repeat(32),
    releasePins: TEST_RELEASE_PINS,
    fetchImpl: async () => response({
      ...swapPayload(),
      result: {
        ...swapPayload().result,
        lightning: { state: 'SETTLED', payment_preimage: PREIMAGE },
      },
    }),
  });
  await assert.rejects(() => client.swap(SWAP_ID), /forbidden secret field/);
});

test('client rejects coordinator substitution of both status signer and signed quote', async () => {
  const signed = await signQuoteEnvelope(quote());
  const payload = swapPayload();
  delete payload.result.quote;
  payload.result.signed_quote = signed.envelope;
  const substituting = new LightningNavcoinClient({
    csrf: 'ab'.repeat(32),
    releasePins: TEST_RELEASE_PINS,
    fetchImpl: async url => (
      url.endsWith('/status')
        ? response(statusPayload({
            quote_signer_public_key_hex: signed.publicKeyHex,
          }))
        : response(payload)
    ),
  });
  await assert.rejects(
    () => substituting.status(),
    /reviewed wallet release pin/,
  );

  const fixedStatus = new LightningNavcoinClient({
    csrf: 'ab'.repeat(32),
    releasePins: TEST_RELEASE_PINS,
    fetchImpl: async url => (
      url.endsWith('/status')
        ? response(statusPayload())
        : response(payload)
    ),
  });
  await fixedStatus.status();
  await assert.rejects(
    () => fixedStatus.swap(SWAP_ID),
    /does not match coordinator status pin/,
  );
});

test('pending on-ramp withholds invoice but remains safely pollable', async () => {
  const pending = validateSwapSnapshot(pendingSwapPayload());
  assert.equal(pending.quote, null);
  assert.equal(pending.swapId, SWAP_ID);
  assert.equal(pending.invoiceAmountMsat, '1000000');
  assert.equal(pending.walletAddress, RECIPIENT);
  assert.equal(pending.canExecute, false);

  const client = new LightningNavcoinClient({
    csrf: 'ab'.repeat(32),
    releasePins: TEST_RELEASE_PINS,
    fetchImpl: async () => response(pendingSwapPayload()),
  });
  const created = await client.createQuote({
    direction: LIGHTNING_TO_PFTL,
    amountMsat: '1000000',
    walletAddress: RECIPIENT,
    clientRequestId: SWAP_ID,
  });
  assert.equal(created.swapId, SWAP_ID);
  assert.equal(created.quote, null);

  assert.throws(
    () => validateSwapSnapshot(pendingSwapPayload({ can_execute: true })),
    /pending swap cannot execute/,
  );
  await assert.rejects(
    () => new LightningNavcoinClient({
      csrf: 'ab'.repeat(32),
      releasePins: TEST_RELEASE_PINS,
      fetchImpl: async () => response(pendingSwapPayload({
        wallet_address: OWNER,
      })),
    }).createQuote({
      direction: LIGHTNING_TO_PFTL,
      amountMsat: '1000000',
      walletAddress: RECIPIENT,
      clientRequestId: SWAP_ID,
    }),
    /substituted the requesting wallet/,
  );
});

test('Phoenix preimage is hashed locally and encoded as canonical fulfillment', async () => {
  const verified = await verifyPhoenixPreimage(PREIMAGE, PAYMENT_HASH, webcrypto.subtle);
  assert.equal(verified.paymentHash, PAYMENT_HASH);
  assert.equal(verified.fulfillment, `a0228020${PREIMAGE}`);

  await assert.rejects(
    () => verifyPhoenixPreimage('ff'.repeat(32), PAYMENT_HASH, webcrypto.subtle),
    /does not match/,
  );
  await assert.rejects(
    () => verifyPhoenixPreimage(PREIMAGE.toUpperCase(), PAYMENT_HASH, webcrypto.subtle),
    /canonical lowercase/,
  );
});

test('wallet constructs only the typed local escrow operation for its direction', async () => {
  const onramp = validateSwapSnapshot(swapPayload(LIGHTNING_TO_PFTL));
  const proof = await verifyPhoenixPreimage(PREIMAGE, PAYMENT_HASH, webcrypto.subtle);
  assert.deepEqual(buildEscrowFinishOperation(onramp, proof.fulfillment, RECIPIENT), {
    operation: 'escrow_finish',
    escrow_id: ESCROW_ID,
    owner: OWNER,
    recipient: RECIPIENT,
    fulfillment: `a0228020${PREIMAGE}`,
  });
  assert.throws(
    () => buildEscrowFinishOperation(onramp, proof.fulfillment, OWNER),
    /not the quoted escrow recipient/,
  );

  const offramp = validateSwapSnapshot(swapPayload(PFTL_TO_LIGHTNING));
  assert.deepEqual(buildEscrowCreateOperation(offramp, RECIPIENT), {
    operation: 'escrow_create',
    owner: RECIPIENT,
    recipient: OWNER,
    asset_id: ASSET_ID,
    amount: '1000000',
    condition: `a0258020${PAYMENT_HASH}810120`,
    finish_after: 0,
    cancel_after: 1200,
  });
  const refundable = validateSwapSnapshot(swapPayload(PFTL_TO_LIGHTNING, {
    state: 'REFUND_ELIGIBLE',
    can_execute: false,
    hold_reasons: ['swap_state_not_executable'],
    pftl: { ...pftl(PFTL_TO_LIGHTNING), height: 1200 },
  }));
  assert.deepEqual(buildEscrowCancelOperation(refundable, RECIPIENT), {
    operation: 'escrow_cancel',
    escrow_id: ESCROW_ID,
    owner: RECIPIENT,
  });
  assert.throws(
    () => buildEscrowCancelOperation(refundable, OWNER),
    /not the quoted escrow owner/,
  );
});

test('off-ramp wallet rechecks a pure fresh snapshot immediately before signing', () => {
  const executable = validateSwapSnapshot(swapPayload(PFTL_TO_LIGHTNING, {
    state: 'PFTL_LOCK_SUBMITTED',
  }));
  assert.equal(assertOfframpLockExecutable(executable, NOW), executable);
  assert.throws(
    () => assertOfframpLockExecutable(executable, NOW + 120),
    /execution window expired/,
  );
  const held = validateSwapSnapshot(swapPayload(PFTL_TO_LIGHTNING, {
    state: 'PFTL_LOCK_SUBMITTED',
    can_execute: false,
    hold_reasons: ['value_authorization_expired'],
  }));
  assert.throws(
    () => assertOfframpLockExecutable(held, NOW),
    /not executable/,
  );
});

test('invoice presentation needs arming, accepted 6-of-6 lock, and unexpired bounds', () => {
  const status = validateCoordinatorStatus(statusPayload(), TEST_RELEASE_PINS);
  const swap = validateSwapSnapshot(swapPayload());
  assert.equal(safeToRevealInvoice(status, swap, NOW, TEST_RELEASE_PINS), true);

  const heldStatus = validateCoordinatorStatus(statusPayload({
    mode: 'HOLD',
    can_execute: false,
    hold_reasons: ['founder dust authorization absent'],
  }), TEST_RELEASE_PINS);
  assert.equal(
    safeToRevealInvoice(heldStatus, swap, NOW, TEST_RELEASE_PINS),
    false,
  );

  const rejected = validateSwapSnapshot(swapPayload(LIGHTNING_TO_PFTL, {
    pftl: {
      ...pftl(),
      receipt: { accepted: false, code: 'rejected' },
    },
  }));
  assert.equal(
    safeToRevealInvoice(status, rejected, NOW, TEST_RELEASE_PINS),
    false,
  );
  assert.equal(
    safeToRevealInvoice(status, swap, NOW + 901, TEST_RELEASE_PINS),
    false,
  );
});

test('independent escrow read must exactly match the signed quote and wallet', () => {
  const swap = validateSwapSnapshot(swapPayload());
  const coordinatorStatus = validateCoordinatorStatus(
    statusPayload(),
    TEST_RELEASE_PINS,
  );
  const independentStatus = independentPftlStatus();
  const rpc = {
    ok: true,
    result: {
      found: true,
      escrow: {
        escrow_id: ESCROW_ID,
        owner: OWNER,
        recipient: RECIPIENT,
        asset_id: ASSET_ID,
        amount: '1000000',
        condition_hash: sha3_384DomainHex(
          'postfiat.escrow_condition_hash.v1',
          `a0258020${PAYMENT_HASH}810120`,
        ),
        finish_after: 0,
        cancel_after: 1200,
        state: 'open',
      },
    },
  };
  assert.equal(assertFinalizedEscrowMatches(
    swap,
    rpc,
    RECIPIENT,
    coordinatorStatus,
    independentStatus,
    independentStatus,
  ).state, 'open');

  const wrongAmount = structuredClone(rpc);
  wrongAmount.result.escrow.amount = '999999';
  assert.throws(
    () => assertFinalizedEscrowMatches(
      swap,
      wrongAmount,
      RECIPIENT,
      coordinatorStatus,
      independentStatus,
      independentStatus,
    ),
    /does not match/,
  );
  assert.throws(
    () => assertFinalizedEscrowMatches(
      swap,
      rpc,
      RECIPIENT,
      coordinatorStatus,
      independentPftlStatus({ state_root: 'cc'.repeat(48) }),
      independentPftlStatus({ state_root: 'cc'.repeat(48) }),
    ),
    /not on the coordinator six-validator consensus view/,
  );
  assert.throws(
    () => assertFinalizedEscrowMatches(
      swap,
      rpc,
      RECIPIENT,
      coordinatorStatus,
      independentStatus,
      independentPftlStatus({ block_height: 43 }),
    ),
    /changed around the escrow read/,
  );
});

test('off-ramp quote request accepts only a Bitcoin-mainnet invoice', async () => {
  let body;
  const client = new LightningNavcoinClient({
    csrf: 'ab'.repeat(32),
    releasePins: TEST_RELEASE_PINS,
    fetchImpl: async (_url, options) => {
      body = JSON.parse(options.body);
      return response(pendingSwapPayload({
        direction: PFTL_TO_LIGHTNING,
        wallet_address: RECIPIENT,
      }));
    },
  });
  await client.createQuote({
    direction: PFTL_TO_LIGHTNING,
    amountMsat: '1000000',
    walletAddress: RECIPIENT,
    invoice: 'lnbc10n1p5xyqqq',
    clientRequestId: SWAP_ID,
  });
  assert.equal(body.invoice, 'lnbc10n1p5xyqqq');

  await assert.rejects(
    () => client.createQuote({
      direction: PFTL_TO_LIGHTNING,
      amountMsat: '1000000',
      walletAddress: RECIPIENT,
      invoice: 'lnbcrt1regtest',
      clientRequestId: 'ee'.repeat(32),
    }),
    /Bitcoin-mainnet BOLT11/,
  );
});
