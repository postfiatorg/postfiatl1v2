import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ArrowRight,
  Check,
  ChevronDown,
  Clock,
  Info,
  Landmark,
  Loader2,
  RefreshCw,
  ShieldCheck,
  Wallet,
} from 'lucide-react';
import * as evm from '../lib/evm.js';
import * as utils from '../lib/utils.js';
import {
  relayVaultDeposit,
  waitForBridgeReadiness,
} from '../lib/bridge-relay.js';
import { loadGovernedVaultBridgeRoute } from '../lib/bridge-route.js';
import { acquireAutoLockLease } from '../lib/vault.js';

const ETHEREUM_CHAIN_ID = utils.ETH_MAINNET_CHAIN_ID || 1;
const ETHEREUM_USDC = utils.ETH_MAINNET_USDC;

const FLOW_STEPS = [
  { id: 1, label: 'Connect', Icon: Wallet },
  { id: 2, label: 'Approve', Icon: ShieldCheck },
  { id: 3, label: 'Deposit', Icon: Landmark },
  { id: 4, label: 'Relay', Icon: Clock },
];

const STATUS_COPY = {
  disconnected: 'Connect MetaMask to use Ethereum mainnet USDC.',
  connecting: 'Connecting MetaMask and switching to Ethereum mainnet.',
  connected: 'Choose how much Ethereum USDC to deposit.',
  approving: 'Confirm the USDC approval in MetaMask.',
  approved: 'Approval confirmed. The vault deposit is ready.',
  depositing: 'Confirm the governed vault deposit in MetaMask.',
  deposited: 'Ethereum deposit confirmed. Starting the PFTL relay.',
  relaying: 'Proving and relaying the deposit into PFTL.',
  complete: 'pfUSDC is now in the PFTL wallet.',
  error: 'This action needs attention.',
};

function trimUsdc(value) {
  return String(value || '0').replace(/(\.\d*?)0+$/, '$1').replace(/\.$/, '');
}

function normalizeAmountInput(value) {
  const cleaned = String(value || '').replace(/[^\d.]/g, '');
  const [whole, ...rest] = cleaned.split('.');
  return rest.length ? `${whole}.${rest.join('').slice(0, 6)}` : whole;
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

function usdcLabel(atoms) {
  return `${trimUsdc(evm.atomsToUsdc(atoms || 0n))} USDC`;
}

function pfusdcLabel(atoms) {
  return `${trimUsdc(evm.atomsToUsdc(atoms || 0n))} pfUSDC`;
}

function ethLabel(wei) {
  const n = BigInt(wei || 0n);
  const scale = 10n ** 18n;
  const whole = n / scale;
  const fraction = (n % scale).toString().padStart(18, '0').slice(0, 8).replace(/0+$/, '');
  return `${whole}${fraction ? `.${fraction}` : ''} ETH`;
}

function compact(value, size = 7) {
  return utils.truncateMiddle ? utils.truncateMiddle(value, size) : value;
}

function normalizeTxHash(value) {
  const text = String(value || '').trim();
  const prefixed = text.toLowerCase().startsWith('0x') ? text : `0x${text}`;
  return /^0x[0-9a-fA-F]{64}$/.test(prefixed) ? prefixed : '';
}

function etherscanTx(txHash) {
  return txHash ? `https://etherscan.io/tx/${txHash}` : '';
}

function humanEvmError(error) {
  const message = error?.message || String(error || 'unknown error');
  const data = typeof error?.data === 'string' ? error.data.toLowerCase() : '';
  if (data.startsWith('0xbe24f3c5')) return 'The vault could not pull USDC. Approve USDC again, then retry.';
  if (data.startsWith('0xda9f8b34')) return 'The governed vault is paused.';
  if (data.startsWith('0x2c5211c6')) return 'Deposit amount must be greater than zero.';
  if (data.startsWith('0x02694994')) return 'The PFTL recipient is missing.';
  if (data.startsWith('0xfa98d908')) return 'This nonce was already used. Retry to generate a fresh nonce.';
  if (/insufficient funds|insufficient balance/i.test(message)) {
    return 'Not enough Ethereum ETH for gas.';
  }
  return message;
}

function accountPftlBalance(response) {
  if (response?.ok !== true || !response.result) {
    throw new Error(response?.error?.message || 'PFTL asset balance is unavailable.');
  }
  const assets = Array.isArray(response.result)
    ? response.result
    : (response.result.assets || []);
  const row = assets.find((item) => (
    String(item?.asset_id || item?.id || '').toLowerCase() === utils.PFUSDC_ASSET_ID
  ));
  return BigInt(row?.balance ?? row?.amount ?? 0);
}

function BalanceRow({ label, value, active = false }) {
  return (
    <div className={`pfb-balance-row${active ? ' active' : ''}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function ContextRow({ label, value, href = '' }) {
  return (
    <div className="pfb-context-row">
      <span>{label}</span>
      {href
        ? <a href={href} target="_blank" rel="noreferrer">{value}</a>
        : <strong>{value}</strong>}
    </div>
  );
}

export default function Bridge({ address, rpc, proxyAuthToken = '' }) {
  const [phase, setPhase] = useState('disconnected');
  const [connectedAddress, setConnectedAddress] = useState('');
  const [chainId, setChainId] = useState(0);
  const [usdcBalance, setUsdcBalance] = useState(0n);
  const [ethBalance, setEthBalance] = useState(0n);
  const [balanceLoading, setBalanceLoading] = useState(false);
  const [amount, setAmount] = useState('');
  const [approvedAtoms, setApprovedAtoms] = useState(null);
  const [approvalTx, setApprovalTx] = useState('');
  const [depositTx, setDepositTx] = useState('');
  const [depositId, setDepositId] = useState('');
  const [relayTxs, setRelayTxs] = useState([]);
  const [pfusdcBalance, setPfusdcBalance] = useState(null);
  const [route, setRoute] = useState(null);
  const [routeStatus, setRouteStatus] = useState('loading');
  const [routeError, setRouteError] = useState('');
  const [relayStatus, setRelayStatus] = useState('');
  const [error, setError] = useState('');
  const [manualOpen, setManualOpen] = useState(false);
  const [manualTx, setManualTx] = useState('');

  const amountAtoms = useMemo(() => safeAtoms(amount), [amount]);
  const vault = route?.vaultAddress || '';

  const loadRoute = useCallback(async ({ expectedProfileHash = '' } = {}) => {
    if (!rpc) throw new Error('Wallet RPC is not connected.');
    setRouteStatus('loading');
    setRouteError('');
    try {
      const next = await loadGovernedVaultBridgeRoute(rpc, {
        assetId: utils.PFUSDC_ASSET_ID,
        chainId: utils.CHAIN_ID,
        genesisHash: utils.GENESIS_HASH,
        sourceChainId: ETHEREUM_CHAIN_ID,
        tokenAddress: ETHEREUM_USDC,
      });
      if (expectedProfileHash && next.profileHash !== expectedProfileHash) {
        throw new Error('The governed bridge route changed. Review it before signing.');
      }
      await waitForBridgeReadiness(next);
      setRoute(next);
      setRouteStatus('ready');
      return next;
    } catch (failure) {
      const message = failure?.message || 'Governed route discovery failed.';
      setRoute(null);
      setRouteError(message);
      setRouteStatus('error');
      throw failure;
    }
  }, [rpc]);

  useEffect(() => {
    let cancelled = false;
    let retryTimer = null;
    let retriesRemaining = 2;
    const discover = async () => {
      try {
        await loadRoute();
      } catch (_) {
        if (!cancelled && retriesRemaining > 0) {
          retriesRemaining -= 1;
          retryTimer = setTimeout(discover, 3000);
        }
      }
    };
    discover();
    return () => {
      cancelled = true;
      if (retryTimer) clearTimeout(retryTimer);
    };
  }, [loadRoute]);

  const refreshBalances = useCallback(async (owner = connectedAddress) => {
    if (!owner) return;
    setBalanceLoading(true);
    try {
      await evm.ensureEthereumMainnet();
      const [usdc, eth, currentChain] = await Promise.all([
        evm.getEthereumUsdcBalance(owner),
        evm.getEthereumEthBalance(owner),
        window.ethereum.request({ method: 'eth_chainId' }),
      ]);
      setUsdcBalance(usdc);
      setEthBalance(eth);
      setChainId(Number.parseInt(currentChain, 16));
    } finally {
      setBalanceLoading(false);
    }
  }, [connectedAddress]);

  useEffect(() => {
    if (!evm.hasMetaMask()) return undefined;
    let active = true;
    window.ethereum.request({ method: 'eth_accounts' }).then(async (accounts) => {
      if (!active || !accounts?.length) return;
      setConnectedAddress(accounts[0]);
      setPhase('connected');
      await refreshBalances(accounts[0]).catch(() => {});
    }).catch(() => {});
    const accountsChanged = (accounts) => {
      const owner = accounts?.[0] || '';
      setConnectedAddress(owner);
      setApprovedAtoms(null);
      setPhase(owner ? 'connected' : 'disconnected');
      if (owner) refreshBalances(owner).catch(() => {});
    };
    const chainChanged = (next) => {
      setChainId(Number.parseInt(next, 16));
      if (connectedAddress) refreshBalances(connectedAddress).catch(() => {});
    };
    window.ethereum.on?.('accountsChanged', accountsChanged);
    window.ethereum.on?.('chainChanged', chainChanged);
    return () => {
      active = false;
      window.ethereum.removeListener?.('accountsChanged', accountsChanged);
      window.ethereum.removeListener?.('chainChanged', chainChanged);
    };
  }, [connectedAddress, refreshBalances]);

  const amountError = useMemo(() => {
    if (!amount) return '';
    if (amountAtoms === null || amountAtoms <= 0n) return 'Enter an amount greater than zero.';
    if (amountAtoms > usdcBalance) return `Amount exceeds your Ethereum balance (${usdcLabel(usdcBalance)}).`;
    return '';
  }, [amount, amountAtoms, usdcBalance]);

  const refreshAndVerifyRoute = async () => {
    const active = await loadRoute({ expectedProfileHash: route?.profileHash || '' });
    await evm.ensureEthereumMainnet();
    await Promise.all([
      evm.assertContractCodeHash(active.vaultAddress, active.vaultRuntimeCodeHash),
      evm.assertContractCodeHash(active.tokenAddress, active.tokenRuntimeCodeHash),
    ]);
    await waitForBridgeReadiness(active);
    return active;
  };

  const connect = async () => {
    setError('');
    try {
      setPhase('connecting');
      const owner = await evm.connectMetaMask();
      await evm.ensureEthereumMainnet();
      setConnectedAddress(owner);
      setChainId(ETHEREUM_CHAIN_ID);
      await refreshBalances(owner);
      setPhase('connected');
    } catch (failure) {
      setPhase('error');
      setError(humanEvmError(failure));
    }
  };

  const assertAmountReady = () => {
    if (!address) throw new Error('Unlock the PFTL wallet before depositing.');
    if (!route) throw new Error(routeError || 'No active governed Ethereum route is available.');
    if (!amountAtoms || amountAtoms <= 0n || amountError) {
      throw new Error(amountError || 'Enter an amount greater than zero.');
    }
  };

  const assertAffordable = (fee, label) => {
    if (fee?.maxCostWei > ethBalance) {
      throw new Error(`${label} needs about ${ethLabel(fee.maxCostWei)}; the wallet has ${ethLabel(ethBalance)}.`);
    }
  };

  const approve = async () => {
    setError('');
    try {
      assertAmountReady();
      setPhase('approving');
      const active = await refreshAndVerifyRoute();
      const fee = await evm.estimateEthereumApproveUsdcFee(
        active.vaultAddress,
        amountAtoms,
        connectedAddress,
      );
      assertAffordable(fee, 'USDC approval');
      const txHash = await evm.approveEthereumUsdc(active.vaultAddress, amountAtoms);
      setApprovalTx(txHash);
      await evm.waitForReceipt(txHash);
      const allowance = await evm.getEthereumUsdcAllowance(connectedAddress, active.vaultAddress);
      if (allowance < amountAtoms) throw new Error('Approval confirmed, but the vault allowance is still too low.');
      setApprovedAtoms(amountAtoms);
      setPhase('approved');
    } catch (failure) {
      setPhase('error');
      setError(`USDC approval failed: ${humanEvmError(failure)}`);
    }
  };

  const relay = async ({ txHash, event, activeRoute }) => {
    setPhase('relaying');
    const routeBinding = evm.governedRouteBinding(activeRoute.profileHash, activeRoute.routeEpoch);
    const result = await relayVaultDeposit({
      depositTxHash: txHash,
      depositId: event?.deposit_id || '',
      pftlRecipient: address,
      depositor: event?.depositor || connectedAddress,
      amountAtoms: event?.amount?.toString() || amountAtoms?.toString() || '',
      idempotencyKey: `vault-relay:${txHash.toLowerCase()}`,
      routeProfileHash: activeRoute.profileHash,
      routeEpoch: activeRoute.routeEpoch,
      routeBinding,
      routeId: activeRoute.profile.route_id,
      sourceChainId: activeRoute.profile.source_chain_id,
      proxyAuthToken,
      onStatus: (next) => setRelayStatus(next.status || ''),
    });
    const relayId = result.tx_id || result.receipt_id;
    setRelayTxs(
      relayId
        ? [{
            kind: result.tx_id ? 'PFTL claim' : 'PFTL receipt',
            tx_id: relayId,
          }]
        : [],
    );
    if (result.after_balance_atoms !== undefined && result.after_balance_atoms !== null) {
      setPfusdcBalance(BigInt(result.after_balance_atoms));
    } else {
      // The durable relay receipt proves acceptance, but older bridge workers
      // do not include the resulting account balance. Read it from PFTL so the
      // completion screen always shows what the user actually received.
      try {
        setPfusdcBalance(accountPftlBalance(await rpc.accountAssets(address)));
      } catch (_) {
        setPfusdcBalance(null);
      }
    }
    setPhase('complete');
  };

  const deposit = async () => {
    setError('');
    let confirmed = false;
    const releaseAutoLock = acquireAutoLockLease();
    try {
      assertAmountReady();
      if (approvedAtoms === null || approvedAtoms < amountAtoms) {
        throw new Error('Approve this USDC amount before depositing.');
      }
      setPhase('depositing');
      const active = await refreshAndVerifyRoute();
      const allowance = await evm.getEthereumUsdcAllowance(connectedAddress, active.vaultAddress);
      if (allowance < amountAtoms) throw new Error('The current USDC allowance is below the deposit amount.');
      const nonce = evm.generateNonce();
      const routeBinding = evm.governedRouteBinding(active.profileHash, active.routeEpoch);
      const fee = await evm.estimateEthereumBridgeDepositFee(
        active.vaultAddress,
        amountAtoms,
        address,
        nonce,
        routeBinding,
        connectedAddress,
      );
      assertAffordable(fee, 'Vault deposit');
      const txHash = await evm.depositToEthereumBridge(
        active.vaultAddress,
        amountAtoms,
        address,
        nonce,
        routeBinding,
      );
      setDepositTx(txHash);
      await evm.waitForReceipt(txHash);
      confirmed = true;
      const event = await evm.watchDepositEvent(active.vaultAddress, txHash, routeBinding);
      if (!event) throw new Error('The confirmed transaction did not contain the expected vault deposit event.');
      if (event.pftl_recipient !== address) throw new Error('The vault event recipient does not match this PFTL wallet.');
      if (event.token.toLowerCase() !== active.tokenAddress.toLowerCase()) throw new Error('The vault event token is not governed USDC.');
      if (event.source_chain_id !== BigInt(ETHEREUM_CHAIN_ID)) throw new Error('The vault event came from the wrong source chain.');
      setDepositId(event.deposit_id);
      setPhase('deposited');
      await refreshBalances(connectedAddress);
      await relay({ txHash, event, activeRoute: active });
    } catch (failure) {
      setPhase('error');
      setError(`${confirmed ? 'Deposit confirmed, but relay failed' : 'Vault deposit failed'}: ${humanEvmError(failure)}`);
    } finally {
      releaseAutoLock();
    }
  };

  const resumeRelay = async () => {
    setError('');
    const releaseAutoLock = acquireAutoLockLease();
    try {
      const txHash = normalizeTxHash(manualTx);
      if (!txHash) throw new Error('Enter a valid Ethereum transaction hash.');
      if (!address) throw new Error('Unlock the PFTL wallet before relaying.');
      const active = await refreshAndVerifyRoute();
      await evm.waitForReceipt(txHash);
      const routeBinding = evm.governedRouteBinding(active.profileHash, active.routeEpoch);
      const event = await evm.watchDepositEvent(active.vaultAddress, txHash, routeBinding);
      if (!event) throw new Error('No governed vault deposit was found in this transaction.');
      if (event.pftl_recipient !== address) throw new Error('This deposit was made to a different PFTL recipient.');
      if (event.token.toLowerCase() !== active.tokenAddress.toLowerCase()) throw new Error('This deposit used the wrong token.');
      if (event.source_chain_id !== BigInt(ETHEREUM_CHAIN_ID)) throw new Error('This is not an Ethereum mainnet deposit.');
      setDepositTx(txHash);
      setDepositId(event.deposit_id);
      await relay({ txHash, event, activeRoute: active });
    } catch (failure) {
      setPhase('error');
      setError(`Relay recovery failed: ${humanEvmError(failure)}`);
    } finally {
      releaseAutoLock();
    }
  };

  const reset = () => {
    setAmount('');
    setApprovedAtoms(null);
    setApprovalTx('');
    setDepositTx('');
    setDepositId('');
    setRelayTxs([]);
    setRelayStatus('');
    setPfusdcBalance(null);
    setError('');
    setPhase(connectedAddress ? 'connected' : 'disconnected');
    if (connectedAddress) refreshBalances(connectedAddress).catch(() => {});
  };

  const currentStep = !connectedAddress
    ? 1
    : ['connected', 'approving', 'error'].includes(phase) && approvedAtoms === null
      ? 2
      : ['approved', 'depositing'].includes(phase)
        ? 3
        : 4;
  const busy = ['connecting', 'approving', 'depositing', 'relaying'].includes(phase);
  const status = routeStatus === 'error' ? 'error' : phase;
  const canApprove = Boolean(
    connectedAddress && route && address && amountAtoms && amountAtoms > 0n
    && proxyAuthToken && !amountError && !busy,
  );
  const canDeposit = Boolean(canApprove && approvedAtoms !== null && approvedAtoms >= amountAtoms);

  return (
    <div className="pf-page pfb-page">
      <header className="pfb-hero">
        <div>
          <div className="pf-eyebrow">MetaMask bridge-in · Ethereum mainnet</div>
          <h1>Bridge USDC to pfUSDC</h1>
          <p>
            Deposit canonical Ethereum USDC into the governed PFTL vault. The confirmed deposit
            is proof-verified and relayed into pfUSDC for this PFTL wallet.
          </p>
        </div>
        <div className={`pfb-status ${status}`}>
          <span>{status === 'complete' ? 'Complete' : status === 'error' ? 'Blocked' : 'Ready'}</span>
          <small>{routeStatus === 'loading' ? 'Loading the governed route…' : STATUS_COPY[status] || STATUS_COPY.connected}</small>
        </div>
      </header>

      <div className="pfb-banner">
        <div>
          <strong>Arbitrum is retired for new pfUSDC deposits.</strong>
          Use USDC already on Ethereum mainnet. Do not bridge new USDC to Arbitrum for this flow.
        </div>
      </div>

      {routeStatus === 'error' && (
        <div className="pf-warning">
          Bridge deposits are blocked: {routeError}{' '}
          <button className="pf-link" type="button" onClick={() => loadRoute().catch(() => {})}>
            Retry route discovery
          </button>
        </div>
      )}
      {error && <div className="pf-warning">{error}</div>}
      {!proxyAuthToken && (
        <div className="pf-warning">
          Bridge deposits are blocked until the session-only proxy access token is entered in More.
        </div>
      )}

      <div className="pfb-manual">
        <button className="pf-link" type="button" onClick={() => setManualOpen((open) => !open)}>
          Already deposited into the Ethereum vault? Resume from the transaction hash
          <ChevronDown size={14} className={manualOpen ? 'open' : ''} />
        </button>
        {manualOpen && (
          <div className="pfb-manual-row">
            <label>
              <input
                value={manualTx}
                onChange={(event) => setManualTx(event.target.value)}
                placeholder="0x… Ethereum vault deposit transaction"
                spellCheck="false"
              />
            </label>
            <button className="pfb-secondary small" type="button" onClick={resumeRelay} disabled={busy}>
              {phase === 'relaying' ? <Loader2 size={14} className="pfb-spin" /> : <RefreshCw size={14} />}
              Resume relay
            </button>
          </div>
        )}
      </div>

      <div className="pfb-layout">
        <main className="pfb-main-flow">
          <div className="pfb-stepper four" aria-label="Bridge progress">
            {FLOW_STEPS.map((step, index) => (
              <React.Fragment key={step.id}>
                <div className={`pfb-step${step.id < currentStep || phase === 'complete' ? ' done' : ''}${step.id === currentStep && phase !== 'complete' ? ' active' : ''}`}>
                  <span>{step.id < currentStep || phase === 'complete' ? <Check size={15} /> : <step.Icon size={15} />}</span>
                  <strong>{step.label}</strong>
                </div>
                {index < FLOW_STEPS.length - 1 && <i className={step.id < currentStep || phase === 'complete' ? 'done' : ''} />}
              </React.Fragment>
            ))}
          </div>

          <section className="pfb-action-card">
            {!connectedAddress ? (
              <>
                <div className="pfb-card-head"><Wallet size={15} /> Step 1 of 4</div>
                <h2>Connect MetaMask</h2>
                <p>Connect the wallet holding your Ethereum mainnet USDC. MetaMask will switch to Ethereum mainnet.</p>
                <button className="pfb-primary" type="button" onClick={connect} disabled={!evm.hasMetaMask() || busy}>
                  {phase === 'connecting' ? <Loader2 size={16} className="pfb-spin" /> : <Wallet size={16} />}
                  {evm.hasMetaMask() ? 'Connect MetaMask' : 'MetaMask not found'}
                </button>
              </>
            ) : phase === 'complete' ? (
              <>
                <div className="pfb-card-head"><Check size={15} /> Complete</div>
                <h2>pfUSDC received on PFTL</h2>
                <p>The Ethereum vault deposit was confirmed, verified, finalized, and claimed into this PFTL wallet.</p>
                {pfusdcBalance !== null && <div className="pfb-readout"><span>PFTL balance</span><strong>{pfusdcLabel(pfusdcBalance)}</strong></div>}
                <button className="pfb-secondary" type="button" onClick={reset}>Make another deposit</button>
              </>
            ) : (
              <>
                <div className="pfb-card-head">
                  {currentStep === 2 ? <ShieldCheck size={15} /> : currentStep === 3 ? <Landmark size={15} /> : <Clock size={15} />}
                  Step {currentStep} of 4
                </div>
                <h2>{currentStep === 2 ? 'Approve Ethereum USDC' : currentStep === 3 ? 'Deposit into the governed vault' : 'Relay into PFTL'}</h2>
                <p>
                  {currentStep === 2
                    ? 'Approve only the amount you want the current governed Ethereum vault to pull.'
                    : currentStep === 3
                      ? 'Submit the route-bound vault deposit. Do not send USDC directly to the vault address.'
                      : 'The deposit is confirmed. The wallet is completing the proof-backed PFTL relay.'}
                </p>
                <label className="pfb-field">
                  <span>Amount to deposit</span>
                  <div className="pfb-amount">
                    <input
                      inputMode="decimal"
                      value={amount}
                      onChange={(event) => {
                        setAmount(normalizeAmountInput(event.target.value));
                        setApprovedAtoms(null);
                        if (!busy) setPhase('connected');
                      }}
                      placeholder="0.00"
                      disabled={busy || currentStep === 4}
                    />
                    <span>USDC</span>
                    <button
                      className="pfb-secondary small"
                      type="button"
                      onClick={() => setAmount(trimUsdc(evm.atomsToUsdc(usdcBalance)))}
                      disabled={busy || currentStep === 4}
                    >
                      Max
                    </button>
                  </div>
                  {amountError && <small className="pfb-inline-error">{amountError}</small>}
                </label>
                {currentStep === 2 && (
                  <button className="pfb-primary" type="button" onClick={approve} disabled={!canApprove}>
                    {phase === 'approving' ? <Loader2 size={16} className="pfb-spin" /> : <ShieldCheck size={16} />}
                    {phase === 'approving' ? 'Waiting for approval…' : 'Approve Ethereum USDC'}
                  </button>
                )}
                {currentStep === 3 && (
                  <button className="pfb-primary" type="button" onClick={deposit} disabled={!canDeposit}>
                    {phase === 'depositing' ? <Loader2 size={16} className="pfb-spin" /> : <Landmark size={16} />}
                    {phase === 'depositing' ? 'Depositing…' : <>Deposit and relay <ArrowRight size={16} /></>}
                  </button>
                )}
                {currentStep === 4 && (
                  <div className="pfb-progress-card">
                    <strong><Loader2 size={14} className="pfb-spin" /> Proof and relay in progress</strong>
                    <span>
                      {relayStatus ? `Current stage: ${relayStatus.replaceAll('_', ' ')}. ` : ''}
                      The backend job is durable; a confirmed deposit can be resumed by transaction hash.
                    </span>
                  </div>
                )}
              </>
            )}
          </section>
        </main>

        <aside className="pfb-side">
          <div className="pfb-location">
            <div className="pfb-location-head"><Landmark size={14} /> Current route</div>
            <h2>Ethereum USDC → PFTL pfUSDC</h2>
            <p>One Ethereum vault deposit followed by the PFTL proof and claim relay. No Arbitrum hop.</p>
          </div>
          <div className="pfb-side-section">
            <div className="pfb-side-title">
              Balances
              <button type="button" onClick={() => refreshBalances()} disabled={!connectedAddress || balanceLoading} aria-label="Refresh balances">
                <RefreshCw size={14} className={balanceLoading ? 'pfb-spin' : ''} />
              </button>
            </div>
            <BalanceRow label="Ethereum USDC" value={balanceLoading ? '…' : usdcLabel(usdcBalance)} active={currentStep === 2 || currentStep === 3} />
            <BalanceRow label="Ethereum gas" value={balanceLoading ? '…' : ethLabel(ethBalance)} />
            {pfusdcBalance !== null && <BalanceRow label="PFTL pfUSDC" value={pfusdcLabel(pfusdcBalance)} active={phase === 'complete'} />}
          </div>
          <div className="pfb-side-section">
            <div className="pfb-side-title">Transaction context</div>
            <ContextRow label="MetaMask" value={connectedAddress ? compact(connectedAddress, 6) : 'not connected'} />
            <ContextRow label="Network" value={chainId === ETHEREUM_CHAIN_ID ? 'Ethereum mainnet' : chainId ? `Wrong chain (${chainId})` : 'not connected'} />
            <ContextRow label="PFTL recipient" value={address ? compact(address, 8) : 'wallet locked'} />
            <ContextRow label="Vault" value={vault ? compact(vault, 6) : 'unavailable'} />
          </div>
          <details className="pfb-details">
            <summary><span><Info size={14} /> Verified route</span><ChevronDown size={14} /></summary>
            <p>The wallet accepts only the active route returned by PFTL and checks its chain, token, epoch, profile hash, route binding, and deployed bytecode before signing.</p>
            <ContextRow label="Route ID" value={route?.profile?.route_id || 'unavailable'} />
            <ContextRow label="Route epoch" value={route ? String(route.routeEpoch) : 'unavailable'} />
            <ContextRow label="Profile" value={route ? compact(route.profileHash, 8) : 'unavailable'} />
            <ContextRow label="Evidence" value={route?.evidenceTier || 'unavailable'} />
          </details>
          {(approvalTx || depositTx || relayTxs.length > 0) && (
            <div className="pfb-side-section">
              <div className="pfb-side-title">Activity</div>
              {approvalTx && <ContextRow label="Approval" value={compact(approvalTx, 6)} href={etherscanTx(approvalTx)} />}
              {depositTx && <ContextRow label="Deposit" value={compact(depositTx, 6)} href={etherscanTx(depositTx)} />}
              {depositId && <ContextRow label="Deposit ID" value={compact(depositId, 7)} />}
              {relayTxs.map((tx, index) => (
                <ContextRow
                  key={`${tx.tx_id || tx.txid || index}`}
                  label={tx.kind || `PFTL tx ${index + 1}`}
                  value={compact(tx.tx_id || tx.txid || String(tx), 7)}
                />
              ))}
            </div>
          )}
        </aside>
      </div>
    </div>
  );
}
