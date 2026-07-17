import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import test from 'node:test';

import {
  assertContractCodeHash,
  atomsToUsdc,
  encodeBridgeDepositData,
  estimateApproveUsdcFee,
  estimateBridgeDepositFee,
  generateNonce,
  governedRouteBinding,
  getArbitrumEthBalance,
  getArbitrumUsdcAllowance,
  sha3_384DomainHex,
  usdcToAtoms,
} from './evm.js';
import { USDC_CONTRACT_ARBITRUM } from './utils.js';

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

test('getArbitrumEthBalance reads native ETH over the Arbitrum proxy', async () => {
  const owner = '0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0';
  const calls = [];
  const balance = await withFetch(async (_url, options) => {
    const request = JSON.parse(options.body);
    calls.push(request);
    return new Response(JSON.stringify({ jsonrpc: '2.0', id: request.id, result: '0x2a' }), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });
  }, () => getArbitrumEthBalance(owner));

  assert.equal(balance, 42n);
  assert.deepEqual(calls[0].params, [owner, 'latest']);
  assert.equal(calls[0].method, 'eth_getBalance');
});

test('getArbitrumUsdcAllowance reads allowance over the Arbitrum proxy', async () => {
  const owner = '0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0';
  const spender = '0x1A15e6103D6Af4e88924F748e13B829D3948DEa9';
  const calls = [];
  const allowance = await withFetch(async (_url, options) => {
    const request = JSON.parse(options.body);
    calls.push(request);
    return new Response(JSON.stringify({ jsonrpc: '2.0', id: request.id, result: '0xf4240' }), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });
  }, () => getArbitrumUsdcAllowance(owner, spender));

  assert.equal(allowance, 1000000n);
  assert.equal(calls[0].method, 'eth_call');
  assert.equal(calls[0].params[0].to, USDC_CONTRACT_ARBITRUM);
  assert.match(calls[0].params[0].data, /^0xdd62ed3e/i);
  assert.match(calls[0].params[0].data, new RegExp(owner.slice(2).toLowerCase(), 'i'));
  assert.match(calls[0].params[0].data, new RegExp(spender.slice(2).toLowerCase(), 'i'));
});

test('estimateApproveUsdcFee estimates approval gas before MetaMask submit', async () => {
  const owner = '0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0';
  const vault = '0x1A15e6103D6Af4e88924F748e13B829D3948DEa9';
  const calls = [];
  const fee = await withFetch(async (_url, options) => {
    const request = JSON.parse(options.body);
    calls.push(request);
    const result = request.method === 'eth_estimateGas' ? '0x5208' : '0x3b9aca00';
    return new Response(JSON.stringify({ jsonrpc: '2.0', id: request.id, result }), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });
  }, () => estimateApproveUsdcFee(vault, 1000000n, owner));

  assert.equal(fee.gas, 21000n);
  assert.equal(fee.gasPrice, 1000000000n);
  assert.equal(fee.maxCostWei, 21000000000000n);
  assert.equal(calls[0].method, 'eth_estimateGas');
  assert.equal(calls[0].params[0].from, owner);
  assert.equal(calls[0].params[0].to, USDC_CONTRACT_ARBITRUM);
  assert.match(calls[0].params[0].data, /^0x095ea7b3/i);
  assert.equal(calls[1].method, 'eth_gasPrice');
});

test('estimateBridgeDepositFee estimates vault deposit gas before MetaMask submit', async () => {
  const owner = '0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0';
  const vault = '0x1A15e6103D6Af4e88924F748e13B829D3948DEa9';
  const calls = [];
  const fee = await withFetch(async (_url, options) => {
    const request = JSON.parse(options.body);
    calls.push(request);
    const result = request.method === 'eth_estimateGas' ? '0x015f90' : '0x05f5e100';
    return new Response(JSON.stringify({ jsonrpc: '2.0', id: request.id, result }), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });
  }, () => estimateBridgeDepositFee(
    vault,
    1000000n,
    sampleRecipient,
    sampleNonce,
    sampleRouteBinding,
    owner,
  ));

  assert.equal(fee.gas, 90000n);
  assert.equal(fee.gasPrice, 100000000n);
  assert.equal(fee.maxCostWei, 9000000000000n);
  assert.equal(calls[0].method, 'eth_estimateGas');
  assert.equal(calls[0].params[0].from, owner);
  assert.equal(calls[0].params[0].to, vault);
  assert.match(calls[0].params[0].data, /^0x2391b457/i);
});
