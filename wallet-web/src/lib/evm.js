// EVM integration via MetaMask (window.ethereum).
// Handles USDC approval and bridge vault deposit on Arbitrum.
// The wallet proxy relays confirmed vault deposits into PFTL.

import {
  ARBITRUM_CHAIN_ID,
  ARBITRUM_RPC,
  ARBITRUM_RPC_BROWSER,
  USDC_CONTRACT_ARBITRUM,
} from './utils.js';

// ERC20 ABI — minimal subset for approve + transfer + balanceOf
const ERC20_ABI = [
  { "constant": false, "inputs": [{ "name": "spender", "type": "address" }, { "name": "amount", "type": "uint256" }], "name": "approve", "outputs": [{ "name": "", "type": "bool" }], "type": "function" },
  { "constant": true, "inputs": [{ "name": "owner", "type": "address" }], "name": "balanceOf", "outputs": [{ "name": "", "type": "uint256" }], "type": "function" },
  { "constant": true, "inputs": [{ "name": "owner", "type": "address" }, { "name": "spender", "type": "address" }], "name": "allowance", "outputs": [{ "name": "", "type": "uint256" }], "type": "function" },
];

// USDC has 6 decimals
const USDC_DECIMALS = 6;
const WORD_HEX_LENGTH = 64;
const KECCAK_256_RATE_BYTES = 136;
const SHA3_384_RATE_BYTES = 104;
const MASK_64 = (1n << 64n) - 1n;
const DEPOSIT_SIGNATURE = 'depositV2(uint256,string,bytes32,bytes32)';
const DEPOSIT_EVENT_SIGNATURE = 'ERC20BridgeDepositedV2(bytes32,address,bytes32,string,uint256,bytes32,bytes32,uint256,address,address)';
const ROUTE_BINDING_DOMAIN = 'postfiat.vault_bridge.route_binding.v1';

const KECCAK_ROTATION_OFFSETS = [
  0, 1, 62, 28, 27,
  36, 44, 6, 55, 20,
  3, 10, 43, 25, 39,
  41, 45, 15, 21, 8,
  18, 2, 61, 56, 14,
];

const KECCAK_ROUND_CONSTANTS = [
  0x0000000000000001n, 0x0000000000008082n, 0x800000000000808an,
  0x8000000080008000n, 0x000000000000808bn, 0x0000000080000001n,
  0x8000000080008081n, 0x8000000000008009n, 0x000000000000008an,
  0x0000000000000088n, 0x0000000080008009n, 0x000000008000000an,
  0x000000008000808bn, 0x800000000000008bn, 0x8000000000008089n,
  0x8000000000008003n, 0x8000000000008002n, 0x8000000000000080n,
  0x000000000000800an, 0x800000008000000an, 0x8000000080008081n,
  0x8000000000008080n, 0x0000000080000001n, 0x8000000080008008n,
];

export function hasMetaMask() {
  return typeof window !== 'undefined' && typeof window.ethereum !== 'undefined';
}

export async function connectMetaMask() {
  if (!hasMetaMask()) {
    throw new Error('MetaMask not found. Please install MetaMask or enable the browser extension.');
  }
  try {
    const accounts = await window.ethereum.request({ method: 'eth_requestAccounts' });
    return accounts[0];
  } catch (e) {
    throw new Error('MetaMask connection rejected: ' + (e.message || 'unknown error'));
  }
}

export async function ensureArbitrum() {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  try {
    const chainId = await window.ethereum.request({ method: 'eth_chainId' });
    if (parseInt(chainId, 16) === ARBITRUM_CHAIN_ID) return;
    // Try to switch to Arbitrum
    try {
      await window.ethereum.request({
        method: 'wallet_switchEthereumChain',
        params: [{ chainId: '0x' + ARBITRUM_CHAIN_ID.toString(16) }],
      });
    } catch (switchError) {
      // If chain not added, add it
      if (switchError.code === 4902) {
        await window.ethereum.request({
          method: 'wallet_addEthereumChain',
          params: [{
            chainId: '0x' + ARBITRUM_CHAIN_ID.toString(16),
            chainName: 'Arbitrum One',
            nativeCurrency: { name: 'ETH', symbol: 'ETH', decimals: 18 },
            rpcUrls: [ARBITRUM_RPC],
            blockExplorerUrls: ['https://arbiscan.io'],
          }],
        });
      } else {
        throw switchError;
      }
    }
  } catch (e) {
    throw new Error('Could not switch to Arbitrum: ' + (e.message || 'unknown'));
  }
}

export async function getEvmBalance() {
  if (!hasMetaMask()) return null;
  const accounts = await window.ethereum.request({ method: 'eth_accounts' });
  if (!accounts.length) return null;
  return accounts[0];
}

// Encode ERC20 approve call
function encodeApprove(spender, amount) {
  // Function selector for approve(address,uint256) = 0x095ea7b3
  const selector = '0x095ea7b3';
  const paddedSpender = spender.toLowerCase().replace('0x', '').padStart(64, '0');
  const paddedAmount = BigInt(amount).toString(16).padStart(64, '0');
  return selector + paddedSpender + paddedAmount;
}

// Encode ERC20 balanceOf call
function encodeBalanceOf(owner) {
  const selector = '0x70a08231';
  const paddedOwner = owner.toLowerCase().replace('0x', '').padStart(64, '0');
  return selector + paddedOwner;
}

function encodeAllowance(owner, spender) {
  const selector = '0xdd62ed3e';
  const paddedOwner = owner.toLowerCase().replace('0x', '').padStart(64, '0');
  const paddedSpender = spender.toLowerCase().replace('0x', '').padStart(64, '0');
  return selector + paddedOwner + paddedSpender;
}

function utf8Bytes(value) {
  return new TextEncoder().encode(String(value));
}

function bytesToHex(bytes) {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

function normalizeHex(value) {
  if (typeof value !== 'string') throw new Error('Expected hex string');
  return value.toLowerCase().startsWith('0x') ? value.slice(2) : value;
}

function hexBytes(value, expectedBytes, label) {
  const hex = normalizeHex(value);
  if (!new RegExp(`^[0-9a-f]{${expectedBytes * 2}}$`, 'i').test(hex)) {
    throw new Error(`${label} must be exactly ${expectedBytes} bytes of hex`);
  }
  const bytes = new Uint8Array(expectedBytes);
  for (let i = 0; i < expectedBytes; i++) {
    bytes[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function padWord(hex) {
  if (hex.length > WORD_HEX_LENGTH) throw new Error('ABI word overflow');
  return hex.padStart(WORD_HEX_LENGTH, '0');
}

function encodeUint256(value) {
  const n = BigInt(value);
  if (n < 0n) throw new Error('uint256 cannot be negative');
  return padWord(n.toString(16));
}

function encodeBytes32(value) {
  const hex = normalizeHex(value);
  if (!/^[0-9a-f]*$/i.test(hex) || hex.length !== WORD_HEX_LENGTH) {
    throw new Error('bytes32 value must be a 0x-prefixed 32-byte hex string');
  }
  return hex.toLowerCase();
}

function encodeAddressFromWord(word) {
  const hex = normalizeHex(word).padStart(WORD_HEX_LENGTH, '0');
  return '0x' + hex.slice(-40);
}

function encodeAbiString(value) {
  const bytes = utf8Bytes(value);
  const length = encodeUint256(bytes.length);
  const dataHex = bytesToHex(bytes);
  const paddedLength = Math.ceil(dataHex.length / WORD_HEX_LENGTH) * WORD_HEX_LENGTH;
  return length + dataHex.padEnd(paddedLength, '0');
}

function decodeAbiString(dataHex, offsetBytes) {
  const offsetHex = offsetBytes * 2;
  const length = Number(BigInt('0x' + dataHex.slice(offsetHex, offsetHex + WORD_HEX_LENGTH)));
  const start = offsetHex + WORD_HEX_LENGTH;
  const bytes = new Uint8Array(length);
  const valueHex = dataHex.slice(start, start + length * 2);
  for (let i = 0; i < length; i++) {
    bytes[i] = Number.parseInt(valueHex.slice(i * 2, i * 2 + 2), 16);
  }
  return new TextDecoder().decode(bytes);
}

function rotateLeft64(value, offset) {
  if (offset === 0) return value & MASK_64;
  const shift = BigInt(offset);
  return ((value << shift) | (value >> (64n - shift))) & MASK_64;
}

function keccakF1600(state) {
  for (const roundConstant of KECCAK_ROUND_CONSTANTS) {
    const c = new Array(5).fill(0n);
    const d = new Array(5).fill(0n);
    const b = new Array(25).fill(0n);

    for (let x = 0; x < 5; x++) {
      c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
    }
    for (let x = 0; x < 5; x++) {
      d[x] = c[(x + 4) % 5] ^ rotateLeft64(c[(x + 1) % 5], 1);
    }
    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        state[x + 5 * y] = (state[x + 5 * y] ^ d[x]) & MASK_64;
      }
    }
    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        b[y + 5 * ((2 * x + 3 * y) % 5)] = rotateLeft64(
          state[x + 5 * y],
          KECCAK_ROTATION_OFFSETS[x + 5 * y],
        );
      }
    }
    for (let x = 0; x < 5; x++) {
      for (let y = 0; y < 5; y++) {
        state[x + 5 * y] = (b[x + 5 * y] ^ ((~b[((x + 1) % 5) + 5 * y]) & b[((x + 2) % 5) + 5 * y])) & MASK_64;
      }
    }
    state[0] = (state[0] ^ roundConstant) & MASK_64;
  }
}

function spongeHash(bytes, rateBytes, outputBytes, domainPadding) {
  const state = new Array(25).fill(0n);
  const block = new Uint8Array(rateBytes);
  let offset = 0;

  while (offset + rateBytes <= bytes.length) {
    for (let i = 0; i < rateBytes; i++) block[i] = bytes[offset + i];
    absorbBlock(state, block, rateBytes);
    keccakF1600(state);
    offset += rateBytes;
  }

  block.fill(0);
  block.set(bytes.slice(offset));
  block[bytes.length - offset] ^= domainPadding;
  block[rateBytes - 1] ^= 0x80;
  absorbBlock(state, block, rateBytes);
  keccakF1600(state);

  if (outputBytes > rateBytes) throw new Error('multi-block sponge output is unsupported');
  const out = new Uint8Array(outputBytes);
  for (let i = 0; i < outputBytes; i++) {
    const lane = state[Math.floor(i / 8)];
    out[i] = Number((lane >> BigInt(8 * (i % 8))) & 0xffn);
  }
  return out;
}

function absorbBlock(state, block, rateBytes) {
  for (let laneIndex = 0; laneIndex < rateBytes / 8; laneIndex++) {
    let lane = 0n;
    for (let byteIndex = 0; byteIndex < 8; byteIndex++) {
      lane |= BigInt(block[laneIndex * 8 + byteIndex]) << BigInt(8 * byteIndex);
    }
    state[laneIndex] = (state[laneIndex] ^ lane) & MASK_64;
  }
}

function keccak256Utf8(value) {
  return '0x' + bytesToHex(spongeHash(utf8Bytes(value), KECCAK_256_RATE_BYTES, 32, 0x01));
}

export function sha3_384DomainHex(domain, payload) {
  const domainBytes = utf8Bytes(domain);
  const payloadBytes = utf8Bytes(payload);
  const preimage = new Uint8Array(domainBytes.length + 1 + payloadBytes.length);
  preimage.set(domainBytes, 0);
  preimage[domainBytes.length] = 0;
  preimage.set(payloadBytes, domainBytes.length + 1);
  return bytesToHex(spongeHash(preimage, SHA3_384_RATE_BYTES, 48, 0x06));
}

export function governedRouteBinding(profileHash, routeEpoch) {
  if (!Number.isSafeInteger(routeEpoch) || routeEpoch <= 0 || routeEpoch > 0xffffffff) {
    throw new Error('routeEpoch must be a positive u32');
  }
  const domain = utf8Bytes(ROUTE_BINDING_DOMAIN);
  const profile = hexBytes(profileHash, 48, 'route profile hash');
  const preimage = new Uint8Array(domain.length + 1 + profile.length + 4);
  preimage.set(domain, 0);
  preimage[domain.length] = 0;
  preimage.set(profile, domain.length + 1);
  const epochOffset = domain.length + 1 + profile.length;
  preimage[epochOffset] = (routeEpoch >>> 24) & 0xff;
  preimage[epochOffset + 1] = (routeEpoch >>> 16) & 0xff;
  preimage[epochOffset + 2] = (routeEpoch >>> 8) & 0xff;
  preimage[epochOffset + 3] = routeEpoch & 0xff;
  return '0x' + bytesToHex(spongeHash(preimage, KECCAK_256_RATE_BYTES, 32, 0x01));
}

function hexDataBytes(value) {
  const text = String(value || '');
  const unprefixed = text.startsWith('0x') ? text.slice(2) : text;
  if (!unprefixed || unprefixed.length % 2 !== 0 || !/^[0-9a-fA-F]+$/.test(unprefixed)) {
    throw new Error('contract bytecode must be nonempty even-length hex');
  }
  const bytes = new Uint8Array(unprefixed.length / 2);
  for (let index = 0; index < bytes.length; index++) {
    bytes[index] = Number.parseInt(unprefixed.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
}

export async function assertContractCodeHash(contractAddress, expectedCodeHash) {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  if (!/^0x[0-9a-fA-F]{40}$/.test(String(contractAddress || ''))) {
    throw new Error('bridge vault address is invalid');
  }
  const expected = String(expectedCodeHash || '').toLowerCase();
  if (!/^0x[0-9a-f]{64}$/.test(expected)) {
    throw new Error('bridge vault code hash is not configured');
  }
  const code = await window.ethereum.request({
    method: 'eth_getCode',
    params: [contractAddress, 'latest'],
  });
  const actual = '0x' + bytesToHex(
    spongeHash(hexDataBytes(code), KECCAK_256_RATE_BYTES, 32, 0x01),
  );
  if (actual !== expected) {
    throw new Error(`bridge vault code hash mismatch: expected ${expected}, received ${actual}`);
  }
  return actual;
}

function functionSelector(signature) {
  return keccak256Utf8(signature).slice(0, 10);
}

export function encodeBridgeDepositData(amount, pftlRecipient, nonce, routeBinding) {
  if (!pftlRecipient || typeof pftlRecipient !== 'string') {
    throw new Error('pftlRecipient is required');
  }
  const selector = functionSelector(DEPOSIT_SIGNATURE);
  const head = [
    encodeUint256(amount),
    encodeUint256(128),
    encodeBytes32(nonce),
    encodeBytes32(routeBinding),
  ].join('');
  const tail = encodeAbiString(pftlRecipient);
  return selector + head + tail;
}

export function generateNonce() {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  return '0x' + bytesToHex(bytes);
}

export async function getUsdcBalance(evmAddress) {
  if (!hasMetaMask()) return 0n;
  const data = encodeBalanceOf(evmAddress);
  const result = await window.ethereum.request({
    method: 'eth_call',
    params: [
      { to: USDC_CONTRACT_ARBITRUM, data },
      'latest',
    ],
  });
  return BigInt(result);
}

export async function getArbitrumUsdcBalance(evmAddress) {
  const data = encodeBalanceOf(evmAddress);
  const response = await fetch(ARBITRUM_RPC_BROWSER, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method: 'eth_call',
      params: [
        { to: USDC_CONTRACT_ARBITRUM, data },
        'latest',
      ],
    }),
  });
  const payload = await response.json();
  if (!response.ok || payload.error) {
    throw new Error(payload.error?.message || `Arbitrum RPC failed: ${response.status}`);
  }
  return BigInt(payload.result || '0x0');
}

async function fetchArbitrumRpc(method, params = []) {
  const response = await fetch(ARBITRUM_RPC_BROWSER, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method,
      params,
    }),
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok || payload.error) {
    const error = new Error(payload.error?.message || `Arbitrum RPC ${method} failed: ${response.status}`);
    error.code = payload.error?.code;
    error.data = payload.error?.data;
    throw error;
  }
  return payload.result;
}

export async function getArbitrumEthBalance(evmAddress) {
  const result = await fetchArbitrumRpc('eth_getBalance', [evmAddress, 'latest']);
  return BigInt(result || '0x0');
}

export async function getArbitrumUsdcAllowance(owner, spender) {
  const result = await fetchArbitrumRpc('eth_call', [{
    to: USDC_CONTRACT_ARBITRUM,
    data: encodeAllowance(owner, spender),
  }, 'latest']);
  return BigInt(result || '0x0');
}

export async function estimateArbitrumTransactionFee({ from, to, data, value = '0x0' }) {
  const tx = { from, to, data, value };
  const [gasHex, gasPriceHex] = await Promise.all([
    fetchArbitrumRpc('eth_estimateGas', [tx]),
    fetchArbitrumRpc('eth_gasPrice', []),
  ]);
  const gas = BigInt(gasHex || '0x0');
  const gasPrice = BigInt(gasPriceHex || '0x0');
  return {
    gas,
    gasPrice,
    maxCostWei: gas * gasPrice,
  };
}

export async function estimateApproveUsdcFee(spender, amount, from) {
  return estimateArbitrumTransactionFee({
    from,
    to: USDC_CONTRACT_ARBITRUM,
    data: encodeApprove(spender, amount),
  });
}

export async function estimateBridgeDepositFee(vaultContract, amount, pftlRecipient, nonce, routeBinding, from) {
  return estimateArbitrumTransactionFee({
    from,
    to: vaultContract,
    data: encodeBridgeDepositData(amount, pftlRecipient, nonce, routeBinding),
  });
}

export async function waitForReceipt(txHash, timeoutMs = 120000, pollIntervalMs = 3000) {
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
    } catch (e) {
      const message = e?.message || '';
      if (!message.includes('receipt') && !message.includes('not found')) {
        throw e;
      }
    }
    await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
  }
  throw new Error(`Transaction ${txHash} not confirmed within ${timeoutMs / 1000}s`);
}

export async function approveUsdc(spender, amount) {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  await ensureArbitrum();
  const accounts = await window.ethereum.request({ method: 'eth_accounts' });
  const from = accounts[0];
  const data = encodeApprove(spender, amount);

  const tx = await window.ethereum.request({
    method: 'eth_sendTransaction',
    params: [{
      from,
      to: USDC_CONTRACT_ARBITRUM,
      data,
    }],
  });
  return tx;
}

export async function depositToBridge(vaultContract, amount, pftlRecipient, nonce, routeBinding) {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  await ensureArbitrum();
  const accounts = await window.ethereum.request({ method: 'eth_accounts' });
  const from = accounts[0];

  const data = encodeBridgeDepositData(amount, pftlRecipient, nonce, routeBinding);

  const tx = await window.ethereum.request({
    method: 'eth_sendTransaction',
    params: [{
      from,
      to: vaultContract,
      data,
    }],
  });
  return tx;
}

export async function watchDepositEvent(vaultContract, txHash, expectedRouteBinding) {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  const receipt = await window.ethereum.request({
    method: 'eth_getTransactionReceipt',
    params: [txHash],
  });
  if (!receipt || !Array.isArray(receipt.logs)) return null;

  const eventTopic = keccak256Utf8(DEPOSIT_EVENT_SIGNATURE);
  const vaultAddress = vaultContract.toLowerCase();
  for (const log of receipt.logs) {
    if (
      !log
      || String(log.address || '').toLowerCase() !== vaultAddress
      || !Array.isArray(log.topics)
      || String(log.topics[0] || '').toLowerCase() !== eventTopic
    ) {
      continue;
    }
    const parsed = parseDepositEventLog(log);
    if (String(parsed.route_binding).toLowerCase() !== String(expectedRouteBinding || '').toLowerCase()) {
      throw new Error('Vault deposit event route binding does not match the confirmed governed route');
    }
    return { tx_hash: txHash, ...parsed };
  }
  return null;
}

function parseDepositEventLog(log) {
  const dataHex = normalizeHex(log.data || '');
  const pftlRecipientOffset = Number(BigInt('0x' + dataHex.slice(0, WORD_HEX_LENGTH)));
  const amount = BigInt('0x' + dataHex.slice(WORD_HEX_LENGTH, WORD_HEX_LENGTH * 2));
  const nonce = '0x' + dataHex.slice(WORD_HEX_LENGTH * 2, WORD_HEX_LENGTH * 3);
  const routeBinding = '0x' + dataHex.slice(WORD_HEX_LENGTH * 3, WORD_HEX_LENGTH * 4);
  const sourceChainId = BigInt('0x' + dataHex.slice(WORD_HEX_LENGTH * 4, WORD_HEX_LENGTH * 5));
  const vault = encodeAddressFromWord(dataHex.slice(WORD_HEX_LENGTH * 5, WORD_HEX_LENGTH * 6));
  const token = encodeAddressFromWord(dataHex.slice(WORD_HEX_LENGTH * 6, WORD_HEX_LENGTH * 7));
  return {
    deposit_id: log.topics[1],
    depositor: encodeAddressFromWord(log.topics[2]),
    pftl_recipient_hash: log.topics[3],
    pftl_recipient: decodeAbiString(dataHex, pftlRecipientOffset),
    amount,
    nonce,
    route_binding: routeBinding,
    source_chain_id: sourceChainId,
    vault,
    token,
    log_index: log.logIndex,
    raw_log: log,
  };
}

export function usdcToAtoms(usdcAmount) {
  // USDC has 6 decimals
  const parts = String(usdcAmount).split('.');
  const whole = BigInt(parts[0] || '0');
  let frac = parts[1] || '';
  frac = frac.padEnd(USDC_DECIMALS, '0').slice(0, USDC_DECIMALS);
  return whole * BigInt(10 ** USDC_DECIMALS) + BigInt(frac || '0');
}

export function atomsToUsdc(atoms) {
  const n = BigInt(atoms);
  const whole = n / BigInt(10 ** USDC_DECIMALS);
  const frac = n % BigInt(10 ** USDC_DECIMALS);
  return `${whole}.${frac.toString().padStart(USDC_DECIMALS, '0')}`;
}
