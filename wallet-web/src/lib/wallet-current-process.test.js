import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const SRC_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');

async function source(relativePath) {
  return readFile(resolve(SRC_ROOT, relativePath), 'utf8');
}

test('mounted wallet navigation presents the current process in order', async () => {
  const app = await source('App.jsx');
  const expected = [
    "{ id: 'wallet', label: 'Wallet' }",
    "{ id: 'bridge', label: 'Bridge' }",
    "{ id: 'market', label: 'NAV Markets' }",
    "{ id: 'send', label: 'Send' }",
    "{ id: 'swap', label: 'Process' }",
    "{ id: 'nav', label: 'NavCoins' }",
    "{ id: 'more', label: 'More' }",
  ];
  let previous = -1;
  for (const entry of expected) {
    const index = app.indexOf(entry);
    assert.ok(index > previous, `${entry} must be mounted in current-process order`);
    previous = index;
  }
  assert.doesNotMatch(app, /FastSwapDemo|ProductPrivateSwap/);
});

test('bridge-in is Ethereum mainnet USDC and never executes a retired route', async () => {
  const [bridge, evm, utils] = await Promise.all([
    source('components/Bridge.jsx'),
    source('lib/evm.js'),
    source('lib/utils.js'),
  ]);
  const runtime = `${bridge}\n${evm}\n${utils}`;

  assert.match(bridge, /MetaMask bridge-in · Ethereum mainnet/);
  assert.match(bridge, /loadGovernedVaultBridgeRoute/);
  assert.match(bridge, /depositToEthereumBridge/);
  assert.match(utils, /ETH_MAINNET_CHAIN_ID = 1/);
  assert.doesNotMatch(runtime, /ARBITRUM_CHAIN_ID|USDC_CONTRACT_ARBITRUM|ensureArbitrum|getArbitrum/);
  assert.doesNotMatch(runtime, /cctpBridgeUsdc|ensureEthereumSepolia/);
});

test('NAVCoin market UI is registry-driven while its deployed adapter remains route-pinned', async () => {
  const [market, registry, route, process] = await Promise.all([
    source('components/A666Market.jsx'),
    source('lib/navcoin-markets.js'),
    source('lib/a666-primary-route.js'),
    source('components/Swap.jsx'),
  ]);

  assert.match(registry, /NAVCOIN_MARKETS/);
  assert.match(route, /pftl-a666-ethereum-wA666-usdc-v1/);
  assert.match(route, /521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c/);
  assert.match(route, /02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b/);
  assert.match(route, /pftl_uniswap_export_debit/);
  assert.match(market, /Deliver to MetaMask/);
  assert.match(market, /await readWrappedNavcoinBalance\(selected, market\)/);
  assert.match(market, /setMetamaskNavcoinBalance\(wrappedBalance\.toString\(\)\)/);
  assert.match(market, /waits for its trustless Ethereum finality proof before reporting success/);
  assert.match(process, /market\.symbol/);
  assert.match(process, /onNavigate\?\.\('market', \{ marketKey: market\.key \}\)/);
});

test('visible asset surfaces contain no executable a651/a652 assumptions or fake verification', async () => {
  const visibleAssets = await Promise.all([
    source('components/WalletHome.jsx'),
    source('components/Send.jsx'),
    source('components/NavList.jsx'),
    source('components/NavDetail.jsx'),
  ]);
  const runtime = visibleAssets.join('\n');

  assert.doesNotMatch(runtime, /A651_ASSET_ID|A652_ASSET_ID|legacy_a651_uniswap/);
  assert.doesNotMatch(runtime, /setVerified\s*\(\s*true\s*\)/);
  assert.match(runtime, /NAVCoin/);
  assert.match(runtime, /Governed PFTL settlement asset|Governed NAVCoin settlement asset/);
});

test('retired browser workflow modules and proxy are absent', async () => {
  const [app, vite] = await Promise.all([
    source('App.jsx'),
    readFile(resolve(SRC_ROOT, '../vite.config.js'), 'utf8'),
  ]);

  assert.doesNotMatch(app, /fastswap-demo|eth-fast-lane|navswap-flow|product-private-swap|cctp/);
  assert.doesNotMatch(vite, /fastswap-demo|FASTSWAP_DEMO/);
});
