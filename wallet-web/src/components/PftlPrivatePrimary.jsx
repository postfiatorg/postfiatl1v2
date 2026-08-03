import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ArrowRight, Check, Loader2, Lock, RefreshCw, ShieldCheck } from 'lucide-react';

import { getWasm } from '../lib/wasm-loader.js';
import {
  buildPftlPrivateIntent,
  completePftlPrivateIntent,
  createPftlPrivateIdempotencyKey,
  createPftlPrivateQuote,
  loadPftlPrivateReadiness,
  loadPftlPrivateRecoveries,
  parsePrivateNavcoinAmountAtoms,
  savePftlPrivateRecoveries,
  signPftlPrivateIntent,
  submitPftlPrivateIntent,
} from '../lib/pftl-private-primary.js';
import { truncateMiddle } from '../lib/utils.js';

const ACTIVE_STATES = new Set([
  'SIGNED', 'JOURNALED', 'PROVING', 'PREPARED', 'PUBLISHED',
  'FAILED_PREPUBLISH', 'INTERRUPTED_PREPUBLISH',
]);

function stageLabel(record) {
  const state = record?.response?.swap?.state || record?.status;
  const labels = {
    SIGNED: 'Intent signed locally',
    JOURNALED: 'Durable intent journaled',
    PROVING: 'Private proofs running',
    PREPARED: 'Private batch prepared',
    PUBLISHED: 'Waiting for six-validator finality',
    COMMITTED: 'Private primary swap committed',
    REJECTED: 'Intent rejected safely',
    FAILED_PREPUBLISH: 'Stopped before publication',
    INTERRUPTED_PREPUBLISH: 'Interrupted safely before publication',
  };
  return labels[state] || 'Ready for a signed private swap';
}

function replaceRecord(records, next) {
  const index = records.findIndex(record => record.idempotency_key === next.idempotency_key);
  if (index < 0) return [...records, next];
  return records.map((record, position) => position === index ? next : record);
}

function committedPrivateIssue(record) {
  return record?.direction === 'issue'
    && record?.output_mode === 'private'
    && record?.status === 'COMMITTED'
    && Array.isArray(record?.response?.output_note_refs)
    && record.response.output_note_refs.length === 1;
}

function formatAtoms(value, decimals) {
  let atoms;
  try { atoms = BigInt(String(value)); } catch (_) { return '—'; }
  if (!Number.isInteger(decimals) || decimals < 0 || decimals > 18) return '—';
  if (decimals === 0) return atoms.toString();
  const scale = 10n ** BigInt(decimals);
  return `${atoms / scale}.${String(atoms % scale).padStart(decimals, '0')}`;
}

export default function PftlPrivatePrimary({
  walletAddress = '',
  backupJson = '',
  proxyAuthToken = '',
  symbol = 'NAVCoin',
  decimals = 6,
  routeId = '',
  onComplete = null,
}) {
  const [readiness, setReadiness] = useState(null);
  const [records, setRecords] = useState([]);
  const [direction, setDirection] = useState('issue');
  const [outputMode, setOutputMode] = useState('private');
  const [amount, setAmount] = useState(() => decimals === 0 ? '1' : `1.${'0'.repeat(decimals)}`);
  const [sourceIssueId, setSourceIssueId] = useState('');
  const [loading, setLoading] = useState(true);
  const [executing, setExecuting] = useState(false);
  const [error, setError] = useState('');
  const preparationLock = useRef(false);

  const persist = useCallback((nextRecords) => {
    const saved = savePftlPrivateRecoveries(window.localStorage, walletAddress, nextRecords);
    setRecords(saved);
    return saved;
  }, [walletAddress]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const ready = await loadPftlPrivateReadiness({ proxyAuthToken, expectedRouteId: routeId });
      setReadiness(ready);
    } catch (failure) {
      setReadiness(null);
      setError(failure.message || 'The resident Private NAVCoin service is unavailable');
    } finally {
      setLoading(false);
    }
  }, [proxyAuthToken, routeId]);

  useEffect(() => {
    if (!walletAddress || typeof window === 'undefined') return;
    const recovered = loadPftlPrivateRecoveries(window.localStorage, walletAddress);
    setRecords(recovered);
    const source = [...recovered].reverse().find(committedPrivateIssue);
    if (source) setSourceIssueId(source.idempotency_key);
  }, [walletAddress]);

  useEffect(() => { refresh(); }, [refresh]);

  const privateIssues = useMemo(
    () => {
      const consumed = new Set(records
        .filter(record => record.direction === 'redeem' && record.status === 'COMMITTED')
        .map(record => record.source_issue_idempotency_key)
        .filter(Boolean));
      return records.filter(record => committedPrivateIssue(record) && !consumed.has(record.idempotency_key));
    },
    [records],
  );
  const current = records[records.length - 1] || null;
  const controlledWalletId = readiness?.controlled_wallet_id || '';
  const controlled = Boolean(walletAddress
    && controlledWalletId
    && walletAddress.toLowerCase() === controlledWalletId.toLowerCase());

  const runRecord = useCallback(async (record, signal) => {
    setExecuting(true);
    setError('');
    try {
      const completed = await completePftlPrivateIntent(record.signed_intent, {
        proxyAuthToken,
        signal,
        onStatus: (response) => {
          const next = {
            ...record,
            response,
            status: response?.swap?.state || record.status,
          };
          setRecords(previous => {
            try {
              const durable = loadPftlPrivateRecoveries(window.localStorage, walletAddress);
              return savePftlPrivateRecoveries(window.localStorage, walletAddress, replaceRecord(durable, next));
            } catch (_) { return replaceRecord(previous, next); }
          });
        },
      });
      const next = { ...record, response: completed, status: 'COMMITTED' };
      const saved = persist(replaceRecord(
        loadPftlPrivateRecoveries(window.localStorage, walletAddress), next,
      ));
      if (committedPrivateIssue(next)) setSourceIssueId(next.idempotency_key);
      if (next.direction === 'redeem') setSourceIssueId('');
      onComplete?.(next, saved);
      await refresh();
    } finally {
      setExecuting(false);
    }
  }, [onComplete, persist, proxyAuthToken, refresh, walletAddress]);

  const activeRecovery = useMemo(
    () => [...records].reverse().find(record => ACTIVE_STATES.has(record.status)) || null,
    [records],
  );
  const activeRecoveryId = activeRecovery?.idempotency_key || '';
  const residentReady = readiness?.upstream?.ready === true;

  useEffect(() => {
    if (direction !== 'redeem' || !sourceIssueId) return;
    const source = privateIssues.find(record => record.idempotency_key === sourceIssueId);
    const formatted = formatAtoms(source?.quote?.output_amount_atoms, decimals);
    if (formatted !== '—') setAmount(formatted);
  }, [decimals, direction, privateIssues, sourceIssueId]);

  useEffect(() => {
    if (!residentReady || !activeRecoveryId) return undefined;
    const active = loadPftlPrivateRecoveries(window.localStorage, walletAddress)
      .find(record => record.idempotency_key === activeRecoveryId);
    if (!active) return undefined;
    const controller = new AbortController();
    runRecord(active, controller.signal).catch((failure) => {
      if (failure.name !== 'AbortError') setError(failure.message || 'Private NAVCoin recovery remains pending');
    });
    return () => controller.abort();
  }, [activeRecoveryId, residentReady, runRecord, walletAddress]);

  const execute = useCallback(async () => {
    if (!readiness || executing || preparationLock.current) return;
    preparationLock.current = true;
    setExecuting(true);
    setError('');
    try {
      const maximumAtoms = Number(readiness?.upstream?.checks?.admission?.max_nav_amount_atoms);
      const navAmountAtoms = parsePrivateNavcoinAmountAtoms(amount, { decimals, maximumAtoms });
      const source = direction === 'redeem'
        ? privateIssues.find(record => record.idempotency_key === sourceIssueId)
        : null;
      if (direction === 'redeem' && !source) throw new Error('Select a committed private NAVCoin issue to redeem');
      let inputReference = 'transparent-pfusdc';
      let nextRecords = records;
      if (source) {
        const authoritative = await submitPftlPrivateIntent(source.signed_intent, { proxyAuthToken });
        if (authoritative?.swap?.state !== 'COMMITTED'
          || !Array.isArray(authoritative.output_note_refs)
          || authoritative.output_note_refs.length !== 1
          || !/^[0-9a-f]{64}$/.test(String(authoritative.output_note_refs[0]))) {
          throw new Error('Selected private NAVCoin issue is not committed and spendable');
        }
        inputReference = authoritative.output_note_refs[0];
        nextRecords = replaceRecord(records, {
          ...source,
          response: authoritative,
          status: 'COMMITTED',
        });
      }
      const quote = await createPftlPrivateQuote({
        direction,
        navAmountAtoms,
        outputMode,
        expectedRouteId: routeId,
        proxyAuthToken,
      });
      if (direction === 'redeem'
        && Number(source.quote?.output_amount_atoms) !== Number(quote.input_amount_atoms)) {
        throw new Error('Selected private NAVCoin note amount does not match the redeem quote');
      }
      const idempotencyKey = createPftlPrivateIdempotencyKey(direction);
      const intent = buildPftlPrivateIntent({
        quote,
        walletAddress,
        controlledWalletId,
        inputReference,
        idempotencyKey,
      });
      const signedIntent = signPftlPrivateIntent({ wasm: getWasm(), backupJson, intent });
      const record = {
        idempotency_key: idempotencyKey,
        direction,
        output_mode: outputMode,
        quote,
        signed_intent: signedIntent,
        response: null,
        source_issue_idempotency_key: source?.idempotency_key || null,
        status: 'SIGNED',
        created_at_unix_ms: Date.now(),
      };
      persist([...nextRecords, record]);
    } catch (failure) {
      setExecuting(false);
      throw failure;
    } finally {
      preparationLock.current = false;
    }
  }, [
    amount, backupJson, controlledWalletId, decimals, direction, executing, outputMode, persist,
    privateIssues, proxyAuthToken, readiness, records, routeId, sourceIssueId, walletAddress,
  ]);

  const onExecute = useCallback(() => {
    if (activeRecovery) {
      const controller = new AbortController();
      runRecord(activeRecovery, controller.signal)
        .catch(failure => setError(failure.message || 'Private NAVCoin recovery remains pending'));
      return;
    }
    execute().catch(failure => setError(failure.message || 'Private NAVCoin execution failed'));
  }, [activeRecovery, execute, runRecord]);

  const canExecute = Boolean(
    readiness?.upstream?.ready
    && controlled
    && backupJson
    && proxyAuthToken
    && !executing
    && (activeRecovery || direction === 'issue' || sourceIssueId),
  );
  const maximumDisplay = formatAtoms(
    readiness?.upstream?.checks?.admission?.max_nav_amount_atoms,
    decimals,
  );

  return (
    <section className="pfs-card" style={{ marginTop: 16 }} id="private-navcoin-primary">
      <div className="pfs-route-head">
        <span><ShieldCheck size={15} /> Controlled Private {symbol} primary market</span>
        <div className="pfs-pill-row">
          <span className={`pf-pill${readiness?.upstream?.ready ? ' good' : ' warn'}`}>
            <Lock size={11} /> {readiness?.upstream?.ready ? 'resident prover ready' : 'not ready'}
          </span>
        </div>
      </div>
      <p>
        Sign a bounded private-primary intent in this browser. The resident service proves and submits it;
        the wallet backup and signing key never cross the browser boundary.
      </p>
      {error && <div className="pf-error" style={{ marginTop: 12 }}>{error}</div>}

      <div className={`pfs-readiness${readiness?.upstream?.ready && controlled ? ' ready' : ''}`} style={{ marginTop: 14 }}>
        <div>
          <span>{loading ? 'VERIFYING' : readiness?.upstream?.ready && controlled ? 'CONTROLLED WALLET VERIFIED' : 'ACTION BLOCKED'}</span>
          <strong>{loading ? 'Checking resident prover and six-validator round driver…' : stageLabel(current)}</strong>
        </div>
        <button className="pfb-secondary small" type="button" onClick={refresh} disabled={loading || executing}>
          {loading ? <Loader2 size={14} className="pfb-spin" /> : <RefreshCw size={14} />} Refresh
        </button>
      </div>

      <div className="pftl-private-form" style={{ marginTop: 14 }}>
        <label className="pfb-field">
          <span>Action</span>
          <select className="pf-select" value={direction} onChange={(event) => {
            const next = event.target.value;
            setDirection(next);
            setOutputMode(next === 'redeem' ? 'transparent' : 'private');
          }} disabled={executing || Boolean(activeRecovery)}>
            <option value="issue">Issue private {symbol}</option>
            <option value="redeem">Redeem private {symbol}</option>
          </select>
        </label>
        <label className="pfb-field">
          <span>Output custody</span>
          <select className="pf-select" value={outputMode} onChange={event => setOutputMode(event.target.value)} disabled={executing || Boolean(activeRecovery)}>
            <option value="private">Private note</option>
            <option value="transparent">Controlled transparent wallet</option>
          </select>
        </label>
        <label className="pfb-field">
          <span>{symbol} amount (controlled max {maximumDisplay})</span>
          <input className="pf-input" value={amount} onChange={event => setAmount(event.target.value)} inputMode="decimal" disabled={executing || Boolean(activeRecovery)} />
        </label>
        {direction === 'redeem' && (
          <label className="pfb-field">
            <span>Committed private {symbol} note</span>
            <select className="pf-select" value={sourceIssueId} onChange={event => setSourceIssueId(event.target.value)} disabled={executing || Boolean(activeRecovery)}>
              <option value="">Select an issued note</option>
              {privateIssues.map(record => (
                <option key={record.idempotency_key} value={record.idempotency_key}>
                  {truncateMiddle(record.idempotency_key, 12)} · {record.quote?.output_amount_atoms} atoms
                </option>
              ))}
            </select>
          </label>
        )}
      </div>

      {current && (
        <div className={`pf-notice${current.status === 'COMMITTED' ? ' good' : ''}`} style={{ marginTop: 14 }}>
          <strong>{stageLabel(current)}</strong><br />
          <span>Durable intent {truncateMiddle(current.idempotency_key, 14)}</span>
          {current.response?.swap?.committed_height && <><br /><span>Committed at PFTL height {current.response.swap.committed_height}</span></>}
        </div>
      )}

      <button className="pf-primary" type="button" onClick={onExecute} disabled={!canExecute} style={{ marginTop: 16 }}>
        {executing
          ? <><Loader2 size={15} className="pfb-spin" /> {stageLabel(current)}</>
          : activeRecovery
            ? <><RefreshCw size={15} /> Resume durable intent</>
          : current?.status === 'COMMITTED'
            ? <><Check size={15} /> Sign another {direction}</>
            : <>Sign and {direction} privately <ArrowRight size={15} /></>}
      </button>
      {!controlled && readiness?.controlled_wallet_id && (
        <p className="pf-hint">This controlled qualification route is bound to {truncateMiddle(readiness.controlled_wallet_id, 12)}.</p>
      )}
      {!proxyAuthToken && <p className="pf-hint">An authenticated localhost wallet session is required.</p>}
      <p className="pf-hint">
        Limited-availability controlled execution. Public quote, signature, idempotency lineage, commitments,
        and finality references are recoverable locally; note openings and private keys are never stored here.
      </p>
    </section>
  );
}
