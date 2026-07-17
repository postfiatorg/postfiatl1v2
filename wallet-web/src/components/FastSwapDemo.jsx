import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  fastSwapDemoApi, formatAtoms, navPresentation, receiptPresentation, shorten,
} from '../lib/fastswap-demo.js';

const PROGRESS_STEPS = [
  ['Two owners sign', 'Buyer and liquidity provider authorize one exact exchange.'],
  ['Assets lock together', 'Five validators must agree that both inputs are spendable.'],
  ['Both move or neither', 'A second certificate confirms; a third applies both outputs.'],
  ['Receipt is verified', 'The wallet checks accepted code, all six replicas, and conservation.'],
];

function Fact({ label, value, detail }) {
  return (
    <div className="fs-fact">
      <span>{label}</span>
      <strong>{value}</strong>
      {detail && <small>{detail}</small>}
    </div>
  );
}

function TechnicalDetails({ nav, chain, quote, result }) {
  return (
    <details className="fs-tech">
      <summary>Show cryptographic and chain details</summary>
      <div className="fs-tech-grid">
        <Fact label="Network" value={chain?.chain_id || '—'} detail={`height ${chain?.height ?? '—'}`} />
        <Fact label="Validator view" value={chain?.exact_six ? '6 / 6 identical' : 'not proven'} detail={`quorum is ${chain?.quorum_required ?? 5} / 6`} />
        <Fact label="NAV epoch" value={nav?.epoch ?? '—'} detail={`envelope ${shorten(nav?.market_envelope_hash)}`} />
        <Fact label="FastSwap policy" value={`epoch ${nav?.policy_epoch ?? '—'}`} detail={shorten(nav?.policy_hash)} />
        <Fact label="Quote intent" value={shorten(quote?.quote_id)} detail={`expires at height ${quote?.expires_at_height ?? '—'}`} />
        <Fact label="Terminal receipt" value={result?.receipt?.code || 'not submitted'} detail={result?.swap_id ? shorten(result.swap_id) : 'No mutation yet'} />
      </div>
    </details>
  );
}

export default function FastSwapDemo({ walletAddress }) {
  const [status, setStatus] = useState(null);
  const [quote, setQuote] = useState(null);
  const [result, setResult] = useState(null);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [progress, setProgress] = useState(0);
  const requestedFaucetAddress = new URLSearchParams(window.location.search).get('faucet');
  const [faucetAddress, setFaucetAddress] = useState(requestedFaucetAddress || walletAddress || '');
  const [faucetLoading, setFaucetLoading] = useState(false);
  const [faucetError, setFaucetError] = useState('');
  const [faucetResult, setFaucetResult] = useState(null);
  const progressTimer = useRef(null);

  const nav = quote?.nav || status?.nav;
  const chain = quote?.chain || status?.chain;
  const navView = useMemo(() => navPresentation(nav), [nav]);
  const receipt = receiptPresentation(result);

  const refresh = async () => {
    setLoading(true);
    setError('');
    setResult(null);
    try {
      const [nextStatus, nextQuote] = await Promise.all([
        fastSwapDemoApi.status(), fastSwapDemoApi.quote(),
      ]);
      setStatus(nextStatus);
      setQuote(nextQuote);
      if (new URLSearchParams(window.location.search).get('receipt') === 'latest'
        && nextStatus?.latest_swap) {
        setResult(nextStatus.latest_swap);
      }
    } catch (cause) {
      setError(cause.message || 'Could not verify the live NAV and FastSwap policy.');
      setQuote(null);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
    return () => clearInterval(progressTimer.current);
  }, []);

  useEffect(() => {
    if (!requestedFaucetAddress && walletAddress) setFaucetAddress(walletAddress);
  }, [walletAddress, requestedFaucetAddress]);

  useEffect(() => {
    const address = requestedFaucetAddress || walletAddress;
    if (!/^pf[0-9a-f]{40}$/.test(address || '')) return undefined;
    let active = true;
    fastSwapDemoApi.faucetStatus(address).then((grant) => {
      if (active && grant?.claimed === true) {
        setFaucetAddress(address);
        setFaucetError('');
        setFaucetResult(grant);
      }
    }).catch(() => {
      // A read-only grant lookup is non-gating; the explicit button retains its error path.
    });
    return () => { active = false; };
  }, [walletAddress, requestedFaucetAddress]);

  const requestPft = async () => {
    setFaucetLoading(true);
    setFaucetError('');
    setFaucetResult(null);
    try {
      const terminal = await fastSwapDemoApi.faucet(faucetAddress.trim());
      if (terminal?.receipt?.accepted !== true || terminal.receipt.code !== 'accepted') {
        throw new Error('The faucet did not return an accepted on-chain receipt.');
      }
      setFaucetResult(terminal);
      await refresh();
    } catch (cause) {
      setFaucetError(cause.message || 'The devnet faucet request failed.');
    } finally {
      setFaucetLoading(false);
    }
  };

  const execute = async () => {
    setConfirming(false);
    setSubmitting(true);
    setError('');
    setResult(null);
    setProgress(0);
    progressTimer.current = setInterval(() => {
      setProgress((current) => Math.min(current + 1, PROGRESS_STEPS.length - 1));
    }, 420);
    try {
      const terminal = await fastSwapDemoApi.swap(quote.quote_id);
      setResult(terminal);
      setProgress(PROGRESS_STEPS.length);
      const nextStatus = await fastSwapDemoApi.status();
      setStatus(nextStatus);
      setQuote(null);
    } catch (cause) {
      setError(cause.message || 'The wallet could not prove an accepted terminal receipt.');
      setProgress(0);
    } finally {
      clearInterval(progressTimer.current);
      setSubmitting(false);
    }
  };

  return (
    <section className="fs-page" data-testid="fastswap-demo">
      <div className="fs-hero">
        <div>
          <div className="fs-kicker"><span className="fs-live-dot" /> LIVE DEVNET · DIRECT OTC</div>
          <h1>Buy a651 at verified NAV</h1>
          <p>
            Exchange pfUSDC directly with a liquidity provider. This does <strong>not</strong> route
            through Uniswap, so there is no Uniswap trading fee or pool slippage.
          </p>
        </div>
        <button className="pf-button secondary fs-refresh" onClick={refresh} disabled={loading || submitting}>
          {loading ? 'VERIFYING…' : 'REFRESH PROOF'}
        </button>
      </div>

      <div className={`fs-verdict ${navView.protocolFresh ? 'good' : 'bad'}`}>
        <div className="fs-shield">{navView.protocolFresh ? '✓' : '!'}</div>
        <div>
          <span>CERTIFIED PRICE</span>
          <strong>{navView.price} per a651</strong>
          <small>Epoch {nav?.epoch ?? '—'} · {navView.verdict}</small>
        </div>
        <div className="fs-age">
          <span>HOW OLD IS IT?</span>
          <strong>{navView.ageLabel}</strong>
          <small>{navView.expiresLabel} · {navView.blocksRemaining}</small>
        </div>
      </div>

      <div className="fs-explain">
        <div className="fs-explain-number">1</div>
        <div><strong>First, the wallet verifies the price.</strong><p>All six validators report the same certified NAV policy, reserve-packet age, and chain state.</p></div>
        <div className="fs-explain-number">2</div>
        <div><strong>Then, two owners approve one deal.</strong><p>Your pfUSDC and the provider’s a651 are locked under the same intent.</p></div>
        <div className="fs-explain-number">3</div>
        <div><strong>Finally, both assets move—or neither does.</strong><p>Three 5-of-6 certificates settle in about a second. A receipt code—not a spinner—proves success.</p></div>
      </div>

      <div className="fs-faucet" data-testid="devnet-pft-faucet">
        <div className="fs-faucet-copy">
          <span>DEVNET GAS</span>
          <strong>Need PFT to test the wallet?</strong>
          <p>Request 5,000 PFT atoms from the controlled faucet. One accepted grant per address; the wallet verifies the receipt code and all six validators.</p>
        </div>
        <div className="fs-faucet-form">
          <label htmlFor="fs-faucet-address">Recipient address</label>
          <input
            id="fs-faucet-address"
            value={faucetAddress}
            onChange={(event) => setFaucetAddress(event.target.value)}
            spellCheck="false"
            autoComplete="off"
            placeholder="pf…"
            disabled={faucetLoading}
          />
          <button className="pf-button" onClick={requestPft} disabled={faucetLoading || !faucetAddress.trim()}>
            {faucetLoading ? 'FINALIZING ON 6 VALIDATORS…' : 'REQUEST DEVNET PFT'}
          </button>
        </div>
        {faucetError && <div className="fs-faucet-result bad" role="alert"><strong>NOT FUNDED</strong><span>{faucetError}</span></div>}
        {faucetResult && (
          <div className="fs-faucet-result good" data-testid="faucet-terminal">
            <strong>ACCEPTED · 5,000 ATOMS FUNDED</strong>
            <span>Balance {faucetResult.balance_atoms?.toLocaleString?.() ?? faucetResult.balance_atoms} atoms · receipt code accepted · exact 6/6</span>
            <small>Transaction {shorten(faucetResult.tx_id)}</small>
          </div>
        )}
      </div>

      {error && (
        <div className="fs-terminal rejected" role="alert">
          <div className="fs-terminal-mark">×</div>
          <div><strong>NOT PROVEN</strong><p>{error}</p><small>No success is shown and the quote must be refreshed.</small></div>
        </div>
      )}

      {quote && !result && (
        <div className="fs-trade-card">
          <div className="fs-trade-heading">
            <div><span>YOU SEND</span><strong>{formatAtoms(quote.sell.amount_atoms, quote.sell.decimals)} pfUSDC</strong><small>{quote.sell.amount_atoms} atomic units</small></div>
            <div className="fs-arrow">→</div>
            <div><span>YOU RECEIVE</span><strong>{formatAtoms(quote.buy.amount_atoms, quote.buy.decimals)} a651</strong><small>{quote.buy.amount_atoms} atomic unit</small></div>
          </div>
          <div className="fs-fee-row">
            <span>Uniswap trade</span><strong>Not used</strong>
            <span>Uniswap fee</span><strong>$0.00</strong>
            <span>Price source</span><strong>Certified NAV</strong>
          </div>
          <div className="fs-rounding-note"><strong>Why do the tiny numbers look uneven?</strong> {quote.rounding_note}</div>
          <div className="fs-demo-custody">
            <strong>Demo custody:</strong> this live devnet proof uses two funded controlled accounts.
            Your open browser wallet ({shorten(walletAddress)}) observes the exchange; production wiring
            moves the buyer signature into this wallet.
          </div>
          <button
            className="pf-button fs-confirm"
            disabled={!navView.protocolFresh || submitting}
            onClick={() => setConfirming(true)}
          >
            REVIEW &amp; CONFIRM LIVE SWAP
          </button>
        </div>
      )}

      {submitting && (
        <div className="fs-progress" aria-live="polite">
          <div className="fs-progress-title"><span className="fs-spinner" /> Settling the atomic swap…</div>
          {PROGRESS_STEPS.map(([title, detail], index) => (
            <div key={title} className={`fs-progress-step ${index < progress ? 'done' : index === progress ? 'active' : ''}`}>
              <span>{index < progress ? '✓' : index + 1}</span><div><strong>{title}</strong><small>{detail}</small></div>
            </div>
          ))}
        </div>
      )}

      {result && (
        <div className={`fs-terminal ${receipt.tone}`} data-testid="fastswap-terminal">
          <div className="fs-terminal-mark">{receipt.tone === 'accepted' ? '✓' : '!'}</div>
          <div className="fs-terminal-main">
            <span>ON-CHAIN RECEIPT</span>
            <h2>{receipt.label}</h2>
            <p>{receipt.message}</p>
            <div className="fs-check-grid">
              <span>✓ Two owner signatures</span><span>✓ Three quorum certificates</span>
              <span>✓ Applied on all 6 validators</span><span>✓ Both asset totals conserved</span>
            </div>
            <div className="fs-result-speed">
              FastSwap settlement <strong>{result.timings_ms?.settlement ?? '—'} ms</strong>
              <span>Total including exact-six audit {result.timings_ms?.total_with_exact_six_audit ?? '—'} ms</span>
            </div>
            <button className="pf-button secondary" onClick={refresh}>PREPARE ANOTHER DEMO</button>
          </div>
        </div>
      )}

      <TechnicalDetails nav={nav} chain={chain} quote={quote} result={result} />

      {confirming && (
        <div className="fs-modal-backdrop" role="presentation" onMouseDown={() => setConfirming(false)}>
          <div className="fs-modal" role="dialog" aria-modal="true" aria-labelledby="fs-confirm-title" onMouseDown={(event) => event.stopPropagation()}>
            <div className="fs-modal-icon">⇄</div>
            <h2 id="fs-confirm-title">Confirm a real devnet exchange</h2>
            <p>The wallet will ask both controlled owners to sign and submit three FastSwap certificate rounds.</p>
            <div className="fs-modal-summary">
              <span>Pay</span><strong>{formatAtoms(quote.sell.amount_atoms, 8)} pfUSDC</strong>
              <span>Receive</span><strong>{formatAtoms(quote.buy.amount_atoms, 8)} a651</strong>
              <span>Certified NAV</span><strong>{navView.price}</strong>
              <span>Failure behavior</span><strong>Both or neither</strong>
            </div>
            <button className="pf-button fs-confirm" onClick={execute}>YES, EXECUTE LIVE SWAP</button>
            <button className="pf-button secondary" onClick={() => setConfirming(false)}>CANCEL</button>
          </div>
        </div>
      )}
    </section>
  );
}
