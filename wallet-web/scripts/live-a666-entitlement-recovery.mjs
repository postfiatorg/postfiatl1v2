import { mkdir, readFile, writeFile } from 'node:fs/promises';

import { chromium } from 'playwright';

import {
  buildA666IssueExportDraft,
  finalizeA666IssueExportOperations,
} from '../src/lib/a666-primary-route.js';
import { TxBuilder } from '../src/lib/tx-builder.js';
import * as walletWasm from '../src/wasm/postfiat_wallet_wasm.js';
const walletUrl = process.env.WALLET_WEB_URL || 'https://127.0.0.1:5173';
const ethereumRpc = process.env.ETHEREUM_RPC_URL || 'https://ethereum-rpc.publicnode.com';
const proxyUrl = process.env.WALLET_PROXY_URL || 'http://127.0.0.1:8080';
const proxyTokenFile = process.env.E2E_PROXY_TOKEN_FILE
  || '/home/postfiat/.local/state/postfiat-a666-wallet/proxy-tokens.json';
const backupFile = process.env.E2E_PFTL_BACKUP_FILE;
const evidenceDir = process.env.E2E_EVIDENCE_DIR;
const reservationId = String(process.env.E2E_RESERVATION_ID || '').toLowerCase();
const ethereumRecipient = String(process.env.E2E_ETH_RECIPIENT || '').toLowerCase();

const routeId = 'pftl-a666-ethereum-wA666-usdc-v1';
const routeConfigDigest = '12ed00ca87e29554ce4b978da1710fffc0830767e84e62f08df257f727db953efdd89bcf6ea99f5634d6e5ea8aca2933';
const wrappedA666 = '0xee4c92edb03efdd9b519339edc19ad70c69a9be5';
const expectedPftlAddress = 'pfab9b9228942e5c529633a13aa271d5297bec6353';
const amountAtoms = 1_000_000n;
const settlementAtoms = 905_664n;
const entitlementExpiryHeight = 740;

if (!backupFile || !evidenceDir || !/^[0-9a-f]{96}$/.test(reservationId)
  || !/^0x[0-9a-f]{40}$/.test(ethereumRecipient)) {
  throw new Error(
    'E2E_PFTL_BACKUP_FILE, E2E_EVIDENCE_DIR, E2E_RESERVATION_ID, and E2E_ETH_RECIPIENT are required',
  );
}

await mkdir(evidenceDir, { recursive: true, mode: 0o700 });
const backup = JSON.parse(await readFile(backupFile, 'utf8'));
if (!/^[0-9a-f]{64}$/.test(String(backup.master_seed_hex || ''))) {
  throw new Error('PFTL wallet backup is missing a valid master seed');
}
const backupJson = JSON.stringify(backup);
const tokenConfig = JSON.parse(await readFile(proxyTokenFile, 'utf8'));
const proxyAuthToken = String(tokenConfig['local-demo'] || '');
if (proxyAuthToken.length < 32) throw new Error('local-demo proxy token is unavailable');

const wasmBytes = await readFile(new URL('../src/wasm/postfiat_wallet_wasm_bg.wasm', import.meta.url));
walletWasm.initSync({ module: wasmBytes });

let ethereumRpcId = 0;
async function ethereumRequest(method, params = []) {
  const response = await fetch(ethereumRpc, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: `a666-recovery-${++ethereumRpcId}`, method, params }),
    signal: AbortSignal.timeout(30_000),
  });
  const payload = await response.json();
  if (payload.error) throw new Error(payload.error.message || `Ethereum RPC ${method} failed`);
  return payload.result;
}

async function wrappedBalance() {
  const data = `0x70a08231${ethereumRecipient.slice(2).padStart(64, '0')}`;
  return BigInt(await ethereumRequest('eth_call', [{ to: wrappedA666, data }, 'latest']));
}

async function relayRequest(path, options = {}) {
  const response = await fetch(`${proxyUrl}${path}`, {
    cache: 'no-store',
    ...options,
    headers: {
      Accept: 'application/json',
      Authorization: `Bearer ${proxyAuthToken}`,
      Origin: new URL(walletUrl).origin,
      ...(options.body ? { 'Content-Type': 'application/json' } : {}),
      ...(options.headers || {}),
    },
    signal: AbortSignal.timeout(90_000),
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok || payload.ok !== true) {
    throw new Error(payload.message || `wallet proxy request failed with HTTP ${response.status}`);
  }
  return payload;
}

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ ignoreHTTPSErrors: true });
const page = await context.newPage();
page.setDefaultTimeout(120_000);
await page.goto(walletUrl, { waitUntil: 'domcontentloaded' });

async function rpcCall(method, params = {}, timeoutMs = 180_000) {
  return page.evaluate(async ({ method, params, token, timeoutMs }) => {
    const socketUrl = `${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/rpc`;
    const ws = new WebSocket(socketUrl);
    const id = crypto.randomUUID();
    try {
      await new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('wallet RPC connection timed out')), 10_000);
        ws.addEventListener('open', () => { clearTimeout(timeout); resolve(); }, { once: true });
        ws.addEventListener('error', () => { clearTimeout(timeout); reject(new Error('wallet RPC connection failed')); }, { once: true });
      });
      return await new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error(`wallet RPC ${method} timed out`)), timeoutMs);
        ws.addEventListener('message', (event) => {
          const response = JSON.parse(event.data);
          if (response.id !== id) return;
          clearTimeout(timeout);
          resolve(response);
        });
        ws.send(JSON.stringify({
          version: 'postfiat-local-rpc-v1', id, method, params, proxy_auth_token: token,
        }));
      });
    } finally {
      ws.close();
    }
  }, { method, params, token: proxyAuthToken, timeoutMs });
}

const rpc = {
  status: () => rpcCall('status'),
  navcoinBridgeSupplyStatus: (id) => rpcCall('navcoin_bridge_supply_status', { route_id: id }),
  assetFeeQuote: (source, operationJson) => rpcCall('asset_fee_quote', {
    source, operation_json: operationJson,
  }),
  submitSignedAssetTransactionFinality: (signedAssetJson) => rpcCall(
    'mempool_submit_signed_asset_transaction_finality',
    { signed_asset_transaction_json: signedAssetJson },
    240_000,
  ),
  tx: (txId) => rpcCall('tx', { tx_id: txId }),
  receipts: (txId) => rpcCall('receipts', { tx_id: txId }),
};

const startedAt = Date.now();
const balanceBefore = await wrappedBalance();
let publicPlan = null;
let relayJobId = '';
try {
  const [chainResponse, routeResponse] = await Promise.all([
    rpc.status(), rpc.navcoinBridgeSupplyStatus(routeId),
  ]);
  if (chainResponse.ok !== true || routeResponse.ok !== true) {
    throw new Error('PFTL route state is unavailable');
  }
  const chain = chainResponse.result;
  const route = routeResponse.result;
  if (route.route_id !== routeId || route.route_config_digest !== routeConfigDigest
    || String(route.wrapped_navcoin_token || '').toLowerCase() !== wrappedA666
    || route.invariant_holds !== true || route.paused === true) {
    throw new Error('the governed A666 route does not match the recovery pins');
  }
  if (Number(chain.block_height) >= entitlementExpiryHeight) {
    throw new Error('the stranded export entitlement has expired');
  }
  if (Number(route.export_entitlement_count) !== 1
    || BigInt(route.export_entitlement_atoms) !== amountAtoms
    || Number(route.active_reservation_count) !== 0) {
    throw new Error('chain export-entitlement totals do not match the single stranded entitlement');
  }
  const ownerBalance = (route.native_spendable_balances || [])
    .find(row => row.wallet === expectedPftlAddress);
  if (!ownerBalance || BigInt(ownerBalance.amount_atoms) < amountAtoms) {
    throw new Error('the recovery wallet does not hold the stranded native A666');
  }

  const txBuilder = new TxBuilder(rpc, async () => walletWasm);
  const draft = buildA666IssueExportDraft({
    walletAddress: expectedPftlAddress,
    ethereumRecipient,
    supplyStatus: route,
    chainHeight: chain.block_height,
    amountAtoms,
    settlementAtoms,
    reservationId,
  });
  const preparedPacket = await txBuilder.preparePftlUniswapMintPacket(
    draft.policyHash,
    draft.mintPacket,
  );
  const prepared = finalizeA666IssueExportOperations(draft, preparedPacket);
  if (prepared.export.reservation_id !== reservationId
    || BigInt(prepared.export.amount_atoms) !== amountAtoms
    || BigInt(prepared.export.settlement_value_atoms) !== settlementAtoms
    || prepared.export.ethereum_recipient !== ethereumRecipient) {
    throw new Error('prepared export changed a bounded recovery field');
  }

  publicPlan = {
    schema: 'postfiat.wallet.live_a666_entitlement_recovery_plan.v1',
    route_id: routeId,
    route_config_digest: routeConfigDigest,
    reservation_id: reservationId,
    ethereum_recipient: ethereumRecipient,
    amount_atoms: amountAtoms.toString(),
    settlement_value_atoms: settlementAtoms.toString(),
    source_height: Number(chain.block_height),
    entitlement_expiry_height: entitlementExpiryHeight,
    packet_hash: prepared.packetHash,
    packet_digest: prepared.packetDigest,
    deadline_seconds: prepared.destinationDeadlineSeconds,
    export_operation: prepared.export,
  };
  await writeFile(`${evidenceDir}/prepared-public.json`, `${JSON.stringify(publicPlan, null, 2)}\n`, {
    mode: 0o600,
  });

  const created = await relayRequest('/api/a666/export-jobs', {
    method: 'POST',
    body: JSON.stringify({
      route_id: routeId,
      route_config_digest: routeConfigDigest,
      packet_hash: prepared.packetHash,
      packet_digest: prepared.packetDigest,
      ethereum_recipient: ethereumRecipient,
      amount_atoms: amountAtoms.toString(),
      deadline_seconds: prepared.destinationDeadlineSeconds,
    }),
  });
  relayJobId = created.job_id;
  if (!/^0x[0-9a-f]{64}$/.test(relayJobId)) throw new Error('relay returned an invalid job id');

  const exportResult = await txBuilder.sendAssetTransfer(
    backupJson,
    expectedPftlAddress,
    { operation: prepared.export },
  );

  const relayDeadline = Date.now() + 2_700_000;
  let relay = null;
  while (Date.now() < relayDeadline) {
    relay = await relayRequest(`/api/a666/export-jobs/${encodeURIComponent(relayJobId)}`);
    if (relay.status === 'accepted') break;
    if (relay.status === 'failed') throw new Error(relay.message || 'A666 export relay failed');
    await new Promise(resolve => setTimeout(resolve, 5_000));
  }
  if (relay?.status !== 'accepted') throw new Error('A666 export relay did not finalize before timeout');

  const balanceAfter = await wrappedBalance();
  if (balanceAfter - balanceBefore !== amountAtoms) {
    throw new Error(`wA666 balance delta was ${balanceAfter - balanceBefore}, expected ${amountAtoms}`);
  }
  const routeAfterResponse = await rpc.navcoinBridgeSupplyStatus(routeId);
  const routeAfter = routeAfterResponse.result || {};
  if (routeAfterResponse.ok !== true || Number(routeAfter.export_entitlement_count) !== 0
    || BigInt(routeAfter.export_entitlement_atoms || 0) !== 0n
    || routeAfter.invariant_holds !== true) {
    throw new Error('PFTL export entitlement was not consumed cleanly');
  }

  const result = {
    ok: true,
    schema: 'postfiat.wallet.live_a666_entitlement_recovery.v1',
    reservation_id: reservationId,
    packet_hash: prepared.packetHash,
    packet_digest: prepared.packetDigest,
    pftl_export_tx_id: exportResult.txId,
    relay_job_id: relayJobId,
    relay_status: relay.status,
    ethereum_tx_hash: relay.ethereum_tx_hash || null,
    wrapped_balance_before_atoms: balanceBefore.toString(),
    wrapped_balance_after_atoms: balanceAfter.toString(),
    elapsed_ms: Date.now() - startedAt,
  };
  await writeFile(`${evidenceDir}/result.json`, `${JSON.stringify(result, null, 2)}\n`, { mode: 0o600 });
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
} catch (error) {
  const failure = {
    ok: false,
    schema: 'postfiat.wallet.live_a666_entitlement_recovery_failure.v1',
    error: error.message,
    reservation_id: reservationId,
    packet_hash: publicPlan?.packet_hash || null,
    relay_job_id: relayJobId || null,
    wrapped_balance_before_atoms: balanceBefore.toString(),
    wrapped_balance_now_atoms: (await wrappedBalance().catch(() => 0n)).toString(),
    elapsed_ms: Date.now() - startedAt,
  };
  await writeFile(`${evidenceDir}/failure.json`, `${JSON.stringify(failure, null, 2)}\n`, { mode: 0o600 });
  throw error;
} finally {
  await browser.close();
}
