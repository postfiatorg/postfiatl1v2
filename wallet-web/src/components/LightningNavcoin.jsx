import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { QRCodeSVG } from 'qrcode.react';

import {
  CONTROLLED_CLAIM,
  LIGHTNING_TO_PFTL,
  PFTL_TO_LIGHTNING,
  LightningNavcoinClient,
  assertFinalizedEscrowMatches,
  assertOfframpLockExecutable,
  buildPayerFeeAcknowledgement,
  buildEscrowCancelOperation,
  buildEscrowCreateOperation,
  buildEscrowFinishOperation,
  safeToRevealInvoice,
  verifyPhoenixPreimage,
} from '../lib/lightning-navcoin.js';

const TERMINAL_STATES = new Set(['PFTL_FINISH_FINAL', 'PFTL_CANCEL_FINAL', 'FAILED_TERMINAL']);
const FINISHABLE_STATES = new Set(['LN_SETTLED']);

function short(value, left = 12, right = 10) {
  const text = String(value || '');
  if (!text) return '—';
  return text.length > left + right + 1 ? `${text.slice(0, left)}…${text.slice(-right)}` : text;
}

function exact(value) {
  return value === null || value === undefined || value === '' ? '—' : String(value);
}

function expiryLabel(unix) {
  if (!Number.isSafeInteger(unix)) return '—';
  const left = unix - Math.floor(Date.now() / 1000);
  if (left <= 0) return `${new Date(unix * 1000).toISOString()} · expired`;
  return `${new Date(unix * 1000).toISOString()} · ${Math.floor(left / 60)}m ${left % 60}s left`;
}

function assertSameSwap(before, after) {
  if (
    after.swapId !== before.swapId
    || after.direction !== before.direction
    || after.invoiceAmountMsat !== before.invoiceAmountMsat
    || after.walletAddress !== before.walletAddress
    || after.pftlAmountAtoms !== before.pftlAmountAtoms
    || after.paymentHash !== before.paymentHash
  ) {
    throw new Error('coordinator changed immutable swap terms after the PFTL receipt');
  }
  return after;
}

function lightningState(snapshot) {
  const state = snapshot?.lightning?.state || snapshot?.lightning?.status;
  if (typeof state === 'string' && state) return state;
  if (snapshot?.state?.startsWith('LN_')) return snapshot.state;
  return 'NOT STARTED';
}

function StatusRow({ label, value, title = '' }) {
  return (
    <div className="lnnav-fact">
      <span>{label}</span>
      <strong title={title || String(value || '')}>{value}</strong>
    </div>
  );
}

function Step({ number, title, detail, state = 'pending' }) {
  return (
    <div className={`lnnav-step ${state}`}>
      <span className="lnnav-step-number">{state === 'done' ? '✓' : number}</span>
      <div>
        <strong>{title}</strong>
        <small>{detail}</small>
      </div>
    </div>
  );
}

export default function LightningNavcoin({
  rpc,
  txBuilder,
  backupJson,
  address,
  onToast,
}) {
  const client = useMemo(() => new LightningNavcoinClient(), []);
  const [direction, setDirection] = useState(LIGHTNING_TO_PFTL);
  const [amountSats, setAmountSats] = useState('1000');
  const [payerFeeSats, setPayerFeeSats] = useState('0');
  const [payerFeeAcknowledged, setPayerFeeAcknowledged] = useState(false);
  const [finalPayerFeeSats, setFinalPayerFeeSats] = useState('0');
  const [payerFeeEvidence, setPayerFeeEvidence] = useState(null);
  const [phoenixInvoice, setPhoenixInvoice] = useState('');
  const [status, setStatus] = useState(null);
  const [snapshot, setSnapshot] = useState(null);
  const [independentEscrow, setIndependentEscrow] = useState(null);
  const [preimage, setPreimage] = useState('');
  const [localReceipt, setLocalReceipt] = useState(null);
  const [loading, setLoading] = useState('');
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');

  const refreshStatus = useCallback(async () => {
    try {
      const next = await client.status();
      setStatus(next);
      setError('');
      return next;
    } catch (cause) {
      setStatus(null);
      setError(`Coordinator status unavailable: ${cause.message}`);
      return null;
    }
  }, [client]);

  const refreshSwap = useCallback(async () => {
    if (!snapshot?.swapId) return null;
    try {
      const next = await client.swap(snapshot.swapId);
      if (
        next.direction !== snapshot.direction
        || next.invoiceAmountMsat !== snapshot.invoiceAmountMsat
        || next.walletAddress !== snapshot.walletAddress
        || next.pftlAmountAtoms !== snapshot.pftlAmountAtoms
        || next.paymentHash !== snapshot.paymentHash
      ) {
        throw new Error('coordinator changed immutable swap terms while polling');
      }
      setSnapshot(next);
      setError('');
      return next;
    } catch (cause) {
      setError(`Secret-free swap poll failed: ${cause.message}`);
      return null;
    }
  }, [
    client,
    snapshot?.swapId,
    snapshot?.direction,
    snapshot?.invoiceAmountMsat,
    snapshot?.walletAddress,
    snapshot?.pftlAmountAtoms,
    snapshot?.paymentHash,
  ]);

  const verifyIndependentOpenEscrow = useCallback(async current => {
    if (!rpc) throw new Error('independent PFTL RPC is unavailable');
    const coordinatorStatus = await client.status();
    const statusBefore = await rpc.status();
    const escrowResponse = await rpc.escrowInfo(current.quote.escrowId);
    const statusAfter = await rpc.status();
    const escrow = assertFinalizedEscrowMatches(
      current,
      escrowResponse,
      address,
      coordinatorStatus,
      statusBefore,
      statusAfter,
    );
    setStatus(coordinatorStatus);
    return escrow;
  }, [address, client, rpc]);

  useEffect(() => {
    let disposed = false;
    let timer = null;
    const poll = async () => {
      await refreshStatus();
      if (!disposed) timer = setTimeout(poll, 5000);
    };
    poll();
    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
    };
  }, [refreshStatus]);

  useEffect(() => {
    if (!snapshot?.swapId || TERMINAL_STATES.has(snapshot.state)) return undefined;
    let disposed = false;
    let timer = null;
    const poll = async () => {
      await refreshSwap();
      if (!disposed) timer = setTimeout(poll, 2000);
    };
    timer = setTimeout(poll, 1000);
    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
    };
  }, [refreshSwap, snapshot?.swapId, snapshot?.state]);

  useEffect(() => {
    if (!snapshot?.swapId || !TERMINAL_STATES.has(snapshot.state)) return;
    try {
      client.clearQuoteRequest(snapshot.swapId);
    } catch (cause) {
      setError(`Durable quote-request cleanup failed: ${cause.message}`);
    }
  }, [client, snapshot?.state, snapshot?.swapId]);

  useEffect(() => {
    setIndependentEscrow(null);
    const quote = snapshot?.quote;
    if (
      !quote
      || quote.direction !== LIGHTNING_TO_PFTL
      || snapshot.state !== 'PFTL_LOCK_FINAL'
      || !rpc
    ) return undefined;

    let disposed = false;
    (async () => {
      try {
        const escrow = await verifyIndependentOpenEscrow(snapshot);
        if (!disposed) setIndependentEscrow({ verified: true, escrow });
      } catch (cause) {
        if (!disposed) {
          setIndependentEscrow({ verified: false, error: cause.message });
          setError(`Independent PFTL escrow check failed: ${cause.message}`);
        }
      }
    })();
    return () => { disposed = true; };
  }, [
    address,
    rpc,
    verifyIndependentOpenEscrow,
    snapshot?.swapId,
    snapshot?.quote?.escrowId,
    snapshot?.state,
  ]);

  const resetRun = (nextDirection = direction) => {
    try {
      client.clearQuoteRequest();
    } catch (cause) {
      setError(`Durable quote-request reset failed: ${cause.message}`);
      return;
    }
    setDirection(nextDirection);
    setSnapshot(null);
    setIndependentEscrow(null);
    setPreimage('');
    setLocalReceipt(null);
    setError('');
    setNotice('');
    setPayerFeeAcknowledged(false);
    setFinalPayerFeeSats('0');
    setPayerFeeEvidence(null);
  };

  const requestQuote = async () => {
    setError('');
    setNotice('');
    setLocalReceipt(null);
    setIndependentEscrow(null);
    if (!address) {
      setError('Wallet address is unavailable');
      return;
    }
    if (!/^[1-9][0-9]*$/.test(amountSats)) {
      setError('Amount must be a positive whole number of satoshis');
      return;
    }
    const amountMsat = BigInt(amountSats) * 1000n;
    if (direction === LIGHTNING_TO_PFTL) {
      if (!/^(0|[1-9][0-9]*)$/.test(payerFeeSats)) {
        setError('Displayed Lightning wallet/LSP fee must be a whole number of satoshis');
        return;
      }
      if (!payerFeeAcknowledged) {
        setError('Acknowledge the displayed payer-wallet fee before requesting a real-value quote');
        return;
      }
      try {
        buildPayerFeeAcknowledgement(
          status,
          amountMsat.toString(),
          payerFeeSats,
        );
      } catch (cause) {
        setError(cause.message);
        return;
      }
    }
    if (status?.maxAmountMsat !== null && amountMsat > BigInt(status.maxAmountMsat)) {
      setError(`Amount exceeds the coordinator hard cap of ${status.maxAmountMsat} msat`);
      return;
    }

    setLoading('quote');
    try {
      const next = await client.createQuote({
        direction,
        amountMsat: amountMsat.toString(),
        walletAddress: address,
        ...(direction === PFTL_TO_LIGHTNING ? { invoice: phoenixInvoice.trim() } : {}),
      });
      if (next.walletAddress !== address) {
        throw new Error('coordinator quote is not bound to this wallet');
      }
      setSnapshot(next);
      setFinalPayerFeeSats(payerFeeSats);
      setPayerFeeEvidence(null);
      setNotice(next.quote
        ? (next.canExecute
            ? 'Signed quote received. Value cannot move until every displayed gate is green.'
            : 'Quote is non-executable and remains on HOLD.')
        : 'Quote terms are fixed; the payable invoice remains withheld pending nazgul authorization and PFTL lock finality.');
    } catch (cause) {
      setError(cause.message);
    } finally {
      setLoading('');
    }
  };

  const copyInvoice = async () => {
    const invoice = snapshot?.quote?.invoice;
    if (!invoice || !invoiceIsVisible) return;
    try {
      await navigator.clipboard.writeText(invoice);
      onToast?.('Mainnet Lightning invoice copied');
    } catch (_) {
      setError('Clipboard write was denied; select and copy the invoice manually');
    }
  };

  const acknowledgeFinalPayerFee = checked => {
    setError('');
    if (!checked) {
      setPayerFeeEvidence(null);
      return;
    }
    try {
      if (!snapshot?.quote || snapshot.quote.direction !== LIGHTNING_TO_PFTL) {
        throw new Error('a signed on-ramp quote is required for final fee acknowledgement');
      }
      const evidence = buildPayerFeeAcknowledgement(
        status,
        snapshot.invoiceAmountMsat,
        finalPayerFeeSats,
      );
      setPayerFeeEvidence(Object.freeze({
        ...evidence,
        swap_id: snapshot.swapId,
        payment_hash: snapshot.paymentHash,
      }));
    } catch (cause) {
      setPayerFeeEvidence(null);
      setError(cause.message);
    }
  };

  const finishOnramp = async () => {
    setError('');
    setNotice('');
    if (!snapshot || snapshot.quote.direction !== LIGHTNING_TO_PFTL) return;
    if (!FINISHABLE_STATES.has(snapshot.state)) {
      setError('The swap is not in a locally finishable state');
      return;
    }
    if (!backupJson || !txBuilder || !rpc) {
      setError('The unlocked wallet and PFTL transaction builder are required');
      return;
    }

    setLoading('finish');
    try {
      const proof = await verifyPhoenixPreimage(preimage, snapshot.quote.paymentHash);
      const escrow = await verifyIndependentOpenEscrow(snapshot);
      setIndependentEscrow({ verified: true, escrow });
      const operation = buildEscrowFinishOperation(snapshot, proof.fulfillment, address);
      const result = await txBuilder.sendEscrowTransaction(
        backupJson,
        address,
        { operation },
      );
      if (result.receipt?.accepted !== true || result.receipt?.code !== 'accepted') {
        throw new Error('PFTL did not return the literal accepted=true, code=accepted finish receipt');
      }
      setLocalReceipt({
        action: 'escrow_finish',
        txId: result.txId,
        accepted: true,
        code: result.receipt.code,
      });
      const finalized = assertSameSwap(
        snapshot,
        await client.notifyPftlFinish(snapshot.swapId, result.txId),
      );
      setSnapshot(finalized);
      setNotice('Lightning preimage matched locally; PFTL escrow finish is final.');
      onToast?.('NAVcoin delivery ACCEPTED');
    } catch (cause) {
      setError(cause.message);
    } finally {
      // The Lightning preimage is deliberately short-lived browser state.
      setPreimage('');
      setLoading('');
    }
  };

  const lockOfframp = async () => {
    setError('');
    setNotice('');
    if (!snapshot || snapshot.quote.direction !== PFTL_TO_LIGHTNING) return;
    if (!routeReady || !snapshot.canExecute) {
      setError('Real-value execution is HOLD; NAVcoin will not be locked');
      return;
    }
    if (!backupJson || !txBuilder) {
      setError('The unlocked wallet and PFTL transaction builder are required');
      return;
    }

    setLoading('lock');
    try {
      // Re-read the pure status endpoint immediately before local signing.
      // A previously rendered quote cannot authorize a lock after its cutoff.
      const fresh = assertSameSwap(
        snapshot,
        await client.swap(snapshot.swapId),
      );
      assertOfframpLockExecutable(fresh);
      setSnapshot(fresh);
      const operation = buildEscrowCreateOperation(fresh, address);
      const result = await txBuilder.sendEscrowTransaction(
        backupJson,
        address,
        { operation },
        { sequence: fresh.quote.ownerSequence },
      );
      if (result.receipt?.accepted !== true || result.receipt?.code !== 'accepted') {
        throw new Error('PFTL did not return the literal accepted=true, code=accepted lock receipt');
      }
      setLocalReceipt({
        action: 'escrow_create',
        txId: result.txId,
        accepted: true,
        code: result.receipt.code,
      });
      const inFlight = assertSameSwap(
        fresh,
        await client.notifyPftlLock(fresh.swapId, result.txId),
      );
      setSnapshot(inFlight);
      setNotice('NAVcoin escrow lock is final. The coordinator must independently observe quorum before paying Phoenix.');
      onToast?.('NAVcoin lock ACCEPTED');
    } catch (cause) {
      setError(cause.message);
    } finally {
      setLoading('');
    }
  };

  const cancelOfframp = async () => {
    setError('');
    setNotice('');
    if (!snapshot || snapshot.quote.direction !== PFTL_TO_LIGHTNING) return;
    if (!backupJson || !txBuilder) {
      setError('The unlocked wallet and PFTL transaction builder are required');
      return;
    }
    setLoading('cancel');
    try {
      const fresh = assertSameSwap(snapshot, await client.swap(snapshot.swapId));
      const operation = buildEscrowCancelOperation(fresh, address);
      const result = await txBuilder.sendEscrowTransaction(
        backupJson,
        address,
        { operation },
      );
      if (result.receipt?.accepted !== true || result.receipt?.code !== 'accepted') {
        throw new Error('PFTL did not return the literal accepted=true, code=accepted cancel receipt');
      }
      setLocalReceipt({
        action: 'escrow_cancel',
        txId: result.txId,
        accepted: true,
        code: result.receipt.code,
      });
      const finalized = assertSameSwap(
        fresh,
        await client.notifyPftlCancel(fresh.swapId, result.txId),
      );
      setSnapshot(finalized);
      setNotice('Lightning did not settle; the wallet-owned NAVcoin refund is final.');
      onToast?.('NAVcoin refund ACCEPTED');
    } catch (cause) {
      setError(cause.message);
    } finally {
      setLoading('');
    }
  };

  const quote = snapshot?.quote;
  const quorum = snapshot?.pftl?.quorum || status?.pftl?.quorum;
  const invoiceIsVisible = Boolean(
    safeToRevealInvoice(status, snapshot)
    && independentEscrow?.verified === true,
  );
  const finalPayerFeeMsat = /^(0|[1-9][0-9]*)$/.test(finalPayerFeeSats)
    ? (BigInt(finalPayerFeeSats) * 1000n).toString()
    : null;
  const finalPaymentAuthorized = Boolean(
    invoiceIsVisible
    && payerFeeEvidence
    && payerFeeEvidence.swap_id === snapshot?.swapId
    && payerFeeEvidence.payment_hash === snapshot?.paymentHash
    && payerFeeEvidence.principal_msat === snapshot?.invoiceAmountMsat
    && payerFeeEvidence.displayed_fee_msat === finalPayerFeeMsat
    && payerFeeEvidence.coordinator_max_fee_msat === status?.maxFeeMsat
  );
  const receipt = localReceipt || snapshot?.pftl?.receipt;
  const armed = status?.mode === 'ARMED' && status?.canExecute === true;
  const routeReady = Boolean(
    armed
    && status?.pftl?.quorum?.converged
    && status.pftl.quorum.observed >= status.pftl.quorum.required
    && status.pftl.quorum.required === 6
    && status.pftl.quorum.observed === 6
    && status.pftl.quorum.validatorCount === 6
    && status.valuationBinding?.verified === true
    && status.proofAssurance?.profile === 'multi-fetch-quorum'
    && status.proofAssurance?.consensusNativeGroth16Verification === false
    && quote
    && status.pftl.chainId === quote.chainId
    && status.pftl.genesisHash === quote.genesisHash
    && status.pftl.assetId === quote.assetId
    && status.pftl.navEpoch === quote.navEpoch
    && status.pftl.navReservePacketHash === quote.navReservePacketHash
  );
  const pftlFinal = snapshot?.state === 'PFTL_LOCK_FINAL'
    || FINISHABLE_STATES.has(snapshot?.state)
    || snapshot?.state === 'PFTL_FINISH_FINAL';
  const lnSettled = lightningState(snapshot).includes('SETTLED')
    || ['LN_SETTLED', 'PFTL_FINISH_SUBMITTED', 'PFTL_FINISH_FINAL'].includes(snapshot?.state);
  const terminal = snapshot?.state === 'PFTL_FINISH_FINAL';

  return (
    <div className="pf-page lnnav-page">
      <section className="lnnav-hero">
        <div>
          <div className="pf-eyebrow">REAL-VALUE LIGHTNING INTERFACE</div>
          <h1 className="pf-h1">BTC → Lightning → NAVcoin</h1>
          <p>
            Mainnet Lightning invoices meet a PFTL PREIMAGE-SHA-256 escrow.
            The wallet signs PFTL legs locally.
          </p>
        </div>
        <div className="lnnav-labels">
          <span className="pf-pill warn">CONTROLLED</span>
          <span className={`pf-pill ${armed ? 'good' : 'bad'}`}>
            {armed ? 'ARMED' : `${status?.mode || 'HOLD'} · NO VALUE`}
          </span>
        </div>
      </section>

      <div className="lnnav-trust">
        <strong>{CONTROLLED_CLAIM}</strong>
        <span>
          The coordinator is trusted for cross-ledger timing and liquidity. The payment hash
          binds both legs; wallet keys never leave this browser, and the Phoenix preimage is sent
          only in the locally signed PFTL fulfillment—not to the coordinator API.
        </span>
      </div>

      {error && <div className="pf-error">{error}</div>}
      {notice && <div className="pf-success">{notice}</div>}
      {!armed && (
        <div className="pf-notice">
          <strong>DRY-RUN / HOLD.</strong> No payable invoice is exposed and NAVcoin cannot be
          locked. {status?.holdReasons?.length ? status.holdReasons.join(' · ') : 'Operator arming is absent.'}
        </div>
      )}

      <section className="lnnav-grid">
        <div className="pf-card lnnav-main">
          <div className="lnnav-tabs" role="tablist" aria-label="Lightning swap direction">
            <button
              role="tab"
              aria-selected={direction === LIGHTNING_TO_PFTL}
              className={direction === LIGHTNING_TO_PFTL ? 'on' : ''}
              onClick={() => resetRun(LIGHTNING_TO_PFTL)}
            >
              BTC → NAVcoin
            </button>
            <button
              role="tab"
              aria-selected={direction === PFTL_TO_LIGHTNING}
              className={direction === PFTL_TO_LIGHTNING ? 'on' : ''}
              onClick={() => resetRun(PFTL_TO_LIGHTNING)}
            >
              NAVcoin → BTC
            </button>
          </div>

          {!snapshot && (
            <div className="lnnav-form">
              <label htmlFor="lnnav-amount">Amount (whole satoshis)</label>
              <div className="lnnav-amount">
                <input
                  id="lnnav-amount"
                  className="pf-input"
                  inputMode="numeric"
                  autoComplete="off"
                  value={amountSats}
                  onChange={event => setAmountSats(event.target.value)}
                  disabled={Boolean(loading)}
                />
                <span>sats</span>
              </div>
              {direction === LIGHTNING_TO_PFTL && (
                <>
                  <label htmlFor="lnnav-payer-fee">Wallet/LSP fee shown before payment (sats)</label>
                  <input
                    id="lnnav-payer-fee"
                    className="pf-input"
                    inputMode="numeric"
                    autoComplete="off"
                    value={payerFeeSats}
                    onChange={event => {
                      setPayerFeeSats(event.target.value);
                      setPayerFeeAcknowledged(false);
                    }}
                    disabled={Boolean(loading)}
                  />
                  <label className="lnnav-fee-ack">
                    <input
                      type="checkbox"
                      checked={payerFeeAcknowledged}
                      onChange={event => setPayerFeeAcknowledged(event.target.checked)}
                    />
                    I confirm this displayed payer fee; principal + fee remains within the $5 run cap.
                  </label>
                  <small>
                    The coordinator cannot observe or enforce Phoenix/Coinbase routing or LSP fees.
                    Recheck the wallet’s final payment screen and do not pay if its fee differs.
                  </small>
                </>
              )}
              {direction === PFTL_TO_LIGHTNING && (
                <>
                  <label htmlFor="lnnav-phoenix-invoice">
                    Phoenix Bitcoin-mainnet invoice
                  </label>
                  <textarea
                    id="lnnav-phoenix-invoice"
                    className="lnnav-invoice-input"
                    value={phoenixInvoice}
                    onChange={event => setPhoenixInvoice(event.target.value)}
                    spellCheck="false"
                    autoComplete="off"
                    placeholder="lnbc…"
                    disabled={Boolean(loading)}
                  />
                  <small>
                    Use a single-path invoice. AMP is rejected. The invoice is sent only to the
                    same-origin coordinator for strict LND decoding and signed-quote binding.
                  </small>
                </>
              )}
              <button className="pf-primary" onClick={requestQuote} disabled={loading === 'quote' || !status}>
                {loading === 'quote' ? 'Validating…' : armed ? 'Request bounded quote' : 'Run quote dry-check'}
              </button>
            </div>
          )}

          {snapshot && quote && (
            <>
              <div className="lnnav-flow">
                {direction === LIGHTNING_TO_PFTL ? (
                  <>
                    <Step
                      number="1"
                      title="PFTL NAVcoin locked"
                      detail={`${quorum?.observed || 0}/${quorum?.validatorCount || 6} validators · ${snapshot.state}`}
                      state={pftlFinal ? 'done' : 'running'}
                    />
                    <Step
                      number="2"
                      title="Phoenix pays Lightning"
                      detail={
                        lnSettled
                          ? lightningState(snapshot)
                          : finalPaymentAuthorized
                            ? 'final payment-screen fee gate acknowledged'
                            : 'final payment-screen fee gate required'
                      }
                      state={lnSettled ? 'done' : finalPaymentAuthorized ? 'running' : 'pending'}
                    />
                    <Step
                      number="3"
                      title="Wallet claims NAVcoin"
                      detail={receipt?.accepted ? `ACCEPTED · ${receipt.code || 'receipt'}` : 'preimage finish pending'}
                      state={terminal || localReceipt?.action === 'escrow_finish' ? 'done' : 'pending'}
                    />
                  </>
                ) : (
                  <>
                    <Step
                      number="1"
                      title="Wallet locks NAVcoin"
                      detail={receipt?.accepted ? `ACCEPTED · ${receipt.code || 'receipt'}` : 'local signature pending'}
                      state={localReceipt?.action === 'escrow_create' || pftlFinal ? 'done' : 'running'}
                    />
                    <Step
                      number="2"
                      title="Coordinator pays Phoenix"
                      detail={lightningState(snapshot)}
                      state={lnSettled ? 'done' : pftlFinal ? 'running' : 'pending'}
                    />
                    <Step
                      number="3"
                      title="Coordinator claims NAVcoin"
                      detail={terminal ? 'finalized' : 'requires the same Lightning preimage'}
                      state={terminal ? 'done' : 'pending'}
                    />
                  </>
                )}
              </div>

              {direction === LIGHTNING_TO_PFTL && (
                <div className="lnnav-payment">
                  <div className="lnnav-section-title">
                    <span>Mainnet Lightning invoice</span>
                    <span className={`pf-pill ${invoiceIsVisible ? 'good' : 'bad'}`}>
                      {invoiceIsVisible ? 'SAFE TO PRESENT' : 'WITHHELD'}
                    </span>
                  </div>
                  {invoiceIsVisible ? (
                    <>
                      <div className="lnnav-qr" aria-label="Mainnet Lightning invoice QR code">
                        <QRCodeSVG
                          value={quote.invoice.toUpperCase()}
                          size={232}
                          level="M"
                          marginSize={2}
                          title="Scan this Bitcoin mainnet Lightning invoice with Phoenix"
                        />
                      </div>
                      <textarea
                        className="lnnav-invoice"
                        readOnly
                        spellCheck="false"
                        aria-label="Bitcoin mainnet Lightning invoice"
                        value={quote.invoice}
                      />
                      <button className="pf-ghost" onClick={copyInvoice}>Copy for Phoenix</button>
                      <label htmlFor="lnnav-final-payer-fee">
                        Fee on Phoenix’s final payment screen (sats)
                      </label>
                      <input
                        id="lnnav-final-payer-fee"
                        className="pf-input"
                        inputMode="numeric"
                        autoComplete="off"
                        value={finalPayerFeeSats}
                        onChange={event => {
                          setFinalPayerFeeSats(event.target.value);
                          setPayerFeeEvidence(null);
                        }}
                      />
                      <label className="lnnav-fee-ack">
                        <input
                          type="checkbox"
                          checked={finalPaymentAuthorized}
                          onChange={event => acknowledgeFinalPayerFee(event.target.checked)}
                        />
                        I rechecked the final payment screen. This exact fee is within both
                        the reserved fee ceiling and the $5 all-in run cap.
                      </label>
                      <div className={`lnnav-withheld ${finalPaymentAuthorized ? 'good' : ''}`}>
                        <strong>
                          {finalPaymentAuthorized
                            ? 'FINAL PAYMENT GATE ACKNOWLEDGED'
                            : 'DO NOT CONFIRM PAYMENT IN PHOENIX'}
                        </strong>
                        <p>
                          Coordinator fee ceiling: {exact(status?.maxFeeMsat)} msat.
                          {' '}The coordinator cannot observe the payer-side fee; this is an
                          explicit operator/wallet acknowledgement.
                        </p>
                        {payerFeeEvidence && (
                          <p>
                            Evidence: swap {short(payerFeeEvidence.swap_id)} · fee
                            {' '}{payerFeeEvidence.displayed_fee_msat} msat · all-in
                            {' '}{payerFeeEvidence.all_in_usd_e8} USD-e8 ·
                            {' '}{payerFeeEvidence.acknowledged_at_unix}
                          </p>
                        )}
                      </div>
                    </>
                  ) : (
                    <div className="lnnav-withheld">
                      Invoice hidden until ARMING, 6-of-6 convergence, accepted PFTL lock receipt,
                      and an independent finalized escrow read all pass.
                    </div>
                  )}

                  <label htmlFor="lnnav-preimage">Phoenix payment preimage (32-byte hex)</label>
                  <input
                    id="lnnav-preimage"
                    className="pf-input"
                    type="password"
                    value={preimage}
                    onChange={event => setPreimage(event.target.value)}
                    autoComplete="off"
                    spellCheck="false"
                    placeholder="shown by Phoenix after payment"
                  />
                  <small>
                    SHA-256 is checked with WebCrypto against the signed quote. The preimage is
                    encoded into the locally signed PFTL finish and is never sent to the coordinator.
                  </small>
                  <button
                    className="pf-primary"
                    onClick={finishOnramp}
                    disabled={loading === 'finish' || preimage.length !== 64 || !FINISHABLE_STATES.has(snapshot.state)}
                  >
                    {loading === 'finish' ? 'Verifying + signing…' : 'Verify preimage + claim NAVcoin'}
                  </button>
                </div>
              )}

              {direction === PFTL_TO_LIGHTNING && (
                <div className="lnnav-payment">
                  <div className="lnnav-section-title">
                    <span>Wallet-owned PFTL escrow</span>
                    <span className={`pf-pill ${routeReady && snapshot.canExecute ? 'good' : 'bad'}`}>
                      {routeReady && snapshot.canExecute ? 'READY' : 'HOLD'}
                    </span>
                  </div>
                  <p>
                    Locking is the only wallet-signed first leg. The coordinator must observe its
                    finalized six-validator state before LND may pay the bound Phoenix invoice.
                  </p>
                  <button
                    className="pf-primary"
                    onClick={lockOfframp}
                    disabled={
                      loading === 'lock'
                      || !routeReady
                      || !snapshot.canExecute
                      || localReceipt?.action === 'escrow_create'
                    }
                  >
                    {loading === 'lock' ? 'Signing + finalizing…' : 'Lock NAVcoin locally'}
                  </button>
                  {snapshot.state === 'REFUND_ELIGIBLE' && (
                    <button
                      className="pf-primary"
                      onClick={cancelOfframp}
                      disabled={
                        loading === 'cancel'
                        || snapshot.pftl.height === null
                        || snapshot.pftl.height < snapshot.quote.cancelAfter
                      }
                    >
                      {loading === 'cancel' ? 'Signing refund…' : 'Cancel escrow + reclaim NAVcoin'}
                    </button>
                  )}
                </div>
              )}

              <button className="pf-ghost lnnav-new" onClick={() => resetRun(direction)}>
                Close this quote
              </button>
            </>
          )}

          {snapshot && !quote && (
            <div className="lnnav-withheld">
              <strong>Value authorization pending</strong>
              <p>
                Swap {short(snapshot.swapId)} is durably quoted for
                {' '}{snapshot.invoiceAmountMsat} msat and {snapshot.pftlAmountAtoms} NAVcoin
                atoms. No payable invoice or signed execution quote is exposed yet.
              </p>
              <p>
                State: {snapshot.state} · {snapshot.holdReasons.join(' · ') || 'HOLD'}
              </p>
            </div>
          )}
        </div>

        <aside className="pf-card lnnav-evidence">
          <div className="lnnav-section-title">
            <span>Live evidence</span>
            <button className="pf-link" onClick={async () => {
              await refreshStatus();
              if (snapshot) await refreshSwap();
            }}>
              Refresh
            </button>
          </div>
          <StatusRow label="Coordinator mode" value={status?.mode || 'UNAVAILABLE'} />
          <StatusRow
            label="LND"
            value={status?.lnd?.synced_to_chain === true ? 'MAINNET · SYNCED' : 'MAINNET · NOT VERIFIED'}
          />
          <StatusRow
            label="PFTL quorum"
            value={`${quorum?.observed || 0}/${quorum?.validatorCount || 6}${quorum?.converged ? ' · CONVERGED' : ' · HOLD'}`}
          />
          <StatusRow label="Swap state" value={snapshot?.state || 'NO QUOTE'} />
          <StatusRow label="Lightning payment" value={lightningState(snapshot)} />
          <StatusRow
            label="Payment hash"
            value={short(quote?.paymentHash)}
            title={quote?.paymentHash}
          />
          <StatusRow
            label="Lightning amount"
            value={quote ? `${quote.invoiceAmountMsat} msat` : '—'}
          />
          <StatusRow label="Invoice expiry" value={expiryLabel(quote?.invoiceExpires)} />
          <StatusRow
            label="PFTL asset"
            value={short(quote?.assetId || status?.pftl?.raw?.asset_id)}
            title={quote?.assetId || ''}
          />
          <StatusRow
            label="Escrow"
            value={short(quote?.escrowId)}
            title={quote?.escrowId}
          />
          <StatusRow
            label="NAVcoin amount"
            value={quote ? `${quote.amountAtoms} atoms` : '—'}
          />
          <StatusRow
            label="Attested NAV epoch"
            value={quote ? `${quote.navEpoch} · ${short(quote.navReservePacketHash)}` : '—'}
            title={quote?.navReservePacketHash}
          />
          <StatusRow
            label="NAV assurance"
            value={
              status?.valuationBinding?.verified
                ? '6-ledger binding · 1 attestor · CONTROLLED'
                : 'HOLD · UNVERIFIED'
            }
            title="Proof bytes are stored and hash-bound under multi-fetch-quorum; Groth16 is not verified natively by consensus."
          />
          <StatusRow
            label="Refund height"
            value={quote ? exact(quote.cancelAfter) : '—'}
          />
          <StatusRow
            label="PFTL receipt"
            value={receipt ? `${receipt.accepted ? 'ACCEPTED' : 'REJECTED'} · ${receipt.code || 'no code'}` : '—'}
          />
          <StatusRow
            label="Receipt tx"
            value={short(localReceipt?.txId || receipt?.txId)}
            title={localReceipt?.txId || receipt?.txId}
          />
          <StatusRow
            label="NAVcoin balance"
            value={`${exact(snapshot?.pftl?.balanceAtoms)} atoms`}
          />
          <StatusRow
            label="Independent escrow read"
            value={
              independentEscrow === null
                ? 'PENDING'
                : independentEscrow.verified ? 'MATCHED · OPEN' : 'FAILED'
            }
          />
          <div className="lnnav-bound">
            <strong>Bounded real-value policy</strong>
            <span>
              NAV: attested, hash-bound multi-fetch-quorum; not consensus-native
              Groth16 verification.
            </span>
            <span>Per run: {status?.perRunUsdE8 ? `${status.perRunUsdE8} USD-e8` : 'unverified'}</span>
            <span>Total: {status?.totalUsdE8 ? `${status.totalUsdE8} USD-e8` : 'unverified'}</span>
            <span>Network: Bitcoin mainnet Lightning only</span>
          </div>
        </aside>
      </section>
    </div>
  );
}
