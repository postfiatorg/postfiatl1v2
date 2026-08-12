import React, { useRef, useState } from 'react';
import { CHAIN_ID, ACCOUNT_INDEX, isValidAddress } from '../lib/utils.js';
import { getWasm } from '../lib/wasm-loader.js';

export default function Onboard({ wasmReady, onCreate, onImport, onImportBackup, existingVault }) {
  const [mode, setMode] = useState('none');
  const [seed, setSeed] = useState('');
  const [address, setAddress] = useState('');
  const [passphrase, setPassphrase] = useState('');
  const [passphraseConfirm, setPassphraseConfirm] = useState('');
  const [seedSaved, setSeedSaved] = useState(false);
  const [importSeed, setImportSeed] = useState('');
  const [showImportSeed, setShowImportSeed] = useState(false);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const backupInputRef = useRef(null);

  const normalizedImportSeed = () => importSeed.trim().toLowerCase();

  const handleCreateClick = async () => {
    if (!wasmReady) { setError('WASM not ready'); return; }
    try {
      const wasm = getWasm();
      const newSeed = wasm.random_master_seed();
      const result = wasm.wallet_keygen(CHAIN_ID, newSeed, ACCOUNT_INDEX);
      setSeed(newSeed);
      setAddress(result.address);
      setMode('create');
      setSeedSaved(false);
      setPassphrase('');
      setPassphraseConfirm('');
      setError('');
    } catch (e) {
      setError('Keygen failed: ' + e.message);
    }
  };

  const handleImportClick = async () => {
    setMode('import');
    setError('');
    setImportSeed('');
    setPassphrase('');
    setPassphraseConfirm('');
  };

  const handleImportSeed = async () => {
    setError('');
    const seedHex = normalizedImportSeed();
    if (!/^[0-9a-f]{64}$/.test(seedHex)) {
      setError('Seed must be exactly 64 hex characters (0-9, a-f)');
      return;
    }
    if (!wasmReady) { setError('WASM not ready'); return; }
    try {
      const wasm = getWasm();
      const result = wasm.wallet_keygen(CHAIN_ID, seedHex, ACCOUNT_INDEX);
      setImportSeed(seedHex);
      setSeed(seedHex);
      setAddress(result.address);
      setMode('import-confirm');
      setPassphrase('');
      setPassphraseConfirm('');
    } catch (e) {
      setError('Invalid seed: ' + e.message);
    }
  };

  const handleImportBackupFile = async (event) => {
    const file = event.target.files?.[0];
    if (!file) return;
    setError('');
    setBusy(true);
    try {
      const backup = JSON.parse(await file.text());
      if (!backup?.vault || !backup?.metadata) {
        throw new Error('Invalid backup file — missing encrypted vault or metadata');
      }
      if (
        typeof backup.vault.salt !== 'string'
        || typeof backup.vault.iv !== 'string'
        || typeof backup.vault.ciphertext !== 'string'
      ) {
        throw new Error('Invalid backup file — encrypted vault is malformed');
      }
      if (!isValidAddress(backup.metadata.address)) {
        throw new Error('Invalid address in backup file');
      }
      await onImportBackup(backup, null);
    } catch (failure) {
      setError(`Backup import failed: ${failure?.message || 'unknown error'}`);
    } finally {
      event.target.value = '';
      setBusy(false);
    }
  };

  const handleSave = async () => {
    setError('');
    if (passphrase.length < 10) { setError('Passphrase must be at least 10 characters'); return; }
    if (passphrase !== passphraseConfirm) { setError('Passphrases do not match'); return; }
    if (mode === 'create' && !seedSaved) { setError('Please confirm you saved your seed'); return; }

    setBusy(true);
    try {
      if (mode === 'create') {
        await onCreate(seed, passphrase);
      } else if (mode === 'import-confirm') {
        await onImport(seed, passphrase);
      }
      setPassphrase('');
      setPassphraseConfirm('');
      setSeed('');
      setImportSeed('');
    } catch (e) {
      setError(e.message);
    } finally {
      setBusy(false);
    }
  };

  if (!wasmReady) {
    return (
      <div style={{ display: 'grid', placeItems: 'center', height: '100vh', padding: 24, gap: 20 }}>
        <div className="pf-mark" style={{ width: 56, height: 56, borderRadius: 16, fontSize: 18 }}>PF</div>
        <div style={{ fontFamily: 'var(--mono)', fontSize: 13, color: 'var(--dim)' }}>Loading WASM module…</div>
      </div>
    );
  }

  return (
    <div style={{ display: 'grid', placeItems: 'center', height: '100vh', padding: 24 }}>
      <div style={{ width: '100%', maxWidth: 460, display: 'grid', gap: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, justifyContent: 'center' }}>
          <div className="pf-mark" style={{ width: 48, height: 48, borderRadius: 14, fontSize: 16 }}>PF</div>
          <div>
            <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.02em' }}>PostFiat</div>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--dim)' }}>self-custody wallet</div>
          </div>
        </div>

        {mode === 'none' && (
          <div className="pf-card" style={{ display: 'grid', gap: 12 }}>
            <div style={{ fontSize: 14, color: 'var(--muted)' }}>
              {existingVault
                ? 'A wallet already exists. Unlock it or remove it first from Settings.'
                : 'Choose how to open your PostFiat wallet on this browser. Restoring an existing wallet does not move any funds.'}
            </div>
            <button className="pf-primary" onClick={handleImportClick}>Restore from recovery seed</button>
            <button className="pf-ghost" onClick={() => backupInputRef.current?.click()} disabled={busy}>
              {busy ? 'Opening backup…' : 'Restore from encrypted backup'}
            </button>
            <button className="pf-ghost" onClick={handleCreateClick}>Create a new wallet</button>
            <input
              ref={backupInputRef}
              type="file"
              accept=".json,application/json"
              style={{ display: 'none' }}
              onChange={handleImportBackupFile}
            />
            <div style={{ fontSize: 12, color: 'var(--dim)', lineHeight: 1.5 }}>
              A recovery seed is the 64-character secret created with the wallet. An encrypted backup is a PostFiat JSON file and requires its unlock passphrase.
            </div>
          </div>
        )}

        {mode === 'create' && (
          <div className="pf-card" style={{ display: 'grid', gap: 12 }}>
            <div className="pf-seed-warning">⚠ SAVE YOUR SEED — YOU WILL LOSE FUNDS WITHOUT IT</div>
            <div className="pf-seed-display">{seed}</div>
            <div className="pf-eyebrow">Derived Address</div>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 13, color: 'var(--text)' }}>{address}</div>
            <label className="pf-checkbox">
              <input type="checkbox" checked={seedSaved} onChange={e => setSeedSaved(e.target.checked)} />
              <span>I have saved my seed in a secure location</span>
            </label>
            <input className="pf-input" type="password" placeholder="Encryption passphrase (min 10 chars)"
              value={passphrase} onChange={e => setPassphrase(e.target.value)} />
            <input className="pf-input" type="password" placeholder="Confirm passphrase"
              value={passphraseConfirm} onChange={e => setPassphraseConfirm(e.target.value)} />
            <button className="pf-primary" onClick={handleSave} disabled={busy || !seedSaved}>
              {busy ? 'Creating…' : 'Create Wallet'}
            </button>
            <button className="pf-ghost" onClick={() => setMode('none')} disabled={busy}>Back</button>
          </div>
        )}

        {mode === 'import' && (
          <div className="pf-card" style={{ display: 'grid', gap: 12 }}>
            <div className="pf-eyebrow">Restore from recovery seed</div>
            <div className="pf-warning" style={{ fontSize: 12, lineHeight: 1.5 }}>
              Enter this secret only on the PostFiat wallet you intended to open. PostFiat support will never ask you to send it. Validation happens in this browser.
            </div>
            <p style={{ color: 'var(--muted)', fontSize: 13, lineHeight: 1.5 }}>
              This is the 64-character hexadecimal recovery seed—not the passphrase used to unlock a wallet already stored in your browser.
            </p>
            <input className="pf-input" type={showImportSeed ? 'text' : 'password'} placeholder="64-character recovery seed"
              value={importSeed} onChange={e => setImportSeed(e.target.value)}
              spellCheck="false" autoCapitalize="none" autoCorrect="off"
              style={{ fontFamily: 'var(--mono)', fontSize: 12 }} />
            <label className="pf-checkbox">
              <input type="checkbox" checked={showImportSeed} onChange={e => setShowImportSeed(e.target.checked)} />
              <span>Show recovery seed</span>
            </label>
            <button className="pf-primary" onClick={handleImportSeed}>Continue and preview address</button>
            <button className="pf-ghost" onClick={() => setMode('none')}>Back</button>
          </div>
        )}

        {mode === 'import-confirm' && (
          <div className="pf-card" style={{ display: 'grid', gap: 12 }}>
            <div className="pf-eyebrow">Confirm the wallet address</div>
            <p style={{ color: 'var(--muted)', fontSize: 13, lineHeight: 1.5 }}>
              Verify this is the public address you expected. The new passphrase below encrypts this wallet in this browser; it is not your recovery seed.
            </p>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 13, color: 'var(--text)' }}>{address}</div>
            <input className="pf-input" type="password" placeholder="Encryption passphrase (min 10 chars)"
              autoComplete="new-password" value={passphrase} onChange={e => setPassphrase(e.target.value)} />
            <input className="pf-input" type="password" placeholder="Confirm passphrase"
              autoComplete="new-password" value={passphraseConfirm} onChange={e => setPassphraseConfirm(e.target.value)} />
            <button className="pf-primary" onClick={handleSave} disabled={busy}>
              {busy ? 'Restoring…' : 'Restore this wallet'}
            </button>
            <button className="pf-ghost" onClick={() => setMode('import')} disabled={busy}>Back</button>
          </div>
        )}

        {error && <div className="pf-error">{error}</div>}
      </div>
    </div>
  );
}
