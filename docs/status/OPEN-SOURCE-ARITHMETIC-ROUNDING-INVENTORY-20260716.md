# Open-source arithmetic and rounding inventory — 2026-07-16

**Scope:** production Rust paths that can change balances, supply, custody,
fees, NAV, offers, bridge state, redemptions, heights, sequences, or canonical
identifiers. Circuit field arithmetic is classified separately because modular
field operations are intentional and are constrained by the circuit.

**Method:** source trace plus
`cargo clippy ... -- -W clippy::arithmetic-side-effects` over `types`,
`execution`, `bridge`, `storage`, `ordering_fast`, `privacy`,
`privacy_orchard`, `rpc_sdk`, and `node`. Every emitted production expression
was traced to its input guard or replaced with checked arithmetic. Ordinary
strict Clippy remains `-D warnings`; this audit lint is diagnostic because it
also reports intentionally modular curve/field operations.

## Confirmed defects found and closed

| Boundary | Pre-fix failure | Local remediation | Regression |
|---|---|---|---|
| Normal account sequence | `sequence + 1` panicked in debug and wrapped in release at `u64::MAX` | checked next sequence and deterministic `sequence_overflow` rejection before mutation/signing | execution and node-builder exhausted-sequence tests |
| RPC block adjacency | `previous_height + 1` panicked on an untrusted `u64::MAX` response | checked adjacency with typed SDK error | RPC SDK boundary test |
| FastSwap bootstrap height | `tip.height + 1` panicked at exhausted tip | checked height and fail-closed generator error | bootstrap boundary test |
| NAV/vault deadlines | caller-controlled height additions could overflow | saturating terminal deadlines with explicit exhausted-height behavior | NAV/vault deadline regressions |
| Native custody burn | FastLane debited a fee but reported no receipt burn | exact receipt fee/burn plus replay transition oracle | FastLane receipt and native replay tests |
| Issued custody cap | mint supply omitted issued FastLane reserves and live AssetOrchard balances | checked aggregate at execution admission and replicated-state proposal/commit/replay; finalized NAV supply also bounds the aggregate | `P0-ISSUED-SUPPLY-02` regressions |

## Monetary path classification

| Domain and code | Arithmetic/rounding contract | Classification |
|---|---|---|
| Native transfer/payment and state fees — `crates/execution/src/entrypoints.rs` | fee/amount totals use checked addition; balance subtraction follows an exact sufficient-balance test; sequence is checked; receipt burn is explicit | checked or guard-dominated |
| Issued balances and supply — `issued_asset_ledger_helpers.rs` | trustline, escrow, offer and FastLane totals use checked accumulation; credits and caps use checked addition; debits follow balance guards | checked or guard-dominated |
| Atomic swap — `entrypoints.rs`, `atomic_swap_execution_tests.rs` | both legs precompute checked debits/credits; whole transaction mutates only after both validate; supply is conserved; fee burn is exact | checked and atomic |
| FastLane/FastSwap — `fastswap_bridge.rs`, `fastswap.rs`, `fastswap_checkpoint.rs` | deposits/redeems move exact reserve amounts; reserve/liability sums are `u128` checked; quote division requires nonzero denominator and declared rounding; checkpoint burns are checked | checked with explicit floor/down/exact modes |
| Escrow/NFT/offer — `nft_escrow_asset_execution.rs`, `fees_offer_planning.rs` | subtraction is after ownership/balance checks; offer ratios use `u128`; GCD zero and zero price units fail closed; fill products are checked | guard-dominated and checked |
| NAV calculations — `market_nav_asset_types.rs`, `market_policy.rs` | floor uses integer division; claim/collateral requirements use checked ceil; unit scale and all divisors must be nonzero; BPS inputs are range checked; integer square root is bounded over `u128` | explicit floor/ceil policy |
| Vault bridge — `nav_vault_asset_execution.rs`, `vault_bridge_policy.rs` | counted value, allocation, redemption queue and settlement conversions use checked add/mul and preconditioned subtract; atom scaling declares floor/ceil direction | checked with explicit rounding |
| AssetOrchard — `orchard_state_application.rs`, `shielded_batch_actions.rs` | per-asset ingress/egress/live totals use checked add and guarded subtract; native turnstile identity is deposit = live + burn + withdrawal; global issued cap includes live private custody | checked and replay-verified |
| PFTL/Uniswap bridge ledger — `crates/bridge/src/lib.rs` | all live supply components and caps use checked helpers; status truncation subtraction follows `total > limit`; NAV seed conversion is explicit floor | checked or guard-dominated; external route remains contained by `P0-BRIDGE-01` |
| Native replay/history — `block_replay_wallet.rs`, `history.rs` | every block requires `live_before - receipt_burn == live_after`; checkpoint v2 requires `live + cumulative_burn == genesis_supply` | checked invariant |

## Diagnostic-warning disposition

The remaining `arithmetic-side-effects` diagnostics do not expose unchecked
monetary state:

- byte/hex encoders use `len * 2`, nibble subtraction, and `index + 1` only
  after bounded allocation/even-length/range checks;
- collection counters are bounded by explicit consensus or request limits
  before increment (`MAX_*` constants) or by already allocated vectors;
- slice offsets in SP1, WAL, ABI, and Orchard decoders are preceded by checked
  range/end calculations; malformed/truncated input returns a typed error;
- BFT quorum math first rejects empty/oversized validator sets, so subtraction,
  multiplication, and modulo operate inside a small validated domain;
- guarded balance/queue/reserve subtractions occur only after comparing the
  same immutable value in the same single-threaded state transition;
- elliptic-curve, scalar, Poseidon, and Halo2 expression arithmetic in
  `privacy_orchard` is intentionally modular field arithmetic. Range,
  booleanity, conservation, nonzero-inverse, tag, and rounding constraints are
  enforced by the circuit and its adversarial tests rather than machine-integer
  overflow checks;
- benchmark/test fixture increments do not enter production state.

Platform-dependent casts affecting protocol quantities were separately traced.
Consensus monetary values are fixed-width integers. Vector lengths enter
canonical state through bounded collections or checked fixed-width conversion;
no `usize` value is used as a balance, supply, NAV, or fee. The remaining
`usize` index arithmetic is local addressing over an already allocated vector.

## Rounding policy summary

- NAV per unit and executable quote-down amounts: floor.
- Required collateral, required claim value, input needed to move a pool, and
  minimum fees: ceil.
- Atomic-swap quote: the signed policy declares `exact`, `down`, or `up`; exact
  rejects a nonzero remainder.
- Offer matching: reduced integer ratio; fills only whole reduced-ratio units.
- BPS policy: integer floor/ceil helper selected by the named policy formula;
  inputs above 10,000 reject.
- No floating point enters consensus state or a signing/hash preimage.

## Evidence and remaining gate

- `cargo test -p postfiat-execution --lib -- --test-threads=1`: PASS `136/136`.
- targeted RPC SDK, bootstrap, state-root, FastLane reserve, AssetOrchard
  round-trip, native replay, and checkpoint regressions: PASS.
- `cargo check --workspace --all-targets --locked`: PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- `cargo fmt --all -- --check`: PASS.

This closes the checked-arithmetic and explicit-rounding inventory for supported
production monetary paths in the current candidate tree. It does not close the
separate external-bridge proof boundary, storage-scale work, or the remaining
external-audit gates. The supported issued mint/custody/burn/redemption
transition gate was subsequently closed in
`OPEN-SOURCE-ISSUED-SUPPLY-INVENTORY-20260716.md`.
