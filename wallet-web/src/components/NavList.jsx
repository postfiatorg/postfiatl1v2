import React, { useCallback, useEffect, useState } from 'react';

import {
  A666_NATIVE_ASSET_ID,
  A666_PRIMARY_ROUTE_ID,
  A666_SETTLEMENT_ASSET_ID,
  formatA666Nav,
  formatA666Units,
} from '../lib/a666-primary-route.js';
import { formatAssetBalance, shortenAssetId } from '../lib/utils.js';

function result(response) {
  return response?.ok && response.result ? response.result : null;
}

function assetRows(value) {
  return Array.isArray(value) ? value : (value?.assets || []);
}

function assetBalance(assets, assetId) {
  const row = assets.find(asset => String(asset?.asset_id || asset?.id || '').toLowerCase() === assetId);
  return String(row?.balance ?? row?.amount ?? 0);
}

function hasBalance(value) {
  try {
    return BigInt(String(value ?? '0')) > 0n;
  } catch (_) {
    return false;
  }
}

export default function NavList({ rpc, address, go }) {
  const [snapshot, setSnapshot] = useState(null);
  const [error, setError] = useState('');

  const refresh = useCallback(async () => {
    if (!rpc) return;
    setError('');
    try {
      const [routeResponse, navResponse, assetsResponse, statusResponse] = await Promise.all([
        rpc.navcoinBridgeSupplyStatus(A666_PRIMARY_ROUTE_ID),
        rpc.vaultBridgeStatus(A666_NATIVE_ASSET_ID),
        address ? rpc.accountAssets(address) : Promise.resolve(null),
        rpc.status(),
      ]);
      const route = result(routeResponse);
      const nav = result(navResponse);
      const assets = assetRows(result(assetsResponse));
      if (!route || !nav) throw new Error('The live A666 route or NAV packet is unavailable.');
      setSnapshot({
        route,
        nav,
        assets,
        chain: result(statusResponse),
      });
    } catch (failure) {
      setError(failure.message || 'Unable to load current NavCoins');
    }
  }, [address, rpc]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const route = snapshot?.route;
  const nav = snapshot?.nav;
  const assets = snapshot?.assets || [];
  const navVerified = Boolean(
    route?.route_id === A666_PRIMARY_ROUTE_ID
    && route?.native_nav_asset_id === A666_NATIVE_ASSET_ID
    && route?.settlement_asset_id === A666_SETTLEMENT_ASSET_ID
    && route?.live_value_enabled === true
    && route?.paused === false
    && route?.invariant_holds === true
    && nav?.asset_id === A666_NATIVE_ASSET_ID
    && nav?.finalized_epoch === route?.pricing_nav_epoch
    && nav?.finalized_reserve_packet_hash === route?.pricing_reserve_packet_hash
  );
  const a666Balance = assetBalance(assets, A666_NATIVE_ASSET_ID);
  const pfusdcBalance = assetBalance(assets, A666_SETTLEMENT_ASSET_ID);
  const otherAssets = assets.filter(asset => {
    const id = String(asset?.asset_id || asset?.id || '').toLowerCase();
    return id !== A666_NATIVE_ASSET_ID && id !== A666_SETTLEMENT_ASSET_ID;
  });

  const rows = [
    {
      id: 'A666',
      name: 'A666 NAVCoin fund share',
      holding: a666Balance,
      assetId: A666_NATIVE_ASSET_ID,
      status: navVerified ? `${formatA666Nav(nav?.nav_per_unit)} verified NAV` : 'NAV verification blocked',
    },
    {
      id: 'pfUSDC',
      name: 'Ethereum-vault-backed settlement asset',
      holding: pfusdcBalance,
      assetId: A666_SETTLEMENT_ASSET_ID,
      status: 'Funds A666 minting and receives redemptions',
    },
    ...otherAssets.map(asset => {
      const assetId = String(asset?.asset_id || asset?.id || '');
      return {
        id: shortenAssetId(assetId),
        name: 'Other or legacy issued asset · send only',
        holding: String(asset?.balance ?? asset?.amount ?? 0),
        assetId,
        status: 'Not part of the current A666 market',
      };
    }),
  ];

  return (
    <div className="pf-page">
      <div className="pf-eyebrow">Current proof-of-reserves assets</div>
      <h1 className="pf-h1">NavCoins</h1>
      <p style={{ fontSize: 13.5, color: 'var(--muted)', lineHeight: 1.55, marginTop: 10, maxWidth: 680 }}>
        A666 is the active NAVCoin. pfUSDC is its PFTL settlement and reserve asset.
        Historical issued assets may remain visible in balances, but they are not offered as current markets.
      </p>

      {error && <div className="pf-error" style={{ marginTop: 16 }}>{error}</div>}

      <div className="pf-card" style={{ padding: 0, marginTop: 18 }}>
        <div className="pf-thead">
          <span className="pf-th" style={{ cursor: 'default' }}>Asset</span>
          <span className="pf-th r" style={{ cursor: 'default' }}>Held</span>
          <span className="pf-th r" style={{ cursor: 'default' }}>Asset ID</span>
          <span className="pf-th" style={{ cursor: 'default' }} />
          <span />
          <span />
        </div>
        {rows.map(row => (
          <React.Fragment key={row.assetId}>
            <div className="pf-trow-d" onClick={() => go('navDetail', row.id === 'A666' || row.id === 'pfUSDC' ? row.id : row.assetId)}>
              <div>
                <span style={{ fontWeight: 700, fontSize: 16 }}>{row.id}</span>
                <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--dim)', marginTop: 2 }}>{row.name}</div>
              </div>
              <div className="pf-num" style={{ color: hasBalance(row.holding) ? 'var(--text)' : 'var(--dim)' }}>
                {hasBalance(row.holding) ? formatAssetBalance(row.assetId, row.holding) : '—'}
              </div>
              <div className="pf-num" style={{ color: 'var(--dim)', fontSize: 11 }}>{shortenAssetId(row.assetId)}</div>
              <div />
              <div />
              <span style={{ color: 'var(--dim)', textAlign: 'right' }}>→</span>
            </div>
            <div className="pf-trow-m" onClick={() => go('navDetail', row.id === 'A666' || row.id === 'pfUSDC' ? row.id : row.assetId)}>
              <div className="pf-row">
                <span style={{ fontWeight: 700, fontSize: 16 }}>{row.id}</span>
                {hasBalance(row.holding) && <span className="pf-pill">{formatAssetBalance(row.assetId, row.holding)} held</span>}
              </div>
              <div style={{ fontFamily: 'var(--mono)', fontSize: 11.5, color: 'var(--dim)' }}>{row.status}</div>
            </div>
          </React.Fragment>
        ))}
      </div>

      <div className="pf-evidence">
        <div>
          <div className="pf-eyebrow">Live A666 reserve evidence</div>
          <div style={{ fontSize: 13, color: 'var(--muted)', marginTop: 6 }}>
            {navVerified
              ? 'The route policy and finalized StakeHub reserve packet match.'
              : 'A666 actions are blocked until the route policy and reserve packet match.'}
          </div>
        </div>
        <div className="pf-evidence-stats">
          <div><div className="pf-eyebrow" style={{ fontSize: 10 }}>NAV</div><div style={{ fontFamily: 'var(--mono)', fontSize: 16 }}>{formatA666Nav(nav?.nav_per_unit)}</div></div>
          <div><div className="pf-eyebrow" style={{ fontSize: 10 }}>pfUSDC reserve</div><div style={{ fontFamily: 'var(--mono)', fontSize: 16 }}>{formatA666Units(route?.settlement_reserve_atoms)}</div></div>
          <div><div className="pf-eyebrow" style={{ fontSize: 10 }}>Ledger</div><div style={{ fontFamily: 'var(--mono)', fontSize: 16 }}>{snapshot?.chain?.block_height ?? '—'}</div></div>
        </div>
      </div>
    </div>
  );
}
