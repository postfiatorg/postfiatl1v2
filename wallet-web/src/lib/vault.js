// Encrypted vault using Web Crypto API + IndexedDB.
// Master seed encrypted with AES-256-GCM, key derived from passphrase via PBKDF2.

import {
  clearCustodyMaterialRegistry,
  registerCustodyMaterial,
} from './custody-boundary.js';

const SALT_BYTES = 16;
const IV_BYTES = 12;
const PBKDF2_ITERATIONS = 310000;
const DB_NAME = 'postfiat-wallet';
const DB_VERSION = 1;
const STORE_VAULTS = 'vaults';
const STORE_SETTINGS = 'settings';

// Default RPC endpoint.
//
// - HTTPS wallet page (raw IP / public host): crypto.subtle requires a secure
//   context, so the page is served over HTTPS and the browser cannot open a
//   mixed-content ws:// socket to the plain proxy on 8080. Route the WebSocket
//   through Vite's same-origin /rpc tunnel, which terminates TLS at the page
//   and forwards plain ws:// to the proxy.
// - HTTP wallet page on localhost / 127.0.0.1: crypto.subtle is available and
//   a direct ws:// to the proxy on 8080 avoids depending on the Vite WS proxy
//   path (which is dev-only and not part of the deployed wallet contract).
// - Built/hosted wallet pages (no 5173): same-origin /rpc as before.
export function defaultRpcEndpoint() {
  const isHttps = window.location.protocol === 'https:';
  const proto = isHttps ? 'wss:' : 'ws:';
  if (window.location.port === '5173') {
    if (isHttps) {
      return `${proto}//${window.location.host}/rpc`;
    }
    const host = window.location.hostname || '127.0.0.1';
    return `${proto}//${host}:8080`;
  }
  return `${proto}//${window.location.host}/rpc`;
}

export function defaultSwapServerUrl() {
  if (window.location.port === '5173') {
    if (window.location.protocol === 'https:') {
      return `${window.location.protocol}//${window.location.host}`;
    }
    const host = window.location.hostname || '127.0.0.1';
    return `http://${host}:8080`;
  }
  return `${window.location.protocol}//${window.location.host}`;
}

function defaultSettings() {
  return {
    rpcEndpoint: defaultRpcEndpoint(),
    autoLockMinutes: 15,
    swapServerUrl: defaultSwapServerUrl(),
  };
}

function isLoopbackHost(hostname) {
  const host = String(hostname || '').toLowerCase();
  return host === 'localhost' || host === '127.0.0.1' || host === '::1' || host === '[::1]';
}

export function normalizeRpcEndpoint(endpoint) {
  const value = String(endpoint || '').trim();
  if (!value) return defaultRpcEndpoint();

  let parsed;
  try {
    parsed = new URL(value);
  } catch (_) {
    return value;
  }

  const isWebSocket = parsed.protocol === 'ws:' || parsed.protocol === 'wss:';
  const pageIsHttps = window.location.protocol === 'https:';
  const pageIsLoopback = isLoopbackHost(window.location.hostname);

  // Saved localhost/127.0.0.1 proxy endpoints are only valid when the wallet
  // itself is running locally over HTTP. Over HTTPS they are mixed content; on
  // a remote/public page they point at the user's own machine, not this server.
  if (isWebSocket && isLoopbackHost(parsed.hostname) && (pageIsHttps || !pageIsLoopback)) {
    return defaultRpcEndpoint();
  }

  return value;
}

export function normalizeSwapServerUrl(url) {
  const value = String(url || '').trim();
  if (!value) return defaultSwapServerUrl();

  let parsed;
  try {
    parsed = new URL(value);
  } catch (_) {
    return value;
  }

  const pageIsHttps = window.location.protocol === 'https:';
  const pageIsLoopback = isLoopbackHost(window.location.hostname);
  if (isLoopbackHost(parsed.hostname) && (pageIsHttps || !pageIsLoopback)) {
    return defaultSwapServerUrl();
  }

  return value.replace(/\/+$/, '');
}

function normalizeSettings(settings = {}) {
  const defaults = defaultSettings();
  return {
    ...defaults,
    ...settings,
    rpcEndpoint: normalizeRpcEndpoint(settings.rpcEndpoint),
    autoLockMinutes: settings.autoLockMinutes || defaults.autoLockMinutes,
    swapServerUrl: normalizeSwapServerUrl(settings.swapServerUrl || defaults.swapServerUrl),
  };
}

// Module-scope only — never on window
let decryptedSeed = null;
let decryptedBackupJson = null;
let autoLockTimer = null;
let autoLockMinutes = 15;

export function clearSensitiveMemory() {
  decryptedSeed = null;
  decryptedBackupJson = null;
  clearCustodyMaterialRegistry();
}

export function getDecryptedSeed() {
  return decryptedSeed;
}

export function getDecryptedBackup() {
  return decryptedBackupJson;
}

export function setDecryptedState(seed, backupJson) {
  decryptedSeed = seed;
  decryptedBackupJson = backupJson;
  clearCustodyMaterialRegistry();
  registerCustodyMaterial({ seed, backupJson });
}

export function setAutoLockMinutes(minutes) {
  autoLockMinutes = minutes;
}

export function resetAutoLock(onLock) {
  if (autoLockTimer) clearTimeout(autoLockTimer);
  autoLockTimer = setTimeout(() => {
    clearSensitiveMemory();
    autoLockTimer = null;
    if (onLock) onLock();
  }, autoLockMinutes * 60 * 1000);
}

export function clearAutoLock() {
  if (autoLockTimer) {
    clearTimeout(autoLockTimer);
    autoLockTimer = null;
  }
}

// --- Encryption ---

function bytesToBase64(bytes) {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function base64ToBytes(value) {
  const binary = atob(String(value || ''));
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function ensureWebCrypto() {
  if (!crypto?.subtle) {
    const isSecure = typeof isSecureContext !== 'undefined' ? isSecureContext
      : (window.location.protocol === 'https:'
        || window.location.hostname === 'localhost'
        || window.location.hostname === '127.0.0.1');
    throw new Error(
      isSecure
        ? 'WebCrypto (crypto.subtle) is unavailable in this browser context.'
        : 'Encrypted vault requires a secure context. Open the wallet over HTTPS, '
          + 'localhost, or 127.0.0.1 — crypto.subtle is disabled on plain HTTP '
          + `at ${window.location.host}.`
    );
  }
}

export async function encryptVault(masterSeedHex, passphrase) {
  ensureWebCrypto();
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
    salt: bytesToBase64(salt),
    iv: bytesToBase64(iv),
    ciphertext: bytesToBase64(new Uint8Array(ciphertext)),
  };
}

export async function decryptVault(blob, passphrase) {
  ensureWebCrypto();
  if (!blob || !blob.salt || !blob.iv || !blob.ciphertext) {
    throw new Error('Invalid encrypted blob');
  }
  const enc = new TextEncoder();
  const salt = base64ToBytes(blob.salt);
  const iv = base64ToBytes(blob.iv);
  const ciphertext = base64ToBytes(blob.ciphertext);

  const keyMaterial = await crypto.subtle.importKey(
    'raw', enc.encode(passphrase), 'PBKDF2', false, ['deriveKey']
  );
  const key = await crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt, iterations: PBKDF2_ITERATIONS, hash: 'SHA-256' },
    keyMaterial,
    { name: 'AES-GCM', length: 256 },
    false,
    ['decrypt']
  );

  try {
    const plaintext = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv },
      key,
      ciphertext
    );
    return new TextDecoder().decode(plaintext);
  } catch (e) {
    throw new Error('Incorrect passphrase');
  }
}

// --- IndexedDB ---

function openDB() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE_VAULTS)) {
        db.createObjectStore(STORE_VAULTS, { keyPath: 'accountId' });
      }
      if (!db.objectStoreNames.contains(STORE_SETTINGS)) {
        db.createObjectStore(STORE_SETTINGS, { keyPath: 'key' });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

function withStore(storeName, mode, callback) {
  return openDB().then(db => {
    return new Promise((resolve, reject) => {
      const tx = db.transaction(storeName, mode);
      const store = tx.objectStore(storeName);
      let result;
      tx.oncomplete = () => { db.close(); resolve(result); };
      tx.onerror = () => { db.close(); reject(tx.error); };
      result = callback(store);
    });
  });
}

export async function saveVault(accountId, blob, metadata) {
  await withStore(STORE_VAULTS, 'readwrite', (store) => {
    store.put({
      accountId,
      vault: blob,
      metadata,
      updatedAt: new Date().toISOString(),
    });
  });
}

export async function loadVault(accountId = 'default') {
  try {
    const record = await withStore(STORE_VAULTS, 'readonly', (store) => {
      return new Promise((resolve, reject) => {
        const req = store.get(accountId);
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
      });
    });
    if (!record) return null;
    return { blob: record.vault, metadata: record.metadata };
  } catch (e) {
    return null;
  }
}

export async function removeVault(accountId = 'default') {
  await withStore(STORE_VAULTS, 'readwrite', (store) => {
    store.delete(accountId);
  });
  // Also clear settings
  await withStore(STORE_SETTINGS, 'readwrite', (store) => {
    store.delete('settings');
  });
}

export async function saveSettings(settings) {
  await withStore(STORE_SETTINGS, 'readwrite', (store) => {
    store.put({ key: 'settings', ...normalizeSettings(settings) });
  });
}

export async function loadSettings() {
  try {
    const record = await withStore(STORE_SETTINGS, 'readonly', (store) => {
      return new Promise((resolve, reject) => {
        const req = store.get('settings');
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
      });
    });
    if (!record) return normalizeSettings();
    const { key, ...settings } = record;
    return normalizeSettings(settings);
  } catch (e) {
    return normalizeSettings();
  }
}

// --- Page unload cleanup ---

export function setupUnloadCleanup() {
  window.addEventListener('beforeunload', () => {
    clearSensitiveMemory();
  });

  // Reset auto-lock on user activity
  ['click', 'keydown', 'touchstart'].forEach(event => {
    window.addEventListener(event, () => {
      if (decryptedSeed) {
        // Re-export resetAutoLock for App to call with callback
        // We just clear and let App re-set it
      }
    });
  });
}
