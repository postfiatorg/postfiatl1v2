import React, { useCallback, useEffect, useMemo, useState } from 'react';

import {
  A666_NATIVE_ASSET_ID,
  A666_PRIMARY_ROUTE_ID,
  A666_SETTLEMENT_ASSET_ID,
  A666_WRAPPED_TOKEN,
  buildA666IssueExportDraft,
  buildA666IssueOperations,
  buildA666RedeemOperation,
  evaluateA666ResidentMarket,
  formatA666Nav,
  formatA666Units,
  finalizeA666IssueExportOperations,
  parseA666Units,
} from '../lib/a666-primary-route.js';
import { truncateMiddle } from '../lib/utils.js';

const EMPTY_PROGRESS = [];
const ERC20_BALANCE_OF_SELECTOR = '0x70a08231';

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

async function ensureEthereumMainnet() {
  if (!window.ethereum?.request) throw new Error('MetaMask is not available in this browser');
  let chainId = String(await window.ethereum.request({ method: 'eth_chainId' }) || '').toLowerCase();
  if (chainId !== '0x1') {
    await window.ethereum.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x1' }] });
    chainId = String(await window.ethereum.request({ method: 'eth_chainId' }) || '').toLowerCase();
  }
  if (chainId !== '0x1') throw new Error('MetaMask must be connected to Ethereum mainnet');
}

async function readWrappedA666Balance(recipient) {
  await ensureEthereumMainnet();
  const data = `${ERC20_BALANCE_OF_SELECTOR}${recipient.slice(2).padStart(64, '0')}`;
  const result = await window.ethereum.request({
    method: 'eth_call',
    params: [{ to: A666_WRAPPED_TOKEN, data }, 'latest'],
  });
  if (!/^0x[0-9a-f]+$/i.test(String(result || ''))) throw new Error('MetaMask returned a malformed wA666 balance');
  return BigInt(result);
}

async function watchWrappedA666() {
  await ensureEthereumMainnet();
  return window.ethereum.request({
    method: 'wallet_watchAsset',
    params: {
      type: 'ERC20',
      options: { address: A666_WRAPPED_TOKEN, symbol: 'wA666', decimals: 6 },
    },
  });
}

async function waitForWrappedA666(recipient, minimumBalance, onBalance, timeoutMs = 30 * 60_000) {
  const started = Date.now();
  let last = 0n;
  while (Date.now() - started <= timeoutMs) {
    last = await readWrappedA666Balance(recipient);
    onBalance?.(last);
    if (last >= minimumBalance) return last;
    await new Promise(resolve => setTimeout(resolve, 12_000));
  }
  throw new Error(`PFTL export finalized, but the trustless Ethereum proof relay has not minted wA666 yet (last balance ${formatA666Units(last)})`);
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
  const [delivery, setDelivery] = useState('ethereum');
  const [metamaskA666Balance, setMetamaskA666Balance] = useState(null);
  const [exportPacketHash, setExportPacketHash] = useState('');

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
      try {
        await watchWrappedA666();
      } catch (_) {
        // Rejecting token discovery must not hide a valid on-chain balance.
      }
      const wrappedBalance = await readWrappedA666Balance(selected);
      setMetamaskA666Balance(wrappedBalance.toString());
    } catch (error) {
      setActionError(error.message || 'Unable to connect MetaMask');
    }
  };

  const execute = async () => {
    setActionError('');
    setLastCompleted(null);
    setExportPacketHash('');
    setExecuting(true);
    let issueOperations = null;
    let reserved = false;
    let released = false;
    let exported = false;
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
        let wrappedBalanceBefore = null;
        if (delivery === 'ethereum') {
          await watchWrappedA666();
          wrappedBalanceBefore = await readWrappedA666Balance(ethereumRecipient);
          setMetamaskA666Balance(wrappedBalanceBefore.toString());
          const draft = buildA666IssueExportDraft({
            walletAddress: address,
            ethereumRecipient,
            supplyStatus: fresh.route,
            chainHeight: fresh.chain.block_height,
            amountAtoms,
            settlementAtoms: freshEvaluation.quote.settlementAtoms,
          });
          const preparedPacket = await txBuilder.preparePftlUniswapMintPacket(
            draft.policyHash,
            draft.mintPacket,
          );
          issueOperations = finalizeA666IssueExportOperations(draft, preparedPacket);
        } else {
          issueOperations = buildA666IssueOperations({
            walletAddress: address,
            ethereumRecipient,
            supplyStatus: fresh.route,
            chainHeight: fresh.chain.block_height,
            amountAtoms,
            settlementAtoms: freshEvaluation.quote.settlementAtoms,
          });
        }
        setProgress(delivery === 'ethereum' ? [
          { label: 'Reserve order', state: 'pending', detail: 'Bind verified NAV, capacity, and MetaMask recipient' },
          { label: 'Mint A666', state: 'pending', detail: 'Exchange pfUSDC and increase native supply' },
          { label: 'Export A666', state: 'pending', detail: 'Consume the entitlement and finalize the proof packet' },
          { label: 'Mint wA666', state: 'pending', detail: 'Waiting for the trustless finality proof on Ethereum' },
          { label: 'Verify MetaMask', state: 'pending', detail: 'Read the mainnet ERC-20 balance' },
        ] : [
          { label: 'Reserve order', state: 'pending', detail: 'Bind price, capacity, and recipient' },
          { label: 'Mint A666', state: 'pending', detail: 'Exchange pfUSDC at verified NAV' },
          { label: 'Close reservation', state: 'pending', detail: 'Release unused export entitlement' },
          { label: 'Verify balances', state: 'pending', detail: 'Read finalized PFTL state' },
        ]);
        await runStep(0, issueOperations.reserve, 'Order reservation');
        reserved = true;
        await runStep(1, issueOperations.subscribe, 'A666 issuance');
        if (delivery === 'ethereum') {
          await runStep(2, issueOperations.export, 'A666 export');
          exported = true;
          setExportPacketHash(issueOperations.packetHash);
          updateStep(3, { state: 'running', detail: `Packet ${truncateMiddle(issueOperations.packetHash, 8)} finalized; proving PFTL finality…` });
          const expectedBalance = wrappedBalanceBefore + BigInt(amountAtoms);
          const wrappedBalance = await waitForWrappedA666(
            ethereumRecipient,
            expectedBalance,
            balance => {
              setMetamaskA666Balance(balance.toString());
              updateStep(3, { state: 'running', detail: `Proof relay active · ${formatA666Units(balance)} wA666 visible` });
            },
          );
          updateStep(3, { state: 'done', detail: `${formatA666Units(amountAtoms)} wA666 minted on Ethereum` });
          updateStep(4, { state: 'done', detail: `${formatA666Units(wrappedBalance)} wA666 held by ${truncateMiddle(ethereumRecipient, 7)}` });
        } else {
          await runStep(2, issueOperations.release, 'Reservation release');
          released = true;
          updateStep(3, { state: 'running', detail: 'Refreshing finalized balances…' });
        }
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
      if (mode !== 'issue' || delivery !== 'ethereum') {
        const verifyIndex = mode === 'issue' ? 3 : 1;
        updateStep(verifyIndex, {
          state: 'done',
          detail: `${formatA666Units(verified.a666Balance)} A666 · ${formatA666Units(verified.pfusdcBalance)} pfUSDC`,
        });
      }
      setLastCompleted(mode === 'issue' && delivery === 'ethereum' ? 'ethereum' : mode);
      onToast?.(mode === 'issue'
        ? (delivery === 'ethereum' ? 'wA666 is now held in MetaMask' : 'A666 issued at verified NAV')
        : 'A666 redeemed to pfUSDC');
    } catch (error) {
      if (issueOperations && reserved && !released && !exported) {
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
          && !exported ? ' The reservation could not be released automatically; do not retry until route status is reconciled.'
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
    <section
      className="a666-page"
      data-testid="a666-market"
      data-export-packet-hash={exportPacketHash || undefined}
    >
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
        <div><span>3</span><strong>Export</strong><small>Proof-bound on PFTL</small></div><i />
        <div className={delivery === 'ethereum' ? 'active' : ''}><span>4</span><strong>Hold</strong><small>wA666 in MetaMask</small></div>
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
              <div className="a666-tabs" role="group" aria-label="A666 delivery destination">
                <button className={delivery === 'ethereum' ? 'on' : ''} onClick={() => setDelivery('ethereum')} disabled={executing}>Deliver to MetaMask</button>
                <button className={delivery === 'pftl' ? 'on' : ''} onClick={() => setDelivery('pftl')} disabled={executing}>Keep on PFTL</button>
              </div>
              <div>
                <label className="a666-label" htmlFor="a666-eth-recipient">Ethereum recipient</label>
                <small>{delivery === 'ethereum'
                  ? 'The proof-bound export mints wA666 directly to this MetaMask account.'
                  : 'Bound for recovery safety; the purchased A666 remains native on PFTL.'}</small>
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
            {executing ? (delivery === 'ethereum' ? 'Exporting to MetaMask…' : 'Finalizing on PFTL…') : mode === 'issue'
              ? `${delivery === 'ethereum' ? 'Mint & export' : 'Mint'} ${amountAtoms ? formatA666Units(amountAtoms) : '—'} A666`
              : `Redeem ${amountAtoms ? formatA666Units(amountAtoms) : '—'} A666`}
          </button>
          <p className="a666-signing">Your ML-DSA key signs locally. The proxy receives only signed transactions.</p>
        </div>

        <aside className="a666-side">
          <div className="pf-card">
            <div className="a666-side-title"><span>YOUR PFTL BALANCES</span><small>finalized</small></div>
            <div className="a666-balance"><span>pfUSDC</span><strong>{formatA666Units(snapshot?.pfusdcBalance)}</strong></div>
            <div className="a666-balance"><span>A666</span><strong>{formatA666Units(snapshot?.a666Balance)}</strong></div>
            <div className="a666-balance"><span>wA666 · MetaMask</span><strong>{formatA666Units(metamaskA666Balance)}</strong></div>
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
            <strong>{lastCompleted ? (lastCompleted === 'ethereum' ? 'wA666 delivered to MetaMask' : lastCompleted === 'issue' ? 'A666 purchase complete' : 'A666 redemption complete') : 'Finality progress'}</strong>
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
        <strong>{delivery === 'ethereum' ? 'wA666 is delivered directly to MetaMask.' : 'A666 is delivered natively on PFTL.'}</strong>
        <span>
          The deployed Ethereum token is {route?.wrapped_navcoin_token ? truncateMiddle(route.wrapped_navcoin_token, 8) : 'wA666'}.
          {delivery === 'ethereum'
            ? ' The wallet preserves the issuance entitlement, finalizes the PFTL export, and waits for its trustless Ethereum finality proof before reporting success.'
            : ' This mode closes the unused export entitlement and keeps the balance on PFTL.'}
        </span>
      </div>
    </section>
  );
}
