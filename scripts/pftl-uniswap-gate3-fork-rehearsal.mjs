#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const HELP = `
Usage:
  node scripts/pftl-uniswap-gate3-fork-rehearsal.mjs \\
    --launch-config-file docs/plans/pftl-uniswap-launch-config.json \\
    --rpc-url "$ETHEREUM_RPC_URL" \\
    --seed-export-packet-hash <96-hex> \\
    --seed-receipt-root <96-hex> \\
    --seed-mint-tx <0x...> \\
    --seed-lp-tx <0x...> \\
    --external-buy-tx <0x...> \\
    --external-sell-tx <0x...> \\
    --mint-only-packet-tx <0x...> \\
    --mint-and-swap-packet-tx <0x...> \\
    --user-buy-usdc-spent-atoms <u64> \\
    --user-buy-wrapped-received-atoms <u64> \\
    --user-sell-wrapped-spent-atoms <u64> \\
    --user-sell-usdc-received-atoms <u64> \\
    --canonical-supply-before-external-trades-atoms <u64> \\
    --canonical-supply-after-external-trades-atoms <u64> \\
    --output-file docs/plans/pftl-uniswap-fork-rehearsal-evidence.json

Required source:
  --rpc-url may be omitted when ETHEREUM_RPC_URL or MAINNET_RPC_URL is set.

Optional:
  --launch-config-digest <96-hex>
  --fork-block-number <u64>
  --rehearsal-id <string>
  --packet-consumed-without-manual-mint true|false
  --min-output-failure-reverted-without-consume true|false
`;

const LAUNCH_CONFIG_DOMAIN = 'postfiat.pftl_uniswap.launch_config.v1';
const EXPECTED_LAUNCH_SCHEMA = 'postfiat-pftl-uniswap-launch-config-v1';
const EXPECTED_EVIDENCE_SCHEMA = 'postfiat-pftl-uniswap-fork-rehearsal-evidence-v1';
const HEX_48_RE = /^(?:0x)?[0-9a-fA-F]{96}$/;
const EVM_ADDRESS_RE = /^0x[0-9a-fA-F]{40}$/;
const TX_HASH_RE = /^0x[0-9a-fA-F]{64}$/;
const BYTES32_RE = /^(?:0x)?[0-9a-fA-F]{64}$/;
const U128_MAX = (1n << 128n) - 1n;
const U64_MAX = (1n << 64n) - 1n;

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === '--help' || token === '-h') {
      args.help = true;
      continue;
    }
    if (!token.startsWith('--')) {
      throw new Error(`unexpected positional argument: ${token}`);
    }
    const key = token.slice(2);
    const value = argv[i + 1];
    if (value === undefined || value.startsWith('--')) {
      throw new Error(`missing value for --${key}`);
    }
    args[toCamelCase(key)] = value;
    i += 1;
  }
  return args;
}

function toCamelCase(value) {
  return value.replace(/-([a-z])/g, (_, char) => char.toUpperCase());
}

function requireArg(args, name) {
  const value = args[name];
  if (value === undefined || value === '') {
    throw new Error(`missing required --${name.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`)}`);
  }
  return value;
}

function normalizeHash48(value, label) {
  if (!HEX_48_RE.test(String(value))) {
    throw new Error(`${label} must be a 48-byte hex string`);
  }
  return String(value).replace(/^0x/i, '').toLowerCase();
}

function normalizeTxHash(value, label) {
  if (!TX_HASH_RE.test(String(value))) {
    throw new Error(`${label} must be a 32-byte transaction hash`);
  }
  return String(value).toLowerCase();
}

function txHashNoPrefix(value) {
  return value.replace(/^0x/i, '');
}

function normalizeBytes32NoPrefix(value, label) {
  if (!BYTES32_RE.test(String(value))) {
    throw new Error(`${label} must be a bytes32 value`);
  }
  return String(value).replace(/^0x/i, '').toLowerCase();
}

function normalizeAddress(value, label) {
  if (!EVM_ADDRESS_RE.test(String(value))) {
    throw new Error(`${label} must be an EVM address`);
  }
  return String(value).toLowerCase();
}

function parseDecimal(value, label, max = U64_MAX) {
  const text = String(value);
  if (!/^(0|[1-9][0-9]*)$/.test(text)) {
    throw new Error(`${label} must be an unsigned decimal integer`);
  }
  const parsed = BigInt(text);
  if (parsed > max) {
    throw new Error(`${label} exceeds supported max ${max.toString()}`);
  }
  return parsed;
}

function rawNumber(value, label, max) {
  return {
    __rawNumber: parseDecimal(value, label, max).toString(),
  };
}

function parseBoolean(value, label, defaultValue) {
  if (value === undefined) {
    return defaultValue;
  }
  if (value === 'true') {
    return true;
  }
  if (value === 'false') {
    return false;
  }
  throw new Error(`${label} must be true or false`);
}

function assertObject(value, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
}

function requireString(object, key, label = key) {
  const value = object[key];
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
}

function requireNumber(object, key, label = key) {
  const value = object[key];
  if (!Number.isInteger(value) || value < 0) {
    throw new Error(`${label} must be a non-negative integer`);
  }
  return value;
}

function requireSignedInteger(object, key, label = key) {
  const value = object[key];
  if (!Number.isInteger(value)) {
    throw new Error(`${label} must be an integer`);
  }
  return value;
}

function requireBoolean(object, key, label = key) {
  const value = object[key];
  if (typeof value !== 'boolean') {
    throw new Error(`${label} must be a boolean`);
  }
  return value;
}

function maybeLargeIntegerLiteral(rawJson) {
  return /:\s*(?:0|[1-9][0-9]{15,})\b/.test(rawJson);
}

function orderedOfficialUniswap(value) {
  assertObject(value, 'official_uniswap');
  return {
    chain_id: requireNumber(value, 'chain_id', 'official_uniswap.chain_id'),
    deployments_source_url: requireString(
      value,
      'deployments_source_url',
      'official_uniswap.deployments_source_url',
    ),
    deployments_table_hash: normalizeBytes32NoPrefix(
      requireString(value, 'deployments_table_hash', 'official_uniswap.deployments_table_hash'),
      'official_uniswap.deployments_table_hash',
    ),
    checked_at_utc: requireString(value, 'checked_at_utc', 'official_uniswap.checked_at_utc'),
    pool_manager: normalizeAddress(requireString(value, 'pool_manager', 'official_uniswap.pool_manager'), 'official_uniswap.pool_manager'),
    position_manager: normalizeAddress(
      requireString(value, 'position_manager', 'official_uniswap.position_manager'),
      'official_uniswap.position_manager',
    ),
    universal_router: normalizeAddress(
      requireString(value, 'universal_router', 'official_uniswap.universal_router'),
      'official_uniswap.universal_router',
    ),
    permit2: normalizeAddress(requireString(value, 'permit2', 'official_uniswap.permit2'), 'official_uniswap.permit2'),
    state_view: normalizeAddress(
      requireString(value, 'state_view', 'official_uniswap.state_view'),
      'official_uniswap.state_view',
    ),
  };
}

function orderedPoolSeed(value) {
  assertObject(value, 'seed');
  return {
    pricing_nav_epoch: requireNumber(value, 'pricing_nav_epoch', 'seed.pricing_nav_epoch'),
    pricing_reserve_packet_hash: normalizeHash48(
      requireString(value, 'pricing_reserve_packet_hash', 'seed.pricing_reserve_packet_hash'),
      'seed.pricing_reserve_packet_hash',
    ),
    seed_usdc_atoms: requireNumber(value, 'seed_usdc_atoms', 'seed.seed_usdc_atoms'),
    seed_wrapped_navcoin_atoms: requireNumber(
      value,
      'seed_wrapped_navcoin_atoms',
      'seed.seed_wrapped_navcoin_atoms',
    ),
    nav_price_settlement_atoms_per_nav_atom: requireNumber(
      value,
      'nav_price_settlement_atoms_per_nav_atom',
      'seed.nav_price_settlement_atoms_per_nav_atom',
    ),
    tick_lower: requireSignedInteger(value, 'tick_lower', 'seed.tick_lower'),
    tick_upper: requireSignedInteger(value, 'tick_upper', 'seed.tick_upper'),
    fee_pips: requireNumber(value, 'fee_pips', 'seed.fee_pips'),
    lp_recipient: normalizeAddress(requireString(value, 'lp_recipient', 'seed.lp_recipient'), 'seed.lp_recipient'),
    position_recipient: normalizeAddress(
      requireString(value, 'position_recipient', 'seed.position_recipient'),
      'seed.position_recipient',
    ),
    lp_custody_policy: requireString(value, 'lp_custody_policy', 'seed.lp_custody_policy'),
  };
}

function orderedLaunchConfig(value) {
  assertObject(value, 'launch config');
  return {
    schema: requireString(value, 'schema'),
    route_id: requireString(value, 'route_id'),
    route_config_digest: normalizeHash48(requireString(value, 'route_config_digest'), 'route_config_digest'),
    route_trust_class: requireString(value, 'route_trust_class'),
    native_nav_asset_id: requireString(value, 'native_nav_asset_id'),
    settlement_asset_id: requireString(value, 'settlement_asset_id'),
    wrapped_navcoin_token: normalizeAddress(requireString(value, 'wrapped_navcoin_token'), 'wrapped_navcoin_token'),
    usdc_token: normalizeAddress(requireString(value, 'usdc_token'), 'usdc_token'),
    handoff_controller: normalizeAddress(requireString(value, 'handoff_controller'), 'handoff_controller'),
    receipt_verifier: normalizeAddress(requireString(value, 'receipt_verifier'), 'receipt_verifier'),
    settlement_adapter: normalizeAddress(requireString(value, 'settlement_adapter'), 'settlement_adapter'),
    official_uniswap: orderedOfficialUniswap(value.official_uniswap),
    uniswap_pool_key_hash: normalizeBytes32NoPrefix(requireString(value, 'uniswap_pool_key_hash'), 'uniswap_pool_key_hash'),
    uniswap_pool_id: normalizeBytes32NoPrefix(requireString(value, 'uniswap_pool_id'), 'uniswap_pool_id'),
    seed: orderedPoolSeed(value.seed),
    fork_rehearsal_required: requireBoolean(value, 'fork_rehearsal_required'),
  };
}

function digestLaunchConfig(config) {
  const hash = createHash('sha3-384');
  hash.update(Buffer.from(LAUNCH_CONFIG_DOMAIN, 'utf8'));
  hash.update(Buffer.from([0]));
  hash.update(Buffer.from(JSON.stringify(orderedLaunchConfig(config)), 'utf8'));
  return hash.digest('hex');
}

async function rpcCall(rpcUrl, method, params) {
  const response = await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method,
      params,
    }),
  });
  if (!response.ok) {
    throw new Error(`${method} HTTP ${response.status}`);
  }
  const payload = await response.json();
  if (payload.error) {
    throw new Error(`${method} RPC error: ${payload.error.message || JSON.stringify(payload.error)}`);
  }
  return payload.result;
}

function hexToBigInt(value, label) {
  if (typeof value !== 'string' || !/^0x[0-9a-fA-F]+$/.test(value)) {
    throw new Error(`${label} must be a hex quantity`);
  }
  return BigInt(value);
}

function blockTag(blockNumber) {
  return `0x${blockNumber.toString(16)}`;
}

async function requireCode(rpcUrl, address, label) {
  const code = await rpcCall(rpcUrl, 'eth_getCode', [address, 'latest']);
  if (typeof code !== 'string' || code === '0x') {
    throw new Error(`${label} has no bytecode at ${address}`);
  }
  return {
    label,
    address,
    code_bytes: (code.length - 2) / 2,
  };
}

async function requireReceipt(rpcUrl, txHash, label) {
  const receipt = await rpcCall(rpcUrl, 'eth_getTransactionReceipt', [txHash]);
  if (!receipt) {
    throw new Error(`${label} receipt not found for ${txHash}`);
  }
  if (receipt.status !== '0x1') {
    throw new Error(`${label} transaction did not succeed: ${txHash}`);
  }
  return {
    label,
    tx_hash: txHash,
    block_number: hexToBigInt(receipt.blockNumber, `${label}.blockNumber`),
    gas_used: hexToBigInt(receipt.gasUsed, `${label}.gasUsed`),
  };
}

async function callStateView(rpcUrl, stateView, selector, poolId, blockNumber, label) {
  const data = `${selector}${normalizeBytes32NoPrefix(poolId, 'uniswap_pool_id')}`;
  const result = await rpcCall(rpcUrl, 'eth_call', [{ to: stateView, data }, blockTag(blockNumber)]);
  if (typeof result !== 'string' || result.length < 66) {
    throw new Error(`${label} returned invalid data`);
  }
  return result;
}

async function getLiquidityAt(rpcUrl, stateView, poolId, blockNumber, label) {
  const result = await callStateView(rpcUrl, stateView, '0xfa6793d5', poolId, blockNumber, label);
  const liquidity = BigInt(`0x${result.slice(-64)}`);
  if (liquidity > U128_MAX) {
    throw new Error(`${label} liquidity exceeds u128`);
  }
  return liquidity;
}

async function getSlot0At(rpcUrl, stateView, poolId, blockNumber, label) {
  const result = await callStateView(rpcUrl, stateView, '0xc815641c', poolId, blockNumber, label);
  if ((result.length - 2) < 64 * 4) {
    throw new Error(`${label} getSlot0 returned too few words`);
  }
  const words = result.slice(2).match(/.{64}/g) || [];
  return {
    sqrt_price_x96: BigInt(`0x${words[0]}`).toString(),
    tick_word: `0x${words[1]}`,
    protocol_fee: BigInt(`0x${words[2]}`).toString(),
    lp_fee: BigInt(`0x${words[3]}`).toString(),
  };
}

function assertAscending(receipts) {
  for (let i = 1; i < receipts.length; i += 1) {
    if (receipts[i].block_number < receipts[i - 1].block_number) {
      throw new Error(
        `${receipts[i].label} block ${receipts[i].block_number.toString()} precedes ${receipts[
          i - 1
        ].label} block ${receipts[i - 1].block_number.toString()}`,
      );
    }
  }
}

function renderJson(value, indent = 0) {
  if (value && typeof value === 'object' && Object.hasOwn(value, '__rawNumber')) {
    return value.__rawNumber;
  }
  if (value === null || typeof value === 'string' || typeof value === 'boolean') {
    return JSON.stringify(value);
  }
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new Error(`cannot render unsafe numeric value: ${value}`);
    }
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return '[]';
    }
    const childIndent = indent + 2;
    const body = value.map((item) => `${' '.repeat(childIndent)}${renderJson(item, childIndent)}`).join(',\n');
    return `[\n${body}\n${' '.repeat(indent)}]`;
  }
  if (value && typeof value === 'object') {
    const entries = Object.entries(value);
    if (entries.length === 0) {
      return '{}';
    }
    const childIndent = indent + 2;
    const body = entries
      .map(([key, item]) => `${' '.repeat(childIndent)}${JSON.stringify(key)}: ${renderJson(item, childIndent)}`)
      .join(',\n');
    return `{\n${body}\n${' '.repeat(indent)}}`;
  }
  throw new Error(`cannot render value of type ${typeof value}`);
}

function assertPositive(value, label) {
  if (value <= 0n) {
    throw new Error(`${label} must be greater than zero`);
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    process.stdout.write(HELP.trimStart());
    return;
  }

  const launchConfigFile = requireArg(args, 'launchConfigFile');
  const launchConfigRaw = await readFile(launchConfigFile, 'utf8');
  const launchConfig = JSON.parse(launchConfigRaw);
  const orderedConfig = orderedLaunchConfig(launchConfig);
  if (orderedConfig.schema !== EXPECTED_LAUNCH_SCHEMA) {
    throw new Error(`unsupported launch config schema: ${orderedConfig.schema}`);
  }
  if (!orderedConfig.fork_rehearsal_required) {
    throw new Error('launch config must require fork rehearsal');
  }

  const suppliedDigest = args.launchConfigDigest
    ? normalizeHash48(args.launchConfigDigest, 'launch_config_digest')
    : null;
  let launchConfigDigest = suppliedDigest;
  if (suppliedDigest) {
    launchConfigDigest = suppliedDigest;
  } else if (!maybeLargeIntegerLiteral(launchConfigRaw)) {
    const computed = digestLaunchConfig(launchConfig);
    launchConfigDigest = computed;
  } else {
    throw new Error(
      'launch config contains large integer literals; pass --launch-config-digest from the node launch-config template report',
    );
  }

  const rpcUrl = args.rpcUrl || process.env.ETHEREUM_RPC_URL || process.env.MAINNET_RPC_URL;
  if (!rpcUrl) {
    throw new Error('missing --rpc-url, ETHEREUM_RPC_URL, or MAINNET_RPC_URL');
  }

  const chainId = Number(hexToBigInt(await rpcCall(rpcUrl, 'eth_chainId', []), 'eth_chainId'));
  if (chainId !== orderedConfig.official_uniswap.chain_id) {
    throw new Error(
      `RPC chain id ${chainId} does not match launch config chain id ${orderedConfig.official_uniswap.chain_id}`,
    );
  }

  const codeChecks = await Promise.all([
    requireCode(rpcUrl, orderedConfig.official_uniswap.pool_manager, 'official_uniswap.pool_manager'),
    requireCode(rpcUrl, orderedConfig.official_uniswap.position_manager, 'official_uniswap.position_manager'),
    requireCode(rpcUrl, orderedConfig.official_uniswap.universal_router, 'official_uniswap.universal_router'),
    requireCode(rpcUrl, orderedConfig.official_uniswap.permit2, 'official_uniswap.permit2'),
    requireCode(rpcUrl, orderedConfig.official_uniswap.state_view, 'official_uniswap.state_view'),
    requireCode(rpcUrl, orderedConfig.wrapped_navcoin_token, 'wrapped_navcoin_token'),
    requireCode(rpcUrl, orderedConfig.usdc_token, 'usdc_token'),
    requireCode(rpcUrl, orderedConfig.handoff_controller, 'handoff_controller'),
    requireCode(rpcUrl, orderedConfig.receipt_verifier, 'receipt_verifier'),
    requireCode(rpcUrl, orderedConfig.settlement_adapter, 'settlement_adapter'),
  ]);

  const txHashes = {
    seedMint: normalizeTxHash(requireArg(args, 'seedMintTx'), 'seed_mint_tx'),
    seedLp: normalizeTxHash(requireArg(args, 'seedLpTx'), 'seed_lp_tx'),
    externalBuy: normalizeTxHash(requireArg(args, 'externalBuyTx'), 'external_buy_tx'),
    externalSell: normalizeTxHash(requireArg(args, 'externalSellTx'), 'external_sell_tx'),
    mintOnlyPacket: normalizeTxHash(requireArg(args, 'mintOnlyPacketTx'), 'mint_only_packet_tx'),
    mintAndSwapPacket: normalizeTxHash(requireArg(args, 'mintAndSwapPacketTx'), 'mint_and_swap_packet_tx'),
  };

  const receipts = await Promise.all([
    requireReceipt(rpcUrl, txHashes.seedMint, 'seed_mint_tx'),
    requireReceipt(rpcUrl, txHashes.seedLp, 'seed_lp_tx'),
    requireReceipt(rpcUrl, txHashes.externalBuy, 'external_buy_tx'),
    requireReceipt(rpcUrl, txHashes.externalSell, 'external_sell_tx'),
    requireReceipt(rpcUrl, txHashes.mintOnlyPacket, 'mint_only_packet_tx'),
    requireReceipt(rpcUrl, txHashes.mintAndSwapPacket, 'mint_and_swap_packet_tx'),
  ]);
  assertAscending(receipts.slice(0, 4));

  const receiptByLabel = Object.fromEntries(receipts.map((receipt) => [receipt.label, receipt]));
  const liquidityAfterSeed = await getLiquidityAt(
    rpcUrl,
    orderedConfig.official_uniswap.state_view,
    orderedConfig.uniswap_pool_id,
    receiptByLabel.seed_lp_tx.block_number,
    'state_view_liquidity_after_seed',
  );
  const liquidityAfterBuy = await getLiquidityAt(
    rpcUrl,
    orderedConfig.official_uniswap.state_view,
    orderedConfig.uniswap_pool_id,
    receiptByLabel.external_buy_tx.block_number,
    'state_view_liquidity_after_buy',
  );
  const liquidityAfterSell = await getLiquidityAt(
    rpcUrl,
    orderedConfig.official_uniswap.state_view,
    orderedConfig.uniswap_pool_id,
    receiptByLabel.external_sell_tx.block_number,
    'state_view_liquidity_after_sell',
  );
  assertPositive(liquidityAfterSeed, 'state_view_liquidity_after_seed');
  assertPositive(liquidityAfterBuy, 'state_view_liquidity_after_buy');
  assertPositive(liquidityAfterSell, 'state_view_liquidity_after_sell');

  const slot0 = {
    after_seed: await getSlot0At(
      rpcUrl,
      orderedConfig.official_uniswap.state_view,
      orderedConfig.uniswap_pool_id,
      receiptByLabel.seed_lp_tx.block_number,
      'state_view_slot0_after_seed',
    ),
    after_buy: await getSlot0At(
      rpcUrl,
      orderedConfig.official_uniswap.state_view,
      orderedConfig.uniswap_pool_id,
      receiptByLabel.external_buy_tx.block_number,
      'state_view_slot0_after_buy',
    ),
    after_sell: await getSlot0At(
      rpcUrl,
      orderedConfig.official_uniswap.state_view,
      orderedConfig.uniswap_pool_id,
      receiptByLabel.external_sell_tx.block_number,
      'state_view_slot0_after_sell',
    ),
  };

  const userBuyUsdcSpent = parseDecimal(requireArg(args, 'userBuyUsdcSpentAtoms'), 'user_buy_usdc_spent_atoms');
  const userBuyWrappedReceived = parseDecimal(
    requireArg(args, 'userBuyWrappedReceivedAtoms'),
    'user_buy_wrapped_received_atoms',
  );
  const userSellWrappedSpent = parseDecimal(
    requireArg(args, 'userSellWrappedSpentAtoms'),
    'user_sell_wrapped_spent_atoms',
  );
  const userSellUsdcReceived = parseDecimal(
    requireArg(args, 'userSellUsdcReceivedAtoms'),
    'user_sell_usdc_received_atoms',
  );
  const canonicalSupplyBefore = parseDecimal(
    requireArg(args, 'canonicalSupplyBeforeExternalTradesAtoms'),
    'canonical_supply_before_external_trades_atoms',
  );
  const canonicalSupplyAfter = parseDecimal(
    requireArg(args, 'canonicalSupplyAfterExternalTradesAtoms'),
    'canonical_supply_after_external_trades_atoms',
  );
  assertPositive(userBuyUsdcSpent, 'user_buy_usdc_spent_atoms');
  assertPositive(userBuyWrappedReceived, 'user_buy_wrapped_received_atoms');
  assertPositive(userSellWrappedSpent, 'user_sell_wrapped_spent_atoms');
  assertPositive(userSellUsdcReceived, 'user_sell_usdc_received_atoms');
  assertPositive(canonicalSupplyBefore, 'canonical_supply_before_external_trades_atoms');
  if (canonicalSupplyAfter !== canonicalSupplyBefore) {
    throw new Error('canonical supply changed during external Uniswap buy/sell rehearsal');
  }

  const packetConsumedWithoutManualMint = parseBoolean(
    args.packetConsumedWithoutManualMint,
    'packet_consumed_without_manual_mint',
    true,
  );
  const minOutputFailureRevertedWithoutConsume = parseBoolean(
    args.minOutputFailureRevertedWithoutConsume,
    'min_output_failure_reverted_without_consume',
    true,
  );
  if (!packetConsumedWithoutManualMint) {
    throw new Error('packet_consumed_without_manual_mint must be true');
  }
  if (!minOutputFailureRevertedWithoutConsume) {
    throw new Error('min_output_failure_reverted_without_consume must be true');
  }

  const forkBlockNumber = args.forkBlockNumber
    ? parseDecimal(args.forkBlockNumber, 'fork_block_number')
    : receiptByLabel.external_sell_tx.block_number;
  const outputFile = args.outputFile || 'docs/plans/pftl-uniswap-fork-rehearsal-evidence.json';
  const evidence = {
    schema: EXPECTED_EVIDENCE_SCHEMA,
    rehearsal_id: args.rehearsalId || `gate3-${new Date().toISOString()}`,
    launch_config_digest: launchConfigDigest,
    route_config_digest: orderedConfig.route_config_digest,
    fork_chain_id: chainId,
    fork_block_number: rawNumber(forkBlockNumber, 'fork_block_number', U64_MAX),
    official_uniswap: launchConfig.official_uniswap,
    uniswap_pool_key_hash: launchConfig.uniswap_pool_key_hash,
    uniswap_pool_id: launchConfig.uniswap_pool_id,
    seed_export_packet_hash: normalizeHash48(requireArg(args, 'seedExportPacketHash'), 'seed_export_packet_hash'),
    seed_receipt_root: normalizeHash48(requireArg(args, 'seedReceiptRoot'), 'seed_receipt_root'),
    seed_mint_tx_hash: txHashNoPrefix(txHashes.seedMint),
    seed_lp_tx_hash: txHashNoPrefix(txHashes.seedLp),
    external_buy_tx_hash: txHashNoPrefix(txHashes.externalBuy),
    external_sell_tx_hash: txHashNoPrefix(txHashes.externalSell),
    mint_only_packet_tx_hash: txHashNoPrefix(txHashes.mintOnlyPacket),
    mint_and_swap_packet_tx_hash: txHashNoPrefix(txHashes.mintAndSwapPacket),
    state_view_liquidity_after_seed: rawNumber(liquidityAfterSeed, 'state_view_liquidity_after_seed', U128_MAX),
    state_view_liquidity_after_buy: rawNumber(liquidityAfterBuy, 'state_view_liquidity_after_buy', U128_MAX),
    state_view_liquidity_after_sell: rawNumber(liquidityAfterSell, 'state_view_liquidity_after_sell', U128_MAX),
    user_buy_usdc_spent_atoms: rawNumber(userBuyUsdcSpent, 'user_buy_usdc_spent_atoms', U64_MAX),
    user_buy_wrapped_received_atoms: rawNumber(
      userBuyWrappedReceived,
      'user_buy_wrapped_received_atoms',
      U64_MAX,
    ),
    user_sell_wrapped_spent_atoms: rawNumber(userSellWrappedSpent, 'user_sell_wrapped_spent_atoms', U64_MAX),
    user_sell_usdc_received_atoms: rawNumber(userSellUsdcReceived, 'user_sell_usdc_received_atoms', U64_MAX),
    canonical_supply_before_external_trades_atoms: rawNumber(
      canonicalSupplyBefore,
      'canonical_supply_before_external_trades_atoms',
      U64_MAX,
    ),
    canonical_supply_after_external_trades_atoms: rawNumber(
      canonicalSupplyAfter,
      'canonical_supply_after_external_trades_atoms',
      U64_MAX,
    ),
    packet_consumed_without_manual_mint: packetConsumedWithoutManualMint,
    min_output_failure_reverted_without_consume: minOutputFailureRevertedWithoutConsume,
  };

  const rendered = `${renderJson(evidence)}\n`;
  await writeFile(outputFile, rendered, 'utf8');

  const summary = {
    output_file: path.resolve(outputFile),
    launch_config_digest: launchConfigDigest,
    chain_id: chainId,
    fork_block_number: forkBlockNumber.toString(),
    code_checks: codeChecks,
    tx_blocks: receipts.map((receipt) => ({
      label: receipt.label,
      tx_hash: receipt.tx_hash,
      block_number: receipt.block_number.toString(),
      gas_used: receipt.gas_used.toString(),
    })),
    state_view: {
      liquidity_after_seed: liquidityAfterSeed.toString(),
      liquidity_after_buy: liquidityAfterBuy.toString(),
      liquidity_after_sell: liquidityAfterSell.toString(),
      slot0,
    },
  };
  process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
}

main().catch((error) => {
  process.stderr.write(`error: ${error.message}\n`);
  process.exitCode = 1;
});
