import { assertNoCustodyMaterial } from './custody-boundary.js';

const TERMINAL = new Set(['accepted', 'failed']);

async function relayJson(url, options = {}) {
  const response = await fetch(url, { cache: 'no-store', ...options });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok || payload.ok !== true) {
    const error = new Error(payload.message || `A666 export relay failed with HTTP ${response.status}`);
    error.payload = payload;
    error.httpStatus = response.status;
    throw error;
  }
  return payload;
}

function transient(error) {
  return !error?.httpStatus || error.httpStatus === 408 || error.httpStatus === 429
    || error.httpStatus >= 500;
}

async function relayJsonWithRetry(url, options = {}, attempts = 4) {
  let lastError;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      return await relayJson(url, options);
    } catch (error) {
      lastError = error;
      if (!transient(error) || attempt === attempts) throw error;
      await new Promise(resolve => setTimeout(resolve, Math.min(5_000, 500 * (2 ** (attempt - 1)))));
    }
  }
  throw lastError;
}

export async function loadA666ExportReadiness() {
  return relayJson('/api/a666/export-readiness');
}

export async function waitForA666ExportJob(
  jobId,
  { pollIntervalMs = 5_000, timeoutMs = 4 * 60 * 60 * 1000, onStatus = null } = {},
) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    let result;
    try {
      result = await relayJsonWithRetry(`/api/a666/export-jobs/${encodeURIComponent(jobId)}`);
    } catch (error) {
      if (!transient(error)) throw error;
      await new Promise(resolve => setTimeout(resolve, pollIntervalMs));
      continue;
    }
    onStatus?.(result);
    if (TERMINAL.has(result.status)) {
      if (result.status === 'accepted') return result;
      const error = new Error(result.message || 'The trustless A666 export failed a safety gate.');
      error.payload = result;
      throw error;
    }
    await new Promise(resolve => setTimeout(resolve, pollIntervalMs));
  }
  const error = new Error('The export remains safely queued. It will continue without this browser.');
  error.code = 'a666_export_job_poll_timeout';
  throw error;
}

export async function relayA666Export({
  routeId,
  routeConfigDigest,
  packetHash,
  packetDigest,
  ethereumRecipient,
  amountAtoms,
  deadlineSeconds,
  proxyAuthToken,
  onStatus = null,
} = {}) {
  const created = await createA666ExportJob({
    routeId, routeConfigDigest, packetHash, packetDigest, ethereumRecipient,
    amountAtoms, deadlineSeconds, proxyAuthToken,
  });
  onStatus?.(created);
  return waitForA666ExportJob(created.job_id, { onStatus });
}

export async function createA666ExportJob({
  routeId,
  routeConfigDigest,
  packetHash,
  packetDigest,
  ethereumRecipient,
  amountAtoms,
  deadlineSeconds,
  proxyAuthToken,
} = {}) {
  const body = {
    route_id: routeId,
    route_config_digest: routeConfigDigest,
    packet_hash: packetHash,
    packet_digest: packetDigest,
    ethereum_recipient: ethereumRecipient,
    amount_atoms: String(amountAtoms || ''),
    deadline_seconds: Number(deadlineSeconds),
  };
  assertNoCustodyMaterial(body, 'A666 export relay request');
  const created = await relayJsonWithRetry('/api/a666/export-jobs', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(proxyAuthToken ? { Authorization: `Bearer ${proxyAuthToken}` } : {}),
    },
    body: JSON.stringify(body),
  });
  return created;
}
