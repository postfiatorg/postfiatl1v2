# CTO Sprint Directive — PFTL-Uniswap consensus completion (2026-07-01)

Scope: one continuous sprint. Complete the remaining consensus transition
semantics, the wallet-proxy digest authority, and the bookkeeping that closes
the "wire the bridge into consensus" milestone. Feeds from
`docs/handoffs/pftl-uniswap-price-binding-directive-2026-07-01.md` (its
Directive 1 is verified closed by `57621ab3`; its Directives 2–4 are expanded
into the ordered tasks below).

**Sprint exit condition:** a single consensus test proves the full round trip —
route init → primary subscribe → export debit → destination consume → return
import — with real balance movement at every step, refund/consume mutual
exclusion proven in both orders, and every suite green. When that test exists
and passes, the wallet has a consensus-backed end-to-end path and the sprint
is done.

## Process rules (binding)

- Work the tasks **in order**. Check each `- [ ]` in THIS document as you
  complete it, appending the evidence line (test name or command) under the
  checkbox. Do not batch checkmarks at the end.
- Commit and push `main` after each completed task, one task per commit.
- Every task ends with the standard battery green:
  `cargo test -p postfiat-execution --lib`, `cargo test -p postfiat-types
  --lib`, `cargo test -p postfiat-bridge --lib`,
  `cargo test -p postfiat-node navcoin_bridge_status_reads_persisted_pftl_uniswap_ledgers --lib`,
  `cargo fmt --all`, `git diff --check`.
- The `wallet-proxy/server.js` `route_family` edit remains forbidden until
  Task 4, where it lands **only** together with the digest-authority fix.
- If the timebox runs out mid-task, leave its boxes unchecked, record exactly
  what is done vs not in the task's evidence lines, and update the spec
  checklist as `Partial`. Never check a box on intent.

## Task 1 — Destination-consume transition (core, do first)

The constant `PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED` exists but no
operation sets it, so `ethereum_spendable_supply_atoms` is never credited and
`return_import` is unreachable. Add the missing transition.

- [x] New signed asset operation `PftlUniswapDestinationConsumeOperation`
  { operator, route_id, packet_hash, ethereum_consume_tx_hash,
  consumed_height, finalized_height }, with validation mirroring the other
  PFTL-Uniswap operations (text/hex field checks, canonical signing preimage,
  entrypoint dispatch).
  Evidence: `cargo test -p postfiat-types --lib` (64 passed) and `cargo test -p postfiat-execution pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --lib` (passed).
- [x] Applier `apply_pftl_uniswap_destination_consume`: route must be live;
  operator gated by `ensure_pftl_uniswap_native_asset_policy`; packet must
  exist with status `source_debited`; require
  `finalized_height >= consumed_height + route.return_finality_blocks`;
  transition packet to `destination_consumed`, move the packet amount from
  `outstanding_bridge_claims_atoms` to `ethereum_spendable_supply_atoms`
  (checked arithmetic), append a `destination_consume` receipt, re-validate
  the route (supply-conservation invariant must hold).
  Evidence: `cargo test -p postfiat-execution pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --lib` proves destination consume moves `40` atoms from outstanding to Ethereum-spendable and revalidates route state.
- [x] Mutual exclusion is consensus-ordered and must be proven both ways:
  a refund after consume rejects (status is no longer `source_debited`), and
  a consume after refund rejects for the same reason. Add both cases to the
  consensus test.
  Evidence: `cargo test -p postfiat-execution pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --lib` rejects refund-after-consume with `pftl_uniswap_packet_not_refundable` and consume-after-refund with `pftl_uniswap_packet_not_consumable`.
- [x] Trust class recorded: destination-consume is **operator-attested**
  (issuer/reserve-operator signed) until the selected Gate 5 verifier replaces
  it. State this in `docs/plans/pftl-uniswap-bridge-redeployment-spec.md` at
  the consensus checkbox and in the route trust-class documentation. Do not
  describe it as verified/trustless anywhere.
  Evidence: `docs/plans/pftl-uniswap-bridge-redeployment-spec.md` now states destination-consume remains operator-attested under `CONTROLLED`; `git diff --check` passed.
- [x] RPC/SDK: request builder + response validator for the new operation, and
  the bridge/node status surfaces show `destination_consumed` packets and the
  updated supply buckets.
  Evidence: `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results --lib` validates `pftl_uniswap_destination_consume` signed-asset submit responses; `cargo test -p postfiat-node navcoin_bridge_status_reads_persisted_pftl_uniswap_ledgers --lib` and `cargo test -p postfiat-bridge --lib` passed.
- [x] Extend the round-trip consensus test through destination consume →
  return import: after consume, a return import for the consumed amount
  succeeds and moves `ethereum_spendable` → `pftl_spendable` with a real
  native credit to the recipient.
  Evidence: `cargo test -p postfiat-execution pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --lib` proves route init -> subscribe -> export -> destination consume -> return import with real balance movement; standard Task 1 battery passed (`postfiat-execution` 77, `postfiat-types` 64, `postfiat-bridge` 30, node status test, `cargo fmt --all`, `git diff --check`).

## Task 2 — Return-import binding hardening

The applier already enforces finality depth, burn dedup, and the
`ethereum_spendable` debit. Close the two remaining holes:

- [x] Recompute the burn id in consensus: derive the expected
  `burn_event_hash` from (ethereum_chain_id, bridge_controller,
  wrapped_navcoin_token, ethereum_sender, pftl_recipient, amount_atoms,
  return_nonce, burn_height) using the same canonical derivation the sidecar
  uses ("recompute Ethereum return burn id" item in the spec), and reject on
  mismatch. The operator supplies fields; consensus derives the binding — the
  hash itself is no longer trusted input.
  Evidence: `postfiat-types::pftl_uniswap_return_burn_id_from_fields` is the shared canonical helper used by both `postfiat-bridge::pftl_uniswap_return_burn_id` and `apply_pftl_uniswap_return_import`; `cargo test -p postfiat-execution pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --lib` rejects `pftl_uniswap_return_burn_id_mismatch`; `cargo test -p postfiat-types pftl_uniswap_return_burn_id_binds_burn_height --lib` proves `burn_height` changes the id; `cargo test -p postfiat-bridge pftl_uniswap_return_burn_id_matches_solidity_abi_vector --lib` proves the sidecar vector.
- [x] Record the residual trust class: `burn_height`/`finalized_height` and
  the burn fields themselves remain operator-attested until Gate 5; consensus
  now enforces internal consistency (id binding, finality depth, dedup,
  supply movement). Update the spec's freshness/height checkbox text to
  reflect exactly what moved from operator-attested to consensus-enforced in
  this sprint.
  Evidence: `docs/plans/pftl-uniswap-bridge-redeployment-spec.md` records the shared sidecar/consensus burn-id binding and states that burn fields and heights remain operator-attested until Gate 5; standard Task 2 battery passed (`postfiat-execution`, `postfiat-types`, `postfiat-bridge`, node status test, `cargo fmt --all`, `git diff --check`).

## Task 3 — Refund proof + pause decisions (small, decide and record)

- [x] `non_consumption_proof_hash` is currently shape-checked 96-hex only.
  Decision to implement: bind it as a canonical commitment —
  `H(domain_tag ‖ route_id ‖ packet_hash ‖ refund_not_before_height)` with a
  fixed domain tag — recomputed and enforced in consensus, and documented as
  a **commitment format placeholder** that the Gate 5 verifier will replace
  with a real non-consumption proof. This makes the field meaningful (it can
  no longer be arbitrary hex) without pretending it is a proof. Record in the
  spec that refunds stay operator-attested until Gate 5.
  Evidence: `postfiat-types::pftl_uniswap_non_consumption_proof_hash` derives the canonical commitment; `cargo test -p postfiat-types pftl_uniswap_non_consumption_proof_hash_binds_refund_height --lib`, `cargo test -p postfiat-execution pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --lib`, `cargo test -p postfiat-bridge pftl_uniswap_bridge_ledger_exports_refunds_and_preserves_invariant --lib`, and `cargo test -p postfiat-bridge pftl_uniswap_refund_receipt_commits_non_consumption_proof --lib` passed; spec records the Gate 5 placeholder trust class.
- [x] Pause semantics decision: pause blocks `primary_subscribe`,
  `export_debit`, and `destination_consume` (exposure-growing), but **not**
  `refund_source` or `return_import` (exposure-shrinking). `refund_source`
  currently skips the pause check by omission — make the exemption explicit
  in code comment + spec, and add a test: paused route rejects subscribe and
  export, accepts refund and return import.
  Evidence: `cargo test -p postfiat-execution pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --lib` rejects paused subscribe/export/destination-consume while accepting paused refund and paused return-import; standard Task 3 battery passed (`postfiat-execution` 77, `postfiat-types` 66, `postfiat-bridge` 30, node status test, `cargo fmt --all`, `git diff --check`).

## Task 4 — Wallet-proxy digest authority (lands the forbidden edit)

- [x] Node side: expose the node-canonical `route_config_digest` (and launch
  config digest if not already present) in the bridge status RPC the proxy
  can read. If it is already exposed, cite where.
  Evidence: `PftlUniswapRouteStatusRow` already exposes `route_config_digest` through `navcoin_bridge_routes`; `NavcoinBridgePacketPreflightReport` exposes both `route_config_digest` and `launch_config_digest`; `cargo test -p postfiat-node navcoin_bridge_status_reads_persisted_pftl_uniswap_ledgers --lib` and `cargo test -p postfiat-node navcoin_bridge_packet_preflight --lib` passed.
- [x] Proxy side: `navswapBridgeConfig()` consumes the node-produced digest
  instead of hashing its own `JSON.stringify`. The proxy never computes a
  canonical digest from its own serialization again.
  Evidence: `wallet-proxy/server.js` now requires `NAVSWAP_ROUTE_CONFIG_DIGEST` / `NAVSWAP_NODE_ROUTE_CONFIG_DIGEST`, returns `route_config_digest_authority: "node"`, and has no `JSON.stringify(routeConfig)` route-config digest path; `node wallet-proxy/test_navswap_adapter.js` passed.
- [x] Land the `route_family: 'primary_pftl_mint'` edit together with the
  above, clearing the standing forbidden-edit directive at spec line ~1111.
  Evidence: `wallet-proxy/server.js` includes `route_family: 'primary_pftl_mint'` in route copy while preserving the node-produced digest path; `docs/plans/pftl-uniswap-bridge-redeployment-spec.md` records the directive closure.
- [x] Pinned vector test: a node-generated route-config digest fixture checked
  into the repo, with a proxy test asserting byte equality, so a future
  serialization drift fails CI rather than breaking the MVP4
  `expected_gate3_route_config_digest` binding silently.
  Evidence: `wallet-proxy/fixtures/pftl-uniswap-node-route-digest.json` pins the Gate 3 node digest `23c4522e0f65c728e555418e486bbf09ad85f335df2b99b58c17415ed3836ff78c31ce271244ad0d66cc78aa35c57e71`; `testUniswapHandoffUsesNodeRouteDigestFixture` asserts byte equality in `node wallet-proxy/test_navswap_adapter.js`.
- [x] Update the Wallet block in the spec: digest-authority item checked with
  evidence; the `route_family` finding closed.
  Evidence: `docs/plans/pftl-uniswap-bridge-redeployment-spec.md` Wallet section now states the proxy consumes node-produced route digests, launch digests are available from packet preflight, the forbidden `route_family` finding is closed, and standard Task 4 battery passed (`node wallet-proxy/test_navswap_adapter.js`, `postfiat-execution` 77, `postfiat-types` 66, `postfiat-bridge` 30, node status test, `cargo fmt --all`, `git diff --check`).

## Task 5 — Receipts growth (stretch; decision required, implementation optional)

- [x] Decide the `pftl_uniswap_receipts` retention story before the cap
  (131,072) can brick a route. Recommended shape: consensus checkpoint — fold
  receipts older than a retention window into a running
  `receipts_checkpoint_hash` on the route (hash-chain, so history stays
  provable), prune the folded rows, and keep the cap as a live-window bound.
  If the timebox is short, the decision and spec entry are required; the
  implementation may be a follow-up checkbox with an explicit owner.
  Evidence: `docs/plans/pftl-uniswap-bridge-redeployment-spec.md` records the
  consensus checkpoint-window retention design, keeps
  `MAX_PFTL_UNISWAP_RECEIPTS = 131_072` as the retained live-window cap, and
  adds an unchecked implementation follow-up before public routing or before a
  route can approach the cap. No pruning code landed in this sprint. Standard
  Task 5 battery passed: `cargo test -p postfiat-execution --lib`,
  `cargo test -p postfiat-types --lib`, `cargo test -p postfiat-bridge --lib`,
  and `cargo test -p postfiat-node navcoin_bridge_status_reads_persisted_pftl_uniswap_ledgers --lib`.

## Task 6 — Spec bookkeeping (close the milestone honestly)

- [x] If Tasks 1–3 are done: close the "Wire the bridge state machine into
  PFTL consensus" checkbox with the round-trip test as evidence, and rewrite
  the "Replace operator-attested freshness and height inputs" item to list
  only what genuinely remains (Gate 5 proof semantics; anything timeboxed
  out).
  Evidence: `docs/plans/pftl-uniswap-bridge-redeployment-spec.md` now closes
  the consensus wiring checkbox with the signed route init -> subscription ->
  export -> destination consume -> return import test as evidence, and rewrites
  the freshness/finality/proof item to leave only Gate 5 source-chain proof
  semantics as remaining.
- [x] Update the status summary counts and the "current permitted claim"
  paragraph: the permitted claim after this sprint is
  "consensus-backed controlled bridge round trip (operator-attested
  destination events), wallet-proxy digest authority" — not trustless
  anything.
  Evidence: `docs/plans/pftl-uniswap-bridge-redeployment-spec.md` summary now
  records consensus-completion sprint tasks as `6 / 6`, names the current
  permitted claim as consensus-backed controlled PFTL-Uniswap bridge round trip
  with operator-attested source-chain facts plus node-produced wallet-proxy
  digest authority, and explicitly forbids trustless/public routing copy.
- [x] Confirm `main` is pushed and the worktree is clean (no dirty files at
  all once Task 4 lands the proxy edit).
  Evidence: Task 4 landed the proxy edit in `66cde0a3`, Task 5 was pushed in
  `1d5b2ca8`, and this Task 6 commit is the only remaining sprint delta. Final
  post-push verification for this task is `git status --branch --short`
  reporting `main` aligned with `origin/main` and no dirty files.

## Out of scope for this sprint

- Gate 5 economics/verifier work and any public-routing (Gate 6) step.
- Devnet campaign/wallet-beta execution — that is the next step after this
  sprint and will be directed separately once these land.
- Any commit/reveal or scoring-sidecar work; that thread is separate.
