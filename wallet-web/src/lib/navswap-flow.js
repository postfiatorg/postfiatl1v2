const DEFAULT_TRANSPARENT_NAVSWAP_PAIR = Object.freeze({
  from: 'pfUSDC',
  to: 'a651',
  amountAsset: 'a651',
  settlementAsset: 'pfUSDC',
  amountSemantics: 'requested_nav_mint_atoms',
});

const DEFAULT_TRANSPARENT_NAVSWAP_ASSETS = Object.freeze(['PFT', 'pfUSDC', 'a651']);

export function transparentNavswapPrimaryStep({
  readiness = null,
  preparedActionCount = 0,
  fundingAvailable = false,
  quoteFreshness = null,
} = {}) {
  if (!preparedActionCount) {
    return { kind: 'quote' };
  }

  if (quoteFreshness?.expired === true) {
    return {
      kind: 'refresh_readiness',
      reason: 'quote expired',
    };
  }

  if (fundingAvailable) {
    return { kind: 'funding' };
  }

  if (readiness?.can_execute === true) {
    return { kind: 'submit_actions' };
  }

  return {
    kind: 'refresh_readiness',
    reason: readiness?.next_steps?.[0] || readiness?.status || 'not_ready',
  };
}

export function transparentNavswapPairFromCapability(
  capability,
  supportedAssets = DEFAULT_TRANSPARENT_NAVSWAP_ASSETS,
) {
  const allowedAssets = new Set(Array.isArray(supportedAssets) ? supportedAssets : DEFAULT_TRANSPARENT_NAVSWAP_ASSETS);
  const currentPair = capability?.current_pair || {};
  const pickAsset = (value, fallback) => (typeof value === 'string' && allowedAssets.has(value) ? value : fallback);
  const amountAsset = pickAsset(currentPair.amount_asset || currentPair.to_asset, DEFAULT_TRANSPARENT_NAVSWAP_PAIR.amountAsset);
  const settlementAsset = pickAsset(
    currentPair.settlement_asset || currentPair.from_asset,
    DEFAULT_TRANSPARENT_NAVSWAP_PAIR.settlementAsset,
  );
  return {
    from: pickAsset(currentPair.from_asset || settlementAsset, settlementAsset),
    to: pickAsset(currentPair.to_asset || amountAsset, amountAsset),
    amountAsset,
    settlementAsset,
    amountSemantics: typeof currentPair.amount_semantics === 'string' && currentPair.amount_semantics
      ? currentPair.amount_semantics
      : DEFAULT_TRANSPARENT_NAVSWAP_PAIR.amountSemantics,
  };
}

export function transparentNavswapAutoReadinessSignature({
  route = null,
  routeCanQuote = false,
  swapServerConfigured = false,
  address = '',
  activeRunId = null,
  routeQuote = null,
  navswapReadiness = null,
  phase = 'idle',
  readinessRefreshing = false,
  amount = '',
  from = '',
  to = '',
  routeStatus = '',
} = {}) {
  if (route !== 'transparent_navswap') return null;
  if (!routeCanQuote || !swapServerConfigured || !address) return null;
  if (activeRunId || routeQuote || navswapReadiness) return null;
  if (phase !== 'idle' || readinessRefreshing) return null;
  const parsed = Number.parseFloat(amount);
  if (!Number.isFinite(parsed) || parsed <= 0) return null;
  return [address, from, to, String(amount), routeStatus || ''].join(':');
}

export function transparentNavswapFundingFollowup(readiness) {
  const funding = readiness?.funding || null;
  if (funding?.available === true) {
    return { kind: 'fund' };
  }
  const reason = funding?.unavailable_reason
    || readiness?.next_steps?.[0]
    || readiness?.status
    || 'refresh readiness before requesting funding';
  return { kind: 'blocked', reason };
}

export function transparentNavswapPftFeeStatus(readiness) {
  const walletPft = readiness?.wallet_pft || null;
  if (!walletPft) return null;
  const preflight = walletPft.fee_preflight || null;
  const ok = walletPft.sufficient_for_prepared_actions === true && preflight?.ok !== false;
  return {
    ok,
    balanceAtoms: walletPft.balance_atoms ?? null,
    totalMinimumFeeAtoms: preflight?.total_minimum_fee_atoms ?? null,
    actionCount: preflight?.action_count ?? null,
    failedCode: preflight?.failed_action?.code || null,
    failedMessage: preflight?.failed_action?.message || null,
    failedStage: preflight?.failed_action?.stage || null,
    status: preflight?.status || (ok ? 'fee_preflight_ready' : 'fee_preflight_unavailable'),
  };
}

export function transparentNavswapRunIsTerminal(status, streamTerminal = false) {
  if (streamTerminal === true) return true;
  if (!status) return false;
  if (status.terminal === true) return true;
  if (status.ok === true || status.ok === false) return true;
  return [
    'operator_mint_submitted',
    'operator_redeem_settle_submitted',
    'destination_consume_submitted',
    'complete',
    'failed',
    'interrupted',
    'transparent_complete',
  ].includes(status.status);
}

export function transparentNavswapActiveRunIdAfterStatus(activeRunId, status, streamTerminal = false) {
  if (!activeRunId) return null;
  return transparentNavswapRunIsTerminal(status, streamTerminal) ? null : activeRunId;
}

export function transparentNavswapCanStartFreshQuote({
  route = null,
  phase = null,
  status = null,
} = {}) {
  return route === 'transparent_navswap'
    && phase === 'done'
    && status?.ok === true
    && transparentNavswapRunIsTerminal(status);
}

export function transparentNavswapRecoveredRunState({
  run = null,
  dismissedRunIds = [],
} = {}) {
  if (!run?.run_id) return null;
  if (runIdCollectionHas(dismissedRunIds, run.run_id)) return null;
  if (transparentNavswapRunIsTerminal(run)) {
    if (run.ok !== true) return null;
    return {
      activeRunId: null,
      phase: 'done',
      message: run.message || 'Recovered completed NAVSwap run',
    };
  }
  return {
    activeRunId: run.run_id,
    phase: 'running',
    message: run.message || 'Recovered active NAVSwap run',
  };
}

export function transparentNavswapQuoteFreshness(quote, nowMs = Date.now()) {
  const actions = navswapPreparedActions(quote);
  const candidates = [];

  const addCandidate = (source) => {
    if (!source || typeof source !== 'object') return;
    const generatedAtMs = positiveInteger(source.quote_generated_at_ms);
    const expiresAtMs = positiveInteger(source.quote_expires_at_ms);
    if (!generatedAtMs && !expiresAtMs) return;
    candidates.push({
      generatedAtMs,
      expiresAtMs,
      reservePacketFresh: source.reserve_packet_fresh,
      supplyPacketFresh: source.supply_packet_fresh,
      proofStatus: source.proof_status,
      marketOpsStatus: source.market_ops_status,
    });
  };

  addCandidate(quote?.quote_freshness);
  addCandidate(quote?.navswap_freshness);
  addCandidate(quote?.prepared_action_batch?.quote_freshness);
  addCandidate(quote?.prepared_action_batch?.navswap_freshness);
  for (const action of actions) {
    addCandidate(action?.user_intent);
    addCandidate(action?.user_intent?.navswap_freshness);
  }

  const expiresValues = candidates.map(item => item.expiresAtMs).filter(Boolean);
  const generatedValues = candidates.map(item => item.generatedAtMs).filter(Boolean);
  const expiresAtMs = expiresValues.length ? Math.min(...expiresValues) : null;
  const generatedAtMs = generatedValues.length ? Math.min(...generatedValues) : null;
  const expiresInMs = expiresAtMs ? expiresAtMs - nowMs : null;
  return {
    present: candidates.length > 0,
    generatedAtMs,
    expiresAtMs,
    expiresInMs,
    expired: Boolean(expiresAtMs && expiresAtMs <= nowMs),
    reservePacketFresh: firstDefined(candidates.map(item => item.reservePacketFresh)),
    supplyPacketFresh: firstDefined(candidates.map(item => item.supplyPacketFresh)),
    proofStatus: firstDefined(candidates.map(item => item.proofStatus)),
    marketOpsStatus: firstDefined(candidates.map(item => item.marketOpsStatus)),
  };
}

function navswapPreparedActions(quote) {
  const batchActions = quote?.prepared_action_batch?.actions;
  if (Array.isArray(batchActions)) return batchActions;
  if (Array.isArray(quote?.prepared_actions)) return quote.prepared_actions;
  if (Array.isArray(quote?.actions)) return quote.actions;
  return [];
}

function positiveInteger(value) {
  if (typeof value === 'number' && Number.isSafeInteger(value) && value > 0) return value;
  if (typeof value === 'string' && /^[1-9][0-9]*$/.test(value)) {
    const parsed = Number(value);
    return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null;
  }
  return null;
}

function firstDefined(values) {
  for (const value of values) {
    if (value !== undefined && value !== null) return value;
  }
  return undefined;
}

function runIdCollectionHas(collection, runId) {
  if (!runId || !collection) return false;
  if (typeof collection.has === 'function') return collection.has(runId);
  if (Array.isArray(collection)) return collection.includes(runId);
  return false;
}
