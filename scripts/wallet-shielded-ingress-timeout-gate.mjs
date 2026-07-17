#!/usr/bin/env node
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import net from 'node:net';
import process from 'node:process';
import WebSocket from '../wallet-proxy/node_modules/ws/index.js';

import { RpcClient } from '../wallet-web/src/lib/rpc-client.js';
import { ACCOUNT_INDEX, A651_ASSET_ID, CHAIN_ID } from '../wallet-web/src/lib/utils.js';
import {
  buildAssetOrchardIngressPayload,
  SHIELDED_NAVSWAP_ROUTE,
} from '../wallet-web/src/lib/shielded-navswap.js';
import * as walletWasm from '../wallet-web/src/wasm/postfiat_wallet_wasm.js';
import { configuredFleetEndpoints } from './lib/configured-fleet-endpoints.mjs';

globalThis.WebSocket = WebSocket;

const APP_URL = process.env.ORCHARD_INGRESS_GATE_URL || 'http://127.0.0.1:5173/';
const RPC_URL = process.env.ORCHARD_INGRESS_GATE_RPC || 'ws://127.0.0.1:8080/rpc';
const OUT_DIR = process.env.ORCHARD_INGRESS_GATE_OUT_DIR
  || `docs/evidence/shielded-round-timeout-fix-${new Date().toISOString().replace(/[:.]/g, '')}`;
const AMOUNT_ATOMS = BigInt(process.env.ORCHARD_INGRESS_GATE_AMOUNT_ATOMS || '1');
const SENSITIVE_FILE = process.env.ORCHARD_INGRESS_GATE_WALLET_SENSITIVE_FILE
  || '/tmp/postfiat-orchard-ingress-sensitive-1783010845694/wallet-sensitive.json';
const LOCAL_SERVICE = process.env.ASSET_ORCHARD_LOCAL_SERVICE_URL || 'http://127.0.0.1:8789';
const { hosts: VALIDATOR_HOSTS, ports: VALIDATOR_PORTS } = configuredFleetEndpoints();

mkdirSync(OUT_DIR, { recursive: true });
let walletWasmReady = false;

function assertOk(condition, message) {
  if (!condition) throw new Error(message);
}

function writeJson(file, value) {
  writeFileSync(file, `${JSON.stringify(value, (_key, val) => (
    typeof val === 'bigint' ? val.toString() : val
  ), 2)}\n`);
}

function ensureWalletWasm() {
  if (walletWasmReady) return walletWasm;
  walletWasm.initSync({
    module: readFileSync(join(process.cwd(), 'wallet-web/src/wasm/postfiat_wallet_wasm_bg.wasm')),
  });
  walletWasmReady = true;
  return walletWasm;
}

function walletBackupFromSensitive(sensitive) {
  const wasm = ensureWalletWasm();
  const result = wasm.wallet_keygen(CHAIN_ID, sensitive.seed, ACCOUNT_INDEX);
  assertOk(
    result.address === sensitive.accountAddress,
    `sensitive wallet address mismatch: derived ${result.address}, expected ${sensitive.accountAddress}`,
  );
  return result.backup_json;
}

function assetItems(result) {
  if (Array.isArray(result)) return result;
  if (Array.isArray(result?.assets)) return result.assets;
  return [];
}

function canonicalBalanceAtoms(result, assetId) {
  let total = 0n;
  for (const item of assetItems(result)) {
    const id = item.asset_id || item.id;
    if (id === assetId) total += BigInt(item.balance ?? item.amount ?? 0);
  }
  return total;
}

async function signAssetOperationWithWallet(rpc, source, operation, backupJson) {
  const quoteResp = await rpc.assetFeeQuote(source, JSON.stringify(operation));
  assertOk(quoteResp.ok, `asset_fee_quote failed: ${quoteResp.error?.message || 'unknown'}`);
  const quote = quoteResp.result;
  assertOk(quote.sender_meets_reserve_after_fee !== false, `wallet ${source} lacks asset transaction fee reserve`);
  const signFields = {
    chain_id: quote.chain_id,
    genesis_hash: quote.genesis_hash,
    protocol_version: quote.protocol_version,
    source: quote.source,
    fee: quote.minimum_fee,
    sequence: quote.sequence,
    operation: quote.operation || operation,
  };
  const signed = ensureWalletWasm().wallet_sign_asset_transaction_fields(
    backupJson,
    JSON.stringify(signFields),
  );
  return { quote, signed, signFields };
}

async function proxyPost(path, body) {
  const response = await fetch(new URL(path, APP_URL), {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  const json = await response.json();
  if (!response.ok || json.ok === false) {
    throw new Error(`${path} failed: ${json.message || json.code || response.statusText}`);
  }
  return json;
}

async function localPost(path, body) {
  const response = await fetch(`${LOCAL_SERVICE}${path}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  const json = await response.json();
  if (!response.ok || json.ok === false) {
    throw new Error(`${path} failed: ${json.message || json.error || response.statusText}`);
  }
  return json;
}

function validatorStatus(host, port, idx) {
  return new Promise((resolve, reject) => {
    const sock = net.createConnection({ host, port, timeout: 6000 }, () => {
      sock.write(`${JSON.stringify({
        version: 'postfiat-local-rpc-v1',
        id: `shielded-timeout-gate-status-${idx}`,
        method: 'status',
        params: {},
      })}\n`);
    });
    let raw = '';
    sock.setEncoding('utf8');
    sock.on('data', chunk => {
      raw += chunk;
      if (raw.includes('\n')) sock.end();
    });
    sock.on('timeout', () => {
      sock.destroy();
      reject(new Error(`validator-${idx} status timeout`));
    });
    sock.on('error', reject);
    sock.on('end', () => {
      try {
        const response = JSON.parse(raw.trim());
        if (!response.ok) throw new Error(response.error?.message || `validator-${idx} status failed`);
        resolve({
          idx,
          host,
          port,
          height: response.result.block_height,
          tip: response.result.block_tip_hash,
          root: response.result.state_root,
          mempool: response.result.mempool_pending,
        });
      } catch (error) {
        reject(error);
      }
    });
  });
}

async function waitForFleetConvergenceNoRepair(label, timeoutMs = 240_000) {
  const deadline = Date.now() + timeoutMs;
  let statuses = [];
  while (Date.now() <= deadline) {
    statuses = await Promise.all(VALIDATOR_HOSTS.map((host, idx) => (
      validatorStatus(host, VALIDATOR_PORTS[idx], idx).catch(error => ({
        idx,
        host,
        port: VALIDATOR_PORTS[idx],
        error: error.message || String(error),
      }))
    )));
    const healthy = statuses.filter(status => !status.error);
    const keys = new Set(healthy.map(status => `${status.height}:${status.tip}:${status.root}`));
    if (keys.size === 1 && healthy.length === 6) {
      const report = {
        schema: 'postfiat-shielded-round-timeout-gate-fleet-convergence-v1',
        label,
        captured_at: new Date().toISOString(),
        converged: true,
        repair_invocations: [],
        count: statuses.length,
        height: statuses[0].height,
        tip: statuses[0].tip,
        root: statuses[0].root,
        statuses,
      };
      writeJson(join(OUT_DIR, `${label}-fleet-convergence-no-repair.json`), report);
      return report;
    }
    await new Promise(resolve => setTimeout(resolve, 3000));
  }
  const report = {
    schema: 'postfiat-shielded-round-timeout-gate-fleet-convergence-v1',
    label,
    captured_at: new Date().toISOString(),
    converged: false,
    repair_invocations: [],
    statuses,
  };
  writeJson(join(OUT_DIR, `${label}-fleet-convergence-no-repair.json`), report);
  throw new Error(`fleet did not converge without repair after ${label}`);
}

function roundReport(payload) {
  return payload?.report?.transport || payload?.report || null;
}

function roundTimingSummary(payload) {
  const report = roundReport(payload);
  const voteTargets = report?.timings?.vote_request_targets || [];
  const sendTargets = report?.timings?.certified_send_targets || [];
  const v0Vote = voteTargets.find(row => row.target === 'validator-0') || null;
  const v0Send = sendTargets.find(row => row.target === 'validator-0') || null;
  return {
    schema: 'postfiat-shielded-round-timeout-gate-timing-summary-v1',
    captured_at: new Date().toISOString(),
    round_ok: report?.round_ok ?? null,
    timeout_ms: report?.timeout_ms ?? null,
    allow_peer_failures: report?.allow_peer_failures ?? null,
    quorum_early_full_propagation: report?.quorum_early_full_propagation ?? null,
    local_apply_before_certified_send: report?.local_apply_before_certified_send ?? null,
    unresolved_vote_targets: report?.unresolved_vote_targets || [],
    skipped_certified_send_targets: report?.skipped_certified_send_targets || [],
    failed_vote_request_count: report?.failed_vote_request_count ?? null,
    failed_send_count: report?.failed_send_count ?? null,
    all_vote_requests_verified: report?.all_vote_requests_verified ?? null,
    all_sends_verified: report?.all_sends_verified ?? null,
    vote_request_targets: voteTargets,
    certified_send_targets: sendTargets,
    validator_0_vote_request: v0Vote,
    validator_0_certified_send: v0Send,
    timings: report?.timings || null,
  };
}

async function main() {
  const sensitive = JSON.parse(readFileSync(SENSITIVE_FILE, 'utf8'));
  assertOk(/^[0-9a-f]{64}$/i.test(sensitive.seed), 'sensitive wallet seed is malformed');
  assertOk(/^pf[a-f0-9]{40}$/i.test(sensitive.accountAddress), 'sensitive wallet address is malformed');
  const backupJson = walletBackupFromSensitive(sensitive);
  const rpc = new RpcClient(RPC_URL);
  try {
    const beforeAssets = await rpc.accountAssets(sensitive.accountAddress);
    assertOk(beforeAssets.ok, `account_assets failed: ${beforeAssets.error?.message || 'unknown'}`);
    const beforeBalance = canonicalBalanceAtoms(beforeAssets.result, A651_ASSET_ID);
    assertOk(beforeBalance >= AMOUNT_ATOMS, `wallet has ${beforeBalance} a651 atoms; need ${AMOUNT_ATOMS}`);
    writeJson(join(OUT_DIR, 'before-public-balance.json'), {
      schema: 'postfiat-shielded-round-timeout-gate-public-balance-v1',
      captured_at: new Date().toISOString(),
      wallet_address: sensitive.accountAddress,
      asset_id: A651_ASSET_ID,
      balance_atoms: beforeBalance.toString(),
      amount_atoms: AMOUNT_ATOMS.toString(),
    });

    const preflight = await proxyPost('/api/shielded-nav-swap/preflight', {
      route: SHIELDED_NAVSWAP_ROUTE,
      wallet_address: sensitive.accountAddress,
      asset_id: A651_ASSET_ID,
      amount_atoms: AMOUNT_ATOMS.toString(),
    });
    writeJson(join(OUT_DIR, 'preflight.json'), preflight);

    const noteResult = await localPost('/asset-orchard/ingress-notes', {
      route: SHIELDED_NAVSWAP_ROUTE,
      wallet_address: sensitive.accountAddress,
      asset_id: A651_ASSET_ID,
      amount_atoms: AMOUNT_ATOMS.toString(),
      preflight,
    });
    writeJson(join(OUT_DIR, 'local-note-public.json'), {
      schema: 'postfiat-shielded-round-timeout-gate-local-note-public-v1',
      captured_at: new Date().toISOString(),
      wallet_address: sensitive.accountAddress,
      asset_id: A651_ASSET_ID,
      amount_atoms: AMOUNT_ATOMS.toString(),
      output_commitment: noteResult.wallet_note?.output_commitment || null,
    });

    const signedBurn = await signAssetOperationWithWallet(
      rpc,
      sensitive.accountAddress,
      preflight.operation,
      backupJson,
    );
    const ingressPayload = buildAssetOrchardIngressPayload({
      signedBurnTransaction: signedBurn.signed,
      assetId: A651_ASSET_ID,
      amountAtoms: AMOUNT_ATOMS.toString(),
      walletNote: noteResult.wallet_note,
      encryptedOutput: noteResult.encrypted_output,
    });
    const ingress = await proxyPost('/api/shielded-nav-swap/ingress', {
      route: SHIELDED_NAVSWAP_ROUTE,
      wallet_address: sensitive.accountAddress,
      ingress_payload: ingressPayload,
    });
    writeJson(join(OUT_DIR, 'ingress-response.json'), ingress);
    const timings = roundTimingSummary(ingress);
    writeJson(join(OUT_DIR, 'round-timings.json'), timings);
    assertOk(timings.round_ok === true, 'shielded ingress round did not return round_ok=true');
    assertOk(timings.failed_vote_request_count === 0, 'shielded ingress had vote request failures');
    assertOk(timings.failed_send_count === 0, 'shielded ingress had certified send failures');
    assertOk(timings.all_vote_requests_verified === true, 'shielded ingress did not verify all vote requests');
    assertOk(timings.all_sends_verified === true, 'shielded ingress did not verify all certified sends');
    assertOk(timings.validator_0_certified_send?.result === 'ok', 'validator-0 certified send was not successful');

    const convergence = await waitForFleetConvergenceNoRepair('after-shielded-ingress');
    const report = {
      schema: 'postfiat-shielded-round-timeout-gate-report-v1',
      captured_at: new Date().toISOString(),
      app_url: APP_URL,
      rpc_url: RPC_URL,
      wallet_address: sensitive.accountAddress,
      asset_id: A651_ASSET_ID,
      amount_atoms: AMOUNT_ATOMS.toString(),
      output_commitment: noteResult.wallet_note?.output_commitment || null,
      artifact_dir: ingress.artifact_dir || null,
      repair_invocations: [],
      timing_summary_file: 'round-timings.json',
      convergence_file: 'after-shielded-ingress-fleet-convergence-no-repair.json',
      convergence: {
        height: convergence.height,
        root: convergence.root,
        count: convergence.count,
      },
      validator_0_certified_send_ms: timings.validator_0_certified_send?.duration_ms ?? null,
      total_round_ms: timings.timings?.total_ms ?? null,
    };
    writeJson(join(OUT_DIR, 'report.json'), report);
    console.log(JSON.stringify({ ok: true, out_dir: OUT_DIR, report }, null, 2));
  } finally {
    rpc.close();
  }
}

main().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
