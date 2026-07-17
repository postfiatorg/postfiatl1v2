#!/usr/bin/env node
import { execFile } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { promisify } from 'node:util';
import WebSocket from '../wallet-proxy/node_modules/ws/index.js';

import { RpcClient } from '../wallet-web/src/lib/rpc-client.js';
import { A651_ASSET_ID, PFUSDC_ASSET_ID } from '../wallet-web/src/lib/utils.js';

globalThis.WebSocket = WebSocket;

const execFileAsync = promisify(execFile);

const DEFAULT_PFTL_RPC = 'ws://127.0.0.1:8080/rpc';
const DEFAULT_ETH_RPC = 'https://ethereum.publicnode.com';
const DEFAULT_ARBITRUM_RPC = 'https://arb1.arbitrum.io/rpc';

const A651_ETH_TOKEN = '0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e';
const ETH_MAINNET_USDC = '0xA0b86991c6218b36c1d19D4A2e9Eb0cE3606eB48';
const ARBITRUM_USDC = '0xaf88d065e77c8cC2239327C5EDb3A432268e5831';
const STAKEHUB_EVM_OWNER = '0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0';
const UNISWAP_V4_POOL_MANAGER = '0x000000000004444c5dc75cB358380D2e3dE08A90';
const UNISWAP_V4_POSITION_MANAGER = '0xbD216513d74C8cf14cf4747E6AaA6420FF64ee9e';
const UNISWAP_V4_STATE_VIEW = '0x7ffe42c4a5deea5b0fec41c94c136cf115597227';
const A651_USDC_POOL_ID = '0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84';
const OLD_ARBITRUM_PFUSDC_VAULT = '0x1A15e6103D6Af4e88924F748e13B829D3948DEa9';

const DEFAULT_PFTL_ACCOUNTS = [
  'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
  'pf07381735ddb7de134e8be8402b465c9cd8ec7546',
  'pfac0562296948fbf35fec6d18d47498b412850a8c',
  'pfa95c2c765a41b24867b23703ac688d9eaa8a9264',
];

function usage() {
  return `Usage:
  node scripts/navswap-custody-inventory.mjs [--out-dir DIR]

Read-only inventory for the overnight NAVSwap work. It queries PFTL account
state through the local wallet proxy and Ethereum/Arbitrum balances through
public RPC via cast. It never signs, submits, approves, or transfers funds.

Options:
  --out-dir DIR              Output directory. Default: /tmp/navswap-custody-inventory-<timestamp>
  --pftl-rpc URL             Wallet proxy WebSocket RPC. Default: ${DEFAULT_PFTL_RPC}
  --eth-rpc URL              Ethereum mainnet RPC. Default: ${DEFAULT_ETH_RPC}
  --arbitrum-rpc URL         Arbitrum One RPC. Default: ${DEFAULT_ARBITRUM_RPC}
  --pftl-account ADDR        Additional PFTL account to inspect. May be repeated.
  --skip-evm                 Skip Ethereum/Arbitrum reads.
  --skip-pftl                Skip PFTL reads.
`;
}

function parseArgs(argv) {
  const args = {
    pftlRpc: DEFAULT_PFTL_RPC,
    ethRpc: DEFAULT_ETH_RPC,
    arbitrumRpc: DEFAULT_ARBITRUM_RPC,
    pftlAccounts: [...DEFAULT_PFTL_ACCOUNTS],
    skipEvm: false,
    skipPftl: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--help' || arg === '-h') {
      args.help = true;
    } else if (arg === '--skip-evm') {
      args.skipEvm = true;
    } else if (arg === '--skip-pftl') {
      args.skipPftl = true;
    } else if (arg === '--pftl-account') {
      const value = argv[i + 1];
      if (!value || value.startsWith('--')) throw new Error('--pftl-account requires a value');
      args.pftlAccounts.push(value);
      i += 1;
    } else if (arg.startsWith('--')) {
      const key = arg.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
      const value = argv[i + 1];
      if (!value || value.startsWith('--')) throw new Error(`${arg} requires a value`);
      args[key] = value;
      i += 1;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  args.pftlAccounts = [...new Set(args.pftlAccounts)];
  for (const account of args.pftlAccounts) {
    if (!/^pf[0-9a-f]{40}$/.test(account)) {
      throw new Error(`invalid PFTL account: ${account}`);
    }
  }
  return args;
}

function jsonReplacer(_key, value) {
  return typeof value === 'bigint' ? value.toString() : value;
}

async function writeJson(file, value) {
  await fs.writeFile(file, `${JSON.stringify(value, jsonReplacer, 2)}\n`);
}

function formatUnits(raw, decimals) {
  const value = BigInt(String(raw ?? 0));
  const scale = 10n ** BigInt(decimals);
  const whole = value / scale;
  const fraction = value % scale;
  if (fraction === 0n) return whole.toString();
  return `${whole}.${fraction.toString().padStart(decimals, '0').replace(/0+$/, '')}`;
}

function assetItems(result) {
  if (Array.isArray(result)) return result;
  if (Array.isArray(result?.assets)) return result.assets;
  return [];
}

function assetBalance(result, assetId) {
  let total = 0n;
  for (const item of assetItems(result)) {
    if ((item.asset_id || item.id) === assetId) {
      total += BigInt(item.balance ?? item.amount ?? 0);
    }
  }
  return total;
}

function nativeBalance(accountResult) {
  const account = accountResult?.account || accountResult || {};
  return BigInt(account.balance ?? account.pft_balance ?? account.native_balance ?? 0);
}

function classifyPftlBalance(asset, raw) {
  const amount = BigInt(raw);
  if (asset === 'PFT') return amount > 0n ? 'spendable_fee_balance' : 'empty';
  if (asset === 'pfUSDC') return amount > 0n ? 'spendable_issued_asset_if_trustline_authorized' : 'empty';
  if (asset === 'a651') return amount > 0n ? 'spendable_nav_asset_if_trustline_authorized' : 'empty';
  return amount > 0n ? 'unknown_spendability' : 'empty';
}

async function castCall(rpcUrl, target, signature, args = []) {
  const { stdout } = await execFileAsync(
    'cast',
    ['call', '--rpc-url', rpcUrl, target, signature, ...args],
    { timeout: 30000, maxBuffer: 2 * 1024 * 1024 },
  );
  return stdout.trim();
}

async function castBalance(rpcUrl, address) {
  const { stdout } = await execFileAsync(
    'cast',
    ['balance', '--rpc-url', rpcUrl, address],
    { timeout: 30000, maxBuffer: 2 * 1024 * 1024 },
  );
  return stdout.trim();
}

function castUint(raw) {
  const trimmed = String(raw || '').trim();
  if (/^0x[0-9a-fA-F]+$/.test(trimmed)) return BigInt(trimmed).toString();
  const firstNumber = trimmed.match(/-?[0-9]+/);
  if (!firstNumber) return null;
  return BigInt(firstNumber[0]).toString();
}

function castTupleNumbers(raw) {
  return String(raw || '')
    .split(/\r?\n/)
    .map(line => line.trim().match(/^-?[0-9]+/)?.[0] || null)
    .filter(value => value !== null);
}

async function safeRead(label, fn) {
  try {
    return { ok: true, result: await fn() };
  } catch (error) {
    return {
      ok: false,
      error: {
        label,
        message: error?.message || String(error),
      },
    };
  }
}

async function collectPftl(args) {
  const rpc = new RpcClient(args.pftlRpc);
  const accounts = [];
  const assetInfo = {};
  const vaultBridge = {};
  try {
    const [pfusdcInfo, a651Info, pfusdcVault, a651Vault] = await Promise.all([
      safeRead('asset_info pfUSDC', () => rpc.assetInfo(PFUSDC_ASSET_ID)),
      safeRead('asset_info a651', () => rpc.assetInfo(A651_ASSET_ID)),
      safeRead('vault_bridge_status pfUSDC', () => rpc.call('vault_bridge_status', { asset_id: PFUSDC_ASSET_ID })),
      safeRead('vault_bridge_status a651', () => rpc.call('vault_bridge_status', { asset_id: A651_ASSET_ID })),
    ]);
    assetInfo.pfUSDC = pfusdcInfo;
    assetInfo.a651 = a651Info;
    vaultBridge.pfUSDC = pfusdcVault;
    vaultBridge.a651 = a651Vault;

    for (const account of args.pftlAccounts) {
      const [accountResp, assetsResp, linesResp] = await Promise.all([
        safeRead(`account ${account}`, () => rpc.call('account', { address: account })),
        safeRead(`account_assets ${account}`, () => rpc.accountAssets(account)),
        safeRead(`account_lines ${account}`, () => rpc.accountLines(account)),
      ]);
      const pftAtoms = accountResp.ok && accountResp.result.ok
        ? nativeBalance(accountResp.result.result).toString()
        : null;
      const pfusdcAtoms = assetsResp.ok && assetsResp.result.ok
        ? assetBalance(assetsResp.result.result, PFUSDC_ASSET_ID).toString()
        : null;
      const a651Atoms = assetsResp.ok && assetsResp.result.ok
        ? assetBalance(assetsResp.result.result, A651_ASSET_ID).toString()
        : null;
      accounts.push({
        account,
        reads: { account: accountResp, account_assets: assetsResp, account_lines: linesResp },
        balances: [
          {
            chain: 'postfiat-wan-devnet',
            address: account,
            asset: 'PFT',
            asset_id: null,
            raw_atoms: pftAtoms,
            human: pftAtoms === null ? null : formatUnits(pftAtoms, 6),
            spendability: pftAtoms === null ? 'unknown' : classifyPftlBalance('PFT', pftAtoms),
          },
          {
            chain: 'postfiat-wan-devnet',
            address: account,
            asset: 'pfUSDC',
            asset_id: PFUSDC_ASSET_ID,
            raw_atoms: pfusdcAtoms,
            human: pfusdcAtoms === null ? null : formatUnits(pfusdcAtoms, 6),
            spendability: pfusdcAtoms === null ? 'unknown' : classifyPftlBalance('pfUSDC', pfusdcAtoms),
          },
          {
            chain: 'postfiat-wan-devnet',
            address: account,
            asset: 'a651',
            asset_id: A651_ASSET_ID,
            raw_atoms: a651Atoms,
            human: a651Atoms,
            spendability: a651Atoms === null ? 'unknown' : classifyPftlBalance('a651', a651Atoms),
          },
        ],
      });
    }
  } finally {
    rpc.close?.();
  }
  return { rpc: args.pftlRpc, asset_info: assetInfo, vault_bridge: vaultBridge, accounts };
}

async function collectEvm(args) {
  const ethereum = {
    chain: 'ethereum-mainnet',
    rpc: args.ethRpc,
    operator: STAKEHUB_EVM_OWNER,
    a651_token: A651_ETH_TOKEN,
    usdc_token: ETH_MAINNET_USDC,
    legacy_pool_id: A651_USDC_POOL_ID,
    balances: [],
    pool: {
      pool_manager: UNISWAP_V4_POOL_MANAGER,
      position_manager: UNISWAP_V4_POSITION_MANAGER,
      state_view: UNISWAP_V4_STATE_VIEW,
      pool_id: A651_USDC_POOL_ID,
    },
  };
  const arbitrum = {
    chain: 'arbitrum-one',
    rpc: args.arbitrumRpc,
    operator: STAKEHUB_EVM_OWNER,
    usdc_token: ARBITRUM_USDC,
    old_pfusdc_vault: OLD_ARBITRUM_PFUSDC_VAULT,
    balances: [],
  };

  const ethReads = await Promise.all([
    safeRead('ethereum operator ETH', () => castBalance(args.ethRpc, STAKEHUB_EVM_OWNER)),
    safeRead('ethereum operator a651', () => castCall(args.ethRpc, A651_ETH_TOKEN, 'balanceOf(address)(uint256)', [STAKEHUB_EVM_OWNER])),
    safeRead('ethereum operator USDC', () => castCall(args.ethRpc, ETH_MAINNET_USDC, 'balanceOf(address)(uint256)', [STAKEHUB_EVM_OWNER])),
    safeRead('ethereum operator position NFT count', () => castCall(args.ethRpc, UNISWAP_V4_POSITION_MANAGER, 'balanceOf(address)(uint256)', [STAKEHUB_EVM_OWNER])),
    safeRead('ethereum pool manager a651', () => castCall(args.ethRpc, A651_ETH_TOKEN, 'balanceOf(address)(uint256)', [UNISWAP_V4_POOL_MANAGER])),
    safeRead('ethereum stateview slot0', () => castCall(args.ethRpc, UNISWAP_V4_STATE_VIEW, 'getSlot0(bytes32)(uint160,int24,uint24,uint24)', [A651_USDC_POOL_ID])),
    safeRead('ethereum stateview liquidity', () => castCall(args.ethRpc, UNISWAP_V4_STATE_VIEW, 'getLiquidity(bytes32)(uint128)', [A651_USDC_POOL_ID])),
  ]);
  const [
    operatorEth,
    operatorA651,
    operatorUsdc,
    operatorPositionNfts,
    poolManagerA651,
    slot0,
    liquidity,
  ] = ethReads;

  const operatorEthRaw = operatorEth.ok ? castUint(operatorEth.result) : null;
  const operatorA651Raw = operatorA651.ok ? castUint(operatorA651.result) : null;
  const operatorUsdcRaw = operatorUsdc.ok ? castUint(operatorUsdc.result) : null;
  const operatorNftRaw = operatorPositionNfts.ok ? castUint(operatorPositionNfts.result) : null;
  const poolA651Raw = poolManagerA651.ok ? castUint(poolManagerA651.result) : null;
  const liquidityRaw = liquidity.ok ? castUint(liquidity.result) : null;
  const slot0Values = slot0.ok ? castTupleNumbers(slot0.result) : [];

  ethereum.balances.push(
    {
      chain: 'ethereum-mainnet',
      address: STAKEHUB_EVM_OWNER,
      asset: 'ETH',
      raw_atoms: operatorEthRaw,
      human: operatorEthRaw === null ? null : formatUnits(operatorEthRaw, 18),
      spendability: operatorEthRaw === null ? 'unknown' : 'spendable_gas_if_key_unlocked',
    },
    {
      chain: 'ethereum-mainnet',
      address: STAKEHUB_EVM_OWNER,
      asset: 'a651',
      token: A651_ETH_TOKEN,
      raw_atoms: operatorA651Raw,
      human: operatorA651Raw === null ? null : formatUnits(operatorA651Raw, 18),
      spendability: operatorA651Raw === null ? 'unknown' : 'operator_spendable_legacy_erc20_if_key_unlocked',
    },
    {
      chain: 'ethereum-mainnet',
      address: STAKEHUB_EVM_OWNER,
      asset: 'USDC',
      token: ETH_MAINNET_USDC,
      raw_atoms: operatorUsdcRaw,
      human: operatorUsdcRaw === null ? null : formatUnits(operatorUsdcRaw, 6),
      spendability: operatorUsdcRaw === null ? 'unknown' : 'operator_spendable_erc20_if_key_unlocked',
    },
    {
      chain: 'ethereum-mainnet',
      address: STAKEHUB_EVM_OWNER,
      asset: 'Uniswap v4 PositionManager NFT',
      token: UNISWAP_V4_POSITION_MANAGER,
      raw_atoms: operatorNftRaw,
      human: operatorNftRaw,
      spendability: operatorNftRaw === null ? 'unknown' : 'lp_positioned_or_historical_position',
    },
    {
      chain: 'ethereum-mainnet',
      address: UNISWAP_V4_POOL_MANAGER,
      asset: 'a651',
      token: A651_ETH_TOKEN,
      raw_atoms: poolA651Raw,
      human: poolA651Raw === null ? null : formatUnits(poolA651Raw, 18),
      spendability: poolA651Raw === null ? 'unknown' : 'pool_manager_controlled_not_operator_spendable',
    },
  );
  ethereum.pool.reads = {
    operator_eth: operatorEth,
    operator_a651: operatorA651,
    operator_usdc: operatorUsdc,
    operator_position_nfts: operatorPositionNfts,
    pool_manager_a651: poolManagerA651,
    stateview_slot0: slot0,
    stateview_liquidity: liquidity,
  };
  ethereum.pool.slot0 = slot0Values.length >= 4
    ? {
      sqrtPriceX96: slot0Values[0],
      tick: slot0Values[1],
      protocolFee: slot0Values[2],
      lpFee: slot0Values[3],
    }
    : null;
  ethereum.pool.liquidity_raw = liquidityRaw;
  ethereum.pool.status = liquidityRaw === '0'
    ? 'legacy_pool_inactive_zero_stateview_liquidity'
    : liquidityRaw === null
      ? 'unknown'
      : 'legacy_pool_has_stateview_liquidity';

  const arbReads = await Promise.all([
    safeRead('arbitrum operator ETH', () => castBalance(args.arbitrumRpc, STAKEHUB_EVM_OWNER)),
    safeRead('arbitrum operator USDC', () => castCall(args.arbitrumRpc, ARBITRUM_USDC, 'balanceOf(address)(uint256)', [STAKEHUB_EVM_OWNER])),
    safeRead('arbitrum old vault USDC', () => castCall(args.arbitrumRpc, ARBITRUM_USDC, 'balanceOf(address)(uint256)', [OLD_ARBITRUM_PFUSDC_VAULT])),
  ]);
  const [arbEth, arbUsdc, arbVaultUsdc] = arbReads;
  const arbEthRaw = arbEth.ok ? castUint(arbEth.result) : null;
  const arbUsdcRaw = arbUsdc.ok ? castUint(arbUsdc.result) : null;
  const arbVaultUsdcRaw = arbVaultUsdc.ok ? castUint(arbVaultUsdc.result) : null;
  arbitrum.balances.push(
    {
      chain: 'arbitrum-one',
      address: STAKEHUB_EVM_OWNER,
      asset: 'ETH',
      raw_atoms: arbEthRaw,
      human: arbEthRaw === null ? null : formatUnits(arbEthRaw, 18),
      spendability: arbEthRaw === null ? 'unknown' : 'spendable_gas_if_key_unlocked',
    },
    {
      chain: 'arbitrum-one',
      address: STAKEHUB_EVM_OWNER,
      asset: 'USDC',
      token: ARBITRUM_USDC,
      raw_atoms: arbUsdcRaw,
      human: arbUsdcRaw === null ? null : formatUnits(arbUsdcRaw, 6),
      spendability: arbUsdcRaw === null ? 'unknown' : 'operator_spendable_erc20_if_key_unlocked',
    },
    {
      chain: 'arbitrum-one',
      address: OLD_ARBITRUM_PFUSDC_VAULT,
      asset: 'USDC',
      token: ARBITRUM_USDC,
      raw_atoms: arbVaultUsdcRaw,
      human: arbVaultUsdcRaw === null ? null : formatUnits(arbVaultUsdcRaw, 6),
      spendability: arbVaultUsdcRaw === null
        ? 'unknown'
        : (arbVaultUsdcRaw === '0' ? 'drained_old_vault' : 'bridge_controlled_vault'),
    },
  );
  arbitrum.reads = {
    operator_eth: arbEth,
    operator_usdc: arbUsdc,
    old_vault_usdc: arbVaultUsdc,
  };
  return { ethereum, arbitrum };
}

function flattenBalances(report) {
  const rows = [];
  for (const account of report.pftl?.accounts || []) {
    rows.push(...account.balances);
  }
  rows.push(...(report.evm?.ethereum?.balances || []));
  rows.push(...(report.evm?.arbitrum?.balances || []));
  return rows;
}

function markdownTable(rows) {
  const lines = [
    '| Chain | Address | Asset | Raw balance | Human balance | Spendability |',
    '| --- | --- | --- | ---: | ---: | --- |',
  ];
  for (const row of rows) {
    lines.push(`| ${row.chain} | \`${row.address}\` | ${row.asset} | \`${row.raw_atoms ?? 'unavailable'}\` | ${row.human ?? 'unavailable'} | ${row.spendability} |`);
  }
  return lines.join('\n');
}

function renderMarkdown(report) {
  const rows = flattenBalances(report);
  const pool = report.evm?.ethereum?.pool;
  return `# NAVSwap Custody Inventory

Generated: \`${report.generated_at}\`

This is a read-only inventory. It does not sign, submit, approve, or transfer.

## Balances

${markdownTable(rows)}

## Legacy Ethereum Pool

| Field | Value |
| --- | --- |
| Pool id | \`${A651_USDC_POOL_ID}\` |
| StateView liquidity | \`${pool?.liquidity_raw ?? 'unavailable'}\` |
| Status | ${pool?.status ?? 'skipped'} |
| Tick | \`${pool?.slot0?.tick ?? 'unavailable'}\` |
| LP fee | \`${pool?.slot0?.lpFee ?? 'unavailable'}\` |

## Classification

- Ethereum mainnet \`a651/USDC\` is legacy secondary liquidity, not the trustless PFTL-to-Uniswap handoff route.
- If StateView liquidity is \`0\`, the legacy pool is not usable for active wallet routing.
- Arbitrum old pfUSDC vault is bridge-controlled when nonzero and drained when zero.
- PFTL issued-asset balances are spendable only if their trustlines are healthy and the account owner signs locally.
`;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    process.stdout.write(usage());
    return;
  }

  const outDir = args.outDir || path.join('/tmp', `navswap-custody-inventory-${new Date().toISOString().replace(/[:.]/g, '')}`);
  await fs.mkdir(outDir, { recursive: true });

  const report = {
    ok: true,
    schema: 'postfiat-navswap-custody-inventory-v1',
    generated_at: new Date().toISOString(),
    mode: 'read_only',
    constants: {
      pftl_chain: 'postfiat-wan-devnet',
      pfusdc_asset_id: PFUSDC_ASSET_ID,
      a651_asset_id: A651_ASSET_ID,
      ethereum_a651_token: A651_ETH_TOKEN,
      ethereum_usdc_token: ETH_MAINNET_USDC,
      arbitrum_usdc_token: ARBITRUM_USDC,
      stakehub_evm_owner: STAKEHUB_EVM_OWNER,
      old_arbitrum_pfusdc_vault: OLD_ARBITRUM_PFUSDC_VAULT,
    },
  };

  if (!args.skipPftl) {
    report.pftl = await collectPftl(args);
  }
  if (!args.skipEvm) {
    report.evm = await collectEvm(args);
  }
  report.balance_rows = flattenBalances(report);
  report.summary = {
    custody_row_count: report.balance_rows.length,
    legacy_pool_status: report.evm?.ethereum?.pool?.status ?? 'skipped',
    live_transaction_required: false,
  };

  const markdown = renderMarkdown(report);
  await writeJson(path.join(outDir, 'inventory.json'), report);
  await fs.writeFile(path.join(outDir, 'inventory.md'), markdown);
  process.stdout.write(`${JSON.stringify({
    ok: true,
    out_dir: outDir,
    summary: report.summary,
  }, jsonReplacer, 2)}\n`);
}

main().catch((error) => {
  process.stderr.write(`${error.stack || error.message}\n`);
  process.exit(1);
});
