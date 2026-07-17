import React, { useState, useRef } from 'react';
import { isValidAddress } from '../lib/utils.js';
import { defaultRpcEndpoint, defaultSwapServerUrl, normalizeRpcEndpoint } from '../lib/vault.js';

function rpcSelection(endpoint) {
  const normalized = normalizeRpcEndpoint(endpoint || '');
  const defaultEndpoint = defaultRpcEndpoint();
  if (!endpoint || normalized === defaultEndpoint) {
    return { selected: '', custom: '' };
  }
  if (normalized === 'ws://localhost:8080') {
    return { selected: normalized, custom: '' };
  }
  return { selected: 'custom', custom: normalized };
}

export default function More({ settings, proxyAuthToken = '', onSave, onRemove, onImportBackup, onExportBackup }) {
  const initialRpc = rpcSelection(settings?.rpcEndpoint);
  const [rpcEndpoint, setRpcEndpoint] = useState(initialRpc.selected);
  const [customRpc, setCustomRpc] = useState(initialRpc.custom);
  const [autoLock, setAutoLock] = useState(settings?.autoLockMinutes || 15);
  const [swapServerUrl, setSwapServerUrl] = useState(settings?.swapServerUrl || defaultSwapServerUrl());
  const [proxyToken, setProxyToken] = useState(proxyAuthToken);
  const [success, setSuccess] = useState('');
  const [error, setError] = useState('');
  const [confirmRemove, setConfirmRemove] = useState(false);
  const fileInputRef = useRef(null);

  const handleSave = () => {
    const endpoint = rpcEndpoint === 'custom' ? customRpc : rpcEndpoint;
    onSave({
      rpcEndpoint: endpoint,
      autoLockMinutes: autoLock,
      swapServerUrl,
      proxyAuthToken: proxyToken,
    });
    setSuccess('Settings saved');
    setError('');
    setTimeout(() => setSuccess(''), 2000);
  };

  const handleExport = () => {
    onExportBackup();
    setSuccess('Backup exported');
    setTimeout(() => setSuccess(''), 2000);
  };

  const handleImportClick = () => {
    fileInputRef.current?.click();
  };

  const handleImportFile = async (e) => {
    const file = e.target.files[0];
    if (!file) return;
    setError(''); setSuccess('');
    try {
      const text = await file.text();
      const data = JSON.parse(text);
      if (!data.vault || !data.metadata) {
        setError('Invalid backup file — missing vault or metadata');
        return;
      }
      if (!isValidAddress(data.metadata.address)) {
        setError('Invalid address in backup');
        return;
      }
      if (!confirm(`Import wallet ${data.metadata.address}? This will overwrite any existing wallet.`)) {
        return;
      }
      await onImportBackup(data, null);
      setSuccess('Backup imported. Wallet locked — unlock with your passphrase.');
      setTimeout(() => setSuccess(''), 3000);
    } catch (e) {
      setError('Import failed: ' + e.message);
    }
    e.target.value = '';
  };

  const handleRemove = () => {
    if (!confirmRemove) {
      setConfirmRemove(true);
      return;
    }
    onRemove();
    setConfirmRemove(false);
  };

  const Field = ({ label, children }) => (
    <div style={{ display: 'grid', gap: 7 }}>
      <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--dim)', letterSpacing: '0.06em', textTransform: 'uppercase' }}>{label}</span>
      {children}
    </div>
  );

  return (
    <div className="pf-page">
      <div className="pf-stage-inner" style={{ maxWidth: 980 }}>
        <div className="pf-eyebrow">Settings</div>
        <h1 className="pf-h1" style={{ marginBottom: 22 }}>More</h1>

        <div className="pf-even" style={{ alignItems: 'start' }}>
          {/* network */}
          <div className="pf-card" style={{ display: 'grid', gap: 16 }}>
            <div className="pf-eyebrow">Network</div>
            <Field label="RPC endpoint">
              <select className="pf-select" value={rpcEndpoint} onChange={e => setRpcEndpoint(e.target.value)}>
                <option value="">Same-origin /rpc (default)</option>
                <option value="ws://localhost:8080">Local Proxy (localhost:8080)</option>
                <option value="custom">Custom…</option>
              </select>
            </Field>
            {rpcEndpoint === 'custom' && (
              <input className="pf-input" placeholder="ws://your-host:port" value={customRpc} onChange={e => setCustomRpc(e.target.value)} />
            )}
            <Field label="Swap server">
              <input className="pf-input" value={swapServerUrl} onChange={e => setSwapServerUrl(e.target.value)} />
            </Field>
            <Field label="Proxy mutation token (session only)">
              <input
                className="pf-input"
                type="password"
                autoComplete="off"
                placeholder="Required for sends, swaps, bridge, and funding"
                value={proxyToken}
                onChange={e => setProxyToken(e.target.value)}
              />
            </Field>
            <p style={{ margin: 0, color: 'var(--dim)', fontSize: 12, lineHeight: 1.5 }}>
              Bridge deposits require a reviewed build-time vault binding. Money destinations cannot be changed from wallet settings.
            </p>
          </div>

          {/* wallet */}
          <div className="pf-card" style={{ display: 'grid', gap: 16 }}>
            <div className="pf-eyebrow">Wallet</div>
            <Field label="Auto-lock (minutes)">
              <select className="pf-select" value={autoLock} onChange={e => setAutoLock(parseInt(e.target.value, 10))}>
                <option value="5">5</option>
                <option value="15">15</option>
                <option value="30">30</option>
                <option value="60">60</option>
              </select>
            </Field>
            <button className="pf-primary" onClick={handleSave}>Save settings</button>
            <div className="pf-even">
              <button className="pf-ghost" onClick={handleExport}>Export backup</button>
              <button className="pf-ghost" onClick={handleImportClick}>Import backup</button>
            </div>
            <input type="file" ref={fileInputRef} accept=".json" style={{ display: 'none' }} onChange={handleImportFile} />
            <button style={{
              width: '100%', background: 'var(--red-soft)', border: '1px solid rgba(239,106,106,0.3)',
              color: 'var(--red)', borderRadius: 12, padding: 14, fontSize: 14, fontWeight: 600, cursor: 'pointer',
            }} onClick={handleRemove}>
              {confirmRemove ? 'Click again to confirm removal' : 'Remove wallet'}
            </button>
          </div>
        </div>

        {success && <div className="pf-success" style={{ marginTop: 16 }}>{success}</div>}
        {error && <div className="pf-error" style={{ marginTop: 16 }}>{error}</div>}
      </div>
    </div>
  );
}
