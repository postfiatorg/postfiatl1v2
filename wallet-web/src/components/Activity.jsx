import React, { useCallback, useEffect, useMemo, useState } from 'react';

import { loadBridgeJobs } from '../lib/bridge-relay.js';
import * as evm from '../lib/evm.js';
import { loadNavcoinExportJobs } from '../lib/navcoin-export-relay.js';
import { displayAssetSymbol } from '../lib/navcoin-markets.js';
import { loadNavcoinReturnJobs } from '../lib/navcoin-return-relay.js';
import { loadPfusdcWithdrawalJobs } from '../lib/pfusdc-withdrawal.js';
import { PFUSDC_ASSET_ID, truncateMiddle } from '../lib/utils.js';

function responseRows(response) {
  if (response?.ok !== true || !response.result) return [];
  if (Array.isArray(response.result.rows)) return response.result.rows;
  // Older account-history adapters used `transactions`. Keep the fallback so
  // a rolling validator upgrade does not make a funded wallet look inactive.
  return Array.isArray(response.result.transactions) ? response.result.transactions : [];
}

function formatAtoms(value, decimals = 6) {
  const atoms = BigInt(String(value || 0));
  const scale = 10n ** BigInt(decimals);
  const whole = atoms / scale;
  const fraction = (atoms % scale).toString().padStart(decimals, '0').replace(/0+$/, '');
  return `${whole.toLocaleString()}${fraction ? `.${fraction}` : ''}`;
}

function bridgeStatus(job) {
  if (job.status === 'accepted' && job.receipt_code === 'ACCEPTED') return ['Completed', 'var(--mint)'];
  if (job.status === 'failed') return ['Failed', 'var(--red)'];
  return ['In progress', 'var(--amber)'];
}

function redemptionState(redemptions, txId) {
  return redemptions.find(row => row?.burn_tx_id === txId)?.state || '';
}

export function describeAccountActivity(row, address, markets = [], redemptions = [], withdrawalJobs = []) {
  const kind = String(row?.transaction_kind || 'transfer');
  const source = String(row?.from || row?.sender || '');
  const destination = String(row?.to || row?.recipient || '');
  const sent = source.toLowerCase() === String(address || '').toLowerCase();
  const routeKind = kind.startsWith('pftl_uniswap_');
  const market = markets.find(candidate => (
    candidate.routeId === destination
    || candidate.routeId === source
    || candidate.navAssetId === row?.asset_id
    || candidate.settlementAssetId === row?.asset_id
  )) || (routeKind && markets.length === 1 ? markets[0] : null);
  const inferredAssetId = row?.asset_id
    || (kind === 'pftl_uniswap_order_reserve'
      || kind === 'pftl_uniswap_primary_redeem'
      || kind === 'pftl_uniswap_export_debit'
      ? market?.navAssetId : '');
  const symbol = displayAssetSymbol(markets, inferredAssetId, inferredAssetId === PFUSDC_ASSET_ID ? 'pfUSDC' : 'Issued asset');
  const decimals = market && inferredAssetId === market.navAssetId
    ? market.decimals : market && inferredAssetId === market.settlementAssetId
      ? market.settlementDecimals : 6;
  const amount = formatAtoms(row?.amount, decimals);
  const accepted = row?.accepted === true;
  const failed = row?.accepted === false;
  let title = sent ? 'Sent PFT' : 'Received PFT';
  let value = `${sent ? '−' : '+'}${formatAtoms(row?.amount, 6)} PFT`;
  let detail = sent
    ? `To ${truncateMiddle(destination, 8)}`
    : `From ${truncateMiddle(source, 8)}`;

  switch (kind) {
    case 'issued_payment':
      title = sent ? `Sent ${symbol}` : `Received ${symbol}`;
      value = `${sent ? '−' : '+'}${amount} ${symbol}`;
      break;
    case 'vault_bridge_deposit_claim':
      title = 'Received pfUSDC from Ethereum';
      value = `+${amount} pfUSDC`;
      detail = 'Ethereum USDC deposit finalized on PFTL';
      break;
    case 'pftl_uniswap_order_reserve':
      title = `Reserved ${market?.symbol || 'NAV asset'} purchase`;
      value = `${amount} ${market?.symbol || symbol}`;
      detail = 'Purchase reservation created before funds moved';
      break;
    case 'pftl_uniswap_order_release':
      title = 'Released unused purchase reservation';
      value = '';
      detail = 'No NAV asset was issued by this step';
      break;
    case 'pftl_uniswap_primary_subscribe_v2':
      title = `Bought ${market?.symbol || 'NAV asset'} at verified NAV`;
      value = `−${amount} ${market?.settlementSymbol || symbol}`;
      detail = 'Primary-market settlement accepted on PFTL';
      break;
    case 'pftl_uniswap_export_debit':
      title = `Sent ${market?.symbol || 'NAV asset'} to Ethereum`;
      value = `−${amount} ${market?.symbol || symbol}`;
      detail = `${market?.wrappedSymbol || 'Wrapped asset'} delivery requested for ${truncateMiddle(destination, 7)}`;
      break;
    case 'pftl_uniswap_return_import':
      title = `Returned ${market?.wrappedSymbol || 'wrapped NAV asset'} to PFTL`;
      value = `+${amount} ${market?.symbol || symbol}`;
      detail = 'Ethereum burn was finalized and the native asset was restored';
      break;
    case 'pftl_uniswap_primary_redeem':
      title = `Redeemed ${market?.symbol || 'NAV asset'} at verified NAV`;
      value = `−${amount} ${market?.symbol || symbol}`;
      detail = `Settlement returned as ${market?.settlementSymbol || 'the settlement asset'}`;
      break;
    case 'vault_bridge_burn_to_redeem': {
      const state = redemptionState(redemptions, row?.tx_id);
      title = state === 'settled' ? 'Withdrew pfUSDC to Ethereum' : 'Started pfUSDC withdrawal';
      value = `−${amount} pfUSDC`;
      detail = state === 'settled'
        ? 'Ethereum USDC was claimed and PFTL accounting was settled'
        : 'PFTL burn accepted; the Ethereum payout is still processing';
      break;
    }
    default:
      if (row?.asset_id) {
        title = `${sent ? 'Sent' : 'Received'} ${symbol}`;
        value = `${sent ? '−' : '+'}${amount} ${symbol}`;
      }
  }

  let status = failed ? 'Failed' : accepted ? 'Accepted' : 'Pending';
  let color = failed ? 'var(--red)' : accepted ? 'var(--mint)' : 'var(--amber)';
  if (kind === 'vault_bridge_burn_to_redeem') {
    const job = withdrawalJobs.find(candidate => candidate?.request?.burn_tx_id === row?.tx_id);
    if (job?.status === 'failed') {
      title = 'pfUSDC withdrawal needs attention'; status = 'Failed'; color = 'var(--red)';
      detail = job.message || 'The saved withdrawal could not complete automatically';
    } else if (job?.status === 'accepted') {
      title = 'Withdrew pfUSDC to Ethereum'; status = 'Completed'; color = 'var(--mint)';
      detail = `Ethereum USDC received${job.ethereum_tx_hash ? ` · payout ${truncateMiddle(job.ethereum_tx_hash, 7)}` : ''}`;
    } else if (job) {
      title = 'Withdrawing pfUSDC to Ethereum'; status = 'In progress'; color = 'var(--amber)';
      detail = job.stage || 'The saved withdrawal is continuing automatically';
    }
  }
  return {
    key: `pftl:${row?.tx_id || `${row?.block_height}:${row?.transaction_index}`}`,
    title,
    detail,
    value,
    status,
    color,
    blockHeight: Number(row?.block_height || 0),
    txId: String(row?.tx_id || ''),
  };
}

function bridgeActivity(job, index) {
  const [status, color] = bridgeStatus(job);
  const request = job.request || {};
  const createdAt = Number(job.created_at_unix || 0);
  const when = createdAt > 0 ? new Date(createdAt * 1000).toLocaleString() : 'Time unavailable';
  return {
    key: `bridge:${job.job_id || index}`,
    title: 'Deposited Ethereum USDC to PFTL',
    detail: `${when} · Ethereum transaction ${truncateMiddle(request.deposit_tx_hash || '', 7)}`,
    value: `${formatAtoms(request.amount_atoms, 6)} USDC`,
    status,
    color,
    blockHeight: 0,
    txId: String(request.deposit_tx_hash || ''),
  };
}

function relayActivity(job, market, direction, index) {
  const failed = job.status === 'failed';
  const accepted = job.status === 'accepted';
  const symbol = direction === 'export' ? market.symbol : market.wrappedSymbol;
  const value = `${formatAtoms(job.request?.amount_atoms, market.decimals)} ${symbol}`;
  const when = Number(job.created_at_unix || 0) > 0
    ? new Date(Number(job.created_at_unix) * 1000).toLocaleString() : 'Saved wallet job';
  return {
    key: `${direction}:${job.job_id || index}`,
    title: direction === 'export'
      ? `${failed ? 'Failed to send' : accepted ? 'Sent' : 'Sending'} ${market.symbol} to Ethereum`
      : `${failed ? 'Failed to return' : accepted ? 'Returned' : 'Returning'} ${market.wrappedSymbol} to PFTL`,
    detail: `${when} · ${job.message || String(job.status || 'queued').replaceAll('_', ' ')}`,
    value, status: failed ? 'Failed' : accepted ? 'Completed' : 'In progress',
    color: failed ? 'var(--red)' : accepted ? 'var(--mint)' : 'var(--amber)',
    blockHeight: 0,
    txId: direction === 'return' ? String(job.request?.transaction_hash || '') : '',
  };
}

function withdrawalActivity(job, index) {
  const failed = job.status === 'failed';
  return {
    key: `withdrawal:${job.job_id || index}`,
    title: failed ? 'pfUSDC withdrawal needs attention' : 'Withdrawing pfUSDC to Ethereum',
    detail: job.message || job.stage || 'Saved withdrawal is continuing automatically',
    value: `−${formatAtoms(job.request?.amount_atoms, 6)} pfUSDC`,
    status: failed ? 'Failed' : 'In progress', color: failed ? 'var(--red)' : 'var(--amber)',
    blockHeight: 0, txId: String(job.request?.burn_tx_id || ''),
  };
}

export default function Activity({ rpc, address, markets = [], proxyAuthToken = '' }) {
  const [rows, setRows] = useState([]);
  const [bridgeJobs, setBridgeJobs] = useState([]);
  const [redemptions, setRedemptions] = useState([]);
  const [withdrawalJobs, setWithdrawalJobs] = useState([]);
  const [relayJobs, setRelayJobs] = useState([]);
  const [truncated, setTruncated] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const refresh = useCallback(async () => {
    if (!rpc || !address) return;
    setLoading(true);
    setError('');
    try {
      const ethereumAccounts = evm.hasMetaMask()
        ? await window.ethereum.request({ method: 'eth_accounts' }).catch(() => []) : [];
      const ethereumAddress = String(ethereumAccounts?.[0] || '').toLowerCase();
      const [accountResult, bridgeResult, bridgeStateResult, withdrawalResult, relayResult] = await Promise.allSettled([
        rpc.accountTx(address, { limit: 200 }),
        proxyAuthToken ? loadBridgeJobs(address, proxyAuthToken, 100) : Promise.resolve({ jobs: [] }),
        rpc.vaultBridgeStatus(PFUSDC_ASSET_ID),
        proxyAuthToken ? loadPfusdcWithdrawalJobs(address, proxyAuthToken, 100) : Promise.resolve({ jobs: [] }),
        proxyAuthToken ? Promise.all(markets.flatMap(market => [
          ethereumAddress ? loadNavcoinExportJobs(market.routeId, ethereumAddress, proxyAuthToken, 100) : Promise.resolve({ jobs: [] }),
          loadNavcoinReturnJobs(market.routeId, address, proxyAuthToken, 100),
        ])) : Promise.resolve([]),
      ]);
      const response = accountResult.status === 'fulfilled' ? accountResult.value : null;
      const jobs = bridgeResult.status === 'fulfilled' && Array.isArray(bridgeResult.value?.jobs)
        ? bridgeResult.value.jobs : [];
      const bridgeState = bridgeStateResult.status === 'fulfilled' && bridgeStateResult.value?.ok === true
        ? bridgeStateResult.value.result : null;
      if (response?.ok !== true && bridgeResult.status !== 'fulfilled') {
        throw new Error(response?.error?.message || 'Activity is unavailable');
      }
      setRows(responseRows(response));
      setTruncated(response?.result?.truncated === true);
      setBridgeJobs(jobs);
      setRedemptions(Array.isArray(bridgeState?.redemptions) ? bridgeState.redemptions : []);
      setWithdrawalJobs(withdrawalResult.status === 'fulfilled' && Array.isArray(withdrawalResult.value?.jobs)
        ? withdrawalResult.value.jobs : []);
      setRelayJobs(relayResult.status === 'fulfilled' ? relayResult.value.flatMap((payload, index) => {
        const market = markets[Math.floor(index / 2)];
        const direction = index % 2 === 0 ? 'export' : 'return';
        return (payload?.jobs || []).map((job, jobIndex) => ({ job, market, direction, index: jobIndex }));
      }) : []);
    } catch (_) {
      setError('Your activity could not be loaded. No transaction was submitted. Retry when the network reconnects.');
    } finally {
      setLoading(false);
    }
  }, [address, markets, proxyAuthToken, rpc]);

  useEffect(() => { refresh(); }, [refresh]);

  const activity = useMemo(() => {
    const chainDepositAmounts = new Map();
    for (const row of rows.filter(row => row.transaction_kind === 'vault_bridge_deposit_claim')) {
      const key = String(row.amount || 0);
      chainDepositAmounts.set(key, (chainDepositAmounts.get(key) || 0) + 1);
    }
    const unmatchedBridgeJobs = bridgeJobs.filter(job => {
      if (!(job.status === 'accepted' && job.receipt_code === 'ACCEPTED')) return true;
      const key = String(job.request?.amount_atoms || 0);
      const remaining = chainDepositAmounts.get(key) || 0;
      if (remaining <= 0) return true;
      chainDepositAmounts.set(key, remaining - 1);
      return false;
    });
    const burnIds = new Set(rows.filter(row => row.transaction_kind === 'vault_bridge_burn_to_redeem').map(row => row.tx_id));
    const incompleteWithdrawals = withdrawalJobs.filter(job => job.status !== 'accepted' && !burnIds.has(job.request?.burn_tx_id));
    const incompleteRelays = relayJobs.filter(({ job }) => job.status !== 'accepted');
    return [
      ...incompleteWithdrawals.map(withdrawalActivity),
      ...incompleteRelays.map(({ job, market, direction, index }) => relayActivity(job, market, direction, index)),
      ...rows.slice().reverse().map(row => describeAccountActivity(row, address, markets, redemptions, withdrawalJobs)),
      ...unmatchedBridgeJobs.map(bridgeActivity),
    ];
  }, [address, bridgeJobs, markets, redemptions, relayJobs, rows, withdrawalJobs]);

  return (
    <div className="pf-page">
      <div className="pf-band" style={{ alignItems: 'start' }}>
        <div>
          <div className="pf-eyebrow">This wallet</div>
          <h1 className="pf-h1">Activity</h1>
          <p style={{ marginTop: 8, color: 'var(--muted)', fontSize: 13.5 }}>
            PFTL transfers, issued assets, NAV trades, cross-chain movements, and Ethereum USDC deposits associated with {truncateMiddle(address || '', 10)}.
          </p>
        </div>
        <button className="pf-ghost" style={{ width: 'auto' }} onClick={refresh} disabled={loading}>
          {loading ? 'Refreshing…' : 'Refresh'}
        </button>
      </div>

      {error && <div className="pf-error">{error}</div>}
      {truncated && (
        <div className="pf-notice" style={{ marginBottom: 14 }}>
          Showing the newest activity retained by this network index.
        </div>
      )}
      <div className="pf-card" style={{ padding: 0 }}>
        {loading && activity.length === 0 ? (
          <div style={{ padding: 24, color: 'var(--muted)' }}>Loading wallet activity…</div>
        ) : activity.length === 0 ? (
          <div style={{ padding: 24, display: 'grid', gap: 8 }}>
            <strong>No wallet activity found</strong>
            <span style={{ color: 'var(--muted)', fontSize: 13 }}>
              Transactions associated with this address will appear here after finality.
            </span>
          </div>
        ) : activity.map(item => (
          <div className="pf-act" key={item.key}>
            <div className="pf-act-l">
              <div className="pf-act-t">{item.title}</div>
              <div className="pf-act-s">
                {item.detail}{item.blockHeight > 0 ? ` · Block ${item.blockHeight}` : ''}
                {item.txId ? ` · ${truncateMiddle(item.txId, 7)}` : ''}
              </div>
            </div>
            <div style={{ textAlign: 'right', display: 'grid', gap: 3 }}>
              {item.value && <strong>{item.value}</strong>}
              <span style={{ color: item.color, fontSize: 11 }}>{item.status}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
