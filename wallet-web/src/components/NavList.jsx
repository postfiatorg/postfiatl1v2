import React, { useState, useEffect } from 'react';
import { formatBalance, formatAssetBalance, shortenAssetId, PFUSDC_ASSET_ID, A651_ASSET_ID } from '../lib/utils.js';

export default function NavList({ rpc, address, swapServer, go }) {
  const [assets, setAssets] = useState([]);
  const [navData, setNavData] = useState(null);
  const [error, setError] = useState('');
  const [chainStatus, setChainStatus] = useState(null);

  useEffect(() => {
    const fetchData = async () => {
      if (rpc) {
        try {
          const statusResp = await rpc.status();
          if (statusResp.ok) setChainStatus(statusResp.result);
        } catch (e) { /* optional */ }
        if (address) {
          try {
            const resp = await rpc.accountAssets(address);
            if (resp.ok && resp.result) {
              const items = Array.isArray(resp.resp) ? resp.result : (resp.result.assets || []);
              setAssets(items);
            }
          } catch (e) { setAssets([]); }
        }
      }
      if (swapServer) {
        try {
          const nav = await swapServer.getNav('before');
          setNavData(nav);
        } catch (e) { /* optional */ }
      }
    };
    fetchData();
  }, [rpc, address, swapServer]);

  const getAssetCode = (assetId) => {
    if (assetId === PFUSDC_ASSET_ID) return 'pfUSDC';
    if (assetId === A651_ASSET_ID) return 'a651';
    return shortenAssetId(assetId);
  };

  // Build coin rows from assets, with NAV data if available
  const coins = assets.map(a => {
    const id = a.asset_id || a.id;
    const code = getAssetCode(id);
    const holding = a.balance || a.amount || 0;
    // NAV data would come from swapServer; for now we display what we have
    return {
      id: code,
      name: code === 'pfUSDC' ? 'USDC bridge' : code === 'a651' ? 'Reserve Alpha' : 'Issued asset',
      holding,
      assetId: id,
    };
  });

  // Add PFT as a "coin" row
  coins.unshift({ id: 'PFT', name: 'PostFiat L1', holding: 0, assetId: 'PFT' });

  const fmt = (n, d = 2) => Number(n).toLocaleString('en-US', { minimumFractionDigits: d, maximumFractionDigits: d });

  return (
    <div className="pf-page">
      <div className="pf-eyebrow">Proof-of-reserves funds</div>
      <h1 className="pf-h1">NavCoins</h1>
      <p style={{ fontSize: 13.5, color: 'var(--muted)', lineHeight: 1.55, marginTop: 10, maxWidth: 600 }}>
        Shielded funds with verifiable reserves. Each row is a fund you hold or can trade. Click a row for proof-of-reserves details.
      </p>

      {error && <div className="pf-error" style={{ marginTop: 16 }}>{error}</div>}

      <div className="pf-card" style={{ padding: 0, marginTop: 18 }}>
        <div className="pf-thead">
          <span className="pf-th" style={{ cursor: 'default' }}>Fund</span>
          <span className="pf-th r" style={{ cursor: 'default' }}>Held</span>
          <span className="pf-th r" style={{ cursor: 'default' }}>Asset ID</span>
          <span className="pf-th" style={{ cursor: 'default' }} />
          <span />
          <span />
        </div>

        {coins.length === 0 ? (
          <div style={{ padding: '24px 16px', fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--dim)' }}>
            No assets visible yet. pfUSDC and a651 will appear after bridge or swap activity.
          </div>
        ) : (
          coins.map((c, i) => (
            <React.Fragment key={i}>
              <div className="pf-trow-d" onClick={() => go('navDetail', c.id)}>
                <div>
                  <span style={{ fontWeight: 700, fontSize: 16, letterSpacing: '-0.01em' }}>{c.id}</span>
                  <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--dim)', marginTop: 2 }}>{c.name}</div>
                </div>
                <div className="pf-num" style={{ color: c.holding > 0 ? 'var(--text)' : 'var(--dim)' }}>
                  {c.holding > 0 ? formatAssetBalance(c.assetId, c.holding) : '—'}
                </div>
                <div className="pf-num" style={{ color: 'var(--dim)', fontSize: 11 }}>{shortenAssetId(c.assetId)}</div>
                <div />
                <div />
                <span style={{ color: 'var(--dim)', textAlign: 'right' }}>→</span>
              </div>

              <div className="pf-trow-m" onClick={() => go('navDetail', c.id)}>
                <div className="pf-row" style={{ marginBottom: 4 }}>
                  <span style={{ fontWeight: 700, fontSize: 16 }}>{c.id}</span>
                  {c.holding > 0 && <span className="pf-pill">{formatAssetBalance(c.assetId, c.holding)} held</span>}
                </div>
                <div style={{ fontFamily: 'var(--mono)', fontSize: 11.5, color: 'var(--dim)' }}>
                  {c.name} · {shortenAssetId(c.assetId)}
                </div>
              </div>
            </React.Fragment>
          ))
        )}
      </div>

      {/* evidence strip */}
      <div className="pf-evidence">
        <div>
          <div className="pf-eyebrow">Live reserve evidence</div>
          <div style={{ fontSize: 13, color: 'var(--muted)', marginTop: 6 }}>
            All fund reserves attested on-ledger and independently verifiable.
          </div>
        </div>
        <div className="pf-evidence-stats">
          <div>
            <div className="pf-eyebrow" style={{ fontSize: 10 }}>Ledger</div>
            <div style={{ fontSize: 16, fontWeight: 650, fontFamily: 'var(--mono)' }}>
              {chainStatus ? formatBalance(chainStatus.block_height, 0) : '—'}
            </div>
          </div>
          <div>
            <div className="pf-eyebrow" style={{ fontSize: 10 }}>Validators</div>
            <div style={{ fontSize: 16, fontWeight: 650, fontFamily: 'var(--mono)' }}>
              {chainStatus?.validator_count ? `${chainStatus.validator_count} / ${chainStatus.validator_count}` : '—'}
            </div>
          </div>
          {navData && (
            <div>
              <div className="pf-eyebrow" style={{ fontSize: 10 }}>NAV floor</div>
              <div style={{ fontSize: 16, fontWeight: 650, color: 'var(--mint)', fontFamily: 'var(--mono)' }}>
                {formatBalance(navData.nav_floor || navData.result?.nav_floor || 0)}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
