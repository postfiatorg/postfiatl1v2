// Background service worker for PostFiat Wallet.
// Tracks lock state only — does NOT hold seed or backup.
// Seed/backup stay in popup module scope and are cleared on popup close.

let walletAddress = null;
let unlocked = false;
let autoLockTimer = null;
let autoLockMinutes = 15;
let settingsLoaded = false;

const EXTENSION_ID = chrome.runtime.id;

chrome.runtime.onInstalled.addListener(() => {
  console.log('PostFiat Wallet installed');
});

// Load settings on startup
chrome.storage.local.get('settings', (data) => {
  if (data.settings?.autoLockMinutes) {
    autoLockMinutes = data.settings.autoLockMinutes;
  }
  settingsLoaded = true;
});

function resetAutoLock() {
  if (autoLockTimer) clearTimeout(autoLockTimer);
  autoLockTimer = setTimeout(() => {
    unlocked = false;
    walletAddress = null;
    autoLockTimer = null;
    console.log('Wallet auto-locked');
  }, autoLockMinutes * 60 * 1000);
}

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  // S4.1: Verify message comes from our own extension only
  if (sender.id !== EXTENSION_ID) {
    sendResponse({ ok: false, error: 'unauthorized sender' });
    return true;
  }

  if (message.type === 'unlock') {
    // S4.3: Require address field
    if (!message.address || typeof message.address !== 'string') {
      sendResponse({ ok: false, error: 'missing address' });
      return true;
    }
    // Reload settings to get latest autoLockMinutes
    chrome.storage.local.get('settings', (data) => {
      if (data.settings?.autoLockMinutes) {
        autoLockMinutes = data.settings.autoLockMinutes;
      }
      unlocked = true;
      walletAddress = message.address;
      resetAutoLock();
      sendResponse({ ok: true });
    });
    return true; // async response
  } else if (message.type === 'lock') {
    unlocked = false;
    walletAddress = null;
    if (autoLockTimer) {
      clearTimeout(autoLockTimer);
      autoLockTimer = null;
    }
    sendResponse({ ok: true });
  } else if (message.type === 'getState') {
    // S4.2: Only return lock state and address — NO seed, NO backup
    sendResponse({ unlocked, address: walletAddress });
  } else if (message.type === 'poke') {
    if (unlocked) resetAutoLock();
    sendResponse({ ok: true });
  } else {
    // Reject unknown message types
    sendResponse({ ok: false, error: 'unknown message type' });
  }
  return true;
});
