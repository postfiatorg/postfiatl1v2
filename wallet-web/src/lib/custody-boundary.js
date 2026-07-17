// Runtime self-custody boundary for browser-originated network payloads.
// Public keys, signatures, signed transactions, proofs, and commitments may
// cross the boundary. Seeds, backups, private keys, note openings, and signing
// authority must remain in browser memory.

const MAX_INSPECTION_DEPTH = 64;
const MAX_JSON_STRING_BYTES = 1_048_576;
const MIN_SECRET_SENTINEL_LENGTH = 16;

const PRIVATE_KEY_PATTERNS = Object.freeze([
  /(^|_)backup(_json)?$/,
  /(^|_)decrypted_backup$/,
  /(^|_)key_file$/,
  /(^|_)master_seed(_hex)?$/,
  /(^|_)mnemonic$/,
  /(^|_)note_file(s)?$/,
  /(^|_)note_opening(s)?$/,
  /(^|_)passphrase$/,
  /(^|_)private_key(_hex|_json)?$/,
  /(^|_)secret_key(_hex|_json)?$/,
  /(^|_)seed(_phrase|_hex)?$/,
  /(^|_)signature_seed(_hex)?$/,
  /(^|_)signature_randomness$/,
  /(^|_)signing_authority$/,
  /(^|_)signing_key(_hex)?$/,
  /(^|_)spend_auth_signing_key$/,
  /(^|_)spend_authority$/,
  /(^|_)spend_authorization_key$/,
  /(^|_)spend_key$/,
  /(^|_)spending_key$/,
]);

const registeredSecretValues = new Set();

function normalizeKey(key) {
  return String(key || '')
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/[^A-Za-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toLowerCase();
}

function isForbiddenKey(key) {
  const normalized = normalizeKey(key);
  return PRIVATE_KEY_PATTERNS.some(pattern => pattern.test(normalized));
}

function maybeJson(value) {
  if (typeof value !== 'string' || value.length > MAX_JSON_STRING_BYTES) return null;
  const trimmed = value.trim();
  if (!trimmed || !['{', '['].includes(trimmed[0])) return null;
  try {
    return JSON.parse(trimmed);
  } catch (_) {
    return null;
  }
}

function matchingRegisteredSecret(value) {
  if (typeof value !== 'string' || value.length < MIN_SECRET_SENTINEL_LENGTH) return false;
  for (const secret of registeredSecretValues) {
    if (value.includes(secret)) return true;
  }
  return false;
}

export function registerCustodyMaterial({ seed, backupJson } = {}) {
  for (const value of [seed, backupJson]) {
    if (typeof value === 'string' && value.length >= MIN_SECRET_SENTINEL_LENGTH) {
      registeredSecretValues.add(value);
    }
  }
}

export function clearCustodyMaterialRegistry() {
  registeredSecretValues.clear();
}

export function findCustodyMaterial(value) {
  const hits = [];
  const seen = new WeakSet();

  function visit(current, path, depth) {
    if (depth > MAX_INSPECTION_DEPTH) {
      hits.push({ path, reason: 'inspection-depth-exceeded' });
      return;
    }
    if (typeof current === 'string') {
      if (matchingRegisteredSecret(current)) {
        hits.push({ path, reason: 'registered-secret-value' });
        return;
      }
      const trimmed = current.trim();
      if (['{', '['].includes(trimmed[0]) && current.length > MAX_JSON_STRING_BYTES) {
        hits.push({ path, reason: 'serialized-json-inspection-limit-exceeded' });
        return;
      }
      const parsed = maybeJson(current);
      if (parsed !== null) visit(parsed, `${path}<json>`, depth + 1);
      return;
    }
    if (current === null || typeof current !== 'object') return;
    if (seen.has(current)) return;
    seen.add(current);

    if (Array.isArray(current)) {
      current.forEach((item, index) => visit(item, `${path}[${index}]`, depth + 1));
      return;
    }

    for (const [key, child] of Object.entries(current)) {
      const childPath = `${path}.${key}`;
      if (isForbiddenKey(key)) {
        hits.push({ path: childPath, reason: 'forbidden-custody-field' });
        continue;
      }
      visit(child, childPath, depth + 1);
    }
  }

  visit(value, '$', 0);
  return hits;
}

export function assertNoCustodyMaterial(value, context = 'network payload') {
  const hits = findCustodyMaterial(value);
  if (hits.length === 0) return;
  const locations = hits.map(hit => `${hit.path} (${hit.reason})`).join(', ');
  throw new Error(`${context} contains forbidden custody material at ${locations}`);
}
