import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ArrowLeftRight,
  ArrowRight,
  Check,
  ChevronDown,
  Clock,
  Fuel,
  Info,
  Landmark,
  Loader2,
  RefreshCw,
  ShieldCheck,
} from 'lucide-react';
import * as evm from '../lib/evm.js';
import * as utils from '../lib/utils.js';
import { relayVaultDeposit } from '../lib/bridge-relay.js';
import { loadGovernedVaultBridgeRoute } from '../lib/bridge-route.js';

const cctpModules = import.meta.glob('../lib/cctp.js');

const ETH_MAINNET_CHAIN_ID = utils.ETH_MAINNET_CHAIN_ID || 1;
const ETH_MAINNET_USDC = utils.ETH_MAINNET_USDC || '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
const ETH_MAINNET_RPC = Reflect.get(utils, 'ETH_MAINNET_RPC_BROWSER') || '/eth-rpc';
const ARBITRUM_CHAIN_ID = utils.ARBITRUM_CHAIN_ID || 42161;
const ARBITRUM_RPC = Reflect.get(utils, 'ARBITRUM_RPC_BROWSER') || '/arb-rpc';
const ARBITRUM_USDC = utils.USDC_CONTRACT_ARBITRUM || '0xaf88d065e77c8cC2239327C5EDb3A432268e5831';
const ARBITRUM_ETH_BRIDGE_URL = 'https://bridge.arbitrum.io/?sourceChain=ethereum&destinationChain=arbitrum-one';
const LAST_CCTP_BURN_TX_KEY = 'postfiat:cctp:lastBurnTx';

const STATUS_COPY = {
  disconnected: 'Connect MetaMask to detect USDC on Ethereum and Arbitrum.',
  detecting: 'Reading Ethereum and Arbitrum USDC balances.',
  connected: 'Arbitrum USDC is ready for the vault deposit.',
  'need-gas': 'Add Arbitrum ETH gas before bridging USDC or depositing to the vault.',
  'need-bridge': 'Bridge Ethereum USDC to Arbitrum before depositing to the vault.',
  bridging: 'Ethereum to Arbitrum Fast Transfer is in progress.',
  approving: 'USDC approval is waiting for confirmation.',
  approved: 'USDC is approved. Submit the bridge vault deposit when ready.',
  depositing: 'Vault deposit transaction is being submitted and indexed.',
  deposited: 'Deposit confirmed. Starting the PFTL relay.',
  relaying: 'Relay is submitting the PFTL mint and claim.',
  complete: 'pfUSDC is in the PFTL wallet.',
  error: 'Bridge action needs attention.',
};

const FLOW_STEPS = [
  { id: 1, key: 'gas', label: 'Gas', full: 'Add Arbitrum ETH gas', Icon: Fuel },
  { id: 2, key: 'bridge', label: 'Bridge', full: 'Bridge USDC to Arbitrum', Icon: ArrowLeftRight },
  { id: 3, key: 'approve', label: 'Approve', full: 'Approve the vault', Icon: ShieldCheck },
  { id: 4, key: 'deposit', label: 'Deposit', full: 'Deposit to the vault', Icon: Landmark },
  { id: 5, key: 'relay', label: 'Relay', full: 'Wait for the relay', Icon: Clock },
];

function trimUsdc(value) {
  return String(value || '0')
    .replace(/(\.\d*?)0+$/, '$1')
    .replace(/\.$/, '');
}

function normalizeAmountInput(value) {
  const cleaned = String(value || '').replace(/[^\d.]/g, '');
  const [whole, ...rest] = cleaned.split('.');
  if (!rest.length) return whole;
  return `${whole}.${rest.join('').slice(0, 6)}`;
}

function safeAtoms(value) {
  const text = String(value || '').trim();
  if (!text || !/^\d*(\.\d{0,6})?$/.test(text) || text === '.') return null;
  try {
    return evm.usdcToAtoms(text);
  } catch (_) {
    return null;
  }
}

function balanceLabel(atoms) {
  return `${trimUsdc(evm.atomsToUsdc(atoms || 0n))} USDC`;
}

function ethLabel(wei) {
  const n = BigInt(wei || 0n);
  const scale = 10n ** 18n;
  const whole = n / scale;
  const fraction = n % scale;
  if (fraction === 0n) return `${whole} ETH`;
  return `${whole}.${fraction.toString().padStart(18, '0').slice(0, 8).replace(/0+$/, '') || '0'} ETH`;
}

function elapsedLabel(ms) {
  const totalSeconds = Math.max(0, Math.floor((ms || 0) / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, '0');
  return `${minutes}:${seconds} elapsed`;
}

function compactHash(value, len = 7) {
  return utils.truncateMiddle ? utils.truncateMiddle(value, len) : value;
}

function chainHex(chainId) {
  return `0x${Number(chainId).toString(16)}`;
}

function chainName(chainId) {
  if (Number(chainId) === ETH_MAINNET_CHAIN_ID) return 'Ethereum';
  if (Number(chainId) === ARBITRUM_CHAIN_ID) return 'Arbitrum';
  if (!chainId) return 'unknown';
  return `chain ${chainId}`;
}

function etherscanTxUrl(txHash) {
  return txHash ? `https://etherscan.io/tx/${txHash}` : '';
}

function arbiscanTxUrl(txHash) {
  return txHash ? `https://arbiscan.io/tx/${txHash}` : '';
}

function cctpBurnTx(burn) {
  return normalizeTxHash(burn?.burnTxHash || burn?.txHash || burn?.transactionHash);
}

function isTxHash(value) {
  return Boolean(normalizeTxHash(value));
}

function normalizeTxHash(value) {
  const text = String(value || '').trim();
  if (!text) return '';
  const prefixed = text.toLowerCase().startsWith('0x') ? text : `0x${text}`;
  return /^0x[0-9a-fA-F]{64}$/.test(prefixed) ? prefixed : '';
}

function readStoredBurnTx() {
  if (typeof window === 'undefined') return '';
  try {
    return window.localStorage?.getItem(LAST_CCTP_BURN_TX_KEY) || '';
  } catch (_) {
    return '';
  }
}

function storeBurnTx(txHash) {
  const normalizedTxHash = normalizeTxHash(txHash);
  if (!normalizedTxHash || typeof window === 'undefined') return;
  try {
    window.localStorage?.setItem(LAST_CCTP_BURN_TX_KEY, normalizedTxHash);
  } catch (_) {
    // Local storage is best-effort; the manual resume input remains available.
  }
}

function pendingStatusKind(status) {
  if (typeof status === 'string') {
    const value = status.toLowerCase();
    if (value === 'not_found') return 'not-found';
    if (value === 'complete' || value === 'ready' || value === 'attestation_complete') return 'ready';
    return 'pending';
  }
  if (!status || status.found === false || status.status === 'not_found') return 'not-found';
  const normalized = String(status.status || status.attestationStatus || '').toLowerCase();
  if (
    status.attestation
    || status.attestationComplete
    || status.attestationReady
    || status.ready
    || normalized === 'complete'
    || normalized === 'ready'
    || normalized === 'attestation_complete'
  ) {
    return 'ready';
  }
  return 'pending';
}

function humanEvmError(error) {
  const message = error?.message || String(error || 'unknown error');
  const data = typeof error?.data === 'string' ? error.data.toLowerCase() : '';
  if (data.startsWith('0xbe24f3c5')) {
    return 'The vault could not pull USDC from this wallet. Approve Arbitrum USDC for the vault again, then retry the deposit.';
  }
  if (data.startsWith('0xda9f8b34')) {
    return 'The bridge vault is paused.';
  }
  if (data.startsWith('0x2c5211c6')) {
    return 'Deposit amount must be greater than 0.';
  }
  if (data.startsWith('0x02694994')) {
    return 'PFTL recipient is missing.';
  }
  if (data.startsWith('0x73b49782')) {
    return 'PFTL recipient is too long for the bridge vault.';
  }
  if (data.startsWith('0xfa98d908')) {
    return 'This deposit nonce was already used. Retry to generate a fresh deposit nonce.';
  }
  if (/insufficient funds|insufficient balance/i.test(message)) {
    return 'Not enough Arbitrum ETH for gas. Add a small amount of ETH on Arbitrum, then retry.';
  }
  if (/execution reverted|revert/i.test(message)) {
    return `Transaction preflight reverted: ${message}`;
  }
  return message;
}

function isAttestationReady(burn) {
  return pendingStatusKind(burn) === 'ready';
}

function cctpBurnAmountLabel(burn) {
  const amount = burn?.amount ?? burn?.amountAtoms;
  if (amount === null || amount === undefined || amount === '') return 'USDC';
  try {
    return `${trimUsdc(evm.atomsToUsdc(BigInt(amount)))} USDC`;
  } catch (_) {
    const parsed = Number(amount);
    return Number.isFinite(parsed) ? `${trimUsdc(parsed / 1e6)} USDC` : 'USDC';
  }
}

function BridgeBalanceRow({ label, value, unit = '', active = false }) {
  return (
    <div className={`pfb-balance-row${active ? ' active' : ''}`}>
      <span>{label}</span>
      <strong>{value}{unit ? <small> {unit}</small> : null}</strong>
    </div>
  );
}

function BridgeContextRow({ label, value, href = '' }) {
  return (
    <div className="pfb-context-row">
      <span>{label}</span>
      {href ? (
        <a href={href} target="_blank" rel="noreferrer">{value}</a>
      ) : (
        <strong>{value}</strong>
      )}
    </div>
  );
}

function BridgeLedgerRow({ label, value, href = '' }) {
  const empty = !value || value === 'none' || value === 'pending';
  return (
    <div className="pfb-ledger-row">
      <span>{label}</span>
      {href ? (
        <a href={href} target="_blank" rel="noreferrer">{value}</a>
      ) : (
        <strong className={empty ? 'empty' : ''}>{value || 'none'}</strong>
      )}
    </div>
  );
}

function resumeStepLabel(step, data = {}) {
  if (
    step === 'fetching_burn_receipt'
    || step === 'fetching_receipt'
    || step === 'fetching-receipt'
    || step === 'burn_receipt'
  ) {
    return 'Fetching burn receipt...';
  }
  if (step === 'extracting_message' || step === 'extracting-message' || step === 'message_sent') {
    return 'Extracting message...';
  }
  if (
    step === 'fetching_attestation'
    || step === 'fetching-attestation'
    || step === 'attesting'
    || step === 'attestation_pending'
  ) {
    return `Waiting for Circle attestation... ${data.messageHash ? compactHash(data.messageHash, 8) : ''}`;
  }
  if (step === 'attestation_complete') {
    return 'Ready to mint - auto-triggering...';
  }
  if (step === 'minting' || step === 'mint_start' || step === 'mint_submitted') {
    return `Minting on Arbitrum... ${data.txHash ? compactHash(data.txHash, 8) : ''}`;
  }
  if (step === 'done') return 'Done! USDC minted.';
  return 'Resuming CCTP bridge...';
}

function encodeErc20BalanceOf(owner) {
  const selector = '0x70a08231';
  const paddedOwner = owner.toLowerCase().replace('0x', '').padStart(64, '0');
  return selector + paddedOwner;
}

function encodeErc20Approve(spender, amount) {
  const selector = '0x095ea7b3';
  const paddedSpender = spender.toLowerCase().replace('0x', '').padStart(64, '0');
  const paddedAmount = BigInt(amount).toString(16).padStart(64, '0');
  return selector + paddedSpender + paddedAmount;
}

async function jsonRpcCall(url, method, params) {
  const response = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: Date.now(), method, params }),
  });
  if (!response.ok) throw new Error(`RPC returned HTTP ${response.status}`);
  const payload = await response.json();
  if (payload.error) throw new Error(payload.error.message || 'RPC returned an error');
  return payload.result;
}

async function readEthereumUsdcBalance(evmAddress, currentChainId) {
  const data = encodeErc20BalanceOf(evmAddress);
  if (Number(currentChainId) === ETH_MAINNET_CHAIN_ID && evm.hasMetaMask()) {
    const result = await window.ethereum.request({
      method: 'eth_call',
      params: [{ to: ETH_MAINNET_USDC, data }, 'latest'],
    });
    return BigInt(result || 0);
  }
  const result = await jsonRpcCall(ETH_MAINNET_RPC, 'eth_call', [{ to: ETH_MAINNET_USDC, data }, 'latest']);
  return BigInt(result || 0);
}

async function readArbitrumUsdcBalance(evmAddress) {
  if (typeof evm.getArbitrumUsdcBalance === 'function') {
    return BigInt(await evm.getArbitrumUsdcBalance(evmAddress));
  }
  const data = encodeErc20BalanceOf(evmAddress);
  const result = await jsonRpcCall(ARBITRUM_RPC, 'eth_call', [{ to: ARBITRUM_USDC, data }, 'latest']);
  return BigInt(result || 0);
}

async function readCurrentChainId() {
  const hex = await window.ethereum.request({ method: 'eth_chainId' });
  return Number.parseInt(hex, 16);
}

async function ensureChain(chainId) {
  await window.ethereum.request({
    method: 'wallet_switchEthereumChain',
    params: [{ chainId: chainHex(chainId) }],
  });
}

async function waitForTransactionReceipt(txHash, { timeoutMs = 1200_000, intervalMs = 2500 } = {}) {
  if (!txHash || !window.ethereum?.request) return null;
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    const receipt = await window.ethereum.request({
      method: 'eth_getTransactionReceipt',
      params: [txHash],
    });
    if (receipt?.blockNumber) return receipt;
    await new Promise(resolve => setTimeout(resolve, intervalMs));
  }
  throw new Error('Timed out waiting for transaction confirmation.');
}

async function approveEthereumUsdc(spender, amount, from) {
  await ensureChain(ETH_MAINNET_CHAIN_ID);
  const txHash = await window.ethereum.request({
    method: 'eth_sendTransaction',
    params: [{
      from,
      to: ETH_MAINNET_USDC,
      data: encodeErc20Approve(spender, amount),
    }],
  });
  await waitForTransactionReceipt(txHash);
  return txHash;
}

async function loadCctpBridge() {
  const loader = cctpModules['../lib/cctp.js'];
  if (!loader) {
    throw new Error('Circle CCTP bridge module is not available yet. Reload after the CCTP bridge module lands.');
  }
  const mod = await loader();
  if (typeof mod.cctpBridgeUsdcV2 !== 'function') {
    throw new Error('Circle CCTP V2 Fast Transfer helper is unavailable. Reload after the CCTP bridge module update lands.');
  }
  return mod;
}

export default function Bridge({
  address,
  rpc,
}) {
  const [phase, setPhase] = useState('disconnected');
  const [connectedAddress, setConnectedAddress] = useState('');
  const [currentChainId, setCurrentChainId] = useState(null);
  const [ethBalanceAtoms, setEthBalanceAtoms] = useState(0n);
  const [arbBalanceAtoms, setArbBalanceAtoms] = useState(0n);
  const [arbEthBalanceWei, setArbEthBalanceWei] = useState(0n);
  const [balanceStatus, setBalanceStatus] = useState('idle');
  const [amount, setAmount] = useState('');
  const [approvedAtoms, setApprovedAtoms] = useState(null);
  const [approvalTx, setApprovalTx] = useState('');
  const [bridgeApprovalTx, setBridgeApprovalTx] = useState('');
  const [bridgeTx, setBridgeTx] = useState('');
  const [bridgeMintTx, setBridgeMintTx] = useState('');
  const [bridgeMessageHash, setBridgeMessageHash] = useState('');
  const [bridgeStep, setBridgeStep] = useState('');
  const [bridgeStartedAt, setBridgeStartedAt] = useState(null);
  const [bridgeElapsedMs, setBridgeElapsedMs] = useState(0);
  const [bridgeStage, setBridgeStage] = useState('idle');
  const [manualMintAvailable, setManualMintAvailable] = useState(false);
  const [resumeBurnTx, setResumeBurnTx] = useState(readStoredBurnTx);
  const [resumeStatus, setResumeStatus] = useState('idle');
  const [resumeMessage, setResumeMessage] = useState('');
  const [pendingBurns, setPendingBurns] = useState([]);
  const [showManualResume, setShowManualResume] = useState(false);
  const [depositTx, setDepositTx] = useState('');
  const [depositId, setDepositId] = useState('');
  const [relayStatus, setRelayStatus] = useState('idle');
  const [relayMessage, setRelayMessage] = useState('');
  const [relayResult, setRelayResult] = useState(null);
  const [relayTxs, setRelayTxs] = useState([]);
  const [pfusdcBalanceAtoms, setPfusdcBalanceAtoms] = useState(null);
  const [error, setError] = useState('');
  const [governedRoute, setGovernedRoute] = useState(null);
  const [routeStatus, setRouteStatus] = useState('loading');
  const [routeError, setRouteError] = useState('');

  const refreshGovernedRoute = useCallback(async ({ expectedProfileHash = '' } = {}) => {
    setRouteStatus('loading');
    setRouteError('');
    try {
      const loaded = await loadGovernedVaultBridgeRoute(rpc, {
        assetId: utils.PFUSDC_ASSET_ID,
        chainId: utils.CHAIN_ID,
        genesisHash: utils.GENESIS_HASH,
        sourceChainId: ARBITRUM_CHAIN_ID,
        tokenAddress: ARBITRUM_USDC,
      });
      setGovernedRoute(loaded);
      setRouteStatus('ready');
      if (expectedProfileHash && loaded.profileHash !== expectedProfileHash) {
        throw new Error('The governed bridge route changed. Review the new route before signing.');
      }
      return loaded;
    } catch (routeFailure) {
      const message = routeFailure?.message || 'Governed bridge route discovery failed';
      setRouteError(message);
      setRouteStatus('error');
      throw routeFailure;
    }
  }, [rpc]);

  useEffect(() => {
    if (!rpc) {
      setRouteStatus('error');
      setRouteError('Wallet RPC is not connected.');
      return;
    }
    refreshGovernedRoute().catch(() => {});
  }, [rpc, refreshGovernedRoute]);

  const vaultContract = governedRoute?.vaultAddress || '';

  const metaMaskAvailable = evm.hasMetaMask();
  const amountAtoms = useMemo(() => safeAtoms(amount), [amount]);
  const bridgeHelpersReady = typeof evm.generateNonce === 'function'
    && typeof evm.watchDepositEvent === 'function'
    && typeof evm.governedRouteBinding === 'function';
  const cctpReady = Boolean(cctpModules['../lib/cctp.js']);
  const hasEthUsdc = ethBalanceAtoms > 0n;
  const hasArbUsdc = arbBalanceAtoms > 0n;
  const hasRequestedAmount = Boolean(amountAtoms && amountAtoms > 0n);
  const hasRequestedArbUsdc = Boolean(hasRequestedAmount && amountAtoms <= arbBalanceAtoms);
  const hasArbUsdcForCurrentAmount = hasRequestedAmount ? hasRequestedArbUsdc : hasArbUsdc;
  const hasArbGas = arbEthBalanceWei > 0n;
  const needsArbitrumGas = Boolean(connectedAddress && !hasArbGas);

  const needsL1Bridge = Boolean(
    connectedAddress
    && amountAtoms
    && amountAtoms > 0n
    && amountAtoms > arbBalanceAtoms
    && ethBalanceAtoms >= amountAtoms
  );

  const amountError = useMemo(() => {
    if (!amount) return '';
    if (amountAtoms === null || amountAtoms <= 0n) return 'Enter an amount greater than 0 USDC.';
    if (amountAtoms <= arbBalanceAtoms || ethBalanceAtoms >= amountAtoms) return '';
    return `Insufficient USDC. Ethereum: ${balanceLabel(ethBalanceAtoms)} | Arbitrum: ${balanceLabel(arbBalanceAtoms)}.`;
  }, [amount, amountAtoms, arbBalanceAtoms, ethBalanceAtoms]);
  const bridgeAmountError = amountAtoms && amountAtoms > ethBalanceAtoms
    ? `Amount exceeds Ethereum USDC balance (${balanceLabel(ethBalanceAtoms)}).`
    : '';
  const vaultAmountError = amountAtoms && amountAtoms > arbBalanceAtoms
    ? `Amount exceeds Arbitrum USDC balance (${balanceLabel(arbBalanceAtoms)}).`
    : '';
  const bridgeDisabledReason = !amountAtoms || amountAtoms <= 0n
    ? 'Enter an amount first'
    : bridgeAmountError
      ? bridgeAmountError
      : !hasArbGas
        ? 'Bridge ETH for Arbitrum gas first'
        : !cctpReady
          ? 'Circle CCTP bridge module is not available yet'
          : '';
  const vaultDisabledReason = !amountAtoms || amountAtoms <= 0n
    ? 'Enter an amount first'
    : vaultAmountError
      ? vaultAmountError
      : !vaultContract
        ? 'Bridge vault is not configured'
        : !hasArbGas
          ? 'Bridge ETH for Arbitrum gas first'
          : '';
  const canStartBridge = Boolean(!bridgeDisabledReason && phase !== 'bridging');
  const canStartVaultApproval = Boolean(!vaultDisabledReason && phase !== 'approving' && phase !== 'depositing');
  const readyPendingBurn = pendingBurns.find(isAttestationReady);
  const pendingBurnsNeedAttestation = pendingBurns.some(burn => pendingStatusKind(burn) === 'pending');

  const refreshBalances = useCallback(async (evmAddress = connectedAddress, options = {}) => {
    if (!evmAddress) return;
    const silent = options?.silent === true;
    setBalanceStatus('loading');
    setPhase(current => (current === 'disconnected' || current === 'connected' || current === 'need-bridge' || current === 'error' ? 'detecting' : current));
    setError('');
    try {
      const chainId = await readCurrentChainId();
      setCurrentChainId(chainId);
      const [ethBalance, arbBalance, arbEthBalance] = await Promise.all([
        readEthereumUsdcBalance(evmAddress, chainId),
        readArbitrumUsdcBalance(evmAddress),
        typeof evm.getArbitrumEthBalance === 'function'
          ? evm.getArbitrumEthBalance(evmAddress)
          : 0n,
      ]);
      setEthBalanceAtoms(ethBalance);
      setArbBalanceAtoms(arbBalance);
      setArbEthBalanceWei(arbEthBalance);
      setBalanceStatus('ok');
      setPhase(current => (current === 'detecting' ? 'connected' : current));
    } catch (e) {
      setBalanceStatus('error');
      if (!silent) {
        setPhase('error');
        setError('Could not read USDC balances: ' + (e.message || 'unknown error'));
      }
    }
  }, [connectedAddress]);

  const rememberBurnTx = useCallback((txHash) => {
    const normalizedTxHash = normalizeTxHash(txHash);
    if (!normalizedTxHash) return;
    setBridgeTx(normalizedTxHash);
    setResumeBurnTx(normalizedTxHash);
    storeBurnTx(normalizedTxHash);
  }, []);

  const applyDetectedPendingBurns = useCallback((burns) => {
    const nextBurns = Array.isArray(burns) ? burns.filter(burn => isTxHash(cctpBurnTx(burn))) : [];
    setPendingBurns(nextBurns);
    if (!nextBurns.length) return;

    const mostRecentTx = cctpBurnTx(nextBurns[0]);
    if (mostRecentTx) {
      setResumeBurnTx(mostRecentTx);
      storeBurnTx(mostRecentTx);
      setBridgeTx(mostRecentTx);
    }

    const actionableBurn = nextBurns.find(isAttestationReady) || nextBurns[0];
    const kind = pendingStatusKind(actionableBurn);
    if (kind === 'ready') {
      setResumeStatus('ready');
      setResumeMessage('Attestation ready! Mint on Arbitrum.');
    } else if (kind === 'pending') {
      setResumeStatus('pending');
      setResumeMessage('Waiting for Circle attestation...');
    }
    if (actionableBurn?.messageHash) setBridgeMessageHash(actionableBurn.messageHash);
  }, []);

  const detectPendingBurns = useCallback(async () => {
    if (!connectedAddress || !cctpReady) return null;
    const { detectPendingCctpBurns } = await loadCctpBridge();
    if (typeof detectPendingCctpBurns !== 'function') return null;
    return detectPendingCctpBurns(connectedAddress);
  }, [cctpReady, connectedAddress]);

  const handleCheckResumeStatus = useCallback(async ({ silent = false } = {}) => {
    const burnTxHash = String(resumeBurnTx || bridgeTx || '').trim();
    if (!isTxHash(burnTxHash)) {
      setResumeStatus('error');
      setResumeMessage('Enter a valid Ethereum burn tx hash.');
      return;
    }
    if (!silent) {
      setResumeStatus('checking');
      setResumeMessage('Checking Circle burn status...');
    }
    try {
      const { getPendingCctpStatus } = await loadCctpBridge();
      if (typeof getPendingCctpStatus !== 'function') {
        throw new Error('CCTP status helper is unavailable. Reload after the CCTP module update lands.');
      }
      const status = await getPendingCctpStatus(burnTxHash);
      const kind = pendingStatusKind(status);
      if (kind === 'ready') {
        setResumeStatus('ready');
        setResumeMessage('Attestation ready! Mint on Arbitrum.');
        if (status?.messageHash) setBridgeMessageHash(status.messageHash);
      } else if (kind === 'pending') {
        setResumeStatus('pending');
        setResumeMessage('Attestation pending. Polling every 15s...');
        if (status?.messageHash) setBridgeMessageHash(status.messageHash);
      } else {
        setResumeStatus('not-found');
        setResumeMessage('No CCTP burn found for this tx hash.');
      }
    } catch (e) {
      setResumeStatus('error');
      setResumeMessage(e.message || 'Could not check CCTP status.');
    }
  }, [bridgeTx, resumeBurnTx]);

  const handleResumeMint = async (burnTxOverride = '') => {
    const explicitBurnTx = typeof burnTxOverride === 'string' ? burnTxOverride : '';
    const burnTxHash = normalizeTxHash(explicitBurnTx || resumeBurnTx || bridgeTx);
    if (!connectedAddress) {
      setResumeStatus('error');
      setResumeMessage('Connect MetaMask before resuming the bridge.');
      return;
    }
    if (!isTxHash(burnTxHash)) {
      setResumeStatus('error');
      setResumeMessage('Enter a valid Ethereum burn tx hash.');
      return;
    }
    try {
      setError('');
      setResumeStatus('minting');
      setResumeMessage('Fetching burn receipt...');
      setBridgeStep('Fetching burn receipt...');
      setBridgeStage('resume');
      setBridgeStartedAt(Date.now());
      setBridgeElapsedMs(0);
      setManualMintAvailable(false);
      rememberBurnTx(burnTxHash);
      const { resumeCctpBridge } = await loadCctpBridge();
      if (typeof resumeCctpBridge !== 'function') {
        throw new Error('CCTP resume helper is unavailable. Reload after the CCTP module update lands.');
      }
      const result = await resumeCctpBridge({
        burnTxHash,
        fromAddress: connectedAddress,
        onUpdate: (step, data = {}) => {
          const label = resumeStepLabel(step, data);
          setResumeMessage(label);
          setBridgeStep(label);
          if (step === 'fetching-attestation' || step === 'attestation_pending') setBridgeStage('attesting');
          if (step === 'attestation_complete') setBridgeStage('ready');
          if (step === 'minting' || step === 'mint_start' || step === 'mint_submitted') setBridgeStage('minting');
          if (step === 'done') setBridgeStage('done');
          if (data.messageHash) setBridgeMessageHash(data.messageHash);
          if (data.txHash && (step === 'minting' || step === 'mint_start' || step === 'mint_submitted')) {
            setBridgeMintTx(data.txHash);
          }
          if (data.mintTxHash) setBridgeMintTx(data.mintTxHash);
        },
      });
      if (result?.messageHash) setBridgeMessageHash(result.messageHash);
      if (result?.mintTxHash) setBridgeMintTx(result.mintTxHash);
      setResumeStatus('done');
      setResumeMessage('Done! USDC minted.');
      setBridgeStage('done');
      setBridgeStep('Bridge complete! USDC minted on Arbitrum.');
      setManualMintAvailable(false);
      setPendingBurns(currentBurns => currentBurns.filter(
        burn => cctpBurnTx(burn).toLowerCase() !== burnTxHash.toLowerCase(),
      ));
      await refreshBalances(connectedAddress);
      setPhase('connected');
    } catch (e) {
      setResumeStatus('error');
      setResumeMessage('Resume failed: ' + (e.message || 'unknown error'));
      setBridgeStage('ready');
      setManualMintAvailable(true);
    }
  };

  useEffect(() => {
    if (!connectedAddress || phase === 'detecting' || phase === 'bridging' || phase === 'approving' || phase === 'depositing' || phase === 'deposited' || phase === 'relaying' || phase === 'complete' || phase === 'error') {
      return;
    }
    if (phase === 'approved' && approvedAtoms !== null && amountAtoms !== null && approvedAtoms !== amountAtoms) {
      setApprovedAtoms(null);
      setApprovalTx('');
      setPhase(needsArbitrumGas ? 'need-gas' : needsL1Bridge ? 'need-bridge' : 'connected');
      return;
    }
    if (needsArbitrumGas && phase !== 'need-gas') {
      setPhase('need-gas');
    } else if (!needsArbitrumGas && phase === 'need-gas') {
      setPhase(needsL1Bridge ? 'need-bridge' : 'connected');
    } else if (needsL1Bridge && phase !== 'need-bridge') {
      setPhase('need-bridge');
    } else if (!needsL1Bridge && phase === 'need-bridge') {
      setPhase('connected');
    }
  }, [amountAtoms, approvedAtoms, connectedAddress, needsArbitrumGas, needsL1Bridge, phase]);

  useEffect(() => {
    if (bridgeTx && !resumeBurnTx) setResumeBurnTx(bridgeTx);
  }, [bridgeTx, resumeBurnTx]);

  useEffect(() => {
    if (!connectedAddress || !cctpReady) return undefined;
    let cancelled = false;
    (async () => {
      try {
        const burns = await detectPendingBurns();
        if (!cancelled && burns) applyDetectedPendingBurns(burns);
      } catch (_) {
        // Auto-detection is opportunistic; manual resume remains available.
      }
    })();
    return () => { cancelled = true; };
  }, [applyDetectedPendingBurns, cctpReady, connectedAddress, detectPendingBurns]);

  useEffect(() => {
    if (!connectedAddress || !cctpReady || !pendingBurnsNeedAttestation) return undefined;
    const timer = window.setInterval(async () => {
      try {
        const burns = await detectPendingBurns();
        if (burns) applyDetectedPendingBurns(burns);
      } catch (_) {
        // Keep the last known pending state visible if a refresh fails.
      }
    }, 15000);
    return () => window.clearInterval(timer);
  }, [applyDetectedPendingBurns, cctpReady, connectedAddress, detectPendingBurns, pendingBurnsNeedAttestation]);

  useEffect(() => {
    if (pendingBurns.length > 0 || resumeStatus !== 'pending' || !isTxHash(resumeBurnTx)) return undefined;
    const timer = window.setInterval(() => {
      handleCheckResumeStatus({ silent: true });
    }, 15000);
    return () => window.clearInterval(timer);
  }, [handleCheckResumeStatus, pendingBurns.length, resumeBurnTx, resumeStatus]);

  useEffect(() => {
    if (!bridgeStartedAt || (phase !== 'bridging' && resumeStatus !== 'minting')) return undefined;
    const updateElapsed = () => setBridgeElapsedMs(Date.now() - bridgeStartedAt);
    updateElapsed();
    const timer = window.setInterval(updateElapsed, 1000);
    return () => window.clearInterval(timer);
  }, [bridgeStartedAt, phase, resumeStatus]);

  const handleConnect = async () => {
    setError('');
    if (!metaMaskAvailable) {
      setPhase('error');
      setError('MetaMask is not available in this browser. Install or enable MetaMask, then try again.');
      return;
    }
    try {
      const evmAddress = await evm.connectMetaMask();
      setConnectedAddress(evmAddress);
      setPhase('detecting');
      await refreshBalances(evmAddress);
    } catch (e) {
      setPhase('error');
      setError(e.message || 'MetaMask connection failed.');
    }
  };

  const handleSwitchToArbitrum = async () => {
    setError('');
    try {
      await evm.ensureArbitrum();
      const chainId = await readCurrentChainId();
      setCurrentChainId(chainId);
      await refreshArbitrumGasBalance();
    } catch (e) {
      setError('Could not switch to Arbitrum: ' + humanEvmError(e));
    }
  };

  const handleBridgeToArbitrum = async () => {
    setError('');
    if (!connectedAddress || !amountAtoms || amountAtoms <= 0n) {
      setError('Enter a valid USDC amount before bridging.');
      return;
    }
    if (!hasArbGas) {
      setError('Add a small amount of ETH on Arbitrum for gas before bridging USDC. Circle burns on Ethereum, but the final Arbitrum mint still needs gas.');
      return;
    }
    let activeBurnTx = '';
    const rememberActiveBurn = (txHash) => {
      const normalizedTxHash = normalizeTxHash(txHash);
      if (!normalizedTxHash) return;
      activeBurnTx = normalizedTxHash;
      rememberBurnTx(normalizedTxHash);
    };
    const updateBridgeProgress = (step, data = {}) => {
      const approvalTxHash = data.approvalTxHash || data.approveTxHash || data.txHash;
      const burnTxHash = data.burnTxHash || data.burn_tx_hash || data.txHash;
      const mintTxHash = data.mintTxHash || data.mint_tx_hash || data.txHash;

      if (step === 'approving' || step === 'approve_start') {
        if (isTxHash(approvalTxHash)) setBridgeApprovalTx(normalizeTxHash(approvalTxHash));
        setBridgeStep('Step 1: Approving USDC to Circle TokenMessenger V2...');
        setBridgeStage('approving');
      } else if (step === 'approve_submitted' || step === 'approve_confirmed') {
        if (isTxHash(approvalTxHash)) setBridgeApprovalTx(normalizeTxHash(approvalTxHash));
        setBridgeStep('Step 1: USDC approval confirmed for Fast Transfer.');
        setBridgeStage('approving');
      } else if (step === 'burning' || step === 'burn_start') {
        if (isTxHash(burnTxHash)) rememberActiveBurn(burnTxHash);
        setBridgeStep('Step 2: Burning USDC on Ethereum (Fast Transfer)...');
        setBridgeStage('burning');
      } else if (step === 'burn_submitted' || step === 'burn_confirmed') {
        if (isTxHash(burnTxHash)) rememberActiveBurn(burnTxHash);
        setBridgeStep(step === 'burn_confirmed'
          ? 'Step 2: Burn confirmed. Waiting for Circle attestation in seconds.'
          : `Fast Transfer burn submitted... ${isTxHash(burnTxHash) ? compactHash(normalizeTxHash(burnTxHash), 8) : ''}`);
        setBridgeStage(step === 'burn_confirmed' ? 'attesting' : 'burning');
      } else if (step === 'attesting' || step === 'message_sent' || step === 'attestation_pending') {
        if (isTxHash(data.burnTxHash)) rememberActiveBurn(data.burnTxHash);
        if (data.messageHash) setBridgeMessageHash(data.messageHash);
        setBridgeStep(`Step 3: Waiting for Circle attestation (seconds)... ${data.messageHash ? compactHash(data.messageHash, 8) : ''}`);
        setBridgeStage('attesting');
      } else if (step === 'attestation_complete') {
        if (data.messageHash) setBridgeMessageHash(data.messageHash);
        setBridgeStep('Step 3: Attestation ready. Auto-minting on Arbitrum...');
        setBridgeStage('ready');
      } else if (step === 'minting' || step === 'mint_start' || step === 'mint_submitted') {
        if (isTxHash(mintTxHash)) setBridgeMintTx(normalizeTxHash(mintTxHash));
        setBridgeStep(`Step 4: Minting USDC on Arbitrum... ${isTxHash(mintTxHash) ? compactHash(normalizeTxHash(mintTxHash), 8) : ''}`);
        setBridgeStage('minting');
      } else if (step === 'done') {
        const doneMintTxHash = data.mintTxHash || data.txHash;
        if (isTxHash(doneMintTxHash)) setBridgeMintTx(normalizeTxHash(doneMintTxHash));
        setBridgeStage('minting');
        setBridgeStep(`Step 5: Mint confirmed on Arbitrum. Preparing the vault deposit...`);
      }
    };
    const runBridge = (bridgeFn) => {
      return bridgeFn({
        amount: amountAtoms,
        fromAddress: connectedAddress,
        onUpdate: updateBridgeProgress,
      });
    };
    try {
      setPhase('bridging');
      setBridgeStep('Preparing Circle CCTP Fast Transfer...');
      setBridgeStage('approving');
      setBridgeStartedAt(Date.now());
      setBridgeElapsedMs(0);
      setManualMintAvailable(false);
      setBridgeApprovalTx('');
      setBridgeTx('');
      setBridgeMintTx('');
      setBridgeMessageHash('');
      await ensureChain(ETH_MAINNET_CHAIN_ID);
      const cctp = await loadCctpBridge();
      const result = await runBridge(cctp.cctpBridgeUsdcV2);

      const resultApprovalTx = result?.approvalTxHash || result?.approveTxHash;
      if (isTxHash(resultApprovalTx)) setBridgeApprovalTx(normalizeTxHash(resultApprovalTx));
      if (result?.burnTxHash) rememberActiveBurn(result.burnTxHash);
      if (result?.messageHash) setBridgeMessageHash(result.messageHash);
      if (result?.mintTxHash) {
        const normalizedMintTxHash = normalizeTxHash(result.mintTxHash);
        setBridgeMintTx(normalizedMintTxHash);
        setBridgeStage('minting');
        setBridgeStep(`Step 4: Minting USDC on Arbitrum... ${compactHash(normalizedMintTxHash, 8)}`);
        await waitForTransactionReceipt(normalizedMintTxHash);
      }
      setBridgeStage('done');
      setBridgeStep(`Step 5: Bridge complete! ${balanceLabel(amountAtoms)} now on Arbitrum.`);
      setArbBalanceAtoms(current => current + amountAtoms);
      setEthBalanceAtoms(current => current > amountAtoms ? current - amountAtoms : 0n);
      setPhase('connected');
      await evm.ensureArbitrum();
      refreshBalances(connectedAddress, { silent: true });
    } catch (e) {
      setPhase('error');
      if (isTxHash(activeBurnTx)) {
        setResumeBurnTx(activeBurnTx);
        setResumeStatus('ready');
        setResumeMessage('Auto-mint failed. Retry minting on Arbitrum.');
        setBridgeStage('ready');
        setManualMintAvailable(true);
      }
      setError('Circle CCTP Fast Transfer failed: ' + (e.message || 'unknown error') + (activeBurnTx ? ' Use Mint on Arbitrum to retry the final mint.' : ''));
    }
  };

  const refreshArbitrumGasBalance = async () => {
    if (!connectedAddress || typeof evm.getArbitrumEthBalance !== 'function') return arbEthBalanceWei;
    const balance = await evm.getArbitrumEthBalance(connectedAddress);
    setArbEthBalanceWei(balance);
    return balance;
  };

  const assertAffordableArbitrumFee = (fee, balanceWei, label) => {
    if (!fee || fee.maxCostWei <= 0n) return;
    if (balanceWei >= fee.maxCostWei) return;
    throw new Error(
      `${label} needs about ${ethLabel(fee.maxCostWei)} for Arbitrum gas; wallet has ${ethLabel(balanceWei)}.`,
    );
  };

  const handleApprove = async () => {
    setError('');
    if (!vaultContract) {
      setPhase('error');
      setError(routeError || 'No active governed bridge route is available.');
      return;
    }
    if (!amountAtoms || amountAtoms <= 0n || amountError || amountAtoms > arbBalanceAtoms) {
      setError(amountError || 'Bridge enough USDC to Arbitrum before approving the vault deposit.');
      return;
    }
    if (!hasArbGas) {
      setError('Add a small amount of ETH on Arbitrum for gas before approving the vault deposit.');
      return;
    }
    try {
      setPhase('approving');
      const activeRoute = await refreshGovernedRoute({
        expectedProfileHash: governedRoute?.profileHash || '',
      });
      const activeVault = activeRoute.vaultAddress;
      await evm.ensureArbitrum();
      await Promise.all([
        evm.assertContractCodeHash(activeVault, activeRoute.vaultRuntimeCodeHash),
        evm.assertContractCodeHash(activeRoute.tokenAddress, activeRoute.tokenRuntimeCodeHash),
      ]);
      const gasBalanceWei = await refreshArbitrumGasBalance();
      if (gasBalanceWei <= 0n) {
        throw new Error('Add a small amount of ETH on Arbitrum for gas before approving the vault deposit.');
      }
      try {
        const fee = await evm.estimateApproveUsdcFee(activeVault, amountAtoms, connectedAddress);
        assertAffordableArbitrumFee(fee, gasBalanceWei, 'USDC approval');
      } catch (preflightError) {
        throw new Error('USDC approval preflight failed: ' + humanEvmError(preflightError));
      }
      const txHash = await evm.approveUsdc(activeVault, amountAtoms);
      setApprovalTx(txHash);
      await waitForTransactionReceipt(txHash);
      if (typeof evm.getArbitrumUsdcAllowance === 'function') {
        const allowance = await evm.getArbitrumUsdcAllowance(connectedAddress, activeVault);
        if (allowance < amountAtoms) {
          throw new Error('USDC approval confirmed, but vault allowance is still below the deposit amount. Approve Arbitrum USDC again.');
        }
      }
      setApprovedAtoms(amountAtoms);
      setPhase('approved');
    } catch (e) {
      setPhase('error');
      setError('USDC approval failed: ' + (e.message || 'unknown error'));
    }
  };

  const handleRelayDeposit = async ({
    txHash = depositTx,
    eventDepositId = depositId,
    atoms = amountAtoms,
    route = governedRoute,
    routeBinding = '',
  } = {}) => {
    const normalizedTxHash = normalizeTxHash(txHash);
    if (!normalizedTxHash) {
      throw new Error('Vault deposit transaction hash is missing.');
    }
    if (!address) {
      throw new Error('PFTL recipient address is unavailable. Unlock the wallet before relaying.');
    }
    setPhase('relaying');
    setRelayStatus('running');
    setRelayMessage('Submitting PFTL relay: propose, attest, finalize, claim.');
    setRelayResult(null);
    setRelayTxs([]);
    const idempotencyKey = `vault-relay:${normalizedTxHash.toLowerCase()}`;
    const confirmedRouteBinding = routeBinding || evm.governedRouteBinding(
      route?.profileHash || '',
      route?.routeEpoch || 0,
    );
    const result = await relayVaultDeposit({
      depositTxHash: normalizedTxHash,
      depositId: eventDepositId || '',
      pftlRecipient: address,
      depositor: connectedAddress,
      amountAtoms: atoms ? atoms.toString() : '',
      idempotencyKey,
      routeProfileHash: route?.profileHash || '',
      routeEpoch: route?.routeEpoch || 0,
      routeBinding: confirmedRouteBinding,
    });
    const submitted = Array.isArray(result.submitted) ? result.submitted : [];
    setRelayResult(result);
    setRelayTxs(submitted);
    setRelayStatus('done');
    setRelayMessage('Relay complete. pfUSDC is now in the PFTL wallet.');
    if (result.after_balance_atoms !== undefined && result.after_balance_atoms !== null) {
      try {
        setPfusdcBalanceAtoms(BigInt(result.after_balance_atoms));
      } catch (_) {
        setPfusdcBalanceAtoms(null);
      }
    }
    setPhase('complete');
    return result;
  };

  const handleDeposit = async () => {
    setError('');
    let confirmedDepositTx = '';
    if (!vaultContract) {
      setPhase('error');
      setError(routeError || 'No active governed bridge route is available.');
      return;
    }
    if (!address) {
      setPhase('error');
      setError('PFTL recipient address is unavailable. Unlock the wallet before depositing.');
      return;
    }
    if (!bridgeHelpersReady) {
      setPhase('error');
      setError('Bridge deposit helpers are not available yet. Reload after the EVM bridge module update lands.');
      return;
    }
    if (!amountAtoms || amountAtoms <= 0n || amountError || amountAtoms > arbBalanceAtoms) {
      setError(amountError || 'Bridge enough USDC to Arbitrum before depositing.');
      return;
    }
    if (!hasArbGas) {
      setError('Add a small amount of ETH on Arbitrum for gas before depositing to the bridge vault.');
      return;
    }
    try {
      setPhase('depositing');
      const activeRoute = await refreshGovernedRoute({
        expectedProfileHash: governedRoute?.profileHash || '',
      });
      const activeVault = activeRoute.vaultAddress;
      const routeBinding = evm.governedRouteBinding(
        activeRoute.profileHash,
        activeRoute.routeEpoch,
      );
      await evm.ensureArbitrum();
      await Promise.all([
        evm.assertContractCodeHash(activeVault, activeRoute.vaultRuntimeCodeHash),
        evm.assertContractCodeHash(activeRoute.tokenAddress, activeRoute.tokenRuntimeCodeHash),
      ]);
      const nonce = evm.generateNonce();
      const gasBalanceWei = await refreshArbitrumGasBalance();
      if (gasBalanceWei <= 0n) {
        throw new Error('Add a small amount of ETH on Arbitrum for gas before depositing to the bridge vault.');
      }
      if (typeof evm.getArbitrumUsdcAllowance === 'function') {
        const allowance = await evm.getArbitrumUsdcAllowance(connectedAddress, activeVault);
        if (allowance < amountAtoms) {
          setApprovedAtoms(null);
          setApprovalTx('');
          setPhase('connected');
          throw new Error(`Approve Arbitrum USDC for the vault before depositing. Current allowance is ${balanceLabel(allowance)}; deposit needs ${balanceLabel(amountAtoms)}.`);
        }
      }
      try {
        const fee = await evm.estimateBridgeDepositFee(
          activeVault,
          amountAtoms,
          address,
          nonce,
          routeBinding,
          connectedAddress,
        );
        assertAffordableArbitrumFee(fee, gasBalanceWei, 'Bridge vault deposit');
      } catch (preflightError) {
        throw new Error('Bridge vault deposit preflight failed: ' + humanEvmError(preflightError));
      }
      const txHash = await evm.depositToBridge(
        activeVault,
        amountAtoms,
        address,
        nonce,
        routeBinding,
      );
      confirmedDepositTx = txHash;
      setDepositTx(txHash);
      await waitForTransactionReceipt(txHash);
      const event = await evm.watchDepositEvent(activeVault, txHash, routeBinding);
      const eventDepositId = event?.deposit_id || event?.depositId || event?.id || '';
      setDepositId(eventDepositId);
      setPhase('deposited');
      await refreshBalances(connectedAddress);
      await handleRelayDeposit({
        txHash,
        eventDepositId,
        atoms: amountAtoms,
        route: activeRoute,
        routeBinding,
      });
    } catch (e) {
      setPhase('error');
      const prefix = confirmedDepositTx || depositTx || phase === 'relaying' || relayStatus === 'running'
        ? 'Vault deposit confirmed, but relay failed: '
        : 'Vault deposit failed: ';
      setRelayStatus(current => (current === 'running' ? 'error' : current));
      setRelayMessage(current => current || (e.message || 'unknown error'));
      setError(prefix + (e.message || 'unknown error'));
    }
  };

  const resetFlow = () => {
    setPhase(connectedAddress ? 'connected' : 'disconnected');
    setApprovedAtoms(null);
    setApprovalTx('');
    setBridgeApprovalTx('');
    setBridgeTx('');
    setBridgeMintTx('');
    setBridgeMessageHash('');
    setBridgeStep('');
    setBridgeStartedAt(null);
    setBridgeElapsedMs(0);
    setBridgeStage('idle');
    setManualMintAvailable(false);
    setDepositTx('');
    setDepositId('');
    setRelayStatus('idle');
    setRelayMessage('');
    setRelayResult(null);
    setRelayTxs([]);
    setPfusdcBalanceAtoms(null);
    setError('');
  };

  const setMaxBridgeAmount = () => {
    setAmount(trimUsdc(evm.atomsToUsdc(ethBalanceAtoms)));
  };

  const setMaxDepositAmount = () => {
    setAmount(trimUsdc(evm.atomsToUsdc(arbBalanceAtoms)));
  };

  const setMaxAmount = () => {
    if (hasArbUsdc) setMaxDepositAmount();
    else setMaxBridgeAmount();
  };

  const bridgeComplete = Boolean(bridgeStage === 'done' || bridgeStep.startsWith('Bridge complete!'));
  const showBridgeProgress = Boolean(
    phase === 'bridging'
    || resumeStatus === 'minting'
    || bridgeTx
    || bridgeMessageHash
    || bridgeMintTx
    || manualMintAvailable
  );
  const bridgeElapsed = bridgeStartedAt ? elapsedLabel(bridgeElapsedMs) : '';
  const flowStepId = !connectedAddress
    ? 0
    : phase === 'deposited' || phase === 'relaying' || phase === 'complete'
      ? 5
      : !hasArbGas
        ? 1
        : phase === 'bridging' || needsL1Bridge
          ? 2
        : phase === 'depositing' || phase === 'approved'
          ? 4
          : phase === 'approving' || hasArbUsdcForCurrentAmount
            ? 3
            : hasEthUsdc
              ? 2
              : 1;
  const hasBridgeActivity = Boolean(bridgeApprovalTx || bridgeTx || bridgeMessageHash || bridgeMintTx || approvalTx || depositTx || depositId);
  const primaryBusy = phase === 'bridging' || phase === 'approving' || phase === 'depositing' || phase === 'relaying' || resumeStatus === 'minting';
  const amountReady = Boolean(amountAtoms && amountAtoms > 0n && !amountError);
  const balanceLoading = balanceStatus === 'loading';
  const locationTitle = !connectedAddress
    ? 'Connect MetaMask to locate your USDC'
    : phase === 'complete'
      ? 'pfUSDC is in your PFTL wallet'
      : phase === 'relaying'
        ? 'USDC is being relayed to pfUSDC'
        : phase === 'deposited' || depositTx
      ? 'USDC is deposited to the PFTL vault'
      : needsL1Bridge
        ? 'USDC needs to move from Ethereum to Arbitrum'
      : arbBalanceAtoms > 0n
        ? 'Your bridged USDC is on Arbitrum'
        : bridgeTx && !bridgeMintTx
          ? 'USDC was burned on Ethereum; mint may need resume'
          : ethBalanceAtoms > 0n
            ? 'USDC is still on Ethereum'
            : 'No USDC detected in this MetaMask account';
  const locationBody = !connectedAddress
    ? 'This page cannot see your Ethereum or Arbitrum balances until MetaMask connects.'
    : phase === 'complete'
      ? `${pfusdcBalanceAtoms !== null ? `${trimUsdc(evm.atomsToUsdc(pfusdcBalanceAtoms))} pfUSDC` : 'pfUSDC'} is now visible in this PFTL wallet.`
      : phase === 'relaying'
        ? 'The backend is submitting the PFTL relay legs now: propose, attest, finalize, and claim.'
        : phase === 'deposited' || depositTx
      ? 'The Arbitrum vault deposit is confirmed. The wallet is relaying it to PFTL so pfUSDC appears here.'
      : needsL1Bridge
        ? `You have ${balanceLabel(arbBalanceAtoms)} on Arbitrum and ${balanceLabel(ethBalanceAtoms)} on Ethereum. The requested ${balanceLabel(amountAtoms)} deposit is larger than the Arbitrum balance, so the next action bridges USDC with Circle CCTP.`
      : arbBalanceAtoms > 0n
        ? `${balanceLabel(arbBalanceAtoms)} is native Arbitrum USDC in your MetaMask account. It is not pfUSDC yet. Approve and deposit it to the PFTL vault next.`
        : bridgeTx && !bridgeMintTx
          ? 'Open resume, paste the Ethereum burn tx if needed, and mint the CCTP message on Arbitrum.'
          : ethBalanceAtoms > 0n
            ? `${balanceLabel(ethBalanceAtoms)} is still on Ethereum. Bridge it with Circle CCTP to move it to Arbitrum.`
            : 'Use the same MetaMask account that submitted the bridge transactions, then refresh balances.';

  const stepState = (stepId) => {
    if (!connectedAddress) return 'pending';
    if (stepId === 1) return hasArbGas ? 'done' : flowStepId === 1 ? 'active' : 'pending';
    if (stepId === 2) return hasArbUsdcForCurrentAmount || bridgeComplete ? 'done' : flowStepId === 2 ? 'active' : 'pending';
    if (stepId === 3) return phase === 'approved' || phase === 'depositing' || phase === 'deposited' || phase === 'relaying' || phase === 'complete' ? 'done' : flowStepId === 3 ? 'active' : 'pending';
    if (stepId === 4) return phase === 'deposited' || phase === 'relaying' || phase === 'complete' || depositTx ? 'done' : flowStepId === 4 ? 'active' : 'pending';
    if (stepId === 5) return phase === 'complete' ? 'done' : phase === 'deposited' || phase === 'relaying' ? 'active' : 'pending';
    return 'pending';
  };

  const renderAmountInput = (label, maxKind) => (
    <label className="pfb-field">
      <span>{label}</span>
      <div className="pfb-amount">
        <input
          value={amount}
          onChange={e => setAmount(normalizeAmountInput(e.target.value))}
          placeholder="0.00"
          inputMode="decimal"
          aria-invalid={Boolean(amountError)}
        />
        <span>USDC</span>
        <button
          type="button"
          className="pfb-secondary small"
          onClick={maxKind === 'arbitrum' ? setMaxDepositAmount : setMaxBridgeAmount}
          disabled={maxKind === 'arbitrum' ? arbBalanceAtoms <= 0n : ethBalanceAtoms <= 0n}
        >
          Max
        </button>
      </div>
      {amountError && <strong className="pfb-inline-error">{amountError}</strong>}
    </label>
  );

  const renderActionCard = () => {
    if (!connectedAddress) {
      return (
        <>
          <div className="pfb-card-head">
            <ArrowLeftRight size={18} />
            <span>Start</span>
          </div>
          <h2>Connect MetaMask</h2>
          <p>The wallet will read Ethereum USDC, Arbitrum USDC, and Arbitrum ETH gas for the connected account.</p>
          <button className="pfb-primary" onClick={handleConnect}>
            Connect MetaMask <ArrowRight size={16} />
          </button>
        </>
      );
    }

    if (flowStepId === 1) {
      return (
        <>
          <div className="pfb-card-head">
            <Fuel size={18} />
            <span>Step 1 of 5</span>
          </div>
          <h2>Add Arbitrum ETH gas</h2>
          <p>Approvals, vault deposit, and CCTP mint retries all require a small amount of ETH on Arbitrum.</p>
          <div className="pfb-readout">
            <span>Arbitrum ETH</span>
            <strong>{balanceLoading ? '...' : ethLabel(arbEthBalanceWei)}</strong>
          </div>
          <a className="pfb-primary" href={ARBITRUM_ETH_BRIDGE_URL} target="_blank" rel="noreferrer">
            Bridge ETH for gas <ArrowRight size={16} />
          </a>
          <div className="pfb-inline-actions">
            <button className="pfb-secondary" type="button" onClick={handleSwitchToArbitrum}>Switch to Arbitrum</button>
            <button className="pfb-secondary" type="button" onClick={() => refreshArbitrumGasBalance()}>Refresh gas</button>
          </div>
        </>
      );
    }

    if (flowStepId === 2) {
      return (
        <>
          <div className="pfb-card-head">
            <ArrowLeftRight size={18} />
            <span>Step 2 of 5</span>
          </div>
          <h2>Bridge USDC to Arbitrum</h2>
          <p>Circle CCTP burns USDC on Ethereum and mints native USDC on Arbitrum. This route does not use LI.FI.</p>
          <div className="pfb-readout">
            <span>Ethereum USDC</span>
            <strong>{balanceLoading ? '...' : balanceLabel(ethBalanceAtoms)}</strong>
          </div>
          {renderAmountInput('Amount to bridge', 'ethereum')}
          <button
            className="pfb-primary"
            onClick={handleBridgeToArbitrum}
            disabled={primaryBusy || !amountReady || Boolean(bridgeDisabledReason)}
            title={bridgeDisabledReason}
          >
            {phase === 'bridging' ? <><Loader2 size={16} className="pfb-spin" /> Bridging...</> : <>Bridge via Circle CCTP <ArrowRight size={16} /></>}
          </button>
        </>
      );
    }

    if (flowStepId === 3) {
      return (
        <>
          <div className="pfb-card-head">
            <ShieldCheck size={18} />
            <span>Step 3 of 5</span>
          </div>
          <h2>Approve the vault</h2>
          <p>Approve the PFTL vault to pull native Arbitrum USDC from this MetaMask wallet.</p>
          <div className="pfb-readout">
            <span>Arbitrum USDC</span>
            <strong>{balanceLoading ? '...' : balanceLabel(arbBalanceAtoms)}</strong>
          </div>
          {renderAmountInput('Amount to approve', 'arbitrum')}
          <button
            className="pfb-primary"
            onClick={handleApprove}
            disabled={primaryBusy || !amountReady || !canStartVaultApproval}
            title={vaultDisabledReason}
          >
            {phase === 'approving' ? <><Loader2 size={16} className="pfb-spin" /> Approving...</> : <>Approve Arbitrum USDC <ArrowRight size={16} /></>}
          </button>
        </>
      );
    }

    if (flowStepId === 4) {
      return (
        <>
          <div className="pfb-card-head">
            <Landmark size={18} />
            <span>Step 4 of 5</span>
          </div>
          <h2>Deposit to the vault</h2>
          <p>Deposit approved Arbitrum USDC. The wallet will relay the confirmed deposit to PFTL automatically.</p>
          <div className="pfb-readout">
            <span>Depositing</span>
            <strong>{amountReady ? balanceLabel(amountAtoms) : 'Enter amount'}</strong>
          </div>
          <BridgeContextRow label="Vault" value={vaultContract ? compactHash(vaultContract, 6) : 'not configured'} />
          <button
            className="pfb-primary"
            onClick={handleDeposit}
            disabled={primaryBusy || !amountReady || phase !== 'approved'}
            title={phase !== 'approved' ? 'Approve Arbitrum USDC first.' : vaultDisabledReason}
          >
            {phase === 'depositing' ? <><Loader2 size={16} className="pfb-spin" /> Depositing...</> : <>Deposit to vault <ArrowRight size={16} /></>}
          </button>
        </>
      );
    }

    return (
      <>
        <div className="pfb-card-head">
          {phase === 'complete' ? <Check size={18} /> : <Clock size={18} />}
          <span>Step 5 of 5</span>
        </div>
        <h2>{phase === 'complete' ? 'pfUSDC minted' : 'Relay to PFTL'}</h2>
        <p>
          {phase === 'complete'
            ? 'The vault deposit was relayed and claimed. pfUSDC is now in this PFTL wallet.'
            : 'The Arbitrum vault deposit is confirmed. The relay is submitting propose, attest, finalize, and claim on PFTL.'}
        </p>
        <div className="pfb-readout">
          <span>{phase === 'complete' ? 'PFTL pfUSDC' : 'Deposit ID'}</span>
          <strong>
            {phase === 'complete' && pfusdcBalanceAtoms !== null
              ? `${trimUsdc(evm.atomsToUsdc(pfusdcBalanceAtoms))} pfUSDC`
              : depositId || 'pending'}
          </strong>
        </div>
        {relayMessage && <div className={relayStatus === 'error' ? 'pf-error' : relayStatus === 'done' ? 'pf-success' : 'pf-notice'}>{relayMessage}</div>}
        {phase === 'relaying' && (
          <button className="pfb-primary" type="button" disabled>
            <Loader2 size={16} className="pfb-spin" /> Relaying to PFTL...
          </button>
        )}
        {phase !== 'relaying' && phase !== 'complete' && depositTx && (
          <button
            className="pfb-primary"
            type="button"
            onClick={() => handleRelayDeposit()}
            disabled={relayStatus === 'running'}
          >
            {relayStatus === 'running' ? <><Loader2 size={16} className="pfb-spin" /> Relaying...</> : <>Relay now <ArrowRight size={16} /></>}
          </button>
        )}
        <button className="pfb-secondary" type="button" onClick={() => refreshBalances()}>
          <RefreshCw size={15} /> Refresh wallet balances
        </button>
        <button className="pfb-secondary" type="button" onClick={resetFlow}>Start another bridge</button>
      </>
    );
  };

  return (
    <div className="pf-page pfb-page">
      {pendingBurns.length > 0 && connectedAddress && (
        <div className="pfb-banner">
          <div>
            <strong>Pending CCTP transfer detected</strong>
            <div>
              {pendingBurns.map((burn, index) => {
                const burnTxHash = cctpBurnTx(burn);
                return (
                  <div key={burnTxHash || index}>
                    {cctpBurnAmountLabel(burn)}: {isAttestationReady(burn)
                      ? 'attestation ready.'
                      : 'attestation pending.'}{' '}
                    {burnTxHash && (
                      <a href={etherscanTxUrl(burnTxHash)} target="_blank" rel="noreferrer">
                        {compactHash(burnTxHash, 8)}
                      </a>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
          {readyPendingBurn && (
            <button
              className="pf-primary"
              onClick={() => handleResumeMint(cctpBurnTx(readyPendingBurn))}
              disabled={resumeStatus === 'minting'}
            >
              {resumeStatus === 'minting' ? 'Minting...' : 'Mint on Arbitrum'}
            </button>
          )}
        </div>
      )}

      <section className="pfb-hero">
        <div>
          <div className="pf-eyebrow">MetaMask bridge-in</div>
          <h1>Bridge USDC to pfUSDC</h1>
          <p>
            Bring USDC to Arbitrum, deposit it in the PFTL vault, and relay it into pfUSDC in the same flow.
          </p>
        </div>
        <div className={`pfb-status ${phase}`}>
          <span>{phase}</span>
          <small>{STATUS_COPY[phase]}</small>
        </div>
      </section>

      {routeStatus !== 'ready' && (
        <div className="pf-warning">
          {routeStatus === 'loading' ? 'Loading the governed bridge route from PFTL…' : `Bridge deposits are blocked: ${routeError}`}
          {routeStatus === 'error' && rpc && (
            <button className="pf-link" onClick={() => refreshGovernedRoute().catch(() => {})}>
              Retry route discovery
            </button>
          )}
        </div>
      )}

      {routeStatus === 'ready' && governedRoute && (
        <div className="pf-notice">
          <strong>Governed route verified</strong>{' '}
          Epoch {governedRoute.routeEpoch} · profile {compactHash(governedRoute.profileHash, 8)} · expires in {governedRoute.remainingBlocks.toLocaleString()} PFTL blocks.
          <div>
            {governedRoute.evidenceTier === 'receipt-proven'
              ? 'Receipt-proven: validators verify the configured source-chain receipt proof.'
              : 'Independently observed: the governed validator observer quorum checks the finalized Arbitrum receipt and confirmation depth.'}
          </div>
        </div>
      )}

      {!cctpReady && (
        <div className="pf-notice">
          Circle CCTP bridge module is not available yet. Balance detection and Arbitrum vault deposit still render; Ethereum to Arbitrum bridging will enable when the module lands.
        </div>
      )}

      {error && <div className="pf-error">{error}</div>}

      <div className="pfb-manual">
        <button className="pf-link" onClick={() => setShowManualResume(v => !v)}>
          Already burned USDC on Ethereum? Enter the tx hash <ChevronDown size={12} className={showManualResume ? 'open' : ''} />
        </button>
        {showManualResume && (
          <div className="pfb-manual-row">
            <label>
              <input
                value={resumeBurnTx}
                onChange={e => setResumeBurnTx(e.target.value.trim())}
                placeholder="0x burn transaction hash"
                inputMode="text"
                aria-invalid={resumeStatus === 'error'}
              />
            </label>
            <button
              className="pfb-secondary"
              onClick={() => handleCheckResumeStatus()}
              disabled={resumeStatus === 'checking' || !cctpReady}
            >
              {resumeStatus === 'checking' ? 'Checking...' : 'Check Status'}
            </button>
            {resumeStatus === 'ready' && (
              <button
                className="pfb-primary compact"
                onClick={() => handleResumeMint()}
                disabled={resumeStatus === 'minting' || !connectedAddress}
                title={!connectedAddress ? 'Connect MetaMask before minting' : ''}
              >
                Mint on Arbitrum
              </button>
            )}
            {resumeMessage && (
              <div className={resumeStatus === 'error' || resumeStatus === 'not-found' ? 'pf-error' : resumeStatus === 'done' ? 'pf-success' : 'pf-notice'}>
                {resumeMessage}
              </div>
            )}
          </div>
        )}
      </div>

      <section className="pfb-layout">
        <main className="pfb-main-flow">
          <div className="pfb-stepper" aria-label="Bridge progress">
            {FLOW_STEPS.map((step, index) => {
              const state = stepState(step.id);
              const Icon = step.Icon;
              return (
                <React.Fragment key={step.key}>
                  <div className={`pfb-step ${state}`}>
                    <span>{state === 'done' ? <Check size={14} /> : <Icon size={15} />}</span>
                    <strong>{step.label}</strong>
                  </div>
                  {index < FLOW_STEPS.length - 1 && <i className={state === 'done' ? 'done' : ''} />}
                </React.Fragment>
              );
            })}
          </div>

          <div className="pfb-action-card">
            {renderActionCard()}
          </div>

          {showBridgeProgress && (
            <div className="pfb-progress-card">
              <strong>{bridgeStep || 'Preparing Circle CCTP bridge...'}</strong>
              <span>{bridgeElapsed || 'Live bridge progress'}</span>
              <BridgeLedgerRow label="Burn tx" value={bridgeTx ? compactHash(bridgeTx, 8) : 'pending'} href={bridgeTx ? etherscanTxUrl(bridgeTx) : ''} />
              <BridgeLedgerRow label="Message hash" value={bridgeMessageHash ? compactHash(bridgeMessageHash, 10) : 'pending'} />
              <BridgeLedgerRow label="Mint tx" value={bridgeMintTx ? compactHash(bridgeMintTx, 8) : 'pending'} href={bridgeMintTx ? arbiscanTxUrl(bridgeMintTx) : ''} />
              {manualMintAvailable && (
                <button
                  className="pfb-primary compact"
                  onClick={() => handleResumeMint(resumeBurnTx || bridgeTx)}
                  disabled={resumeStatus === 'minting' || !connectedAddress}
                >
                  {resumeStatus === 'minting' ? 'Minting...' : 'Mint on Arbitrum'}
                </button>
              )}
            </div>
          )}
        </main>

        <aside className="pfb-side">
          <section className="pfb-location">
            <div className="pfb-location-head">
              <Info size={14} />
              <span>Where is my USDC?</span>
            </div>
            <h2>{locationTitle}</h2>
            <p>{locationBody}</p>
          </section>

          <section className="pfb-side-section">
            <div className="pfb-side-title">
              <span>Balances</span>
              <button type="button" onClick={() => refreshBalances()} aria-label="Refresh balances">
                <RefreshCw size={13} className={balanceLoading ? 'pfb-spin' : ''} />
              </button>
            </div>
            <BridgeBalanceRow label="Ethereum USDC" value={balanceLoading ? '...' : trimUsdc(evm.atomsToUsdc(ethBalanceAtoms))} unit="USDC" active={flowStepId === 2} />
            <BridgeBalanceRow label="Arbitrum USDC" value={balanceLoading ? '...' : trimUsdc(evm.atomsToUsdc(arbBalanceAtoms))} unit="USDC" active={flowStepId === 3 || flowStepId === 4} />
            <BridgeBalanceRow label="Arbitrum ETH" value={balanceLoading ? '...' : ethLabel(arbEthBalanceWei).replace(' ETH', '')} unit="ETH" active={flowStepId === 1} />
          </section>

          <section className="pfb-side-section">
            <BridgeContextRow label="MetaMask" value={connectedAddress ? compactHash(connectedAddress, 6) : 'not connected'} />
            <BridgeContextRow label="Network" value={chainName(currentChainId)} />
            <BridgeContextRow label="PFTL recipient" value={address ? compactHash(address, 8) : 'wallet unavailable'} />
          </section>

          <details className="pfb-details">
            <summary>
              <span><Info size={12} /> Transaction details</span>
              <ChevronDown size={14} />
            </summary>
            <p>Circle CCTP handles Ethereum to Arbitrum USDC. After the vault deposit, the wallet proxy submits the PFTL relay and claim.</p>
            <BridgeLedgerRow label="Vault" value={vaultContract ? compactHash(vaultContract, 6) : 'not configured'} />
            <BridgeLedgerRow label="Route profile" value={governedRoute ? compactHash(governedRoute.profileHash, 8) : 'unavailable'} />
            <BridgeLedgerRow label="Route epoch" value={governedRoute ? String(governedRoute.routeEpoch) : 'unavailable'} />
            <BridgeLedgerRow label="Evidence tier" value={governedRoute?.evidenceTier || 'unavailable'} />
            <BridgeLedgerRow label="CCTP approval" value={bridgeApprovalTx ? compactHash(bridgeApprovalTx, 6) : 'none'} href={bridgeApprovalTx ? etherscanTxUrl(bridgeApprovalTx) : ''} />
            <BridgeLedgerRow label="Burn tx" value={bridgeTx ? compactHash(bridgeTx, 6) : 'none'} href={bridgeTx ? etherscanTxUrl(bridgeTx) : ''} />
            <BridgeLedgerRow label="Message hash" value={bridgeMessageHash ? compactHash(bridgeMessageHash, 10) : 'none'} />
            <BridgeLedgerRow label="Mint tx" value={bridgeMintTx ? compactHash(bridgeMintTx, 6) : 'none'} href={bridgeMintTx ? arbiscanTxUrl(bridgeMintTx) : ''} />
            <BridgeLedgerRow label="Vault approval" value={approvalTx ? compactHash(approvalTx, 6) : 'none'} href={approvalTx ? arbiscanTxUrl(approvalTx) : ''} />
            <BridgeLedgerRow label="Vault deposit" value={depositTx ? compactHash(depositTx, 6) : 'none'} href={depositTx ? arbiscanTxUrl(depositTx) : ''} />
            <BridgeLedgerRow label="Deposit ID" value={depositId || 'pending'} />
            {relayTxs.map((item) => (
              <BridgeLedgerRow
                key={`${item.label}-${item.tx_id || item.height || 'pending'}`}
                label={`Relay ${item.label}`}
                value={item.tx_id ? compactHash(item.tx_id, 6) : item.height ? `height ${item.height}` : 'pending'}
              />
            ))}
            <BridgeLedgerRow
              label="pfUSDC balance"
              value={pfusdcBalanceAtoms !== null ? `${trimUsdc(evm.atomsToUsdc(pfusdcBalanceAtoms))} pfUSDC` : 'pending'}
            />
          </details>

          {(phase === 'relaying' || phase === 'deposited') && (
            <div className="pf-notice">
              Vault deposit confirmed. Relay is submitting PFTL finality transactions now.
            </div>
          )}
          {phase === 'complete' && (
            <div className="pf-success">
              Relay complete. pfUSDC is visible in this wallet.
            </div>
          )}
        </aside>
      </section>
    </div>
  );
}
