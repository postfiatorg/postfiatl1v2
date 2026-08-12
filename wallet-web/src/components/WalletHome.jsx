import React, { useState, useEffect, useCallback } from 'react';
import { formatBalance, formatAssetBalance, shortenAssetId, truncateMiddle, pftToAtoms } from '../lib/utils.js';
import { displayAssetSymbol, navcoinMarketForAsset } from '../lib/navcoin-markets.js';
import {
  FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,
  fetchOwnedObjectsSnapshot,
  humanRpcErrorMessage,
  parseAccountResult,
  pollOwnedObjectsTotal,
  rpcErrorMessage,
} from '../lib/rpc-client.js';
import {
  loadFastPayRecoveries,
  removeFastPayRecovery,
  saveFastPayRecovery,
} from '../lib/fastpay-recovery-store.js';

function formatUsdE8(value) {
  const cents = (BigInt(value) + 500_000n) / 1_000_000n;
  return `$${(cents / 100n).toLocaleString()}.${(cents % 100n).toString().padStart(2, '0')}`;
}

export default function WalletHome({ markets = [], rpc, txBuilder, backupJson, address, publicKeyHex, chainStatus, chainCapabilities, liveSnapshot = null, walletFeedStatus = null, onCopy, go, visible = true }) {
  const fastpayEnabled = chainCapabilities?.owned_lane_enabled === true;
  const [balance, setBalance] = useState(null);
  const [sequence, setSequence] = useState(null);
  const [publishedPublicKey, setPublishedPublicKey] = useState(null); // null=unknown, string=published, false=not published
  const [publishBusy, setPublishBusy] = useState(false);
  const [publishError, setPublishError] = useState('');
  const [publishSuccess, setPublishSuccess] = useState('');
  const [rpcError, setRpcError] = useState('');
  const [fastpayBalance, setFastpayBalance] = useState(null);
  const [fastpayObjects, setFastpayObjects] = useState([]);
  const [fastpayStatus, setFastpayStatus] = useState('loading');
  const [fastpayError, setFastpayError] = useState('');
  const [fastpayRefreshing, setFastpayRefreshing] = useState(false);
  const [fastpayRecoveries, setFastpayRecoveries] = useState([]);
  const [fastpayRecoveryBusy, setFastpayRecoveryBusy] = useState('');
  const [assets, setAssets] = useState([]);
  const [navByAssetId, setNavByAssetId] = useState({});
  const [refreshing, setRefreshing] = useState(false);
  const [txs, setTxs] = useState([]);
  const [wrapOpen, setWrapOpen] = useState(false);
  const [fastpaySheetMode, setFastpaySheetMode] = useState('wrap');
  const [wrapAmt, setWrapAmt] = useState('');
  const [wrapBusy, setWrapBusy] = useState(false);
  const [wrapError, setWrapError] = useState('');
  const [wrapSuccess, setWrapSuccess] = useState('');

  const applyFastpaySnapshot = useCallback((snapshot) => {
    setFastpayBalance(snapshot.totalValue ?? snapshot.total_value ?? 0);
    setFastpayObjects(Array.isArray(snapshot.objects) ? snapshot.objects : []);
    setFastpayStatus('ok');
    setFastpayError('');
  }, []);

  const refreshFastpayRecoveries = useCallback(() => {
    if (!publicKeyHex || typeof window === 'undefined') {
      setFastpayRecoveries([]);
      return;
    }
    try {
      setFastpayRecoveries(loadFastPayRecoveries(window.localStorage, publicKeyHex));
    } catch (error) {
      setFastpayError(`FastPay recovery store is unreadable: ${error.message}`);
    }
  }, [publicKeyHex]);

  useEffect(() => refreshFastpayRecoveries(), [refreshFastpayRecoveries]);

  const fetchFastpayBalance = useCallback(async ({ showLoading = true } = {}) => {
    if (!publicKeyHex) {
      setFastpayBalance(null);
      setFastpayObjects([]);
      setFastpayStatus('missing_public_key');
      setFastpayError('');
      return { totalValue: 0, objects: [] };
    }

    if (showLoading) setFastpayStatus('loading');
    try {
      const snapshot = await fetchOwnedObjectsSnapshot(rpc, publicKeyHex, { asset: 'PFT', limit: FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT });
      applyFastpaySnapshot(snapshot);
      return snapshot;
    } catch (e) {
      setFastpayStatus('error');
      setFastpayError(`FastPay balance unavailable: ${humanRpcErrorMessage(e)}`);
      throw e;
    }
  }, [rpc, publicKeyHex, applyFastpaySnapshot]);

  const fetchAccount = useCallback(async () => {
    if (!rpc || !address) return;
    setRefreshing(true);
    setRpcError('');
    try {
      const resp = await rpc.account(address);
      const account = parseAccountResult(resp);
      setBalance(account.balance);
      setSequence(account.sequence);
      // public_key_hex is null/undefined until the wallet submits its first
      // Account-lane transfer or payment (entrypoints.rs:341/589). FastPay
      // senders cannot address this wallet until it is published.
      // Publication is monotonic on-chain. Never let a stale live/read replica
      // overwrite a key that this session already confirmed at finality.
      setPublishedPublicKey(current => current || account.public_key_hex || false);
    } catch (e) {
      setRpcError(`Account balance unavailable: ${humanRpcErrorMessage(e)}`);
    } finally {
      setRefreshing(false);
    }

    // Fetch assets, owned objects, and tx history in parallel (not blocking balance display)
    Promise.allSettled([
      (async () => {
        try {
          const assetResp = await rpc.accountAssets(address);
          if (assetResp.ok && assetResp.result) {
            const items = Array.isArray(assetResp.result) ? assetResp.result : (assetResp.result.assets || []);
            setAssets(items);
          }
        } catch (e) { /* keep existing */ }
      })(),
      fetchFastpayBalance(),
      (async () => {
        try {
          const txResp = await rpc.accountTx(address, { limit: 20 });
          if (txResp.ok && txResp.result) {
            const items = Array.isArray(txResp.result) ? txResp.result : (txResp.result.transactions || []);
            setTxs(items);
          }
        } catch (e) { /* keep existing */ }
      })(),
    ]);
  }, [rpc, address, fetchFastpayBalance]);

  useEffect(() => {
    if (!visible || !liveSnapshot) return;
    if (liveSnapshot.address && address && liveSnapshot.address.toLowerCase() !== address.toLowerCase()) return;
    if (
      liveSnapshot.owner_public_key_hex
      && publicKeyHex
      && liveSnapshot.owner_public_key_hex.toLowerCase() !== publicKeyHex.toLowerCase()
    ) return;

    if (liveSnapshot.account) {
      try {
        const account = parseAccountResult({ ok: true, result: liveSnapshot.account });
        setBalance(account.balance);
        setSequence(account.sequence);
        setPublishedPublicKey(current => current || account.public_key_hex || false);
        setRpcError('');
      } catch (e) {
        setRpcError(`Account balance unavailable: ${humanRpcErrorMessage(e)}`);
      }
    } else if (liveSnapshot.account_error && balance === null) {
      setRpcError(`Account balance unavailable: ${humanRpcErrorMessage(liveSnapshot.account_error)}`);
    }

    if (liveSnapshot.owned) {
      applyFastpaySnapshot(liveSnapshot.owned);
    } else if (liveSnapshot.owned_error && fastpayBalance === null) {
      setFastpayStatus('error');
      setFastpayError(`FastPay balance unavailable: ${humanRpcErrorMessage(liveSnapshot.owned_error)}`);
    }

    if (liveSnapshot.assets) {
      const items = Array.isArray(liveSnapshot.assets)
        ? liveSnapshot.assets
        : (liveSnapshot.assets.assets || []);
      setAssets(items);
    }
  }, [visible, liveSnapshot, address, publicKeyHex, balance, fastpayBalance, applyFastpaySnapshot]);

  const openWrap = (mode = 'wrap') => {
    setFastpaySheetMode(mode);
    setWrapOpen(true);
    setWrapAmt('');
    setWrapError('');
    setWrapSuccess('');
  };

  const confirmPublishedPublicKey = useCallback(async () => {
    let lastError = null;
    for (let attempt = 0; attempt < 8; attempt += 1) {
      try {
        const resp = await rpc.account(address);
        const account = parseAccountResult(resp);
        const published = account.public_key_hex || null;
        if (published) {
          if (publicKeyHex && published.toLowerCase() !== publicKeyHex.toLowerCase()) {
            throw new Error('Ledger public key does not match this wallet');
          }
          setBalance(account.balance);
          setSequence(account.sequence);
          setPublishedPublicKey(published);
          return published;
        }
      } catch (error) {
        lastError = error;
      }
      await new Promise(resolve => setTimeout(resolve, 500));
    }
    if (lastError) throw lastError;
    throw new Error('The activation receipt finalized, but the ledger public key is not visible yet');
  }, [rpc, address, publicKeyHex]);

  const activatePublicKey = useCallback(async ({ automatic = false } = {}) => {
    setPublishError('');
    setPublishSuccess('');
    if (!rpc || !txBuilder || !backupJson || !address) {
      setPublishError('Wallet not unlocked');
      return;
    }
    if (chainCapabilities?.read_only) {
      setPublishError('RPC is read-only; cannot submit transactions.');
      return;
    }
    setPublishBusy(true);
    try {
      const result = await txBuilder.publishPublicKey(backupJson, address);
      if (result.receipt?.accepted !== true || (result.receipt.code && result.receipt.code !== 'accepted')) {
        throw new Error(`Activation rejected: ${result.receipt?.code || 'missing accepted receipt code'} ${result.receipt?.message || ''}`.trim());
      }
      await confirmPublishedPublicKey();
      setPublishSuccess('FastPay activated. This wallet can now receive FastPay transfers.');
      onCopy?.('FastPay activated');
    } catch (e) {
      // A connection can close after the mutation commits but before its
      // response arrives. Reconcile ledger state before surfacing a retry so
      // the wallet never blindly resubmits an ambiguous activation.
      try {
        await confirmPublishedPublicKey();
        setPublishSuccess('FastPay activated. This wallet can now receive FastPay transfers.');
        onCopy?.('FastPay activated');
        return;
      } catch (_) {
        const detail = humanRpcErrorMessage(e, 'Public-key activation failed');
        setPublishError(`${automatic ? 'Automatic FastPay activation failed' : 'Activation failed'}: ${detail}`);
      }
    } finally {
      setPublishBusy(false);
    }
  }, [rpc, txBuilder, backupJson, address, chainCapabilities?.read_only, confirmPublishedPublicKey, onCopy]);

  const handlePublishPublicKey = () => activatePublicKey({ automatic: false });

  const closeWrap = () => {
    setWrapOpen(false);
    setFastpaySheetMode('wrap');
    setWrapAmt('');
    setWrapError('');
    setWrapSuccess('');
  };

  const handleWrap = async () => {
    setWrapError('');
    setWrapSuccess('');
    if (!rpc || !txBuilder || !backupJson || !address || !publicKeyHex) { setWrapError('Wallet not connected'); return; }
    const atoms = pftToAtoms(wrapAmt);
    if (!atoms || atoms <= 0) { setWrapError('Enter a valid amount'); return; }
    if (balance === null || BigInt(atoms + 1) > BigInt(balance)) {
      setWrapError(`Insufficient Account balance. Available: ${formatBalance(balance ?? 0)} PFT`);
      return;
    }
    setWrapBusy(true);
    setFastpayRefreshing(true);
    try {
      const result = await txBuilder.depositToFastPay(
        backupJson,
        address,
        publicKeyHex,
        Number(wrapAmt),
      );
      setWrapSuccess(`Deposited ${formatBalance(atoms)} PFT to FastPay through consensus. Receipt: ${result.receipt.code}.`);
      setPublishedPublicKey(publicKeyHex);
      onCopy('FastPay deposit accepted');
      setWrapAmt('');
      await Promise.allSettled([
        fetchFastpayBalance({ showLoading: false }),
        fetchAccount(),
      ]);
    } catch (e) {
      setWrapError('FastPay deposit error: ' + e.message);
    } finally {
      setFastpayRefreshing(false);
      setWrapBusy(false);
    }
  };

  const handleUnwrap = async () => {
    setWrapError('');
    setWrapSuccess('');
    if (!rpc || !txBuilder || !backupJson || !address || !publicKeyHex) { setWrapError('Wallet not connected'); return; }
    const atoms = pftToAtoms(wrapAmt);
    if (!atoms || atoms <= 0) { setWrapError('Enter a valid amount'); return; }
    if (fastpayStatus !== 'ok') { setWrapError('FastPay balance is unavailable. Refresh before unwrapping.'); return; }
    if (BigInt(atoms) > BigInt(fastpayBalance ?? 0)) {
      setWrapError(`Insufficient FastPay balance. Available: ${formatBalance(fastpayBalance ?? 0)} PFT`);
      return;
    }
    setWrapBusy(true);
    setFastpayRefreshing(true);
    try {
      const validatorsResp = await rpc.validators();
      if (!validatorsResp.ok || !validatorsResp.result) {
        setWrapError('Could not fetch FastPay validators');
        return;
      }
      const validators = Array.isArray(validatorsResp.result)
        ? validatorsResp.result
        : (validatorsResp.result.validators || []);
      const result = await txBuilder.unwrapOwnedTransfer(
        backupJson,
        publicKeyHex,
        fastpayObjects,
        address,
        Number(wrapAmt),
        0,
        validators,
      );
      setWrapSuccess(`Unwrapped ${formatBalance(atoms)} PFT to Account. ${result.votes?.length || 0} validator votes collected.`);
      onCopy('Unwrap successful');
      setWrapAmt('');
      await Promise.allSettled([
        fetchFastpayBalance({ showLoading: false }),
        fetchAccount(),
      ]);
    } catch (e) {
      if (e?.code === 'fastpay_recovery_pending' && e.recovery) {
        try {
          saveFastPayRecovery(window.localStorage, publicKeyHex, e.recovery);
          refreshFastpayRecoveries();
          setWrapError(`FastPay recovery pending: ${e.message}. Do not resubmit; use the recovery action on this screen.`);
        } catch (storageError) {
          setWrapError(`FastPay recovery pending, but the record could not be saved: ${storageError.message}. Do not resubmit.`);
        }
      } else {
        setWrapError('Unwrap error: ' + e.message);
      }
    } finally {
      setFastpayRefreshing(false);
      setWrapBusy(false);
    }
  };

  const handleFastpayRecovery = async (record) => {
    setFastpayError('');
    setFastpayRecoveryBusy(record.lock_id);
    try {
      const result = await txBuilder.recoverFastPay(record.pending);
      if (result.status === 'confirmed_by_recovery' || result.status === 'confirmed' || result.status === 'cancelled') {
        removeFastPayRecovery(window.localStorage, record.lock_id);
      }
      setWrapSuccess(
        result.status === 'certificate_revealed'
          ? `Recovery certificate accepted. Final decision becomes available at height ${result.next_action_height}.`
          : `FastPay recovery completed: ${result.status}. Receipt: ${result.receipt?.code || 'already finalized'}.`,
      );
      refreshFastpayRecoveries();
      await Promise.allSettled([fetchFastpayBalance({ showLoading: false }), fetchAccount()]);
    } catch (error) {
      setFastpayError(`FastPay recovery remains pending: ${error.message}`);
    } finally {
      setFastpayRecoveryBusy('');
    }
  };

  useEffect(() => {
    if (visible) fetchAccount();
  }, [visible, fetchAccount]);

  useEffect(() => {
    const handler = () => {
      if (visible && document.visibilityState === 'visible') fetchAccount();
    };
    document.addEventListener('visibilitychange', handler);
    return () => document.removeEventListener('visibilitychange', handler);
  }, [visible, fetchAccount]);

  useEffect(() => {
    if (!visible || !rpc || markets.length === 0) {
      setNavByAssetId({});
      return undefined;
    }
    let disposed = false;
    let timer = null;
    const refreshNav = async () => {
      const entries = await Promise.all(markets.map(async market => {
        try {
          const response = await rpc.vaultBridgeStatus(market.navAssetId);
          const nav = response?.ok === true ? response.result?.nav_per_unit : null;
          return nav === null || nav === undefined ? null : [market.navAssetId, BigInt(String(nav))];
        } catch (_) { return null; }
      }));
      if (!disposed) {
        setNavByAssetId(Object.fromEntries(entries.filter(Boolean).map(([assetId, nav]) => [assetId, nav.toString()])));
        timer = setTimeout(refreshNav, 30_000);
      }
    };
    refreshNav();
    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
    };
  }, [markets, rpc, visible]);

  const normalizeCode = (code) => String(code || '').toUpperCase() === 'PFUSDC' ? 'pfUSDC' : String(code || '');
  const getAssetCode = (assetOrId) => {
    const asset = typeof assetOrId === 'object' ? assetOrId : null;
    const assetId = asset ? (asset.asset_id || asset.id) : assetOrId;
    return normalizeCode(displayAssetSymbol(markets, assetId, normalizeCode(asset?.code) || shortenAssetId(assetId)));
  };
  const getAssetBalance = (asset) => asset?.balance ?? asset?.amount ?? 0;
  const getAssetBalanceLabel = (asset) => {
    const id = asset?.asset_id || asset?.id;
    const code = getAssetCode(asset);
    return `${formatAssetBalance(id, getAssetBalance(asset))} ${code}`;
  };
  const settlementAssets = [...new Map(markets.map(market => [market.settlementAssetId, market])).values()];
  const settlementAssetIds = new Set(settlementAssets.map(market => market.settlementAssetId));
  const issuedAssetRows = [
    ...settlementAssets.map(market => {
      const asset = assets.find(item => (item.asset_id || item.id) === market.settlementAssetId);
      return [
        normalizeCode(market.settlementSymbol),
        `${formatAssetBalance(market.settlementAssetId, getAssetBalance(asset))} ${normalizeCode(market.settlementSymbol)}`,
        asset ? 'Settlement asset on PFTL' : 'Settlement asset on PFTL · no balance',
      ];
    }),
    ...assets
      .filter(a => !settlementAssetIds.has(a.asset_id || a.id))
      .map(a => {
        const code = getAssetCode(a);
        const isLegacy = /^[a-z]/.test(String(a.code || ''));
        return [code, getAssetBalanceLabel(a), navcoinMarketForAsset(markets, a.asset_id || a.id) ? 'Verified NAV asset on PFTL' : isLegacy ? 'Legacy issued asset on PFTL' : 'Issued asset on PFTL'];
      }),
  ];

  const accountKnown = balance !== null && balance !== undefined;
  const accountBalance = accountKnown ? balance : 0;
  const fastpayKnown = fastpayStatus === 'ok' && fastpayBalance !== null && fastpayBalance !== undefined;
  const totalBalance = fastpayKnown
    ? BigInt(accountBalance) + BigInt(fastpayBalance)
    : BigInt(accountBalance);
  const online = chainStatus && chainStatus.block_height > 0;
  const balanceLoading = balance === null && !rpcError;
  const totalBalanceLabel = rpcError && !accountKnown
    ? 'Unavailable'
    : balanceLoading
      ? '…'
      : formatBalance(totalBalance);
  const accountBalanceLabel = rpcError && !accountKnown
    ? 'Unavailable'
    : balanceLoading
      ? '…'
      : formatBalance(accountBalance);
  const fastpayBalanceLabel = fastpayStatus === 'loading'
    ? '…'
    : fastpayStatus === 'ok'
      ? formatBalance(fastpayBalance)
      : fastpayStatus === 'error'
        ? 'Unavailable'
        : '0';
  const fastpayReady = Boolean(publishedPublicKey) && fastpayStatus === 'ok' && !chainCapabilities?.read_only;
  const fastpayReadyLabel = walletFeedStatus?.status === 'live'
    ? 'Ready to go. Public key published; FastPay balance feed is live.'
    : 'Ready to go. Public key published; FastPay can receive transfers.';
  const stableAsset = assets.find(asset => String(asset?.code || '').toUpperCase() === 'PFUSDC')
    || assets.find(asset => settlementAssetIds.has(asset.asset_id || asset.id));
  // Asset symbols are presentation metadata, not identity. The funded wallet
  // also contains a lowercase legacy `a666`, so matching the label first can
  // silently promote the wrong holding. Resolve the active governed asset by
  // its registry-bound ID and use the symbol only as an offline fallback.
  const navAsset = assets.find(asset => navcoinMarketForAsset(markets, asset.asset_id || asset.id))
    || assets.find(asset => String(asset?.code || '') === 'A666');
  const portfolioAssetCount = assets.length + 1;
  const feeBalanceLow = accountKnown && BigInt(accountBalance) < 1_000n;
  const knownPortfolioUsdE8 = assets.reduce((total, asset) => {
    const assetId = asset.asset_id || asset.id;
    const balanceAtoms = BigInt(String(getAssetBalance(asset)));
    if (String(asset.code || '').toUpperCase() === 'PFUSDC') return total + balanceAtoms * 100n;
    const market = navcoinMarketForAsset(markets, assetId);
    const nav = market ? navByAssetId[assetId] : null;
    return nav ? total + (balanceAtoms * BigInt(nav)) / (10n ** BigInt(market.decimals)) : total;
  }, 0n);
  const pricedAssetIds = new Set(assets.filter(asset => {
    const assetId = asset.asset_id || asset.id;
    return String(asset.code || '').toUpperCase() === 'PFUSDC'
      || Boolean(navcoinMarketForAsset(markets, assetId) && navByAssetId[assetId]);
  }).map(asset => asset.asset_id || asset.id));
  const unpricedCount = assets.length - pricedAssetIds.size + 1;
  const knownPortfolioLabel = knownPortfolioUsdE8 > 0n
    ? formatUsdE8(knownPortfolioUsdE8)
    : null;

  const getDirection = (tx) => {
    const from = tx.from || tx.sender;
    return from === address ? 'out' : 'in';
  };

  const formatActivity = (tx) => {
    const dir = getDirection(tx);
    const counterparty = dir === 'out' ? (tx.to || tx.recipient) : (tx.from || tx.sender);
    const amt = tx.amount || tx.value;
    const kind = tx.transaction_kind || tx.kind || 'Transfer';
    return {
      k: kind,
      d: counterparty ? `to ${truncateMiddle(counterparty, 6)}` : '',
      v: `${dir === 'in' ? '+' : '−'}${formatBalance(amt)}`,
      dir,
      t: `H:${tx.block_height || tx.height || '?'}`,
    };
  };

  return (
    <div className="pf-page">
      {/* portfolio band */}
      <div className="pf-band">
        <div>
          <div className="pf-eyebrow">Your portfolio</div>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 12, marginTop: 6 }}>
            <span style={{ fontSize: 58, fontWeight: 700, letterSpacing: '-0.045em', lineHeight: 1, color: 'var(--green)' }}>
              {knownPortfolioLabel || portfolioAssetCount}
            </span>
            <span style={{ fontFamily: 'var(--mono)', fontSize: 15, color: 'var(--muted)' }}>{knownPortfolioLabel ? 'known value' : 'assets'}</span>
          </div>
          <div style={{ fontSize: 12.5, color: 'var(--dim)', marginTop: 8, maxWidth: 620 }}>
            {knownPortfolioLabel
              ? `Based on pfUSDC and verified A666 NAV. ${unpricedCount} ${unpricedCount === 1 ? 'holding is' : 'holdings are'} excluded because no reliable price is available.`
              : 'Issued assets and the PFT network-fee balance are itemized below. Pricing is still loading.'}
          </div>
        </div>
        <div className="pf-actions">
          <button className="pf-ghost" onClick={() => { navigator.clipboard?.writeText(address || ''); onCopy('Address copied'); }}>Receive</button>
          <button className="pf-ghost" onClick={() => go('send', { sendSource: assets.length ? 'asset' : 'account' })}>Send</button>
          <button className="pf-ghost" onClick={() => go('market')}>Trade</button>
          <button className="pf-ghost" onClick={() => go('bridge')}>Bridge</button>
        </div>
      </div>

      {/* warnings */}
      {chainCapabilities && chainCapabilities.read_only && (
        <div className="pf-warning">RPC is read-only; transaction submission is disabled.</div>
      )}
      {chainCapabilities && !chainCapabilities.read_only && chainStatus && chainStatus.mempool_pending > 0 &&
        chainCapabilities.last_run_unix &&
        Date.now() / 1000 - chainCapabilities.last_run_unix > 300 && (
        <div className="pf-notice">Network has pending transactions at height {chainStatus.block_height} that haven't been processed.</div>
      )}
      {rpcError && <div className="pf-error">{rpcError}</div>}
      {fastpayRecoveries.map(record => (
        <div className="pf-card" key={record.lock_id} style={{ marginTop: 14, display: 'grid', gap: 10, borderColor: 'var(--warning)' }}>
          <div style={{ fontSize: 14, fontWeight: 600 }}>FastPay recovery pending</div>
          <div style={{ fontSize: 12, color: 'var(--muted)', lineHeight: 1.5 }}>
            This payment did not obtain a cryptographically verified apply quorum. Do not resend it.
            The wallet retained the signed recovery record and will either confirm the certified payment or cancel the abandoned lock on chain.
          </div>
          <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--dim)' }}>
            Lock {truncateMiddle(record.lock_id, 18)}
          </div>
          <button
            className="pf-primary"
            onClick={() => handleFastpayRecovery(record)}
            disabled={!!fastpayRecoveryBusy}
          >
            {fastpayRecoveryBusy === record.lock_id ? 'Checking recovery…' : 'Continue FastPay recovery'}
          </button>
        </div>
      ))}

      {/* most useful balances */}
      <div className="pf-stats">
        <div className="pf-tile">
          <div className="pf-eyebrow" style={{ fontSize: 10 }}>Stable balance</div>
          <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.02em' }}>
            {stableAsset ? formatAssetBalance(stableAsset.asset_id || stableAsset.id, getAssetBalance(stableAsset)) : '0'}{' '}
            <span style={{ fontSize: 14, color: 'var(--muted)' }}>pfUSDC</span>
          </div>
          <div style={{ fontSize: 11.5, color: 'var(--dim)' }}>Spendable on PFTL</div>
        </div>
        <div className="pf-tile">
          <div className="pf-eyebrow" style={{ fontSize: 10 }}>NAV asset</div>
          <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.02em' }}>
            {navAsset ? formatAssetBalance(navAsset.asset_id || navAsset.id, getAssetBalance(navAsset)) : '0'}{' '}
            <span style={{ fontSize: 14, color: 'var(--muted)' }}>A666</span>
          </div>
          <div style={{ fontSize: 11.5, color: 'var(--dim)' }}>{markets.length ? 'Verified NAV market available' : 'Market details unavailable'}</div>
        </div>
        <div className="pf-tile">
          <div className="pf-eyebrow" style={{ fontSize: 10 }}>Network fees</div>
          <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.02em' }}>
            {accountBalanceLabel} <span style={{ fontSize: 14, color: 'var(--muted)' }}>PFT</span>
          </div>
          <div style={{ fontSize: 11.5, color: feeBalanceLow ? 'var(--amber)' : 'var(--dim)' }}>
            {feeBalanceLow ? 'Low balance — some transactions may not have enough PFT for fees' : 'Used to pay PFTL transaction fees'}
          </div>
        </div>
      </div>

      {/* body */}
      <div className="pf-dash">
        <div className="pf-dash-col">
          {/* balances */}
          <div>
            <div className="pf-eyebrow" style={{ marginBottom: 12 }}>All balances</div>
            <div className="pf-card" style={{ padding: '6px 18px' }}>
              {[
                ...issuedAssetRows,
                ['PFT', rpcError && !accountKnown ? 'Unavailable' : `${accountBalanceLabel} PFT`, rpcError ? 'Balance unavailable' : 'PFTL network fees'],
              ].map(([k, v, note], i, arr) => (
                <div key={i} className="pf-row" style={{ padding: '14px 0', borderBottom: i < arr.length - 1 ? '1px solid var(--border-soft)' : 'none' }}>
                  <div>
                    <div style={{ fontSize: 14, fontWeight: 600 }}>{k}</div>
                    <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--dim)' }}>{note}</div>
                  </div>
                  <div style={{ fontFamily: 'var(--mono)', fontSize: 15 }}>{v}</div>
                </div>
              ))}
            </div>
          </div>

        </div>

        {/* activity */}
        <div>
          <div className="pf-row" style={{ marginBottom: 12 }}>
            <div className="pf-eyebrow">Recent activity</div>
          </div>
          <div className="pf-card pf-activity-card" style={{ height: 'calc(100% - 30px)' }}>
            <div className="pf-feed">
              {txs.length === 0 ? (
                <div style={{ padding: '20px 14px', fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--dim)' }}>
                      No native PFT transfers returned. Open Activity for wallet-scoped USDC deposits; issued-asset and NAV history is not available from this network endpoint yet.
                </div>
              ) : (
                txs.map((tx, i) => {
                  const a = formatActivity(tx);
                  return (
                    <div key={i} className="pf-act">
                      <div className="pf-act-l">
                        <div className="pf-act-t">{a.k}</div>
                        <div className="pf-act-s">{a.d} · {a.t}</div>
                      </div>
                      <div className="pf-act-v" style={{ color: a.dir === 'in' ? 'var(--mint)' : 'var(--muted)' }}>{a.v}</div>
                    </div>
                  );
                })
              )}
            </div>
          </div>
        </div>
      </div>

      {/* wrap modal */}
      {wrapOpen && (
        <div className="pf-sheet-wrap" onClick={closeWrap}>
          <div className="pf-sheet" onClick={e => e.stopPropagation()} style={{ display: 'grid', gap: 16 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div>
                <div className="pf-eyebrow">{fastpaySheetMode === 'wrap' ? 'Wrap to FastPay' : 'Unwrap from FastPay'}</div>
                <h1 className="pf-h1" style={{ fontSize: 22, marginBottom: 0 }}>
                  {fastpaySheetMode === 'wrap' ? 'Account → FastPay' : 'FastPay → Account'}
                </h1>
              </div>
              <button onClick={closeWrap} style={{
                background: 'none', border: '1px solid var(--border)', borderRadius: '8px',
                color: 'var(--dim)', fontSize: 16, cursor: 'pointer', width: 30, height: 30, padding: 0,
                display: 'grid', placeItems: 'center',
              }}>×</button>
            </div>

            <div className="pf-even">
              <button className={`pf-ghost${fastpaySheetMode === 'wrap' ? ' on' : ''}`} onClick={() => { setFastpaySheetMode('wrap'); setWrapError(''); setWrapSuccess(''); }}>Wrap in</button>
              <button className={`pf-ghost${fastpaySheetMode === 'unwrap' ? ' on' : ''}`} onClick={() => { setFastpaySheetMode('unwrap'); setWrapError(''); setWrapSuccess(''); }}>Unwrap out</button>
            </div>

            <div className="pf-card" style={{ display: 'grid', gap: 8 }}>
              <div className="pf-row">
                <span className="pf-rk">Account balance</span>
                <span className="pf-rv">{rpcError && !accountKnown ? 'Unavailable' : `${accountBalanceLabel} PFT`}</span>
              </div>
              <div className="pf-row">
                <span className="pf-rk">FastPay balance</span>
                <span className="pf-rv">{fastpayStatus === 'ok' ? `${formatBalance(fastpayBalance)} PFT` : fastpayStatus === 'error' ? 'Unavailable' : '…'}</span>
              </div>
            </div>

            {fastpaySheetMode === 'wrap' ? (
              <div className="pf-card">
                <div className="pf-eyebrow" style={{ marginBottom: 10 }}>Amount to wrap</div>
                <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
                  <input value={wrapAmt} onChange={e => setWrapAmt(e.target.value.replace(/[^0-9.]/g, ''))} placeholder="0" inputMode="decimal"
                    style={{ background: 'transparent', border: 'none', outline: 'none', color: wrapAmt ? 'var(--text)' : 'var(--dim)', fontSize: 38, fontWeight: 700, letterSpacing: '-0.03em', width: '100%' }} />
                  <span style={{ fontFamily: 'var(--mono)', fontSize: 14, color: 'var(--muted)' }}>PFT</span>
                </div>
                <div style={{ display: 'flex', gap: 8, marginTop: 10 }}>
                  <button className="pf-ghost" style={{ fontSize: 11, padding: '6px 10px' }} onClick={() => setWrapAmt(formatBalance(accountBalance).replace(/,/g, ''))}>Max</button>
                  <button className="pf-ghost" style={{ fontSize: 11, padding: '6px 10px' }} onClick={() => setWrapAmt(String(Math.floor(Number(formatBalance(accountBalance).replace(/,/g, '')) / 2)))}>Half</button>
                </div>
              </div>
            ) : (
              <div className="pf-card">
                <div className="pf-eyebrow" style={{ marginBottom: 10 }}>Amount to move to Account</div>
                <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
                  <input value={wrapAmt} onChange={e => setWrapAmt(e.target.value.replace(/[^0-9.]/g, ''))} placeholder="0" inputMode="decimal"
                    style={{ background: 'transparent', border: 'none', outline: 'none', color: wrapAmt ? 'var(--text)' : 'var(--dim)', fontSize: 38, fontWeight: 700, letterSpacing: '-0.03em', width: '100%' }} />
                  <span style={{ fontFamily: 'var(--mono)', fontSize: 14, color: 'var(--muted)' }}>PFT</span>
                </div>
                <div style={{ display: 'flex', gap: 8, marginTop: 10 }}>
                  <button className="pf-ghost" style={{ fontSize: 11, padding: '6px 10px' }} onClick={() => setWrapAmt(formatBalance(fastpayBalance ?? 0).replace(/,/g, ''))}>Max</button>
                  <button className="pf-ghost" style={{ fontSize: 11, padding: '6px 10px' }} onClick={() => setWrapAmt(String(Math.floor(Number(formatBalance(fastpayBalance ?? 0).replace(/,/g, '')) / 2)))}>Half</button>
                </div>
                {fastpayObjects.length === 0 && (
                  <div className="pf-notice" style={{ marginTop: 10 }}>No FastPay owned objects are available to unwrap.</div>
                )}
              </div>
            )}

            <div style={{ fontSize: 12, color: 'var(--muted)', lineHeight: 1.5, fontFamily: 'var(--sans)' }}>
              {fastpaySheetMode === 'wrap'
                ? 'Your wallet signs an account-to-FastPay deposit locally. Normal consensus commits it, publishes your public key if needed, and mints the owned object only after an accepted receipt.'
                : 'Moves the requested amount from FastPay to your Account. The wallet selects objects and returns change automatically.'}
            </div>

            {wrapError && <div className="pf-error">{wrapError}</div>}
            {wrapSuccess && <div className="pf-success">{wrapSuccess}</div>}

            <button
              className="pf-primary"
              disabled={!wrapAmt || wrapBusy || (fastpaySheetMode === 'unwrap' && fastpayObjects.length === 0)}
              onClick={fastpaySheetMode === 'wrap' ? handleWrap : handleUnwrap}
            >
              {wrapBusy
                ? (fastpaySheetMode === 'wrap' ? 'Wrapping…' : 'Unwrapping…')
                : (fastpaySheetMode === 'wrap' ? 'Deposit to FastPay' : 'Unwrap to Account')}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
