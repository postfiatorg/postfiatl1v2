import React, { useCallback, useEffect, useState } from 'react';

import { loadBridgeJobs } from '../lib/bridge-relay.js';
import { formatBalance, truncateMiddle } from '../lib/utils.js';

function resultRows(response) {
  if (response?.ok !== true || !response.result) return [];
  return Array.isArray(response.result) ? response.result : (response.result.transactions || []);
}

function direction(transaction, address) {
  const source = transaction.from || transaction.sender || '';
  return source.toLowerCase() === String(address || '').toLowerCase() ? 'Sent' : 'Received';
}

function formatUsdcAtoms(value) {
  const atoms = BigInt(String(value || 0));
  const whole = atoms / 1_000_000n;
  const fraction = (atoms % 1_000_000n).toString().padStart(6, '0').replace(/0+$/, '');
  return `${whole.toLocaleString()}${fraction ? `.${fraction}` : ''}`;
}

function bridgeStatus(job) {
  if (job.status === 'accepted' && job.receipt_code === 'ACCEPTED') return ['Completed', 'var(--mint)'];
  if (job.status === 'failed') return ['Failed', 'var(--red)'];
  return ['In progress', 'var(--amber)'];
}

export default function Activity({ rpc, address, proxyAuthToken = '' }) {
  const [rows, setRows] = useState([]);
  const [bridgeJobs, setBridgeJobs] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const refresh = useCallback(async () => {
    if (!rpc || !address) return;
    setLoading(true);
    setError('');
    try {
      const [accountResult, bridgeResult] = await Promise.allSettled([
        rpc.accountTx(address, { limit: 50 }),
        proxyAuthToken ? loadBridgeJobs(address, proxyAuthToken, 50) : Promise.resolve({ jobs: [] }),
      ]);
      const response = accountResult.status === 'fulfilled' ? accountResult.value : null;
      const jobs = bridgeResult.status === 'fulfilled' && Array.isArray(bridgeResult.value?.jobs)
        ? bridgeResult.value.jobs : [];
      if (response?.ok !== true && bridgeResult.status !== 'fulfilled') {
        throw new Error(response?.error?.message || 'Activity is unavailable');
      }
      setRows(resultRows(response));
      setBridgeJobs(jobs);
    } catch (failure) {
      setError('Your activity could not be loaded. No transaction was submitted. Retry when the network reconnects.');
    } finally {
      setLoading(false);
    }
  }, [address, proxyAuthToken, rpc]);

  useEffect(() => { refresh(); }, [refresh]);

  return (
    <div className="pf-page">
      <div className="pf-band" style={{ alignItems: 'start' }}>
        <div>
          <div className="pf-eyebrow">This wallet</div>
          <h1 className="pf-h1">Activity</h1>
          <p style={{ marginTop: 8, color: 'var(--muted)', fontSize: 13.5 }}>
            Transactions associated with {truncateMiddle(address || '', 10)}.
          </p>
        </div>
        <button className="pf-ghost" style={{ width: 'auto' }} onClick={refresh} disabled={loading}>
          {loading ? 'Refreshing…' : 'Refresh'}
        </button>
      </div>

      {error && <div className="pf-error">{error}</div>}
      {bridgeJobs.length > 0 && (
        <div style={{ display: 'grid', gap: 10, marginBottom: 18 }}>
          <div className="pf-eyebrow">USDC deposits</div>
          <div className="pf-card" style={{ padding: 0 }}>
            {bridgeJobs.map((job, index) => {
              const [status, color] = bridgeStatus(job);
              const request = job.request || {};
              const when = Number(job.created_at_unix || 0) > 0
                ? new Date(Number(job.created_at_unix) * 1000).toLocaleString() : 'Time unavailable';
              return (
                <div className="pf-act" key={job.job_id || index}>
                  <div className="pf-act-l">
                    <div className="pf-act-t">Ethereum USDC → PFTL pfUSDC</div>
                    <div className="pf-act-s">
                      {when} · Ethereum transaction {truncateMiddle(request.deposit_tx_hash || '', 7)}
                    </div>
                  </div>
                  <div style={{ textAlign: 'right', display: 'grid', gap: 3 }}>
                    <strong>{formatUsdcAtoms(request.amount_atoms)} USDC</strong>
                    <span style={{ color, fontSize: 11 }}>{status}</span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
      {rows.length > 0 && <div className="pf-eyebrow" style={{ marginBottom: 10 }}>PFT transfers</div>}
      <div className="pf-card" style={{ padding: 0 }}>
        {loading && rows.length === 0 && bridgeJobs.length === 0 ? (
          <div style={{ padding: 24, color: 'var(--muted)' }}>Loading wallet activity…</div>
        ) : rows.length === 0 && bridgeJobs.length === 0 ? (
          <div style={{ padding: 24, display: 'grid', gap: 8 }}>
            <strong>No wallet activity found</strong>
            <span style={{ color: 'var(--muted)', fontSize: 13 }}>
              New bridge deposits and PFT transfers associated with this address will appear here. Issued-asset history is not available from this network endpoint yet.
            </span>
          </div>
        ) : rows.length === 0 ? (
          <div style={{ padding: 18, color: 'var(--muted)', fontSize: 13 }}>No native PFT transfers found.</div>
        ) : rows.map((transaction, index) => {
          const kind = direction(transaction, address);
          const counterparty = kind === 'Sent'
            ? (transaction.to || transaction.recipient)
            : (transaction.from || transaction.sender);
          return (
            <div className="pf-act" key={transaction.tx_id || transaction.id || index}>
              <div className="pf-act-l">
                <div className="pf-act-t">{kind} PFT</div>
                <div className="pf-act-s">
                  {counterparty ? `${kind === 'Sent' ? 'To' : 'From'} ${truncateMiddle(counterparty, 8)}` : 'Counterparty unavailable'}
                  {' · '}{transaction.block_height || transaction.height ? `Block ${transaction.block_height || transaction.height}` : 'Pending height'}
                </div>
              </div>
              <div className="pf-act-v" style={{ color: kind === 'Received' ? 'var(--mint)' : 'var(--text)' }}>
                {kind === 'Received' ? '+' : '−'}{formatBalance(transaction.amount || transaction.value || 0)} PFT
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
