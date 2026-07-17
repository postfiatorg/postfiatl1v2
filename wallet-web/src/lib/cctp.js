import { ensureArbitrum, hasMetaMask, usdcToAtoms, waitForReceipt } from './evm.js';
import { ARBITRUM_RPC_BROWSER, ETH_MAINNET_CHAIN_ID, ETH_MAINNET_USDC } from './utils.js';

const WORD_HEX_LENGTH = 64;
const KECCAK_RATE_BYTES = 136;
const MASK_64 = (1n << 64n) - 1n;

export const CCTP = {
  mainnet: {
    tokenMessenger: '0xBd3fa81B58Ba92a82136038B25aDec7066af3155',
    messageTransmitter: '0x0a992d191DEeC32aFe36203Ad87D7d289a738F81',
    domain: 0,
  },
  arbitrum: {
    tokenMessenger: '0x19330d10D9Cc8751218eaf51E8885D058642E08A',
    messageTransmitter: '0xC30362313FBBA5cf9163F0bb16a0e01f01A896ca',
    domain: 3,
    rpcProxy: ARBITRUM_RPC_BROWSER,
  },
};

export const CCTP_V2 = {
  mainnet: {
    tokenMessenger: '0x28b5a0e9C621a5BadaA536219b3a228C8168cf5d',
    messageTransmitter: '0x81D40F21F12A8F0E3252Bccb954D722d4c464B64',
    domain: 0,
  },
  arbitrum: {
    messageTransmitter: '0x81D40F21F12A8F0E3252Bccb954D722d4c464B64',
    domain: 3,
  },
};

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

function emit(onUpdate, step, data = {}) {
  if (typeof onUpdate === 'function') onUpdate(step, data);
}

function utf8Bytes(value) {
  return new TextEncoder().encode(String(value));
}

function bytesToHex(bytes) {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

function hexToBytes(value) {
  const hex = normalizeHex(value);
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function normalizeHex(value) {
  if (typeof value !== 'string') throw new Error('Expected hex string');
  const hex = value.toLowerCase().startsWith('0x') ? value.slice(2) : value;
  if (hex.length % 2 !== 0 || !/^[0-9a-f]*$/i.test(hex)) {
    throw new Error('Expected even-length hex string');
  }
  return hex.toLowerCase();
}

function normalizeAddress(address) {
  const hex = normalizeHex(address);
  if (hex.length !== 40) throw new Error('Expected 20-byte EVM address');
  return '0x' + hex;
}

export function normalizeTxHash(txHash) {
  const hex = normalizeHex(String(txHash || '').trim());
  if (hex.length !== WORD_HEX_LENGTH) throw new Error('Expected 32-byte transaction hash');
  return '0x' + hex;
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

function encodeUint32(value) {
  const n = BigInt(value);
  if (n < 0n || n > 0xffffffffn) throw new Error('uint32 out of range');
  return encodeUint256(n);
}

function encodeAddressWord(address) {
  return normalizeAddress(address).slice(2).padStart(WORD_HEX_LENGTH, '0');
}

function encodeBytes32(value) {
  const hex = normalizeHex(value);
  if (hex.length !== WORD_HEX_LENGTH) {
    throw new Error('bytes32 value must be 32 bytes');
  }
  return hex;
}

function encodeAbiBytesTail(value) {
  const hex = normalizeHex(value);
  const paddedLength = Math.ceil(hex.length / WORD_HEX_LENGTH) * WORD_HEX_LENGTH;
  return encodeUint256(hex.length / 2) + hex.padEnd(paddedLength, '0');
}

function rotateLeft64(value, offset) {
  if (offset === 0) return value & MASK_64;
  const shift = BigInt(offset);
  return ((value << shift) | (value >> (64n - shift))) & MASK_64;
}

function absorbBlock(state, block) {
  for (let laneIndex = 0; laneIndex < KECCAK_RATE_BYTES / 8; laneIndex++) {
    let lane = 0n;
    for (let byteIndex = 0; byteIndex < 8; byteIndex++) {
      lane |= BigInt(block[laneIndex * 8 + byteIndex]) << BigInt(8 * byteIndex);
    }
    state[laneIndex] = (state[laneIndex] ^ lane) & MASK_64;
  }
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

function spongeHash256(bytes) {
  const state = new Array(25).fill(0n);
  const block = new Uint8Array(KECCAK_RATE_BYTES);
  let offset = 0;

  while (offset + KECCAK_RATE_BYTES <= bytes.length) {
    for (let i = 0; i < KECCAK_RATE_BYTES; i++) block[i] = bytes[offset + i];
    absorbBlock(state, block);
    keccakF1600(state);
    offset += KECCAK_RATE_BYTES;
  }

  block.fill(0);
  block.set(bytes.slice(offset));
  block[bytes.length - offset] ^= 0x01;
  block[KECCAK_RATE_BYTES - 1] ^= 0x80;
  absorbBlock(state, block);
  keccakF1600(state);

  const out = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    const lane = state[Math.floor(i / 8)];
    out[i] = Number((lane >> BigInt(8 * (i % 8))) & 0xffn);
  }
  return out;
}

export function keccak256Hex(value) {
  return '0x' + bytesToHex(spongeHash256(hexToBytes(value)));
}

function keccak256Utf8(value) {
  return '0x' + bytesToHex(spongeHash256(utf8Bytes(value)));
}

function functionSelector(signature) {
  return keccak256Utf8(signature).slice(0, 10);
}

export const DEPOSIT_FOR_BURN_SELECTOR = functionSelector('depositForBurn(uint256,uint32,bytes32,address)');
export const V2_DEPOSIT_FOR_BURN_SELECTOR = functionSelector(
  'depositForBurn(uint256,uint32,bytes32,address,bytes32,uint256,uint32)',
);
export const RECEIVE_MESSAGE_SELECTOR = functionSelector('receiveMessage(bytes,bytes)');
export const APPROVE_SELECTOR = '0x095ea7b3';
export const MESSAGE_SENT_TOPIC = keccak256Utf8('MessageSent(bytes)');
export const DEPOSIT_FOR_BURN_TOPIC = keccak256Utf8(
  'DepositForBurn(uint64,address,uint256,address,bytes32,uint32,bytes32,bytes32)',
);
export const V2_DEPOSIT_FOR_BURN_TOPIC = keccak256Utf8(
  'DepositForBurn(address,uint256,address,bytes32,uint32,bytes32,bytes32,uint256,uint32,bytes)',
);
const ZERO_BYTES32 = '0x' + '0'.repeat(WORD_HEX_LENGTH);
const CIRCLE_API_BROWSER = '/circle';
const CIRCLE_API_NODE = 'https://iris-api.circle.com';
const CCTP_V2_FAST_FINALITY_THRESHOLD = 1000;

export function encodeMintRecipient(address) {
  return '0x' + '0'.repeat(24) + normalizeAddress(address).slice(2);
}

export function encodeApproveData(spender, amount) {
  return APPROVE_SELECTOR + encodeAddressWord(spender) + encodeUint256(amount);
}

export function encodeDepositForBurnData(amount, mintRecipient, burnToken = ETH_MAINNET_USDC) {
  return DEPOSIT_FOR_BURN_SELECTOR
    + encodeUint256(amount)
    + encodeUint32(CCTP.arbitrum.domain)
    + encodeBytes32(mintRecipient)
    + encodeAddressWord(burnToken);
}

export function encodeV2DepositForBurnData(
  amount,
  destinationDomain,
  mintRecipient,
  burnToken,
  destinationCaller,
  maxFee,
  minFinalityThreshold,
) {
  return V2_DEPOSIT_FOR_BURN_SELECTOR
    + encodeUint256(amount)
    + encodeUint32(destinationDomain)
    + encodeBytes32(mintRecipient)
    + encodeAddressWord(burnToken)
    + encodeBytes32(destinationCaller)
    + encodeUint256(maxFee)
    + encodeUint32(minFinalityThreshold);
}

export function encodeReceiveMessageData(messageBytes, attestationBytes) {
  const messageTail = encodeAbiBytesTail(messageBytes);
  const attestationTail = encodeAbiBytesTail(attestationBytes);
  const attestationOffset = 64 + messageTail.length / 2;
  return RECEIVE_MESSAGE_SELECTOR
    + encodeUint256(64)
    + encodeUint256(attestationOffset)
    + messageTail
    + attestationTail;
}

function decodeAbiBytes(data, offsetBytes) {
  const dataHex = normalizeHex(data);
  const lengthOffset = offsetBytes * 2;
  const lengthHex = dataHex.slice(lengthOffset, lengthOffset + WORD_HEX_LENGTH);
  if (lengthHex.length !== WORD_HEX_LENGTH) throw new Error('ABI bytes length word missing');
  const length = Number(BigInt('0x' + lengthHex));
  const start = lengthOffset + WORD_HEX_LENGTH;
  const end = start + length * 2;
  if (dataHex.length < end) throw new Error('ABI bytes payload truncated');
  return '0x' + dataHex.slice(start, end);
}

export function parseMessageSentLog(log) {
  if (!log || !Array.isArray(log.topics)) return null;
  if (String(log.topics[0] || '').toLowerCase() !== MESSAGE_SENT_TOPIC.toLowerCase()) {
    return null;
  }
  const dataHex = normalizeHex(log.data || '');
  const offsetHex = dataHex.slice(0, WORD_HEX_LENGTH);
  if (offsetHex.length !== WORD_HEX_LENGTH) throw new Error('MessageSent data offset missing');
  const offsetBytes = Number(BigInt('0x' + offsetHex));
  return decodeAbiBytes(dataHex, offsetBytes);
}

export function extractMessageSent(receipt) {
  if (!receipt || !Array.isArray(receipt.logs)) {
    throw new Error('Burn receipt has no logs');
  }
  for (const log of receipt.logs) {
    const message = parseMessageSentLog(log);
    if (message) return message;
  }
  throw new Error('MessageSent event not found in burn receipt');
}

function burnProtocolForReceipt(receipt) {
  if (!receipt || !Array.isArray(receipt.logs)) return CCTP_LOG_SOURCES[1];
  const burnLog = receipt.logs.find((log) => {
    const topic = String(log?.topics?.[0] || '').toLowerCase();
    return topic === V2_DEPOSIT_FOR_BURN_TOPIC.toLowerCase()
      || topic === DEPOSIT_FOR_BURN_TOPIC.toLowerCase();
  });
  if (String(burnLog?.topics?.[0] || '').toLowerCase() === V2_DEPOSIT_FOR_BURN_TOPIC.toLowerCase()) {
    return CCTP_LOG_SOURCES[0];
  }
  return CCTP_LOG_SOURCES[1];
}

function circleApiUrl(pathAndQuery) {
  const base = typeof window === 'undefined' ? CIRCLE_API_NODE : CIRCLE_API_BROWSER;
  return `${base}${pathAndQuery}`;
}

async function fetchCircleJson(pathAndQuery, context) {
  const response = await fetch(circleApiUrl(pathAndQuery));
  const payload = await response.json().catch(() => ({}));
  const errorMessage = payload.message || payload.error || '';
  if (response.status === 404 || String(errorMessage).toLowerCase().includes('not found')) {
    return { status: 'not_found', payload };
  }
  if (!response.ok) {
    throw new Error(errorMessage || `${context} failed: ${response.status}`);
  }
  return payload;
}

async function fetchEthereumRpc(method, params = []) {
  const response = await fetch('/eth-rpc/', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      method,
      params,
      id: 1,
    }),
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok || payload.error) {
    throw new Error(payload.error?.message || `Ethereum RPC ${method} failed: ${response.status}`);
  }
  return payload.result;
}

async function fetchEthereumTransactionReceipt(txHash) {
  const receipt = await fetchEthereumRpc('eth_getTransactionReceipt', [normalizeTxHash(txHash)]);
  return receipt || null;
}

function parseDepositForBurnLog(log) {
  const dataHex = normalizeHex(log.data || '');
  const words = [];
  for (let index = 0; index < 5; index++) {
    const word = dataHex.slice(index * WORD_HEX_LENGTH, (index + 1) * WORD_HEX_LENGTH);
    if (word.length !== WORD_HEX_LENGTH) {
      throw new Error('DepositForBurn log data is truncated');
    }
    words.push(word);
  }
  return {
    amount: BigInt('0x' + words[0]),
    mintRecipient: '0x' + words[1],
    destinationDomain: Number(BigInt('0x' + words[2])),
  };
}

function parseV2DepositForBurnLog(log) {
  const dataHex = normalizeHex(log.data || '');
  const words = [];
  for (let index = 0; index < 7; index++) {
    const word = dataHex.slice(index * WORD_HEX_LENGTH, (index + 1) * WORD_HEX_LENGTH);
    if (word.length !== WORD_HEX_LENGTH) {
      throw new Error('V2 DepositForBurn log data is truncated');
    }
    words.push(word);
  }
  return {
    amount: BigInt('0x' + words[0]),
    mintRecipient: '0x' + words[1],
    destinationDomain: Number(BigInt('0x' + words[2])),
    maxFee: BigInt('0x' + words[5]),
  };
}

function compareLogsNewestFirst(a, b) {
  const aLog = a.log || a;
  const bLog = b.log || b;
  const aBlock = BigInt(aLog.blockNumber || '0x0');
  const bBlock = BigInt(bLog.blockNumber || '0x0');
  if (aBlock !== bBlock) return aBlock > bBlock ? -1 : 1;
  const aIndex = BigInt(aLog.logIndex || '0x0');
  const bIndex = BigInt(bLog.logIndex || '0x0');
  if (aIndex === bIndex) return 0;
  return aIndex > bIndex ? -1 : 1;
}

const CCTP_LOG_SOURCES = [
  {
    version: 'v2',
    tokenMessenger: CCTP_V2.mainnet.tokenMessenger,
    eventTopic: V2_DEPOSIT_FOR_BURN_TOPIC,
    depositorTopicIndex: 2,
    destinationDomain: CCTP_V2.arbitrum.domain,
    messageTransmitter: CCTP_V2.arbitrum.messageTransmitter,
    parseLog: parseV2DepositForBurnLog,
  },
  {
    version: 'v1',
    tokenMessenger: CCTP.mainnet.tokenMessenger,
    eventTopic: DEPOSIT_FOR_BURN_TOPIC,
    depositorTopicIndex: 3,
    destinationDomain: CCTP.arbitrum.domain,
    messageTransmitter: CCTP.arbitrum.messageTransmitter,
    parseLog: parseDepositForBurnLog,
  },
];

function depositForBurnTopics(source, address) {
  const topics = [source.eventTopic];
  while (topics.length < source.depositorTopicIndex) topics.push(null);
  topics[source.depositorTopicIndex] = encodeMintRecipient(address);
  return topics;
}

async function fetchDepositForBurnLogs(address, blockWindow, source) {
  const currentBlockHex = await fetchEthereumRpc('eth_blockNumber');
  const currentBlock = Number(BigInt(currentBlockHex));
  const fromBlock = Math.max(0, currentBlock - blockWindow);
  return fetchEthereumRpc('eth_getLogs', [{
    fromBlock: '0x' + fromBlock.toString(16),
    toBlock: 'latest',
    address: source.tokenMessenger,
    topics: depositForBurnTopics(source, address),
  }]);
}

async function fetchDepositForBurnLogsChunked(address, blockWindow, source, chunkSize = 64) {
  const currentBlockHex = await fetchEthereumRpc('eth_blockNumber');
  const currentBlock = Number(BigInt(currentBlockHex));
  const oldestBlock = Math.max(0, currentBlock - blockWindow);
  const logs = [];

  for (let toBlock = currentBlock; toBlock >= oldestBlock; toBlock -= chunkSize) {
    const fromBlock = Math.max(oldestBlock, toBlock - chunkSize + 1);
    let chunk;
    try {
      chunk = await fetchEthereumRpc('eth_getLogs', [{
        fromBlock: '0x' + fromBlock.toString(16),
        toBlock: '0x' + toBlock.toString(16),
        address: source.tokenMessenger,
        topics: depositForBurnTopics(source, address),
      }]);
    } catch (_e) {
      break;
    }
    if (Array.isArray(chunk) && chunk.length > 0) {
      logs.push(...chunk);
      if (logs.length >= 10) break;
    }
  }
  return logs;
}

async function fetchAttestationStatus(messageHash) {
  const payload = await fetchCircleJson(`/v1/attestations/${messageHash}`, 'Circle attestation');
  if (payload.status === 'not_found') return payload;
  return {
    status: payload.status || 'pending',
    attestation: payload.attestation,
    payload,
  };
}

function parseDecimalUnits(value) {
  const text = String(value);
  if (!/^\d+(\.\d+)?$/.test(text)) throw new Error(`Invalid decimal value from Circle: ${text}`);
  const [whole, fraction = ''] = text.split('.');
  const scale = 10n ** BigInt(fraction.length);
  return {
    units: BigInt(whole) * scale + BigInt(fraction || '0'),
    scale,
  };
}

function ceilDiv(numerator, denominator) {
  return (numerator + denominator - 1n) / denominator;
}

export function feeBpsToAtoms(amountAtoms, minimumFeeBps) {
  const { units, scale } = parseDecimalUnits(minimumFeeBps);
  return ceilDiv(BigInt(amountAtoms) * units, 10000n * scale);
}

export async function fetchCctpV2FastFee({
  sourceDomain = CCTP_V2.mainnet.domain,
  destinationDomain = CCTP_V2.arbitrum.domain,
  finalityThreshold = CCTP_V2_FAST_FINALITY_THRESHOLD,
} = {}) {
  const payload = await fetchCircleJson(
    `/v2/burn/USDC/fees/${sourceDomain}/${destinationDomain}`,
    'Circle CCTP v2 fee lookup',
  );
  if (!Array.isArray(payload)) {
    throw new Error('Circle CCTP v2 fee lookup returned an unexpected payload');
  }
  const entry = payload.find(item => Number(item?.finalityThreshold) === Number(finalityThreshold));
  if (!entry || entry.minimumFee === undefined || entry.minimumFee === null) {
    throw new Error(`Circle CCTP v2 fee lookup did not return threshold ${finalityThreshold}`);
  }
  return {
    finalityThreshold: Number(entry.finalityThreshold),
    minimumFeeBps: String(entry.minimumFee),
    raw: entry,
  };
}

export async function fetchCctpV2FastAllowance() {
  const payload = await fetchCircleJson('/v2/fastBurn/USDC/allowance', 'Circle CCTP v2 fast allowance');
  if (payload.status === 'not_found') return null;
  return payload;
}

function firstCircleV2Message(payload) {
  if (!payload || payload.status === 'not_found') return null;
  if (Array.isArray(payload.messages)) return payload.messages[0] || null;
  if (Array.isArray(payload.message)) return payload.message[0] || null;
  if (payload.message || payload.attestation || payload.status) return payload;
  return null;
}

function normalizeCircleV2Message(payload) {
  const message = firstCircleV2Message(payload);
  if (!message) return { found: false, attestationStatus: 'not_found' };
  const attestation = message.attestation || payload.attestation || '';
  const messageBytes = message.message || payload.message || '';
  const status = String(message.status || payload.status || '').toLowerCase();
  const attestationReady = /^0x[0-9a-f]+$/i.test(attestation);
  return {
    found: true,
    message: messageBytes,
    messageHash: message.messageHash || payload.messageHash || (messageBytes ? keccak256Hex(messageBytes) : ''),
    attestation: attestationReady ? attestation : '',
    attestationStatus: attestationReady || status === 'complete' ? 'complete' : 'pending',
    eventNonce: message.eventNonce || payload.eventNonce || '',
    raw: payload,
  };
}

async function fetchCctpV2MessageStatus(burnTxHash) {
  const params = new URLSearchParams({ transactionHash: normalizeTxHash(burnTxHash) });
  const payload = await fetchCircleJson(
    `/v2/messages/${CCTP_V2.mainnet.domain}?${params}`,
    'Circle CCTP v2 message lookup',
  );
  return normalizeCircleV2Message(payload);
}

export async function pollCctpV2Message(
  burnTxHash,
  onUpdate,
  intervalMs = 5000,
  timeoutMs = 20 * 60 * 1000,
) {
  const normalizedBurnTxHash = normalizeTxHash(burnTxHash);
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const status = await fetchCctpV2MessageStatus(normalizedBurnTxHash);
    if (status.found && status.message) {
      emit(onUpdate, 'message_sent', {
        burnTxHash: normalizedBurnTxHash,
        messageHash: status.messageHash,
        messageBytes: status.message,
      });
    }
    if (status.attestationStatus === 'complete' && status.attestation && status.message) {
      emit(onUpdate, 'attestation_complete', {
        burnTxHash: normalizedBurnTxHash,
        messageHash: status.messageHash,
        attestation: status.attestation,
      });
      return status;
    }
    emit(onUpdate, 'attestation_pending', {
      burnTxHash: normalizedBurnTxHash,
      messageHash: status.messageHash || '',
      status: status.attestationStatus || 'pending',
    });
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }
  throw new Error(`Circle CCTP v2 attestation not complete within ${timeoutMs / 1000}s`);
}

export async function pollAttestation(
  messageHash,
  onUpdate,
  intervalMs = 15000,
  timeoutMs = 30 * 60 * 1000,
) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const attestationStatus = await fetchAttestationStatus(messageHash);
    if (attestationStatus.status === 'complete' && attestationStatus.attestation) {
      emit(onUpdate, 'attestation_complete', { messageHash, attestation: attestationStatus.attestation });
      return attestationStatus.attestation;
    }
    emit(onUpdate, 'attestation_pending', { messageHash, status: attestationStatus.status || 'pending' });
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }
  throw new Error(`Circle attestation not complete within ${timeoutMs / 1000}s`);
}

export async function getPendingCctpStatus(burnTxHash) {
  const normalizedBurnTxHash = normalizeTxHash(burnTxHash);
  const receipt = await fetchEthereumTransactionReceipt(normalizedBurnTxHash);
  if (!receipt) {
    return { found: false, attestationStatus: 'not_found' };
  }

  const burnProtocol = burnProtocolForReceipt(receipt);
  if (burnProtocol.version === 'v2') {
    return fetchCctpV2MessageStatus(normalizedBurnTxHash);
  }

  let messageBytes;
  try {
    messageBytes = extractMessageSent(receipt);
  } catch (_e) {
    return { found: false, attestationStatus: 'not_found' };
  }

  const messageHash = keccak256Hex(messageBytes);
  const attestationStatus = await fetchAttestationStatus(messageHash);
  if (attestationStatus.status === 'complete' && attestationStatus.attestation) {
    return {
      found: true,
      messageHash,
      attestationStatus: 'complete',
      attestation: attestationStatus.attestation,
    };
  }
  return {
    found: true,
    messageHash,
    attestationStatus: attestationStatus.status === 'not_found' ? 'not_found' : 'pending',
  };
}

export async function detectPendingCctpBurns(address) {
  const logs = [];
  for (const source of CCTP_LOG_SOURCES) {
    let sourceLogs = null;
    let lastError = null;
    for (const blockWindow of [50000, 20000, 5000, 1000]) {
      try {
        sourceLogs = await fetchDepositForBurnLogs(address, blockWindow, source);
        break;
      } catch (e) {
        lastError = e;
      }
    }
    if (!sourceLogs) {
      sourceLogs = await fetchDepositForBurnLogsChunked(address, 5000, source).catch((e) => {
        if (source.version === 'v1') throw lastError || e;
        return [];
      });
    }
    if (Array.isArray(sourceLogs) && sourceLogs.length > 0) {
      logs.push(...sourceLogs.map((log) => ({ log, source })));
    }
  }
  if (!Array.isArray(logs) || logs.length === 0) return [];

  const results = [];
  const sortedLogs = [...logs].sort(compareLogsNewestFirst);
  for (const entry of sortedLogs) {
    const { log, source } = entry;
    let parsed;
    try {
      parsed = source.parseLog(log);
    } catch (_e) {
      continue;
    }
    if (parsed.destinationDomain !== source.destinationDomain) continue;

    let burnTxHash;
    try {
      burnTxHash = normalizeTxHash(log.transactionHash);
    } catch (_e) {
      continue;
    }

    let message;
    let messageHash;
    try {
      const receipt = await fetchEthereumTransactionReceipt(burnTxHash);
      message = extractMessageSent(receipt);
      messageHash = keccak256Hex(message);
    } catch (_e) {
      continue;
    }

    const attestation = source.version === 'v2'
      ? await fetchCctpV2MessageStatus(burnTxHash)
      : await fetchAttestationStatus(messageHash);
    if (attestation.status === 'not_found' || attestation.found === false) continue;

    const result = {
      burnTxHash,
      amount: parsed.amount.toString(),
      destinationDomain: parsed.destinationDomain,
      attestationStatus: attestation.attestationStatus === 'complete' || (attestation.status === 'complete' && attestation.attestation) ? 'complete' : 'pending',
      messageHash: attestation.messageHash || messageHash,
      message: attestation.message || message,
    };
    result.version = source.version;
    result.messageTransmitter = source.messageTransmitter;
    if ((attestation.attestationStatus === 'complete' || attestation.status === 'complete') && attestation.attestation) {
      result.attestation = attestation.attestation;
    }
    results.push(result);
    if (results.length >= 5) break;
  }
  return results;
}

function normalizeAmountAtoms(amount) {
  if (typeof amount === 'bigint') return amount;
  if (typeof amount === 'number' && !Number.isInteger(amount)) return usdcToAtoms(amount);
  if (typeof amount === 'string' && amount.includes('.')) return usdcToAtoms(amount);
  return BigInt(amount);
}

async function ensureEthereumMainnet() {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  const chainId = await window.ethereum.request({ method: 'eth_chainId' });
  if (Number.parseInt(chainId, 16) === ETH_MAINNET_CHAIN_ID) return;
  await window.ethereum.request({
    method: 'wallet_switchEthereumChain',
    params: [{ chainId: '0x' + ETH_MAINNET_CHAIN_ID.toString(16) }],
  });
}

async function sendEvmTransaction(from, to, data) {
  return window.ethereum.request({
    method: 'eth_sendTransaction',
    params: [{ from, to, data }],
  });
}

export async function resumeCctpBridge({ burnTxHash, fromAddress, onUpdate }) {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  const from = normalizeAddress(fromAddress);
  const normalizedBurnTxHash = normalizeTxHash(burnTxHash);

  emit(onUpdate, 'fetching-receipt', { burnTxHash: normalizedBurnTxHash });
  const burnReceipt = await fetchEthereumTransactionReceipt(normalizedBurnTxHash);
  if (!burnReceipt) {
    throw new Error(`Burn transaction ${normalizedBurnTxHash} receipt not found on Ethereum`);
  }

  emit(onUpdate, 'extracting-message', { burnTxHash: normalizedBurnTxHash });
  const burnProtocol = burnProtocolForReceipt(burnReceipt);
  const receiptMessageBytes = extractMessageSent(burnReceipt);
  const receiptMessageHash = keccak256Hex(receiptMessageBytes);

  emit(onUpdate, 'fetching-attestation', { messageHash: receiptMessageHash });
  const attestationStatus = burnProtocol.version === 'v2'
    ? await pollCctpV2Message(normalizedBurnTxHash, onUpdate)
    : await fetchAttestationStatus(receiptMessageHash);
  const attestation = burnProtocol.version === 'v2'
    ? attestationStatus.attestation
    : attestationStatus.status === 'complete' && attestationStatus.attestation
      ? attestationStatus.attestation
      : await pollAttestation(receiptMessageHash, onUpdate);
  const messageBytes = burnProtocol.version === 'v2'
    ? attestationStatus.message || receiptMessageBytes
    : receiptMessageBytes;
  const messageHash = burnProtocol.version === 'v2'
    ? attestationStatus.messageHash || receiptMessageHash
    : receiptMessageHash;

  await ensureArbitrum();
  emit(onUpdate, 'minting', { messageHash });
  const mintTxHash = await sendEvmTransaction(
    from,
    burnProtocol.messageTransmitter,
    encodeReceiveMessageData(messageBytes, attestation),
  );
  const mintReceipt = await waitForReceipt(mintTxHash);
  emit(onUpdate, 'done', { mintTxHash, receipt: mintReceipt });

  return {
    messageHash,
    attestation,
    mintTxHash,
    message_bytes: messageBytes,
    version: burnProtocol.version,
  };
}

export async function cctpBridgeUsdcV2({ amount, fromAddress, onUpdate }) {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  const from = normalizeAddress(fromAddress);
  const amountAtoms = normalizeAmountAtoms(amount);
  const mintRecipient = encodeMintRecipient(from);
  const feeQuote = await fetchCctpV2FastFee();
  const maxFee = feeBpsToAtoms(amountAtoms, feeQuote.minimumFeeBps);
  if (maxFee >= amountAtoms) {
    throw new Error('CCTP Fast Transfer fee is greater than or equal to the bridge amount.');
  }
  const minFinalityThreshold = feeQuote.finalityThreshold;

  await ensureEthereumMainnet();

  emit(onUpdate, 'approving', {
    amount: amountAtoms.toString(),
    spender: CCTP_V2.mainnet.tokenMessenger,
  });
  const approveTxHash = await sendEvmTransaction(
    from,
    ETH_MAINNET_USDC,
    encodeApproveData(CCTP_V2.mainnet.tokenMessenger, amountAtoms),
  );
  const approveReceipt = await waitForReceipt(approveTxHash);

  emit(onUpdate, 'burning', {
    amount: amountAtoms.toString(),
    approveTxHash,
    approveReceipt,
    destinationDomain: CCTP_V2.arbitrum.domain,
    maxFee: maxFee.toString(),
    minimumFeeBps: feeQuote.minimumFeeBps,
    minFinalityThreshold,
    mintRecipient,
  });
  const burnTxHash = await sendEvmTransaction(
    from,
    CCTP_V2.mainnet.tokenMessenger,
    encodeV2DepositForBurnData(
      amountAtoms,
      CCTP_V2.arbitrum.domain,
      mintRecipient,
      ETH_MAINNET_USDC,
      ZERO_BYTES32,
      maxFee,
      minFinalityThreshold,
    ),
  );
  const burnReceipt = await waitForReceipt(burnTxHash);

  emit(onUpdate, 'attesting', { burnTxHash });

  const messageStatus = await pollCctpV2Message(burnTxHash, onUpdate);
  const messageBytes = messageStatus.message || extractMessageSent(burnReceipt);
  const messageHash = messageStatus.messageHash || keccak256Hex(messageBytes);
  const attestation = messageStatus.attestation;

  await ensureArbitrum();
  emit(onUpdate, 'minting', { messageHash });
  const mintTxHash = await sendEvmTransaction(
    from,
    CCTP_V2.arbitrum.messageTransmitter,
    encodeReceiveMessageData(messageBytes, attestation),
  );
  const mintReceipt = await waitForReceipt(mintTxHash);
  emit(onUpdate, 'done', { mintTxHash, receipt: mintReceipt });

  return {
    burnTxHash,
    messageHash,
    attestation,
    mintTxHash,
  };
}

export async function cctpBridgeUsdc({ amount, fromAddress, onUpdate }) {
  if (!hasMetaMask()) throw new Error('MetaMask not found');
  const from = normalizeAddress(fromAddress);
  const amountAtoms = normalizeAmountAtoms(amount);
  const mintRecipient = encodeMintRecipient(from);

  await ensureEthereumMainnet();

  emit(onUpdate, 'approve_start', {
    amount: amountAtoms.toString(),
    spender: CCTP.mainnet.tokenMessenger,
  });
  const approveTxHash = await sendEvmTransaction(
    from,
    ETH_MAINNET_USDC,
    encodeApproveData(CCTP.mainnet.tokenMessenger, amountAtoms),
  );
  emit(onUpdate, 'approve_submitted', { txHash: approveTxHash });
  const approveReceipt = await waitForReceipt(approveTxHash);
  emit(onUpdate, 'approve_confirmed', { txHash: approveTxHash, receipt: approveReceipt });

  emit(onUpdate, 'burn_start', {
    amount: amountAtoms.toString(),
    destinationDomain: CCTP.arbitrum.domain,
    mintRecipient,
  });
  const burnTxHash = await sendEvmTransaction(
    from,
    CCTP.mainnet.tokenMessenger,
    encodeDepositForBurnData(amountAtoms, mintRecipient, ETH_MAINNET_USDC),
  );
  emit(onUpdate, 'burn_submitted', { txHash: burnTxHash });
  const burnReceipt = await waitForReceipt(burnTxHash);
  emit(onUpdate, 'burn_confirmed', { txHash: burnTxHash, receipt: burnReceipt });

  const messageBytes = extractMessageSent(burnReceipt);
  const messageHash = keccak256Hex(messageBytes);
  emit(onUpdate, 'message_sent', { burnTxHash, messageHash, messageBytes });

  const attestation = await pollAttestation(messageHash, onUpdate);

  await ensureArbitrum();
  emit(onUpdate, 'mint_start', { messageHash });
  const mintTxHash = await sendEvmTransaction(
    from,
    CCTP.arbitrum.messageTransmitter,
    encodeReceiveMessageData(messageBytes, attestation),
  );
  emit(onUpdate, 'mint_submitted', { txHash: mintTxHash });

  return {
    burnTxHash,
    messageHash,
    attestation,
    mintTxHash,
  };
}
