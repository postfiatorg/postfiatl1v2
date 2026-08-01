import React, { useCallback, useEffect, useState } from 'react';
import {
  ArrowRight,
  Check,
  Eye,
  Landmark,
  Loader2,
  Lock,
  RefreshCw,
  Send,
  ShieldCheck,
} from 'lucide-react';

import { formatNavcoinNav, formatNavcoinUnits } from '../lib/navcoin-primary-route.js';
import { truncateMiddle } from '../lib/utils.js';

function responseResult(response, label) {
  if (!response?.ok || !response.result) {
    throw new Error(response?.error?.message || `${label} is unavailable`);
  }
  return response.result;
}

function routeIsCurrent(route, nav, market) {
  if (!market) return false;
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

function privateNavcoinIsReady(capabilities, market) {
  if (!market) return false;
  const route = capabilities?.routes?.shielded_navswap;
  const assets = Array.isArray(route?.asset_registry) ? route.asset_registry : [];
  return Boolean(
    route?.enabled === true
    && route?.can_run === true
    && route?.can_ingress === true
    && route?.can_egress === true
    && assets.some(asset => (
      String(asset?.asset_id || '').toLowerCase() === market.navAssetId
      && asset?.supported === true
    )),
  );
}

function ProcessStep({ number, title, detail, state, action, onClick, Icon }) {
  return (
    <article className={`pfs-card pfs-route-card wallet-process-step ${state}`}>
      <div className="pfs-route-head">
        <span><Icon size={15} /> {number}. {title}</span>
        <div className="pfs-pill-row">
          <span className={`pf-pill${state === 'ready' ? ' good' : state === 'blocked' ? ' warn' : ''}`}>
            {state === 'ready' ? <Check size={11} /> : state === 'blocked' ? <Lock size={11} /> : <Eye size={11} />}
            {state === 'ready' ? 'available' : state === 'blocked' ? 'not enabled' : 'informational'}
          </span>
        </div>
      </div>
      <p>{detail}</p>
      {action && (
        <button className="pf-primary" type="button" onClick={onClick} disabled={state === 'blocked'}>
          {action} <ArrowRight size={15} />
        </button>
      )}
    </article>
  );
}

export default function Swap({ rpc, swapServer, onNavigate, market = null }) {
  const [snapshot, setSnapshot] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const refresh = useCallback(async () => {
    if (!rpc || !market) return;
    setLoading(true);
    setError('');
    try {
      const [routeResponse, navResponse, statusResponse, capabilities] = await Promise.all([
        rpc.navcoinBridgeSupplyStatus(market.routeId),
        rpc.vaultBridgeStatus(market.navAssetId),
        rpc.status(),
        swapServer?.getNavswapCapabilities?.().catch(() => null) || null,
      ]);
      setSnapshot({
        route: responseResult(routeResponse, `${market.symbol} primary route`),
        nav: responseResult(navResponse, `${market.symbol} NAV`),
        chain: responseResult(statusResponse, 'PFTL status'),
        capabilities,
      });
    } catch (failure) {
      setError(failure.message || 'Unable to verify the current wallet process');
    } finally {
      setLoading(false);
    }
  }, [market, rpc, swapServer]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const route = snapshot?.route;
  const nav = snapshot?.nav;
  const currentRoute = routeIsCurrent(route, nav, market);
  const privateReady = privateNavcoinIsReady(snapshot?.capabilities, market);

  if (!market) {
    return <div className="pf-page"><div className="pf-error">No governed NAVCoin market is registered on this network.</div></div>;
  }

  return (
    <div className="pf-page pf-swap-page">
      <div className="pfs-shell wallet-process-shell">
        <main className="pfs-main">
          <header className="pfs-header">
            <div className="pf-eyebrow">Current wallet process</div>
            <h1>USDC → {market.settlementSymbol} → {market.symbol}</h1>
            <p>
              This is the supported acquisition and redemption path. Historical a651/a652,
              OTC, Sepolia, and Arbitrum workflows are not wallet routes.
            </p>
          </header>

          {error && <div className="pf-error">{error}</div>}

          <div className={`pfs-readiness${currentRoute ? ' ready' : ''}`}>
            <div>
              <span>{loading ? 'VERIFYING' : currentRoute ? 'CURRENT ROUTE VERIFIED' : 'TRADING BLOCKED'}</span>
              <strong>
                {loading
                  ? 'Reading governed PFTL state…'
                  : currentRoute
                    ? `${formatNavcoinNav(nav?.nav_per_unit)} NAV · epoch ${route?.pricing_nav_epoch}`
                    : `The ${market.symbol} route or NAV packet is unavailable or mismatched.`}
              </strong>
            </div>
            <button className="pfb-secondary small" type="button" onClick={refresh} disabled={loading}>
              {loading ? <Loader2 size={14} className="pfb-spin" /> : <RefreshCw size={14} />}
              Refresh
            </button>
          </div>

          <div className="wallet-process-grid">
            <ProcessStep
              number="1"
              title={`Fund ${market.settlementSymbol}`}
              detail="Deposit canonical Ethereum mainnet USDC into the active governed vault, then proof-relay it into this PFTL wallet."
              state="ready"
              action="Open Ethereum bridge-in"
              onClick={() => onNavigate?.('bridge')}
              Icon={Landmark}
            />
            <ProcessStep
              number="2"
              title={`Mint or redeem ${market.symbol}`}
              detail={`Exchange ${market.settlementSymbol} for newly issued ${market.symbol} at the finalized pre-inflow NAV, or burn ${market.symbol} to receive ${market.settlementSymbol} from its settlement reserve.`}
              state={currentRoute ? 'ready' : 'blocked'}
              action={`Open ${market.symbol} primary market`}
              onClick={() => onNavigate?.('market', { marketKey: market.key })}
              Icon={ShieldCheck}
            />
            <ProcessStep
              number="3"
              title="Move assets on PFTL"
              detail={`Send ${market.settlementSymbol} or native ${market.symbol} to another PFTL account with locally signed, certified asset finality.`}
              state="ready"
              action="Send issued asset"
              onClick={() => onNavigate?.('send', { sendSource: 'asset' })}
              Icon={Send}
            />
            <ProcessStep
              number="4"
              title={`Private ${market.symbol} execution`}
              detail={privateReady
                ? `The resident Asset-Orchard service advertises ${market.symbol} ingress, private execution, and egress for this wallet.`
                : `The live browser service does not currently advertise an enabled ${market.symbol} private route. Operational test scripts are not presented as a user wallet feature.`}
              state={privateReady ? 'ready' : 'blocked'}
              action={privateReady ? 'Private route available' : ''}
              Icon={Lock}
            />
            <ProcessStep
              number="5"
              title="Private pfUSDC/pNOK FIX"
              detail="Exchange exactly 20.000000 pfUSDC for 210 sandbox WNOK-backed pNOK at the finalized zero-fee demo fix, with private execution on PFTL."
              state="ready"
              action="Open private FX"
              onClick={() => onNavigate?.('fx')}
              Icon={ShieldCheck}
            />
            <ProcessStep
              number="6"
              title={`Export ${market.symbol} to Ethereum`}
              detail={`The primary market can deliver ${market.wrappedSymbol} directly to MetaMask through the governed, proof-bound export route.`}
              state={currentRoute ? 'ready' : 'blocked'}
              action={currentRoute ? `Open ${market.symbol} export` : ''}
              onClick={() => onNavigate?.('market', { marketKey: market.key })}
              Icon={Landmark}
            />
          </div>
        </main>

        <aside className="pfs-side">
          <section className="pfs-card">
            <div className="pfs-route-head"><span>LIVE {market.symbol} ROUTE</span></div>
            <div className="pfs-detail-list">
              <div><span>Route</span><strong>{route?.route_id ? truncateMiddle(route.route_id, 10) : '—'}</strong></div>
              <div><span>Verified NAV</span><strong>{formatNavcoinNav(nav?.nav_per_unit)}</strong></div>
              <div><span>{market.settlementSymbol} reserve</span><strong>{formatNavcoinUnits(route?.settlement_reserve_atoms, market.settlementDecimals)}</strong></div>
              <div><span>Mint capacity</span><strong>{formatNavcoinUnits(route?.available_issue_atoms, market.decimals)}</strong></div>
              <div><span>Redeem capacity</span><strong>{formatNavcoinUnits(route?.available_redeem_atoms, market.decimals)}</strong></div>
              <div><span>PFTL height</span><strong>{snapshot?.chain?.block_height ?? '—'}</strong></div>
            </div>
          </section>
          <section className="pfs-card">
            <div className="pfs-route-head"><span>IMPORTANT</span></div>
            <p>
              “Bridge-in” means Ethereum USDC becomes {market.settlementSymbol} on PFTL.
              “Export” means native {market.symbol} becomes {market.wrappedSymbol} on Ethereum.
              They are different operations.
            </p>
          </section>
        </aside>
      </div>
    </div>
  );
}
