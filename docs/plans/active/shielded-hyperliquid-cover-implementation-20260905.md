# Hyperliquid operator-cover implementation — paused

Task Node: `task_ed942369a9d92007a389dc32ffbefa5c` (accepted; reward unclaimed).
Research: [locked revision 3](../../specs/shielded-perp-venue-funding-privacy-hardening-20260905.md).
Locked SHA-256: `5fea0fa99d8372f7e09828b1f7cae081cbefa600d90d39aada75eedeaa626d48`.
The [denomination amendment](../../specs/shielded-funding-configurable-denomination-20260905.md)
authorizes a separate functional pilot at an explicit small amount; it preserves
the scored research and qualified-cover requirements.

## Paused — September 6

The user paused this effort after review of the whole-note public-amount
limitation and cover-capital economics. The intended inexpensive private
Hyperliquid funding objective remains unmet. The successful venue credits are
functional evidence, not demonstrated funding anonymity. Railgun-funded cover
was proposed but neither integrated nor executed. No new funding experiments
or development are scheduled. Remaining items below are deferred, not completed.
See the research retrospective in
`postfiatorg.github.io/content/research/shielded-funding-hyperliquid-lighter.md`.

## Retained result — September 6

Both authorized 0.005 ETH principals passed shielded ingress, private exit,
certified burn, finalized Ethereum release and native settlement. Full local
replay through block 992 matched all six validators. Lighter's exact 0.005 ETH
spot credit is complete. Hyperliquid's original Arbitrum deposit finalized and
its exact 12.466675 USDC credit was independently verified as withdrawable.
Both CLI runs and the desktop/mobile interface report completion.

The route uses no CEX. This pilot is `functional-unqualified`: common ownership
and a thin pool do not establish independent participation or demonstrated
anonymity. Qualified cover service remains open. Code paths below are in the
sibling StakeHub repository; live transaction/account mappings remain private.

The objective is private Hyperliquid funding without a 50 bps privacy tariff,
with expected use around $100,000 per transaction. This is not a minimum size.
The pilot spent 0.000066701467202275 ETH on Hyperliquid gas. Its high percentage
of a tiny test deposit does not block the objective. Cost accounting remains
informational; there is no all-in 50 bps admission requirement. The unfinished
objective is demonstrating funding anonymity through the integrated route.

## Implemented and verified

- [x] Research lock: first compliant 15-score average 86.20; research Task Node rewarded.
- [x] Configurable decimal principal with exact integer atom/wei binding through proofs, burns, bridge delivery and venue settlement; no invented 1 ETH or USD minimum.
- [x] Versioned exact observer models, partial matchings, subset DP, anchor membership, source/destination grouping and separate operator-aware view: `stakehub/shielded_exit_privacy.py`.
- [x] Encrypted isolated state, separate capital/fee reservations, finite independent schedules and pause/recovery: `shielded_exit_cover.py`, `shielded_exit_controller.py`.
- [x] Working functional funding controller with local private-note operations, pinned public-state import, ingress relay, burns/releases and shared sponsorship: `shielded_exit_funding.py`, `shielded_exit_runtime.py`.
- [x] Durable signed transaction identity, persistent sponsor stage reservations through finality, stale nonce detection and exact-hash recovery from rebroadcast races: `shielded_exit_bridge.py`, `shielded_exit_sponsor.py`.
- [x] Direct-EOA Ethereum Inbox deposit and exact finalized type-100 Arbitrum delivery. Aliased message sender and unaliased EOA recipient are verified separately.
- [x] Live bounded CPU SP1 proof backend with pinned native Gnark binary/program, retained work and deployed-verifier acceptance: `shielded_exit_public_cpu_prover.py`, `shielded_exit_resources.py`. Ordinary native workers retain a 48 GiB cap; public proofs explicitly select 96 GiB, zero swap and bounded runtime.
- [x] Both exact finalized releases and native settlements pass full replay through block 992 and six-validator agreement. Lighter's public proof used a verified direct-parent checkpoint and completed in about 24 minutes at 43.2 GiB peak memory.
- [x] Python CLI and loopback interface run/status/resume existing fixed jobs, display exact amounts and results, and reject arbitrary browser arguments: `shielded_exit_cover_cli.py`, `shielded_exit_cover_ui.py`, `dashboard/shielded-cover.html`.
- [x] Combined shielded-funding/dashboard Python suites: 388 passed, four native opt-ins skipped. Actual live proofs and full native replay are separate evidence. Strict StakeHub documentation build passed.
- [x] Funded Lighter probe: authenticated exact finalized Ethereum deposit and 0.005 ETH spot credit, margin disabled.
- [x] Funded Hyperliquid probe: original Arbitrum transaction finalized; exact 12.466675 USDC credit independently verified. CLI completed at 06:47 UTC; desktop/mobile UI shows both exact credits.
- [x] Refreshed activation code hashes and owner-only pilot completion evidence. The generic qualified-cover checklist remains separate from actual completed pilot results.
- [x] Finalized gas receipt audit and informational gas estimates in the Python CLI and local interface. The unrequested percentage admission restriction has been removed. Original transaction fee authorizations and settlement recovery remain intact.
- [x] Authenticated native cohort observations retain the full replay and real time intervals. Live block 992 has zero remaining pfETH notes; three historical notes/exits are retained: `shielded_exit_native_history.py`.
- [x] Read-only public lineage matched all three pfETH notes/exits to canonical Ethereum deposits/releases. The Hyperliquid tail independently matched the original 12.466675 USDC credit: `shielded_exit_lineage.py`.
- [x] Live cohort composition reports unresolved evidence explicitly; interval inference returns conservative posterior bounds. Fresh egress simulation and assessment bind the same native checkpoint: `shielded_exit_funding_history.py`, `shielded_exit_interval_privacy.py`, `shielded_exit_admission.py`.
- [x] Finite operator schedule adapter uses the actual funding driver, stable run identities and common sponsor. An operator-funded sponsor is reusable by a different principal source: `shielded_exit_funding_cover.py`, `shielded_exit_funding.py`.
- [x] Updated shielded-funding/dashboard Python verification: 443 passed, four native opt-ins skipped. Strict docs passed. No new deposits or proof-worker rentals were performed for these changes.
- [x] Live loopback interface exposes the read-only history actions. Desktop 1440 px and mobile 390 px verified the current zero-note cohort and preserved pilot credits, with no JavaScript errors or horizontal overflow.

## Deferred work — requires a new decision to resume

- [ ] Validate the history importer and common user/cover execution against an actual funded cohort, including ownership disclosures, venue behavior and later linkage. Empty-cohort coverage is not anonymity.
- [ ] Exercise exact-packet privacy assessment before a real user exit with observed cover. The research model's eight-candidate/six-hour defaults are model parameters, not a user-selected requirement or minimum deposit.
- [ ] Explicit cover-operator inventory, fee/proof budgets and finite holding capacity, distinct from the already authorized small user pilot.
- [ ] Funded history passes every selected observer model; disclose the operator-aware result and later-disclosure sensitivity separately.
- [ ] Measure sustained capacity, inventory, fees and recovery under the funded cover schedule.
- [ ] Demonstrate private funding without the 50 bps privacy tariff at the intended transaction scale; report gas, swaps and cover expenses separately. `shielded_exit_costs.py` provides informational accounting, not a percentage admission gate.
- [ ] Task Node initial evidence, requested verification and rewarded implementation outcome.
- [ ] Retire this milestone when the required qualified functionality and Task Node lifecycle are complete.

## Recovery references

The block 962 replay incident was a missing historical compatibility boundary
for base-to-family reserve validation, compounded by omitted bonded native PFT
custody. The repair preserves original receipts, block hashes and state roots
while enforcing the new rules on new execution. All six validators were updated
under the user's devnet authorization. See [replay repair evidence](../../status/shielded-funding-hardening-implementation-20260905/pfeth-replay-fix/README.md).

A stale temporary native ingress profile was restored by issuer-signed
registration at block 978. Funding preflight now verifies the original SP1
profile across the fleet. The shielded guard checks the natively recomputed v2
root against full replay and six exact live parents. Original paid deposits and
burns were retained throughout recovery. Temporary GPU, Docker ACL and runtime
overrides have been cleaned up. Operational commands and recovery behavior are
documented in StakeHub's `docs/current-sprint/hyperliquid-cover-pilot.md`.
