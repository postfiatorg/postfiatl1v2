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
                : 'No wallet found. Create a new self-custody wallet or import an existing seed.'}
            </div>
            <button className="pf-primary" onClick={handleCreateClick}>Create Wallet</button>
            <button className="pf-ghost" onClick={handleImportClick}>Import Wallet</button>
            <button className="pf-ghost" onClick={() => backupInputRef.current?.click()} disabled={busy}>
              {busy ? 'Importing…' : 'Import Encrypted Backup'}
            </button>
            <input
              ref={backupInputRef}
              type="file"
              accept=".json,application/json"
              style={{ display: 'none' }}
              onChange={handleImportBackupFile}
            />
            <div style={{ fontSize: 12, color: 'var(--dim)', lineHeight: 1.5 }}>
              Use an exported wallet backup when you have its encryption passphrase but not the raw seed.
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
          </div>
        )}

        {mode === 'import' && (
          <div className="pf-card" style={{ display: 'grid', gap: 12 }}>
            <div className="pf-eyebrow">Paste your 64-char hex master seed</div>
            <input className="pf-input" placeholder="e.g. a1b2c3… (64 hex chars)"
              value={importSeed} onChange={e => setImportSeed(e.target.value)}
              spellCheck="false" autoCapitalize="none" autoCorrect="off"
              style={{ fontFamily: 'var(--mono)', fontSize: 12 }} />
            <button className="pf-primary" onClick={handleImportSeed}>Validate Seed</button>
          </div>
        )}

        {mode === 'import-confirm' && (
          <div className="pf-card" style={{ display: 'grid', gap: 12 }}>
            <div className="pf-eyebrow">Imported seed derives to address</div>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 13, color: 'var(--text)' }}>{address}</div>
            <input className="pf-input" type="password" placeholder="Encryption passphrase (min 10 chars)"
              autoComplete="new-password" value={passphrase} onChange={e => setPassphrase(e.target.value)} />
            <input className="pf-input" type="password" placeholder="Confirm passphrase"
              autoComplete="new-password" value={passphraseConfirm} onChange={e => setPassphraseConfirm(e.target.value)} />
            <button className="pf-primary" onClick={handleSave} disabled={busy}>
              {busy ? 'Importing…' : 'Confirm Import'}
            </button>
          </div>
        )}

        {error && <div className="pf-error">{error}</div>}
      </div>
    </div>
  );
}
