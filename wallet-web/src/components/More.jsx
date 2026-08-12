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

export default function More({
  settings,
  proxyAuthToken = '',
  controlledLocalSession = false,
  onSave,
  onRemove,
  onChangePassphrase,
  onImportBackup,
  onExportBackup,
}) {
  const initialRpc = rpcSelection(settings?.rpcEndpoint);
  const [rpcEndpoint, setRpcEndpoint] = useState(initialRpc.selected);
  const [customRpc, setCustomRpc] = useState(initialRpc.custom);
  const [autoLock, setAutoLock] = useState(settings?.autoLockMinutes || 15);
  const [swapServerUrl, setSwapServerUrl] = useState(settings?.swapServerUrl || defaultSwapServerUrl());
  const [proxyToken, setProxyToken] = useState(proxyAuthToken);
  const [success, setSuccess] = useState('');
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [newPassphrase, setNewPassphrase] = useState('');
  const [confirmPassphrase, setConfirmPassphrase] = useState('');
  const fileInputRef = useRef(null);

  const handleSave = async () => {
    const endpoint = rpcEndpoint === 'custom' ? customRpc : rpcEndpoint;
    setError('');
    setSaving(true);
    try {
      await onSave({
        rpcEndpoint: endpoint,
        autoLockMinutes: autoLock,
        swapServerUrl,
        proxyAuthToken: proxyToken,
      });
      setSuccess('Settings saved');
      setTimeout(() => setSuccess(''), 2000);
    } catch (failure) {
      setError(`Settings save failed: ${failure?.message || 'unknown error'}`);
    } finally {
      setSaving(false);
    }
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

  const handleChangePassphrase = async () => {
    setError(''); setSuccess('');
    if (newPassphrase.length < 10) { setError('New passphrase must be at least 10 characters'); return; }
    if (newPassphrase !== confirmPassphrase) { setError('New passphrases do not match'); return; }
    setSaving(true);
    try {
      await onChangePassphrase(newPassphrase);
      setNewPassphrase('');
      setConfirmPassphrase('');
      setSuccess('Wallet passphrase changed. Existing backup files still require their original passphrase; download a new backup.');
    } catch (failure) {
      setError(`Passphrase change failed: ${failure?.message || 'unknown error'}`);
    } finally {
      setSaving(false);
    }
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
        <div className="pf-eyebrow">Wallet</div>
        <h1 className="pf-h1" style={{ marginBottom: 8 }}>Settings</h1>
        <p style={{ color: 'var(--muted)', fontSize: 13.5, lineHeight: 1.55, marginBottom: 22 }}>
          Protect and back up this browser's encrypted wallet. Network configuration is hidden under Advanced because changing it can disconnect the wallet.
        </p>

        <div className="pf-even" style={{ alignItems: 'start' }}>
          {/* security and recovery */}
          <div className="pf-card" style={{ display: 'grid', gap: 16 }}>
            <div className="pf-eyebrow">Security & recovery</div>
            <Field label="Auto-lock (minutes)">
              <select className="pf-select" value={autoLock} onChange={e => setAutoLock(parseInt(e.target.value, 10))}>
                <option value="5">5</option>
                <option value="15">15</option>
                <option value="30">30</option>
                <option value="60">60</option>
              </select>
            </Field>
            <button className="pf-primary" onClick={handleSave} disabled={saving}>
              {saving ? 'Saving…' : 'Save auto-lock setting'}
            </button>
            <div style={{ borderTop: '1px solid var(--border-soft)', paddingTop: 14, display: 'grid', gap: 10 }}>
              <strong style={{ fontSize: 14 }}>Change unlock passphrase</strong>
              <p style={{ margin: 0, color: 'var(--muted)', fontSize: 12.5, lineHeight: 1.5 }}>
                This re-encrypts the wallet stored in this browser. It does not change the wallet address or recovery seed.
              </p>
              <input className="pf-input" type="password" autoComplete="new-password" placeholder="New passphrase (min 10 chars)" value={newPassphrase} onChange={e => setNewPassphrase(e.target.value)} />
              <input className="pf-input" type="password" autoComplete="new-password" placeholder="Confirm new passphrase" value={confirmPassphrase} onChange={e => setConfirmPassphrase(e.target.value)} />
              <button className="pf-ghost" onClick={handleChangePassphrase} disabled={saving || !newPassphrase || !confirmPassphrase}>Change passphrase</button>
            </div>
            <div style={{ borderTop: '1px solid var(--border-soft)', paddingTop: 14, display: 'grid', gap: 10 }}>
              <strong style={{ fontSize: 14 }}>Encrypted backup</strong>
              <p style={{ margin: 0, color: 'var(--muted)', fontSize: 12.5, lineHeight: 1.5 }}>
                The backup contains the encrypted recovery material and public wallet metadata. Store it somewhere safe; restoring it requires the wallet passphrase.
              </p>
            </div>
            <div className="pf-even">
              <button className="pf-ghost" onClick={handleExport}>Download encrypted backup</button>
              <button className="pf-ghost" onClick={handleImportClick}>Restore a backup</button>
            </div>
            <input type="file" ref={fileInputRef} accept=".json" style={{ display: 'none' }} onChange={handleImportFile} />
            <button style={{
              width: '100%', background: 'var(--red-soft)', border: '1px solid rgba(239,106,106,0.3)',
              color: 'var(--red)', borderRadius: 12, padding: 14, fontSize: 14, fontWeight: 600, cursor: 'pointer',
            }} onClick={handleRemove}>
              {confirmRemove ? 'Confirm: remove local wallet only' : 'Remove wallet from this browser'}
            </button>
            {confirmRemove && (
              <div className="pf-warning" style={{ fontSize: 12, lineHeight: 1.5 }}>
                This does not move or delete on-chain funds. You will need the recovery seed or an encrypted backup and its passphrase to regain access.
              </div>
            )}
          </div>

          {/* network */}
          <div className="pf-card" style={{ display: 'grid', gap: 16 }}>
            <div className="pf-eyebrow">Connection</div>
            <div className="pf-success">Wallet services are connected for this browser session.</div>
            <p style={{ margin: 0, color: 'var(--muted)', fontSize: 12.5, lineHeight: 1.5 }}>
              The wallet verifies active bridge and NAV routes before asking you to sign. Route destinations cannot be changed from this screen.
            </p>
            <details style={{ borderTop: '1px solid var(--border-soft)', paddingTop: 14 }}>
              <summary style={{ cursor: 'pointer', fontWeight: 650, fontSize: 13 }}>Advanced network settings</summary>
              <div style={{ display: 'grid', gap: 14, paddingTop: 16 }}>
                <div className="pf-warning" style={{ fontSize: 12 }}>Changing these values can disconnect the wallet. Leave them unchanged unless you operate the network endpoint.</div>
                <Field label="RPC endpoint">
                  <select className="pf-select" value={rpcEndpoint} onChange={e => setRpcEndpoint(e.target.value)}>
                    <option value="">Automatic (recommended)</option>
                    <option value="ws://localhost:8080">Local endpoint</option>
                    <option value="custom">Custom…</option>
                  </select>
                </Field>
                {rpcEndpoint === 'custom' && (
                  <input className="pf-input" placeholder="wss://your-host/rpc" value={customRpc} onChange={e => setCustomRpc(e.target.value)} />
                )}
                <Field label="Optional route-status service">
                  <input className="pf-input" value={swapServerUrl} onChange={e => setSwapServerUrl(e.target.value)} />
                </Field>
                {!controlledLocalSession && (
                  <Field label="Session access token">
                    <input className="pf-input" type="password" autoComplete="off" value={proxyToken} onChange={e => setProxyToken(e.target.value)} />
                  </Field>
                )}
                <button className="pf-ghost" onClick={handleSave} disabled={saving}>
                  {saving ? 'Saving…' : 'Save advanced settings'}
                </button>
              </div>
            </details>
          </div>
        </div>

        {success && <div className="pf-success" style={{ marginTop: 16 }}>{success}</div>}
        {error && <div className="pf-error" style={{ marginTop: 16 }}>{error}</div>}
      </div>
    </div>
  );
}
