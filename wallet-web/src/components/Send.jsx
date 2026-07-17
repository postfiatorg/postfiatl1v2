import React, { useState, useCallback, useEffect, useRef } from 'react';
import { isValidAddress, formatBalance, formatAssetBalance, pftToAtoms, shortenAssetId, PFUSDC_ASSET_ID, A651_ASSET_ID } from '../lib/utils.js';
import { encodePaymentMemoFields, hasMemoFields, PAYMENT_MEMO_LIMITS } from '../lib/tx-builder.js';
import {
  FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,
  fetchOwnedObjectsSnapshot,
  humanRpcErrorMessage,
  parseAccountResult,
  pollOwnedObjectsTotal,
  rpcErrorMessage,
} from '../lib/rpc-client.js';
import { resolveFastpayRecipientPublicKey } from '../lib/fastpay.js';
import { saveFastPayRecovery } from '../lib/fastpay-recovery-store.js';

export default function Send({ rpc, txBuilder, backupJson, address, publicKeyHex, initialSource = 'account', onToast, chainCapabilities, liveSnapshot = null, walletFeedStatus = null, visible = true }) {
  const fastpayEnabled = chainCapabilities?.owned_lane_enabled === true;
  const [lane, setLane] = useState(initialSource === 'asset' ? 'asset' : initialSource);
  const [amt, setAmt] = useState('');
  const [to, setTo] = useState('');
  const [memoType, setMemoType] = useState('');
  const [memoFormat, setMemoFormat] = useState('');
  const [memoData, setMemoData] = useState('');
  const [assets, setAssets] = useState([]);
  const [selectedAsset, setSelectedAsset] = useState('');
  const [accountBalance, setAccountBalance] = useState(null);
  const [accountStatus, setAccountStatus] = useState('loading');
  const [accountError, setAccountError] = useState('');
  const [ownKeyPublished, setOwnKeyPublished] = useState(null); // null=unknown, true/false after account fetch
  const [fastpayBalance, setFastpayBalance] = useState(null);
  const [fastpayObjects, setFastpayObjects] = useState([]);
  const [fastpayStatus, setFastpayStatus] = useState('loading');
  const [fastpayError, setFastpayError] = useState('');
  const [fastpayRefreshing, setFastpayRefreshing] = useState(false);
  const [quote, setQuote] = useState(null);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [busy, setBusy] = useState(false);
  const [signing, setSigning] = useState(false);
  const lastSignTime = useRef(0);

  useEffect(() => {
    setLane(initialSource === 'asset' ? 'asset' : initialSource);
    setQuote(null);
    setError('');
  }, [initialSource]);

  useEffect(() => {
    if (!fastpayEnabled && lane === 'fastpay') setLane('account');
  }, [fastpayEnabled, lane]);

  const fetchAccountBalance = useCallback(async () => {
    if (!rpc || !address) {
      setAccountBalance(null);
      setAccountStatus('missing_wallet');
      setAccountError('');
      return;
    }
    setAccountStatus('loading');
    setAccountError('');
    try {
      const resp = await rpc.account(address);
      const account = parseAccountResult(resp);
      setAccountBalance(account.balance);
      setOwnKeyPublished(!!account.public_key_hex);
      setAccountStatus('ok');
    } catch (e) {
      setAccountStatus('error');
      setAccountError(`Account balance unavailable: ${humanRpcErrorMessage(e)}`);
    }
  }, [rpc, address]);

  useEffect(() => {
    if (visible) fetchAccountBalance();
  }, [visible, fetchAccountBalance]);

  const applyFastpaySnapshot = useCallback((snapshot) => {
    setFastpayBalance(snapshot.totalValue ?? snapshot.total_value ?? 0);
    setFastpayObjects(Array.isArray(snapshot.objects) ? snapshot.objects : []);
    setFastpayStatus('ok');
    setFastpayError('');
  }, []);

  const fetchFastpayBalance = useCallback(async ({ showLoading = true } = {}) => {
    if (!fastpayEnabled || !publicKeyHex) {
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
  }, [rpc, publicKeyHex, applyFastpaySnapshot, fastpayEnabled]);

  useEffect(() => {
    if (visible) fetchFastpayBalance().catch(() => {});
  }, [visible, fetchFastpayBalance]);

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
        setAccountBalance(account.balance);
        setOwnKeyPublished(!!account.public_key_hex);
        setAccountStatus('ok');
        setAccountError('');
      } catch (e) {
        setAccountStatus('error');
        setAccountError(`Account balance unavailable: ${humanRpcErrorMessage(e)}`);
      }
    } else if (liveSnapshot.account_error && accountBalance === null) {
      setAccountStatus('error');
      setAccountError(`Account balance unavailable: ${humanRpcErrorMessage(liveSnapshot.account_error)}`);
    }

    if (liveSnapshot.owned) {
      applyFastpaySnapshot(liveSnapshot.owned);
    } else if (liveSnapshot.owned_error && fastpayBalance === null) {
      setFastpayStatus('error');
      setFastpayError(`FastPay balance unavailable: ${humanRpcErrorMessage(liveSnapshot.owned_error)}`);
    }
  }, [visible, liveSnapshot, address, publicKeyHex, accountBalance, fastpayBalance, applyFastpaySnapshot]);

  useEffect(() => {
    const fetchAssets = async () => {
      if (!rpc || !address) return;
      try {
        const resp = await rpc.accountAssets(address);
        if (resp.ok && resp.result) {
          const items = Array.isArray(resp.result) ? resp.result : (resp.result.assets || []);
          setAssets(items);
        }
      } catch (e) { setAssets([]); }
    };
    if (visible) fetchAssets();
  }, [visible, rpc, address]);

  const getAssetCode = (assetId) => {
    if (assetId === PFUSDC_ASSET_ID) return 'pfUSDC';
    if (assetId === A651_ASSET_ID) return 'a651';
    return shortenAssetId(assetId);
  };

  const currentMemos = () => ({
    memo_type: memoType,
    memo_format: memoFormat,
    memo_data: memoData,
  });

  const setLaneSafe = (next) => {
    setLane(next);
    setQuote(null);
    setError('');
    setSuccess('');
  };

  const handleWrap = async () => {
    setError('');
    setSuccess('');
    if (!rpc || !txBuilder || !backupJson || !address || !publicKeyHex) { setError('Wallet not connected'); return; }
    const depositAtoms = pftToAtoms(amt);
    if (!depositAtoms || depositAtoms <= 0) { setError('Enter an amount to deposit'); return; }
    if (accountStatus !== 'ok' || accountBalance === null) { setError('Account balance is unavailable. Refresh before depositing.'); return; }
    if (BigInt(depositAtoms + 1) > BigInt(accountBalance)) {
      setError(`Insufficient Account balance. Available: ${formatBalance(accountBalance)} PFT`);
      return;
    }
    setBusy(true);
    setFastpayRefreshing(true);
    try {
      const result = await txBuilder.depositToFastPay(
        backupJson,
        address,
        publicKeyHex,
        Number(amt),
      );
      setSuccess(`Deposited ${formatBalance(depositAtoms)} PFT to FastPay through consensus. Receipt: ${result.receipt.code}.`);
      setOwnKeyPublished(true);
      onToast('FastPay deposit accepted');
      setAmt('');
      await Promise.allSettled([
        fetchFastpayBalance({ showLoading: false }),
        fetchAccountBalance(),
      ]);
    } catch (e) {
      setError('FastPay deposit error: ' + e.message);
    } finally {
      setFastpayRefreshing(false);
      setBusy(false);
    }
  };

  const handleUnwrap = async () => {
    setError('');
    setSuccess('');
    if (!rpc || !txBuilder || !backupJson || !address || !publicKeyHex) { setError('Wallet not connected'); return; }
    const unwrapAmt = pftToAtoms(amt);
    if (!unwrapAmt || unwrapAmt <= 0) { setError('Enter an amount to unwrap'); return; }
    if (fastpayStatus !== 'ok') { setError('FastPay balance is unavailable. Refresh before unwrapping.'); return; }
    if (!fastpayObjects || fastpayObjects.length === 0) { setError('No owned objects to unwrap'); return; }
    if (BigInt(unwrapAmt) > BigInt(fastpayBalance ?? 0)) {
      setError(`Insufficient FastPay balance. Available: ${formatBalance(fastpayBalance ?? 0)} PFT`);
      return;
    }
    setBusy(true);
    setFastpayRefreshing(true);
    try {
      const validatorsResp = await rpc.validators();
      if (!validatorsResp.ok || !validatorsResp.result) {
        setError('Could not fetch FastPay validators');
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
        Number(amt),
        0,
        validators,
      );
      setSuccess(`Unwrapped ${formatBalance(unwrapAmt)} PFT to Account. ${result.votes?.length || 0} validator votes collected.`);
      onToast('Unwrap successful');
      setAmt('');
      await Promise.allSettled([
        fetchFastpayBalance({ showLoading: false }),
        fetchAccountBalance(),
      ]);
    } catch (e) {
      setError('Unwrap error: ' + e.message);
    } finally {
      setFastpayRefreshing(false);
      setBusy(false);
    }
  };

  const handleQuote = async () => {
    setError('');
    setSuccess('');
    setQuote(null);

    if (lane === 'fastpay') {
      if (fastpayStatus !== 'ok' || !fastpayObjects || fastpayObjects.length === 0) {
        setError('No owned objects available for FastPay transfer. Wrap account balance to owned objects first.');
        return;
      }
      // FastPay uses owned objects directly — no fee quote needed
      // Show a review card with the owned object to be consumed
      const parsed = pftToAtoms(amt);
      if (!parsed || parsed <= 0) { setError('Amount must be a positive number'); return; }
      const fee = 1; // 1 atom fee for FastPay
      const input = fastpayObjects.find(o => o.value >= parsed + fee);
      if (!input) {
        setError(`Insufficient owned object balance. Largest object: ${formatBalance(fastpayObjects[0]?.value || 0)} PFT`);
        return;
      }
      setBusy(true);
      try {
        const recipientPubkeyHex = await resolveFastpayRecipientPublicKey({
          rpc,
          recipient: to,
          ownAddress: address,
          ownPublicKeyHex: publicKeyHex,
        });
        setQuote({
          _isFastpay: true,
          _input: input,
          _fee: fee,
          _amount: parsed,
          _recipient: to,
          _recipientPubkeyHex: recipientPubkeyHex,
        });
      } catch (e) {
        setError(e.message);
      } finally {
        setBusy(false);
      }
      return;
    }

    if (!isValidAddress(to)) { setError('Invalid recipient address'); return; }
    const parsed = pftToAtoms(amt);
    if (!parsed || parsed <= 0) { setError('Amount must be a positive number'); return; }

    let memoFields;
    let memoInput;
    if (lane === 'account') {
      memoInput = currentMemos();
      if (hasMemoFields(memoInput)) {
        try {
          memoFields = encodePaymentMemoFields(memoInput);
        } catch (e) {
          setError(e.message);
          return;
        }
      }
    }

    setBusy(true);
    try {
      if (lane === 'asset') {
        if (!selectedAsset) { setError('Select an asset'); return; }
        const operation = {
          issued_payment: { asset_id: selectedAsset, destination: to, amount: parsed },
        };
        const operationJson = JSON.stringify(operation);
        const quoteResp = await rpc.assetFeeQuote(address, operationJson);
        if (!quoteResp.ok) { setError('Quote failed: ' + (quoteResp.error?.message || 'unknown')); return; }
        setQuote({ ...quoteResp.result, _isAsset: true, _operation: operation });
      } else {
        const resp = await rpc.transferFeeQuote(address, to, parsed, memoFields);
        if (!resp.ok) { setError('Quote failed: ' + (resp.error?.message || 'unknown')); return; }
        setQuote({
          ...resp.result,
          _memos: memoFields || null,
          _memoInput: memoInput || { memo_type: '', memo_format: '', memo_data: '' },
        });
      }
    } catch (e) {
      setError('Quote error: ' + e.message);
    } finally {
      setBusy(false);
    }
  };

  const handleConfirm = async () => {
    setError('');
    setSuccess('');
    if (!quote) { setError('Get a quote first'); return; }
    if (!backupJson) { setError('Wallet not unlocked'); return; }

    const now = Date.now();
    if (now - lastSignTime.current < 3000) { setError('Please wait 3 seconds between sign attempts'); return; }
    lastSignTime.current = now;

    setSigning(true);
    try {
      let result;
      if (quote._isFastpay) {
        // FastPay owned-transfer flow
        // Get validators list
        const validatorsResp = await rpc.validators();
        if (!validatorsResp.ok || !validatorsResp.result) {
          throw new Error('Failed to fetch validator list');
        }
        const validators = Array.isArray(validatorsResp.result) ? validatorsResp.result : (validatorsResp.result.validators || []);

        result = await txBuilder.sendOwnedTransfer(
          backupJson,
          publicKeyHex,
          fastpayObjects,
          quote._recipientPubkeyHex,
          parseFloat(amt),
          quote._fee / 1_000_000, // fee in PFT
          validators,
        );

        setSuccess(`FastPay transfer applied. ${result.votes?.length || 0} validator votes collected.`);
        onToast('FastPay transfer successful');
      } else if (quote._isAsset) {
        result = await txBuilder.sendAssetTransfer(backupJson, address, { operation: quote._operation });

        if (result.receipt?.accepted === true) {
          setSuccess(`Transaction accepted. TX ID: ${result.txId}`);
          onToast('Transfer successful');
        } else if (result.receipt?.accepted === false) {
          setError(`Transaction rejected: ${result.receipt.code || ''} ${result.receipt.message || ''}`);
        } else {
          setError(`Transaction was submitted but no final receipt was returned. TX ID: ${result.txId}. Do not treat it as final.`);
        }
      } else {
        result = await txBuilder.sendTransfer(
          backupJson,
          address,
          to,
          pftToAtoms(amt),
          quote._memoInput,
          quote,
        );

        if (result.receipt?.accepted === true) {
          setSuccess(`Transaction accepted. TX ID: ${result.txId}`);
          onToast('Transfer successful');
        } else if (result.receipt?.accepted === false) {
          setError(`Transaction rejected: ${result.receipt.code || ''} ${result.receipt.message || ''}`);
        } else {
          setError(`Transaction was submitted but no final receipt was returned. TX ID: ${result.txId}. Do not treat it as final.`);
        }
      }
      setQuote(null);
      setTo('');
      setAmt('');
      setMemoType('');
      setMemoFormat('');
      setMemoData('');
      setSelectedAsset('');
      // Refresh balances
      fetchAccountBalance();
      fetchFastpayBalance().catch(() => {});
    } catch (e) {
      if (e?.code === 'fastpay_recovery_pending' && e.recovery && publicKeyHex) {
        try {
          saveFastPayRecovery(window.localStorage, publicKeyHex, e.recovery);
          setError(`FastPay recovery pending: ${e.message}. The signed recovery record is saved locally; use Recovery on the wallet home screen. Do not resend.`);
        } catch (storageError) {
          setError(`FastPay recovery pending: ${e.message}. Recovery record could not be saved: ${storageError.message}. Do not resend.`);
        }
      } else {
        setError('Send failed: ' + e.message);
      }
    } finally {
      setSigning(false);
    }
  };

  const laneLabel = lane === 'account' ? 'Account lane' : lane === 'fastpay' ? 'FastPay lane' : 'Asset lane';
  const settleLabel = lane === 'account' ? 'Cobalt finality · ~1.5s' : lane === 'fastpay' ? 'Sub-second' : 'Cobalt finality · ~1.5s';
  const accountBalanceLabel = accountStatus === 'loading'
    ? '…'
    : accountStatus === 'ok'
      ? `${formatBalance(accountBalance)} PFT`
      : 'Unavailable';
  const fastpayBalanceLabel = fastpayStatus === 'loading'
    ? '…'
    : fastpayStatus === 'ok'
      ? `${formatBalance(fastpayBalance)} PFT`
      : fastpayStatus === 'missing_public_key'
        ? 'No public key'
        : 'Unavailable';
  return (
    <div className="pf-page">
      <div className="pf-stage-inner" style={{ maxWidth: 900 }}>
        <div className="pf-eyebrow">Send</div>
        <h1 className="pf-h1" style={{ marginBottom: 22 }}>{laneLabel}</h1>

        {chainCapabilities && chainCapabilities.read_only && (
          <div className="pf-warning">RPC is read-only; transaction submission is disabled.</div>
        )}
        {chainCapabilities && !fastpayEnabled && (
          <div className="pf-notice">Experimental FastPay mutations are disabled on this endpoint. Account and issued-asset sends remain available.</div>
        )}
        {accountError && <div className="pf-error">{accountError}</div>}
        {fastpayError && <div className="pf-warning">{fastpayError}</div>}
        {fastpayRefreshing && <div className="pf-notice">Refreshing FastPay balance…</div>}

        <div className="pf-two">
          <div style={{ display: 'grid', gap: 16 }}>
            {/* lane toggle */}
            <div className="pf-even">
              <button className={`pf-ghost${lane === 'account' ? ' on' : ''}`} onClick={() => setLaneSafe('account')}>Account</button>
              {fastpayEnabled && <button className={`pf-ghost${lane === 'fastpay' ? ' on' : ''}`} onClick={() => setLaneSafe('fastpay')}>FastPay (experimental)</button>}
            </div>
            {assets.length > 0 && (
              <button className={`pf-ghost${lane === 'asset' ? ' on' : ''}`} onClick={() => setLaneSafe('asset')}>Send Issued Asset</button>
            )}

            {/* account balance info */}
            {lane === 'account' && (
              <div className="pf-card" style={{ display: 'grid', gap: 8 }}>
                <div className="pf-row">
                  <span className="pf-rk">Account balance</span>
                  <span className="pf-rv">
                    {accountBalanceLabel}
                  </span>
                </div>
                {accountStatus === 'ok' && (accountBalance ?? 0) === 0 && fastpayStatus === 'ok' && (fastpayBalance ?? 0) > 0 && (
                  <div className="pf-notice">
                    Your account balance is 0, but you have {formatBalance(fastpayBalance)} PFT in FastPay owned objects. Switch to the FastPay lane to send from those funds.
                  </div>
                )}
                {accountStatus === 'ok' && (accountBalance ?? 0) === 0 && fastpayStatus === 'ok' && (fastpayBalance ?? 0) === 0 && (
                  <div className="pf-notice">No PFT available to send. Receive funds to your wallet address first.</div>
                )}
              </div>
            )}

            {/* asset selector */}
            {lane === 'asset' && (
              <div>
                <div className="pf-eyebrow" style={{ marginBottom: 8 }}>Asset</div>
                <select className="pf-select" value={selectedAsset} onChange={e => setSelectedAsset(e.target.value)}>
                  <option value="">Select asset…</option>
                  {assets.map((asset, i) => (
                    <option key={i} value={asset.asset_id || asset.id}>
                      {getAssetCode(asset.asset_id || asset.id)} — {formatAssetBalance(asset.asset_id || asset.id, asset.balance || asset.amount)}
                    </option>
                  ))}
                </select>
              </div>
            )}

            {/* amount */}
            <div className="pf-card">
              <div className="pf-eyebrow" style={{ marginBottom: 10 }}>Amount</div>
              <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
                <input value={amt} onChange={e => setAmt(e.target.value.replace(/[^0-9.]/g, ''))} placeholder="0" inputMode="decimal"
                  style={{ background: 'transparent', border: 'none', outline: 'none', color: amt ? 'var(--text)' : 'var(--dim)', fontSize: 46, fontWeight: 700, letterSpacing: '-0.03em', width: '100%' }} />
                <span style={{ fontFamily: 'var(--mono)', fontSize: 14, color: 'var(--muted)' }}>PFT</span>
              </div>
            </div>

            {/* recipient */}
            <div>
              <div className="pf-eyebrow" style={{ marginBottom: 8 }}>Recipient</div>
              <input
                className="pf-input"
                value={to}
                onChange={e => setTo(e.target.value)}
                placeholder={lane === 'fastpay' ? 'pf… or public key hex' : 'pf…'}
              />
            </div>

            {lane === 'account' && (
              <details className="pf-card" style={{ display: 'grid', gap: 12 }}>
                <summary className="pf-eyebrow" style={{ cursor: 'pointer', listStylePosition: 'inside' }}>Memo (optional)</summary>
                <div style={{ display: 'grid', gap: 12, paddingTop: 4 }}>
                  <label style={{ display: 'grid', gap: 8 }}>
                    <span className="pf-eyebrow">Memo Type</span>
                    <input
                      className="pf-input"
                      value={memoType}
                      onChange={e => setMemoType(e.target.value)}
                      maxLength={PAYMENT_MEMO_LIMITS.memo_type}
                    />
                  </label>
                  <label style={{ display: 'grid', gap: 8 }}>
                    <span className="pf-eyebrow">Memo Format</span>
                    <input
                      className="pf-input"
                      value={memoFormat}
                      onChange={e => setMemoFormat(e.target.value)}
                      maxLength={PAYMENT_MEMO_LIMITS.memo_format}
                    />
                  </label>
                  <label style={{ display: 'grid', gap: 8 }}>
                    <span className="pf-eyebrow">Memo Data</span>
                    <input
                      className="pf-input"
                      value={memoData}
                      onChange={e => setMemoData(e.target.value)}
                      maxLength={PAYMENT_MEMO_LIMITS.memo_data}
                    />
                  </label>
                </div>
              </details>
            )}

            {/* fastpay balance info */}
            {lane === 'fastpay' && (
              <div className="pf-card" style={{ display: 'grid', gap: 8 }}>
                <div className="pf-row">
                  <span className="pf-rk">FastPay balance</span>
                  <span className="pf-rv">
                    {fastpayBalanceLabel}
                  </span>
                </div>
                {ownKeyPublished === false && (
                  <div className="pf-warning">
                    Your public key is not published on the ledger. You can still <strong>send</strong> FastPay transfers, but <strong>other wallets cannot send FastPay to you</strong>. Go to the Wallet tab and tap “Publish public key” (costs only the network fee) to enable incoming FastPay.
                  </div>
                )}
                {fastpayRefreshing && (
                  <div className="pf-notice">Refreshing owned objects.</div>
                )}
                {walletFeedStatus?.status === 'live' && !fastpayRefreshing && (
                  <div className="pf-notice">Live balance feed connected.</div>
                )}
                {fastpayStatus === 'error' && (
                  <div className="pf-warning">FastPay balance is unavailable.</div>
                )}
                {fastpayStatus === 'ok' && fastpayObjects.length === 0 && (
                  <div className="pf-notice">No owned objects yet. Wrap account balance below to use FastPay.</div>
                )}
                {fastpayStatus === 'ok' && fastpayObjects.length > 0 && (
                  <div className="pf-notice">{fastpayObjects.length} owned object(s). Unwrap uses the amount field and returns FastPay change automatically.</div>
                )}
                {/* Wrap / Unwrap controls */}
                <div style={{ display: 'flex', gap: 8, marginTop: 4 }}>
                  <button className="pf-ghost" style={{ flex: 1, fontSize: 12 }} onClick={handleWrap} disabled={busy || signing || !amt || accountStatus !== 'ok'}>
                    Account → FastPay
                  </button>
                  <button className="pf-ghost" style={{ flex: 1, fontSize: 12 }} onClick={handleUnwrap} disabled={busy || signing || fastpayObjects.length === 0 || !amt}>
                    Unwrap → Account
                  </button>
                </div>
              </div>
            )}

            <button className="pf-primary" disabled={!amt || !to || (lane === 'asset' && !selectedAsset)} onClick={handleQuote}>
              {busy ? 'Getting quote…' : 'Review send'}
            </button>
          </div>

          {/* what happens panel */}
          <div className="pf-card" style={{ display: 'grid', gap: 14, background: 'var(--surface2)' }}>
            <div className="pf-eyebrow">What happens</div>
            <div className="pf-row"><span className="pf-rk">Lane</span><span className="pf-rv">{lane === 'account' ? 'Account' : lane === 'fastpay' ? 'FastPay' : 'Asset'}</span></div>
            <div className="pf-row"><span className="pf-rk">Settles in</span><span className="pf-rv">{settleLabel}</span></div>
            <div className="pf-row"><span className="pf-rk">Visibility</span><span className="pf-rv">Public on the explorer</span></div>
            <div className="pf-row"><span className="pf-rk">Network fee</span><span className="pf-rv">≈ 0.001 PFT</span></div>
            <div style={{ borderTop: '1px solid var(--border-soft)', paddingTop: 12, fontSize: 13, color: 'var(--muted)', lineHeight: 1.5 }}>
              {lane === 'account'
                ? 'The account lane carries the full balance and finalizes through Cobalt certification. Use it for any standard transfer.'
                : lane === 'fastpay'
                  ? 'FastPay moves owned objects directly for near-instant settlement. Best for small, frequent payments.'
                  : 'Send issued assets (pfUSDC, a651, etc.) to another account. Settles with Cobalt finality.'}
            </div>
          </div>
        </div>

        {/* review card */}
        {quote && (
          <div className="pf-card" style={{ marginTop: 20, display: 'grid', gap: 12, borderColor: 'var(--green-border)', background: 'var(--green-soft)' }}>
            <div className="pf-eyebrow">Review</div>
            {quote._isFastpay ? (
              <>
                <div className="pf-row"><span className="pf-rk">You send</span><span className="pf-rv">{amt} PFT</span></div>
                <div className="pf-row"><span className="pf-rk">Network fee</span><span className="pf-rv">{formatBalance(quote._fee)} PFT</span></div>
                <div className="pf-row"><span className="pf-rk">Recipient key</span><span className="pf-rv" style={{ fontFamily: 'var(--mono)', fontSize: 11 }}>{quote._recipientPubkeyHex?.slice(0, 16)}…</span></div>
                <div className="pf-row"><span className="pf-rk">Input object</span><span className="pf-rv" style={{ fontFamily: 'var(--mono)', fontSize: 11 }}>{quote._input.id?.slice(0, 16)}…</span></div>
                <div className="pf-row"><span className="pf-rk">Lane</span><span className="pf-rv">FastPay (consensusless)</span></div>
                <div className="pf-notice">Signs the owned-transfer order, collects validator votes, and applies the certificate.</div>
              </>
            ) : (
              <>
                <div className="pf-row"><span className="pf-rk">You send</span><span className="pf-rv">{amt} {quote._isAsset ? getAssetCode(selectedAsset) : 'PFT'}</span></div>
                <div className="pf-row"><span className="pf-rk">Network fee</span><span className="pf-rv">{formatBalance(quote.minimum_fee)} PFT</span></div>
                {!quote._isAsset && quote.sender_balance_after_amount_and_fee !== undefined && (
                  <div className="pf-row"><span className="pf-rk">Balance after</span><span className="pf-rv">{formatBalance(quote.sender_balance_after_amount_and_fee)} PFT</span></div>
                )}
                <div className="pf-row"><span className="pf-rk">Sequence</span><span className="pf-rv">{quote.sequence}</span></div>
                {!quote._isAsset && hasMemoFields(quote._memoInput) && (
                  <>
                    <div className="pf-row"><span className="pf-rk">Payment version</span><span className="pf-rv">v2 memo</span></div>
                    {quote._memoInput.memo_type && (
                      <div className="pf-row"><span className="pf-rk">Memo type</span><span className="pf-rv">{quote._memoInput.memo_type}</span></div>
                    )}
                    {quote._memoInput.memo_format && (
                      <div className="pf-row"><span className="pf-rk">Memo format</span><span className="pf-rv">{quote._memoInput.memo_format}</span></div>
                    )}
                    {quote._memoInput.memo_data && (
                      <div className="pf-row"><span className="pf-rk">Memo data</span><span className="pf-rv">{quote._memoInput.memo_data}</span></div>
                    )}
                  </>
                )}
                {!quote._isAsset && quote.recipient_exists !== undefined && (
                  <div className="pf-row"><span className="pf-rk">Recipient exists</span><span className="pf-rv">{quote.recipient_exists ? 'Yes' : 'No, account will be created'}</span></div>
                )}
                {!quote._isAsset && quote.sender_meets_reserve_after_transfer === false && (
                  <div className="pf-warning">Insufficient balance after transfer.</div>
                )}
              </>
            )}
            <button className="pf-primary" onClick={handleConfirm} disabled={signing || (!quote._isFastpay && !quote._isAsset && quote.sender_meets_reserve_after_transfer === false)}>
              {signing ? 'Signing and submitting…' : 'Confirm and Sign'}
            </button>
          </div>
        )}

        {error && <div className="pf-error" style={{ marginTop: 16 }}>{error}</div>}
        {success && <div className="pf-success" style={{ marginTop: 16 }}>{success}</div>}
      </div>
    </div>
  );
}
