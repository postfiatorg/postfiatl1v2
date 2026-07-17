export const FASTPAY_RECOVERY_STORE_SCHEMA = 'postfiat-wallet-fastpay-recovery-store-v1';
export const FASTPAY_RECOVERY_STORE_KEY = 'postfiat.fastpay.recovery.v1';

const MAX_PENDING_RECOVERIES = 16;
const MAX_STORE_BYTES = 1_000_000;

function recoveryLockId(pending) {
  return pending?.signed_order?.signed_order?.order?.recovery?.lock_id
    || pending?.certificate?.order?.recovery?.lock_id
    || '';
}

function recoveryOwner(pending) {
  return pending?.signed_order?.signed_order?.owner_pubkey_hex
    || pending?.certificate?.owner_pubkey_hex
    || '';
}

function requirePending(pending, ownerPublicKeyHex) {
  const lockId = recoveryLockId(pending);
  const owner = recoveryOwner(pending);
  if (
    !/^[0-9a-f]{96}$/.test(lockId)
    || !owner
    || owner.toLowerCase() !== String(ownerPublicKeyHex || '').toLowerCase()
  ) {
    throw new Error('FastPay recovery record does not match this wallet');
  }
  return { lockId, owner };
}

export function loadFastPayRecoveries(storage, ownerPublicKeyHex) {
  if (!storage || !ownerPublicKeyHex) return [];
  const raw = storage.getItem(FASTPAY_RECOVERY_STORE_KEY);
  if (!raw) return [];
  if (raw.length > MAX_STORE_BYTES) throw new Error('FastPay recovery store exceeds its size limit');
  const parsed = JSON.parse(raw);
  if (parsed?.schema !== FASTPAY_RECOVERY_STORE_SCHEMA || !Array.isArray(parsed.records)) {
    throw new Error('FastPay recovery store schema is invalid');
  }
  return parsed.records
    .filter(record => recoveryOwner(record?.pending).toLowerCase() === ownerPublicKeyHex.toLowerCase())
    .map(record => ({ lock_id: record.lock_id, pending: record.pending }));
}

export function saveFastPayRecovery(storage, ownerPublicKeyHex, pending) {
  if (!storage) throw new Error('Persistent browser storage is unavailable');
  const { lockId } = requirePending(pending, ownerPublicKeyHex);
  let records = [];
  const raw = storage.getItem(FASTPAY_RECOVERY_STORE_KEY);
  if (raw) {
    if (raw.length > MAX_STORE_BYTES) throw new Error('FastPay recovery store exceeds its size limit');
    const parsed = JSON.parse(raw);
    if (parsed?.schema !== FASTPAY_RECOVERY_STORE_SCHEMA || !Array.isArray(parsed.records)) {
      throw new Error('FastPay recovery store schema is invalid');
    }
    records = parsed.records;
  }
  records = records.filter(record => record.lock_id !== lockId);
  records.push({ lock_id: lockId, pending });
  records = records.slice(-MAX_PENDING_RECOVERIES);
  const encoded = JSON.stringify({ schema: FASTPAY_RECOVERY_STORE_SCHEMA, records });
  if (encoded.length > MAX_STORE_BYTES) throw new Error('FastPay recovery record exceeds its size limit');
  storage.setItem(FASTPAY_RECOVERY_STORE_KEY, encoded);
  return lockId;
}

export function removeFastPayRecovery(storage, lockId) {
  if (!storage || !/^[0-9a-f]{96}$/.test(lockId || '')) return;
  const raw = storage.getItem(FASTPAY_RECOVERY_STORE_KEY);
  if (!raw || raw.length > MAX_STORE_BYTES) return;
  const parsed = JSON.parse(raw);
  if (parsed?.schema !== FASTPAY_RECOVERY_STORE_SCHEMA || !Array.isArray(parsed.records)) return;
  const records = parsed.records.filter(record => record.lock_id !== lockId);
  storage.setItem(FASTPAY_RECOVERY_STORE_KEY, JSON.stringify({
    schema: FASTPAY_RECOVERY_STORE_SCHEMA,
    records,
  }));
}
