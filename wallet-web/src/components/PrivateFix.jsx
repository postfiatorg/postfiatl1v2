import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { ArrowRight, Check, Loader2, Lock, RefreshCw, ShieldCheck } from 'lucide-react';

import {
  createPnokFixClientRequestId,
  createPnokFixJob,
  formatAssetAtoms,
  loadPnokFixJob,
  loadPnokFixReadiness,
  verifyPnokFixQuote,
  waitForPnokFixJob,
} from '../lib/pnok-fix.js';
import { truncateMiddle } from '../lib/utils.js';

const RECOVERY_KEY = 'postfiat.pnok_private_fix.active_job.v1';
const TERMINAL = new Set(['accepted', 'failed']);

function readRecovery() {
  try {
    const value = JSON.parse(localStorage.getItem(RECOVERY_KEY) || 'null');
    return value && typeof value === 'object' ? value : null;
  } catch (_) { return null; }
}

function writeRecovery(value) {
  localStorage.setItem(RECOVERY_KEY, JSON.stringify(value));
}

function stageLabel(job) {
  if (!job) return 'Ready to execute';
  if (job.status === 'accepted') return 'Private swap complete';
  if (job.status === 'failed') return 'Execution stopped safely';
  const labels = {
    created: 'Durable intent created',
    quote_verified: 'Finalized FIX verified',
    action_built: 'Private proof verified locally',
    reservation_finalized: 'Capacity reserved on PFTL',
    batch_built: 'Shielded batch prepared',
    submitted: 'Waiting for PFTL finality',
    finalized: 'Private swap finalized',
    local_finalized: 'Scanning owned output',
    complete: 'Private swap complete',
  };
  return labels[job.execution_stage] || 'Durable private execution running';
}

export default function PrivateFix({ rpc, proxyAuthToken = '' }) {
  const [readiness, setReadiness] = useState(null);
  const [market, setMarket] = useState(null);
  const [job, setJob] = useState(null);
  const [loading, setLoading] = useState(true);
  const [executing, setExecuting] = useState(false);
  const [error, setError] = useState('');

  const refresh = useCallback(async () => {
    if (!rpc) return;
    setLoading(true);
    setError('');
    try {
      const ready = await loadPnokFixReadiness();
      const list = await rpc.fxFixList({
        baseAssetId: ready.base_asset_id,
        quoteAssetId: ready.quote_asset_id,
        activeOnly: true,
        limit: 16,
      });
      const rows = list?.ok === true && Array.isArray(list.result?.fixes) ? list.result.fixes : [];
      if (rows.length !== 1) throw new Error('asset pair does not resolve to exactly one active demo FIX');
      const hash = rows[0]?.state?.packet?.packet_hash;
      const quote = await rpc.fxFixQuote(hash, ready.base_atoms);
      setReadiness(ready);
      setMarket(verifyPnokFixQuote(ready, list, quote));
    } catch (failure) {
      setReadiness(null);
      setMarket(null);
      setError(failure.message || 'The private FIX market is unavailable');
    } finally {
      setLoading(false);
    }
  }, [rpc]);

  useEffect(() => { refresh(); }, [refresh]);

  const refreshCompletedMarket = useCallback(async (current) => {
    if (!rpc || current?.status !== 'accepted' || !current?.fix_packet_hash) return;
    const info = await rpc.fxFixInfo(current.fix_packet_hash);
    const finalized = info?.ok === true && info.result?.found === true ? info.result.fix : null;
    if (!finalized) throw new Error('finalized FIX state is unavailable after private execution');
    setMarket((previous) => {
      if (!previous || previous.packet?.packet_hash !== current.fix_packet_hash) return previous;
      return { ...previous, row: finalized, packet: finalized.state?.packet || previous.packet };
    });
  }, [rpc]);

  const resume = useCallback(async (recovery, signal) => {
    if (!recovery) return;
    setExecuting(true);
    try {
      let current;
      if (recovery.job_id) {
        current = await loadPnokFixJob(recovery.job_id);
      } else {
        if (!proxyAuthToken) throw new Error('Authenticated localhost wallet session is unavailable');
        current = await createPnokFixJob({
          clientRequestId: recovery.client_request_id,
          baseAssetId: recovery.base_asset_id,
          quoteAssetId: recovery.quote_asset_id,
          baseAtoms: recovery.base_atoms,
          proxyAuthToken,
        });
        recovery = { ...recovery, job_id: current.job_id };
        writeRecovery(recovery);
      }
      if (current.status !== 'accepted') setJob(current);
      if (!TERMINAL.has(current.status)) {
        current = await waitForPnokFixJob(current.job_id, {
          signal,
          onStatus: (status) => {
            if (status.status !== 'accepted') setJob(status);
          },
        });
      }
      if (current.status === 'failed') {
        setJob(current);
        throw new Error(current.message || 'Private execution stopped safely');
      }
      await refreshCompletedMarket(current);
      setJob(current);
    } finally {
      setExecuting(false);
    }
  }, [proxyAuthToken, refreshCompletedMarket]);

  useEffect(() => {
    const recovery = readRecovery();
    if (!recovery) return undefined;
    const controller = new AbortController();
    resume(recovery, controller.signal).catch((failure) => {
      if (failure.name !== 'AbortError') setError(failure.message || 'Could not recover private execution');
    });
    return () => controller.abort();
  }, [resume]);

  const execute = useCallback(async () => {
    if (!readiness || !market || executing) return;
    setError('');
    const recovery = {
      schema: 'postfiat-pnok-private-fix-browser-recovery-v1',
      client_request_id: createPnokFixClientRequestId(),
      base_asset_id: readiness.base_asset_id,
      quote_asset_id: readiness.quote_asset_id,
      base_atoms: String(readiness.base_atoms),
      job_id: null,
    };
    writeRecovery(recovery);
    const controller = new AbortController();
    try {
      await resume(recovery, controller.signal);
      await refresh();
    } catch (failure) {
      if (failure.name !== 'AbortError') setError(failure.message || 'Private execution failed');
    }
  }, [executing, market, readiness, refresh, resume]);

  const packet = market?.packet;
  const quote = market?.quote;
  const baseDisplay = useMemo(() => formatAssetAtoms(readiness?.base_atoms, readiness?.base_precision), [readiness]);
  const quoteDisplay = useMemo(() => formatAssetAtoms(readiness?.quote_atoms, readiness?.quote_precision), [readiness]);
  const accepted = job?.status === 'accepted';
  const canExecute = Boolean(readiness?.ready && market && proxyAuthToken && !executing && !accepted);

  return (
    <div className="pf-page pf-swap-page">
      <div className="pfs-shell wallet-process-shell">
        <main className="pfs-main">
          <header className="pfs-header">
            <div className="pf-eyebrow">Private foreign exchange</div>
            <h1>{readiness?.base_symbol || 'pfUSDC'} → {readiness?.quote_symbol || 'pNOK'}</h1>
            <p>
              Acquire sandbox WNOK-backed pNOK at a finalized public demo fix while the two
              input notes, amounts, owners, and output ownership execute privately on PFTL.
            </p>
          </header>

          {error && <div className="pf-error">{error}</div>}

          <div className={`pfs-readiness${readiness?.ready && market ? ' ready' : ''}`}>
            <div>
              <span>{loading ? 'VERIFYING' : readiness?.ready && market ? 'FIX VERIFIED' : 'ACTION BLOCKED'}</span>
              <strong>{loading ? 'Reading the finalized FIX and resident prover…' : stageLabel(job)}</strong>
            </div>
            <button className="pfb-secondary small" type="button" onClick={refresh} disabled={loading || executing}>
              {loading ? <Loader2 size={14} className="pfb-spin" /> : <RefreshCw size={14} />} Refresh
            </button>
          </div>

          <section className="pfs-card" style={{ marginTop: 16 }}>
            <div className="pfs-route-head">
              <span><ShieldCheck size={15} /> Demo fix</span>
              <div className="pfs-pill-row">
                <span className="pf-pill good"><Lock size={11} /> private on PFTL</span>
                <span className="pf-pill warn">controlled sandbox checkpoint</span>
              </div>
            </div>
            <div className="pfs-detail-list">
              <div><span>You exchange</span><strong>{baseDisplay} {readiness?.base_symbol || 'pfUSDC'}</strong></div>
              <div><span>You receive exactly</span><strong>{quoteDisplay} {readiness?.quote_symbol || 'pNOK'}</strong></div>
              <div><span>Public demo fix</span><strong>10.500000 pNOK/pfUSDC</strong></div>
              <div><span>Fee</span><strong>0 {readiness?.base_symbol || 'pfUSDC'}</strong></div>
              <div><span>Price impact</span><strong>0 bps</strong></div>
              <div><span>FIX epoch</span><strong>{packet?.epoch ?? '—'}</strong></div>
              <div><span>Expires at PFTL height</span><strong>{packet?.expires_at_height ?? '—'}</strong></div>
              <div><span>Remaining bounded fills</span><strong>{market?.row?.remaining_fill_slots ?? '—'}</strong></div>
              <div><span>Packet</span><strong>{packet?.packet_hash ? truncateMiddle(packet.packet_hash, 10) : '—'}</strong></div>
              <div><span>PFTL height checked</span><strong>{quote?.current_height ?? '—'}</strong></div>
            </div>

            {job && (
              <div className={`pf-notice${accepted ? ' good' : ''}`} style={{ marginTop: 14 }}>
                <strong>{stageLabel(job)}</strong><br />
                <span>Durable job {truncateMiddle(job.job_id, 10)}</span>
                {job.reservation_id && <><br /><span>Reservation {truncateMiddle(job.reservation_id, 10)}</span></>}
                {accepted && <><br /><span><Check size={13} /> 210 pNOK scanned for the controlled demo wallet.</span></>}
              </div>
            )}

            <button className="pf-primary" type="button" onClick={execute} disabled={!canExecute} style={{ marginTop: 16 }}>
              {executing ? <><Loader2 size={15} className="pfb-spin" /> {stageLabel(job)}</> : accepted
                ? <><Check size={15} /> Private swap complete</>
                : <>Privately swap {baseDisplay} {readiness?.base_symbol || 'pfUSDC'} <ArrowRight size={15} /></>}
            </button>
            {!proxyAuthToken && <p className="pf-hint">Authenticated localhost wallet session is required.</p>}
            <p className="pf-hint">
              Controlled demo only. The isolated runner authorizes both test participants; this does not claim
              Tier-4 source finality, unattended production custody, or an official Norges Bank fixing.
            </p>
          </section>
        </main>

        <aside className="pfs-side">
          <section className="pfs-card">
            <div className="pfs-route-head"><span>WHAT IS PUBLIC</span></div>
            <p>The asset pair, demo fix, zero fee, expiry, reservation, and final action identifiers.</p>
          </section>
          <section className="pfs-card">
            <div className="pfs-route-head"><span>WHAT STAYS PRIVATE</span></div>
            <p>
              The two note openings, spend authorization material, owners, input amounts in the proof,
              and output ownership. Because this demo exposes one exact trade size, an observer may still
              infer the amount from timing and the public FIX; this is not anonymity against correlation.
            </p>
          </section>
        </aside>
      </div>
    </div>
  );
}
