import React, { useCallback, useEffect, useState } from 'react';

import { formatNavcoinNav, formatNavcoinUnits } from '../lib/navcoin-primary-route.js';
import { loadGovernedVaultBridgeRoute } from '../lib/bridge-route.js';
import {
  navcoinMarketForAsset,
  navcoinMarketForSettlementAsset,
} from '../lib/navcoin-markets.js';
import {
  CHAIN_ID,
  GENESIS_HASH,
  formatAssetBalance,
  shortenAssetId,
  truncateMiddle,
} from '../lib/utils.js';

function result(response) {
  return response?.ok && response.result ? response.result : null;
}

function resolveAssetId(markets, id) {
  const value = String(id || '');
  const bySymbol = markets.find(market => (
    market.symbol === value || market.settlementSymbol === value
  ));
  if (bySymbol) return bySymbol.symbol === value ? bySymbol.navAssetId : bySymbol.settlementAssetId;
  return /^[0-9a-f]{96}$/.test(value) ? value : '';
}

function balanceFromAssets(value, assetId) {
  const assets = Array.isArray(value) ? value : (value?.assets || []);
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

export default function NavDetail({ markets = [], id, rpc, address, go }) {
  const assetId = resolveAssetId(markets, id);
  const market = navcoinMarketForAsset(markets, assetId);
  const settlementMarket = navcoinMarketForSettlementAsset(markets, assetId);
  const [snapshot, setSnapshot] = useState(null);
  const [error, setError] = useState('');

  const refresh = useCallback(async () => {
    if (!rpc || !assetId) return;
    setError('');
    try {
      const requests = [
        rpc.assetInfo(assetId),
        address ? rpc.accountAssets(address) : Promise.resolve(null),
        rpc.status(),
      ];
      if (market) {
        requests.push(rpc.navcoinBridgeSupplyStatus(market.routeId));
        requests.push(rpc.vaultBridgeStatus(market.navAssetId));
      } else if (settlementMarket) {
        requests.push(loadGovernedVaultBridgeRoute(rpc, {
          assetId: settlementMarket.settlementAssetId,
          chainId: CHAIN_ID,
          genesisHash: GENESIS_HASH,
        }));
        requests.push(rpc.vaultBridgeStatus(settlementMarket.settlementAssetId));
      }
      const values = await Promise.all(requests);
      setSnapshot({
        asset: result(values[0]),
        balance: balanceFromAssets(result(values[1]), assetId),
        chain: result(values[2]),
        route: values[3]?.result || values[3] || null,
        reserve: result(values[4]),
      });
    } catch (failure) {
      setError(failure.message || 'Unable to load verified asset state');
    }
  }, [address, assetId, market, settlementMarket, rpc]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const asset = snapshot?.asset?.asset || snapshot?.asset;
  const route = snapshot?.route;
  const reserve = snapshot?.reserve;
  const navcoinVerified = Boolean(
    market
    && route?.route_id === market.routeId
    && route?.native_nav_asset_id === market.navAssetId
    && route?.settlement_asset_id === market.settlementAssetId
    && route?.live_value_enabled === true
    && route?.paused === false
    && route?.invariant_holds === true
    && reserve?.asset_id === market.navAssetId
    && reserve?.finalized_epoch === route?.pricing_nav_epoch
    && reserve?.finalized_reserve_packet_hash === route?.pricing_reserve_packet_hash
  );
  const settlementVerified = Boolean(
    settlementMarket
    && route?.profile?.asset_id === settlementMarket.settlementAssetId
    && route?.evidenceTier === 'receipt-proven'
  );
  const verified = navcoinVerified || settlementVerified;
  const displayName = market
    ? market.name
    : settlementMarket
      ? settlementMarket.settlementName
      : asset?.display_name || (/^[a-z]/.test(String(asset?.code || '')) ? 'Legacy issued asset' : 'Issued asset');
  const rawDisplayCode = market?.symbol || settlementMarket?.settlementSymbol || asset?.code || 'Unknown asset';
  const displayCode = String(rawDisplayCode).toUpperCase() === 'PFUSDC' ? 'pfUSDC' : rawDisplayCode;

  if (!assetId) {
    return (
      <div className="pf-page">
        <button className="pf-link" onClick={() => go('nav')}>← Assets</button>
        <div className="pf-error">This asset is not part of the current wallet registry.</div>
      </div>
    );
  }

  return (
    <div className="pf-page">
      <button className="pf-link" onClick={() => go('nav')} style={{ fontSize: 12, marginBottom: 14, alignSelf: 'start' }}>← Assets</button>
      <div className="pf-two">
        <div style={{ display: 'grid', gap: 20 }}>
          <div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <h1 className="pf-h1" style={{ marginTop: 0 }}>{displayCode}</h1>
              {snapshot && hasBalance(snapshot.balance) && (
                <span className="pf-pill">{formatAssetBalance(assetId, snapshot.balance)} held</span>
              )}
            </div>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--dim)', marginTop: 4 }}>{displayName}</div>
          </div>

          {error && <div className="pf-error">{error}</div>}

          <div className="pf-card" style={{ display: 'grid', gap: 12 }}>
            <div className="pf-eyebrow">Your holding</div>
            <div className="pf-row"><span className="pf-rk">Your balance</span><span className="pf-rv">{formatAssetBalance(assetId, snapshot?.balance || 0)}</span></div>
            <div className="pf-row"><span className="pf-rk">Status</span><span className="pf-rv">{market ? 'Verified NAV asset' : settlementMarket ? 'Settlement asset' : /^[a-z]/.test(String(asset?.code || '')) ? 'Legacy asset' : 'Issued asset'}</span></div>
            <details style={{ borderTop: '1px solid var(--border-soft)', paddingTop: 12 }}>
              <summary style={{ cursor: 'pointer', fontSize: 12.5, fontWeight: 650 }}>Advanced on-chain details</summary>
              <div style={{ display: 'grid', gap: 10, paddingTop: 12 }}>
                <div className="pf-row"><span className="pf-rk">Asset ID</span><span className="pf-rv">{shortenAssetId(assetId)}</span></div>
                <div className="pf-row"><span className="pf-rk">Issuer</span><span className="pf-rv">{truncateMiddle(asset?.issuer || '—', 12)}</span></div>
                <div className="pf-row"><span className="pf-rk">Precision</span><span className="pf-rv">{asset?.precision ?? '—'}</span></div>
                <div className="pf-row"><span className="pf-rk">Outstanding supply</span><span className="pf-rv">{formatNavcoinUnits(asset?.outstanding_supply, asset?.precision ?? 6)}</span></div>
              </div>
            </details>
          </div>

          {market && (
            <div className="pf-card" style={{ display: 'grid', gap: 12 }}>
              <div className="pf-eyebrow">Primary market</div>
              <div className="pf-row"><span className="pf-rk">Verified NAV</span><span className="pf-rv">{formatNavcoinNav(reserve?.nav_per_unit)}</span></div>
              <div className="pf-row"><span className="pf-rk">{market.settlementSymbol} reserve</span><span className="pf-rv">{formatNavcoinUnits(route?.settlement_reserve_atoms, market.settlementDecimals)}</span></div>
              <div className="pf-row"><span className="pf-rk">Available to buy</span><span className="pf-rv">{formatNavcoinUnits(route?.available_issue_atoms, market.decimals)}</span></div>
              <div className="pf-row"><span className="pf-rk">Available to redeem</span><span className="pf-rv">{formatNavcoinUnits(route?.available_redeem_atoms, market.decimals)}</span></div>
              <button className="pf-primary" onClick={() => go('market', { marketKey: market.key })}>Buy or redeem {market.symbol}</button>
            </div>
          )}

          {settlementMarket && (
            <div className="pf-even">
              <button className="pf-primary" onClick={() => go('bridge')}>Add {settlementMarket.settlementSymbol} from Ethereum</button>
              <button className="pf-ghost" onClick={() => go('market', { marketKey: settlementMarket.key })}>Trade NAV assets</button>
            </div>
          )}
          <button className="pf-ghost" onClick={() => go('send', { sendSource: 'asset' })}>Send this PFTL asset →</button>
        </div>

        <aside className="pf-card" style={{ display: 'grid', gap: 16, alignSelf: 'start', borderColor: verified ? 'var(--green-border)' : 'var(--border)' }}>
          <div className="pf-row">
            <div className="pf-eyebrow">Verification</div>
            <span className={`pf-pill${verified ? ' good' : market || settlementMarket ? ' warn' : ''}`}>{verified ? 'VERIFIED' : market || settlementMarket ? 'UNAVAILABLE' : 'NO MARKET'}</span>
          </div>
          {market && (
            <>
              <div className="pf-row"><span className="pf-rk">NAV epoch</span><span className="pf-rv">{route?.pricing_nav_epoch ?? '—'}</span></div>
              <div className="pf-row"><span className="pf-rk">Reserve packet</span><span className="pf-rv">{truncateMiddle(route?.pricing_reserve_packet_hash || '—', 8)}</span></div>
              <div className="pf-row"><span className="pf-rk">Invariant</span><span className="pf-rv">{route?.invariant_holds ? 'holds' : 'not verified'}</span></div>
              <div style={{ fontSize: 12.5, color: 'var(--muted)', lineHeight: 1.5 }}>
                Verified only when this market's active policy, finalized PFTL NAV epoch, reserve packet, and route invariant all match.
              </div>
            </>
          )}
          {settlementMarket && (
            <>
              <div className="pf-row"><span className="pf-rk">Source chain</span><span className="pf-rv">Ethereum mainnet</span></div>
              <div className="pf-row"><span className="pf-rk">Vault</span><span className="pf-rv">{truncateMiddle(route?.vaultAddress || '—', 7)}</span></div>
              <div className="pf-row"><span className="pf-rk">Evidence</span><span className="pf-rv">{route?.evidenceTier || '—'}</span></div>
              <div style={{ fontSize: 12.5, color: 'var(--muted)', lineHeight: 1.5 }}>
                Verified against the active governed Ethereum vault profile and its receipt-validated withdrawal contract.
              </div>
            </>
          )}
          {!market && !settlementMarket && (
            <div style={{ fontSize: 12.5, color: 'var(--muted)', lineHeight: 1.5 }}>
              This historical or unregistered asset has no current market or reserve route in the wallet. It remains sendable on PFTL.
            </div>
          )}
        </aside>
      </div>
    </div>
  );
}
