import React, { useCallback, useEffect, useMemo, useState } from 'react';

import {
  buildNavcoinIssueExportDraft,
  buildNavcoinIssueOperations,
  buildNavcoinRedeemOperation,
  evaluateNavcoinResidentMarket,
  formatNavcoinNav,
  formatNavcoinUnits,
  finalizeNavcoinIssueExportOperations,
  parseNavcoinUnits,
} from '../lib/navcoin-primary-route.js';
import { ETH_MAINNET_USDC, truncateMiddle } from '../lib/utils.js';
import { createNavcoinExportJob, loadNavcoinExportReadiness, waitForNavcoinExportJob } from '../lib/navcoin-export-relay.js';
import {
  buildNavcoinReturnBurnCalldata,
  createNavcoinReturnJob,
  createNavcoinReturnNonce,
  loadNavcoinReturnJob,
  loadNavcoinReturnReadiness,
  waitForNavcoinReturnJob,
} from '../lib/navcoin-return-relay.js';

const EMPTY_PROGRESS = [];
const ERC20_BALANCE_OF_SELECTOR = '0x70a08231';

function returnStorageKey(routeId, walletAddress) {
  return `postfiat:navcoin-return:v1:${routeId}:${String(walletAddress || '').toLowerCase()}`;
}

function responseResult(response, label) {
  if (!response?.ok || !response.result) {
    throw new Error(response?.error?.message || `${label} is unavailable`);
  }
  return response.result;
}

function assetBalance(result, assetId) {
  const assets = Array.isArray(result) ? result : (result?.assets || []);
  const row = assets.find(item => String(item?.asset_id || item?.id || '').toLowerCase() === assetId);
  return String(row?.balance ?? row?.amount ?? 0);
}

function requireAccepted(result, label) {
  if (result?.receipt?.accepted !== true) {
    const detail = result?.receipt?.message || result?.receipt?.code || 'finality was not confirmed';
    throw new Error(`${label} was not confirmed: ${detail}`);
  }
  return result;
}

function transactionId(result) {
  return result?.txId || result?.receipt?.tx_id || 'finalized';
}

function shortTx(value) {
  return value === 'finalized' ? value : truncateMiddle(String(value), 7);
}

function verificationCopy(trustClass) {
  if (trustClass === 'TRUSTLESS_FINALITY') return 'automatic finality verification';
  if (trustClass === 'BFT_CHECKPOINT') return 'validator-confirmed finality check';
  if (trustClass === 'CONTROLLED') return 'operator-confirmed finality check';
  return 'route finality check';
}

async function ensureEthereumMainnet() {
  if (!window.ethereum?.request) throw new Error('MetaMask is not available in this browser');
  let chainId = String(await window.ethereum.request({ method: 'eth_chainId' }) || '').toLowerCase();
  if (chainId !== '0x1') {
    await window.ethereum.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x1' }] });
    chainId = String(await window.ethereum.request({ method: 'eth_chainId' }) || '').toLowerCase();
  }
  if (chainId !== '0x1') throw new Error('MetaMask must be connected to Ethereum mainnet');
}

async function readWrappedNavcoinBalance(recipient, market) {
  await ensureEthereumMainnet();
  const data = `${ERC20_BALANCE_OF_SELECTOR}${recipient.slice(2).padStart(64, '0')}`;
  const result = await window.ethereum.request({
    method: 'eth_call',
    params: [{ to: market.wrappedToken, data }, 'latest'],
  });
  if (!/^0x[0-9a-f]+$/i.test(String(result || ''))) throw new Error(`MetaMask returned a malformed ${market.wrappedSymbol} balance`);
  return BigInt(result);
}

async function watchWrappedNavcoin(market) {
  await ensureEthereumMainnet();
  return window.ethereum.request({
    method: 'wallet_watchAsset',
    params: {
      type: 'ERC20',
      options: { address: market.wrappedToken, symbol: market.wrappedSymbol, decimals: market.decimals },
    },
  });
}

export default function NavcoinPrimaryMarket({
  market,
  markets = [],
  onSelectMarket,
  rpc,
  txBuilder,
  backupJson,
  address,
  chainStatus,
  chainCapabilities,
  proxyAuthToken = '',
  liveSnapshot = null,
  onToast,
  onNavigate,
}) {
  const [mode, setMode] = useState('issue');
  const [amount, setAmount] = useState('1');
  const [ethereumRecipient, setEthereumRecipient] = useState('');
  const [snapshot, setSnapshot] = useState(null);
  const [loading, setLoading] = useState(true);
  const [refreshError, setRefreshError] = useState('');
  const [actionError, setActionError] = useState('');
  const [progress, setProgress] = useState(EMPTY_PROGRESS);
  const [executing, setExecuting] = useState(false);
  const [lastCompleted, setLastCompleted] = useState(null);
  const [delivery, setDelivery] = useState('ethereum');
  const [redeemSource, setRedeemSource] = useState('pftl');
  const [metamaskNavcoinBalance, setMetamaskNavcoinBalance] = useState(null);
  const [pendingReturn, setPendingReturn] = useState(null);
  const [exportPacketHash, setExportPacketHash] = useState('');
  const navSymbol = market.symbol;
  const wrappedSymbol = market.wrappedSymbol;
  const settlementSymbol = market.settlementSymbol;

  const refresh = useCallback(async () => {
    if (!rpc || !address) return null;
    setLoading(true);
    try {
      const [routeResponse, navResponse, assetsResponse, statusResponse] = await Promise.all([
        rpc.navcoinBridgeSupplyStatus(market.routeId),
        rpc.vaultBridgeStatus(market.navAssetId),
        rpc.accountAssets(address),
        rpc.status(),
      ]);
      const next = {
        route: responseResult(routeResponse, `${navSymbol} route`),
        nav: responseResult(navResponse, `${navSymbol} NAV`),
        assets: responseResult(assetsResponse, 'wallet assets'),
        chain: responseResult(statusResponse, 'chain status'),
      };
      next.settlementBalance = assetBalance(next.assets, market.settlementAssetId);
      next.navcoinBalance = assetBalance(next.assets, market.navAssetId);
      setSnapshot(next);
      setRefreshError('');
      return next;
    } catch (error) {
      setRefreshError(error.message || `Unable to load ${navSymbol} market`);
      return null;
    } finally {
      setLoading(false);
    }
  }, [address, market, navSymbol, rpc]);

  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, 12_000);
    return () => clearInterval(timer);
  }, [refresh]);

  useEffect(() => {
    const assets = liveSnapshot?.assets;
    if (!snapshot || !assets) return;
    setSnapshot(current => current ? {
      ...current,
      assets,
      settlementBalance: assetBalance(assets, market.settlementAssetId),
      navcoinBalance: assetBalance(assets, market.navAssetId),
    } : current);
  }, [liveSnapshot, market, snapshot?.route?.ledger_hash]);

  useEffect(() => {
    if (!address || !market?.routeId || typeof localStorage === 'undefined') return undefined;
    const key = returnStorageKey(market.routeId, address);
    let stopped = false;
    const probe = async () => {
      let stored = null;
      try { stored = JSON.parse(localStorage.getItem(key) || 'null'); } catch (_) { /* ignore corrupt browser state */ }
      if (!stored || stopped) return;
      setPendingReturn(stored);
      try {
        if (!stored.job_id && proxyAuthToken) {
          const created = await createNavcoinReturnJob({
            routeId: stored.route_id,
            routeConfigDigest: stored.route_config_digest,
            transactionHash: stored.transaction_hash,
            ethereumSender: stored.ethereum_sender,
            pftlRecipient: stored.pftl_recipient,
            nativeNavAssetId: stored.native_nav_asset_id,
            amountAtoms: stored.amount_atoms,
            returnNonce: stored.return_nonce,
            proxyAuthToken,
          });
          stored = { ...stored, job_id: created.job_id, status: created.status };
          localStorage.setItem(key, JSON.stringify(stored));
          setPendingReturn(stored);
        }
        if (!stored.job_id) return;
        const status = await loadNavcoinReturnJob(stored.route_id, stored.job_id);
        if (stopped) return;
        const next = { ...stored, status: status.status, message: status.message || stored.message };
        setPendingReturn(next);
        localStorage.setItem(key, JSON.stringify(next));
        if (status.status === 'accepted') {
          localStorage.removeItem(key);
          setPendingReturn(null);
          setRedeemSource('pftl');
          await refresh();
        }
      } catch (_) { /* the durable job remains recoverable by its stored id */ }
    };
    probe();
    const timer = setInterval(probe, 12_000);
    return () => { stopped = true; clearInterval(timer); };
  }, [address, market?.routeId, proxyAuthToken, refresh]);

  const amountAtoms = useMemo(
    () => parseNavcoinUnits(amount, market?.decimals),
    [amount, market?.decimals],
  );
  const redeemBalance = redeemSource === 'ethereum' ? metamaskNavcoinBalance : snapshot?.navcoinBalance;
  const evaluation = useMemo(() => evaluateNavcoinResidentMarket({
    market,
    supplyStatus: snapshot?.route,
    navStatus: snapshot?.nav,
    chainStatus: snapshot?.chain || chainStatus,
    direction: mode,
    amountAtoms,
    settlementBalanceAtoms: snapshot?.settlementBalance,
    navcoinBalanceAtoms: mode === 'redeem' ? redeemBalance : snapshot?.navcoinBalance,
  }), [amountAtoms, chainStatus, market, mode, redeemBalance, snapshot]);
  const quote = evaluation.quote;

  const finalityReady = chainCapabilities?.read_only === false
    && chainCapabilities?.mempool_submit_asset_transaction_finality_enabled === true;
  const validEthereumRecipient = /^0x[0-9a-f]{40}$/.test(ethereumRecipient);
  const canExecute = evaluation.ok
    && finalityReady
    && Boolean(backupJson)
    && Boolean(txBuilder)
    && !executing
    && (mode === 'redeem'
      ? (redeemSource === 'pftl' || (validEthereumRecipient && Boolean(proxyAuthToken)))
      : (validEthereumRecipient && (delivery !== 'ethereum' || Boolean(proxyAuthToken))));

  const updateStep = (index, patch) => {
    setProgress(current => current.map((step, stepIndex) => (
      stepIndex === index ? { ...step, ...patch } : step
    )));
  };

  const runStep = async (index, operation, label) => {
    updateStep(index, { state: 'running', detail: 'Requesting fee quote and local signature…' });
    const result = requireAccepted(
      await txBuilder.sendAssetTransfer(backupJson, address, { operation }),
      label,
    );
    updateStep(index, { state: 'done', detail: shortTx(transactionId(result)), txId: transactionId(result) });
    return result;
  };

  const connectEthereum = async () => {
    setActionError('');
    try {
      if (!window.ethereum?.request) throw new Error('MetaMask is not available in this browser');
      const accounts = await window.ethereum.request({ method: 'eth_requestAccounts' });
      const selected = String(accounts?.[0] || '').toLowerCase();
      if (!/^0x[0-9a-f]{40}$/.test(selected)) throw new Error('MetaMask did not return a valid Ethereum address');
      setEthereumRecipient(selected);
      try {
        await watchWrappedNavcoin(market);
      } catch (_) {
        // Rejecting token discovery must not hide a valid on-chain balance.
      }
      const wrappedBalance = await readWrappedNavcoinBalance(selected, market);
      setMetamaskNavcoinBalance(wrappedBalance.toString());
      if (mode === 'redeem' && BigInt(snapshot?.navcoinBalance || 0) < BigInt(amountAtoms || 0)
        && wrappedBalance >= BigInt(amountAtoms || 0)) setRedeemSource('ethereum');
    } catch (error) {
      setActionError(error.message || 'Unable to connect MetaMask');
    }
  };

  const execute = async () => {
    setActionError('');
    setLastCompleted(null);
    setExportPacketHash('');
    setExecuting(true);
    let issueOperations = null;
    let reserved = false;
    let released = false;
    let exported = false;
    let returned = false;
    try {
      const fresh = await refresh();
      if (!fresh) throw new Error('Could not refresh the market immediately before signing');
      let wrappedBalanceBefore = null;
      if (mode === 'redeem' && redeemSource === 'ethereum') {
        if (!proxyAuthToken) throw new Error('Authenticated unattended return relay access is not available');
        const accounts = await window.ethereum?.request?.({ method: 'eth_requestAccounts' });
        const selected = String(accounts?.[0] || '').toLowerCase();
        if (selected !== ethereumRecipient) throw new Error('The connected MetaMask account changed; reconnect it before returning tokens');
        wrappedBalanceBefore = await readWrappedNavcoinBalance(selected, market);
        setMetamaskNavcoinBalance(wrappedBalanceBefore.toString());
      }
      const freshEvaluation = evaluateNavcoinResidentMarket({
        market,
        supplyStatus: fresh.route,
        navStatus: fresh.nav,
        chainStatus: fresh.chain,
        direction: mode,
        amountAtoms,
        settlementBalanceAtoms: fresh.settlementBalance,
        navcoinBalanceAtoms: mode === 'redeem' && redeemSource === 'ethereum'
          ? wrappedBalanceBefore?.toString() : fresh.navcoinBalance,
      });
      if (!freshEvaluation.ok) throw new Error(freshEvaluation.blockingReasons.join('. '));

      if (mode === 'issue') {
        if (!validEthereumRecipient) throw new Error('Connect or enter a lowercase Ethereum address');
        let wrappedBalanceBefore = null;
        if (delivery === 'ethereum') {
          if (!proxyAuthToken) throw new Error('Authenticated export relay access is not available');
          const relayReadiness = await loadNavcoinExportReadiness(fresh.route.route_id);
          if (relayReadiness.ready !== true
            || relayReadiness.route_id !== fresh.route.route_id
            || relayReadiness.route_config_digest !== fresh.route.route_config_digest
            || String(relayReadiness.wrapped_token || '').toLowerCase() !== market.wrappedToken) {
            throw new Error(`The unattended ${navSymbol} export relay is not ready for this governed route`);
          }
          await watchWrappedNavcoin(market);
          wrappedBalanceBefore = await readWrappedNavcoinBalance(ethereumRecipient, market);
          setMetamaskNavcoinBalance(wrappedBalanceBefore.toString());
          const draft = buildNavcoinIssueExportDraft({
            walletAddress: address,
            ethereumRecipient,
            supplyStatus: fresh.route,
            chainHeight: fresh.chain.block_height,
            amountAtoms,
            settlementAtoms: freshEvaluation.quote.settlementAtoms,
          });
          const preparedPacket = await txBuilder.preparePftlUniswapMintPacket(
            draft.policyHash,
            draft.mintPacket,
          );
          issueOperations = finalizeNavcoinIssueExportOperations(draft, preparedPacket);
        } else {
          issueOperations = buildNavcoinIssueOperations({
            walletAddress: address,
            ethereumRecipient,
            supplyStatus: fresh.route,
            chainHeight: fresh.chain.block_height,
            amountAtoms,
            settlementAtoms: freshEvaluation.quote.settlementAtoms,
          });
        }
        let exportRelayJob = null;
        if (delivery === 'ethereum') {
          exportRelayJob = await createNavcoinExportJob({
            routeId: fresh.route.route_id,
            routeConfigDigest: fresh.route.route_config_digest,
            packetHash: issueOperations.packetHash,
            packetDigest: issueOperations.packetDigest,
            ethereumRecipient,
            amountAtoms,
            deadlineSeconds: issueOperations.destinationDeadlineSeconds,
            proxyAuthToken,
          });
        }
        setProgress(delivery === 'ethereum' ? [
          { label: 'Reserve order', state: 'pending', detail: 'Bind verified NAV, capacity, and MetaMask recipient' },
          { label: `Mint ${navSymbol}`, state: 'pending', detail: `Exchange ${settlementSymbol} and increase native supply` },
          { label: `Export ${navSymbol}`, state: 'pending', detail: 'Consume the entitlement and finalize the proof packet' },
          { label: `Mint ${wrappedSymbol}`, state: 'pending', detail: `Waiting for the ${verificationCopy(fresh.route.outbound_verification_class)} on Ethereum` },
          { label: 'Verify MetaMask', state: 'pending', detail: 'Read the mainnet ERC-20 balance' },
        ] : [
          { label: 'Reserve order', state: 'pending', detail: 'Bind price, capacity, and recipient' },
          { label: `Mint ${navSymbol}`, state: 'pending', detail: `Exchange ${settlementSymbol} at verified NAV` },
          { label: 'Close reservation', state: 'pending', detail: 'Release unused export entitlement' },
          { label: 'Verify balances', state: 'pending', detail: 'Read finalized PFTL state' },
        ]);
        await runStep(0, issueOperations.reserve, 'Order reservation');
        reserved = true;
        await runStep(1, issueOperations.subscribe, `${navSymbol} issuance`);
        if (delivery === 'ethereum') {
          await runStep(2, issueOperations.export, `${navSymbol} export`);
          exported = true;
          setExportPacketHash(issueOperations.packetHash);
          updateStep(3, { state: 'running', detail: `Packet ${truncateMiddle(issueOperations.packetHash, 8)} finalized; enqueueing durable relay…` });
          const expectedBalance = wrappedBalanceBefore + BigInt(amountAtoms);
          const relayResult = await waitForNavcoinExportJob(fresh.route.route_id, exportRelayJob.job_id, {
            onStatus: status => updateStep(3, {
              state: 'running',
              detail: status.message || `Durable relay · ${String(status.status || 'queued').replaceAll('_', ' ')}`,
            }),
          });
          const wrappedBalance = await readWrappedNavcoinBalance(ethereumRecipient, market);
          if (wrappedBalance < expectedBalance) {
            throw new Error(`Relay reported finality but the expected ${wrappedSymbol} balance is not visible`);
          }
          setMetamaskNavcoinBalance(wrappedBalance.toString());
          updateStep(3, { state: 'done', detail: `${formatNavcoinUnits(amountAtoms, market.decimals)} ${wrappedSymbol} minted · ${shortTx(relayResult.ethereum_tx_hash || 'finalized')}` });
          updateStep(4, { state: 'done', detail: `${formatNavcoinUnits(wrappedBalance, market.decimals)} ${wrappedSymbol} held by ${truncateMiddle(ethereumRecipient, 7)}` });
        } else {
          await runStep(2, issueOperations.release, 'Reservation release');
          released = true;
          updateStep(3, { state: 'running', detail: 'Refreshing finalized balances…' });
        }
      } else {
        if (redeemSource === 'ethereum') {
          const readiness = await loadNavcoinReturnReadiness(fresh.route.route_id);
          if (readiness.ready !== true || readiness.route_id !== fresh.route.route_id
            || readiness.route_config_digest !== fresh.route.route_config_digest
            || String(readiness.controller || '').toLowerCase() !== String(fresh.route.handoff_controller || '').toLowerCase()) {
            throw new Error(`The unattended ${navSymbol} return relay is not ready for this governed route`);
          }
          if (wrappedBalanceBefore < BigInt(amountAtoms)) throw new Error(`MetaMask ${wrappedSymbol} balance is insufficient`);
          const returnNonce = createNavcoinReturnNonce();
          const calldata = buildNavcoinReturnBurnCalldata({
            amountAtoms,
            pftlRecipient: address,
            nativeNavAssetId: market.navAssetId,
            returnNonce,
          });
          setProgress([
            { label: `Return ${wrappedSymbol}`, state: 'running', detail: 'Confirm the self-custodial burn in MetaMask' },
            { label: 'Confirm Ethereum burn', state: 'pending', detail: 'Durably bind the transaction to this PFTL wallet' },
            { label: 'Prove finality', state: 'pending', detail: `Wait for Ethereum finality and certify the ${verificationCopy(fresh.route.return_verification_class)}` },
            { label: `Restore ${navSymbol}`, state: 'pending', detail: `Import native ${navSymbol} to PFTL` },
            { label: `Redeem ${navSymbol}`, state: 'pending', detail: `Burn ${navSymbol} and receive ${settlementSymbol}` },
            { label: 'Verify balances', state: 'pending', detail: 'Read finalized PFTL state' },
          ]);
          const transactionHash = String(await window.ethereum.request({
            method: 'eth_sendTransaction',
            params: [{ from: ethereumRecipient, to: fresh.route.handoff_controller, data: calldata, value: '0x0' }],
          }) || '').toLowerCase();
          if (!/^0x[0-9a-f]{64}$/.test(transactionHash)) throw new Error('MetaMask did not return a valid burn transaction hash');
          const storageKey = returnStorageKey(fresh.route.route_id, address);
          let recovery = {
            route_id: fresh.route.route_id,
            route_config_digest: fresh.route.route_config_digest,
            transaction_hash: transactionHash,
            ethereum_sender: ethereumRecipient,
            pftl_recipient: address,
            native_nav_asset_id: market.navAssetId,
            amount_atoms: String(amountAtoms),
            return_nonce: returnNonce,
            status: 'ethereum_submitted',
          };
          localStorage.setItem(storageKey, JSON.stringify(recovery));
          setPendingReturn(recovery);
          updateStep(0, { state: 'done', detail: shortTx(transactionHash), txId: transactionHash });
          updateStep(1, { state: 'running', detail: 'Registering the burn with the durable return relay…' });
          const job = await createNavcoinReturnJob({
            routeId: fresh.route.route_id,
            routeConfigDigest: fresh.route.route_config_digest,
            transactionHash,
            ethereumSender: ethereumRecipient,
            pftlRecipient: address,
            nativeNavAssetId: market.navAssetId,
            amountAtoms,
            returnNonce,
            proxyAuthToken,
          });
          recovery = { ...recovery, job_id: job.job_id, status: job.status };
          localStorage.setItem(storageKey, JSON.stringify(recovery));
          setPendingReturn(recovery);
          updateStep(1, { state: 'done', detail: `Durable job ${truncateMiddle(job.job_id, 8)}` });
          updateStep(2, { state: 'running', detail: job.message || 'Waiting for Ethereum finality…' });
          const returnResult = await waitForNavcoinReturnJob(fresh.route.route_id, job.job_id, {
            onStatus: status => {
              const stage = ['proving_ethereum_receipt', 'submitting_pftl_import'].includes(status.status) ? 3 : 2;
              updateStep(stage, { state: 'running', detail: status.message || String(status.status).replaceAll('_', ' ') });
              recovery = { ...recovery, status: status.status, message: status.message };
              localStorage.setItem(storageKey, JSON.stringify(recovery));
              setPendingReturn(recovery);
            },
          });
          returned = true;
          updateStep(2, { state: 'done', detail: `Ethereum block ${returnResult.ethereum_block_number || 'finalized'}` });
          updateStep(3, { state: 'done', detail: `${formatNavcoinUnits(amountAtoms, market.decimals)} ${navSymbol} restored on PFTL` });
          localStorage.removeItem(storageKey);
          setPendingReturn(null);
          const returnedSnapshot = await refresh();
          if (!returnedSnapshot || BigInt(returnedSnapshot.navcoinBalance || 0) < BigInt(amountAtoms)) {
            throw new Error(`Return finalized, but the restored ${navSymbol} balance is not visible yet`);
          }
          const returnedEvaluation = evaluateNavcoinResidentMarket({
            market,
            supplyStatus: returnedSnapshot.route,
            navStatus: returnedSnapshot.nav,
            chainStatus: returnedSnapshot.chain,
            direction: 'redeem',
            amountAtoms,
            settlementBalanceAtoms: returnedSnapshot.settlementBalance,
            navcoinBalanceAtoms: returnedSnapshot.navcoinBalance,
          });
          if (!returnedEvaluation.ok) throw new Error(`Return completed safely; redemption is blocked: ${returnedEvaluation.blockingReasons.join('. ')}`);
          if (BigInt(returnedEvaluation.quote.settlementAtoms) < BigInt(freshEvaluation.quote.settlementAtoms)) {
            throw new Error(`Return completed safely; the live redemption quote moved below ${formatNavcoinUnits(freshEvaluation.quote.settlementAtoms, market.settlementDecimals)} ${settlementSymbol}`);
          }
          const operation = buildNavcoinRedeemOperation({
            walletAddress: address,
            supplyStatus: returnedSnapshot.route,
            chainHeight: returnedSnapshot.chain.block_height,
            amountAtoms,
            minimumSettlementAtoms: freshEvaluation.quote.settlementAtoms,
          });
          await runStep(4, operation, `${navSymbol} redemption`);
          updateStep(5, { state: 'running', detail: 'Refreshing finalized balances…' });
        } else {
          const operation = buildNavcoinRedeemOperation({
            walletAddress: address,
            supplyStatus: fresh.route,
            chainHeight: fresh.chain.block_height,
            amountAtoms,
            minimumSettlementAtoms: freshEvaluation.quote.settlementAtoms,
          });
          setProgress([
            { label: `Redeem ${navSymbol}`, state: 'pending', detail: `Burn ${navSymbol} and receive ${settlementSymbol}` },
            { label: 'Verify balances', state: 'pending', detail: 'Read finalized PFTL state' },
          ]);
          await runStep(0, operation, `${navSymbol} redemption`);
          updateStep(1, { state: 'running', detail: 'Refreshing finalized balances…' });
        }
      }

      const verified = await refresh();
      if (!verified) throw new Error('Transaction finalized, but refreshed balances are unavailable');
      if (mode !== 'issue' || delivery !== 'ethereum') {
        const verifyIndex = mode === 'issue' ? 3 : (redeemSource === 'ethereum' ? 5 : 1);
        updateStep(verifyIndex, {
          state: 'done',
          detail: `${formatNavcoinUnits(verified.navcoinBalance, market.decimals)} ${navSymbol} · ${formatNavcoinUnits(verified.settlementBalance, market.settlementDecimals)} ${settlementSymbol}`,
        });
      }
      setLastCompleted(mode === 'issue' && delivery === 'ethereum' ? 'ethereum' : mode);
      onToast?.(mode === 'issue'
        ? (delivery === 'ethereum' ? `${wrappedSymbol} is now held in MetaMask` : `${navSymbol} issued at verified NAV`)
        : `${navSymbol} redeemed to ${settlementSymbol}`);
    } catch (error) {
      if (issueOperations && reserved && !released && !exported) {
        try {
          await txBuilder.sendAssetTransfer(backupJson, address, { operation: issueOperations.release });
          released = true;
        } catch (_) {
          // Surface the recovery requirement below. Never imply cleanup succeeded.
        }
      }
      setProgress(current => current.map(step => (
        step.state === 'running'
          ? { ...step, state: 'failed', detail: error.message || 'Transaction failed' }
          : step
      )));
      setActionError(
        `${error.message || `${navSymbol} transaction failed`}${issueOperations && reserved && !released
          && !exported ? ' The reservation could not be released automatically; do not retry until route status is reconciled.'
          : ''}${returned ? ` ${navSymbol} was restored to PFTL; do not burn ${wrappedSymbol} again.` : ''}`,
      );
      await refresh();
    } finally {
      setExecuting(false);
    }
  };

  const route = snapshot?.route;
  const nav = snapshot?.nav;
  const navPacketMatches = Boolean(
    route?.pricing_reserve_packet_hash
    && nav?.finalized_reserve_packet_hash
    && nav.finalized_reserve_packet_hash === route.pricing_reserve_packet_hash,
  );
  const routeHealthy = route?.live_value_enabled === true
    && route?.paused === false
    && route?.invariant_holds === true
    && navPacketMatches;
  const outboundProofLabel = verificationCopy(route?.outbound_verification_class);
  const returnProofLabel = verificationCopy(route?.return_verification_class);
  const uniswapBuyUrl = `https://app.uniswap.org/swap?chain=mainnet&inputCurrency=${ETH_MAINNET_USDC}&outputCurrency=${market.wrappedToken}`;
  const uniswapSellUrl = `https://app.uniswap.org/swap?chain=mainnet&inputCurrency=${market.wrappedToken}&outputCurrency=${ETH_MAINNET_USDC}`;
  const displayBlockers = evaluation.blockingReasons
    .filter(reason => reason !== `enter a positive ${navSymbol} amount`)
    .map(reason => {
      if (mode === 'redeem' && redeemSource === 'ethereum'
        && reason === `wallet ${navSymbol} balance is insufficient`) return `MetaMask ${wrappedSymbol} balance is insufficient or not connected`;
      if (/balance is insufficient|capacity is insufficient|exceeds|paused/i.test(reason)) return reason;
      return 'Live NAV and backing could not be verified. Refresh the market before trying again.';
    })
    .filter((reason, index, reasons) => reasons.indexOf(reason) === index);

  return (
    <section
      className="navcoin-primary-page"
      data-testid="navcoin-market"
      data-export-packet-hash={exportPacketHash || undefined}
    >
      <header className="navcoin-primary-hero">
        <div>
          <div className="fs-kicker"><span className="fs-live-dot" /> {navSymbol} · VERIFIED NAV</div>
          <h1>Buy or redeem {navSymbol}<br /> at verified NAV.</h1>
          <p>
            Buy newly issued {navSymbol} with {settlementSymbol}, keep it on PFTL or deliver it as {wrappedSymbol} to Ethereum, and redeem PFTL or Ethereum holdings back to {settlementSymbol}.
            These prices come from the verified NAV reserve, not the separate Uniswap market.
          </p>
        </div>
        <div style={{ display: 'grid', gap: 8, justifyItems: 'end' }}>
          <label className="navcoin-primary-label" htmlFor="navcoin-market-select">NAVCoin market</label>
          <select
            id="navcoin-market-select"
            className="pf-input"
            value={market.key}
            onChange={event => onSelectMarket?.(event.target.value)}
            disabled={executing || markets.length < 2}
          >
            {markets.map(option => <option key={option.key} value={option.key}>{option.symbol}</option>)}
          </select>
          <button className="pf-button secondary" onClick={refresh} disabled={loading || executing}>
            {loading ? 'Refreshing…' : 'Refresh market'}
          </button>
        </div>
      </header>

      {refreshError && <div className="pf-error">{refreshError}</div>}

      <div className="navcoin-primary-safety">
        <div className={`navcoin-primary-shield ${loading || routeHealthy ? '' : 'bad'}`}>
          {loading ? '…' : routeHealthy ? '✓' : '!'}
        </div>
        <div>
          <span>MARKET STATUS</span>
          <strong>{loading
            ? 'Checking live NAV and backing…'
            : routeHealthy ? 'Ready · NAV and backing are verified' : 'Trading is unavailable'}</strong>
          <small>{loading
            ? 'The wallet is loading the latest finalized market state. No signature is requested.'
            : routeHealthy ? 'The wallet rechecks the active route and NAV before signing.' : 'No funds will move while verification is unavailable.'}</small>
        </div>
        <div className="navcoin-primary-pins">
          <span>BACKING AVAILABLE FOR REDEMPTION</span>
          <strong>{formatNavcoinUnits(route?.settlement_reserve_atoms, market.settlementDecimals)} {settlementSymbol}</strong>
          <small>{navPacketMatches ? 'Matched to the finalized backing record' : 'Backing verification unavailable'}</small>
        </div>
      </div>

      <div className="navcoin-primary-metrics">
        <div><span>Verified NAV</span><strong>{formatNavcoinNav(nav?.nav_per_unit)}</strong><small>USD per {navSymbol} · pre-inflow</small></div>
        <div><span>Reserve</span><strong>{formatNavcoinUnits(route?.settlement_reserve_atoms, market.settlementDecimals)}</strong><small>{settlementSymbol} counted on PFTL</small></div>
        <div><span>Available to buy</span><strong>{formatNavcoinUnits(route?.available_issue_atoms, market.decimals)}</strong><small>{navSymbol} at the current verified NAV</small></div>
        <div><span>Available to redeem</span><strong>{formatNavcoinUnits(route?.available_redeem_atoms, market.decimals)}</strong><small>{navSymbol} against the reserve</small></div>
      </div>

      <div className="navcoin-primary-workspace">
        <div className="navcoin-primary-trade-card">
          <div className="navcoin-primary-tabs">
            <button className={mode === 'issue' ? 'on' : ''} onClick={() => { setMode('issue'); setProgress([]); setActionError(''); }}>Buy at NAV</button>
            <button className={mode === 'redeem' ? 'on' : ''} onClick={() => {
              setMode('redeem');
              if (BigInt(snapshot?.navcoinBalance || 0) < BigInt(amountAtoms || 0)) setRedeemSource('ethereum');
              setProgress([]);
              setActionError('');
            }}>Redeem at NAV</button>
          </div>

          <label className="navcoin-primary-label" htmlFor="navcoin-amount">{navSymbol} amount</label>
          <div className="navcoin-primary-amount">
            <input id="navcoin-amount" inputMode="decimal" value={amount} onChange={event => setAmount(event.target.value)} disabled={executing} />
            <strong>{navSymbol}</strong>
            <button onClick={() => {
              const maximum = mode === 'issue'
                ? route?.available_issue_atoms
                : (BigInt(redeemBalance || 0) < BigInt(route?.available_redeem_atoms || 0)
                  ? redeemBalance
                  : route?.available_redeem_atoms);
              if (maximum) setAmount(formatNavcoinUnits(maximum, market.decimals));
            }}>MAX</button>
          </div>

          <div className="navcoin-primary-quote">
            <div><span>{mode === 'issue' ? 'You receive' : 'You redeem'}</span><strong>{amountAtoms ? formatNavcoinUnits(amountAtoms, market.decimals) : '—'} {navSymbol}</strong></div>
            <div><span>{mode === 'issue' ? 'You pay' : 'You receive at least'}</span><strong>{formatNavcoinUnits(quote?.settlementAtoms, market.settlementDecimals)} {settlementSymbol}</strong></div>
            <div><span>NAV reserve value</span><strong>{formatNavcoinUnits(quote?.baseReserveAtoms, market.settlementDecimals)} {settlementSymbol}</strong></div>
            <div><span>{mode === 'issue' ? 'Purchase spread' : 'Redemption spread'}</span><strong>{formatNavcoinUnits(quote?.spreadAtoms, market.settlementDecimals)} {settlementSymbol}</strong></div>
            <div><span>PFTL network fees</span><strong>Quoted and checked before each local signature</strong></div>
          </div>

          {mode === 'issue' && (
            <div className="navcoin-primary-recipient">
              <div className="navcoin-primary-tabs" role="group" aria-label={`${navSymbol} delivery destination`}>
                <button className={delivery === 'ethereum' ? 'on' : ''} onClick={() => setDelivery('ethereum')} disabled={executing}>Deliver to MetaMask</button>
                <button className={delivery === 'pftl' ? 'on' : ''} onClick={() => setDelivery('pftl')} disabled={executing}>Keep on PFTL</button>
              </div>
              <div>
                <label className="navcoin-primary-label" htmlFor="navcoin-eth-recipient">Ethereum recipient</label>
                <small>{delivery === 'ethereum'
                  ? `After PFTL finalizes the export, ${wrappedSymbol} is minted directly to this MetaMask account.`
                  : `Bound for recovery safety; the purchased ${navSymbol} remains native on PFTL.`}</small>
              </div>
              <button className="pf-button secondary" onClick={connectEthereum} disabled={executing}>Connect MetaMask</button>
              <input
                id="navcoin-eth-recipient"
                className="pf-input"
                placeholder="0x…"
                value={ethereumRecipient}
                onChange={event => setEthereumRecipient(event.target.value.trim())}
                disabled={executing}
              />
            </div>
          )}

          {mode === 'redeem' && (
            <div className="navcoin-primary-recipient">
              <div className="navcoin-primary-tabs" role="group" aria-label={`${navSymbol} redemption source`}>
                <button className={redeemSource === 'pftl' ? 'on' : ''} onClick={() => setRedeemSource('pftl')} disabled={executing}>From PFTL</button>
                <button className={redeemSource === 'ethereum' ? 'on' : ''} onClick={() => setRedeemSource('ethereum')} disabled={executing}>From MetaMask</button>
              </div>
              <div>
                <label className="navcoin-primary-label" htmlFor="navcoin-return-account">Redemption source</label>
                <small>{redeemSource === 'ethereum'
                  ? `Burn ${wrappedSymbol} in MetaMask, wait for Ethereum confirmation, then redeem the restored ${navSymbol} to ${settlementSymbol} in one guided flow.`
                  : `Redeem native ${navSymbol} already held by this PFTL wallet.`}</small>
              </div>
              {redeemSource === 'ethereum' && <>
                <button className="pf-button secondary" onClick={connectEthereum} disabled={executing}>Connect MetaMask</button>
                <input id="navcoin-return-account" className="pf-input" placeholder="Connect MetaMask" value={ethereumRecipient} readOnly disabled={executing} />
              </>}
            </div>
          )}

          {pendingReturn && mode === 'redeem' && (
            <div className="navcoin-primary-blockers">
              <span>• Existing MetaMask return: {String(pendingReturn.status || 'submitted').replaceAll('_', ' ')}.</span>
              <span>• Transaction {truncateMiddle(pendingReturn.transaction_hash, 8)} will continue through the durable relay.</span>
            </div>
          )}

          {!loading && displayBlockers.length > 0 && (
            <div className="navcoin-primary-blockers">
              {displayBlockers.slice(0, 4).map(reason => <span key={reason}>• {reason}</span>)}
            </div>
          )}
          {mode === 'issue' && evaluation.blockingReasons.includes(`wallet ${settlementSymbol} balance is insufficient`) && (
            <button className="pf-button secondary" onClick={() => onNavigate?.('bridge')} disabled={executing}>
              Add {settlementSymbol} from Ethereum
            </button>
          )}
          {!finalityReady && <div className="navcoin-primary-blockers"><span>• Transactions are temporarily unavailable. No funds will move.</span></div>}
          {actionError && <div className="pf-error">{actionError}</div>}
          {mode === 'issue' && delivery === 'ethereum' && !proxyAuthToken && (
            <div className="navcoin-primary-blockers"><span>• Ethereum delivery is temporarily unavailable. Keep the purchase on PFTL or retry later.</span></div>
          )}
          {mode === 'redeem' && redeemSource === 'ethereum' && !proxyAuthToken && (
            <div className="navcoin-primary-blockers"><span>• Returning {wrappedSymbol} from Ethereum is temporarily unavailable. No token has been burned.</span></div>
          )}

          <button className="pf-primary" disabled={!canExecute} onClick={execute}>
            {executing ? (mode === 'redeem' && redeemSource === 'ethereum' ? 'Returning & redeeming…'
              : delivery === 'ethereum' ? 'Exporting to MetaMask…' : 'Finalizing on PFTL…') : mode === 'issue'
              ? `${delivery === 'ethereum' ? 'Mint & export' : 'Mint'} ${amountAtoms ? formatNavcoinUnits(amountAtoms, market.decimals) : '—'} ${navSymbol}`
              : `${redeemSource === 'ethereum' ? 'Return & redeem' : 'Redeem'} ${amountAtoms ? formatNavcoinUnits(amountAtoms, market.decimals) : '—'} ${navSymbol}`}
          </button>
          <p className="navcoin-primary-signing">{mode === 'redeem' && redeemSource === 'ethereum'
            ? `MetaMask confirms the ${wrappedSymbol} return; this browser signs the PFTL redemption. Your recovery seed never leaves the browser.`
            : 'This browser signs locally. Your recovery seed never leaves the browser.'}</p>
        </div>

        <aside className="navcoin-primary-side">
          <div className="pf-card">
            <div className="navcoin-primary-side-title"><span>YOUR BALANCES</span><small>finalized</small></div>
            <div className="navcoin-primary-balance"><span>{settlementSymbol}</span><strong>{formatNavcoinUnits(snapshot?.settlementBalance, market.settlementDecimals)}</strong></div>
            <div className="navcoin-primary-balance"><span>{navSymbol}</span><strong>{formatNavcoinUnits(snapshot?.navcoinBalance, market.decimals)}</strong></div>
            <div className="navcoin-primary-balance"><span>{wrappedSymbol} · MetaMask</span><strong>{formatNavcoinUnits(metamaskNavcoinBalance, market.decimals)}</strong></div>
          </div>
          <div className="pf-card" style={{ display: 'grid', gap: 12 }}>
            <div className="navcoin-primary-side-title" style={{ marginBottom: 0 }}><span>SECONDARY MARKET</span><small>Ethereum</small></div>
            <strong style={{ fontSize: 16 }}>Trade {wrappedSymbol} on Uniswap</strong>
            <p style={{ color: 'var(--muted)', fontSize: 12, lineHeight: 1.55 }}>
              Uniswap sets the live market price, slippage, and Ethereum gas. Review its quote before confirming in MetaMask.
            </p>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
              <a className="pf-button" style={{ textDecoration: 'none' }} href={uniswapBuyUrl} target="_blank" rel="noopener noreferrer">Buy {wrappedSymbol}</a>
              <a className="pf-button secondary" style={{ textDecoration: 'none' }} href={uniswapSellUrl} target="_blank" rel="noopener noreferrer">Sell {wrappedSymbol}</a>
            </div>
            <small style={{ color: 'var(--dim)', lineHeight: 1.5 }}>
              To redeem a purchase at NAV, return here and choose Redeem at NAV → From MetaMask.
            </small>
          </div>
          <details className="pf-card navcoin-primary-details">
            <summary style={{ cursor: 'pointer', fontWeight: 650 }}>Advanced verification details</summary>
            <div style={{ display: 'grid', gap: 10, paddingTop: 14 }}>
              <div><span>Buy price</span><strong>{route?.issue_multiplier_bps ? `${(Number(route.issue_multiplier_bps) / 10000).toFixed(3)} × NAV` : '—'}</strong></div>
              <div><span>Redeem price</span><strong>{route?.redeem_multiplier_bps ? `${(Number(route.redeem_multiplier_bps) / 10000).toFixed(4)} × NAV` : '—'}</strong></div>
              <div><span>Reserve proof</span><strong>{route?.pricing_reserve_packet_hash ? truncateMiddle(route.pricing_reserve_packet_hash, 8) : '—'}</strong></div>
              <div><span>Route</span><strong title={route?.route_id}>{route?.route_id ? truncateMiddle(route.route_id, 12) : '—'}</strong></div>
              <div><span>{wrappedSymbol}</span><strong title={route?.wrapped_navcoin_token}>{route?.wrapped_navcoin_token ? truncateMiddle(route.wrapped_navcoin_token, 8) : '—'}</strong></div>
            </div>
          </details>
        </aside>
      </div>

      {progress.length > 0 && (
        <div className="navcoin-primary-progress">
          <div className="navcoin-primary-progress-head">
            <strong>{lastCompleted ? (lastCompleted === 'ethereum' ? `${wrappedSymbol} delivered to MetaMask` : lastCompleted === 'issue' ? `${navSymbol} purchase complete` : `${navSymbol} redemption complete`) : 'Finality progress'}</strong>
            <small>Proof relay jobs continue safely if this page closes.</small>
          </div>
          {progress.map((step, index) => (
            <div className={`navcoin-primary-progress-step ${step.state}`} key={`${step.label}-${index}`}>
              <span>{step.state === 'done' ? '✓' : index + 1}</span>
              <div><strong>{step.label}</strong><small>{step.detail}</small></div>
            </div>
          ))}
        </div>
      )}

      <div className="navcoin-primary-venue-note">
        <strong>{mode === 'redeem'
          ? (redeemSource === 'ethereum' ? `${wrappedSymbol} returns through PFTL before redemption.` : `${navSymbol} redeems natively on PFTL.`)
          : (delivery === 'ethereum' ? `${wrappedSymbol} is delivered directly to MetaMask.` : `${navSymbol} is delivered natively on PFTL.`)}</strong>
        <span>
          The deployed Ethereum token is {route?.wrapped_navcoin_token ? truncateMiddle(route.wrapped_navcoin_token, 8) : wrappedSymbol}.
          {mode === 'redeem'
            ? (redeemSource === 'ethereum'
              ? ` MetaMask burns the wrapped token, the durable relay restores native ${navSymbol} from the ${returnProofLabel}, and only then does your wallet sign redemption.`
              : ` Native ${navSymbol} is burned only when the ${settlementSymbol} redemption finalizes.`)
            : (delivery === 'ethereum'
              ? ` The wallet preserves the issuance entitlement, finalizes the PFTL export, and waits for its ${outboundProofLabel} before reporting success.`
              : ' This mode closes the unused export entitlement and keeps the balance on PFTL.')}
        </span>
      </div>
    </section>
  );
}
