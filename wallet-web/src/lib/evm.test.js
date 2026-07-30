import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import test from 'node:test';

import {
  assertContractCodeHash,
  atomsToUsdc,
  approveEthereumUsdc,
  encodeBridgeDepositData,
  ensureEthereumMainnet,
  estimateEthereumApproveUsdcFee,
  getEthereumUsdcAllowance,
  generateNonce,
  governedRouteBinding,
  sha3_384DomainHex,
  usdcToAtoms,
} from './evm.js';
import { ETH_MAINNET_USDC } from './utils.js';

const sampleRecipient = 'pf1234567890abcdef1234567890abcdef12345678';
const sampleNonce = '0x' + 'ab'.repeat(32);
const sampleProfileHash = '11'.repeat(48);
const sampleRouteEpoch = 7;
const sampleRouteBinding = governedRouteBinding(sampleProfileHash, sampleRouteEpoch);

function wordsFromCalldata(data) {
  const body = data.slice(10);
  const words = [];
  for (let i = 0; i < body.length; i += 64) {
    words.push(body.slice(i, i + 64));
  }
  return words;
}

test('bridge deposit calldata uses route-bound depositV2 selector', () => {
  const data = encodeBridgeDepositData(1000000n, sampleRecipient, sampleNonce, sampleRouteBinding);
  assert.equal(data.slice(0, 10), '0x2391b457');
  assert.notEqual(data.slice(0, 10), '0x14b8b441');
  assert.notEqual(data.slice(0, 10), '0x6c7eca6d');
  assert.notEqual(data.slice(0, 10), '0xb6b55f25');
});

test('bridge deposit calldata ABI-encodes amount, string recipient, and nonce', () => {
  const data = encodeBridgeDepositData(1000000n, sampleRecipient, sampleNonce, sampleRouteBinding);
  const words = wordsFromCalldata(data);
  const recipientHex = Buffer.from(sampleRecipient, 'utf8').toString('hex');
  const paddedRecipientLength = Math.ceil(recipientHex.length / 64) * 64;

  assert.equal(words[0], 'f4240'.padStart(64, '0'));
  assert.equal(words[1], '80'.padStart(64, '0'));
  assert.equal(words[2], sampleNonce.slice(2));
  assert.equal(words[3], sampleRouteBinding.slice(2));
  assert.equal(words[4], sampleRecipient.length.toString(16).padStart(64, '0'));
  assert.equal(words.slice(5).join('').slice(0, paddedRecipientLength), recipientHex.padEnd(paddedRecipientLength, '0'));
  assert.equal(data.length, 10 + (4 * 64) + 64 + paddedRecipientLength);
});

test('governed route binding commits the exact SHA3-384 profile hash and u32 epoch', () => {
  assert.equal(sampleRouteBinding, '0xbceb5f7d7b32245250a394adb9f4a29c83e8806f805d6427caa4e055aa17473a');
  assert.notEqual(governedRouteBinding('12'.repeat(48), sampleRouteEpoch), sampleRouteBinding);
  assert.notEqual(governedRouteBinding(sampleProfileHash, sampleRouteEpoch + 1), sampleRouteBinding);
  assert.throws(() => governedRouteBinding(sampleProfileHash, 0), /positive u32/);
  assert.throws(() => governedRouteBinding('11'.repeat(32), sampleRouteEpoch), /exactly 48 bytes/);
});

test('browser SHA3-384 domain hash matches the platform implementation', () => {
  const domain = 'postfiat.vault_bridge.route_profile_hash.v1';
  const payload = 'schema=test\nroute_epoch=7\n';
  const expected = createHash('sha3-384')
    .update(domain)
    .update(Uint8Array.of(0))
    .update(payload)
    .digest('hex');
  assert.equal(sha3_384DomainHex(domain, payload), expected);
});

test('generateNonce returns a 32-byte hex nonce', () => {
  const previousCrypto = globalThis.crypto;
  Object.defineProperty(globalThis, 'crypto', {
    value: {
      getRandomValues(bytes) {
        for (let i = 0; i < bytes.length; i++) bytes[i] = i;
        return bytes;
      },
    },
    configurable: true,
  });
  try {
    const nonce = generateNonce();
    assert.match(nonce, /^0x[0-9a-f]{64}$/);
    assert.equal(
      nonce,
      '0x000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f',
    );
  } finally {
    if (previousCrypto === undefined) {
      delete globalThis.crypto;
    } else {
      Object.defineProperty(globalThis, 'crypto', { value: previousCrypto, configurable: true });
    }
  }
});

test('USDC conversion helpers use six decimals', () => {
  assert.equal(usdcToAtoms('1.5'), 1500000n);
  assert.equal(atomsToUsdc(1500000n), '1.500000');
});

test('Ethereum mainnet helpers switch chains and bind approval to canonical USDC', async () => {
  const previousWindow = globalThis.window;
  const owner = '0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0';
  const vault = '0xaaa78fda7062efce769e95cd72fc55e507bc8183';
  const calls = [];
  globalThis.window = {
    ethereum: {
      async request(request) {
        calls.push(request);
        if (request.method === 'eth_chainId') return '0xa4b1';
        if (request.method === 'eth_accounts') return [owner];
        if (request.method === 'eth_sendTransaction') return '0x' + '12'.repeat(32);
        return null;
      },
    },
  };
  try {
    await ensureEthereumMainnet();
    const txHash = await approveEthereumUsdc(vault, 1000000n);
    assert.equal(txHash, '0x' + '12'.repeat(32));
    assert.equal(calls.filter((call) => call.method === 'wallet_switchEthereumChain').length, 2);
    const send = calls.find((call) => call.method === 'eth_sendTransaction');
    assert.equal(send.params[0].from, owner);
    assert.equal(send.params[0].to, ETH_MAINNET_USDC);
    assert.match(send.params[0].data, /^0x095ea7b3/i);
  } finally {
    if (previousWindow === undefined) delete globalThis.window;
    else globalThis.window = previousWindow;
  }
});

test('Ethereum allowance and fee preflight use MetaMask on canonical USDC', async () => {
  const previousWindow = globalThis.window;
  const owner = '0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0';
  const vault = '0xaaa78fda7062efce769e95cd72fc55e507bc8183';
  const calls = [];
  globalThis.window = {
    ethereum: {
      async request(request) {
        calls.push(request);
        if (request.method === 'eth_chainId') return '0x1';
        if (request.method === 'eth_call') return '0xf4240';
        if (request.method === 'eth_estimateGas') return '0x5208';
        if (request.method === 'eth_gasPrice') return '0x3b9aca00';
        return null;
      },
    },
  };
  try {
    assert.equal(await getEthereumUsdcAllowance(owner, vault), 1000000n);
    const fee = await estimateEthereumApproveUsdcFee(vault, 1000000n, owner);
    assert.equal(fee.maxCostWei, 21000000000000n);
    const allowanceCall = calls.find((call) => call.method === 'eth_call');
    assert.equal(allowanceCall.params[0].to, ETH_MAINNET_USDC);
    const gasCall = calls.find((call) => call.method === 'eth_estimateGas');
    assert.equal(gasCall.params[0].to, ETH_MAINNET_USDC);
    assert.equal(gasCall.params[0].from, owner);
  } finally {
    if (previousWindow === undefined) delete globalThis.window;
    else globalThis.window = previousWindow;
  }
});

test('bridge contract preflight binds exact deployed bytecode hash', async () => {
  const previousWindow = globalThis.window;
  const calls = [];
  globalThis.window = {
    ethereum: {
      async request(request) {
        calls.push(request);
        return '0x6000';
      },
    },
  };
  try {
    await assertContractCodeHash(
      '0x1111111111111111111111111111111111111111',
      '0x07ad118d6cc8642c86c03827f276d8b791a65e5c99a3845faf186be720a1455d',
    );
    await assert.rejects(
      assertContractCodeHash(
        '0x1111111111111111111111111111111111111111',
        '0x' + '22'.repeat(32),
      ),
      /code hash mismatch/,
    );
    assert.equal(calls[0].method, 'eth_getCode');
  } finally {
    if (previousWindow === undefined) delete globalThis.window;
    else globalThis.window = previousWindow;
  }
});
