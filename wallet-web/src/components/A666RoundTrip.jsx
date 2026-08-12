import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  A666_ROUNDTRIP_AMOUNT,
  loadA666RoundtripStatus,
  startA666Roundtrip,
} from '../lib/a666-roundtrip.js';

const ROUTE_STAGES = [
  ['deposit', 'Ethereum USDC deposit'],
  ['bridge_in', 'pfUSDC finalized on PFTL'],
  ['subscribe', 'A666 issued at verified NAV'],
  ['export', 'A666 exported as wA666'],
  ['uniswap_forward', 'wA666 sold on Uniswap'],
  ['uniswap_reverse', 'USDC buys wA666 back'],
  ['return_import', 'wA666 returned as PFTL A666'],
  ['redeem', 'A666 redeemed at verified NAV'],
  ['bridge_out', 'pfUSDC withdrawn as Ethereum USDC'],
  ['reconcile', 'Terminal conservation reconciled'],
];

function displayAtoms(atoms, precision = 6) {
  try {
    const scale = 10n ** BigInt(precision);
    const value = BigInt(String(atoms));
    return `${value / scale}.${(value % scale).toString().padStart(precision, '0')}`;
  } catch (_) { return '—'; }
}

function outcome(status) {
  if (status?.active?.status === 'PASS' || status?.active?.result?.verdict === 'PASS') return 'PASS';
  if (status?.active?.status === 'RUNNING') return 'RUNNING';
  if (status?.preflight?.ready === true) return 'READY';
  return 'BLOCKED';
}

export default function A666RoundTrip({ proxyAuthToken = '', onAuthorize = null }) {
  const [status, setStatus] = useState(null);
  const [loading, setLoading] = useState(true);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState('');
  const refreshInFlight = useRef(false);

  const refresh = useCallback(async ({ quiet = false } = {}) => {
    if (refreshInFlight.current || !proxyAuthToken) {
      if (!proxyAuthToken) setLoading(false);
      return;
    }
    refreshInFlight.current = true;
    if (!quiet) setLoading(true);
    try {
      const next = await loadA666RoundtripStatus(proxyAuthToken);
      setStatus(next);
      setError('');
    } catch (failure) {
      setError(failure.message || 'A666 round-trip status is unavailable');
    } finally {
      refreshInFlight.current = false;
      if (!quiet) setLoading(false);
    }
  }, [proxyAuthToken]);

  useEffect(() => {
    let disposed = false;
    let timer = null;
    const poll = async () => {
      if (disposed) return;
      await refresh({ quiet: true });
      if (!disposed) timer = setTimeout(poll, 15_000);
    };
    setLoading(true);
    refresh().finally(() => {
      if (!disposed) timer = setTimeout(poll, 15_000);
    });
    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
    };
  }, [refresh]);

  const run = useCallback(async () => {
    if (!status?.preflight?.ready || starting) return;
    const confirmed = window.confirm(
      `Run the full A666 round trip with exactly ${A666_ROUNDTRIP_AMOUNT} USDC?\n\n`
      + 'This executes Ethereum and PFTL transactions. The protected pre-existing wA666 balance is excluded.',
    );
    if (!confirmed) return;
    setStarting(true);
    setError('');
    try {
      const next = await startA666Roundtrip(proxyAuthToken);
      setStatus(next);
    } catch (failure) {
      setError(failure.message || 'A666 round trip did not start');
    } finally {
      setStarting(false);
    }
  }, [proxyAuthToken, starting, status]);

  const verdict = outcome(status);
  const checks = status?.preflight?.checks || [];
  const failedChecks = checks.filter((check) => check?.ok !== true);
  const progress = new Map((status?.active?.progress || []).map((stage) => [stage.id, stage]));
  const result = status?.active?.result;
  const protectedAtoms = result?.protected_wa666?.final_atoms
    ?? checks.find((check) => check?.name === 'ethereum_wallet')?.wa666_atoms;
  const returnedUsdc = result?.verified_nav_redeem?.pfusdc_output_atoms;
  const loss = useMemo(() => {
    if (returnedUsdc === undefined) return null;
    return 10_000_000n - BigInt(String(returnedUsdc));
  }, [returnedUsdc]);
  const canRun = Boolean(proxyAuthToken && status?.preflight?.ready && verdict !== 'RUNNING' && !starting);

  return (
    <div className="pf-page a666rt-page">
      <header className="a666rt-hero">
        <div>
          <div className="pf-eyebrow">Live cross-chain acceptance</div>
          <h1 className="pf-h1">A666 round trip</h1>
          <p>One bounded 10.000000-USDC loop through verified NAV, Ethereum wA666, Uniswap, and back to Ethereum USDC.</p>
        </div>
        <span className={`pf-pill ${verdict === 'PASS' || verdict === 'READY' ? 'good' : verdict === 'RUNNING' ? 'warn' : 'bad'}`}>
          {loading && !status ? 'CHECKING' : verdict}
        </span>
      </header>

      {!proxyAuthToken && (
        <div className="pf-notice a666rt-auth">
          <span>Authorize this wallet session once to read preflight and run the loop. No transaction JSON is required.</span>
          {onAuthorize && <button type="button" className="pf-ghost" onClick={onAuthorize}>Open More</button>}
        </div>
      )}
      {error && <div className="pf-error">{error}</div>}

      <section className="a666rt-grid">
        <div className="pf-card a666rt-route">
          <div className="a666rt-card-head">
            <div><span>Exact route</span><strong>USDC → pfUSDC → A666 → wA666 → Uniswap → A666 → pfUSDC → USDC</strong></div>
            <button type="button" className="pf-ghost" onClick={() => refresh()} disabled={loading || starting}>Refresh</button>
          </div>
          <ol className="a666rt-stages">
            {ROUTE_STAGES.map(([id, label], index) => {
              const stage = progress.get(id);
              const stageState = stage?.complete ? 'done' : verdict === 'RUNNING' ? 'pending' : '';
              return (
                <li key={id} className={stageState}>
                  <span>{stage?.complete ? '✓' : index + 1}</span>
                  <strong>{stage?.label || label}</strong>
                </li>
              );
            })}
          </ol>
        </div>

        <aside className="pf-card a666rt-action">
          <div className="pf-eyebrow">Fixed live amount</div>
          <strong className="a666rt-amount">{A666_ROUNDTRIP_AMOUNT} <small>USDC</small></strong>
          <div className="a666rt-facts">
            <div><span>Current preflight</span><strong>{status?.preflight?.ready ? 'READY' : 'BLOCKED'}</strong></div>
            <div><span>Checks passed</span><strong>{checks.filter((check) => check?.ok).length}/{checks.length || '—'}</strong></div>
            <div><span>Protected wA666</span><strong>{displayAtoms(protectedAtoms)} wA666</strong></div>
          </div>
          {failedChecks.length > 0 && (
            <div className="a666rt-blockers">
              <span>Execution blocker{failedChecks.length === 1 ? '' : 's'}</span>
              {failedChecks.map((check) => <p key={check.name}><strong>{check.name}</strong>: {check.detail}</p>)}
            </div>
          )}
          <button type="button" className="pf-primary" onClick={run} disabled={!canRun}>
            {starting ? 'Starting…' : verdict === 'RUNNING' ? 'Round trip running…' : `Confirm and run ${A666_ROUNDTRIP_AMOUNT} USDC`}
          </button>
          <small className="a666rt-safety">One confirmation. No pasted payload. The server re-checks every prerequisite before spending.</small>
        </aside>
      </section>

      {result?.verdict === 'PASS' && (
        <section className="pf-card a666rt-result">
          <div className="a666rt-card-head">
            <div><span>Last completed live run</span><strong>PASS · {status.active.workflow_id}</strong></div>
            <span className="pf-pill good">CONSERVATION PASSED</span>
          </div>
          <div className="a666rt-result-grid">
            <div><span>Started with</span><strong>10.000000 USDC</strong></div>
            <div><span>Returned</span><strong>{displayAtoms(returnedUsdc)} USDC</strong></div>
            <div><span>Loop cost</span><strong>{loss === null ? '—' : displayAtoms(loss)} USDC</strong></div>
            <div><span>Protected balance</span><strong>{displayAtoms(result.protected_wa666?.final_atoms)} wA666</strong></div>
          </div>
        </section>
      )}
    </div>
  );
}
