import assert from 'node:assert/strict';
import test from 'node:test';

import {
  CCTP_V2,
  DEPOSIT_FOR_BURN_SELECTOR,
  MESSAGE_SENT_TOPIC,
  RECEIVE_MESSAGE_SELECTOR,
  V2_DEPOSIT_FOR_BURN_SELECTOR,
  V2_DEPOSIT_FOR_BURN_TOPIC,
  encodeDepositForBurnData,
  encodeMintRecipient,
  encodeReceiveMessageData,
  encodeV2DepositForBurnData,
  extractMessageSent,
  feeBpsToAtoms,
  fetchCctpV2FastFee,
  normalizeTxHash,
  pollAttestation,
  pollCctpV2Message,
} from './cctp.js';
import { ETH_MAINNET_USDC } from './utils.js';

function word(value) {
  return BigInt(value).toString(16).padStart(64, '0');
}

function abiEncodedBytes(hexValue) {
  const hex = hexValue.replace(/^0x/i, '');
  return '0x' + word(32) + word(hex.length / 2) + hex.padEnd(Math.ceil(hex.length / 64) * 64, '0');
}

function withFetch(fetchImpl, fn) {
  const previousFetch = globalThis.fetch;
  Object.defineProperty(globalThis, 'fetch', {
    value: fetchImpl,
    configurable: true,
  });
  return Promise.resolve()
    .then(fn)
    .finally(() => {
      if (previousFetch === undefined) {
        delete globalThis.fetch;
      } else {
        Object.defineProperty(globalThis, 'fetch', { value: previousFetch, configurable: true });
      }
    });
}

test('CCTP ABI selectors and MessageSent topic are correct', () => {
  assert.equal(DEPOSIT_FOR_BURN_SELECTOR, '0x6fd3504e');
  assert.equal(V2_DEPOSIT_FOR_BURN_SELECTOR, '0x8e0250ee');
  assert.equal(RECEIVE_MESSAGE_SELECTOR, '0x57ecfd28');
  assert.equal(MESSAGE_SENT_TOPIC, '0x8c5261668696ce22758910d05bab8f186d6eb247ceac2af2e82c7dc17669b036');
  assert.equal(V2_DEPOSIT_FOR_BURN_TOPIC, '0x0c8c1cbdc5190613ebd485511d4e2812cfa45eecb79d845893331fedad5130a5');
});

test('CCTP v2 mainnet addresses match Circle production contracts', () => {
  assert.equal(CCTP_V2.mainnet.domain, 0);
  assert.equal(CCTP_V2.arbitrum.domain, 3);
  assert.equal(CCTP_V2.mainnet.tokenMessenger, '0x28b5a0e9C621a5BadaA536219b3a228C8168cf5d');
  assert.equal(CCTP_V2.mainnet.messageTransmitter, '0x81D40F21F12A8F0E3252Bccb954D722d4c464B64');
  assert.equal(CCTP_V2.arbitrum.messageTransmitter, '0x81D40F21F12A8F0E3252Bccb954D722d4c464B64');
});

test('encodeMintRecipient pads an address to bytes32', () => {
  const address = '0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0';
  assert.equal(
    encodeMintRecipient(address),
    '0x0000000000000000000000001455bd7fbfbf92a171ef36025e13959e3b0ad8c0',
  );
});

test('normalizeTxHash accepts bare and prefixed transaction hashes', () => {
  const bareHash = 'Aa'.repeat(32);
  assert.equal(normalizeTxHash(bareHash), `0x${bareHash.toLowerCase()}`);
  assert.equal(normalizeTxHash(`0x${bareHash}`), `0x${bareHash.toLowerCase()}`);
  assert.throws(() => normalizeTxHash('0x1234'), /32-byte transaction hash/);
});

test('encodeDepositForBurnData encodes static CCTP burn arguments', () => {
  const mintRecipient = encodeMintRecipient('0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0');
  const data = encodeDepositForBurnData(1000000n, mintRecipient, ETH_MAINNET_USDC);
  assert.equal(data.slice(0, 10), DEPOSIT_FOR_BURN_SELECTOR);
  assert.equal(data.slice(10, 74), word(1000000));
  assert.equal(data.slice(74, 138), word(3));
  assert.equal(data.slice(138, 202), mintRecipient.slice(2));
  assert.equal(data.slice(202, 266), ETH_MAINNET_USDC.slice(2).padStart(64, '0'));
});

test('encodeV2DepositForBurnData encodes fast transfer fee and threshold', () => {
  const mintRecipient = encodeMintRecipient('0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0');
  const data = encodeV2DepositForBurnData(
    1000000n,
    3,
    mintRecipient,
    ETH_MAINNET_USDC,
    '0x' + '00'.repeat(32),
    100n,
    1000,
  );
  assert.equal(data.slice(0, 10), V2_DEPOSIT_FOR_BURN_SELECTOR);
  assert.equal(data.slice(10, 74), word(1000000));
  assert.equal(data.slice(74, 138), word(3));
  assert.equal(data.slice(138, 202), mintRecipient.slice(2));
  assert.equal(data.slice(202, 266), ETH_MAINNET_USDC.slice(2).padStart(64, '0'));
  assert.equal(data.slice(266, 330), '0'.repeat(64));
  assert.equal(data.slice(330, 394), word(100));
  assert.equal(data.slice(394, 458), word(1000));
});

test('encodeReceiveMessageData encodes two dynamic bytes arguments', () => {
  const data = encodeReceiveMessageData('0x12345678', '0xabcdef');
  assert.equal(data.slice(0, 10), RECEIVE_MESSAGE_SELECTOR);
  assert.equal(data.slice(10, 74), word(64));
  assert.equal(data.slice(74, 138), word(128));
  assert.equal(data.slice(138, 202), word(4));
  assert.equal(data.slice(202, 266), '12345678'.padEnd(64, '0'));
  assert.equal(data.slice(266, 330), word(3));
  assert.equal(data.slice(330, 394), 'abcdef'.padEnd(64, '0'));
});

test('extractMessageSent decodes ABI-encoded MessageSent bytes', () => {
  const message = '0x1234567890abcdef';
  const receipt = {
    logs: [
      { topics: ['0xdeadbeef'], data: '0x' },
      { topics: [MESSAGE_SENT_TOPIC], data: abiEncodedBytes(message) },
    ],
  };

  assert.equal(extractMessageSent(receipt), message);
});

test('pollAttestation returns the completed Circle attestation', async () => {
  const messageHash = '0x' + '11'.repeat(32);
  const requestedUrls = [];
  const updates = [];
  let calls = 0;

  const attestation = await withFetch(async (url) => {
    requestedUrls.push(String(url));
    calls += 1;
    if (calls === 1) {
      return new Response(JSON.stringify({ status: 'pending' }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      });
    }
    return new Response(JSON.stringify({ status: 'complete', attestation: '0xabc123' }), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });
  }, () => pollAttestation(messageHash, (step, data) => updates.push({ step, data }), 0, 1000));

  assert.equal(attestation, '0xabc123');
  assert.deepEqual(requestedUrls, [
    `https://iris-api.circle.com/v1/attestations/${messageHash}`,
    `https://iris-api.circle.com/v1/attestations/${messageHash}`,
  ]);
  assert.equal(updates[0].step, 'attestation_pending');
  assert.equal(updates[1].step, 'attestation_complete');
});

test('feeBpsToAtoms converts Circle v2 fee bps to USDC atoms with ceiling', () => {
  assert.equal(feeBpsToAtoms(1000000n, 1), 100n);
  assert.equal(feeBpsToAtoms(1n, 1), 1n);
  assert.equal(feeBpsToAtoms(40000000n, '1'), 4000n);
  assert.equal(feeBpsToAtoms(1000000n, '0.5'), 50n);
});

test('fetchCctpV2FastFee reads Circle v2 fast fee shape', async () => {
  const fee = await fetchCctpV2FastFee();
  assert.equal(fee.finalityThreshold, 1000);
  assert.match(fee.minimumFeeBps, /^\d+(\.\d+)?$/);
});

test('pollCctpV2Message reads message and attestation by burn tx hash', async () => {
  const bareBurnTxHash = 'aa'.repeat(32);
  const normalizedBurnTxHash = `0x${bareBurnTxHash}`;
  const message = '0x1234567890abcdef';
  const attestation = '0x' + 'bb'.repeat(65);
  const requestedUrls = [];
  const updates = [];
  let calls = 0;

  const status = await withFetch(async (url) => {
    requestedUrls.push(String(url));
    calls += 1;
    const body = calls === 1
      ? { messages: [{ message, attestation: 'PENDING', status: 'pending' }] }
      : { messages: [{ message, attestation, status: 'complete' }] };
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });
  }, () => pollCctpV2Message(bareBurnTxHash, (step, data) => updates.push({ step, data }), 0, 1000));

  assert.equal(status.message, message);
  assert.equal(status.attestation, attestation);
  assert.deepEqual(requestedUrls, [
    `https://iris-api.circle.com/v2/messages/0?transactionHash=${normalizedBurnTxHash}`,
    `https://iris-api.circle.com/v2/messages/0?transactionHash=${normalizedBurnTxHash}`,
  ]);
  assert.equal(updates[0].data.burnTxHash, normalizedBurnTxHash);
  assert.equal(updates[0].step, 'message_sent');
  assert.equal(updates[1].step, 'attestation_pending');
  assert.equal(updates.at(-1).step, 'attestation_complete');
});
