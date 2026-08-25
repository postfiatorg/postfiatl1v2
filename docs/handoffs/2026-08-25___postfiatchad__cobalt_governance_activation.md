# Cobalt governance activation

- **Operator:** Post Fiat Chad (`postfiatchad`)
- **Date:** 2026-08-25 UTC

## BLUF

Today we moved Cobalt from an unresolved research feature to a release-qualified
validator-governance implementation with a clear controlled-testnet activation
decision. Cobalt now matches the frozen oracle across all 18 decisive cases,
passes the isolated six-validator liveness simulation, requires a real signed
RBC -> ABBA -> MVBA -> DABC decision at live registry admission, and has a
qualified handoff and rollback packet against the exact 915-block controlled
testnet lineage. Consensus v2 remains the only block-finality protocol.

Deployment was deliberately stopped before live mutation. Foundation authority
is still active, Cobalt authority is inactive, and no validator service or live
registry changed. The active milestone remains
[the Cobalt Activation Milestone](../plans/active/cobalt-activate-or-retire-milestone.md)
until the controlled-testnet cutover, one real Cobalt-authorized registry
change, terminal CLI and browser verification, final packet, and documentation
are complete.

## What we completed today

### Decision model and RippleD comparison

- Froze an independent decision contract and 18-case manifest with per-validator
  terminal expectations. The oracle has no production Cobalt dependency.
- Fixed the non-uniform support boundary in production Cobalt. All 18 cases now
  match the oracle with zero per-node mismatches and zero conflicting Cobalt
  roots.
- Proved the precise RippleD difference in
  `six-divergent-local-quorums`: two locally unanimous 3-of-3 UNL groups admit
  two incompatible validator-registry roots in the governance adapter. Cobalt
  detects nine unsafe cross-group trust pairs and preserves the current root.
  The native RippleD CSF ledger control remains separately labeled and stayed
  on one ledger branch.
- Verified the three 20-validator 90%-overlap boundaries: compatible and exact
  support-boundary cases decide; the below-boundary case halts.

### Admission safety and bounded certificates

- Replaced signature-count admission with a mandatory validator-key-bound Cobalt
  protocol decision over the exact update, chain domain, current registry root,
  and trust graph.
- Added deterministic shared support proofs, quorum-minimal signer retention,
  bounded canonical compression, and a 1 MiB certificate limit.
- Kept the scaled 20-validator ML-DSA certificate at 938,032 bytes and the
  sidecar commit request below its 2 MiB frame.
- Verified quorum-only, tampered, replayed, wrong-root, cross-chain,
  mixed-authority, and new-set self-authorization rejection paths.

### Liveness and recovery

- Ran six isolated simulated validator domains with separate identities, keys,
  trust views, durable state, endpoints, schedules, and fault controls. This is
  protocol-capability simulation; it is not an independent-operator or
  decentralization claim.
- Demonstrated five-of-six progress, four-of-six safe halt, signed catch-up,
  crash/restart recovery, and byte-identical durable history.
- Covered admission, removal, ML-DSA key rotation, compatible trust-view
  transition, delay, loss, reordering, duplication, stale replay, equivocation,
  crash/restart, and partition healing.
- Paired 50 baseline Consensus v2 rounds with 50 Cobalt integration rounds.
  Client-visible p95 finality moved from 1617.88 ms to 1660.42 ms, a 2.63%
  increase inside the 5% budget. Cobalt covered 99.9985% of the integration
  window under the production 25% CPU quota.

### Release qualification

- Repaired deterministic replay of the migrated controlled-testnet archive and
  verified all 915 blocks to tip
  `2333396826284869daaf47d93de5f14641e6fe8b0ebbe74fc3f5b910b5df66d4d81ed35ff3f7d7b2b776dce39d596450`
  and state root
  `b8a0aef3f17b50c422e7cccf270c809a722587fd6af0cbff17fdcba7dd5c72edf2ba36b6ee00d20467c6a9e29d2bbe5a`.
- Rehearsed future-height activation, a scoped validator-key rotation, six
  negative cases, and separately authorized forward rollback on disposable
  copies of the current six validator signers.
- Passed all 15 release-packet checks. The qualified packet root is
  `f4f2f202111dc327ee590310ba65dc53e0611a578041ba878a3e23298e47a3e2`.
- Confirmed the six live validator processes, restart counts, binaries, registry
  roots, trust-graph roots, authority flags, and block-control flags were
  unchanged before and after qualification.

### Publication

- Published
  [Cobalt: Further Evaluation](https://postfiat.org/blog/cobalt-further-evaluation/)
  with the activation recommendation, deterministic liveness evidence, and the
  exact RippleD validator-governance comparison.
- The final Text Improvement Harness score is 89.87: GPT 92.20, Fable 88.00,
  and GLM 89.40. Site commit: `6cd20b9`.

## Current live boundary

- Foundation validator-trust authority: **active**.
- Cobalt validator-trust authority: **inactive**.
- Cobalt block-consensus control: **inactive by design and must remain so**.
- Consensus v2 block finality: **unchanged**.
- Live registry mutations performed today: **none**.
- Live service restarts performed for activation: **none**.
- Active plan count: **one**, the Cobalt Activation Milestone.
- Live cutover Task Node task: **not requested**.
- Milestone-document task `task_18b8d92d981221b88d0a38159ea1fd26`
  is still recorded as accepted; confirm its final rewarded state before
  closing the milestone.

## Next action: controlled-testnet activation

The next operator should execute this as one substantial Task Node-governed
terminal operation, not as a series of microtasks.

1. Request and inspect one personal Task Node task governing the future-height
   Cobalt cutover, first live Cobalt-authorized registry change, terminal CLI
   and browser verification, final activation packet, documentation, and
   rollback readiness. Accept it only if it preserves the boundaries in this
   handoff.
2. Reconfirm the six-validator live baseline immediately before scheduling:
   chain height and finalized tip, validator identities, binary hashes, registry
   root, trust-graph root, Foundation authority, Cobalt authority, Consensus v2
   health, process IDs, and restart counts.
3. Re-run the compact preflight against the qualified release lineage and
   disposable signer-state clones. Verify the packet root, six negative cases,
   exact archive replay, and forward rollback. Stop if any source, binary,
   signer, root, or live-state input differs without an explained and
   requalified change.
4. Choose and record a future activation height with enough operational margin.
   Submit the old-registry ML-DSA-authorized handoff through the existing
   consensus-ordered governance path. Do not restart services or mutate live
   files outside the runbook.
5. Observe the activation height and prove from live authenticated state that
   Cobalt governs validator-trust updates while Consensus v2 alone continues to
   order and finalize blocks.
6. Execute one bounded real registry change under active Cobalt authority,
   preferably the already rehearsed scoped validator-key rotation. Require the
   exact signed Cobalt decision certificate and accepted consensus receipt.
7. Run the early, stale, replayed, wrong-root, mixed-authority, and
   self-authorized negative cases. Each must reject without registry mutation.
8. Verify the Python CLI and read-only browser interface against the terminal
   live state. They must show the authority mode, registry root, trust-graph
   root, transition history, terminal decision, and verifier result.
9. Write `activation-status.json` with `ACTIVATED`; assemble the compact
   checksum-bound activation packet; run its verifier, focused Rust and Python
   tests, formatting, strict Clippy, workspace checks/tests, documentation
   build, link checks, and redaction checks. Run the broad workspace/Orchard
   suite once at this final gate, not during each Cobalt-only iteration.
10. Submit honest Task Node evidence through final verification. After every
    accepted task is rewarded, refresh README, STATUS, architecture, governance,
    CLI help, and operator documentation; move the active milestone to
    `docs/plans/completed/`.

If live activation cannot satisfy any gate, preserve the last valid authority
state. Before the activation height, leave Foundation authority in control.
After Cobalt activation, use only the separately authorized forward rollback
path rehearsed in the qualification packet; never rewrite finalized history.

## Verification already completed

- `cargo test -p postfiat-node --lib wan_devnet2_legacy_non_nav_spread_supply_window_is_chain_and_boundary_bound --locked -- --nocapture`
- `cargo test -p postfiat-node --lib wan_devnet2_ --locked -- --nocapture`
- `cargo test -p postfiat-node --lib archive_replay --locked -- --nocapture`
- `cargo test -p postfiat-execution --lib ar11_issued_asset_supply_counts_non_nav_spread_custody --locked -- --nocapture`
- `cargo fmt --all -- --check`
- Full locked workspace test run after the authority receipt-boundary change:
  292 node-library tests, 115 node-binary tests, remaining crates, long
  AssetOrchard cases, and doc tests; zero failures.
- Section 2 decisive packet: PASS.
- Section 3 isolated-validator and finality packet: PASS.
- Release qualification packet: 15/15 checks passed.

No live cutover tests or live Cobalt-authorized registry mutation have been run,
because work stopped before deployment.

## References

- [Active Cobalt Activation Milestone](../plans/active/cobalt-activate-or-retire-milestone.md)
- [Locked Activation Research Specification](../governance/cobalt-activate-or-retire-research-spec.md)
- [Deterministic Governance Overview](../governance/deterministic-governance-overview.md)
- [Release qualification summary](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/cobalt-handoff-rehearsal/release-qualification-v1.json)
- [Release-qualified packet](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/cobalt-handoff-rehearsal/packet-release-qualified-v1)
- [Decisive Section 2 packet](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/cobalt-activate-or-retire/section2-packet)
- [Isolated-validator Section 3 packet](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/cobalt-activate-or-retire/section3-packet)
- [Cobalt authority certificate](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/node/src/cobalt_authority_certificate.rs)
- [Cobalt handoff implementation](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/node/src/cobalt_handoff.rs)
- [Python Cobalt CLI](https://github.com/postfiatorg/postfiatl1v2/blob/main/python/postfiat_rpc/cobalt.py)
- [Read-only Cobalt browser interface](https://github.com/postfiatorg/postfiatl1v2/blob/main/python/postfiat_rpc/cobalt_ui.py)
