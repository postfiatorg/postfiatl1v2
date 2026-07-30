import React, { useCallback, useEffect, useState } from 'react';

import {
  A666_NATIVE_ASSET_ID,
  A666_PRIMARY_ROUTE_ID,
  A666_SETTLEMENT_ASSET_ID,
  formatA666Nav,
  formatA666Units,
} from '../lib/a666-primary-route.js';
import {
  CHAIN_ID,
  GENESIS_HASH,
  ETH_MAINNET_CHAIN_ID,
  ETH_MAINNET_USDC,
  formatAssetBalance,
  shortenAssetId,
  truncateMiddle,
} from '../lib/utils.js';
import { loadGovernedVaultBridgeRoute } from '../lib/bridge-route.js';

function result(response) {
  return response?.ok && response.result ? response.result : null;
}

function resolveAssetId(id) {
  if (id === 'A666') return A666_NATIVE_ASSET_ID;
  if (id === 'pfUSDC') return A666_SETTLEMENT_ASSET_ID;
  return /^[0-9a-f]{96}$/.test(String(id || '')) ? id : '';
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

export default function NavDetail({ id, rpc, address, go }) {
  const assetId = resolveAssetId(id);
  const isA666 = assetId === A666_NATIVE_ASSET_ID;
  const isPfusdc = assetId === A666_SETTLEMENT_ASSET_ID;
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
      if (isA666) {
        requests.push(rpc.navcoinBridgeSupplyStatus(A666_PRIMARY_ROUTE_ID));
        requests.push(rpc.vaultBridgeStatus(A666_NATIVE_ASSET_ID));
      } else if (isPfusdc) {
        requests.push(loadGovernedVaultBridgeRoute(rpc, {
          assetId: A666_SETTLEMENT_ASSET_ID,
          chainId: CHAIN_ID,
          genesisHash: GENESIS_HASH,
          sourceChainId: ETH_MAINNET_CHAIN_ID,
          tokenAddress: ETH_MAINNET_USDC,
        }));
        requests.push(rpc.vaultBridgeStatus(A666_SETTLEMENT_ASSET_ID));
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
  }, [address, assetId, isA666, isPfusdc, rpc]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const asset = snapshot?.asset;
  const route = snapshot?.route;
  const reserve = snapshot?.reserve;
  const a666Verified = Boolean(
    isA666
    && route?.route_id === A666_PRIMARY_ROUTE_ID
    && route?.native_nav_asset_id === A666_NATIVE_ASSET_ID
    && route?.settlement_asset_id === A666_SETTLEMENT_ASSET_ID
    && route?.live_value_enabled === true
    && route?.paused === false
    && route?.invariant_holds === true
    && reserve?.asset_id === A666_NATIVE_ASSET_ID
    && reserve?.finalized_epoch === route?.pricing_nav_epoch
    && reserve?.finalized_reserve_packet_hash === route?.pricing_reserve_packet_hash
  );
  const pfusdcVerified = Boolean(
    isPfusdc
    && route?.profile?.asset_id === A666_SETTLEMENT_ASSET_ID
    && route?.profile?.source_chain_id === ETH_MAINNET_CHAIN_ID
    && route?.profile?.token_address === ETH_MAINNET_USDC
    && route?.evidenceTier === 'receipt-proven'
  );
  const verified = a666Verified || pfusdcVerified;
  const displayName = isA666
    ? 'A666 NAVCoin fund share'
    : isPfusdc
      ? 'Ethereum-vault-backed PFTL settlement asset'
      : 'Other or legacy issued asset';
  const displayCode = isA666 ? 'A666' : isPfusdc ? 'pfUSDC' : shortenAssetId(assetId);

  if (!assetId) {
    return (
      <div className="pf-page">
        <button className="pf-link" onClick={() => go('nav')}>← NavCoins</button>
        <div className="pf-error">This asset is not part of the current wallet registry.</div>
      </div>
    );
  }

  return (
    <div className="pf-page">
      <button className="pf-link" onClick={() => go('nav')} style={{ fontSize: 12, marginBottom: 14, alignSelf: 'start' }}>← NavCoins</button>
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
            <div className="pf-eyebrow">On-chain asset details</div>
            <div className="pf-row"><span className="pf-rk">Asset ID</span><span className="pf-rv">{shortenAssetId(assetId)}</span></div>
            <div className="pf-row"><span className="pf-rk">Issuer</span><span className="pf-rv">{truncateMiddle(asset?.issuer || '—', 12)}</span></div>
            <div className="pf-row"><span className="pf-rk">Precision</span><span className="pf-rv">{asset?.precision ?? '—'}</span></div>
            <div className="pf-row"><span className="pf-rk">Outstanding supply</span><span className="pf-rv">{formatA666Units(asset?.outstanding_supply)}</span></div>
            <div className="pf-row"><span className="pf-rk">Your balance</span><span className="pf-rv">{formatAssetBalance(assetId, snapshot?.balance || 0)}</span></div>
          </div>

          {isA666 && (
            <div className="pf-card" style={{ display: 'grid', gap: 12 }}>
              <div className="pf-eyebrow">Primary market</div>
              <div className="pf-row"><span className="pf-rk">Verified NAV</span><span className="pf-rv">{formatA666Nav(reserve?.nav_per_unit)}</span></div>
              <div className="pf-row"><span className="pf-rk">pfUSDC reserve</span><span className="pf-rv">{formatA666Units(route?.settlement_reserve_atoms)}</span></div>
              <div className="pf-row"><span className="pf-rk">Mint capacity</span><span className="pf-rv">{formatA666Units(route?.available_issue_atoms)}</span></div>
              <div className="pf-row"><span className="pf-rk">Redeem capacity</span><span className="pf-rv">{formatA666Units(route?.available_redeem_atoms)}</span></div>
              <button className="pf-primary" onClick={() => go('a666')}>Open A666 primary market</button>
            </div>
          )}

          {isPfusdc && (
            <div className="pf-even">
              <button className="pf-primary" onClick={() => go('bridge')}>Add pfUSDC from Ethereum</button>
              <button className="pf-ghost" onClick={() => go('a666')}>Use pfUSDC for A666</button>
            </div>
          )}
          <button className="pf-ghost" onClick={() => go('send', { sendSource: 'asset' })}>Send this PFTL asset →</button>
        </div>

        <aside className="pf-card" style={{ display: 'grid', gap: 16, alignSelf: 'start', borderColor: verified ? 'var(--green-border)' : 'var(--border)' }}>
          <div className="pf-row">
            <div className="pf-eyebrow">Verification</div>
            <span className={`pf-pill${verified ? ' good' : ' warn'}`}>{verified ? 'VERIFIED' : 'BLOCKED'}</span>
          </div>
          {isA666 && (
            <>
              <div className="pf-row"><span className="pf-rk">NAV epoch</span><span className="pf-rv">{route?.pricing_nav_epoch ?? '—'}</span></div>
              <div className="pf-row"><span className="pf-rk">Reserve packet</span><span className="pf-rv">{truncateMiddle(route?.pricing_reserve_packet_hash || '—', 8)}</span></div>
              <div className="pf-row"><span className="pf-rk">Invariant</span><span className="pf-rv">{route?.invariant_holds ? 'holds' : 'not verified'}</span></div>
              <div style={{ fontSize: 12.5, color: 'var(--muted)', lineHeight: 1.5 }}>
                Verified only when the active A666 policy, finalized StakeHub NAV epoch, reserve packet, and route invariant all match.
              </div>
            </>
          )}
          {isPfusdc && (
            <>
              <div className="pf-row"><span className="pf-rk">Source chain</span><span className="pf-rv">Ethereum mainnet</span></div>
              <div className="pf-row"><span className="pf-rk">Vault</span><span className="pf-rv">{truncateMiddle(route?.vaultAddress || '—', 7)}</span></div>
              <div className="pf-row"><span className="pf-rk">Evidence</span><span className="pf-rv">{route?.evidenceTier || '—'}</span></div>
              <div style={{ fontSize: 12.5, color: 'var(--muted)', lineHeight: 1.5 }}>
                Verified against the active governed Ethereum vault profile and its receipt-proof verifier.
              </div>
            </>
          )}
          {!isA666 && !isPfusdc && (
            <div style={{ fontSize: 12.5, color: 'var(--muted)', lineHeight: 1.5 }}>
              This historical or unregistered asset has no current market or reserve route in the wallet. It remains sendable on PFTL.
            </div>
          )}
          <div className="pf-row"><span className="pf-rk">PFTL height</span><span className="pf-rv">{snapshot?.chain?.block_height ?? '—'}</span></div>
        </aside>
      </div>
    </div>
  );
}
