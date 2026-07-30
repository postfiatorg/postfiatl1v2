// Utility functions for address validation, formatting, etc.

export function isValidAddress(addr) {
  if (typeof addr !== 'string') return false;
  if (!addr.startsWith('pf')) return false;
  const hex = addr.slice(2);
  if (hex.length !== 40) return false;
  return /^[0-9a-fA-F]+$/.test(hex);
}

// 1 PFT = 1,000,000 atoms (PFTL_PRECISION)
const PFTL_PRECISION = 1_000_000n;

export function formatBalance(raw) {
  if (raw === null || raw === undefined) return '0';
  const n = BigInt(raw);
  const pft = n / PFTL_PRECISION;
  const remainder = n % PFTL_PRECISION;
  if (remainder === 0n) {
    return pft.toLocaleString();
  }
  // Show fractional PFT (e.g. "1.5" for 1,500,000 atoms)
  const fracStr = remainder.toString().padStart(6, '0');
  const trimmed = fracStr.replace(/0+$/, '');
  return `${pft.toLocaleString()}.${trimmed}`;
}

export function formatAssetBalance(assetId, raw) {
  if (raw === null || raw === undefined) return '0';
  return formatBalance(raw);
}

// Convert PFT (human-readable) to atoms for RPC submission
export function pftToAtoms(pftValue) {
  const num = parseFloat(pftValue);
  if (isNaN(num) || num <= 0) return 0;
  return Math.round(num * 1_000_000);
}

// Convert atoms to PFT for display
export function atomsToPft(raw) {
  if (raw === null || raw === undefined) return 0;
  return Number(BigInt(raw)) / 1_000_000;
}

export function copyToClipboard(text) {
  navigator.clipboard.writeText(text).then(() => {
    showToast('Copied to clipboard');
  }).catch(() => {
    // Fallback
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    document.body.removeChild(ta);
    showToast('Copied to clipboard');
  });
}

let toastTimer = null;
function showToast(msg) {
  let toast = document.getElementById('copy-toast');
  if (!toast) {
    toast = document.createElement('div');
    toast.id = 'copy-toast';
    toast.className = 'copy-toast';
    document.body.appendChild(toast);
  }
  toast.textContent = msg;
  toast.style.display = 'block';
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { toast.style.display = 'none'; }, 2000);
}

export function truncateMiddle(str, len = 12) {
  if (str.length <= len * 2) return str;
  return str.slice(0, len) + '...' + str.slice(-len);
}

export function shortenAssetId(assetId) {
  if (!assetId || assetId.length < 20) return assetId;
  return assetId.slice(0, 8) + '...' + assetId.slice(-8);
}

// Current governed asset IDs on the WAN devnet.
export const PFUSDC_ASSET_ID = '02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b';
export const A666_ASSET_ID = '521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c';

const buildEnv = import.meta.env || {};
const processEnv = typeof process !== 'undefined' ? process.env || {} : {};

function chainEnv(name) {
  return processEnv[name] || buildEnv[`VITE_${name}`] || buildEnv[name] || '';
}

// Public wallet defaults must match the chain domain served by the WAN devnet.
// A wallet backup is domain-bound: signing fails closed when its chain_id does
// not exactly match the fee quote. Deployments for any other network must set
// VITE_POSTFIAT_CHAIN_ID and VITE_POSTFIAT_GENESIS_HASH explicitly.
export const CHAIN_ID = chainEnv('POSTFIAT_CHAIN_ID') || 'postfiat-wan-devnet-2';
export const GENESIS_HASH = chainEnv('POSTFIAT_GENESIS_HASH') || 'ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9';
export const LEGACY_CHAIN_IDS = Object.freeze(['postfiat-wan-devnet']);
export const PROTOCOL_VERSION = 1;
export const ACCOUNT_INDEX = 0;

// EVM chain constants for MetaMask bridge
export const ETH_MAINNET_CHAIN_ID = 1;
export const ETH_MAINNET_USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
// Browser-safe Ethereum RPC path, proxied through the local wallet server.
export const ETH_MAINNET_RPC_BROWSER = '/eth-rpc';
