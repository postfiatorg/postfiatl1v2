import React, { useState } from 'react';

export default function LockScreen({ address, onUnlock, onRemove, wasmReady, chainStatus }) {
  const [passphrase, setPassphrase] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);

  const handleUnlock = async () => {
    setError('');
    if (!passphrase) { setError('Passphrase required'); return; }
    setBusy(true);
    try {
      await onUnlock(passphrase);
      setPassphrase('');
    } catch (e) {
      setError(e.message || 'Unlock failed');
    } finally {
      setBusy(false);
    }
  };

  const handleRemove = async () => {
    setError('');
    setBusy(true);
    try {
      await onRemove();
    } catch (e) {
      setError(e.message || 'Reset failed');
    } finally {
      setBusy(false);
      setConfirmRemove(false);
    }
  };

  return (
    <div style={{ display: 'grid', placeItems: 'center', height: '100vh', padding: 24, gap: 20 }}>
      <div className="pf-mark" style={{ width: 56, height: 56, borderRadius: 16, fontSize: 18 }}>PF</div>
      <div style={{ textAlign: 'center' }}>
        <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.02em' }}>Unlock this wallet</div>
        <div style={{ fontSize: 13, color: 'var(--muted)', marginTop: 8, maxWidth: 380 }}>
          Enter the passphrase that encrypts the wallet stored in this browser. This is not your recovery seed.
        </div>
        <div style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--dim)', marginTop: 6 }}>
          {address || 'Wallet address unavailable'}
        </div>
      </div>
      <div style={{ width: '100%', maxWidth: 320, display: 'grid', gap: 10 }}>
        <input className="pf-input" type="password" placeholder="Passphrase"
          value={passphrase} onChange={e => setPassphrase(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter' && !busy) handleUnlock(); }} />
        <button className="pf-primary" onClick={handleUnlock} disabled={busy || !wasmReady}>
          {busy ? 'Unlocking…' : 'Unlock'}
        </button>
        {error && <div className="pf-error">{error}</div>}
      </div>

      {/* Escape hatch for a forgotten passphrase. Removing the local vault only
          deletes the encrypted blob from this browser; funds stay on-chain and
          can be recovered by re-importing the 64-hex master seed. */}
      <div style={{ width: '100%', maxWidth: 320, display: 'grid', gap: 8 }}>
        {confirmRemove ? (
          <>
            <div className="pf-warning" style={{ fontSize: 12, lineHeight: 1.5 }}>
              This removes only the encrypted local copy. It does not move or delete on-chain funds. You must have the 64-character recovery seed or an encrypted backup and its passphrase before continuing.
            </div>
            <div className="pf-even">
              <button className="pf-ghost" onClick={handleRemove} disabled={busy}>
                {busy ? 'Removing…' : 'Remove local wallet'}
              </button>
              <button className="pf-ghost" onClick={() => setConfirmRemove(false)} disabled={busy}>
                Cancel
              </button>
            </div>
          </>
        ) : (
          <button className="pf-ghost" style={{ fontSize: 12 }} onClick={() => setConfirmRemove(true)}>
            Forgot passphrase? Remove and restore wallet
          </button>
        )}
      </div>
    </div>
  );
}
