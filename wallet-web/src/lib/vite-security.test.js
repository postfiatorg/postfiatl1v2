import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import viteConfig from '../../vite.config.js';

test('wallet development and preview servers are loopback-only by default', () => {
  assert.equal(viteConfig.server?.host, '127.0.0.1');
  assert.equal(viteConfig.server?.strictPort, true);
  assert.equal(viteConfig.preview?.host, '127.0.0.1');
  assert.equal(viteConfig.preview?.strictPort, true);
});

test('production CSP does not grant arbitrary websocket origins', () => {
  const csp = viteConfig.preview?.headers?.['Content-Security-Policy'];
  assert.equal(typeof csp, 'string');
  assert.doesNotMatch(csp, /(?:^|\s)ws:(?:\s|;|$)/);
  assert.doesNotMatch(csp, /(?:^|\s)wss:(?:\s|;|$)/);
});

test('wallet settings cannot redirect the bridge money destination', () => {
  const moreSource = readFileSync(new URL('../components/More.jsx', import.meta.url), 'utf8');
  assert.doesNotMatch(moreSource, /bridgeVaultAddr/);
  assert.doesNotMatch(moreSource, /Bridge vault contract[^.]*<input/);
});

test('bridge destination is chain-discovered and both contracts are verified before money moves', () => {
  const bridgeSource = readFileSync(new URL('../components/Bridge.jsx', import.meta.url), 'utf8');
  const appSource = readFileSync(new URL('../App.jsx', import.meta.url), 'utf8');
  assert.match(bridgeSource, /loadGovernedVaultBridgeRoute/);
  assert.equal((bridgeSource.match(/assertContractCodeHash\(activeVault, activeRoute\.vaultRuntimeCodeHash\)/g) || []).length, 2);
  assert.equal((bridgeSource.match(/assertContractCodeHash\(activeRoute\.tokenAddress, activeRoute\.tokenRuntimeCodeHash\)/g) || []).length, 2);
  assert.doesNotMatch(appSource, /BRIDGE_VAULT_(?:CONTRACT|CODE_HASH)/);
});
