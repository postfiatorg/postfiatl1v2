import assert from 'node:assert/strict';
import { webcrypto } from 'node:crypto';
import test from 'node:test';

import {
  assertNoShieldedPrivateMaterial,
  buildAssetOrchardIngressPayload,
  deriveShieldedNoteVaultKey,
  LocalAssetOrchardProverClient,
  normalizeLocalProverReadiness,
  normalizeShieldedNavswapCapability,
  normalizeShieldedNavswapQuote,
  openShieldedNoteVault,
  reconcileShieldedNotes,
  sealShieldedNoteVault,
  shieldedPrivateEgressDisclosureFields,
  shieldedPrivateEgressDisclosureHash,
  spendableShieldedNotes,
  verifyAssetOrchardPrivateEgressJson,
  verifyAssetOrchardSwapActionJson,
  ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA,
  ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA,
  ASSET_ORCHARD_POOL_ID,
  ASSET_ORCHARD_SWAP_ACTION_SCHEMA,
  SHIELDED_NAVSWAP_EGRESS_POLICY_ID,
  SHIELDED_NAVSWAP_ROUTE,
} from './shielded-navswap.js';

test('shielded request guard rejects private wallet material', () => {
  const forbiddenBodies = [
    { seed: 'seed words' },
    { backupJson: { wallet: 'secret' } },
    { private_key: '0xabc' },
    { nested: { note_openings: ['opening'] } },
    { files: [{ note_file: '/tmp/note.json' }] },
    { spendKey: 'spend-secret' },
    { diversifier: '01'.repeat(11) },
    { g_d: '02'.repeat(32) },
    { pk_d: '03'.repeat(32) },
    { rho: '04'.repeat(32) },
    { psi: '05'.repeat(32) },
    { rcm: '06'.repeat(32) },
    { nk: '07'.repeat(32) },
    { rivk: '08'.repeat(32) },
    { rseed: '09'.repeat(32) },
    { spend_auth_signing_key: '0a'.repeat(32) },
    { full_viewing_key_hex: '0b'.repeat(32) },
  ];
  for (const body of forbiddenBodies) {
    assert.throws(
      () => assertNoShieldedPrivateMaterial(body),
      /forbidden private wallet material/,
    );
  }
  const oversizedSerializedAction = JSON.stringify({
    padding: 'x'.repeat(1_048_576),
    private_witness: { diversifier: '0c'.repeat(11) },
  });
  assert.throws(
    () => assertNoShieldedPrivateMaterial({ swap_action_json: oversizedSerializedAction }),
    /forbidden private wallet material/,
    'JSON-looking strings above the inspection limit must fail closed',
  );
  assert.doesNotThrow(() => assertNoShieldedPrivateMaterial({
    route: SHIELDED_NAVSWAP_ROUTE,
    wallet_address: 'pfwallet',
    asset_registry: [{ symbol: 'a652' }],
  }));
});

test('shielded capability normalization stays blocked without quote config', () => {
  const cap = normalizeShieldedNavswapCapability({
    enabled: true,
    can_quote: true,
    can_run: false,
    custody_boundary: 'wallet_local_note_keys_only',
    requires_local_prover: true,
    requires_note_scan: true,
    liquidity_mode: 'dark_pool',
    privacy_label: 'Private note-settled route',
    asset_registry: [
      {
        symbol: 'a651',
        asset_id: 'aa'.repeat(48),
        precision: 6,
        issuer: 'pfissuer',
        nav_source: 'finalized_nav_state',
        policy_hash: '11'.repeat(48),
        supported: true,
      },
      {
        symbol: 'a652',
        asset_id: 'bb'.repeat(48),
        precision: 6,
        issuer: 'pfissuer',
        nav_source: 'finalized_nav_state',
        policy_hash: '22'.repeat(48),
      },
    ],
    supported_pairs: [
      { from: 'pfusdc', to: 'a652', enabled: true },
      { from: 'a651', to: 'a652', enabled: true },
    ],
    local_prover: {
      local_only: true,
      ready: true,
      pool_id: ASSET_ORCHARD_POOL_ID,
      circuit_id: 'asset-orchard-swap-v1',
      k: 15,
      params_hash: '33'.repeat(48),
      vk_hash: '44'.repeat(48),
    },
  });
  assert.equal(cap.enabled, true);
  assert.equal(cap.can_quote, false);
  assert.equal(cap.can_run, false);
  assert.equal(cap.can_ingress, false);
  assert.equal(cap.adapter_can_run, false);
  assert.equal(cap.status, 'preflight_only');
  assert.equal(cap.asset_registry.find(asset => asset.symbol === 'a652').ok, true);
  assert.equal(cap.asset_registry.find(asset => asset.symbol === 'a652').supported, false);
  assert.equal(cap.supported_pairs[0].ok, false);
  assert.match(cap.supported_pairs[0].errors.join(' '), /pfusdc|display-only|registry/);
  assert.equal(cap.supported_pairs[1].ok, false);
  assert.match(cap.disabled_reason, /adapter cannot quote/);
});

test('shielded capability normalization allows Step 7 private swap submit', () => {
  const cap = normalizeShieldedNavswapCapability({
    enabled: true,
    can_quote: true,
    can_run: true,
    custody_boundary: 'wallet_local_private_swap_submit_boundary',
    requires_local_prover: true,
    requires_note_scan: true,
    liquidity_mode: 'pool_managed_note',
    asset_registry: [
      {
        symbol: 'a651',
        asset_id: 'aa'.repeat(48),
        precision: 6,
        issuer: 'pfissuer',
        nav_source: 'finalized_nav_state',
        policy_hash: '11'.repeat(32),
        supported: true,
      },
      {
        symbol: 'a652',
        asset_id: 'bb'.repeat(48),
        precision: 6,
        issuer: 'pfissuer',
        nav_source: 'finalized_nav_state',
        policy_hash: '11'.repeat(32),
        supported: true,
      },
    ],
    supported_pairs: [
      { from_asset: 'a651', to_asset: 'a652', enabled: true, liquidity_mode: 'pool_managed_note' },
    ],
    quote: { enabled: true, liquidity_mode: 'pool_managed_note', policy_hash: '11'.repeat(32) },
    swap: {
      enabled: true,
      endpoint: '/api/shielded-nav-swap/swap',
      quote_binding_enforcement: 'proxy_checked_quote_freshness_and_liquidity_commitment_not_circuit_external_binding',
    },
  });
  assert.equal(cap.can_quote, true);
  assert.equal(cap.can_run, true);
  assert.equal(cap.status, 'step6_quote_ready');
  assert.equal(cap.disabled_reason, '');
  assert.equal(cap.swap.endpoint, '/api/shielded-nav-swap/swap');
});

test('shielded capability normalization exposes explicit Step 9 public exit', () => {
  const cap = normalizeShieldedNavswapCapability({
    enabled: true,
    can_quote: true,
    can_run: true,
    can_egress: true,
    bridge_out_requires_public_exit_receipt: true,
    custody_boundary: 'wallet_local_private_egress_proof_proxy_certified_public_exit',
    requires_local_prover: true,
    requires_note_scan: true,
    liquidity_mode: 'pool_managed_note',
    asset_registry: [
      {
        symbol: 'a651',
        asset_id: 'aa'.repeat(48),
        precision: 6,
        issuer: 'pfissuer',
        nav_source: 'finalized_nav_state',
        policy_hash: '11'.repeat(32),
        supported: true,
      },
      {
        symbol: 'a652',
        asset_id: 'bb'.repeat(48),
        precision: 6,
        issuer: 'pfissuer',
        nav_source: 'finalized_nav_state',
        policy_hash: '11'.repeat(32),
        supported: true,
      },
    ],
    supported_pairs: [
      { from_asset: 'a651', to_asset: 'a652', enabled: true, liquidity_mode: 'pool_managed_note' },
    ],
    egress: {
      enabled: true,
      endpoint: '/api/shielded-nav-swap/egress',
      policy_id: SHIELDED_NAVSWAP_EGRESS_POLICY_ID,
      bridge_out_requires_public_exit_receipt: true,
    },
  });
  assert.equal(cap.can_egress, true);
  assert.equal(cap.bridge_out_requires_public_exit_receipt, true);
  assert.equal(cap.egress.endpoint, '/api/shielded-nav-swap/egress');
  assert.equal(cap.p9_status.status, 'explicit_public_exit_required');
  assert.match(cap.p9_status.copy, /stay private by default/i);
});

test('shielded capability normalization allows Step 6 quote preview but not run', () => {
  const cap = normalizeShieldedNavswapCapability({
    enabled: true,
    can_quote: true,
    can_run: false,
    custody_boundary: 'wallet_local_note_keys_only',
    requires_local_prover: true,
    requires_note_scan: true,
    liquidity_mode: 'pool_managed_note',
    asset_registry: [
      {
        symbol: 'a651',
        asset_id: 'aa'.repeat(48),
        precision: 6,
        issuer: 'pfissuer',
        nav_source: 'finalized_nav_state',
        policy_hash: '11'.repeat(32),
        supported: true,
      },
      {
        symbol: 'a652',
        asset_id: 'bb'.repeat(48),
        precision: 6,
        issuer: 'pfissuer',
        nav_source: 'finalized_nav_state',
        policy_hash: '11'.repeat(32),
        supported: true,
      },
    ],
    supported_pairs: [
      { from_asset: 'a651', to_asset: 'a652', enabled: true, liquidity_mode: 'pool_managed_note' },
      { from_asset: 'a652', to_asset: 'a651', enabled: true, liquidity_mode: 'pool_managed_note' },
    ],
    quote: {
      enabled: true,
      liquidity_mode: 'pool_managed_note',
      policy_hash: '11'.repeat(32),
    },
  });
  assert.equal(cap.can_quote, true);
  assert.equal(cap.can_run, false);
  assert.equal(cap.status, 'step6_quote_ready');
  assert.equal(cap.supported_pairs.length, 2);
  assert.equal(cap.supported_pairs.every(pair => pair.ok), true);
  assert.match(cap.disabled_reason, /Step 7/);
});

test('shielded quote normalization requires live commitment and non-expired expiry', () => {
  const quote = {
    ok: true,
    schema: 'postfiat-shielded-navswap-quote-v1',
    from_asset: 'a651',
    to_asset: 'a652',
    input_amount_atoms: '1000000',
    output_amount_atoms: '1000000',
    quote_generated_at_ms: '2000',
    quote_expires_at_ms: '5000',
    liquidity: {
      mode: 'pool_managed_note',
      commitment: 'aa'.repeat(32),
      commitment_status: 'live',
    },
    policy_hash: '11'.repeat(32),
    quote_binding_hash: '22'.repeat(32),
    can_prove: false,
    can_run: false,
    submit_enabled: false,
  };
  const live = normalizeShieldedNavswapQuote(quote, 3000);
  assert.equal(live.ready, true);
  assert.equal(live.expired, false);
  assert.equal(live.can_run, false);
  const expired = normalizeShieldedNavswapQuote(quote, 5001);
  assert.equal(expired.ready, false);
  assert.equal(expired.expired, true);
  assert.ok(expired.missing.includes('quote_not_expired'));
});

test('private egress disclosure hash is stable and public-field bound', async () => {
  const disclosure = shieldedPrivateEgressDisclosureFields({
    walletAddress: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
    to: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
    assetId: 'aa'.repeat(48),
    amountAtoms: '2000',
    noteCommitment: 'bb'.repeat(32),
    policyId: SHIELDED_NAVSWAP_EGRESS_POLICY_ID,
  });
  const hash = await shieldedPrivateEgressDisclosureHash(disclosure, { cryptoImpl: webcrypto });
  const same = await shieldedPrivateEgressDisclosureHash({ ...disclosure }, { cryptoImpl: webcrypto });
  const changed = await shieldedPrivateEgressDisclosureHash({
    ...disclosure,
    amount_atoms: '2001',
  }, { cryptoImpl: webcrypto });
  assert.match(hash, /^[0-9a-f]{64}$/);
  assert.equal(hash, same);
  assert.notEqual(hash, changed);
  assert.deepEqual(disclosure.visible_after_submit, ['destination', 'asset_id', 'amount_atoms', 'receipt_timing']);
  assert.ok(disclosure.stays_private.includes('note_opening'));
});

test('private egress verifier permits public proof fields and rejects note openings', () => {
  const file = {
    schema: ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA,
    payload: {
      version: 1,
      schema: ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA,
      pool_id: ASSET_ORCHARD_POOL_ID,
      to: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      asset_id: 'aa'.repeat(48),
      amount: 2000,
      fee: 0,
      policy_id: SHIELDED_NAVSWAP_EGRESS_POLICY_ID,
      disclosure_hash: '11'.repeat(32),
      proof_system_id: 'postfiat.privacy.asset-orchard-halo2.v1',
      circuit_id: 'asset_orchard.private_egress.v1',
      pool_domain: '22'.repeat(32),
      anchor: '33'.repeat(32),
      nullifier: '44'.repeat(32),
      randomized_verification_key: '55'.repeat(32),
      asset_tag_lo: '0x01',
      asset_tag_hi: '0x02',
      exit_binding_hash: '66'.repeat(64),
      proof: '77'.repeat(64),
      spend_authorization_signature: '88'.repeat(64),
    },
  };
  const verified = verifyAssetOrchardPrivateEgressJson(file, {
    pool_id: ASSET_ORCHARD_POOL_ID,
    to: file.payload.to,
    asset_id: file.payload.asset_id,
    amount_atoms: '2000',
    policy_id: SHIELDED_NAVSWAP_EGRESS_POLICY_ID,
    disclosure_hash: file.payload.disclosure_hash,
  });
  assert.equal(verified.amount_atoms, '2000');
  assert.equal(verified.proof_bytes, 64);
  assert.throws(
    () => verifyAssetOrchardPrivateEgressJson({
      ...file,
      payload: { ...file.payload, note_opening: { value: 2000 } },
    }),
    /forbidden private material/,
  );
});

test('local prover readiness requires local-only K=15 hashes', () => {
  assert.deepEqual(
    normalizeLocalProverReadiness({
      local_only: true,
      ready: true,
      pool_id: ASSET_ORCHARD_POOL_ID,
      circuit_id: 'asset-orchard-swap-v1',
      k: 15,
      params_hash: 'a'.repeat(96),
      vk_hash: 'b'.repeat(96),
    }).missing,
    [],
  );
  const bad = normalizeLocalProverReadiness({
    local_only: false,
    ready: true,
    circuit_id: 'asset-orchard-swap-v1',
    k: 14,
    params_hash: 'a'.repeat(96),
  });
  assert.equal(bad.ready, false);
  assert.deepEqual(bad.missing, ['k=15', 'vk_hash', 'local_only']);
});

test('shielded note vault encryption recovers spendable state without selecting nullified notes', async () => {
  const { key, salt } = await deriveShieldedNoteVaultKey('test-passphrase', { cryptoImpl: webcrypto });
  const snapshot = {
    notes: [
      { note_id: 'n1', state: 'pending', commitment: 'cm1' },
      { note_id: 'n2', state: 'spendable', commitment: 'cm2' },
    ],
    keys: { spend_key: 'local-only-secret' },
  };
  const envelope = await sealShieldedNoteVault(snapshot, { key, salt, cryptoImpl: webcrypto });
  const encoded = JSON.stringify(envelope);
  assert.doesNotMatch(encoded, /local-only-secret/);
  assert.doesNotMatch(encoded, /spend_key/);

  const opened = await openShieldedNoteVault(envelope, { key, cryptoImpl: webcrypto });
  assert.equal(opened.keys.spend_key, 'local-only-secret');
  const reconciled = reconcileShieldedNotes(opened.notes, [
    { note_id: 'n1', confirmed: true },
    { note_id: 'n2', nullified: true, nullifier: 'nf2' },
  ]);
  assert.deepEqual(spendableShieldedNotes(reconciled).map(note => note.note_id), ['n1']);
});

test('Asset-Orchard action verification rejects cleartext assets and note openings', () => {
  const action = {
    schema: ASSET_ORCHARD_SWAP_ACTION_SCHEMA,
    chain_id: 'postfiat-wan-devnet',
    genesis_hash: 'genesis',
    pool_id: ASSET_ORCHARD_POOL_ID,
    circuit_id: 'asset-orchard-swap-v1',
    anchor: 'anchor-1',
    nullifiers: ['nf1', 'nf2'],
    output_commitments: ['cm1', 'cm2'],
    accounting_inputs: [
      { output_commitment: 'ai1', value_commitment: 'vc1' },
      { output_commitment: 'ai2', value_commitment: 'vc2' },
    ],
    accounting_outputs: [
      { output_commitment: 'ao1', value_commitment: 'vc3' },
      { output_commitment: 'ao2', value_commitment: 'vc4' },
    ],
    proof: 'proof-bytes',
    spend_authorization_signatures: ['sig1', 'sig2'],
  };
  const verified = verifyAssetOrchardSwapActionJson(action, {
    chain_id: 'postfiat-wan-devnet',
    genesis_hash: 'genesis',
    pool_id: ASSET_ORCHARD_POOL_ID,
    anchor: 'anchor-1',
    nullifier_count: 2,
    output_count: 2,
    accounting_input_count: 2,
    accounting_output_count: 2,
  });
  assert.equal(verified.ok, true);

  assert.throws(
    () => verifyAssetOrchardSwapActionJson({ ...action, asset_id: 'aa'.repeat(48) }),
    /forbidden cleartext/,
  );
  assert.throws(
    () => verifyAssetOrchardSwapActionJson({ ...action, input: { note_opening: 'secret' } }),
    /forbidden cleartext/,
  );
  for (const privateField of [
    'diversifier',
    'g_d',
    'pk_d',
    'rho',
    'psi',
    'rcm',
    'nk',
    'rivk',
    'rseed',
    'spend_auth_signing_key',
    'full_viewing_key_hex',
  ]) {
    assert.throws(
      () => verifyAssetOrchardSwapActionJson({
        ...action,
        private_witness: { [privateField]: 'ab'.repeat(32) },
      }),
      /forbidden cleartext/,
      privateField,
    );
  }
});

test('local Asset-Orchard prover client is loopback-only and verifies returned actions', async () => {
  assert.throws(
    () => new LocalAssetOrchardProverClient({ baseUrl: 'https://prover.example.com' }),
    /local-only/,
  );
  const calls = [];
  const action = {
    schema: ASSET_ORCHARD_SWAP_ACTION_SCHEMA,
    chain_id: 'postfiat-wan-devnet',
    genesis_hash: 'genesis',
    pool_id: ASSET_ORCHARD_POOL_ID,
    circuit_id: 'asset-orchard-swap-v1',
    anchor: 'anchor-1',
    nullifiers: ['nf1', 'nf2'],
    output_commitments: ['cm1', 'cm2'],
    accounting_inputs: [{ output_commitment: 'ai1' }, { output_commitment: 'ai2' }],
    accounting_outputs: [{ output_commitment: 'ao1' }, { output_commitment: 'ao2' }],
    proof: 'proof-bytes',
  };
  const client = new LocalAssetOrchardProverClient({
    baseUrl: 'http://127.0.0.1:8789',
    fetchImpl: async (url, options) => {
      calls.push({ url, options });
      return {
        ok: true,
        async json() {
          return { ok: true, action };
        },
      };
    },
  });
  const result = await client.buildSwapAction({
    route: SHIELDED_NAVSWAP_ROUTE,
    note_refs: ['note-local-ref-1', 'note-local-ref-2'],
    quote_commitment: 'quote-commitment',
  }, {
    chain_id: 'postfiat-wan-devnet',
    genesis_hash: 'genesis',
    pool_id: ASSET_ORCHARD_POOL_ID,
    anchor: 'anchor-1',
    nullifier_count: 2,
    output_count: 2,
  });
  assert.equal(result.ok, true);
  assert.equal(calls[0].url, 'http://127.0.0.1:8789/asset-orchard/swap-actions');
  assert.equal(calls[0].options.method, 'POST');
  assert.deepEqual(JSON.parse(calls[0].options.body).note_refs, ['note-local-ref-1', 'note-local-ref-2']);

  await assert.rejects(
    () => client.buildSwapAction({
      route: SHIELDED_NAVSWAP_ROUTE,
      note_opening: 'secret',
    }),
    /forbidden private wallet material/,
  );
});

test('Asset-Orchard ingress helpers build public payload without spend material', () => {
  const walletNote = {
    schema: 'postfiat-asset-orchard-wallet-note-v1',
    pool_id: ASSET_ORCHARD_POOL_ID,
    asset_id: 'aa'.repeat(48),
    value: 7,
    output_commitment: 'bb'.repeat(32),
    note: {
      diversifier: '11'.repeat(11),
      g_d: '22'.repeat(32),
      pk_d: '33'.repeat(32),
      asset_tag_lo: '0x1',
      asset_tag_hi: '0x2',
      value: 7,
      rho: '44'.repeat(32),
      psi: '55'.repeat(32),
      rcm: '66'.repeat(32),
    },
    nk: 'local-only-secret',
    rivk: 'local-only-secret',
    spend_auth_signing_key: 'local-only-secret',
  };
  const signedBurn = {
    unsigned: {
      source: 'pfwallet',
      transaction_kind: 'asset_burn',
      asset_burn: {
        owner: 'pfwallet',
        issuer: 'pfissuer',
        asset_id: 'aa'.repeat(48),
        amount: 7,
      },
    },
    signature_hex: '77',
  };

  const payload = buildAssetOrchardIngressPayload({
    signedBurnTransaction: signedBurn,
    assetId: 'aa'.repeat(48),
    amountAtoms: '7',
    walletNote,
    encryptedOutput: `5046414f454e4331${'00'.repeat(64)}`,
  });

  assert.equal(payload.pool_id, ASSET_ORCHARD_POOL_ID);
  assert.equal(payload.output_commitment, 'bb'.repeat(32));
  assert.equal(payload.encrypted_output, `5046414f454e4331${'00'.repeat(64)}`);
  assert.equal(Object.hasOwn(payload, 'note'), false);
  for (const privateField of ['value', 'rho', 'psi', 'rcm', 'diversifier', 'g_d', 'pk_d']) {
    assert.equal(Object.hasOwn(payload, privateField), false);
  }
  assert.equal(JSON.stringify(payload).includes('local-only-secret'), false);

  assert.throws(
    () => buildAssetOrchardIngressPayload({
      signedBurnTransaction: signedBurn,
      assetId: 'aa'.repeat(48),
      amountAtoms: '7',
      walletNote,
    }),
    /encrypted output is required/i,
  );
  assert.throws(
    () => buildAssetOrchardIngressPayload({
      signedBurnTransaction: signedBurn,
      assetId: 'aa'.repeat(48),
      amountAtoms: '7',
      walletNote,
      encryptedOutput: '00'.repeat(72),
    }),
    /PFAOENC1/i,
  );
});
