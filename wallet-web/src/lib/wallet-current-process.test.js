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

test('empty-wallet onboarding can restore an encrypted wallet backup', async () => {
  const [app, onboard] = await Promise.all([
    source('App.jsx'),
    source('components/Onboard.jsx'),
  ]);

  assert.match(app, /onImportBackup=\{handleImportBackup\}/);
  assert.match(onboard, /Import Encrypted Backup/);
  assert.match(onboard, /backup\.vault\.ciphertext/);
  assert.match(onboard, /isValidAddress\(backup\.metadata\.address\)/);
  assert.doesNotMatch(onboard, /decryptVault/);
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

test('NAVCoin market UI is registry-driven and has no hard-coded asset identity', async () => {
  const [market, registry, route, process] = await Promise.all([
    source('components/NavcoinPrimaryMarket.jsx'),
    source('lib/navcoin-markets.js'),
    source('lib/navcoin-primary-route.js'),
    source('components/Swap.jsx'),
  ]);

  assert.match(registry, /navcoinMarketsFromRoutes/);
  assert.doesNotMatch(registry, /A666|a666|521c6c630bb48d4a37ab/);
  assert.match(route, /pftl_uniswap_export_debit/);
  assert.match(market, /Deliver to MetaMask/);
  assert.match(market, /await readWrappedNavcoinBalance\(selected, market\)/);
  assert.match(market, /setMetamaskNavcoinBalance\(wrappedBalance\.toString\(\)\)/);
  assert.match(market, /verificationCopy\(route\?\.outbound_verification_class\)/);
  assert.doesNotMatch(market, /Return \$\{wrappedSymbol\} trustlessly/);
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
