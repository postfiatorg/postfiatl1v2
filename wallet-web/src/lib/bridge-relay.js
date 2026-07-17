import { assertNoCustodyMaterial } from './custody-boundary.js';

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
} = {}) {
  const body = {
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
  const response = await fetch('/api/bridge/relay', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(idempotencyKey ? { 'Idempotency-Key': idempotencyKey } : {}),
    },
    body: JSON.stringify(body),
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok || payload.ok !== true) {
    const error = new Error(payload.message || `Bridge relay failed with HTTP ${response.status}`);
    error.payload = payload;
    throw error;
  }
  return payload;
}
