import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Check, Clock, Loader2, RefreshCw, Wallet } from 'lucide-react';
import * as evm from '../lib/evm.js';
import { loadGovernedVaultBridgeRoute } from '../lib/bridge-route.js';
import {
  createPfusdcWithdrawalJob,
  loadPfusdcWithdrawalJobs,
  loadPfusdcWithdrawalReadiness,
  pfusdcWithdrawalCapacity,
  preparePfusdcWithdrawal,
  recoverablePfusdcWithdrawal,
  retryPfusdcWithdrawalJob,
  waitForPfusdcWithdrawalJob,
} from '../lib/pfusdc-withdrawal.js';
import { CHAIN_ID, formatBalance, GENESIS_HASH, PFUSDC_ASSET_ID, truncateMiddle } from '../lib/utils.js';
import { acquireAutoLockLease } from '../lib/vault.js';

const USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';

function atoms(value) {
  const text = String(value || '').trim();
  if (!/^\d+(\.\d{0,6})?$/.test(text)) return null;
  const [whole, fraction = ''] = text.split('.');
  return BigInt(whole) * 1_000_000n + BigInt(fraction.padEnd(6, '0'));
}

function units(value) {
  const n = BigInt(value || 0); const whole = n / 1_000_000n;
  const fraction = (n % 1_000_000n).toString().padStart(6, '0').replace(/0+$/, '');
  return `${whole.toLocaleString()}${fraction ? `.${fraction}` : ''}`;
}

function pftBalance(response) {
  const result = response?.result?.account || response?.result;
  return BigInt(result?.balance || 0);
}

function assetBalance(response) {
  const rows = Array.isArray(response?.result) ? response.result : (response?.result?.assets || []);
  const row = rows.find(item => String(item.asset_id || item.id).toLowerCase() === PFUSDC_ASSET_ID);
  return BigInt(row?.balance ?? row?.amount ?? 0);
}

export default function BridgeWithdraw({ address, rpc, txBuilder, backupJson, proxyAuthToken }) {
  const [ethereumAddress, setEthereumAddress] = useState('');
  const [amount, setAmount] = useState('0.1');
  const [pfusdc, setPfusdc] = useState(0n);
  const [pft, setPft] = useState(0n);
  const [capacity, setCapacity] = useState(0n);
  const [serviceReady, setServiceReady] = useState(false);
  const [availability, setAvailability] = useState('Checking the active Ethereum reserve');
  const [review, setReview] = useState(null);
  const [phase, setPhase] = useState('loading');
  const [progress, setProgress] = useState('Checking balances and reserve availability');
  const [error, setError] = useState('');
  const [failedJob, setFailedJob] = useState(null);
  const [jobsChecked, setJobsChecked] = useState(false);
  const amountAtoms = useMemo(() => atoms(amount), [amount]);

  const connect = useCallback(async () => {
    setError('');
    try {
      const owner = await evm.connectMetaMask();
      await evm.ensureEthereumMainnet();
      setEthereumAddress(owner.toLowerCase());
      return owner.toLowerCase();
    } catch (failure) { setError(failure.message); return ''; }
  }, []);

  const load = useCallback(async () => {
    if (!rpc || !address) return;
    setPhase(current => current === 'running' ? current : 'loading');
    try {
      const [assetResponse, accountResponse] = await Promise.all([rpc.accountAssets(address), rpc.account(address)]);
      setPfusdc(assetBalance(assetResponse)); setPft(pftBalance(accountResponse));
      setPhase(current => current === 'running' ? current : 'ready');
    } catch (_) { setError('PFTL balances are unavailable. Retry when the network reconnects.'); setPhase('error'); }
  }, [address, rpc]);

  const loadCapacity = useCallback(async () => {
    if (!rpc || !proxyAuthToken) return;
    try {
      const [route, statusResponse] = await Promise.all([
        loadGovernedVaultBridgeRoute(rpc, { assetId: PFUSDC_ASSET_ID, chainId: CHAIN_ID, genesisHash: GENESIS_HASH, sourceChainId: 1, tokenAddress: USDC }),
        rpc.vaultBridgeStatus(PFUSDC_ASSET_ID),
      ]);
      if (statusResponse?.ok !== true) throw new Error('The active Ethereum reserve is unavailable.');
      const next = pfusdcWithdrawalCapacity({ status: statusResponse.result, route });
      setCapacity(next.amountAtoms);
      try {
        const readiness = await loadPfusdcWithdrawalReadiness(proxyAuthToken);
        setServiceReady(readiness.ready === true);
        setAvailability(readiness.ready === true
          ? `${units(next.amountAtoms)} USDC can be withdrawn now`
          : `${units(next.amountAtoms)} USDC is backed; withdrawals are temporarily paused while the payout service is checked`);
      } catch (_) {
        setServiceReady(false);
        setAvailability(`${units(next.amountAtoms)} USDC is backed; withdrawals are temporarily paused while the payout service is checked`);
      }
    } catch (failure) {
      setCapacity(0n); setServiceReady(false);
      setAvailability(failure.message || 'The active Ethereum reserve is unavailable.');
    }
  }, [proxyAuthToken, rpc]);

  useEffect(() => { load(); }, [load]);
  useEffect(() => { loadCapacity(); }, [loadCapacity]);
  useEffect(() => {
    if (!address || !proxyAuthToken) return undefined;
    const controller = new AbortController();
    let releaseLease = null;
    setJobsChecked(false);
    loadPfusdcWithdrawalJobs(address, proxyAuthToken, 10).then(async result => {
      const jobs = result.jobs || [];
      let latest = jobs.find(job => job.status !== 'accepted' && job.status !== 'failed')
        || jobs.find(job => job.status === 'failed') || jobs[0];
      const [route, statusResponse, readiness] = await Promise.all([
        loadGovernedVaultBridgeRoute(rpc, { assetId: PFUSDC_ASSET_ID, chainId: CHAIN_ID, genesisHash: GENESIS_HASH, sourceChainId: 1, tokenAddress: USDC }),
        rpc.vaultBridgeStatus(PFUSDC_ASSET_ID),
        loadPfusdcWithdrawalReadiness(proxyAuthToken).catch(() => ({ ready: false })),
      ]);
      if (statusResponse?.ok === true && readiness.ready === true) {
        const recovery = recoverablePfusdcWithdrawal({ status: statusResponse.result, route, owner: address, jobs });
        if (recovery) latest = await createPfusdcWithdrawalJob(recovery, proxyAuthToken);
      }
      if (controller.signal.aborted) return;
      setJobsChecked(true);
      if (!latest) return;
      if (latest.status === 'accepted') {
        setEthereumAddress(latest.request.ethereum_recipient);
        setProgress(`${units(latest.request.amount_atoms)} USDC received by ${truncateMiddle(latest.request.ethereum_recipient, 8)}`);
        setPhase('complete');
        return;
      }
      if (latest.status === 'failed') {
        setFailedJob(latest);
        setError('The accepted withdrawal is saved, but its Ethereum payout needs to be resumed.');
        return;
      }
      setPhase('running'); setProgress(latest.stage || 'Resuming saved withdrawal progress');
      releaseLease = acquireAutoLockLease();
      try {
        const complete = await waitForPfusdcWithdrawalJob(latest.job_id, proxyAuthToken, {
          signal: controller.signal,
          onStatus: next => setProgress(next.stage || 'Withdrawal in progress'),
        });
        if (!controller.signal.aborted) {
          setEthereumAddress(complete.request.ethereum_recipient);
          setProgress(`${units(complete.request.amount_atoms)} USDC received by ${truncateMiddle(complete.request.ethereum_recipient, 8)}`);
          setPhase('complete');
        }
      } catch (failure) {
        if (failure.name !== 'AbortError' && !controller.signal.aborted) { setError(failure.message); setPhase('error'); }
      } finally { releaseLease?.(); releaseLease = null; }
    }).catch(() => {
      if (!controller.signal.aborted) {
        setError('Saved withdrawal progress is unavailable. Retry after the wallet connection recovers.');
        setPhase('error');
      }
    });
    return () => { controller.abort(); releaseLease?.(); releaseLease = null; };
  }, [address, proxyAuthToken, rpc]);

  const retryPayout = async () => {
    if (!failedJob) return;
    setError(''); setFailedJob(null); setPhase('running'); setProgress('Resuming saved withdrawal progress');
    const release = acquireAutoLockLease();
    try {
      const resumed = await retryPfusdcWithdrawalJob(failedJob.job_id, proxyAuthToken);
      const complete = await waitForPfusdcWithdrawalJob(resumed.job_id, proxyAuthToken, { onStatus: next => setProgress(next.stage || 'Withdrawal in progress') });
      setEthereumAddress(complete.request.ethereum_recipient);
      setProgress(`${units(complete.request.amount_atoms)} USDC received by ${truncateMiddle(complete.request.ethereum_recipient, 8)}`);
      setPhase('complete'); await load();
    } catch (failure) { setError(failure.message); setFailedJob(failedJob); setPhase('error'); }
    finally { release(); }
  };
  useEffect(() => {
    if (!evm.hasMetaMask()) return;
    window.ethereum.request({ method: 'eth_accounts' }).then(accounts => {
      if (accounts?.[0]) setEthereumAddress(accounts[0].toLowerCase());
    }).catch(() => {});
  }, []);

  const prepare = async () => {
    setError(''); setReview(null);
    try {
      const recipient = ethereumAddress || await connect();
      if (!recipient) return;
      if (!jobsChecked) throw new Error('Saved withdrawal progress is still being checked.');
      if (!amountAtoms || amountAtoms <= 0n || amountAtoms > pfusdc) throw new Error(`Enter an amount up to ${units(pfusdc)} pfUSDC.`);
      if (!proxyAuthToken) throw new Error('Wallet transaction services are unavailable.');
      const [route, statusResponse, readiness] = await Promise.all([
        loadGovernedVaultBridgeRoute(rpc, { assetId: PFUSDC_ASSET_ID, chainId: CHAIN_ID, genesisHash: GENESIS_HASH, sourceChainId: 1, tokenAddress: USDC }),
        rpc.vaultBridgeStatus(PFUSDC_ASSET_ID),
        loadPfusdcWithdrawalReadiness(proxyAuthToken),
      ]);
      if (readiness.ready !== true || statusResponse?.ok !== true) throw new Error('Withdrawal service is not ready.');
      const operation = preparePfusdcWithdrawal({ status: statusResponse.result, route, owner: address, ethereumRecipient: recipient, amountAtoms });
      const nextCapacity = pfusdcWithdrawalCapacity({ status: statusResponse.result, route });
      setCapacity(nextCapacity.amountAtoms); setServiceReady(readiness.ready === true);
      const quote = await rpc.assetFeeQuote(address, JSON.stringify(operation));
      if (quote?.ok !== true) throw new Error(quote?.error?.message || 'PFTL fee quote is unavailable.');
      setReview({ operation, fee: BigInt(quote.result.minimum_fee || 0), quote: quote.result });
      setPhase('review');
    } catch (failure) { setError(failure.message); setPhase('error'); }
  };

  const execute = async () => {
    if (!review) return;
    setError(''); setPhase('running'); setProgress('Confirming the pfUSDC burn on PFTL');
    const release = acquireAutoLockLease();
    try {
      if (review.fee > pft || review.quote.sender_meets_reserve_after_fee === false) throw new Error(`You need ${formatBalance(review.fee)} PFT for the network fee; this wallet has ${formatBalance(pft)} PFT.`);
      const result = await txBuilder.sendAssetTransfer(backupJson, address, { operation: review.operation });
      if (result.receipt?.accepted !== true || !result.txId) throw new Error(result.receipt?.message || 'The withdrawal burn was not accepted.');
      setProgress('Preparing the Ethereum payout. You can leave and return; progress is saved.');
      const job = await createPfusdcWithdrawalJob({ burn_tx_id: result.txId, owner: address, ethereum_recipient: ethereumAddress, amount_atoms: String(review.operation.amount_atoms), asset_id: PFUSDC_ASSET_ID }, proxyAuthToken);
      await waitForPfusdcWithdrawalJob(job.job_id, proxyAuthToken, { onStatus: next => setProgress(next.stage || 'Withdrawal in progress') });
      setProgress(`${units(review.operation.amount_atoms)} USDC received by ${truncateMiddle(ethereumAddress, 8)}`);
      setPhase('complete'); await load();
    } catch (failure) { setError(failure.message); setPhase('error'); }
    finally { release(); }
  };

  const maximum = pfusdc < capacity ? pfusdc : capacity;
  return <div className="pf-page pfb-page">
    <header className="pfb-hero"><div><div className="pf-eyebrow">PFTL → Ethereum</div><h1>Withdraw USDC</h1><p>Burn pfUSDC in this PostFiat wallet and receive the same amount of USDC in your connected Ethereum wallet.</p></div><div className={`pfb-status ${phase === 'complete' ? 'complete' : phase === 'error' || !serviceReady ? 'error' : 'connected'}`}><span>{phase === 'complete' ? 'Complete' : phase === 'running' ? 'In progress' : phase === 'review' ? 'Review' : serviceReady ? 'Ready' : 'Checking'}</span><small>{phase === 'running' || phase === 'complete' ? progress : 'One wallet confirmation. Ethereum gas is included.'}</small></div></header>
    {error && <div className="pf-warning">{error}{failedJob && <button className="pfb-secondary" style={{ marginLeft: 12 }} onClick={retryPayout}>Retry payout</button>}</div>}
    <div className="pfb-layout"><main className="pfb-main-flow"><section className="pfb-action-card">
      {phase === 'complete' ? <><div className="pfb-card-head"><Check size={15}/> Complete</div><h2>USDC received on Ethereum</h2><p>{progress}</p><button className="pfb-secondary" onClick={() => { setReview(null); setPhase('ready'); }}>Make another withdrawal</button></> : phase === 'running' ? <><div className="pfb-card-head"><Loader2 size={15} className="pfb-spin"/> Withdrawal in progress</div><h2>Keep this page open or return later</h2><div className="pfb-progress-card"><strong><Clock size={14}/> {progress}</strong><span>The accepted PFTL burn and relay job are durable. Retrying the same job cannot withdraw twice.</span></div></> : <>
        <div className="pfb-card-head"><Wallet size={15}/> {review ? 'Review withdrawal' : 'Choose amount'}</div><h2>{review ? `${units(review.operation.amount_atoms)} pfUSDC → ${units(review.operation.amount_atoms)} USDC` : 'Withdraw to Ethereum mainnet'}</h2>
        {!ethereumAddress ? <button className="pfb-secondary" onClick={connect}>Connect MetaMask</button> : <p>Destination: {truncateMiddle(ethereumAddress, 9)} on Ethereum mainnet</p>}
        <label className="pfb-field"><span>Amount to withdraw</span><div className="pfb-amount"><input inputMode="decimal" value={amount} disabled={!!review} onChange={event => setAmount(event.target.value.replace(/[^0-9.]/g, ''))}/><span>pfUSDC</span><button className="pfb-secondary small" disabled={!!review || maximum <= 0n} onClick={() => setAmount(units(maximum))}>Max</button></div></label>
        <div className="pfb-progress-card"><strong>{availability}</strong><span>Your wallet has {units(pfusdc)} pfUSDC. The currently available amount is limited by USDC held in the active Ethereum reserve.</span></div>
        {review && <div className="pfb-readout"><span>You receive</span><strong>{units(review.operation.amount_atoms)} USDC</strong><span>PFTL network fee</span><strong>{formatBalance(review.fee)} PFT</strong><span>Ethereum gas</span><strong>Included</strong><span>Timing</span><strong>Local verification usually takes 20–40 minutes, then Ethereum confirms; progress is saved</strong></div>}
        <button className="pfb-primary" disabled={!ethereumAddress || phase === 'loading' || !jobsChecked || !serviceReady || maximum <= 0n} onClick={review ? execute : prepare}>{review ? 'Confirm and withdraw' : jobsChecked ? 'Review withdrawal' : 'Checking saved progress…'}</button>
        {review && <button className="pfb-secondary" onClick={() => { setReview(null); setPhase('ready'); }}>Back</button>}
      </>}
    </section></main><aside className="pfb-side"><div className="pfb-location"><div className="pfb-location-head">You are withdrawing</div><h2>PFTL pfUSDC → Ethereum USDC</h2><p>The amount is 1:1. The PFTL burn is irreversible after acceptance, so the service saves progress and resumes automatically until Ethereum payout completes.</p></div><div className="pfb-side-section"><div className="pfb-side-title">Balances <button onClick={() => { load(); loadCapacity(); }}><RefreshCw size={14}/></button></div><div className="pfb-balance-row active"><span>PFTL pfUSDC</span><strong>{phase === 'loading' ? '…' : `${units(pfusdc)} pfUSDC`}</strong></div><div className="pfb-balance-row"><span>Available to withdraw</span><strong>{units(capacity)} USDC</strong></div><div className="pfb-balance-row"><span>PFTL network fees</span><strong>{formatBalance(pft)} PFT</strong></div></div><div className="pfb-side-section"><div className="pfb-side-title">Accounts</div><div className="pfb-context-row"><span>From</span><strong>{truncateMiddle(address, 8)}</strong></div><div className="pfb-context-row"><span>To</span><strong>{ethereumAddress ? truncateMiddle(ethereumAddress, 8) : 'Connect MetaMask'}</strong></div></div></aside></div>
  </div>;
}
