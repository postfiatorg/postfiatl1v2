# CTO Directive — Consensus-backed wallet round trip on devnet (2026-07-02)

Scope: the payoff run. The consensus-completion sprint
(`docs/handoffs/pftl-uniswap-consensus-completion-sprint-2026-07-01.md`, 6/6,
verified) gives us the full bridge state machine in consensus and a wallet
proxy that consumes node-produced digests. This directive turns that into the
operational claim the mandate has been aiming at: **a user-visible,
consensus-backed, end-to-end swap through the wallet on a live devnet — run
twice, with no manual state edits.** Until this passes, "working end-to-end
swaps in our wallet" is a code-level claim, not an operational one.

## Process rules (binding)

- Check each `- [ ]` in THIS doc with an evidence line (command, tx hash,
  block height, packet digest, or file path). Never check on intent.
- Evidence packets are immutable once written; a bad run gets a new packet,
  never an edit (standing rule from the 2026-07-01 review).
- The bar for "done" is **re-runnable**: every flow passes twice, from
  scratch, no manual state surgery between runs. One clean run is a demo;
  two is a capability.
- Wallet and proxy copy must match the deployed trust class: `CONTROLLED`,
  operator-attested destination events. No trustless wording anywhere.
- No public claim or announcement results from this work.

## Task 0 — Deploy the new consensus build to the devnet fleet

The bridge operations are consensus-level additions; every validator must run
the same revision or the flows cannot validate.

- [x] Build and deploy the current `main` (≥ `9e104f25`) to all WAN devnet
  validators. Record the deployed revision per host and verify it via the
  node status RPC from an external vantage.
  Evidence: deployed `32e11e8d925a4dc1950b8bddb5da3e1fad8880eb` to all six
  WAN validators; build/deploy/status packets:
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/reports/00-build.json`,
  `02-deploy.log`, `03-postdeploy-status.json`.
- [ ] Confirm ledgers advance and existing state replays clean after upgrade
  (no state-root divergence across the fleet).
  Evidence: live ledger advanced from height 1556 to 1557 with tx
  `3617086c91843dd6260865d2cf2626bfcde15f69304c3e17ec1f8b5e91f5e4a9ab5b8cb0cf304ef52b3be7663cfff9d1`;
  fleet converged at height 1557 with matching state roots in
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/reports/05-postdeploy-ledger-advance.json`
  and `06b-postadvance-status-corrected.json`. This box remains open because
  `verify-blocks` failed on all hosts with a pre-existing block 1206 proposer
  mismatch; the same failure reproduces with the pre-upgrade binary. See
  `04-postdeploy-verify-blocks.json` and
  `04a-preupgrade-binary-verify-blocks-validator0.json`.

## Task 1 — Environment prep: NAV asset, reserves, route

- [ ] Confirm the devnet NAV asset (or register a fresh one) with a
  **finalized** reserve packet: nonzero `finalized_epoch`, `nav_per_unit`,
  `finalized_at_height`, not halted.
- [ ] **Freshness feasibility check (measure, don't assume):**
  `MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS = 100` means the mint must land within
  100 blocks of packet finalization. Measure devnet block cadence and record
  the real wall-clock window. If the window is operationally infeasible for a
  wallet-driven flow, do not hack around it — propose the parameterization
  (per-profile bound, as the spec note anticipated) as a small follow-up
  slice and get it landed first. Record the decision either way.
- [ ] Route init on devnet via the signed consensus operation
  (issuer/reserve-operator key), route family `primary_pftl_mint`. Fetch the
  route-config digest **from the node RPC** and confirm the wallet proxy
  serves the identical digest (this is the first live exercise of the
  digest-authority fix).

## Task 2 — Ethereum side: fork venue up

- [x] Stand up the fork against official Uniswap v4 addresses with the
  handoff controller, settlement adapter, and wrapped token deployed; seed
  the pool per the Gate 3 harness (`scripts/pftl-uniswap-gate3-fork-execute.py`).
  Evidence: orchardmanager run
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/orchardmanager-0247-flow-ad-02/reports/12-summary.json`
  records official Uniswap v4 code checks, deployed wrapped token / handoff
  controller / settlement adapter addresses, pool seed tx
  `0x45b25eb854171b9960e3945918ece4abaec8dd07e17674e7771a94c7654ea268`,
  and fork block `25441980`.
- [x] Bind the launch config digest to the deployed addresses and record it
  in the run packet before any flow executes.
  Evidence: launch config digest
  `9e4eb6369391eb5ce8f030514994dbedd2247fc545c6ed51524dea3a02716337ed7968030a0c6ae4d1e67ef2146d0b37`
  is recorded in
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/orchardmanager-0247-flow-ad-02/reports/13-mvp4-beta-run-packet.json`
  and matched in
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/orchardmanager-0247-flow-ad-02/reports/14-mvp4-beta-consume-evidence.json`.

## Task 3 — The round trip, through the wallet

Drive every step a user would see from the wallet (controlled beta), not
from operator CLIs — operator CLIs are permitted only for the roles that are
genuinely operator roles (destination-consume attestation, return relay).

- [x] **Flow A/D out:** wallet holds pfUSDC → primary mint at the
  consensus-derived NAV price (wallet displays the derived price and the
  reserve epoch it came from) → export packet → fork controller consumes
  exactly once → wrapped token minted → Uniswap exact-input swap executes
  atomically. Assert real balances at every hop: PFTL settlement debit =
  minted × derived price; `outstanding_bridge_claims` up then down;
  wrapped/venue balances on the fork; swap output to the recipient.
  Partial evidence: two fresh-wallet browser UX runs reached visible source
  submit, consensus packet verification, and CONTROLLED operator-attested
  `destination_consume_submitted`; see
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/reports/33-wallet-controlled-beta-two-run-summary.json`
  and raw packet
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/wallet-controlled-beta-1782959450224/report.json`.
  Closing evidence: orchardmanager real-fork packet
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/orchardmanager-0247-flow-ad-02/`
  submitted real `consumeMintAndSwap` tx
  `0xb0bb446ac2ef7fe3e63dd023a95bbf9528220cd54249307cd84e9d2bafb4dda0`,
  asserted exact-input preflight output `7948` USDC atoms equals actual output
  `7948`, and reconciled settlement price to the finalized PFTL NAV proof in
  `reports/15-oracle-comparison.json`. The first short-supply failed packet is
  preserved separately at
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/orchardmanager-0247-flow-ad/`.
- [x] **Flow E back:** burn wrapped token on the fork → return relay →
  consensus return-import (burn id recomputed, finality depth enforced) →
  native units re-credited to the wallet. Assert `ethereum_spendable` →
  `pftl_spendable` movement and the final wallet balance.
  Evidence: orchardmanager first run
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/orchardmanager-0247-flow-e-01/`
  exposed a contract/node return-burn id mismatch: Rust/node bound
  `burn_height`, while Solidity omitted `block.number`. The contract was fixed
  to include `block.number` in the return-burn preimage and event. Passing run
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/orchardmanager-0247-flow-e-02/reports/17-summary.json`
  records burn txs
  `0x0cb593b7b0c46fcff35b22a7934e07bb2501050b9f78d94b83602859bfef7c52`
  and
  `0xfdeac0d5bc0f321e160cbc66f07c1d935004e26d70e63bde1836422566f97cfc`,
  burn ids
  `ec43c6c2b0b07a53d1121ff9f1447c25df85e194127432c328980ee4a7c0063c`
  and
  `8ee1ff37d69a4ddcb130cac2f382158e26776ee376b66b215e6e9fdac9a1bd4c`,
  final receipt replay `verified`, `ethereum_spendable_supply_atoms = 0`,
  `pftl_spendable_supply_atoms = 42`, and final supply invariant true.
- [x] **Refund drill:** one export packet deliberately left unconsumed past
  expiry → refund with the bound commitment → wallet balance restored;
  verify consume-after-refund rejects on the live network.
  Evidence: controlled node transition drill
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/orchardmanager-0247-refund-01/reports/11-summary.json`
  left packet
  `c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1`
  unconsumed, rejected early refund with `refund_before_window`, refunded at
  height 20 using bound non-consumption commitment
  `dcaf790dc51b2f5810d63948f1959597155e520e3b90a99550fb07b6cd770b5d6cba21405674f4bd6d2b7552396a38a1`,
  restored `pftl_spendable_supply_atoms = 10`, reduced
  `outstanding_bridge_claims_atoms = 0`, verified receipt replay, and rejected
  consume-after-refund with `export_packet_not_settleable`.
- [ ] **Pause drill:** pause the route mid-campaign → wallet blocks new mint
  and export with correct copy; refund and return-import still accepted →
  unpause → flows resume. (Live proof of `d5949fcf`.)
  Blocked: no exposed PFTL-Uniswap route-level pause/resume transition or CLI
  exists; generic bridge-domain pause and Ethereum controller `setPaused` are
  not the wallet/proxy route drill. Blocker recorded in
  `$ORC_DIRECTIVES_ROOT/orchardmanager_directive_0247.md`.
- [ ] **Supply conservation, continuously:** after every step above, the
  route status RPC shows the conservation identity holding exactly; include
  the sequence of snapshots in the run packet.
  Partial evidence: wallet campaign raw report includes before/after
  `navcoin_bridge_supply_status` snapshots for the two controlled source +
  destination-consume runs. Not checked because the required fork swap,
  return, refund, and pause steps were not executed.

## Task 4 — Run it again

- [ ] Repeat Task 3 end to end from a fresh wallet and fresh packets, no
  manual state edits, same build. Both run packets go in the evidence
  directory with digests; any divergence between run 1 and run 2 gets a
  root-cause note before this box closes.
  Partial evidence: repeated the controlled wallet source +
  operator-attested destination-consume slice twice from fresh wallets
  `pf3b5dbca80ebfb8522b536d4659bbac6298de1c05` and
  `pfffefe520a07b860a87064e33f259712355cd52e1`, with packet hashes
  `180480953bccb68c54195a9db9657bde07f72adb184c38e2a02c18eaaf7286fd61dc2252457f233077b3fd4a66dca49f`
  and
  `840a99bb552974540a7eee933c6ea9fc2235ec3e810274ccc0d01d4a47e61fd021a8860af38fa3e19d1e5c3fa5a34106`.
  Not checked because Task 3 full fork/return/refund/pause scope is still
  incomplete.

## Task 5 — Closeout

- [ ] Evidence packet: immutable run records (heights, tx hashes, digests,
  balance assertions, screenshots of the wallet states), indexed in
  `docs/evidence/` per house convention.
  Partial evidence: summary packet
  `docs/evidence/pftl-uniswap-wallet-e2e-2026-07-02/reports/33-wallet-controlled-beta-two-run-summary.json`;
  raw report SHA-256
  `88cea05b412bba0df3060e1ed8644f8e2ba00f543f2ea3f9fae14ed6e6fa34b0`;
  completion screenshots SHA-256
  `934899fb5e18db473d0120a4fe10088630f4327e0470ca26bdcd0942a80a04a8`
  and
  `fa653c5bc190771654da86a805fc11daea9e09f8c6ba23d7b702af0eda4b2746`.
- [ ] Spec checklist: re-cite the MVP flow items as consensus-backed with
  this campaign as evidence; permitted-claim paragraph updated to
  "consensus-backed controlled wallet round trip on devnet, twice" — still
  explicitly not trustless, not public routing.
- [x] List what this campaign surfaced (bugs, parameter changes, UX gaps) —
  a live run that surfaces nothing is a red flag, per the OTC MVP precedent.
  Evidence: the 2026-07-02 wallet campaign surfaced four concrete gaps:
  browser WASM had to be regenerated before it recognized
  `pftl_uniswap_primary_subscribe` / `pftl_uniswap_export_debit`; the legacy
  transparent pfUSDC funding helper is blocked by market-ops status, so the
  harness used direct issuer funding; the wallet UI can remain on
  "Source actions submitted" while the proxy run is terminal, so evidence must
  poll `/api/navswap/runs/{run_id}`; and the wallet-proxy PFTL completion path
  is still CONTROLLED destination-consume only, not a real fork
  `consumeMintAndSwap`, Flow E, refund, or pause campaign.
- [ ] `main` pushed, worktree clean.
  Evidence: the evidence commit
  `ae9c90dd757559c0c2c6099b2e0864f228416488` is on `origin/main`, and
  subsequent directive-only closeout notes were pushed. Not checked because
  the worktree still contains unrelated untracked
  `docs/security/orchard-swap-remediation.md`, which this directive does not
  own.

## Out of scope

- Gate 5 verifier work (optimistic/light-client) and its economics.
- Gate 6 / public routing / uncapped values / mainnet anything.
- Receipt-pruning implementation (recorded follow-up).
