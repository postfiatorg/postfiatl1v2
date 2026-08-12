import React from 'react';

import PftlUniswapPrimaryMarket from './NavcoinPrimaryMarket.jsx';

const MARKET_ADAPTERS = Object.freeze({
  'pftl-uniswap-primary-v2': PftlUniswapPrimaryMarket,
});

export default function NavcoinMarket({ market, ...props }) {
  const MarketAdapter = MARKET_ADAPTERS[market?.transactionAdapter];
  if (!MarketAdapter) {
    return (
      <div className="pf-page" data-testid="navcoin-market">
        <div className="pf-stage-inner" style={{ maxWidth: 680 }}>
          <div className="pf-eyebrow">Trade</div>
          <h1 className="pf-h1">Market temporarily unavailable</h1>
          <div className="pf-card" style={{ marginTop: 18, display: 'grid', gap: 12 }}>
            <strong>No active NAV market could be verified from the connected network.</strong>
            <p style={{ color: 'var(--muted)', fontSize: 13.5, lineHeight: 1.55 }}>
              Your assets have not moved. You can still inspect or send held assets while the market connection recovers.
            </p>
            <button className="pf-ghost" onClick={() => props.onNavigate?.('nav')}>View assets</button>
          </div>
        </div>
      </div>
    );
  }
  return <MarketAdapter market={market} {...props} />;
}
