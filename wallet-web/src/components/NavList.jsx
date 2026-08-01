import React, { useCallback, useEffect, useState } from 'react';

import { formatNavcoinNav, formatNavcoinUnits } from '../lib/navcoin-primary-route.js';
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

function marketIsVerified(market, route, nav) {
  return Boolean(
    route?.route_id === market.routeId
    && route?.native_nav_asset_id === market.navAssetId
    && route?.settlement_asset_id === market.settlementAssetId
    && route?.live_value_enabled === true
    && route?.paused === false
    && route?.invariant_holds === true
    && nav?.asset_id === market.navAssetId
    && nav?.finalized_epoch === route?.pricing_nav_epoch
    && nav?.finalized_reserve_packet_hash === route?.pricing_reserve_packet_hash
  );
}

export default function NavList({ markets = [], rpc, address, go }) {
  const [snapshot, setSnapshot] = useState(null);
  const [error, setError] = useState('');

  const refresh = useCallback(async () => {
    if (!rpc) return;
    setError('');
    try {
      const [marketStates, assetsResponse, statusResponse] = await Promise.all([
        Promise.all(markets.map(async market => {
          try {
            const [routeResponse, navResponse] = await Promise.all([
              rpc.navcoinBridgeSupplyStatus(market.routeId),
              rpc.vaultBridgeStatus(market.navAssetId),
            ]);
            const route = result(routeResponse);
            const nav = result(navResponse);
            if (!route || !nav) throw new Error('governed route or NAV packet unavailable');
            return { market, route, nav, error: '' };
          } catch (failure) {
            return { market, route: null, nav: null, error: failure.message || 'market unavailable' };
          }
        })),
        address ? rpc.accountAssets(address) : Promise.resolve(null),
        rpc.status(),
      ]);
      setSnapshot({
        markets: marketStates,
        assets: assetRows(result(assetsResponse)),
        chain: result(statusResponse),
      });
    } catch (failure) {
      setError(failure.message || 'Unable to load current NAVCoins');
    }
  }, [address, markets, rpc]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const marketStates = snapshot?.markets || markets.map(market => ({ market, route: null, nav: null, error: '' }));
  const assets = snapshot?.assets || [];
  const configuredAssetIds = new Set(markets.flatMap(market => [market.navAssetId, market.settlementAssetId]));
  const settlementMarkets = [...new Map(markets.map(market => [market.settlementAssetId, market])).values()];
  const otherAssets = assets.filter(asset => !configuredAssetIds.has(String(asset?.asset_id || asset?.id || '').toLowerCase()));

  const rows = [
    ...marketStates.map(({ market, route, nav }) => ({
      id: market.symbol,
      name: market.name,
      holding: assetBalance(assets, market.navAssetId),
      assetId: market.navAssetId,
      status: marketIsVerified(market, route, nav) ? `${formatNavcoinNav(nav?.nav_per_unit)} verified NAV` : 'NAV verification blocked',
    })),
    ...settlementMarkets.map(market => ({
      id: market.settlementSymbol,
      name: 'Governed NAVCoin settlement asset',
      holding: assetBalance(assets, market.settlementAssetId),
      assetId: market.settlementAssetId,
      status: `Funds ${markets.filter(item => item.settlementAssetId === market.settlementAssetId).map(item => item.symbol).join(', ')} primary markets`,
    })),
    ...otherAssets.map(asset => {
      const assetId = String(asset?.asset_id || asset?.id || '');
      return {
        id: shortenAssetId(assetId),
        name: 'Other or legacy issued asset · send only',
        holding: String(asset?.balance ?? asset?.amount ?? 0),
        assetId,
        status: 'Not part of a configured NAVCoin market',
      };
    }),
  ];

  return (
    <div className="pf-page">
      <div className="pf-eyebrow">Governed proof-of-reserves assets</div>
      <h1 className="pf-h1">NAVCoins</h1>
      <p style={{ fontSize: 13.5, color: 'var(--muted)', lineHeight: 1.55, marginTop: 10, maxWidth: 680 }}>
        Each NAVCoin has its own governed route, verified NAV, settlement reserve, and Ethereum token.
        Historical issued assets may remain visible in balances without being offered as current markets.
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
            <div className="pf-trow-d" onClick={() => go('navDetail', row.assetId)}>
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
            <div className="pf-trow-m" onClick={() => go('navDetail', row.assetId)}>
              <div className="pf-row">
                <span style={{ fontWeight: 700, fontSize: 16 }}>{row.id}</span>
                {hasBalance(row.holding) && <span className="pf-pill">{formatAssetBalance(row.assetId, row.holding)} held</span>}
              </div>
              <div style={{ fontFamily: 'var(--mono)', fontSize: 11.5, color: 'var(--dim)' }}>{row.status}</div>
            </div>
          </React.Fragment>
        ))}
      </div>

      {marketStates.map(({ market, route, nav, error: marketError }) => {
        const verified = marketIsVerified(market, route, nav);
        return (
          <div className="pf-evidence" key={market.key}>
            <div>
              <div className="pf-eyebrow">Live {market.symbol} reserve evidence</div>
              <div style={{ fontSize: 13, color: 'var(--muted)', marginTop: 6 }}>
                {verified
                  ? 'The route policy and finalized PFTL reserve packet match.'
                  : `${market.symbol} actions are blocked${marketError ? `: ${marketError}` : ' until the route policy and reserve packet match.'}`}
              </div>
              <button className="pf-link" onClick={() => go('market', { marketKey: market.key })} style={{ marginTop: 8 }}>
                Open primary market →
              </button>
            </div>
            <div className="pf-evidence-stats">
              <div><div className="pf-eyebrow" style={{ fontSize: 10 }}>NAV</div><div style={{ fontFamily: 'var(--mono)', fontSize: 16 }}>{formatNavcoinNav(nav?.nav_per_unit)}</div></div>
              <div><div className="pf-eyebrow" style={{ fontSize: 10 }}>{market.settlementSymbol} reserve</div><div style={{ fontFamily: 'var(--mono)', fontSize: 16 }}>{formatNavcoinUnits(route?.settlement_reserve_atoms, market.settlementDecimals)}</div></div>
              <div><div className="pf-eyebrow" style={{ fontSize: 10 }}>Ledger</div><div style={{ fontFamily: 'var(--mono)', fontSize: 16 }}>{snapshot?.chain?.block_height ?? '—'}</div></div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
