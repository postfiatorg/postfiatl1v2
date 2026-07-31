import React, { useState, useEffect, useRef, useCallback } from 'react';
import { initWasm, getWasm } from './lib/wasm-loader.js';
import { FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT, RpcClient } from './lib/rpc-client.js';
import { TxBuilder } from './lib/tx-builder.js';
import {
  loadVault, saveVault, removeVault, encryptVault, decryptVault,
  saveSettings, loadSettings, normalizeRpcEndpoint,
  clearSensitiveMemory, getDecryptedSeed, getDecryptedBackup, setDecryptedState,
  setAutoLockMinutes, resetAutoLock, clearAutoLock, setupUnloadCleanup,
} from './lib/vault.js';
import { CHAIN_ID, LEGACY_CHAIN_IDS, ACCOUNT_INDEX, isValidAddress, truncateMiddle } from './lib/utils.js';
import { SwapServer } from './lib/swap-server.js';

import Onboard from './components/Onboard.jsx';
import LockScreen from './components/LockScreen.jsx';
import WalletHome from './components/WalletHome.jsx';
import Send from './components/Send.jsx';
import Swap from './components/Swap.jsx';
import Bridge from './components/Bridge.jsx';
import NavList from './components/NavList.jsx';
import NavDetail from './components/NavDetail.jsx';
import More from './components/More.jsx';
import NavcoinMarket from './components/NavcoinMarket.jsx';
import { DEFAULT_NAVCOIN_MARKET, NAVCOIN_MARKETS, navcoinMarketByKey } from './lib/navcoin-markets.js';

const PROXY_AUTH_SESSION_KEY = 'postfiat.wallet_proxy_api_token';

async function loadControlledLocalProxySession() {
  const response = await fetch('/api/bridge/local-session', {
    method: 'GET',
    headers: { Accept: 'application/json' },
    cache: 'no-store',
  });
  if (!response.ok) return '';
  const payload = await response.json();
  if (
    payload?.ok !== true
    || payload?.schema !== 'postfiat-local-wallet-session-v1'
    || typeof payload?.token !== 'string'
    || payload.token.length < 32
  ) {
    throw new Error('localhost wallet session response is invalid');
  }
  return payload.token;
}

const NAV_ITEMS = [
  { id: 'wallet', label: 'Wallet' }, { id: 'bridge', label: 'Bridge' },
  { id: 'market', label: 'NAV Markets' }, { id: 'send', label: 'Send' },
  { id: 'swap', label: 'Process' },
  { id: 'nav', label: 'NavCoins' }, { id: 'more', label: 'More' },
];
const isOn = (tab, id) => tab === id || (id === 'nav' && tab === 'navDetail');

export default function App() {
  const [tab, setTab] = useState('onboard');
  const [coinId, setCoinId] = useState(null);
  const [marketKey, setMarketKey] = useState(DEFAULT_NAVCOIN_MARKET.key);
  const [wasmReady, setWasmReady] = useState(false);
  const [rpc, setRpc] = useState(null);
  const [txBuilder, setTxBuilder] = useState(null);
  const [swapServer, setSwapServer] = useState(null);
  const [settings, setSettings] = useState(null);
  const [walletAddress, setWalletAddress] = useState(null);
  const [walletPublicKeyHex, setWalletPublicKeyHex] = useState(null);
  const [chainStatus, setChainStatus] = useState(null);
  const [chainCapabilities, setChainCapabilities] = useState(null);
  const [walletLiveSnapshot, setWalletLiveSnapshot] = useState(null);
  const [walletFeedStatus, setWalletFeedStatus] = useState({ status: 'idle' });
  const [rpcEpoch, setRpcEpoch] = useState(0);
  const [error, setError] = useState('');
  const [walletNotice, setWalletNotice] = useState('');
  const [toast, setToast] = useState('');
  const [sendSource, setSendSource] = useState('account');
  const [proxyAuthToken, setProxyAuthToken] = useState('');
  const [controlledLocalSession, setControlledLocalSession] = useState(false);
  const creatingRef = useRef(false);

  // --- Init ---
  useEffect(() => {
    (async () => {
      setupUnloadCleanup();
      const s = await loadSettings();
      const controlledToken = await loadControlledLocalProxySession().catch(() => '');
      const sessionProxyAuthToken = controlledToken
        || sessionStorage.getItem(PROXY_AUTH_SESSION_KEY)
        || '';
      setControlledLocalSession(Boolean(controlledToken));
      setProxyAuthToken(sessionProxyAuthToken);
      setSettings(s);
      setAutoLockMinutes(s.autoLockMinutes || 15);

      try { setSwapServer(new SwapServer(s.swapServerUrl, sessionProxyAuthToken)); } catch (e) { /* optional */ }

      const vault = await loadVault();
      if (vault) {
        setWalletAddress(vault.metadata?.address);
        setWalletPublicKeyHex(vault.metadata?.public_key_hex || null);
        setTab('locked');
      } else {
        setTab('onboard');
      }

      try {
        await initWasm();
        setWasmReady(true);
      } catch (e) {
        setError('WASM init failed: ' + e.message);
        return;
      }

      try {
        const client = new RpcClient(normalizeRpcEndpoint(s.rpcEndpoint), sessionProxyAuthToken);
        setRpc(client);
        setTxBuilder(new TxBuilder(client));
        try {
          const status = await client.status();
          if (status.ok) setChainStatus(status.result);
          else setChainStatus(null);
        } catch (e) { setChainStatus(null); }
        try {
          const caps = await client.serverCapabilities();
          setChainCapabilities(caps);
        } catch (e) { setChainCapabilities(null); }
      } catch (e) {
        setError('RPC init failed: ' + e.message);
      }
    })();
  }, []);

  useEffect(() => {
    if (!rpc) return undefined;

    let disposed = false;
    let timer = null;

    const refresh = async () => {
      try {
        const [status, caps] = await Promise.all([
          rpc.status().catch(() => null),
          rpc.serverCapabilities().catch(() => null),
        ]);
        if (disposed) return;
        setChainStatus(status?.ok ? status.result : null);
        setChainCapabilities(caps?.ok ? caps : null);
      } catch (_) {
        if (!disposed) {
          setChainStatus(null);
          setChainCapabilities(null);
        }
      } finally {
        if (!disposed) timer = setTimeout(refresh, 5000);
      }
    };

    refresh();
    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
    };
  }, [rpc, rpcEpoch]);

  // --- Auto-lock ---
  const handleLock = useCallback(() => {
    clearSensitiveMemory();
    clearAutoLock();
    setWalletLiveSnapshot(null);
    setTab('locked');
  }, []);

  const handleActivity = useCallback(() => {
    if (getDecryptedSeed()) {
      resetAutoLock(handleLock);
    }
  }, [handleLock]);

  useEffect(() => {
    if (tab !== 'locked' && tab !== 'onboard') {
      ['click', 'keydown', 'touchstart'].forEach(ev => {
        window.addEventListener(ev, handleActivity);
      });
      return () => {
        ['click', 'keydown', 'touchstart'].forEach(ev => {
          window.removeEventListener(ev, handleActivity);
        });
      };
    }
  }, [tab, handleActivity]);

  const walletUnlocked = tab !== 'locked' && tab !== 'onboard';

  useEffect(() => {
    if (!rpc || !walletAddress || !walletUnlocked) {
      setWalletFeedStatus({ status: 'idle' });
      return;
    }

    let disposed = false;
    let activeSubscription = null;
    let retryTimer = null;

    const clearRetry = () => {
      if (retryTimer) {
        clearTimeout(retryTimer);
        retryTimer = null;
      }
    };

    const scheduleRetry = (delayMs = 2500) => {
      if (disposed) return;
      clearRetry();
      retryTimer = setTimeout(() => {
        retryTimer = null;
        subscribe();
      }, delayMs);
    };

    setWalletFeedStatus({ status: 'connecting' });

    const subscribe = async () => {
      if (disposed) return;
      clearRetry();
      setWalletFeedStatus((current) => ({
        ...current,
        status: current.status === 'live' ? 'live' : 'connecting',
        error: undefined,
      }));

      try {
        const subscription = await rpc.walletSubscribe({
          address: walletAddress,
          owner_public_key_hex: walletPublicKeyHex,
          asset: 'PFT',
          include_assets: true,
          owned_limit: FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,
          interval_ms: 1500,
        }, (snapshot, meta) => {
          if (disposed) return;
          setWalletLiveSnapshot(snapshot);
          setWalletFeedStatus({
            status: 'live',
            subscriptionId: meta.subscription_id,
            intervalMs: meta.interval_ms,
            lastUpdateMs: Date.now(),
          });
        });

        if (disposed) {
          subscription.unsubscribe();
          return;
        }
        activeSubscription = subscription;
        setWalletFeedStatus((current) => ({
          ...current,
          status: current.status === 'live' ? 'live' : 'connecting',
          subscriptionId: subscription.subscriptionId,
          intervalMs: subscription.intervalMs,
        }));
      } catch (e) {
        if (disposed) return;
        setWalletFeedStatus({
          status: 'error',
          error: e.message || 'wallet feed unavailable',
        });
        scheduleRetry();
      }
    };

    const offClose = rpc.onConnectionClose(() => {
      if (disposed) return;
      activeSubscription?.drop?.();
      activeSubscription = null;
      setWalletFeedStatus((current) => ({
        ...current,
        status: 'connecting',
        error: 'wallet feed reconnecting',
      }));
      scheduleRetry(500);
    });

    subscribe();

    return () => {
      disposed = true;
      clearRetry();
      offClose();
      if (activeSubscription) activeSubscription.unsubscribe();
    };
  }, [rpc, rpcEpoch, walletAddress, walletPublicKeyHex, walletUnlocked]);

  // --- Wallet creation ---
  const handleCreateWallet = async (seed, passphrase) => {
    if (creatingRef.current) return;
    creatingRef.current = true;
    try {
      const wasm = getWasm();
      const result = wasm.wallet_keygen(CHAIN_ID, seed, ACCOUNT_INDEX);
      const { address, backup_json } = result;

      const blob = await encryptVault(seed, passphrase);
      const metadata = { address, public_key_hex: result.public_key_hex, chain_id: CHAIN_ID, created_at: new Date().toISOString() };
      await saveVault('default', blob, metadata);

      setDecryptedState(seed, backup_json);
      setWalletAddress(address);
      setWalletPublicKeyHex(result.public_key_hex);
      resetAutoLock(handleLock);
      setTab('wallet');
      return { seed, address };
    } catch (e) {
      setError('Wallet creation failed: ' + e.message);
      throw e;
    } finally {
      creatingRef.current = false;
    }
  };

  // --- Wallet import ---
  const handleImportWallet = async (seedHex, passphrase) => {
    if (creatingRef.current) return;
    creatingRef.current = true;
    try {
      if (!/^[0-9a-f]{64}$/.test(seedHex)) {
        throw new Error('Seed must be 64 hex characters');
      }
      const wasm = getWasm();
      const result = wasm.wallet_keygen(CHAIN_ID, seedHex, ACCOUNT_INDEX);
      const { address, backup_json } = result;

      const blob = await encryptVault(seedHex, passphrase);
      const metadata = { address, public_key_hex: result.public_key_hex, chain_id: CHAIN_ID, created_at: new Date().toISOString() };
      await saveVault('default', blob, metadata);

      setDecryptedState(seedHex, backup_json);
      setWalletAddress(address);
      setWalletPublicKeyHex(result.public_key_hex);
      resetAutoLock(handleLock);
      setTab('wallet');
    } catch (e) {
      setError('Wallet import failed: ' + e.message);
      throw e;
    } finally {
      creatingRef.current = false;
    }
  };

  // --- Unlock ---
  const handleUnlock = async (passphrase) => {
    if (!passphrase) throw new Error('Passphrase required');
    const vault = await loadVault();
    if (!vault) throw new Error('No wallet found');

    try {
      const seed = await decryptVault(vault.blob, passphrase);
      const wasm = getWasm();
      const result = wasm.wallet_keygen(CHAIN_ID, seed, ACCOUNT_INDEX);
      if (result.address !== vault.metadata?.address) {
        const legacy = LEGACY_CHAIN_IDS
          .map(chainId => ({ chainId, result: wasm.wallet_keygen(chainId, seed, ACCOUNT_INDEX) }))
          .find(candidate => candidate.result.address === vault.metadata?.address);
        if (!legacy) throw new Error('Address mismatch — corrupt vault');

        const previousAddress = vault.metadata.address;
        const metadata = {
          ...vault.metadata,
          address: result.address,
          public_key_hex: result.public_key_hex,
          chain_id: CHAIN_ID,
          migrated_from_chain_id: legacy.chainId,
          migrated_from_address: previousAddress,
          migrated_at: new Date().toISOString(),
        };
        await saveVault('default', vault.blob, metadata);
        setWalletNotice(
          `This devnet wallet was upgraded from the retired ${legacy.chainId} signing domain. `
          + `Its current address is ${result.address}; request devnet PFT for this address. `
          + `The prior test-only address ${previousAddress} was derived with the stale domain and cannot sign for the current network; do not fund it again.`,
        );
      }
      setDecryptedState(seed, result.backup_json);
      setWalletAddress(result.address);
      setWalletPublicKeyHex(result.public_key_hex);
      resetAutoLock(handleLock);
      setTab('wallet');
    } catch (e) {
      throw e;
    }
  };

  // --- Remove wallet ---
  const handleRemoveWallet = async () => {
    clearSensitiveMemory();
    clearAutoLock();
    await removeVault('default');
    setWalletAddress(null);
    setWalletPublicKeyHex(null);
    setWalletLiveSnapshot(null);
    setTab('onboard');
  };

  // --- Settings save ---
  const handleSaveSettings = async (newSettings) => {
    const nextProxyAuthToken = controlledLocalSession
      ? proxyAuthToken
      : String(newSettings.proxyAuthToken || '');
    if (controlledLocalSession) {
      sessionStorage.removeItem(PROXY_AUTH_SESSION_KEY);
    } else if (nextProxyAuthToken) {
      sessionStorage.setItem(PROXY_AUTH_SESSION_KEY, nextProxyAuthToken);
    } else {
      sessionStorage.removeItem(PROXY_AUTH_SESSION_KEY);
    }
    setProxyAuthToken(nextProxyAuthToken);
    const { proxyAuthToken: _sessionOnlyProxyAuthToken, ...persistentSettings } = newSettings;
    const normalizedSettings = {
      ...persistentSettings,
      rpcEndpoint: normalizeRpcEndpoint(persistentSettings.rpcEndpoint),
    };
    await saveSettings(normalizedSettings);
    setSettings(normalizedSettings);
    setAutoLockMinutes(normalizedSettings.autoLockMinutes || 15);
    if (rpc) {
      rpc.setProxyAuthToken(nextProxyAuthToken);
      rpc.setUrl(normalizedSettings.rpcEndpoint);
      setWalletLiveSnapshot(null);
      setRpcEpoch(value => value + 1);
    }
    if (normalizedSettings.swapServerUrl && swapServer) {
      swapServer.setProxyAuthToken(nextProxyAuthToken);
      swapServer.setUrl(normalizedSettings.swapServerUrl);
    }
    if (rpc) {
      try {
        const status = await rpc.status();
        if (status.ok) setChainStatus(status.result);
        else setChainStatus(null);
      } catch (e) { setChainStatus(null); }
      try {
        const caps = await rpc.serverCapabilities();
        setChainCapabilities(caps);
      } catch (e) { setChainCapabilities(null); }
    }
  };

  // --- Import backup ---
  const handleImportBackup = async (backupData, passphrase) => {
    if (!backupData.vault || !backupData.metadata) {
      throw new Error('Invalid backup file structure');
    }
    if (!isValidAddress(backupData.metadata.address)) {
      throw new Error('Invalid address in backup file');
    }
    await saveVault('default', backupData.vault, backupData.metadata);
    setWalletAddress(backupData.metadata.address);
    setWalletPublicKeyHex(backupData.metadata.public_key_hex || null);
    clearSensitiveMemory();
    setTab('locked');
  };

  const showToast = (msg) => {
    setToast(msg);
    setTimeout(() => setToast(''), 2000);
  };

  const go = (next, payload) => {
    if (next === 'navDetail') { setCoinId(payload); setTab('navDetail'); return; }
    if (next === 'market') {
      if (payload?.marketKey) setMarketKey(navcoinMarketByKey(payload.marketKey).key);
      setTab('market');
      return;
    }
    if (next === 'send' && payload?.sendSource) setSendSource(payload.sendSource);
    setTab(next);
  };

  // --- Onboard / Locked ---
  if (tab === 'onboard') {
    return (
      <div className="pf-root">
        {error && <div className="pf-error" style={{ position: 'fixed', top: 0, left: 0, right: 0, zIndex: 200, borderRadius: 0 }}>{error}</div>}
        <Onboard
          wasmReady={wasmReady}
          onCreate={handleCreateWallet}
          onImport={handleImportWallet}
          existingVault={walletAddress !== null}
        />
        {toast && <div className="pf-toast">{toast}</div>}
      </div>
    );
  }

  if (tab === 'locked') {
    return (
      <div className="pf-root">
        {error && <div className="pf-error" style={{ position: 'fixed', top: 0, left: 0, right: 0, zIndex: 200, borderRadius: 0 }}>{error}</div>}
        <LockScreen
          address={walletAddress}
          onUnlock={handleUnlock}
          onRemove={handleRemoveWallet}
          wasmReady={wasmReady}
          chainStatus={chainStatus}
        />
        {toast && <div className="pf-toast">{toast}</div>}
      </div>
    );
  }

  // --- Wallet unlocked — show shell with sidebar/bottomnav ---
  const backupJson = getDecryptedBackup();
  const seed = getDecryptedSeed();
  const shortAddr = walletAddress ? truncateMiddle(walletAddress, 8) : '';

  const online = chainStatus && chainStatus.block_height > 0;
  const writable = chainCapabilities && !chainCapabilities.read_only;
  // On the WAN devnet, blocks are produced on-demand. A stale last_run_unix
  // with no pending mempool is normal — the chain is just idle, not stalled.
  // Only show "stalled" if there are pending mempool entries not being processed.
  const mempoolStalled = online && chainStatus.mempool_pending > 0 &&
    chainCapabilities && chainCapabilities.last_run_unix &&
    Date.now() / 1000 - chainCapabilities.last_run_unix > 300;
  let dotClass = 'off';
  let healthLabel = 'offline';
  if (online) {
    if (mempoolStalled) {
      dotClass = 'stalled';
      healthLabel = `stalled @ ${chainStatus.block_height}`;
    } else if (writable) {
      dotClass = '';
      healthLabel = `writable @ ${chainStatus.block_height}`;
    } else {
      dotClass = 'off';
      healthLabel = `read-only @ ${chainStatus.block_height}`;
    }
  }

  return (
    <div className="pf-root">
      {error && <div className="pf-error" style={{ position: 'fixed', top: 0, left: 0, right: 0, zIndex: 200, borderRadius: 0 }}>{error}</div>}
      <div className="pf-shell">
        <aside className="pf-sidebar">
          <div className="pf-brand">
            <div className="pf-mark">PF</div>
            <div>
              <div style={{ fontWeight: 680, fontSize: 16, letterSpacing: '-0.02em' }}>PostFiat</div>
              <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--dim)' }}>self-custody</div>
            </div>
          </div>
          <button className="pf-chip" style={{ margin: '0 8px 14px' }}
            onClick={() => { navigator.clipboard?.writeText(walletAddress || ''); showToast('Address copied'); }}>
            <span className={`pf-dot ${dotClass}`} />
            {shortAddr || 'wallet pending'}
          </button>
          {NAV_ITEMS.map((x) => (
            <button key={x.id} className={`pf-nav${isOn(tab, x.id) ? ' on' : ''}`} onClick={() => go(x.id)}>
              <span className="pf-nav-badge">{x.label[0]}</span>{x.label}
            </button>
          ))}
          <button className="pf-nav" onClick={handleLock} style={{ color: 'var(--dim)' }}>
            <span className="pf-nav-badge">L</span> Lock
          </button>
          <div className="pf-ledger">
            {chainCapabilities?.chain_id || 'connecting…'}<br />
            {online ? `height ${chainStatus.block_height}` : 'rpc offline'}<br />
            {chainCapabilities?.validator_count ? `${chainCapabilities.validator_count} validators` : ''}
          </div>
          <div style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--dim)', padding: '0 8px 8px', opacity: 0.7 }}>
            v{import.meta.env.VITE_APP_VERSION || 'dev'}
          </div>
        </aside>

        <main className="pf-main">
          <div className="pf-topbar">
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <div className="pf-mark" style={{ width: 30, height: 30, fontSize: 12 }}>PF</div>
              <div style={{ fontWeight: 680, fontSize: 15, letterSpacing: '-0.02em' }}>PostFiat</div>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <span className={`pf-dot ${dotClass}`} />
              <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--muted)' }}>{healthLabel}</span>
              <button className="pf-chip" onClick={handleLock} style={{ fontSize: 11, letterSpacing: '0.08em' }}>LOCK</button>
            </div>
          </div>

          {walletNotice && (
            <div className="pf-notice" style={{ margin: '12px 18px 0', display: 'flex', gap: 12, alignItems: 'center', justifyContent: 'space-between' }}>
              <span>{walletNotice}</span>
              <button className="pf-ghost" onClick={() => setWalletNotice('')}>Dismiss</button>
            </div>
          )}

          {tab === 'wallet' && (
            <WalletHome
              rpc={rpc}
              txBuilder={txBuilder}
              backupJson={backupJson}
              address={walletAddress}
              publicKeyHex={walletPublicKeyHex}
              chainStatus={chainStatus}
              chainCapabilities={chainCapabilities}
              proxyAuthToken={proxyAuthToken}
              liveSnapshot={walletLiveSnapshot}
              walletFeedStatus={walletFeedStatus}
              onCopy={showToast}
              go={go}
              visible={tab === 'wallet'}
            />
          )}
          {tab === 'send' && (
            <Send
              rpc={rpc}
              txBuilder={txBuilder}
              backupJson={backupJson}
              address={walletAddress}
              publicKeyHex={walletPublicKeyHex}
              initialSource={sendSource}
              onToast={showToast}
              chainCapabilities={chainCapabilities}
              liveSnapshot={walletLiveSnapshot}
              walletFeedStatus={walletFeedStatus}
              visible={tab === 'send'}
            />
          )}
          {tab === 'swap' && (
            <Swap
              market={navcoinMarketByKey(marketKey)}
              rpc={rpc}
              txBuilder={txBuilder}
              backupJson={backupJson}
              address={walletAddress}
              swapServer={swapServer}
              onToast={showToast}
              onNavigate={go}
              chainCapabilities={chainCapabilities}
              liveSnapshot={walletLiveSnapshot}
              walletFeedStatus={walletFeedStatus}
            />
          )}
          {tab === 'market' && (
            <NavcoinMarket
              market={navcoinMarketByKey(marketKey)}
              markets={NAVCOIN_MARKETS}
              onSelectMarket={setMarketKey}
              rpc={rpc}
              txBuilder={txBuilder}
              backupJson={backupJson}
              address={walletAddress}
              chainStatus={chainStatus}
              chainCapabilities={chainCapabilities}
              proxyAuthToken={proxyAuthToken}
              liveSnapshot={walletLiveSnapshot}
              onToast={showToast}
              onNavigate={go}
            />
          )}
          {tab === 'bridge' && (
            <Bridge
              address={walletAddress}
              rpc={rpc}
              proxyAuthToken={proxyAuthToken}
            />
          )}
          {tab === 'nav' && (
            <NavList
              rpc={rpc}
              address={walletAddress}
              go={go}
            />
          )}
          {tab === 'navDetail' && (
            <NavDetail
              id={coinId}
              rpc={rpc}
              address={walletAddress}
              go={go}
              onToast={showToast}
            />
          )}
          {tab === 'more' && (
            <More
              settings={settings}
              proxyAuthToken={proxyAuthToken}
              controlledLocalSession={controlledLocalSession}
              onSave={handleSaveSettings}
              onRemove={handleRemoveWallet}
              onImportBackup={handleImportBackup}
              onExportBackup={async () => {
                const vault = await loadVault();
                if (!vault) return;
                const data = { vault: vault.blob, metadata: vault.metadata, exported_at: new Date().toISOString() };
                const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = `postfiat-wallet-backup-${Date.now()}.json`;
                a.click();
                URL.revokeObjectURL(url);
              }}
            />
          )}

          <nav className="pf-bottomnav">
            {NAV_ITEMS.map((x) => (
              <button key={x.id} className={`pf-bnav${isOn(tab, x.id) ? ' on' : ''}`} onClick={() => go(x.id)}>
                <span className="pf-bnav-badge">{x.label[0]}</span>
                <span className="pf-bnav-l">{x.label}</span>
              </button>
            ))}
          </nav>
        </main>
      </div>
      {toast && <div className="pf-toast">{toast}</div>}
    </div>
  );
}
