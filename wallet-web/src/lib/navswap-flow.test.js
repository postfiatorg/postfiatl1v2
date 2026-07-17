import assert from 'node:assert/strict';
import test from 'node:test';

import {
  transparentNavswapActiveRunIdAfterStatus,
  transparentNavswapAutoReadinessSignature,
  transparentNavswapCanStartFreshQuote,
  transparentNavswapFundingFollowup,
  transparentNavswapPairFromCapability,
  transparentNavswapPftFeeStatus,
  transparentNavswapPrimaryStep,
  transparentNavswapQuoteFreshness,
  transparentNavswapRecoveredRunState,
  transparentNavswapRunIsTerminal,
} from './navswap-flow.js';

test('transparentNavswapPrimaryStep starts with quote before prepared actions', () => {
  assert.deepEqual(transparentNavswapPrimaryStep(), { kind: 'quote' });
});

test('transparentNavswapPairFromCapability reads adapter amount and settlement metadata', () => {
  assert.deepEqual(transparentNavswapPairFromCapability({
    current_pair: {
      from_asset: 'pfUSDC',
      to_asset: 'a651',
      amount_asset: 'a651',
      settlement_asset: 'pfUSDC',
      amount_semantics: 'requested_nav_mint_atoms',
    },
  }), {
    from: 'pfUSDC',
    to: 'a651',
    amountAsset: 'a651',
    settlementAsset: 'pfUSDC',
    amountSemantics: 'requested_nav_mint_atoms',
  });
});

test('transparentNavswapPairFromCapability falls back to the live transparent pair', () => {
  assert.deepEqual(transparentNavswapPairFromCapability({
    current_pair: {
      from_asset: 'unsupported',
      to_asset: 'also-unsupported',
      amount_asset: 'bad',
      settlement_asset: null,
      amount_semantics: '',
    },
  }), {
    from: 'pfUSDC',
    to: 'a651',
    amountAsset: 'a651',
    settlementAsset: 'pfUSDC',
    amountSemantics: 'requested_nav_mint_atoms',
  });
});

test('transparentNavswapAutoReadinessSignature enables the initial transparent quote load', () => {
  assert.equal(
    transparentNavswapAutoReadinessSignature({
      route: 'transparent_navswap',
      routeCanQuote: true,
      swapServerConfigured: true,
      address: 'pfwallet',
      phase: 'idle',
      amount: '1',
      from: 'pfUSDC',
      to: 'a651',
      routeStatus: 'quote_ready',
    }),
    'pfwallet:pfUSDC:a651:1:quote_ready',
  );
});

test('transparentNavswapAutoReadinessSignature stops after quote or readiness exists', () => {
  const base = {
    route: 'transparent_navswap',
    routeCanQuote: true,
    swapServerConfigured: true,
    address: 'pfwallet',
    phase: 'idle',
    amount: '1',
    from: 'pfUSDC',
    to: 'a651',
    routeStatus: 'quote_ready',
  };
  assert.equal(transparentNavswapAutoReadinessSignature({ ...base, routeQuote: { ok: true } }), null);
  assert.equal(transparentNavswapAutoReadinessSignature({ ...base, navswapReadiness: { ok: true } }), null);
  assert.equal(transparentNavswapAutoReadinessSignature({ ...base, activeRunId: 'navswap-run' }), null);
});

test('transparentNavswapAutoReadinessSignature rejects unavailable or invalid states', () => {
  const base = {
    route: 'transparent_navswap',
    routeCanQuote: true,
    swapServerConfigured: true,
    address: 'pfwallet',
    phase: 'idle',
    amount: '1',
    from: 'pfUSDC',
    to: 'a651',
    routeStatus: 'quote_ready',
  };
  assert.equal(transparentNavswapAutoReadinessSignature({ ...base, route: 'pftl_atomic_settlement' }), null);
  assert.equal(transparentNavswapAutoReadinessSignature({ ...base, routeCanQuote: false }), null);
  assert.equal(transparentNavswapAutoReadinessSignature({ ...base, swapServerConfigured: false }), null);
  assert.equal(transparentNavswapAutoReadinessSignature({ ...base, address: '' }), null);
  assert.equal(transparentNavswapAutoReadinessSignature({ ...base, phase: 'running' }), null);
  assert.equal(transparentNavswapAutoReadinessSignature({ ...base, readinessRefreshing: true }), null);
  assert.equal(transparentNavswapAutoReadinessSignature({ ...base, amount: '0' }), null);
  assert.equal(transparentNavswapAutoReadinessSignature({ ...base, amount: 'nope' }), null);
});

test('transparentNavswapFundingFollowup requests funding when the helper is available', () => {
  assert.deepEqual(transparentNavswapFundingFollowup({
    funding: {
      available: true,
      amount_atoms: '6958370',
    },
  }), { kind: 'fund' });
});

test('transparentNavswapFundingFollowup surfaces hard funding blockers', () => {
  assert.deepEqual(transparentNavswapFundingFollowup({
    funding: {
      enabled: true,
      signing_configured: true,
      available: false,
      unavailable_reason: 'recipient_window_exceeded',
    },
  }), {
    kind: 'blocked',
    reason: 'recipient_window_exceeded',
  });
  assert.deepEqual(transparentNavswapFundingFollowup({
    status: 'not_ready',
    next_steps: ['fund the wallet with the required settlement asset'],
  }), {
    kind: 'blocked',
    reason: 'fund the wallet with the required settlement asset',
  });
});

test('transparentNavswapPftFeeStatus summarizes prepared-action fee readiness', () => {
  assert.equal(transparentNavswapPftFeeStatus(null), null);
  assert.deepEqual(transparentNavswapPftFeeStatus({
    wallet_pft: {
      balance_atoms: '6999955',
      sufficient_for_prepared_actions: true,
      fee_preflight: {
        ok: true,
        status: 'fee_preflight_ready',
        action_count: 2,
        total_minimum_fee_atoms: '56',
      },
    },
  }), {
    ok: true,
    balanceAtoms: '6999955',
    totalMinimumFeeAtoms: '56',
    actionCount: 2,
    failedCode: null,
    failedMessage: null,
    failedStage: null,
    status: 'fee_preflight_ready',
  });
  assert.deepEqual(transparentNavswapPftFeeStatus({
    wallet_pft: {
      balance_atoms: '15',
      sufficient_for_prepared_actions: false,
      fee_preflight: {
        ok: false,
        status: 'fee_preflight_failed',
        action_count: 2,
        total_minimum_fee_atoms: '35',
        failed_action: {
          stage: 'nav_subscription_allocate',
          code: 'navswap_action_fee_preflight_failed',
          message: 'Fund the wallet with PFT for NAVSwap fees/reserves.',
        },
      },
    },
  }), {
    ok: false,
    balanceAtoms: '15',
    totalMinimumFeeAtoms: '35',
    actionCount: 2,
    failedCode: 'navswap_action_fee_preflight_failed',
    failedMessage: 'Fund the wallet with PFT for NAVSwap fees/reserves.',
    failedStage: 'nav_subscription_allocate',
    status: 'fee_preflight_failed',
  });
});

test('transparentNavswapRunIsTerminal follows adapter terminal state', () => {
  assert.equal(transparentNavswapRunIsTerminal(null), false);
  assert.equal(transparentNavswapRunIsTerminal({ ok: null, status: 'running' }), false);
  assert.equal(transparentNavswapRunIsTerminal({ ok: true, status: 'operator_mint_submitted' }), true);
  assert.equal(transparentNavswapRunIsTerminal({ ok: null, status: 'operator_mint_submitted' }), true);
  assert.equal(transparentNavswapRunIsTerminal({ ok: null, status: 'operator_redeem_settle_submitted' }), true);
  assert.equal(transparentNavswapRunIsTerminal({ ok: null, status: 'destination_consume_submitted' }), true);
  assert.equal(transparentNavswapRunIsTerminal({ ok: false, status: 'awaiting_operator_signature' }), true);
  assert.equal(transparentNavswapRunIsTerminal({ ok: null, status: 'failed' }), true);
  assert.equal(transparentNavswapRunIsTerminal({ ok: null, status: 'running', terminal: true }), true);
  assert.equal(transparentNavswapRunIsTerminal({ ok: null, status: 'running' }, true), true);
});

test('transparentNavswapActiveRunIdAfterStatus releases completed runs', () => {
  assert.equal(
    transparentNavswapActiveRunIdAfterStatus('navswap-run', { ok: null, status: 'running' }),
    'navswap-run',
  );
  assert.equal(
    transparentNavswapActiveRunIdAfterStatus('navswap-run', { ok: true, status: 'operator_mint_submitted' }),
    null,
  );
  assert.equal(
    transparentNavswapActiveRunIdAfterStatus('navswap-run', { ok: null, status: 'running' }, true),
    null,
  );
  assert.equal(
    transparentNavswapActiveRunIdAfterStatus(null, { ok: true, status: 'operator_mint_submitted' }),
    null,
  );
});

test('transparentNavswapCanStartFreshQuote only enables after successful transparent completion', () => {
  assert.equal(transparentNavswapCanStartFreshQuote({
    route: 'transparent_navswap',
    phase: 'done',
    status: { ok: true, status: 'operator_mint_submitted' },
  }), true);
  assert.equal(transparentNavswapCanStartFreshQuote({
    route: 'transparent_navswap',
    phase: 'quoted',
    status: { ok: true, status: 'operator_mint_submitted' },
  }), false);
  assert.equal(transparentNavswapCanStartFreshQuote({
    route: 'pftl_atomic_settlement',
    phase: 'done',
    status: { ok: true, status: 'operator_mint_submitted' },
  }), false);
  assert.equal(transparentNavswapCanStartFreshQuote({
    route: 'transparent_navswap',
    phase: 'done',
    status: { ok: false, status: 'failed' },
  }), false);
});

test('transparentNavswapRecoveredRunState separates active and completed recovery', () => {
  assert.deepEqual(transparentNavswapRecoveredRunState({
    run: {
      run_id: 'run-active',
      status: 'operator_mint_pending',
      message: 'still running',
    },
  }), {
    activeRunId: 'run-active',
    phase: 'running',
    message: 'still running',
  });
  assert.deepEqual(transparentNavswapRecoveredRunState({
    run: {
      run_id: 'run-done',
      ok: true,
      status: 'operator_mint_submitted',
      terminal: true,
    },
  }), {
    activeRunId: null,
    phase: 'done',
    message: 'Recovered completed NAVSwap run',
  });
  assert.equal(transparentNavswapRecoveredRunState({
    run: {
      run_id: 'run-done',
      ok: true,
      status: 'operator_mint_submitted',
      terminal: true,
    },
    dismissedRunIds: ['run-done'],
  }), null);
  assert.equal(transparentNavswapRecoveredRunState({
    run: {
      run_id: 'run-failed',
      ok: false,
      status: 'failed',
      terminal: true,
    },
  }), null);
});

test('transparentNavswapPrimaryStep requests guarded funding directly when available', () => {
  const step = transparentNavswapPrimaryStep({
    preparedActionCount: 2,
    fundingAvailable: true,
    readiness: {
      can_execute: false,
    },
  });
  assert.deepEqual(step, { kind: 'funding' });
});

test('transparentNavswapPrimaryStep refreshes an expired prepared quote before any wallet action', () => {
  const step = transparentNavswapPrimaryStep({
    preparedActionCount: 2,
    fundingAvailable: true,
    quoteFreshness: {
      present: true,
      expired: true,
      expiresAtMs: 1000,
    },
    readiness: {
      can_execute: true,
    },
  });
  assert.deepEqual(step, {
    kind: 'refresh_readiness',
    reason: 'quote expired',
  });
});

test('transparentNavswapPrimaryStep submits wallet actions when readiness can execute', () => {
  const step = transparentNavswapPrimaryStep({
    preparedActionCount: 2,
    fundingAvailable: false,
    readiness: {
      can_execute: true,
    },
  });
  assert.deepEqual(step, { kind: 'submit_actions' });
});

test('transparentNavswapPrimaryStep refreshes readiness when prepared but blocked', () => {
  const step = transparentNavswapPrimaryStep({
    preparedActionCount: 2,
    fundingAvailable: false,
    readiness: {
      can_execute: false,
      status: 'not_ready',
      next_steps: ['fund the wallet with the required settlement asset'],
    },
  });
  assert.deepEqual(step, {
    kind: 'refresh_readiness',
    reason: 'fund the wallet with the required settlement asset',
  });
});

test('transparentNavswapQuoteFreshness reads direct user_intent freshness fields', () => {
  const freshness = transparentNavswapQuoteFreshness({
    prepared_action_batch: {
      actions: [
        {
          stage: 'nav_subscription_allocate',
          user_intent: {
            quote_generated_at_ms: 1000,
            quote_expires_at_ms: 7000,
            reserve_packet_fresh: true,
            supply_packet_fresh: true,
            proof_status: 'active',
            market_ops_status: 'active',
          },
        },
        {
          stage: 'nav_subscription_allocate',
          user_intent: {
            quote_generated_at_ms: 1000,
            quote_expires_at_ms: 6000,
            reserve_packet_fresh: true,
            supply_packet_fresh: true,
          },
        },
      ],
    },
  }, 5000);

  assert.deepEqual(freshness, {
    present: true,
    generatedAtMs: 1000,
    expiresAtMs: 6000,
    expiresInMs: 1000,
    expired: false,
    reservePacketFresh: true,
    supplyPacketFresh: true,
    proofStatus: 'active',
    marketOpsStatus: 'active',
  });
});

test('transparentNavswapQuoteFreshness marks stale prepared quotes expired', () => {
  const freshness = transparentNavswapQuoteFreshness({
    prepared_action_batch: {
      actions: [{
        stage: 'nav_subscription_allocate',
        user_intent: {
          quote_generated_at_ms: '1000',
          quote_expires_at_ms: '6000',
        },
      }],
    },
  }, 6000);

  assert.equal(freshness.present, true);
  assert.equal(freshness.expiresAtMs, 6000);
  assert.equal(freshness.expired, true);
});
