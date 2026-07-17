import assert from 'node:assert/strict';
import test from 'node:test';

import {
  NAVSWAP_WALLET_ACTION_SCHEMA,
  PFTL_UNISWAP_BETA_ROUTE,
  submitNavswapPreparedAssetAction,
  submitNavswapPreparedAssetActions,
  verifyNavswapPreparedAssetAction,
} from './navswap-actions.js';

const wallet = 'pfwallet123';
const pfusdc = '87'.repeat(48);
const a651 = 'dc'.repeat(48);
const a666 = 'aa'.repeat(48);
const legacyPftlA651 = 'dcddbf56e7e15f7893d0038e8e0e6089d5a41418dead75353aabb8c016cf626beeb93bc802929f29883c078d910f59d5';
const legacyEthereumA651 = '0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e';
const reservePacketHash = 'ab'.repeat(48);

function allocateRequest(overrides = {}) {
  return {
    schema: NAVSWAP_WALLET_ACTION_SCHEMA,
    route: 'transparent_navswap',
    action_id: 'action-1',
    stage: 'nav_subscription_allocate',
    source: wallet,
    wallet_address: wallet,
    user_intent: {
      wallet_address: wallet,
      from_asset_id: pfusdc,
      to_asset_id: a651,
      max_settlement_amount_atoms: '1000000',
      subscription_id: 'navsub-test-1',
      operator: 'pfissuer',
      route_family: 'primary_pftl_mint',
      purchase_kind: 'primary_pftl_mint',
      route_trust_class: 'CONTROLLED',
      supply_effect: 'mints_new_native_navcoin_supply',
      pricing_source: 'finalized_pre_inflow_nav_snapshot',
      settlement_reserve_effect: 'added_after_primary_fill',
      uniswap_supply_effect: 'not_uniswap_supply',
      mint_amount_atoms: '143650',
      pricing_nav_epoch: '3',
      primary_nav_price_atoms: '6961850',
      pricing_reserve_packet_hash: reservePacketHash,
      nav_epoch: '3',
      nav_per_unit: '6961850',
      reserve_packet_hash: reservePacketHash,
      nav_reserve_packet_hash: reservePacketHash,
    },
    operation: {
      operation: 'vault_bridge_nav_subscription_allocate',
      operator: 'pfissuer',
      nav_asset_id: a651,
      settlement_asset_id: pfusdc,
      settlement_bucket_id: 'bucket-1',
      settlement_receipt_id: 'receipt-1',
      settlement_amount_atoms: 500000,
      consume_supply_owner: wallet,
      consume_supply_allocation_id: 'allocation-1',
      nav_recipient: wallet,
      subscription_id: 'navsub-test-1',
    },
    ...overrides,
  };
}

function betaCompositeRequest(overrides = {}) {
  const base = allocateRequest({
    route: PFTL_UNISWAP_BETA_ROUTE,
    user_intent: {
      ...allocateRequest().user_intent,
      to_asset_id: a666,
      target_nav_asset_id: a666,
      route_family: 'composite_primary_mint_to_ethereum_venue',
      purchase_kind: 'composite_primary_mint_to_ethereum_venue',
      bridge_packet_effect: 'minted_navcoin_exported_or_claimed',
      ethereum_supply_effect: 'mints_wrapped_venue_token_from_pftl_packet',
      wrapped_navcoin_token: '0x2222222222222222222222222222222222222222',
      uniswap_pool_id: `0x${'11'.repeat(32)}`,
      route_supply_cap_atoms: '10000000',
      supply_cap_remaining_atoms: '9999500',
      packet_notional_cap_atoms: '200000000',
      route_paused: false,
      public_routing_enabled: false,
    },
    operation: {
      ...allocateRequest().operation,
      nav_asset_id: a666,
    },
  });
  return {
    ...base,
    ...overrides,
    user_intent: {
      ...base.user_intent,
      ...(overrides.user_intent || {}),
    },
    operation: {
      ...base.operation,
      ...(overrides.operation || {}),
    },
  };
}

function pftlUniswapPrimarySubscribeRequest(overrides = {}) {
  const base = betaCompositeRequest({
    stage: 'pftl_uniswap_primary_subscribe',
    user_intent: {
      ...betaCompositeRequest().user_intent,
      route_id: 'pftl-a651-usdc-wallet-e2e-20260702-v1',
      route_config_digest: 'cd'.repeat(48),
      launch_config_digest: 'ef'.repeat(48),
      settlement_value_atoms: '7000000',
      max_settlement_amount_atoms: '7000000',
      mint_amount_atoms: '1000000',
      export_amount_atoms: '1000000',
      nav_price_settlement_atoms_per_nav_atom: '7',
      subscription_nonce: '12'.repeat(32),
      packet_hash: '34'.repeat(48),
      export_nonce: '56'.repeat(32),
      ethereum_recipient: '0x7777777777777777777777777777777777777777',
      destination_deadline_seconds: '1924992000',
      refund_delay_blocks: '5',
    },
    operation: {
      operation: 'pftl_uniswap_primary_subscribe',
      subscriber: wallet,
      route_id: 'pftl-a651-usdc-wallet-e2e-20260702-v1',
      settlement_asset_id: pfusdc,
      subscription_nonce: '12'.repeat(32),
      settlement_value_atoms: 7000000,
      nav_price_settlement_atoms_per_nav_atom: 7,
      pricing_nav_epoch: 3,
      pricing_reserve_packet_hash: reservePacketHash,
    },
  });
  return {
    ...base,
    ...overrides,
    user_intent: {
      ...base.user_intent,
      ...(overrides.user_intent || {}),
    },
    operation: {
      ...base.operation,
      ...(overrides.operation || {}),
    },
  };
}

function pftlUniswapExportDebitRequest(overrides = {}) {
  const base = pftlUniswapPrimarySubscribeRequest({
    stage: 'pftl_uniswap_export_debit',
    operation: {
      operation: 'pftl_uniswap_export_debit',
      owner: wallet,
      route_id: 'pftl-a651-usdc-wallet-e2e-20260702-v1',
      packet_hash: '34'.repeat(48),
      export_nonce: '56'.repeat(32),
      ethereum_recipient: '0x7777777777777777777777777777777777777777',
      amount_atoms: 1000000,
      destination_deadline_seconds: 1924992000,
      refund_delay_blocks: 5,
    },
  });
  return {
    ...base,
    ...overrides,
    user_intent: {
      ...base.user_intent,
      ...(overrides.user_intent || {}),
    },
    operation: {
      ...base.operation,
      ...(overrides.operation || {}),
    },
  };
}

function optimisticCompositeRequest(overrides = {}) {
  return betaCompositeRequest({
    ...overrides,
    user_intent: {
      route_trust_class: 'OPTIMISTIC',
      release_stage: 'optimistic_public_beta',
      optimistic_public_beta: true,
      public_routing_enabled: true,
      poster_bond_wei: '1000000000000000000',
      challenger_bond_wei: '1000000000000000000',
      challenge_gas_cost_with_margin_wei: '414367324620564',
      challenge_window_seconds: '1668',
      challenge_resolution_window_seconds: '900',
      watcher_liveness_slo: 'detect_posted_claims_within_60s_classify_within_300s',
      optimistic_launch_binding_digest:
        'b21f3cffd0a2b981e39e1b19883d31689fb0c77c135fa593bf20bbb501e2732bacf9393e307688dc5c9aa2daa4854980',
      fail_closed_conditions: [
        'pause if watcher liveness fails',
        'pause if verifier parameters differ from binding digest',
      ],
      canonical_nav_price: '6.996659',
      uniswap_market_price: '7.041102',
      proof_freshness: 'fresh:42s',
      bridge_verifier_mode: 'OPTIMISTIC',
      challenge_resolution_mode: 'owner_arbitrated',
      packet_status: 'pending_challenge_window',
      refund_deadline_unix_seconds: '1782936000',
      route_trust_label: 'OPTIMISTIC owner-arbitrated public beta',
      ...(overrides.user_intent || {}),
    },
  });
}

function trustRequest(overrides = {}) {
  return {
    schema: NAVSWAP_WALLET_ACTION_SCHEMA,
    route: 'transparent_navswap',
    action_id: 'trust-1',
    stage: 'trust_set',
    source: wallet,
    wallet_address: wallet,
    user_intent: {
      wallet_address: wallet,
      trust_asset_id: a651,
      limit_atoms: '1000000',
      issuer: 'pfissuer',
    },
    operation: {
      operation: 'trust_set',
      account: wallet,
      issuer: 'pfissuer',
      asset_id: a651,
      limit: 1000000,
    },
    ...overrides,
  };
}

function redeemRequest(overrides = {}) {
  return {
    schema: NAVSWAP_WALLET_ACTION_SCHEMA,
    route: 'transparent_navswap',
    action_id: 'redeem-1',
    stage: 'nav_redeem_at_nav',
    source: wallet,
    wallet_address: wallet,
    user_intent: {
      wallet_address: wallet,
      from_asset_id: a651,
      amount_atoms: '500000',
      nav_epoch: '3',
      reserve_packet_hash: reservePacketHash,
      issuer: 'pfissuer',
    },
    operation: {
      operation: 'nav_redeem_at_nav',
      owner: wallet,
      issuer: 'pfissuer',
      asset_id: a651,
      amount: 500000,
      epoch: 3,
      reserve_packet_hash: reservePacketHash,
    },
    ...overrides,
  };
}

test('verifyNavswapPreparedAssetAction accepts wallet-owned NAV subscription allocation', () => {
  const verified = verifyNavswapPreparedAssetAction(allocateRequest(), wallet);

  assert.equal(verified.ok, true);
  assert.equal(verified.stage, 'nav_subscription_allocate');
  assert.equal(verified.source, wallet);
  assert.equal(verified.operation.operation, 'vault_bridge_nav_subscription_allocate');
  assert.equal(verified.intent.route_family, 'primary_pftl_mint');
  assert.equal(verified.intent.pricing_source, 'finalized_pre_inflow_nav_snapshot');
  assert.equal(verified.intent.supply_effect, 'mints_new_native_navcoin_supply');
  assert.deepEqual(verified.fields, { operation: verified.operation });
});

test('verifyNavswapPreparedAssetAction rejects NAV subscription without primary mint economics', () => {
  const request = allocateRequest({
    user_intent: {
      ...allocateRequest().user_intent,
      route_family: undefined,
      purchase_kind: undefined,
    },
  });

  assert.throws(
    () => verifyNavswapPreparedAssetAction(request, wallet),
    /route_family/,
  );
});

test('verifyNavswapPreparedAssetAction rejects NAV subscription that claims Uniswap creates supply', () => {
  const request = allocateRequest({
    user_intent: {
      ...allocateRequest().user_intent,
      uniswap_supply_effect: 'mints_navcoin_supply',
    },
  });

  assert.throws(
    () => verifyNavswapPreparedAssetAction(request, wallet),
    /uniswap_supply_effect/,
  );
});

test('verifyNavswapPreparedAssetAction rejects trustless label without finality verifier evidence', () => {
  const request = allocateRequest({
    user_intent: {
      ...allocateRequest().user_intent,
      route_trust_class: 'TRUSTLESS_FINALITY',
    },
  });

  assert.throws(
    () => verifyNavswapPreparedAssetAction(request, wallet),
    /TRUSTLESS_FINALITY/,
  );
});

test('verifyNavswapPreparedAssetAction accepts bridge-aware composite handoff when not legacy a651', () => {
  const request = allocateRequest({
    user_intent: {
      ...allocateRequest().user_intent,
      to_asset_id: a666,
      target_nav_asset_id: a666,
      route_family: 'composite_primary_mint_to_ethereum_venue',
      purchase_kind: 'composite_primary_mint_to_ethereum_venue',
      bridge_packet_effect: 'minted_navcoin_exported_or_claimed',
      ethereum_supply_effect: 'mints_wrapped_venue_token_from_pftl_packet',
      wrapped_navcoin_token: '0x2222222222222222222222222222222222222222',
      uniswap_pool_id: `0x${'11'.repeat(32)}`,
    },
    operation: {
      ...allocateRequest().operation,
      nav_asset_id: a666,
    },
  });

  const verified = verifyNavswapPreparedAssetAction(request, wallet);
  assert.equal(verified.ok, true);
  assert.equal(verified.intent.route_family, 'composite_primary_mint_to_ethereum_venue');
});

test('verifyNavswapPreparedAssetAction rejects legacy a651 Uniswap route as bridge handoff', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(allocateRequest({ route: 'legacy_a651_uniswap' }), wallet),
    /inspection-only/,
  );
});

test('verifyNavswapPreparedAssetAction accepts native a651 in bridge-aware composite handoff', () => {
  const request = allocateRequest({
    user_intent: {
      ...allocateRequest().user_intent,
      to_asset_id: legacyPftlA651,
      target_nav_asset_id: legacyPftlA651,
      route_family: 'composite_primary_mint_to_ethereum_venue',
      purchase_kind: 'composite_primary_mint_to_ethereum_venue',
      bridge_packet_effect: 'minted_navcoin_exported_or_claimed',
      ethereum_supply_effect: 'mints_wrapped_venue_token_from_pftl_packet',
      wrapped_navcoin_token: '0x2222222222222222222222222222222222222222',
      uniswap_pool_id: `0x${'11'.repeat(32)}`,
    },
    operation: {
      ...allocateRequest().operation,
      nav_asset_id: legacyPftlA651,
    },
  });

  const verified = verifyNavswapPreparedAssetAction(request, wallet);
  assert.equal(verified.ok, true);
  assert.equal(verified.operation.nav_asset_id, legacyPftlA651);
});

test('verifyNavswapPreparedAssetAction rejects legacy Ethereum a651 token in composite handoff', () => {
  const request = allocateRequest({
    user_intent: {
      ...allocateRequest().user_intent,
      to_asset_id: a666,
      target_nav_asset_id: a666,
      route_family: 'composite_primary_mint_to_ethereum_venue',
      purchase_kind: 'composite_primary_mint_to_ethereum_venue',
      bridge_packet_effect: 'minted_navcoin_exported_or_claimed',
      ethereum_supply_effect: 'mints_wrapped_venue_token_from_pftl_packet',
      wrapped_navcoin_token: legacyEthereumA651,
      uniswap_pool_id: `0x${'11'.repeat(32)}`,
    },
    operation: {
      ...allocateRequest().operation,
      nav_asset_id: a666,
    },
  });

  assert.throws(
    () => verifyNavswapPreparedAssetAction(request, wallet),
    /legacy a651/,
  );
});

test('verifyNavswapPreparedAssetAction rejects legacy a651 pool in composite handoff', () => {
  const request = allocateRequest({
    user_intent: {
      ...allocateRequest().user_intent,
      to_asset_id: legacyPftlA651,
      target_nav_asset_id: legacyPftlA651,
      route_family: 'composite_primary_mint_to_ethereum_venue',
      purchase_kind: 'composite_primary_mint_to_ethereum_venue',
      bridge_packet_effect: 'minted_navcoin_exported_or_claimed',
      ethereum_supply_effect: 'mints_wrapped_venue_token_from_pftl_packet',
      wrapped_navcoin_token: '0x2222222222222222222222222222222222222222',
      uniswap_pool_id: '0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84',
    },
    operation: {
      ...allocateRequest().operation,
      nav_asset_id: legacyPftlA651,
    },
  });

  assert.throws(
    () => verifyNavswapPreparedAssetAction(request, wallet),
    /legacy a651/,
  );
});

test('verifyNavswapPreparedAssetAction accepts controlled PFTL-Uniswap beta composite handoff', () => {
  const verified = verifyNavswapPreparedAssetAction(betaCompositeRequest(), wallet);

  assert.equal(verified.ok, true);
  assert.equal(verified.route, PFTL_UNISWAP_BETA_ROUTE);
  assert.equal(verified.intent.route_trust_class, 'CONTROLLED');
  assert.equal(verified.intent.route_family, 'composite_primary_mint_to_ethereum_venue');
});

test('verifyNavswapPreparedAssetAction accepts PFTL-Uniswap primary subscribe source action', () => {
  const verified = verifyNavswapPreparedAssetAction(pftlUniswapPrimarySubscribeRequest(), wallet);

  assert.equal(verified.ok, true);
  assert.equal(verified.route, PFTL_UNISWAP_BETA_ROUTE);
  assert.equal(verified.stage, 'pftl_uniswap_primary_subscribe');
  assert.equal(verified.operation.operation, 'pftl_uniswap_primary_subscribe');
  assert.equal(verified.intent.route_config_digest, 'cd'.repeat(48));
});

test('verifyNavswapPreparedAssetAction accepts PFTL-Uniswap export debit source action', () => {
  const verified = verifyNavswapPreparedAssetAction(pftlUniswapExportDebitRequest(), wallet);

  assert.equal(verified.ok, true);
  assert.equal(verified.route, PFTL_UNISWAP_BETA_ROUTE);
  assert.equal(verified.stage, 'pftl_uniswap_export_debit');
  assert.equal(verified.operation.packet_hash, '34'.repeat(48));
});

test('verifyNavswapPreparedAssetAction rejects PFTL-Uniswap export amount mutation', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(pftlUniswapExportDebitRequest({
      operation: { amount_atoms: 1000001 },
    }), wallet),
    /export amount_atoms exceeds/,
  );
});

test('verifyNavswapPreparedAssetAction rejects PFTL-Uniswap route digest missing', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(pftlUniswapPrimarySubscribeRequest({
      user_intent: { route_config_digest: '' },
    }), wallet),
    /route_config_digest/,
  );
});

test('verifyNavswapPreparedAssetAction accepts optimistic PFTL-Uniswap public beta composite handoff', () => {
  const verified = verifyNavswapPreparedAssetAction(optimisticCompositeRequest(), wallet);

  assert.equal(verified.ok, true);
  assert.equal(verified.route, PFTL_UNISWAP_BETA_ROUTE);
  assert.equal(verified.intent.route_trust_class, 'OPTIMISTIC');
  assert.equal(verified.intent.route_trust_label, 'OPTIMISTIC owner-arbitrated public beta');
  assert.equal(verified.intent.challenge_resolution_mode, 'owner_arbitrated');
  assert.equal(verified.intent.optimistic_launch_binding_digest.length, 96);
});

test('verifyNavswapPreparedAssetAction rejects beta handoff without supported trust label', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(betaCompositeRequest({
      user_intent: { route_trust_class: 'DISABLED' },
    }), wallet),
    /CONTROLLED or OPTIMISTIC/,
  );
});

test('verifyNavswapPreparedAssetAction rejects optimistic handoff missing Gate 6 public-beta terms', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(optimisticCompositeRequest({
      user_intent: {
        watcher_liveness_slo: '',
      },
    }), wallet),
    /watcher_liveness_slo/,
  );
});

test('verifyNavswapPreparedAssetAction rejects optimistic handoff with misleading visible trust copy', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(optimisticCompositeRequest({
      user_intent: {
        route_trust_label: 'TRUSTLESS_FINALITY',
        bridge_verifier_mode: 'TRUSTLESS_FINALITY',
      },
    }), wallet),
    /must disclose OPTIMISTIC/,
  );
});

test('verifyNavswapPreparedAssetAction rejects arbitrated optimistic handoff without arbitrated copy', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(optimisticCompositeRequest({
      user_intent: {
        route_trust_label: 'OPTIMISTIC public beta',
      },
    }), wallet),
    /must disclose ARBITRATED/,
  );
});

test('verifyNavswapPreparedAssetAction rejects paused beta handoff', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(betaCompositeRequest({
      user_intent: { route_paused: true },
    }), wallet),
    /paused/,
  );
});

test('verifyNavswapPreparedAssetAction rejects beta handoff that exceeds packet cap', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(betaCompositeRequest({
      user_intent: { packet_notional_cap_atoms: '143649' },
    }), wallet),
    /packet cap/,
  );
});

test('verifyNavswapPreparedAssetAction rejects non-composite beta handoff', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(betaCompositeRequest({
      user_intent: {
        route_family: 'primary_pftl_mint',
        purchase_kind: 'primary_pftl_mint',
      },
    }), wallet),
    /composite_primary_mint_to_ethereum_venue/,
  );
});

test('verifyNavswapPreparedAssetAction rejects beta redeem actions', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(redeemRequest({ route: PFTL_UNISWAP_BETA_ROUTE }), wallet),
    /only supports wallet-owned primary subscribe and export debit/,
  );
});

test('verifyNavswapPreparedAssetAction accepts fresh quote metadata', () => {
  const now = Date.now();
  const verified = verifyNavswapPreparedAssetAction(allocateRequest({
    user_intent: {
      ...allocateRequest().user_intent,
      quote_generated_at_ms: String(now),
      quote_expires_at_ms: String(now + 60_000),
      proof_status: 'active',
      market_ops_status: 'active',
      reserve_packet_fresh: true,
      supply_packet_fresh: true,
    },
  }), wallet);

  assert.equal(verified.ok, true);
  assert.equal(verified.intent.reserve_packet_fresh, true);
});

test('verifyNavswapPreparedAssetAction rejects stale quote metadata', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(allocateRequest({
      user_intent: {
        ...allocateRequest().user_intent,
        reserve_packet_fresh: false,
        supply_packet_fresh: true,
      },
    }), wallet),
    /reserve packet is stale/,
  );
});

test('verifyNavswapPreparedAssetAction rejects expired quote metadata', () => {
  const now = Date.now();
  assert.throws(
    () => verifyNavswapPreparedAssetAction(allocateRequest({
      user_intent: {
        ...allocateRequest().user_intent,
        quote_generated_at_ms: String(now - 120_000),
        quote_expires_at_ms: String(now - 60_000),
      },
    }), wallet),
    /quote has expired/,
  );
});

test('verifyNavswapPreparedAssetAction rejects source substitution', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(allocateRequest({ source: 'pfattacker' }), wallet),
    /source does not match/,
  );
});

test('verifyNavswapPreparedAssetAction rejects amount expansion after approval', () => {
  const request = allocateRequest({
    operation: {
      ...allocateRequest().operation,
      settlement_amount_atoms: '1000001',
    },
  });

  assert.throws(
    () => verifyNavswapPreparedAssetAction(request, wallet),
    /settlement_amount_atoms exceeds/,
  );
});

test('verifyNavswapPreparedAssetAction rejects NAV allocation operator mutation', () => {
  const request = allocateRequest({
    operation: {
      ...allocateRequest().operation,
      operator: 'pfotherissuer',
    },
  });

  assert.throws(
    () => verifyNavswapPreparedAssetAction(request, wallet),
    /operator does not match/,
  );
});

test('verifyNavswapPreparedAssetAction rejects NAV subscription id mutation', () => {
  const request = allocateRequest({
    operation: {
      ...allocateRequest().operation,
      subscription_id: 'navsub-attacker',
    },
  });

  assert.throws(
    () => verifyNavswapPreparedAssetAction(request, wallet),
    /subscription_id does not match/,
  );
});

test('verifyNavswapPreparedAssetAction rejects issuer-owned mint actions', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(allocateRequest({
      stage: 'nav_mint_at_nav',
      source: 'pfissuer',
      operation: {
        operation: 'nav_mint_at_nav',
        issuer: 'pfissuer',
        to: wallet,
        asset_id: a651,
        amount: 500000,
        epoch: 3,
        reserve_packet_hash: reservePacketHash,
        settlement_asset_id: pfusdc,
        settlement_bucket_id: 'bucket-1',
        settlement_allocation_id: 'allocation-1',
        settlement_amount_atoms: 500000,
      },
    }), wallet),
    /source does not match|not wallet-owned/,
  );
});

test('verifyNavswapPreparedAssetAction accepts wallet-owned NAV redeem', () => {
  const request = redeemRequest();

  const verified = verifyNavswapPreparedAssetAction(request, wallet);
  assert.equal(verified.stage, 'nav_redeem_at_nav');
  assert.equal(verified.operation.owner, wallet);
});

test('verifyNavswapPreparedAssetAction rejects trust_set actions', () => {
  const request = trustRequest();

  assert.throws(
    () => verifyNavswapPreparedAssetAction(request, wallet),
    /not wallet-owned or is not supported/,
  );
});

test('verifyNavswapPreparedAssetAction rejects trust_set before issuer checks', () => {
  const request = {
    schema: NAVSWAP_WALLET_ACTION_SCHEMA,
    route: 'transparent_navswap',
    action_id: 'trust-1',
    stage: 'trust_set',
    source: wallet,
    user_intent: {
      wallet_address: wallet,
      trust_asset_id: a651,
      issuer: 'pfissuer',
    },
    operation: {
      operation: 'trust_set',
      account: wallet,
      issuer: 'pfotherissuer',
      asset_id: a651,
      limit: '1000000',
    },
  };

  assert.throws(
    () => verifyNavswapPreparedAssetAction(request, wallet),
    /not wallet-owned or is not supported/,
  );
});

test('verifyNavswapPreparedAssetAction rejects NAV reserve packet mutation', () => {
  const request = {
    schema: NAVSWAP_WALLET_ACTION_SCHEMA,
    route: 'transparent_navswap',
    source: wallet,
    user_intent: {
      wallet_address: wallet,
      from_asset_id: a651,
      amount_atoms: '500000',
      nav_epoch: 3,
      reserve_packet_hash: reservePacketHash,
      issuer: 'pfissuer',
    },
    operation: {
      operation: 'nav_redeem_at_nav',
      owner: wallet,
      issuer: 'pfissuer',
      asset_id: a651,
      amount: 500000,
      epoch: 3,
      reserve_packet_hash: 'cd'.repeat(48),
    },
  };

  assert.throws(
    () => verifyNavswapPreparedAssetAction(request, wallet),
    /reserve_packet_hash does not match/,
  );
});

test('verifyNavswapPreparedAssetAction rejects NAV redeem issuer mutation', () => {
  const request = {
    schema: NAVSWAP_WALLET_ACTION_SCHEMA,
    route: 'transparent_navswap',
    source: wallet,
    user_intent: {
      wallet_address: wallet,
      from_asset_id: a651,
      amount_atoms: '500000',
      nav_epoch: 3,
      reserve_packet_hash: reservePacketHash,
      issuer: 'pfissuer',
    },
    operation: {
      operation: 'nav_redeem_at_nav',
      owner: wallet,
      issuer: 'pfotherissuer',
      asset_id: a651,
      amount: 500000,
      epoch: 3,
      reserve_packet_hash: reservePacketHash,
    },
  };

  assert.throws(
    () => verifyNavswapPreparedAssetAction(request, wallet),
    /issuer does not match/,
  );
});

test('verifyNavswapPreparedAssetAction rejects custody key material in request payload', () => {
  assert.throws(
    () => verifyNavswapPreparedAssetAction(allocateRequest({ key_file: '/tmp/user-key.json' }), wallet),
    /key material/,
  );
});

test('submitNavswapPreparedAssetAction verifies and submits exact reviewed operation', async () => {
  const request = allocateRequest();
  const calls = [];
  const txBuilder = {
    async sendAssetTransfer(backupJson, source, fields) {
      calls.push([backupJson, source, fields]);
      return { txId: 'tx-navswap-action', receipt: { accepted: true } };
    },
  };

  const result = await submitNavswapPreparedAssetAction({
    request,
    walletAddress: wallet,
    backupJson: 'backup-json',
    txBuilder,
  });

  assert.equal(result.txId, 'tx-navswap-action');
  assert.equal(result.navswap_action.stage, 'nav_subscription_allocate');
  assert.deepEqual(calls, [['backup-json', wallet, { operation: request.operation }]]);
});

test('submitNavswapPreparedAssetAction rejects expired quote before signing', async () => {
  const now = Date.now();
  let signed = false;
  const txBuilder = {
    async sendAssetTransfer() {
      signed = true;
      return { txId: 'tx-should-not-sign' };
    },
  };

  await assert.rejects(
    () => submitNavswapPreparedAssetAction({
      request: allocateRequest({
        user_intent: {
          ...allocateRequest().user_intent,
          quote_generated_at_ms: String(now - 120_000),
          quote_expires_at_ms: String(now - 60_000),
        },
      }),
      walletAddress: wallet,
      backupJson: 'backup-json',
      txBuilder,
    }),
    /quote has expired/,
  );
  assert.equal(signed, false);
});

test('submitNavswapPreparedAssetActions verifies all actions before sequential signing', async () => {
  const requests = [allocateRequest(), redeemRequest()];
  const calls = [];
  const progress = [];
  const txBuilder = {
    async sendAssetTransfer(backupJson, source, fields) {
      calls.push([backupJson, source, fields.operation.operation]);
      return {
        txId: `tx-${calls.length}`,
        receipt: { accepted: true },
      };
    },
  };

  const result = await submitNavswapPreparedAssetActions({
    requests,
    walletAddress: wallet,
    backupJson: 'backup-json',
    txBuilder,
    onProgress: event => progress.push(event),
  });

  assert.equal(result.ok, true);
  assert.equal(result.count, 2);
  assert.deepEqual(result.actions.map(action => action.stage), [
    'nav_subscription_allocate',
    'nav_redeem_at_nav',
  ]);
  assert.deepEqual(calls, [
    ['backup-json', wallet, 'vault_bridge_nav_subscription_allocate'],
    ['backup-json', wallet, 'nav_redeem_at_nav'],
  ]);
  assert.deepEqual(
    progress.filter(event => event.status === 'submitted').map(event => event.stage),
    ['nav_subscription_allocate', 'nav_redeem_at_nav'],
  );
});

test('submitNavswapPreparedAssetActions rejects invalid batch before signing anything', async () => {
  const calls = [];
  const txBuilder = {
    async sendAssetTransfer() {
      calls.push(['submit']);
      return { txId: 'should-not-submit' };
    },
  };

  await assert.rejects(
    () => submitNavswapPreparedAssetActions({
      requests: [
        redeemRequest(),
        allocateRequest({
          operation: {
            ...allocateRequest().operation,
            settlement_amount_atoms: 1000001,
          },
        }),
      ],
      walletAddress: wallet,
      backupJson: 'backup-json',
      txBuilder,
    }),
    /settlement_amount_atoms exceeds/,
  );
  assert.deepEqual(calls, []);
});

test('submitNavswapPreparedAssetActions exposes partial results when a submit fails', async () => {
  const txBuilder = {
    async sendAssetTransfer(backupJson, source, fields) {
      if (fields.operation.operation === 'vault_bridge_nav_subscription_allocate') {
        throw new Error('asset submit failed');
      }
      return { txId: 'tx-redeem', receipt: { accepted: true } };
    },
  };

  await assert.rejects(
    async () => {
      try {
        await submitNavswapPreparedAssetActions({
          requests: [redeemRequest(), allocateRequest()],
          walletAddress: wallet,
          backupJson: 'backup-json',
          txBuilder,
        });
      } catch (error) {
        assert.equal(error.partial_results.length, 1);
        assert.equal(error.failed_action.stage, 'nav_subscription_allocate');
        throw error;
      }
    },
    /asset submit failed/,
  );
});

test('submitNavswapPreparedAssetActions preserves string signer failures', async () => {
  const txBuilder = {
    async sendAssetTransfer() {
      throw 'asset transaction chain_id mismatch';
    },
  };

  await assert.rejects(
    async () => {
      try {
        await submitNavswapPreparedAssetActions({
          requests: [redeemRequest()],
          walletAddress: wallet,
          backupJson: 'backup-json',
          txBuilder,
        });
      } catch (error) {
        assert.equal(error.partial_results.length, 0);
        assert.equal(error.failed_action.stage, 'nav_redeem_at_nav');
        throw error;
      }
    },
    /asset transaction chain_id mismatch/,
  );
});
