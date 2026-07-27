// Ethereum Sepolia L1 fast lane (route ethereum-sepolia-usdc-v1).
// MetaMask-compatible USDC approve/deposit plus a deterministic six-stage
// state machine: deposit -> credit -> transparent send -> Orchard shielded
// send -> burn -> withdrawal. Scope is Ethereum Sepolia (chain id 11155111)
// only; no Arbitrum markers, no observer/mock fallback.

export const ETH_SEPOLIA_CHAIN_ID = 11155111;
export const ETH_SEPOLIA_CHAIN_ID_HEX = '0xaa36a7';
export const ETH_SEPOLIA_USDC = '0x1c7d4b196cb0c7b01d743fbc6116a902379c7238';
export const ETH_FAST_LANE_ROUTE_ID = 'ethereum-sepolia-usdc-v1';
export const ETH_FAST_LANE_PROOF_KIND = 'sp1-ethereum-finality-v1';
export const LIFECYCLE_RELEASE_REQUIRED = false;

export const ETH_FAST_LANE_STAGES = Object.freeze([
  'deposit',
  'credit',
  'transparent_send',
  'orchard_send',
  'burn',
  'withdrawal',
]);

const PFUSDC_DECIMALS = 6;
const HEX_32 = /^(0x)?[0-9a-f]{64}$/;
const EVM_ADDRESS = /^0x[0-9a-f]{40}$/;

export function hasMetaMask() {
  return typeof window !== 'undefined' && typeof window.ethereum !== 'undefined';
}

export function assertEthFastLaneRoute(route) {
  const haystack = JSON.stringify(route ?? {}).toLowerCase();
  if (haystack.includes('arbitrum')) {
    throw new Error('Arbitrum route markers are outside the Ethereum L1 fast-lane scope');
  }
  if (!route || typeof route !== 'object') {
    throw new Error('route config required');
  }
  if (route.route_id !== ETH_FAST_LANE_ROUTE_ID) {
    throw new Error(`route_id must be ${ETH_FAST_LANE_ROUTE_ID}`);
  }
  if (Number(route.source_chain_id) !== ETH_SEPOLIA_CHAIN_ID) {
    throw new Error(`source_chain_id must be ${ETH_SEPOLIA_CHAIN_ID}`);
  }
  if (typeof route.token_address !== 'string' || route.token_address.toLowerCase() !== ETH_SEPOLIA_USDC) {
    throw new Error('token must be canonical Circle Sepolia USDC');
  }
  return route;
}

export async function connectMetaMask() {
  if (!hasMetaMask()) {
    throw new Error('MetaMask not found. Install MetaMask or enable the browser extension.');
  }
  try {
    const accounts = await window.ethereum.request({ method: 'eth_requestAccounts' });
    return accounts[0];
  } catch (error) {
    throw new Error('MetaMask connection rejected: ' + (error?.message || 'unknown error'));
  }
}

export async function ensureEthereumSepolia() {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  const chainId = await window.ethereum.request({ method: 'eth_chainId' });
  if (parseInt(chainId, 16) === ETH_SEPOLIA_CHAIN_ID) return;
  try {
    await window.ethereum.request({
      method: 'wallet_switchEthereumChain',
      params: [{ chainId: ETH_SEPOLIA_CHAIN_ID_HEX }],
    });
  } catch (error) {
    if (error?.code === 4902) {
      await window.ethereum.request({
        method: 'wallet_addEthereumChain',
        params: [{
          chainId: ETH_SEPOLIA_CHAIN_ID_HEX,
          chainName: 'Ethereum Sepolia',
          nativeCurrency: { name: 'Sepolia ETH', symbol: 'ETH', decimals: 18 },
          rpcUrls: ['https://ethereum-sepolia-rpc.publicnode.com'],
          blockExplorerUrls: ['https://sepolia.etherscan.io'],
        }],
      });
      return;
    }
    throw error;
  }
}

function strip0x(value) {
  return String(value).startsWith('0x') ? String(value).slice(2) : String(value);
}

function pad32(hex) {
  return strip0x(hex).padStart(64, '0');
}

export function encodeApprove(spender, amountAtoms) {
  if (!EVM_ADDRESS.test(spender.toLowerCase())) throw new Error('spender must be an EVM address');
  const amount = BigInt(amountAtoms);
  if (amount < 0n) throw new Error('amount must be nonnegative');
  return '0x095ea7b3' + pad32(spender) + amount.toString(16).padStart(64, '0');
}

export function encodeBalanceOf(owner) {
  if (!EVM_ADDRESS.test(owner.toLowerCase())) throw new Error('owner must be an EVM address');
  return '0x70a08231' + pad32(owner);
}

export async function getSepoliaUsdcBalance(evmAddress) {
  if (!hasMetaMask()) return 0n;
  const result = await window.ethereum.request({
    method: 'eth_call',
    params: [{ to: ETH_SEPOLIA_USDC, data: encodeBalanceOf(evmAddress) }, 'latest'],
  });
  return BigInt(result);
}

export async function approveSepoliaUsdc(spender, amountAtoms) {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  await ensureEthereumSepolia();
  const [from] = await window.ethereum.request({ method: 'eth_accounts' });
  return window.ethereum.request({
    method: 'eth_sendTransaction',
    params: [{ from, to: ETH_SEPOLIA_USDC, data: encodeApprove(spender, amountAtoms) }],
  });
}

export async function depositToSepoliaVault(vaultAddress, depositCalldata) {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  if (!EVM_ADDRESS.test(vaultAddress.toLowerCase())) {
    throw new Error('vault address must be an EVM address');
  }
  if (typeof depositCalldata !== 'string' || !depositCalldata.startsWith('0x')) {
    throw new Error('deposit calldata must be 0x-prefixed hex');
  }
  await ensureEthereumSepolia();
  const [from] = await window.ethereum.request({ method: 'eth_accounts' });
  return window.ethereum.request({
    method: 'eth_sendTransaction',
    params: [{ from, to: vaultAddress, data: depositCalldata }],
  });
}

export async function waitForSepoliaReceipt(txHash, timeoutMs = 300000, pollIntervalMs = 3000) {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const receipt = await window.ethereum.request({
        method: 'eth_getTransactionReceipt',
        params: [txHash],
      });
      if (receipt && receipt.blockNumber) {
        const status = Number.parseInt(receipt.status || '0x0', 16);
        if (status === 1) return receipt;
        throw new Error(`Transaction ${txHash} failed (status ${receipt.status})`);
      }
    } catch (error) {
      const message = error?.message || '';
      if (!message.includes('receipt') && !message.includes('not found')) throw error;
    }
    await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
  }
  throw new Error(`Transaction ${txHash} not confirmed within ${timeoutMs / 1000}s`);
}

/// Exact-atoms display: 6-decimal pfUSDC/USDC rendering with no float drift.
export function formatAtoms(atoms) {
  const negative = BigInt(atoms) < 0n;
  const absolute = negative ? -BigInt(atoms) : BigInt(atoms);
  const whole = absolute / 10n ** BigInt(PFUSDC_DECIMALS);
  const fraction = (absolute % 10n ** BigInt(PFUSDC_DECIMALS))
    .toString()
    .padStart(PFUSDC_DECIMALS, '0');
  return `${negative ? '-' : ''}${whole}.${fraction}`;
}

export function parseUsdcAtoms(display) {
  const text = String(display).trim();
  if (!/^\d+(\.\d{1,6})?$/.test(text)) throw new Error('amount must be a decimal with <=6 places');
  const [whole, fraction = ''] = text.split('.');
  return BigInt(whole) * 10n ** BigInt(PFUSDC_DECIMALS)
    + BigInt((fraction + '000000').slice(0, PFUSDC_DECIMALS));
}

/// Deterministic six-stage lane state machine. Transition order is fixed;
/// credit is spendable immediately because the Ethereum-finality path has no
/// escrowed lifecycle (lifecycle_release_required=false).
export function initialLaneState() {
  return {
    stage: 'deposit',
    completed: [],
    credit_state: null,
    lifecycle_release_required: LIFECYCLE_RELEASE_REQUIRED,
    lifecycle_release: 'not_applicable',
    proof_kind: ETH_FAST_LANE_PROOF_KIND,
    balances_atoms: {},
    provenance: [],
  };
}

export function laneTransition(state, event) {
  const expected = ETH_FAST_LANE_STAGES[state.completed.length];
  if (!event || event.stage !== expected) {
    throw new Error(`lane stage out of order: expected ${expected}, got ${event?.stage}`);
  }
  if (JSON.stringify(event).toLowerCase().includes('arbitrum')) {
    throw new Error('Arbitrum markers are outside the Ethereum L1 fast-lane scope');
  }
  const next = { ...state, completed: [...state.completed, event.stage] };
  next.stage = ETH_FAST_LANE_STAGES[next.completed.length] || 'done';
  if (event.stage === 'credit') {
    if (!Number.isSafeInteger(event.credited_atoms) || event.credited_atoms <= 0) {
      throw new Error('credit event must carry exact positive credited_atoms');
    }
    next.credit_state = 'spendable';
    next.credited_atoms = event.credited_atoms;
  }
  if (event.stage === 'transparent_send' || event.stage === 'orchard_send') {
    if (!event.debit_provenance || event.debit_provenance !== 'ingress_credit') {
      throw new Error(`${event.stage} must debit the newly credited ingress atoms`);
    }
    next.provenance = [...state.provenance, { stage: event.stage, source: 'ingress_credit' }];
  }
  if (event.stage === 'withdrawal') {
    if (!event.recipient || !EVM_ADDRESS.test(String(event.recipient).toLowerCase())) {
      throw new Error('withdrawal must name a valid recipient EVM address');
    }
    if (event.recipient.toLowerCase() === String(event.burn_source || '').toLowerCase()) {
      throw new Error('withdrawal recipient must differ from the burn/deposit source');
    }
    next.recipient = event.recipient;
  }
  if (event.balances_atoms) {
    next.balances_atoms = { ...state.balances_atoms, ...event.balances_atoms };
  }
  return next;
}

/// Conservation check: V = S + D + B - R over exact atom counts.
export function checkConservation({ vault_atoms, supply_atoms, deposit_atoms, burn_atoms, redeemed_atoms }) {
  const lhs = BigInt(vault_atoms);
  const rhs = BigInt(supply_atoms) + BigInt(deposit_atoms) + BigInt(burn_atoms) - BigInt(redeemed_atoms);
  return { ok: lhs === rhs, vault_atoms: lhs, expected: rhs };
}

export function isValidProofRef(ref) {
  return typeof ref === 'string' && HEX_32.test(ref);
}
