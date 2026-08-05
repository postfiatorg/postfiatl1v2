import React from 'react';

import PftlUniswapPrimaryMarket from './NavcoinPrimaryMarket.jsx';

const MARKET_ADAPTERS = Object.freeze({
  'pftl-uniswap-primary-v2': PftlUniswapPrimaryMarket,
});

export default function NavcoinMarket({ market, ...props }) {
  const MarketAdapter = MARKET_ADAPTERS[market?.transactionAdapter];
  return (
    <div data-testid="navcoin-market">
      {MarketAdapter
        ? <MarketAdapter market={market} {...props} />
        : (
          <div className="pf-page">
            <div className="pf-error">This governed NAVCoin route has no installed wallet transaction adapter.</div>
          </div>
        )}
    </div>
  );
}
