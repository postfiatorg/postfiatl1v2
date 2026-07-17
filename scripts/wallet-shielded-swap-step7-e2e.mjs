#!/usr/bin/env node
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import net from 'node:net';
import { join } from 'node:path';
import process from 'node:process';
import WebSocket from '../wallet-proxy/node_modules/ws/index.js';

import { RpcClient } from '../wallet-web/src/lib/rpc-client.js';
import { ACCOUNT_INDEX, A651_ASSET_ID, CHAIN_ID } from '../wallet-web/src/lib/utils.js';
import {
  buildAssetOrchardIngressPayload,
  SHIELDED_NAVSWAP_ROUTE,
} from '../wallet-web/src/lib/shielded-navswap.js';
import * as walletWasm from '../wallet-web/src/wasm/postfiat_wallet_wasm.js';
import {
  buildNoPrivateMaterialRequestLog,
  buildStep10EvidenceSummary,
} from './lib/wallet-shielded-step10-evidence.mjs';
import { configuredFleetEndpoints } from './lib/configured-fleet-endpoints.mjs';

globalThis.WebSocket = WebSocket;

const require = createRequire(new URL('../wallet-web/package.json', import.meta.url));
const { chromium } = require('playwright');

const APP_URL = process.env.ORCHARD_SWAP_E2E_URL || 'http://127.0.0.1:5173/';
const RPC_URL = process.env.ORCHARD_SWAP_E2E_RPC || 'ws://127.0.0.1:8080/rpc';
const STEP_MODE = process.env.ORCHARD_SWAP_E2E_STEP || 'step7';
const STEP8_REVERSE = STEP_MODE === 'step8-reverse';
const STEP9_EGRESS = STEP_MODE === 'step9-egress';
const STEP10_PAIR = STEP_MODE === 'step10-pair';
const STEP10_PREP = STEP_MODE === 'step10-prep';
const STEP10_RESCAN = STEP_MODE === 'step10-rescan';
const STEP10_LIVE_WINDOW = ['1', 'true', 'yes'].includes(String(process.env.ORCHARD_SWAP_E2E_LIVE_WINDOW || '').toLowerCase());
const EVIDENCE_STEP = STEP10_PAIR || STEP10_PREP || STEP10_RESCAN ? 'step10' : STEP9_EGRESS ? 'step9-egress' : STEP8_REVERSE ? 'step8-reverse' : 'step7';
const RUNS = Number.parseInt(process.env.ORCHARD_SWAP_E2E_RUNS || '2', 10);
const SWAP_AMOUNT = process.env.ORCHARD_SWAP_E2E_AMOUNT || '0.002';
const SWAP_AMOUNT_ATOMS = parseAmountAtoms(SWAP_AMOUNT);
const QUOTE_TTL_MS = process.env.ORCHARD_SWAP_E2E_QUOTE_TTL_MS || '3600000';
const PROXY_TRANSPORT_TIMEOUT_MS = process.env.ORCHARD_SWAP_E2E_PROXY_TIMEOUT_MS || '2400000';
const SUBMIT_WAIT_MS = Number.parseInt(process.env.ORCHARD_SWAP_E2E_SUBMIT_WAIT_MS || '2700000', 10);
const ZERO_REPAIR = STEP8_REVERSE || ['1', 'true', 'yes'].includes(String(process.env.ORCHARD_SWAP_E2E_ZERO_REPAIR || '').toLowerCase());
const ALLOW_SINGLE_RUN = ['1', 'true', 'yes'].includes(String(process.env.ORCHARD_SWAP_E2E_ALLOW_SINGLE_RUN || '').toLowerCase());
const USE_EXISTING_WALLET_NOTES = STEP8_REVERSE || ['1', 'true', 'yes'].includes(String(process.env.ORCHARD_SWAP_E2E_USE_EXISTING_WALLET_NOTES || '').toLowerCase());
const A652_ASSET_ID = (process.env.A652_ASSET_ID || 'b15cf53c383c81de56b71d2b55c897d249426d5431232f07fa77778f92b1cea852829520f1098553d125252cb3a85505').toLowerCase();
const ASSET_IDS = {
  a651: A651_ASSET_ID,
  a652: A652_ASSET_ID,
};
const DEFAULT_SWAP_PLAN = STEP10_PAIR || STEP10_PREP
  ? [
      { from: 'a651', to: 'a652' },
      { from: 'a652', to: 'a651' },
    ]
  : STEP8_REVERSE
  ? [
      { from: 'a652', to: 'a651' },
      { from: 'a652', to: 'a651' },
    ]
  : [
      { from: 'a651', to: 'a652' },
      { from: 'a651', to: 'a652' },
    ];
const SWAP_PLAN = parseSwapPlan(process.env.ORCHARD_SWAP_E2E_PLAN, DEFAULT_SWAP_PLAN);
const STEP8_DEFAULT_WALLET_INPUT_COMMITMENTS = [
  '5219d61ff58b10344bc56a3df0d60bc89971708d05a7d2a67c7d1bd85e23342b',
  'ffc2497432706f97a4705eb394141f9f59cfcd633cf22f129cab07cb7a0ae438',
];
const EXISTING_WALLET_INPUT_COMMITMENTS = (process.env.ORCHARD_SWAP_E2E_WALLET_INPUT_COMMITMENTS
  ? process.env.ORCHARD_SWAP_E2E_WALLET_INPUT_COMMITMENTS.split(',')
  : STEP8_REVERSE ? STEP8_DEFAULT_WALLET_INPUT_COMMITMENTS : [])
  .map(value => value.trim().toLowerCase())
  .filter(Boolean);
const EXISTING_POOL_NOTE_COMMITMENTS = (process.env.ORCHARD_SWAP_E2E_POOL_NOTE_COMMITMENTS
  ? process.env.ORCHARD_SWAP_E2E_POOL_NOTE_COMMITMENTS.split(',')
  : [])
  .map(value => value.trim().toLowerCase())
  .filter(Boolean);
const OUT_DIR = process.env.ORCHARD_SWAP_E2E_OUT_DIR
  || `docs/evidence/wallet-private-swap-${EVIDENCE_STEP === 'step10' ? (STEP10_PREP ? 'step10-prep' : 'step10-live') : EVIDENCE_STEP === 'step9-egress' ? 'step9-egress' : EVIDENCE_STEP === 'step8-reverse' ? 'step8-reverse' : 'step7-live'}-${new Date().toISOString().replace(/[:.]/g, '')}`;
const SENSITIVE_FILE = process.env.ORCHARD_SWAP_E2E_WALLET_SENSITIVE_FILE
  || '/tmp/postfiat-orchard-ingress-sensitive-1783010845694/wallet-sensitive.json';
const LOCAL_VAULT_DIR = process.env.ASSET_ORCHARD_LOCAL_VAULT_DIR
  || join(process.env.XDG_DATA_HOME || join(homedir(), '.local/share'), 'postfiat/asset-orchard-local-vault');
const LOCAL_SERVICE = process.env.ASSET_ORCHARD_LOCAL_SERVICE_URL || 'http://127.0.0.1:8789';
const HOST_SIGNER_MODE = ['1', 'true', 'yes', 'host'].includes(String(process.env.ORCHARD_SWAP_E2E_HOST_SIGNER || '').toLowerCase());
const HOST_PROXY_RESTART_MODE = ['1', 'true', 'yes', 'host'].includes(String(process.env.ORCHARD_SWAP_E2E_HOST_PROXY_RESTART || '').toLowerCase());
const HOST_PROXY_ENV_NUL = process.env.ORCHARD_SWAP_E2E_HOST_PROXY_ENV_NUL || '';
const HOST_PROXY_PID_FILE = process.env.ORCHARD_SWAP_E2E_HOST_PROXY_PID_FILE || '';
const HOST_PROXY_LOG = process.env.ORCHARD_SWAP_E2E_HOST_PROXY_LOG || '';
const HOST_PROXY_WORKDIR = process.env.ORCHARD_SWAP_E2E_HOST_PROXY_WORKDIR
  || join(process.cwd(), 'wallet-proxy');
const HOST_NODE_BIN = process.env.ORCHARD_SWAP_E2E_HOST_NODE_BIN
  || join(process.cwd(), 'target/release/postfiat-node');
const HOST_ISSUER_KEY_FILE = String(process.env.ORCHARD_SWAP_E2E_HOST_ISSUER_KEY_FILE || '').trim();
const HOST_POOL_OPERATOR_KEY_FILE = String(process.env.ORCHARD_SWAP_E2E_HOST_POOL_OPERATOR_KEY_FILE || '').trim();
const SYNC_RELAY_STATE_CMD = process.env.ORCHARD_SWAP_E2E_SYNC_RELAY_STATE_CMD
  || 'scripts/wallet-shielded-ingress-sync-state';
const VALIDATOR_REPAIR_CMD = process.env.ORCHARD_SWAP_E2E_VALIDATOR_REPAIR_CMD
  || 'scripts/wan-devnet-state-sync';
const PFT_FUNDER = process.env.ORCHARD_SWAP_E2E_FUNDER
  || 'pff3e396f771a8f490ca330e1720472d473bcfcb6d';
const PFT_FUNDER_KEY_FILE = process.env.ORCHARD_SWAP_E2E_FUNDER_KEY_FILE
  || '/run/secrets/navswap-issuer.key.json';
const POOL_OPERATOR = process.env.ORCHARD_SWAP_E2E_POOL_OPERATOR
  || 'pf65c9783ceafc0f519a74195e78cc7909f92429c3';
const POOL_OPERATOR_KEY_FILE = process.env.ORCHARD_SWAP_E2E_POOL_OPERATOR_KEY_FILE
  || '/run/secrets/vault-bridge-holder.key.json';
const WALLET_PFT_FUND_ATOMS = BigInt(process.env.ORCHARD_SWAP_E2E_WALLET_PFT_ATOMS || '100000');
const POOL_OPERATOR_PFT_FUND_ATOMS = BigInt(process.env.ORCHARD_SWAP_E2E_POOL_OPERATOR_PFT_ATOMS || '100000');
const { hosts: VALIDATOR_HOSTS, ports: VALIDATOR_PORTS } = configuredFleetEndpoints();
const STEP9_EGRESS_COMMITMENT = (process.env.ORCHARD_SWAP_E2E_EGRESS_COMMITMENT
  || '3f587679ad96cfb77dc4555f12ff47f5c059f222e349f1f12849532e55ce6f28').toLowerCase();
const STEP9_HOLD_COMMITMENT_RAW = process.env.ORCHARD_SWAP_E2E_HOLD_COMMITMENT
  || '98e255264576f57379e43188e5ebc1a0274d29e835cfd1d96d52e490bd527519';
const STEP9_HOLD_COMMITMENT = ['none', 'skip', ''].includes(STEP9_HOLD_COMMITMENT_RAW.toLowerCase())
  ? null
  : STEP9_HOLD_COMMITMENT_RAW.toLowerCase();
const STEP9_EXPECTED_BASELINE_A651 = BigInt(process.env.ORCHARD_SWAP_E2E_EXPECTED_BASELINE_A651 || '15999');
const SERVICE_WARMTH_LABELS = (process.env.ORCHARD_SWAP_E2E_SERVICE_WARMTH_LABELS || '')
  .split(',')
  .map(value => value.trim())
  .filter(Boolean);

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

function readJson(file, fallback = undefined) {
  try {
    return JSON.parse(readFileSync(file, 'utf8'));
  } catch (error) {
    if (fallback !== undefined && error && error.code === 'ENOENT') return fallback;
    throw error;
  }
}

function evidenceSchema(suffix) {
  return `postfiat-wallet-private-swap-${EVIDENCE_STEP}-${suffix}`;
}

function utcStampForPath() {
  return new Date().toISOString().replace(/[-:.]/g, '').replace(/Z$/, 'Z');
}

function compactHash(value, edge = 8) {
  const text = String(value || '').trim();
  if (text.length <= edge * 2 + 1) return text;
  return `${text.slice(0, edge)}…${text.slice(-edge)}`;
}

function parseAmountAtoms(value, precision = 6) {
  const text = String(value || '').trim();
  assertOk(/^[0-9]+(?:\.[0-9]+)?$/.test(text), `invalid decimal amount ${value}`);
  const [whole, frac = ''] = text.split('.');
  assertOk(frac.length <= precision, `amount ${value} exceeds ${precision} decimals`);
  return BigInt(whole) * (10n ** BigInt(precision)) + BigInt(frac.padEnd(precision, '0') || '0');
}

function parseSwapPlan(value, fallback) {
  if (!value) return fallback;
  return String(value)
    .split(',')
    .map(part => {
      const [from, to] = part.split('->').map(item => item.trim());
      assertOk(from && to, `invalid swap plan leg ${part}`);
      return { from, to };
    });
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

async function waitForAssetBalance(rpc, account, assetId, predicate, label, timeoutMs = 180_000) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() <= deadline) {
    const resp = await rpc.accountAssets(account);
    last = resp;
    const balance = canonicalBalanceAtoms(resp.result, assetId);
    if (resp.ok && predicate(balance)) return { resp, balance };
    await new Promise(resolve => setTimeout(resolve, 1500));
  }
  throw new Error(`asset ${assetId} balance wait failed for ${account}: ${label}; last=${JSON.stringify(last)}`);
}

function signWithProxy(command, quote, keyFile = PFT_FUNDER_KEY_FILE) {
  if (HOST_SIGNER_MODE) {
    return signWithHostNode(command, quote, keyFile);
  }
  const signer = [
    'tmp=$(mktemp)',
    'trap "rm -f $tmp" EXIT',
    'cat > "$tmp"',
    `/usr/local/bin/postfiat-node ${command} --key-file "${keyFile}" --quote-file "$tmp"`,
  ].join('; ');
  const proc = spawnSync('docker', [
    'compose',
    '-f',
    'docker-compose.wallet.yml',
    'exec',
    '-T',
    'wallet-proxy',
    'sh',
    '-lc',
    signer,
  ], {
    cwd: process.cwd(),
    input: JSON.stringify(quote),
    encoding: 'utf8',
    maxBuffer: 4 * 1024 * 1024,
  });
  if (proc.error) throw proc.error;
  assertOk(proc.status === 0, `wallet-proxy signer failed (${proc.status}): ${proc.stderr || proc.stdout}`);
  return JSON.parse(proc.stdout);
}

function mapHostSignerKeyFile(keyFile) {
  if (keyFile === '/run/secrets/navswap-issuer.key.json') return HOST_ISSUER_KEY_FILE;
  if (keyFile === '/run/secrets/vault-bridge-holder.key.json') return HOST_POOL_OPERATOR_KEY_FILE;
  return keyFile;
}

function signWithHostNode(command, quote, keyFile = PFT_FUNDER_KEY_FILE) {
  const dir = mkdtempSync(join(tmpdir(), 'postfiat-wallet-signer-'));
  const quoteFile = join(dir, 'quote.json');
  try {
    writeJson(quoteFile, quote);
    const proc = spawnSync(HOST_NODE_BIN, [
      command,
      '--key-file',
      mapHostSignerKeyFile(keyFile),
      '--quote-file',
      quoteFile,
    ], {
      cwd: process.cwd(),
      encoding: 'utf8',
      maxBuffer: 4 * 1024 * 1024,
    });
    if (proc.error) throw proc.error;
    assertOk(proc.status === 0, `host signer failed (${proc.status}): ${proc.stderr || proc.stdout}`);
    return JSON.parse(proc.stdout);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

async function signAssetOperationWithProxy(rpc, source, operation, keyFile = PFT_FUNDER_KEY_FILE) {
  const quoteResp = await rpc.assetFeeQuote(source, JSON.stringify(operation));
  assertOk(quoteResp.ok, `asset_fee_quote failed: ${quoteResp.error?.message || 'unknown'}`);
  return {
    quote: quoteResp.result,
    signed: signWithProxy('wallet-sign-asset-transaction', quoteResp.result, keyFile),
  };
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

async function signAssetOperationWithWallet(rpc, source, operation, backupJson) {
  const quoteResp = await rpc.assetFeeQuote(source, JSON.stringify(operation));
  assertOk(quoteResp.ok, `asset_fee_quote failed: ${quoteResp.error?.message || 'unknown'}`);
  const quote = quoteResp.result;
  assertOk(quote.sender_meets_reserve_after_fee !== false, `wallet ${source} lacks asset transaction fee reserve`);
  assertOk(!quote.source || quote.source === source, `asset fee quote source ${quote.source} did not match ${source}`);
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

async function ensurePftBalance(rpc, recipient, minimumAtoms) {
  const before = await rpc.account(recipient);
  const beforeBalance = BigInt(before.result?.balance || 0);
  if (before.ok && beforeBalance >= minimumAtoms) {
    return {
      skipped: true,
      recipient,
      balance_atoms: beforeBalance.toString(),
      minimum_atoms: minimumAtoms.toString(),
    };
  }
  const quoteResp = await rpc.transferFeeQuote(PFT_FUNDER, recipient, Number(minimumAtoms));
  assertOk(quoteResp.ok, `PFT funding transfer_fee_quote failed: ${quoteResp.error?.message || 'unknown'}`);
  const signed = signWithProxy('wallet-sign-transfer', quoteResp.result, PFT_FUNDER_KEY_FILE);
  const submitResp = await rpc.submitSignedTransferFinality(JSON.stringify(signed));
  assertOk(submitResp.ok, `PFT funding submit failed: ${submitResp.error?.message || 'unknown'}`);
  const deadline = Date.now() + 120_000;
  let last = null;
  while (Date.now() <= deadline) {
    const resp = await rpc.account(recipient);
    last = resp;
    const balance = BigInt(resp.result?.balance || 0);
    if (resp.ok && balance >= minimumAtoms) {
      return {
        skipped: false,
        recipient,
        tx_id: submitResp.result?.tx_id || null,
        finality_height: submitResp.result?.finality?.block?.header?.height || null,
        balance_atoms: balance.toString(),
        minimum_atoms: minimumAtoms.toString(),
      };
    }
    await new Promise(resolve => setTimeout(resolve, 1500));
  }
  throw new Error(`PFT balance for ${recipient} did not reach ${minimumAtoms}; last=${JSON.stringify(last)}`);
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

async function fundAsset(rpc, recipient, assetId, amountAtoms) {
  const before = await rpc.accountAssets(recipient);
  const beforeBalance = canonicalBalanceAtoms(before.result, assetId);
  const operation = {
    operation: 'issued_payment',
    from: PFT_FUNDER,
    to: recipient,
    issuer: PFT_FUNDER,
    asset_id: assetId,
    amount: Number(amountAtoms),
  };
  const signed = await signAssetOperationWithProxy(rpc, PFT_FUNDER, operation);
  const submitResp = await rpc.submitSignedAssetTransactionFinality(JSON.stringify(signed.signed));
  assertOk(submitResp.ok, `asset funding submit failed: ${submitResp.error?.message || 'unknown'}`);
  const after = await waitForAssetBalance(
    rpc,
    recipient,
    assetId,
    balance => balance >= beforeBalance + amountAtoms,
    'issued asset funding visible',
  );
  return {
    asset_id: assetId,
    amount_atoms: amountAtoms.toString(),
    tx_id: submitResp.result?.tx_id || null,
    finality_height: submitResp.result?.finality?.block?.header?.height || null,
    before_balance_atoms: beforeBalance.toString(),
    balance_atoms: after.balance.toString(),
  };
}

function syncRelayState(label) {
  const reportFile = join(OUT_DIR, `${label}-relay-state-sync.json`);
  const proc = spawnSync(SYNC_RELAY_STATE_CMD, [], {
    cwd: process.cwd(),
    env: {
      ...process.env,
      REPORT_FILE: reportFile,
    },
    encoding: 'utf8',
    maxBuffer: 8 * 1024 * 1024,
  });
  if (proc.error) throw proc.error;
  assertOk(proc.status === 0, `relay state sync failed (${proc.status}): ${proc.stderr || proc.stdout}`);
  return JSON.parse(readFileSync(reportFile, 'utf8'));
}

function validatorStatus(host, port, idx) {
  return new Promise((resolve, reject) => {
    const sock = net.createConnection({ host, port, timeout: 6000 }, () => {
      sock.write(`${JSON.stringify({
        version: 'postfiat-local-rpc-v1',
        id: `wallet-shielded-swap-step7-status-${idx}`,
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

function validatorRpcRequest(host, port, idx, method, params = {}) {
  return new Promise((resolve, reject) => {
    const sock = net.createConnection({ host, port, timeout: 6000 }, () => {
      sock.write(`${JSON.stringify({
        version: 'postfiat-local-rpc-v1',
        id: `wallet-shielded-swap-${EVIDENCE_STEP}-${method}-${idx}`,
        method,
        params,
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
      reject(new Error(`validator-${idx} ${method} timeout`));
    });
    sock.on('error', reject);
    sock.on('end', () => {
      try {
        const response = JSON.parse(raw.trim());
        if (!response.ok) throw new Error(response.error?.message || `validator-${idx} ${method} failed`);
        resolve({ idx, host, port, response, result: response.result });
      } catch (error) {
        reject(error);
      }
    });
  });
}

async function validatorAccountAssetsSnapshot(label, account, assetId) {
  const rows = await Promise.all(VALIDATOR_HOSTS.map((host, idx) => (
    validatorRpcRequest(host, VALIDATOR_PORTS[idx], idx, 'account_assets', { account })
      .then(({ idx: validator, host: rowHost, port, result }) => ({
        validator,
        host: rowHost,
        port,
        ok: true,
        balance_atoms: canonicalBalanceAtoms(result, assetId).toString(),
        result,
      }))
      .catch(error => ({
        validator: idx,
        host,
        port: VALIDATOR_PORTS[idx],
        ok: false,
        error: error.message || String(error),
      }))
  )));
  const report = {
    schema: evidenceSchema('validator-account-assets-v1'),
    label,
    captured_at: new Date().toISOString(),
    account,
    asset_id: assetId,
    rows,
  };
  writeJson(join(OUT_DIR, `${label}-validator-account-assets.json`), report);
  return report;
}

async function waitForFleetConvergence(label, timeoutMs = 180_000) {
  const deadline = Date.now() + timeoutMs;
  let statuses = [];
  let repair = null;
  const repairInvocations = [];
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
        schema: evidenceSchema('fleet-convergence-v1'),
        label,
        captured_at: new Date().toISOString(),
        converged: true,
        count: statuses.length,
        height: statuses[0].height,
        tip: statuses[0].tip,
        root: statuses[0].root,
        statuses,
        validator_repair: repair,
        repair_invocations: repairInvocations,
        zero_repair_required: ZERO_REPAIR,
      };
      writeJson(join(OUT_DIR, `${label}-fleet-convergence.json`), report);
      return report;
    }
    if (!repair && !ZERO_REPAIR) {
      const candidate = validatorRepairCandidate(statuses);
      if (candidate) {
        repair = repairValidatorFromMajority(label, candidate);
        repairInvocations.push(repair);
        await new Promise(resolve => setTimeout(resolve, 3000));
        continue;
      }
    }
    await new Promise(resolve => setTimeout(resolve, 3000));
  }
  const report = {
    schema: evidenceSchema('fleet-convergence-v1'),
    label,
    captured_at: new Date().toISOString(),
    converged: false,
    statuses,
    validator_repair: repair,
    repair_invocations: repairInvocations,
    zero_repair_required: ZERO_REPAIR,
  };
  writeJson(join(OUT_DIR, `${label}-fleet-convergence.json`), report);
  throw new Error(`fleet did not converge after ${label}`);
}

function validatorRepairCandidate(statuses) {
  if (!Array.isArray(statuses) || statuses.length !== 6) return null;
  const groups = new Map();
  for (const status of statuses) {
    if (status.error) continue;
    const key = `${status.height}:${status.tip}:${status.root}`;
    const rows = groups.get(key) || [];
    rows.push(status);
    groups.set(key, rows);
  }
  const ordered = [...groups.entries()].sort((a, b) => b[1].length - a[1].length);
  if (ordered.length < 2 || ordered[0][1].length < 5) return null;
  const [majorityKey, majorityRows] = ordered[0];
  const outliers = statuses.filter(status => status.error || `${status.height}:${status.tip}:${status.root}` !== majorityKey);
  if (outliers.length !== 1) return null;
  return {
    source_validator: majorityRows[0].idx,
    target_validator: outliers[0].idx,
    majority: {
      key: majorityKey,
      count: majorityRows.length,
      height: majorityRows[0].height,
      tip: majorityRows[0].tip,
      root: majorityRows[0].root,
    },
    outlier: outliers[0],
  };
}

function repairValidatorFromMajority(label, candidate) {
  const reportFile = join(OUT_DIR, `${label}-validator-${candidate.target_validator}-repair.json`);
  const proc = spawnSync(VALIDATOR_REPAIR_CMD, [], {
    cwd: process.cwd(),
    env: {
      ...process.env,
      SOURCE_VALIDATOR: String(candidate.source_validator),
      TARGET_VALIDATOR: String(candidate.target_validator),
    },
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  if (proc.error) throw proc.error;
  const report = {
    schema: evidenceSchema('validator-repair-v1'),
    label,
    captured_at: new Date().toISOString(),
    command: VALIDATOR_REPAIR_CMD,
    source_validator: candidate.source_validator,
    target_validator: candidate.target_validator,
    majority: candidate.majority,
    outlier: candidate.outlier,
    status: proc.status,
    ok: proc.status === 0,
    stdout: proc.stdout,
    stderr: proc.stderr,
  };
  writeJson(reportFile, report);
  if (!report.ok && /not in majority group/i.test(`${proc.stderr}\n${proc.stdout}`)) {
    return {
      report_file: reportFile,
      source_validator: candidate.source_validator,
      target_validator: candidate.target_validator,
      majority: candidate.majority,
      stale_candidate: true,
    };
  }
  assertOk(report.ok, `validator repair failed (${proc.status}): ${proc.stderr || proc.stdout}`);
  return {
    report_file: reportFile,
    source_validator: candidate.source_validator,
    target_validator: candidate.target_validator,
    majority: candidate.majority,
  };
}

function restartProxyForLiquidity(commitment) {
  if (HOST_PROXY_RESTART_MODE) {
    restartHostProxyForLiquidity(commitment);
    return;
  }
  const proc = spawnSync('docker', [
    'compose',
    '-f',
    'docker-compose.wallet.yml',
    'up',
    '-d',
    '--force-recreate',
    '--no-deps',
    'wallet-proxy',
  ], {
    cwd: process.cwd(),
    env: {
      ...process.env,
      A652_ASSET_ID,
      NAVSWAP_ENABLE_SHIELDED_QUOTES: 'true',
      NAVSWAP_ENABLE_SHIELDED_SWAPS: 'true',
      NAVSWAP_SHIELDED_QUOTE_TTL_MS: QUOTE_TTL_MS,
      NAVSWAP_SHIELDED_INGRESS_TIMEOUT_MS: PROXY_TRANSPORT_TIMEOUT_MS,
      NAVSWAP_SHIELDED_LIQUIDITY_MODE: 'pool_managed_note',
      NAVSWAP_SHIELDED_LIQUIDITY_PROVIDER: 'controlled_pool_operator',
      NAVSWAP_SHIELDED_LIQUIDITY_COMMITMENT: commitment,
      NAVSWAP_SHIELDED_ASSET_ISSUER: PFT_FUNDER,
    },
    encoding: 'utf8',
    maxBuffer: 8 * 1024 * 1024,
  });
  if (proc.error) throw proc.error;
  assertOk(proc.status === 0, `wallet-proxy restart failed (${proc.status}): ${proc.stderr || proc.stdout}`);
  const health = spawnSync('docker', [
    'compose',
    '-f',
    'docker-compose.wallet.yml',
    'exec',
    '-T',
    'wallet-proxy',
    'sh',
    '-lc',
    'for i in $(seq 1 60); do curl -sf http://localhost:8080/api/shielded-nav-swap/status >/dev/null && exit 0; sleep 1; done; exit 1',
  ], {
    cwd: process.cwd(),
    encoding: 'utf8',
    maxBuffer: 1024 * 1024,
  });
  assertOk(health.status === 0, `wallet-proxy did not become healthy: ${health.stderr || health.stdout}`);
}

function envFromNulFile(file) {
  const env = {};
  if (!file) return env;
  for (const entry of readFileSync(file).toString('utf8').split('\0')) {
    if (!entry) continue;
    const idx = entry.indexOf('=');
    if (idx <= 0) continue;
    env[entry.slice(0, idx)] = entry.slice(idx + 1);
  }
  return env;
}

function restartHostProxyForLiquidity(commitment) {
  assertOk(HOST_PROXY_ENV_NUL && existsSync(HOST_PROXY_ENV_NUL), 'host proxy restart requires ORCHARD_SWAP_E2E_HOST_PROXY_ENV_NUL');
  assertOk(HOST_PROXY_PID_FILE, 'host proxy restart requires ORCHARD_SWAP_E2E_HOST_PROXY_PID_FILE');
  assertOk(HOST_PROXY_LOG, 'host proxy restart requires ORCHARD_SWAP_E2E_HOST_PROXY_LOG');
  const env = {
    ...envFromNulFile(HOST_PROXY_ENV_NUL),
    ...process.env,
    A652_ASSET_ID,
    NAVSWAP_ENABLE_SHIELDED_QUOTES: 'true',
    NAVSWAP_ENABLE_SHIELDED_SWAPS: 'true',
    NAVSWAP_ENABLE_SHIELDED_EGRESS: 'true',
    NAVSWAP_SHIELDED_QUOTE_TTL_MS: QUOTE_TTL_MS,
    NAVSWAP_SHIELDED_INGRESS_TIMEOUT_MS: PROXY_TRANSPORT_TIMEOUT_MS,
    NAVSWAP_SHIELDED_LIQUIDITY_MODE: 'pool_managed_note',
    NAVSWAP_SHIELDED_LIQUIDITY_PROVIDER: 'controlled_pool_operator',
    NAVSWAP_SHIELDED_LIQUIDITY_COMMITMENT: commitment,
    NAVSWAP_SHIELDED_ASSET_ISSUER: PFT_FUNDER,
    ASSET_ORCHARD_LOCAL_SERVICE_URL: LOCAL_SERVICE,
  };
  const script = [
    'set -euo pipefail',
    'if [ -s "$ORCHARD_HOST_PROXY_PID_FILE" ]; then',
    '  old=$(cat "$ORCHARD_HOST_PROXY_PID_FILE")',
    '  kill "$old" 2>/dev/null || true',
    '  for i in $(seq 1 50); do kill -0 "$old" 2>/dev/null || break; sleep 0.1; done',
    '  if kill -0 "$old" 2>/dev/null; then',
    '    kill -KILL "$old" 2>/dev/null || true',
    '    for i in $(seq 1 50); do kill -0 "$old" 2>/dev/null || break; sleep 0.1; done',
    '  fi',
    'fi',
    'port="${LISTEN_PORT:-8080}"',
    'listeners=$(ss -ltnp "sport = :$port" 2>/dev/null | sed -n "s/.*pid=\\([0-9][0-9]*\\).*/\\1/p" | sort -u || true)',
    'for listener in $listeners; do kill "$listener" 2>/dev/null || true; done',
    'for i in $(seq 1 50); do ss -ltnp "sport = :$port" 2>/dev/null | grep -q LISTEN || break; sleep 0.1; done',
    'listeners=$(ss -ltnp "sport = :$port" 2>/dev/null | sed -n "s/.*pid=\\([0-9][0-9]*\\).*/\\1/p" | sort -u || true)',
    'for listener in $listeners; do kill -KILL "$listener" 2>/dev/null || true; done',
    'for i in $(seq 1 50); do ss -ltnp "sport = :$port" 2>/dev/null | grep -q LISTEN || break; sleep 0.1; done',
    'cd "$ORCHARD_HOST_PROXY_WORKDIR"',
    'setsid node server.js >> "$ORCHARD_HOST_PROXY_LOG" 2>&1 &',
    'echo $! > "$ORCHARD_HOST_PROXY_PID_FILE"',
    'healthy=0',
    'for i in $(seq 1 100); do curl -sf "http://127.0.0.1:$port/api/shielded-nav-swap/status" >/dev/null && healthy=1 && break; sleep 0.2; done',
    'if [ "$healthy" = "1" ]; then',
    '  listener=$(ss -ltnp "sport = :$port" 2>/dev/null | sed -n "s/.*pid=\\([0-9][0-9]*\\).*/\\1/p" | tail -n 1 || true)',
    '  if [ -n "$listener" ]; then echo "$listener" > "$ORCHARD_HOST_PROXY_PID_FILE"; fi',
    '  if [ -n "${NAVSWAP_SHIELDED_CERTIFIER_READY_FILE:-}" ]; then',
    '    loop_ready=0',
    '    for i in $(seq 1 1200); do [ -s "$NAVSWAP_SHIELDED_CERTIFIER_READY_FILE" ] && loop_ready=1 && break; sleep 0.5; done',
    '    [ "$loop_ready" = "1" ] || exit 1',
    '  fi',
    '  for ready_var in POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE POSTFIAT_TRANSPORT_BLOCK_VOTE_READY_FILE; do',
    '    ready_file="${!ready_var:-}"',
    '    if [ -n "$ready_file" ]; then',
    '      transport_ready=0',
    '      for i in $(seq 1 1200); do [ -s "$ready_file" ] && transport_ready=1 && break; sleep 0.5; done',
    '      [ "$transport_ready" = "1" ] || exit 1',
    '    fi',
    '  done',
    '  exit 0',
    'fi',
    'exit 1',
  ].join('\n');
  const proc = spawnSync('bash', ['-lc', script], {
    cwd: process.cwd(),
    env: {
      ...env,
      ORCHARD_HOST_PROXY_PID_FILE: HOST_PROXY_PID_FILE,
      ORCHARD_HOST_PROXY_WORKDIR: HOST_PROXY_WORKDIR,
      ORCHARD_HOST_PROXY_LOG: HOST_PROXY_LOG,
    },
    encoding: 'utf8',
    maxBuffer: 4 * 1024 * 1024,
  });
  if (proc.error) throw proc.error;
  assertOk(proc.status === 0, `host wallet-proxy restart failed (${proc.status}): ${proc.stderr || proc.stdout}`);
}

async function createPoolNote(rpc, runIndex, plan) {
  const poolAssetId = ASSET_IDS[plan.to];
  assertOk(poolAssetId, `unsupported pool asset for run ${runIndex}: ${plan.to}`);
  const funding = await fundAsset(rpc, POOL_OPERATOR, poolAssetId, SWAP_AMOUNT_ATOMS);
  const fundingConvergence = await waitForFleetConvergence(`pool-note-${runIndex}-after-public-funding`);
  const fundingSync = syncRelayState(`pool-note-${runIndex}-after-public-funding`);
  const preflight = await proxyPost('/api/shielded-nav-swap/preflight', {
    route: SHIELDED_NAVSWAP_ROUTE,
    wallet_address: POOL_OPERATOR,
    asset_id: poolAssetId,
    amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
  });
  const noteResult = await localPost('/asset-orchard/ingress-notes', {
    route: SHIELDED_NAVSWAP_ROUTE,
    wallet_address: POOL_OPERATOR,
    asset_id: poolAssetId,
    amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
    preflight,
  });
  const signedBurn = await signAssetOperationWithProxy(rpc, POOL_OPERATOR, preflight.operation, POOL_OPERATOR_KEY_FILE);
  const ingressPayload = buildAssetOrchardIngressPayload({
    signedBurnTransaction: signedBurn.signed,
    assetId: poolAssetId,
    amountAtoms: SWAP_AMOUNT_ATOMS.toString(),
    walletNote: noteResult.wallet_note,
    encryptedOutput: noteResult.encrypted_output,
  });
  const ingress = await proxyPost('/api/shielded-nav-swap/ingress', {
    route: SHIELDED_NAVSWAP_ROUTE,
    wallet_address: POOL_OPERATOR,
    ingress_payload: ingressPayload,
  });
  const outputCommitment = ingress.output_commitment || ingressPayload.output_commitment;
  assertOk(/^[0-9a-f]{64}$/.test(String(outputCommitment || '')), `pool note ${runIndex} missing output commitment`);
  const evidence = {
    schema: evidenceSchema('pool-note-v1'),
    run_index: runIndex,
    direction: `${plan.from}->${plan.to}`,
    captured_at: new Date().toISOString(),
    operator_wallet: POOL_OPERATOR,
    asset_symbol: plan.to,
    asset_id: poolAssetId,
    amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
    output_commitment: outputCommitment,
    funding,
    funding_convergence: {
      height: fundingConvergence.height,
      root: fundingConvergence.root,
      count: fundingConvergence.count,
    },
    funding_relay_state_sync: {
      local_after_height: fundingSync.local_after?.block_height || null,
      local_after_root: fundingSync.local_after?.state_root || null,
    },
    preflight: {
      ok: preflight.ok,
      status: preflight.status,
      amount_atoms: preflight.amount_atoms,
    },
    ingress: {
      ok: ingress.ok,
      status: ingress.status,
      message: ingress.message,
      artifact_dir: ingress.artifact_dir || null,
      receipts: ingress.receipts || [],
      report_round_ok: ingress.report?.round_ok ?? ingress.report?.transport?.round_ok ?? null,
    },
  };
  writeJson(join(OUT_DIR, `pool-note-${runIndex}.json`), evidence);
  return evidence;
}

async function createWalletNote(rpc, walletAddress, backupJson, runIndex, plan) {
  const walletAssetId = ASSET_IDS[plan.from];
  assertOk(walletAssetId, `unsupported wallet asset for run ${runIndex}: ${plan.from}`);
  const funding = await fundAsset(rpc, walletAddress, walletAssetId, SWAP_AMOUNT_ATOMS);
  const fundingConvergence = await waitForFleetConvergence(`wallet-note-${runIndex}-after-public-funding`);
  const fundingSync = syncRelayState(`wallet-note-${runIndex}-after-public-funding`);
  const preflight = await proxyPost('/api/shielded-nav-swap/preflight', {
    route: SHIELDED_NAVSWAP_ROUTE,
    wallet_address: walletAddress,
    asset_id: walletAssetId,
    amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
  });
  const noteResult = await localPost('/asset-orchard/ingress-notes', {
    route: SHIELDED_NAVSWAP_ROUTE,
    wallet_address: walletAddress,
    asset_id: walletAssetId,
    amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
    preflight,
  });
  const signedBurn = await signAssetOperationWithWallet(rpc, walletAddress, preflight.operation, backupJson);
  const ingressPayload = buildAssetOrchardIngressPayload({
    signedBurnTransaction: signedBurn.signed,
    assetId: walletAssetId,
    amountAtoms: SWAP_AMOUNT_ATOMS.toString(),
    walletNote: noteResult.wallet_note,
    encryptedOutput: noteResult.encrypted_output,
  });
  const ingress = await proxyPost('/api/shielded-nav-swap/ingress', {
    route: SHIELDED_NAVSWAP_ROUTE,
    wallet_address: walletAddress,
    ingress_payload: ingressPayload,
  });
  const outputCommitment = ingress.output_commitment || ingressPayload.output_commitment;
  assertOk(/^[0-9a-f]{64}$/.test(String(outputCommitment || '')), `wallet note ${runIndex} missing output commitment`);
  const evidence = {
    schema: evidenceSchema('wallet-note-v1'),
    run_index: runIndex,
    direction: `${plan.from}->${plan.to}`,
    captured_at: new Date().toISOString(),
    wallet_address: walletAddress,
    asset_symbol: plan.from,
    asset_id: walletAssetId,
    amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
    output_commitment: outputCommitment,
    funding,
    funding_convergence: {
      height: fundingConvergence.height,
      root: fundingConvergence.root,
      count: fundingConvergence.count,
    },
    funding_relay_state_sync: {
      local_after_height: fundingSync.local_after?.block_height || null,
      local_after_root: fundingSync.local_after?.state_root || null,
    },
    preflight: {
      ok: preflight.ok,
      status: preflight.status,
      amount_atoms: preflight.amount_atoms,
    },
    ingress: {
      ok: ingress.ok,
      status: ingress.status,
      message: ingress.message,
      artifact_dir: ingress.artifact_dir || null,
      receipts: ingress.receipts || [],
      report_round_ok: ingress.report?.round_ok ?? ingress.report?.transport?.round_ok ?? null,
    },
  };
  writeJson(join(OUT_DIR, `wallet-note-${runIndex}.json`), evidence);
  return evidence;
}

function existingWalletNote(walletAddress, runIndex, plan, commitment = '') {
  const walletAssetId = ASSET_IDS[plan.from];
  assertOk(walletAssetId, `unsupported wallet asset for run ${runIndex}: ${plan.from}`);
  assertOk(/^[0-9a-f]{64}$/.test(commitment), `run ${runIndex} requires a 32-byte existing wallet note commitment`);
  const file = join(LOCAL_VAULT_DIR, `${commitment}.json`);
  assertOk(existsSync(file), `existing wallet note ${commitment} is missing from local vault`);
  const record = publicVaultRecord(file);
  assertOk(record.wallet_address === walletAddress, `existing wallet note ${commitment} wallet mismatch`);
  assertOk(record.asset_id === walletAssetId, `existing wallet note ${commitment} asset mismatch`);
  assertOk(record.amount_atoms === SWAP_AMOUNT_ATOMS.toString(), `existing wallet note ${commitment} amount mismatch`);
  assertOk(record.state === 'spendable', `existing wallet note ${commitment} is ${record.state}, not spendable`);
  const evidence = {
    schema: evidenceSchema('existing-wallet-note-v1'),
    run_index: runIndex,
    direction: `${plan.from}->${plan.to}`,
    captured_at: new Date().toISOString(),
    wallet_address: walletAddress,
    asset_symbol: plan.from,
    asset_id: walletAssetId,
    amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
    output_commitment: commitment,
    source: STEP8_REVERSE ? 'accepted_step7_swap_output' : 'existing_local_vault_note',
    vault_record: record,
  };
  writeJson(join(OUT_DIR, `wallet-note-${runIndex}.json`), evidence);
  return evidence;
}

function existingPoolNote(runIndex, plan, commitment = '') {
  const poolAssetId = ASSET_IDS[plan.to];
  assertOk(poolAssetId, `unsupported pool asset for run ${runIndex}: ${plan.to}`);
  assertOk(/^[0-9a-f]{64}$/.test(commitment), `run ${runIndex} requires a 32-byte existing pool note commitment`);
  const file = join(LOCAL_VAULT_DIR, `${commitment}.json`);
  assertOk(existsSync(file), `existing pool note ${commitment} is missing from local vault`);
  const record = publicVaultRecord(file);
  assertOk(record.wallet_address === POOL_OPERATOR, `existing pool note ${commitment} wallet mismatch`);
  assertOk(record.asset_id === poolAssetId, `existing pool note ${commitment} asset mismatch`);
  assertOk(record.amount_atoms === SWAP_AMOUNT_ATOMS.toString(), `existing pool note ${commitment} amount mismatch`);
  assertOk(record.state === 'spendable', `existing pool note ${commitment} is ${record.state}, not spendable`);
  const evidence = {
    schema: evidenceSchema('existing-pool-note-v1'),
    run_index: runIndex,
    direction: `${plan.from}->${plan.to}`,
    captured_at: new Date().toISOString(),
    operator_wallet: POOL_OPERATOR,
    asset_symbol: plan.to,
    asset_id: poolAssetId,
    amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
    output_commitment: commitment,
    source: 'existing_local_vault_note',
    vault_record: record,
  };
  writeJson(join(OUT_DIR, `pool-note-${runIndex}.json`), evidence);
  return evidence;
}

function publicVaultRecord(file) {
  const record = JSON.parse(readFileSync(file, 'utf8'));
  const stat = statSync(file);
  return {
    file,
    mode_octal: (stat.mode & 0o777).toString(8).padStart(3, '0'),
    state: record.state || null,
    wallet_address: record.wallet_address || null,
    asset_id: record.asset_id || null,
    amount_atoms: record.amount_atoms?.toString?.() || String(record.amount_atoms || ''),
    output_commitment: record.wallet_note?.output_commitment || null,
    swap_id: record.swap_id || null,
    quote_binding_hash: record.quote_binding_hash || null,
    egress_id: record.egress_id || null,
    disclosure_hash: record.disclosure_hash || null,
  };
}

function vaultSnapshot(label, walletAddress, poolCommitment = null) {
  const rows = readdirSync(LOCAL_VAULT_DIR)
    .filter(name => name.endsWith('.json'))
    .map(name => join(LOCAL_VAULT_DIR, name))
    .map(publicVaultRecord)
    .filter(row => row.wallet_address === walletAddress || row.wallet_address === POOL_OPERATOR || row.output_commitment === poolCommitment)
    .sort((a, b) => String(a.output_commitment || '').localeCompare(String(b.output_commitment || '')));
  const snapshot = {
    schema: evidenceSchema('note-state-v1'),
    label,
    captured_at: new Date().toISOString(),
    wallet_address: walletAddress,
    pool_operator: POOL_OPERATOR,
    records: rows,
  };
  writeJson(join(OUT_DIR, `${label}-note-state.json`), snapshot);
  return snapshot;
}

const CLEAR_ACTION_KEYS = new Set([
  'amount',
  'amount_atoms',
  'asset_id',
  'asset_tag',
  'asset_tag_hi',
  'asset_tag_lo',
  'input_note',
  'input_notes',
  'note',
  'note_opening',
  'note_openings',
  'output_note',
  'output_notes',
  'rho',
  'rseed',
  'spend_key',
  'spending_key',
]);

function scanActionPrivacy(value, path = '$', hits = []) {
  if (!value || typeof value !== 'object') {
    if (typeof value === 'string') {
      const lower = value.toLowerCase();
      if (lower === A651_ASSET_ID || lower === A652_ASSET_ID || lower === SWAP_AMOUNT_ATOMS.toString()) {
        hits.push({ path, type: 'clear_value', value });
      }
    } else if (typeof value === 'number' && Number.isInteger(value) && BigInt(value) === SWAP_AMOUNT_ATOMS) {
      hits.push({ path, type: 'clear_numeric_amount', value });
    }
    return hits;
  }
  for (const [key, child] of Object.entries(value)) {
    const normalized = String(key)
      .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
      .replace(/[^A-Za-z0-9]+/g, '_')
      .replace(/^_+|_+$/g, '')
      .toLowerCase();
    const childPath = `${path}.${key}`;
    if (CLEAR_ACTION_KEYS.has(normalized)) hits.push({ path: childPath, type: 'clear_key', key });
    scanActionPrivacy(child, childPath, hits);
  }
  return hits;
}

function writeWirePrivacyEvidence(runIndex, action) {
  const hits = scanActionPrivacy(action);
  const evidence = {
    schema: evidenceSchema('wire-privacy-v1'),
    run_index: runIndex,
    captured_at: new Date().toISOString(),
    ok: hits.length === 0,
    scanner: 'swap_action_json_only',
    checked_against: {
      clear_asset_ids: [A651_ASSET_ID, A652_ASSET_ID],
      amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
      forbidden_keys: [...CLEAR_ACTION_KEYS].sort(),
    },
    hits,
    action_shape: {
      schema: action?.schema || null,
      pool_id: action?.pool_id || null,
      nullifiers: Array.isArray(action?.nullifiers) ? action.nullifiers.length : null,
      output_commitments: Array.isArray(action?.output_commitments) ? action.output_commitments.length : null,
      accounting_inputs: Array.isArray(action?.accounting_inputs) ? action.accounting_inputs.length : null,
      accounting_outputs: Array.isArray(action?.accounting_outputs) ? action.accounting_outputs.length : null,
      swap_binding_hash: action?.swap_binding_hash || null,
    },
  };
  writeJson(join(OUT_DIR, `run-${runIndex}-wire-privacy.json`), evidence);
  writeJson(join(OUT_DIR, `run-${runIndex}-swap-action.json`), action);
  assertOk(evidence.ok, `wire privacy scan failed for run ${runIndex}: ${JSON.stringify(hits)}`);
  return evidence;
}

const EGRESS_FORBIDDEN_KEYS = new Set([
  'input_note',
  'input_notes',
  'note',
  'note_file',
  'note_files',
  'note_opening',
  'note_openings',
  'opening',
  'rho',
  'rseed',
  'rcm',
  'seed',
  'seed_hex',
  'spend_key',
  'spending_key',
  'wallet_note',
]);

function scanEgressPrivacy(value, holdCommitment, path = '$', hits = []) {
  if (!value || typeof value !== 'object') {
    if (typeof value === 'string' && holdCommitment && value.toLowerCase().includes(holdCommitment)) {
      hits.push({ path, type: 'hold_private_commitment_disclosed', value });
    }
    return hits;
  }
  for (const [key, child] of Object.entries(value)) {
    const normalized = String(key)
      .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
      .replace(/[^A-Za-z0-9]+/g, '_')
      .replace(/^_+|_+$/g, '')
      .toLowerCase();
    const childPath = `${path}.${key}`;
    if (EGRESS_FORBIDDEN_KEYS.has(normalized)) hits.push({ path: childPath, type: 'forbidden_private_key', key });
    scanEgressPrivacy(child, holdCommitment, childPath, hits);
  }
  return hits;
}

function writeEgressPrivacyEvidence(egressFile, holdCommitment) {
  const hits = scanEgressPrivacy(egressFile, holdCommitment);
  const payload = egressFile?.payload || {};
  const evidence = {
    schema: evidenceSchema('egress-wire-privacy-v1'),
    captured_at: new Date().toISOString(),
    ok: hits.length === 0,
    scanner: 'private_egress_json_public_exit_payload',
    hold_commitment: holdCommitment,
    forbidden_keys: [...EGRESS_FORBIDDEN_KEYS].sort(),
    hits,
    action_shape: {
      file_schema: egressFile?.schema || null,
      action_schema: payload.schema || null,
      pool_id: payload.pool_id || null,
      to: payload.to || null,
      asset_id: payload.asset_id || null,
      amount_atoms: payload.amount?.toString?.() || String(payload.amount || ''),
      nullifier: payload.nullifier || null,
      exit_binding_hash: payload.exit_binding_hash || null,
      proof_bytes: typeof payload.proof === 'string' ? payload.proof.length / 2 : null,
    },
  };
  writeJson(join(OUT_DIR, 'step9-egress-wire-privacy.json'), evidence);
  writeJson(join(OUT_DIR, 'step9-private-egress-action.json'), egressFile);
  assertOk(evidence.ok, `egress privacy scan failed: ${JSON.stringify(hits)}`);
  return evidence;
}

function sensitiveLabels(sensitive) {
  return [
    { label: 'wallet_seed', value: sensitive?.seed || '' },
    { label: 'wallet_passphrase', value: sensitive?.passphrase || '' },
  ].filter(item => item.value);
}

function writeNoPrivateMaterialRequestEvidence(runIndex, captured, sensitive) {
  const evidence = buildNoPrivateMaterialRequestLog({
    entries: captured,
    runIndex,
    schema: evidenceSchema('no-private-material-request-log-v1'),
    sensitiveLabels: sensitiveLabels(sensitive),
  });
  writeJson(join(OUT_DIR, `run-${runIndex}-no-private-material-request-log.json`), evidence);
  assertOk(evidence.ok, `run ${runIndex} proxy request log contained private material: ${JSON.stringify(evidence.hits)}`);
  assertOk(evidence.scanned_request_count > 0, `run ${runIndex} captured no proxy-bound shielded requests`);
  return evidence;
}

function walletOutputFromFinalize(outputs, walletAddress, assetId) {
  return outputs.find(row => (
    row.wallet_address === walletAddress
    && row.asset_id === assetId
    && String(row.amount_atoms) === SWAP_AMOUNT_ATOMS.toString()
  )) || null;
}

function expectedReloadRecordsFromRuns(runs) {
  const records = new Map();
  for (const run of runs) {
    const expectedInput = run.expected_wallet_note?.output_commitment || null;
    if (expectedInput) records.set(expectedInput, {
      commitment: expectedInput,
      asset_id: run.from_asset_id,
      amount_atoms: run.amount_atoms,
      expected_state: 'spent',
      reason: `run-${run.run_index}-wallet-input`,
    });
    const output = run.wallet_output?.output_commitment || null;
    if (output) records.set(output, {
      commitment: output,
      asset_id: run.to_asset_id,
      amount_atoms: run.amount_atoms,
      expected_state: 'spendable',
      reason: `run-${run.run_index}-wallet-output`,
    });
  }
  for (const run of runs) {
    const consumed = run.expected_wallet_note?.output_commitment || null;
    if (consumed && records.has(consumed)) {
      records.set(consumed, {
        ...records.get(consumed),
        expected_state: 'spent',
        reason: `${records.get(consumed).reason}; consumed-by-run-${run.run_index}`,
      });
    }
  }
  return [...records.values()];
}

async function writeReloadRescanProof(page, sensitive, runs) {
  const before = vaultSnapshot('reload-rescan-before', sensitive.accountAddress);
  const capturedStart = await page.evaluate(() => window.__orchardSwapE2e?.entries?.length || 0);
  await refreshUnlockedWallet(page, sensitive);
  await selectShieldedRoute(page);
  const localNotes = await page.evaluate(async () => {
    const response = await fetch('http://127.0.0.1:8789/asset-orchard/notes');
    return response.json();
  });
  await page.screenshot({ path: join(OUT_DIR, 'reload-rescan-after-wallet-reload.png'), fullPage: true });
  const after = vaultSnapshot('reload-rescan-after', sensitive.accountAddress);
  const captured = await page.evaluate(start => (
    window.__orchardSwapE2e?.entries || []
  ).slice(start), capturedStart);
  const requestPrivacy = buildNoPrivateMaterialRequestLog({
    entries: captured,
    runIndex: 'reload-rescan',
    schema: evidenceSchema('reload-rescan-no-private-material-request-log-v1'),
    sensitiveLabels: sensitiveLabels(sensitive),
  });
  const expected = expectedReloadRecordsFromRuns(runs);
  const recordsByCommitment = new Map(after.records.map(row => [row.output_commitment, row]));
  const noteRows = Array.isArray(localNotes?.notes) ? localNotes.notes : [];
  const localById = new Map(noteRows.map(row => [row.id, row]));
  const checks = expected.map(item => {
    const vault = recordsByCommitment.get(item.commitment) || null;
    const local = localById.get(item.commitment) || null;
    return {
      ...item,
      vault_state: vault?.state || null,
      local_service_state: local?.state || null,
      ok: vault?.state === item.expected_state && local?.state === item.expected_state,
    };
  });
  const proxyRequests = captured.filter(entry => String(entry.url || '').includes('/api/shielded-nav-swap/'));
  const proof = {
    schema: evidenceSchema('reload-rescan-proof-v1'),
    captured_at: new Date().toISOString(),
    ok: checks.length > 0 && checks.every(check => check.ok) && requestPrivacy.ok === true,
    wallet_address: sensitive.accountAddress,
    local_service: LOCAL_SERVICE,
    browser_reload_performed: true,
    local_vault_rescan_performed: true,
    remote_proxy_request_count: proxyRequests.length,
    remote_proxy_request_private_material_ok: requestPrivacy.ok === true,
    remote_proxy_request_log_file: 'reload-rescan-no-private-material-request-log.json',
    expected_records: checks,
    local_note_count: noteRows.length,
    before_file: 'reload-rescan-before-note-state.json',
    after_file: 'reload-rescan-after-note-state.json',
    screenshot: 'reload-rescan-after-wallet-reload.png',
  };
  writeJson(join(OUT_DIR, 'reload-rescan-browser-capture.json'), captured);
  writeJson(join(OUT_DIR, 'reload-rescan-no-private-material-request-log.json'), requestPrivacy);
  writeJson(join(OUT_DIR, 'reload-rescan-proof.json'), proof);
  assertOk(proof.ok, `reload/rescan proof failed: ${JSON.stringify({ checks, request_hits: requestPrivacy.hits })}`);
  return proof;
}

async function installCapture(page) {
  await page.addInitScript(() => {
    window.__orchardSwapE2e = { entries: [] };
    const cloneBody = (value) => {
      try {
        return JSON.parse(JSON.stringify(value));
      } catch (_) {
        return null;
      }
    };
    const originalFetch = window.fetch.bind(window);
    window.fetch = async (...args) => {
      const startedAtUnixMs = Date.now();
      const startedAt = new Date(startedAtUnixMs).toISOString();
      const url = typeof args[0] === 'string' ? args[0] : args[0]?.url || '';
      let requestBody = null;
      if (url.includes('/api/shielded-nav-swap/swap')) {
        try {
          const parsed = JSON.parse(args[1]?.body || '{}');
          requestBody = {
            route: parsed.route,
            wallet_address: parsed.wallet_address,
            quote_binding_hash: parsed.quote_binding_hash,
            quote: parsed.quote ? {
              schema: parsed.quote.schema,
              quote_binding_hash: parsed.quote.quote_binding_hash,
              quote_expires_at_ms: parsed.quote.quote_expires_at_ms,
              liquidity: parsed.quote.liquidity,
            } : null,
            swap_action_json: parsed.swap_action_json || null,
          };
        } catch (error) {
          requestBody = { parse_error: String(error) };
        }
      } else if (url.includes('/api/shielded-nav-swap/egress')) {
        try {
          const parsed = JSON.parse(args[1]?.body || '{}');
          requestBody = {
            route: parsed.route,
            wallet_address: parsed.wallet_address,
            to: parsed.to,
            asset_id: parsed.asset_id,
            amount_atoms: parsed.amount_atoms,
            note_commitment: parsed.note_commitment,
            policy_id: parsed.policy_id,
            disclosure_hash: parsed.disclosure_hash,
            disclosure_ack: parsed.disclosure_ack,
            egress_json: parsed.egress_json || null,
          };
        } catch (error) {
          requestBody = { parse_error: String(error) };
        }
      } else if (url.includes('/api/shielded-nav-swap/')) {
        try {
          requestBody = args[1]?.body ? JSON.parse(args[1].body) : null;
        } catch (error) {
          requestBody = { parse_error: String(error) };
        }
      }
      const response = await originalFetch(...args);
      if (url.includes('/api/shielded-nav-swap/') || url.includes('/asset-orchard/')) {
        response.clone().json().then((body) => {
          const atUnixMs = Date.now();
          const redacted = cloneBody(body);
          if (url.includes('/asset-orchard/ingress-notes') && redacted?.wallet_note) {
            redacted.wallet_note = { output_commitment: redacted.wallet_note.output_commitment };
          }
          window.__orchardSwapE2e.entries.push({
            started_at: startedAt,
            started_at_unix_ms: startedAtUnixMs,
            at: new Date(atUnixMs).toISOString(),
            at_unix_ms: atUnixMs,
            elapsed_ms: atUnixMs - startedAtUnixMs,
            url,
            status: response.status,
            ok: response.ok,
            request: requestBody,
            body: redacted,
          });
        }).catch((error) => {
          const atUnixMs = Date.now();
          window.__orchardSwapE2e.entries.push({
            started_at: startedAt,
            started_at_unix_ms: startedAtUnixMs,
            at: new Date(atUnixMs).toISOString(),
            at_unix_ms: atUnixMs,
            elapsed_ms: atUnixMs - startedAtUnixMs,
            url,
            status: response.status,
            ok: response.ok,
            request: requestBody,
            error: String(error),
          });
        });
      }
      return response;
    };
  });
}

async function importWallet(page, sensitive) {
  await page.goto(APP_URL, { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 30_000 }).catch(() => {});
  await page.screenshot({ path: join(OUT_DIR, '00-onboard.png'), fullPage: true });
  await page.getByRole('button', { name: /^Import Wallet$/ }).click({ timeout: 30_000 });
  await page.locator('input[placeholder^="e.g."]').fill(sensitive.seed);
  await page.getByRole('button', { name: /^Validate Seed$/ }).click({ timeout: 30_000 });
  await page.waitForSelector(`text=${sensitive.accountAddress}`, { timeout: 30_000 });
  const passphraseInputs = page.locator('input[type="password"]');
  await passphraseInputs.nth(0).fill(sensitive.passphrase);
  await passphraseInputs.nth(1).fill(sensitive.passphrase);
  await page.getByRole('button', { name: /^Confirm Import$/ }).click({ timeout: 30_000 });
  await page.waitForSelector('text=/Account|Assets|Swap/i', { timeout: 45_000 });
  await page.screenshot({ path: join(OUT_DIR, '01-wallet-imported.png'), fullPage: true });
  return { accountAddress: sensitive.accountAddress };
}

async function unlockWalletIfNeeded(page, sensitive) {
  const unlockButton = page.locator('button').filter({ hasText: /^Unlock(?: Wallet)?$/i }).first();
  const unlockVisible = await unlockButton.isVisible({ timeout: 2000 }).catch(() => false);
  if (unlockVisible) {
    const password = page.locator('input[type="password"]').first();
    await password.fill(sensitive.passphrase);
    await unlockButton.click({ timeout: 30_000 });
  }
  await page.waitForSelector('text=/Account|Assets|Swap/i', { timeout: 45_000 });
}

async function refreshUnlockedWallet(page, sensitive) {
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 30_000 }).catch(() => {});
  await unlockWalletIfNeeded(page, sensitive);
}

async function clickEnabledButton(page, pattern, timeoutMs = 180_000) {
  const deadline = Date.now() + timeoutMs;
  let lastText = '';
  while (Date.now() <= deadline) {
    lastText = await page.locator('body').innerText().catch(() => '');
    const button = page.locator('button').filter({ hasText: pattern }).first();
    const visible = await button.isVisible({ timeout: 1000 }).catch(() => false);
    if (visible) {
      const disabled = await button.evaluate(node => Boolean(node.disabled)).catch(() => true);
      if (!disabled) {
        await button.click({ timeout: 30_000 });
        return true;
      }
    }
    await page.waitForTimeout(1000);
  }
  throw new Error(`timed out waiting for enabled button ${pattern}; last body:\n${lastText}`);
}

async function selectShieldedRoute(page) {
  await page.locator('button').filter({ hasText: 'Swap' }).first().click({ timeout: 15_000 });
  await page.waitForSelector('text=/Move between assets/i', { timeout: 45_000 });
  if (!/Private quote preview/i.test(await page.locator('body').innerText())) {
    await clickEnabledButton(page, /Shielded NAVSwap/i, 120_000);
    await page.waitForSelector('text=/Private quote preview/i', { timeout: 45_000 });
  }
}

async function selectShieldedDirection(page, plan) {
  const inputForDirection = page.locator(`input[aria-label="Amount of ${plan.from} to swap"]`).first();
  if (await inputForDirection.isVisible({ timeout: 2000 }).catch(() => false)) {
    return inputForDirection;
  }
  await page.getByRole('button', { name: /^Switch swap direction$/ }).click({ timeout: 30_000 });
  await inputForDirection.waitFor({ timeout: 45_000 });
  return inputForDirection;
}

function latestCaptured(entries, needle) {
  return [...entries].reverse().find(entry => String(entry.url || '').includes(needle)) || null;
}

function finiteUnixMs(value) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
}

function capturedAtUnixMs(entry) {
  const direct = finiteUnixMs(entry?.at_unix_ms);
  if (direct !== null) return direct;
  const parsed = Date.parse(entry?.at || '');
  return Number.isFinite(parsed) ? parsed : null;
}

function capturedStartedAtUnixMs(entry) {
  const direct = finiteUnixMs(entry?.started_at_unix_ms);
  if (direct !== null) return direct;
  const parsed = Date.parse(entry?.started_at || '');
  return Number.isFinite(parsed) ? parsed : null;
}

function serviceWarmthForRun(runIndex) {
  return SERVICE_WARMTH_LABELS[runIndex - 1]
    || (runIndex === 1 ? 'cold_fresh_service_first_swap' : 'warm_same_service_second_swap');
}

function nonNegativeSpan(startMs, endMs) {
  if (!Number.isFinite(startMs) || !Number.isFinite(endMs)) return null;
  return Math.max(0, endMs - startMs);
}

function buildClickReceiptWallClock({
  runIndex,
  direction,
  clickedAtUnixMs,
  clickedAt,
  terminalObservedAtUnixMs,
  terminalObservedAt,
  localEntry,
  swapEntry,
  finalizeEntry,
}) {
  const localEndMs = capturedAtUnixMs(localEntry);
  const swapEndMs = capturedAtUnixMs(swapEntry);
  const finalizeEndMs = capturedAtUnixMs(finalizeEntry);
  const certifiedReceiptAtUnixMs = swapEndMs || terminalObservedAtUnixMs;
  const stages = [];
  let cursorMs = clickedAtUnixMs;
  if (localEndMs !== null && localEndMs >= cursorMs && localEndMs <= certifiedReceiptAtUnixMs) {
    stages.push({
      order: stages.length,
      stage: 'local_action',
      metric: 'browser_fetch_response_ms',
      ms: nonNegativeSpan(cursorMs, localEndMs),
      source: '/asset-orchard/swap-actions',
      start_at_unix_ms: cursorMs,
      end_at_unix_ms: localEndMs,
      note: 'browser submit click through local Asset-Orchard swap action/proof response',
    });
    cursorMs = localEndMs;
  }
  stages.push({
    order: stages.length,
    stage: 'proxy_certified_receipt',
    metric: 'browser_fetch_response_ms',
    ms: nonNegativeSpan(cursorMs, certifiedReceiptAtUnixMs),
    source: '/api/shielded-nav-swap/swap',
    start_at_unix_ms: cursorMs,
    end_at_unix_ms: certifiedReceiptAtUnixMs,
    note: 'wallet-proxy certified receipt path; includes shield-batch-swap subprocess and certified round, not a pure proof bucket',
  });
  const clickToReceiptMs = nonNegativeSpan(clickedAtUnixMs, certifiedReceiptAtUnixMs);
  const clickToUiCompleteMs = nonNegativeSpan(clickedAtUnixMs, terminalObservedAtUnixMs);
  const uiCompletionStages = [];
  if (finalizeEndMs !== null && swapEndMs !== null && finalizeEndMs >= swapEndMs) {
    uiCompletionStages.push({
      order: uiCompletionStages.length,
      stage: 'local_finalize',
      metric: 'browser_fetch_response_ms',
      ms: nonNegativeSpan(swapEndMs, finalizeEndMs),
      source: '/asset-orchard/swap-finalize',
      start_at_unix_ms: swapEndMs,
      end_at_unix_ms: finalizeEndMs,
    });
  }
  if (terminalObservedAtUnixMs !== null) {
    const settleStart = finalizeEndMs !== null && finalizeEndMs >= certifiedReceiptAtUnixMs
      ? finalizeEndMs
      : certifiedReceiptAtUnixMs;
    if (terminalObservedAtUnixMs >= settleStart) {
      uiCompletionStages.push({
        order: uiCompletionStages.length,
        stage: 'ui_settle',
        metric: 'browser_observed_terminal_state_ms',
        ms: nonNegativeSpan(settleStart, terminalObservedAtUnixMs),
        source: 'browser DOM terminal text',
        start_at_unix_ms: settleStart,
        end_at_unix_ms: terminalObservedAtUnixMs,
      });
    }
  }
  return {
    schema: 'postfiat-wallet-private-swap-click-receipt-wall-clock-v1',
    run_index: runIndex,
    direction,
    circuit: 'swap',
    measurement: 'click_to_certified_receipt',
    run_label: serviceWarmthForRun(runIndex),
    service_warmth: serviceWarmthForRun(runIndex),
    boundary: 'browser before clicking Submit private swap -> /api/shielded-nav-swap/swap response captured by browser fetch wrapper',
    total_metric: 'click_to_certified_receipt_ms',
    clicked_at: clickedAt,
    clicked_at_unix_ms: clickedAtUnixMs,
    certified_receipt_at: certifiedReceiptAtUnixMs ? new Date(certifiedReceiptAtUnixMs).toISOString() : null,
    certified_receipt_at_unix_ms: certifiedReceiptAtUnixMs,
    terminal_observed_at: terminalObservedAt,
    terminal_observed_at_unix_ms: terminalObservedAtUnixMs,
    local_action_started_at_unix_ms: capturedStartedAtUnixMs(localEntry),
    proxy_swap_started_at_unix_ms: capturedStartedAtUnixMs(swapEntry),
    click_to_certified_receipt_ms: clickToReceiptMs,
    click_to_ui_complete_ms: clickToUiCompleteMs,
    stages,
    ui_completion_stages: uiCompletionStages,
    proxy_timings_ms: swapEntry?.body?.timings_ms || null,
    notes: [
      'No stage named proof is emitted here; proof-only timing must come from direct Halo2 generation instrumentation.',
      'proxy_certified_receipt is intentionally broad because the live wallet proxy currently wraps a shield-batch-swap subprocess plus certified transport.',
    ],
  };
}

async function runSwap(page, wallet, poolNote, runIndex, sensitive, plan, walletNote = null) {
  const fromAssetId = ASSET_IDS[plan.from];
  const toAssetId = ASSET_IDS[plan.to];
  assertOk(fromAssetId && toAssetId, `run ${runIndex} has unsupported direction ${plan.from}->${plan.to}`);
  restartProxyForLiquidity(poolNote.output_commitment);
  const syncBefore = syncRelayState(`run-${runIndex}-before-swap`);
  await refreshUnlockedWallet(page, sensitive);
  await selectShieldedRoute(page);
  const input = await selectShieldedDirection(page, plan);
  await input.fill(SWAP_AMOUNT);
  await page.screenshot({ path: join(OUT_DIR, `run-${runIndex}-before-quote.png`), fullPage: true });

  const beforeState = vaultSnapshot(`run-${runIndex}-before`, wallet.accountAddress, poolNote.output_commitment);
  const capturedStart = await page.evaluate(() => window.__orchardSwapE2e?.entries?.length || 0);
  await clickEnabledButton(page, /Get private quote|Refresh private quote/i, 180_000);
  await page.waitForFunction(
    () => /Private quote loaded/i.test(document.body.innerText || ''),
    null,
    { timeout: 180_000 },
  );
  await page.screenshot({ path: join(OUT_DIR, `run-${runIndex}-quote-ready.png`), fullPage: true });
  const clickedAtUnixMs = Date.now();
  const clickedAt = new Date(clickedAtUnixMs).toISOString();
  await clickEnabledButton(page, /Submit private swap/i, 300_000);
  await page.waitForFunction(
    () => /Private swap certified|Private swap blocked|Swap needs attention/i.test(document.body.innerText || ''),
    null,
    { timeout: SUBMIT_WAIT_MS },
  );
  const terminalObservedAtUnixMs = Date.now();
  const terminalObservedAt = new Date(terminalObservedAtUnixMs).toISOString();
  await page.waitForTimeout(1500);
  const finalText = await page.locator('body').innerText();
  await page.screenshot({ path: join(OUT_DIR, `run-${runIndex}-after-swap.png`), fullPage: true });
  const captured = await page.evaluate((start) => (
    window.__orchardSwapE2e?.entries || []
  ).slice(start), capturedStart);
  writeJson(join(OUT_DIR, `run-${runIndex}-browser-capture.json`), captured);
  assertOk(!/Private swap blocked|Swap needs attention/i.test(finalText), `swap run ${runIndex} failed in UX:\n${finalText}`);
  const quoteEntry = latestCaptured(captured, '/api/shielded-nav-swap/quote');
  const localEntry = latestCaptured(captured, '/asset-orchard/swap-actions');
  const swapEntry = latestCaptured(captured, '/api/shielded-nav-swap/swap');
  const finalizeEntry = latestCaptured(captured, '/asset-orchard/swap-finalize');
  assertOk(quoteEntry?.body?.ok === true, `run ${runIndex} missing successful quote`);
  assertOk(localEntry?.body?.ok === true, `run ${runIndex} missing successful local swap action`);
  assertOk(swapEntry?.body?.ok === true, `run ${runIndex} missing successful certified swap`);
  assertOk(finalizeEntry?.body?.ok === true, `run ${runIndex} missing successful local finalize`);
  const wallClock = buildClickReceiptWallClock({
    runIndex,
    direction: `${plan.from}->${plan.to}`,
    clickedAtUnixMs,
    clickedAt,
    terminalObservedAtUnixMs,
    terminalObservedAt,
    localEntry,
    swapEntry,
    finalizeEntry,
  });
  const finalizedInputs = Array.isArray(finalizeEntry.body.inputs) ? finalizeEntry.body.inputs : [];
  const finalizedOutputs = Array.isArray(finalizeEntry.body.outputs) ? finalizeEntry.body.outputs : [];
  const walletOutput = walletOutputFromFinalize(finalizedOutputs, wallet.accountAddress, toAssetId);
  const action = JSON.parse(swapEntry.request.swap_action_json);
  const requestPrivacy = writeNoPrivateMaterialRequestEvidence(runIndex, captured, sensitive);
  const wirePrivacy = writeWirePrivacyEvidence(runIndex, action);
  const afterState = vaultSnapshot(`run-${runIndex}-after`, wallet.accountAddress, poolNote.output_commitment);
  const convergence = await waitForFleetConvergence(`run-${runIndex}-after-swap`);
  const syncAfter = syncRelayState(`run-${runIndex}-after-swap`);
  const evidence = {
    schema: evidenceSchema('live-run-v1'),
    run_index: runIndex,
    direction: `${plan.from}->${plan.to}`,
    captured_at: new Date().toISOString(),
    wallet_address: wallet.accountAddress,
    from_asset: plan.from,
    to_asset: plan.to,
    from_asset_id: fromAssetId,
    to_asset_id: toAssetId,
    amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
    wall_clock: wallClock,
    expected_wallet_note: walletNote ? {
      output_commitment: walletNote.output_commitment,
      source: walletNote.source || null,
    } : null,
    wallet_output: walletOutput ? {
      output_commitment: walletOutput.id || walletOutput.output_commitment || null,
      asset_id: walletOutput.asset_id || null,
      amount_atoms: String(walletOutput.amount_atoms || ''),
      state: walletOutput.state || null,
    } : null,
    pool_note: poolNote,
    quote: {
      ok: quoteEntry.body.ok,
      status: quoteEntry.body.status,
      quote_binding_hash: quoteEntry.body.quote_binding_hash,
      quote_expires_at_ms: quoteEntry.body.quote_expires_at_ms,
      liquidity_commitment: quoteEntry.body.liquidity?.commitment || null,
      submit_enabled: quoteEntry.body.submit_enabled,
    },
    local_action: {
      ok: localEntry.body.ok,
      swap_id: localEntry.body.swap_id,
      verification: localEntry.body.verification,
      vault_update: localEntry.body.vault_update,
    },
    relay: {
      ok: swapEntry.body.ok,
      status: swapEntry.body.status,
      message: swapEntry.body.message,
      quote_binding_hash: swapEntry.body.quote_binding_hash,
      liquidity_commitment: swapEntry.body.liquidity_commitment,
      artifact_dir: swapEntry.body.artifact_dir || null,
      receipts: swapEntry.body.receipts || [],
      report_round_ok: swapEntry.body.report?.round_ok ?? swapEntry.body.report?.transport?.round_ok ?? null,
      trust_class: swapEntry.body.trust_class,
      quote_binding_enforcement: swapEntry.body.quote_binding_enforcement,
      timings_ms: swapEntry.body.timings_ms || null,
      laggard_catch_up: swapEntry.body.laggard_catch_up || null,
    },
    finalize: {
      ok: finalizeEntry.body.ok,
      swap_id: finalizeEntry.body.swap_id,
      accepted: finalizeEntry.body.accepted,
      inputs: finalizeEntry.body.inputs,
      outputs: finalizeEntry.body.outputs,
    },
    wire_privacy: wirePrivacy,
    request_privacy: requestPrivacy,
    note_state_files: {
      before: `run-${runIndex}-before-note-state.json`,
      after: `run-${runIndex}-after-note-state.json`,
    },
    relay_state_sync: {
      before_height: syncBefore.local_after?.block_height || null,
      after_height: syncAfter.local_after?.block_height || null,
    },
    fleet_convergence: {
      height: convergence.height,
      root: convergence.root,
      count: convergence.count,
      repair_invocations: convergence.repair_invocations || [],
      zero_repair_required: ZERO_REPAIR,
    },
    screenshots: [
      `run-${runIndex}-before-quote.png`,
      `run-${runIndex}-quote-ready.png`,
      `run-${runIndex}-after-swap.png`,
    ],
    assertions: {
      wallet_input_spent: finalizedInputs.some(row => row.wallet_address === wallet.accountAddress && row.asset_id === fromAssetId && String(row.amount_atoms) === SWAP_AMOUNT_ATOMS.toString() && row.state === 'spent'),
      expected_wallet_input_spent: walletNote
        ? finalizedInputs.some(row => row.id === walletNote.output_commitment && row.wallet_address === wallet.accountAddress && row.asset_id === fromAssetId && String(row.amount_atoms) === SWAP_AMOUNT_ATOMS.toString() && row.state === 'spent')
        : null,
      wallet_output_spendable: finalizedOutputs.some(row => row.wallet_address === wallet.accountAddress && row.asset_id === toAssetId && String(row.amount_atoms) === SWAP_AMOUNT_ATOMS.toString() && row.state === 'spendable'),
      expected_wallet_output_recorded: Boolean(walletOutput?.id || walletOutput?.output_commitment),
      pool_input_spent: finalizedInputs.some(row => row.id === poolNote.output_commitment && row.state === 'spent'),
      zero_repair: !Array.isArray(convergence.repair_invocations) || convergence.repair_invocations.length === 0,
      no_private_material_proxy_requests: requestPrivacy.ok,
    },
  };
  assertOk(evidence.assertions.wallet_input_spent, `run ${runIndex} did not mark ${plan.from} input spent`);
  if (walletNote) {
    assertOk(evidence.assertions.expected_wallet_input_spent, `run ${runIndex} spent a different ${plan.from} note than expected ${walletNote.output_commitment}`);
  }
  assertOk(evidence.assertions.wallet_output_spendable, `run ${runIndex} did not record spendable ${plan.to} output`);
  assertOk(evidence.assertions.expected_wallet_output_recorded, `run ${runIndex} did not expose wallet output commitment`);
  assertOk(evidence.assertions.pool_input_spent, `run ${runIndex} did not mark pool input spent`);
  assertOk(evidence.assertions.zero_repair, `run ${runIndex} recorded a validator repair invocation`);
  assertOk(evidence.assertions.no_private_material_proxy_requests, `run ${runIndex} proxy request privacy failed`);
  writeJson(join(OUT_DIR, `run-${runIndex}-evidence.json`), evidence);
  return evidence;
}

function balanceForValidator(snapshot, validator) {
  const row = snapshot.rows.find(item => item.validator === validator);
  assertOk(row?.ok, `validator-${validator} account_assets missing: ${row?.error || 'not found'}`);
  return BigInt(row.balance_atoms);
}

function assertStep10CanonicalPlan() {
  assertOk(Number.isInteger(RUNS) && RUNS === 2, 'Step 10 requires exactly two wallet-local runs');
  assertOk(SWAP_PLAN.length === 2, `Step 10 swap plan length ${SWAP_PLAN.length} is not 2`);
  assertOk(
    SWAP_PLAN[0].from === 'a651'
      && SWAP_PLAN[0].to === 'a652'
      && SWAP_PLAN[1].from === 'a652'
      && SWAP_PLAN[1].to === 'a651',
    `Step 10 swap plan must be a651->a652,a652->a651; got ${SWAP_PLAN.map(row => `${row.from}->${row.to}`).join(',')}`,
  );
}

function step10LiveEvidenceSlot() {
  return process.env.STEP10_LIVE_EVIDENCE_SLOT
    || `docs/evidence/wallet-private-swap-step10-live-${utcStampForPath()}`;
}

function step10StakehubEvidenceSlot() {
  return process.env.STEP10_STAKEHUB_EVIDENCE_SLOT
    || `docs/evidence/stakehub-shielded-navswap-step10-${utcStampForPath()}`;
}

function writeStep10OperatorDemoCommand(evidenceDir = OUT_DIR) {
  const evidenceSlot = step10StakehubEvidenceSlot();
  const command = [
    'cd "$STAKEHUB_REPO"',
    `python3 scripts/shielded-nav-swap-e2e-live.py --report-dir '${evidenceSlot}'`,
  ].join(' && ');
  const artifact = {
    schema: evidenceSchema('stakehub-operator-demo-command-v1'),
    prepared_at: new Date().toISOString(),
    prepared_only: true,
    not_executed_by_this_harness: true,
    live_window_required: true,
    command,
    command_cwd: '$STAKEHUB_REPO',
    script: 'scripts/shielded-nav-swap-e2e-live.py',
    evidence_slot: evidenceSlot,
    acceptance_role: 'operator-demo evidence rerun after Step 10 live window opens',
  };
  writeJson(join(evidenceDir, 'stakehub-operator-demo-command.json'), artifact);
  return artifact;
}

function writeStep10PrepPacket() {
  assertStep10CanonicalPlan();
  const liveEvidenceSlot = step10LiveEvidenceSlot();
  const stakehub = writeStep10OperatorDemoCommand(OUT_DIR);
  const liveWalletCommand = [
    'cd "$POSTFIAT_REPO"',
    [
      "ORCHARD_SWAP_E2E_STEP=step10-pair",
      "ORCHARD_SWAP_E2E_LIVE_WINDOW=true",
      "ORCHARD_SWAP_E2E_RUNS=2",
      "ORCHARD_SWAP_E2E_PLAN='a651->a652,a652->a651'",
      "ORCHARD_SWAP_E2E_ZERO_REPAIR=true",
      `ORCHARD_SWAP_E2E_OUT_DIR='${liveEvidenceSlot}'`,
      'node scripts/wallet-shielded-swap-step7-e2e.mjs',
    ].join(' '),
  ].join(' && ');
  const packageCommand = [
    'cd "$POSTFIAT_REPO"',
    `STEP10_EVIDENCE_DIR='${liveEvidenceSlot}' node scripts/wallet-shielded-swap-step10-package.mjs`,
  ].join(' && ');
  const prep = {
    schema: evidenceSchema('prep-plan-v1'),
    prepared_at: new Date().toISOString(),
    prep_only: true,
    live_devnet_rounds_executed: false,
    live_window_required: true,
    stop_gate: 'await STEP 10 LIVE window line before executing live wallet or StakeHub commands',
    harness_mode: 'step10-pair',
    canonical_pair: SWAP_PLAN.map(row => `${row.from}->${row.to}`),
    amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
    amount_display: SWAP_AMOUNT,
    wallet_live_evidence_slot: liveEvidenceSlot,
    wallet_live_command: liveWalletCommand,
    package_command: packageCommand,
    stakehub_operator_demo: stakehub,
    required_artifacts_after_live_run: [
      'run-1-evidence.json',
      'run-2-evidence.json',
      'run-1-no-private-material-request-log.json',
      'run-2-no-private-material-request-log.json',
      'reload-rescan-proof.json',
      'reload-rescan-no-private-material-request-log.json',
      'stakehub-operator-demo-command.json',
      'step10-summary.json',
    ],
    custody_boundary: {
      proxy_bound_request_scanner: 'scripts/lib/wallet-shielded-step10-evidence.mjs',
      private_material_kept_local: [
        'wallet seed',
        'wallet passphrase',
        'wallet note openings',
        'spend material',
      ],
    },
    can_run_approval: {
      operator_approval_required: true,
      can_run_changed_by_prep: false,
      note: 'Step 10 final acceptance is the operator approval to enable can_run; this prep packet does not flip it.',
    },
  };
  writeJson(join(OUT_DIR, 'step10-prep-plan.json'), prep);
  writeJson(join(OUT_DIR, 'step10-prep-status.json'), {
    schema: evidenceSchema('prep-status-v1'),
    captured_at: new Date().toISOString(),
    ok: true,
    prep_only: true,
    live_devnet_rounds_executed: false,
    artifacts: [
      'step10-prep-plan.json',
      'step10-prep-status.json',
      'stakehub-operator-demo-command.json',
    ],
    stop_gate: prep.stop_gate,
  });
  console.log(JSON.stringify({
    ok: true,
    prep_only: true,
    out_dir: OUT_DIR,
    live_wallet_command: liveWalletCommand,
    package_command: packageCommand,
    stakehub_operator_demo_command: stakehub.command,
    stop_gate: prep.stop_gate,
  }, null, 2));
}

async function runStep10Pair() {
  assertOk(
    STEP10_LIVE_WINDOW,
    'Step 10 live pair is blocked until ORCHARD_SWAP_E2E_LIVE_WINDOW=true is set after the explicit STEP 10 LIVE window line',
  );
  assertStep10CanonicalPlan();
  assertOk(ZERO_REPAIR, 'Step 10 requires the zero-repair bar; set ORCHARD_SWAP_E2E_ZERO_REPAIR=true');
  assertOk(existsSync(SENSITIVE_FILE), `missing sensitive wallet file: ${SENSITIVE_FILE}`);
  const sensitive = JSON.parse(readFileSync(SENSITIVE_FILE, 'utf8'));
  assertOk(/^[0-9a-f]{64}$/i.test(sensitive.seed), 'sensitive wallet seed is malformed');
  assertOk(/^pf[a-f0-9]{40}$/i.test(sensitive.accountAddress), 'sensitive wallet address is malformed');
  if (USE_EXISTING_WALLET_NOTES) {
    assertOk(
      EXISTING_WALLET_INPUT_COMMITMENTS.length === 1,
      `Step 10 existing-wallet-note mode requires exactly one initial a651 note, got ${EXISTING_WALLET_INPUT_COMMITMENTS.length}`,
    );
  }

  const rpc = new RpcClient(RPC_URL);
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
  });
  const poolNotes = [];
  const walletNotes = [];
  const runs = [];
  let wallet = null;
  let walletPftFunding = null;
  let poolPftFunding = null;
  let reloadProof = null;
  try {
    const backupJson = walletBackupFromSensitive(sensitive);
    syncRelayState('initial');
    await waitForFleetConvergence('initial');
    walletPftFunding = await ensurePftBalance(rpc, sensitive.accountAddress, WALLET_PFT_FUND_ATOMS);
    writeJson(join(OUT_DIR, 'wallet-pft-funding.json'), walletPftFunding);
    poolPftFunding = await ensurePftBalance(rpc, POOL_OPERATOR, POOL_OPERATOR_PFT_FUND_ATOMS);
    writeJson(join(OUT_DIR, 'pool-operator-pft-funding.json'), poolPftFunding);

    if (USE_EXISTING_WALLET_NOTES) {
      walletNotes.push(existingWalletNote(
        sensitive.accountAddress,
        1,
        SWAP_PLAN[0],
        EXISTING_WALLET_INPUT_COMMITMENTS[0],
      ));
    } else {
      walletNotes.push(await createWalletNote(rpc, sensitive.accountAddress, backupJson, 1, SWAP_PLAN[0]));
      await waitForFleetConvergence('wallet-note-1-after-ingress');
      syncRelayState('wallet-note-1-after-ingress');
    }
    poolNotes.push(await createPoolNote(rpc, 1, SWAP_PLAN[0]));
    await waitForFleetConvergence('pool-note-1-after-ingress');
    syncRelayState('pool-note-1-after-ingress');
    poolNotes.push(await createPoolNote(rpc, 2, SWAP_PLAN[1]));
    await waitForFleetConvergence('pool-note-2-after-ingress');
    syncRelayState('pool-note-2-after-ingress');

    const context = await browser.newContext({
      ignoreHTTPSErrors: true,
      viewport: { width: 1440, height: 1100 },
      permissions: ['local-network-access'],
    });
    await context.grantPermissions(['local-network-access'], { origin: new URL(APP_URL).origin });
    const page = await context.newPage();
    page.setDefaultTimeout(60_000);
    page.on('console', (message) => {
      const text = message.text();
      if (/orchard|shielded|swap|navswap|wallet/i.test(text)) {
        console.log(`[browser:${message.type()}] ${text}`);
      }
    });
    await installCapture(page);
    wallet = await importWallet(page, sensitive);
    runs.push(await runSwap(page, wallet, poolNotes[0], 1, sensitive, SWAP_PLAN[0], walletNotes[0]));
    const run1Output = runs[0].wallet_output?.output_commitment || null;
    assertOk(/^[0-9a-f]{64}$/.test(String(run1Output || '')), `Step 10 run 1 did not produce a usable ${SWAP_PLAN[0].to} wallet output`);
    walletNotes.push({
      schema: evidenceSchema('previous-run-wallet-output-v1'),
      run_index: 2,
      direction: `${SWAP_PLAN[1].from}->${SWAP_PLAN[1].to}`,
      captured_at: new Date().toISOString(),
      wallet_address: wallet.accountAddress,
      asset_symbol: SWAP_PLAN[1].from,
      asset_id: ASSET_IDS[SWAP_PLAN[1].from],
      amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
      output_commitment: run1Output,
      source: 'step10_run_1_wallet_output',
    });
    writeJson(join(OUT_DIR, 'wallet-note-2.json'), walletNotes[1]);
    runs.push(await runSwap(page, wallet, poolNotes[1], 2, sensitive, SWAP_PLAN[1], walletNotes[1]));
    reloadProof = await writeReloadRescanProof(page, sensitive, runs);
    await context.close();
  } finally {
    await browser.close();
    rpc.close();
  }

  const stakehub = writeStep10OperatorDemoCommand(OUT_DIR);
  const report = {
    schema: evidenceSchema('live-e2e-v1'),
    captured_at: new Date().toISOString(),
    step_mode: STEP_MODE,
    app_url: APP_URL,
    rpc_url: RPC_URL,
    route: SHIELDED_NAVSWAP_ROUTE,
    local_service: LOCAL_SERVICE,
    local_vault_dir: LOCAL_VAULT_DIR,
    wallet_address: wallet?.accountAddress || sensitive.accountAddress,
    pool_operator: POOL_OPERATOR,
    pool_operator_pft_funding: poolPftFunding,
    swap_plan: SWAP_PLAN,
    amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
    wallet_notes: walletNotes,
    pool_notes: poolNotes,
    wallet_pft_funding: walletPftFunding,
    runs,
    reload_rescan_proof: reloadProof,
    stakehub_operator_demo_command: stakehub,
    zero_repair_required: ZERO_REPAIR,
    repair_invocations: runs.flatMap(run => run.fleet_convergence?.repair_invocations || []),
    operator_approval_required_for_can_run: true,
    can_run_changed_by_harness: false,
    sensitive_material_file: SENSITIVE_FILE,
    sensitive_material_note: 'Wallet seed/passphrase remain in this local /tmp file and are not copied into docs/evidence.',
    redaction: {
      proxy_requests: 'captured and scanned for forbidden private keys/sensitive values',
      swap_action_json: 'captured and scanned because it is proxy-bound public action data',
      wallet_note_openings: 'not sent to proxy and not written under docs/evidence',
      spend_material: 'kept behind loopback/local service boundary',
    },
  };
  writeJson(join(OUT_DIR, 'report.json'), report);
  const summary = buildStep10EvidenceSummary({
    evidenceDir: OUT_DIR,
    files: { readJson },
  });
  writeJson(join(OUT_DIR, 'step10-summary.json'), summary);
  assertOk(summary.package_ok, `Step 10 evidence package did not pass: ${JSON.stringify(summary.pass_fail_summaries)}`);
  console.log(JSON.stringify({
    ok: true,
    out_dir: OUT_DIR,
    wallet_address: report.wallet_address,
    run_count: runs.length,
    directions: runs.map(run => run.direction),
    package_ok: summary.package_ok,
    operator_approval_required_for_can_run: true,
  }, null, 2));
}

async function runStep10RescanOnly() {
  assertOk(STEP10_LIVE_WINDOW, 'Step 10 rescan is blocked until ORCHARD_SWAP_E2E_LIVE_WINDOW=true is set');
  assertStep10CanonicalPlan();
  assertOk(existsSync(SENSITIVE_FILE), `missing sensitive wallet file: ${SENSITIVE_FILE}`);
  const sensitive = JSON.parse(readFileSync(SENSITIVE_FILE, 'utf8'));
  assertOk(/^[0-9a-f]{64}$/i.test(sensitive.seed), 'sensitive wallet seed is malformed');
  assertOk(/^pf[a-f0-9]{40}$/i.test(sensitive.accountAddress), 'sensitive wallet address is malformed');
  const runs = [1, 2].map(index => readJson(join(OUT_DIR, `run-${index}-evidence.json`)));
  assertOk(runs.length === 2, 'Step 10 rescan requires run-1-evidence.json and run-2-evidence.json');

  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
  });
  try {
    const context = await browser.newContext({
      ignoreHTTPSErrors: true,
      viewport: { width: 1440, height: 1100 },
      permissions: ['local-network-access'],
    });
    await context.grantPermissions(['local-network-access'], { origin: new URL(APP_URL).origin });
    const page = await context.newPage();
    page.setDefaultTimeout(60_000);
    await installCapture(page);
    await importWallet(page, sensitive);
    const reloadProof = await writeReloadRescanProof(page, sensitive, runs);
    await context.close();

    const stakehub = writeStep10OperatorDemoCommand(OUT_DIR);
    const summary = buildStep10EvidenceSummary({
      evidenceDir: OUT_DIR,
      files: { readJson },
    });
    writeJson(join(OUT_DIR, 'step10-summary.json'), summary);
    assertOk(summary.package_ok, `Step 10 evidence package did not pass: ${JSON.stringify(summary.pass_fail_summaries)}`);
    console.log(JSON.stringify({
      ok: true,
      rescan_only: true,
      out_dir: OUT_DIR,
      reload_rescan_ok: reloadProof.ok,
      package_ok: summary.package_ok,
      stakehub_operator_demo_command: stakehub.command,
    }, null, 2));
  } finally {
    await browser.close();
  }
}

async function runStep9Egress() {
  assertOk(existsSync(SENSITIVE_FILE), `missing sensitive wallet file: ${SENSITIVE_FILE}`);
  assertOk(/^[0-9a-f]{64}$/.test(STEP9_EGRESS_COMMITMENT), 'Step 9 egress commitment is malformed');
  assertOk(STEP9_HOLD_COMMITMENT === null || /^[0-9a-f]{64}$/.test(STEP9_HOLD_COMMITMENT), 'Step 9 hold commitment is malformed');
  const sensitive = JSON.parse(readFileSync(SENSITIVE_FILE, 'utf8'));
  assertOk(/^[0-9a-f]{64}$/i.test(sensitive.seed), 'sensitive wallet seed is malformed');
  assertOk(/^pf[a-f0-9]{40}$/i.test(sensitive.accountAddress), 'sensitive wallet address is malformed');

  const egressBeforeRecord = publicVaultRecord(join(LOCAL_VAULT_DIR, `${STEP9_EGRESS_COMMITMENT}.json`));
  const holdBeforeRecord = STEP9_HOLD_COMMITMENT
    ? publicVaultRecord(join(LOCAL_VAULT_DIR, `${STEP9_HOLD_COMMITMENT}.json`))
    : null;
  assertOk(egressBeforeRecord.wallet_address === sensitive.accountAddress, 'Step 9 egress note wallet mismatch');
  assertOk(egressBeforeRecord.asset_id === A651_ASSET_ID, 'Step 9 egress note asset mismatch');
  assertOk(egressBeforeRecord.amount_atoms === SWAP_AMOUNT_ATOMS.toString(), 'Step 9 egress note amount mismatch');
  assertOk(egressBeforeRecord.state === 'spendable', `Step 9 egress note is ${egressBeforeRecord.state}, not spendable`);
  if (holdBeforeRecord) {
    assertOk(holdBeforeRecord.wallet_address === sensitive.accountAddress, 'Step 9 hold note wallet mismatch');
    assertOk(holdBeforeRecord.asset_id === A651_ASSET_ID, 'Step 9 hold note asset mismatch');
    assertOk(holdBeforeRecord.amount_atoms === SWAP_AMOUNT_ATOMS.toString(), 'Step 9 hold note amount mismatch');
    assertOk(holdBeforeRecord.state === 'spendable', `Step 9 hold note is ${holdBeforeRecord.state}, not spendable`);
  }

  const rpc = new RpcClient(RPC_URL);
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
  });
  let wallet = null;
  try {
    syncRelayState('step9-initial');
    const initialConvergence = await waitForFleetConvergence('step9-initial');
    const publicBefore = await validatorAccountAssetsSnapshot('step9-before-public-a651', sensitive.accountAddress, A651_ASSET_ID);
    const baseline = balanceForValidator(publicBefore, 0);
    assertOk(balanceForValidator(publicBefore, 3) === baseline, 'validator-0 and validator-3 public a651 baselines differ');
    assertOk(
      baseline === STEP9_EXPECTED_BASELINE_A651,
      `Step 9 public a651 baseline ${baseline} differs from directive baseline ${STEP9_EXPECTED_BASELINE_A651}`,
    );
    const beforeState = vaultSnapshot('step9-before-egress', sensitive.accountAddress);

    const context = await browser.newContext({
      ignoreHTTPSErrors: true,
      viewport: { width: 1440, height: 1100 },
      permissions: ['local-network-access'],
    });
    await context.grantPermissions(['local-network-access'], { origin: new URL(APP_URL).origin });
    const page = await context.newPage();
    page.setDefaultTimeout(60_000);
    page.on('console', (message) => {
      const text = message.text();
      if (/orchard|shielded|egress|navswap|wallet/i.test(text)) {
        console.log(`[browser:${message.type()}] ${text}`);
      }
    });
    await installCapture(page);
    wallet = await importWallet(page, sensitive);
    await selectShieldedRoute(page);
    const input = await selectShieldedDirection(page, { from: 'a651', to: 'a652' });
    await input.fill(SWAP_AMOUNT);
    await page.waitForSelector('text=/Private note exit/i', { timeout: 60_000 });
    await page.waitForFunction(
      commitment => document.body.innerText.includes(`${commitment.slice(0, 8)}…${commitment.slice(-8)}`),
      STEP9_EGRESS_COMMITMENT,
      { timeout: 60_000 },
    );
    const noteButton = page.locator('.pfs-note-list button').filter({ hasText: compactHash(STEP9_EGRESS_COMMITMENT, 8) }).first();
    await noteButton.click({ timeout: 30_000 });
    await page.screenshot({ path: join(OUT_DIR, 'step9-02-disclosure-before-ack.png'), fullPage: true });
    await page.locator('label.pfs-disclosure input[type="checkbox"]').check({ timeout: 30_000 });
    await page.screenshot({ path: join(OUT_DIR, 'step9-03-disclosure-acknowledged.png'), fullPage: true });
    const capturedStart = await page.evaluate(() => window.__orchardSwapE2e?.entries?.length || 0);
    await clickEnabledButton(page, /Exit selected note to public/i, 300_000);
    await page.waitForFunction(
      () => /Public exit certified|Public exit blocked|Swap needs attention/i.test(document.body.innerText || ''),
      null,
      { timeout: SUBMIT_WAIT_MS },
    );
    await page.waitForTimeout(1500);
    const finalText = await page.locator('body').innerText();
    await page.screenshot({ path: join(OUT_DIR, 'step9-04-after-egress.png'), fullPage: true });
    assertOk(!/Public exit blocked|Swap needs attention/i.test(finalText), `Step 9 egress failed in UX:\n${finalText}`);

    const captured = await page.evaluate(start => (
      window.__orchardSwapE2e?.entries || []
    ).slice(start), capturedStart);
    writeJson(join(OUT_DIR, 'step9-browser-capture.json'), captured);
    const localEntry = latestCaptured(captured, '/asset-orchard/private-egress-actions');
    const egressEntry = latestCaptured(captured, '/api/shielded-nav-swap/egress');
    const finalizeEntry = latestCaptured(captured, '/asset-orchard/private-egress-finalize');
    assertOk(localEntry?.body?.ok === true, 'Step 9 missing successful local private egress action');
    assertOk(egressEntry?.body?.ok === true, 'Step 9 missing successful certified egress');
    assertOk(finalizeEntry?.body?.ok === true, 'Step 9 missing successful private egress finalize');
    const egressFile = localEntry.body.egress || JSON.parse(egressEntry.request.egress_json);
    const wirePrivacy = writeEgressPrivacyEvidence(egressFile, STEP9_HOLD_COMMITMENT);

    const afterState = vaultSnapshot('step9-after-egress', sensitive.accountAddress);
    const egressAfterRecord = publicVaultRecord(join(LOCAL_VAULT_DIR, `${STEP9_EGRESS_COMMITMENT}.json`));
    const holdAfterRecord = STEP9_HOLD_COMMITMENT
      ? publicVaultRecord(join(LOCAL_VAULT_DIR, `${STEP9_HOLD_COMMITMENT}.json`))
      : null;
    assertOk(egressAfterRecord.state === 'egressed', `Step 9 egress note state ${egressAfterRecord.state}, expected egressed`);
    if (holdAfterRecord) {
      assertOk(holdAfterRecord.state === 'spendable', `Step 9 hold note state ${holdAfterRecord.state}, expected spendable`);
    }
    const convergence = await waitForFleetConvergence('step9-after-egress');
    const syncAfter = syncRelayState('step9-after-egress');
    const publicAfter = await validatorAccountAssetsSnapshot('step9-after-public-a651', sensitive.accountAddress, A651_ASSET_ID);
    const expectedAfter = baseline + SWAP_AMOUNT_ATOMS;
    const v0After = balanceForValidator(publicAfter, 0);
    const v3After = balanceForValidator(publicAfter, 3);
    assertOk(v0After === expectedAfter, `validator-0 public a651 ${v0After}, expected ${expectedAfter}`);
    assertOk(v3After === expectedAfter, `validator-3 public a651 ${v3After}, expected ${expectedAfter}`);
    assertOk(!Array.isArray(convergence.repair_invocations) || convergence.repair_invocations.length === 0, 'Step 9 recorded a validator repair invocation');

    await context.close();
    const report = {
      schema: evidenceSchema('live-egress-v1'),
      captured_at: new Date().toISOString(),
      step_mode: STEP_MODE,
      app_url: APP_URL,
      rpc_url: RPC_URL,
      route: SHIELDED_NAVSWAP_ROUTE,
      local_service: LOCAL_SERVICE,
      local_vault_dir: LOCAL_VAULT_DIR,
      wallet_address: wallet.accountAddress,
      asset_symbol: 'a651',
      asset_id: A651_ASSET_ID,
      amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
      task0_adjusted_baseline_atoms: baseline.toString(),
      directive_baseline_atoms: STEP9_EXPECTED_BASELINE_A651.toString(),
      expected_after_atoms: expectedAfter.toString(),
      egress_commitment: STEP9_EGRESS_COMMITMENT,
      hold_private_commitment: STEP9_HOLD_COMMITMENT,
      egress_note_before: egressBeforeRecord,
      hold_note_before: holdBeforeRecord,
      egress_note_after: egressAfterRecord,
      hold_note_after: holdAfterRecord,
      public_before_file: 'step9-before-public-a651-validator-account-assets.json',
      public_after_file: 'step9-after-public-a651-validator-account-assets.json',
      v0_before_atoms: baseline.toString(),
      v3_before_atoms: balanceForValidator(publicBefore, 3).toString(),
      v0_after_atoms: v0After.toString(),
      v3_after_atoms: v3After.toString(),
      relay: {
        ok: egressEntry.body.ok,
        status: egressEntry.body.status,
        message: egressEntry.body.message,
        policy_id: egressEntry.body.policy_id,
        disclosure_hash: egressEntry.body.disclosure_hash,
        note_commitment: egressEntry.body.note_commitment,
        bridge_out_enabled: egressEntry.body.bridge_out_enabled,
        public_exit_receipt_required_for_bridge_out: egressEntry.body.public_exit_receipt_required_for_bridge_out,
        artifact_dir: egressEntry.body.artifact_dir || null,
        receipts: egressEntry.body.receipts || [],
        report_round_ok: egressEntry.body.report?.round_ok ?? egressEntry.body.report?.transport?.round_ok ?? null,
      },
      finalize: {
        ok: finalizeEntry.body.ok,
        egress_id: finalizeEntry.body.egress_id,
        accepted: finalizeEntry.body.accepted,
        input: finalizeEntry.body.input,
      },
      wire_privacy: wirePrivacy,
      note_state_files: {
        before: 'step9-before-egress-note-state.json',
        after: 'step9-after-egress-note-state.json',
      },
      relay_state_sync: {
        after_height: syncAfter.local_after?.block_height || null,
        after_root: syncAfter.local_after?.state_root || null,
      },
      fleet_convergence: {
        initial_height: initialConvergence.height,
        height: convergence.height,
        root: convergence.root,
        count: convergence.count,
        repair_invocations: convergence.repair_invocations || [],
        zero_repair_required: ZERO_REPAIR,
      },
      screenshots: [
        'step9-02-disclosure-before-ack.png',
        'step9-03-disclosure-acknowledged.png',
        'step9-04-after-egress.png',
      ],
      assertions: {
        public_a651_delta_atoms: (v0After - baseline).toString(),
        v0_v3_after_match: v0After === v3After,
        egress_note_marked_egressed: egressAfterRecord.state === 'egressed',
        hold_note_remains_spendable: holdAfterRecord ? holdAfterRecord.state === 'spendable' : null,
        hold_note_check_skipped: holdAfterRecord === null,
        hold_commitment_absent_from_egress_payload: wirePrivacy.ok,
        zero_repair: !Array.isArray(convergence.repair_invocations) || convergence.repair_invocations.length === 0,
      },
      sensitive_material_file: SENSITIVE_FILE,
      sensitive_material_note: 'Wallet seed/passphrase remain in this local /tmp file and are not copied into docs/evidence.',
    };
    writeJson(join(OUT_DIR, 'report.json'), report);
    console.log(JSON.stringify({
      ok: true,
      out_dir: OUT_DIR,
      wallet_address: report.wallet_address,
      egress_id: report.finalize.egress_id,
      public_delta_atoms: report.assertions.public_a651_delta_atoms,
      v0_after_atoms: report.v0_after_atoms,
      v3_after_atoms: report.v3_after_atoms,
    }, null, 2));
  } finally {
    await browser.close();
    rpc.close();
  }
}

async function main() {
  if (STEP10_PREP) {
    writeStep10PrepPacket();
    return;
  }
  if (STEP10_PAIR) {
    await runStep10Pair();
    return;
  }
  if (STEP10_RESCAN) {
    await runStep10RescanOnly();
    return;
  }
  if (STEP9_EGRESS) {
    await runStep9Egress();
    return;
  }
  if (ALLOW_SINGLE_RUN) {
    assertOk(Number.isInteger(RUNS) && RUNS >= 1, `${EVIDENCE_STEP} single-run measurement requires at least one live run`);
  } else {
    assertOk(Number.isInteger(RUNS) && RUNS === 2, `${EVIDENCE_STEP} directive requires exactly two live runs`);
  }
  assertOk(existsSync(SENSITIVE_FILE), `missing sensitive wallet file: ${SENSITIVE_FILE}`);
  const sensitive = JSON.parse(readFileSync(SENSITIVE_FILE, 'utf8'));
  assertOk(/^[0-9a-f]{64}$/i.test(sensitive.seed), 'sensitive wallet seed is malformed');
  assertOk(/^pf[a-f0-9]{40}$/i.test(sensitive.accountAddress), 'sensitive wallet address is malformed');

  const rpc = new RpcClient(RPC_URL);
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
  });
  const poolNotes = [];
  const walletNotes = [];
  const runs = [];
  let wallet = null;
  let walletPftFunding = null;
  let poolPftFunding = null;
  try {
    assertOk(SWAP_PLAN.length === RUNS, `swap plan length ${SWAP_PLAN.length} does not match run count ${RUNS}`);
    if (USE_EXISTING_WALLET_NOTES) {
      assertOk(
        EXISTING_WALLET_INPUT_COMMITMENTS.length === RUNS,
        `existing wallet note commitment count ${EXISTING_WALLET_INPUT_COMMITMENTS.length} does not match run count ${RUNS}`,
      );
    }
    if (EXISTING_POOL_NOTE_COMMITMENTS.length > 0) {
      assertOk(
        EXISTING_POOL_NOTE_COMMITMENTS.length === RUNS,
        `existing pool note commitment count ${EXISTING_POOL_NOTE_COMMITMENTS.length} does not match run count ${RUNS}`,
      );
    }
    const backupJson = walletBackupFromSensitive(sensitive);
    syncRelayState('initial');
    await waitForFleetConvergence('initial');
    walletPftFunding = await ensurePftBalance(rpc, sensitive.accountAddress, WALLET_PFT_FUND_ATOMS);
    writeJson(join(OUT_DIR, 'wallet-pft-funding.json'), walletPftFunding);
    poolPftFunding = await ensurePftBalance(rpc, POOL_OPERATOR, POOL_OPERATOR_PFT_FUND_ATOMS);
    writeJson(join(OUT_DIR, 'pool-operator-pft-funding.json'), poolPftFunding);
    for (let runIndex = 1; runIndex <= RUNS; runIndex += 1) {
      if (USE_EXISTING_WALLET_NOTES) {
        walletNotes.push(existingWalletNote(
          sensitive.accountAddress,
          runIndex,
          SWAP_PLAN[runIndex - 1],
          EXISTING_WALLET_INPUT_COMMITMENTS[runIndex - 1],
        ));
      } else {
        walletNotes.push(await createWalletNote(rpc, sensitive.accountAddress, backupJson, runIndex, SWAP_PLAN[runIndex - 1]));
        await waitForFleetConvergence(`wallet-note-${runIndex}-after-ingress`);
        syncRelayState(`wallet-note-${runIndex}-after-ingress`);
      }
      if (EXISTING_POOL_NOTE_COMMITMENTS.length > 0) {
        poolNotes.push(existingPoolNote(
          runIndex,
          SWAP_PLAN[runIndex - 1],
          EXISTING_POOL_NOTE_COMMITMENTS[runIndex - 1],
        ));
      } else {
        poolNotes.push(await createPoolNote(rpc, runIndex, SWAP_PLAN[runIndex - 1]));
        await waitForFleetConvergence(`pool-note-${runIndex}-after-ingress`);
        syncRelayState(`pool-note-${runIndex}-after-ingress`);
      }
    }

    const context = await browser.newContext({
      ignoreHTTPSErrors: true,
      viewport: { width: 1440, height: 1100 },
      permissions: ['local-network-access'],
    });
    await context.grantPermissions(['local-network-access'], { origin: new URL(APP_URL).origin });
    const page = await context.newPage();
    page.setDefaultTimeout(60_000);
    page.on('console', (message) => {
      const text = message.text();
      if (/orchard|shielded|swap|navswap|wallet/i.test(text)) {
        console.log(`[browser:${message.type()}] ${text}`);
      }
    });
    await installCapture(page);
    wallet = await importWallet(page, sensitive);
    for (let runIndex = 1; runIndex <= RUNS; runIndex += 1) {
      runs.push(await runSwap(
        page,
        wallet,
        poolNotes[runIndex - 1],
        runIndex,
        sensitive,
        SWAP_PLAN[runIndex - 1],
        walletNotes[runIndex - 1],
      ));
    }
    await context.close();
  } finally {
    await browser.close();
    rpc.close();
  }

  const report = {
    schema: evidenceSchema('live-e2e-v1'),
    captured_at: new Date().toISOString(),
    step_mode: STEP_MODE,
    app_url: APP_URL,
    rpc_url: RPC_URL,
    route: SHIELDED_NAVSWAP_ROUTE,
    local_service: LOCAL_SERVICE,
    local_vault_dir: LOCAL_VAULT_DIR,
    wallet_address: wallet?.accountAddress || sensitive.accountAddress,
    pool_operator: POOL_OPERATOR,
    pool_operator_pft_funding: poolPftFunding,
    swap_plan: SWAP_PLAN,
    from_asset_id: ASSET_IDS[SWAP_PLAN[0]?.from],
    to_asset_id: ASSET_IDS[SWAP_PLAN[0]?.to],
    amount_atoms: SWAP_AMOUNT_ATOMS.toString(),
    wallet_notes: walletNotes,
    pool_notes: poolNotes,
    wallet_pft_funding: walletPftFunding,
    runs,
    zero_repair_required: ZERO_REPAIR,
    repair_invocations: runs.flatMap(run => run.fleet_convergence?.repair_invocations || []),
    sensitive_material_file: SENSITIVE_FILE,
    sensitive_material_note: 'Wallet seed/passphrase remain in this local /tmp file and are not copied into docs/evidence.',
    redaction: {
      swap_action_json: 'captured and scanned because it is proxy-bound public action data',
      wallet_note_openings: 'not sent to proxy and not written under docs/evidence',
      spend_material: 'kept behind loopback/local service boundary',
    },
  };
  writeJson(join(OUT_DIR, 'report.json'), report);
  console.log(JSON.stringify({
    ok: true,
    out_dir: OUT_DIR,
    wallet_address: report.wallet_address,
    run_count: runs.length,
    swap_ids: runs.map(run => run.local_action.swap_id),
    pool_commitments: poolNotes.map(note => note.output_commitment),
  }, null, 2));
}

main().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
