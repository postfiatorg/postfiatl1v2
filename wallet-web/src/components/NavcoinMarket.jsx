import React from 'react';

import A666MainnetMarketAdapter from './A666Market.jsx';

const MARKET_ADAPTERS = Object.freeze({
  'a666-mainnet-v2': A666MainnetMarketAdapter,
});

export default function NavcoinMarket({ market, ...props }) {
  const MarketAdapter = MARKET_ADAPTERS[market?.transactionAdapter];
  if (!MarketAdapter) {
    return (
      <div className="pf-page">
        <div className="pf-error">This governed NAVCoin route has no installed wallet transaction adapter.</div>
      </div>
    );
  }
  return <MarketAdapter market={market} {...props} />;
}
