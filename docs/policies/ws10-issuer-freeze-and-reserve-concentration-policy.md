# WS10 — Issuer-freeze and reserve-concentration policy

Status: **policy documentation only**, authored 2026-07-25 by Ghash [orc] under
dispatch #75. No code, fleet, or Lane B/C artifact is changed by this document.
Governing spec: `docs/plans/A666-MAINNET-TRUSTLESS-MINT-SPEC-20260725.md`, WS10
(lines 261-266): record that Circle/Tether can administratively freeze vault
addresses on any chain, and set reserve-policy responses — per-issuer
concentration limits, haircuts, and a halt/deadman path (`nav_halt`).

Every numeric limit below is a **proposal requiring explicit user sign-off before
activation**, consistent with spec §8. Nothing here is active configuration, and
nothing here may be read as a claim that a control is already enforced on-chain.

## 1. The risk, stated plainly

Fiat-backed stablecoin reserves are administratively controllable by their
issuer. Circle (USDC) and Tether (USDT) can freeze or blacklist any address,
including a bridge vault address, on any chain where the token is deployed, at
their discretion or on legal order. When that happens the vault still *holds* the
balance but can no longer *move* it.

No proof system mitigates this, and PFTL must not imply otherwise:

- An SP1/Groth16 reserve or ingress proof attests to observed chain state — a
  finalized balance, a deposit log, an execution state root. It proves the tokens
  are *there*; it cannot prove they are *spendable*.
- A freeze therefore leaves every proof in the system valid while redemption
  economics have changed. The failure is an availability and solvency failure of
  the reserve, not a proof failure.
- Consequences: redemption at the floor cannot be honoured from the frozen
  bucket; the posted NAV overstates realisable value; naive continued minting
  against a frozen reserve dilutes existing holders.

Corollaries the policy accepts: freeze risk is per-issuer and per-chain, cannot
be diversified to zero while any fiat-backed reserve is held, and is
uninsurable inside the protocol. It is managed by concentration limits,
valuation haircuts, and a fail-closed halt.

## 2. Detection signals

The policy relies only on signals this stack already produces:

| Signal | Source available today |
| --- | --- |
| Vault balance divergence from the proven reserve packet | reserve packet / NAV epoch readback versus a direct vault balance read |
| Proof staleness | route profile `max_snapshot_age_blocks`, `max_epoch_gap_blocks` |
| Attestation shortfall | route profile `min_attestations` |
| Valuation drift | route profile `tolerance_bp` against `valuation_policy_hash` |
| Challenge/settlement stall | route profile `challenge_window_blocks`, `settle_deadline_blocks` |
| Issuer freeze itself | operator readback of the issuer's own blacklist view for the vault address on each chain where a reserve asset is held |

Requirement: the issuer-freeze readback runs on the same cadence as NAV epoch
publication, once per epoch minimum, for every vault address in the backing set,
and its result is recorded in the epoch evidence bundle. A missing or failed
readback is treated as an unknown, and an unknown is treated as a freeze for
haircut and halt purposes (fail-closed).

## 3. Per-issuer concentration limits (proposed, sign-off required)

Limits are expressed as a share of counted NAV backing value, evaluated at every
epoch publication and before any cap increase:

| Dimension | Proposed limit | Rationale |
| --- | --- | --- |
| Single issuer (e.g. Circle USDC) | ≤ 70 % of counted backing | one administrative action cannot immobilise the whole reserve |
| Second issuer (e.g. Tether USDT) | ≤ 30 % | keeps a second, independently controlled leg |
| Any single vault address | ≤ 50 % | limits blast radius of one address-level freeze |
| Any single chain | ≤ 80 % | preserves an off-chain-of-failure remainder |
| Non-issuer-freezable buffer (e.g. native ETH or an unfreezable asset held for gas/ops) | ≥ 2 % target, informational | keeps operational liveness during a freeze |

Enforcement points, in order of preference: (1) refuse to raise the finalized NAV
circulating-supply cap while any limit is breached; (2) refuse new primary mint
against the breaching bucket; (3) require rebalancing before the next epoch is
finalized. A breach is never resolved by relaxing the limit inside an incident.

Concentration is measured on counted (post-haircut) value, not gross balance, so
a haircut cannot be used to hide a breach and a breach cannot be hidden by
re-labelling the same issuer across chains: aggregation is by *issuer*, then by
*vault address*, then by *chain*.

## 4. Haircuts (proposed, sign-off required)

Haircuts reduce the value counted into NAV backing; they never change the
recorded balance, and they are applied inside the valuation policy so that the
resulting `valuation_policy_hash` binds the haircut set into the proven epoch.

| Reserve state | Counted value |
| --- | --- |
| Fiat-backed stablecoin, healthy, fresh proof, freeze readback clean | 100 % less the standing issuer haircut (proposed 0.5 % for the primary issuer, 1.5 % for the secondary) |
| Freeze readback unavailable or stale beyond `max_snapshot_age_blocks` | 50 % |
| Issuer freeze confirmed for the vault address | **0 %** |
| Concentration limit breached | excess above the limit counted at 0 % |
| Depeg observed beyond `tolerance_bp` versus the valuation policy | counted at the observed market price, never at par |

Rules that make haircuts meaningful: haircuts only ever decrease counted value
inside an incident; restoring a haircut to a lower level requires a fresh proven
epoch plus explicit sign-off; and a haircut is never applied retroactively to
understate an already-settled redemption obligation.

## 5. The `nav_halt` deadman path

The protocol already carries the halt primitive this policy depends on:

- Operation `nav_halt` — `NAV_HALT_TRANSACTION_KIND` in
  `crates/types/src/core_chain.rs:31`, payload `NavHaltOperation { issuer,
  asset_id, halted, reason }` in
  `crates/types/src/transactions_mempool_receipts.rs:912`. Validation requires a
  well-formed issuer and asset id and a **non-empty reason whenever `halted` is
  true** (same file, lines 920-927), so a halt is always attributable.
- Effect: while an asset is halted, execution fails closed — `nav_asset_halted`
  and `pftl_uniswap_nav_asset_halted` rejections in
  `crates/execution/src/nav_vault_asset_execution.rs:4115` and `:5592`, carrying
  the recorded halt reason. Mint/subscribe paths stop; the halt is not advisory.
- The halted flag is visible in the live NAV profile surface (`halted` /
  `halt_reason`), so operators and external verifiers observe the same state.

Deadman triggers — any one of these fires a `nav_halt` for the affected asset,
without waiting for consensus among operators:

1. Confirmed issuer freeze on any vault address in the backing set.
2. Freeze readback missing for longer than one epoch (unknown treated as freeze).
3. Proof staleness beyond `max_snapshot_age_blocks` or an epoch gap beyond
   `max_epoch_gap_blocks`.
4. Attestations below `min_attestations` for the current epoch.
5. Reserve divergence: proven reserve versus vault readback outside
   `tolerance_bp`.
6. Prover or attestation-operator outage that prevents publishing the next epoch
   inside its cadence.
7. Concentration breach that cannot be remediated before the next epoch.

Halt semantics: halting is cheap, unhalting is expensive. A halt requires only
the issuer signature plus a reason; an unhalt requires (a) a fresh proven NAV
epoch, (b) a reserve reconciliation showing counted value ≥ outstanding
obligations after haircuts, (c) freeze readback clean for every vault address,
(d) concentration limits satisfied, and (e) explicit sign-off recorded in the
incident evidence. Redemption policy during a halt is a sign-off decision, not an
implementation detail of this document; what this document fixes is that new
supply cannot be created while halted.

## 6. Incident runbook (documentation-level)

1. Record the trigger, the affected issuer/asset/vault, and the evidence hashes.
2. Issue `nav_halt` with a specific reason string; confirm the halted flag is
   observable on all six validators.
3. Re-run the reserve reconciliation with the freeze haircut applied and publish
   the resulting counted value and coverage ratio.
4. Freeze cap growth; no cap increase and no primary mint while halted.
5. Rebalance or replace the affected reserve leg within the concentration limits.
6. Unhalt only against the five criteria in §5, with the evidence bundle
   attached.
7. Post-incident: record whether limits or haircuts were adequate, and propose
   changes for sign-off — never adjust them silently during the incident.

## 7. Limits of this policy

- It is documentation. It adds no enforcement code, and its numbers are inert
  until signed off and configured.
- It cannot restore access to reserves that are already frozen; it bounds
  exposure, prices the risk honestly, and stops new supply.
- It makes no claim of trustlessness for any fiat-backed reserve: an issuer
  freeze is an accepted, disclosed, unmitigable dependency of holding that asset.
