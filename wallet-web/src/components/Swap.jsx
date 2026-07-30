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

import {
  A666_NATIVE_ASSET_ID,
  A666_PRIMARY_ROUTE_ID,
  A666_SETTLEMENT_ASSET_ID,
  formatA666Nav,
  formatA666Units,
} from '../lib/a666-primary-route.js';
import { truncateMiddle } from '../lib/utils.js';

function responseResult(response, label) {
  if (!response?.ok || !response.result) {
    throw new Error(response?.error?.message || `${label} is unavailable`);
  }
  return response.result;
}

function routeIsCurrent(route, nav) {
  return Boolean(
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
}

function privateA666IsReady(capabilities) {
  const route = capabilities?.routes?.shielded_navswap;
  const assets = Array.isArray(route?.asset_registry) ? route.asset_registry : [];
  return Boolean(
    route?.enabled === true
    && route?.can_run === true
    && route?.can_ingress === true
    && route?.can_egress === true
    && assets.some(asset => (
      String(asset?.asset_id || '').toLowerCase() === A666_NATIVE_ASSET_ID
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

export default function Swap({ rpc, swapServer, onNavigate }) {
  const [snapshot, setSnapshot] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const refresh = useCallback(async () => {
    if (!rpc) return;
    setLoading(true);
    setError('');
    try {
      const [routeResponse, navResponse, statusResponse, capabilities] = await Promise.all([
        rpc.navcoinBridgeSupplyStatus(A666_PRIMARY_ROUTE_ID),
        rpc.vaultBridgeStatus(A666_NATIVE_ASSET_ID),
        rpc.status(),
        swapServer?.getNavswapCapabilities?.().catch(() => null) || null,
      ]);
      setSnapshot({
        route: responseResult(routeResponse, 'A666 primary route'),
        nav: responseResult(navResponse, 'A666 NAV'),
        chain: responseResult(statusResponse, 'PFTL status'),
        capabilities,
      });
    } catch (failure) {
      setError(failure.message || 'Unable to verify the current wallet process');
    } finally {
      setLoading(false);
    }
  }, [rpc, swapServer]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const route = snapshot?.route;
  const nav = snapshot?.nav;
  const currentRoute = routeIsCurrent(route, nav);
  const privateReady = privateA666IsReady(snapshot?.capabilities);

  return (
    <div className="pf-page pf-swap-page">
      <div className="pfs-shell wallet-process-shell">
        <main className="pfs-main">
          <header className="pfs-header">
            <div className="pf-eyebrow">Current wallet process</div>
            <h1>USDC → pfUSDC → A666</h1>
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
                    ? `${formatA666Nav(nav?.nav_per_unit)} NAV · epoch ${route?.pricing_nav_epoch}`
                    : 'The A666 route or NAV packet is unavailable or mismatched.'}
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
              title="Fund pfUSDC"
              detail="Deposit canonical Ethereum mainnet USDC into the active governed vault, then proof-relay it into this PFTL wallet."
              state="ready"
              action="Open Ethereum bridge-in"
              onClick={() => onNavigate?.('bridge')}
              Icon={Landmark}
            />
            <ProcessStep
              number="2"
              title="Mint or redeem A666"
              detail="Exchange pfUSDC for newly issued A666 at the finalized pre-inflow NAV, or burn A666 to receive pfUSDC from its settlement reserve."
              state={currentRoute ? 'ready' : 'blocked'}
              action="Open A666 primary market"
              onClick={() => onNavigate?.('a666')}
              Icon={ShieldCheck}
            />
            <ProcessStep
              number="3"
              title="Move assets on PFTL"
              detail="Send pfUSDC or native A666 to another PFTL account with locally signed, certified asset finality."
              state="ready"
              action="Send issued asset"
              onClick={() => onNavigate?.('send', { sendSource: 'asset' })}
              Icon={Send}
            />
            <ProcessStep
              number="4"
              title="Private A666 execution"
              detail={privateReady
                ? 'The resident Asset-Orchard service advertises A666 ingress, private execution, and egress for this wallet.'
                : 'The live browser service does not currently advertise an enabled A666 private route. Operational test scripts are not presented as a user wallet feature.'}
              state={privateReady ? 'ready' : 'blocked'}
              action={privateReady ? 'Private route available' : ''}
              Icon={Lock}
            />
            <ProcessStep
              number="5"
              title="Bridge A666 to Ethereum"
              detail="The route binds native A666 to wA666 on Ethereum, but this wallet does not yet expose the export transaction and proof lifecycle. Buying A666 does not automatically create wA666."
              state="blocked"
              Icon={Landmark}
            />
          </div>
        </main>

        <aside className="pfs-side">
          <section className="pfs-card">
            <div className="pfs-route-head"><span>LIVE A666 ROUTE</span></div>
            <div className="pfs-detail-list">
              <div><span>Route</span><strong>{route?.route_id ? truncateMiddle(route.route_id, 10) : '—'}</strong></div>
              <div><span>Verified NAV</span><strong>{formatA666Nav(nav?.nav_per_unit)}</strong></div>
              <div><span>pfUSDC reserve</span><strong>{formatA666Units(route?.settlement_reserve_atoms)}</strong></div>
              <div><span>Mint capacity</span><strong>{formatA666Units(route?.available_issue_atoms)}</strong></div>
              <div><span>Redeem capacity</span><strong>{formatA666Units(route?.available_redeem_atoms)}</strong></div>
              <div><span>PFTL height</span><strong>{snapshot?.chain?.block_height ?? '—'}</strong></div>
            </div>
          </section>
          <section className="pfs-card">
            <div className="pfs-route-head"><span>IMPORTANT</span></div>
            <p>
              “Bridge-in” means Ethereum USDC becomes pfUSDC on PFTL.
              “Bridge-out” means native A666 becomes wA666 on Ethereum.
              They are different operations.
            </p>
          </section>
        </aside>
      </div>
    </div>
  );
}
