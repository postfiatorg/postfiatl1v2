# Open NAVCoin SDK (`@postfiat/navcoin-sdk` & `postfiat_rpc.navcoin_sdk`)

Neutral, exact-atom SDK, CLI, and cross-language test vector suite for Post Fiat L1 NAVCoins (Pillar 7, frozen L1 Upgrade Train B contract).

## Design Principles

- **100% optional fee schedules:** Launching a zero-fee NAVCoin omits `NavFeeScheduleV1` completely—no fee operations, no fee state, and zero fee clutter.
- **Locked protocol caps (when enabled):**
  - Annual time-based rate: `<= 500 bps` (`5% / year`)
  - High-water-mark gain rate: `<= 5,000 bps` (`50%`)
  - Primary issue / redeem spreads: `<= 500 bps` (`5%`)
  - FX fix fee cap: `<= 100 bps` (`1%`)
  - Minimum fee-increase notice: `>= 604,800` blocks (`7 days` at `31,536,000` blocks/year)
- **Universal NAV integrity:** Every NAVCoin (zero-fee or fee-enabled) enforces `nav_per_unit == floor(V * 10^precision / S)` with exact integer division and zero floating-point drift.
- **Byte-identical Python + JavaScript/TypeScript math:** Both `python/postfiat_rpc/navcoin_sdk.py` (arbitrary-precision `int`) and `sdk/navcoin/src/engine.mjs` (`BigInt`) match Rust `u256` intermediates to the single atom across all 120 vectors in [`vectors/nav_fee_vectors_v1.json`](vectors/nav_fee_vectors_v1.json).

## Exact 4-Step Epoch Accrual Formula

Given verified net assets $V$, circulating supply $S$, scale $U = 10^{\text{precision}}$, high-water mark $H$, applied annual rate $m$ (bps), applied HWM rate $p$ (bps), capped block delta $\Delta h = \min(h - h_{\text{last}}, \text{accrual\_cap\_blocks})$, and $Y = 31{,}536{,}000$:

1. **Time-based fee:** $F_m = \lfloor V \cdot m \cdot \Delta h / (10{,}000 \cdot Y) \rfloor$, $N_m = \lfloor (V - F_m) \cdot U / S \rfloor$
2. **High-water-mark gain fee:** $g = \max(0, N_m - H)$, $F_p = \lfloor g \cdot S \cdot p / (10{,}000 \cdot U) \rfloor$
3. **Atomic unit mint:** $F = F_m + F_p$, $\Delta S = \lfloor F \cdot S / (V - F) \rfloor$ (clamped to `max_supply` if set; forgone with zero carry-forward debt)
4. **State update:** $S' = S + \Delta S$, $N_{\text{after}} = \lfloor V \cdot U / S' \rfloor$, $H' = \max(H, N_{\text{after}})$

## Quick Start

### JavaScript / TypeScript

```js
import { createNavcoin, verifyNavPerUnit, inspectPendingHike } from "@postfiat/navcoin-sdk";

// 1. Launch a zero-fee NAVCoin (zero fee operations emitted)
const zeroFeePlan = createNavcoin({
  chainId: "postfiat-wan-devnet-2",
  issuer: "pf1111111111111111111111111111111111111111",
  code: "OPENNAV",
  displayName: "Open Reserve Unit",
  precision: 6,
});

// 2. Launch a fee-enabled NAVCoin (opt-in NavFeeScheduleV1)
const scheduledPlan = createNavcoin({
  chainId: "postfiat-wan-devnet-2",
  issuer: "pf1111111111111111111111111111111111111111",
  code: "FEENAV",
  displayName: "Scheduled Reserve Unit",
  precision: 8,
  feeSchedule: {
    annual_rate_bps: 100,
    hwm_rate_bps: 1000,
    issue_spread_bps: 50,
    redeem_spread_bps: 50,
    fix_fee_cap_bps: 25,
    recipient: "pf3333333333333333333333333333333333333333",
    increase_notice_blocks: 604_800n,
    accrual_cap_blocks: 86_400n,
  },
});

// 3. Verify exact universal NAV equation
verifyNavPerUnit({
  verifiedNetAssets: 1_100_000_000_000n,
  circulatingSupply: 1_000_000_000_000n,
  precision: 8,
  claimedNavPerUnit: 110_000_000n,
});
```

### Python

```python
from postfiat_rpc.navcoin_sdk import create_navcoin, verify_nav_per_unit, recompute_fees

zero_fee_plan = create_navcoin(
    chain_id="postfiat-wan-devnet-2",
    issuer="pf1111111111111111111111111111111111111111",
    code="OPENNAV",
    display_name="Open Reserve Unit",
)
```

### CLI

```bash
# Verify all 120 cross-language exact-atom vectors
node sdk/navcoin/bin/navcoin-cli.mjs verify-vectors

# Launch plan for a zero-fee NAVCoin
node sdk/navcoin/bin/navcoin-cli.mjs create \
  --chain-id postfiat-wan-devnet-2 \
  --issuer pf1111111111111111111111111111111111111111 \
  --code OPENNAV \
  --name "Open Reserve Unit"

# Verify universal floor(V * U / S)
node sdk/navcoin/bin/navcoin-cli.mjs verify-nav \
  --net-assets 1100000000000 \
  --supply 1000000000000 \
  --precision 8 \
  --claimed-nav 110000000
```

## Running Tests

```bash
# Node exact-atom vector & SDK test suite
node --test sdk/navcoin/test/nav_fee_vectors.test.mjs

# Python exact-atom vector & SDK test suite
PYTHONPATH=python pytest python/tests/test_navcoin_sdk.py
```
