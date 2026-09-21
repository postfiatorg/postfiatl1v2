# Swap and settlement execution review — 2026-09-21

A3 of the [burn 6 campaign](qa-campaign-20260921-burn6-brief.md), reviewed
from clean `burn6-work` at `cf12b64f` after the required release-branch pull.
All 8,797 lines of the three named files were read:
`crates/execution/src/nav_vault_asset_execution.rs` (8,527),
`crates/execution/src/pftl_source_settlement.rs` (113), and
`crates/execution/src/vault_bridge_profile_resolution.rs` (157).
Focus: subscription/redemption conservation, fee and spread rounding,
reservation and entitlement release, replay, source/family binding, and
ordering within a block. References below identify the pre-repair revision.

## Findings

### SWX-01 — P2 — public redemption omits the policy NAV-age limit

**Source:** `crates/execution/src/nav_vault_asset_execution.rs:5410-5425`,
with the general freshness check at `6465-6523` and the corresponding
subscription check at `4768-4784`.

**Condition:** An otherwise valid public primary redemption executes after
`finalized_at_height + primary_market_policy.max_nav_age_blocks`, while the
NAV packet remains inside `MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS` and both the
policy and transaction remain unexpired. For example, a packet finalized at
height 10 under a five-block policy is stale at height 16.

**Observed behaviour:** `apply_pftl_uniswap_primary_redeem` checks the general
window, epoch and packet identity, then prices and pays the redemption. It
does not enforce the policy's shorter NAV-age limit. Public subscription and
private redemption explicitly enforce that limit. The public path can
therefore burn NAV and release reserve principal at a policy-stale price.
This finding is established by code inspection; the repair unit will record
the regression result separately.

**Expected behaviour:** Reject with `stale_pftl_uniswap_policy_pricing` before
any custody, balance, supply, nonce or receipt mutation. Accept the exact
freshness boundary when all other checks pass.

**Suggested minimal change:** Apply the existing checked policy-freshness
calculation to public primary redemption. Reuse the calculation without
changing subscription/private-path semantics. Test the last fresh height,
the first stale height, unchanged state on rejection, and pooled and
source-specific settlement. **Consensus-affecting: yes:** an accepted state
transition becomes rejected. Re-qualify the release tip before deployment;
this review does not qualify or deploy it.

No P1 or P3 finding was established.

## Areas with no findings

- Subscription allocation and NAV mint/redeem settlement: receipt/bucket/asset
  and recipient binding, one-time retirement, released-capacity accounting,
  source-series selection, top-up capacity and supply-allocation return.
  Checked integer conversions and reserve/supply bounds reject overflow.
- Route subscription, reservation, entitlement release and burn accounting:
  settlement maximum enters custody before issuance; subscription separates
  principal, spread and refund; cancelling a reservation refunds custody,
  while releasing an export entitlement does not refund paid settlement.
  Nonces and terminal state fence repeated consumption. Route validation
  remains a delegated invariant boundary.
- `pftl_source_settlement.rs`: exact family/source identity and precision,
  active par-backed buckets, governed issue admission, source-specific
  escrow/refund/principal/spread, and pooled redemptions excluding source
  principal. These mutating helpers rely on their caller's trial-ledger
  rollback; helper errors alone do not promise atomicity.
- `vault_bridge_profile_resolution.rs`: registered-profile shape, source
  domain and route-policy matching, issuer/operator ownership for historical
  resolution, current-profile disambiguation and fail-closed ambiguity.
- Export/refund/return and vault burn/settle paths: terminal-state checks,
  source/bucket binding, checked supply and queue movement, withdrawal
  observation binding, and deterministic allocation-release order. Transaction
  order may legitimately determine capacity availability; no additional
  ordering defect was established in the named functions.

## Review limits and skips

Only the three named implementation files received a correctness review.
Selected fixtures and assertions in `market_nav_execution_tests.rs` were
read for test construction and coverage, not as a fourth whole-file audit;
`lib.rs` was inspected only for test/module placement. The burn 5 swap review
supplied the report format. Repository orientation documents were read.

Not reviewed: outer execution dispatch/authorization/rollback, type validators,
canonical route hashing and state roots, issued-family supply helpers,
storage/replay, node/RPC workflows, external source authenticity, or delegated
SP1/Groth16 and Orchard verification. Reading their calls in the permitted
file does not establish those implementations' correctness. Legacy generic
SP1 commitment-only and attestation paths are not new proof-verification
evidence. No malformed witness or live source event was constructed.

The private-primary route functions were read only within the permitted file;
their callers, private custody and proof/nullifier coupling remain excluded.
No repair to a private path or proof system is proposed. A1/A2 were not
re-reviewed; A4/A5 and B were not started. No inventory edit or scoring.
No Task Node, fleet, RPC, deployment, spend, signup or installation action.
Frozen artifacts and the excluded release checkout were untouched. Network
use is git only. Full-suite and historical-release qualification verdicts
remain separate from focused local verification.
