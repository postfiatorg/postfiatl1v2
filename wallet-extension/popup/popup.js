import { RpcClient } from '../lib/rpc-client.js';
import { keystore } from '../lib/keystore.js';
import * as wasmMod from '../wasm/postfiat_wallet_wasm.js';

let rpc = null;
let wasmReady = false;
let currentBackup = null;
let walletAddress = null;
let chainId = 'postfiat-wan-devnet';

// Pending seed is kept in module scope (not window global) so it doesn't
// persist after popup close. Cleared on lock and after wallet creation.
let pendingSeed = null;
let pendingBackupJson = null;

function clearSensitiveMemory() {
  currentBackup = null;
  pendingSeed = null;
  pendingBackupJson = null;
  // Clear the seed display from DOM
  const seedText = document.getElementById('seedText');
  if (seedText) seedText.textContent = '';
}

// Init WASM synchronously from ArrayBuffer
async function initWasm() {
  if (wasmReady) return;
  const url = chrome.runtime.getURL('wasm/postfiat_wallet_wasm_bg.wasm');
  const resp = await fetch(url);
  const buf = await resp.arrayBuffer();
  wasmMod.initSync({ module: buf });
  wasmReady = true;
}

async function init() {
  // Show a view IMMEDIATELY so the user never sees a blank popup.
  // Check wallet state first — this doesn't need WASM or RPC.
  const wallet = await keystore.loadWallet();
  if (!wallet) {
    showView('noWallet');
  } else {
    walletAddress = wallet.metadata?.address;
    document.getElementById('lockedAddress').textContent = walletAddress || '...';
    showView('locked');
  }

  // Now load WASM in the background — non-blocking
  try {
    await initWasm();
  } catch (e) {
    document.getElementById('chainStatus').textContent = 'wasm err';
    document.getElementById('chainStatus').classList.add('offline');
    return;
  }

  // Load settings and create RPC client
  const settings = await keystore.loadSettings();
  try {
    rpc = new RpcClient(settings.rpcEndpoint);
  } catch (e) {
    document.getElementById('chainStatus').textContent = 'bad rpc';
    document.getElementById('chainStatus').classList.add('offline');
    return;
  }

  // Check chain status (non-fatal if it fails)
  try {
    const status = await rpc.status();
    if (status.ok) {
      document.getElementById('chainStatus').textContent = 'H:' + status.result.block_height;
      chainId = status.result.chain_id;
    } else {
      document.getElementById('chainStatus').textContent = 'offline';
      document.getElementById('chainStatus').classList.add('offline');
    }
  } catch (e) {
    document.getElementById('chainStatus').textContent = 'offline';
    document.getElementById('chainStatus').classList.add('offline');
  }

  // Check background unlock state
  chrome.runtime.sendMessage({ type: 'getState' }, (state) => {
    if (chrome.runtime.lastError) {
      return;
    }
    // Either way, popup shows locked view — user must re-unlock to get backup back
  });
}
}

function showView(name) {
  ['noWalletView', 'lockedView', 'walletView'].forEach(id => {
    document.getElementById(id).classList.add('hidden');
  });
  document.getElementById(name + 'View').classList.remove('hidden');
}

function showWalletView() {
  showView('wallet');
  document.getElementById('walletAddress').textContent = walletAddress;
  loadBalance();
}

async function loadBalance() {
  try {
    const resp = await rpc.account(walletAddress);
    if (resp.ok && resp.result) {
      const bal = resp.result.balance || 0;
      document.getElementById('walletBalance').textContent = bal.toLocaleString() + ' PFT';
    } else {
      document.getElementById('walletBalance').textContent = '0 PFT';
    }
  } catch (e) {
    document.getElementById('walletBalance').textContent = '? PFT';
  }
}

// --- Create wallet ---
document.getElementById('createBtn').addEventListener('click', async () => {
  // Prevent creating if a wallet already exists
  const existing = await keystore.loadWallet();
  if (existing) {
    alert('A wallet already exists. Remove it first in Settings if you want to create a new one.');
    return;
  }
  const pass = document.getElementById('createPassphrase').value;
  if (!pass || pass.length < 4) {
    alert('Passphrase must be at least 4 characters');
    return;
  }
  const savedCheck = document.getElementById('seedSavedCheck');
  if (!savedCheck.checked) {
    // Show seed first, require checkbox
    const seed = wasmMod.random_master_seed();
    const result = wasmMod.wallet_keygen(chainId, seed, 0);
    document.getElementById('seedText').textContent = seed;
    document.getElementById('newWalletAddress').textContent = result.address;
    document.getElementById('seedDisplay').classList.remove('hidden');
    // Keep in module scope only — NOT window global
    pendingSeed = seed;
    pendingBackupJson = result.backup_json;
    alert('Please save your seed, check the box, then click Create again.');
    return;
  }
  let seed, backupJson, address;
  if (pendingSeed) {
    seed = pendingSeed;
    backupJson = pendingBackupJson;
    address = wasmMod.wallet_address_from_seed(chainId, seed, 0);
  } else {
    // User checked box without seeing seed — generate fresh
    seed = wasmMod.random_master_seed();
    const result = wasmMod.wallet_keygen(chainId, seed, 0);
    backupJson = result.backup_json;
    address = result.address;
  }

  const blob = await keystore.encrypt(seed, pass);
  await keystore.saveWallet(blob, { address, accountIndex: 0, chainId });

  currentBackup = backupJson;
  walletAddress = address;
  // Notify background of unlock — send only the address, NOT the seed or backup
  // (seed/backup stay in popup module scope; background only needs to track lock state)
  chrome.runtime.sendMessage({ type: 'unlock', address: walletAddress });

  // Clear sensitive data immediately
  pendingSeed = null;
  pendingBackupJson = null;
  document.getElementById('seedText').textContent = '';
  document.getElementById('seedDisplay').classList.add('hidden');
  document.getElementById('seedSavedCheck').checked = false;
  document.getElementById('createPassphrase').value = '';
  showWalletView();
});

// --- Import wallet ---
document.getElementById('importBtn').addEventListener('click', async () => {
  // Prevent importing if a wallet already exists
  const existing = await keystore.loadWallet();
  if (existing) {
    alert('A wallet already exists. Remove it first in Settings if you want to import a different one.');
    return;
  }
  const seed = document.getElementById('importSeed').value.trim();
  const pass = document.getElementById('createPassphrase').value;
  if (!seed || seed.length !== 64 || !/^[0-9a-f]{64}$/.test(seed)) {
    alert('Seed must be 64 hex characters');
    return;
  }
  if (!pass || pass.length < 4) {
    alert('Passphrase must be at least 4 characters');
    return;
  }
  const result = wasmMod.wallet_keygen(chainId, seed, 0);

  const blob = await keystore.encrypt(seed, pass);
  await keystore.saveWallet(blob, { address: result.address, accountIndex: 0, chainId });

  currentBackup = result.backup_json;
  walletAddress = result.address;
  // Only send address to background — not seed or backup
  chrome.runtime.sendMessage({ type: 'unlock', address: walletAddress });

  // Clear sensitive fields
  document.getElementById('importSeed').value = '';
  document.getElementById('createPassphrase').value = '';
  alert('Wallet imported!\nAddress: ' + result.address);
  showWalletView();
});

// --- Unlock ---
document.getElementById('unlockBtn').addEventListener('click', async () => {
  const pass = document.getElementById('unlockPass').value;
  if (!pass) {
    document.getElementById('unlockError').textContent = 'Enter your passphrase';
    return;
  }
  const wallet = await keystore.loadWallet();
  if (!wallet) {
    document.getElementById('unlockError').textContent = 'No wallet found';
    return;
  }
  try {
    const seed = await keystore.decrypt(wallet.blob, pass);
    const result = wasmMod.wallet_keygen(chainId, seed, 0);
    currentBackup = result.backup_json;
    walletAddress = wallet.metadata?.address || result.address;
    // Only send address — not seed or backup
    chrome.runtime.sendMessage({ type: 'unlock', address: walletAddress });
    // Clear passphrase from input
    document.getElementById('unlockPass').value = '';
    document.getElementById('unlockError').textContent = '';
    showWalletView();
  } catch (e) {
    document.getElementById('unlockError').textContent = 'Wrong passphrase';
  }
});

// --- Lock ---
document.getElementById('lockBtn').addEventListener('click', () => {
  chrome.runtime.sendMessage({ type: 'lock' });
  clearSensitiveMemory();
  showView('locked');
});

// --- Copy address ---
document.getElementById('walletAddress').addEventListener('click', () => {
  navigator.clipboard.writeText(walletAddress);
  showToast('Address copied');
});

function showToast(msg) {
  const t = document.createElement('div');
  t.className = 'copy-toast';
  t.textContent = msg;
  document.body.appendChild(t);
  setTimeout(() => t.remove(), 2000);
}

// --- Send flow ---
document.getElementById('sendBtn').addEventListener('click', () => {
  document.getElementById('sendView').classList.remove('hidden');
  document.getElementById('historyView').classList.add('hidden');
  document.getElementById('settingsView').classList.add('hidden');
  document.getElementById('quoteView').classList.add('hidden');
  document.getElementById('sendError').textContent = '';
  document.getElementById('sendSuccess').textContent = '';
});

document.getElementById('quoteBtn').addEventListener('click', async () => {
  const to = document.getElementById('sendTo').value.trim();
  const amount = parseInt(document.getElementById('sendAmount').value);
  const err = document.getElementById('sendError');
  err.textContent = '';

  if (!to || !to.startsWith('pf') || to.length !== 42 || !/^pf[0-9a-f]{40}$/.test(to)) {
    err.textContent = 'Invalid recipient address (must be pf + 40 hex chars)';
    return;
  }
  if (!amount || amount <= 0) {
    err.textContent = 'Amount must be a positive integer';
    return;
  }

  try {
    const quote = await rpc.transferFeeQuote(walletAddress, to, amount);
    if (!quote.ok) {
      err.textContent = quote.error?.message || 'Quote failed';
      return;
    }
    const q = quote.result;
    document.getElementById('quoteFee').textContent = q.minimum_fee + ' PFT';
    document.getElementById('quoteTotal').textContent = (amount + q.minimum_fee) + ' PFT';
    document.getElementById('quoteSeq').textContent = q.sequence;
    document.getElementById('quoteAfter').textContent = (q.sender_balance_after_amount_and_fee !== null && q.sender_balance_after_amount_and_fee !== undefined) ? q.sender_balance_after_amount_and_fee + ' PFT' : '-';
    document.getElementById('quoteView').classList.remove('hidden');
  } catch (e) {
    err.textContent = e.message || String(e);
  }
});

document.getElementById('confirmSendBtn').addEventListener('click', async () => {
  const to = document.getElementById('sendTo').value.trim();
  const amount = parseInt(document.getElementById('sendAmount').value);
  const err = document.getElementById('sendError');
  const success = document.getElementById('sendSuccess');
  err.textContent = '';
  success.textContent = '';

  // Validate inputs before attempting send
  if (!to || !to.startsWith('pf') || to.length !== 42 || !/^pf[0-9a-f]{40}$/.test(to)) {
    err.textContent = 'Invalid recipient address (must be pf + 40 hex chars)';
    return;
  }
  if (!amount || amount <= 0) {
    err.textContent = 'Amount must be a positive integer';
    return;
  }
  if (!currentBackup) {
    err.textContent = 'Wallet not unlocked. Please unlock first.';
    return;
  }

  success.textContent = 'Signing...';

  try {
    // Get fresh quote
    const quote = await rpc.transferFeeQuote(walletAddress, to, amount);
    if (!quote.ok) {
      err.textContent = quote.error?.message || 'Quote failed';
      success.textContent = '';
      return;
    }

    // Sign with WASM
    const signed = wasmMod.wallet_sign_transfer(currentBackup, JSON.stringify(quote.result));
    const signedJson = JSON.stringify(signed);

    // Submit
    success.textContent = 'Submitting...';
    const submit = await rpc.submitSignedTransfer(signedJson);
    if (!submit.ok) {
      err.textContent = submit.error?.message || 'Submit failed';
      success.textContent = '';
      return;
    }

    const txId = submit.result.tx_id;
    success.textContent = 'Submitted! tx_id: ' + txId.slice(0, 16) + '... Polling...';

    // Poll for receipt
    let receipt = null;
    for (let i = 0; i < 15; i++) {
      await new Promise(r => setTimeout(r, 2000));
      const rResp = await rpc.receipts(txId);
      if (rResp.ok && rResp.result && rResp.result.length > 0) {
        receipt = rResp.result[0];
        break;
      }
    }

    if (receipt && receipt.accepted) {
      success.textContent = 'Confirmed! tx_id: ' + txId.slice(0, 16) + '...';
    } else if (receipt && !receipt.accepted) {
      err.textContent = 'Rejected: ' + (receipt.code || '') + ' - ' + (receipt.message || '');
      success.textContent = '';
    } else {
      success.textContent = 'Pending (no receipt yet). tx_id: ' + txId;
    }

    loadBalance();
    document.getElementById('quoteView').classList.add('hidden');
  } catch (e) {
    err.textContent = e.message || String(e);
    success.textContent = '';
  }
});

// --- History ---
// HTML-escape function to prevent XSS from RPC response data
function escapeHtml(s) {
  if (s == null) return '';
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

document.getElementById('historyBtn').addEventListener('click', async () => {
  document.getElementById('historyView').classList.remove('hidden');
  document.getElementById('sendView').classList.add('hidden');
  document.getElementById('settingsView').classList.add('hidden');
  document.getElementById('quoteView').classList.add('hidden');
  document.getElementById('txList').textContent = 'Loading...';

  try {
    const resp = await rpc.accountTx(walletAddress, { limit: 20 });
    if (!resp.ok || !resp.result || !resp.result.rows) {
      document.getElementById('txList').textContent = 'No transactions';
      return;
    }
    const rows = resp.result.rows;
    if (rows.length === 0) {
      document.getElementById('txList').textContent = 'No transactions yet';
      return;
    }
    document.getElementById('txList').innerHTML = rows.map(function(r) {
      var isIn = r.recipient === walletAddress;
      var counterparty = isIn ? (r.sender || '?') : (r.recipient || '?');
      var amt = r.amount || 0;
      return '<div class="tx-item"><span class="from">H:' + escapeHtml(r.block_height || '?') + ' ' + escapeHtml(counterparty.slice(0, 12)) + '...</span><span class="amt ' + (isIn ? 'in' : 'out') + '">' + (isIn ? '+' : '-') + escapeHtml(amt) + '</span></div>';
    }).join('');
  } catch (e) {
    document.getElementById('txList').textContent = 'Error: ' + (e.message || String(e));
  }
});

// --- Settings ---
document.getElementById('settingsBtn').addEventListener('click', async () => {
  document.getElementById('settingsView').classList.remove('hidden');
  document.getElementById('sendView').classList.add('hidden');
  document.getElementById('historyView').classList.add('hidden');
  const settings = await keystore.loadSettings();
  const select = document.getElementById('rpcEndpointSelect');
  const customInput = document.getElementById('rpcEndpointCustom');
  const current = settings.rpcEndpoint || 'ws://127.0.0.1:8080';
  // Check if current matches a preset
  const presetOption = Array.from(select.options).find(o => o.value === current);
  if (presetOption) {
    select.value = current;
    customInput.style.display = 'none';
  } else {
    select.value = 'custom';
    customInput.value = current;
    customInput.style.display = 'block';
  }
  document.getElementById('autoLockSelect').value = settings.autoLockMinutes || 15;
  document.getElementById('settingsError').textContent = '';
  document.getElementById('settingsSuccess').textContent = '';
});

// Show/hide custom endpoint input based on dropdown selection
document.getElementById('rpcEndpointSelect').addEventListener('change', () => {
  const select = document.getElementById('rpcEndpointSelect');
  const customInput = document.getElementById('rpcEndpointCustom');
  if (select.value === 'custom') {
    customInput.style.display = 'block';
  } else {
    customInput.style.display = 'none';
  }
});

document.getElementById('saveSettingsBtn').addEventListener('click', async () => {
  const select = document.getElementById('rpcEndpointSelect');
  let endpoint;
  if (select.value === 'custom') {
    endpoint = document.getElementById('rpcEndpointCustom').value.trim();
  } else {
    endpoint = select.value;
  }
  const autoLock = parseInt(document.getElementById('autoLockSelect').value);
  if (!endpoint || (!endpoint.startsWith('ws://') && !endpoint.startsWith('wss://'))) {
    document.getElementById('settingsError').textContent = 'Endpoint must start with ws:// or wss://';
    return;
  }
  await keystore.saveSettings({ rpcEndpoint: endpoint, autoLockMinutes: autoLock });
  rpc = new RpcClient(endpoint);
  document.getElementById('settingsError').textContent = '';
  document.getElementById('settingsSuccess').textContent = 'Settings saved!';
  // Re-check chain status with new endpoint
  try {
    const status = await rpc.status();
    if (status.ok) {
      document.getElementById('chainStatus').textContent = 'H:' + status.result.block_height;
      document.getElementById('chainStatus').classList.remove('offline');
    } else {
      document.getElementById('chainStatus').textContent = 'offline';
      document.getElementById('chainStatus').classList.add('offline');
    }
  } catch (e) {
    document.getElementById('chainStatus').textContent = 'offline';
    document.getElementById('chainStatus').classList.add('offline');
  }
  setTimeout(() => { document.getElementById('settingsSuccess').textContent = ''; }, 2000);
});

// --- Export Backup ---
document.getElementById('exportBackupBtn').addEventListener('click', async () => {
  const wallet = await keystore.loadWallet();
  if (!wallet) return;
  const blob = new Blob([JSON.stringify(wallet, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = 'postfiat-wallet-backup-' + (wallet.metadata?.address || 'unknown') + '.json';
  a.click();
  URL.revokeObjectURL(url);
});

// --- Import Backup ---
document.getElementById('importBackupBtn').addEventListener('click', () => {
  document.getElementById('importBackupFile').click();
});

document.getElementById('importBackupFile').addEventListener('change', async (e) => {
  const file = e.target.files[0];
  if (!file) return;
  const text = await file.text();
  try {
    const data = JSON.parse(text);
    // Validate structure — reject if missing required fields
    if (!data.blob || typeof data.blob !== 'object' ||
        !data.blob.salt || !data.blob.iv || !data.blob.ciphertext ||
        !data.metadata || typeof data.metadata !== 'object' ||
        !data.metadata.address || typeof data.metadata.address !== 'string') {
      document.getElementById('settingsError').textContent = 'Invalid backup file format';
      return;
    }
    // Validate address format
    if (!/^pf[0-9a-f]{40}$/.test(data.metadata.address)) {
      document.getElementById('settingsError').textContent = 'Invalid address in backup';
      return;
    }
    // Warn if a wallet already exists
    const existing = await keystore.loadWallet();
    if (existing && !confirm('A wallet already exists. Importing will replace it. Continue?')) {
      return;
    }
    // Only store known fields — don't blindly persist unknown keys
    const cleanBlob = {
      salt: data.blob.salt,
      iv: data.blob.iv,
      ciphertext: data.blob.ciphertext
    };
    const cleanMeta = {
      address: data.metadata.address,
      accountIndex: typeof data.metadata.accountIndex === 'number' ? data.metadata.accountIndex : 0,
      chainId: typeof data.metadata.chainId === 'string' ? data.metadata.chainId : 'postfiat-wan-devnet'
    };
    await keystore.saveWallet(cleanBlob, cleanMeta);
    document.getElementById('settingsSuccess').textContent = 'Backup imported! Address: ' + cleanMeta.address;
    document.getElementById('settingsError').textContent = '';
    walletAddress = cleanMeta.address;
    document.getElementById('lockedAddress').textContent = walletAddress;
    // Lock the wallet — user must unlock with their passphrase to use it
    clearSensitiveMemory();
    chrome.runtime.sendMessage({ type: 'lock' });
    showView('locked');
  } catch (err) {
    document.getElementById('settingsError').textContent = 'Import failed: ' + err.message;
  }
  // Reset file input so same file can be re-selected
  e.target.value = '';
});

// --- Remove Wallet ---
document.getElementById('removeWalletBtn').addEventListener('click', async () => {
  if (!confirm('Are you sure? This will permanently delete your wallet from this browser. Make sure you have your seed saved!')) {
    return;
  }
  await keystore.removeWallet();
  chrome.runtime.sendMessage({ type: 'lock' });
  clearSensitiveMemory();
  walletAddress = null;
  document.getElementById('lockedAddress').textContent = '...';
  document.getElementById('settingsSuccess').textContent = 'Wallet removed';
  // Clear settings view state
  document.getElementById('settingsView').classList.add('hidden');
  setTimeout(() => showView('noWallet'), 1000);
});

init();
