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

  const online = chainStatus && chainStatus.block_height > 0;

  return (
    <div style={{ display: 'grid', placeItems: 'center', height: '100vh', padding: 24, gap: 20 }}>
      <div className="pf-mark" style={{ width: 56, height: 56, borderRadius: 16, fontSize: 18 }}>PF</div>
      <div style={{ textAlign: 'center' }}>
        <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.02em' }}>Wallet locked</div>
        <div style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--dim)', marginTop: 6 }}>
          self-custody · {address ? `${address.slice(0, 8)}…${address.slice(-6)}` : '…'}
        </div>
        {online && (
          <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--muted)', marginTop: 4 }}>
            height {chainStatus.block_height}
          </div>
        )}
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
            <div className="pf-warning" style={{ fontSize: 12 }}>
              This erases the encrypted wallet from this browser. You will need the
              64-char master seed to recover funds. Continue?
            </div>
            <div className="pf-even">
              <button className="pf-ghost" onClick={handleRemove} disabled={busy}>
                {busy ? 'Resetting…' : 'Erase wallet'}
              </button>
              <button className="pf-ghost" onClick={() => setConfirmRemove(false)} disabled={busy}>
                Cancel
              </button>
            </div>
          </>
        ) : (
          <button className="pf-ghost" style={{ fontSize: 12 }} onClick={() => setConfirmRemove(true)}>
            Reset / forgot passphrase
          </button>
        )}
      </div>
    </div>
  );
}
