import { PFUSDC_ASSET_ID } from './utils.js';

const HASH48_RE = /^[0-9a-f]{96}$/;
const PFTL_RE = /^pf[0-9a-f]{40}$/;
const EVM_RE = /^0x[0-9a-f]{40}$/;

export function pfusdcWithdrawalCapacity({ status, route }) {
  const profile = route?.profile;
  const sourceDomain = `erc20_bridge_vault:${profile?.source_chain_id}:${String(profile?.vault_address || '').toLowerCase()}:${String(profile?.token_address || '').toLowerCase()}`;
  const buckets = (status?.buckets || []).filter(bucket => (
    bucket?.source_domain === sourceDomain
    && bucket?.policy_hash === route?.profileHash
    && bucket?.status === 'active'
  ));
  if (buckets.length !== 1 || !HASH48_RE.test(String(buckets[0]?.bucket_id || ''))) {
    throw new Error(buckets.length === 0
      ? 'No active Ethereum reserve bucket can fund this withdrawal.'
      : 'The Ethereum reserve route is ambiguous; withdrawal is paused safely.');
  }
  if (status?.source_series_enforced === true
    && (!HASH48_RE.test(String(buckets[0]?.source_series_id || ''))
      || String(buckets[0].source_series_id).toLowerCase() === PFUSDC_ASSET_ID)) {
    throw new Error('The active reserve has no valid source-series identity; withdrawal is paused safely.');
  }
  const amountAtoms = BigInt(buckets[0]?.outstanding_vault_bridge_atoms || 0);
  if (amountAtoms <= 0n) throw new Error('The active Ethereum reserve bucket is empty.');
  return Object.freeze({ bucket: buckets[0], amountAtoms });
}

export function preparePfusdcWithdrawal({ status, route, owner, ethereumRecipient, amountAtoms }) {
  const amount = BigInt(amountAtoms || 0);
  const recipient = String(ethereumRecipient || '').toLowerCase();
  if (!PFTL_RE.test(String(owner || '')) || !EVM_RE.test(recipient) || amount <= 0n) {
    throw new Error('Withdrawal owner, recipient, or amount is invalid.');
  }
  if (amount > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error('Withdrawal amount exceeds this wallet build limit.');
  if (status?.asset_id !== PFUSDC_ASSET_ID || !PFTL_RE.test(String(status?.issuer || ''))
    || !Number.isSafeInteger(Number(status?.finalized_epoch)) || Number(status.finalized_epoch) <= 0
    || !HASH48_RE.test(String(status?.finalized_reserve_packet_hash || ''))) {
    throw new Error('The current pfUSDC reserve state is unavailable.');
  }
  const capacity = pfusdcWithdrawalCapacity({ status, route });
  if (capacity.amountAtoms < amount) {
    throw new Error(`Only ${capacity.amountAtoms} pfUSDC atoms are currently available on the active Ethereum reserve route.`);
  }
  return {
    operation: 'vault_bridge_burn_to_redeem', owner, issuer: status.issuer,
    asset_id: PFUSDC_ASSET_ID, bucket_id: capacity.bucket.bucket_id,
    amount_atoms: Number(amount), epoch: Number(status.finalized_epoch),
    reserve_packet_hash: status.finalized_reserve_packet_hash,
    destination_ref: `evm-erc20:1:${recipient}`,
  };
}

export function recoverablePfusdcWithdrawal({ status, route, owner, jobs = [] }) {
  const normalizedOwner = String(owner || '').toLowerCase();
  if (!PFTL_RE.test(normalizedOwner) || !Array.isArray(jobs)) return null;
  const capacity = pfusdcWithdrawalCapacity({ status, route });
  const knownBurns = new Set(jobs.map(job => String(job?.request?.burn_tx_id || '').toLowerCase()));
  const candidates = (status?.redemptions || []).filter(row => (
    row?.state === 'pending'
    && String(row?.owner || '').toLowerCase() === normalizedOwner
    && String(row?.bucket_id || '').toLowerCase() === String(capacity.bucket.bucket_id).toLowerCase()
    && HASH48_RE.test(String(row?.burn_tx_id || '').toLowerCase())
    && EVM_RE.test(String(row?.withdrawal_recipient || '').toLowerCase())
    && /^[1-9]\d*$/.test(String(row?.amount_atoms || ''))
    && !knownBurns.has(String(row.burn_tx_id).toLowerCase())
  )).sort((left, right) => Number(right.created_at_height || 0) - Number(left.created_at_height || 0));
  if (candidates.length === 0) return null;
  const row = candidates[0];
  return Object.freeze({
    burn_tx_id: String(row.burn_tx_id).toLowerCase(),
    owner: normalizedOwner,
    ethereum_recipient: String(row.withdrawal_recipient).toLowerCase(),
    amount_atoms: String(row.amount_atoms),
    asset_id: PFUSDC_ASSET_ID,
  });
}

async function request(path, token, options = {}) {
  const response = await fetch(path, {
    ...options,
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}`, ...(options.headers || {}) },
  });
  const body = await response.json().catch(() => ({}));
  if (!response.ok || body.ok === false) throw new Error(body.message || 'Withdrawal service is unavailable.');
  return body;
}

export const loadPfusdcWithdrawalReadiness = token => request('/api/bridge/withdrawals/readiness', token);
export const createPfusdcWithdrawalJob = (body, token) => request('/api/bridge/withdrawals', token, { method: 'POST', body: JSON.stringify(body) });
export const retryPfusdcWithdrawalJob = (id, token) => request(`/api/bridge/withdrawals/${encodeURIComponent(id)}/retry`, token, { method: 'POST', body: '{}' });
export const loadPfusdcWithdrawalJob = (id, token) => request(`/api/bridge/withdrawals/${encodeURIComponent(id)}`, token);
export const loadPfusdcWithdrawalJobs = (owner, token, limit = 20) => request(`/api/bridge/withdrawals?owner=${encodeURIComponent(owner)}&limit=${limit}`, token);

export async function waitForPfusdcWithdrawalJob(id, token, { onStatus, signal } = {}) {
  for (;;) {
    if (signal?.aborted) throw new DOMException('Aborted', 'AbortError');
    const job = await loadPfusdcWithdrawalJob(id, token);
    onStatus?.(job);
    if (job.status === 'accepted') return job;
    if (job.status === 'failed') throw new Error(job.message || 'Withdrawal could not be completed.');
    await new Promise((resolve, reject) => {
      const timer = setTimeout(resolve, 4_000);
      signal?.addEventListener('abort', () => { clearTimeout(timer); reject(new DOMException('Aborted', 'AbortError')); }, { once: true });
    });
  }
}
