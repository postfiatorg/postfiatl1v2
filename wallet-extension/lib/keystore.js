// Encrypted key store using Web Crypto API + chrome.storage.local.

const SALT_BYTES = 16;
const IV_BYTES = 12;
const LEGACY_PBKDF2_ITERATIONS = 100000;
export const PBKDF2_ITERATIONS = 310000;
export const MIN_PASSPHRASE_LENGTH = 10;

export function validateNewPassphrase(passphrase) {
  if (typeof passphrase !== 'string' || passphrase.length < MIN_PASSPHRASE_LENGTH) {
    throw new Error(`Passphrase must be at least ${MIN_PASSPHRASE_LENGTH} characters`);
  }
}

function vaultIterations(blob) {
  const iterations = blob?.iterations ?? LEGACY_PBKDF2_ITERATIONS;
  if (![LEGACY_PBKDF2_ITERATIONS, PBKDF2_ITERATIONS].includes(iterations)) {
    throw new Error('Unsupported encrypted vault KDF parameters');
  }
  return iterations;
}

export class KeyStore {
  async encrypt(masterSeedHex, passphrase) {
    validateNewPassphrase(passphrase);
    const enc = new TextEncoder();
    const salt = crypto.getRandomValues(new Uint8Array(SALT_BYTES));
    const iv = crypto.getRandomValues(new Uint8Array(IV_BYTES));

    const keyMaterial = await crypto.subtle.importKey(
      'raw', enc.encode(passphrase), 'PBKDF2', false, ['deriveKey']
    );
    const key = await crypto.subtle.deriveKey(
      { name: 'PBKDF2', salt, iterations: PBKDF2_ITERATIONS, hash: 'SHA-256' },
      keyMaterial,
      { name: 'AES-GCM', length: 256 },
      false,
      ['encrypt']
    );

    const ciphertext = await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv },
      key,
      enc.encode(masterSeedHex)
    );

    return {
      salt: btoa(String.fromCharCode(...salt)),
      iv: btoa(String.fromCharCode(...iv)),
      ciphertext: btoa(String.fromCharCode(...new Uint8Array(ciphertext))),
      iterations: PBKDF2_ITERATIONS
    };
  }

  async decrypt(blob, passphrase) {
    if (!blob || !blob.salt || !blob.iv || !blob.ciphertext) {
      throw new Error('Invalid encrypted blob');
    }
    const enc = new TextEncoder();
    const iterations = vaultIterations(blob);
    const salt = Uint8Array.from(atob(blob.salt), c => c.charCodeAt(0));
    const iv = Uint8Array.from(atob(blob.iv), c => c.charCodeAt(0));
    const ciphertext = Uint8Array.from(atob(blob.ciphertext), c => c.charCodeAt(0));

    const keyMaterial = await crypto.subtle.importKey(
      'raw', enc.encode(passphrase), 'PBKDF2', false, ['deriveKey']
    );
    const key = await crypto.subtle.deriveKey(
      { name: 'PBKDF2', salt, iterations, hash: 'SHA-256' },
      keyMaterial,
      { name: 'AES-GCM', length: 256 },
      false,
      ['decrypt']
    );

    const plaintext = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv },
      key,
      ciphertext
    );

    return new TextDecoder().decode(plaintext);
  }

  async saveWallet(blob, metadata) {
    await chrome.storage.local.set({
      wallet_encrypted: blob,
      wallet_metadata: metadata
    });
  }

  async loadWallet() {
    const data = await chrome.storage.local.get(['wallet_encrypted', 'wallet_metadata']);
    if (!data.wallet_encrypted) return null;
    return { blob: data.wallet_encrypted, metadata: data.wallet_metadata };
  }

  async removeWallet() {
    await chrome.storage.local.remove(['wallet_encrypted', 'wallet_metadata', 'tx_history', 'settings']);
  }

  async saveSettings(settings) {
    await chrome.storage.local.set({ settings });
  }

  async loadSettings() {
    const data = await chrome.storage.local.get('settings');
    return data.settings || { rpcEndpoint: 'ws://127.0.0.1:8080', autoLockMinutes: 15 };
  }
}

export const keystore = new KeyStore();
