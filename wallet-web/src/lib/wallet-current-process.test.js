import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const SRC_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');

async function source(relativePath) {
  return readFile(resolve(SRC_ROOT, relativePath), 'utf8');
}

test('mounted wallet navigation presents user goals in order and hides operator tools', async () => {
  const app = await source('App.jsx');
  const expected = [
    "id: 'wallet', label: 'Home'",
    "id: 'nav', label: 'Assets'",
    "id: 'market', label: 'Trade'",
    "id: 'bridge', label: 'Bridge'",
    "id: 'send', label: 'Send'",
    "id: 'activity', label: 'Activity'",
    "id: 'more', label: 'Settings'",
  ];
  let previous = -1;
  for (const entry of expected) {
    const index = app.indexOf(entry);
    assert.ok(index > previous, `${entry} must be mounted in current-process order`);
    previous = index;
  }
  assert.doesNotMatch(app, /FastSwapDemo|ProductPrivateSwap|A666RoundTrip|PrivateFix|label: 'Process'|label: 'A666 Loop'/);
});

test('empty-wallet onboarding can restore an encrypted wallet backup', async () => {
  const [app, onboard] = await Promise.all([
    source('App.jsx'),
    source('components/Onboard.jsx'),
  ]);

  assert.match(app, /onImportBackup=\{handleImportBackup\}/);
  assert.match(onboard, /Restore from encrypted backup/);
  assert.match(onboard, /This is the 64-character hexadecimal recovery seed—not the passphrase/);
  assert.match(onboard, /backup\.vault\.ciphertext/);
  assert.match(onboard, /isValidAddress\(backup\.metadata\.address\)/);
  assert.doesNotMatch(onboard, /decryptVault/);
});

test('unlocked wallet can rotate its browser passphrase without changing custody identity', async () => {
  const [app, settings] = await Promise.all([
    source('App.jsx'),
    source('components/More.jsx'),
  ]);

  assert.match(app, /const seed = getDecryptedSeed\(\)/);
  assert.match(app, /encryptVault\(seed, newPassphrase\)/);
  assert.match(app, /\.\.\.vault\.metadata/);
  assert.match(settings, /does not change the wallet address or recovery seed/);
  assert.match(settings, /Existing backup files still require their original passphrase/);
});

test('bridge-in is Ethereum mainnet USDC and never executes a retired route', async () => {
  const [bridge, evm, utils] = await Promise.all([
    source('components/Bridge.jsx'),
    source('lib/evm.js'),
    source('lib/utils.js'),
  ]);
  const runtime = `${bridge}\n${evm}\n${utils}`;

  assert.match(bridge, /Ethereum → PFTL/);
  assert.match(bridge, /Deposit USDC/);
  assert.match(bridge, /loadGovernedVaultBridgeRoute/);
  assert.match(bridge, /depositToEthereumBridge/);
  assert.match(utils, /ETH_MAINNET_CHAIN_ID = 1/);
  assert.doesNotMatch(runtime, /Arbitrum|ARBITRUM_CHAIN_ID|USDC_CONTRACT_ARBITRUM|ensureArbitrum|getArbitrum/);
  assert.doesNotMatch(runtime, /cctpBridgeUsdc|ensureEthereumSepolia/);
});

test('NAVCoin market UI is registry-driven and has no hard-coded asset identity', async () => {
  const [market, registry, route] = await Promise.all([
    source('components/NavcoinPrimaryMarket.jsx'),
    source('lib/navcoin-markets.js'),
    source('lib/navcoin-primary-route.js'),
  ]);

  assert.match(registry, /navcoinMarketsFromRoutes/);
  assert.doesNotMatch(registry, /A666|a666|521c6c630bb48d4a37ab/);
  assert.match(route, /pftl_uniswap_export_debit/);
  assert.match(market, /Deliver to MetaMask/);
  assert.match(market, /await readWrappedNavcoinBalance\(selected, market\)/);
  assert.match(market, /setMetamaskNavcoinBalance\(wrappedBalance\.toString\(\)\)/);
  assert.match(market, /verificationCopy\(route\?\.outbound_verification_class\)/);
  assert.match(market, /https:\/\/app\.uniswap\.org\/swap\?chain=mainnet/);
  assert.match(market, /inputCurrency=\$\{ETH_MAINNET_USDC\}&outputCurrency=\$\{market\.wrappedToken\}/);
  assert.match(market, /Redeem at NAV → From MetaMask/);
  assert.doesNotMatch(market, /Return \$\{wrappedSymbol\} trustlessly/);
  assert.match(registry, /loadNavcoinMarkets/);
  assert.match(registry, /assetInfo\(assetId\)/);
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
  assert.match(runtime, /Verified NAV asset|NAV asset/);
  assert.match(runtime, /Settlement asset on PFTL|Settlement asset/);
  assert.doesNotMatch(runtime, /Total balance/);
  assert.doesNotMatch(runtime, /other or legacy issued asset/i);
});

test('retired browser workflow modules and proxy are absent', async () => {
  const [app, vite] = await Promise.all([
    source('App.jsx'),
    readFile(resolve(SRC_ROOT, '../vite.config.js'), 'utf8'),
  ]);

  assert.doesNotMatch(app, /fastswap-demo|eth-fast-lane|navswap-flow|product-private-swap|cctp/);
  assert.doesNotMatch(vite, /fastswap-demo|FASTSWAP_DEMO/);
});
