const assert = require('assert');
const fs = require('fs');
const http = require('http');
const net = require('net');
const os = require('os');
const path = require('path');

const {
  buildNavswapQuoteResponse,
  buildNavswapRunResponse,
  buildNavswapNavProofResponse,
  buildShieldedCertifiedRoundArgs,
  closeUpstreamRpcConnections,
  clearNavswapDevnetFundingUsageForTest,
  clearNavswapIdempotencyForTest,
  clearNavswapRunsForTest,
  executeNavswapAtomicTemplate,
  executeNavswapCapabilities,
  executeNavswapQuote,
  executeNavswapDevnetPfusdcFunding,
  executeNavswapIdempotentRequest,
  executeNavswapRun,
  executeShieldedNavswapEgress,
  executeShieldedNavswapProverReadiness,
  executeShieldedNavswapQuote,
  executeShieldedNavswapSwap,
  executeTransparentNavswapReadiness,
  executeTransparentNavswapRun,
  loadNavswapIdempotencyStore,
  loadNavswapRunStore,
  navswapIdempotencyStorePath,
  navswapCapabilities,
  navswapRunEvents,
  navswapRunList,
  navswapRunPublic,
  navswapRunReceipts,
  navswapRunStorePath,
  navswapRunStreamSnapshot,
  navswapStakehubTransparentConfig,
  normalizeAtomicTemplateParams,
  planTransparentNavswapWalletActions,
  prepareNavswapWalletAction,
  prepareNavswapWalletActionBatch,
  server: navswapHttpServer,
  runShieldedLaggardCatchUp,
  shieldedPrivateEgressDisclosureFields,
  shieldedPrivateEgressDisclosureHash,
  verifyAtomicTemplateResult,
  verifyAtomicTemplateSymmetry,
} = require('./server');
const { withEnvAsync } = require('./test_navswap_env');
const {
  runNavswapPolicyPersistenceTests,
} = require('./test_navswap_policy_persistence');

const NODE_ROUTE_DIGEST_VECTOR = JSON.parse(
  fs.readFileSync(path.join(__dirname, 'fixtures', 'pftl-uniswap-node-route-digest.json'), 'utf8')
);

function withEnv(updates, fn) {
  const previous = {};
  for (const key of Object.keys(updates)) {
    previous[key] = process.env[key];
    if (updates[key] === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = updates[key];
    }
  }
  try {
    return fn();
  } finally {
    for (const key of Object.keys(updates)) {
      if (previous[key] === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = previous[key];
      }
    }
  }
}

function uniswapBetaEnv(overrides = {}) {
  return {
    NAVSWAP_NATIVE_NAV_ASSET_ID: 'd'.repeat(96),
    NAVSWAP_ROUTE_SUPPLY_CAP_ATOMS: '100000000',
    NAVSWAP_SUPPLY_CAP_REMAINING_ATOMS: '99999000',
    NAVSWAP_PACKET_NOTIONAL_CAP_ATOMS: '1000000',
    NAVSWAP_SEED_NAV_EPOCH: '7',
    NAVSWAP_SEED_USDC_ATOMS: '100000000',
    NAVSWAP_SEED_WRAPPED_NAVCOIN_ATOMS: '100000',
    NAVSWAP_LP_RECIPIENT: '0x7777777777777777777777777777777777777777',
    NAVSWAP_LP_CUSTODY_POLICY: 'controlled_launch_lp',
    NAVSWAP_ROUTE_CONFIG_DIGEST: NODE_ROUTE_DIGEST_VECTOR.route_config_digest,
    NAVSWAP_WRAPPED_NAVCOIN_TOKEN: '0x4444444444444444444444444444444444444444',
    NAVSWAP_HANDOFF_CONTROLLER: '0x1111111111111111111111111111111111111111',
    NAVSWAP_SETTLEMENT_ADAPTER: '0x1212121212121212121212121212121212121212',
    NAVSWAP_VERIFIER_MODE: 'threshold-controlled',
    NAVSWAP_UNISWAP_POOL_ID: '0x2222222222222222222222222222222222222222222222222222222222222222',
    NAVSWAP_UNISWAP_ROUTER: '0x3333333333333333333333333333333333333333',
    NAVSWAP_UNISWAP_OUTPUT_TOKEN: '0x5555555555555555555555555555555555555555',
    NAVSWAP_ENABLE_UNISWAP_BETA_ROUTE: 'true',
    NAVSWAP_UNISWAP_ROUTE_PAUSED: 'false',
    NAVSWAP_UNISWAP_PUBLIC_ROUTING_ENABLED: 'false',
    ...overrides,
  };
}

function assertNoTrustlessDisplay(value) {
  assert.doesNotMatch(JSON.stringify(value), /trustless/i);
}

async function waitForNavswapRun(runId, predicate, timeoutMs = 2000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const status = navswapRunPublic(runId);
    if (status && predicate(status)) return status;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error(`timed out waiting for NAVSwap run ${runId}`);
}

async function collectSseEvents(url, stopEvent = 'navswap_run_done', timeoutMs = 2000) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const response = await fetch(url, {
    headers: { Accept: 'text/event-stream' },
    signal: controller.signal,
  });
  assert.strictEqual(response.status, 200);
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  const events = [];
  const parseBufferedEvents = () => {
    let boundary = buffer.indexOf('\n\n');
    while (boundary >= 0) {
      const raw = buffer.slice(0, boundary);
      buffer = buffer.slice(boundary + 2);
      const parsed = { event: 'message', data: null };
      for (const line of raw.split(/\r?\n/)) {
        if (line.startsWith('event:')) parsed.event = line.slice(6).trim();
        if (line.startsWith('data:')) parsed.data = JSON.parse(line.slice(5).trim());
      }
      if (parsed.data) events.push(parsed);
      if (parsed.event === stopEvent) return true;
      boundary = buffer.indexOf('\n\n');
    }
    return false;
  };

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      if (parseBufferedEvents()) break;
    }
  } finally {
    clearTimeout(timer);
    controller.abort();
  }
  return events;
}

function atomicTemplateFixture({
  settlementId = 'settlement',
  conditionHash = 'hash',
  leftEscrowId = 'escrow-left',
  rightEscrowId = 'escrow-right',
  issuedAssetId = 'a'.repeat(96),
  swapped = false,
} = {}) {
  const left = swapped
    ? {
        owner: 'pf-right',
        recipient: 'pf-left',
        asset_id: issuedAssetId,
        escrow_id: rightEscrowId,
      }
    : {
        owner: 'pf-left',
        recipient: 'pf-right',
        asset_id: 'PFT',
        escrow_id: leftEscrowId,
      };
  const right = swapped
    ? {
        owner: 'pf-left',
        recipient: 'pf-right',
        asset_id: 'PFT',
        escrow_id: leftEscrowId,
      }
    : {
        owner: 'pf-right',
        recipient: 'pf-left',
        asset_id: issuedAssetId,
        escrow_id: rightEscrowId,
      };
  return {
    schema: 'postfiat-atomic-settlement-template-v1',
    settlement_id: settlementId,
    condition_hash: conditionHash,
    condition: 'shared-secret',
    left: {
      ...left,
      transaction_kind: 'escrow_create',
      operation: { operation: 'escrow_create', condition: 'shared-secret' },
    },
    right: {
      ...right,
      transaction_kind: 'escrow_create',
      operation: { operation: 'escrow_create', condition: 'shared-secret' },
    },
  };
}

function testCapabilitiesGateUniswapHandoff() {
  const caps = navswapCapabilities(new Date('2026-06-29T00:00:00.000Z'));
  assert.strictEqual(caps.schema, 'postfiat-navswap-capabilities-v1');
  assert.strictEqual(caps.routes.uniswap_atomic_handoff.enabled, false);
  assert.strictEqual(caps.routes.transparent_navswap.enabled, true);
  assert.strictEqual(caps.routes.transparent_navswap.route_family, 'primary_pftl_mint');
  assert.strictEqual(caps.routes.transparent_navswap.route_trust_class, 'CONTROLLED');
  assert.strictEqual(caps.routes.transparent_navswap.primary_supply_effect, 'mints_new_native_navcoin_supply');
  assert.strictEqual(caps.routes.transparent_navswap.pricing_source, 'finalized_pre_inflow_nav_snapshot');
  assert.strictEqual(caps.routes.transparent_navswap.status, 'operator_key_required');
  assert.strictEqual(caps.routes.transparent_navswap.can_quote, true);
  assert.strictEqual(caps.routes.transparent_navswap.can_run, false);
  assert.strictEqual(caps.routes.transparent_navswap.privacy.schema, 'postfiat-navswap-route-privacy-v1');
  assert.strictEqual(caps.routes.transparent_navswap.privacy.label, 'Public');
  assert.strictEqual(caps.routes.transparent_navswap.privacy.mode, 'public_wallet_signed');
  assert.ok(caps.routes.transparent_navswap.privacy.public_fields.includes('allocation_id'));
  assert.ok(caps.routes.transparent_navswap.privacy.private_fields.includes('wallet_seed'));
  assert.strictEqual(
    caps.routes.transparent_navswap.prepared_action_schema,
    'postfiat-navswap-wallet-action-request-v1',
  );
  assert.strictEqual(caps.routes.transparent_navswap.planner_fed_quote_supported, true);
  assert.strictEqual(caps.routes.transparent_navswap.quote_requires_planner_actions, true);
  assert.ok(caps.routes.transparent_navswap.required_next.includes('configure NAVSWAP_OPERATOR_ISSUER_KEY_FILE'));
  assert.ok(caps.routes.transparent_navswap.required_next.includes('manual browser UI click-through from the target user wallet'));
  assert.ok(!caps.routes.transparent_navswap.required_next.includes('live devnet before/after balance evidence'));
  assert.ok(!caps.routes.transparent_navswap.required_next.includes('planner-fed action UI orchestration'));
  assert.deepStrictEqual(caps.routes.transparent_navswap.supported_pairs, ['pfUSDC->a651', 'a651->pfUSDC']);
  assert.deepStrictEqual(caps.routes.transparent_navswap.current_pair, {
    from_asset: 'pfUSDC',
    to_asset: 'a651',
    amount_asset: 'a651',
    settlement_asset: 'pfUSDC',
    amount_semantics: 'display_nav_amount_decimal',
    amount_precision: 6,
  });
  assert.strictEqual(caps.routes.transparent_navswap.automatic_planner_input_selection, 'default_wallet_quote');
  assert.strictEqual(caps.routes.transparent_navswap.operator_completion.endpoint, '/api/navswap/runs');
  assert.strictEqual(caps.routes.transparent_navswap.readiness_endpoint, '/api/navswap/readiness');
  assert.strictEqual(caps.routes.transparent_navswap.devnet_settlement_funding.endpoint, '/api/navswap/devnet-fund-pfusdc');
  assert.strictEqual(caps.routes.transparent_navswap.devnet_settlement_funding.enabled, false);
  assert.strictEqual(caps.routes.transparent_navswap.devnet_settlement_funding.max_amount_atoms, '10000000');
  assert.strictEqual(caps.routes.transparent_navswap.devnet_settlement_funding.max_recipient_window_atoms, '10000000');
  assert.strictEqual(caps.routes.transparent_navswap.operator_completion.signing_configured, false);
  assert.strictEqual(caps.routes.transparent_navswap.operator_completion.submit_method, 'mempool_submit_signed_asset_transaction_finality');
  assert.strictEqual(caps.routes.transparent_navswap.operator_completion.asset_transaction_finality_enabled, true);
  assert.strictEqual(caps.routes.transparent_navswap.planner_inputs_endpoint, '/api/navswap/planner-inputs');
  assert.strictEqual(
    caps.routes.transparent_navswap.prepare_action_batch_endpoint,
    '/api/navswap/actions/prepare-batch',
  );
  assert.strictEqual(
    caps.routes.transparent_navswap.prepared_action_batch_schema,
    'postfiat-navswap-wallet-action-batch-prepare-v1',
  );
  assert.deepStrictEqual(
    caps.routes.transparent_navswap.prepared_action_stages,
    ['nav_subscription_allocate', 'nav_redeem_at_nav'],
  );
  assert.deepStrictEqual(
    caps.routes.transparent_navswap.wallet_owned_actions,
    ['vault_bridge_nav_subscription_allocate', 'nav_redeem_at_nav'],
  );
  assert.strictEqual(caps.routes.stakehub_transparent_roundtrip.status, 'operator_not_configured');
  assert.strictEqual(caps.routes.stakehub_transparent_roundtrip.can_run, false);
  assert.strictEqual(caps.routes.stakehub_transparent_roundtrip.privacy.label, 'Public operator route');
  assert.strictEqual(caps.routes.shielded_navswap.status, 'step6_quote_configuration_required');
  assert.strictEqual(caps.routes.shielded_navswap.enabled, false);
  assert.strictEqual(caps.routes.shielded_navswap.can_quote, false);
  assert.strictEqual(caps.routes.shielded_navswap.can_run, false);
  assert.strictEqual(caps.routes.shielded_navswap.can_ingress, false);
  assert.strictEqual(caps.routes.shielded_navswap.custody_boundary, 'wallet-local-note-and-burn-signing');
  assert.strictEqual(caps.routes.shielded_navswap.privacy.label, 'Private quote preview');
  assert.strictEqual(caps.routes.shielded_navswap.privacy.mode, 'wallet_local_quote_and_ingress_boundary');
  assert.ok(caps.routes.shielded_navswap.privacy.disclosed_fields.includes('liquidity_commitment'));
  assert.ok(caps.routes.shielded_navswap.privacy.public_fields.includes('burn_transaction'));
  assert.match(caps.routes.shielded_navswap.privacy.warning, /Step 7 review gate/);
  assert.strictEqual(caps.routes.shielded_navswap.quote.endpoint, '/api/shielded-nav-swap/quote');
  assert.strictEqual(caps.routes.shielded_navswap.quote.enabled, false);
  assert.ok(caps.routes.shielded_navswap.quote.missing.includes('A652_ASSET_ID'));
  assert.strictEqual(caps.routes.shielded_navswap.ingress.endpoint, '/api/shielded-nav-swap/ingress');
  assert.strictEqual(caps.routes.shielded_navswap.ingress.enabled, false);
  assert.ok(caps.routes.shielded_navswap.ingress.missing.includes('NAVSWAP_ENABLE_SHIELDED_INGRESS=true'));
  assert.strictEqual(caps.routes.pftl_atomic_settlement.privacy.label, 'Public atomic');
  assert.ok(caps.routes.pftl_atomic_settlement.privacy.public_fields.includes('condition_hash'));
  assert.match(
    caps.routes.uniswap_atomic_handoff.reason,
    /bridge-aware wrapped NAVCoin token/,
  );
  assert.strictEqual(caps.routes.uniswap_atomic_handoff.route_family, 'composite_primary_mint_to_ethereum_venue');
  assert.strictEqual(caps.routes.uniswap_atomic_handoff.route_trust_class, 'DISABLED');
  assert.strictEqual(caps.routes.uniswap_atomic_handoff.explicit_beta, false);
  assert.strictEqual(caps.routes.uniswap_atomic_handoff.public_routing_enabled, false);
  assert.strictEqual(caps.routes.uniswap_atomic_handoff.paused, false);
  assert.strictEqual(caps.routes.uniswap_atomic_handoff.enabled, false);
  assert.strictEqual(caps.routes.uniswap_atomic_handoff.can_quote, false);
  assert.strictEqual(caps.routes.uniswap_atomic_handoff.can_run, false);
  assert.strictEqual(caps.routes.uniswap_atomic_handoff.privacy.label, 'Disabled public handoff');
  assert.ok(caps.routes.uniswap_atomic_handoff.privacy.public_fields.includes('recipient'));
  assert.strictEqual(caps.routes.uniswap_atomic_handoff.config.legacy_pool_selected, false);
  assertNoTrustlessDisplay(caps.routes.uniswap_atomic_handoff);
  assert.strictEqual(caps.routes.legacy_a651_uniswap.status, 'inspection_only');
  assert.strictEqual(caps.routes.legacy_a651_uniswap.privacy.label, 'Public inspection');
  assertNoTrustlessDisplay(caps.routes.legacy_a651_uniswap);
  assert.strictEqual(
    caps.routes.legacy_a651_uniswap.pool_id,
    '0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84',
  );
}

function shieldedQuoteEnv(overrides = {}) {
  return {
    A652_ASSET_ID: 'e'.repeat(96),
    NAVSWAP_SHIELDED_ASSET_ISSUER: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
    NAVSWAP_SHIELDED_LIQUIDITY_COMMITMENT: 'c'.repeat(64),
    NAVSWAP_SHIELDED_LIQUIDITY_MODE: 'pool_managed_note',
    NAVSWAP_SHIELDED_LIQUIDITY_PROVIDER: 'controlled_pool_operator',
    NAVSWAP_SHIELDED_QUOTE_TTL_MS: '120000',
    ...overrides,
  };
}

function shieldedSwapEnv(overrides = {}) {
  return shieldedQuoteEnv({
    NAVSWAP_ENABLE_SHIELDED_INGRESS: 'true',
    NAVSWAP_ENABLE_SHIELDED_SWAPS: 'true',
    NAVSWAP_SHIELDED_INGRESS_DATA_DIR: '/tmp/postfiat-test-shielded-data',
    NAVSWAP_SHIELDED_INGRESS_TOPOLOGY: '/tmp/postfiat-test-shielded-topology.json',
    NAVSWAP_SHIELDED_INGRESS_KEY_FILE: __filename,
    NAVSWAP_SHIELDED_INGRESS_NODE_BIN: process.execPath,
    ...overrides,
  });
}

function testShieldedNavswapCapabilitiesExposeQuotePreflight() {
  withEnv(shieldedQuoteEnv(), () => {
    const caps = navswapCapabilities(new Date('2026-07-02T17:12:00.000Z'));
    const route = caps.routes.shielded_navswap;
    assert.strictEqual(route.status, 'step6_quote_ready');
    assert.strictEqual(route.enabled, true);
    assert.strictEqual(route.can_quote, true);
    assert.strictEqual(route.can_run, false);
    assert.strictEqual(route.quote.enabled, true);
    assert.strictEqual(route.quote.endpoint, '/api/shielded-nav-swap/quote');
    assert.strictEqual(route.quote.liquidity_mode, 'pool_managed_note');
    assert.strictEqual(route.quote.liquidity_commitment, 'c'.repeat(64));
    assert.strictEqual(route.quote.liquidity.commitment, 'c'.repeat(64));
    assert.strictEqual(route.quote.liquidity.counterparty, 'controlled_pool_operator');
    assert.strictEqual(route.quote.liquidity_commitment_status, 'live');
    assert.strictEqual(route.quote.submit_gate, 'Step 7 private swap submit');
    assert.match(route.quote.policy_hash, /^[0-9a-f]{64}$/);
    assert.deepStrictEqual(
      route.supported_pairs.map(pair => `${pair.from_asset}->${pair.to_asset}:${pair.enabled}`),
      ['a651->a652:true', 'a652->a651:true'],
    );
    assert.strictEqual(route.asset_registry.find(asset => asset.symbol === 'a652').asset_id, 'e'.repeat(96));
    assertNoTrustlessDisplay(route);
  });
}

async function testShieldedNavswapQuoteBindsLiquidityAndExpiry() {
  await withEnvAsync(shieldedQuoteEnv(), async () => {
    const quote = await executeShieldedNavswapQuote({
      route: 'shielded_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      from_asset: 'a651',
      to_asset: 'a652',
      amount_atoms: '2000000',
    });
    assert.strictEqual(quote.ok, true);
    assert.strictEqual(quote.schema, 'postfiat-shielded-navswap-quote-v1');
    assert.strictEqual(quote.from_asset, 'a651');
    assert.strictEqual(quote.to_asset, 'a652');
    assert.strictEqual(quote.output_amount_atoms, '2000000');
    assert.strictEqual(quote.minimum_output_atoms, '2000000');
    assert.strictEqual(quote.liquidity.mode, 'pool_managed_note');
    assert.strictEqual(quote.liquidity.commitment, 'c'.repeat(64));
    assert.strictEqual(quote.liquidity.commitment_status, 'live');
    assert.strictEqual(quote.liquidity.trust_class, 'CONTROLLED');
    assert.match(quote.policy_hash, /^[0-9a-f]{64}$/);
    assert.match(quote.quote_binding_hash, /^[0-9a-f]{64}$/);
    assert.strictEqual(quote.can_prove, false);
    assert.strictEqual(quote.can_run, false);
    assert.strictEqual(quote.submit_enabled, false);
    assert.strictEqual(quote.next_gate, 'Step 7 private swap submit');
    assert.ok(Number(quote.quote_expires_at_ms) > Number(quote.quote_generated_at_ms));
    assert.strictEqual(quote.quote_freshness.market_ops_status, 'controlled_pool_liquidity_commitment_live');

    const reverse = await executeShieldedNavswapQuote({
      route: 'shielded_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      from_asset: 'a652',
      to_asset: 'a651',
      amount_atoms: '3000000',
    });
    assert.strictEqual(reverse.ok, true);
    assert.strictEqual(reverse.from_asset, 'a652');
    assert.strictEqual(reverse.to_asset, 'a651');

    const generic = await executeNavswapQuote({
      route: 'shielded_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      from_asset: 'a651',
      to_asset: 'a652',
      amount_atoms: '2000000',
    });
    assert.strictEqual(generic.ok, false);
    assert.strictEqual(generic.code, 'shielded_navswap_use_shielded_quote_endpoint');
  });
}

async function testShieldedNavswapSwapGateRequiresFreshOpaqueAction() {
  await withEnvAsync(shieldedSwapEnv({ NAVSWAP_ENABLE_SHIELDED_EGRESS: 'false' }), async () => {
    const caps = navswapCapabilities(new Date('2026-07-02T17:40:00.000Z'));
    const route = caps.routes.shielded_navswap;
    assert.strictEqual(route.status, 'step7_swap_ready');
    assert.strictEqual(route.can_quote, true);
    assert.strictEqual(route.can_run, true);
    assert.strictEqual(route.quote.liquidity_commitment, 'c'.repeat(64));
    assert.strictEqual(route.swap.enabled, true);
    assert.strictEqual(route.swap.endpoint, '/api/shielded-nav-swap/swap');
    assertNoTrustlessDisplay(route);

    const quote = await executeShieldedNavswapQuote({
      route: 'shielded_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      from_asset: 'a651',
      to_asset: 'a652',
      amount_atoms: '2000000',
    });
    assert.strictEqual(quote.ok, true);
    assert.strictEqual(quote.submit_enabled, true);
    assert.strictEqual(quote.can_run, true);

    const opaqueAction = {
      schema: 'postfiat-asset-orchard-swap-action-v1',
      pool_id: 'asset-orchard-v1',
      nullifiers: ['11'.repeat(32), '22'.repeat(32)],
      output_commitments: ['33'.repeat(32), '44'.repeat(32)],
      accounting_inputs: [{ value_commitment: '55'.repeat(32) }, { value_commitment: '66'.repeat(32) }],
      accounting_outputs: [{ value_commitment: '77'.repeat(32) }, { value_commitment: '88'.repeat(32) }],
      proof: '99',
      spend_authorization_signatures: ['aa', 'bb'],
    };
    const rejected = await executeShieldedNavswapSwap({
      route: 'shielded_navswap',
      wallet_address: quote.wallet_address,
      quote,
      quote_binding_hash: quote.quote_binding_hash,
      swap_action_json: JSON.stringify({ ...opaqueAction, asset_id: 'aa'.repeat(48) }),
    });
    assert.strictEqual(rejected.ok, false);
    assert.strictEqual(rejected.code, 'shielded_swap_action_cleartext_rejected');

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
      const privateActionRejected = await executeShieldedNavswapSwap({
        route: 'shielded_navswap',
        wallet_address: quote.wallet_address,
        quote,
        quote_binding_hash: quote.quote_binding_hash,
        swap_action_json: JSON.stringify({
          ...opaqueAction,
          private_witness: { [privateField]: 'ab'.repeat(32) },
        }),
      });
      assert.strictEqual(privateActionRejected.ok, false, privateField);
      assert.strictEqual(
        privateActionRejected.code,
        'shielded_navswap_private_material_rejected',
        privateField,
      );
    }

    const oversizedSerializedAction = JSON.stringify({
      ...opaqueAction,
      padding: 'x'.repeat(1_048_576),
      private_witness: { diversifier: 'ab'.repeat(11) },
    });
    const oversizedPrivateActionRejected = await executeShieldedNavswapSwap({
      route: 'shielded_navswap',
      wallet_address: quote.wallet_address,
      quote,
      quote_binding_hash: quote.quote_binding_hash,
      swap_action_json: oversizedSerializedAction,
    });
    assert.strictEqual(oversizedPrivateActionRejected.ok, false);
    assert.strictEqual(
      oversizedPrivateActionRejected.code,
      'shielded_navswap_private_material_rejected',
      'oversized JSON-looking strings must fail closed at the custody boundary',
    );

    const custodyRejected = await executeShieldedNavswapSwap({
      route: 'shielded_navswap',
      wallet_address: quote.wallet_address,
      quote,
      quote_binding_hash: quote.quote_binding_hash,
      note_opening: 'do-not-send-this-to-the-proxy',
      swap_action_json: JSON.stringify(opaqueAction),
    });
    assert.strictEqual(custodyRejected.ok, false);
    assert.strictEqual(custodyRejected.code, 'shielded_navswap_private_material_rejected');

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
      const privateFieldRejected = await executeShieldedNavswapSwap({
        route: 'shielded_navswap',
        wallet_address: quote.wallet_address,
        quote,
        quote_binding_hash: quote.quote_binding_hash,
        [privateField]: 'ab'.repeat(32),
        swap_action_json: JSON.stringify(opaqueAction),
      });
      assert.strictEqual(privateFieldRejected.ok, false, privateField);
      assert.strictEqual(
        privateFieldRejected.code,
        'shielded_navswap_private_material_rejected',
        privateField,
      );
    }

    const mismatchedBinding = await executeShieldedNavswapSwap({
      route: 'shielded_navswap',
      wallet_address: quote.wallet_address,
      quote,
      quote_binding_hash: '0'.repeat(64),
      swap_action_json: JSON.stringify(opaqueAction),
    });
    assert.strictEqual(mismatchedBinding.ok, false);
    assert.strictEqual(mismatchedBinding.code, 'shielded_swap_quote_binding_mismatch');

    const stale = await executeShieldedNavswapSwap({
      route: 'shielded_navswap',
      wallet_address: quote.wallet_address,
      quote: { ...quote, quote_expires_at_ms: '1' },
      quote_binding_hash: quote.quote_binding_hash,
      swap_action_json: JSON.stringify(opaqueAction),
    });
    assert.strictEqual(stale.ok, false);
    assert.strictEqual(stale.code, 'shielded_swap_quote_expired');
  });
}

async function testShieldedNavswapSwapSubmitUsesWarmServiceForBatchOnly() {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-shielded-swap-route-test-'));
  const nodeBin = path.join(tmpDir, 'fake-postfiat-node.js');
  const artifactRoot = path.join(tmpDir, 'artifacts');
  let batchRequests = 0;
  let batchRequestBody = null;
  let certifiedTransports = 0;
  let service;

  fs.writeFileSync(nodeBin, `#!/usr/bin/env node
const fs = require('fs');
const args = process.argv.slice(2);
if (args[0] === 'shield-batch-swap') {
  process.stderr.write('shield-batch-swap subprocess must not be used\\n');
  process.exit(17);
}
if (args[0] !== 'transport-peer-certified-batch-round') {
  process.stderr.write('unexpected command ' + args[0] + '\\n');
  process.exit(18);
}
const batchFile = args[args.indexOf('--batch-file') + 1];
if (!batchFile || !fs.existsSync(batchFile)) {
  process.stderr.write('missing batch file\\n');
  process.exit(19);
}
JSON.parse(fs.readFileSync(batchFile, 'utf8'));
process.stdout.write(JSON.stringify({
  round_ok: true,
  local_accepted_count: '1',
  local_rejected_count: '0',
  local_hot_finality: [{ receipt: { accepted: true, id: 'warm-service-test' } }]
}));
`, { mode: 0o700 });
  fs.chmodSync(nodeBin, 0o700);

  const batch = {
    schema: 'postfiat-shielded-action-batch-v1',
    actions: [{ kind: 'shielded_swap_v1', payload: { swap_json: '{}' } }],
  };
  const opaqueAction = {
    schema: 'postfiat-asset-orchard-swap-action-v1',
    pool_id: 'asset-orchard-v1',
    nullifiers: ['11'.repeat(32), '22'.repeat(32)],
    output_commitments: ['33'.repeat(32), '44'.repeat(32)],
    accounting_inputs: [{ value_commitment: '55'.repeat(32) }, { value_commitment: '66'.repeat(32) }],
    accounting_outputs: [{ value_commitment: '77'.repeat(32) }, { value_commitment: '88'.repeat(32) }],
    proof: '99',
    spend_authorization_signatures: ['aa', 'bb'],
  };
  const rawAction = JSON.stringify(opaqueAction);

  service = http.createServer((req, res) => {
    assert.strictEqual(req.method, 'POST');
    assert.strictEqual(req.url, '/asset-orchard/swap-batch');
    let raw = '';
    req.on('data', (chunk) => { raw += chunk.toString('utf8'); });
    req.on('end', () => {
      batchRequests += 1;
      batchRequestBody = JSON.parse(raw);
      assert.strictEqual(batchRequestBody.route, 'shielded_navswap');
      assert.strictEqual(batchRequestBody.swap_action_json, rawAction);
      assert.strictEqual(batchRequestBody.note_opening, undefined);
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        ok: true,
        schema: 'postfiat-asset-orchard-local-swap-batch-v1',
        batch,
        batch_json: JSON.stringify(batch, null, 2),
      }));
    });
  });
  await new Promise((resolve) => service.listen(0, '127.0.0.1', resolve));
  const { port } = service.address();

  try {
    await withEnvAsync(shieldedSwapEnv({
      ASSET_ORCHARD_LOCAL_SERVICE_URL: `http://127.0.0.1:${port}`,
      ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS: '1000',
      NAVSWAP_SHIELDED_INGRESS_NODE_BIN: nodeBin,
      NAVSWAP_SHIELDED_SWAP_ARTIFACT_ROOT: artifactRoot,
    }), async () => {
      const quote = await executeShieldedNavswapQuote({
        route: 'shielded_navswap',
        wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
        from_asset: 'a651',
        to_asset: 'a652',
        amount_atoms: '2000000',
      });
      assert.strictEqual(quote.ok, true);
      const response = await executeShieldedNavswapSwap({
        route: 'shielded_navswap',
        wallet_address: quote.wallet_address,
        quote,
        quote_binding_hash: quote.quote_binding_hash,
        swap_action_json: rawAction,
      });
      assert.strictEqual(response.ok, true);
      assert.strictEqual(response.status, 'swap_certified');
      assert.deepStrictEqual(response.batch, batch);
      assert.strictEqual(response.timings_ms.batch_route, 'resident_service');
      assert.strictEqual(response.timings_ms.batch_subprocess_ms, null);
      assert.strictEqual(typeof response.timings_ms.batch_resident_service_ms, 'number');
      assert.strictEqual(typeof response.timings_ms.certified_round_ms, 'number');
      assert.strictEqual(
        response.timings_ms.phase_timings.schema,
        'postfiat-wallet-proxy-shielded-round-phase-timings-v1',
      );
      assert.strictEqual(typeof response.timings_ms.phase_timings.batch_ready_to_round_start_ms, 'number');
      certifiedTransports += 1;
    });
    assert.strictEqual(batchRequests, 1);
    assert.strictEqual(certifiedTransports, 1);
  } finally {
    if (service) await new Promise((resolve) => service.close(resolve));
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

function startStatusRpcServer(statusFactory) {
  const sockets = new Set();
  const server = net.createServer((socket) => {
    sockets.add(socket);
    socket.on('close', () => sockets.delete(socket));
    let buffer = '';
    socket.on('data', (chunk) => {
      buffer += chunk.toString('utf8');
      let idx;
      while ((idx = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, idx).trim();
        buffer = buffer.slice(idx + 1);
        if (!line) continue;
        JSON.parse(line);
        socket.write(`${JSON.stringify({ ok: true, result: statusFactory() })}\n`);
      }
    });
  });
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      server._testSockets = sockets;
      resolve(server);
    });
  });
}

async function closeServer(server) {
  if (!server) return;
  if (server._testSockets) {
    for (const socket of server._testSockets) socket.destroy();
  }
  await new Promise((resolve) => server.close(resolve));
}

async function testShieldedLaggardCatchUpRunsRpcCatchUpAndRechecksConvergence() {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-shielded-catchup-test-'));
  const markerFile = path.join(tmpDir, 'caught-up.marker');
  const sshBin = path.join(tmpDir, 'fake-ssh.js');
  let laggardServer = null;
  let sourceServer = null;

  fs.writeFileSync(sshBin, `#!/usr/bin/env node
const fs = require('fs');
const args = process.argv.slice(2);
fs.writeFileSync(process.env.CATCHUP_MARKER, JSON.stringify(args, null, 2));
process.stdout.write(JSON.stringify({
  schema: 'postfiat-rpc-catch-up-v1',
  status: 'caught_up',
  local_node: 'validator-0',
  source_node: 'validator-2',
  local_height_before: 9,
  source_height: 10,
  applied_count: 1,
  local_height_after: 10,
  local_state_root_after: 'root-10'
}));
`, { mode: 0o700 });
  fs.chmodSync(sshBin, 0o700);

  try {
    laggardServer = await startStatusRpcServer(() => (
      fs.existsSync(markerFile)
        ? { node_id: 'validator-0', block_height: 10, state_root: 'root-10', block_tip_hash: 'tip-10' }
        : { node_id: 'validator-0', block_height: 9, state_root: 'root-9', block_tip_hash: 'tip-9' }
    ));
    sourceServer = await startStatusRpcServer(() => ({
      node_id: 'validator-2',
      block_height: 10,
      state_root: 'root-10',
      block_tip_hash: 'tip-10',
    }));

    const laggardPort = laggardServer.address().port;
    const sourcePort = sourceServer.address().port;
    const topology = {
      topology_id: 'test-catchup',
      peers: [
        { node_id: 'validator-0', host: '127.0.0.1', rpc_port: laggardPort },
        { node_id: 'validator-2', host: '127.0.0.1', rpc_port: sourcePort },
      ],
    };
    const topologyFile = path.join(tmpDir, 'topology.json');
    fs.writeFileSync(topologyFile, JSON.stringify(topology, null, 2), { mode: 0o600 });

    await withEnvAsync({
      SHIELDED_EARLY_QUORUM: 'true',
      SHIELDED_LAGGARD_CATCHUP_SSH_BIN: sshBin,
      SHIELDED_LAGGARD_CATCHUP_SSH_KEY: path.join(tmpDir, 'fake-key'),
      SHIELDED_LAGGARD_CATCHUP_SSH_USER: 'root',
      SHIELDED_LAGGARD_CATCHUP_REMOTE_USER: 'postfiat',
      SHIELDED_LAGGARD_CATCHUP_REMOTE_NODE_BIN: '/usr/local/bin/postfiat-node',
      SHIELDED_LAGGARD_CATCHUP_DATA_DIR_TEMPLATE: '/var/lib/postfiat/{validator}',
      SHIELDED_LAGGARD_CATCHUP_SOURCES: 'validator-2',
      SHIELDED_LAGGARD_CATCHUP_RECHECK_ATTEMPTS: '2',
      SHIELDED_LAGGARD_CATCHUP_RECHECK_DELAY_MS: '10',
      CATCHUP_MARKER: markerFile,
    }, async () => {
      const result = await runShieldedLaggardCatchUp({
        topology: topologyFile,
      }, {
        certification: { block_height: 10 },
        local_state: { block_height: 10, state_root: 'root-10', block_tip_hash: 'tip-10' },
      }, tmpDir);

      assert.strictEqual(result.enabled, true);
      assert.strictEqual(result.ok, true);
      assert.strictEqual(result.status, 'converged');
      assert.strictEqual(result.laggard_count, 1);
      assert.strictEqual(result.laggards[0].validator_id, 'validator-0');
      assert.strictEqual(result.catch_ups.length, 1);
      assert.strictEqual(result.catch_ups[0].target, 'validator-0');
      assert.strictEqual(result.catch_ups[0].source, 'validator-2');
      assert.strictEqual(result.catch_ups[0].work_dir, '/var/lib/postfiat/validator-0/rpc-catch-up-work');
      assert.strictEqual(result.convergence.ok_count, 2);
      assert.strictEqual(result.convergence.height, 10);
      assert.strictEqual(result.convergence.root, 'root-10');
      assert.ok(fs.existsSync(path.join(tmpDir, 'laggard-catch-up.json')));

      const sshArgs = JSON.parse(fs.readFileSync(markerFile, 'utf8'));
      const sshCommand = sshArgs.join(' ');
      assert.ok(sshCommand.includes('rpc-catch-up'));
      assert.ok(sshCommand.includes('--source-rpc-port'));
      assert.ok(sshCommand.includes(String(sourcePort)));
      assert.ok(sshCommand.includes('/var/lib/postfiat/validator-0'));
      assert.ok(sshCommand.includes('--work-dir'));
      assert.ok(sshCommand.includes('/var/lib/postfiat/validator-0/rpc-catch-up-work'));
    });
  } finally {
    closeUpstreamRpcConnections();
    await closeServer(laggardServer);
    await closeServer(sourceServer);
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

async function testShieldedLaggardCatchUpWaitsForDeferredSource() {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-shielded-catchup-wait-test-'));
  const markerFile = path.join(tmpDir, 'caught-up.marker');
  const sshBin = path.join(tmpDir, 'fake-ssh.js');
  let laggardServer = null;
  let sourceServer = null;
  let sourceStatusCalls = 0;

  fs.writeFileSync(sshBin, `#!/usr/bin/env node
const fs = require('fs');
const args = process.argv.slice(2);
fs.writeFileSync(process.env.CATCHUP_MARKER, JSON.stringify(args, null, 2));
process.stdout.write(JSON.stringify({
  schema: 'postfiat-rpc-catch-up-v1',
  status: 'caught_up',
  local_node: 'validator-0',
  source_node: 'validator-2',
  local_height_before: 9,
  source_height: 10,
  applied_count: 1,
  local_height_after: 10,
  local_state_root_after: 'root-10'
}));
`, { mode: 0o700 });
  fs.chmodSync(sshBin, 0o700);

  try {
    laggardServer = await startStatusRpcServer(() => (
      fs.existsSync(markerFile)
        ? { node_id: 'validator-0', block_height: 10, state_root: 'root-10', block_tip_hash: 'tip-10' }
        : { node_id: 'validator-0', block_height: 9, state_root: 'root-9', block_tip_hash: 'tip-9' }
    ));
    sourceServer = await startStatusRpcServer(() => {
      sourceStatusCalls += 1;
      return sourceStatusCalls >= 3
        ? { node_id: 'validator-2', block_height: 10, state_root: 'root-10', block_tip_hash: 'tip-10' }
        : { node_id: 'validator-2', block_height: 9, state_root: 'root-9', block_tip_hash: 'tip-9' };
    });

    const laggardPort = laggardServer.address().port;
    const sourcePort = sourceServer.address().port;
    const topology = {
      topology_id: 'test-catchup-wait',
      peers: [
        { node_id: 'validator-0', host: '127.0.0.1', rpc_port: laggardPort },
        { node_id: 'validator-2', host: '127.0.0.1', rpc_port: sourcePort },
      ],
    };
    const topologyFile = path.join(tmpDir, 'topology.json');
    fs.writeFileSync(topologyFile, JSON.stringify(topology, null, 2), { mode: 0o600 });

    await withEnvAsync({
      SHIELDED_EARLY_QUORUM: 'true',
      SHIELDED_LAGGARD_CATCHUP_SSH_BIN: sshBin,
      SHIELDED_LAGGARD_CATCHUP_SSH_KEY: path.join(tmpDir, 'fake-key'),
      SHIELDED_LAGGARD_CATCHUP_SSH_USER: 'root',
      SHIELDED_LAGGARD_CATCHUP_REMOTE_USER: 'postfiat',
      SHIELDED_LAGGARD_CATCHUP_REMOTE_NODE_BIN: '/usr/local/bin/postfiat-node',
      SHIELDED_LAGGARD_CATCHUP_DATA_DIR_TEMPLATE: '/var/lib/postfiat/{validator}',
      SHIELDED_LAGGARD_CATCHUP_SOURCES: 'validator-2',
      SHIELDED_LAGGARD_CATCHUP_SOURCE_WAIT_ATTEMPTS: '4',
      SHIELDED_LAGGARD_CATCHUP_SOURCE_WAIT_DELAY_MS: '10',
      SHIELDED_LAGGARD_CATCHUP_RECHECK_ATTEMPTS: '2',
      SHIELDED_LAGGARD_CATCHUP_RECHECK_DELAY_MS: '10',
      CATCHUP_MARKER: markerFile,
    }, async () => {
      const result = await runShieldedLaggardCatchUp({
        topology: topologyFile,
      }, {
        certification: { block_height: 10 },
        local_state: { block_height: 10, state_root: 'root-10', block_tip_hash: 'tip-10' },
      }, tmpDir);

      assert.strictEqual(result.enabled, true);
      assert.strictEqual(result.ok, true);
      assert.strictEqual(result.status, 'converged');
      assert.strictEqual(result.source_wait.status, 'source_available');
      assert.ok(result.source_wait.attempts > 1);
      assert.ok(result.source_wait.waited_ms >= 10);
      assert.deepStrictEqual(result.source_wait.source_validator_ids, ['validator-2']);
      assert.strictEqual(result.laggard_count, 1);
      assert.strictEqual(result.laggards[0].validator_id, 'validator-0');
      assert.strictEqual(result.catch_ups.length, 1);
      assert.strictEqual(result.catch_ups[0].target, 'validator-0');
      assert.strictEqual(result.catch_ups[0].source, 'validator-2');
      const sourceReadyRow = result.source_ready.find((row) => row.endpoint?.validatorId === 'validator-2');
      assert.strictEqual(sourceReadyRow.status.block_height, 10);
      assert.strictEqual(result.convergence.ok_count, 2);
      assert.strictEqual(result.convergence.height, 10);
      assert.strictEqual(result.convergence.root, 'root-10');
      assert.ok(fs.existsSync(path.join(tmpDir, 'laggard-catch-up.json')));
    });
  } finally {
    closeUpstreamRpcConnections();
    await closeServer(laggardServer);
    await closeServer(sourceServer);
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

async function testShieldedNavswapEgressRequiresDisclosureBoundary() {
  await withEnvAsync(shieldedSwapEnv({ NAVSWAP_ENABLE_SHIELDED_EGRESS: 'true' }), async () => {
    const caps = navswapCapabilities(new Date('2026-07-02T22:42:00.000Z'));
    const route = caps.routes.shielded_navswap;
    assert.strictEqual(route.status, 'step9_egress_ready');
    assert.strictEqual(route.can_egress, true);
    assert.strictEqual(route.egress.enabled, true);
    assert.strictEqual(route.egress.endpoint, '/api/shielded-nav-swap/egress');
    assert.strictEqual(route.egress.bridge_out_requires_public_exit_receipt, true);
    assertNoTrustlessDisplay(route);

    const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
    const assetId = 'd'.repeat(96);
    const disclosure = shieldedPrivateEgressDisclosureFields({
      walletAddress: wallet,
      to: wallet,
      assetId,
      amountAtoms: '2000',
      noteCommitment: 'a'.repeat(64),
      policyId: route.egress.policy_id,
    });
    const disclosureHash = shieldedPrivateEgressDisclosureHash(disclosure);

    const missingAck = await executeShieldedNavswapEgress({
      route: 'shielded_navswap',
      wallet_address: wallet,
      to: wallet,
      asset_id: assetId,
      amount_atoms: '2000',
      note_commitment: 'a'.repeat(64),
      policy_id: route.egress.policy_id,
      disclosure_hash: disclosureHash,
      egress_json: '{}',
    });
    assert.strictEqual(missingAck.ok, false);
    assert.strictEqual(missingAck.code, 'shielded_egress_disclosure_ack_required');

    const mismatchedHash = await executeShieldedNavswapEgress({
      route: 'shielded_navswap',
      wallet_address: wallet,
      to: wallet,
      asset_id: assetId,
      amount_atoms: '2000',
      note_commitment: 'a'.repeat(64),
      policy_id: route.egress.policy_id,
      disclosure_hash: '0'.repeat(64),
      disclosure_ack: true,
      egress_json: '{}',
    });
    assert.strictEqual(mismatchedHash.ok, false);
    assert.strictEqual(mismatchedHash.code, 'shielded_egress_disclosure_hash_mismatch');
    assert.strictEqual(mismatchedHash.expected_disclosure_hash, disclosureHash);
  });
}

async function testShieldedNavswapProverReadinessUsesLocalService() {
  let calls = 0;
  const localService = http.createServer((req, res) => {
    calls += 1;
    assert.strictEqual(req.method, 'GET');
    assert.strictEqual(req.url, '/asset-orchard/readiness');
    const body = {
      ok: true,
      ready: true,
      local_only: true,
      service: 'asset-orchard-local-service',
      bind: '127.0.0.1:8789',
      pool_id: 'asset-orchard-v1',
      circuit_id: 'asset-orchard-swap-v1',
      k: 15,
      prover_warm: {
        schema: 'postfiat-asset-orchard-local-service-prewarm-ready-v1',
        enabled: true,
        ready: true,
        status: 'ready',
        prewarm_ready_file: '/tmp/prewarm-ready.json',
        circuits: {
          swap: {
            circuit_id: 'asset-orchard-swap-v1',
            ready: true,
            status: 'ready',
            k: 15,
            params_hash: 'swap-params',
            vk_hash: 'swap-vk',
          },
          private_egress: {
            circuit_id: 'asset-orchard-private-egress-v1',
            ready: true,
            status: 'ready',
            k: 15,
            params_hash: 'egress-params',
            vk_hash: 'egress-vk',
          },
          ingress_notes: {
            circuit_id: 'asset-orchard-ingress-notes',
            ready: true,
            status: 'not_applicable',
          },
        },
      },
    };
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(body));
  });
  await new Promise((resolve) => localService.listen(0, '127.0.0.1', resolve));
  const { port } = localService.address();
  try {
    await withEnvAsync({
      ASSET_ORCHARD_LOCAL_SERVICE_URL: `http://127.0.0.1:${port}`,
      ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS: '1000',
    }, async () => {
      const readiness = await executeShieldedNavswapProverReadiness();
      assert.strictEqual(readiness.ok, true);
      assert.strictEqual(readiness.ready, true);
      assert.strictEqual(readiness.status, 'ready');
      assert.strictEqual(readiness.params_hash, 'swap-params');
      assert.strictEqual(readiness.vk_hash, 'swap-vk');
      assert.strictEqual(readiness.prover_warm.prewarm_ready_file, '/tmp/prewarm-ready.json');
      assert.strictEqual(calls, 1);
    });
  } finally {
    await new Promise((resolve) => localService.close(resolve));
  }
}

function testShieldedCertifiedRoundArgsWaitForFullFleet() {
  const args = buildShieldedCertifiedRoundArgs({
    data_dir: '/data',
    topology: '/data/remote-topology.json',
    key_file: '/data/validator_keys.json',
    proposal_key_file: '/data/proposal_validator_keys.json',
    timeout_ms: 2400000,
  }, '/tmp/batch.json', '/tmp/artifacts');
  assert.ok(args.includes('transport-peer-certified-batch-round'));
  assert.deepStrictEqual(
    args.slice(args.indexOf('--timeout-ms'), args.indexOf('--timeout-ms') + 2),
    ['--timeout-ms', '2400000'],
  );
  assert.ok(args.includes('--proposal-key-file'));
  assert.ok(args.includes('--allow-existing-mempool'));
  assert.ok(!args.includes('--allow-peer-failures'));
  assert.ok(!args.includes('--quorum-early-full-propagation'));
  assert.ok(!args.includes('--local-apply-before-certified-send'));
}

async function testShieldedNavswapQuoteRequiresLiquidityConfig() {
  await withEnvAsync({
    A652_ASSET_ID: undefined,
    NAVSWAP_SHIELDED_ASSET_ISSUER: undefined,
    NAVSWAP_SHIELDED_LIQUIDITY_COMMITMENT: undefined,
  }, async () => {
    const quote = await executeShieldedNavswapQuote({
      route: 'shielded_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      from_asset: 'a651',
      to_asset: 'a652',
      amount_atoms: '2000000',
    });
    assert.strictEqual(quote.ok, false);
    assert.strictEqual(quote.code, 'shielded_navswap_liquidity_configuration_required');
    assert.strictEqual(quote.can_prove, false);
    assert.strictEqual(quote.can_run, false);
    assert.ok(quote.missing.includes('A652_ASSET_ID'));
    assert.ok(quote.missing.includes('NAVSWAP_SHIELDED_LIQUIDITY_COMMITMENT'));
  });
}

function testUniswapHandoffRejectsLegacyPoolConfig() {
  withEnv({
    NAVSWAP_WRAPPED_NAVCOIN_TOKEN: '0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e',
    NAVSWAP_HANDOFF_CONTROLLER: '0x1111111111111111111111111111111111111111',
    NAVSWAP_SETTLEMENT_ADAPTER: '0x1212121212121212121212121212121212121212',
    NAVSWAP_VERIFIER_MODE: 'threshold-controlled',
    NAVSWAP_UNISWAP_POOL_ID: '0x2222222222222222222222222222222222222222222222222222222222222222',
    NAVSWAP_UNISWAP_ROUTER: '0x3333333333333333333333333333333333333333',
  }, () => {
    const caps = navswapCapabilities(new Date('2026-06-29T00:00:00.000Z'));
    const route = caps.routes.uniswap_atomic_handoff;
    assert.strictEqual(route.status, 'disabled_legacy_pool_rejected');
    assert.strictEqual(route.enabled, false);
    assert.strictEqual(route.can_quote, false);
    assert.strictEqual(route.config.legacy_pool_selected, true);
    assert.ok(route.config.missing.includes('bridge-aware token/pool must not be the legacy a651/USDC pool'));

    const quote = buildNavswapQuoteResponse({ route: 'uniswap_atomic_handoff', amount: '1' });
    assert.strictEqual(quote.ok, false);
    assert.strictEqual(quote.code, 'legacy_pool_rejected');
    assert.strictEqual(quote.legacy_pool_id, '0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84');
    assert.strictEqual(quote.legacy_token, '0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e');
    assertNoTrustlessDisplay(route);
    assertNoTrustlessDisplay(quote);
  });

  withEnv({
    NAVSWAP_NATIVE_NAV_ASSET_ID: 'd'.repeat(96),
    NAVSWAP_ROUTE_SUPPLY_CAP_ATOMS: '100000000',
    NAVSWAP_PACKET_NOTIONAL_CAP_ATOMS: '1000000',
    NAVSWAP_SEED_NAV_EPOCH: '7',
    NAVSWAP_SEED_USDC_ATOMS: '100000000',
    NAVSWAP_SEED_WRAPPED_NAVCOIN_ATOMS: '100000',
    NAVSWAP_LP_RECIPIENT: '0x7777777777777777777777777777777777777777',
    NAVSWAP_LP_CUSTODY_POLICY: 'controlled_launch_lp',
    NAVSWAP_WRAPPED_NAVCOIN_TOKEN: '0x4444444444444444444444444444444444444444',
    NAVSWAP_HANDOFF_CONTROLLER: '0x1111111111111111111111111111111111111111',
    NAVSWAP_SETTLEMENT_ADAPTER: '0x1212121212121212121212121212121212121212',
    NAVSWAP_VERIFIER_MODE: 'threshold-controlled',
    NAVSWAP_UNISWAP_POOL_ID: '0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84',
    NAVSWAP_UNISWAP_ROUTER: '0x3333333333333333333333333333333333333333',
  }, () => {
    const caps = navswapCapabilities(new Date('2026-06-29T00:00:00.000Z'));
    const route = caps.routes.uniswap_atomic_handoff;
    assert.strictEqual(route.status, 'disabled_legacy_pool_rejected');
    assert.strictEqual(route.config.legacy_pool_selected, true);

    const quote = buildNavswapQuoteResponse({ route: 'uniswap_atomic_handoff', amount: '1' });
    assert.strictEqual(quote.ok, false);
    assert.strictEqual(quote.code, 'legacy_pool_rejected');
    assertNoTrustlessDisplay(route);
    assertNoTrustlessDisplay(quote);
  });
}

function testUniswapHandoffControlledBetaCapabilityAndRunPacket() {
  withEnv(uniswapBetaEnv({ NAVSWAP_ENABLE_UNISWAP_BETA_RUNS: 'true' }), () => {
    const caps = navswapCapabilities(new Date('2026-07-01T00:00:00.000Z'));
    const route = caps.routes.uniswap_atomic_handoff;
    assert.strictEqual(route.status, 'controlled_beta_run_ready');
    assert.strictEqual(route.enabled, true);
    assert.strictEqual(route.can_quote, true);
    assert.strictEqual(route.can_run, true);
    assert.strictEqual(route.route_family, 'composite_primary_mint_to_ethereum_venue');
    assert.strictEqual(route.route_trust_class, 'CONTROLLED');
    assert.strictEqual(route.release_stage, 'explicit_beta');
    assert.strictEqual(route.explicit_beta, true);
    assert.strictEqual(route.public_routing_enabled, false);
    assert.strictEqual(route.paused, false);
    assert.strictEqual(route.route_supply_cap_atoms, 100000000);
    assert.strictEqual(route.supply_cap_remaining_atoms, 99999000);
    assert.strictEqual(route.packet_notional_cap_atoms, 1000000);
    assert.strictEqual(route.privacy.label, 'CONTROLLED beta');
    assert.match(route.privacy.warning, /Public routing is disabled/);
    assertNoTrustlessDisplay(route);
    assert.deepStrictEqual(route.required_next, []);

    const quote = buildNavswapQuoteResponse({
      route: 'uniswap_atomic_handoff',
      from_asset: 'pfUSDC',
      to_asset: 'USDC',
      amount: '100',
      recipient: '0x6666666666666666666666666666666666666666',
      minimum_output: '95',
      deadline: '1924992000',
      swap_path_hash: '0x' + '9'.repeat(64),
    });
    assert.strictEqual(quote.ok, true);
    assert.strictEqual(quote.status, 'controlled_beta_run_ready');
    assert.strictEqual(quote.can_run, true);
    assert.strictEqual(quote.route_family, 'composite_primary_mint_to_ethereum_venue');
    assert.strictEqual(quote.public_routing_enabled, false);
    assert.strictEqual(quote.paused, false);
    assert.strictEqual(quote.route_supply_cap_atoms, 100000000);
    assert.strictEqual(quote.supply_cap_remaining_atoms, 99999000);
    assert.strictEqual(quote.packet_notional_cap_atoms, 1000000);
    assert.strictEqual(quote.mint_and_swap_uniswap.execution_enabled, false);
    assert.strictEqual(quote.mint_and_swap_uniswap.route_family, 'composite_primary_mint_to_ethereum_venue');
    assertNoTrustlessDisplay(quote);

    const run = buildNavswapRunResponse({
      route: 'uniswap_atomic_handoff',
      from_asset: 'pfUSDC',
      to_asset: 'USDC',
      amount: '100',
      recipient: '0x6666666666666666666666666666666666666666',
      minimum_output: '95',
      deadline: '1924992000',
      swap_path_hash: '0x' + '9'.repeat(64),
    });
    assert.strictEqual(run.ok, true);
    assert.strictEqual(run.status, 'controlled_beta_packet_ready');
    assert.strictEqual(run.run_packet.schema, 'postfiat-pftl-uniswap-controlled-beta-run-packet-v1');
    assert.strictEqual(run.run_packet.route_trust_class, 'CONTROLLED');
    assert.strictEqual(run.run_packet.public_routing_enabled, false);
    assert.strictEqual(run.run_packet.route_supply_cap_atoms, 100000000);
    assert.strictEqual(run.run_packet.supply_cap_remaining_atoms, 99999000);
    assert.strictEqual(run.run_packet.packet_notional_cap_atoms, 1000000);
    assert.match(run.run_packet.route_config_digest, /^[0-9a-f]{96}$/);
    assert.ok(run.run_packet.terminal_states.includes('source_refundable_after_timeout'));
    assertNoTrustlessDisplay(run);
  });

  withEnv(uniswapBetaEnv({ NAVSWAP_UNISWAP_ROUTE_PAUSED: 'true' }), () => {
    const caps = navswapCapabilities(new Date('2026-07-01T00:00:00.000Z'));
    const route = caps.routes.uniswap_atomic_handoff;
    assert.strictEqual(route.status, 'configured_beta_disabled');
    assert.strictEqual(route.enabled, false);
    assert.strictEqual(route.can_quote, false);
    assert.strictEqual(route.paused, true);
    assert.ok(route.required_next.includes('route is paused'));

    const quote = buildNavswapQuoteResponse({
      route: 'uniswap_atomic_handoff',
      amount: '100',
      recipient: '0x6666666666666666666666666666666666666666',
      minimum_output: '95',
      deadline: '1924992000',
      swap_path_hash: '0x' + '9'.repeat(64),
    });
    assert.strictEqual(quote.ok, false);
    assert.strictEqual(quote.code, 'uniswap_handoff_beta_not_enabled');
    assert.ok(quote.blockers.includes('route is paused'));
  });
}

function testUniswapHandoffUsesNodeRouteDigestFixture() {
  withEnv(uniswapBetaEnv({ NAVSWAP_ENABLE_UNISWAP_BETA_RUNS: 'true' }), () => {
    const caps = navswapCapabilities(new Date('2026-07-01T00:00:00.000Z'));
    const route = caps.routes.uniswap_atomic_handoff;
    assert.strictEqual(route.config.route_config_digest, NODE_ROUTE_DIGEST_VECTOR.route_config_digest);
    assert.strictEqual(route.config.route_config_digest_authority, 'node');
    assert.strictEqual(route.config.route_config.route_family, 'primary_pftl_mint');
    assert.strictEqual(route.route_config_digest, NODE_ROUTE_DIGEST_VECTOR.route_config_digest);

    const quote = buildNavswapQuoteResponse({
      route: 'uniswap_atomic_handoff',
      from_asset: 'pfUSDC',
      to_asset: 'USDC',
      amount: '100',
      recipient: '0x6666666666666666666666666666666666666666',
      minimum_output: '95',
      deadline: '1924992000',
      swap_path_hash: '0x' + '9'.repeat(64),
    });
    assert.strictEqual(quote.ok, true);
    assert.strictEqual(quote.route_config_digest, NODE_ROUTE_DIGEST_VECTOR.route_config_digest);
    assert.strictEqual(
      quote.mint_and_swap_uniswap.route_config_digest,
      NODE_ROUTE_DIGEST_VECTOR.route_config_digest,
    );
  });
}

function testUniswapHandoffFinalityClassRequiresThreeWayAgreement() {
  withEnv(uniswapBetaEnv({
    NAVSWAP_ROUTE_TRUST_CLASS: 'TRUSTLESS_FINALITY',
    NAVSWAP_VERIFIER_MODE: 'trustless-finality',
  }), () => {
    const caps = navswapCapabilities(new Date('2026-07-01T00:00:00.000Z'));
    const route = caps.routes.uniswap_atomic_handoff;
    assert.strictEqual(route.route_trust_class, 'DISABLED');
    assert.strictEqual(route.enabled, false);
    assert.strictEqual(route.can_quote, false);
    assert.strictEqual(route.config.verifier_mode, 'finality_pending');
    assert.strictEqual(route.config.finality_agreement.status, 'incomplete');
    assert.strictEqual(route.config.finality_agreement.display_allowed, false);
    assert.ok(route.required_next.includes('finality agreement missing across registry, controller, and config digest'));
    assertNoTrustlessDisplay(route);

    const quote = buildNavswapQuoteResponse({
      route: 'uniswap_atomic_handoff',
      amount: '100',
      recipient: '0x6666666666666666666666666666666666666666',
      minimum_output: '95',
      deadline: '1924992000',
      swap_path_hash: '0x' + '9'.repeat(64),
    });
    assert.strictEqual(quote.ok, false);
    assert.strictEqual(quote.code, 'bridge_aware_pool_not_configured');
    assert.ok(quote.missing.includes('finality agreement missing across registry, controller, and config digest'));
    assertNoTrustlessDisplay(quote);
  });

  withEnv(uniswapBetaEnv({
    NAVSWAP_ROUTE_TRUST_CLASS: 'TRUSTLESS_FINALITY',
    NAVSWAP_VERIFIER_MODE: 'trustless-finality',
    NAVSWAP_ROUTE_REGISTRY_TRUST_CLASS: 'TRUSTLESS_FINALITY',
    NAVSWAP_ETHEREUM_CONTROLLER_TRUST_CLASS: 'TRUSTLESS_FINALITY',
    NAVSWAP_CONFIG_DIGEST_TRUST_CLASS: 'TRUSTLESS_FINALITY',
  }), () => {
    const route = navswapCapabilities(new Date('2026-07-01T00:00:00.000Z')).routes.uniswap_atomic_handoff;
    assert.strictEqual(route.route_trust_class, 'TRUSTLESS_FINALITY');
    assert.strictEqual(route.enabled, false);
    assert.strictEqual(route.can_quote, false);
    assert.strictEqual(route.config.verifier_mode, 'trustless-finality');
    assert.strictEqual(route.config.finality_agreement.status, 'agreed');
    assert.strictEqual(route.config.finality_agreement.display_allowed, true);
    assert.ok(route.required_next.includes('route trust class must be CONTROLLED for beta'));
  });
}

function testUniswapHandoffQuoteBindsMintAndSwapFields() {
  withEnv(uniswapBetaEnv(), () => {
    const missing = buildNavswapQuoteResponse({
      route: 'uniswap_atomic_handoff',
      from_asset: 'a651',
      to_asset: 'USDC',
      amount: '100',
    });
    assert.strictEqual(missing.ok, false);
    assert.strictEqual(missing.code, 'uniswap_handoff_quote_fields_required');
    assert.deepStrictEqual(missing.missing, ['recipient', 'minimum_output', 'deadline', 'swap_path_hash']);

    const quote = buildNavswapQuoteResponse({
      route: 'uniswap_atomic_handoff',
      from_asset: 'a651',
      to_asset: 'USDC',
      amount: '100',
      recipient: '0x6666666666666666666666666666666666666666',
      minimum_output: '95',
      deadline: '1924992000',
      swap_path_hash: '0x' + '9'.repeat(64),
      failure_behavior: 'refund_unconsumed_pftl_packet',
    });
    assert.strictEqual(quote.ok, true);
    assert.strictEqual(quote.status, 'controlled_beta_quote_ready');
    assert.strictEqual(quote.can_run, false);
    assert.strictEqual(quote.route_family, 'composite_primary_mint_to_ethereum_venue');
    assert.strictEqual(quote.route_trust_class, 'CONTROLLED');
    assert.strictEqual(quote.release_stage, 'explicit_beta');
    assert.strictEqual(quote.public_routing_enabled, false);
    assert.strictEqual(quote.paused, false);
    assert.strictEqual(quote.route_supply_cap_atoms, 100000000);
    assert.strictEqual(quote.supply_cap_remaining_atoms, 99999000);
    assert.strictEqual(quote.packet_notional_cap_atoms, 1000000);
    assert.match(quote.route_config_digest, /^[0-9a-f]{96}$/);
    assert.match(quote.quote_binding_hash, /^[0-9a-f]{64}$/);
    assert.strictEqual(quote.mint_and_swap_uniswap.schema, 'postfiat-navswap-mint-and-swap-uniswap-quote-v1');
    assert.strictEqual(quote.mint_and_swap_uniswap.operation, 'mint_and_swap_uniswap');
    assert.strictEqual(quote.mint_and_swap_uniswap.route_family, 'composite_primary_mint_to_ethereum_venue');
    assert.strictEqual(quote.mint_and_swap_uniswap.route_trust_class, 'CONTROLLED');
    assert.strictEqual(quote.mint_and_swap_uniswap.route_config_digest, quote.route_config_digest);
    assert.strictEqual(quote.mint_and_swap_uniswap.native_nav_asset_id, 'd'.repeat(96));
    assert.strictEqual(quote.config.route_config.settlement_adapter, '0x1212121212121212121212121212121212121212');
    assert.strictEqual(quote.mint_and_swap_uniswap.settlement_adapter, '0x1212121212121212121212121212121212121212');
    assert.strictEqual(quote.mint_and_swap_uniswap.swap_path_hash, '9'.repeat(64));
    assert.strictEqual(quote.mint_and_swap_uniswap.pool_id_or_path, '0x2222222222222222222222222222222222222222222222222222222222222222');
    assert.strictEqual(quote.mint_and_swap_uniswap.router, '0x3333333333333333333333333333333333333333');
    assert.strictEqual(quote.mint_and_swap_uniswap.token_in, '0x4444444444444444444444444444444444444444');
    assert.strictEqual(quote.mint_and_swap_uniswap.token_out, '0x5555555555555555555555555555555555555555');
    assert.strictEqual(quote.mint_and_swap_uniswap.amount_in, '100');
    assert.strictEqual(quote.mint_and_swap_uniswap.minimum_output, '95');
    assert.strictEqual(quote.mint_and_swap_uniswap.recipient, '0x6666666666666666666666666666666666666666');
    assert.strictEqual(quote.mint_and_swap_uniswap.deadline, '1924992000');
    assert.strictEqual(quote.mint_and_swap_uniswap.failure_behavior, 'refund_unconsumed_pftl_packet');
    assert.strictEqual(quote.mint_and_swap_uniswap.execution_enabled, false);
  });
}

async function testUniswapHandoffPreparesWalletOwnedSourceBatchFromNodeState() {
  await withEnvAsync(uniswapBetaEnv({
    NAVSWAP_SETTLEMENT_ASSET_ID: '8'.repeat(96),
    NAVSWAP_ENABLE_UNISWAP_BETA_RUNS: 'true',
    NAVSWAP_LAUNCH_CONFIG_DIGEST: '7'.repeat(96),
  }), async () => {
    const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
    const routeId = 'pftl-navcoin-uniswap-v1';
    const nativeAssetId = 'd'.repeat(96);
    const settlementAssetId = '8'.repeat(96);
    const reservePacketHash = 'e'.repeat(96);
    const routeConfigDigest = NODE_ROUTE_DIGEST_VECTOR.route_config_digest;
    const calls = [];
    const rpcStub = async (host, port, request) => {
      calls.push(request.method);
      if (request.method === 'navcoin_bridge_routes') {
        return {
          ok: true,
          result: {
            schema: 'postfiat-pftl-uniswap-routes-status-v1',
            route_count: 1,
            routes: [{
              route_id: routeId,
              route_family: 'primary_pftl_mint',
              route_config_digest: routeConfigDigest,
              route_trust_class: 'CONTROLLED',
              route_live: true,
              paused: false,
              native_nav_asset_id: nativeAssetId,
              settlement_asset_id: settlementAssetId,
              wrapped_navcoin_token: '0x4444444444444444444444444444444444444444',
              handoff_controller: '0x1111111111111111111111111111111111111111',
              settlement_adapter: '0x1212121212121212121212121212121212121212',
              ethereum_chain_id: 1,
              latest_finalized_nav_epoch: 7,
              route_supply_cap_atoms: 100000000,
              packet_notional_cap_atoms: 1000000,
              authorized_valid_supply_atoms: 0,
              supply_cap_remaining_atoms: 100000000,
              outstanding_bridge_claims_atoms: 0,
              pending_return_import_claims_atoms: 0,
              primary_subscription_count: 0,
              export_packet_count: 0,
              outstanding_export_packet_count: 0,
              consumed_export_packet_count: 0,
              refunded_export_packet_count: 0,
              return_burn_count: 0,
              pending_return_burn_count: 0,
              imported_return_burn_count: 0,
              ledger_hash: 'a'.repeat(96),
            }],
          },
        };
      }
      if (request.method === 'navcoin_bridge_supply_status') {
        assert.deepStrictEqual(request.params, { route_id: routeId });
        return {
          ok: true,
          result: {
            route_id: routeId,
            route_config_digest: routeConfigDigest,
            ledger_hash: 'b'.repeat(96),
            invariant_holds: true,
          },
        };
      }
      if (request.method === 'vault_bridge_status') {
        return {
          ok: true,
          result: request.params.asset_id === nativeAssetId
            ? {
              asset_id: nativeAssetId,
              finalized_epoch: 7,
              nav_per_unit: 699665834,
              valuation_unit: 'usd_1e8',
              finalized_reserve_packet_hash: reservePacketHash,
            }
            : {
              asset_id: settlementAssetId,
              finalized_epoch: 1,
              nav_per_unit: 1000000,
              valuation_unit: 'USDC',
              finalized_reserve_packet_hash: 'f'.repeat(96),
            },
        };
      }
      if (request.method === 'asset_info') {
        return {
          ok: true,
          result: {
            asset: {
              asset_id: request.params.asset_id,
              issuer: request.params.asset_id === nativeAssetId ? 'pfissuer' : 'pfsettlement',
              precision: 6,
            },
          },
        };
      }
      if (request.method === 'server_info') {
        return {
          ok: true,
          result: { ledger: { height: 20 } },
        };
      }
      if (request.method === 'account') {
        assert.deepStrictEqual(request.params, { address: wallet });
        return { ok: true, result: { account: { address: wallet, balance: 100 } } };
      }
      if (request.method === 'asset_fee_quote') {
        assert.strictEqual(request.params.source, wallet);
        return {
          ok: true,
          result: {
            source: wallet,
            minimum_fee: 1,
            sequence: calls.filter(method => method === 'asset_fee_quote').length,
            operation: JSON.parse(request.params.operation_json),
            sender_meets_reserve_after_fee: true,
            sender_meets_reserve_after_fee_and_reserve: true,
          },
        };
      }
      throw new Error(`unexpected RPC method ${request.method}`);
    };

    const prepared = await prepareNavswapWalletActionBatch({
      route: 'uniswap_atomic_handoff',
      wallet_address: wallet,
      amount: '1',
      ethereum_recipient: '0x7777777777777777777777777777777777777777',
    }, rpcStub);
    assert.strictEqual(prepared.ok, true);
    assert.deepStrictEqual(prepared.stages, ['pftl_uniswap_primary_subscribe', 'pftl_uniswap_export_debit']);
    assert.strictEqual(prepared.actions[0].operation.operation, 'pftl_uniswap_primary_subscribe');
    assert.strictEqual(prepared.actions[0].operation.settlement_value_atoms, 7000000);
    assert.strictEqual(prepared.actions[0].operation.nav_price_settlement_atoms_per_nav_atom, 7);
    assert.strictEqual(prepared.actions[1].operation.operation, 'pftl_uniswap_export_debit');
    assert.strictEqual(prepared.actions[1].operation.amount_atoms, 1000000);
    assert.match(prepared.actions[1].operation.packet_hash, /^[0-9a-f]{96}$/);
    assert.strictEqual(prepared.actions[0].user_intent.route_config_digest, routeConfigDigest);
    assert.strictEqual(prepared.actions[0].user_intent.route_trust_class, 'CONTROLLED');

    const quote = await executeNavswapQuote({
      route: 'uniswap_atomic_handoff',
      wallet_address: wallet,
      amount: '1',
      ethereum_recipient: '0x7777777777777777777777777777777777777777',
    }, rpcStub);
    assert.strictEqual(quote.ok, true);
    assert.strictEqual(quote.status, 'prepared_actions_ready');
    assert.strictEqual(quote.prepared_action_batch.action_count, 2);
    assert.strictEqual(quote.settlement_amount_atoms, '7000000');
    assert.strictEqual(quote.mint_amount_atoms, '1000000');
    assert.strictEqual(quote.wallet_pft.fee_preflight.ok, true);
    assert.strictEqual(quote.operator_completion.stage, 'pftl_uniswap_destination_consume');
    assert.ok(calls.includes('navcoin_bridge_routes'));
    assert.ok(calls.includes('asset_fee_quote'));
  });
}

async function testUniswapHandoffRunVerifiesPacketAndSubmitsDestinationConsume() {
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const routeId = 'pftl-a651-usdc-wallet-e2e-20260702-v1';
  const nativeAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const routeConfigDigest = NODE_ROUTE_DIGEST_VECTOR.route_config_digest;
  const reservePacketHash = 'f'.repeat(96);
  const operator = 'pfissuer0000000000000000000000000000000000';
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-uniswap-run-test-'));
  const keyFile = path.join(root, 'issuer.key.json');
  const signer = path.join(root, 'fake-postfiat-node.js');
  fs.writeFileSync(keyFile, JSON.stringify({ address: operator }), { mode: 0o600 });
  fs.writeFileSync(signer, `#!/usr/bin/env node
const fs = require('fs');
const args = process.argv.slice(2);
const quoteFile = args[args.indexOf('--quote-file') + 1];
const quote = JSON.parse(fs.readFileSync(quoteFile, 'utf8'));
process.stdout.write(JSON.stringify({
  schema: 'postfiat-signed-asset-transaction-v1',
  unsigned: { source: quote.source, operation: quote.operation },
  signature_hex: 'aa'
}));
`, { mode: 0o700 });
  fs.chmodSync(signer, 0o700);

  const methods = [];
  let submitted = null;
  try {
    await withEnvAsync(uniswapBetaEnv({
      NAVSWAP_ROUTE_ID: routeId,
      NAVSWAP_NATIVE_NAV_ASSET_ID: nativeAssetId,
      NAVSWAP_SETTLEMENT_ASSET_ID: settlementAssetId,
      NAVSWAP_ENABLE_UNISWAP_BETA_RUNS: 'true',
      NAVSWAP_OPERATOR_ISSUER_KEY_FILE: keyFile,
      NAVSWAP_OPERATOR_NODE_BIN: signer,
      NAVSWAP_UNISWAP_RETURN_FINALITY_BLOCKS: '64',
    }), async () => {
      const rpcStub = async (host, port, request) => {
        methods.push(request.method);
        if (request.method === 'navcoin_bridge_routes') {
          return {
            ok: true,
            result: {
              route_count: 1,
              routes: [{
                route_id: routeId,
                route_family: 'primary_pftl_mint',
                route_trust_class: 'CONTROLLED',
                route_live: true,
                paused: false,
                native_nav_asset_id: nativeAssetId,
                settlement_asset_id: settlementAssetId,
                route_config_digest: routeConfigDigest,
                route_supply_cap_atoms: 100000000,
                supply_cap_remaining_atoms: 99999000,
                packet_notional_cap_atoms: 1000000,
                ledger_hash: 'a'.repeat(96),
              }],
            },
          };
        }
        if (request.method === 'navcoin_bridge_supply_status') {
          return {
            ok: true,
            result: {
              route_id: routeId,
              route_config_digest: routeConfigDigest,
              ledger_hash: 'b'.repeat(96),
              invariant_holds: true,
            },
          };
        }
        if (request.method === 'vault_bridge_status') {
          return {
            ok: true,
            result: request.params.asset_id === nativeAssetId
              ? {
                asset_id: nativeAssetId,
                finalized_epoch: 7,
                nav_per_unit: 699665834,
                valuation_unit: 'usd_1e8',
                finalized_reserve_packet_hash: reservePacketHash,
              }
              : {
                asset_id: settlementAssetId,
                finalized_epoch: 1,
                nav_per_unit: 1000000,
                valuation_unit: 'USDC',
                finalized_reserve_packet_hash: 'e'.repeat(96),
              },
          };
        }
        if (request.method === 'asset_info') {
          return {
            ok: true,
            result: {
              asset: {
                asset_id: request.params.asset_id,
                issuer: request.params.asset_id === nativeAssetId ? operator : 'pfsettlement',
                precision: 6,
              },
            },
          };
        }
        if (request.method === 'server_info') {
          return { ok: true, result: { ledger: { height: 20 } } };
        }
        if (request.method === 'account') {
          return { ok: true, result: { account: { address: wallet, balance: 100 } } };
        }
        if (request.method === 'asset_fee_quote') {
          const operation = JSON.parse(request.params.operation_json);
          return {
            ok: true,
            result: {
              schema: 'postfiat-asset-fee-quote-v1',
              source: request.params.source,
              minimum_fee: 1,
              sequence: methods.filter(method => method === 'asset_fee_quote').length,
              chain_id: 'test-chain',
              genesis_hash: 'genesis',
              protocol_version: 1,
              sender_meets_reserve_after_fee: true,
              sender_meets_reserve_after_fee_and_reserve: true,
              operation,
            },
          };
        }
        if (request.method === 'navcoin_bridge_packet') {
          return {
            ok: true,
            result: {
              schema: 'postfiat-pftl-uniswap-packet-status-v1',
              route_id: routeId,
              route_config_digest: routeConfigDigest,
              packet_hash: request.params.packet_hash,
              ledger_hash: 'c'.repeat(96),
              packet: {
                packet_hash: request.params.packet_hash,
                nonce: quote.prepared_action_batch.actions[1].operation.export_nonce,
                source_wallet: wallet,
                ethereum_recipient: '0x7777777777777777777777777777777777777777',
                amount_atoms: 1000000,
                source_height: 21,
                destination_deadline_seconds: quote.prepared_action_batch.actions[1].operation.destination_deadline_seconds,
                refund_not_before_height: 26,
                status: 'SourceDebited',
                claim_class: 'outstanding_bridge_claim',
              },
            },
          };
        }
        if (request.method === 'mempool_submit_signed_asset_transaction_finality') {
          submitted = JSON.parse(request.params.signed_asset_transaction_json);
          return {
            ok: true,
            result: { tx_id: 'destination-consume-tx', round_ok: true },
          };
        }
        throw new Error(`unexpected RPC method ${request.method}`);
      };

      const quote = await executeNavswapQuote({
        route: 'uniswap_atomic_handoff',
        wallet_address: wallet,
        amount: '1',
        ethereum_recipient: '0x7777777777777777777777777777777777777777',
      }, rpcStub);
      assert.strictEqual(quote.ok, true);
      const walletResult = {
        count: 2,
        actions: quote.prepared_action_batch.actions,
        submissions: quote.prepared_action_batch.actions.map((action, index) => ({
          txId: index === 0 ? 'primary-subscribe-tx' : 'export-debit-tx',
          navswap_action: action,
          receipt: { accepted: true },
        })),
      };
      const run = await executeNavswapRun({
        route: 'uniswap_atomic_handoff',
        wallet_address: wallet,
        quote,
        wallet_action_result: walletResult,
        async: true,
        ethereum_consume_tx_hash: '9'.repeat(64),
        ethereum_consumed_height: '100',
      }, rpcStub);
      assert.strictEqual(run.ok, true);
      assert.strictEqual(run.status, 'running');
      const finalStatus = await waitForNavswapRun(
        run.run_id,
        status => status.status === 'destination_consume_submitted',
        1000,
      );
      assert.strictEqual(finalStatus.ok, true);
      assert.strictEqual(finalStatus.terminal, true);
      assert.strictEqual(submitted.unsigned.operation.operation, 'pftl_uniswap_destination_consume');
      assert.strictEqual(submitted.unsigned.operation.packet_hash, quote.operator_completion.packet_hash);
      assert.strictEqual(submitted.unsigned.operation.ethereum_consume_tx_hash, '9'.repeat(64));
      assert.strictEqual(submitted.unsigned.operation.consumed_height, 100);
      assert.strictEqual(submitted.unsigned.operation.finalized_height, 164);
      assert.strictEqual(finalStatus.result.receipt_verification.ok, true);
      assert.strictEqual(finalStatus.result.receipt_verification.trust_class, 'CONTROLLED');
      assert.strictEqual(finalStatus.result.receipt_verification.operator_attested_destination_events, true);
      assert.strictEqual(finalStatus.result.receipt_verification.operator_tx_id, 'destination-consume-tx');
      const events = navswapRunEvents(run.run_id);
      assert(events.events.some((event) => event.type === 'source_export_packet_verified'));
      const receipts = navswapRunReceipts(run.run_id);
      assert.strictEqual(receipts.receipts[0].type, 'pftl_uniswap_controlled_destination_completion');
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function testTransparentQuoteRefusesPlaceholder() {
  const quote = buildNavswapQuoteResponse({
    route: 'transparent_navswap',
    from_asset: 'pfUSDC',
    to_asset: 'a651',
    amount: '1',
  });
  assert.strictEqual(quote.ok, false);
  assert.strictEqual(quote.code, 'transparent_navswap_planner_inputs_required');
  assert.match(quote.message, /self-transfer placeholder/);
  assert.strictEqual(quote.next_endpoint, '/api/navswap/actions/prepare-batch');
  assert.ok(quote.required_planner_fields.includes('actions[]'));
}

async function testTransparentTrustSetActionPrepareIsRejected() {
  const assetId = 'd'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const result = await prepareNavswapWalletAction({
    route: 'transparent_navswap',
    stage: 'trust_set',
    wallet_address: wallet,
    asset_id: assetId,
    limit_atoms: '1000000',
  }, async () => {
    throw new Error('asset_info should not be called for rejected trust_set stage');
  });

  assert.strictEqual(result.ok, false);
  assert.strictEqual(result.schema, 'postfiat-navswap-wallet-action-prepare-v1');
  assert.strictEqual(result.code, 'transparent_navswap_trust_set_not_supported');
  assert.strictEqual(result.rejected_stage, 'trust_set');
}

async function testTransparentAllocateActionPrepareBuildsWalletAction() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const bucketId = 'b'.repeat(96);
  const receiptId = 'c'.repeat(96);
  const allocationId = 'a'.repeat(96);
  const reservePacketHash = 'e'.repeat(96);
  const calls = [];
  const result = await prepareNavswapWalletAction({
    route: 'transparent_navswap',
    stage: 'nav_subscription_allocate',
    wallet_address: wallet,
    nav_asset_id: navAssetId,
    settlement_asset_id: settlementAssetId,
    settlement_bucket_id: bucketId,
    settlement_receipt_id: receiptId,
    settlement_amount_atoms: '250000',
    mint_amount_atoms: '125000',
    pricing_nav_epoch: '7',
    primary_nav_price_atoms: '2000000',
    pricing_reserve_packet_hash: reservePacketHash,
    consume_supply_allocation_id: allocationId,
    subscription_id: 'navsub-test-1',
  }, async (host, port, request) => {
    calls.push({ host, port, request });
    assert.strictEqual(request.method, 'asset_info');
    assert.deepStrictEqual(request.params, { asset_id: navAssetId });
    return {
      ok: true,
      result: {
        asset: {
          asset_id: navAssetId,
          issuer: 'pfissuer',
        },
      },
    };
  });

  assert.strictEqual(result.ok, true);
  assert.strictEqual(result.schema, 'postfiat-navswap-wallet-action-prepare-v1');
  assert.strictEqual(result.stage, 'nav_subscription_allocate');
  assert.strictEqual(result.action.schema, 'postfiat-navswap-wallet-action-request-v1');
  assert.strictEqual(result.action.source, wallet);
  assert.strictEqual(result.action.user_intent.from_asset_id, settlementAssetId);
  assert.strictEqual(result.action.user_intent.to_asset_id, navAssetId);
  assert.strictEqual(result.action.user_intent.operator, 'pfissuer');
  assert.strictEqual(result.action.user_intent.subscription_id, 'navsub-test-1');
  assert.strictEqual(result.action.user_intent.route_family, 'primary_pftl_mint');
  assert.strictEqual(result.action.user_intent.route_trust_class, 'CONTROLLED');
  assert.strictEqual(result.action.user_intent.supply_effect, 'mints_new_native_navcoin_supply');
  assert.strictEqual(result.action.user_intent.pricing_source, 'finalized_pre_inflow_nav_snapshot');
  assert.strictEqual(result.action.user_intent.settlement_reserve_effect, 'added_after_primary_fill');
  assert.strictEqual(result.action.user_intent.uniswap_supply_effect, 'not_uniswap_supply');
  assert.strictEqual(result.action.user_intent.mint_amount_atoms, '125000');
  assert.strictEqual(result.action.user_intent.pricing_nav_epoch, '7');
  assert.strictEqual(result.action.user_intent.primary_nav_price_atoms, '2000000');
  assert.strictEqual(result.action.user_intent.pricing_reserve_packet_hash, reservePacketHash);
  assert.deepStrictEqual(result.action.operation, {
    operation: 'vault_bridge_nav_subscription_allocate',
    operator: 'pfissuer',
    nav_asset_id: navAssetId,
    settlement_asset_id: settlementAssetId,
    settlement_bucket_id: bucketId,
    settlement_receipt_id: receiptId,
    settlement_amount_atoms: 250000,
    consume_supply_owner: wallet,
    consume_supply_allocation_id: allocationId,
    nav_recipient: wallet,
    subscription_id: 'navsub-test-1',
  });
  assert.strictEqual(calls.length, 1);
}

async function testTransparentRedeemActionPrepareBuildsWalletAction() {
  const navAssetId = 'd'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const reservePacketHash = 'e'.repeat(96);
  const calls = [];
  const result = await prepareNavswapWalletAction({
    route: 'transparent_navswap',
    stage: 'nav_redeem_at_nav',
    wallet_address: wallet,
    nav_asset_id: navAssetId,
    redeem_amount_atoms: '125000',
    nav_epoch: '7',
    reserve_packet_hash: reservePacketHash,
  }, async (host, port, request) => {
    calls.push({ host, port, request });
    assert.strictEqual(request.method, 'asset_info');
    assert.deepStrictEqual(request.params, { asset_id: navAssetId });
    return {
      ok: true,
      result: {
        asset: {
          asset_id: navAssetId,
          issuer: 'pfissuer',
        },
      },
    };
  });

  assert.strictEqual(result.ok, true);
  assert.strictEqual(result.schema, 'postfiat-navswap-wallet-action-prepare-v1');
  assert.strictEqual(result.stage, 'nav_redeem_at_nav');
  assert.strictEqual(result.action.schema, 'postfiat-navswap-wallet-action-request-v1');
  assert.strictEqual(result.action.source, wallet);
  assert.strictEqual(result.action.user_intent.from_asset_id, navAssetId);
  assert.strictEqual(result.action.user_intent.max_redeem_amount_atoms, '125000');
  assert.strictEqual(result.action.user_intent.nav_epoch, '7');
  assert.strictEqual(result.action.user_intent.reserve_packet_hash, reservePacketHash);
  assert.strictEqual(result.action.user_intent.issuer, 'pfissuer');
  assert.deepStrictEqual(result.action.operation, {
    operation: 'nav_redeem_at_nav',
    owner: wallet,
    issuer: 'pfissuer',
    asset_id: navAssetId,
    amount: 125000,
    epoch: 7,
    reserve_packet_hash: reservePacketHash,
  });
  assert.strictEqual(calls.length, 1);
}

async function testTransparentActionBatchPrepareBuildsOrderedWalletActions() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const bucketId = 'b'.repeat(96);
  const receiptId = 'c'.repeat(96);
  const allocationId = 'a'.repeat(96);
  const reservePacketHash = 'e'.repeat(96);
  const calls = [];
  const result = await prepareNavswapWalletActionBatch({
    route: 'transparent_navswap',
    wallet_address: wallet,
    nav_asset_id: navAssetId,
    settlement_asset_id: settlementAssetId,
    actions: [
      {
        stage: 'nav_subscription_allocate',
        settlement_bucket_id: bucketId,
        settlement_receipt_id: receiptId,
        settlement_amount_atoms: '250000',
        mint_amount_atoms: '125000',
        pricing_nav_epoch: '7',
        primary_nav_price_atoms: '2000000',
        pricing_reserve_packet_hash: reservePacketHash,
        consume_supply_allocation_id: allocationId,
      },
      {
        stage: 'nav_redeem_at_nav',
        redeem_amount_atoms: '125000',
        nav_epoch: '7',
        reserve_packet_hash: reservePacketHash,
      },
    ],
  }, async (host, port, request) => {
    calls.push({ host, port, request });
    assert.strictEqual(request.method, 'asset_info');
    assert.deepStrictEqual(request.params, { asset_id: navAssetId });
    return {
      ok: true,
      result: {
        asset: {
          asset_id: navAssetId,
          issuer: 'pfissuer',
        },
      },
    };
  });

  assert.strictEqual(result.ok, true);
  assert.strictEqual(result.schema, 'postfiat-navswap-wallet-action-batch-prepare-v1');
  assert.strictEqual(result.action_schema, 'postfiat-navswap-wallet-action-request-v1');
  assert.strictEqual(result.action_count, 2);
  assert.deepStrictEqual(result.stages, ['nav_subscription_allocate', 'nav_redeem_at_nav']);
  assert.deepStrictEqual(result.actions.map(action => action.operation.operation), [
    'vault_bridge_nav_subscription_allocate',
    'nav_redeem_at_nav',
  ]);
  assert.strictEqual(result.actions.every(action => action.source === wallet), true);
  assert.strictEqual(calls.length, 2);
}

async function testTransparentActionPrepareRejectsUnsupportedStage() {
  const result = await prepareNavswapWalletAction({
    route: 'transparent_navswap',
    stage: 'nav_mint_at_nav',
    wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
  }, async () => {
    throw new Error('asset_info should not be called for unsupported stage');
  });
  assert.strictEqual(result.ok, false);
  assert.strictEqual(result.code, 'unsupported_navswap_wallet_action_stage');
}

async function testTransparentActionBatchPrepareRejectsFailedItem() {
  const result = await prepareNavswapWalletActionBatch({
    route: 'transparent_navswap',
    wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
    actions: [
      { stage: 'trust_set', asset_id: 'd'.repeat(96), limit_atoms: '1000000' },
      { stage: 'nav_mint_at_nav' },
    ],
  }, async () => {
    throw new Error('asset_info should not be called for rejected trust_set stage');
  });
  assert.strictEqual(result.ok, false);
  assert.strictEqual(result.code, 'transparent_navswap_trust_set_not_supported');
  assert.strictEqual(result.failed_index, 0);
  assert.strictEqual(result.prepared_count, 0);
}

async function testTransparentQuoteWithPlannerActionsReturnsPreparedBatch() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const bucketId = 'b'.repeat(96);
  const receiptId = 'c'.repeat(96);
  const allocationId = 'a'.repeat(96);
  const reservePacketHash = 'e'.repeat(96);
  const quote = await executeNavswapQuote({
    route: 'transparent_navswap',
    from_asset: settlementAssetId,
    to_asset: navAssetId,
    amount: '250000',
    wallet_address: wallet,
    nav_asset_id: navAssetId,
    settlement_asset_id: settlementAssetId,
    actions: [
      {
        stage: 'nav_subscription_allocate',
        settlement_bucket_id: bucketId,
        settlement_receipt_id: receiptId,
        settlement_amount_atoms: '250000',
        mint_amount_atoms: '250000',
        pricing_nav_epoch: '7',
        primary_nav_price_atoms: '1000000',
        pricing_reserve_packet_hash: reservePacketHash,
        consume_supply_allocation_id: allocationId,
      },
    ],
  }, async (host, port, request) => ({
    ok: true,
    result: {
      asset: {
        asset_id: request.params.asset_id,
        issuer: 'pfissuer',
      },
    },
  }));

  assert.strictEqual(quote.ok, true);
  assert.strictEqual(quote.schema, 'postfiat-navswap-quote-v1');
  assert.strictEqual(quote.route, 'transparent_navswap');
  assert.strictEqual(quote.status, 'prepared_actions_ready');
  assert.strictEqual(quote.route_family, 'primary_pftl_mint');
  assert.strictEqual(quote.route_trust_class, 'CONTROLLED');
  assert.strictEqual(quote.pricing_source, 'finalized_pre_inflow_nav_snapshot');
  assert.strictEqual(quote.supply_effect, 'mints_new_native_navcoin_supply');
  assert.strictEqual(quote.uniswap_supply_effect, 'not_uniswap_supply');
  assert.strictEqual(quote.requires_wallet_submit, true);
  assert.strictEqual(quote.from_asset, settlementAssetId);
  assert.strictEqual(quote.to_asset, navAssetId);
  assert.strictEqual(quote.amount, '250000');
  assert.strictEqual(quote.input_amount_atoms, '250000');
  assert.strictEqual(quote.settlement_amount_atoms, '250000');
  assert.strictEqual(quote.redeem_amount_atoms, null);
  assert.strictEqual(quote.expected_output, null);
  assert.strictEqual(quote.expected_output_asset, null);
  assert.strictEqual(quote.expected_output_unavailable_reason, 'operator_nav_mint_at_nav_not_prepared');
  assert.strictEqual(quote.prepared_action_batch.schema, 'postfiat-navswap-wallet-action-batch-prepare-v1');
  assert.deepStrictEqual(quote.prepared_action_batch.stages, ['nav_subscription_allocate']);
  assert.deepStrictEqual(
    quote.prepared_action_batch.actions.map(action => action.operation.operation),
    ['vault_bridge_nav_subscription_allocate'],
  );
}

async function testTransparentQuoteRejectsTrustSetPlannerAction() {
  const quote = await executeNavswapQuote({
    route: 'transparent_navswap',
    from_asset: '8'.repeat(96),
    to_asset: 'd'.repeat(96),
    amount: '250000',
    wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
    actions: [
      { stage: 'trust_set', asset_id: 'd'.repeat(96), limit_atoms: '1000000' },
    ],
  }, async () => {
    throw new Error('trust_set planner actions should be rejected before preparation');
  });
  assert.strictEqual(quote.ok, false);
  assert.strictEqual(quote.code, 'transparent_navswap_trust_set_not_supported');
  assert.strictEqual(quote.rejected_stage, 'trust_set');
}

async function testTransparentPlannerInputsSelectsSettlementSource() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const bucketId = 'b'.repeat(96);
  const receiptId = 'c'.repeat(96);
  const allocationId = 'a'.repeat(96);
  const calls = [];
  const plan = await planTransparentNavswapWalletActions({
    route: 'transparent_navswap',
    from_asset_id: settlementAssetId,
    to_asset_id: navAssetId,
    amount: '250000',
    wallet_address: wallet,
    subscription_id: 'navsub-test-1',
  }, async (host, port, request) => {
    calls.push(request.method);
    if (request.method === 'vault_bridge_status') {
      assert.ok([settlementAssetId, navAssetId].includes(request.params.asset_id));
      if (request.params.asset_id === navAssetId) {
        return {
          ok: true,
          result: {
            asset_id: navAssetId,
            issuer: 'pfissuer',
            valuation_unit: 'NAV_UNIT',
            finalized_epoch: 7,
            nav_per_unit: 1,
            finalized_reserve_packet_hash: 'e'.repeat(96),
          },
        };
      }
      return {
        ok: true,
        result: {
          asset_id: settlementAssetId,
          valuation_unit: 'SOURCE_UNIT',
          finalized_epoch: 7,
          bucket_count: 1,
          receipt_count: 1,
          allocation_count: 1,
          buckets: [{ bucket_id: bucketId, status: 'active' }],
          receipts: [{
            receipt_id: receiptId,
            bucket_id: bucketId,
            status: 'counted',
            unallocated_value_atoms: 0,
            counted_at_height: 12,
          }],
          allocations: [{
            allocation_id: allocationId,
            receipt_id: receiptId,
            bucket_id: bucketId,
            amount_atoms: 500000,
            released_atoms: 0,
            remaining_atoms: 500000,
            purpose: 'vault_bridge_supply',
            created_at_height: 13,
            retired_at_height: 0,
          }],
        },
      };
    }
    if (request.method === 'asset_info') {
      return {
        ok: true,
        result: {
          asset: {
            asset_id: request.params.asset_id,
            issuer: request.params.asset_id === navAssetId ? 'pfissuer' : 'pfsettlement',
            precision: request.params.asset_id === navAssetId ? 0 : 6,
          },
        },
      };
    }
    if (request.method === 'account_lines') {
      assert.deepStrictEqual(request.params, { account: wallet });
      return {
        ok: true,
        result: { account: wallet, lines: [] },
      };
    }
    if (request.method === 'market_ops_status') {
      assert.deepStrictEqual(request.params, { asset_id: navAssetId });
      return {
        ok: true,
        result: {
          asset_id: navAssetId,
          market_operations_status: 'active',
          envelope_epoch: 7,
          reserve_packet_fresh: true,
          supply_packet_fresh: true,
          current_mint_cap_atoms: 1000000,
        },
      };
    }
    throw new Error(`unexpected RPC method ${request.method}`);
  });

  assert.strictEqual(plan.ok, true);
  assert.strictEqual(plan.schema, 'postfiat-navswap-transparent-planner-inputs-v1');
  assert.strictEqual(plan.planner.direction, 'subscribe');
  assert.strictEqual(plan.planner.mint_amount_atoms, 250000);
  assert.strictEqual(plan.planner.settlement_amount_atoms, 250000);
  assert.deepStrictEqual(plan.actions.map(action => action.stage), ['nav_subscription_allocate']);
  assert.strictEqual(plan.actions[0].settlement_bucket_id, bucketId);
  assert.strictEqual(plan.actions[0].settlement_receipt_id, receiptId);
  assert.strictEqual(plan.actions[0].consume_supply_allocation_id, allocationId);
  assert.strictEqual(plan.actions[0].subscription_id, 'navsub-test-1');
  assert.strictEqual(plan.operator_completion.stage, 'nav_mint_at_nav');
  assert.strictEqual(
    plan.operator_completion.allocation_lookup.consumer_id,
    `nav_subscription:${navAssetId}:${wallet}:navsub-test-1`,
  );
  assert.strictEqual(
    plan.operator_completion.allocation_lookup.legacy_consumer_id,
    `nav_subscription:${navAssetId}:${wallet}`,
  );
  assert.strictEqual(plan.operator_completion.operation_template.amount, 250000);
  assert.strictEqual(plan.operator_completion.operation_template.settlement_amount_atoms, 250000);
  assert.deepStrictEqual(calls.sort(), ['asset_info', 'asset_info', 'market_ops_status', 'server_info', 'vault_bridge_status', 'vault_bridge_status']);
}

async function testTransparentPlannerInputsComputesRequiredSettlement() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const bucketId = 'b'.repeat(96);
  const receiptId = 'c'.repeat(96);
  const allocationId = 'a'.repeat(96);
  const plan = await planTransparentNavswapWalletActions({
    route: 'transparent_navswap',
    from_asset_id: settlementAssetId,
    to_asset_id: navAssetId,
    amount: '1',
    wallet_address: wallet,
  }, async (host, port, request) => {
    if (request.method === 'vault_bridge_status' && request.params.asset_id === settlementAssetId) {
      return {
        ok: true,
        result: {
          asset_id: settlementAssetId,
          valuation_unit: 'USDC',
          finalized_epoch: 7,
          bucket_count: 1,
          receipt_count: 1,
          allocation_count: 1,
          buckets: [{ bucket_id: bucketId, status: 'active' }],
          receipts: [{
            receipt_id: receiptId,
            bucket_id: bucketId,
            status: 'counted',
            unallocated_value_atoms: 0,
            counted_at_height: 12,
          }],
          allocations: [{
            allocation_id: allocationId,
            receipt_id: receiptId,
            bucket_id: bucketId,
            amount_atoms: 10_000_000,
            released_atoms: 0,
            remaining_atoms: 10_000_000,
            purpose: 'vault_bridge_supply',
            created_at_height: 13,
            retired_at_height: 0,
          }],
        },
      };
    }
    if (request.method === 'vault_bridge_status' && request.params.asset_id === navAssetId) {
      return {
        ok: true,
        result: {
          asset_id: navAssetId,
          issuer: 'pfissuer',
          valuation_unit: 'usd_1e8',
          finalized_epoch: 7,
          nav_per_unit: 508_236_346,
          finalized_reserve_packet_hash: 'e'.repeat(96),
        },
      };
    }
    if (request.method === 'asset_info') {
      return {
        ok: true,
        result: {
          asset: {
            asset_id: request.params.asset_id,
            issuer: request.params.asset_id === navAssetId ? 'pfissuer' : 'pfsettlement',
            precision: request.params.asset_id === navAssetId ? 0 : 6,
          },
        },
      };
    }
    if (request.method === 'account_lines') {
      return {
        ok: true,
        result: { account: wallet, lines: [] },
      };
    }
    if (request.method === 'market_ops_status') {
      return {
        ok: true,
        result: {
          asset_id: navAssetId,
          market_operations_status: 'active',
          envelope_epoch: 7,
          reserve_packet_fresh: true,
          supply_packet_fresh: true,
          current_mint_cap_atoms: 100,
        },
      };
    }
    throw new Error(`unexpected RPC method ${request.method}`);
  });

  assert.strictEqual(plan.ok, true);
  assert.strictEqual(plan.planner.mint_amount_atoms, 1);
  assert.strictEqual(plan.planner.settlement_amount_atoms, 5_082_364);
  assert.deepStrictEqual(plan.actions.map(action => action.stage), ['nav_subscription_allocate']);
  assert.strictEqual(plan.actions[0].settlement_amount_atoms, '5082364');
  assert.strictEqual(plan.actions[0].mint_amount_atoms, '1');
  assert.strictEqual(plan.actions[0].pricing_nav_epoch, '7');
  assert.strictEqual(plan.actions[0].primary_nav_price_atoms, '508236346');
  assert.strictEqual(plan.actions[0].pricing_reserve_packet_hash, 'e'.repeat(96));
  assert.strictEqual(plan.operator_completion.operation_template.amount, 1);
  assert.strictEqual(plan.operator_completion.operation_template.settlement_amount_atoms, 5_082_364);
}

async function testTransparentPlannerInputsAcceptsFractionalNavAmount() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const bucketId = 'b'.repeat(96);
  const receiptId = 'c'.repeat(96);
  const allocationId = 'a'.repeat(96);
  const plan = await planTransparentNavswapWalletActions({
    route: 'transparent_navswap',
    from_asset_id: settlementAssetId,
    to_asset_id: navAssetId,
    amount: '0.5',
    wallet_address: wallet,
  }, async (host, port, request) => {
    if (request.method === 'vault_bridge_status' && request.params.asset_id === settlementAssetId) {
      return {
        ok: true,
        result: {
          asset_id: settlementAssetId,
          valuation_unit: 'USDC',
          finalized_epoch: 7,
          bucket_count: 1,
          receipt_count: 1,
          allocation_count: 1,
          buckets: [{ bucket_id: bucketId, status: 'active' }],
          receipts: [{
            receipt_id: receiptId,
            bucket_id: bucketId,
            status: 'counted',
            unallocated_value_atoms: 0,
            counted_at_height: 12,
          }],
          allocations: [{
            allocation_id: allocationId,
            receipt_id: receiptId,
            bucket_id: bucketId,
            amount_atoms: 10_000_000,
            released_atoms: 0,
            remaining_atoms: 10_000_000,
            purpose: 'vault_bridge_supply',
            created_at_height: 13,
            retired_at_height: 0,
          }],
        },
      };
    }
    if (request.method === 'vault_bridge_status' && request.params.asset_id === navAssetId) {
      return {
        ok: true,
        result: {
          asset_id: navAssetId,
          issuer: 'pfissuer',
          valuation_unit: 'usd_1e8',
          finalized_epoch: 7,
          nav_per_unit: 508_236_346,
          finalized_reserve_packet_hash: 'e'.repeat(96),
        },
      };
    }
    if (request.method === 'asset_info') {
      return {
        ok: true,
        result: {
          asset: {
            asset_id: request.params.asset_id,
            issuer: request.params.asset_id === navAssetId ? 'pfissuer' : 'pfsettlement',
            precision: request.params.asset_id === navAssetId ? 6 : 6,
          },
        },
      };
    }
    if (request.method === 'account_lines') {
      return { ok: true, result: { account: wallet, lines: [] } };
    }
    if (request.method === 'market_ops_status') {
      return {
        ok: true,
        result: {
          asset_id: navAssetId,
          market_operations_status: 'active',
          envelope_epoch: 7,
          reserve_packet_fresh: true,
          supply_packet_fresh: true,
          current_mint_cap_atoms: 1_000_000,
        },
      };
    }
    throw new Error(`unexpected RPC method ${request.method}`);
  });

  assert.strictEqual(plan.ok, true);
  assert.strictEqual(plan.planner.nav_asset_precision, 6);
  assert.strictEqual(plan.planner.mint_amount_atoms, 500_000);
  assert.strictEqual(plan.planner.settlement_amount_atoms, 2_541_182);
  assert.strictEqual(plan.actions[0].settlement_amount_atoms, '2541182');
  assert.strictEqual(plan.actions[0].mint_amount_atoms, '500000');
  assert.strictEqual(plan.actions[0].pricing_nav_epoch, '7');
  assert.strictEqual(plan.actions[0].primary_nav_price_atoms, '508236346');
  assert.strictEqual(plan.actions[0].pricing_reserve_packet_hash, 'e'.repeat(96));
  assert.strictEqual(plan.operator_completion.operation_template.amount, 500_000);
  assert.strictEqual(plan.operator_completion.operation_template.settlement_amount_atoms, 2_541_182);
}

async function testTransparentPlannerInputsBuildsRedeemSettlementCompletion() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const bucketId = 'b'.repeat(96);
  const allocationId = 'a'.repeat(96);
  const plan = await planTransparentNavswapWalletActions({
    route: 'transparent_navswap',
    from_asset_id: navAssetId,
    to_asset_id: settlementAssetId,
    amount: '1',
    wallet_address: wallet,
    direction: 'redeem',
  }, async (_host, _port, request) => {
    if (request.method === 'vault_bridge_status' && request.params.asset_id === navAssetId) {
      return {
        ok: true,
        result: {
          asset_id: navAssetId,
          issuer: 'pfissuer',
          valuation_unit: 'usd_1e8',
          finalized_epoch: 7,
          nav_per_unit: 508_236_346,
          finalized_reserve_packet_hash: 'e'.repeat(96),
        },
      };
    }
    if (request.method === 'vault_bridge_status' && request.params.asset_id === settlementAssetId) {
      return {
        ok: true,
        result: {
          asset_id: settlementAssetId,
          valuation_unit: 'USDC',
          finalized_epoch: 7,
          bucket_count: 1,
          receipt_count: 1,
          allocation_count: 1,
          allocations: [{
            allocation_id: allocationId,
            bucket_id: bucketId,
            asset_id: settlementAssetId,
            amount_atoms: 6_000_000,
            remaining_atoms: 6_000_000,
            purpose: 'nav_subscription',
            consumer_id: `nav_subscription:${navAssetId}:${wallet}`,
            created_at_height: 40,
            retired_at_height: 41,
          }],
        },
      };
    }
    if (request.method === 'asset_info') {
      return {
        ok: true,
        result: {
          asset: {
            asset_id: request.params.asset_id,
            issuer: request.params.asset_id === navAssetId ? 'pfissuer' : 'pfsettlement',
            precision: request.params.asset_id === navAssetId ? 0 : 6,
          },
        },
      };
    }
    if (request.method === 'market_ops_status') {
      return {
        ok: true,
        result: {
          asset_id: navAssetId,
          market_operations_status: 'active',
          envelope_epoch: 7,
          reserve_packet_fresh: true,
          supply_packet_fresh: true,
        },
      };
    }
    throw new Error(`unexpected RPC method ${request.method}`);
  });

  assert.strictEqual(plan.ok, true);
  assert.strictEqual(plan.planner.direction, 'redeem');
  assert.strictEqual(plan.planner.amount_atoms, 1);
  assert.strictEqual(plan.planner.settlement_amount_atoms, 5_082_364);
  assert.deepStrictEqual(plan.actions.map(action => action.stage), ['nav_redeem_at_nav']);
  assert.strictEqual(plan.actions[0].redeem_amount_atoms, '1');
  assert.strictEqual(plan.operator_completion.stage, 'nav_redeem_settle');
  assert.strictEqual(plan.operator_completion.operation_template.operation, 'nav_redeem_settle');
  assert.strictEqual(plan.operator_completion.operation_template.settlement_asset_id, settlementAssetId);
  assert.strictEqual(plan.operator_completion.operation_template.settlement_amount_atoms, 5_082_364);
  assert.strictEqual(plan.operator_completion.operation_template.settlement_bucket_id, bucketId);
  assert.strictEqual(plan.operator_completion.operation_template.settlement_allocation_id, allocationId);
  assert.strictEqual(plan.operator_completion.allocation_lookup.owner, wallet);
  assert.strictEqual(plan.selected.settlement_allocation_id, allocationId);
  assert.strictEqual(plan.selected.backing_allocation_remaining_atoms, '6000000');
}

async function testTransparentPlannerRejectsUnbackedRedeemBeforeWalletAction() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const calls = [];
  const plan = await planTransparentNavswapWalletActions({
    route: 'transparent_navswap',
    from_asset_id: navAssetId,
    to_asset_id: settlementAssetId,
    amount: '1',
    wallet_address: wallet,
    direction: 'redeem',
  }, async (_host, _port, request) => {
    calls.push(request.method);
    if (request.method === 'vault_bridge_status') {
      if (request.params.asset_id === navAssetId) {
        return {
          ok: true,
          result: {
            asset_id: navAssetId,
            issuer: 'pfissuer',
            valuation_unit: 'usd_1e8',
            finalized_epoch: 7,
            nav_per_unit: 508_236_346,
            finalized_reserve_packet_hash: 'e'.repeat(96),
          },
        };
      }
      return {
        ok: true,
        result: {
          asset_id: settlementAssetId,
          valuation_unit: 'USDC',
          finalized_epoch: 7,
          allocation_count: 1,
          allocations: [{
            allocation_id: 'a'.repeat(96),
            bucket_id: 'b'.repeat(96),
            asset_id: settlementAssetId,
            amount_atoms: 5_000_000,
            remaining_atoms: 5_000_000,
            purpose: 'nav_subscription',
            consumer_id: `nav_subscription:${navAssetId}:${wallet}`,
            retired_at_height: 41,
          }],
        },
      };
    }
    if (request.method === 'asset_info') {
      return {
        ok: true,
        result: { asset: { asset_id: request.params.asset_id, precision: request.params.asset_id === navAssetId ? 0 : 6 } },
      };
    }
    if (request.method === 'market_ops_status') {
      return {
        ok: true,
        result: {
          asset_id: navAssetId,
          market_operations_status: 'active',
          envelope_epoch: 7,
          reserve_packet_fresh: true,
          supply_packet_fresh: true,
        },
      };
    }
    throw new Error(`unexpected RPC method ${request.method}`);
  });

  assert.strictEqual(plan.ok, false);
  assert.strictEqual(plan.code, 'transparent_navswap_redeem_backing_allocation_missing');
  assert.match(plan.message, /fully settle/);
  assert.deepStrictEqual(
    calls.sort(),
    ['asset_info', 'asset_info', 'market_ops_status', 'vault_bridge_status', 'vault_bridge_status'].sort(),
  );
}

async function testTransparentQuoteAutoPlanReturnsPreparedBatch() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const bucketId = 'b'.repeat(96);
  const receiptId = 'c'.repeat(96);
  const allocationId = 'a'.repeat(96);
  const quote = await executeNavswapQuote({
    route: 'transparent_navswap',
    from_asset: settlementAssetId,
    to_asset: navAssetId,
    amount: '250000',
    wallet_address: wallet,
    auto_plan: true,
  }, async (host, port, request) => {
    if (request.method === 'vault_bridge_status') {
      if (request.params.asset_id === navAssetId) {
        return {
          ok: true,
          result: {
            asset_id: navAssetId,
            issuer: 'pfissuer',
            valuation_unit: 'NAV_UNIT',
            finalized_epoch: 7,
            nav_per_unit: 1,
            finalized_reserve_packet_hash: 'e'.repeat(96),
          },
        };
      }
      return {
        ok: true,
        result: {
          asset_id: settlementAssetId,
          valuation_unit: 'SOURCE_UNIT',
          finalized_epoch: 7,
          bucket_count: 1,
          receipt_count: 1,
          allocation_count: 1,
          buckets: [{ bucket_id: bucketId, status: 'active' }],
          receipts: [{
            receipt_id: receiptId,
            bucket_id: bucketId,
            status: 'counted',
            unallocated_value_atoms: 500000,
            counted_at_height: 12,
          }],
          allocations: [{
            allocation_id: allocationId,
            receipt_id: receiptId,
            bucket_id: bucketId,
            amount_atoms: 500000,
            released_atoms: 0,
            remaining_atoms: 500000,
            purpose: 'vault_bridge_supply',
            created_at_height: 13,
            retired_at_height: 0,
          }],
        },
      };
    }
    if (request.method === 'market_ops_status') {
      return {
        ok: true,
        result: {
          asset_id: navAssetId,
          market_operations_status: 'active',
          envelope_epoch: 7,
          reserve_packet_fresh: true,
          supply_packet_fresh: true,
          current_mint_cap_atoms: 1000000,
        },
      };
    }
    if (request.method === 'asset_info') {
      return {
        ok: true,
        result: {
          asset: {
            asset_id: request.params.asset_id,
            issuer: request.params.asset_id === navAssetId ? 'pfissuer' : 'pfsettlement',
            precision: request.params.asset_id === navAssetId ? 0 : 6,
          },
        },
      };
    }
    if (request.method === 'account_lines') {
      return {
        ok: true,
        result: { account: wallet, lines: [] },
      };
    }
    throw new Error(`unexpected RPC method ${request.method}`);
  });

  assert.strictEqual(quote.ok, true);
  assert.strictEqual(quote.status, 'prepared_actions_ready');
  assert.strictEqual(quote.from_asset, settlementAssetId);
  assert.strictEqual(quote.to_asset, navAssetId);
  assert.strictEqual(quote.amount, '250000');
  assert.strictEqual(quote.input_amount_atoms, '250000');
  assert.strictEqual(quote.settlement_amount_atoms, '250000');
  assert.strictEqual(quote.redeem_amount_atoms, null);
  assert.strictEqual(quote.mint_amount_atoms, '250000');
  assert.strictEqual(quote.expected_output, '250000');
  assert.strictEqual(quote.expected_output_asset, navAssetId);
  assert.strictEqual(quote.expected_output_unavailable_reason, undefined);
  assert.strictEqual(quote.operator_completion.stage, 'nav_mint_at_nav');
  assert.strictEqual(quote.operator_completion.operation_template.amount, 250000);
  assert.strictEqual(quote.planner_inputs.ok, true);
  assert.strictEqual(quote.planner_inputs.quote_freshness.reserve_packet_fresh, true);
  assert.strictEqual(quote.planner_inputs.quote_freshness.supply_packet_fresh, true);
  assert.strictEqual(quote.planner_inputs.quote_freshness.nav_epoch, '7');
  assert.strictEqual(quote.planner_inputs.quote_freshness.reserve_packet_hash, 'e'.repeat(96));
  assert.match(quote.planner_inputs.quote_freshness.quote_expires_at_ms, /^[1-9][0-9]*$/);
  assert.deepStrictEqual(quote.prepared_action_batch.stages, ['nav_subscription_allocate']);
  assert.strictEqual(
    quote.prepared_action_batch.actions[0].user_intent.route_family,
    'primary_pftl_mint',
  );
  assert.strictEqual(
    quote.prepared_action_batch.actions[0].user_intent.route_trust_class,
    'CONTROLLED',
  );
  assert.strictEqual(
    quote.prepared_action_batch.actions[0].user_intent.supply_effect,
    'mints_new_native_navcoin_supply',
  );
  assert.strictEqual(
    quote.prepared_action_batch.actions[0].user_intent.pricing_source,
    'finalized_pre_inflow_nav_snapshot',
  );
  assert.strictEqual(
    quote.prepared_action_batch.actions[0].user_intent.uniswap_supply_effect,
    'not_uniswap_supply',
  );
  assert.strictEqual(
    quote.prepared_action_batch.actions[0].user_intent.mint_amount_atoms,
    '250000',
  );
  assert.strictEqual(
    quote.prepared_action_batch.actions[0].user_intent.pricing_nav_epoch,
    '7',
  );
  assert.strictEqual(
    quote.prepared_action_batch.actions[0].user_intent.primary_nav_price_atoms,
    '1',
  );
  assert.strictEqual(
    quote.prepared_action_batch.actions[0].user_intent.reserve_packet_fresh,
    true,
  );
  assert.strictEqual(
    quote.prepared_action_batch.actions[0].user_intent.supply_packet_fresh,
    true,
  );
  assert.strictEqual(
    quote.prepared_action_batch.actions[0].user_intent.quote_expires_at_ms,
    quote.planner_inputs.quote_freshness.quote_expires_at_ms,
  );
  assert.strictEqual(
    quote.prepared_action_batch.actions[0].operation.consume_supply_allocation_id,
    allocationId,
  );
}

async function testTransparentReadinessReportsSettlementFundingBlocker() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const bucketId = 'b'.repeat(96);
  const receiptId = 'c'.repeat(96);
  const allocationId = 'a'.repeat(96);
  const calls = [];

  await withEnvAsync({
    NAVSWAP_OPERATOR_ISSUER_KEY_FILE: '/tmp/test-navswap-issuer.key.json',
  }, async () => {
    const readiness = await executeTransparentNavswapReadiness({
      route: 'transparent_navswap',
      from_asset: settlementAssetId,
      to_asset: navAssetId,
      amount: '250000',
      wallet_address: wallet,
      auto_plan: true,
    }, async (_host, _port, request) => {
      calls.push(request.method);
      if (request.method === 'vault_bridge_status') {
        if (request.params.asset_id === navAssetId) {
          return {
            ok: true,
            result: {
              asset_id: navAssetId,
              issuer: 'pfissuer',
              valuation_unit: 'NAV_UNIT',
              finalized_epoch: 7,
              nav_per_unit: 1,
              finalized_reserve_packet_hash: 'e'.repeat(96),
            },
          };
        }
        return {
          ok: true,
          result: {
            asset_id: settlementAssetId,
            valuation_unit: 'SOURCE_UNIT',
            finalized_epoch: 7,
            bucket_count: 1,
            receipt_count: 1,
            allocation_count: 1,
            buckets: [{ bucket_id: bucketId, status: 'active' }],
            receipts: [{
              receipt_id: receiptId,
              bucket_id: bucketId,
              status: 'counted',
              counted_at_height: 12,
            }],
            allocations: [{
              allocation_id: allocationId,
              receipt_id: receiptId,
              bucket_id: bucketId,
              amount_atoms: 500000,
              released_atoms: 0,
              remaining_atoms: 500000,
              purpose: 'vault_bridge_supply',
              created_at_height: 13,
              retired_at_height: 0,
            }],
          },
        };
      }
      if (request.method === 'market_ops_status') {
        return {
          ok: true,
          result: {
            asset_id: navAssetId,
            market_operations_status: 'active',
            envelope_epoch: 7,
            reserve_packet_fresh: true,
            supply_packet_fresh: true,
            current_mint_cap_atoms: 1000000,
          },
        };
      }
      if (request.method === 'asset_info') {
        return {
          ok: true,
          result: {
            asset: {
              asset_id: request.params.asset_id,
              issuer: request.params.asset_id === navAssetId ? 'pfissuer' : 'pfsettlement',
              precision: request.params.asset_id === navAssetId ? 0 : 6,
            },
          },
        };
      }
      if (request.method === 'account') {
        return {
          ok: true,
          result: {
            address: wallet,
            balance: 1000000,
            sequence: 1,
          },
        };
      }
      if (request.method === 'account_lines') {
        return {
          ok: true,
          result: {
            account: wallet,
            lines: [{
              asset_id: settlementAssetId,
              issuer: 'pfsettlement',
              limit: 250000,
              balance: 100000,
              authorized: true,
              frozen: false,
            }],
          },
        };
      }
      if (request.method === 'account_assets') {
        return {
          ok: true,
          result: {
            account: wallet,
            assets: [{
              asset_id: settlementAssetId,
              balance: 100000,
            }],
          },
        };
      }
      if (request.method === 'asset_fee_quote') {
        const operation = JSON.parse(request.params.operation_json);
        assert.strictEqual(operation.operation, 'vault_bridge_nav_subscription_allocate');
        return {
          ok: true,
          result: {
            schema: 'postfiat-asset-fee-quote-v1',
            source: request.params.source,
            minimum_fee: 23,
            account_reserve: 10,
            sender_meets_reserve_after_fee: true,
            sender_meets_reserve_after_fee_and_reserve: true,
            operation,
          },
        };
      }
      throw new Error(`unexpected RPC method ${request.method}`);
    });

    assert.strictEqual(readiness.ok, true);
    assert.strictEqual(readiness.schema, 'postfiat-navswap-readiness-v1');
    assert.strictEqual(readiness.status, 'not_ready');
    assert.strictEqual(readiness.can_execute, false);
    assert.strictEqual(readiness.capabilities.can_run, true);
    assert.strictEqual(readiness.quote.ok, true);
    assert.strictEqual(readiness.required_settlement_atoms, '250000');
    assert.strictEqual(readiness.settlement_asset.balance_atoms, '100000');
    assert.strictEqual(readiness.settlement_asset.sufficient, false);
    assert.strictEqual(readiness.wallet_pft.balance_atoms, '1000000');
    assert.strictEqual(readiness.wallet_pft.sufficient_for_prepared_actions, true);
    assert.strictEqual(readiness.wallet_pft.fee_preflight.total_minimum_fee_atoms, '23');
    assert.deepStrictEqual(readiness.prepared_stages, ['nav_subscription_allocate']);
    assert.ok(readiness.next_steps.includes('fund the wallet with the required settlement asset'));
  });

  assert.ok(calls.includes('account_assets'));
  assert.ok(calls.includes('account'));
  assert.ok(calls.includes('asset_fee_quote'));
}

async function testTransparentReadinessBlocksLowPftFeeReserve() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const bucketId = 'b'.repeat(96);
  const receiptId = 'c'.repeat(96);
  const allocationId = 'a'.repeat(96);

  await withEnvAsync({
    NAVSWAP_OPERATOR_ISSUER_KEY_FILE: '/tmp/test-navswap-issuer.key.json',
  }, async () => {
    const readiness = await executeTransparentNavswapReadiness({
      route: 'transparent_navswap',
      from_asset: settlementAssetId,
      to_asset: navAssetId,
      amount: '250000',
      wallet_address: wallet,
      auto_plan: true,
    }, async (_host, _port, request) => {
      if (request.method === 'vault_bridge_status') {
        if (request.params.asset_id === navAssetId) {
          return {
            ok: true,
            result: {
              asset_id: navAssetId,
              issuer: 'pfissuer',
              valuation_unit: 'NAV_UNIT',
              finalized_epoch: 7,
              nav_per_unit: 1,
              finalized_reserve_packet_hash: 'e'.repeat(96),
            },
          };
        }
        return {
          ok: true,
          result: {
            asset_id: settlementAssetId,
            valuation_unit: 'SOURCE_UNIT',
            finalized_epoch: 7,
            bucket_count: 1,
            receipt_count: 1,
            allocation_count: 1,
            buckets: [{ bucket_id: bucketId, status: 'active' }],
            receipts: [{
              receipt_id: receiptId,
              bucket_id: bucketId,
              status: 'counted',
              counted_at_height: 12,
            }],
            allocations: [{
              allocation_id: allocationId,
              receipt_id: receiptId,
              bucket_id: bucketId,
              amount_atoms: 500000,
              released_atoms: 0,
              remaining_atoms: 500000,
              purpose: 'vault_bridge_supply',
              created_at_height: 13,
              retired_at_height: 0,
            }],
          },
        };
      }
      if (request.method === 'market_ops_status') {
        return {
          ok: true,
          result: {
            asset_id: navAssetId,
            market_operations_status: 'active',
            envelope_epoch: 7,
            reserve_packet_fresh: true,
            supply_packet_fresh: true,
            current_mint_cap_atoms: 1000000,
          },
        };
      }
      if (request.method === 'asset_info') {
        return {
          ok: true,
          result: {
            asset: {
              asset_id: request.params.asset_id,
              issuer: request.params.asset_id === navAssetId ? 'pfissuer' : 'pfsettlement',
              precision: request.params.asset_id === navAssetId ? 0 : 6,
            },
          },
        };
      }
      if (request.method === 'account') {
        return {
          ok: true,
          result: {
            address: wallet,
            balance: 15,
            sequence: 1,
          },
        };
      }
      if (request.method === 'account_lines') {
        return {
          ok: true,
          result: {
            account: wallet,
            lines: [{
              asset_id: settlementAssetId,
              issuer: 'pfsettlement',
              limit: 1000000,
              balance: 250000,
              authorized: true,
              frozen: false,
            }],
          },
        };
      }
      if (request.method === 'account_assets') {
        return {
          ok: true,
          result: {
            account: wallet,
            assets: [{
              asset_id: settlementAssetId,
              balance: 250000,
            }],
          },
        };
      }
      if (request.method === 'asset_fee_quote') {
        const operation = JSON.parse(request.params.operation_json);
        return {
          ok: true,
          result: {
            schema: 'postfiat-asset-fee-quote-v1',
            source: request.params.source,
            minimum_fee: 23,
            account_reserve: 10,
            sender_meets_reserve_after_fee: operation.operation !== 'vault_bridge_nav_subscription_allocate',
            sender_meets_reserve_after_fee_and_reserve: operation.operation !== 'vault_bridge_nav_subscription_allocate',
            operation,
          },
        };
      }
      throw new Error(`unexpected RPC method ${request.method}`);
    });

    assert.strictEqual(readiness.ok, true);
    assert.strictEqual(readiness.status, 'not_ready');
    assert.strictEqual(readiness.can_execute, false);
    assert.strictEqual(readiness.settlement_asset.sufficient, true);
    assert.strictEqual(readiness.wallet_pft.balance_atoms, '15');
    assert.strictEqual(readiness.wallet_pft.sufficient_for_prepared_actions, false);
    assert.strictEqual(readiness.wallet_pft.fee_preflight.ok, false);
    assert.strictEqual(readiness.wallet_pft.fee_preflight.failed_action.stage, 'nav_subscription_allocate');
    assert.ok(readiness.next_steps.includes('fund the wallet with PFT for NAVSwap fees/reserves'));
  });
}

async function testDevnetPfusdcFundingSubmitsShortfallWithoutTrustlineGate() {
  clearNavswapDevnetFundingUsageForTest();
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = navswapCapabilities().assets.pfUSDC.asset_id;
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const bucketId = 'b'.repeat(96);
  const receiptId = 'c'.repeat(96);
  const allocationId = 'a'.repeat(96);
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-navswap-funding-test-'));
  const keyFile = path.join(root, 'pfusdc.key.json');
  const signer = path.join(root, 'fake-postfiat-node.js');
  fs.writeFileSync(keyFile, JSON.stringify({ address: 'pfsettlement' }), { mode: 0o600 });
  fs.writeFileSync(signer, `#!/usr/bin/env node
const fs = require('fs');
const args = process.argv.slice(2);
const quoteFile = args[args.indexOf('--quote-file') + 1];
const quote = JSON.parse(fs.readFileSync(quoteFile, 'utf8'));
process.stdout.write(JSON.stringify({
  schema: 'postfiat-signed-asset-transaction-v1',
  unsigned: { source: quote.source, operation: quote.operation },
  signature_hex: 'aa'
}));
`, { mode: 0o700 });
  fs.chmodSync(signer, 0o700);
  let submitCount = 0;

  const rpcStub = ({ trustline = true } = {}) => async (_host, _port, request) => {
    if (request.method === 'vault_bridge_status') {
      if (request.params.asset_id === navAssetId) {
        return {
          ok: true,
          result: {
            asset_id: navAssetId,
            issuer: 'pfissuer',
            valuation_unit: 'NAV_UNIT',
            finalized_epoch: 7,
            nav_per_unit: 1,
            finalized_reserve_packet_hash: 'e'.repeat(96),
          },
        };
      }
      return {
        ok: true,
        result: {
          asset_id: settlementAssetId,
          valuation_unit: 'SOURCE_UNIT',
          finalized_epoch: 7,
          bucket_count: 1,
          receipt_count: 1,
          allocation_count: 1,
          buckets: [{ bucket_id: bucketId, status: 'active' }],
          receipts: [{
            receipt_id: receiptId,
            bucket_id: bucketId,
            status: 'counted',
            counted_at_height: 12,
          }],
          allocations: [{
            allocation_id: allocationId,
            receipt_id: receiptId,
            bucket_id: bucketId,
            amount_atoms: 500000,
            released_atoms: 0,
            remaining_atoms: 500000,
            purpose: 'vault_bridge_supply',
            created_at_height: 13,
            retired_at_height: 0,
          }],
        },
      };
    }
    if (request.method === 'market_ops_status') {
      return {
        ok: true,
        result: {
          asset_id: navAssetId,
          market_operations_status: 'active',
          envelope_epoch: 7,
          reserve_packet_fresh: true,
          supply_packet_fresh: true,
          current_mint_cap_atoms: 1000000,
        },
      };
    }
    if (request.method === 'asset_info') {
      return {
        ok: true,
        result: {
          asset: {
            asset_id: request.params.asset_id,
            issuer: request.params.asset_id === navAssetId ? 'pfissuer' : 'pfsettlement',
            precision: request.params.asset_id === navAssetId ? 0 : 6,
          },
        },
      };
    }
    if (request.method === 'account') {
      return {
        ok: true,
        result: {
          address: wallet,
          balance: 1000000,
          sequence: 1,
        },
      };
    }
    if (request.method === 'account_lines') {
      return {
        ok: true,
        result: {
          account: wallet,
          lines: trustline ? [{
            asset_id: settlementAssetId,
            issuer: 'pfsettlement',
            limit: 1000000,
            balance: 100000,
            authorized: true,
            frozen: false,
          }] : [],
        },
      };
    }
    if (request.method === 'account_assets') {
      return {
        ok: true,
        result: {
          account: wallet,
          assets: [{
            asset_id: settlementAssetId,
            balance: 100000,
          }],
        },
      };
    }
    if (request.method === 'asset_fee_quote') {
      const operation = JSON.parse(request.params.operation_json);
      if (operation.operation !== 'issued_payment') {
        assert.strictEqual(operation.operation, 'vault_bridge_nav_subscription_allocate');
        return {
          ok: true,
          result: {
            schema: 'postfiat-asset-fee-quote-v1',
            source: request.params.source,
            minimum_fee: 23,
            account_reserve: 10,
            sender_meets_reserve_after_fee: true,
            sender_meets_reserve_after_fee_and_reserve: true,
            operation,
          },
        };
      }
      assert.strictEqual(operation.operation, 'issued_payment');
      assert.strictEqual(operation.from, 'pfsettlement');
      assert.strictEqual(operation.to, wallet);
      assert.strictEqual(operation.asset_id, settlementAssetId);
      assert.strictEqual(operation.amount, 150000);
      return {
        ok: true,
        result: {
          schema: 'postfiat-asset-fee-quote-v1',
          source: 'pfsettlement',
          minimum_fee: 1,
          sequence: 9,
          chain_id: 'test-chain',
          genesis_hash: 'genesis',
          protocol_version: 1,
          sender_meets_reserve_after_fee: true,
          operation,
        },
      };
    }
    if (request.method === 'mempool_submit_signed_asset_transaction_finality') {
      submitCount += 1;
      const signed = JSON.parse(request.params.signed_asset_transaction_json);
      assert.strictEqual(signed.unsigned.operation.amount, 150000);
      return {
        ok: true,
        result: { tx_id: 'pfusdc-funding-tx', round_ok: true },
      };
    }
    throw new Error(`unexpected RPC method ${request.method}`);
  };

  try {
    await withEnvAsync({
      NAVSWAP_ENABLE_DEVNET_PFUSDC_FUNDING: 'true',
      NAVSWAP_PFUSDC_ISSUER_KEY_FILE: keyFile,
      NAVSWAP_OPERATOR_NODE_BIN: signer,
      NAVSWAP_DEVNET_PFUSDC_FUNDING_MAX_ATOMS: '200000',
      NAVSWAP_OPERATOR_ISSUER_KEY_FILE: '/tmp/test-navswap-issuer.key.json',
    }, async () => {
      const noTrustlineReadiness = await executeTransparentNavswapReadiness({
        route: 'transparent_navswap',
        from_asset: 'pfUSDC',
        to_asset: navAssetId,
        amount: '250000',
        wallet_address: wallet,
      }, rpcStub({ trustline: false }));
      assert.strictEqual(noTrustlineReadiness.ok, true);
      assert.strictEqual(noTrustlineReadiness.settlement_asset.trustline_usable, undefined);
      assert.strictEqual(noTrustlineReadiness.settlement_asset.trustline_fee_preflight, undefined);
      assert.strictEqual(noTrustlineReadiness.funding.available, true);

      const funded = await executeNavswapDevnetPfusdcFunding({
        route: 'transparent_navswap',
        from_asset: 'pfUSDC',
        to_asset: navAssetId,
        amount: '250000',
        amount_atoms: '150000',
        wallet_address: wallet,
      }, rpcStub({ trustline: false }));
      assert.strictEqual(funded.ok, true);
      assert.strictEqual(funded.schema, 'postfiat-navswap-devnet-funding-v1');
      assert.strictEqual(funded.status, 'funding_submitted');
      assert.strictEqual(funded.amount_atoms, '150000');
      assert.strictEqual(funded.tx_id, 'pfusdc-funding-tx');
      assert.strictEqual(funded.readiness.funding.available, true);
      assert.strictEqual(funded.recipient_window.used_atoms, '150000');
      assert.strictEqual(funded.recipient_window.remaining_atoms, '50000');

      const capped = await executeNavswapDevnetPfusdcFunding({
        route: 'transparent_navswap',
        from_asset: 'pfUSDC',
        to_asset: navAssetId,
        amount: '250000',
        wallet_address: wallet,
      }, rpcStub({ trustline: false }));
      assert.strictEqual(capped.ok, false);
      assert.strictEqual(capped.code, 'devnet_pfusdc_funding_recipient_window_exceeded');
      assert.strictEqual(capped.recipient_window.remaining_atoms, '50000');
      assert.strictEqual(submitCount, 1);
    });
  } finally {
    clearNavswapDevnetFundingUsageForTest();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTransparentPlannerInputsFailWithoutSettlementSource() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const plan = await planTransparentNavswapWalletActions({
    route: 'transparent_navswap',
    from_asset: settlementAssetId,
    to_asset: navAssetId,
    amount: '250000',
    wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
  }, async (host, port, request) => {
    if (request.method === 'vault_bridge_status') {
      if (request.params.asset_id === navAssetId) {
        return {
          ok: true,
          result: {
            asset_id: navAssetId,
            issuer: 'pfissuer',
            valuation_unit: 'NAV_UNIT',
            finalized_epoch: 7,
            nav_per_unit: 1,
            finalized_reserve_packet_hash: 'e'.repeat(96),
          },
        };
      }
      return {
        ok: true,
        result: {
          asset_id: settlementAssetId,
          valuation_unit: 'SOURCE_UNIT',
          bucket_count: 1,
          receipt_count: 1,
          allocation_count: 0,
          buckets: [{ bucket_id: 'b'.repeat(96), status: 'active' }],
          receipts: [{
            receipt_id: 'c'.repeat(96),
            bucket_id: 'b'.repeat(96),
            status: 'counted',
            unallocated_value_atoms: 500000,
            counted_at_height: 12,
          }],
          allocations: [],
        },
      };
    }
    if (request.method === 'asset_info') {
      return {
        ok: true,
        result: {
          asset: {
            asset_id: request.params.asset_id,
            issuer: request.params.asset_id === navAssetId ? 'pfissuer' : 'pfsettlement',
            precision: request.params.asset_id === navAssetId ? 0 : 6,
          },
        },
      };
    }
    if (request.method === 'account_lines') {
      return {
        ok: true,
        result: { account: request.params.account, lines: [] },
      };
    }
    if (request.method === 'market_ops_status') {
      return {
        ok: true,
        result: {
          asset_id: navAssetId,
          market_operations_status: 'active',
          envelope_epoch: 7,
          reserve_packet_fresh: true,
          supply_packet_fresh: true,
          current_mint_cap_atoms: 1000000,
        },
      };
    }
    throw new Error(`unexpected RPC method ${request.method}`);
  });

  assert.strictEqual(plan.ok, false);
  assert.strictEqual(plan.code, 'transparent_navswap_no_settlement_source');
  assert.strictEqual(plan.settlement_status.allocation_count, 0);
}

async function testTransparentPlannerInputsRejectsStaleSettlementSource() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const plan = await planTransparentNavswapWalletActions({
    route: 'transparent_navswap',
    from_asset: settlementAssetId,
    to_asset: navAssetId,
    amount: '250000',
    wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
    current_height: 118,
    settlement_max_snapshot_age_blocks: 100,
    settlement_receipt_safety_blocks: 5,
  }, async (host, port, request) => {
    if (request.method === 'vault_bridge_status') {
      if (request.params.asset_id === navAssetId) {
        return {
          ok: true,
          result: {
            asset_id: navAssetId,
            issuer: 'pfissuer',
            valuation_unit: 'NAV_UNIT',
            finalized_epoch: 7,
            nav_per_unit: 1,
            finalized_reserve_packet_hash: 'e'.repeat(96),
          },
        };
      }
      return {
        ok: true,
        result: {
          asset_id: settlementAssetId,
          valuation_unit: 'SOURCE_UNIT',
          bucket_count: 1,
          receipt_count: 1,
          allocation_count: 1,
          buckets: [{ bucket_id: 'b'.repeat(96), status: 'active' }],
          receipts: [{
            receipt_id: 'c'.repeat(96),
            bucket_id: 'b'.repeat(96),
            status: 'counted',
            created_at_height: 10,
            counted_at_height: 11,
          }],
          allocations: [{
            allocation_id: 'a'.repeat(96),
            receipt_id: 'c'.repeat(96),
            bucket_id: 'b'.repeat(96),
            amount_atoms: 500000,
            released_atoms: 0,
            remaining_atoms: 500000,
            purpose: 'vault_bridge_supply',
            created_at_height: 11,
            retired_at_height: 0,
          }],
        },
      };
    }
    if (request.method === 'asset_info') {
      return {
        ok: true,
        result: {
          asset: {
            asset_id: request.params.asset_id,
            issuer: request.params.asset_id === navAssetId ? 'pfissuer' : 'pfsettlement',
            precision: request.params.asset_id === navAssetId ? 0 : 6,
          },
        },
      };
    }
    if (request.method === 'market_ops_status') {
      return {
        ok: true,
        result: {
          asset_id: navAssetId,
          market_operations_status: 'active',
          envelope_epoch: 7,
          reserve_packet_fresh: true,
          supply_packet_fresh: true,
          current_mint_cap_atoms: 1000000,
        },
      };
    }
    throw new Error(`unexpected RPC method ${request.method}`);
  });

  assert.strictEqual(plan.ok, false);
  assert.strictEqual(plan.code, 'transparent_navswap_no_fresh_settlement_source');
  assert.strictEqual(plan.settlement_status.stale_candidate_count, 1);
  assert.strictEqual(plan.settlement_status.current_height, 118);
  assert.strictEqual(plan.settlement_status.freshest_rejected_receipt.freshness.usable_until_height, 110);
  assert.match(plan.message, /Bridge fresh pfUSDC/);
}

async function testTransparentPlannerInputsReportsMissingMarketOpsEnvelope() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const plan = await planTransparentNavswapWalletActions({
    route: 'transparent_navswap',
    from_asset: settlementAssetId,
    to_asset: navAssetId,
    amount: '250000',
    wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
  }, async (host, port, request) => {
    if (request.method === 'vault_bridge_status') {
      if (request.params.asset_id === navAssetId) {
        return {
          ok: true,
          result: {
            asset_id: navAssetId,
            issuer: 'pfissuer',
            valuation_unit: 'NAV_UNIT',
            finalized_epoch: 7,
            nav_per_unit: 1,
            finalized_reserve_packet_hash: 'e'.repeat(96),
          },
        };
      }
      return {
        ok: true,
        result: {
          asset_id: settlementAssetId,
          valuation_unit: 'SOURCE_UNIT',
          bucket_count: 0,
          receipt_count: 0,
          allocation_count: 0,
          buckets: [],
          receipts: [],
          allocations: [],
        },
      };
    }
    if (request.method === 'asset_info') {
      return {
        ok: true,
        result: { asset: { asset_id: settlementAssetId, issuer: 'pfsettlement', precision: 6 } },
      };
    }
    if (request.method === 'account_lines') {
      return {
        ok: true,
        result: { account: request.params.account, lines: [] },
      };
    }
    if (request.method === 'market_ops_status') {
      return {
        ok: false,
        error: {
          code: 'rpc_error',
          message: `rpc market_ops_status failed: missing finalized market ops envelope for asset \`${navAssetId}\``,
        },
      };
    }
    throw new Error(`unexpected RPC method ${request.method}`);
  });

  assert.strictEqual(plan.ok, false);
  assert.strictEqual(plan.code, 'transparent_navswap_market_ops_envelope_missing');
  assert.match(plan.message, /missing finalized market ops envelope/);
  assert.strictEqual(plan.rpc_error.code, 'rpc_error');
}

function transparentCompletionFixture() {
  const navAssetId = 'd'.repeat(96);
  const settlementAssetId = '8'.repeat(96);
  const wallet = 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468';
  const bucketId = 'b'.repeat(96);
  const receiptId = 'c'.repeat(96);
  const supplyAllocationId = 'a'.repeat(96);
  const subscriptionAllocationId = 'f'.repeat(96);
  const reservePacketHash = 'e'.repeat(96);
  const settlementAmount = 5_082_364;
  const mintAmount = 1;
  const allocateOperation = {
    operation: 'vault_bridge_nav_subscription_allocate',
    operator: 'pfissuer',
    nav_asset_id: navAssetId,
    settlement_asset_id: settlementAssetId,
    settlement_bucket_id: bucketId,
    settlement_receipt_id: receiptId,
    settlement_amount_atoms: settlementAmount,
    consume_supply_owner: wallet,
    consume_supply_allocation_id: supplyAllocationId,
    nav_recipient: wallet,
  };
  const quote = {
    ok: true,
    schema: 'postfiat-navswap-quote-v1',
    route: 'transparent_navswap',
    status: 'prepared_actions_ready',
    operator_completion: {
      stage: 'nav_mint_at_nav',
      requires_operator_signature: true,
      status: 'awaiting_wallet_allocation',
      operation_template: {
        operation: 'nav_mint_at_nav',
        issuer: 'pfissuer',
        to: wallet,
        asset_id: navAssetId,
        amount: mintAmount,
        epoch: 7,
        reserve_packet_hash: reservePacketHash,
        settlement_asset_id: settlementAssetId,
        settlement_bucket_id: bucketId,
        settlement_allocation_id: null,
        settlement_amount_atoms: settlementAmount,
      },
      allocation_lookup: {
        purpose: 'nav_subscription',
        consumer_id: `nav_subscription:${navAssetId}:${wallet}`,
        fallback_consumer_id: `nav_subscription:${navAssetId}`,
        settlement_bucket_id: bucketId,
        settlement_receipt_id: receiptId,
        settlement_amount_atoms: String(settlementAmount),
      },
    },
    prepared_action_batch: {
      actions: [{
        stage: 'trust_set',
        operation: {
          operation: 'trust_set',
          account: wallet,
          issuer: 'pfissuer',
          asset_id: navAssetId,
          limit: mintAmount,
        },
      }, {
        stage: 'nav_subscription_allocate',
        operation: allocateOperation,
      }],
    },
  };
  const walletResult = {
    ok: true,
    count: 2,
    submissions: [{
      txId: 'trust-tx',
      receipt: { accepted: true },
      navswap_action: quote.prepared_action_batch.actions[0],
    }, {
      txId: 'allocate-tx',
      receipt: { accepted: true },
      navswap_action: {
        stage: 'nav_subscription_allocate',
        operation: allocateOperation,
      },
    }],
  };
  const allocation = {
    allocation_id: subscriptionAllocationId,
    receipt_id: receiptId,
    bucket_id: bucketId,
    amount_atoms: settlementAmount,
    released_atoms: 0,
    remaining_atoms: settlementAmount,
    purpose: 'nav_subscription',
    consumer_id: `nav_subscription:${navAssetId}:${wallet}`,
    created_at_height: 42,
    retired_at_height: 0,
  };
  return {
    navAssetId,
    settlementAssetId,
    wallet,
    quote,
    walletResult,
    allocation,
    settlementAmount,
    subscriptionAllocationId,
  };
}

async function testTransparentCompletionWaitsForOperatorKeyAfterAllocationVerified() {
  const fixture = transparentCompletionFixture();
  const calls = [];
  await withEnvAsync({
    NAVSWAP_OPERATOR_ISSUER_KEY_FILE: undefined,
    NAVSWAP_ISSUER_KEY_FILE: undefined,
    ISSUER_KEY_FILE: undefined,
  }, async () => {
    const run = await executeTransparentNavswapRun({
      route: 'transparent_navswap',
      wallet_address: fixture.wallet,
      quote: fixture.quote,
      wallet_action_result: fixture.walletResult,
    }, async (host, port, request) => {
      calls.push(request.method);
      assert.strictEqual(request.method, 'vault_bridge_status');
      assert.deepStrictEqual(request.params, { asset_id: fixture.settlementAssetId });
      return {
        ok: true,
        result: {
          asset_id: fixture.settlementAssetId,
          allocation_count: 1,
          allocations: [fixture.allocation],
        },
      };
    });

    assert.strictEqual(run.ok, false);
    assert.strictEqual(run.status, 'awaiting_operator_signature');
    assert.strictEqual(run.code, 'navswap_operator_key_not_configured');
    assert.strictEqual(calls.filter(method => method === 'asset_fee_quote').length, 0);
    const events = navswapRunEvents(run.run_id);
    assert(events.events.some(event => event.type === 'wallet_batch_verified'));
    assert(events.events.some(event => event.type === 'subscription_allocation_verified'));
    const receipts = navswapRunReceipts(run.run_id);
    assert.strictEqual(receipts.receipts[0].type, 'transparent_navswap_operator_completion');
    assert.strictEqual(
      receipts.receipts[0].payload.allocation.allocation_id,
      fixture.subscriptionAllocationId,
    );
    assert.strictEqual(receipts.receipts[0].payload.receipt_verification.schema, 'postfiat-navswap-receipt-verification-v1');
    assert.strictEqual(receipts.receipts[0].payload.receipt_verification.ok, false);
    assert.strictEqual(receipts.receipts[0].payload.receipt_verification.status, 'awaiting_operator_signature');
    assert.strictEqual(receipts.receipts[0].payload.receipt_verification.allocation_id, fixture.subscriptionAllocationId);
    assert.strictEqual(receipts.receipts[0].payload.receipt_verification.checks.wallet_submission_matches_prepared, true);
    assert.strictEqual(receipts.receipts[0].payload.receipt_verification.checks.live_allocation_matches_quote, true);
  });
}

async function testTransparentCompletionFailsUntilAllocationVisible() {
  const fixture = transparentCompletionFixture();
  const run = await executeTransparentNavswapRun({
    route: 'transparent_navswap',
    wallet_address: fixture.wallet,
    quote: fixture.quote,
    wallet_action_result: fixture.walletResult,
  }, async (host, port, request) => {
    assert.strictEqual(request.method, 'vault_bridge_status');
    return {
      ok: true,
      result: {
        asset_id: fixture.settlementAssetId,
        allocation_count: 0,
        allocations: [],
      },
    };
  });

  assert.strictEqual(run.ok, false);
  assert.strictEqual(run.status, 'failed');
  assert.strictEqual(run.code, 'transparent_navswap_subscription_allocation_missing');
  const receipts = navswapRunReceipts(run.run_id);
  assert.strictEqual(receipts.receipts[0].payload.code, 'transparent_navswap_subscription_allocation_missing');
}

async function testTransparentCompletionSignsAndSubmitsConfiguredOperatorMint() {
  const fixture = transparentCompletionFixture();
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-navswap-operator-test-'));
  const keyFile = path.join(root, 'issuer.key.json');
  const signer = path.join(root, 'fake-postfiat-node.js');
  fs.writeFileSync(keyFile, '{}', { mode: 0o600 });
  fs.writeFileSync(signer, `#!/usr/bin/env node
const fs = require('fs');
const args = process.argv.slice(2);
const quoteFile = args[args.indexOf('--quote-file') + 1];
const quote = JSON.parse(fs.readFileSync(quoteFile, 'utf8'));
process.stdout.write(JSON.stringify({
  schema: 'postfiat-signed-asset-transaction-v1',
  unsigned: { source: quote.source, operation: quote.operation },
  signature_hex: 'aa'
}));
`, { mode: 0o700 });
  fs.chmodSync(signer, 0o700);

  const methods = [];
  let submitted = null;
  try {
    await withEnvAsync({
      NAVSWAP_OPERATOR_ISSUER_KEY_FILE: keyFile,
      NAVSWAP_OPERATOR_NODE_BIN: signer,
      NAVSWAP_ISSUER_KEY_FILE: undefined,
      ISSUER_KEY_FILE: undefined,
    }, async () => {
      const start = await executeTransparentNavswapRun({
        route: 'transparent_navswap',
        wallet_address: fixture.wallet,
        quote: fixture.quote,
        wallet_action_result: fixture.walletResult,
        async: true,
      }, async (host, port, request) => {
        methods.push(request.method);
        if (request.method === 'vault_bridge_status') {
          return {
            ok: true,
            result: {
              asset_id: fixture.settlementAssetId,
              allocation_count: 1,
              allocations: [fixture.allocation],
            },
          };
        }
        if (request.method === 'asset_fee_quote') {
          const operation = JSON.parse(request.params.operation_json);
          assert.strictEqual(operation.operation, 'nav_mint_at_nav');
          assert.strictEqual(operation.settlement_allocation_id, fixture.subscriptionAllocationId);
          return {
            ok: true,
            result: {
              schema: 'postfiat-asset-fee-quote-v1',
              source: 'pfissuer',
              minimum_fee: 1,
              sequence: 9,
              chain_id: 'test-chain',
              genesis_hash: 'genesis',
              protocol_version: 1,
              sender_meets_reserve_after_fee: true,
              operation,
            },
          };
        }
        if (request.method === 'mempool_submit_signed_asset_transaction_finality') {
          submitted = JSON.parse(request.params.signed_asset_transaction_json);
          return {
            ok: true,
            result: { tx_id: 'operator-mint-tx', round_ok: true },
          };
        }
        throw new Error(`unexpected RPC method ${request.method}`);
      });

      assert.strictEqual(start.ok, true);
      assert.strictEqual(start.status, 'running');
      assert.strictEqual(navswapRunPublic(start.run_id).terminal, false);
      const finalStatus = await waitForNavswapRun(
        start.run_id,
        status => status.status === 'operator_mint_submitted',
        1000,
      );
      assert.strictEqual(finalStatus.ok, true);
      assert.strictEqual(finalStatus.terminal, true);
      assert.deepStrictEqual(methods, [
        'vault_bridge_status',
        'asset_fee_quote',
        'mempool_submit_signed_asset_transaction_finality',
      ]);
      assert.strictEqual(submitted.unsigned.operation.operation, 'nav_mint_at_nav');
      assert.strictEqual(finalStatus.result.operator_completion.tx_id, 'operator-mint-tx');
      assert.strictEqual(finalStatus.result.receipt_verification.schema, 'postfiat-navswap-receipt-verification-v1');
      assert.strictEqual(finalStatus.result.receipt_verification.ok, true);
      assert.strictEqual(finalStatus.result.receipt_verification.nav_subscription_tx_id, 'allocate-tx');
      assert.strictEqual(finalStatus.result.receipt_verification.operator_tx_id, 'operator-mint-tx');
      assert.strictEqual(finalStatus.result.receipt_verification.allocation_id, fixture.subscriptionAllocationId);
      assert.strictEqual(finalStatus.result.receipt_verification.checks.operator_operation_matches_live_allocation, true);
      assert.strictEqual(finalStatus.result.receipt_verification.checks.operator_submit_accepted, true);
      const receipts = navswapRunReceipts(start.run_id);
      assert.strictEqual(receipts.receipts[0].type, 'transparent_navswap_operator_completion');
      assert.strictEqual(receipts.receipts[0].payload.receipt_verification.ok, true);
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function testTransparentCompletionSignsAndSubmitsConfiguredOperatorRedeemSettle() {
  const fixture = transparentCompletionFixture();
  const redeemOperation = {
    operation: 'nav_redeem_at_nav',
    owner: fixture.wallet,
    issuer: 'pfissuer',
    asset_id: fixture.navAssetId,
    amount: 1,
    epoch: 7,
    reserve_packet_hash: 'e'.repeat(96),
  };
  const quote = {
    ok: true,
    schema: 'postfiat-navswap-quote-v1',
    route: 'transparent_navswap',
    direction: 'redeem',
    status: 'prepared_actions_ready',
    redeem_amount_atoms: '1',
    settlement_amount_atoms: String(fixture.settlementAmount),
    operator_completion: {
      stage: 'nav_redeem_settle',
      requires_operator_signature: true,
      status: 'awaiting_wallet_redeem',
      operation_template: {
        operation: 'nav_redeem_settle',
        issuer: 'pfissuer',
        asset_id: fixture.navAssetId,
        redemption_id: null,
        settlement_receipt_hash: null,
        settlement_asset_id: fixture.settlementAssetId,
        settlement_bucket_id: null,
        settlement_allocation_id: null,
        settlement_amount_atoms: fixture.settlementAmount,
      },
      allocation_lookup: {
        purpose: 'nav_subscription',
        nav_asset_id: fixture.navAssetId,
        settlement_asset_id: fixture.settlementAssetId,
        owner: fixture.wallet,
        settlement_amount_atoms: String(fixture.settlementAmount),
      },
    },
    prepared_action_batch: {
      actions: [{
        stage: 'nav_redeem_at_nav',
        operation: redeemOperation,
      }],
    },
  };
  const walletResult = {
    ok: true,
    count: 1,
    submissions: [{
      txId: 'redeem-tx',
      receipt: { accepted: true },
      quote: {
        chain_id: 'test-chain',
        sequence: 14,
      },
      navswap_action: {
        stage: 'nav_redeem_at_nav',
        operation: redeemOperation,
      },
    }],
  };
  const retiredAllocation = {
    ...fixture.allocation,
    retired_at_height: 88,
    released_atoms: 0,
    remaining_atoms: fixture.settlementAmount,
  };
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-navswap-operator-redeem-test-'));
  const keyFile = path.join(root, 'issuer.key.json');
  const signer = path.join(root, 'fake-postfiat-node.js');
  fs.writeFileSync(keyFile, '{}', { mode: 0o600 });
  fs.writeFileSync(signer, `#!/usr/bin/env node
const fs = require('fs');
const args = process.argv.slice(2);
const quoteFile = args[args.indexOf('--quote-file') + 1];
const quote = JSON.parse(fs.readFileSync(quoteFile, 'utf8'));
process.stdout.write(JSON.stringify({
  schema: 'postfiat-signed-asset-transaction-v1',
  unsigned: { source: quote.source, operation: quote.operation },
  signature_hex: 'aa'
}));
`, { mode: 0o700 });
  fs.chmodSync(signer, 0o700);

  const methods = [];
  let submitted = null;
  try {
    await withEnvAsync({
      NAVSWAP_OPERATOR_ISSUER_KEY_FILE: keyFile,
      NAVSWAP_OPERATOR_NODE_BIN: signer,
      NAVSWAP_ISSUER_KEY_FILE: undefined,
      ISSUER_KEY_FILE: undefined,
    }, async () => {
      const start = await executeTransparentNavswapRun({
        route: 'transparent_navswap',
        wallet_address: fixture.wallet,
        quote,
        wallet_action_result: walletResult,
        async: true,
      }, async (_host, _port, request) => {
        methods.push(request.method);
        if (request.method === 'vault_bridge_status') {
          assert.deepStrictEqual(request.params, { asset_id: fixture.settlementAssetId });
          return {
            ok: true,
            result: {
              asset_id: fixture.settlementAssetId,
              allocation_count: 1,
              allocations: [retiredAllocation],
            },
          };
        }
        if (request.method === 'asset_fee_quote') {
          const operation = JSON.parse(request.params.operation_json);
          assert.strictEqual(operation.operation, 'nav_redeem_settle');
          assert.strictEqual(operation.asset_id, fixture.navAssetId);
          assert.strictEqual(operation.settlement_allocation_id, fixture.subscriptionAllocationId);
          assert.strictEqual(operation.settlement_amount_atoms, fixture.settlementAmount);
          assert.match(operation.redemption_id, /^[0-9a-f]{96}$/);
          assert.match(operation.settlement_receipt_hash, /^[0-9a-f]{96}$/);
          return {
            ok: true,
            result: {
              schema: 'postfiat-asset-fee-quote-v1',
              source: 'pfissuer',
              minimum_fee: 1,
              sequence: 10,
              chain_id: 'test-chain',
              genesis_hash: 'genesis',
              protocol_version: 1,
              sender_meets_reserve_after_fee: true,
              operation,
            },
          };
        }
        if (request.method === 'mempool_submit_signed_asset_transaction_finality') {
          submitted = JSON.parse(request.params.signed_asset_transaction_json);
          return {
            ok: true,
            result: { tx_id: 'operator-redeem-settle-tx', round_ok: true },
          };
        }
        throw new Error(`unexpected RPC method ${request.method}`);
      });

      assert.strictEqual(start.ok, true);
      const finalStatus = await waitForNavswapRun(
        start.run_id,
        status => status.status === 'operator_redeem_settle_submitted',
        1000,
      );
      assert.strictEqual(finalStatus.ok, true);
      assert.deepStrictEqual(methods, [
        'vault_bridge_status',
        'asset_fee_quote',
        'mempool_submit_signed_asset_transaction_finality',
      ]);
      assert.strictEqual(submitted.unsigned.operation.operation, 'nav_redeem_settle');
      assert.strictEqual(finalStatus.result.operator_completion.tx_id, 'operator-redeem-settle-tx');
      assert.strictEqual(finalStatus.result.receipt_verification.schema, 'postfiat-navswap-redeem-receipt-verification-v1');
      assert.strictEqual(finalStatus.result.receipt_verification.ok, true);
      assert.strictEqual(finalStatus.result.receipt_verification.nav_redeem_tx_id, 'redeem-tx');
      assert.strictEqual(finalStatus.result.receipt_verification.operator_tx_id, 'operator-redeem-settle-tx');
      assert.strictEqual(finalStatus.result.receipt_verification.settlement_allocation_id, fixture.subscriptionAllocationId);
      assert.strictEqual(finalStatus.result.receipt_verification.checks.operator_submit_accepted, true);
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function testStakehubTransparentRequiresConfigAndPositiveAmount() {
  withEnv({
    NAVSWAP_STAKEHUB_BASE_URL: undefined,
    NAVSWAP_STAKEHUB_URL: undefined,
    NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS: undefined,
  }, () => {
    const config = navswapStakehubTransparentConfig();
    assert.strictEqual(config.configured, false);

    const invalid = buildNavswapQuoteResponse({
      route: 'stakehub_transparent_roundtrip',
      from_asset: 'pfUSDC',
      to_asset: 'a651',
      amount: '0',
    });
    assert.strictEqual(invalid.ok, false);
    assert.strictEqual(invalid.code, 'stakehub_transparent_amount_invalid');

    const missingConfig = buildNavswapQuoteResponse({
      route: 'stakehub_transparent_roundtrip',
      from_asset: 'pfUSDC',
      to_asset: 'a651',
      amount: '0.25',
    });
    assert.strictEqual(missingConfig.ok, false);
    assert.strictEqual(missingConfig.code, 'stakehub_transparent_operator_not_configured');
  });
}

function testStakehubTransparentQuoteAndRunGate() {
  withEnv({
    NAVSWAP_STAKEHUB_BASE_URL: 'http://127.0.0.1:9999',
    NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS: undefined,
    NAVSWAP_STAKEHUB_MAX_A651_AMOUNT: '2',
  }, () => {
    const caps = navswapCapabilities(new Date('2026-06-29T00:00:00.000Z'));
    assert.strictEqual(caps.routes.stakehub_transparent_roundtrip.status, 'operator_quote_only');
    assert.strictEqual(caps.routes.stakehub_transparent_roundtrip.can_quote, true);
    assert.strictEqual(caps.routes.stakehub_transparent_roundtrip.can_run, false);

    const quote = buildNavswapQuoteResponse({
      route: 'stakehub_transparent_roundtrip',
      from_asset: 'pfUSDC',
      to_asset: 'a651',
      amount: '1',
    });
    assert.strictEqual(quote.ok, true);
    assert.strictEqual(quote.status, 'operator_quote_only');
    assert.strictEqual(quote.expected_output, '1');
    assert.strictEqual(quote.custody_boundary, 'stakehub-operator-wallet');

    const tooLarge = buildNavswapQuoteResponse({
      route: 'stakehub_transparent_roundtrip',
      from_asset: 'pfUSDC',
      to_asset: 'a651',
      amount: '3',
    });
    assert.strictEqual(tooLarge.ok, false);
    assert.strictEqual(tooLarge.code, 'stakehub_transparent_amount_exceeds_limit');

    const run = buildNavswapRunResponse({
      route: 'stakehub_transparent_roundtrip',
      from_asset: 'pfUSDC',
      to_asset: 'a651',
      amount: '1',
    });
    assert.strictEqual(run.ok, false);
    assert.strictEqual(run.code, 'stakehub_transparent_runs_disabled');
  });
}

function testIssuedToIssuedAtomicQuoteRequiresPft() {
  const quote = buildNavswapQuoteResponse({
    route: 'pftl_atomic_settlement',
    from_asset: 'pfUSDC',
    to_asset: 'a651',
    amount: '1',
  });
  assert.strictEqual(quote.ok, false);
  assert.strictEqual(quote.code, 'issued_to_issued_requires_pft_intermediary');
}

function testAtomicTemplateParamsRequirePftLeg() {
  assert.throws(
    () => normalizeAtomicTemplateParams({
      left_owner: 'pf-left',
      left_recipient: 'pf-right',
      left_asset_id: 'pfUSDC',
      left_amount: 1,
      right_owner: 'pf-right',
      right_recipient: 'pf-left',
      right_asset_id: 'a651',
      right_amount: 1,
      condition: 'secret',
      cancel_after: 100,
    }),
    /one PFT leg/,
  );

  const params = normalizeAtomicTemplateParams({
    left_owner: 'pf-left',
    left_recipient: 'pf-right',
    left_asset_id: 'PFT',
    left_amount: '1',
    right_owner: 'pf-right',
    right_recipient: 'pf-left',
    right_asset_id: 'a651',
    right_amount: '2',
    condition: 'secret',
    cancel_after: '100',
    right_sequence: '9',
  });
  assert.strictEqual(params.left_asset_id, 'PFT');
  assert.strictEqual(params.left_amount, 1);
  assert.match(params.right_asset_id, /^[0-9a-f]{96}$/);
  assert.strictEqual(params.right_amount, 2);
  assert.strictEqual(params.finish_after, 0);
  assert.strictEqual(params.cancel_after, 100);
  assert.strictEqual(params.right_sequence, 9);
}

function testUniswapRunCannotUseLegacyPool() {
  const run = buildNavswapRunResponse({
    route: 'uniswap_atomic_handoff',
    pool_id: '0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84',
  });
  assert.strictEqual(run.ok, false);
  assert.strictEqual(run.code, 'legacy_pool_rejected');
}

function testAtomicTemplateVerification() {
  const summary = verifyAtomicTemplateResult(atomicTemplateFixture());
  assert.deepStrictEqual(summary, {
    schema: 'postfiat-atomic-settlement-template-v1',
    settlement_id: 'settlement',
    condition_hash: 'hash',
    left_owner: 'pf-left',
    right_owner: 'pf-right',
    left_asset_id: 'PFT',
    right_asset_id: 'a'.repeat(96),
    left_escrow_id: 'escrow-left',
    right_escrow_id: 'escrow-right',
  });

  assert.throws(
    () => verifyAtomicTemplateResult(atomicTemplateFixture({
      leftEscrowId: 'same',
      rightEscrowId: 'same',
    })),
    /distinct escrow ids/,
  );
  assert.throws(
    () => verifyAtomicTemplateResult({
      ...atomicTemplateFixture(),
      settlement_id: null,
      settlement: undefined,
    }),
    /missing settlement_id/,
  );
  assert.throws(
    () => verifyAtomicTemplateResult({
      ...atomicTemplateFixture(),
      left: {
        ...atomicTemplateFixture().left,
        asset_id: 'c'.repeat(96),
      },
      right: {
        ...atomicTemplateFixture().right,
        asset_id: 'b'.repeat(96),
      },
    }),
    /one PFT leg/,
  );
  assert.throws(
    () => verifyAtomicTemplateResult({
      ...atomicTemplateFixture(),
      left: {
        ...atomicTemplateFixture().left,
        operation: { operation: 'transfer' },
        transaction_kind: undefined,
      },
    }),
    /missing escrow_create operation/,
  );
}

function testAtomicTemplateSymmetryVerification() {
  const symmetry = verifyAtomicTemplateSymmetry(
    atomicTemplateFixture(),
    atomicTemplateFixture({ swapped: true }),
  );
  assert.deepStrictEqual(symmetry, {
    schema: 'postfiat-navswap-atomic-template-symmetry-v1',
    stable: true,
    settlement_id: 'settlement',
    condition_hash: 'hash',
    left_escrow_id: 'escrow-left',
    right_escrow_id: 'escrow-right',
  });

  assert.throws(
    () => verifyAtomicTemplateSymmetry(
      atomicTemplateFixture(),
      atomicTemplateFixture({ swapped: true, settlementId: 'different' }),
    ),
    /settlement_id is not symmetric/,
  );
}

async function testAtomicTemplateEndpointExecutesRpcAndVerifies() {
  const seenRequests = [];
  const response = await executeNavswapAtomicTemplate({
    left_owner: 'pf-left',
    left_recipient: 'pf-right',
    left_asset_id: 'PFT',
    left_amount: '2',
    right_owner: 'pf-right',
    right_recipient: 'pf-left',
    right_asset_id: 'a651',
    right_amount: '1',
    condition: 'shared-secret',
    cancel_after: '100',
  }, async (_host, _port, request) => {
    seenRequests.push(request);
    const isSwapped = request.id.endsWith('-swapped');
    return {
      ok: true,
      result: atomicTemplateFixture({
        swapped: isSwapped,
        issuedAssetId: isSwapped ? request.params.left_asset_id : request.params.right_asset_id,
      }),
      events: [{ type: 'stubbed_rpc' }],
    };
  });

  assert.strictEqual(seenRequests.length, 2);
  assert.strictEqual(seenRequests[0].method, 'atomic_settlement_template');
  assert.strictEqual(seenRequests[0].params.left_asset_id, 'PFT');
  assert.strictEqual(seenRequests[0].params.left_amount, 2);
  assert.match(seenRequests[0].params.right_asset_id, /^[0-9a-f]{96}$/);
  assert.strictEqual(seenRequests[0].params.right_amount, 1);
  assert.match(seenRequests[1].id, /-swapped$/);
  assert.strictEqual(seenRequests[1].params.left_asset_id, seenRequests[0].params.right_asset_id);
  assert.strictEqual(seenRequests[1].params.left_amount, 1);
  assert.strictEqual(seenRequests[1].params.right_asset_id, 'PFT');
  assert.strictEqual(seenRequests[1].params.right_amount, 2);
  assert.strictEqual(response.ok, true);
  assert.strictEqual(response.schema, 'postfiat-navswap-atomic-template-v1');
  assert.strictEqual(response.verification.settlement_id, 'settlement');
  assert.strictEqual(response.verification.left_escrow_id, 'escrow-left');
  assert.strictEqual(response.symmetry.stable, true);
  assert.strictEqual(response.symmetry.left_escrow_id, 'escrow-left');
  assert.deepStrictEqual(response.events, [{ type: 'stubbed_rpc' }]);
}

async function testStakehubTransparentRunForwardsToConfiguredEndpoint() {
  let received = null;
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (req.method === 'GET' && url.pathname === '/api/navcoin') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        token: { supply: 4000, nav_per_unit: 4.75 },
        proof: {
          proof_status: 'fresh',
          stale: false,
          freshness_deadline_height: 100338,
          nav_per_unit: 4.75,
          reserve_packet_hash: 'packet-hash',
          envelope_epoch: 3,
          source_receipt_hashes: ['receipt-1'],
        },
        pftl: { chain_id: 'stakehub-demo-chain', current_height: 100300 },
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/navcoin/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        available: true,
        market_operations_status: 'active',
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/balances') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        address: 'pfoperator',
        pfusdc: { balance_atoms: 60003476 },
        a651: { balance_atoms: 3 },
        errors: [],
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ ok: null, status: 'idle' }));
      return;
    }
    assert.strictEqual(req.method, 'POST');
    assert.strictEqual(url.pathname, '/api/shielded-nav-swap/action');
    let raw = '';
    req.on('data', (chunk) => { raw += chunk.toString('utf8'); });
    req.on('end', () => {
      received = JSON.parse(raw);
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        ok: true,
        status: 'transparent_complete',
        message: 'stub transparent roundtrip complete',
        result: {
          summary_file: '/tmp/pftl-only-summary.json',
          report: {
            primary_mint: { settlement_receipt_id: 'settle-in' },
            nav_exit: { redemption_id: 'redeem-1' },
          },
        },
      }));
    });
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const port = server.address().port;
    await withEnvAsync({
      NAVSWAP_STAKEHUB_BASE_URL: `http://127.0.0.1:${port}`,
      NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS: 'true',
      NAVSWAP_STAKEHUB_MAX_A651_AMOUNT: '2',
      NAVSWAP_STAKEHUB_TIMEOUT_MS: '5000',
    }, async () => {
      const run = await executeNavswapRun({
        route: 'stakehub_transparent_roundtrip',
        from_asset: 'pfUSDC',
        to_asset: 'a651',
        amount: '1',
        wallet_address: 'pfwallet',
      });
      assert.strictEqual(run.ok, true);
      assert.strictEqual(run.status, 'transparent_complete');
      assert.strictEqual(run.amount, '1');
      assert.match(run.run_id, /^navswap-/);
      const status = navswapRunPublic(run.run_id);
      assert.strictEqual(status.status, 'transparent_complete');
      assert.strictEqual(status.quote.nav_proof.reserve_packet_hash, 'packet-hash');
      assert.strictEqual(status.quote.stakehub_preflight.balances.address, 'pfoperator');
      const events = navswapRunEvents(run.run_id);
      assert(events.events.some((event) => event.type === 'nav_proof_checked'));
      assert(events.events.some((event) => event.type === 'stakehub_forward_started'));
      const receipts = navswapRunReceipts(run.run_id);
      assert.strictEqual(receipts.receipts.length, 1);
      assert.strictEqual(receipts.receipts[0].type, 'stakehub_result');
      assert.strictEqual(received.action, 'transparent_roundtrip');
      assert.strictEqual(received.amount, '1');
      assert.strictEqual(received.wallet_address, 'pfwallet');
      assert.strictEqual(received.source, 'wallet-proxy-navswap-adapter');
    });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

async function testStakehubTransparentRunCanExecuteAsync() {
  let releaseAction;
  let responseReleased = false;
  let actionStarted = false;
  const actionRelease = new Promise((resolve) => { releaseAction = resolve; });
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (req.method === 'GET' && url.pathname === '/api/navcoin') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        token: { supply: 4000, nav_per_unit: 4.75 },
        proof: {
          proof_status: 'fresh',
          stale: false,
          freshness_deadline_height: 100338,
          reserve_packet_hash: 'packet-hash',
          envelope_epoch: 3,
        },
        pftl: { current_height: 100300 },
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/navcoin/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ available: true, market_operations_status: 'active' }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/balances') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        address: 'pfoperator',
        pfusdc: { balance_atoms: 60003476 },
        a651: { balance_atoms: 3 },
        errors: [],
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ ok: null, status: 'idle' }));
      return;
    }
    assert.strictEqual(req.method, 'POST');
    assert.strictEqual(url.pathname, '/api/shielded-nav-swap/action');
    actionStarted = true;
    req.resume();
    actionRelease.then(() => {
      responseReleased = true;
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        ok: true,
        status: 'transparent_complete',
        message: 'stub async transparent roundtrip complete',
        result: { summary_file: '/tmp/pftl-only-summary.json' },
      }));
    });
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const port = server.address().port;
    await withEnvAsync({
      NAVSWAP_STAKEHUB_BASE_URL: `http://127.0.0.1:${port}`,
      NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS: 'true',
      NAVSWAP_STAKEHUB_MAX_A651_AMOUNT: '2',
      NAVSWAP_STAKEHUB_TIMEOUT_MS: '5000',
    }, async () => {
      const run = await executeNavswapRun({
        route: 'stakehub_transparent_roundtrip',
        from_asset: 'pfUSDC',
        to_asset: 'a651',
        amount: '1',
        wallet_address: 'pfwallet',
        async: true,
      });
      assert.strictEqual(run.ok, true);
      assert.strictEqual(run.status, 'running');
      assert.match(run.run_id, /^navswap-/);
      assert.strictEqual(navswapRunPublic(run.run_id).status, 'running');
      await waitForNavswapRun(run.run_id, () => actionStarted, 1000);
      assert.strictEqual(responseReleased, false);
      releaseAction();
      const finalStatus = await waitForNavswapRun(
        run.run_id,
        (status) => status.status === 'transparent_complete',
        1000,
      );
      assert.strictEqual(finalStatus.ok, true);
      const events = navswapRunEvents(run.run_id);
      assert(events.events.some((event) => event.type === 'async_run_accepted'));
      assert(events.events.some((event) => event.type === 'stakehub_forward_started'));
      const receipts = navswapRunReceipts(run.run_id);
      assert.strictEqual(receipts.receipts.length, 1);
    });
  } finally {
    releaseAction();
    await new Promise((resolve) => server.close(resolve));
  }
}

async function testNavswapRunListFindsLatestWalletRun() {
  clearNavswapRunsForTest();
  let releaseAction;
  let actionStartedCount = 0;
  const actionRelease = new Promise((resolve) => { releaseAction = resolve; });
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (req.method === 'GET' && url.pathname === '/api/navcoin') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        token: { supply: 4000, nav_per_unit: 4.75 },
        proof: {
          proof_status: 'fresh',
          stale: false,
          freshness_deadline_height: 100338,
          reserve_packet_hash: 'packet-hash',
          envelope_epoch: 3,
        },
        pftl: { current_height: 100300 },
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/navcoin/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ available: true, market_operations_status: 'active' }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/balances') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        address: 'pfoperator',
        pfusdc: { balance_atoms: 60003476 },
        a651: { balance_atoms: 3 },
        errors: [],
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ ok: null, status: 'idle' }));
      return;
    }
    assert.strictEqual(req.method, 'POST');
    assert.strictEqual(url.pathname, '/api/shielded-nav-swap/action');
    actionStartedCount += 1;
    req.resume();
    actionRelease.then(() => {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        ok: true,
        status: 'transparent_complete',
        message: 'listed transparent roundtrip complete',
        result: { summary_file: '/tmp/listed-transparent-roundtrip.json' },
      }));
    });
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  await new Promise((resolve) => navswapHttpServer.listen(0, '127.0.0.1', resolve));
  try {
    const port = server.address().port;
    const proxyPort = navswapHttpServer.address().port;
    const walletA = transparentCompletionFixture().wallet;
    const walletB = 'pfaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
    await withEnvAsync({
      NAVSWAP_STAKEHUB_BASE_URL: `http://127.0.0.1:${port}`,
      NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS: 'true',
      NAVSWAP_STAKEHUB_MAX_A651_AMOUNT: '2',
      NAVSWAP_STAKEHUB_TIMEOUT_MS: '5000',
    }, async () => {
      const runA = await executeNavswapRun({
        route: 'stakehub_transparent_roundtrip',
        from_asset: 'pfUSDC',
        to_asset: 'a651',
        amount: '1',
        wallet_address: walletA,
        async: true,
      });
      const runB = await executeNavswapRun({
        route: 'stakehub_transparent_roundtrip',
        from_asset: 'pfUSDC',
        to_asset: 'a651',
        amount: '1',
        wallet_address: walletB,
        async: true,
      });

      await waitForNavswapRun(runA.run_id, () => actionStartedCount >= 2, 1000);
      const listed = navswapRunList({
        wallet_address: walletA,
        route: 'stakehub_transparent_roundtrip',
      });
      assert.strictEqual(listed.ok, true);
      assert.strictEqual(listed.count, 1);
      assert.strictEqual(listed.latest_run.run_id, runA.run_id);
      assert.strictEqual(listed.latest_run.request.wallet_address, walletA);
      assert(!listed.runs.some((run) => run.run_id === runB.run_id));

      const httpResp = await fetch(`http://127.0.0.1:${proxyPort}/api/navswap/runs?wallet_address=${walletA}&route=stakehub_transparent_roundtrip`);
      assert.strictEqual(httpResp.status, 200);
      const httpListed = await httpResp.json();
      assert.strictEqual(httpListed.schema, 'postfiat-navswap-run-list-v1');
      assert.strictEqual(httpListed.count, 1);
      assert.strictEqual(httpListed.latest_run.run_id, runA.run_id);

      releaseAction();
      await waitForNavswapRun(runA.run_id, (status) => status.status === 'transparent_complete', 1000);
      await waitForNavswapRun(runB.run_id, (status) => status.status === 'transparent_complete', 1000);
      assert.strictEqual(navswapRunList({ wallet_address: walletA }).count, 0);
      const withTerminal = navswapRunList({ wallet_address: walletA, include_terminal: true });
      assert.strictEqual(withTerminal.count, 1);
      assert.strictEqual(withTerminal.latest_run.run_id, runA.run_id);
    });
  } finally {
    releaseAction();
    await new Promise((resolve) => navswapHttpServer.close(resolve));
    await new Promise((resolve) => server.close(resolve));
    clearNavswapRunsForTest();
  }
}

async function testNavswapRunStreamPublishesAsyncUpdates() {
  let releaseAction;
  let actionStarted = false;
  const actionRelease = new Promise((resolve) => { releaseAction = resolve; });
  const stakehub = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (req.method === 'GET' && url.pathname === '/api/navcoin') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        token: { supply: 4000, nav_per_unit: 4.75 },
        proof: {
          proof_status: 'fresh',
          stale: false,
          freshness_deadline_height: 100338,
          reserve_packet_hash: 'packet-hash',
          envelope_epoch: 3,
        },
        pftl: { current_height: 100300 },
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/navcoin/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ available: true, market_operations_status: 'active' }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/balances') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        address: 'pfoperator',
        pfusdc: { balance_atoms: 60003476 },
        a651: { balance_atoms: 3 },
        errors: [],
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ ok: null, status: 'idle' }));
      return;
    }
    assert.strictEqual(req.method, 'POST');
    assert.strictEqual(url.pathname, '/api/shielded-nav-swap/action');
    actionStarted = true;
    req.resume();
    actionRelease.then(() => {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        ok: true,
        status: 'transparent_complete',
        message: 'streamed transparent roundtrip complete',
        result: { summary_file: '/tmp/pftl-only-summary.json' },
      }));
    });
  });
  await new Promise((resolve) => stakehub.listen(0, '127.0.0.1', resolve));
  await new Promise((resolve) => navswapHttpServer.listen(0, '127.0.0.1', resolve));
  try {
    const stakehubPort = stakehub.address().port;
    const proxyPort = navswapHttpServer.address().port;
    await withEnvAsync({
      NAVSWAP_STAKEHUB_BASE_URL: `http://127.0.0.1:${stakehubPort}`,
      NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS: 'true',
      NAVSWAP_STAKEHUB_MAX_A651_AMOUNT: '2',
      NAVSWAP_STAKEHUB_TIMEOUT_MS: '5000',
    }, async () => {
      const run = await executeNavswapRun({
        route: 'stakehub_transparent_roundtrip',
        from_asset: 'pfUSDC',
        to_asset: 'a651',
        amount: '1',
        wallet_address: 'pfwallet',
        async: true,
      });
      assert.strictEqual(run.ok, true);
      assert.strictEqual(run.stream_endpoint, `/api/navswap/runs/${run.run_id}/stream`);
      const snapshot = navswapRunStreamSnapshot(run.run_id);
      assert.strictEqual(snapshot.schema, 'postfiat-navswap-run-stream-event-v1');
      assert.strictEqual(snapshot.status.stream_endpoint, run.stream_endpoint);
      const streamPromise = collectSseEvents(
        `http://127.0.0.1:${proxyPort}${run.stream_endpoint}`,
        'navswap_run_done',
        3000,
      );
      await waitForNavswapRun(run.run_id, () => actionStarted, 1000);
      releaseAction();
      const sseEvents = await streamPromise;
      assert(sseEvents.some((event) => event.event === 'navswap_run_snapshot'));
      assert(sseEvents.some((event) => event.event === 'navswap_run_update'));
      const done = sseEvents.find((event) => event.event === 'navswap_run_done');
      assert(done);
      assert.strictEqual(done.data.status.status, 'transparent_complete');
      assert.strictEqual(done.data.status.ok, true);
      assert.strictEqual(done.data.receipts.length, 1);
    });
  } finally {
    releaseAction();
    await new Promise((resolve) => navswapHttpServer.close(resolve));
    await new Promise((resolve) => stakehub.close(resolve));
  }
}

async function testNavswapRunStoreRestoresCompletedRun() {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-navswap-runs-'));
  const storePath = path.join(tmpDir, 'runs.jsonl');
  const stakehub = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (req.method === 'GET' && url.pathname === '/api/navcoin') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        token: { supply: 4000, nav_per_unit: 4.75 },
        proof: {
          proof_status: 'fresh',
          stale: false,
          freshness_deadline_height: 100338,
          reserve_packet_hash: 'packet-hash',
          envelope_epoch: 3,
        },
        pftl: { current_height: 100300 },
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/navcoin/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ available: true, market_operations_status: 'active' }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/balances') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        address: 'pfoperator',
        pfusdc: { balance_atoms: 60003476 },
        a651: { balance_atoms: 3 },
        errors: [],
      }));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/status') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ ok: null, status: 'idle' }));
      return;
    }
    assert.strictEqual(req.method, 'POST');
    assert.strictEqual(url.pathname, '/api/shielded-nav-swap/action');
    req.resume();
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({
      ok: true,
      status: 'transparent_complete',
      message: 'persisted transparent roundtrip complete',
      result: { summary_file: '/tmp/persisted-transparent-roundtrip.json' },
    }));
  });
  await new Promise((resolve) => stakehub.listen(0, '127.0.0.1', resolve));
  try {
    const stakehubPort = stakehub.address().port;
    await withEnvAsync({
      NAVSWAP_RUN_STORE_PATH: storePath,
      NAVSWAP_STAKEHUB_BASE_URL: `http://127.0.0.1:${stakehubPort}`,
      NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS: 'true',
      NAVSWAP_STAKEHUB_MAX_A651_AMOUNT: '2',
      NAVSWAP_STAKEHUB_TIMEOUT_MS: '5000',
    }, async () => {
      assert.strictEqual(navswapRunStorePath(), storePath);
      const run = await executeNavswapRun({
        route: 'stakehub_transparent_roundtrip',
        from_asset: 'pfUSDC',
        to_asset: 'a651',
        amount: '1',
        wallet_address: 'pfwallet',
      });
      assert.strictEqual(run.ok, true);
      assert.strictEqual(run.status, 'transparent_complete');
      assert(fs.existsSync(storePath));
      const runId = run.run_id;

      clearNavswapRunsForTest();
      assert.strictEqual(navswapRunPublic(runId), null);

      const loaded = loadNavswapRunStore();
      assert.strictEqual(loaded.enabled, true);
      assert.strictEqual(loaded.loaded_count, 1);
      assert.strictEqual(loaded.interrupted_count, 0);
      const restored = navswapRunPublic(runId);
      assert.strictEqual(restored.status, 'transparent_complete');
      assert.strictEqual(restored.quote.nav_proof.reserve_packet_hash, 'packet-hash');
      assert(navswapRunEvents(runId).events.some((event) => event.type === 'stakehub_forward_started'));
      assert.strictEqual(navswapRunReceipts(runId).receipts.length, 1);
    });
  } finally {
    clearNavswapRunsForTest();
    await new Promise((resolve) => stakehub.close(resolve));
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

async function main() {
  testCapabilitiesGateUniswapHandoff();
  testShieldedNavswapCapabilitiesExposeQuotePreflight();
  await testShieldedNavswapQuoteBindsLiquidityAndExpiry();
  await testShieldedNavswapSwapGateRequiresFreshOpaqueAction();
  await testShieldedNavswapSwapSubmitUsesWarmServiceForBatchOnly();
  await testShieldedLaggardCatchUpRunsRpcCatchUpAndRechecksConvergence();
  await testShieldedLaggardCatchUpWaitsForDeferredSource();
  await testShieldedNavswapEgressRequiresDisclosureBoundary();
  await testShieldedNavswapProverReadinessUsesLocalService();
  testShieldedCertifiedRoundArgsWaitForFullFleet();
  await testShieldedNavswapQuoteRequiresLiquidityConfig();
  testUniswapHandoffRejectsLegacyPoolConfig();
  testUniswapHandoffControlledBetaCapabilityAndRunPacket();
  testUniswapHandoffUsesNodeRouteDigestFixture();
  testUniswapHandoffFinalityClassRequiresThreeWayAgreement();
  testUniswapHandoffQuoteBindsMintAndSwapFields();
  await testUniswapHandoffPreparesWalletOwnedSourceBatchFromNodeState();
  await testUniswapHandoffRunVerifiesPacketAndSubmitsDestinationConsume();
  testTransparentQuoteRefusesPlaceholder();
  await testTransparentTrustSetActionPrepareIsRejected();
  await testTransparentAllocateActionPrepareBuildsWalletAction();
  await testTransparentRedeemActionPrepareBuildsWalletAction();
  await testTransparentActionBatchPrepareBuildsOrderedWalletActions();
  await testTransparentActionPrepareRejectsUnsupportedStage();
  await testTransparentActionBatchPrepareRejectsFailedItem();
  await testTransparentQuoteWithPlannerActionsReturnsPreparedBatch();
  await testTransparentQuoteRejectsTrustSetPlannerAction();
  await testTransparentPlannerInputsSelectsSettlementSource();
  await testTransparentPlannerInputsComputesRequiredSettlement();
  await testTransparentPlannerInputsAcceptsFractionalNavAmount();
  await testTransparentPlannerInputsBuildsRedeemSettlementCompletion();
  await testTransparentPlannerRejectsUnbackedRedeemBeforeWalletAction();
  await testTransparentQuoteAutoPlanReturnsPreparedBatch();
  await testTransparentReadinessReportsSettlementFundingBlocker();
  await testTransparentReadinessBlocksLowPftFeeReserve();
  await testDevnetPfusdcFundingSubmitsShortfallWithoutTrustlineGate();
  await testTransparentPlannerInputsFailWithoutSettlementSource();
  await testTransparentPlannerInputsRejectsStaleSettlementSource();
  await testTransparentPlannerInputsReportsMissingMarketOpsEnvelope();
  await testTransparentCompletionWaitsForOperatorKeyAfterAllocationVerified();
  await testTransparentCompletionFailsUntilAllocationVisible();
  await testTransparentCompletionSignsAndSubmitsConfiguredOperatorMint();
  await testTransparentCompletionSignsAndSubmitsConfiguredOperatorRedeemSettle();
  testStakehubTransparentRequiresConfigAndPositiveAmount();
  testStakehubTransparentQuoteAndRunGate();
  testIssuedToIssuedAtomicQuoteRequiresPft();
  testAtomicTemplateParamsRequirePftLeg();
  testUniswapRunCannotUseLegacyPool();
  testAtomicTemplateVerification();
  testAtomicTemplateSymmetryVerification();
  await testAtomicTemplateEndpointExecutesRpcAndVerifies();
  await testStakehubTransparentRunForwardsToConfiguredEndpoint();
  await testStakehubTransparentRunCanExecuteAsync();
  await testNavswapRunListFindsLatestWalletRun();
  await testNavswapRunStreamPublishesAsyncUpdates();
  await testNavswapRunStoreRestoresCompletedRun();
  await runNavswapPolicyPersistenceTests();
  console.log('navswap adapter tests passed');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
