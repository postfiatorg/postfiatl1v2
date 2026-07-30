import React, { useCallback, useEffect, useMemo, useState } from 'react';

import {
  A666_NATIVE_ASSET_ID,
  A666_PRIMARY_ROUTE_ID,
  A666_SETTLEMENT_ASSET_ID,
  buildA666IssueOperations,
  buildA666RedeemOperation,
  evaluateA666ResidentMarket,
  formatA666Nav,
  formatA666Units,
  parseA666Units,
} from '../lib/a666-primary-route.js';
import { truncateMiddle } from '../lib/utils.js';

const EMPTY_PROGRESS = [];

function responseResult(response, label) {
  if (!response?.ok || !response.result) {
    throw new Error(response?.error?.message || `${label} is unavailable`);
  }
  return response.result;
}

function assetBalance(result, assetId) {
  const assets = Array.isArray(result) ? result : (result?.assets || []);
  const row = assets.find(item => String(item?.asset_id || item?.id || '').toLowerCase() === assetId);
  return String(row?.balance ?? row?.amount ?? 0);
}

function requireAccepted(result, label) {
  if (result?.receipt?.accepted !== true) {
    const detail = result?.receipt?.message || result?.receipt?.code || 'finality was not confirmed';
    throw new Error(`${label} was not confirmed: ${detail}`);
  }
  return result;
}

function transactionId(result) {
  return result?.txId || result?.receipt?.tx_id || 'finalized';
}

function shortTx(value) {
  return value === 'finalized' ? value : truncateMiddle(String(value), 7);
}

export default function A666Market({
  rpc,
  txBuilder,
  backupJson,
  address,
  chainStatus,
  chainCapabilities,
  liveSnapshot = null,
  onToast,
  onNavigate,
}) {
  const [mode, setMode] = useState('issue');
  const [amount, setAmount] = useState('1');
  const [ethereumRecipient, setEthereumRecipient] = useState('');
  const [snapshot, setSnapshot] = useState(null);
  const [loading, setLoading] = useState(true);
  const [refreshError, setRefreshError] = useState('');
  const [actionError, setActionError] = useState('');
  const [progress, setProgress] = useState(EMPTY_PROGRESS);
  const [executing, setExecuting] = useState(false);
  const [lastCompleted, setLastCompleted] = useState(null);

  const refresh = useCallback(async () => {
    if (!rpc || !address) return null;
    setLoading(true);
    try {
      const [routeResponse, navResponse, assetsResponse, statusResponse] = await Promise.all([
        rpc.navcoinBridgeSupplyStatus(A666_PRIMARY_ROUTE_ID),
        rpc.vaultBridgeStatus(A666_NATIVE_ASSET_ID),
        rpc.accountAssets(address),
        rpc.status(),
      ]);
      const next = {
        route: responseResult(routeResponse, 'A666 route'),
        nav: responseResult(navResponse, 'A666 NAV'),
        assets: responseResult(assetsResponse, 'wallet assets'),
        chain: responseResult(statusResponse, 'chain status'),
      };
      next.pfusdcBalance = assetBalance(next.assets, A666_SETTLEMENT_ASSET_ID);
      next.a666Balance = assetBalance(next.assets, A666_NATIVE_ASSET_ID);
      setSnapshot(next);
      setRefreshError('');
      return next;
    } catch (error) {
      setRefreshError(error.message || 'Unable to load A666 market');
      return null;
    } finally {
      setLoading(false);
    }
  }, [address, rpc]);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, 12_000);
    return () => clearInterval(timer);
  }, [refresh]);

  useEffect(() => {
    const assets = liveSnapshot?.assets;
    if (!snapshot || !assets) return;
    setSnapshot(current => current ? {
      ...current,
      assets,
      pfusdcBalance: assetBalance(assets, A666_SETTLEMENT_ASSET_ID),
      a666Balance: assetBalance(assets, A666_NATIVE_ASSET_ID),
    } : current);
  }, [liveSnapshot, snapshot?.route?.ledger_hash]);

  const amountAtoms = useMemo(() => parseA666Units(amount), [amount]);
  const evaluation = useMemo(() => evaluateA666ResidentMarket({
    supplyStatus: snapshot?.route,
    navStatus: snapshot?.nav,
    chainStatus: snapshot?.chain || chainStatus,
    direction: mode,
    amountAtoms,
    pfusdcBalanceAtoms: snapshot?.pfusdcBalance,
    a666BalanceAtoms: snapshot?.a666Balance,
  }), [amountAtoms, chainStatus, mode, snapshot]);
  const quote = evaluation.quote;

  const finalityReady = chainCapabilities?.read_only === false
    && chainCapabilities?.mempool_submit_asset_transaction_finality_enabled === true;
  const validEthereumRecipient = /^0x[0-9a-f]{40}$/.test(ethereumRecipient);
  const canExecute = evaluation.ok
    && finalityReady
    && Boolean(backupJson)
    && Boolean(txBuilder)
    && !executing
    && (mode === 'redeem' || validEthereumRecipient);

  const updateStep = (index, patch) => {
    setProgress(current => current.map((step, stepIndex) => (
      stepIndex === index ? { ...step, ...patch } : step
    )));
  };

  const runStep = async (index, operation, label) => {
    updateStep(index, { state: 'running', detail: 'Requesting fee quote and local signature…' });
    const result = requireAccepted(
      await txBuilder.sendAssetTransfer(backupJson, address, { operation }),
      label,
    );
    updateStep(index, { state: 'done', detail: shortTx(transactionId(result)), txId: transactionId(result) });
    return result;
  };

  const connectEthereum = async () => {
    setActionError('');
    try {
      if (!window.ethereum?.request) throw new Error('MetaMask is not available in this browser');
      const accounts = await window.ethereum.request({ method: 'eth_requestAccounts' });
      const selected = String(accounts?.[0] || '').toLowerCase();
      if (!/^0x[0-9a-f]{40}$/.test(selected)) throw new Error('MetaMask did not return a valid Ethereum address');
      setEthereumRecipient(selected);
    } catch (error) {
      setActionError(error.message || 'Unable to connect MetaMask');
    }
  };

  const execute = async () => {
    setActionError('');
    setLastCompleted(null);
    setExecuting(true);
    let issueOperations = null;
    let reserved = false;
    let released = false;
    try {
      const fresh = await refresh();
      if (!fresh) throw new Error('Could not refresh the market immediately before signing');
      const freshEvaluation = evaluateA666ResidentMarket({
        supplyStatus: fresh.route,
        navStatus: fresh.nav,
        chainStatus: fresh.chain,
        direction: mode,
        amountAtoms,
        pfusdcBalanceAtoms: fresh.pfusdcBalance,
        a666BalanceAtoms: fresh.a666Balance,
      });
      if (!freshEvaluation.ok) throw new Error(freshEvaluation.blockingReasons.join('. '));

      if (mode === 'issue') {
        if (!validEthereumRecipient) throw new Error('Connect or enter a lowercase Ethereum address');
        issueOperations = buildA666IssueOperations({
          walletAddress: address,
          ethereumRecipient,
          supplyStatus: fresh.route,
          chainHeight: fresh.chain.block_height,
          amountAtoms,
          settlementAtoms: freshEvaluation.quote.settlementAtoms,
        });
        setProgress([
          { label: 'Reserve order', state: 'pending', detail: 'Bind price, capacity, and recipient' },
          { label: 'Mint A666', state: 'pending', detail: 'Exchange pfUSDC at verified NAV' },
          { label: 'Close reservation', state: 'pending', detail: 'Release unused export entitlement' },
          { label: 'Verify balances', state: 'pending', detail: 'Read finalized PFTL state' },
        ]);
        await runStep(0, issueOperations.reserve, 'Order reservation');
        reserved = true;
        await runStep(1, issueOperations.subscribe, 'A666 issuance');
        await runStep(2, issueOperations.release, 'Reservation release');
        released = true;
        updateStep(3, { state: 'running', detail: 'Refreshing finalized balances…' });
      } else {
        const operation = buildA666RedeemOperation({
          walletAddress: address,
          supplyStatus: fresh.route,
          chainHeight: fresh.chain.block_height,
          amountAtoms,
          minimumSettlementAtoms: freshEvaluation.quote.settlementAtoms,
        });
        setProgress([
          { label: 'Redeem A666', state: 'pending', detail: 'Burn A666 and receive pfUSDC' },
          { label: 'Verify balances', state: 'pending', detail: 'Read finalized PFTL state' },
        ]);
        await runStep(0, operation, 'A666 redemption');
        updateStep(1, { state: 'running', detail: 'Refreshing finalized balances…' });
      }

      const verified = await refresh();
      if (!verified) throw new Error('Transaction finalized, but refreshed balances are unavailable');
      const verifyIndex = mode === 'issue' ? 3 : 1;
      updateStep(verifyIndex, {
        state: 'done',
        detail: `${formatA666Units(verified.a666Balance)} A666 · ${formatA666Units(verified.pfusdcBalance)} pfUSDC`,
      });
      setLastCompleted(mode);
      onToast?.(mode === 'issue' ? 'A666 issued at verified NAV' : 'A666 redeemed to pfUSDC');
    } catch (error) {
      if (issueOperations && reserved && !released) {
        try {
          await txBuilder.sendAssetTransfer(backupJson, address, { operation: issueOperations.release });
          released = true;
        } catch (_) {
          // Surface the recovery requirement below. Never imply cleanup succeeded.
        }
      }
      setProgress(current => current.map(step => (
        step.state === 'running'
          ? { ...step, state: 'failed', detail: error.message || 'Transaction failed' }
          : step
      )));
      setActionError(
        `${error.message || 'A666 transaction failed'}${issueOperations && reserved && !released
          ? ' The reservation could not be released automatically; do not retry until route status is reconciled.'
          : ''}`,
      );
      await refresh();
    } finally {
      setExecuting(false);
    }
  };

  const route = snapshot?.route;
  const nav = snapshot?.nav;
  const navPacketMatches = Boolean(
    route?.pricing_reserve_packet_hash
    && nav?.finalized_reserve_packet_hash
    && nav.finalized_reserve_packet_hash === route.pricing_reserve_packet_hash,
  );
  const routeHealthy = route?.live_value_enabled === true
    && route?.paused === false
    && route?.invariant_holds === true
    && navPacketMatches;
  const displayBlockers = evaluation.blockingReasons.filter(reason => reason !== 'enter a positive A666 amount');

  return (
    <section className="a666-page" data-testid="a666-market">
      <header className="a666-hero">
        <div>
          <div className="fs-kicker"><span className="fs-live-dot" /> A666 PRIMARY MARKET · PFTL</div>
          <h1>Mint or redeem A666<br />at verified NAV.</h1>
          <p>
            Mint new fund shares directly against pfUSDC, or redeem shares against the on-chain pfUSDC settlement reserve.
            The Uniswap pool is a separate optional venue—this trade does not consume its liquidity.
          </p>
        </div>
        <button className="pf-button secondary" onClick={refresh} disabled={loading || executing}>
          {loading ? 'Refreshing…' : 'Refresh market'}
        </button>
      </header>

      {refreshError && <div className="pf-error">{refreshError}</div>}

      <div className="a666-safety">
        <div className={`a666-shield ${routeHealthy ? '' : 'bad'}`}>
          {routeHealthy ? '✓' : '!'}
        </div>
        <div>
          <span>MARKET SAFETY</span>
          <strong>{routeHealthy
            ? 'Live route · invariant holds'
            : 'Trading blocked'}</strong>
          <small>Policy {route?.policy_epoch ?? '—'} · NAV epoch {route?.pricing_nav_epoch ?? '—'} · height {snapshot?.chain?.block_height ?? '—'}</small>
        </div>
        <div className="a666-pins">
          <span>RESERVE PACKET</span>
          <strong>{route?.pricing_reserve_packet_hash ? truncateMiddle(route.pricing_reserve_packet_hash, 8) : '—'}</strong>
          <small>{navPacketMatches ? 'matches live StakeHub NAV' : 'packet unavailable or mismatched'}</small>
        </div>
      </div>

      <div className="a666-metrics">
        <div><span>Verified NAV</span><strong>{formatA666Nav(nav?.nav_per_unit)}</strong><small>USD per A666 · pre-inflow</small></div>
        <div><span>Reserve</span><strong>{formatA666Units(route?.settlement_reserve_atoms)}</strong><small>pfUSDC counted on PFTL</small></div>
        <div><span>Mint capacity</span><strong>{formatA666Units(route?.available_issue_atoms)}</strong><small>A666 available now</small></div>
        <div><span>Redeem capacity</span><strong>{formatA666Units(route?.available_redeem_atoms)}</strong><small>A666 available now</small></div>
      </div>

      <div className="a666-flow" aria-label="A666 acquisition flow">
        <div><span>1</span><strong>Fund</strong><small>USDC → pfUSDC</small></div><i />
        <div className="active"><span>2</span><strong>Mint</strong><small>pfUSDC → A666</small></div><i />
        <div><span>3</span><strong>Hold</strong><small>Native on PFTL</small></div><i />
        <div><span>4</span><strong>Bridge-out</strong><small>Not yet in wallet</small></div>
      </div>

      <div className="a666-workspace">
        <div className="a666-trade-card">
          <div className="a666-tabs">
            <button className={mode === 'issue' ? 'on' : ''} onClick={() => { setMode('issue'); setProgress([]); setActionError(''); }}>Mint A666</button>
            <button className={mode === 'redeem' ? 'on' : ''} onClick={() => { setMode('redeem'); setProgress([]); setActionError(''); }}>Redeem</button>
          </div>

          <label className="a666-label" htmlFor="a666-amount">A666 amount</label>
          <div className="a666-amount">
            <input id="a666-amount" inputMode="decimal" value={amount} onChange={event => setAmount(event.target.value)} disabled={executing} />
            <strong>A666</strong>
            <button onClick={() => {
              const maximum = mode === 'issue'
                ? route?.available_issue_atoms
                : (BigInt(snapshot?.a666Balance || 0) < BigInt(route?.available_redeem_atoms || 0)
                  ? snapshot?.a666Balance
                  : route?.available_redeem_atoms);
              if (maximum) setAmount(formatA666Units(maximum));
            }}>MAX</button>
          </div>

          <div className="a666-quote">
            <div><span>{mode === 'issue' ? 'You pay' : 'You receive at least'}</span><strong>{formatA666Units(quote?.settlementAtoms)} pfUSDC</strong></div>
            <div><span>NAV reserve value</span><strong>{formatA666Units(quote?.baseReserveAtoms)} pfUSDC</strong></div>
            <div><span>{mode === 'issue' ? 'Issuance spread' : 'Redemption spread'}</span><strong>{formatA666Units(quote?.spreadAtoms)} pfUSDC</strong></div>
          </div>

          {mode === 'issue' && (
            <div className="a666-recipient">
              <div>
                <label className="a666-label" htmlFor="a666-eth-recipient">Future Ethereum recipient</label>
                <small>Required as an order binding. This purchase stays on PFTL and creates no bridge packet.</small>
              </div>
              <button className="pf-button secondary" onClick={connectEthereum} disabled={executing}>Connect MetaMask</button>
              <input
                id="a666-eth-recipient"
                className="pf-input"
                placeholder="0x…"
                value={ethereumRecipient}
                onChange={event => setEthereumRecipient(event.target.value.trim())}
                disabled={executing}
              />
            </div>
          )}

          {displayBlockers.length > 0 && (
            <div className="a666-blockers">
              {displayBlockers.slice(0, 4).map(reason => <span key={reason}>• {reason}</span>)}
            </div>
          )}
          {mode === 'issue' && displayBlockers.includes('wallet pfUSDC balance is insufficient') && (
            <button className="pf-button secondary" onClick={() => onNavigate?.('bridge')} disabled={executing}>
              Add pfUSDC from Ethereum
            </button>
          )}
          {!finalityReady && <div className="a666-blockers"><span>• Authenticated finality submission is not enabled for this wallet endpoint.</span></div>}
          {actionError && <div className="pf-error">{actionError}</div>}

          <button className="pf-primary" disabled={!canExecute} onClick={execute}>
            {executing ? 'Finalizing on PFTL…' : mode === 'issue'
              ? `Mint ${amountAtoms ? formatA666Units(amountAtoms) : '—'} A666`
              : `Redeem ${amountAtoms ? formatA666Units(amountAtoms) : '—'} A666`}
          </button>
          <p className="a666-signing">Your ML-DSA key signs locally. The proxy receives only signed transactions.</p>
        </div>

        <aside className="a666-side">
          <div className="pf-card">
            <div className="a666-side-title"><span>YOUR PFTL BALANCES</span><small>finalized</small></div>
            <div className="a666-balance"><span>pfUSDC</span><strong>{formatA666Units(snapshot?.pfusdcBalance)}</strong></div>
            <div className="a666-balance"><span>A666</span><strong>{formatA666Units(snapshot?.a666Balance)}</strong></div>
          </div>
          <div className="pf-card a666-details">
            <div className="a666-side-title"><span>EXECUTION DETAILS</span><small>pinned</small></div>
            <div><span>Issue price</span><strong>{route?.issue_multiplier_bps ? `${(Number(route.issue_multiplier_bps) / 10000).toFixed(3)} × NAV` : '—'}</strong></div>
            <div><span>Redeem price</span><strong>{route?.redeem_multiplier_bps ? `${(Number(route.redeem_multiplier_bps) / 10000).toFixed(4)} × NAV` : '—'}</strong></div>
            <div><span>Route</span><strong title={route?.route_id}>{route?.route_id ? truncateMiddle(route.route_id, 12) : '—'}</strong></div>
            <div><span>wA666</span><strong title={route?.wrapped_navcoin_token}>{route?.wrapped_navcoin_token ? truncateMiddle(route.wrapped_navcoin_token, 8) : '—'}</strong></div>
          </div>
        </aside>
      </div>

      {progress.length > 0 && (
        <div className="a666-progress">
          <div className="a666-progress-head">
            <strong>{lastCompleted ? (lastCompleted === 'issue' ? 'A666 purchase complete' : 'A666 redemption complete') : 'Finality progress'}</strong>
            <small>Do not close this page while a step is running.</small>
          </div>
          {progress.map((step, index) => (
            <div className={`a666-progress-step ${step.state}`} key={`${step.label}-${index}`}>
              <span>{step.state === 'done' ? '✓' : index + 1}</span>
              <div><strong>{step.label}</strong><small>{step.detail}</small></div>
            </div>
          ))}
        </div>
      )}

      <div className="a666-venue-note">
        <strong>A666 is delivered natively on PFTL.</strong>
        <span>
          The deployed Ethereum token is {route?.wrapped_navcoin_token ? truncateMiddle(route.wrapped_navcoin_token, 8) : 'wA666'}.
          Bridge-out to that token is a separate operation and is not yet exposed by this wallet.
          Completing a mint here delivers native A666 on PFTL only.
        </span>
      </div>
    </section>
  );
}
