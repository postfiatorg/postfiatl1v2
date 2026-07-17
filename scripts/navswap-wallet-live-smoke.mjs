#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import WebSocket from '../wallet-proxy/node_modules/ws/index.js';

import { RpcClient } from '../wallet-web/src/lib/rpc-client.js';
import { SwapServer } from '../wallet-web/src/lib/swap-server.js';
import { TxBuilder } from '../wallet-web/src/lib/tx-builder.js';
import { submitNavswapPreparedAssetActions } from '../wallet-web/src/lib/navswap-actions.js';
import { A651_ASSET_ID, PFUSDC_ASSET_ID } from '../wallet-web/src/lib/utils.js';
import * as wasm from '../wallet-web/src/wasm/postfiat_wallet_wasm.js';

globalThis.WebSocket = WebSocket;

const DEFAULT_PROXY = 'http://127.0.0.1:8080';
const DEFAULT_RPC = 'ws://127.0.0.1:8080/rpc';
const DEFAULT_CHAIN_ID = 'postfiat-wan-devnet';
let runtimePfusdcAssetId = PFUSDC_ASSET_ID;
let runtimeA651AssetId = A651_ASSET_ID;

function usage() {
  return `Usage:
  node scripts/navswap-wallet-live-smoke.mjs --wallet-address pf... [--amount 0.5] [--from-asset pfUSDC --to-asset a651] [--out-dir DIR]
  node scripts/navswap-wallet-live-smoke.mjs --execute --wallet-backup-file buyer.backup.json [--amount 0.5] [--from-asset pfUSDC --to-asset a651] [--out-dir DIR]
  node scripts/navswap-wallet-live-smoke.mjs --stream-run-id navswap-... [--out-dir DIR]

Dry-run mode quotes the live transparent NAVSwap route and proves the wallet asset feed.
Execution mode additionally signs wallet-owned actions with wallet-web code and starts the operator completion run.
Stream mode records the adapter SSE stream/status/receipts for an existing NAVSwap run.

Options:
  --stream-run-id RUN_ID      Record /api/navswap/runs/{run_id}/stream evidence without wallet access.
  --wallet-address ADDR       Wallet address for dry-run readiness.
  --wallet-backup-file FILE   WalletBackupFile JSON containing master_seed_hex; required for --execute.
  --execute                   Move live funds. Refuses to run without --wallet-backup-file.
  --no-auto-fund              In --execute mode, refuse instead of requesting guarded pfUSDC funding.
  --amount N                  a651 display amount to request; decimals are supported. Default: 1.
  --from-asset SYMBOL         Source asset symbol. Default: pfUSDC.
  --to-asset SYMBOL           Destination asset symbol. Default: a651.
  --pfusdc-asset-id ID        Expected pfUSDC id for balance verification.
  --a651-asset-id ID          Expected a651 id for balance verification.
  --proxy URL                 Wallet proxy HTTP base. Default: ${DEFAULT_PROXY}
  --rpc URL                   Wallet proxy WebSocket RPC. Default: ${DEFAULT_RPC}
  --out-dir DIR               Evidence directory. Default: /tmp/navswap-wallet-live-smoke-<timestamp>
  --timeout-ms N              Poll/feed timeout. Default: 60000.
`;
}

function parseArgs(argv) {
  const args = {
    amount: '1',
    proxy: DEFAULT_PROXY,
    rpc: DEFAULT_RPC,
    chainId: DEFAULT_CHAIN_ID,
    timeoutMs: 60000,
    execute: false,
    autoFund: true,
    fromAsset: 'pfUSDC',
    toAsset: 'a651',
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--help' || arg === '-h') {
      args.help = true;
    } else if (arg === '--execute') {
      args.execute = true;
    } else if (arg === '--no-auto-fund') {
      args.autoFund = false;
    } else if (arg.startsWith('--')) {
      const key = arg.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
      const value = argv[i + 1];
      if (value === undefined || value.startsWith('--')) {
        throw new Error(`${arg} requires a value`);
      }
      args[key] = value;
      i += 1;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  args.timeoutMs = Number.parseInt(args.timeoutMs, 10);
  if (!Number.isFinite(args.timeoutMs) || args.timeoutMs <= 0) {
    throw new Error('--timeout-ms must be a positive integer');
  }
  if (!/^(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)$/.test(String(args.amount)) || Number(args.amount) <= 0) {
    throw new Error('--amount must be a positive a651 display amount');
  }
  if (!args.fromAsset || !args.toAsset || args.fromAsset === args.toAsset) {
    throw new Error('--from-asset and --to-asset must be distinct symbols');
  }
  return args;
}

function jsonReplacer(_key, value) {
  return typeof value === 'bigint' ? value.toString() : value;
}

async function writeJson(file, value) {
  await fs.writeFile(file, `${JSON.stringify(value, jsonReplacer, 2)}\n`);
}

async function initWalletWasm() {
  const wasmPath = new URL('../wallet-web/src/wasm/postfiat_wallet_wasm_bg.wasm', import.meta.url);
  const bytes = await fs.readFile(wasmPath);
  wasm.initSync({ module: bytes });
  return wasm;
}

async function loadWalletFromBackup(file, expectedAddress = null) {
  const raw = await fs.readFile(file, 'utf8');
  const parsed = JSON.parse(raw);
  const backup = parsed.backup_json ? JSON.parse(parsed.backup_json) : parsed;
  if (!backup.master_seed_hex) {
    throw new Error('wallet backup file must contain master_seed_hex; private-key .key.json files are not accepted');
  }
  const keygen = wasm.wallet_keygen(
    backup.chain_id || DEFAULT_CHAIN_ID,
    backup.master_seed_hex,
    Number.parseInt(backup.account_index ?? 0, 10),
  );
  if (expectedAddress && keygen.address !== expectedAddress) {
    throw new Error(`backup address ${keygen.address} does not match --wallet-address ${expectedAddress}`);
  }
  return {
    address: keygen.address,
    publicKeyHex: keygen.public_key_hex,
    backupJson: keygen.backup_json,
  };
}

function assetItems(result) {
  if (Array.isArray(result)) return result;
  if (Array.isArray(result?.assets)) return result.assets;
  return [];
}

function canonicalBalances(assetResult) {
  const balances = { pfUSDC: 0n, a651: 0n };
  for (const item of assetItems(assetResult)) {
    const id = item.asset_id || item.id;
    const value = BigInt(item.balance ?? item.amount ?? 0);
    if (id === runtimePfusdcAssetId) balances.pfUSDC += value;
    if (id === runtimeA651AssetId) balances.a651 += value;
  }
  return balances;
}

function snapshotBalances(snapshot) {
  return canonicalBalances(snapshot?.assets || null);
}

function compactFeedSnapshot(snapshot) {
  return {
    schema: snapshot?.schema || null,
    address: snapshot?.address || null,
    include_assets: snapshot?.include_assets === true,
    observed_at_ms: snapshot?.observed_at_ms || null,
    assets_error: snapshot?.assets_error || null,
    balances: snapshotBalances(snapshot),
  };
}

async function startAssetFeed(rpc, address, timeoutMs) {
  const snapshots = [];
  const waiters = [];
  const subscription = await rpc.walletSubscribe({
    address,
    include_assets: true,
    interval_ms: 1500,
  }, (snapshot, meta) => {
    const entry = { snapshot, meta };
    snapshots.push(entry);
    for (const waiter of [...waiters]) {
      if (waiter.predicate(snapshot, meta)) {
        waiter.resolve(entry);
        waiters.splice(waiters.indexOf(waiter), 1);
      }
    }
  });

  const waitFor = (predicate, label) => new Promise((resolve, reject) => {
    for (const entry of snapshots) {
      if (predicate(entry.snapshot, entry.meta)) {
        resolve(entry);
        return;
      }
    }
    const timer = setTimeout(() => {
      const index = waiters.indexOf(waiter);
      if (index >= 0) waiters.splice(index, 1);
      reject(new Error(`timed out waiting for wallet asset feed: ${label}`));
    }, timeoutMs);
    const waiter = {
      predicate,
      resolve: (entry) => {
        clearTimeout(timer);
        resolve(entry);
      },
      reject,
    };
    waiters.push(waiter);
  });

  return {
    subscription,
    snapshots,
    waitFor,
    async close() {
      await subscription.unsubscribe();
    },
  };
}

function terminalRun(status) {
  if (!status) return false;
  if (status.terminal === true) return true;
  if (status.ok === false) return true;
  return [
    'operator_mint_submitted',
    'operator_redeem_settle_submitted',
    'destination_consume_submitted',
    'complete',
    'transparent_complete',
    'failed',
  ].includes(status.status);
}

async function pollRun(swapServer, runId, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let status = null;
  let events = null;
  let receipts = null;
  while (Date.now() < deadline) {
    status = await swapServer.getNavswapRun(runId);
    events = await swapServer.getNavswapRunEvents(runId).catch(() => null);
    receipts = await swapServer.getNavswapRunReceipts(runId).catch(() => null);
    if (terminalRun(status)) return { status, events, receipts };
    await new Promise(resolve => setTimeout(resolve, 1500));
  }
  throw new Error(`timed out waiting for NAVSwap run ${runId}`);
}

function parseSseBlock(block) {
  const lines = block.split(/\r?\n/);
  let event = 'message';
  const data = [];
  for (const line of lines) {
    if (!line || line.startsWith(':')) continue;
    if (line.startsWith('event:')) {
      event = line.slice('event:'.length).trim();
    } else if (line.startsWith('data:')) {
      data.push(line.slice('data:'.length).trimStart());
    }
  }
  if (!data.length) return null;
  const text = data.join('\n');
  let payload = null;
  try {
    payload = JSON.parse(text);
  } catch (_) {
    payload = text;
  }
  return { event, payload };
}

async function collectRunStream(swapServer, runId, timeoutMs) {
  const streamUrl = swapServer.navswapRunStreamUrl(runId);
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const events = [];
  let terminal = false;
  let terminalEvent = null;
  let buffer = '';

  try {
    const response = await fetch(streamUrl, {
      headers: { Accept: 'text/event-stream' },
      signal: controller.signal,
    });
    if (!response.ok || !response.body) {
      throw new Error(`NAVSwap run stream returned HTTP ${response.status}`);
    }
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    while (!terminal) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      let idx;
      while ((idx = buffer.indexOf('\n\n')) >= 0) {
        const block = buffer.slice(0, idx);
        buffer = buffer.slice(idx + 2);
        const parsed = parseSseBlock(block);
        if (!parsed) continue;
        events.push(parsed);
        if (parsed.event === 'navswap_run_done' || parsed.payload?.terminal === true) {
          terminal = true;
          terminalEvent = parsed;
          break;
        }
      }
    }
    return {
      ok: true,
      schema: 'postfiat-navswap-smoke-run-stream-v1',
      run_id: runId,
      stream_url: streamUrl,
      terminal,
      event_count: events.length,
      events,
      terminal_event: terminalEvent,
    };
  } catch (error) {
    if (error?.name === 'AbortError') {
      return {
        ok: false,
        schema: 'postfiat-navswap-smoke-run-stream-v1',
        run_id: runId,
        stream_url: streamUrl,
        terminal,
        event_count: events.length,
        events,
        code: 'navswap_run_stream_timeout',
        message: `timed out waiting for NAVSwap run stream ${runId}`,
      };
    }
    return {
      ok: false,
      schema: 'postfiat-navswap-smoke-run-stream-v1',
      run_id: runId,
      stream_url: streamUrl,
      terminal,
      event_count: events.length,
      events,
      code: 'navswap_run_stream_failed',
      message: error?.message || 'NAVSwap run stream failed',
    };
  } finally {
    clearTimeout(timer);
    controller.abort();
  }
}

async function pollReadiness(swapServer, request, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() <= deadline) {
    last = await swapServer.getNavswapReadiness(request);
    if (predicate(last)) return last;
    await new Promise(resolve => setTimeout(resolve, 1500));
  }
  const nextSteps = Array.isArray(last?.next_steps) ? last.next_steps.join(', ') : 'n/a';
  throw new Error(`timed out waiting for NAVSwap readiness: ${label}; last status=${last?.status || 'unknown'} next=${nextSteps}`);
}

function navswapDirection(args, quote = null) {
  if (quote?.direction) return quote.direction;
  return String(args.fromAsset).toLowerCase() === 'a651' ? 'redeem' : 'subscribe';
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  runtimePfusdcAssetId = args.pfusdcAssetId || PFUSDC_ASSET_ID;
  runtimeA651AssetId = args.a651AssetId || A651_ASSET_ID;
  if (args.help) {
    process.stdout.write(usage());
    return;
  }
  if (args.streamRunId) {
    const outDir = args.outDir || path.join('/tmp', `navswap-run-stream-${new Date().toISOString().replace(/[:.]/g, '')}`);
    await fs.mkdir(outDir, { recursive: true });
    const swapServer = new SwapServer(args.proxy);
    const [status, events, receipts, stream] = await Promise.all([
      swapServer.getNavswapRun(args.streamRunId),
      swapServer.getNavswapRunEvents(args.streamRunId).catch(error => ({
        ok: false,
        message: error.message,
      })),
      swapServer.getNavswapRunReceipts(args.streamRunId).catch(error => ({
        ok: false,
        message: error.message,
      })),
      collectRunStream(swapServer, args.streamRunId, args.timeoutMs),
    ]);
    await writeJson(path.join(outDir, 'run-status.json'), status);
    await writeJson(path.join(outDir, 'run-events.json'), events);
    await writeJson(path.join(outDir, 'run-receipts.json'), receipts);
    await writeJson(path.join(outDir, 'run-stream.json'), stream);
    const summary = {
      ok: status?.ok !== false && stream.ok === true,
      mode: 'stream',
      run_id: args.streamRunId,
      out_dir: outDir,
      run_status: status?.status || null,
      run_status_terminal: status?.terminal === true,
      stream_terminal: stream.terminal === true,
      stream_event_count: stream.event_count,
      receipt_count: Array.isArray(receipts?.receipts) ? receipts.receipts.length : null,
    };
    await writeJson(path.join(outDir, 'summary.json'), summary);
    process.stdout.write(`${JSON.stringify(summary, jsonReplacer, 2)}\n`);
    return;
  }
  if (args.execute && !args.walletBackupFile) {
    throw new Error('--execute requires --wallet-backup-file');
  }
  if (!args.walletAddress && !args.walletBackupFile) {
    throw new Error('provide --wallet-address or --wallet-backup-file');
  }

  await initWalletWasm();
  const wallet = args.walletBackupFile
    ? await loadWalletFromBackup(args.walletBackupFile, args.walletAddress || null)
    : { address: args.walletAddress, publicKeyHex: null, backupJson: null };
  const outDir = args.outDir || path.join('/tmp', `navswap-wallet-live-smoke-${new Date().toISOString().replace(/[:.]/g, '')}`);
  await fs.mkdir(outDir, { recursive: true });

  const rpc = new RpcClient(args.rpc);
  const swapServer = new SwapServer(args.proxy);
  let feed = null;
  try {
    const capabilities = await swapServer.getNavswapCapabilities();
    await writeJson(path.join(outDir, 'capabilities.json'), capabilities);

    feed = await startAssetFeed(rpc, wallet.address, args.timeoutMs);
    const beforeFeed = await feed.waitFor(
      snapshot => snapshot?.include_assets === true && snapshot.assets && !snapshot.assets_error,
      'initial issued-asset snapshot',
    );
    const before = compactFeedSnapshot(beforeFeed.snapshot);
    await writeJson(path.join(outDir, 'feed-before.json'), before);

    const readinessRequest = {
      route: 'transparent_navswap',
      from_asset: args.fromAsset,
      to_asset: args.toAsset,
      amount: args.amount,
      wallet_address: wallet.address,
      auto_plan: true,
    };
    let adapterReadiness = await swapServer.getNavswapReadiness(readinessRequest);
    await writeJson(path.join(outDir, 'adapter-readiness.json'), adapterReadiness);
    let quote = adapterReadiness.quote?.ok === true
      ? adapterReadiness.quote
      : await swapServer.quoteNavswap({
        route: 'transparent_navswap',
        from_asset: args.fromAsset,
        to_asset: args.toAsset,
        amount: args.amount,
        wallet_address: wallet.address,
        auto_plan: true,
      });
    await writeJson(path.join(outDir, 'quote.json'), quote);

    let requiredSettlement = BigInt(quote.settlement_amount_atoms || 0);
    let mintAmount = BigInt(quote.mint_amount_atoms || quote.expected_output || args.amount);
    let redeemAmount = BigInt(quote.redeem_amount_atoms || (navswapDirection(args, quote) === 'redeem' ? quote.input_amount_atoms : 0) || 0);
    const readiness = {
      ok: quote.ok === true,
      direction: navswapDirection(args, quote),
      wallet_address: wallet.address,
      execute_requested: args.execute,
      auto_fund: args.autoFund,
      quote_status: quote.status || null,
      required_settlement_atoms: requiredSettlement,
      mint_amount_atoms: mintAmount,
      redeem_amount_atoms: redeemAmount,
      before_balances: before.balances,
      settlement_sufficient: before.balances.pfUSDC >= requiredSettlement,
      settlement_asset: adapterReadiness.settlement_asset || null,
      funding: adapterReadiness.funding || null,
      prepared_action_count: Array.isArray(quote.prepared_action_batch?.actions)
        ? quote.prepared_action_batch.actions.length
        : 0,
      prepared_stages: Array.isArray(quote.prepared_action_batch?.actions)
        ? quote.prepared_action_batch.actions.map(action => action.stage)
        : [],
      adapter_readiness_status: adapterReadiness.status || null,
      adapter_can_execute: adapterReadiness.can_execute === true,
      adapter_next_steps: Array.isArray(adapterReadiness.next_steps) ? adapterReadiness.next_steps : [],
    };
    await writeJson(path.join(outDir, 'readiness.json'), readiness);

    if (!args.execute) {
      await writeJson(path.join(outDir, 'summary.json'), {
        ok: true,
        mode: 'dry-run',
        wallet_address: wallet.address,
        out_dir: outDir,
        readiness,
        message: 'Dry-run complete. Re-run with --execute and --wallet-backup-file to move live funds.',
      });
      process.stdout.write(`${JSON.stringify({ ok: true, mode: 'dry-run', out_dir: outDir, readiness }, jsonReplacer, 2)}\n`);
      return;
    }

    if (quote.ok !== true || quote.status !== 'prepared_actions_ready') {
      throw new Error(quote.message || 'NAVSwap quote did not return a prepared action batch');
    }
    const txBuilder = new TxBuilder(rpc, async () => wasm);
    let fundingResult = null;
    let preSwap = before;
    let executionReadiness = adapterReadiness;

    if (executionReadiness.settlement_asset?.trustline_usable === false) {
      throw new Error(executionReadiness.next_steps?.[0] || 'NAVSwap readiness still reports a settlement trustline requirement');
    }

    if (executionReadiness.quote?.ok === true) {
      quote = executionReadiness.quote;
      await writeJson(path.join(outDir, 'quote-execution-pre-funding.json'), quote);
      requiredSettlement = BigInt(quote.settlement_amount_atoms || 0);
      mintAmount = BigInt(quote.mint_amount_atoms || quote.expected_output || args.amount);
      redeemAmount = BigInt(quote.redeem_amount_atoms || (navswapDirection(args, quote) === 'redeem' ? quote.input_amount_atoms : 0) || 0);
    }

    if (navswapDirection(args, quote) === 'subscribe' && executionReadiness.settlement_asset?.sufficient !== true) {
      if (!args.autoFund) {
        throw new Error(`insufficient pfUSDC: requires ${requiredSettlement}, available ${executionReadiness.settlement_asset?.balance_atoms || before.balances.pfUSDC}`);
      }
      if (executionReadiness.funding?.available !== true) {
        throw new Error(executionReadiness.funding?.unavailable_reason || executionReadiness.next_steps?.[0] || 'guarded pfUSDC funding is unavailable');
      }
      const fundingAmountAtoms = String(executionReadiness.funding.amount_atoms || executionReadiness.settlement_asset.shortfall_atoms || '0');
      const balanceBeforeFunding = BigInt(String(executionReadiness.settlement_asset.balance_atoms || '0'));
      fundingResult = await swapServer.fundNavswapPfusdc({
        route: 'transparent_navswap',
        from_asset: args.fromAsset,
        to_asset: args.toAsset,
        amount: args.amount,
        wallet_address: wallet.address,
        amount_atoms: fundingAmountAtoms,
      });
      await writeJson(path.join(outDir, 'funding-result.json'), fundingResult);
      const minFundedBalance = balanceBeforeFunding + BigInt(fundingResult.amount_atoms || fundingAmountAtoms);
      const fundedFeed = await feed.waitFor((snapshot) => {
        const balances = snapshotBalances(snapshot);
        return balances.pfUSDC >= minFundedBalance;
      }, 'guarded pfUSDC funding balance');
      preSwap = compactFeedSnapshot(fundedFeed.snapshot);
      await writeJson(path.join(outDir, 'feed-funded.json'), preSwap);
      executionReadiness = await pollReadiness(
        swapServer,
        readinessRequest,
        result => result?.can_execute === true,
        'ready to submit wallet-owned NAVSwap actions',
        args.timeoutMs,
      );
      await writeJson(path.join(outDir, 'adapter-readiness-after-funding.json'), executionReadiness);
    }

    if (executionReadiness.quote?.ok === true) {
      quote = executionReadiness.quote;
      await writeJson(path.join(outDir, 'quote-execution.json'), quote);
      requiredSettlement = BigInt(quote.settlement_amount_atoms || 0);
      mintAmount = BigInt(quote.mint_amount_atoms || quote.expected_output || args.amount);
      redeemAmount = BigInt(quote.redeem_amount_atoms || (navswapDirection(args, quote) === 'redeem' ? quote.input_amount_atoms : 0) || 0);
    }
    if (executionReadiness.can_execute !== true) {
      throw new Error(executionReadiness.next_steps?.[0] || 'NAVSwap route is not ready to submit wallet-owned actions');
    }

    const walletActionResult = await submitNavswapPreparedAssetActions({
      requests: quote.prepared_action_batch,
      walletAddress: wallet.address,
      backupJson: wallet.backupJson,
      txBuilder,
    });
    await writeJson(path.join(outDir, 'wallet-action-result.json'), walletActionResult);

    const runRequest = {
      route: 'transparent_navswap',
      wallet_address: wallet.address,
      quote,
      wallet_action_result: walletActionResult,
      async: true,
    };
    await writeJson(path.join(outDir, 'run-request.json'), runRequest);
    const runAccepted = await swapServer.runNavswap(runRequest);
    await writeJson(path.join(outDir, 'run-accepted.json'), runAccepted);
    const streamPromise = runAccepted.run_id
      ? collectRunStream(swapServer, runAccepted.run_id, args.timeoutMs)
      : null;
    const runFinal = runAccepted.run_id
      ? await pollRun(swapServer, runAccepted.run_id, args.timeoutMs)
      : { status: runAccepted, events: null, receipts: null };
    await writeJson(path.join(outDir, 'run-final.json'), runFinal);
    const runStream = streamPromise ? await streamPromise : null;
    if (runStream) {
      await writeJson(path.join(outDir, 'run-stream.json'), runStream);
    }
    if (runFinal.status?.ok === false || runFinal.status?.status === 'failed') {
      throw new Error(runFinal.status?.message || 'NAVSwap operator run failed');
    }

    const direction = navswapDirection(args, quote);
    const afterFeed = await feed.waitFor((snapshot) => {
      const balances = snapshotBalances(snapshot);
      if (direction === 'redeem') {
        return balances.a651 <= preSwap.balances.a651 - redeemAmount
          && balances.pfUSDC >= preSwap.balances.pfUSDC + requiredSettlement;
      }
      return balances.a651 >= preSwap.balances.a651 + mintAmount
        && balances.pfUSDC <= preSwap.balances.pfUSDC - requiredSettlement;
    }, 'post-run pfUSDC/a651 balance movement');
    const after = compactFeedSnapshot(afterFeed.snapshot);
    await writeJson(path.join(outDir, 'feed-after.json'), after);

    const summary = {
      ok: true,
      mode: 'execute',
      direction,
      wallet_address: wallet.address,
      out_dir: outDir,
      wallet_action_tx_ids: walletActionResult.submissions.map(submission => submission.txId).filter(Boolean),
      run_id: runFinal.status?.run_id || runAccepted.run_id || null,
      run_status: runFinal.status?.status || null,
      run_status_terminal: runFinal.status?.terminal === true,
      operator_tx_id: runFinal.status?.result?.operator_completion?.tx_id || null,
      funding_tx_id: fundingResult?.tx_id || null,
      funding_amount_atoms: fundingResult?.amount_atoms || null,
      run_stream_terminal: runStream?.terminal === true,
      run_stream_event_count: runStream?.event_count ?? null,
      initial_balances: before.balances,
      pre_swap_balances: preSwap.balances,
      after_balances: after.balances,
      required_settlement_atoms: requiredSettlement,
      mint_amount_atoms: mintAmount,
      redeem_amount_atoms: redeemAmount,
      feed_observed_balance_movement: true,
    };
    await writeJson(path.join(outDir, 'summary.json'), summary);
    process.stdout.write(`${JSON.stringify(summary, jsonReplacer, 2)}\n`);
  } finally {
    if (feed) await feed.close().catch(() => {});
    rpc.close();
  }
}

main().catch((error) => {
  process.stderr.write(`${error.stack || error.message}\n`);
  process.exit(1);
});
