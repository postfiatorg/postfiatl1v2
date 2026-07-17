import React, { useCallback, useRef, useState, useEffect } from 'react';
import { formatBalance, PFUSDC_ASSET_ID, A651_ASSET_ID, shortenAssetId } from '../lib/utils.js';
import {
  findAtomicWalletCancelLeg,
  findAtomicWalletCreateLeg,
  findAtomicWalletFinishLeg,
} from '../lib/atomic-settlement.js';
import { submitNavswapPreparedAssetActions } from '../lib/navswap-actions.js';
import {
  transparentNavswapActiveRunIdAfterStatus,
  transparentNavswapAutoReadinessSignature,
  transparentNavswapCanStartFreshQuote,
  transparentNavswapPairFromCapability,
  transparentNavswapPftFeeStatus,
  transparentNavswapPrimaryStep,
  transparentNavswapRecoveredRunState,
  transparentNavswapQuoteFreshness,
  transparentNavswapRunIsTerminal,
} from '../lib/navswap-flow.js';
import {
  evaluatePftlUniswapBetaRoute,
  PFTL_UNISWAP_BETA_ROUTE,
} from '../lib/pftl-uniswap-route.js';
import {
  buildAssetOrchardIngressPayload,
  LocalAssetOrchardProverClient,
  normalizeShieldedCapabilitiesEnvelope,
  normalizeShieldedNavswapCapability,
  normalizeShieldedNavswapQuote,
  shieldedPrivateEgressDisclosureFields,
  shieldedPrivateEgressDisclosureHash,
  SHIELDED_NAVSWAP_EGRESS_POLICY_ID,
  SHIELDED_NAVSWAP_ROUTE,
} from '../lib/shielded-navswap.js';
import {
  AlertCircle,
  ArrowRight,
  ArrowUpDown,
  Check,
  ChevronDown,
  Clock,
  Info,
  Loader2,
  RefreshCw,
  ShieldCheck,
} from 'lucide-react';
import ProductPrivateSwap from './ProductPrivateSwap.jsx';

const ROUTES = {
  transparent_navswap: {
    name: 'Transparent NAVSwap',
    tag: 'Adapter',
    why: 'Public PFTL NAV route. Disabled until the planner can select real inputs and the wallet can submit every prepared action.',
    time: 'quote',
    vis: 'Public',
  },
  [SHIELDED_NAVSWAP_ROUTE]: {
    name: 'Shielded NAVSwap',
    tag: 'Private quote',
    why: 'Fetches a private a651 ↔ a652 quote and liquidity commitment. Private proof and submit remain disabled until Step 7.',
    time: 'preflight',
    vis: 'Private',
  },
  stakehub_transparent_roundtrip: {
    name: 'StakeHub transparent',
    tag: 'Operator route',
    why: 'Uses the existing StakeHub transparent no-Orchard PFTL roundtrip as a gated smoke route.',
    time: 'operator',
    vis: 'Public',
  },
  pftl_atomic_settlement: {
    name: 'PFTL atomic',
    tag: 'ESCROW-009',
    why: 'Builds same-chain PFT-to-issued-asset escrow templates. Each wallet signs its own leg.',
    time: 'template',
    vis: 'Public',
  },
  [PFTL_UNISWAP_BETA_ROUTE]: {
    name: 'PFTL-Uniswap beta',
    tag: 'Controlled beta',
    why: 'Requires explicit CONTROLLED trust class, route caps, pause state, and no legacy a651 fallback before quoting.',
    time: 'beta',
    vis: 'Public',
  },
  legacy_a651_uniswap: {
    name: 'Legacy a651 pool',
    tag: 'Inspect only',
    why: 'Historical a651/USDC liquidity. It is inspection-only and cannot be used for the PFTL-to-Uniswap route.',
    time: 'read-only',
    vis: 'Public',
  },
};

const DEFAULT_TRANSPARENT_NAVSWAP_AMOUNT = '1';
const BASE_SWAP_ASSETS = ['pfUSDC', 'PFT', 'a651'];
const DISPLAYED_SWAP_ROUTES = ['transparent_navswap', SHIELDED_NAVSWAP_ROUTE, PFTL_UNISWAP_BETA_ROUTE];

function fallbackNavswapCapabilities(reason) {
  return {
    ok: false,
    schema: 'postfiat-navswap-capabilities-v1',
    error: reason,
    routes: Object.fromEntries(Object.keys(ROUTES).map(id => [id, {
      label: ROUTES[id].name,
      status: id === SHIELDED_NAVSWAP_ROUTE ? 'preflight_only' : 'unavailable',
      enabled: false,
      can_quote: false,
      can_run: false,
      custody_boundary: id === SHIELDED_NAVSWAP_ROUTE ? 'wallet_local_note_keys_only' : undefined,
      requires_local_prover: id === SHIELDED_NAVSWAP_ROUTE ? true : undefined,
      requires_note_scan: id === SHIELDED_NAVSWAP_ROUTE ? true : undefined,
      supported_pairs: id === SHIELDED_NAVSWAP_ROUTE ? [] : undefined,
      liquidity_mode: id === SHIELDED_NAVSWAP_ROUTE ? 'preflight_only' : undefined,
      privacy_label: id === SHIELDED_NAVSWAP_ROUTE ? 'Private, wallet-local custody' : undefined,
      disabled_reason: id === SHIELDED_NAVSWAP_ROUTE
        ? 'Shielded NAVSwap is stopped at the wallet preflight review gate.'
        : undefined,
      reason: id === SHIELDED_NAVSWAP_ROUTE
        ? 'Shielded NAVSwap is stopped at the wallet preflight review gate.'
        : reason,
    }])),
  };
}

function compactHash(value, edge = 10) {
  const text = String(value || '').trim();
  if (!text) return null;
  return text.length > edge * 2 + 1 ? `${text.slice(0, edge)}…${text.slice(-edge)}` : text;
}

function compactPath(value) {
  const text = String(value || '').trim();
  if (!text) return null;
  const parts = text.split('/').filter(Boolean);
  return parts.length > 2 ? parts.slice(-2).join('/') : text;
}

function navswapReceiptResult(receipts, status) {
  const stakehubReceipt = (receipts || []).find(item => item?.type === 'stakehub_result');
  const transparentReceipt = (receipts || []).find(item => item?.type === 'transparent_navswap_operator_completion' || item?.type === 'navswap_result');
  return stakehubReceipt?.payload || transparentReceipt?.payload || status?.result || null;
}

function randomCondition() {
  const bytes = new Uint8Array(16);
  const cryptoApi = globalThis.crypto;
  if (!cryptoApi?.getRandomValues) return '';
  cryptoApi.getRandomValues(bytes);
  return `wallet-${Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('')}`;
}

function accountAssetItems(result) {
  if (Array.isArray(result)) return result;
  if (Array.isArray(result?.assets)) return result.assets;
  return [];
}

function accountAssetBalanceMap(result) {
  const map = {};
  for (const item of accountAssetItems(result)) {
    const id = item.asset_id || item.id;
    const code = id === PFUSDC_ASSET_ID ? 'pfUSDC' : id === A651_ASSET_ID ? 'a651' : shortenAssetId(id);
    map[code] = item.balance ?? item.amount ?? 0;
  }
  return map;
}

function formatQuoteFreshnessLabel(freshness) {
  if (!freshness?.present) return 'not returned';
  if (freshness.expired) return 'expired';
  if (!Number.isFinite(freshness.expiresInMs)) return 'fresh';
  const seconds = Math.max(0, Math.ceil(freshness.expiresInMs / 1000));
  if (seconds < 60) return `expires in ${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  return remainder ? `expires in ${minutes}m ${remainder}s` : `expires in ${minutes}m`;
}

function navswapPreparedActionsForUi(quote) {
  const batchActions = quote?.prepared_action_batch?.actions;
  if (Array.isArray(batchActions)) return batchActions;
  if (Array.isArray(quote?.prepared_actions)) return quote.prepared_actions;
  if (Array.isArray(quote?.actions)) return quote.actions;
  return [];
}

function navswapFreshnessPayloadForUi(quote) {
  if (!quote || typeof quote !== 'object') return null;
  const actions = navswapPreparedActionsForUi(quote);
  const candidates = [
    quote?.planner_inputs?.quote_freshness,
    quote?.quote_freshness,
    quote?.navswap_freshness,
    quote?.prepared_action_batch?.quote_freshness,
    quote?.prepared_action_batch?.navswap_freshness,
    ...actions.flatMap(action => [
      action?.user_intent?.navswap_freshness,
      action?.user_intent,
    ]),
  ];
  return candidates.find(item => item && typeof item === 'object') || null;
}

function formatFreshFlag(value) {
  if (value === true) return 'yes';
  if (value === false) return 'no';
  return 'unknown';
}

function formatReceiptFreshness(freshness) {
  if (!freshness) return 'unknown';
  if (freshness.fresh === false) return 'stale';
  if (freshness.checked === false) return 'not height-checked';
  const age = freshness.age_blocks;
  const max = freshness.max_snapshot_age_blocks;
  if (age !== null && age !== undefined && max !== null && max !== undefined) {
    return `${age}/${max} blocks`;
  }
  return freshness.fresh === true ? 'fresh' : 'unknown';
}

function formatSwapBalance(asset, atoms) {
  return formatBalance(atoms ?? 0);
}

function decimalToAtomsString(value, precision = 6) {
  const text = String(value || '').trim();
  if (!/^[0-9]+(?:\.[0-9]+)?$/.test(text)) throw new Error('Amount must be a positive decimal');
  const [whole, frac = ''] = text.split('.');
  if (frac.length > precision) throw new Error(`Amount supports at most ${precision} decimals`);
  const atoms = BigInt(whole) * (10n ** BigInt(precision)) + BigInt((frac.padEnd(precision, '0') || '0'));
  if (atoms <= 0n) throw new Error('Amount must be positive');
  if (atoms > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error('Amount exceeds wallet-safe atom range');
  return atoms.toString();
}

export default function Swap({
  rpc,
  txBuilder,
  backupJson,
  address,
  swapServer,
  onToast,
  onNavigate = null,
  chainCapabilities,
  liveSnapshot = null,
  walletFeedStatus = null,
}) {
  const [from, setFrom] = useState('pfUSDC');
  const [to, setTo] = useState('a651');
  const [amt, setAmt] = useState(DEFAULT_TRANSPARENT_NAVSWAP_AMOUNT);
  const [route, setRoute] = useState('transparent_navswap');
  const [phase, setPhase] = useState('idle'); // idle | running | quoted | done
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [assetBalances, setAssetBalances] = useState({});
  const [navswapCaps, setNavswapCaps] = useState(null);
  const [navswapStatus, setNavswapStatus] = useState('idle');
  const [routeQuote, setRouteQuote] = useState(null);
  const [navswapReadiness, setNavswapReadiness] = useState(null);
  const [activeRunId, setActiveRunId] = useState(null);
  const [runStatus, setRunStatus] = useState(null);
  const [runEvents, setRunEvents] = useState([]);
  const [runReceipts, setRunReceipts] = useState([]);
  const [atomicCounterparty, setAtomicCounterparty] = useState('');
  const [atomicReceiveAmount, setAtomicReceiveAmount] = useState('');
  const [atomicCancelAfter, setAtomicCancelAfter] = useState('');
  const [atomicCondition, setAtomicCondition] = useState(() => randomCondition());
  const [atomicCreateSubmit, setAtomicCreateSubmit] = useState(null);
  const [atomicFinishSubmit, setAtomicFinishSubmit] = useState(null);
  const [atomicCancelSubmit, setAtomicCancelSubmit] = useState(null);
  const [navswapActionSubmit, setNavswapActionSubmit] = useState(null);
  const [assetBalancesLoaded, setAssetBalancesLoaded] = useState(false);
  const [readinessRefreshing, setReadinessRefreshing] = useState(false);
  const [navswapFundingSubmit, setNavswapFundingSubmit] = useState(null);
  const [shieldedIngressSubmit, setShieldedIngressSubmit] = useState(null);
  const [shieldedSwapSubmit, setShieldedSwapSubmit] = useState(null);
  const [shieldedEgressSubmit, setShieldedEgressSubmit] = useState(null);
  const [shieldedNotes, setShieldedNotes] = useState([]);
  const [selectedEgressNoteId, setSelectedEgressNoteId] = useState('');
  const [egressDisclosureAck, setEgressDisclosureAck] = useState(false);
  const [quoteNowMs, setQuoteNowMs] = useState(() => Date.now());
  const autoReadinessRefreshRef = useRef({ signature: '', timer: null });
  const readinessFeedRefreshRef = useRef({ signature: '', timer: null });
  const quoteExpiryRefreshRef = useRef({ expiresAtMs: null, attempted: false });
  const dismissedRecoveredRunIdsRef = useRef(new Set());

  const refreshAssetBalances = useCallback(async () => {
    if (!rpc || !address) return null;
    const resp = await rpc.accountAssets(address);
    if (resp.ok && resp.result) {
      setAssetBalances(accountAssetBalanceMap(resp.result));
      setAssetBalancesLoaded(true);
      return resp.result;
    }
    return null;
  }, [rpc, address]);

  const refreshShieldedNotes = useCallback(async () => {
    try {
      const prover = new LocalAssetOrchardProverClient();
      const notes = await prover.listNotes();
      setShieldedNotes(notes);
      return notes;
    } catch (_) {
      setShieldedNotes([]);
      return [];
    }
  }, []);

  useEffect(() => {
    setAssetBalances({});
    setAssetBalancesLoaded(false);
  }, [address]);

  useEffect(() => {
    const fetchAssets = async () => {
      if (!rpc || !address) return;
      try {
        await refreshAssetBalances();
      } catch (e) { /* no assets */ }
    };
    fetchAssets();
  }, [rpc, address, refreshAssetBalances]);

  useEffect(() => {
    if (!liveSnapshot) return;
    if (liveSnapshot.address && address && liveSnapshot.address.toLowerCase() !== address.toLowerCase()) return;
    if (liveSnapshot.assets) {
      setAssetBalances(accountAssetBalanceMap(liveSnapshot.assets));
      setAssetBalancesLoaded(true);
    }
  }, [liveSnapshot, address]);

  useEffect(() => {
    let disposed = false;
    let timer = null;
    let firstLoad = true;
    const loadCapabilities = async () => {
      if (!swapServer) {
        setNavswapCaps(fallbackNavswapCapabilities('Swap adapter is not configured'));
        setNavswapStatus('unavailable');
        return;
      }
      if (firstLoad) setNavswapStatus('loading');
      try {
        const caps = normalizeShieldedCapabilitiesEnvelope(await swapServer.getNavswapCapabilities());
        if (disposed) return;
        setNavswapCaps(caps);
        setNavswapStatus('ready');
      } catch (e) {
        if (disposed) return;
        setNavswapCaps(fallbackNavswapCapabilities(e.message));
        setNavswapStatus('unavailable');
      } finally {
        firstLoad = false;
      }
    };
    loadCapabilities();
    if (swapServer) timer = setInterval(loadCapabilities, 10000);
    return () => {
      disposed = true;
      if (timer) clearInterval(timer);
    };
  }, [swapServer]);

  useEffect(() => {
    if (!swapServer || !activeRunId) return undefined;
    let disposed = false;
    let timer = null;
    let stream = null;
    let terminalHandled = false;
    const clearPollTimer = () => {
      if (timer) {
        clearInterval(timer);
        timer = null;
      }
    };
    const applyRunPayload = (status, events, receipts, streamTerminal = false) => {
      if (disposed || !status) return false;
      setRunStatus(status);
      if (Array.isArray(events?.events)) setRunEvents(events.events);
      if (Array.isArray(receipts?.receipts)) setRunReceipts(receipts.receipts);
      if (transparentNavswapRunIsTerminal(status, streamTerminal)) {
        clearPollTimer();
        if (stream) {
          stream.close();
          stream = null;
        }
        setActiveRunId(current => transparentNavswapActiveRunIdAfterStatus(current, status, streamTerminal));
        if (!terminalHandled) {
          terminalHandled = true;
          void refreshAssetBalances().catch(() => {});
          if (status.ok === false) {
            setError(status.message || 'NAVSwap run failed');
            setPhase('idle');
          } else {
            setSuccess(status.message || 'NAVSwap run completed');
            setPhase('done');
          }
        }
        return true;
      }
      return false;
    };
    const loadRun = async () => {
      try {
        const [status, events] = await Promise.all([
          swapServer.getNavswapRun(activeRunId),
          swapServer.getNavswapRunEvents(activeRunId),
        ]);
        let receipts = null;
        try {
          receipts = await swapServer.getNavswapRunReceipts(activeRunId);
        } catch (_) {
          receipts = null;
        }
        if (disposed) return;
        applyRunPayload(status, events, receipts);
      } catch (e) {
        if (!disposed) setError(e.message || 'NAVSwap run status unavailable');
      }
    };
    const startPolling = (delayMs = 3000) => {
      if (disposed || timer) return;
      timer = setInterval(loadRun, delayMs);
    };
    const handleStreamEvent = (event) => {
      try {
        const payload = JSON.parse(event.data);
        applyRunPayload(
          payload.status,
          { events: Array.isArray(payload.events) ? payload.events : [] },
          { receipts: Array.isArray(payload.receipts) ? payload.receipts : [] },
          payload.terminal === true,
        );
      } catch (_) {
        startPolling();
      }
    };

    loadRun();
    if (typeof EventSource !== 'undefined' && typeof swapServer.navswapRunStreamUrl === 'function') {
      try {
        stream = new EventSource(swapServer.navswapRunStreamUrl(activeRunId));
        stream.addEventListener('navswap_run_snapshot', handleStreamEvent);
        stream.addEventListener('navswap_run_update', handleStreamEvent);
        stream.addEventListener('navswap_run_done', handleStreamEvent);
        stream.onerror = () => {
          if (!disposed && !terminalHandled) startPolling(5000);
        };
      } catch (_) {
        startPolling();
      }
    } else {
      startPolling();
    }
    return () => {
      disposed = true;
      clearPollTimer();
      if (stream) stream.close();
    };
  }, [swapServer, activeRunId, refreshAssetBalances]);

  useEffect(() => {
    if (!swapServer || !address || activeRunId || route !== 'transparent_navswap' || phase !== 'idle') return undefined;
    if (typeof swapServer.getNavswapRuns !== 'function') return undefined;
    let disposed = false;
    let timer = null;
    const applyRecoveredRun = async (run) => {
      const recovered = transparentNavswapRecoveredRunState({
        run,
        dismissedRunIds: dismissedRecoveredRunIdsRef.current,
      });
      if (!recovered) return false;
      let events = null;
      let receipts = null;
      if (!recovered.activeRunId && run.run_id) {
        [events, receipts] = await Promise.all([
          swapServer.getNavswapRunEvents(run.run_id).catch(() => null),
          swapServer.getNavswapRunReceipts(run.run_id).catch(() => null),
        ]);
      }
      if (disposed) return true;
      setActiveRunId(recovered.activeRunId);
      setRunStatus(run);
      setRunEvents(Array.isArray(events?.events) ? events.events : []);
      setRunReceipts(Array.isArray(receipts?.receipts) ? receipts.receipts : []);
      if (run.quote) setRouteQuote(run.quote);
      setPhase(recovered.phase);
      setSuccess(recovered.message);
      return true;
    };
    const recoverActiveRun = async () => {
      try {
        const list = await swapServer.getNavswapRuns({
          walletAddress: address,
          route: 'transparent_navswap',
          limit: 1,
        });
        if (disposed) return;
        const run = list?.latest_run || (Array.isArray(list?.runs) ? list.runs[0] : null);
        if (await applyRecoveredRun(run)) return;
        const terminalList = await swapServer.getNavswapRuns({
          walletAddress: address,
          route: 'transparent_navswap',
          includeTerminal: true,
          limit: 1,
        });
        if (disposed) return;
        const terminalRun = terminalList?.latest_run || (Array.isArray(terminalList?.runs) ? terminalList.runs[0] : null);
        await applyRecoveredRun(terminalRun);
      } catch (_) {
        // Recovery is best-effort; direct quote/signing must remain available.
      }
    };
    recoverActiveRun();
    timer = setInterval(recoverActiveRun, 10000);
    return () => {
      disposed = true;
      if (timer) clearInterval(timer);
    };
  }, [activeRunId, address, phase, route, swapServer]);

  const resetRouteState = useCallback(() => {
    const autoState = autoReadinessRefreshRef.current;
    autoState.signature = '';
    if (autoState.timer) {
      clearTimeout(autoState.timer);
      autoState.timer = null;
    }
    setRouteQuote(null);
    setNavswapReadiness(null);
    setActiveRunId(null);
    setRunStatus(null);
    setRunEvents([]);
    setRunReceipts([]);
    setAtomicCreateSubmit(null);
    setAtomicFinishSubmit(null);
    setAtomicCancelSubmit(null);
    setNavswapActionSubmit(null);
    setNavswapFundingSubmit(null);
    setShieldedIngressSubmit(null);
    setShieldedEgressSubmit(null);
    setEgressDisclosureAck(false);
    setReadinessRefreshing(false);
    setQuoteNowMs(Date.now());
    setSuccess('');
    setError('');
  }, []);

  const r = ROUTES[route];
  const isTransparentRoute = route === 'transparent_navswap';
  const isPftlUniswapRoute = route === PFTL_UNISWAP_BETA_ROUTE;
  const isShieldedRoute = route === SHIELDED_NAVSWAP_ROUTE;
  const rawRouteCapability = navswapCaps?.routes?.[route] || null;
  const shieldedRouteCapability = isShieldedRoute
    ? normalizeShieldedNavswapCapability(rawRouteCapability || {})
    : null;
  const routeCapability = shieldedRouteCapability || rawRouteCapability;
  const pftlUniswapBetaPolicy = route === PFTL_UNISWAP_BETA_ROUTE
    ? evaluatePftlUniswapBetaRoute({ routeCapability })
    : null;
  const routePolicyOk = pftlUniswapBetaPolicy ? pftlUniswapBetaPolicy.ok : true;
  const transparentRouteCapability = navswapCaps?.routes?.transparent_navswap || null;
  const shieldedAssetSymbols = (shieldedRouteCapability?.asset_registry || [])
    .filter(asset => asset.ok)
    .map(asset => asset.symbol)
    .filter(Boolean);
  const assets = isShieldedRoute && shieldedAssetSymbols.length
    ? Array.from(new Set([...BASE_SWAP_ASSETS, ...shieldedAssetSymbols]))
    : BASE_SWAP_ASSETS;
  const transparentPair = transparentNavswapPairFromCapability(transparentRouteCapability, assets);
  const transparentSubscribePair = from === transparentPair.from && to === transparentPair.to;
  const transparentRedeemPair = from === transparentPair.to && to === transparentPair.from;
  const transparentDirection = transparentRedeemPair ? 'redeem' : 'subscribe';
  const shieldedSelectedPair = isShieldedRoute
    ? (shieldedRouteCapability?.supported_pairs || []).find(pair => pair.ok && pair.from === from && pair.to === to)
    : null;
  const shieldedFromAsset = isShieldedRoute
    ? (shieldedRouteCapability?.asset_registry || []).find(asset => asset.symbol === from)
    : null;
  const shieldedToAsset = isShieldedRoute
    ? (shieldedRouteCapability?.asset_registry || []).find(asset => asset.symbol === to)
    : null;
  const shieldedAssetsSupported = Boolean(
    shieldedFromAsset?.ok
    && shieldedFromAsset.supported !== false
    && shieldedToAsset?.ok
    && shieldedToAsset.supported !== false,
  );
  const shieldedKnownPair = from === 'a651' && to === 'a652';
  const shieldedAdapterCanQuote = shieldedRouteCapability?.can_quote === true
    || shieldedRouteCapability?.adapter_can_quote === true
    || shieldedRouteCapability?.can_run === true;
  const shieldedPairSupported = Boolean(shieldedSelectedPair) || shieldedAssetsSupported || shieldedKnownPair;
  const shieldedCanQuote = isShieldedRoute && shieldedAdapterCanQuote && shieldedPairSupported;
  const routeCanQuote = isShieldedRoute ? shieldedCanQuote : routeCapability?.can_quote === true && routePolicyOk;
  const routeCanRun = !isShieldedRoute && routeCapability?.can_run === true && routePolicyOk;
  const routeStatus = isShieldedRoute
    ? (shieldedRouteCapability?.status || (shieldedRouteCapability?.can_ingress ? 'step5_ingress_ready' : 'preflight_only'))
    : pftlUniswapBetaPolicy?.status || routeCapability?.status || 'unknown';
  const routeReason = pftlUniswapBetaPolicy && !pftlUniswapBetaPolicy.ok
    ? pftlUniswapBetaPolicy.message
    : isShieldedRoute
      ? (shieldedRouteCapability?.disabled_reason || shieldedRouteCapability?.reason || r.why)
      : routeCapability?.reason || r.why;
  const routePrivacy = routeCapability?.privacy || null;
  const shieldedIngressCapability = isShieldedRoute ? (shieldedRouteCapability?.ingress || null) : null;
  const shieldedEgressCapability = isShieldedRoute ? (shieldedRouteCapability?.egress || null) : null;
  const shieldedIngressAssets = Array.isArray(shieldedIngressCapability?.supported_assets)
    ? shieldedIngressCapability.supported_assets
    : [];
  const shieldedIngressAsset = shieldedIngressAssets.find(asset => asset.symbol === from)
    || (from === 'a651' ? { symbol: 'a651', asset_id: A651_ASSET_ID, precision: 6 } : null);
  const shieldedLiquidityModeLabel = shieldedRouteCapability?.quote?.liquidity?.mode_label
    || shieldedRouteCapability?.quote?.raw?.liquidity_mode_label
    || shieldedRouteCapability?.liquidity_mode
    || 'configuration required';
  const shieldedEgressPolicyId = shieldedEgressCapability?.policy_id || SHIELDED_NAVSWAP_EGRESS_POLICY_ID;
  const shieldedSpendableNotes = isShieldedRoute
    ? (shieldedNotes || []).filter(note => (
      note?.state === 'spendable'
      && (!address || String(note.wallet_address || '').toLowerCase() === String(address || '').toLowerCase())
      && (!shieldedFromAsset?.asset_id || String(note.asset_id || '').toLowerCase() === shieldedFromAsset.asset_id)
    ))
    : [];
  const selectedEgressNote = shieldedSpendableNotes.find(note => note.id === selectedEgressNoteId)
    || shieldedSpendableNotes[0]
    || null;

  useEffect(() => {
    if (!isShieldedRoute) return undefined;
    let disposed = false;
    const load = async () => {
      const notes = await refreshShieldedNotes();
      if (disposed) return;
      const spendable = notes.filter(note => (
        note?.state === 'spendable'
        && (!address || String(note.wallet_address || '').toLowerCase() === String(address || '').toLowerCase())
        && (!shieldedFromAsset?.asset_id || String(note.asset_id || '').toLowerCase() === shieldedFromAsset.asset_id)
      ));
      if (!selectedEgressNoteId || !spendable.some(note => note.id === selectedEgressNoteId)) {
        setSelectedEgressNoteId(spendable[0]?.id || '');
      }
    };
    load();
    const timer = setInterval(load, 15000);
    return () => {
      disposed = true;
      clearInterval(timer);
    };
  }, [address, isShieldedRoute, refreshShieldedNotes, selectedEgressNoteId, shieldedFromAsset?.asset_id]);
  const routeVisibilityLabel = routePrivacy?.label || r.vis;
  const routeDisclosureLabel = routePrivacy?.disclosure_label || null;
  const assetFeedStatus = walletFeedStatus?.status || 'idle';
  const assetFeedLabel = assetFeedStatus === 'live'
    ? 'live'
    : assetFeedStatus === 'connecting'
      ? 'syncing'
      : assetFeedStatus === 'error'
        ? 'unavailable'
        : assetFeedStatus;

  const setPair = (f, t) => {
    if (route === 'transparent_navswap') return;
    setFrom(f);
    setTo(t);
    resetRouteState();
  };

  const setTransparentDirection = (direction) => {
    if (route !== 'transparent_navswap') return;
    if (direction === 'redeem') {
      setFrom(transparentPair.to);
      setTo(transparentPair.from);
    } else {
      setFrom(transparentPair.from);
      setTo(transparentPair.to);
    }
    setAmt(current => current || DEFAULT_TRANSPARENT_NAVSWAP_AMOUNT);
    resetRouteState();
  };

  const selectRoute = (id) => {
    if (id === 'transparent_navswap') {
      setFrom(transparentPair.from);
      setTo(transparentPair.to);
      setAmt(current => current || DEFAULT_TRANSPARENT_NAVSWAP_AMOUNT);
    } else if (id === PFTL_UNISWAP_BETA_ROUTE) {
      setFrom('pfUSDC');
      setTo('a651');
      setAmt(current => current || DEFAULT_TRANSPARENT_NAVSWAP_AMOUNT);
    } else if (id === SHIELDED_NAVSWAP_ROUTE) {
      const pair = shieldedRouteCapability?.supported_pairs?.find(item => item.ok)
        || { from: 'a651', to: 'a652' };
      setFrom(pair.from);
      setTo(pair.to);
      setAmt(current => current || DEFAULT_TRANSPARENT_NAVSWAP_AMOUNT);
    }
    setRoute(id);
    resetRouteState();
  };

  useEffect(() => {
    if (route !== 'transparent_navswap') return;
    if (transparentSubscribePair || transparentRedeemPair) return;
    setFrom(transparentPair.from);
    setTo(transparentPair.to);
    resetRouteState();
  }, [
    from,
    resetRouteState,
    route,
    to,
    transparentPair.from,
    transparentPair.to,
    transparentRedeemPair,
    transparentSubscribePair,
  ]);

  useEffect(() => {
    if (route !== SHIELDED_NAVSWAP_ROUTE) return;
    const supportedPairs = shieldedRouteCapability?.supported_pairs || [];
    const currentPairSupported = supportedPairs.some(item => item.ok && item.from === from && item.to === to);
    if (currentPairSupported) return;
    const pair = supportedPairs.find(item => item.ok);
    if (!pair) return;
    setFrom(pair.from);
    setTo(pair.to);
    resetRouteState();
  }, [
    from,
    resetRouteState,
    route,
    shieldedRouteCapability?.supported_pairs,
    to,
  ]);

  const preparedActionStages = Array.isArray(routeCapability?.prepared_action_stages) ? routeCapability.prepared_action_stages : [];
  const routePreflight = routeCapability?.preflight || routeQuote?.stakehub_preflight || null;
  const transparentRoundtrip = routePreflight?.swap_status?.transparent_roundtrip || null;
  const latestIncompleteRun = transparentRoundtrip?.latest_incomplete_run || null;
  const latestTransportRound = latestIncompleteRun?.latest_transport_round || null;
  const latestProposalLock = transparentRoundtrip?.latest_proposal_vote_lock || null;
  const latestRun = runStatus || (routeQuote?.run_id ? routeQuote : null);
  const actionComplete = phase === 'done' || latestRun?.ok === true;
  const canStartFreshTransparentQuote = transparentNavswapCanStartFreshQuote({
    route,
    phase,
    status: latestRun,
  });
  const completedResult = navswapReceiptResult(runReceipts, latestRun);
  const completedVerification = completedResult?.receipt_verification || completedResult?.receiptVerification || null;
  const completedReport = completedResult?.report || null;
  const completedSummaryFile = completedResult?.summary_file || completedReport?.artifact_file || null;
  const completedRunDir = completedResult?.run_dir || completedReport?.artifact_dir || null;
  const completedBridgeResume = completedResult?.bridge_out_resume_file || completedResult?.bridge_out_resume || null;
  const completedPrimaryReceipt = completedReport?.primary_mint?.settlement_receipt_id || completedResult?.primary_mint_settlement_receipt_id || null;
  const completedNavExit = completedReport?.nav_exit?.redemption_id || completedResult?.nav_exit_redemption_id || null;
  const completedSummaryOk = completedReport?.final_summary_ok;
  const atomicVerification = routeQuote?.schema === 'postfiat-navswap-atomic-template-v1' ? routeQuote.verification : null;
  const atomicSymmetry = routeQuote?.schema === 'postfiat-navswap-atomic-template-v1' ? routeQuote.symmetry : null;
  const atomicResult = routeQuote?.schema === 'postfiat-navswap-atomic-template-v1' ? routeQuote.result : null;
  let atomicWalletLeg = null;
  let atomicFinishLeg = null;
  let atomicCancelLeg = null;
  let atomicLegError = '';
  try {
    atomicWalletLeg = findAtomicWalletCreateLeg(atomicResult, address);
    atomicFinishLeg = findAtomicWalletFinishLeg(atomicResult, address, atomicResult?.condition || atomicCondition);
    atomicCancelLeg = findAtomicWalletCancelLeg(atomicResult, address);
  } catch (e) {
    atomicLegError = e.message || 'Atomic wallet leg is invalid';
  }
  const atomicTemplateReady = Boolean(
    amt
    && atomicReceiveAmount
    && atomicCounterparty.trim()
    && atomicCancelAfter
    && atomicCondition.trim()
  );
  const receive = isShieldedRoute && routeQuote?.output_amount_atoms
    ? `${formatBalance(routeQuote.output_amount_atoms)} ${to}`
    : routeQuote?.expected_output
      ? `${routeQuote.expected_output} ${to}`
      : `quote required ${to}`;
  const quoteSettlementDisplay = (route === 'transparent_navswap' || isPftlUniswapRoute) && routeQuote?.settlement_amount_atoms
    ? `${formatBalance(routeQuote.settlement_amount_atoms)} ${transparentPair.settlementAsset}`
    : null;
  const walletSpendAtoms = route === 'transparent_navswap'
    ? (transparentDirection === 'redeem'
      ? (routeQuote?.redeem_amount_atoms || routeQuote?.input_amount_atoms || amt)
      : (routeQuote?.settlement_amount_atoms || routeQuote?.input_amount_atoms || null))
    : isPftlUniswapRoute
      ? (routeQuote?.settlement_amount_atoms || routeQuote?.input_amount_atoms || null)
    : null;
  const walletSpendAsset = route === 'transparent_navswap'
    ? (transparentDirection === 'redeem' ? transparentPair.amountAsset : transparentPair.settlementAsset)
    : isPftlUniswapRoute
      ? 'pfUSDC'
    : from;
  const requiredSettlement = (route === 'transparent_navswap' || isPftlUniswapRoute) && walletSpendAtoms
    ? `${walletSpendAsset === 'a651' ? formatSwapBalance(walletSpendAsset, walletSpendAtoms) : formatBalance(walletSpendAtoms)} ${walletSpendAsset}`
    : null;
  const operatorCompletion = routeQuote?.operator_completion || null;
  const preparedActionBatch = routeQuote?.prepared_action_batch || null;
  const preparedBatchActions = Array.isArray(preparedActionBatch?.actions) ? preparedActionBatch.actions : [];
  const preparedBatchStages = preparedBatchActions
    .map(action => action?.stage)
    .filter(Boolean);
  const preparedBatchStageLabel = preparedBatchStages.length ? preparedBatchStages.join(', ') : null;
  const quoteFreshness = routeQuote ? transparentNavswapQuoteFreshness(routeQuote, quoteNowMs) : null;
  const quoteFreshnessLabel = formatQuoteFreshnessLabel(quoteFreshness);
  const readinessNextStep = navswapReadiness?.next_steps?.[0] || null;
  const readinessSettlement = navswapReadiness?.wallet_spend_asset || navswapReadiness?.settlement_asset || null;
  const readinessFunding = navswapReadiness?.funding || null;
  const readinessPftFees = transparentNavswapPftFeeStatus(navswapReadiness || routeQuote);
  const fundingAvailable = readinessFunding?.available === true;
  const fundingRunning = navswapFundingSubmit?.status === 'running';
  const navswapPrimaryStep = transparentNavswapPrimaryStep({
    readiness: navswapReadiness,
    preparedActionCount: preparedBatchActions.length,
    fundingAvailable,
    quoteFreshness,
  });
  const routeAssetForSymbol = (symbol) => {
    if (symbol === 'pfUSDC') return { symbol, asset_id: PFUSDC_ASSET_ID, precision: 6 };
    if (symbol === 'a651') return { symbol, asset_id: A651_ASSET_ID, precision: 6 };
    return (shieldedRouteCapability?.asset_registry || []).find(asset => asset.symbol === symbol) || null;
  };
  const balanceForSymbol = (symbol) => {
    if (assetBalances[symbol] !== undefined) {
      return { known: true, atoms: assetBalances[symbol] };
    }
    const asset = routeAssetForSymbol(symbol);
    const shortId = shortenAssetId(asset?.asset_id);
    if (shortId && assetBalances[shortId] !== undefined) {
      return { known: true, atoms: assetBalances[shortId] };
    }
    const issued = Boolean(asset) || symbol === 'pfUSDC' || symbol === 'a651';
    return { known: issued && assetBalancesLoaded, atoms: 0 };
  };
  const fromBalance = balanceForSymbol(from);
  const fromIsIssuedAsset = Boolean(routeAssetForSymbol(from));
  const fromBalanceKnown = fromBalance.known;
  const fromBalanceAtoms = fromBalance.atoms;
  const toBalance = balanceForSymbol(to);
  const toIsIssuedAsset = Boolean(routeAssetForSymbol(to));
  const toBalanceKnown = toBalance.known;
  const toBalanceAtoms = toBalance.atoms;
  const transparentAmountMode = route === 'transparent_navswap';
  const pftlUniswapAmountMode = isPftlUniswapRoute;
  const transparentPairTitle = transparentDirection === 'redeem'
    ? `Redeem ${transparentPair.amountAsset} back into ${transparentPair.settlementAsset}`
    : `Mint ${transparentPair.amountAsset} settled with ${transparentPair.settlementAsset}`;
  const amountFieldAsset = transparentAmountMode || pftlUniswapAmountMode ? transparentPair.amountAsset : from;
  const amountFieldBalanceKnown = transparentAmountMode
    ? (transparentDirection === 'redeem' ? fromBalanceKnown : toBalanceKnown)
    : pftlUniswapAmountMode
      ? toBalanceKnown
    : fromBalanceKnown;
  const amountFieldBalanceAtoms = transparentAmountMode
    ? (transparentDirection === 'redeem' ? fromBalanceAtoms : toBalanceAtoms)
    : pftlUniswapAmountMode
      ? toBalanceAtoms
    : fromBalanceAtoms;
  const settlementFieldAsset = transparentAmountMode || pftlUniswapAmountMode ? transparentPair.settlementAsset : to;
  const settlementDisplayAmount = transparentAmountMode || pftlUniswapAmountMode
    ? (quoteSettlementDisplay ? quoteSettlementDisplay.split(' ')[0] : 'quote')
    : receive.split(' ')[0];
  const settlementBalanceKnown = transparentAmountMode
    ? (transparentDirection === 'redeem' ? toBalanceKnown : fromBalanceKnown)
    : pftlUniswapAmountMode
      ? fromBalanceKnown
    : toBalanceKnown;
  const settlementBalanceAtoms = transparentAmountMode
    ? (transparentDirection === 'redeem' ? toBalanceAtoms : fromBalanceAtoms)
    : pftlUniswapAmountMode
      ? fromBalanceAtoms
    : toBalanceAtoms;
  const batchRunning = navswapActionSubmit?.stage === 'batch' && navswapActionSubmit?.status === 'running';
  const batchSubmitted = navswapActionSubmit?.stage === 'batch' && navswapActionSubmit?.ok === true;
  const pftlSourceSubmitted = isPftlUniswapRoute && batchSubmitted;

  const refreshTransparentReadiness = useCallback(async () => {
    if (route !== 'transparent_navswap' || !swapServer || typeof swapServer.getNavswapReadiness !== 'function') return null;
    if (!address) throw new Error('Wallet address unavailable');
    const parsed = Number.parseFloat(amt);
    if (!Number.isFinite(parsed) || parsed <= 0) throw new Error('Amount must be positive');
    setReadinessRefreshing(true);
    try {
      const readiness = await swapServer.getNavswapReadiness({
        route: 'transparent_navswap',
        from_asset: from,
        to_asset: to,
        direction: transparentDirection,
        amount: amt,
        wallet_address: address,
        auto_plan: true,
      });
      setNavswapReadiness(readiness);
      if (readiness.quote?.ok) {
        setRouteQuote(readiness.quote);
        setQuoteNowMs(Date.now());
      }
      return readiness;
    } finally {
      setReadinessRefreshing(false);
    }
  }, [address, amt, from, route, swapServer, to, transparentDirection]);

  useEffect(() => {
    const signature = transparentNavswapAutoReadinessSignature({
      route,
      routeCanQuote,
      swapServerConfigured: Boolean(swapServer && typeof swapServer.getNavswapReadiness === 'function'),
      address,
      activeRunId,
      routeQuote,
      navswapReadiness,
      phase,
      readinessRefreshing,
      amount: amt,
      from,
      to,
      routeStatus,
    });
    if (!signature) return undefined;
    const state = autoReadinessRefreshRef.current;
    if (state.signature === signature) return undefined;
    if (state.timer) clearTimeout(state.timer);
    state.signature = signature;
    state.timer = setTimeout(() => {
      state.timer = null;
      refreshTransparentReadiness()
        .then(readiness => {
          if (readiness?.quote?.ok) {
            setPhase(current => (current === 'idle' ? 'quoted' : current));
          }
        })
        .catch(() => {});
    }, 250);
    return () => {
      if (state.timer) {
        clearTimeout(state.timer);
        state.timer = null;
      }
    };
  }, [
    activeRunId,
    address,
    amt,
    from,
    navswapReadiness,
    phase,
    readinessRefreshing,
    refreshTransparentReadiness,
    route,
    routeCanQuote,
    routeQuote,
    routeStatus,
    swapServer,
    to,
  ]);

  useEffect(() => {
    const state = readinessFeedRefreshRef.current;
    state.signature = '';
    if (state.timer) {
      clearTimeout(state.timer);
      state.timer = null;
    }
  }, [address, route, from, to, amt]);

  useEffect(() => () => {
    const autoState = autoReadinessRefreshRef.current;
    if (autoState.timer) {
      clearTimeout(autoState.timer);
      autoState.timer = null;
    }
    const state = readinessFeedRefreshRef.current;
    if (state.timer) {
      clearTimeout(state.timer);
      state.timer = null;
    }
  }, []);

  useEffect(() => {
    if (route !== 'transparent_navswap') return;
    if (!navswapReadiness || !routeQuote) return;
    if (!liveSnapshot?.assets) return;
    if (actionComplete) return;
    if (liveSnapshot.address && address && liveSnapshot.address.toLowerCase() !== address.toLowerCase()) return;
    if (readinessRefreshing || fundingRunning || batchRunning) return;

    const balances = accountAssetBalanceMap(liveSnapshot.assets);
    const signature = [
      balances.pfUSDC ?? 0,
      balances.a651 ?? 0,
    ].join(':');
    const state = readinessFeedRefreshRef.current;
    if (state.signature === signature) return;
    state.signature = signature;
    if (state.timer) clearTimeout(state.timer);
    state.timer = setTimeout(() => {
      state.timer = null;
      refreshTransparentReadiness().catch(() => {});
    }, 500);
  }, [
    address,
    batchRunning,
    fundingRunning,
    liveSnapshot,
    navswapReadiness,
    readinessRefreshing,
    refreshTransparentReadiness,
    route,
    routeQuote,
    actionComplete,
  ]);

  useEffect(() => {
    if (route !== 'transparent_navswap' || !quoteFreshness?.expiresAtMs) return undefined;
    const expiresInMs = quoteFreshness.expiresAtMs - Date.now();
    const delayMs = Math.max(1000, Math.min(30000, expiresInMs + 500));
    const timer = setTimeout(() => setQuoteNowMs(Date.now()), delayMs);
    return () => clearTimeout(timer);
  }, [quoteFreshness?.expiresAtMs, quoteNowMs, route]);

  useEffect(() => {
    if (route !== 'transparent_navswap' || !routeQuote || !quoteFreshness?.expiresAtMs) return;
    const state = quoteExpiryRefreshRef.current;
    if (state.expiresAtMs !== quoteFreshness.expiresAtMs) {
      state.expiresAtMs = quoteFreshness.expiresAtMs;
      state.attempted = false;
    }
    if (!quoteFreshness.expired) return;
    if (state.attempted) return;
    if (readinessRefreshing || fundingRunning || batchRunning) return;
    state.attempted = true;
    refreshTransparentReadiness().catch(() => {});
  }, [
    batchRunning,
    fundingRunning,
    quoteFreshness?.expired,
    quoteFreshness?.expiresAtMs,
    readinessRefreshing,
    refreshTransparentReadiness,
    route,
    routeQuote,
  ]);

  // --- NAVSwap adapter quote ---
  const handleNavswapQuote = async () => {
    setError(''); setSuccess('');
    const parsed = Number.parseFloat(amt);
    if (!Number.isFinite(parsed) || parsed <= 0) { setError('Amount must be positive'); return; }
    if (!swapServer) { setError('Swap adapter is not configured'); return; }
    if (!routeCanQuote) { setError(routeReason); return; }

    setPhase('running');
    try {
      const request = {
        route,
        from_asset: from,
        to_asset: to,
        direction: transparentDirection,
        amount: amt,
        wallet_address: address,
        ...(route === 'transparent_navswap' ? { auto_plan: true } : {}),
      };
      let quote;
      if (route === 'transparent_navswap' && typeof swapServer.getNavswapReadiness === 'function') {
        const readiness = await refreshTransparentReadiness();
        quote = readiness.quote;
        if (!quote?.ok) {
          setRouteQuote(quote || null);
          setError(readiness.next_steps?.[0] || readiness.message || quote?.message || 'NAVSwap readiness check failed');
          setPhase('idle');
          return;
        }
      } else {
        quote = await swapServer.quoteNavswap(request);
        setNavswapReadiness(null);
      }
      setRouteQuote(quote);
      setSuccess(quote.message || 'Route quote prepared');
      onToast('Route quote prepared');
      setPhase('quoted');
    } catch (e) {
      setRouteQuote(null);
      setNavswapReadiness(null);
      setError(e.data?.message || e.message || 'NAVSwap quote failed');
      setPhase('idle');
    }
  };

  const handleShieldedQuote = async () => {
    setError(''); setSuccess('');
    if (!swapServer || typeof swapServer.getShieldedNavswapQuote !== 'function') {
      setError('Shielded quote adapter is not configured');
      return;
    }
    if (!address) { setError('Wallet address unavailable'); return; }
    if (!shieldedCanQuote) {
      setError(routeReason || 'Shielded quote requires a configured a651 ↔ a652 pair and live liquidity commitment');
      return;
    }
    let amountAtoms;
    try {
      amountAtoms = decimalToAtomsString(amt, shieldedFromAsset?.precision || 6);
    } catch (e) {
      setError(e.message || 'Invalid shielded quote amount');
      return;
    }
    setPhase('running');
    try {
      const quote = await swapServer.getShieldedNavswapQuote({
        route: SHIELDED_NAVSWAP_ROUTE,
        wallet_address: address,
        from_asset: from,
        to_asset: to,
        from_asset_id: shieldedFromAsset?.asset_id,
        to_asset_id: shieldedToAsset?.asset_id,
        amount_atoms: amountAtoms,
      });
      const normalized = normalizeShieldedNavswapQuote(quote, Date.now());
      setRouteQuote(quote);
      setNavswapReadiness(null);
      setQuoteNowMs(Date.now());
      if (!quote?.ok || !normalized.ready) {
        setError(quote?.message || `Shielded quote is not usable: ${normalized.missing.join(', ') || 'unknown issue'}`);
        setPhase('idle');
        return;
      }
      setSuccess(normalized.submit_enabled ? 'Private quote loaded. Ready to build and submit the local swap proof.' : 'Private quote preview loaded. Submit remains locked until Step 7.');
      onToast?.('Private quote preview loaded');
      setPhase('quoted');
    } catch (e) {
      setRouteQuote(null);
      setNavswapReadiness(null);
      setError(e.data?.message || e.message || 'Shielded quote failed');
      setPhase('idle');
    }
  };

  const handleNavswapRun = async () => {
    setError(''); setSuccess('');
    const parsed = Number.parseFloat(amt);
    if (!Number.isFinite(parsed) || parsed <= 0) { setError('Amount must be positive'); return; }
    if (!swapServer) { setError('Swap adapter is not configured'); return; }
    if (!routeCanRun) { setError(routeReason); return; }

    setPhase('running');
    try {
      const runResp = await swapServer.runNavswap({
        route,
        from_asset: from,
        to_asset: to,
        amount: amt,
        wallet_address: address,
        async: true,
      });
      setRouteQuote(runResp);
      setRunStatus(runResp);
      if (runResp.run_id) {
        setActiveRunId(runResp.run_id);
        setSuccess(runResp.message || 'Route run accepted');
      } else {
        setSuccess(runResp.message || 'Route run submitted');
        setPhase('quoted');
      }
      onToast('Route run submitted');
    } catch (e) {
      setRouteQuote(null);
      setRunStatus(null);
      setRunEvents([]);
      setRunReceipts([]);
      setError(e.data?.message || e.message || 'NAVSwap run failed');
      setPhase('idle');
    }
  };

  const handleNavswapFundingRequest = async ({ readinessOverride = null } = {}) => {
    setError(''); setSuccess('');
    const funding = readinessOverride?.funding || readinessFunding;
    const fundingIsAvailable = funding?.available === true;
    if (!swapServer || typeof swapServer.fundNavswapPfusdc !== 'function') {
      setError('NAVSwap pfUSDC funding helper is unavailable');
      return;
    }
    if (!address) { setError('Wallet address unavailable'); return; }
    if (!fundingIsAvailable) {
      setError(funding?.unavailable_reason || 'NAVSwap pfUSDC funding is not available for this quote');
      return;
    }

    setNavswapFundingSubmit({
      status: 'running',
      amountAtoms: funding.amount_atoms,
    });
    try {
      const result = await swapServer.fundNavswapPfusdc({
        route: 'transparent_navswap',
        from_asset: from,
        to_asset: to,
        amount: amt,
        wallet_address: address,
        amount_atoms: funding.amount_atoms,
      });
      setNavswapFundingSubmit({
        status: 'submitted',
        ok: true,
        amountAtoms: result.amount_atoms || funding.amount_atoms,
        txId: result.tx_id,
        result,
      });
      await refreshAssetBalances().catch(() => {});
      await refreshTransparentReadiness().catch(() => {});
      setSuccess('pfUSDC settlement funding submitted');
      onToast?.('pfUSDC settlement funding submitted');
    } catch (e) {
      setNavswapFundingSubmit({
        status: 'failed',
        ok: false,
        amountAtoms: funding?.amount_atoms,
        message: e.data?.message || e.message || 'pfUSDC funding failed',
      });
      setError(e.data?.message || e.message || 'pfUSDC funding failed');
    }
  };

  const handlePreparedActionBatchSubmit = async () => {
    setError(''); setSuccess('');
    if (!backupJson) { setError('Wallet must be unlocked to sign NAVSwap actions'); return; }
    if (!txBuilder) { setError('Wallet transaction builder is unavailable'); return; }
    if (!address) { setError('Wallet address unavailable'); return; }
    let quoteForSubmit = routeQuote;
    let batchForSubmit = quoteForSubmit?.prepared_action_batch || null;
    let actionsForSubmit = Array.isArray(batchForSubmit?.actions) ? batchForSubmit.actions : [];
    let stagesForSubmit = actionsForSubmit.map(action => action?.stage).filter(Boolean);
    let operatorCompletionForSubmit = quoteForSubmit?.operator_completion || null;
    if (transparentNavswapQuoteFreshness(quoteForSubmit).expired) {
      if (route !== 'transparent_navswap') {
        setError('NAVSwap quote expired; refresh the quote before signing.');
        return;
      }
      setSuccess('NAVSwap quote expired; refreshing readiness before signing.');
      const readiness = await refreshTransparentReadiness().catch(e => {
        setError(e.data?.message || e.message || 'NAVSwap readiness refresh failed');
        return null;
      });
      if (!readiness?.quote?.ok) {
        setError(readiness?.next_steps?.[0] || readiness?.message || readiness?.quote?.message || 'NAVSwap readiness refresh failed');
        return;
      }
      quoteForSubmit = readiness.quote;
      batchForSubmit = quoteForSubmit?.prepared_action_batch || null;
      actionsForSubmit = Array.isArray(batchForSubmit?.actions) ? batchForSubmit.actions : [];
      stagesForSubmit = actionsForSubmit.map(action => action?.stage).filter(Boolean);
      operatorCompletionForSubmit = quoteForSubmit?.operator_completion || null;
      if (transparentNavswapQuoteFreshness(quoteForSubmit).expired) {
        setError('NAVSwap quote is still expired after readiness refresh');
        return;
      }
    }
    if (!actionsForSubmit.length) { setError('No prepared NAVSwap action batch is available'); return; }
    const submitSpendAsset = quoteForSubmit?.direction === 'redeem' ? transparentPair.amountAsset : transparentPair.settlementAsset;
    const submitSpendAtoms = quoteForSubmit?.direction === 'redeem'
      ? (quoteForSubmit.redeem_amount_atoms || quoteForSubmit.input_amount_atoms)
      : quoteForSubmit.settlement_amount_atoms;
    if (submitSpendAtoms) {
      if (fromIsIssuedAsset && !fromBalanceKnown) {
        setError(`${from} balance is not loaded yet. Wait for the asset feed and retry.`);
        return;
      }
      try {
        const required = BigInt(submitSpendAtoms);
        const available = BigInt(fromBalanceAtoms);
        if (available < required) {
          const formatSpendAmount = value => (submitSpendAsset === 'a651'
            ? formatSwapBalance(submitSpendAsset, value)
            : formatBalance(value));
          setError(`Insufficient ${from}: requires ${formatSpendAmount(required)} ${from}, available ${formatSpendAmount(available)} ${from}`);
          return;
        }
      } catch (_) {
        setError('Wallet spend balance check failed');
        return;
      }
    }

    setNavswapActionSubmit({
      status: 'running',
      stage: 'batch',
      actionCount: actionsForSubmit.length,
      stages: stagesForSubmit,
    });
    try {
      const result = await submitNavswapPreparedAssetActions({
        requests: batchForSubmit,
        walletAddress: address,
        backupJson,
        txBuilder,
        onProgress: event => {
          setNavswapActionSubmit(prev => ({
            ...prev,
            status: event.status === 'failed' ? 'failed' : 'running',
            stage: 'batch',
            actionCount: actionsForSubmit.length,
            completedCount: event.status === 'submitted'
              ? Math.max(prev?.completedCount || 0, event.index + 1)
              : (prev?.completedCount || 0),
            stages: stagesForSubmit,
            currentIndex: event.index,
            currentStage: event.stage,
            lastEvent: event,
            message: event.message,
          }));
        },
      });
      const txIds = result.submissions.map(item => item.txId).filter(Boolean);
      setNavswapActionSubmit({
        status: 'submitted',
        ok: true,
        stage: 'batch',
        actionCount: result.count,
        completedCount: result.count,
        stages: result.actions.map(action => action.stage).filter(Boolean),
        submissions: result.submissions,
        txIds,
      });
      if ((route === 'transparent_navswap' || isPftlUniswapRoute) && operatorCompletionForSubmit?.stage && swapServer && typeof swapServer.runNavswap === 'function') {
        try {
          const runResp = await swapServer.runNavswap({
            route,
            wallet_address: address,
            quote: quoteForSubmit,
            wallet_action_result: result,
            async: true,
          });
          setRunStatus(runResp);
          if (runResp.run_id) {
            setActiveRunId(runResp.run_id);
            setNavswapActionSubmit(prev => ({
              ...prev,
              operatorRunId: runResp.run_id,
              operatorStatus: runResp.status,
              operatorMessage: runResp.message,
            }));
            setSuccess(runResp.message || 'NAVSwap operator completion started');
            onToast?.('NAVSwap operator completion started');
          } else {
            setSuccess(runResp.message || 'NAVSwap wallet actions submitted');
            onToast?.('NAVSwap wallet actions submitted');
          }
        } catch (operatorError) {
          setNavswapActionSubmit(prev => ({
            ...prev,
            operatorStatus: 'failed',
            operatorMessage: operatorError.data?.message || operatorError.message || 'NAVSwap operator completion failed to start',
          }));
          setSuccess('NAVSwap wallet actions submitted');
          setError(operatorError.data?.message || operatorError.message || 'NAVSwap operator completion failed to start');
        }
      } else {
        setSuccess('NAVSwap wallet actions submitted');
        onToast?.('NAVSwap wallet actions submitted');
      }
      void refreshAssetBalances().catch(() => {});
      if (route === 'transparent_navswap') void refreshTransparentReadiness().catch(() => {});
    } catch (e) {
      const partialResults = Array.isArray(e.partial_results) ? e.partial_results : [];
      setNavswapActionSubmit({
        status: 'failed',
        ok: false,
        stage: 'batch',
        actionCount: actionsForSubmit.length,
        completedCount: partialResults.length,
        stages: stagesForSubmit,
        partialResults,
        failedAction: e.failed_action,
        message: e.data?.message || e.message || 'NAVSwap action batch submit failed',
      });
      setError(e.data?.message || e.message || 'NAVSwap action batch submit failed');
    }
  };

  const handleAtomicTemplate = async () => {
    setError(''); setSuccess('');
    const leftAmount = String(amt || '').trim();
    const rightAmount = String(atomicReceiveAmount || '').trim();
    const parsedCancel = Number.parseInt(atomicCancelAfter, 10);
    if (!/^[1-9][0-9]*$/.test(leftAmount)) { setError('Atomic amount must be a positive whole raw unit'); return; }
    if (!/^[1-9][0-9]*$/.test(rightAmount)) { setError('Counter amount must be a positive whole raw unit'); return; }
    if (!Number.isSafeInteger(parsedCancel) || parsedCancel <= 0) { setError('Cancel height must be positive'); return; }
    if (!address) { setError('Wallet address unavailable'); return; }
    if (!atomicCounterparty.trim()) { setError('Counterparty address is required'); return; }
    if (!atomicCondition.trim()) { setError('Condition is required'); return; }
    if (!swapServer) { setError('Swap adapter is not configured'); return; }

    setPhase('running');
    try {
      const template = await swapServer.buildAtomicSettlementTemplate({
        left_owner: address,
        left_recipient: atomicCounterparty.trim(),
        left_asset_id: from,
        left_amount: leftAmount,
        right_owner: atomicCounterparty.trim(),
        right_recipient: address,
        right_asset_id: to,
        right_amount: rightAmount,
        condition: atomicCondition.trim(),
        cancel_after: parsedCancel,
      });
      setRouteQuote(template);
      setAtomicCreateSubmit(null);
      setAtomicFinishSubmit(null);
      setAtomicCancelSubmit(null);
      setSuccess('Atomic template prepared');
      onToast?.('Atomic template prepared');
      setPhase('quoted');
    } catch (e) {
      setRouteQuote(null);
      setError(e.data?.message || e.message || 'Atomic template failed');
      setPhase('idle');
    }
  };

  const handleAtomicCreateSubmit = async () => {
    setError(''); setSuccess('');
    if (!backupJson) { setError('Wallet must be unlocked to sign this escrow leg'); return; }
    if (!txBuilder) { setError('Wallet transaction builder is unavailable'); return; }
    if (!address) { setError('Wallet address unavailable'); return; }
    if (atomicLegError) { setError(atomicLegError); return; }
    if (!atomicWalletLeg) { setError('This template does not contain an escrow-create leg owned by this wallet'); return; }

    setAtomicCreateSubmit({
      status: 'running',
      side: atomicWalletLeg.side,
      escrowId: atomicWalletLeg.escrowId,
    });
    try {
      const result = await txBuilder.sendEscrowTransaction(
        backupJson,
        address,
        { operation: atomicWalletLeg.operation },
        { sequence: atomicWalletLeg.sequence },
      );
      setAtomicCreateSubmit({
        status: 'submitted',
        ok: true,
        side: atomicWalletLeg.side,
        escrowId: atomicWalletLeg.escrowId,
        txId: result.txId,
        receipt: result.receipt,
        sequence: result.quote?.sequence ?? atomicWalletLeg.sequence,
      });
      setSuccess('Escrow-create leg submitted');
      onToast?.('Escrow-create leg submitted');
    } catch (e) {
      setAtomicCreateSubmit({
        status: 'failed',
        ok: false,
        side: atomicWalletLeg.side,
        escrowId: atomicWalletLeg.escrowId,
        message: e.message || 'Escrow-create submit failed',
      });
      setError(e.message || 'Escrow-create submit failed');
    }
  };

  const ensureEscrowOpen = async (escrowId, label) => {
    if (!rpc || typeof rpc.escrowInfo !== 'function') return null;
    const resp = await rpc.escrowInfo(escrowId);
    if (!resp.ok) {
      throw new Error(`${label} escrow lookup failed: ${resp.error?.message || 'unknown'}`);
    }
    const result = resp.result || {};
    if (result.found === false || !result.escrow) {
      throw new Error(`${label} escrow is not on ledger yet`);
    }
    const state = result.escrow.state || result.escrow.status;
    if (state && state !== 'open') {
      throw new Error(`${label} escrow is ${state}`);
    }
    return result.escrow;
  };

  const handleAtomicFinishSubmit = async () => {
    setError(''); setSuccess('');
    if (!backupJson) { setError('Wallet must be unlocked to finish this escrow'); return; }
    if (!txBuilder) { setError('Wallet transaction builder is unavailable'); return; }
    if (!address) { setError('Wallet address unavailable'); return; }
    if (atomicLegError) { setError(atomicLegError); return; }
    if (!atomicWalletLeg || !atomicFinishLeg) { setError('This template does not contain both wallet create and incoming finish legs'); return; }

    setAtomicFinishSubmit({
      status: 'running',
      side: atomicFinishLeg.side,
      escrowId: atomicFinishLeg.escrowId,
    });
    try {
      await ensureEscrowOpen(atomicWalletLeg.escrowId, 'Your create');
      await ensureEscrowOpen(atomicFinishLeg.escrowId, 'Counterparty create');
      const result = await txBuilder.sendEscrowTransaction(
        backupJson,
        address,
        { operation: atomicFinishLeg.operation },
      );
      setAtomicFinishSubmit({
        status: 'submitted',
        ok: true,
        side: atomicFinishLeg.side,
        escrowId: atomicFinishLeg.escrowId,
        txId: result.txId,
        receipt: result.receipt,
        sequence: result.quote?.sequence,
      });
      setSuccess('Escrow-finish submitted');
      onToast?.('Escrow-finish submitted');
    } catch (e) {
      setAtomicFinishSubmit({
        status: 'failed',
        ok: false,
        side: atomicFinishLeg?.side,
        escrowId: atomicFinishLeg?.escrowId,
        message: e.message || 'Escrow-finish submit failed',
      });
      setError(e.message || 'Escrow-finish submit failed');
    }
  };

  const handleAtomicCancelSubmit = async () => {
    setError(''); setSuccess('');
    if (!backupJson) { setError('Wallet must be unlocked to cancel this escrow'); return; }
    if (!txBuilder) { setError('Wallet transaction builder is unavailable'); return; }
    if (!address) { setError('Wallet address unavailable'); return; }
    if (atomicLegError) { setError(atomicLegError); return; }
    if (!atomicCancelLeg) { setError('This template does not contain an escrow-create leg owned by this wallet'); return; }

    setAtomicCancelSubmit({
      status: 'running',
      side: atomicCancelLeg.side,
      escrowId: atomicCancelLeg.escrowId,
    });
    try {
      await ensureEscrowOpen(atomicCancelLeg.escrowId, 'Your create');
      const result = await txBuilder.sendEscrowTransaction(
        backupJson,
        address,
        { operation: atomicCancelLeg.operation },
      );
      setAtomicCancelSubmit({
        status: 'submitted',
        ok: true,
        side: atomicCancelLeg.side,
        escrowId: atomicCancelLeg.escrowId,
        txId: result.txId,
        receipt: result.receipt,
        sequence: result.quote?.sequence,
      });
      setSuccess('Escrow-cancel submitted');
      onToast?.('Escrow-cancel submitted');
    } catch (e) {
      setAtomicCancelSubmit({
        status: 'failed',
        ok: false,
        side: atomicCancelLeg?.side,
        escrowId: atomicCancelLeg?.escrowId,
        message: e.message || 'Escrow-cancel submit failed',
      });
      setError(e.message || 'Escrow-cancel submit failed');
    }
  };

  const rememberRecoveredRunDismissal = () => {
    if (latestRun?.run_id) dismissedRecoveredRunIdsRef.current.add(latestRun.run_id);
  };

  const startFreshTransparentQuote = () => {
    rememberRecoveredRunDismissal();
    resetRouteState();
    handleNavswapQuote();
  };

  const dismissRouteCard = () => {
    rememberRecoveredRunDismissal();
    setPhase('idle');
    resetRouteState();
  };

  const handleShieldedIngress = async () => {
    setError(''); setSuccess('');
    if (!backupJson) { setError('Wallet must be unlocked to sign the public ingress burn'); return; }
    if (!txBuilder) { setError('Wallet transaction builder is unavailable'); return; }
    if (!swapServer) { setError('Swap adapter is not configured'); return; }
    if (!address) { setError('Wallet address unavailable'); return; }
    if (!shieldedIngressAsset?.asset_id) { setError('Select a supported public asset for shielded ingress'); return; }

    let amountAtoms;
    try {
      amountAtoms = decimalToAtomsString(amt, shieldedIngressAsset.precision || 6);
    } catch (e) {
      setError(e.message || 'Invalid ingress amount');
      return;
    }

    setShieldedIngressSubmit({
      status: 'running',
      stage: 'preflight',
      asset: shieldedIngressAsset,
      amountAtoms,
    });
    try {
      const preflight = await swapServer.getShieldedNavswapPreflight({
        route: SHIELDED_NAVSWAP_ROUTE,
        wallet_address: address,
        asset_id: shieldedIngressAsset.asset_id,
        amount_atoms: amountAtoms,
      });
      setShieldedIngressSubmit(prev => ({ ...prev, status: 'running', stage: 'local_note', preflight }));

      const prover = new LocalAssetOrchardProverClient();
      const noteResult = await prover.buildIngressNote({
        route: SHIELDED_NAVSWAP_ROUTE,
        wallet_address: address,
        asset_id: shieldedIngressAsset.asset_id,
        amount_atoms: amountAtoms,
        preflight,
      });
      const walletNote = noteResult.wallet_note;
      const outputCommitment = walletNote?.output_commitment || null;
      setShieldedIngressSubmit(prev => ({
        ...prev,
        status: 'running',
        stage: 'sign_burn',
        preflight,
        outputCommitment,
        vaultRecord: noteResult.vault_record || null,
      }));

      const signedBurn = await txBuilder.signAssetTransaction(backupJson, address, { operation: preflight.operation });
      setShieldedIngressSubmit(prev => ({ ...prev, status: 'running', stage: 'relay', preflight, outputCommitment }));

      const ingressPayload = buildAssetOrchardIngressPayload({
        signedBurnTransaction: signedBurn.signed,
        assetId: shieldedIngressAsset.asset_id,
        amountAtoms,
        walletNote,
        encryptedOutput: noteResult.encrypted_output,
      });
      const relay = await swapServer.submitShieldedNavswapIngress({
        route: SHIELDED_NAVSWAP_ROUTE,
        wallet_address: address,
        ingress_payload: ingressPayload,
      });
      setShieldedIngressSubmit({
        status: 'submitted',
        ok: true,
        stage: 'complete',
        asset: shieldedIngressAsset,
        amountAtoms,
        outputCommitment: ingressPayload.output_commitment,
        vaultRecord: noteResult.vault_record || null,
        preflight,
        relay,
      });
      await Promise.all([
        refreshAssetBalances().catch(() => null),
        refreshShieldedNotes().catch(() => null),
      ]);
      setSuccess('Public ingress certified; local private note stored in the loopback vault.');
      onToast?.('Shielded ingress certified');
    } catch (e) {
      setShieldedIngressSubmit(prev => ({
        ...prev,
        status: 'failed',
        ok: false,
        message: e.data?.message || e.message || 'Shielded ingress failed',
      }));
      setError(e.data?.message || e.message || 'Shielded ingress failed');
    }
  };

  const handleShieldedSwap = async () => {
    setError(''); setSuccess('');
    if (!swapServer || typeof swapServer.submitShieldedNavswapSwap !== 'function') {
      setError('Shielded swap submit adapter is not configured');
      return;
    }
    if (!address) { setError('Wallet address unavailable'); return; }
    const quote = normalizeShieldedNavswapQuote(activeQuote, Date.now());
    if (!quote.ready || !quote.submit_enabled) {
      setError(quote.expired ? 'Private quote expired. Refresh it before submitting.' : 'Private submit requires a live Step 7 quote.');
      return;
    }
    const quoteRaw = quote.raw || activeQuote;
    const amountAtoms = quote.input_amount_atoms;
    let swapId = null;
    const prover = new LocalAssetOrchardProverClient();
    setShieldedSwapSubmit({
      status: 'running',
      stage: 'local_proof',
      quoteBindingHash: quote.quote_binding_hash,
      amountAtoms,
    });
    try {
      const local = await prover.buildSwapAction({
        route: SHIELDED_NAVSWAP_ROUTE,
        wallet_address: address,
        from_asset_id: quoteRaw.from_asset_id,
        to_asset_id: quoteRaw.to_asset_id,
        amount_atoms: amountAtoms,
        liquidity_commitment: quote.liquidity.commitment,
        quote_binding_hash: quote.quote_binding_hash,
        quote_expires_at_ms: String(quote.quote_expires_at_ms),
      }, {
        pool_id: 'asset-orchard-v1',
        nullifier_count: 2,
        output_count: 2,
        accounting_input_count: 2,
        accounting_output_count: 2,
      });
      swapId = local.swap_id;
      setShieldedSwapSubmit(prev => ({
        ...prev,
        status: 'running',
        stage: 'relay',
        swapId,
        actionVerification: local.verification,
        vaultUpdate: local.vault_update || null,
      }));
      const relay = await swapServer.submitShieldedNavswapSwap({
        route: SHIELDED_NAVSWAP_ROUTE,
        wallet_address: address,
        quote: quoteRaw,
        quote_binding_hash: quote.quote_binding_hash,
        swap_action_json: local.action_json || JSON.stringify(local.action),
        action_verification: local.verification,
        vault_update: local.vault_update || null,
      });
      const finalized = await prover.finalizeSwap({
        swap_id: swapId,
        accepted: relay.ok === true,
        quote_binding_hash: quote.quote_binding_hash,
      });
      setShieldedSwapSubmit({
        status: 'submitted',
        ok: true,
        stage: 'complete',
        swapId,
        quoteBindingHash: quote.quote_binding_hash,
        amountAtoms,
        actionVerification: local.verification,
        vaultUpdate: local.vault_update || null,
        relay,
        finalized,
      });
      await refreshShieldedNotes().catch(() => null);
      setSuccess('Private swap certified. Output stays private; use Public exit only when you want to disclose destination, asset, amount, and timing.');
      onToast?.('Private swap certified');
    } catch (e) {
      if (swapId) {
        await prover.finalizeSwap({
          swap_id: swapId,
          accepted: false,
          quote_binding_hash: quote.quote_binding_hash,
        }).catch(() => {});
      }
      setShieldedSwapSubmit(prev => ({
        ...prev,
        status: 'failed',
        ok: false,
        stage: prev?.stage || 'failed',
        swapId,
        message: e.data?.message || e.message || 'Shielded swap failed',
      }));
      setError(e.data?.message || e.message || 'Shielded swap failed');
    }
  };

  const handleShieldedEgress = async () => {
    setError(''); setSuccess('');
    if (!swapServer || typeof swapServer.submitShieldedNavswapEgress !== 'function') {
      setError('Shielded public-exit adapter is not configured');
      return;
    }
    if (!address) { setError('Wallet address unavailable'); return; }
    if (!selectedEgressNote) { setError('No spendable private note is selected for public exit'); return; }
    if (egressDisclosureAck !== true) {
      setError('Acknowledge the public-exit disclosure before submitting egress.');
      return;
    }

    const assetId = String(selectedEgressNote.asset_id || '').toLowerCase();
    const amountAtoms = String(selectedEgressNote.amount_atoms || '');
    const noteCommitment = String(selectedEgressNote.id || '').toLowerCase();
    const toAddress = address;
    let egressId = null;
    const prover = new LocalAssetOrchardProverClient();
    setShieldedEgressSubmit({
      status: 'running',
      stage: 'disclosure',
      noteCommitment,
      amountAtoms,
      assetId,
    });
    try {
      const disclosure = shieldedPrivateEgressDisclosureFields({
        walletAddress: address,
        to: toAddress,
        assetId,
        amountAtoms,
        noteCommitment,
        policyId: shieldedEgressPolicyId,
      });
      const disclosureHash = await shieldedPrivateEgressDisclosureHash(disclosure);
      setShieldedEgressSubmit(prev => ({
        ...prev,
        status: 'running',
        stage: 'local_proof',
        disclosureHash,
        disclosure,
      }));
      const local = await prover.buildPrivateEgressAction({
        route: SHIELDED_NAVSWAP_ROUTE,
        wallet_address: address,
        to: toAddress,
        asset_id: assetId,
        amount_atoms: amountAtoms,
        note_commitment: noteCommitment,
        policy_id: shieldedEgressPolicyId,
        disclosure_hash: disclosureHash,
        disclosure_ack: true,
      }, {
        pool_id: 'asset-orchard-v1',
        to: toAddress,
        asset_id: assetId,
        amount_atoms: amountAtoms,
        policy_id: shieldedEgressPolicyId,
        disclosure_hash: disclosureHash,
      });
      egressId = local.egress_id;
      setShieldedEgressSubmit(prev => ({
        ...prev,
        status: 'running',
        stage: 'relay',
        egressId,
        actionVerification: local.verification,
        vaultUpdate: local.vault_update || null,
      }));
      const relay = await swapServer.submitShieldedNavswapEgress({
        route: SHIELDED_NAVSWAP_ROUTE,
        wallet_address: address,
        to: toAddress,
        asset_id: assetId,
        amount_atoms: amountAtoms,
        note_commitment: noteCommitment,
        policy_id: shieldedEgressPolicyId,
        disclosure_hash: disclosureHash,
        disclosure_ack: true,
        egress_json: local.egress_json,
        egress_id: egressId,
        action_verification: local.verification,
      });
      const finalized = await prover.finalizePrivateEgress({
        egress_id: egressId,
        accepted: relay.ok === true,
        disclosure_hash: disclosureHash,
      });
      setShieldedEgressSubmit({
        status: 'submitted',
        ok: true,
        stage: 'complete',
        egressId,
        noteCommitment,
        amountAtoms,
        assetId,
        disclosureHash,
        disclosure,
        actionVerification: local.verification,
        vaultUpdate: local.vault_update || null,
        relay,
        finalized,
      });
      setEgressDisclosureAck(false);
      await Promise.all([
        refreshShieldedNotes().catch(() => null),
        refreshAssetBalances().catch(() => null),
      ]);
      setSuccess('Public exit certified. Bridge-out can use this receipt; no bridge-out started automatically.');
      onToast?.('Private note exited to public balance');
    } catch (e) {
      if (egressId) {
        await prover.finalizePrivateEgress({
          egress_id: egressId,
          accepted: false,
        }).catch(() => {});
      }
      setShieldedEgressSubmit(prev => ({
        ...prev,
        status: 'failed',
        ok: false,
        stage: prev?.stage || 'failed',
        egressId,
        message: e.data?.message || e.message || 'Shielded public exit failed',
      }));
      await refreshShieldedNotes().catch(() => null);
      setError(e.data?.message || e.message || 'Shielded public exit failed');
    }
  };

  const activeQuote = routeQuote || navswapReadiness?.quote || null;
  const shieldedQuote = isShieldedRoute ? normalizeShieldedNavswapQuote(activeQuote, quoteNowMs) : null;
  const shieldedQuoteReady = Boolean(shieldedQuote?.ready);
  const shieldedQuoteFetching = isShieldedRoute && phase === 'running' && shieldedIngressSubmit?.status !== 'running';
  const shieldedQuoteCommitment = shieldedQuote?.liquidity?.commitment
    || shieldedRouteCapability?.quote?.liquidity?.commitment
    || shieldedRouteCapability?.quote?.raw?.liquidity_commitment
    || shieldedRouteCapability?.quote?.liquidity_commitment
    || '';
  const shieldedQuoteExpiryLabel = shieldedQuote?.ready && Number.isFinite(shieldedQuote.expires_in_ms)
    ? formatQuoteFreshnessLabel({
        present: true,
        expired: shieldedQuote.expired,
        expiresInMs: shieldedQuote.expires_in_ms,
      })
    : 'quote first';
  useEffect(() => {
    if (!isShieldedRoute || !shieldedQuote?.quote_expires_at_ms || shieldedQuote.expired) return undefined;
    const delayMs = Math.max(1000, Math.min(30000, shieldedQuote.quote_expires_at_ms - Date.now() + 500));
    const timer = setTimeout(() => setQuoteNowMs(Date.now()), delayMs);
    return () => clearTimeout(timer);
  }, [isShieldedRoute, quoteNowMs, shieldedQuote?.expired, shieldedQuote?.quote_expires_at_ms]);
  const navFreshnessPayload = navswapFreshnessPayloadForUi(activeQuote);
  const navFreshnessPlanner = activeQuote?.planner_inputs?.planner || {};
  const navFreshnessSelected = activeQuote?.planner_inputs?.selected || {};
  const navFreshnessEpoch = navFreshnessPlanner.nav_epoch
    || navFreshnessPayload?.nav_epoch
    || navFreshnessSelected.nav_epoch
    || null;
  const navFreshnessReserveHash = navFreshnessPlanner.reserve_packet_hash
    || navFreshnessPayload?.reserve_packet_hash
    || navFreshnessSelected.reserve_packet_hash
    || null;
  const quoteCode = activeQuote?.code || navswapReadiness?.code || '';
  const quoteMessage = activeQuote?.message || navswapReadiness?.message || '';
  const technicalMessage = navswapActionSubmit?.message || quoteMessage || error || '';
  const needsFreshBridge = quoteCode === 'transparent_navswap_no_fresh_settlement_source'
    || /fresh pfUSDC|stale.*receipt|stale_vault_bridge_receipt/i.test(`${quoteCode} ${technicalMessage}`);
  const quotePrepared = activeQuote?.ok === true && preparedBatchActions.length > 0;
  const quotePrice = (() => {
    if (isShieldedRoute && activeQuote?.ok === true) return 1;
    const amountNumber = Number.parseFloat(amt || '0');
    const settlementNumber = Number.parseFloat(settlementDisplayAmount);
    if (!Number.isFinite(amountNumber) || amountNumber <= 0 || !Number.isFinite(settlementNumber)) return null;
    return settlementNumber / amountNumber;
  })();
  const walletSpendBalanceAtoms = readinessSettlement?.balance_atoms ?? fromBalanceAtoms;
  const requiredWalletSpendAtomsForUi = activeQuote?.direction === 'redeem'
    ? (activeQuote?.redeem_amount_atoms || activeQuote?.input_amount_atoms || navswapReadiness?.required_wallet_spend_atoms)
    : (activeQuote?.settlement_amount_atoms
      || activeQuote?.input_amount_atoms
      || navswapReadiness?.required_wallet_spend_atoms
      || navswapReadiness?.required_settlement_atoms);
  const walletSpendBalancePositive = (() => {
    try {
      return BigInt(walletSpendBalanceAtoms || 0) > 0n;
    } catch (_) {
      return false;
    }
  })();
  const settlementSufficient = (() => {
    const balance = walletSpendBalanceAtoms;
    const required = requiredWalletSpendAtomsForUi;
    if (balance === undefined || required === undefined || required === null) return null;
    try {
      if (BigInt(required) <= 0n) return null;
      return BigInt(balance) >= BigInt(required);
    } catch (_) {
      return null;
    }
  })();
  const feeReady = readinessPftFees ? readinessPftFees.ok === true : null;
  const feeIssueText = feeReady === false
    ? (readinessPftFees?.failedMessage
      || (readinessNextStep && /PFT|fee|reserve/i.test(readinessNextStep) ? readinessNextStep : '')
      || readinessPftFees?.failedCode
      || readinessPftFees?.status
      || 'Network fee preflight needs attention')
    : '';
  const feeDisplay = readinessPftFees?.totalMinimumFeeAtoms !== undefined && readinessPftFees?.totalMinimumFeeAtoms !== null
    ? `${formatBalance(readinessPftFees.totalMinimumFeeAtoms)} PFT`
    : feeReady === true
      ? 'ready'
    : feeReady === false
      ? 'needs attention'
      : 'checking';
  const operatorRunning = Boolean(activeRunId && latestRun && !transparentNavswapRunIsTerminal(latestRun));
  const shieldedSwapRunning = shieldedSwapSubmit?.status === 'running';
  const shieldedSwapSubmitted = shieldedSwapSubmit?.status === 'submitted';
  const shieldedSwapFailed = shieldedSwapSubmit?.status === 'failed';
  const shieldedEgressRunning = shieldedEgressSubmit?.status === 'running';
  const shieldedEgressSubmitted = shieldedEgressSubmit?.status === 'submitted';
  const shieldedEgressFailed = shieldedEgressSubmit?.status === 'failed';
  const shieldedEgressCanSubmit = isShieldedRoute
    && shieldedEgressCapability?.enabled === true
    && Boolean(selectedEgressNote)
    && egressDisclosureAck === true
    && !shieldedEgressRunning;
  const shieldedBridgeOutAvailable = shieldedEgressSubmitted
    && shieldedEgressSubmit?.relay?.bridge_out_enabled === true
    && shieldedEgressSubmit?.relay?.public_exit_receipt_required_for_bridge_out === true;
  const shieldedSwapCanSubmit = isShieldedRoute
    && shieldedQuoteReady
    && shieldedQuote?.submit_enabled === true
    && shieldedRouteCapability?.can_run === true
    && !shieldedSwapRunning;
  const actionFailed = !actionComplete && (navswapActionSubmit?.status === 'failed' || shieldedSwapFailed || shieldedEgressFailed || Boolean(error));
  const shieldedIngressRunning = shieldedIngressSubmit?.status === 'running';
  const sourceFreshness = activeQuote?.selected?.receipt_freshness
    || activeQuote?.planner_inputs?.selected?.receipt_freshness
    || activeQuote?.prepared_action_batch?.selected?.receipt_freshness
    || null;
  const navProofStale = quoteFreshness?.expired === true
    || quoteFreshness?.reservePacketFresh === false
    || quoteFreshness?.supplyPacketFresh === false;
  const navProofPresent = Boolean(quoteFreshness?.present || navFreshnessEpoch || navFreshnessReserveHash);
  const navProofFresh = navProofPresent && !navProofStale;
  const navFreshnessStatus = actionComplete
    ? 'used'
    : navProofStale
      ? 'needs refresh'
      : navProofFresh
        ? 'fresh'
        : readinessRefreshing
          ? 'checking'
          : activeQuote
            ? 'pending'
            : 'quote first';
  const navFreshnessDetail = (() => {
    if (actionComplete) {
      return navFreshnessEpoch
        ? `NAV proof from epoch ${navFreshnessEpoch} was used for this submitted swap.`
        : 'NAV proof was used for this submitted swap.';
    }
    if (quoteFreshness?.expired) return 'NAV proof aged out. Refresh the quote before signing.';
    if (quoteFreshness?.reservePacketFresh === false || quoteFreshness?.supplyPacketFresh === false) {
      return 'Reserve or supply packet is stale. Refresh the quote before signing.';
    }
    if (navProofFresh) {
      return navFreshnessEpoch
        ? `Fresh NAV proof for epoch ${navFreshnessEpoch}.`
        : 'Fresh NAV proof is attached to this quote.';
    }
    return 'Get a quote to attach the current NAV epoch and reserve packet hash.';
  })();
  const navFreshnessMeta = navFreshnessEpoch
    ? `epoch ${navFreshnessEpoch}${navFreshnessReserveHash ? ` · ${compactHash(navFreshnessReserveHash, 8)}` : ''}`
    : navFreshnessReserveHash
      ? compactHash(navFreshnessReserveHash, 8)
      : 'proof appears after quote';
  const navFreshnessTone = actionComplete || navProofFresh
    ? 'good'
    : navProofStale
      ? 'warn'
      : readinessRefreshing
        ? 'checking'
        : 'idle';
  const NavFreshnessIcon = navProofStale
    ? AlertCircle
    : readinessRefreshing && !navProofFresh && !actionComplete
      ? Loader2
      : actionComplete || navProofFresh
        ? ShieldCheck
        : Clock;
  const navFreshnessHeadline = actionComplete
    ? 'Price proof used for this swap'
    : navProofStale
      ? 'Refresh the NAV price proof'
      : navProofFresh
        ? 'Fresh NAV price proof attached'
        : readinessRefreshing
          ? 'Checking the NAV price proof'
          : activeQuote
            ? 'Waiting for the NAV price proof'
            : 'Get a quote to load the NAV proof';
  const navFreshnessCopy = actionComplete
    ? 'The submitted transaction was built from this NAV epoch and reserve packet.'
    : navProofStale
      ? 'The quote aged out. Refresh before signing so the NAV price is current.'
      : navProofFresh
        ? 'This is the proof backing the displayed NAV price.'
        : 'The quote response should include a NAV epoch and reserve packet before signing.';
  const navFreshnessClockLabel = actionComplete
    ? 'submitted'
    : quoteFreshness?.present
      ? quoteFreshnessLabel
      : 'quote first';
  const sourceStatus = needsFreshBridge
    ? walletSpendBalancePositive
      ? 'not selectable'
      : 'fresh bridge needed'
    : quotePrepared
      ? sourceFreshness?.usable === false
        ? 'too old'
        : transparentDirection === 'redeem'
          ? 'backed'
          : 'selected'
      : readinessRefreshing
        ? 'checking'
        : 'quote first';
  const sourceLabel = isPftlUniswapRoute
    ? 'Export packet'
    : transparentDirection === 'redeem'
      ? 'Backing allocation'
      : 'Bridge source';
  const settlementIssueText = settlementSufficient === false
    ? `You have ${formatSwapBalance(walletSpendAsset, walletSpendBalanceAtoms)} ${walletSpendAsset}, but this quote requires ${formatSwapBalance(walletSpendAsset, requiredWalletSpendAtomsForUi)} ${walletSpendAsset}. Lower the NAV amount or bridge more ${walletSpendAsset}.`
    : '';
  const freshBridgeIssueText = needsFreshBridge
    ? (transparentDirection === 'redeem'
      ? 'This redemption needs a bridge-backed a651 allocation that has not already been released.'
      : walletSpendBalancePositive
        ? `You have ${formatSwapBalance(walletSpendAsset, walletSpendBalanceAtoms)} ${walletSpendAsset}, but this quote cannot select enough fresh bridge-backed ${walletSpendAsset}. Lower the NAV amount or bridge additional USDC, wait for the relay, then refresh the quote.`
        : 'This swap needs bridge-derived pfUSDC. Bridge USDC into this wallet, wait for the relay, then refresh the quote.')
    : '';
  const friendlyError = settlementIssueText
    || freshBridgeIssueText
    || error
    || quoteMessage
    || navswapReadiness?.next_steps?.[0]
    || '';
  const sourceIssueText = needsFreshBridge ? freshBridgeIssueText : '';
  const lastWalletTxId = navswapActionSubmit?.txIds?.length > 0
    ? navswapActionSubmit.txIds[navswapActionSubmit.txIds.length - 1]
    : null;
  const operatorTxId = completedVerification?.operator_tx_id
    || completedVerification?.operatorTxId
    || completedResult?.operator_tx_id
    || completedResult?.operatorTxId
    || null;
  const canUsePrimaryForFreshQuote = canStartFreshTransparentQuote && actionComplete;
  const primaryBusy = !actionComplete && (
    readinessRefreshing
    || batchRunning
    || fundingRunning
    || operatorRunning
    || shieldedEgressRunning
    || phase === 'running'
  );
  let primaryLabel = canUsePrimaryForFreshQuote ? 'Make another swap' : 'Get quote';
  if (isShieldedRoute) {
    primaryLabel = shieldedSwapSubmitted ? 'Private swap certified' : shieldedSwapCanSubmit ? 'Submit private swap' : 'Private submit locked';
  } else if (isPftlUniswapRoute) {
    if (batchRunning) primaryLabel = 'Submitting source actions';
    else if (quoteFreshness?.expired) primaryLabel = 'Refresh quote';
    else if (quotePrepared) primaryLabel = 'Submit wallet source actions';
    else primaryLabel = 'Get quote';
  } else if (!canUsePrimaryForFreshQuote) {
    if (needsFreshBridge && onNavigate && transparentDirection === 'subscribe') primaryLabel = walletSpendBalancePositive ? 'Bridge more pfUSDC' : 'Bridge fresh pfUSDC';
    else if (primaryBusy) primaryLabel = operatorRunning
      ? (transparentDirection === 'redeem' ? 'Completing redemption' : 'Completing swap')
      : batchRunning
        ? (transparentDirection === 'redeem' ? 'Submitting redemption' : 'Submitting swap')
        : 'Checking route';
    else if (navswapPrimaryStep.kind === 'submit_actions') primaryLabel = transparentDirection === 'redeem' ? 'Submit redemption' : 'Submit swap';
    else if (quoteFreshness?.expired || navswapPrimaryStep.kind === 'refresh_readiness') primaryLabel = 'Refresh quote';
    else if (quotePrepared) primaryLabel = transparentDirection === 'redeem' ? 'Submit redemption' : 'Submit swap';
  }

  const primaryDisabled = canUsePrimaryForFreshQuote ? false : Boolean(
    chainCapabilities?.read_only
    || isShieldedRoute
    || (!needsFreshBridge && !routeCanQuote)
    || primaryBusy
    || (!amt || Number.parseFloat(amt) <= 0)
    || (isPftlUniswapRoute && quotePrepared && (settlementSufficient === false || feeReady === false))
    || (needsFreshBridge && !(onNavigate && transparentDirection === 'subscribe'))
  );
  const completionMessage = latestRun?.message
    || navswapActionSubmit?.operatorMessage
    || success
    || (transparentDirection === 'redeem'
      ? 'Redemption complete. Your balances have updated.'
      : 'Swap complete. Your balances have updated.');
  const statusTitle = actionFailed
    ? 'Swap needs attention'
    : shieldedEgressSubmitted
      ? 'Public exit certified'
    : shieldedSwapSubmitted
      ? 'Private swap certified'
    : shieldedQuoteReady
      ? 'Private quote loaded'
    : pftlSourceSubmitted
      ? 'Source actions submitted'
    : actionComplete
      ? 'Swap complete'
      : 'Swap in progress';
  const statusMessage = actionComplete
    ? completionMessage
    : shieldedEgressSubmitted
      ? 'The private note exited to public balance. Bridge-out is available from this public-exit receipt, but it was not started automatically.'
    : shieldedSwapSubmitted
      ? `The certified swap landed and the ${to} output remains private in the local note vault.`
    : shieldedQuoteReady
      ? (shieldedQuote?.submit_enabled ? 'Quote is live. The wallet can build the private proof locally and submit the opaque action.' : 'Quote preview is bound to liquidity and expiry. Private proof and submit remain locked until Step 7.')
    : pftlSourceSubmitted
      ? 'Wallet primary mint and export debit were submitted. Destination consume and swap are operator-attested CONTROLLED beta steps.'
    : (friendlyError || success || latestRun?.message || navswapActionSubmit?.message || 'Waiting for confirmation.');
  const executionDisplay = isShieldedRoute
    ? (shieldedEgressSubmitted ? 'public_exit_certified' : shieldedEgressRunning ? `egress_${shieldedEgressSubmit?.stage || 'running'}` : shieldedSwapSubmitted ? 'swap_certified_hold_private' : shieldedSwapRunning ? `swap_${shieldedSwapSubmit?.stage || 'running'}` : shieldedQuoteReady ? (shieldedQuote?.submit_enabled ? 'quote_ready_submit_enabled' : 'quote_preview_submit_locked') : 'quote_preview')
    : actionComplete
    ? 'complete'
    : navswapReadiness?.can_execute
      ? 'ready'
      : navswapReadiness?.status || 'checking';
  const displayedQuoteFreshnessLabel = actionComplete ? 'completed' : quoteFreshnessLabel;
  const settlementCheckLabel = isShieldedRoute
    ? 'Custody boundary'
    : actionComplete
    ? (transparentDirection === 'redeem' ? `${transparentPair.amountAsset} redeemed` : 'Settlement spent')
    : (transparentDirection === 'redeem' ? 'Redeem balance' : 'Settlement balance');
  const settlementCheckValue = isShieldedRoute
    ? shieldedRouteCapability?.custody_boundary || 'wallet-local'
    : actionComplete
    ? (requiredSettlement || 'submitted')
    : (fromBalanceKnown ? `${formatSwapBalance(from, fromBalanceAtoms)} ${from}` : 'loading');
  const feeCheckLabel = isShieldedRoute ? 'Note scan' : actionComplete ? 'PFT balance' : 'Fee preflight';
  const feeCheckValue = isShieldedRoute
    ? (shieldedRouteCapability?.requires_note_scan ? 'local required' : 'not declared')
    : actionComplete
    ? (readinessPftFees?.balanceAtoms !== undefined && readinessPftFees?.balanceAtoms !== null
      ? `${formatBalance(readinessPftFees.balanceAtoms)} PFT`
      : 'current')
    : (feeReady === true && feeDisplay !== 'ready' ? `${feeDisplay} required` : feeDisplay);
  const sourceCheckLabel = isShieldedRoute ? 'Local prover' : actionComplete || pftlSourceSubmitted ? 'Wallet tx' : sourceLabel;
  const sourceCheckValue = isShieldedRoute
    ? (shieldedIngressCapability?.enabled ? '127.0.0.1 ready' : (shieldedRouteCapability?.local_prover?.ready ? 'ready' : 'not ready'))
    : actionComplete || pftlSourceSubmitted ? (compactHash(lastWalletTxId) || 'submitted') : sourceStatus;
  const operatorCheckLabel = isShieldedRoute ? 'Swap relay' : actionComplete ? 'Operator tx' : 'Operator leg';
  const operatorCheckValue = isShieldedRoute
    ? (shieldedEgressSubmitted ? 'exit certified' : shieldedEgressRunning ? shieldedEgressSubmit?.stage || 'running' : shieldedSwapSubmitted ? 'hold private' : shieldedSwapRunning ? shieldedSwapSubmit?.stage || 'running' : shieldedQuote?.submit_enabled ? 'ready' : 'locked')
    : actionComplete
    ? (compactHash(operatorTxId) || latestRun?.status || 'complete')
    : (operatorCompletion?.stage || 'quote first');
  const settlementCheckTitle = isShieldedRoute
    ? 'Private keys and note openings stay in the wallet-local vault.'
    : actionComplete
    ? 'This wallet spend was submitted for the completed swap.'
    : settlementIssueText;
  const feeCheckTitle = isShieldedRoute
    ? 'The wallet must scan notes locally before any future private quote can be trusted.'
    : actionComplete
    ? 'This is your current PFT balance available for future network fees.'
    : feeIssueText;
  const sourceCheckTitle = isShieldedRoute
    ? (shieldedIngressCapability?.enabled
      ? 'The browser uses the loopback Asset-Orchard service to create and store the ingress note.'
      : shieldedRouteCapability?.local_prover?.missing?.length
      ? `Missing ${shieldedRouteCapability.local_prover.missing.join(', ')}`
      : 'Local prover readiness hashes are present.')
    : actionComplete
    ? (lastWalletTxId ? `Wallet transaction ${lastWalletTxId}` : 'Wallet transaction submitted.')
    : pftlSourceSubmitted
      ? (lastWalletTxId ? `Wallet source transaction ${lastWalletTxId}` : 'Wallet source transactions submitted.')
    : sourceIssueText;
  const operatorCheckTitle = isShieldedRoute
    ? (shieldedEgressSubmitted ? 'Public exit receipt exists; bridge-out may now be started explicitly.' : shieldedSwapSubmitted ? 'Private swap certified and held private by default.' : shieldedRouteCapability?.swap?.quote_binding_enforcement || shieldedRouteCapability?.p9_status?.copy || 'Private swap submit is gated by Step 7 configuration.')
    : actionComplete
    ? (operatorTxId ? `Operator transaction ${operatorTxId}` : 'Operator completion submitted.')
    : (operatorCompletion?.stage || '');
  const displayTechnicalMessage = !actionComplete && technicalMessage && (Boolean(error) || Boolean(navswapActionSubmit?.message) || activeQuote?.ok === false);

  const handlePrimarySwapAction = () => {
    if (isShieldedRoute) {
      setError(routeReason);
      return;
    }
    if (isPftlUniswapRoute) {
      if (quotePrepared && !quoteFreshness?.expired) {
        handlePreparedActionBatchSubmit();
        return;
      }
      handleNavswapQuote();
      return;
    }
    if (canUsePrimaryForFreshQuote) {
      startFreshTransparentQuote();
      return;
    }
    if (needsFreshBridge && onNavigate && transparentDirection === 'subscribe') {
      onNavigate('bridge');
      return;
    }
    if (navswapPrimaryStep.kind === 'submit_actions' || quotePrepared) {
      handlePreparedActionBatchSubmit();
      return;
    }
    refreshTransparentReadiness()
      .then(readiness => {
        if (readiness?.quote?.ok) {
          setPhase('quoted');
          setSuccess(readiness.quote.message || 'Route quote prepared');
        } else if (readiness?.quote || readiness?.message) {
          setError(readiness.next_steps?.[0] || readiness.message || readiness.quote?.message || 'NAVSwap readiness check failed');
        }
      })
      .catch(e => setError(e.data?.message || e.message || 'NAVSwap readiness refresh failed'));
  };

  return (
    <div className="pf-page pf-swap-page">
      <div className="pfs-shell">
        <main className="pfs-main">
          <header className="pfs-header">
            <div className="pf-eyebrow">Swap</div>
            <h1>Move between assets</h1>
            <p>{isPftlUniswapRoute
              ? 'Mint NAV coins with pfUSDC, export them through the controlled PFTL-Uniswap handoff, then let the operator attest destination consume.'
              : isShieldedRoute
                ? (shieldedRouteCapability?.can_run
                  ? 'Swap private a651 ↔ a652 notes through a controlled pool-managed liquidity note. The proof stays wallet-local; the proxy sees only the quote and opaque action.'
                  : 'Preview a private a651 ↔ a652 quote with an explicit liquidity source. Private proof and submit stay locked until Step 7.')
              : 'Mint NAV coins with pfUSDC at the transparent NAV price.'}</p>
          </header>

          <ProductPrivateSwap address={address} swapServer={swapServer} onToast={onToast} />

          <section className="pfs-route-tabs" aria-label="Swap route">
            {DISPLAYED_SWAP_ROUTES.map(id => {
              const item = ROUTES[id];
              const cap = navswapCaps?.routes?.[id] || null;
              const selected = route === id;
              const disabled = id === SHIELDED_NAVSWAP_ROUTE
                ? false
                : id === PFTL_UNISWAP_BETA_ROUTE
                ? evaluatePftlUniswapBetaRoute({ routeCapability: cap }).ok !== true
                : cap?.can_quote === false;
              return (
                <button
                  key={id}
                  type="button"
                  className={selected ? 'is-active' : ''}
                  onClick={() => selectRoute(id)}
                  disabled={selected || disabled || batchRunning || operatorRunning}
                  title={cap?.reason || item.why}
                >
                  <span>{item.name}</span>
                  <strong>{id === PFTL_UNISWAP_BETA_ROUTE ? 'CONTROLLED' : id === SHIELDED_NAVSWAP_ROUTE ? (cap?.can_run ? 'SWAP' : 'QUOTE') : item.vis}</strong>
                </button>
              );
            })}
          </section>

          {chainCapabilities && chainCapabilities.read_only && (
            <div className="pf-warning">RPC is read-only; transparent swap is disabled.</div>
          )}

          <section className="pfs-card pfs-swap-box" aria-label="NAVSwap quote">
            <div className="pfs-mode">
              {isPftlUniswapRoute ? (
                <button type="button" className="is-active" disabled>
                  Mint + export
                </button>
              ) : isShieldedRoute ? (
                <button type="button" className="is-active" disabled>
                  Quote preview
                </button>
              ) : (
                <>
                  <button
                    type="button"
                    className={transparentDirection === 'subscribe' ? 'is-active' : ''}
                    onClick={() => setTransparentDirection('subscribe')}
                    disabled={batchRunning || operatorRunning}
                  >
                    Mint
                  </button>
                  <button
                    type="button"
                    className={transparentDirection === 'redeem' ? 'is-active' : ''}
                    onClick={() => setTransparentDirection('redeem')}
                    disabled={batchRunning || operatorRunning}
                  >
                    Redeem
                  </button>
                </>
              )}
            </div>

            <div className="pfs-leg">
              <div className="pfs-leg-head">
                <span>{isShieldedRoute ? 'You swap' : transparentDirection === 'redeem' ? 'You redeem' : 'You mint'}</span>
                <span className="pfs-token">{amountFieldAsset}</span>
              </div>
              <input
                value={amt}
                onChange={e => {
                  setAmt(e.target.value.replace(/[^0-9.]/g, ''));
                  resetRouteState();
                }}
                placeholder="0"
                inputMode="decimal"
                disabled={false}
                aria-label={`Amount of ${amountFieldAsset} to ${isShieldedRoute ? 'swap' : transparentDirection === 'redeem' ? 'redeem' : 'mint'}`}
                className="pfs-amount"
              />
              <div className="pfs-balance">
                Balance: {amountFieldBalanceKnown ? formatSwapBalance(amountFieldAsset, amountFieldBalanceAtoms) : 'loading'} {amountFieldAsset}
              </div>
            </div>

            {(isTransparentRoute || isShieldedRoute) && (
              <div className="pfs-switch-row">
                <button
                  type="button"
                  onClick={() => {
                    if (isShieldedRoute) {
                      setFrom(to);
                      setTo(from);
                      resetRouteState();
                    } else {
                      setTransparentDirection(transparentDirection === 'redeem' ? 'subscribe' : 'redeem');
                    }
                  }}
                  disabled={batchRunning || operatorRunning}
                  aria-label="Switch swap direction"
                  title={isShieldedRoute ? 'Switch a651/a652 direction' : transparentPairTitle}
                >
                  <ArrowUpDown size={16} />
                </button>
              </div>
            )}

            <div className="pfs-leg">
              <div className="pfs-leg-head">
                <span>{isShieldedRoute ? 'You receive privately' : transparentDirection === 'redeem' ? 'You receive' : 'You settle with'}</span>
                <span className="pfs-token">{settlementFieldAsset}</span>
              </div>
              <div className="pfs-amount pfs-amount-readonly">≈ {settlementDisplayAmount}</div>
              <div className="pfs-balance">
                Balance: {settlementBalanceKnown ? formatSwapBalance(settlementFieldAsset, settlementBalanceAtoms) : 'loading'} {settlementFieldAsset}
              </div>
            </div>
          </section>

          <section className="pfs-card pfs-route-card">
            <div className="pfs-route-head">
              <span>{r.name}</span>
              <div className="pfs-pill-row">
                <span className={`pf-pill${routeCanQuote ? ' good' : ' warn'}`}>
                    <ShieldCheck size={11} /> {isShieldedRoute ? (shieldedCanQuote ? 'quote ready' : routeStatus) : routeCanQuote ? 'verified' : routeStatus}
                </span>
                <span className={`pf-pill${actionComplete || quotePrepared ? ' good' : quoteFreshness?.expired ? ' warn' : ''}`}>
                  <Clock size={11} /> {displayedQuoteFreshnessLabel}
                </span>
              </div>
            </div>
            <div className="pfs-price">
              {quotePrice ? `1 ${amountFieldAsset} = ${quotePrice.toFixed(5)} ${settlementFieldAsset}` : 'Quote loads the NAV price'}
            </div>
            {isShieldedRoute ? (
              <div
                className={`pfs-nav-proof ${shieldedQuoteReady || shieldedCanQuote ? 'good' : 'warn'}`}
                title={shieldedRouteCapability?.reason || undefined}
                role="status"
                aria-live="polite"
              >
                <div className="pfs-nav-proof-icon">
                  {shieldedQuoteFetching ? <Loader2 size={15} className="pfs-spin" /> : shieldedQuoteReady ? <ShieldCheck size={15} /> : <Clock size={15} />}
                </div>
                <div className="pfs-nav-proof-copy">
                  <span>Liquidity commitment</span>
                  <strong>{shieldedQuoteReady ? 'Live quote bound' : shieldedCanQuote ? 'Ready to quote' : 'Configuration required'}</strong>
                  <small>{shieldedQuoteReady ? 'Quote is bound to the displayed liquidity commitment, policy hash, pair, amount, recipient, and expiry.' : shieldedRouteCapability?.reason || 'Operator quote configuration is not complete.'}</small>
                </div>
                <div className="pfs-nav-proof-meta">
                  <strong>{compactHash(shieldedQuoteCommitment, 8) || shieldedLiquidityModeLabel}</strong>
                  <span>{shieldedQuoteReady ? shieldedQuoteExpiryLabel : 'no quote'}</span>
                </div>
              </div>
            ) : (
              <div
                className={`pfs-nav-proof ${navFreshnessTone}`}
                title={navFreshnessDetail}
                role="status"
                aria-live="polite"
              >
                <div className="pfs-nav-proof-icon">
                  <NavFreshnessIcon
                    size={15}
                    className={readinessRefreshing && navFreshnessTone === 'checking' ? 'pfs-spin' : undefined}
                  />
                </div>
                <div className="pfs-nav-proof-copy">
                  <span>NAV price proof</span>
                  <strong>{navFreshnessHeadline}</strong>
                  <small>{navFreshnessCopy}</small>
                </div>
                <div className="pfs-nav-proof-meta">
                  <strong>{navFreshnessMeta}</strong>
                  <span>{navFreshnessClockLabel}</span>
                </div>
              </div>
            )}
            <p>
              {pftlUniswapBetaPolicy
                ? pftlUniswapBetaPolicy.walletCopy.warning
                : isShieldedRoute
                  ? 'Quote preview identifies who supplies liquidity before the wallet builds any private proof. Note keys, openings, files, spend authority, and backup JSON remain local.'
                : 'Public route. Wallet, amount, allocation, and receipts are visible on-chain.'}
            </p>
          </section>

          {isShieldedRoute && (
            <>
              <section className="pfs-card pfs-route-card">
                <div className="pfs-route-head">
                  <span>Private quote preview</span>
                  <div className="pfs-pill-row">
                    <span className={`pf-pill${shieldedCanQuote ? ' good' : ' warn'}`}>
                      <ShieldCheck size={11} /> {shieldedCanQuote ? 'liquidity ready' : 'config required'}
                    </span>
                  </div>
                </div>
                <p>
                  The adapter returns a bound quote only when a live liquidity commitment exists. This does not build a proof and does not submit a private swap.
                </p>
                <div className="pfs-detail-list">
                  <div><span>Pair</span><strong>{from} → {to}</strong></div>
                  <div><span>Liquidity</span><strong>{shieldedLiquidityModeLabel}</strong></div>
                  <div><span>Commitment</span><strong>{compactHash(shieldedQuoteCommitment, 8) || 'quote first'}</strong></div>
                  <div><span>Policy hash</span><strong>{compactHash(shieldedQuote?.policy_hash || shieldedRouteCapability?.quote?.raw?.policy_hash, 8) || 'quote first'}</strong></div>
                  <div><span>Output</span><strong>{shieldedQuoteReady ? `${formatBalance(shieldedQuote.output_amount_atoms)} ${to}` : 'quote first'}</strong></div>
                  <div><span>Submit gate</span><strong>{shieldedQuote?.next_gate || 'Step 7 private swap submit'}</strong></div>
                </div>
                <section className="pfs-action">
                  <button
                    className="pf-primary"
                    onClick={handleShieldedQuote}
                    disabled={shieldedQuoteFetching || shieldedSwapRunning || !shieldedCanQuote || !address}
                  >
                    {shieldedQuoteFetching ? <Loader2 size={16} className="pfs-spin" /> : <RefreshCw size={16} />}
                    {shieldedQuoteFetching ? 'Fetching quote' : shieldedQuoteReady ? 'Refresh private quote' : 'Get private quote'}
                  </button>
                  <button
                    className="pf-primary"
                    onClick={handleShieldedSwap}
                    disabled={!shieldedSwapCanSubmit}
                  >
                    {shieldedSwapRunning ? <Loader2 size={16} className="pfs-spin" /> : null}
                    {shieldedSwapRunning ? `Swap: ${shieldedSwapSubmit?.stage || 'running'}` : 'Submit private swap'}
                    {!shieldedSwapRunning ? <ArrowRight size={16} /> : null}
                  </button>
                </section>
                {shieldedSwapSubmit && (
                  <section className={`pfs-status ${shieldedSwapSubmit.status === 'failed' ? 'bad' : shieldedSwapSubmit.status === 'submitted' ? 'good' : 'active'}`}>
                    {shieldedSwapSubmit.status === 'failed' ? <AlertCircle size={16} /> : shieldedSwapSubmit.status === 'submitted' ? <Check size={16} /> : <Loader2 size={16} className="pfs-spin" />}
                    <div>
                      <strong>{shieldedSwapSubmit.status === 'submitted' ? 'Private swap certified' : shieldedSwapSubmit.status === 'failed' ? 'Private swap blocked' : 'Private swap running'}</strong>
                      <p>{shieldedSwapSubmit.message || shieldedSwapSubmit.relay?.message || `Stage: ${shieldedSwapSubmit.stage || 'starting'}`}</p>
                      {shieldedSwapSubmit.swapId && <span>{compactHash(shieldedSwapSubmit.swapId, 8)}</span>}
                    </div>
                  </section>
                )}
              </section>

              <section className="pfs-card pfs-route-card">
                <div className="pfs-route-head">
                  <span>Private note exit</span>
                  <div className="pfs-pill-row">
                    <span className={`pf-pill${shieldedEgressCapability?.enabled ? ' good' : ' warn'}`}>
                      <ShieldCheck size={11} /> {shieldedEgressCapability?.enabled ? 'exit ready' : 'exit blocked'}
                    </span>
                    <span className={`pf-pill${shieldedBridgeOutAvailable ? ' good' : ' warn'}`}>
                      <Clock size={11} /> {shieldedBridgeOutAvailable ? 'bridge-out unlocked' : 'bridge-out locked'}
                    </span>
                  </div>
                </div>
                <p>
                  Private swap outputs stay private by default. Public exit credits this wallet's public balance and reveals destination, asset, amount, and timing; the spent note opening stays local.
                </p>
                <div className="pfs-detail-list">
                  <div><span>Spendable private notes</span><strong>{shieldedSpendableNotes.length}</strong></div>
                  <div><span>Selected note</span><strong>{compactHash(selectedEgressNote?.id, 8) || 'none'}</strong></div>
                  <div><span>Public destination</span><strong>{compactHash(address, 8) || 'wallet locked'}</strong></div>
                  <div><span>Exit amount</span><strong>{selectedEgressNote ? `${selectedEgressNote.amount_atoms} atoms` : 'select note'}</strong></div>
                  <div><span>Bridge-out</span><strong>{shieldedBridgeOutAvailable ? 'available from public-exit receipt' : 'locked until public exit certifies'}</strong></div>
                </div>
                {shieldedSpendableNotes.length > 0 && (
                  <div className="pfs-note-list" aria-label="Spendable private notes">
                    {shieldedSpendableNotes.slice(0, 6).map(note => (
                      <button
                        key={note.id}
                        type="button"
                        className={note.id === selectedEgressNote?.id ? 'is-active' : ''}
                        onClick={() => {
                          setSelectedEgressNoteId(note.id);
                          setEgressDisclosureAck(false);
                        }}
                      >
                        <span>{compactHash(note.id, 8)}</span>
                        <strong>{note.amount_atoms} atoms</strong>
                      </button>
                    ))}
                  </div>
                )}
                <label className="pf-checkbox pfs-disclosure">
                  <input
                    type="checkbox"
                    checked={egressDisclosureAck}
                    onChange={event => setEgressDisclosureAck(event.target.checked)}
                    disabled={!selectedEgressNote || shieldedEgressRunning}
                  />
                  <span>I understand this exits to public balance and reveals destination, asset, amount, and timing.</span>
                </label>
                <section className="pfs-action">
                  <button
                    className="pf-primary"
                    onClick={handleShieldedEgress}
                    disabled={!shieldedEgressCanSubmit}
                  >
                    {shieldedEgressRunning ? <Loader2 size={16} className="pfs-spin" /> : null}
                    {shieldedEgressRunning ? `Exit: ${shieldedEgressSubmit?.stage || 'running'}` : 'Exit selected note to public'}
                    {!shieldedEgressRunning ? <ArrowRight size={16} /> : null}
                  </button>
                </section>
                {shieldedEgressSubmit && (
                  <section className={`pfs-status ${shieldedEgressSubmit.status === 'failed' ? 'bad' : shieldedEgressSubmit.status === 'submitted' ? 'good' : 'active'}`}>
                    {shieldedEgressSubmit.status === 'failed' ? <AlertCircle size={16} /> : shieldedEgressSubmit.status === 'submitted' ? <Check size={16} /> : <Loader2 size={16} className="pfs-spin" />}
                    <div>
                      <strong>{shieldedEgressSubmit.status === 'submitted' ? 'Public exit certified' : shieldedEgressSubmit.status === 'failed' ? 'Public exit blocked' : 'Public exit running'}</strong>
                      <p>{shieldedEgressSubmit.message || shieldedEgressSubmit.relay?.message || `Stage: ${shieldedEgressSubmit.stage || 'starting'}`}</p>
                      {shieldedEgressSubmit.egressId && <span>{compactHash(shieldedEgressSubmit.egressId, 8)}</span>}
                    </div>
                  </section>
                )}
              </section>

              <section className="pfs-card pfs-route-card">
                <div className="pfs-route-head">
                  <span>Public ingress to private note</span>
                  <div className="pfs-pill-row">
                    <span className={`pf-pill${shieldedIngressCapability?.enabled ? ' good' : ' warn'}`}>
                      <ShieldCheck size={11} /> {shieldedIngressCapability?.enabled ? 'relay ready' : 'relay blocked'}
                    </span>
                  </div>
                </div>
                <p>
                  Burns public {shieldedIngressAsset?.symbol || from} from this wallet, asks the loopback local service to create and store the private note, then relays the signed ingress batch. This does not execute a private swap.
                </p>
                <div className="pfs-detail-list">
                  <div><span>Asset</span><strong>{shieldedIngressAsset?.symbol || from}</strong></div>
                  <div><span>Amount atoms</span><strong>{(() => { try { return decimalToAtomsString(amt, shieldedIngressAsset?.precision || 6); } catch (_) { return 'enter amount'; } })()}</strong></div>
                  <div><span>Local prover</span><strong>127.0.0.1:8789</strong></div>
                  <div><span>Relay cap</span><strong>{shieldedIngressCapability?.max_amount_atoms || 'unknown'}</strong></div>
                  {shieldedIngressSubmit?.outputCommitment && (
                    <div><span>Output note</span><strong>{compactHash(shieldedIngressSubmit.outputCommitment, 8)}</strong></div>
                  )}
                </div>
                <section className="pfs-action">
                  <button
                    className="pf-primary"
                    onClick={handleShieldedIngress}
                    disabled={shieldedIngressRunning || !backupJson || !shieldedIngressAsset?.asset_id}
                  >
                    {shieldedIngressRunning ? <Loader2 size={16} className="pfs-spin" /> : null}
                    {shieldedIngressRunning ? `Ingress: ${shieldedIngressSubmit?.stage || 'running'}` : 'Create private note'}
                    {!shieldedIngressRunning ? <ArrowRight size={16} /> : null}
                  </button>
                </section>
                {shieldedIngressSubmit && (
                  <section className={`pfs-status ${shieldedIngressSubmit.status === 'failed' ? 'bad' : shieldedIngressSubmit.status === 'submitted' ? 'good' : 'active'}`}>
                    {shieldedIngressSubmit.status === 'failed' ? <AlertCircle size={16} /> : shieldedIngressSubmit.status === 'submitted' ? <Check size={16} /> : <Loader2 size={16} className="pfs-spin" />}
                    <div>
                      <strong>{shieldedIngressSubmit.status === 'submitted' ? 'Ingress certified' : shieldedIngressSubmit.status === 'failed' ? 'Ingress blocked' : 'Ingress running'}</strong>
                      <p>{shieldedIngressSubmit.message || shieldedIngressSubmit.relay?.message || `Stage: ${shieldedIngressSubmit.stage || 'starting'}`}</p>
                    </div>
                  </section>
                )}
              </section>
            </>
          )}

          <section className="pfs-readiness" aria-label="Swap readiness">
            <div
              className={`pfs-check ${actionComplete || settlementSufficient === true ? 'good' : settlementSufficient === false ? 'bad' : ''}`}
              title={settlementCheckTitle || undefined}
              aria-label={settlementCheckTitle || settlementCheckLabel}
            >
              <span>{settlementCheckLabel}</span>
              <strong>{settlementCheckValue}</strong>
              {!actionComplete && settlementIssueText && <small>{settlementIssueText}</small>}
              {actionComplete || settlementSufficient === true ? <Check size={14} /> : settlementSufficient === false ? <AlertCircle size={14} /> : null}
            </div>
            <div
              className={`pfs-check ${actionComplete || feeReady === true ? 'good' : feeReady === false ? 'bad' : ''}`}
              title={feeCheckTitle || undefined}
              aria-label={feeCheckTitle || feeCheckLabel}
            >
              <span>{feeCheckLabel}</span>
              <strong>{feeCheckValue}</strong>
              {!actionComplete && feeIssueText && <small>{feeIssueText}</small>}
              {actionComplete || feeReady === true ? <Check size={14} /> : feeReady === false ? <AlertCircle size={14} /> : null}
            </div>
            <div
              className={`pfs-check ${actionComplete || quotePrepared ? 'good' : needsFreshBridge ? 'bad' : ''}`}
              title={sourceCheckTitle || undefined}
              aria-label={sourceCheckTitle || sourceCheckLabel}
            >
              <span>{sourceCheckLabel}</span>
              <strong>{sourceCheckValue}</strong>
              {!actionComplete && sourceIssueText && <small>{sourceIssueText}</small>}
              {actionComplete || quotePrepared ? <Check size={14} /> : needsFreshBridge ? <AlertCircle size={14} /> : null}
            </div>
            <div
              className={`pfs-check ${actionComplete || operatorCompletion?.stage || shieldedSwapSubmitted ? 'good' : ''}`}
              title={operatorCheckTitle || undefined}
              aria-label={operatorCheckTitle || operatorCheckLabel}
            >
              <span>{operatorCheckLabel}</span>
              <strong>{operatorCheckValue}</strong>
              {actionComplete || operatorCompletion?.stage || shieldedEgressSubmitted || shieldedSwapSubmitted ? <Check size={14} /> : null}
            </div>
          </section>

          <section className="pfs-action">
            <button
              className="pf-primary"
              onClick={handlePrimarySwapAction}
              disabled={primaryDisabled}
            >
              {primaryBusy ? <Loader2 size={16} className="pfs-spin" /> : (primaryLabel === 'Refresh quote' || primaryLabel === 'Make another swap') ? <RefreshCw size={16} /> : null}
              {primaryLabel}
              {!primaryBusy && primaryLabel !== 'Refresh quote' && primaryLabel !== 'Make another swap' ? <ArrowRight size={16} /> : null}
            </button>
            {canStartFreshTransparentQuote && actionComplete && !canUsePrimaryForFreshQuote && (
              <button className="pf-ghost" onClick={startFreshTransparentQuote}>Make another swap</button>
            )}
          </section>

          {(error || success || navswapActionSubmit || activeRunId || actionComplete || shieldedSwapSubmit || shieldedEgressSubmit) && (
            <section className={`pfs-status ${actionFailed ? 'bad' : actionComplete || pftlSourceSubmitted || shieldedEgressSubmitted || shieldedSwapSubmitted || shieldedQuoteReady ? 'good' : 'active'}`}>
              {actionFailed ? <AlertCircle size={16} /> : actionComplete || pftlSourceSubmitted || shieldedEgressSubmitted || shieldedSwapSubmitted || shieldedQuoteReady ? <Check size={16} /> : <Loader2 size={16} className="pfs-spin" />}
              <div>
                <strong>
                  {statusTitle}
                </strong>
                <p>{statusMessage}</p>
                {lastWalletTxId && (
                  <span>{navswapActionSubmit.txIds.length} wallet tx submitted / last {compactHash(lastWalletTxId)}</span>
                )}
                {activeRunId && <span>{compactHash(activeRunId, 8)}</span>}
              </div>
            </section>
          )}
        </main>

        <aside className="pfs-side">
          <section className="pfs-card">
            <div className="pfs-side-head">
              <h2>Balances</h2>
              <button
                type="button"
                onClick={() => {
                  refreshAssetBalances().catch(() => {});
                  refreshShieldedNotes().catch(() => {});
                  refreshTransparentReadiness().catch(() => {});
                }}
                aria-label="Refresh swap balances"
              >
                <RefreshCw size={14} className={readinessRefreshing ? 'pfs-spin' : ''} />
              </button>
            </div>
            <div className="pfs-balance-list">
              <div>
                <span>{from}</span>
                <strong>{fromBalanceKnown ? formatSwapBalance(from, fromBalanceAtoms) : 'loading'}</strong>
              </div>
              <div>
                <span>{to}</span>
                <strong>{toBalanceKnown ? formatSwapBalance(to, toBalanceAtoms) : 'loading'}</strong>
              </div>
              <div>
                <span>PFT balance</span>
                <strong>{readinessPftFees?.balanceAtoms !== undefined ? formatBalance(readinessPftFees.balanceAtoms) : 'checking'}</strong>
              </div>
            </div>
          </section>

          <details className="pfs-card pfs-details">
            <summary>
              <span><Info size={13} /> Transaction details</span>
              <ChevronDown className="pfs-details-chevron" size={14} />
            </summary>
            <div className="pfs-detail-list">
              <div><span>Route</span><strong>{r.name}</strong></div>
              {pftlUniswapBetaPolicy && (
                <>
                  <div><span>Trust class</span><strong>{pftlUniswapBetaPolicy.trustClass || 'missing'}</strong></div>
                  <div><span>Beta route</span><strong>{pftlUniswapBetaPolicy.status}</strong></div>
                  <div><span>Route cap</span><strong>{pftlUniswapBetaPolicy.capRemainingAtoms || 'missing'} / {pftlUniswapBetaPolicy.routeSupplyCapAtoms || 'missing'}</strong></div>
                  <div><span>Packet cap</span><strong>{pftlUniswapBetaPolicy.packetNotionalCapAtoms || 'missing'}</strong></div>
                </>
              )}
              {isShieldedRoute && (
                <>
                  <div><span>enabled</span><strong>{String(shieldedRouteCapability.enabled)}</strong></div>
                  <div><span>can_quote</span><strong>{String(shieldedRouteCapability.can_quote)}</strong></div>
                  <div><span>can_run</span><strong>{String(shieldedRouteCapability.can_run)}</strong></div>
                  <div><span>Custody</span><strong>{shieldedRouteCapability.custody_boundary}</strong></div>
                  <div><span>Local prover</span><strong>{shieldedRouteCapability.requires_local_prover ? 'required' : 'not declared'}</strong></div>
                  <div><span>Note scan</span><strong>{shieldedRouteCapability.requires_note_scan ? 'required' : 'not declared'}</strong></div>
                  <div><span>Liquidity</span><strong>{shieldedRouteCapability.liquidity_mode}</strong></div>
                  <div><span>Quote binding</span><strong>{compactHash(shieldedQuote?.quote_binding_hash, 8) || 'quote first'}</strong></div>
                  <div><span>Policy hash</span><strong>{compactHash(shieldedQuote?.policy_hash || shieldedRouteCapability.quote?.raw?.policy_hash, 8) || 'quote first'}</strong></div>
                  <div><span>Commitment</span><strong>{compactHash(shieldedQuoteCommitment, 8) || 'quote first'}</strong></div>
                  <div><span>Quote output</span><strong>{shieldedQuoteReady ? `${formatBalance(shieldedQuote.output_amount_atoms)} ${to}` : 'quote first'}</strong></div>
                  <div><span>Failure mode</span><strong>{shieldedQuote?.failure_mode || shieldedRouteCapability.quote?.raw?.failure_mode || 'quote expiry before proof/submit'}</strong></div>
                  <div><span>Submit gate</span><strong>{shieldedQuote?.next_gate || 'Step 7 private swap submit'}</strong></div>
                  <div><span>Swap endpoint</span><strong>{shieldedRouteCapability.swap?.enabled ? shieldedRouteCapability.swap.endpoint : 'locked'}</strong></div>
                  <div><span>Swap id</span><strong>{compactHash(shieldedSwapSubmit?.swapId, 8) || 'none'}</strong></div>
                  <div><span>Swap relay</span><strong>{shieldedSwapSubmit?.relay?.status || shieldedSwapSubmit?.status || 'none'}</strong></div>
                  <div><span>Egress endpoint</span><strong>{shieldedEgressCapability?.enabled ? shieldedEgressCapability.endpoint : 'locked'}</strong></div>
                  <div><span>Private notes</span><strong>{shieldedSpendableNotes.length} spendable</strong></div>
                  <div><span>Selected exit note</span><strong>{compactHash(selectedEgressNote?.id, 8) || 'none'}</strong></div>
                  <div><span>Egress id</span><strong>{compactHash(shieldedEgressSubmit?.egressId, 8) || 'none'}</strong></div>
                  <div><span>Egress relay</span><strong>{shieldedEgressSubmit?.relay?.status || shieldedEgressSubmit?.status || 'none'}</strong></div>
                  <div><span>Bridge-out gate</span><strong>{shieldedBridgeOutAvailable ? 'public exit receipt ready' : 'waiting for public exit receipt'}</strong></div>
                  <div><span>Quote enforcement</span><strong>{shieldedRouteCapability.swap?.quote_binding_enforcement || 'quote first'}</strong></div>
                  <div><span>Privacy</span><strong>{shieldedRouteCapability.privacy_label}</strong></div>
                  <div><span>Route disabled</span><strong>{shieldedRouteCapability.disabled_reason}</strong></div>
                  <div><span>Assets</span><strong>{shieldedRouteCapability.asset_registry.length || 'none'}</strong></div>
                  {shieldedRouteCapability.asset_registry.slice(0, 4).map(asset => (
                    <div key={`${asset.symbol}-${asset.asset_id}`}>
                      <span>{asset.symbol || 'asset'}</span>
                      <strong>{asset.ok ? `${asset.precision} dp / ${asset.supported ? 'supported' : 'display-only'}` : `missing ${asset.missing.join(', ')}`}</strong>
                    </div>
                  ))}
                  <div><span>Pairs</span><strong>{shieldedRouteCapability.supported_pairs.length || 'none'}</strong></div>
                  {shieldedRouteCapability.supported_pairs.slice(0, 4).map(pair => (
                    <div key={`${pair.from}-${pair.to}-${pair.index}`}>
                      <span>{pair.from} to {pair.to}</span>
                      <strong>{pair.ok ? 'supported' : pair.errors[0] || 'blocked'}</strong>
                    </div>
                  ))}
                  <div><span>Pool</span><strong>{shieldedRouteCapability.local_prover.pool_id}</strong></div>
                  <div><span>Circuit</span><strong>{shieldedRouteCapability.local_prover.circuit_id || 'missing'}</strong></div>
                  <div><span>K</span><strong>{shieldedRouteCapability.local_prover.k || 'missing'}</strong></div>
                  <div><span>Params</span><strong>{compactHash(shieldedRouteCapability.local_prover.params_hash, 8) || 'missing'}</strong></div>
                  <div><span>VK</span><strong>{compactHash(shieldedRouteCapability.local_prover.vk_hash, 8) || 'missing'}</strong></div>
                  <div><span>P9</span><strong>{shieldedRouteCapability.p9_status.status}</strong></div>
                </>
              )}
              <div><span>Adapter</span><strong>{navswapStatus}</strong></div>
              <div><span>Asset feed</span><strong>{assetFeedLabel}</strong></div>
              {isShieldedRoute ? (
                <>
                  <div><span>Quote status</span><strong>{shieldedQuoteReady ? 'live' : shieldedCanQuote ? 'ready' : 'configuration required'}</strong></div>
                  <div><span>Quote freshness</span><strong>{shieldedQuoteReady ? shieldedQuoteExpiryLabel : 'quote first'}</strong></div>
                </>
              ) : (
                <>
                  <div><span>NAV proof</span><strong>{navFreshnessStatus}</strong></div>
                  <div><span>Quote freshness</span><strong>{displayedQuoteFreshnessLabel}</strong></div>
                  <div><span>NAV epoch</span><strong>{navFreshnessEpoch || 'quote first'}</strong></div>
                  <div><span>Reserve packet</span><strong>{compactHash(navFreshnessReserveHash, 8) || 'quote first'}</strong></div>
                  <div><span>Reserve fresh</span><strong>{formatFreshFlag(quoteFreshness?.reservePacketFresh)}</strong></div>
                  <div><span>Supply fresh</span><strong>{formatFreshFlag(quoteFreshness?.supplyPacketFresh)}</strong></div>
                  <div><span>Receipt age</span><strong>{formatReceiptFreshness(sourceFreshness)}</strong></div>
                </>
              )}
              <div><span>Execution</span><strong>{executionDisplay}</strong></div>
              <div><span>Prepared action</span><strong>{preparedBatchStageLabel || preparedActionStages.join(', ') || 'none'}</strong></div>
              <div><span>Quote batch</span><strong>{preparedBatchActions.length ? `${preparedBatchActions.length} action${preparedBatchActions.length === 1 ? '' : 's'}${actionComplete ? ' submitted' : ''}` : 'none'}</strong></div>
              <div><span>{transparentDirection === 'redeem' ? 'Wallet burn' : 'Wallet spend'}</span><strong>{requiredSettlement || 'quote first'}</strong></div>
              <div><span>Operator leg</span><strong>{actionComplete ? latestRun?.status || operatorCompletion?.stage || 'complete' : operatorCompletion?.stage || 'none'}</strong></div>
              <div><span>Visibility</span><strong>{routeVisibilityLabel}</strong></div>
              {routeDisclosureLabel && <div><span>Disclosed</span><strong>{routeDisclosureLabel}</strong></div>}
              {navswapActionSubmit?.txIds?.length > 0 && (
                <div><span>Wallet tx</span><strong>{compactHash(navswapActionSubmit.txIds[navswapActionSubmit.txIds.length - 1])}</strong></div>
              )}
              {completedVerification?.operator_tx_id && (
                <div><span>Operator tx</span><strong>{compactHash(completedVerification.operator_tx_id)}</strong></div>
              )}
              {displayTechnicalMessage && (
                <pre>{displayTechnicalMessage}</pre>
              )}
              {runEvents.slice(-5).map(event => (
                <div key={`${event.sequence}-${event.type}`}>
                  <span>{event.type}</span>
                  <strong>{event.message}</strong>
                </div>
              ))}
            </div>
          </details>
        </aside>
      </div>
    </div>
  );
}
