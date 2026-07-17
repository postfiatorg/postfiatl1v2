import assert from 'node:assert/strict';
import test from 'node:test';

import { defaultRpcEndpoint, encryptVault, normalizeRpcEndpoint } from './vault.js';

function withLocation(location, fn) {
  const previous = globalThis.window;
  Object.defineProperty(globalThis, 'window', {
    value: { location },
    configurable: true,
  });
  try {
    fn();
  } finally {
    if (previous === undefined) {
      delete globalThis.window;
    } else {
      Object.defineProperty(globalThis, 'window', {
        value: previous,
        configurable: true,
      });
    }
  }
}

test('defaultRpcEndpoint uses wallet proxy for local Vite dev server', () => {
  withLocation({
    protocol: 'http:',
    hostname: '127.0.0.1',
    host: '127.0.0.1:5173',
    port: '5173',
  }, () => {
    assert.equal(defaultRpcEndpoint(), 'ws://127.0.0.1:8080');
  });
});

test('defaultRpcEndpoint keeps remote host when Vite serves over a public address', () => {
  withLocation({
    protocol: 'http:',
    hostname: '192.0.2.1',
    host: '192.0.2.1:5173',
    port: '5173',
  }, () => {
  assert.equal(defaultRpcEndpoint(), 'ws://192.0.2.1:8080');
  });
});

test('defaultRpcEndpoint tunnels rpc through Vite over HTTPS on 5173', () => {
  withLocation({
    protocol: 'https:',
    hostname: '192.0.2.1',
    host: '192.0.2.1:5173',
    port: '5173',
  }, () => {
  assert.equal(defaultRpcEndpoint(), 'wss://192.0.2.1:5173/rpc');
  });
});

test('defaultRpcEndpoint uses same-origin rpc outside Vite dev mode', () => {
  withLocation({
    protocol: 'https:',
    hostname: 'wallet.postfiat.example',
    host: 'wallet.postfiat.example',
    port: '',
  }, () => {
    assert.equal(defaultRpcEndpoint(), 'wss://wallet.postfiat.example/rpc');
  });
});

test('normalizeRpcEndpoint migrates stale loopback endpoints on public HTTPS wallet', () => {
  withLocation({
    protocol: 'https:',
    hostname: '192.0.2.1',
    host: '192.0.2.1:5173',
    port: '5173',
  }, () => {
  assert.equal(normalizeRpcEndpoint('ws://localhost:8080'), 'wss://192.0.2.1:5173/rpc');
  assert.equal(normalizeRpcEndpoint('ws://127.0.0.1:8080'), 'wss://192.0.2.1:5173/rpc');
  });
});

test('normalizeRpcEndpoint keeps local proxy endpoint for local HTTP wallet', () => {
  withLocation({
    protocol: 'http:',
    hostname: '127.0.0.1',
    host: '127.0.0.1:5173',
    port: '5173',
  }, () => {
    assert.equal(normalizeRpcEndpoint('ws://localhost:8080'), 'ws://localhost:8080');
  });
});

test('encryptVault rejects clearly when crypto.subtle is unavailable (insecure origin)', async () => {
  const previousCrypto = globalThis.crypto;
  const previousWindow = globalThis.window;
  Object.defineProperty(globalThis, 'window', {
    value: { location: { protocol: 'http:', hostname: '192.0.2.1', host: '192.0.2.1:5173' } },
    configurable: true,
  });
  // Simulate a non-secure context: crypto exists (getRandomValues) but subtle is absent.
  Object.defineProperty(globalThis, 'crypto', {
    value: { getRandomValues: (arr) => { for (let i = 0; i < arr.length; i++) arr[i] = 0; return arr; } },
    configurable: true,
  });
  try {
    await assert.rejects(
      () => encryptVault('31'.repeat(32), 'passphrase-1234'),
      /secure context/,
    );
  } finally {
    if (previousCrypto === undefined) {
      delete globalThis.crypto;
    } else {
      Object.defineProperty(globalThis, 'crypto', { value: previousCrypto, configurable: true });
    }
    if (previousWindow === undefined) {
      delete globalThis.window;
    } else {
      Object.defineProperty(globalThis, 'window', { value: previousWindow, configurable: true });
    }
  }
});
