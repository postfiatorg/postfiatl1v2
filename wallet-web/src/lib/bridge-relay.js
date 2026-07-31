import { assertNoCustodyMaterial } from './custody-boundary.js';

const BRIDGE_ROUTE_ID = 'ethereum-mainnet-usdc-v1';
const BRIDGE_SOURCE_CHAIN_ID = 1;
const TERMINAL_JOB_STATUSES = new Set(['accepted', 'failed']);

async function bridgeJson(url, options = {}) {
  const response = await fetch(url, options);
  const payload = await response.json().catch(() => ({}));
  if (!response.ok || payload.ok !== true) {
    const error = new Error(payload.message || `Bridge service failed with HTTP ${response.status}`);
    error.payload = payload;
    throw error;
  }
  return payload;
}

export async function loadBridgeReadiness(routeId = BRIDGE_ROUTE_ID) {
  return bridgeJson(`/api/bridge/readiness?route=${encodeURIComponent(routeId)}`);
}

export function assertBridgeReadinessMatchesRoute(readiness, route) {
  const profile = route?.profile || {};
  if (
    readiness?.ready !== true
    || readiness.route_id !== profile.route_id
    || Number(readiness.source_chain_id) !== Number(profile.source_chain_id)
    || readiness.source_proof_kind !== 'sp1-ethereum-finality-v1'
    || readiness.route_profile_hash !== route.profileHash
    || readiness.asset_id !== profile.asset_id
    || readiness.vault_address !== route.vaultAddress
    || readiness.vault_runtime_code_hash !== route.vaultRuntimeCodeHash
    || readiness.token_address !== route.tokenAddress
    || readiness.token_runtime_code_hash !== route.tokenRuntimeCodeHash
    || readiness.program_vkey !== profile.verifier_program_vkey
    || readiness.observer_attestor_enabled !== false
    || readiness.prover_authenticated !== true
    || readiness.prover_healthy !== true
  ) {
    throw new Error('The proof relay is not ready for the active governed PFTL route.');
  }
  return readiness;
}

export async function waitForBridgeReadiness(
  route,
  { attempts = 45, retryMs = 2000 } = {},
) {
  let lastError = new Error('The proof relay is not ready for the active governed PFTL route.');
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    let readiness;
    try {
      readiness = await loadBridgeReadiness(route?.profile?.route_id);
    } catch (error) {
      lastError = error;
    }
    if (readiness?.ready === true) {
      // A ready response with the wrong identity is an integrity failure, not
      // transient availability. Fail immediately instead of retrying it.
      return assertBridgeReadinessMatchesRoute(readiness, route);
    }
    if (readiness) {
      lastError = new Error(
        readiness.message || 'The proof relay is temporarily warming. Retry shortly.',
      );
    }
    if (attempt < attempts - 1) {
      await new Promise((resolve) => setTimeout(resolve, retryMs));
    }
  }
  throw lastError;
}

export async function waitForBridgeJob(
  jobId,
  { pollIntervalMs = 2000, timeoutMs = 30 * 60 * 1000, onStatus = null } = {},
) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const result = await bridgeJson(`/api/bridge/jobs/${encodeURIComponent(jobId)}`);
    onStatus?.(result);
    if (TERMINAL_JOB_STATUSES.has(result.status)) {
      if (result.status === 'accepted' && result.receipt_code === 'ACCEPTED') return result;
      const error = new Error(result.message || 'The proof-backed PFTL claim failed.');
      error.payload = result;
      throw error;
    }
    await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
  }
  const error = new Error('The bridge job is still running. Resume it with the Ethereum transaction hash.');
  error.code = 'bridge_job_poll_timeout';
  throw error;
}

export async function relayVaultDeposit({
  depositTxHash,
  depositId = '',
  pftlRecipient = '',
  depositor = '',
  amountAtoms = '',
  idempotencyKey = '',
  routeProfileHash = '',
  routeEpoch = 0,
  routeBinding = '',
  routeId = BRIDGE_ROUTE_ID,
  sourceChainId = BRIDGE_SOURCE_CHAIN_ID,
  proxyAuthToken = '',
  onStatus = null,
} = {}) {
  const body = {
    route_id: routeId,
    source_chain_id: sourceChainId,
    deposit_tx_hash: depositTxHash,
    deposit_id: depositId,
    pftl_recipient: pftlRecipient,
    depositor,
    amount_atoms: amountAtoms ? String(amountAtoms) : '',
    idempotency_key: idempotencyKey,
    route_profile_hash: routeProfileHash,
    route_epoch: routeEpoch,
    route_binding: routeBinding,
  };
  assertNoCustodyMaterial(body, 'wallet bridge relay request');
  const created = await bridgeJson('/api/bridge/jobs', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(idempotencyKey ? { 'Idempotency-Key': idempotencyKey } : {}),
      ...(proxyAuthToken ? { Authorization: `Bearer ${proxyAuthToken}` } : {}),
    },
    body: JSON.stringify(body),
  });
  onStatus?.(created);
  return waitForBridgeJob(created.job_id, {
    onStatus,
  });
}
