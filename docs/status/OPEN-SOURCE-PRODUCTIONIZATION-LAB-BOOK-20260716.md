# Open-Source Productionization Lab Book

**Branch:** `open-source-productionization-20260716`
**Audited baseline:** `4b5af7bc6bb6e793ed8a60219d13d6d35be03058`
**Controlling checklist:** `OPEN-SOURCE-PRODUCTIONIZATION-REVIEW-CHECKLIST-20260716.md`
**Live blocker register:** `OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md`

This is the append-only engineering record for STEP 1 reproductions and STEP 2
remediations. A targeted green entry is not a public-release waiver: the live
blocker remains `FIXED-LOCAL / NOT INTEGRATED` until the final candidate passes
the complete release battery.

## 2026-07-16 — baseline gates and dependency evidence

- `cargo fmt --all -- --check` — PASS.
- `cargo check --workspace --locked` — PASS.
- `cargo test --workspace --locked` — PASS, including node `165/165`; full run
  completed in approximately 40.7 minutes.
- `cd wallet-web && npm test` — PASS `206/206` on the audited baseline.
- `cd crates/ethereum-contracts && forge test -vv` — PASS `88/88`; this count
  includes the deliberate P0-SUPPLY-01 exploit reproduction.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — RED: one
  FastSwap `BTreeMap` entry-pattern lint and two Orchard manual-saturating-
  arithmetic lints. These are tracked release-gate defects; no lint was waived.
- Fresh `cargo audit --json` using advisory DB commit
  `9f3e138091487e69144f536d36976e427a7a3307` — RED:
  `RUSTSEC-2026-0204` (`crossbeam-epoch 0.9.18`), unmaintained `ansi_term
  0.12.1`, `bincode 1.3.3`, `proc-macro-error2 2.0.1`, and yanked `spin
  0.9.8`.
- `npm audit --json`: wallet proxy clean; wallet web RED with high-severity
  Vite and moderate esbuild advisories.

## 2026-07-16 — P0-SUPPLY-01 reproduction

- Added
  `MintControllerTest.testExploitBeneficiaryCanFabricateSettlementAndReleaseMint`.
- `forge test --match-contract MintControllerTest --match-test
  testExploitBeneficiaryCanFabricateSettlementAndReleaseMint -vv` — PASS on
  the vulnerable implementation: a beneficiary-authored boolean, amount and
  arbitrary proof hash release escrowed mint without settlement evidence.
- No fix is claimed by this entry.

## 2026-07-16 — P0-PROXY-AUTH-01 and P0-CUSTODY-01 fixed locally

### Reproduction

- Added `wallet-proxy/test_public_auth_boundary_regression.js` and ran it
  before remediation — PASS on vulnerable behavior: default `0.0.0.0`,
  tokenless/originless HTTP mutation dispatch, and tokenless/originless native
  custody-signer dispatch.

### Remediation

- Proxy default changed to `127.0.0.1`.
- Non-loopback configuration now fails startup unless both a >=32-byte
  `WALLET_PROXY_API_TOKEN` and nonempty exact `ALLOWED_ORIGINS` are configured.
- Chain-mutating HTTP and WebSocket dispatch requires a constant-time bearer;
  the local proxy credential is deleted before any upstream RPC forwarding.
- Browser origins are exact-allowlisted; an empty allowlist is no longer an
  allow-all policy.
- Public `wallet_sign_owned_transfer` and `wallet_sign_owned_unwrap` methods and
  their temporary-file/native-process implementation were removed.
- Web-wallet FastPay transfer and unwrap owner authorization now always signs
  locally through wallet WASM.
- The session bearer is entered visibly as a mutation credential but stored
  only in `sessionStorage`; it is not saved with wallet settings, committed, or
  logged. Both WebSocket and HTTP wallet clients attach it only to requests.

### Evidence

- `node wallet-proxy/test_public_auth_boundary_regression.js` — PASS with
  inverse assertions: loopback default, unsafe public startup rejected,
  unauthenticated mutation rejected `401 proxy_auth_required`, removed custody
  signer rejected `proxy_method_removed`.
- Offline wallet-proxy regression battery (FastPay certificate finality,
  outbox, channel isolation, quorum, route warmup; NAVSwap adapter, atomic,
  policy persistence; proposer routing; shielded prewarm; auth boundary) —
  PASS.
- `cd wallet-web && npm test` — PASS `208/208`, including local-only owner
  signing and token-attachment regressions.
- `cd wallet-web && npm run build` — PASS; immutable production bundle emitted.
- `cargo check --workspace --locked` — PASS.

**Disposition:** `FIXED-LOCAL / NOT INTEGRATED`. Final closure still requires
the full candidate battery, hostile-origin/CSRF/WebSocket tests, clean dependency
audit, and clean public build/deployment evidence.

## 2026-07-16 — P0-CONSENSUS-01 fixed locally by feature containment

### Reproduction

- Added
  `exploit_cross_view_vote_lock_accepts_conflicting_proposals_with_unresolved_high_qc`.
  On the vulnerable implementation it formed a valid three-of-four timeout
  certificate naming `fabricated-unresolved-qc`, then proved validator 0 could
  sign conflicting proposals at height 1 in views 0 and 2 because the WAL lock
  path included the view.
- `cargo test -p postfiat-node
  exploit_cross_view_vote_lock_accepts_conflicting_proposals_with_unresolved_high_qc
  -- --nocapture` — PASS on the vulnerable implementation (`1 passed`, 165
  filtered), proving the defect before remediation.

### Remediation

- Production proposal validation now rejects every nonzero view. Timeout votes
  and certificates remain independently verifiable diagnostic artifacts but
  cannot authorize a proposal or unlock a height.
- Vote-lock schema v2 is keyed by `(height, validator)`, is atomically reserved
  before signing, and scans/obeys legacy v1 view-scoped locks at the target
  height before creating the new lock.
- The whitepaper, README, and finality architecture now state the implemented
  direct view-zero certificate rule and its explicit halt-on-failed-round
  liveness limitation; no HotStuff view-change or two-chain claim remains on
  this path.

### Evidence

- `cargo fmt --all -- --check` — PASS.
- `cargo test -p postfiat-node
  cross_view_vote_and_legacy_lock_migration_fail_closed -- --nocapture` — PASS.
- `cargo test -p postfiat-node
  proposal_certificate_accepts_three_of_four_bft_quorum -- --nocapture` — PASS.
- `cargo test -p postfiat-node
  timeout_votes_reconstruct_hotstuff_timeout_certificate -- --nocapture` — PASS.
- Full `cargo test -p postfiat-node` started after the targeted gates; final
  result is recorded below when complete.

**Disposition:** `FIXED-LOCAL / FEATURE-CONTAINED / NOT INTEGRATED`. The unsafe
view-change surface is unreachable. This does not claim automated failed-round
recovery; the integrated crash/replay/adversarial candidate gates remain open.

## 2026-07-16 — P0-SUPPLY-01 fixed locally at verifier boundary

### Reproduction

- `MintControllerTest.testExploitBeneficiaryCanFabricateSettlementAndReleaseMint`
  passed before remediation: beneficiary-authored booleans, an arbitrary
  `uint128::max` value, and a new hash released 100 escrowed atoms without
  settlement.

### Remediation and evidence

- `MintController` requires a one-time settlement verifier and binds its lookup
  to pending ID, escrow ID, beneficiary, mint amount, and proof hash. Caller
  claims must exactly match verifier output and remain one-use.
- `forge test --match-contract MintControllerTest -vv` — PASS `10/10`, including
  fabricated, value-inflation, cross-escrow, replay and verifier-replacement
  negatives plus real authorized releases.
- `forge test --match-contract MarketOpsAdversarialTest -vv` — PASS `10/10`.
- `forge test -vv` — PASS `91/91` (the official-fork test remains separately
  classified as a no-op release-gate defect when no RPC URL is configured).

**Disposition:** `FIXED-LOCAL / NOT DEPLOYABLE YET`. The controller no longer
trusts beneficiary data, but a concrete settlement-verifier implementation and
pinned deployment/code-hash policy are still mandatory before this feature may
be deployed or described as production backing verification.

## 2026-07-16 — P0-RPC-01 unsigned wrap removed locally

### Remediation

- `wrap_owned` now rejects all assets with `PermissionDenied` before store
  access; unsigned unwrap was already disabled.
- Removed both methods from remote dispatch and the unconditional allowlist.
- Removed proxy broadcast/object-ID synthesis and browser RPC methods. Wallet
  funding UI now fails closed and names the signed consensus FastLane deposit
  requirement rather than invoking a hidden direct ledger mutation.

### Evidence

- `cargo test -p postfiat-node
  unsigned_wrap_owned_rejects_every_asset_without_mutation -- --nocapture` —
  PASS; PFT/pfUSDC/a651 attempts leave balances and owned objects unchanged.
- `cargo test -p postfiat-node
  unsigned_owned_lane_mutations_are_never_remote_methods -- --nocapture` — PASS
  in the binary's real remote allowlist, including with all mutation feature
  flags enabled.
- `node wallet-proxy/test_proposer_routing.js` — PASS; `wrap_owned` is not a
  broadcast method and gains no proxy object ID.
- Proxy auth and FastPay quorum regressions — PASS.
- Wallet tests/build — PASS `208/208` before the explicit removed-method test
  was added; rerun recorded below.

**Disposition:** `FIXED-LOCAL / REMOVED / NOT INTEGRATED`. Signed
`FastLanePrimaryOperationV1::Deposit` already exists in consensus admission and
execution; browser construction for it is an open usability integration, never
an authorization to restore the unsigned fallback.

## 2026-07-16 — P0-PRIVACY-01 legacy cleartext actions contained locally

### Reproduction

- Existing local-chain coverage admitted `shield_mint`, executed it as an
  accepted receipt, and persisted a `ShieldedNote` containing cleartext owner,
  asset, value, and memo. This confirmed the legacy type was a live action, not
  merely a historical decoder.

### Remediation

- Direct legacy spend and mint/spend batch constructors fail
  `PermissionDenied`.
- Live proposal construction rejects manually assembled legacy mint/spend
  actions before certification.
- Live execution has a second fail-closed guard returning
  `legacy_cleartext_shielded_action_disabled` with no state mutation.
- Archive replay alone may execute the legacy decoder so old roots remain
  reproducible. Asset-Orchard is the supported privacy path. Explicit legacy
  migration remains only for retiring historical notes.

### Evidence

- `cargo test -p postfiat-node
  legacy_cleartext_shielded_actions_are_historical_replay_only -- --nocapture`
  — PASS: creator rejection, admission rejection, live no-mutation receipt, and
  accepted archive replay are all proven with one identical batch.
- Current node suite excluding the three known long Orchard cases started to
  locate unrelated tests that must be migrated away from legacy fixture setup;
  results are recorded below.

**Disposition:** `FIXED-LOCAL / HISTORICAL-REPLAY-ONLY / NOT INTEGRATED`.

## 2026-07-16 — P0-WALLET-02 public development server removed from release path

### Reproduction

- Added `wallet-web/src/lib/vite-security.test.js` and ran it before the fix.
  Both assertions failed: the Vite development server was configured for
  `0.0.0.0`, and the production CSP granted all `ws:` and `wss:` origins.
- `cd wallet-web && npm audit --json` reported a direct high-severity Vite
  advisory path and the transitive esbuild cross-origin development-server
  advisory.

### Remediation

- Development and preview now bind only `127.0.0.1` by default and use
  `strictPort`; production CSP is deny-by-default and names only required
  loopback development connections.
- Upgraded Vite to `8.1.4` and `@vitejs/plugin-react` to `6.0.3` on the
  repository's Node 20.20.2 runtime.
- The hardened same-origin wallet proxy now serves only the built static tree,
  with traversal rejection, CSP, `DENY` framing, `nosniff`, no-referrer,
  permissions policy, no-store HTML, and immutable hashed-asset caching.

### Evidence

- `node --test src/lib/vite-security.test.js` — PASS `2/2` after the fix.
- `cd wallet-web && npm test` — PASS `211/211`.
- `cd wallet-web && npm run build` — PASS under Vite 8.1.4; static HTML, CSS,
  JS and WASM emitted.
- `cd wallet-web && npm audit --audit-level=moderate` — PASS, zero
  vulnerabilities.
- `node wallet-proxy/test_wallet_static_security.js` — PASS, including headers,
  cache policy, traversal and API-path isolation.
- Proxy auth, proposer-routing and FastPay quorum regressions — PASS.

**Disposition:** `FIXED-LOCAL / STATIC-SERVED / NOT INTEGRATED`. The candidate
still needs a clean-checkout artifact scan proving no Vite dev middleware or
public dev invocation is packaged, plus documentation cleanup and the complete
release battery.

## 2026-07-16 — P0-GOVERNANCE-01 unsigned live authorization contained

### Reproduction

- Added
  `unsigned_governance_support_cannot_enter_live_block_proposal`. Without any
  validator private key it built an amendment naming all four validators as
  supporters, created a governance batch, and the vulnerable production
  proposal builder accepted it. The inverse assertion failed and printed the
  complete admitted `BlockProposalFile`.

### Remediation

- Live proposal admission rejects amendments, validator-registry updates, and
  FastSwap bootstrap amendments because the legacy evidence has no signatures
  verifiable against the active registry.
- Direct amendment mutation and direct governance-batch apply reject with
  `PermissionDenied`. Authenticated archive replay remains available for old
  blocks.
- Governance state-machine tests that require synthetic unsigned fixtures now
  call a `cfg(test)`-only internal fixture path; that symbol is not compiled
  into release binaries and cannot be reached over RPC or proposal admission.
- README and whitepaper now identify signed Cobalt governance as a target and
  state that the active registry is fixed at genesis in the current candidate.

### Evidence

- Pre-fix targeted regression — FAIL as intended because the unsigned batch was
  admitted as a live proposal.
- `cargo test -p postfiat-node
  unsigned_governance_support_cannot_enter_live_block_proposal -- --nocapture`
  — PASS after remediation, including direct-apply rejection and zero blocks.
- `cargo test -p postfiat-node governance_ -- --nocapture` — PASS `37/37`.
- `external_proposal_certificates_apply_non_transparent_batches` — PASS after
  proving governance is rejected while shielded and bridge external-certificate
  coverage remains active.

**Disposition:** `FIXED-LOCAL / LIVE-PATH REMOVED / NOT INTEGRATED`. Re-enabling
governance requires the full signed-vote protocol and adversarial test matrix;
unsigned name support is not a release fallback.

## 2026-07-16 — P0-BRIDGE-01 asserted external bridge contained

### Reproduction

- Existing consensus execution coverage constructed a locally recomputable
  “non-consumption proof,” refunded the source after a height delay, and later
  accepted a return import from caller-authored Ethereum burn fields. No header,
  receipt, event inclusion, or finality proof was involved.
- Contract review confirmed the single-owner
  `ControlledPFTLReceiptVerifier` could be instantiated under the
  `TRUSTLESS_FINALITY` label.

### Remediation

- Strict/live execution rejects all six `PftlUniswap*` operations before state
  mutation. The old state machine is available only to explicit archive replay
  and a test-only fixture, preserving deterministic historical validation
  without retaining a live compatibility fallback.
- The initialized consensus regression now calls the strict boundary for route
  creation, source refund, and return import; all reject with
  `pftl_uniswap_external_verification_unavailable` and exact pre/post ledger
  equality.
- The controlled EVM verifier accepts only `CONTROLLED` or `DISABLED`; it cannot
  self-label as optimistic or trustless-finality verification.

### Evidence

- `cargo test -p postfiat-execution
  pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances` — PASS.
- `cargo test -p postfiat-execution` — PASS `134/134`.
- `forge test --match-contract PFTLUniswapHandoffControllerTest -vv` — PASS
  `34/34`, including controlled-verifier label rejection.
- `forge test` — PASS `91/91`.

**Disposition:** `FIXED-LOCAL / HISTORICAL-REPLAY-ONLY / NOT INTEGRATED`. A
future live route requires canonical external event/finality verification and
consume/refund mutual exclusion; the asserted-height path cannot be re-enabled.

## 2026-07-16 — P0-BRIDGE-01 governed Ethereum proof implementation, phase 1

### Implementation

- Added a bounded canonical Ethereum RLP and Merkle-Patricia receipt verifier,
  successful receipt-code enforcement, exact log-index extraction, and strict
  ABI decoders for `PacketConsumed`, `PacketCancelled`, and `ReturnBurned`.
- Added versioned governed Ethereum checkpoints and sorted, distinct,
  exact-quorum ML-DSA-65 certificate verification. Checkpoints bind the PFTL
  chain/genesis/protocol, route/config, Ethereum chain and block/hash/receipts
  root, confirmation depth, committee epoch/root, controller/token addresses,
  and runtime code hashes.
- Live routes now require an exact on-ledger checkpoint authority. Source export
  signs and persists the exact EVM packet digest and external packet schema v1.
  Live consume, cancellation,
  and return paths require the threshold checkpoint plus receipt proof; legacy
  records without those fields retain their historical encoding and remain
  archive-replay-only.
- The Ethereum replay registry now gives consume and post-deadline cancellation
  one shared, durable source-packet/source-receipt exclusion boundary.
- Added `ThresholdPFTLReceiptVerifier` for the reverse direction. The immutable
  `BFT_CHECKPOINT` committee uses sorted distinct bridge ECDSA keys and the exact
  `n-f` BFT threshold; the signed digest binds Ethereum chain/verifier, PFTL
  chain/genesis/protocol, authority epoch/root, source receipt root/hash, route,
  packet digest, finalized height, and the exact `accepted` receipt code. The
  handoff controller consumes the resulting durable acceptance record before
  minting. Rotation requires a new verifier/route drain; no owner can silently
  replace signers or lower threshold.

### Evidence

- `CARGO_TARGET_DIR=/tmp/postfiat-p0-check cargo test -p postfiat-types -p
  postfiat-bridge -p postfiat-execution --lib --locked` — PASS: types 83/83,
  bridge 35/35, execution 139/139.
- The execution test drives the real signed asset-transaction boundary: valid
  3-of-4 checkpoint plus real receipt-trie consume accepted; independently
  certified wrong mint amount rejected with byte-identical ledger; certified
  cancellation accepted; later consume rejected with byte-identical ledger. A
  separately valid certificate carrying the wrong controller runtime code hash
  also rejects at route binding with byte-identical ledger.
- `cargo clippy -p postfiat-types -p postfiat-bridge -p postfiat-execution
  --all-targets --locked -- -D warnings` — PASS.
- Solidity controller focused cancellation race and full local suite previously
  passed; `forge test --no-match-contract
  PFTLUniswapOfficialForkTest` now passes 95/95, including threshold constructor
  loosening/ordering rejects, under-quorum/duplicate/wrong-code/tamper rejects,
  idempotency, and a real 3-of-4 certificate driving the controller mint. The strict official-mainnet fork remains
  fail-closed without `ETHEREUM_MAINNET_RPC_URL`.

**Disposition:** IN PROGRESS, NOT CLOSED. Both governed proof directions and
cryptographic cancellation are implemented. Restart/reorg/partition tests,
global supply integration, and complete bidirectional E2E remain mandatory.

## 2026-07-16T04:55:03Z — publication defaults and operations boundary hardened

### Live-infrastructure default reproduction and fix

- The supposedly local wallet proxy still defaulted its upstream to the real
  public validator `<redacted-public-ip>` and embedded all six real fleet IPs as its
  default proposer-routing topology. `docker-compose.wallet.yml` also selected
  a real validator, enabled controlled funding/beta routes and mounted issuer
  and holder custody keys. Multiple browser E2E scripts globally disabled TLS
  certificate verification while silently defaulting to the live wallet host.
- The proxy and Python WAN preflight now default to six loopback endpoints.
  Runtime scripts use loopback-only application/RPC defaults and no longer set
  `NODE_TLS_REJECT_UNAUTHORIZED=0`. The extension defaults to a local proxy.
  The public Compose file contains no fleet IP, operator account, custody-key
  mount, or enabled money route; it requires explicit RPC fleet and a random
  32-byte mutation token, binds the published port to host loopback, and keeps
  all devnet funding/beta/shielded routes disabled.
- `scripts/public-runtime-default-scan` examines tracked runtime/product
  surfaces and fails on any global IPv4 literal, global Node TLS disable, or
  custody-key mount in the public Compose file. Its isolated regression proves
  safe loopback acceptance and real-IP/TLS-disable rejection.

Evidence:

- `scripts/test-public-runtime-default-scan` — PASS.
- `scripts/public-runtime-default-scan` — PASS.
- `node wallet-proxy/test_public_auth_boundary_regression.js` — PASS, including
  exact loopback upstream/fleet and safe Compose assertions.
- `cd wallet-proxy && npm test` — PASS `20/20`.
- `cd wallet-web && npm test` — PASS `212/212`.
- `PYTHONPATH=python python3 -m unittest python.tests.test_wan_preflight
  python.tests.test_safe_rollout -v` — PASS `17/17`.
- `docker compose -f docker-compose.wallet.yml config --quiet` with explicit
  test-only loopback variables — PASS.

### Complete committee roster pre-deploy boundary

- Reproduction: safe rollout verified signed artifacts and fleet convergence
  but did not inspect mutable per-node `validator_keys.json`; this would not
  catch the prior one-member roster that could sign locally but could not
  verify a five-signature certificate.
- `verify_remote_committee_rosters` now executes each running release binary
  over batch-mode SSH, resolves the binary from the active systemd PID, runs
  `validate-local-keys --validators 6`, parses its JSON and requires exact
  schema, node ID, six keys, six required validators, validity and permissions.
  It runs during read-only preflight and again across all six nodes immediately
  before every `apply-next` mutation. Both evidence sets are retained in the
  durable rollout state.
- `PYTHONPATH=python python3 -m unittest python.tests.test_safe_rollout -v` —
  PASS `13/13`; the real-boundary regression changes validator-0's observed
  count to one and proves preflight fails closed, then proves six exact reports
  pass.

### Service hardening and operator-tool cleanup regression

- Generated validator/RPC units and all three examples now add kernel and
  control-group protection, SUID/SGID restriction, locked personality,
  realtime restriction, native syscall architecture, empty capability sets,
  core-dump disable and task/FD bounds. Validator/RPC bind and write-path
  restrictions are unchanged. The canonical staging test now asserts every
  directive in both generated units.
- The public-tree cleanup had removed the validator doctor, RPC doctor, monitor
  snapshot, account-history query and RPC surface inventory while the active
  day-two runbook still promised them. Only that supported bounded tool set and
  its local harness/smokes were restored; obsolete research/deploy generators
  remain removed.
- The restored RPC inventory initially failed at the real boundary with
  `ValueError: substring not found` because dispatch moved out of `lib.rs`.
  It now reads every modular SDK source plus `rpc_cli.rs` policy and
  `rpc_dispatch.rs` dispatch. Generation passes with 132 union methods: 75
  read-only public, two controlled-write gated, four privacy-alpha gated and 48
  operator/local.
- `python3 -m py_compile` over the five Python operator tools — PASS.
- Markdown relative-link scan over root/docs (excluding fenced code) — PASS,
  zero missing links after correcting four stale removed/external references.

**Disposition:** runtime/default containment and committee-roster deployment
check are `FIXED-LOCAL`. `P1-OPS-01` remains production-open for alerting, log
rotation, production key custody, independent operators and multi-region fault
drills. The restored smoke suites and generated-unit Rust regression run after
the already-running complete workspace suite releases the Cargo lock.

## 2026-07-16T05:05:00Z — operator smokes repaired; P0 FastLane state-root omission reproduced and fixed

### Restored operator boundary

- All three restored smokes initially failed because ordered commit no longer
  rebuilds the account transaction index implicitly. The fixtures asserted a
  ready index immediately after `apply-batch`, contradicting the current
  explicit maintenance contract.
- Each fixture now invokes `account-tx-index-build` before checking status; the
  production node behavior was not weakened or changed.
- `scripts/testnet-validator-doctor-smoke` — PASS, four validators.
- `scripts/testnet-rpc-doctor-smoke` — PASS, four validators and all read-only
  RPC checks.
- `scripts/testnet-monitor-snapshot-smoke` — PASS, four validators.
- `cargo test -p postfiat-node
  deployment_validator_unit_stage_is_canonical_and_non_overwriting --locked
  -- --nocapture` — PASS.

### P0-STATE-01 reproduction

- Source inventory showed `LedgerState` has ten FastLane/FastSwap fields after
  `owned_objects`, while `append_ledger_state` ended immediately after the
  owned-object loop.
- Added the real `replicated_state_root` boundary regression before the fix.
  `cargo test -p postfiat-node
  replicated_state_root_commits_every_fastlane_ledger_field --locked --
  --nocapture` failed as intended: adding a reserve left both roots equal to
  `0f0e8e7e3ac76f5e08f805cc56b8a46c10f02be5702b0a306db9bc09c969304f9ae13769aec672a85a77ee153fa90ee2`.

### P0-STATE-01 remediation and evidence

- The root now commits a tagged presence marker, sorted deterministic record
  commitments over explicit length-delimited canonical binary encodings for
  all nine FastLane/FastSwap collections, and exact optional activation-height
  presence/value. Encoding errors propagate.
- An exhaustive `LedgerState` destructure is a compile-time tripwire for future
  uncommitted fields.
- The regression now checks all ten fields independently, storage-order
  invariance, and amount sensitivity: PASS.
- `cargo test -p postfiat-node
  replicated_state_root_commits_to_chain_domain --locked` — PASS. The legacy
  genesis vector remains unchanged; the new-genesis vector was deliberately
  regenerated because the height-zero v2 marker changes the genesis domain.
- Exhaustive shared quorum math tests for committee sizes 1–64 and exact
  FastSwap committee thresholds for supported sizes 4–64 — PASS. FastSwap
  normal/new-round/control/checkpoint/exit certificate under-quorum and
  duplicate-validator regressions — PASS.
- `cargo fmt --all -- --check` — PASS after the integrated changes.

**Disposition:** `P0-STATE-01` is `FIXED-LOCAL / INTEGRATION PENDING`.
Historical replay and coordinated upgrade/rollback gates must pass before the
candidate is frozen.

### P1-CERT-DOMAIN-01 live registry-root boundary

- The domain audit found that live external, preverified and timeout
  certificates used `certificate_registry_root_or_legacy`, allowing an empty
  root to select the legacy vote preimage. Vote keys and quorum still came from
  the fixed current registry, but the claimed registry snapshot was not bound.
- Live paths now require the exact nonempty current root. Only already-committed
  block-history verification retains legacy behavior.
- `cargo test -p postfiat-node split_block_votes_reconstruct_certificate
  --locked -- --nocapture` — PASS; the real `apply_batch` boundary rejects a
  stripped-root certificate before mutation and then commits the valid rooted
  certificate.
- `cargo test -p postfiat-node
  timeout_votes_reconstruct_hotstuff_timeout_certificate --locked` — PASS.

**Disposition:** `P1-CERT-DOMAIN-01` is `FIXED-LOCAL`.

## 2026-07-16T05:32:00Z — FastLane state-root replay boundary completed

- A proposed chain-ID-only legacy FastLane-root replay exception was rejected
  during implementation review because it would have accepted omitted
  FastLane/FastSwap state on future devnet blocks. No such fallback remains.
- The commitment is encoding-compatible for a legacy genesis before the
  activation height when all ten FastLane fields are empty. No marker or
  records are appended, so historical roots remain byte-identical. Nonempty
  FastLane state is committed at and after the explicit v2 activation.
- `cargo test -p postfiat-node
  replicated_state_root_commits_every_fastlane_ledger_field --locked --
  --nocapture` — PASS.
- `cargo test -p postfiat-node
  fastswap_epoch_one_bootstrap_is_governance_bound_and_canonically_committed
  --locked -- --nocapture` — PASS after extending the test through both
  `verify_state` and `verify_blocks`; persisted nonempty committee, asset-rule,
  activation, receipt and block state replays successfully.
- `cargo test -p postfiat-node
  verify_blocks_replays_historical_registry_after_live_key_rotation --locked --
  --nocapture` — PASS.
- `cargo test -p postfiat-node
  historical_external_certificate_applies_via_catch_up_replay_path --locked --
  --nocapture` — PASS.
- `cargo test -p postfiat-node
  atomic_swap_archive_replay_rejects_preactivation_without_mutation --locked --
  --nocapture` — PASS.

**Disposition:** the P0 state-root defect and active-state replay boundary are
fixed locally. Full-candidate replay plus coordinated upgrade/rollback remain
explicit release gates; they were not replaced by a permissive compatibility
path.

## 2026-07-16T06:02:00Z — P0 state-root transition made versioned

- New genesis documents now include
  `replicated_state_v2_activation_height: 0`; old genesis JSON parses the
  absent field as `None` and serializes it absent, preserving the legacy
  genesis domain.
- Existing chains can schedule the root transition with the committed
  `replicated_state_v2_activation_height` amendment. The earliest committed
  value is irreversible, and admission rejects a value at or below the
  amendment block height.
- The root derives height from the complete ordered-batch prefix, including
  checkpoint history. Before activation it preserves the legacy omission; at
  activation it commits all nonempty FastLane state. No chain-ID allowlist or
  generic legacy-root acceptance was introduced.
- The root regression now additionally proves new-genesis activation,
  pre-activation equality, exact-boundary inequality, governed legacy-genesis
  migration, irreversibility, same-block rejection, and the frozen v2 reserve
  state-root vector
  `378deb19ad0fd04677e52979b9c7c178eb8f825820dc0790995a3d7ad45a0bb94c6d2995aad2c558e20b967784695821`.
- `cargo test -p postfiat-types --locked` — PASS `82/82`.
- `cargo test -p postfiat-node
  replicated_state_root_commits_every_fastlane_ledger_field --locked --
  --nocapture` — PASS.
- `cargo check --workspace --all-targets --locked` — PASS.
- Added `docs/runbooks/replicated-state-v2-activation.md`; it requires a real
  pre/post-activation full-committee drill and prohibits post-activation
  rollback to a v1 binary.

**Disposition:** code-level migration semantics are `FIXED-LOCAL`; the exact
candidate still requires the coordinated transition/recovery drill.

## 2026-07-16T05:50:20Z — P1 operations monitoring and retention increment

- A real monitor defect was found: `testnet-monitor-snapshot` attempted to read
  block and receipt counters from `metrics.storage`, even though the RPC schema
  publishes them under `metrics.ordering` and `metrics.execution`. Production
  reports therefore rendered those fields as `null`.
- The monitor now reads each metric from its actual schema section, uses the
  metrics mempool counter, and summarizes the bounded `receipts` response as
  accepted, rejected, or unknown. Unknown receipt semantics are critical;
  recent rejected receipts warn by default.
- Ordered warning/critical thresholds now cover height lag, RPC p95 and mempool
  depth. Threshold arguments reject negative or inverted values.
- `python3 -m unittest python.tests.test_monitor_snapshot -v` — PASS `2/2`,
  including the real response-file adapter and each warning/critical class.
- `RUN_ID=productionization-20260716
  scripts/testnet-monitor-snapshot-smoke` — PASS against a four-node local RPC
  harness; storage counters are non-null and exact, the accepted receipt sample
  is classified, mempool is zero, and monitor status is `ok`.
- Added `systemd/postfiat-logrotate.example`: 14 daily compressed rotations,
  100-MiB early rotation, owner/mode enforcement, and flat plus per-validator
  log coverage. `scripts/test-postfiat-logrotate` — PASS through the real
  `logrotate --debug` parser and mandatory-policy assertions.

**Disposition:** this closes the concrete null-metrics and absent-retention
subfindings in `P1-OPS-01`. Alert delivery, certificate/proof/disk/clock
telemetry, independent credentials and multi-region drills remain open and are
not represented as complete.

## 2026-07-16T05:58:00Z — ordered-commit crash matrix completed

- The production commit path uses `OrderedCommitDeltaJournal`, but its prior
  exhaustive restart regression covered only ledger plus the append logs. It
  did not exercise crashes after the optional governance, shielded, bridge, or
  validator-registry writes.
- Extended the real atomic-swap journal test to persist all optional consensus
  domains and restart from every one of 11 write prefixes: journal only;
  ledger; governance; shielded; bridge; receipts; ordered batch; archive;
  block without tip; chain tip without registry; and registry before journal
  removal.
- Every prefix recovers to byte-equivalent ledger, governance, shielded, bridge,
  receipts, ordered batches, archive, blocks, chain tip and registry; the
  journal is removed, both swap legs remain atomic, history is not duplicated,
  and `verify_state` passes.
- `cargo test -p postfiat-node --lib --locked
  atomic_swap_delta_journal_recovery_never_exposes_a_half_swap -- --nocapture`
  — PASS `1/1` across all 11 prefixes.

**Disposition:** the known ordered-commit journal-boundary matrix is closed.
Disk-full/short-write/fsync fault injection and the production indexed storage
engine remain open under `P1-STORAGE-01`.

## 2026-07-16T06:18:00Z — retired bridge-vault money destination removed

### Reproduction

- Review of the browser money path found that `wallet-web/src/lib/utils.js`
  exported the retired Arbitrum vault
  `0x1A15e6103D6Af4e88924F748e13B829D3948DEa9` when no build configuration was
  supplied. The repository's own June 29 handoff labels it the drained old
  vault.
- Added `public wallet has no implicit bridge vault money destination` at the
  exported configuration boundary before changing production code. It failed:
  actual was the retired address, expected was the empty fail-closed value.
- The More/settings component also allowed the user-level wallet record to
  carry an arbitrary `bridgeVaultAddr`; that is an invalid authority boundary
  for a money destination.
- A repository-wide runtime-default scan added immediately after the browser
  fix caught the same retired vault as the proxy relay fallback and the
  transaction allowlist in `scripts/stakehub-wallet-bridge-ux-live.mjs`. The
  read-only custody inventory reference is explicitly named `OLD_` evidence;
  it is not used as a mutation destination.

### Remediation

- Removed the source fallback. An absent explicit build binding now exports an
  empty destination and the existing Bridge component disables deposits.
- Added canonical address-shape validation and an explicit deny entry for the
  retired vault, including when it is supplied through build configuration.
- Removed the bridge destination from the wallet settings UI/state and added a
  source-boundary regression so it cannot silently return as a user setting.
- Removed the proxy relay fallback. Missing configuration returns
  `vault_bridge_vault_not_configured` before key, bundle, RPC, or mutation work;
  an explicit retired vault fails process initialization.
- Changed the live UX script to require an explicit reviewed
  `BRIDGE_UX_VAULT_ADDRESS`, with shape and retired-address checks before any
  browser or agent operation.
- A second boundary review showed that address-only configuration still
  permitted an operator typo or redeployed code to become the money target.
  Added tests first: the wallet rejected neither a non-retired address lacking
  a code hash nor a mismatched live bytecode value. The new tests failed on the
  missing exports/checks.
- Browser builds now require `VITE_BRIDGE_VAULT_CODE_HASH` whenever a vault is
  configured and verify `eth_getCode` with Keccak-256 before both approval and
  deposit. The proxy and live UX harness require their corresponding code-hash
  binding and independently verify `cast codehash` before bundle/transaction
  work.
- Extended the tracked public-runtime scanner with a retired-money-destination
  rule and an adversarial fixture.
- Updated the wallet runbook to define the reviewed build-time authority and
  fail-closed default.

### Evidence

- `cd wallet-web && node --test src/lib/utils.test.js` — PASS `4/4`.
- `cd wallet-web && npm test` — PASS `218/218`, including the frozen
  `keccak256(0x6000)` vector and both approve/deposit source boundaries.
- `cd wallet-web && npm run build` — PASS; production static assets emitted.
- `cd wallet-web && npm audit --audit-level=high` — PASS; zero
  vulnerabilities.
- `node wallet-proxy/test_bridge_destination_config.js` — PASS; empty relay
  config is disabled and explicit retired configuration fails closed.
- `cd wallet-proxy && npm test` — PASS `22/22` after adding an exhaustive
  inventory of all 14 HTTP POST handlers. Seven explicit preparation/read
  routes remain public; all money mutations and every future unclassified POST
  fail closed behind authentication.
- Missing-config launch of `scripts/stakehub-wallet-bridge-ux-live.mjs` —
  expected fail-closed before browser/signing work.
- `scripts/test-public-runtime-default-scan` and
  `scripts/public-runtime-default-scan` — PASS.

**Disposition:** `P0-WALLET-BRIDGE-DEST-01` is `FIXED-LOCAL / DISABLED WITHOUT
BINDING`. Enabling bridge deposits still requires a reviewed live contract and
release binding; the fix does not assert that such a deployment exists.

## 2026-07-16T06:44:00Z — certificate participation and clock telemetry added

### Reproduction

- The operations inventory required certificate-participation and clock-skew
  alerts, but `NodeMetrics` exposed neither certificate votes nor a node clock
  sample. The monitor consequently could not express either policy.
- Assertions were added first at the real `metrics` construction boundary for
  two persisted certified blocks and at the monitor response-file boundary.
  Rust failed to compile on all seven absent metric fields; Python failed with
  a missing `observed_unix_ms` key and no low-participation warning.

### Remediation and evidence

- Node metrics now expose bounded recent (last 128 blocks) and lifetime
  certificate/vote counts, local recent vote participation as integer ppm, and
  an observational Unix-millisecond clock. None of these values enters
  consensus or a state root.
- The monitor reports those fields, warns below the configured certificate
  participation threshold, and applies ordered warning/critical cross-node
  clock-skew thresholds.
- `cargo test -p postfiat-node --lib --locked init_then_run_once -- --nocapture`
  — PASS `1/1` (after correcting an initial exact-filter invocation that ran
  zero tests).
- `python3 -m unittest python.tests.test_monitor_snapshot -v` — PASS `2/2`.
- `RUN_ID=productionization-20260716-cert-observability
  scripts/testnet-monitor-snapshot-smoke` — PASS against four real local RPC
  servers; monitor status `ok`.

**Disposition:** the certificate-participation and clock-skew subfindings of
`P1-OPS-01` are fixed locally. Proof-latency, disk-capacity, alert delivery and
independent fault drills remain open.

## 2026-07-16T06:52:00Z — structured evidence privacy scan completed

- `scripts/test-public-secret-scan` — PASS.
- `scripts/public-secret-scan` — PASS across the complete tracked tree; the
  scanner fails rather than skips tracked files above its 32-MiB bound.
- `node scripts/test-navswap-redaction-check.mjs` — PASS.
- `node scripts/navswap-redaction-check.mjs --repo-only --include-private
  --artifact-dir docs/evidence --json` — PASS, `1,286` files scanned, zero
  findings and zero permission-based skips.
- A separate value-shaped search for JSON mnemonic/master-seed/spend/viewing/
  private-key fields returned zero values. `strings -n 8` over all 167 tracked
  PNG/PDF media files returned no embedded private-key marker, local home path,
  mnemonic marker, or Jupyter-token marker. A representative wallet-created
  capture was visually inspected and contains only public address/status data.

**Disposition:** the structured/text evidence-capture privacy inventory is
complete with no additional secret P0. Exhaustive visual inspection or removal
of all 167 screenshots/PDFs remains the explicit media review and bloat queue;
it is not silently claimed complete.

## 2026-07-16T07:02:00Z — current integrated guarding gates

- `cargo fmt --all -- --check` — PASS after the metrics changes.
- `cargo check --workspace --all-targets --locked` — PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS.
- `PYTHONPATH=python python3 -m unittest discover -s python/tests -v` — PASS
  `125/125`.
- `scripts/docs-site-redaction-check` — PASS.
- `scripts/docs-site-build` — PASS under strict MkDocs; informational
  non-navigation page inventory remains visible.
- The complete node library battery started before the additive metrics change
  and remains in its final expensive Orchard proof test. It cannot serve as the
  final-candidate result after source changed; a fresh complete battery remains
  mandatory even if that earlier run completes green.

## 2026-07-16T07:18:00Z — first-funding public-key activation boundary closed

### Review and reproduction

- The React wallet already activates an unpublished funded account
  automatically with one minimal signed Account-lane self-transfer, limits the
  automatic mutation to one attempt per mounted address, reconciles ambiguous
  transport failure by reading the ledger before offering a retry, and rejects
  a ledger public key that differs from the unlocked wallet key.
- The reusable activation helper nevertheless accepted a finality response
  containing `accepted=true` without the terminal receipt `code`. A regression
  was written at the real `TxBuilder.publishPublicKey` boundary and failed with
  `Missing expected rejection`.

### Remediation and evidence

- `publishPublicKey` now returns success only when the on-chain receipt has
  both `accepted=true` and `code=accepted`; rejected, unknown, or malformed
  terminal responses fail closed. `ensurePublicKeyPublished` inherits the same
  boundary instead of applying a weaker second interpretation.
- `cd wallet-web && node --test src/lib/tx-builder.test.js` — the new test failed
  before remediation and passed after it.
- `cd wallet-web && npm test` — PASS `219/219`.
- `cd wallet-web && npm run build` — PASS.
- `cd wallet-web && npm audit --audit-level=high` — PASS, zero vulnerabilities.

**Disposition:** the Phase 6 public-key publication/first-transaction item is
closed locally. Activation is automatic after funding, explicit receipt-code
verified, key-matched, and ambiguity-reconciled; the manual button is only a
visible retry path after a genuine automatic failure.

## 2026-07-16T07:35:00Z — deterministic sequence-exhaustion handling

### Reproduction

- Compiler-assisted arithmetic review (`cargo clippy -p postfiat-execution
  --all-targets --locked -- -W clippy::arithmetic_side_effects`) identified
  unchecked sequence arithmetic in all six normal non-swap entrypoints.
- A real `execute_transfer` regression with a persisted `u64::MAX` sender
  sequence failed before remediation with `attempt to add with overflow` at
  `entrypoints.rs` and left no deterministic receipt.
- A separate test of the node's real local signed-transfer builder failed with
  the same overflow panic before signing.

### Remediation and evidence

- Transfer, PaymentV2, asset, escrow, NFT, and offer entrypoints now reject an
  exhausted sequence with `code=sequence_overflow`, use the checked expected
  value for mutation, and preserve the ledger on rejection. The local builder
  returns `InvalidData` rather than panicking.
- NAV/vault block-window additions flagged by the same audit now use saturating
  height deadlines, preventing debug-panic/release-wrap disagreement at the
  height domain boundary.
- The state-root-v2 activation had intentionally changed new-genesis hashing
  but left one old golden assertion stale. The test now pins both domains: new
  v2 genesis `f340b4b1…6026b83f` and legacy no-activation genesis
  `97982d73…1aa04a`; the legacy vector was retained, not overwritten.
- Targeted execution overflow regression — pre-fix panic, post-fix PASS `1/1`.
- Targeted node builder overflow regression — pre-fix panic, post-fix PASS
  `1/1`.
- `cargo test -p postfiat-execution --lib --locked -q --
  --test-threads=1` — PASS `135/135`. A parallel run produced one transient
  atomic-swap test failure that passed immediately in isolation; test-global
  isolation remains under review and is not counted as a green parallel gate.
- `cargo fmt --all -- --check` — PASS.
- `cargo check --workspace --all-targets --locked` — PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS.

**Disposition:** `P1-ARITH-01` is fixed locally. This closes the concrete
build-profile-dependent sequence defect, not the still-open exhaustive Phase 6
arithmetic/rounding review.

## 2026-07-16T08:02:00Z — mixed-family mempool admission aligned

### Reproduction

- Source comparison showed admission simulated every existing family before a
  new candidate, while proposal/block execution orders families as transfer,
  PaymentV2, asset, atomic swap, FastLane primary, escrow, NFT, offer. Some
  admission copies omitted atomic/FastLane entries entirely.
- A real node test admitted an asset transaction at sender sequence 1 and then
  submitted a transfer at sequence 2. Before remediation the transfer was
  incorrectly returned as an admitted `MempoolEntry` and persisted, even though
  canonical batch order executes that transfer before the asset and rejects
  `bad_sequence`.

### Remediation and evidence

- Candidate simulation now occurs at the candidate's exact canonical family
  boundary. Active atomic/FastLane entries are replayed between asset and
  escrow. FastLane and NFT use canonical prefixes that do not reorder later
  families ahead of the candidate.
- An initial whole-mempool verification approach correctly rejected the
  reproduction but regressed the established liveness rule for an already
  admitted atomic swap later paused by governance. That approach was not kept:
  canonical-prefix simulation skips the now-inactive swap exactly as proposal
  construction does, so unrelated traffic remains admissible.
- Mixed-family failing regression — pre-fix false admission/mutation; post-fix
  PASS `1/1` with `bad_sequence` and byte-equal mempool.
- Paused atomic non-wedging regression — PASS `1/1` after final correction.
- `cargo test -p postfiat-node --lib --locked mempool --
  --test-threads=1` — PASS `13/13`.
- `cargo fmt --all -- --check` — PASS.
- `cargo check --workspace --all-targets --locked` — PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS.

**Disposition:** `P1-MEMPOOL-01` is fixed locally and the Phase 6
admission/execution agreement item is closed. New transaction families remain
subject to the explicit canonical-order inventory.

## 2026-07-16T06:50:00Z — arithmetic review extended to external height boundaries

### Reproduction

- Added an SDK regression containing two structurally valid returned blocks
  whose first height is `u64::MAX`. Before remediation, adjacency validation
  panicked at `response_validation.rs` while computing `previous_height + 1`.
- Extracted the FastSwap bootstrap height-window predicate without changing its
  behavior and added an exhausted-tip regression. Before remediation it
  independently panicked while computing `tip_height + 1`.

### Remediation and evidence

- Both external/input-derived height increments now use `checked_add` and
  return explicit validation/configuration errors when the height domain is
  exhausted.
- SDK exhausted-height regression — pre-fix panic, post-fix PASS `1/1`.
- FastSwap bootstrap exhausted-tip regression — pre-fix panic, post-fix PASS
  `1/1`.
- `cargo test -p postfiat-rpc-sdk --locked --lib` — PASS `55/55`.
- `cargo fmt --all -- --check` — PASS.

**Disposition:** these concrete extensions of `P1-ARITH-01` are fixed locally.
The full monetary arithmetic and rounding inventory remains open and is not
implicitly closed by these boundary fixes.

## 2026-07-16T07:05:00Z — P0 native genesis-supply rewrite closed locally

### Reproduction

- Initialized a real node at height zero, decremented the replay-base faucet
  balance by one atom, and made the identical change to the materialized ledger.
- Before remediation, `verify_blocks` returned `verified=true`, zero blocks,
  and a new state root. The chain/genesis identity contained no native-supply
  commitment, so coordinated local rewriting was accepted.

### Remediation and evidence

- New `Genesis` records explicitly commit
  `native_supply_atoms=1000000000`; any other explicit value is invalid.
- Historical genesis JSON remains readable and retains its prior hash because
  the additive field is optional for decoding. Legacy replay nevertheless
  requires the same fixed supply at the faucet replay boundary.
- `validate_faucet_account` now requires the exact protocol amount, not merely
  a nonzero balance.
- Coordinated-rewrite regression — pre-fix false `verified=true`; post-fix PASS
  `1/1` with `genesis native supply` rejection.
- Existing malformed-key and one-file faucet tamper regression — PASS `1/1`.
- Genesis vectors pin new supply-bound, prior state-v2, and older legacy hashes.
- `cargo test -p postfiat-types --lib --locked` — PASS `82/82`.
- `cargo test -p postfiat-execution --lib --locked -- --test-threads=16` — PASS
  `135/135`.
- `cargo check --workspace --all-targets --locked` — PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS.

**Disposition:** `P0-NATIVE-SUPPLY-01` is fixed locally. The integrated
post-genesis native holdings plus cumulative fee-burn oracle remains a separate
Phase 6 gate and is not claimed complete.

## 2026-07-16T07:23:43Z — native supply replay/checkpoint oracle completed

### Real-boundary reproduction

- Extended the shipping FastLane primary-deposit test to require its accepted
  receipt to report the exact two-atom `fee_pft` debit.
- Before remediation the test failed at the real execution boundary with
  `left: 0, right: 2`: live PFT supply decreased, but the canonical receipt did
  not identify the decrease as a burn.

### Remediation

- Native FastLane deposit receipts now propagate exact charged/burned fees;
  checkpoint receipts likewise report native pending-fee burns removed from
  primary reserves.
- Canonical block replay and history-checkpoint construction compute live PFT
  across accounts, open native escrows, open-offer native sell balances, offer
  reserves, owned native objects, FastLane native reserves, and Orchard live
  turnstile value. Every block must satisfy
  `live_before - receipt_burns == live_after` with checked arithmetic.
- History checkpoints moved to domain-separated schema v2 and commit cumulative
  native fee burns. Validation requires `checkpoint_live + cumulative_burn ==
  genesis_supply`. A syntactically/hash-valid v1 fixture without the burn total
  is rejected and must be rebuilt from archived history.
- The FastLane state-root golden vector changed solely because new genesis now
  commits native supply; the new deterministic value is pinned as
  `8d60f187…74c6f1cb`.

### Evidence

- FastLane pre-fix regression: FAIL `0 != 2`; post-fix PASS `1/1`.
- Native custody-lane/overflow/mismatched-burn oracle: PASS `1/1`.
- Native escrow replay: PASS `1/1`; offer reserve/matching replay: PASS `1/1`.
- Orchard transparent-deposit/turnstile replay: PASS `1/1` in `101.02s`.
- Registry historical replay: PASS `1/1`.
- History prune, v2 cumulative-burn equality, post-prune append, and v1 refusal:
  PASS `1/1`.
- `cargo test -q -p postfiat-execution --lib --locked -- --test-threads=1`:
  PASS `135/135` in `100.35s`.
- `cargo check --workspace --all-targets --locked`: PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- `cargo fmt --all -- --check`: PASS.

**Disposition:** the Phase 6 native-supply-plus-explicit-fee-burn invariant is
closed locally for full replay and supported v2 pruned history. Legacy v1
checkpoints are deliberately not grandfathered without archive-backed rebuild.

## 2026-07-16T07:38:00Z — intermediate complete node suite

- The pre-observability full node library run completed PASS `168/168` in
  `2406.36s`, including the expensive valid/invalid Asset-Orchard proof test.
- It remains intermediate evidence only because additive metrics source changed
  after that run began. A fresh candidate run was started, then deliberately
  interrupted once the arithmetic source changed; no stale run will be labeled
  final-candidate evidence.

## 2026-07-16T07:40:43Z — P0 issued-supply custody omission reproduced and fixed locally

### Reproduction

- The issued-supply helper counted public trustlines, open escrows, and open
  offers, but not issued FastLane reserves or live AssetOrchard balances.
- A real replicated-state test constructed `max_supply=10`, public supply 10,
  and private custody 1. Before remediation,
  `cargo test -p postfiat-node --lib replicated_state_root_rejects_issued_supply_hidden_in_orchard_custody -- --nocapture`
  failed because `replicated_state_root` returned
  `abde05b9…c6cf9ee` instead of rejecting the aggregate supply 11.

### Remediation and evidence

- Execution mint-cap accounting now includes issued FastLane reserves with
  checked `u128` accumulation and checked conversion.
- The replicated-state boundary aggregates transparent/FastLane supply with
  committed live AssetOrchard balances, rejects unknown custody assets, and
  rejects aggregate supply above `max_supply`.
- Asset admission and whole-mempool verification apply the same combined
  invariant before persistence, so an invalid mint cannot wedge proposal
  construction.
- Post-fix state-root over-cap and exact-cap inverse — PASS `1/1`.
- FastLane reserve mint-cap/no-mutation regression — PASS `1/1`.
- Real AssetOrchard issued ingress/disclosed-egress round trip — PASS `1/1`.
- `cargo test -p postfiat-execution --lib -- --test-threads=1` — PASS
  `136/136` in `99.61s`.

**Disposition:** `P0-ISSUED-SUPPLY-02` is fixed locally at mint admission and
proposal/commit/replay state boundaries. The completed supported custody and
transition map is
`docs/status/OPEN-SOURCE-ISSUED-SUPPLY-INVENTORY-20260716.md`; the contained
external route remains a separate bridge P0 rather than a hidden residual.

## 2026-07-16T07:46:08Z — Phase 6 arithmetic and rounding inventory completed

- Ran `clippy::arithmetic-side-effects` diagnostically across the node's full
  dependency graph and separately over the type/execution production paths.
- Traced every monetary warning to checked arithmetic, an exact dominating
  balance/nonzero/range guard, or a named floor/ceil/exact policy. Separately
  classified bounded index/encoding math and intentional finite-field circuit
  arithmetic.
- The pass found `P0-ISSUED-SUPPLY-02`; it was reproduced and fixed rather than
  allowlisted. No additional unguarded monetary panic/wrap path remained.
- Wrote the code-referenced evidence matrix at
  `docs/status/OPEN-SOURCE-ARITHMETIC-ROUNDING-INVENTORY-20260716.md`.
- `cargo check --workspace --all-targets --locked` — PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS.
- `cargo fmt --all -- --check` — PASS.

**Disposition:** `P1-ARITH-01` and the Phase 6 checked-arithmetic/rounding
inventory are closed locally. The issued-supply transition oracle was then
closed in `OPEN-SOURCE-ISSUED-SUPPLY-INVENTORY-20260716.md`.

## 2026-07-16T07:54:36Z — P1 storage concurrent mempool lost-update fixed locally

### Reproduction

- Added a real `NodeStore` boundary regression with 24 synchronized writer
  threads, each constructing a separate store handle and calling
  `append_mempool_entry` once.
- Before remediation all 24 calls returned success, while the final mempool
  contained only 1 entry: every writer raced through the same whole-file
  read-modify-replace sequence.
- Command:
  `cargo test -p postfiat-storage concurrent_mempool_appends_do_not_lose_successful_writes -- --nocapture`
  — FAIL, observed `left: 1`, `right: 24`.

### Remediation and evidence

- Added a private mode-0600 mutation lock file and blocking Unix `flock` held
  across the complete mempool read, family append, serialization, fsync,
  atomic rename, and parent-directory fsync boundary.
- Direct whole-file mempool replacement uses the same lock; internal locked
  appends call a non-reentrant private writer to avoid self-deadlock.
- Target regression — PASS, 24/24 writes durable.
- `cargo test -p postfiat-storage -- --test-threads=1` — PASS `23/23`.
- Workspace check — PASS. Strict Clippy initially caught an unspecified
  `OpenOptions` truncation policy; `.truncate(false)` was made explicit and
  the guarding check/Clippy rerun follows in the next evidence entry.

**Disposition:** this closes a concrete storage-integrity race but not
`P1-STORAGE-01`; the indexed transactional engine, migration, and production
growth/fault gates remain open and are not being relabeled as production-ready.

## 2026-07-16T07:56:51Z — ordered commit/recovery made cross-process exclusive

- Added a separate mode-0600 Unix `flock` for the complete ordered-commit
  journal write, multi-domain apply, journal removal, and startup-recovery
  boundary. The lock is held by the public storage guard, so independent
  `NodeStore` handles and processes share the same exclusion contract.
- Regression `ordered_commit_lock_serializes_independent_store_handles` proves
  a second handle cannot enter while the first owns the lock and proceeds after
  release — PASS.
- `cargo test -p postfiat-storage -- --test-threads=1` — PASS `24/24`.
- `cargo test -p postfiat-node --lib ordered_commit_journal -- --test-threads=1`
  — PASS `1/1` (`status_recovers_pending_ordered_commit_journal`).
- `cargo check --workspace --all-targets --locked` — PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS.
- `cargo fmt --all -- --check` — PASS.

**Disposition:** concurrent commit/recovery interleaving is fail-closed. Linear
JSON state growth, indexed lookup/migration, and production-scale fault testing
remain the explicit `P1-STORAGE-01` blocker.

## 2026-07-16T08:04:17Z — non-production storage disabled by default on daemon surfaces

### Reproduction and remediation

- A real CLI-dispatch regression invoked `transport-validator-serve` with the
  already-required plaintext-signer acknowledgement but no storage
  acknowledgement. Before remediation it advanced to argument parsing
  (`missing --topology`) instead of rejecting the non-production store.
- Added the exact `--unsafe-devnet-json-storage` acknowledgement gate before
  argument parsing/bind/work on `rpc-serve`, `transport-validator-serve`,
  `transport-block-vote-listen`, `run`, both certified batch loops, and the
  certified private-egress loop.
- Generated devnet validator/RPC units, systemd examples, local integration
  harnesses, and controlled smoke callers carry the flag explicitly. README
  and SECURITY state that this is bounded controlled-devnet storage, not a
  production transactional engine.

### Evidence

- `cargo test -p postfiat-node --bin postfiat-node long_running_validator_service_requires_explicit_json_storage_acknowledgement -- --nocapture`
  — PASS; all seven dispatch families reject the missing acknowledgement.
- `cargo test -p postfiat-node --lib deployment_validator_unit_stage_is_canonical_and_non_overwriting -- --nocapture`
  — PASS; both staged units contain the exact flag.
- `bash -n` over repository shell scripts — PASS.
- Workspace check and strict Clippy — PASS.

**Disposition:** `P1-STORAGE-01` is closed for public-source publication by
default-off feature containment, not by pretending the current store is
production-ready. The indexed transactional engine, migration, scale,
backup/restore, and fault campaign remain mandatory real-value launch gates.

## 2026-07-16T08:08:16Z — P1 operations disk-capacity telemetry added

### Reproduction

- The synthetic monitor regression first required a critical result at 49,999
  available ppm and required the endpoint report to expose the metric.
- Before remediation it failed with no `disk_available_critical` and a missing
  `filesystem_available_ppm` key.

### Remediation and evidence

- `NodeStore::filesystem_capacity` uses checked Unix `statvfs` results and
  rejects NUL paths, syscall failure, multiplication/conversion overflow, and
  impossible available-greater-than-total results.
- Node `metrics` exposes total bytes, available bytes, and exact integer ppm.
- The monitor is critical when capacity is missing or at/below 5% and warns at
  or below 15%; threshold ordering is validated fail-closed.
- `cargo test -p postfiat-storage -- --test-threads=1` — PASS `25/25`.
- `cargo test -p postfiat-node --lib init_then_run_once -- --nocapture` — PASS.
- `python3 -m unittest python.tests.test_monitor_snapshot -v` — PASS `2/2`.
- Workspace check and strict Clippy — PASS.

**Disposition:** disk exhaustion is now observable before commit failure.
Proof-operation latency, alert delivery, and independent fault drills remain in
`P1-OPS-01`.

## 2026-07-16T08:54:01Z — P1 proof latency and durable alert emission added

### Reproduction

- The monitor threshold regression initially failed because node metrics had no
  `proofs` section and no `proof_verify_latency_critical` condition.
- The alert-spool boundary test initially failed with missing
  `write_alert_event`; a second adversarial test then proved the first
  implementation followed a symlink spool and wrote into the target directory.
- An initial real-proof run used the default debug profile despite the test's
  explicit release-only annotation. It was stopped without a result and rerun
  unchanged under `--release`; only the authoritative release run is evidence.

### Remediation and evidence

- AssetOrchard private-egress and private-swap verification collectors now
  record the exact Halo2 `verify_proof` duration and success/error result.
  Block-vote reconstruction selects each collector once, avoiding the prior
  cloned-report double-counting risk, and persists the latest duration as
  non-gating local operator telemetry.
- Node metrics expose integer microseconds and observation time. The monitor
  warns above 5 seconds, is critical above 15 seconds, and marks a previously
  observed sample stale after 5 minutes.
- Warning/critical monitor states can be atomically spooled as mode-0600,
  content-identified JSON envelopes in an owned mode-0700 directory. Repeated
  emission is idempotent; symlink spool directories fail closed. No shell hook,
  pager credential, or vendor-specific delivery claim was added.
- `cargo test --release -p postfiat-privacy-orchard swap_consensus_verifier_accepts_real_proof_and_rejects_forged_nonconservation -- --ignored --nocapture`
  — PASS `1/1` in `336.06s` with accepted and rejected timing assertions.
- `cargo test -p postfiat-node --lib init_then_run_once -- --nocapture` — PASS.
- `python3 -m unittest python.tests.test_monitor_snapshot -v` — PASS `4/4`.
- `cargo check --workspace --all-targets --locked` — PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS.
- `cargo fmt --all -- --check` — PASS.

**Disposition:** proof latency and durable local alert emission are implemented.
External alert delivery/dashboard health, a complete SLO/page and public-incident
policy, production custody, and independent fault drills remain in
`P1-OPS-01`; a local spool alone is not claimed as a delivered page.

## 2026-07-16T08:58:16Z — direct RPC saturation telemetry added

### Reproduction and remediation

- The Rust response-boundary regression initially did not compile because no
  runtime saturation snapshot or metrics merge function existed.
- The monitor regression independently failed because a 1,000,000-ppm active
  connection sample produced no `rpc_active_connections_critical` result.
- The listener now maintains atomic current, peak, and cumulative accepted
  connection counters without changing its existing 64-connection scheduling
  ceiling. Successful canonical `metrics` responses are overlaid with those
  counters, the configured limit, and checked integer utilization ppm.
- The monitor fails closed when the value is absent, warns at 750,000 ppm, and
  is critical at 950,000 ppm. Threshold ordering is validated.

### Evidence

- `cargo test -p postfiat-node --bin postfiat-node rpc_serve_metrics_include_direct_connection_saturation -- --nocapture`
  — PASS `1/1`.
- `python3 -m unittest python.tests.test_monitor_snapshot -v` — PASS `4/4`.
- Workspace check, strict Clippy, and formatting — PASS.

**Disposition:** every signal enumerated by the Phase 13 monitoring checklist
 now has an implemented source and ordered threshold. External delivery and
 independent operational drills remain open.

## 2026-07-16T09:00:19Z — incident/SLO contract made machine-readable

- The alert regression first failed because durable events lacked any incident
  policy fields.
- Alert envelopes now bind SEV-1/SEV-2 class, acknowledgement and incident-
  commander deadlines, ordered escalation targets, public-update deadline, and
  the canonical incident runbook.
- `docs/runbooks/incident-response.md` defines numeric controlled-pretestnet
  SLOs for monitor freshness, fleet agreement, receipt semantics, RPC latency
  and saturation, proof latency/freshness, disk headroom, alert delivery, and
  public updates; it also defines containment, evidence, and closure rules.
- Python monitor tests — PASS `4/4`; bytecode compile — PASS.
- Docs redaction and strict MkDocs build — PASS.

**Disposition:** the alert/SLO/severity/runbook/escalation/public-communication
definition checklist item is closed. External delivery health and independent
drill evidence remain unclaimed production-launch requirements.

## 2026-07-16T09:02:41Z — whitepaper shielded-authorization overclaim corrected

### Reproduction

- A semantic docs regression first failed on the candidate statement that
  “ML-DSA signs the outer transaction envelope.”
- Code inspection proved `ShieldedActionBatch` has only `batch_id` and
  `actions`. Asset-Orchard swap/private-egress actions instead verify randomized
  RedPallas signatures over chain/genesis/protocol-bound action sighashes; the
  block proposal/certificate is the ML-DSA-authenticated inclusion layer.

### Remediation and evidence

- The whitepaper now states the exact RedPallas action, Halo2 statement,
  chain-bound batch-id, and ML-DSA block-certificate boundary. It no longer
  invents registry-root/disclosure-policy fields or an account outer signature.
- The threat-model primitive list now discloses classical RedPallas, Halo2,
  Groth16, Orchard encryption, and Ethereum assumptions alongside PQ ML-DSA.
- The proving paragraph no longer equates “consensus verifies” with absence of
  prover tooling from the workspace; deployment separation is an explicit gate.
- `scripts/test-whitepaper-implementation-boundaries` — PASS and added to docs
  CI.
- Docs redaction and strict MkDocs build — PASS.

**Disposition:** this concrete `P1-DOCS-01` mismatch is closed. The remaining
matrix rows continue through the same code-to-claim reconciliation before the
overall docs finding can close.

## 2026-07-16T09:08:00Z — new P0: Asset-Orchard ingress note opening is public

- Code-to-claim tracing found that live `asset_orchard_ingress_v1` includes the
  complete `AssetOrchardIngressNote` in its certified/archived batch payload:
  value, `rho`, `psi`, `rcm`, diversifier, asset tag and recipient key data.
- The wallet copies that local-vault note into the remote relay request and its
  fallback `encrypted_output` is a deterministic plaintext label, not the
  available `PFAOENC1` ChaCha20-Poly1305 envelope.
- This is elevated as `P0-PRIVACY-02`; it is not being papered over as expected
  ingress leakage. The safe closure is a new opaque encrypted ingress version,
  with v1 live-disabled and retained only for exact historical replay.

**Disposition:** OPEN and first in implementation priority. No publication
candidate can close while a supported “private” ingress archives its note
opening.

## 2026-07-16T09:20:33Z — P0-PRIVACY-02 encrypted ingress v2 closed locally

- The pre-fix real batch-boundary regression failed while printing a certified
  `asset_orchard_ingress_v1` payload containing `note.value`, `rho`, `psi`,
  `rcm`, diversifier and recipient key material. The new wallet regression also
  failed because `buildAssetOrchardIngressPayload` still emitted `note` and a
  deterministic plaintext label.
- Added `asset_orchard_ingress_v2`: signed public burn, pool/asset/amount,
  output commitment and bounded `PFAOENC1` ciphertext only. Live proposal and
  execution reject v1 without mutation; the same valid v1 fixture is accepted
  only with explicit archive replay.
- The loopback prover now returns genuine randomized note ciphertext separately
  from the private vault note. React and E2E callers require that ciphertext;
  the plaintext fallback was removed. The proxy and validator both fail closed
  on non-`PFAOENC1` data.
- `cargo test -p postfiat-node --lib asset_orchard_ingress_and_disclosed_egress_round_trip_issued_asset -- --nocapture` — PASS 1/1, including v1 live/replay and v2 serialized-field/conservation assertions.
- `cargo test -p postfiat-node --bin asset-orchard-local-service` — PASS 21/21.
- `npm test && npm run build && npm audit --omit=dev` in `wallet-web` — PASS 219/219, production build PASS, 0 vulnerabilities.
- `node run_tests.js` in `wallet-proxy` — PASS 22/22.
- `cargo check --workspace --all-targets --locked`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS.
- `forge test` — PASS (`Nothing to compile`; Solidity unchanged).

**Disposition:** FIXED-LOCAL / V1 HISTORICAL-REPLAY-ONLY. The full workspace
suite and final documentation candidate gates remain part of integrated
closure; the supported live ingress no longer publishes a note opening.

## 2026-07-16T09:34:00Z — P0-CUSTODY-02 real CLI seed echo removed

- During integrated golden-vector triage, the shipping `wallet-test-vector`
  command was invoked with explicit master and signature seeds. Its stdout
  reproduced both exact values under `master_seed_hex` and
  `signature_seed_hex` while claiming `private_key_material_redacted=true`.
- Removed both fields from `WalletTestVectorReport` and versioned the output as
  `postfiat-wallet-test-vector-v2`; all public derivations and the deterministic
  signed transaction remain available.
- `cargo test -p postfiat-node --lib wallet_test_vector_is_deterministic_and_redacted -- --nocapture` — PASS; it now rejects both secret field names and both input values.
- A real `cargo run ... wallet-test-vector ... > /tmp/...json` subprocess check
  parsed the v2 JSON and proved neither field name nor supplied seed appeared —
  PASS.

**Disposition:** FIXED-LOCAL. This was found by STEP 1 review and closed in STEP
2 immediately because false-redacted secret output is a public-repository P0.

## 2026-07-16 — P0-CUSTODY-02 shipping-boundary artifact scan

- Added `crates/node/tests/wallet_test_vector_redaction.rs`, which invokes the
  compiled `postfiat-node wallet-test-vector` command using mode-0600 seed files.
- The test covers both success and a zero-amount failure after both secret files
  have been ingested. It scans stdout, stderr, the exact argv vector, every
  non-secret working-directory artifact, and crash/panic artifact names for the
  supplied values and sensitive field names.
- `cargo test -p postfiat-node --test wallet_test_vector_redaction --locked --
  --nocapture` — PASS 1/1 in 0.17s.

**Disposition:** FIXED-LOCAL / IMMUTABLE-CANDIDATE RERUN PENDING. No CLI seed
value or secret field name crossed the observed shipping subprocess boundary.

## 2026-07-16T09:48:00Z — integrated workspace RED exposed fixture drift and a stale-atomic liveness regression

- `cargo test --workspace --all-targets --locked` reached the node suite with
  every earlier crate green, then reported 161 passed / 12 failed. Eleven
  failures were audit-fixture drift made visible by the new global monetary and
  state-root invariants: atomic/NAV fixtures declared zero circulating supply
  while constructing 50/80 atoms, a WAN Orchard fixture declared zero for 40
  atoms, a FastLane root fixture reserved nonexistent issued assets, and the
  exact wallet/state-root goldens still described the pre-fix encoding.
- Corrected the fixtures to state exact finalized issued supply, define and
  canonically order the reserved assets, and compare preactivation roots from
  identical base state. Updated exact deterministic roots and public wallet-v2
  derivation/signing/transaction identifiers from the real shipping CLI.
  Focused wallet-vector and state-root tests pass; the ordered FastLane root
  test passes twice with an identical root.
- The atomic-swap filter then exposed a real `P1-MEMPOOL-01` liveness edge: a
  pending atomic swap already stale against the committed ledger caused an
  unrelated transfer to fail admission. Admission now proves whether each
  pending atomic/FastLane entry is valid against the existing canonical prefix
  before applying the candidate. Independently stale entries are skipped for
  admission and later evicted; valid entries still reject candidate-induced
  sequence or balance conflicts.
- `cargo test -p postfiat-node --lib atomic_swap_consensus:: --
  --test-threads=1` — PASS 15/15, including the stale non-wedge regression and
  both valid conflict orderings.

**Disposition:** fixture-only REDs are corrected without weakening the new
invariants; the one real integrated liveness defect is fixed locally. The full
workspace rerun remains the integrated closure gate.

## 2026-07-16T10:06:00Z — P0-PUBLIC-EVIDENCE-01 raw note openings removed from publication tree

- The publication scan found 1,283 tracked files under `docs/evidence/`
  (36,326,352 source bytes). Seven legacy-ingress artifacts contained 21 real
  `rho`/`psi`/`rcm` values plus recipient/amount data and the old plaintext
  `encrypted_output`; the existing secret scanner did not recognize this class.
- Added a real scanner regression with a synthetic note opening. It failed
  before the rule (`shielded note opening must fail`) and passes after adding
  the value-redacted `private-note-opening` rule.
- `scripts/public-secret-scan` then failed on the 21 real tracked-tree values,
  reporting only rule/path/line metadata and no opening bytes.
- Preserved the complete directory as deterministic restricted archive
  `postfiatl1v2-docs-evidence-20260716`: 1,283 files, 38,144,000 tar bytes,
  SHA-256 `ac6911368cb199e475dce8fce2309ffd18811ab9c6ca5048aae9a85084cb5eea`.
  Full listing count and an extracted ingress-batch hash matched the source.
- Removed raw evidence from the publication candidate and retained only ten
  curated evidence summaries plus a redaction-safe policy/archive manifest.
  `scripts/test-public-secret-scan`, the complete tracked-tree scan, and the
  strict documentation build now PASS.

**Disposition:** FIXED-LOCAL for the candidate tree and raw-evidence bloat.
Historical commits are not declared clean; sanitized public history remains a
shared fail-closed gate with `P0-SECRET-01`.

## 2026-07-16T10:18:00Z — retained screenshot/PDF publication review completed

- After raw-evidence removal, 19 non-icon screenshots and two PDFs remained.
  A contact-sheet visual review found controlled wallet balances/addresses and
  superseded bridge UX, but no reason to distribute these unreferenced design
  captures as supported product documentation.
- Archived the 19 screenshots together with the redundant locally downloaded
  VeriLLM PDF as 20 deterministic files, 3,235,840 archive bytes, SHA-256
  `be670b538db5a56d2c00ef4c4fc1cecd07c649687f45d01d50510ea6964caf37`,
  then removed them from the publication tree. VeriLLM remains cited by DOI.
- Retained only the public extension icons and the 500,545-byte Cobalt source
  PDF. The latter already has a pinned hash, local Markdown extraction, source
  URL, retention policy, and verification gate.

**Disposition:** the screenshot/PDF checklist item is complete. No raw browser
capture or unneeded downloaded research PDF remains in the candidate tree.

## 2026-07-16T10:31:00Z — public-source portability and embedded build paths closed

- A new portability regression first failed on a systemd unit, Python
  transaction-matrix defaults, nine proxy tests, 14 files containing live public
  IPv4 literals, and both checked wallet-WASM binaries. The binaries embedded
  this builder's Cargo registry home in panic/source strings.
- Systemd now uses the documented `/opt/postfiat` install prefix. Proxy tests
  resolve the repository from `__dirname`. The transaction matrix and live
  atomic-smoke script require caller-selected wallet files instead of selecting
  controlled local keys. Other live/evidence scripts use XDG/temporary paths or
  explicit environment inputs. Real fleet IPs were replaced with RFC 5737
  documentation addresses, including the replay topology fixture.
- Added `scripts/build-wallet-wasm-release`, which applies a stable Rust
  `--remap-path-prefix`, generates both supported copies from one package, and
  checks byte identity plus absence of the builder home. Two consecutive builds
  produced identical SHA-256
  `395576c1efa2fc5115e94df17645f1fb0f5584fd5ce4f7677e6e3539258ea5a2`.
- `scripts/test-public-source-portability` — PASS across the tracked tree;
  `scripts/public-secret-scan` — PASS; transaction-matrix tests — PASS 6/6;
  wallet-proxy regression suite — PASS 22/22.

**Disposition:** public source no longer exposes a live IP, maintainer home path,
implicit controlled-wallet path, or binary-embedded builder path. Historical
roles/procedures remain only in clearly dated records and contain no live
endpoint or credential value.

## 2026-07-16T10:46:00Z — all 719 full-history generic findings classified

- Parsed the value-redacted 719-row Gitleaks report by field and path without
  emitting or hashing candidate values. Scope remains 2,288 commits, 258.68 MB,
  233 files and 38 commits; only the generic-api-key rule fired.
- Classification reconciles exactly: 660 public EVM/token/pool identifiers, 43
  test/schema/fixture labels, 13 public verification/hash values, and three
  copies of one real Jupyter credential. The four `MASTER_SECRET` rows are the
  canonical public XRPL genesis benchmark credential in removed comparison
  scripts, not PostFiat/operator custody material.
- Wrote `OPEN-SOURCE-SECRET-HISTORY-CLASSIFICATION-20260716.md` with field-class
  counts, path-purpose evidence, and disposition. No broad allowlist was added.

**Disposition:** `P1-HISTORY-01` classification is complete. The three real
credential locations remain one external `P0-SECRET-01` requiring provider
revocation/decommission and sanitized public refs; classification does not
waive that gate.

## 2026-07-16T10:21:26Z — RPC authorization inventory false-read classification corrected

- Added `scripts/test-rpc-method-inventory` against the shipping `rpc-serve`
  policy. Before the fix it failed at the real exposure boundary:
  `fastswap_prepare: expected authorized_protocol_mutation_public, got
  read_only_public`.
- The old generator treated the first no-flag allowlist branch as reads and
  hard-coded only two signed-submit methods. It therefore mislabeled FastSwap
  lock/vote/apply state transitions and omitted twelve gated submission
  methods, including atomic-swap finality and shield finality.
- The v2 generator extracts all signed-submit and Orchard gates from their Rust
  helpers, explicitly partitions the default allowlist, distinguishes the
  owned-lane gate and local CLI dispatch, and fails closed if any observed
  method is unclassified. The corrected result is 135/135 classified: 63
  reads, 12 public cryptographically authorized protocol mutations, 14 gated
  signed submissions, four gated Orchard, four gated owned-lane, and 38
  operator/local; zero unknown.
- `scripts/test-rpc-method-inventory` — PASS;
  `scripts/testnet-rpc-method-inventory --output
  docs/status/OPEN-SOURCE-RPC-AUTHORIZATION-INVENTORY-20260716.json --markdown
  docs/status/OPEN-SOURCE-RPC-AUTHORIZATION-INVENTORY-20260716.md` — PASS with
  every internal verification check true; Python compilation and workflow YAML
  parse — PASS. Product-security CI now runs the regression.
- Snapshot SHA-256: JSON
  `6b369f7f9bbcca9eb218164a1cb7b830daf3bd032bab72d220f74ed5262fdef3`;
  Markdown
  `a7f6749960f2b61957aad80e22f275120f4b40808a31d83b78f7ee967047328f`.

**Disposition:** `P2-API-01` is fixed locally and regression-gated. The
inventory now describes exposure and authorization honestly; it does not call
signed or certificate-authorized state mutation a read.

## 2026-07-16T10:26:09Z — P1-RPC-ERROR-01 remote operator-path disclosure closed

- Added a response-boundary regression using an internal failure containing
  `/home/operator/private-validator/ledger.json`. Before the fix,
  `rpc_serve_internal_failures_do_not_expose_operator_paths` failed because the
  exact path and filename were returned to the remote client.
- The public response helper now emits stable messages for internal, worker,
  timeout, read, status, mempool-status and FastSwap-unavailable failures. Any
  other absolute-path-bearing error is replaced with `request failed`.
  Purpose-specific protocol rejections such as `wrong_nav_epoch` retain their
  safe typed message.
- Focused pre-fix test — FAIL as expected; focused post-fix test — PASS; complete
  `rpc_serve_request_tests::` suite — PASS 18/18; `cargo fmt --all -- --check`,
  `cargo check --workspace --all-targets --locked`, and
  `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS.

**Disposition:** FIXED-LOCAL. Remote errors no longer reveal operator paths or
private state filenames, without collapsing protocol rejection semantics.

## 2026-07-16T10:28:44Z — binary/media bloat disposition made executable

- Re-inventoried the current tracked publication tree after the raw-evidence and
  screenshot archive pass. Exactly 14 binary/media paths remain (13 distinct
  blobs); the largest is the 2,097,220-byte active Asset-Orchard parameter.
- Wrote `OPEN-SOURCE-PUBLIC-ARTIFACT-DISPOSITION-20260716.md` with a keep/archive
  rationale for every class: SP1 verifier fixtures, active and replay-only
  Orchard parameters/VKs, the sole retained Cobalt source PDF, vendored Halo2
  fixture, extension icons, and wallet WASM.
- Added `scripts/test-public-artifact-policy`. It enumerates the exact tracked
  media set from Git, checks each SHA-256, rejects additions/removals, and proves
  the web/extension WASM copies are byte-identical. Product-security CI runs it.
- `scripts/test-public-artifact-policy` — PASS:
  `classified=14 max_bytes=2097220`; Python compilation and workflow YAML parse
  — PASS.

**Disposition:** binary/media artifact bloat is closed and regression-gated.
The duplicated WASM paths share one Git blob and remain deliberately present for
offline package builds. Status-diary and one-off-script classification remain
separate source-organization work, not an unclassified binary artifact gap.

## 2026-07-16T10:41:00Z — determinism, canonical transcripts, and crypto call sites frozen

- Audited the production replicated-transition boundary for wall clocks,
  randomness, environment input, filesystem enumeration, unordered collections
  and floating point. The execution, ordering, mempool-reference and state-root
  builders contain none of those inputs; wider-node matches are local
  timing/timeouts, file uniqueness, pre-submission Orchard randomness,
  membership/deduplication, sorted job enumeration or local metrics.
- Added `scripts/test-consensus-determinism-surface`. It checks 23 production
  Rust files across six forbidden nondeterminism classes with no allowlist and
  passes: `consensus_determinism_surface=ok files=23 categories=6`.
- Completed the enabled canonical-encoding map: account-family fixed
  transcripts, W6 dual-auth preimages, FastSwap binary codecs, canonical block
  artifacts, mempool reference JSON, tagged state roots, governance canonical
  JSON, bridge fixed structs/BTree maps and Orchard fixed public instances.
  Existing golden and mutation vectors were traced to each family.
- Added `scripts/test-crypto-callsite-policy`. It freezes all 46 production
  calls to the generic account context and deterministic seed/key APIs; a new
  purpose or deterministic site fails CI pending review. PASS:
  `crypto_callsite_policy=ok apis=4 calls=46`.
- `cargo test -p postfiat-crypto-provider --locked` — PASS 3/3.
- `target/debug/postfiat-fuzz all --iterations 64` — PASS across 15 targets,
  1,700 parser/invariant cases and zero invariant failures. An initial package
  name typo (`postfiat-fuzz-harness`) was corrected to the manifest package
  `postfiat-fuzz`; it made no repository change.
- Python compilation for both new gates and product-security workflow YAML
  parsing — PASS. Both gates are now blocking CI steps.

**Disposition:** no reachable P0/P1 nondeterministic state input or canonical
encoding collision was found. Cross-architecture archive replay and exhaustive
historical Serde conformance vectors remain real-value release assurance, not
unclassified public-source blockers.

## 2026-07-16T10:41:00Z — canonical whitepaper matrix final local reconciliation

- Reconciled the matrix against the corrected `docs/whitepaper.md` and current
  remediation tree. The paper now enumerates the actual replicated domains,
  while the matrix records the native holdings-plus-burn oracle, fixed-genesis
  registry boundary, view-zero direct-certificate containment, ML-DSA call-site
  policy and enabled/disabled feature status.
- Updated the operations inventory's stale 132-method text to the verified v2
  135-method authorization classification.
- `scripts/docs-site-build` — PASS; `scripts/docs-site-redaction-check` — PASS;
  `scripts/test-whitepaper-implementation-boundaries` — PASS after the sweep.
  MkDocs reported only the known informational list of pages outside navigation.

**Disposition:** `P1-DOCS-01` is fixed locally rather than “reconciling.” Clean
hosted build evidence still depends on freezing a candidate commit; empirical
performance evidence remains explicitly controlled rather than production
proof.

## 2026-07-16T10:48:00Z — sanitized-publication boundary made fail-closed

- Added `scripts/verify-publication-candidate`. It requires a clean,
  non-shallow checked-out staging clone, an exact allowlist of all published
  refs, `HEAD` on one of those refs, and an exact reviewed `HEAD^{tree}` before
  it runs both current-tree and all-reachable-history secret scans.
- The regression constructs real temporary Git repositories. A clean one-ref
  candidate passes; an unexpected tag, unreviewed tree drift, and a synthetic
  credential added then deleted from the current tree each fail. Result:
  `publication_candidate_gate=ok clean=pass unexpected_ref=reject
  tree_drift=reject dirty_history=reject`.
- The test and gate are blocking in product-security CI. Python compilation and
  workflow YAML parsing pass. Gate SHA-256:
  `60f8b69d8be9e272511c18fc4d11c13e2e639ecd04d30a8d54df0b4fcc788fa3`;
  regression SHA-256:
  `74c6c9177e5400525fb933a3b50ae2f3446dc80c470f67c9a012f24c782bbe3e`.
- `scripts/public-secret-scan` — PASS on the tracked candidate tree;
  `scripts/docs-site-build` and `scripts/docs-site-redaction-check` — PASS.

**Disposition:** local publication mechanics can no longer omit hidden refs,
accept a shallow clone, drift from reviewed source, or overlook a credential
deleted only from the current tree. `P0-SECRET-01` remains OPEN-EXTERNAL until
the provider owner supplies revocation/decommission evidence and the sanitized
staging clone itself passes this gate; no credential value was printed or
allowlisted.

## 2026-07-16T10:52:00Z — proof public inputs frozen; redaction reporter stopped re-leaking matches

- Added the machine-readable
  `OPEN-SOURCE-PROOF-PUBLIC-INPUT-INVENTORY-20260716.json`: exact indices and
  bindings for all 28 live AssetOrchard swap fields and all 13 private-egress
  fields, complete private-witness classes, host-only checks, live/replay
  circuit policy, SP1 host-decoded ABI fields and debug-proof reachability.
- The inventory does not hide residuals: the live circuits retain compatible
  older H_action circuit constants while VK policy enforces the new live IDs;
  SP1 verifies all public bytes against a profile vkey but the guest source and
  program-vkey reproduction are absent; the debug proof adapter is supported
  only by bench/legacy-replay code. Those remain explicit activation/profile
  gates.
- Added `scripts/test-proof-public-input-inventory`, which pins five exact proof
  source hashes and exact index coverage. PASS:
  `proof_public_input_inventory=ok systems=4 public_fields=41 source_hashes=5`.
  Inventory SHA-256:
  `d0851cbc69ed0103ef94fda4c2837cca1769e91603b6b41a2aa2ff5b695d1f8f`;
  gate SHA-256:
  `cdca0c0ca2bc2a117c1c8fbdf23f6c331446a34627187eb7a6dd27da73eec66d`.
- A new redaction regression failed before the fix because the JSON report
  echoed the detected master-seed match verbatim; the scanner also failed to
  detect a synthetic `rho`/`psi`/`rcm` note opening. The v2 report removes
  matched samples entirely and detects note openings plus spend-authorization
  keys while emitting only rule/path/line/message metadata.
- Post-fix `node scripts/test-navswap-redaction-check.mjs` — PASS;
  repo-only v2 scan — PASS over all configured public targets;
  `scripts/public-secret-scan` and its regression — PASS. The product-security
  workflow runs both the proof and redaction gates.

**Disposition:** the previously missing circuit public-input deliverable is
closed and code drift now fails CI. The evidence scanner no longer becomes a
secondary secret-disclosure channel. Fresh deployed wire/log/browser captures
and independent circuit review remain real-value gates.

## 2026-07-16T10:54:00Z — first-party full-history privacy baseline reconciled

- Ran `scripts/public-secret-scan --history` against every current private ref.
  It failed closed as required with 27 metadata-only findings: three
  `jupyter-token` occurrences and 24 `private-note-opening` occurrences. No
  matched value or reusable value hash was emitted.
- The 24 privacy findings are exact and explainable: six opening fields in one
  historical legacy-ingress response and three in each of six removed
  private-swap ingress batch/deferred-send captures, across seven paths total.
  This complements the 719-row Gitleaks classification; Gitleaks' generic rule
  never recognized `rho`/`psi`/`rcm`.
- Updated the publication procedure, history classification, P0 evidence and
  closure row. The expected private-history count is diagnostic only. The
  sanitized staging gate still requires absolute zero and has no path/value
  allowlist for either class.

**Disposition:** no new current-tree exposure; `P0-PUBLIC-EVIDENCE-01` remains
fixed locally with its history gate open, and `P0-SECRET-01` remains the sole
externally open P0. Sanitized publication must remove all 27 reachable
findings, not only the provider capture.

## 2026-07-16T10:57:00Z — closure table made executable

- Added `scripts/test-productionization-closure-table`. It extracts all P0/P1
  finding headings, requires exactly one eight-column nonblank closure row per
  finding, rejects duplicates/missing rows, and has a publication mode that
  rejects `OPEN`, `PENDING`, or `WAIVED` status.
- The first run found a real audit-schema defect: `P1-DOCS-01` had seven rather
  than eight columns and conflated integrated evidence with the claim update.
  The row now separates those fields.
- Normal consistency gate — PASS:
  `productionization_closure_table=ok findings=34 p0=18 p1=16 open=2`.
  `--require-closed` — expected RED, naming only
  `P0-PUBLIC-EVIDENCE-01` and `P0-SECRET-01`. Feature-contained storage and the
  versioned state-root deployment drill are correctly classified as real-value
  gates rather than falsely open source-publication P1/P0 rows.
- The gate is blocking in product-security CI, and the public-history runbook
  requires `--require-closed` in the sanitized staging clone. SHA-256:
  `c5d61f126e17cfbde3260552f72713ee436e6b5934ef7f89d52515096adaeba0`.

**Disposition:** the closure register is complete and machine-checked. Exactly
two public-publication rows remain open, both resolved only by the same
sanitized-history operation, with the credential row additionally requiring
provider revocation/decommission evidence.

## 2026-07-16T11:05:00Z — Python release boundary restored and added to CI

- The first repository-wide invocation, `python3 -m pytest python/tests`, failed
  collection because the repository package root was not on `PYTHONPATH`. The
  correct packaged invocation then exposed a real stale import:
  `postfiat_rpc.latency` still imported `DEFAULT_WALLET_A/B` after the public
  portability remediation removed those host-specific wallet paths from
  `transaction_matrix`.
- Removed the stale import and made `latency --wallet-a/--wallet-b` explicit
  required inputs, matching `transaction_matrix`. Added a CLI regression that
  proves the command fails before any fleet or wallet action when either
  descriptor is omitted.
- Added hash-locked `requirements-test.in`/`requirements-test.txt` and a pinned
  Python 3.12 product-security CI job. A fresh temporary virtual environment
  installed with `pip --require-hashes` and ran all Python SDK/operations tests:
  PASS 134/134. The same suite passes in the audit environment.
- Hashes: test lock
  `12925398725af556ef9c7e259501e9a17e087e2530eef962449ae2e601304ad7`;
  latency module
  `e45a72fa459f83dd71954ead0db0e0468e9914fda466acc43aa282db4d6e3642`;
  regression
  `40194b1deecc9f36f68c1d43b054a2685ce8af8f0301fa8afeb8518f8f864a7d`.
- Workflow YAML parsing, `scripts/test-public-source-portability`, and
  `git diff --check` pass after the change. No live endpoint, key, wallet, or
  money path was touched.

**Disposition:** this is a `P1-CI-01` completeness fix, not a newly open
finding. The public SDK no longer depends on deleted operator-local wallet
defaults, and its full test boundary is now a required clean-checkout job.

## 2026-07-16T11:08:00Z — sanitized-history publication rehearsal

- Exported the 1,561 existing candidate files (tracked modifications plus
  non-ignored new files, excluding removed raw evidence and local build/runtime
  output) into a temporary repository with one `main` commit and no other refs.
- Ran the candidate's own `scripts/verify-publication-candidate` against its
  exact tree. Both tracked-tree and all-reachable-history scans passed with
  zero findings; exact ref, clean worktree, non-shallow history and tree binding
  also passed. Rehearsal tree:
  `11710623dee0182e1bb525a4f6caa5c6c9dbad62`.
- Added `crates/node/data/` to `.gitignore` after the export inventory exposed
  a zero-byte ordered-commit lock as untracked runtime output. It was not added
  to the rehearsal or treated as source.

**Disposition:** the local clean-export mechanism is proven and all new audit
files are included in secret scanning once staged. This does not replace the
final reviewed immutable revision, private staging-remote fetch, or provider
revocation record; `P0-SECRET-01` therefore remains open and publication still
fails closed.

## 2026-07-16T11:13:00Z — documentation links made executable

- Added `scripts/public-doc-links`, which renders all root/public docs with the
  repository Markdown extensions and validates local file, image, script,
  stylesheet and cross-document anchor targets. It rejects repository escapes
  and symlink escapes and ignores fenced-code pseudo-links.
- Added a real temporary-tree regression: a valid file+anchor passes, while a
  missing file and missing anchor each fail closed. The gate passes all 249
  current Markdown documents with zero unresolved local targets and is now in
  strict docs CI before the MkDocs build.
- Gate SHA-256:
  `9d4b43be166b45cc55eb3650d61beb9d12b7e5862df245c8fb6f22ad8c9b194f`;
  regression SHA-256:
  `d39836e636f0806d67cfc49854c3442bcfdfd2000eaacc0a9c7cbffeb0e4c9ae`.

**Disposition:** the previously manual documentation-link criterion now has a
blocking reproducible gate. A final clean candidate run is still required
because the current remediation worktree is not an immutable release commit.

## 2026-07-16T12:24:39Z — FastPay default capability and certificate domain restored

- Replaced the audit branch's enable-only experimental posture with default
  signed FastPay RPC availability and an exact emergency
  `--disable-owned-lane` option. The real-process regression
  `crates/node/tests/fastpay_default_rpc.rs` starts both modes and verifies the
  advertised capability and domain.
- Added the v2 FastPay certificate domain to owner authorizations and validator
  votes: schema, chain ID, genesis hash, protocol version, and active registry
  ID. Execution rejects a foreign domain before mutation; wallet SDK/WASM
  refuses to sign when the order chain differs from the wallet chain; the web
  client fails closed when the server omits the domain.
- Guarding commands — PASS:
  - `cargo test -p postfiat-execution --locked`: 137/137;
  - `cargo test -p postfiat-rpc-sdk --locked`: 56/56 plus binaries;
  - `cargo test -p postfiat-wallet-wasm --locked`: 1/1;
  - `cargo test -p postfiat-node fastpay --locked`: seven node safety tests plus
    default-mode CLI coverage;
  - `cargo test -p postfiat-node --test fastpay_default_rpc --locked`: 1/1;
  - `npm test` in `wallet-web`: 220/220;
  - `cargo clippy -p postfiat-execution -p postfiat-rpc-sdk
    -p postfiat-wallet-wasm -p postfiat-node --all-targets --locked -- -D warnings`:
    PASS.
- During the first guarding run, the new WASM canonical-byte regression exposed
  a stale v1 prefix/offset expectation. The test was corrected to parse and
  assert every v2 domain field before the economic fields; no production
  validation was weakened.

**Disposition:** default FastPay payment availability is restored without
domainless signing. `P1-FASTPAY-01` remains open solely for the modeled,
bounded safe-cancellation protocol and its full wallet/six-node performance
acceptance. Legacy v1 locks require an explicit drain or an evidence-preserving
authorized devnet reset before v2 activation; the founder has authorized such
resets, but none was performed in this local step.

## 2026-07-16T12:41:53Z — consensus v2 artifacts, model, and durable safety-store foundation

- Added canonical v2 proposal, prepare/precommit vote, QC, timeout-vote, timeout
  certificate, QC-reference, and durable safety-state types. Every artifact is
  signed over chain/genesis/protocol, committee epoch/root, height/view, parent,
  payload, state root, validator, and phase.
- Reproduced the legacy timeout flaw with a failing regression: the old
  aggregator selected `qc-view-9` over `qc-view-10` by lexical order. It now
  rejects heterogeneous opaque high-QC IDs. V2 replaces those strings with
  graph-resolved typed QC references and numeric round ranking; conflicting QCs
  at one numeric rank reject.
- Selected an explicit two-phase rule for v2: prepare QC establishes the durable
  lock; only a non-nil precommit QC can commit. A caller cannot mutate safety
  state through the public API until proposal/QC signatures and domains verify.
- Added exhaustive quorum-pair models for `n=4` and `n=6`. Minimum intersections
  are respectively 2 and 4, leaving at least 1 and 3 honest validators after the
  `f=1` allowance. Signed simulations prove a failed view-0 proposer advances to
  view 1 and commits for both committee sizes.
- Added the node per-height atomic safety store. It derives the exact live
  committee domain, serializes concurrent authorization, and persists prepare
  round, precommit round, lock, high QC, and last vote digest before a caller can
  emit a signature. Restart tests reject duplicate prepare and precommit votes.
- Evidence — PASS:
  - `cargo test -p postfiat-ordering-fast --locked`: 26/26;
  - node durable prepare/precommit restart regression: 1/1;
  - strict Clippy for types, ordering-fast, and node with all targets;
  - `cargo check -p postfiat-node --all-targets --locked`;
  - `git diff --check`.

**Disposition:** `P0-CONSENSUS-01` remains open. The safe single-view production
containment is unchanged; v2 is not activated or deployed. Next gates are
production vote/transport wiring, signed timeout/view advancement, activation
and legacy replay, partition/crash campaigns, then a six-node failed-proposer
commit/root test. A shared-devnet reset is founder-authorized once that complete
candidate is green and the pre-reset evidence bundle is sealed.

## 2026-07-16T12:50:34Z — durable timeout/QC state and reset-ready activation domain

- Extended the consensus v2 safety record with a durable timeout round and
  timeout-vote digest. Timeout authorization resolves its typed high QC through
  the verified graph and rejects duplicate/regressive timeout signing after restart.
- Added an immutable, atomically persisted QC store. Every loaded certificate is
  signature/domain verified, its filename must equal its certificate ID, and a
  conflicting replacement fails closed.
- Added explicit `init-consensus-v2` and `topology-consensus-v2` commands with a
  positive activation height committed into genesis. Legacy initialization omits
  the field, preserving the existing serialized genesis and v1 replay domain.
- Reconfirmed in the master plan that founder-authorized shared-devnet resets are
  available without another approval, but must be batched, preceded by hashed
  state evidence, and followed by deterministic 6/6 convergence and conservation.
- Evidence — PASS:
  - `cargo test -p postfiat-ordering-fast --locked`: 26/26;
  - `cargo test -p postfiat-node consensus_v2 --locked`: 2/2 selected node
    regressions, including restart-safe timeout/QC reload and exact
    genesis/topology activation-domain equality;
  - `cargo clippy -p postfiat-types -p postfiat-ordering-fast -p postfiat-node
    --all-targets --locked -- -D warnings`: PASS.

**Disposition:** the persistence and activation foundations are implemented but
`P0-CONSENSUS-01` remains open. No production signer calls these v2 authorization
functions yet, no height router has activated them, and no fleet state was changed.

## 2026-07-16T13:37:24Z — consensus v2 production transport, replay, and signer-safe snapshot

- Wired the activated production block-vote transport to the durable v2 prepare,
  precommit, timeout, and QC store. A non-nil precommit QC is now carried in the
  block certificate/header and is required before activated execution mutates state.
- Added self-contained commit ancestry, versioned v2 block IDs, block-log replay,
  archive certificate reconstruction, and fail-closed pre-activation rejection.
  This removes dependence on a transient process QC cache during replay.
- Added signed timeout evidence to the existing production timeout artifact and
  enabled later-view proposals only when that exact v2 certificate verifies and
  authorizes the immediately following view.
- Snapshot v6 now hashes and restores the validator's v2 safety/QC artifacts.
  Snapshot v5 remains importable only for genesis configurations where v2 was
  never activated; it fails closed for an activated v2 signer because it lacks
  anti-equivocation continuity.
- Real-boundary regressions — PASS:
  `cargo test -p postfiat-node
  activated_consensus_v2_transport_survives_failed_view_zero_proposer_n4 --locked
  -- --nocapture` (20.08s) and the corresponding `_n6` test (33.98s).
  Real loopback validator services committed height 1/view 0, signed a timeout
  for a failed height 2/view 0 proposer, rotated deterministically, committed
  height 2/view 1 through prepare and precommit, converged on one tip/root, and
  replay-verified every node.
- Restart/snapshot regression — PASS:
  `cargo test -p postfiat-node
  consensus_v2_finality::tests::four_nodes_require_prepare_and_precommit_qcs_for_exact_block
  --locked -- --nocapture`. The recovered commit verifies from an empty QC cache;
  snapshot restore reproduces the exact durable safety state and verified QC graph.
- Guarding commands — PASS:
  - `cargo test -p postfiat-ordering-fast --locked`: 26/26;
  - timeout quorum and pre-activation proposal regressions: 2/2 selected;
  - signed snapshot roundtrip regression: 1/1 selected;
  - `cargo test -p postfiat-node transport_batch_payload --locked`: 8/8;
  - `cargo check -p postfiat-node --all-targets --locked`;
  - `cargo clippy -p postfiat-types -p postfiat-ordering-fast -p postfiat-node
    --all-targets --locked -- -D warnings`.

**Disposition:** the production v2 commit and later-view path is implemented and
green at real four- and six-node TCP boundaries. `P0-CONSENSUS-01` remains open
for the six-node delay/loss/reorder/partition/crash campaign, retained-history
byte-identical activation replay, and exact whitepaper update. No shared devnet
state was changed or reset in this step.

## 2026-07-16T14:18:00Z — consensus-v2 activation, fault model, and snapshot boundary closed locally

- Changed the real TCP recovery fixture to activation height 2. Every replica
  first commits and replay-verifies height 1 through the unchanged legacy path,
  then produces signed timeout evidence for failed height 2/view 0 and commits
  height 2/view 1 through v2 prepare and precommit QCs. PASS: n4 in 14.03s and
  n6 in 21.03s, identical tip/root on every node.
- Added the adversarial n4/n6 regression
  `adversarial_delay_loss_duplication_reorder_partition_byzantine_and_restart_are_safe_n4_n6`.
  It covers reordered and duplicate votes, under-quorum partitions, durable
  locks, delayed timeout certificates, a Byzantine conflicting reproposal, and
  crash/restart. PASS in 3.38s.
- Added the v5 migration fixture: a legacy snapshot restores only when consensus
  v2 was never activated; the same format fails closed for an activated signer.
  Also reproduced the pre-fix snapshot-overlay defect at the real import API
  (`chain tip chain_id does not match local genesis` after existing files had
  already been overwritten), then made import reject any existing destination
  before mutation. The RED-to-GREEN no-overlay regression and signed/v5
  snapshot regressions all pass.
- Reconfirmed the legacy genesis golden hash and optional-field JSON behavior;
  `scripts/test-whitepaper-implementation-boundaries` passes after updating the
  whitepaper, README, and finality architecture document to the exact implemented
  prepare/precommit and timeout-ancestry rule.

**Disposition:** every P0-CONSENSUS-01-specific implementation checkbox is green
locally. It remains a release-candidate item until the global workspace/CI gates,
deterministic reset rehearsal, and founder-authorized six-node activation gate
are complete. No fleet state was changed or reset here.

## 2026-07-16T15:03:00Z — signed governance restored and committee rotation crosses epochs

- Replaced the temporary live-governance removal with a versioned ML-DSA-65
  authorization envelope. Each validator signature binds the chain/genesis,
  complete amendment or registry-update payload, old registry root, committee
  epoch, exact proposal slot, expiry, validator identity and algorithm. The
  verifier requires distinct old-committee signatures at proposal, apply and
  archived replay boundaries; unsigned legacy artifacts remain replay-only.
- Added isolated-key signing and assembly commands for amendments, validator
  updates and the FastSwap governance bootstrap. Missing, duplicate,
  wrong-chain, wrong-epoch, wrong-registry, wrong-slot, expired, stale-key and
  altered-payload evidence rejects without governance mutation.
- The real TCP rotation test exposed two genuine activation bugs. Historical
  replay initially derived the activation block's proposal domain from the new
  committee even though the old committee certified that block. After fixing
  the strict activation boundary, the first post-rotation block then rejected
  prior-epoch QCs from the shared flat artifact store. Consensus v2 safety/QC
  artifacts are now committee-domain namespaced; prior epochs remain durable
  audit evidence and legacy flat artifacts are validated and migrated
  compatibly rather than deleted.
- Real-boundary evidence — PASS:
  - `cargo test -p postfiat-node --locked
    signed_governance_authorizations_from_isolated_validator_keys_enter_live_proposal`;
  - `cargo test -p postfiat-node --locked
    verify_blocks_replays_historical_registry_after_live_key_rotation`;
  - `cargo test -p postfiat-node --locked
    activated_consensus_v2_transport_survives_failed_view_zero_proposer_n4
    -- --nocapture`: PASS in 50.91s;
  - corresponding `_n6`: PASS in 90.79s;
  - consensus-v2 store tests: 2/2;
  - affected `cargo check --all-targets --locked` and strict Clippy: PASS.

**Additional governance gates:** the signed operation-kind matrix now applies
crypto policy, bridge witness epoch, authority mode, Orchard and atomic-swap
pause/unpause, bridge/atomic-swap/replicated-state activation heights, and an
explicit policy rollback, then replay-verifies all twelve blocks. The existing
FastSwap bootstrap test now uses isolated validator signatures and the normal
live apply path instead of the unsigned test fixture. That matrix exposed and
closed a real Cobalt allowlist omission for Orchard pause and replicated-state
activation. A same-slot competing signed amendment rejects after the chosen
amendment persists and the process re-enters from disk. PASS:
`cargo test -p postfiat-node --locked governance` (40 targeted library tests
plus the FastSwap service test) and `cargo test -p postfiat-consensus-cobalt
--locked` (66/66, including signed RBC/ABBA and trust-graph rollback).

**Disposition:** all P0-GOVERNANCE-01-specific implementation gates are green
locally. Immutable-candidate, full workspace/CI and deployment/activation drills
remain global gates. No shared fleet mutation or reset occurred.

## 2026-07-16T16:24:43Z — P0 bridge crash persistence and global supply accounting closed locally

- Added a real ordered-commit journal recovery regression that constructs a
  terminal BFT-checkpoint bridge transition and its replay/operation receipts,
  then restarts after each persist prefix: journal only, ledger, receipt,
  ordered batch, archive, block, and tip. Every recovery yields the exact same
  terminal state and receipts, removes the journal, and is idempotent on a
  second restart. PASS:
  `cargo test -p postfiat-node
  pftl_uniswap_terminal_state_and_receipt_recover_atomically_after_crash_prefixes
  --locked -- --nocapture` (1/1).
- Extended the global issued-supply oracle to include route-matched outstanding
  exports, pending return imports, Ethereum-spendable representation, and other
  registered venue inventory using checked arithmetic. The replicated-state
  root regression rejects public 60 plus external 40 under cap 99 and accepts
  exact cap 100. PASS:
  `CARGO_TARGET_DIR=/tmp/postfiat-p0-node-bridge-supply cargo test -p
  postfiat-node replicated_state_root_counts_external_bridge_inventory_in_global_supply
  --locked -- --nocapture` (1/1).
- Reworked the live execution fixture to use signed primary subscription and
  signed export operations instead of directly inserting a pending packet. It
  now proves certified Ethereum consume, certified return, and the alternate
  certified refund branch; exact global conservation; cap rejection without
  mutation; terminal replay rejection after serialization restart; and no
  cross-terminal double application. PASS:
  `CARGO_TARGET_DIR=/tmp/postfiat-p0-execution-return cargo test -p
  postfiat-execution
  pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances
  --locked -- --nocapture` (1/1), and
  `cargo test -p postfiat-execution --lib --locked` (139/139).
- Strict gates PASS:
  `cargo clippy -p postfiat-types -p postfiat-bridge -p postfiat-execution
  --all-targets --locked -- -D warnings` and
  `cargo clippy -p postfiat-node --all-targets --locked -- -D warnings`.
  Previously captured same-tree component evidence remains bridge 35/35, types
  83/83, and non-fork Foundry 95/95. The official mainnet-fork test remains
  intentionally fail-closed because `ETHEREUM_MAINNET_RPC_URL` is absent.

**Disposition:** the P0-BRIDGE persistence and global-supply checklist items are
now evidence-green locally. The P0 remains open for an integrated
reorg/partition/delayed-relayer matrix and a single orchestrated local
PFTL-to-Ethereum round trip using production checkpoint/certificate plumbing.
No shared fleet state, funds, keys, or deployment were touched.

## 2026-07-16T16:54:21Z — P0 bridge checkpoint production signer and adversarial matrix closed locally

- Added production checkpoint observation and isolated validator vote tooling.
  The observer binds the live governed route/committee to the execution client's
  chain ID, finalized block/hash/receipts root, and historical controller/token
  runtime-code hashes. The signer independently repeats those RPC checks before
  touching its key or durable state; a manually altered checkpoint file cannot
  induce a vote.
- Added persist-before-sign anti-equivocation state scoped by PFTL genesis,
  route, authority epoch, Ethereum height, and validator. Creation uses an
  fsynced no-overwrite link; exact replay returns the persisted verified vote,
  while conflicting roots reject across restart. The real regression races two
  matching-but-conflicting local Ethereum views in separate threads and proves
  exactly one vote/output wins. An RPC-mismatched candidate produces no vote and
  no durable intent.
- Reconciled the complete adversarial matrix across the real boundaries:
  checkpoint tests reject minority partitions and unvoted reorgs; receipt/event
  tests reject wrong chain/contract/code/topic/token/amount/recipient/nonce,
  malformed/noncanonical proofs, failed receipts, and bad log indexes; execution
  serializes both consume-before-refund and refund-before-delayed-consume orders
  into one terminal state and rejects replay after persistence; ordered-commit
  recovery covers every journal prefix.
- Evidence — PASS on the current tree:
  - `CARGO_TARGET_DIR=/tmp/postfiat-p0-ethereum-checkpoint-rpc cargo test -p postfiat-node --lib isolated_checkpoint_votes_require_live_route_and_assemble_exact_quorum --locked -- --nocapture` (1/1);
  - `CARGO_TARGET_DIR=/tmp/postfiat-p0-bridge-current cargo test -p postfiat-bridge --lib --locked` (36/36);
  - `CARGO_TARGET_DIR=/tmp/postfiat-p0-execution-return cargo test -p postfiat-execution --lib pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --locked -- --nocapture` (1/1; full crate remains 139/139 from the preceding same-tree component gate);
  - `CARGO_TARGET_DIR=/tmp/postfiat-p0-node-checkpoint-cli-clippy cargo clippy -p postfiat-node --all-targets --locked -- -D warnings`;
  - `CARGO_TARGET_DIR=/tmp/postfiat-p0-ethereum-checkpoint-rpc cargo check -p postfiat-node --all-targets --locked`;
  - `cargo fmt --all -- --check` and `git diff --check`.

**Disposition:** the P0-BRIDGE adversarial checklist item is evidence-green. The
P0 remains open for one orchestrated PFTL-node-to-local-Ethereum round trip that
uses the production checkpoint/certificate artifacts and proves accepted
deposit/consume/return plus the refund alternate with exact conservation. No
fleet state, funds, deployment, or shared network was changed.

## 2026-07-16T17:16:31Z — P0 bridge production-artifact round trip closed locally

- Added a bounded production Ethereum receipt-proof builder. It fetches the
  target receipt, canonical block and complete bounded receipt set; validates
  transaction/block/index consistency; canonically encodes legacy and typed
  EIP-2718 receipts; reconstructs the receipt Merkle-Patricia trie; requires the
  computed root to equal the block `receiptsRoot`; and atomically writes the
  exact inclusion proof. The implementation is 1,045 lines and does not add a
  dependency.
- Verified the trie implementation against an independently captured Anvil
  1.7.1 block containing three EIP-1559 receipts. The computed root byte-matches
  `25e6b7af647c519a27cc13276a1e6abc46154b51414d174b072698df1f6c19df`;
  a 260-receipt matrix covers the RLP index boundaries 0, 1, 15, 127, 128, 255,
  and 259 and proves every emitted path with the existing bridge verifier.
- Tightened the integrated bridge test so it no longer manufactures the proof
  passed into execution. Each consume, return and cancellation event is served
  through a bounded Ethereum JSON-RPC boundary, reconstructed by the production
  builder, observed at the governed block/code-hash boundary, independently
  revalidated and durably signed by three isolated validator keys, and assembled
  into the exact 3-of-4 certificate before execution.
- The resulting signed PFTL flow subscribes, exports 40 and 10 atoms, consumes
  the first export, imports a 17-atom return, and refunds the independently
  cancelled second export. Every money receipt asserts `accepted=true` and
  `code=accepted`; global and route totals remain exactly 50 atoms throughout;
  the consume and refund terminals are mutually exclusive.
- Exact-tree evidence at working-tree base
  `4b5af7bc6bb6e793ed8a60219d13d6d35be03058` — PASS:
  - `CARGO_TARGET_DIR=/tmp/postfiat-p0-bridge-final-tests cargo test -p postfiat-node --lib ethereum_checkpoint_signing::tests --locked -- --nocapture`: 2/2;
  - `CARGO_TARGET_DIR=/tmp/postfiat-p0-bridge-final-builder cargo test -p postfiat-node --lib ethereum_receipt_proof_builder::tests --locked -- --nocapture`: 3/3;
  - `CARGO_TARGET_DIR=/tmp/postfiat-p0-bridge-final-components cargo test -p postfiat-types -p postfiat-bridge -p postfiat-execution --lib --locked`: types 83/83, bridge 36/36, execution 139/139;
  - `CARGO_TARGET_DIR=/tmp/postfiat-p0-node-checkpoint-cli-clippy cargo clippy -p postfiat-node --all-targets --locked -- -D warnings`: PASS;
  - `CARGO_TARGET_DIR=/tmp/postfiat-p0-bridge-final-check cargo check -p postfiat-node --all-targets --locked`: PASS;
  - `forge test --no-match-contract PFTLUniswapOfficialForkTest --summary`: 95/95;
  - `cargo fmt --all -- --check` and `git diff --check`: PASS.

**Disposition:** every P0-BRIDGE-01 implementation checkbox is now green locally.
This does not claim public release closure: the immutable-candidate, secret-backed
pinned-fork, test-environment deployment and global gates remain unchecked. No
fleet state, funds, keys, contract deployment, or shared network was changed.

## 2026-07-16T17:25:41Z — P0 settlement-backed mint verifier implemented locally

- Added `ThresholdMintSettlementVerifier`, a concrete `IMintSettlementVerifier`
  for one immutable PFTL authority epoch and mint-controller/token pair. It
  requires the exact BFT quorum of sorted distinct low-`s` secp256k1 signatures
  and derives the settlement proof hash from the complete signed evidence.
- The signed domain covers EVM chain/verifier, PFTL chain/genesis/protocol/epoch,
  committee root, controller/token, pending and escrow IDs, recipient, amount,
  settled and locked values, finalized height/state root, accepted receipt hash,
  route-config digest, and exact `accepted` receipt code. Under-quorum,
  duplicate signer, rejected receipt, wrong amount, cross-verifier and
  cross-controller submissions cannot release the escrow.
- Strengthened `MintController` verifier governance. Initial installation pins
  an exact nonzero runtime code hash; every release rechecks `EXTCODEHASH`.
  Rotation requires that exact hash, a fixed two-day timelock, and zero unresolved
  mint escrows at both schedule and activation. A mint created during the delay
  blocks activation until it resolves; the one-time setter cannot bypass this path.
- Added a production trust-boundary document and compiled pinned-mainnet-fork
  test. The fork suite still fails closed when `ETHEREUM_MAINNET_RPC_URL` is
  absent; no offline run is represented as fork evidence.
- Evidence at working-tree base
  `4b5af7bc6bb6e793ed8a60219d13d6d35be03058` — PASS:
  - `forge test --match-contract 'MintControllerTest|ThresholdMintSettlementVerifierTest|MarketOpsAdversarialTest' -vv`: 25/25 before the added fuzz case;
  - `forge test --match-test testFuzzValidCertificateForWrongAmountCannotReleaseEscrow --fuzz-runs 256 -vv`: 256/256 generated cases;
  - `FOUNDRY_INVARIANT_RUNS=128 FOUNDRY_INVARIANT_DEPTH=64 forge test --match-contract ThresholdMintSettlementVerifierInvariantTest -vv`: 128 runs, 8,192 calls, PASS;
  - `forge test --no-match-path test/PFTLUniswapOfficialFork.t.sol --summary`: 102/102, including the default 256-run/128,000-call invariant campaign;
  - `env -u ETHEREUM_MAINNET_RPC_URL forge test --match-path test/PFTLUniswapOfficialFork.t.sol -vv`: expected RED, both tests fail on the exact missing secret rather than silently passing;
  - `forge build`: PASS.

**Disposition:** the P0-SUPPLY concrete verifier, field binding, drain-safe
timelock/code-hash rotation, and unit/fuzz/invariant/replay/replacement/fork-test
implementation boxes are green. Remaining P0-SUPPLY gates are isolated signer
tooling, the secret-backed pinned-fork execution, a controlled deployment, and
aggregate cross-chain conservation evidence. No funds, keys, deployment, fleet,
or shared network were touched.

## 2026-07-16T17:38:53Z — P0 owned-object asset/source binding closed locally

- Completed the live constructor inventory in
  `docs/status/OPEN-SOURCE-OWNED-OBJECT-CREATION-INVENTORY-20260716.md`.
  Production creates `LedgerState` owned objects only through conserving owned
  transfer, native-only owned unwrap change, native account wrap, or the signed
  consensus-ordered native deposit. Test fixtures and the isolated prototype are
  classified separately.
- Failing execution regressions reproduced four additional state-safety defects:
  duplicate wrap IDs debited and duplicated state; issued objects could unwrap
  into native PFT; legacy unwrap overflow consumed the input; and certified
  unwrap overflow retired inputs before failure. The fixes validate asset,
  collision, and destination arithmetic before any mutation. Zero outputs and
  content-addressed output collisions now also fail without mutation.
- Added a real node-store regression with a correctly owner- and validator-signed
  `pfUSDC` unwrap certificate. The certificate reaches the production apply
  boundary, returns `UnsupportedAsset`, and leaves the persisted ledger exact.
- Added a real concurrent boundary regression: eight simultaneous unsigned wrap
  attempts all return `PermissionDenied` and preserve the persisted ledger.
- Added `owned-object-asset-invariants` to the existing adversarial harness. A
  256-iteration run covered 2,816 wrong-label, unknown/issued lane, zero,
  overflow, collision, valid-wrap, replay, and transfer cases with zero failures.
- Removed the unbacked direct object insertion from the shipping FastPay demo;
  it now debits a native account through `wrap_to_owned`. The research prototype
  now rejects malformed/duplicate genesis fixtures, duplicate inputs and output
  IDs, overflow/zero outputs, and duplicate validator votes.
- Evidence at working-tree base
  `4b5af7bc6bb6e793ed8a60219d13d6d35be03058` — PASS:
  - `cargo test -p postfiat-execution`: 144/144;
  - `cargo test -p postfiat-node tests::fastpay_payment_safety:: -- --nocapture`: 11/11
    after the focused concurrent-wrap regression;
  - `cargo test -p postfiat-fastpay-prototype`: 9/9;
  - `cargo run -p postfiat-fastpay-prototype --bin fastpay-flow`: native-backed
    wrap, certified transfer, exact conservation and replay rejection PASS;
  - `cargo run -p postfiat-fuzz -- owned-object-asset-invariants --iterations 256`:
    2,816 cases, zero invariant failures;
  - `cargo clippy -p postfiat-execution -p postfiat-fastpay-prototype -p postfiat-fuzz -p postfiat-node --all-targets -- -D warnings`: PASS;
  - `cargo fmt --all -- --check`: PASS.

**Disposition:** the P0-ASSET constructor-inventory and adversarial-fuzz boxes
are green locally. The final immutable-candidate workspace/replay/migration
battery remains unchecked; no fleet, funds, keys, deployment, or shared network
state changed.

## 2026-07-16T18:00:15Z — P0 native custody and legacy-checkpoint recovery closed locally

- Completed the compile-enforced native supply inventory at
  `docs/status/OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md`. The production
  oracle destructures every `LedgerState` and `ShieldedState` field and counts
  accounts, open native escrows, offer reserves/native sells, exact-PFT owned
  objects, native FastLane reserves, and live Orchard turnstile value once.
  Duplicate account/escrow/offer/object/reserve keys, overflow, impossible
  Orchard totals, unreported destruction, and issuance fail closed.
- Added the `native-supply-invariants` adversarial target against the real node
  oracle. It covers valid mixed custody, maximum overflow, every duplicate key
  class, unknown issued-owned/FastLane exclusion, and Orchard underflow.
- Reproduced the remaining checkpoint gap: schema v1 correctly failed normal
  verification, but no production recovery operation could rebuild it from the
  retained archive. Added offline
  `history-checkpoint-rebuild-from-archive --backup-file ...`. It trusts only the
  v1 chain domain and boundary, verifies contiguous imported archive bundles,
  reconstructs v2 from genesis, verifies full prefix and retained suffix in
  isolated shadow stores, backs up the exact legacy bytes, and atomically
  replaces the checkpoint only after all checks pass.
- The real prune regression now corrupts an imported bundle and proves the
  rebuild rejects with the live checkpoint byte-identical and no backup written.
  It then restores the bundle, injects `u64::MAX` as malicious v1 faucet state,
  and proves archive replay discards that state, reconstructs the correct balance
  and cumulative burn, verifies the retained post-prune block, and leaves v2.
- The full snapshot round trip now compares the exact all-lane native live total
  before export and after restore and reruns `verify_blocks` on the restored node.
- Evidence at working-tree base
  `4b5af7bc6bb6e793ed8a60219d13d6d35be03058` — PASS:
  - pre-implementation compile regression: missing
    `history_checkpoint_rebuild_from_archive` and options type, as expected;
  - `cargo test -p postfiat-node native_supply_oracle_counts_each_live_custody_lane_once_and_checks_overflow -- --nocapture`: 1/1;
  - `cargo test -p postfiat-node history_prune_writes_checkpoint_and_allows_post_prune_block -- --nocapture`: 1/1;
  - `cargo test -p postfiat-node init_then_run_once -- --nocapture`: 1/1;
  - `cargo run -p postfiat-fuzz -- native-supply-invariants --iterations 256`:
    2,304 cases, zero invariant failures;
  - `cargo clippy -p postfiat-fuzz -p postfiat-node --all-targets -- -D warnings`: PASS;
  - `cargo fmt --all -- --check`: PASS.
- File-size policy remains green: `history.rs` 2,324 lines,
  `block_replay_wallet.rs` 2,239, fuzz harness 2,705, inventory 104.

**Disposition:** the P0-NATIVE implementation, custody inventory, adversarial
arithmetic/classification coverage, v1 refusal/recovery, prune, and snapshot
boxes are green locally. The immutable-candidate genesis-to-tip/pruned-history/
snapshot/post-restore battery remains unchecked. No fleet, funds, keys,
deployment, consensus threshold, or shared network state changed.

## 2026-07-16T18:27:18Z — P0 issued inventory completed; concurrent ordered-commit P0 fixed

### P0-ISSUED-SUPPLY-02 inventory and replay closure

- Added compile-exhaustive issued-custody inventories at the execution and
  node-plus-Orchard boundaries. Duplicate definitions/trustlines/escrows/
  offers/FastLane reserves/routes/Orchard rows, unknown asset references,
  issued owned objects, and checked-arithmetic overflow now fail closed.
- Extended the global oracle across transparent, escrow, offer, FastLane,
  PFTL-Uniswap external, and AssetOrchard custody.
- Upgraded vault-bridge reserve replay to schema v2 and included FastLane,
  external-route, and AssetOrchard rows. FastLane and Orchard bundle tampering
  now fails the real replay boundary.
- `cargo run -p postfiat-fuzz -- issued-supply-invariants --iterations 256` —
  PASS, 4,352 cases, zero invariant failures.
- Real mint/burn/clawback/property, escrow, offer, private ingress/egress,
  snapshot restore, block replay, and vault replay tests — PASS.

### P0-COMMIT-ATOMICITY-01 reproduction

- A barrier-synchronized real-store test submitted the same AssetOrchard
  disclosed-egress batch from eight threads.
- Before the fix, `cargo test -p postfiat-node
  asset_orchard_ingress_and_disclosed_egress_round_trip_issued_asset --
  --nocapture` — FAIL: all eight calls returned `accepted` for the same
  nullifier. The lock existed, but was acquired only when the journal was
  written, after each caller had read and executed against stale state.

### Remediation and evidence

- Transparent, shielded, bridge, and governance apply paths now acquire the
  cross-process ordered-commit lock before recovery and any consensus-state
  read, and hold it through execution and persistence.
- Locked recovery/writer helpers require a borrowed `StorageMutationLock`; the
  unlocked internal writers were removed.
- Post-fix race — PASS: exactly one accepted receipt and seven
  `AlreadyExists` idempotency failures; issued total stays 40, snapshot restore
  matches, and `verify_blocks` passes.
- `cargo test -p postfiat-storage
  ordered_commit_lock_serializes_independent_store_handles -- --nocapture` — PASS.
- Transparent snapshot/replay, governance ordered update, bridge-domain, and
  vault replay targeted tests — PASS.
- `cargo clippy -p postfiat-storage -p postfiat-execution -p postfiat-node -p
  postfiat-fuzz --all-targets -- -D warnings` — PASS.
- `cargo fmt --all -- --check` — PASS.
- Split the touched 5,494-line mixed test include and the three remaining
  over-limit Rust source files at type/function-family boundaries. A repo-wide
  `wc -l` inventory now has no Rust file above 5,000 lines; affected all-target
  compilation passes.

**Disposition:** both P0s are implemented locally. The issued complete
immutable-customer-flow gate and global immutable-candidate battery remain
open; neither is claimed complete. No fleet, funds, keys, deployment, or shared
network state changed.

## 2026-07-16T18:40:30Z — P0-STATE local activation and crash gates closed

- Reconciled the production commitment boundary: compile-exhaustive
  destructures cover `Genesis`, `GovernanceState`, `LedgerState`,
  `ShieldedState`, and `BridgeState`, and each top-level canonical encoder calls
  its inventory guard. The existing all-ten-field root regression passed.
- Expanded `docs/runbooks/replicated-state-v2-activation.md` with the exact
  legacy/candidate versus pre/post-activation matrix, mixed-version refusal,
  v6 snapshot migration, coordinated rollback-before-activation, and
  forward-only recovery after activation.
- Added a focused production-WAL regression for both the governed scheduling
  commit and the first v2 block. It simulates every ordered persistence prefix,
  recovers through the live status boundary, asserts exact ledger, governance,
  receipt, ordered-batch, archive, block, and tip state, proves the journal is
  removed, and reruns state plus block replay.
- Evidence — PASS:
  - `cargo test -p postfiat-node replicated_state_root_commits_every_fastlane_ledger_field -- --nocapture` — 1/1;
  - `cargo test -p postfiat-node replicated_state_v2_activation_journal_recovers_every_persist_prefix -- --nocapture` — 1/1, all 20 crash-prefix recoveries across the two commits;
  - `cargo clippy -p postfiat-node --all-targets -- -D warnings`;
  - `cargo fmt --all -- --check`.
- The new module is 361 lines. No Rust source exceeds 5,000 lines.

**Disposition:** the compiler inventory, compatibility/migration contract, and
activation persistence/crash boxes are green locally. Current-devnet shadow
replay and the isolated six-node rolling activation/rollback drill remain open.
No fleet, funds, keys, deployment, or shared network state changed.

## 2026-07-16T18:58:00Z — P0-STATE isolated six-node drill passed; live supply gate isolated

- Built the current release candidate with `cargo build --release -p
  postfiat-node`: PASS. SHA-256
  `15ed00371d48caa1cd30a10f3e7a3f6e3235a92d3805e171f14f57b463b92f91`.
  The preserved legacy binary SHA-256 is
  `428c4c7327212bd40d361c0dbb7f80bc2fd5d94f1cbccb85757913c0f04fc3c3`.
- Created isolated chain `postfiat-state-v2-six` with six validators and genesis
  `98b1983b2605bef49b2a90860096da96670deb21203805ec01627b262374d85da5d8fc39fb021abc019f350dfbc05167`.
  Rolling replacement of one node at a time preserved exact-six height-0 root
  `21c36a10…c7162` after every restart.
- Used the restored shipping governance sign/assemble routes to commit
  `replicated_state_v2_activation_height=2` at height 1. Six distinct votes,
  six accepted receipts, exact root `b26f311a…20f1d`, exact tip
  `b841c4f1…c509456`.
- Committed the first activation-height batch at height 2 with six distinct
  votes and six accepted receipts. Exact-six root `c1e0d7ea…4efb96fd`, exact
  tip `7d713dc8…d664dc`, empty mempools.
- Exported legacy snapshot v5 at height 0, imported it into six fresh
  directories, started the legacy binary, and reproduced the exact root.
  Exported scheduled snapshot v6 at height 1, imported it into six different
  fresh directories, applied the exact same height-2 batch/certificate, and
  reproduced the exact height-2 root and tip.
- Repeated `postfiat-node verify-state` and `postfiat-node verify-blocks` for all
  six activation stores, all six rollback stores, and all six forward-recovery
  stores: 36/36 invocations PASS. Public evidence and artifact hashes:
  `reports/open-source-p0-state-six-node-20260716T184400Z/README.md`.
- Captured a key-free raw copy of live `postfiat-wan-devnet-2` height 1220. The
  candidate reproduces root `8a534e5c…3986f` and tip `c8156a3c…a511d`, then
  both `verify-state` and `verify-blocks` fail closed on the complete issued
  supply invariant: 291,978,179 transparent + 9 FastLane + 798,070,376
  AssetOrchard = 1,090,048,564 pfUSDC against `max_supply=1,000,000,000`.
  Evidence:
  `reports/open-source-p0-state-shadow-20260716T184400Z/CURRENT-DEVNET-BLOCKER.md`.

**Disposition:** the isolated rolling, scheduled activation, exact-six replay,
rollback, and forward-recovery boxes are green. The current-devnet shadow box
remains correctly open; the final authorized reset/migration batch must create
cap-valid state before replay. The supply oracle was not weakened. No live
fleet write, deployment, reset, key use, or money movement occurred.

## 2026-07-16T19:29:00Z — P0-PROXY-AUTH authenticated TLS edge proven

### Reproduction and remediation

- Added a real-boundary regression for multiple authenticated principals,
  principal-scoped durable idempotency, body/rate/concurrency admission, and
  hostile WebSocket upgrade rejection. Before remediation, its first valid
  multi-principal request failed 401 because the proxy ignored the JSON token
  authority.
- Added mutually exclusive single-token, JSON-map and preferred secret-file
  token sources with constant-time token matching. Durable idempotency v2 binds
  principal, method/path, key and payload; existing v1 records migrate under
  the legacy default principal.
- Added bounded linear HTTP body reads plus principal rate and process-wide
  concurrency admission. Unknown mutations remain fail-closed.
- Added the pinned `docker-compose.wallet-public.yml` and Caddy production
  profile. Only TLS port 443 is published; the proxy stays internal. Both
  containers run non-root with read-only root filesystems and least
  capabilities. The proxy base image is digest-pinned. The TLS edge requires
  the operator-supplied key-owner UID/GID and does not receive a broad
  filesystem capability.
- First deployed-profile run caught a real 0600-key permission failure. The
  corrected edge runs as the key owner and stores its nonpersistent runtime
  state only in a bounded `/tmp` tmpfs.

### Evidence

- Real isolated HTTPS edge: static 200; no-bearer mutation 401; authenticated
  same-origin request reached the disabled money route as 409 with principal
  `demo`; foreign origin 403; over-16-MiB request 413.
- Real WSS edge: exact allowed origin opened; hostile origin received 403
  before upgrade.
- Real deployed admission: requests 1/2 reached the route and request 3 returned
  `proxy_mutation_rate_limited`; while a partial request held the sole slot, a
  second returned `proxy_mutation_concurrency_limited`.
- Edge/proxy log scan: test bearer absent and Authorization field deleted.
- `node --check server.js` and `node --check navswap-persistence-http.js` — PASS.
- `node test_authenticated_edge_profile.js` — PASS.
- `npm test` in `wallet-proxy` — PASS, 23/23; `npm audit --omit=dev` — zero.
- `npm test` in `wallet-web` — PASS, 222/222; production build PASS; `npm audit`
  — zero.
- Compose render and live non-root `caddy validate` — PASS.
- `git diff --check` — PASS; maximum Rust source remains 4,978 lines.
- Sealed non-secret evidence and exact hashes:
  `reports/open-source-p0-proxy-auth-edge-20260716T192900Z/README.md`.

**Disposition:** the authenticated TLS profile and deployed-edge control boxes
are green locally. The immutable-candidate exhaustive route rerun remains open
and is not claimed by this entry. No validator, fleet, key, fund, or chain state
was touched.

## 2026-07-16T21:27:45Z — governed vault route, rotation, and conservation boundary

### Reproduction and remediation

- The prior wallet bridge fix only removed the retired default and required a
  local address/code-hash configuration. That configuration was still the money
  destination authority and supplied no authenticated route rotation semantics.
- Added versioned `VaultBridgeRouteProfileV1`, signed-governance activation,
  deterministic active selection, canonical state-root commitment, and
  `vault_bridge_route(asset_id)` discovery of the complete authenticated
  profile. New ingress requires the current profile; existing deposits,
  receipts, buckets, redemptions, and NAV allocations resolve their immutable
  historical policy.
- Added route-bound `depositV2`; the legacy/unbound Solidity deposit fails
  before token mutation. Wallet and relay consume chain state and cannot
  override vault, token, runtime hashes, epoch, binding, verifier, or evidence
  tier. The UI displays the exact tier and its concrete trust dependency.
- Added the source-backed `vault-bridge-conservation-audit` CLI. It queries the
  exact governed source chain/contracts, verifies runtime code, checks source
  deposit and claimed-withdrawal mappings, aggregates old and current vaults,
  and proves `V = S + D + B - R`. Any unexplained atom is an error.
- Adversarial boundary coverage uses two governed route epochs with source
  balances split 80/15. It proves old deposits/redemptions finish after
  rotation, then rejects missing historical policy, stale new ingress, wrong
  source network, runtime drift, source-absent deposit, PFTL settlement absent
  at source, proxy/wallet route substitution, false-tier downgrade, and a
  one-atom conservation mismatch without mutation.

### Evidence

- `cargo fmt --all -- --check` — PASS.
- `cargo check --workspace --all-targets --locked` — PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS after
  correcting two `needless_borrow` findings introduced by the route refactor.
- `cargo test -p postfiat-execution --all-targets` — PASS `146/146`.
- `cargo test -p postfiat-node vault_bridge --lib -- --nocapture` — PASS `7/7`.
- `cd wallet-web && npm test` — PASS `223/223`.
- `cd wallet-web && npm run build` — PASS; static production bundle emitted.
- `cd wallet-web && npm audit --audit-level=low` — PASS; zero vulnerabilities.
- `cd wallet-proxy && npm test` — PASS `23/23`.
- `cd crates/ethereum-contracts && forge test
  --match-contract ERC20BridgeVaultTest -vv` — PASS `13/13`.
- An initial attempt to filter Node wallet tests put `--test-name-pattern` after
  the npm file glob and was rejected as a nonexistent path; the complete
  correctly invoked `npm test` suite above is green.
- Source limits after the split: `nav_vault_asset_execution.rs` 4,946 lines;
  `vault_bridge_profile_resolution.rs` 129; `vault_bridge_conservation.rs`
  below 1,300; `execution_actions.rs` below 3,100.

### Disposition

`P0-WALLET-BRIDGE-DEST-01` advances from configuration-only containment to a
production-shaped governed authority with rotation and exact conservation.
Three boxes remain open: full snapshot/pre-activation/rollback evidence for the
new route state, one controlled governed deposit+withdrawal, and a stronger
verifier-profile promotion through the unchanged wallet/API contract. No fleet,
deployment, key, signer, or money operation occurred.

## 2026-07-16T21:52:58Z — verifier-neutral governed bridge promotion

### Reproduction and remediation

- The first receipt-proof promotion test exposed a real type mismatch: the
  governed route hash is SHA3-384 (48 bytes), while the SP1 valuation-policy
  public input is 32 bytes. Reusing one field for both made an SP1 governed
  route impossible without weakening one of the commitments.
- Split those authorities explicitly. `NavProofProfile` and its signed
  registration operation now bind an optional 48-byte
  `vault_bridge_route_policy_hash` separately from the verifier-specific
  valuation policy. Empty legacy profiles preserve their serialized form and
  profile ID.
- Extended `VaultBridgeRouteProfileV1` to commit the proof policy, program
  verifying key, encoding, and proof/public-values bounds. Route activation
  exact-matches that entire contract against the registered NAV profile; a
  mismatched program key rejects before governance state mutation.
- Committed the new route and NAV-profile fields to replicated state. The same
  route discovery API now returns either the observer-quorum or SP1 contract,
  with unchanged ledger/accounting semantics.
- A browser adversarial test then exposed that the client trusted the reported
  profile digest rather than hashing every field. The wallet now implements the
  Rust-equivalent SHA3-384 domain hash over the complete canonical profile and
  rejects vault, timing, tier, proof-policy, vkey, encoding, threshold, and
  route-binding substitution.

### Evidence

- `cargo check -p postfiat-types -p postfiat-execution -p postfiat-node
  --all-targets` — PASS.
- `cargo test -p postfiat-node vault_bridge_governed_route --lib -- --nocapture`
  — PASS `5/5`, including verifier promotion and exact-contract mismatch.
- `cargo test -p postfiat-node vault_bridge --lib -- --nocapture` — PASS `8/8`.
- `cargo test -p postfiat-execution --all-targets` — PASS `146/146`.
- `cargo test -p postfiat-execution
  vault_bridge_sp1_bridge_deposit_requires_source_proof_commitments --lib` —
  PASS.
- `cargo test -p postfiat-types --all-targets` — PASS `87/87` after the
  route-policy registration regression was added.
- `cd wallet-web && npm test` — PASS `226/226`; production build PASS.
- `cd wallet-proxy && npm test` — PASS `23/23`.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS.
- `git diff --check` — PASS. All touched Rust sources remain below 5,000 lines;
  the largest remains `nav_vault_asset_execution.rs` at 4,946.

### Disposition

The verifier-promotion box is green without changing the wallet API, bridge
money semantics, consensus threshold, or accounting. Two bridge-destination
boxes remain open: the route-state replay/snapshot/rollback matrix and one
controlled governed deposit plus withdrawal. No fleet, deployment, key,
signer, fund, or chain state was touched.

## 2026-07-16T21:59:10Z — governed route state replay and rollback matrix

### Reproduction and remediation

- Added a real node-store matrix rather than a serializer-only assertion. A
  signed asset/profile setup commits at height 1, signed governance activates
  route authority at height 2, and a signed, certified route profile commits at
  height 3.
- Verified the complete pre-route history, exported its snapshot, and proved no
  route can be discovered before activation. Verified the post-route history,
  exported its snapshot, and proved the exact governed profile is active.
- Restored the pre-route snapshot into a keyless validator directory and
  reproduced the exact height-2 root and tip. Reapplied the original certified
  height-3 governance batch without validator private keys and reproduced the
  exact post-route root and tip.
- Independently restored the post-route snapshot, rediscovered every route
  field through the public API, and replay-verified the complete block history.
  This covers pre-activation compatibility, snapshot migration, operational
  rollback, and deterministic forward reapplication at the real boundary.

### Evidence

- `cargo test -p postfiat-node
  governed_route_state_replays_snapshots_rolls_back_and_reapplies_byte_identically
  --lib -- --nocapture` — PASS.
- `cargo test -p postfiat-node vault_bridge_governed_route --lib -- --nocapture`
  — PASS `6/6`; combined node bridge coverage is `9/9` with conservation tests.
- `cargo clippy -p postfiat-node --all-targets -- -D warnings` — PASS.
- `cargo fmt --all -- --check` and `git diff --check` — PASS.
- New test module remains 1,135 lines; every Rust source remains below the
  5,000-line limit.

### Disposition

The route-state replay/snapshot/rollback box is green. The governed bridge
destination P0 is now `14/15`; its only remaining item is a controlled real
deposit and withdrawal with accepted receipt codes, pinned-route evidence, and
full lifecycle conservation. No fleet, deployed validator, external RPC,
signer, fund, or chain state was touched.

## 2026-07-16T22:15:59Z — real governed bridge deposit and withdrawal

### Boundary and result

- Added a rerunnable ignored Rust integration gate which starts an isolated
  loopback Anvil, deploys the production `PFTLWithdrawalVerifier` and
  `ERC20BridgeVault` plus a test ERC20, and derives the governed route from the
  actual chain ID, deployed addresses and runtime-code hashes.
- The source user executes `depositV2` with the exact route-profile/epoch
  binding. The production receipt-RPC relay reads the real successful receipt
  at confirmation depth 2 and constructs the PFTL operations from its event.
- Production PFTL execution then accepts route activation, observer
  registration, deposit propose/attest/finalize/claim, reserve
  submit/attest/finalize, burn-to-redeem and observed redemption settlement.
  Every one of the 11 receipts is `accepted=true, code=accepted`.
- The production threshold withdrawal verifier accepts and finalizes the signed
  PFTL packet; the production vault accepts, finalizes and claims it. Mint,
  approval, deposit, proof submit/finalize and withdrawal
  submit/finalize/claim are all real Anvil receipts with status `0x1`.
- Source-backed conservation passes at four distinct boundaries: after claim
  (`V=1,000,000; S=1,000,000`), after burn (`V=1,000,000; B=1,000,000`), after
  source release (`V=0; B=R=1,000,000`), and after PFTL settlement (all terms
  zero). The source vault ends empty and the original 1,000,000 token atoms are
  returned exactly.

### Evidence

- Initial fixture runs failed before the first money transition because test
  fees 22 and 25 did not cover the real per-operation minimum; the fixture now
  uses a bounded 100-atom test fee. No safety check or fee threshold changed.
- `POSTFIAT_BRIDGE_ROUNDTRIP_REPORT_DIR=... cargo test -p postfiat-node
  governed_route_real_anvil_deposit_withdrawal_roundtrip --lib -- --ignored
  --nocapture` — PASS `1/1` on the final tree.
- `cargo test -p postfiat-node vault_bridge_governed_route --lib -- --nocapture`
  — PASS `6/6`, with the external gate intentionally ignored in the ordinary
  suite; `cargo test -p postfiat-node vault_bridge_conservation --lib
  -- --nocapture` — PASS `3/3`.
- `cd crates/ethereum-contracts && forge test
  --offline --match-path test/ERC20BridgeVault.t.sol -vv` — PASS `13/13`.
- `cargo clippy -p postfiat-node --all-targets --locked -- -D warnings`,
  `cargo fmt --all -- --check`, and `git diff --check` — PASS.
- Final evidence:
  `reports/open-source-p0-governed-bridge-roundtrip-20260716T221559Z/ACCEPTANCE.json`
  SHA-256
  `02f1832ae42bb6d496676c6e791860062bc2cec9771cfd9476daef665c4a32cf`;
  test log SHA-256
  `9ba9a68f0a7effc0524a951ca6e485cf1ffb496adf5fc88b8879dd7d2740978f`.
  Exact-value scan confirms the public Anvil test key is absent from both
  artifacts; `private_key_material_recorded=false`.
- The enlarged governed-route test module is 2,192 lines; every Rust source
  remains below the 5,000-line limit.

### Disposition

`P0-WALLET-BRIDGE-DEST-01` is `15/15` on its finding-specific burn-down. The
remaining publication work is the shared immutable-candidate/global battery,
not a hidden bridge capability or deferred bridge safety fix. No shared fleet,
external source chain, real asset, validator configuration, or deployed state
was touched.

## 2026-07-16T22:26:14Z — browser self-custody boundary capture

### Boundary and result

- Added a shared browser-runtime custody guard at every PFTL WebSocket RPC,
  swap/private-flow HTTP, bridge-relay and FastSwap-demo egress. It fails closed
  on recursively named private fields, private fields inside JSON strings, and
  the active seed/backup value even if placed under an innocuous key. It allows
  public keys, signatures, proofs and signed envelopes.
- The encrypted vault registers only its current in-memory seed/backup with the
  guard and clears the registry whenever sensitive memory is locked or cleared.
- Added a real headless-Chromium capture which exercises 10 WebSocket mutation
  classes, 10 HTTP money routes and two MetaMask `eth_sendTransaction` classes.
  It proves the random active seed and backup are absent from all captured
  proxy ingress bodies while a public signature marker crosses successfully.
- The same run scans local/session storage, browser console output, the Node and
  Chromium subprocess argument vectors, the persistent Chromium profile and
  crash-artifact names/content. All secret-hit counts and crash counts are zero.

### Evidence

- Pre-fix regression: `node --test src/lib/custody-boundary.test.js` failed with
  `ERR_MODULE_NOT_FOUND` because no general runtime custody boundary existed.
- `POSTFIAT_CUSTODY_REPORT_DIR=../reports/open-source-p0-browser-custody-20260716T225000Z
  npm run test:custody-browser` — PASS `1/1`.
- Evidence artifact:
  `reports/open-source-p0-browser-custody-20260716T225000Z/ACCEPTANCE.json`,
  SHA-256
  `538d08989bd5f5f8584a5ee1f021e54dc8a4eaca7801d51adbf7fbb08c203ac0`.
- `cd wallet-web && npm test` — PASS `232/232`.
- `cd wallet-web && npm run build` — PASS (Vite production bundle).
- `cd wallet-proxy && node run_tests.js` — PASS `23/23`.
- Targeted `git diff --check` — PASS.

### Disposition

The two finding-specific browser-capture and artifact-scan boxes for
`P0-CUSTODY-01` are green. Its only remaining box is the shared immutable-
candidate rerun. No chain, fleet, validator, signer, wallet funds or external
service was touched.

## 2026-07-16T22:39:16Z — historical replay state/parent binding

### Boundary and result

- Added a real catch-up regression starting from the retained height-2 seed.
  Before the fix, changing one account balance by one atom and then applying the
  valid archived height-3 batch/block/certificate succeeded, wrote an accepted
  receipt, and committed the archived header root over divergent local state.
- Historical apply had verified the quorum certificate against the archived
  root but had not compared that root with the state actually produced. It also
  accepted the archived parent from the replay file without first matching the
  current tip.
- Catch-up now requires exact current-parent attachment, exact recomputed
  receipt IDs, and a post-execution state root matching the archived header.
  Retained pre-upgrade history remains supported only through the already
  defined chain/height/batch-gated legacy root computations; arbitrary legacy
  compatibility is not added.
- A public `apply_shielded_batch` regression serializes a correctly identified
  legacy cleartext mint and proves it yields only a rejected receipt while
  shielded state remains byte-identical. Archive compatibility therefore does
  not provide a live legacy-injection surface.

### Evidence

- Pre-fix command: `cargo test -p postfiat-node
  historical_external_certificate_rejects_state_divergent_catch_up_without_mutation
  --lib -- --nocapture` — FAIL because the replay returned an accepted receipt
  instead of rejecting.
- Post-fix: `cargo test -p postfiat-node historical_external_certificate_ --lib
  -- --nocapture` — PASS `3/3`: retained catch-up succeeds; one-atom divergent
  state and a wrong local parent both reject with ledger/tip/block no-mutation.
- `cargo test -p postfiat-node
  legacy_cleartext_shielded_actions_are_historical_replay_only --lib --
  --nocapture` — PASS `1/1`, including the public file-based apply boundary.
- `cargo check -p postfiat-node --all-targets --locked` and `cargo clippy -p
  postfiat-node --all-targets --locked -- -D warnings` — PASS.
- `cargo fmt --all -- --check` and targeted `git diff --check` — PASS.
- Touched Rust files remain below 5,000 lines (`storage_commit.rs` 2,807;
  `snapshot_deployment.rs` 2,595; privacy helper 4,342).

### Disposition

The historical-replay injection box under `P0-PRIVACY-01` is green. At this
point the remaining privacy work was the exhaustive public-surface version
inventory, full deposit/transfer/swap/egress leakage capture, adversarial
encrypted-v2 tests, and immutable-candidate reruns. No live fleet, deployed
chain, signer, key, or fund was touched.

## 2026-07-16T22:44:00Z — close the remaining live legacy privacy creator

### Boundary and result

- Repo-wide public-surface inventory found that shipping CLI and RPC
  `shield_mint` wrappers still reached a direct-state function which created and
  persisted a complete legacy cleartext note. This contradicted the prior audit
  statement that direct legacy mint and spend were both disabled.
- A new regression calls the real public function after initialization. Before
  the fix it failed because `shield_mint` returned a complete `ShieldedNote`
  instead of `PermissionDenied` and had already mutated shielded state.
- `shield_mint` now rejects before store access, matching direct spend and the
  legacy mint/spend batch builders. The secure capability remains live through
  Asset-Orchard v2; the existing historical migration test now seeds its legacy
  input explicitly inside test code and still proves migration into Orchard.
- Production call-site inventory finds no legacy action constructor in wallet,
  proxy, CLI or RPC code. Manually serialized variants remain present only in
  rejection, authenticated replay and archive lookup code. Wallet and proxy
  ingress schemas are exactly v2.

### Evidence

- Pre-fix `cargo test -p postfiat-node
  legacy_cleartext_shielded_actions_are_historical_replay_only --lib --
  --nocapture` — FAIL; direct `shield_mint` returned a complete cleartext note.
- Post-fix same command — PASS `1/1`, covering direct mint, direct spend, both
  batch builders, proposal admission, public file apply and authenticated replay.
- `cargo test -p postfiat-node
  orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers --lib
  -- --nocapture` exercises the preserved historical migration and live Orchard
  path.
- `cd wallet-web && node --test src/lib/shielded-navswap.test.js
  src/lib/swap-server.test.js` — PASS `34/34`; `cd wallet-proxy && node
  test_navswap_adapter.js` — PASS.
- `cargo check -p postfiat-node --all-targets --locked`, strict node Clippy and
  formatting — PASS.

### Disposition

The public privacy-mutation version box under `P0-PRIVACY-01` is green. This is
removal of an insecure legacy creator, not removal of private payments:
Asset-Orchard v2 ingress, transfer, swap, migration and egress remain supported
and tested. No fleet or money state was touched.

## 2026-07-16T22:48:00Z — encrypted ingress v2 adversarial matrix

### Boundary and result

- Extended the real issued-asset ingress/egress round trip with an accepted-v2
  replay attempt and a mixed v1/v2 batch. Replay fails `AlreadyExists`; the
  mixed batch fails live admission because a valid v2 action cannot mask a
  legacy cleartext v1 action.
- Existing cryptographic boundary tests prove the intended recipient recovers
  the note while a wrong recipient and a one-bit ciphertext mutation recover
  nothing. Malformed-length/magic envelopes and caller-provided plaintext
  ciphertext labels fail closed.
- The existing live-v1 execution regression remains no-mutation, while
  authenticated historical replay remains separately constrained by exact
  certificate, parent, root and receipt checks.

### Evidence

- `cargo test -p postfiat-node
  asset_orchard_ingress_and_disclosed_egress_round_trip_issued_asset --lib --
  --nocapture` — PASS `1/1` with mixed-version and replay assertions.
- `cargo test -p postfiat-privacy-orchard
  recipient_recovers_note_from_chain_ciphertext_without_note_file --lib --
  --nocapture` — PASS `1/1`.
- `cargo test -p postfiat-privacy-orchard
  output_validation_rejects_wrong_ciphertext_lengths --lib -- --nocapture` —
  PASS `1/1`.

### Disposition

The wrong-recipient/malformed/replay/downgrade/mixed-version box under
`P0-PRIVACY-02` is green. The remaining v2 P0 gates are the real complete-flow
wire/log/receipt/ledger privacy capture and the two-fresh-wallet immutable-
candidate proof.

## 2026-07-16T22:55:00Z — production wallet browser/runtime boundary

### Reproduction and remediation

- Extended the shipping static-server regression with a real un-hashed asset,
  source map, Vite client, source module and dotfile present on disk. Before the
  fix, `node test_wallet_static_security.js` failed because the un-hashed
  `assets/wallet.js` received `public, max-age=31536000, immutable` instead of
  `no-store`. Source/development files were also servable when present, and the
  final HTTP handler returned `200` for unknown paths.
- Production serving now rejects dotfiles, source maps, Vite, `@fs`, `src` and
  `node_modules` paths, confines canonical paths to the static root (including
  a symlink-escape regression), grants immutable caching only to content-hashed
  build artifacts, provides `/healthz`, and returns `404` elsewhere.
- The wallet Docker image still starts only `node server.js`, has no Vite
  dependency, and both supported Compose profiles mount only the built `dist`
  tree read-only. No wallet capability, mutation API, or signer path was
  removed.

### Real-browser and guarding evidence

- `cd wallet-web && npm run build` — PASS; production assets contain no source
  maps, Vite client, React refresh runtime, or source-map trailers.
- `P0_WALLET_BROWSER_EVIDENCE_DIR=../reports/open-source-p0-wallet-browser-20260716T225500Z
  npm run test:public-browser` — PASS `1/1` in headless Chromium against the
  actual proxy and production bundle. It proves CSP inline/base/connect/frame
  behavior, origin/auth mutation rejection, HTML and hashed-asset cache policy,
  and `404` for Vite/source/map disclosure paths.
- Acceptance SHA-256:
  `465041cdf6e0cedd81f36da2ccad72de74159559c8fe0125811325bf8684e5ea`.
- `cd wallet-web && npm test` — PASS `232/232`; `npm audit` — zero
  vulnerabilities.
- `cd wallet-proxy && npm test` — PASS `23/23`; `npm audit --omit=dev` — zero
  vulnerabilities.
- All touched JavaScript files remain below 5,000 lines (`server.js` 1,205;
  browser regression 274; static regression 124).

### Disposition

The two actionable runtime/browser boxes under `P0-WALLET-02` are green. The
only remaining box is the exact immutable-candidate build/test/audit rerun.
No fleet, chain state, signer, key, or money was touched.

## 2026-07-16T23:50:19Z — complete Asset-Orchard privacy flow and public-artifact boundary

### Real boundary and result

- Extended the real issued-asset ordered-store regression through two encrypted
  ingress-v2 actions, a K15 Asset-Orchard atomic swap, chain-only ciphertext
  recovery and private egress. Every applied money receipt must be
  `accepted=true, code=accepted`; both swap inputs and the egress input must be
  nullified; the egress public balance must increase by the exact note value;
  and global issued supply must remain unchanged.
- The same regression scans 13 public artifacts: both ingress envelopes and
  batches, the swap action and batch, the egress envelope and batch, the batch
  archive, block log, receipt log, ledger and shielded state. It rejects either
  serialized private field names or any exact note-opening/spend-authority value.
- The generic Orchard transfer regression now requires the real ordered-batch
  receipt and shielded-finality linkage, then scans its action, batch,
  withdrawal action/batch, archive, block and receipt representations while
  preserving duplicate-nullifier rejection.
- Historical deployed v1 artifacts contain complete note openings and remain
  reproduction evidence. They were not allowlisted or cited as clean v2 proof.

### Oversized serialized-action reproduction and remediation

- The recursive wallet/proxy JSON scanners parsed serialized strings only up to
  1 MiB even though the HTTP transport permits a larger body. A private action
  could therefore be hidden in an oversized JSON-looking string and reach a
  later parser instead of failing at the custody/privacy boundary.
- Before remediation, the wallet regression failed with `Missing expected
  exception`, and the proxy returned the later
  `shielded_swap_action_cleartext_rejected` classification instead of
  `shielded_navswap_private_material_rejected`.
- JSON-looking serialized fields above the bounded recursive inspection budget
  now fail closed before browser transport or proxy custody dispatch. Ordinary
  non-JSON opaque proof/ciphertext strings remain governed by their existing
  typed size and schema checks.

### Commands and evidence

- `cargo test -p postfiat-node --lib
  tests::wan_devnet_invalid_asset_orchard_swap_proof_is_rejected_and_valid_swap_still_applies
  -- --exact --nocapture` — PASS `1/1`, `0` failed, `0` ignored, 205
  filtered, 2,524.27 seconds. A redundant release-mode duplicate was stopped
  only after this exact run returned green; it is not counted as evidence.
- `cargo test -p postfiat-node --lib
  tests::orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers
  -- --exact --nocapture` — PASS `1/1` in 372.91 seconds.
- `cargo test -p postfiat-privacy-orchard --lib` — PASS `83`, fail `0`,
  with 17 explicit full-shape/release-scale ignores retained as immutable-
  candidate gates.
- `cargo check --workspace --all-targets --locked`, `cargo clippy --workspace
  --all-targets --locked -- -D warnings`, `cargo fmt --all -- --check`, and
  `git diff --check` — PASS.
- `cd wallet-web && npm test` — PASS `232/232`; production build PASS; npm
  audit reports zero vulnerabilities. `cd wallet-proxy && npm test` — PASS
  `23/23`; npm audit reports zero vulnerabilities.
- Headless-Chromium HTTP/WebSocket privacy boundary — PASS `1/1`; artifact
  `reports/open-source-p0-privacy-browser-20260716T231154Z/ACCEPTANCE.json`,
  SHA-256
  `e07bacd757b646022a13bdf28a9730a30a158920141eaf73677ee92f1597df01`.
- Aggregate complete-flow artifact:
  `reports/open-source-p0-privacy-complete-flow-20260716T235019Z/ACCEPTANCE.json`,
  SHA-256
  `b830ce023d078aeed4acc832679591072519827f60af72d778b144dc5d5672ec`.
- All modified Rust/JavaScript/documentation source files remain below 5,000
  lines; the largest modified privacy helper is 4,464 lines.

### Disposition

The finding-specific end-to-end boxes for `P0-PRIVACY-01` and
`P0-PRIVACY-02` are green. Their remaining boxes are deliberately limited to
the frozen immutable-candidate proving/replay and two-fresh-wallet reruns. No
shared fleet, chain state, signer, key, external service or money was touched.

## 2026-07-16T23:59:00Z — production mint-settlement verifier deployed on isolated Anvil

### Boundary and result

- Added a narrow test-environment deployment orchestrator around the production
  `MintController` and production `ThresholdMintSettlementVerifier`. The token
  and accepted-envelope source are explicitly isolated fixtures; the settlement
  verifier is not a mock and its deployed runtime code hash is captured.
- An isolated PFTL node commits a real signed 110-atom backing transfer. Its
  receipt is `accepted=true, code=accepted`; the exact backing account delta,
  height-1 state root and SHA-384 receipt commitment are independently checked
  before any EVM certificate is built.
- Isolated Anvil deploys the controller/verifier stack. Four deterministic
  public test signers are sorted by recovered address; exactly three independently
  sign the production verifier digest binding the PFTL domain, height/root/
  receipt, route digest, controller/token, pending/escrow IDs, recipient, amount
  and accepted receipt code.
- An uncertified release fails before mutation. The certified escrow releases
  once, after which certified backing, released supply and beneficiary balance
  are each 110 atoms; controller escrow and unresolved obligations are zero.
  Certificate replay and release replay both fail.

### Commands and evidence

- `forge build --offline` — PASS with the controlled
  deployment fixture and production contracts compiled by Solc 0.8.24.
- `cargo test -p postfiat-node --lib
  mint_settlement_real_anvil_release_matches_accepted_pftl_backing --no-run` —
  PASS.
- `POSTFIAT_MINT_SETTLEMENT_REPORT_DIR=... cargo test -p postfiat-node --lib
  mint_settlement_real_anvil_release_matches_accepted_pftl_backing -- --ignored
  --nocapture` — PASS `1/1` in 1.91 seconds.
- Acceptance artifact:
  `reports/open-source-p0-mint-settlement-anvil-20260716T235900Z/ACCEPTANCE.json`,
  SHA-256
  `b9cf666416126a81c3ecf18bd9686e485e84c7db055fce48c834f7a0311f66fa`.
- The artifact contains no signer private keys. It records the exact PFTL
  receipt/root, deployment and transaction hashes, controller/verifier runtime
  hashes, certificate/proof bindings and terminal conservation values.
- `git diff --check` — PASS. The Rust test module is 2,533 lines and the
  Solidity fixture is 241 lines, both below the 5,000-line ceiling.

### Disposition

The `P0-SUPPLY-01` controlled test-environment deployment checkbox is green.
The stronger aggregate PFTL-plus-Ethereum mint/return/failure checkbox remains
open until one continuous cross-system oracle covers all three paths; the
separate governed-vault return proof is not being used as indirect closure.
No shared fleet, external chain, production signer, real key or money was used.

## 2026-07-17T00:08:00Z — continuous PFTL/Ethereum mint-return-failure conservation

### Boundary and result

- Strengthened the existing real governed-route Anvil round trip with a failure
  checkpoint on the same asset and state. After source deposit finalization but
  before PFTL claim, a correctly signed claim with an amount one atom above the
  finalized vault evidence executes through the production state-transition
  boundary and rejects with `vault_bridge_deposit_amount_mismatch`.
- The failed path leaves the issued-asset trustline and all bridge deposit/bucket
  state unchanged. The live source-vault/PFTL oracle still reports
  `V=1,000,000`, issued supply `0`, uncredited deposit `1,000,000` and unexplained
  delta `0`.
- The same run then completes the valid claim/mint, return burn, source release
  and final settlement. The exact `V = S + D + B - R` equation passes at five
  checkpoints: failed claim, valid claim, burn, source release and terminal
  settlement. Every unexplained delta is zero.
- All 11 PFTL money receipts are `accepted=true, code=accepted`; the deliberate
  failure is separately recorded as rejected. All eight EVM transaction receipts
  have status `0x1`; terminal source-vault balance and every PFTL pending bucket
  are zero.

### Commands and evidence

- `cargo test -p postfiat-node --lib
  governed_route_real_anvil_deposit_withdrawal_roundtrip --no-run` — PASS.
- `POSTFIAT_BRIDGE_ROUNDTRIP_REPORT_DIR=... cargo test -p postfiat-node --lib
  governed_route_real_anvil_deposit_withdrawal_roundtrip -- --ignored
  --nocapture` — PASS `1/1` in 4.62 seconds.
- Evidence:
  `reports/open-source-p0-governed-bridge-aggregate-20260717T000800Z/ACCEPTANCE.json`,
  SHA-256
  `0a18b1a9ab808fe74c2f70df0c4de1e2866a70990758af0a94b41096e400f8e9`.
- The report contains the rejected receipt and all five complete conservation
  structures, 11 accepted PFTL receipts, eight successful EVM receipts,
  contract/deployment hashes and terminal balances. It contains no Anvil test
  private key.

### Disposition

The second `P0-SUPPLY-01` finding-specific box is green. Combined with the
separate production `MintController`/threshold-verifier deployment proof, this
makes the finding `9/9` locally. Immutable-candidate, production signer-custody
and pinned-mainnet-fork execution remain global release gates. No shared fleet,
external source chain, production signer, real key or money was touched.

## 2026-07-17T00:17:35Z — FastPay bounded-recovery model and protocol selected

### Safety result

- The first executable recovery rule attempted to confirm from `q-f` revealed
  partial votes. It failed for `n=4`: two size-2 sets can intersect only in the
  one Byzantine validator. That rule is rejected and was not implemented in
  production.
- The selected design permits recovery confirmation only from a complete,
  signature-verified normal `n-f` certificate. Without one, ordered recovery
  cancels and atomically advances every input object version, permanently
  fencing delayed old certificates.
- Wallet product finality requires `n-f` distinct durable apply
  acknowledgements. An honest validator persists the complete certificate and
  effect before acknowledging, so a finalized payment has recoverable evidence;
  a broker-only withheld certificate is not reported final and may cancel.
- `docs/specs/fastpay-payment-recovery-v1.md` defines signed domains, bounded
  windows, durable records, recovery states, reconfiguration, RPC/wallet
  semantics, production code map and acceptance gates. Core FastPay remains
  enabled; `owned_safe_unlock` stays fail-closed only until the production
  recovery path replaces it.

### Commands and evidence

- `cargo test -p postfiat-fastpay-prototype cancellation_model -- --nocapture`
  — PASS `12/12`, including exhaustive `n=4` and `n=6` honest/Byzantine vote
  assignments, partial cancellation, delayed certificate fencing, full-cert
  recovery, withheld broker, restart retrieval, committee rotation, replay,
  bounded windows and crash-after-certificate-persist.
- `cargo clippy -p postfiat-fastpay-prototype --all-targets --locked -- -D
  warnings` — PASS.
- `cargo fmt --all -- --check` and targeted `git diff --check` — PASS.
- Evidence:
  `reports/open-source-p1-fastpay-recovery-model-20260717T001735Z/ACCEPTANCE.json`.

### Disposition

The first `P1-FASTPAY-01` recovery checkbox is green for definition and model.
No production recovery claim is made. Signed v3 types, atomic storage,
consensus-ordered decisions, state-root/snapshot integration, recovery RPCs,
wallet UX and six-node latency/correctness acceptance remain active work.

## 2026-07-17T03:16:17Z — FastPay production recovery, anchoring and minority rollback

### Reproduction and remediation

- A real six-validator regression reproduced a missing protocol case: one
  validator applied a valid full FastPay certificate, while the other five did
  not. The minority correctly refused to vote for an ordered block that omitted
  its durable effect. The other five could still form a valid `n-f` block
  certificate, but the minority then rejected that canonical certificate with
  `block proposal omitted a locally durable unanchored FastPay effect`.
- A direct effect with fewer than `n-f` durable apply acknowledgements is now
  explicitly speculative. Before the effect is written, the node atomically
  persists a bounded inverse journal containing the full certificate, original
  owned objects and exact vector positions, and any prior unwrap account state.
- A validator still refuses to vote for omission. Only after independently
  verifying an external block certificate may it roll back the complete
  unanchored suffix in reverse order, retain the certificates for ordered
  recovery, apply the canonical effect list, and commit the ordered block.
  An `n-f`-applied effect cannot be validly omitted because the two quorums
  intersect in honest validators that refuse omission.
- Direct effects are bound into the next block in canonical lock-ID order.
  Lagging validators verify the attached certificate and reconstruct the exact
  pre-state effect before ordered execution. Snapshot v6 now preserves both the
  owned-lock map and speculative recovery journal; legacy snapshots containing
  activated FastPay state without these files fail closed.
- The adversarial minority case uses the unwrap branch: the speculative apply
  consumed the owned input and credited nine native atoms; certified omission
  removed the new account, restored the exact object, retained the full unwrap
  certificate, then survived snapshot export/import and `verify-blocks` replay.
  Core FastPay remained enabled throughout.

### Commands and evidence

- `cargo test -p postfiat-node
  six_validators_certify_anchor_and_catch_up_one_missing_fastpay_effect --
  --nocapture` — PASS `1/1` in 29.98 seconds after unwrap strengthening.
- `cargo test -p postfiat-fastpay-prototype cancellation_model -- --nocapture`
  — PASS `12/12`.
- `cargo test -p postfiat-execution owned_transfer_recovery -- --nocapture` —
  PASS `7/7`.
- `cargo test -p postfiat-node fastpay_payment_safety -- --nocapture` — PASS
  `14/14` before the unwrap strengthening; the changed exact test then passed
  separately as recorded above.
- `cargo test -p postfiat-node snapshot -- --nocapture` — PASS `16/16`.
- `cargo check -p postfiat-node --all-targets` — PASS.
- `cargo clippy -p postfiat-types -p postfiat-execution -p postfiat-node
  --all-targets -- -D warnings` — PASS.
- `cargo fmt --all -- --check` — PASS.
- Wallet web — PASS `240/240`; wallet proxy — PASS `23/23`.
- Evidence:
  `reports/open-source-p1-fastpay-production-recovery-20260717T031617Z/ACCEPTANCE.json`,
  SHA-256
  `50bffc346fa91c728d140de7031e9d7a2de138110ca1a6d6a0a07fbce4e58195`.
- Strict file-size gate remains green: recovery node 986 lines, block replay
  2,565, FastPay safety tests 2,179, consensus artifacts 4,132; all are below
  5,000 lines.

### Disposition

The production recovery, ordered anchoring/catch-up, transfer/unwrap minority
rollback and snapshot-safety slice is green locally. The active P1 is not yet
closed: production committee-rotation/crash-matrix evidence, the immutable
candidate suite, and the real six-node WAN correctness/latency gate remain. No
fleet, money, signer, key, consensus threshold, or deployed feature was changed.

## 2026-07-17T03:30:36Z — governed FastPay committee rotation and old-epoch drain

### Reproduction

- Added the replicated-state transition regression
  `governed_committee_rotation_preserves_old_recovery_and_fences_overlap`.
- Before remediation it failed at the real encoded payload boundary with
  `FastPay policy and committee activation heights differ`: the only governed
  transition was a one-time bootstrap and could not stage a later committee.

### Remediation

- The signed v1 governance envelope remains wire-compatible, but its replicated
  transition now distinguishes initial bootstrap from committee rotation.
- Initial bootstrap retains the strict empty-state, exact activation and
  future-height requirements. Rotation must preserve the exact policy and chain
  domain, use epoch `previous+1`, start exactly at
  `previous.new_orders_through_height+1`, and commit before that height.
  Duplicate roots/epochs, overlap, gaps, policy changes and backdating reject
  before mutation.
- Rotation appends rather than replaces the committee. New admission selects
  the height-active committee, while an old order resolves through its exact
  historical epoch/root. Bootstrap and rotation produce distinct accepted
  receipt codes.

### Commands and evidence

- Pre-fix exact execution test — RED as expected with the activation-equality
  error above.
- `cargo test -p postfiat-execution owned_transfer_recovery -- --nocapture` —
  PASS `8/8`, including overlap atomicity and epoch-1 cancellation/version
  advance after epoch 2 is installed.
- `cargo test -p postfiat-node
  fastpay_recovery_bootstrap_is_signed_future_activated_and_tamper_atomic --
  --nocapture` — PASS `1/1` in 6.02 seconds. The real four-validator path signs
  and commits bootstrap then rotation, checks both receipt codes, replays both
  blocks, and snapshot-restores both committees with the exact state root.
- `cargo test -p postfiat-types fastpay -- --nocapture` — PASS `2/2`.
- Affected node check, strict types/execution/node Clippy, and format — PASS.
- Evidence:
  `reports/open-source-p1-fastpay-committee-rotation-20260717T033036Z/ACCEPTANCE.json`,
  SHA-256
  `93f7b7bbabfc62119891a3b04b060678ff133b31cc2968ee37a2a2fbef965ff5`.

### Disposition

The production committee-rotation transition and old-epoch recovery path are
green locally. P1 closure still requires persistence-boundary fault injection,
the immutable candidate suite, and the six-node WAN correctness/latency gate.
No fleet, money, signer, key, threshold, or deployed feature was changed.

## 2026-07-17T03:37:09Z — FastPay persistence-boundary crash matrix

- The real six-validator minority-unwrap scenario now simulates a crash after
  the inverse journal is durable but before the ledger effect is written.
  Restart exposes the complete old ledger, and an idempotent retry completes
  the byte-identical effect and passes replay.
- The same scenario materializes the certified-omission rollback as a real
  ordered delta journal, then simulates all ten persisted prefixes: journal
  only; ledger; receipts; ordered-batch log; archive; block; and tip. Every
  restart completes the exact terminal ledger, logs, block, tip/root, removes
  the journal, and passes `verify-blocks`.
- Existing real-store tests cover lock-WAL persist-before-vote, restart,
  committed conflict, torn uncommitted tail, and certificate/effect/fence
  persistence before the signed apply acknowledgement. Snapshot restore covers
  the owned locks and inverse journal.
- `cargo test -p postfiat-node fastpay_payment_safety -- --nocapture` — PASS
  `14/14` in 42.83 seconds.
- Strengthened exact six-validator test — PASS `1/1` in 41.52 seconds.
- Model `12/12`, execution recovery `8/8`, and format remain green.
- Evidence:
  `reports/open-source-p1-fastpay-crash-matrix-20260717T033709Z/ACCEPTANCE.json`,
  SHA-256
  `0602315444ae44fa0516be3b65ad9be978766b17f2207127248971a8bfc671e8`.
- This closes the finding-specific Byzantine/partition/withheld-broker/delay/
  expiry/restart/reconfiguration/replay/crash matrix locally. The WAN
  correctness/latency and immutable-candidate gates remain. No fleet, money,
  signer, key, threshold, or deployed feature was changed.

## 2026-07-17T03:49:03Z — stale Python unsigned FastPay funding caller removed

- The public Python client and WAN runner still exposed/called `wrap_owned` and
  `unwrap_owned` after the server, proxy and browser caller had been migrated.
  The inverse client tests failed pre-fix because both unsafe methods existed
  and no signed FastLane-primary submit method existed.
- Both unsigned client methods are removed. `wrap_fastpay` now validates exact
  live chain/genesis/protocol/account state, constructs `OwnedDepositV1`, signs
  locally through the new `wallet-sign-owned-deposit` SDK CLI, submits the
  signed FastLane transaction, requires `accepted=true` and exact receipt code
  `owned_deposit_applied`, and requires one exact created object from bounded
  before/after state.
- `PYTHONPATH=python python3 -m pytest python/tests/test_wallet.py
  python/tests/test_latency.py -q` — PASS `73/73`.
- `cargo test -p postfiat-rpc-sdk --all-targets -- --nocapture` — PASS
  `63/63`; affected check, strict Clippy and Python compileall pass.
- Evidence:
  `reports/open-source-p0-python-fastpay-signed-deposit-20260717T034903Z/ACCEPTANCE.json`,
  SHA-256
  `ac370c7dc094b49b5c6c9606b81fc4f6cf065acde5f0d3ffd14bd47320bf91d1`.
- This makes the earlier “every live caller migrated” P0 claim true rather than
  browser-only. No fleet, money, signer, key, threshold, or deployed feature
  was changed.

## 2026-07-17T03:50:00Z — broad node recovery-slice regression completed

- `cargo test -p postfiat-node --lib -- --nocapture` — PASS `210/210`, with
  `2` ignored, in `2446.99s`.
- The process compiled before the subsequent committee-rotation, crash-matrix,
  and Python caller edits. It is therefore broad supporting evidence for the
  production-recovery slice, not the immutable-current-candidate gate. The
  later edits retain their exact targeted check, test, strict-Clippy, and
  format evidence; the final immutable-candidate suite remains explicitly
  open.
- No fleet, money, signer, key, threshold, or deployed feature was changed.

## 2026-07-17T03:54:12Z — current-tree integrated gates remain green

- `cargo fmt --all -- --check` — PASS.
- `cargo check --workspace --all-targets --locked` — PASS in `26.41s`.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS in
  `36.93s`.
- Wallet-web unit tests — PASS `240/240`; wallet-proxy regression suite — PASS
  `23/23`; wallet-web production build — PASS.
- Chromium custody-boundary and public-service boundary suites — PASS `1/1`
  each. Both wallet npm audits report zero vulnerabilities.
- `git diff --check` — PASS.
- Evidence:
  `reports/open-source-current-tree-integrated-gates-20260717T035412Z/ACCEPTANCE.json`,
  SHA-256
  `0c309463f7bbf796c8dfef589c3b2951a14bd5cf4c02604adf3807a5be44e7d6`.
- This is explicitly current mutable-tree confidence, not immutable-candidate
  closure. The corresponding master-plan boxes remain unchecked until one
  reviewed commit/tree is frozen and these gates are rerun against it.
- No fleet, money, signer, key, threshold, or deployed feature was changed.

## 2026-07-17T03:56:23Z — Foundry offline green; pinned-fork gate fails closed without RPC

- `forge test` — RED by design in exactly the two
  `PFTLUniswapOfficialForkTest` cases because
  `ETHEREUM_MAINNET_RPC_URL` is absent. The suite no longer silently passes or
  turns an official-fork assertion into a no-op when the required provider is
  unavailable. The same run passed the other `103` tests.
- `forge test --no-match-path
  test/PFTLUniswapOfficialFork.t.sol` — PASS `103/103`, including the 256-run,
  128,000-call settlement-verifier invariant.
- This separates the offline contract gate from the mandatory pinned-fork gate;
  it does not waive or mark the latter green. Candidate closure still requires
  an authorized official-mainnet RPC value and a passing complete run.
- No fleet, money, signer, key, threshold, contract deployment, or chain state
  was changed.

## 2026-07-17T04:07:32Z — WAN Python FastPay send/unwrap migrated to recovery-safe v3

### Reproduction

- The new transport contract test failed because `PostFiatRpcClient` exposed no
  recovery-capability or v3 vote/apply methods.
- The new send and unwrap boundary tests then failed because both helpers still
  invoked legacy v2 signing rather than the governed v3 recovery path.

### Remediation and evidence

- Python now reads the fresh recovery capability per operation, validates its
  exact chain domain, active bounded policy, committee epoch, BFT quorum, and
  complete distinct validator roster, and derives a bounded recovery window.
- The Rust CLI derives the canonical lock ID before local v3 signing. Send and
  unwrap collect v3 votes concurrently and broadcast the canonical sorted
  certificate. A second Rust boundary verifies distinct ML-DSA apply
  acknowledgements against the exact domain, epoch, lock, certificate digest,
  common order/terminal state, validator identity, and governed quorum.
- The real cryptographic CLI regression accepts `3/4` distinct acknowledgements
  and rejects a tampered acknowledgement at `2/3`; it does not trust the proxy's
  success label.
- Python wallet/latency tests — PASS `76/76`. RPC SDK all-target tests — PASS
  `64/64`. Affected strict Clippy, format, compileall, and diff checks — PASS.
- Evidence:
  `reports/open-source-p1-fastpay-python-v3-wan-client-20260717T040732Z/ACCEPTANCE.json`,
  SHA-256
  `2ee2a07b9c2c2a65d58ec169827440e4ed545932a09b8eef218f676acd8ddb80`.
- The real six-node WAN correctness/latency checkbox remains open; no fleet,
  money, signer, key, threshold, or deployed feature was changed.

## 2026-07-17T04:32:29Z — immutable candidate security and product gates

The remediation was frozen as separable local commits ending at candidate
`00747667`. The candidate was clean before the following gates. No branch was
pushed and no fleet, signer, key, threshold, contract, or money state changed.

### Candidate-gate REDs and bounded fixes

- `scripts/test-public-source-portability` initially rejected one archived
  maintainer path, maintainer-specific Foundry paths in this lab book, a real
  historical fleet IP, and the public-IP negative fixture in
  `scripts/test-public-runtime-default-scan`. Paths and the historical address
  were redacted or made portable; the negative fixture now constructs the same
  address at runtime, preserving its fail-closed assertion. Both scanners PASS.
- `scripts/test-public-artifact-policy` detected that both checked wallet WASM
  files no longer matched the stale manifest hash. The shipping build script was
  run twice from the frozen Rust tree; both builds produced byte-identical web,
  extension, and package artifacts with SHA-256
  `213fcc6062a8107bb11e25fda18309cda9926a0e6c6ff898eb18102f51751aa2`.
  The exact artifact allowlist was rebound to that reproducible hash and PASSes.
- `scripts/test-rpc-method-inventory` failed closed on the newly implemented v3
  FastPay recovery methods and existing public recovery/route reads. The
  generator and regression now explicitly classify all `143` observed methods;
  zero methods are unknown and the gate PASSes.
- `scripts/verify-vendored-halo2` found that bulk whitespace cleanup had removed
  the terminal blank line from two exact upstream license copies. Both files
  were restored byte-for-byte, path-specific Git whitespace attributes preserve
  the upstream bytes, and the immutable upstream commit plus normalized local
  patch SHA-256 `d51e2e6edaa55be0910f4a72b1fd66ef9f634f9037437247ab3d25f6eb0d7a73`
  verify.

### Exact-candidate evidence

- `cargo fmt --all -- --check` — PASS.
- `cargo check --workspace --all-targets --locked` — PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — PASS.
- `cargo audit` — PASS with zero vulnerabilities and the three documented,
  time-bounded unmaintained-transitive warnings; `cargo deny check` — PASS for
  advisories, bans, licenses, and sources.
- Deterministic CycloneDX generation — PASS twice, `307` components, byte-match,
  SHA-256 `416aff6e6bacf13ede39e8bd002097ab4b9b77bd8d1f4d1c002323e197552d3b`.
- `wallet-web`: unit tests `240/240`, production build, custody Chromium `1/1`,
  public security Chromium `1/1`, and npm audit — PASS, zero vulnerabilities.
- `wallet-proxy`: regression suite `23/23` and npm audit — PASS, zero
  vulnerabilities.
- Python wallet/WAN/rollout/monitor suite — PASS `97` tests plus `8` subtests.
- Shipping `wallet_test_vector_redaction` subprocess boundary — PASS `1/1`.
- Full Foundry suite against Ethereum mainnet block `25,440,306` through an
  archive-capable RPC — PASS `105/105`, including both official Uniswap/finality
  fork tests. An initial public endpoint passed the deployment-code case but
  returned an archive-account `403` for the second case; the same pinned test
  completed through the archive-capable endpoint without changing the test.
- Strict MkDocs, documentation links, redaction, whitepaper-boundary, runtime
  default, secret, source-portability, artifact, RPC inventory, proof-public-
  input, consensus-determinism, crypto-callsite, publication-gate regression,
  logrotate, and NAVSwap redaction gates — PASS.

These results close four P0 immutable-candidate checkboxes and five global
candidate gates. The P0 checklist is now `141/158` (`89.2%`). The long complete
workspace test and explicit release-scale ignored safety tests remain separate
in-progress gates and are not claimed by this entry.

## 2026-07-17T04:39:28Z — publication now hard-gates on private provider action record

### Reproduction

- The publication regression first failed because
  `scripts/verify-publication-candidate` accepted a clean exact-tree/history
  candidate without any provider revocation/decommission record.
- A follow-up negative test proved the first path-normalization draft followed
  a symlink before checking its type and would have accepted a symlinked record.

### Fix and evidence

- `--provider-revocation-record` is now mandatory. The record must remain
  outside the public repository, be a nonsymlink regular file with no
  group/other permission bits, fit within 16 KiB, and match the exact
  `postfiat-provider-credential-revocation-v1` schema.
- Only bounded provider, incident/evidence-reference, terminal-action,
  owner/verifier, and UTC timestamp fields are accepted. Missing/extra fields,
  nonterminal actions, malformed timestamps, missing files, public permissions,
  symlinks, and in-repository records fail closed. The schema contains no token,
  secret, key, or credential-value field.
- `scripts/test-verify-publication-candidate` — PASS for the clean fixture and
  PASS on every required negative boundary: missing record, group-readable
  record, impossible calendar timestamp, symlink, record inside the candidate,
  unexpected ref, tree drift, and deleted-but-reachable historical credential.
- `python3 -m py_compile scripts/verify-publication-candidate` and `bash -n
  scripts/test-verify-publication-candidate` — PASS.

This closes the automatic publication-blocking implementation checkbox. It
does not claim that the provider owner has performed or documented the real
revocation/decommission action; that external P0 checkbox remains open.

## 2026-07-17T04:46:36Z — immutable native-supply lifecycle battery green

- `cargo test -p postfiat-node --lib
  native_supply_oracle_counts_each_live_custody_lane_once_and_checks_overflow
  --locked -- --nocapture` — PASS `1/1` across every live custody lane and
  checked overflow boundary.
- `cargo test -p postfiat-node --lib history_prune --locked -- --nocapture` —
  PASS `2/2`: checkpointed prune, post-prune append, and interrupted-prune
  recovery preserve the native-supply proof.
- `cargo test -p postfiat-node --lib
  signed_snapshot_roundtrip_rejects_tampering_and_preserves_signer_isolation
  --locked -- --nocapture` — PASS `1/1`; the restored snapshot preserves exact
  native supply and block replay.
- `cargo test -p postfiat-node --lib
  verify_blocks_rejects_coordinated_genesis_native_supply_rewrite --locked --
  --nocapture` — PASS `1/1`, rejecting a coordinated faucet/ledger rewrite.
- These candidate commands complement the already-green 256-iteration/2,304-
  case native adversarial harness and legacy-v1 archive rebuild. The P0 native
  genesis-to-tip/prune/snapshot/post-restore checkbox is now closed. No live
  fleet, snapshot, signer, or money state changed.

## 2026-07-17T04:52:32Z — immutable owned-asset/FastPay integration gate green

- `cargo test -q -p postfiat-execution --all-targets --locked` — PASS
  `156/156`, including native-only legacy wrap rejection, signed issued-asset
  deposits, owned transfer/unwrap conservation, duplicate IDs, overflow,
  replay, certificate-domain, recovery, and FastSwap/bridge interactions.
- `cargo test -q -p postfiat-node --lib fastpay_payment_safety --locked` —
  PASS `14/14`, including concurrent unsigned-wrap no-mutation, signed deposit,
  v3 recovery/catch-up, isolated signer/public roster, issued-to-native unwrap
  rejection, WAL recovery, and structural unsafe-unlock disablement.
- `cargo test -p postfiat-fastpay-prototype --all-targets --locked` — PASS
  `21/21`, including duplicate fixture/input/vote, double-spend, conservation,
  full-certificate recovery, expiry, partition, rotation, and Byzantine models.
- Together with the already-green 256-iteration/2,816-case owned-object fuzz
  harness and strict Clippy, this closes the immutable P0-ASSET integration
  checkbox. No capability was disabled and no live fleet or money state changed.

## 2026-07-17T05:02:48Z — FastSwap release restart proof corrected and green

### Reproduction and diagnosis

- `cargo test --release -p postfiat-node --test fastswap_local_six
  fastswap_local_six_process_quorum_replication_conservation_and_restart
  --locked -- --ignored --nocapture` first failed after successful settlement at
  an assertion requiring `fastswap-v1.lock` to have been deleted on shutdown.
- `FastSwapStore::open` deliberately holds a Unix advisory `flock` on a durable
  inode and releases the lock by closing the descriptor. Removing the pathname
  would be unsafe: a concurrent opener could create and lock a different inode
  while an older process still held the unlinked inode. The failure was therefore
  a stale test assertion, not a settlement, durability, or restart defect.

### Correction and evidence

- The storage regression now proves the second concurrent opener fails closed,
  the lock inode remains after the first store drops, and a new store reacquires
  that same durable lock. The six-process test now requires the durable inode and
  then proves the actual boundary: all original processes exit, validator 0
  restarts, reacquires the lock, replays the WAL, and returns the terminal swap.
- `cargo test -p postfiat-storage
  fastswap_store::tests::cross_process_lock_fails_closed --locked` — PASS.
- The corrected release six-process proof — PASS `1/1`: three `5/6` quorum
  waves, exact-six replicated effects, conservation, terminal tombstone, and
  restart/replay; `preview_ms=20`, `settlement_ms=99`, `total_ms=99`.
- `cargo fmt --all -- --check` and `git diff --check` — PASS. No production
  capability, consensus rule, threshold, fleet, or money state changed.

## 2026-07-17T05:05:00Z — complete workspace and Orchard release gates green

- `cargo test --workspace --all-targets --locked -- --nocapture` — PASS after
  2,507.02 seconds in the largest node library target and approximately 47
  minutes end to end. The node library passed `210/210` with two explicit
  external-tool tests selected separately; the node binary passed `173/173`
  with its one explicit performance test selected separately. All remaining
  ordinary workspace crates and targets passed, including Orchard `83/83`,
  ordering/adversarial consensus `27/27`, RPC SDK `59/59`, storage `25/25`,
  types `89/89`, and the shipping wallet-vector subprocess boundary.
- `cargo test --release -p postfiat-privacy-orchard --lib --locked --
  --ignored --skip write_asset_orchard_k15_params_release_artifact --nocapture`
  — PASS `16/16` in 341.26 seconds, covering full-shape proving/verifying,
  tamper, authority, anchor/path, conservation, private egress, key metadata,
  and baseline/cached prover paths.
- The separately selected release parameter writer passed `1/1` and reproduced
  the committed 2,097,220-byte artifact exactly; SHA-256
  `e1fb2974a4a0a87f8ac0dbaaa4c7ea3c4e9f293a560585f7ca6233b78f42d0dd`.
- This closes the P0-PRIVACY-01 immutable proving/replay checkbox. Explicit
  six-process and Foundry-backed node gates remain independently recorded; the
  two-fresh-wallet P0-PRIVACY-02 product gate remains open.

## 2026-07-17T05:08:00Z — all explicit node release gates green

- Mandatory six-validator W6 TCP smoke — PASS `1/1` in 56.43 seconds: accepted
  atomic swap at height 20, deterministic proposer `validator-2`, exact-six
  finality/catch-up, and root
  `d0833af90a5ca96711de012c4dc812d418083b406f9be67cb7e3591364138e3c65cb64b2184d2e67277d82a90f0b12a6`.
- FastSwap six-process 100-warm-operation release gate — PASS `1/1`:
  cold 125 ms, p50 116 ms, p95 148 ms, p99 158 ms; `101/101` accepted,
  exact-six and conserved. The independent in-process 100-operation release
  gate also passed at p50 92 ms, p95 97 ms, p99 100 ms.
- Both explicit isolated Foundry/Anvil bridge tests — PASS `2/2` in 4.67
  seconds: governed-route deposit/withdrawal round trip and mint-settlement
  release bound to accepted PFTL backing.
- With the ordinary complete workspace and explicit Orchard gates, no ignored
  privacy, consensus, replay, migration, performance, or bridge security target
  remains unexecuted. This closes the global complete-workspace checkbox and
  the P0-COMMIT-ATOMICITY immutable batch-kind/crash-matrix checkbox.
- The P0 burn-down is now `146/158` (`92.4%`); the remaining 12 boxes are
  external publication action/staging, two-fresh-wallet and issued-customer
  flow, and the single batched cap-valid devnet migration/shadow proof.

## 2026-07-17T05:21:57Z — two-fresh-wallet encrypted privacy flow green

- The existing real complete-flow regression was strengthened from one holder
  controlling both legs to two distinct fresh accounts with separate master
  keys, accepted public funding receipts, cross-asset trustlines, issued
  holdings, and encrypted-v2 private inputs. This would have failed the old
  single-holder fixture requirement before the test change.
- `cargo test --release -p postfiat-node --lib
  tests::wan_devnet_invalid_asset_orchard_swap_proof_is_rejected_and_valid_swap_still_applies
  --locked -- --exact --nocapture` — PASS `1/1` in 340.41 seconds at commit
  `d1e68ee8`.
- The release proof verifies the real K15 swap, exact accepted receipt code,
  two input nullifiers, chain-only output recovery, accepted private egress,
  spent input, exact positive public delta and global issued-supply
  conservation. Thirteen public artifacts contain no note opening or spend
  authority. Plaintext ciphertext, live/mixed v1, invalid proof, stale epoch,
  wrong packet, off-band pricing and replay all reject at their expected
  boundaries.
- `cargo clippy -p postfiat-node --all-targets --locked -- -D warnings`,
  `cargo fmt --all -- --check`, and `git diff --check` — PASS.
- Evidence:
  `reports/open-source-p0-privacy-two-wallet-20260717T052157Z/ACCEPTANCE.json`
  (SHA-256
  `ffcd818b23f60cc81eaa660d2b0f01bb0fddc9e6a593e9758bce842fa2686978`).
  P0-PRIVACY-02 is closed; the P0 burn-down is now `147/158` (`93.0%`).

## 2026-07-17T05:36:00Z — immutable binary and cap-valid reset bootstrap preflight green

- Commit `ee3ebe2e8ef96b737756798fad895e53346f411e` was built in two
  independent clean Cargo target directories. Both release validator binaries
  are byte-identical with SHA-256
  `f3a05136cc42be195b71fc39fd8748171a6b48bf7e0bd84de3818a7fc9257799`.
- A preliminary build in the reused workspace target directory produced
  `fdba26af...82ee`. It is explicitly excluded from the candidate; only the
  two matching clean-build artifacts qualify for release.
- Read-only WAN preflight is GREEN at the existing fleet's exact-six point:
  height 1220, block hash `c8156a3c...a511d`, state root
  `8a534e5c...3986f`, six empty mempools, and active binary SHA-256
  `428c4c73...c3c3` on all validators. Genesis, registry, topology, service
  units, data sizes, available disk, and key/registry counts were inventoried
  without copying private signer material into the report.
- A controller-only clean-genesis rehearsal generated a signed v6 bootstrap
  for the existing six WAN endpoints. It schedules consensus v2 at height 1,
  activates replicated-state v2 from genesis, keeps one local signing key per
  validator with a six-member public registry, and uses a distinct snapshot
  signer. Six fresh imports passed local-key validation, `verify-state`, and
  `verify-blocks`.
- Evidence:
  `reports/open-source-public-candidate-20260717T052600Z/ACCEPTANCE.json` and
  `bootstrap/bootstrap-manifest.public.json`. The shared devnet was not
  mutated. The P0 count remains `147/158`: the cap-valid live reset/shadow and
  coherent all-lane customer-flow boxes are not claimed by this preflight.

## 2026-07-17T05:58:00Z — candidate startup permission defect rolled back exactly and fixed locally

- A final read-only freeze reconfirmed all six legacy validators at height
  1220, block `c8156a3c...a511d`, root `8a534e5c...3986f`, with empty
  mempools. The one authorized reset attempt then stopped all six services,
  moved every existing data and log directory into a per-host rollback anchor,
  installed the candidate, and ran local-key/state/block verification on all
  six staged stores successfully.
- The first candidate transport service failed in `ExecStartPre` before any
  candidate validator joined or any block was proposed. The controller
  immediately restored all six original stores, logs, and service units. A
  live six-node RPC check proved the rollback byte-exact at the same height,
  block, root, and empty mempools.
- The real service-user reproduction identified the exact cause: the public
  signed deployment manifest and public trust anchor were installed
  `root:root 0600` under a non-traversable release directory, so
  `deployment-manifest-verify` failed with `Permission denied (os error 13)`.
  This was an installer/generator permission defect, not consensus, state, or
  committee divergence.
- The release generator now emits deterministic `0644` permissions for the
  public signed manifest, public trust anchor, topology, circuit metadata,
  units, environments, and runtime bindings, with traversable `0755` staged
  directories and a `0755` binary. The private deployment publisher key
  remains exactly `0600`.
- `cargo test -p postfiat-node snapshot_deployment -- --nocapture` — PASS
  `10/10`, including the new manifest/trust-anchor/service-artifact permission
  regressions. `cargo fmt`, `git diff --check`, and the exact h1220 rollback
  probe also pass. A regenerated byte-reproducible candidate and full offline
  service-user preflight are required before another reset attempt; no closure
  checkbox or P0 count is claimed yet.

## 2026-07-17T06:31:00Z — cap-valid candidate live and P0-STATE current-devnet gate closed

- The deployment-permission fix was committed as `249149bc`. Two independent
  clean release builds are byte-identical at SHA-256
  `c379bfca23d4ed43097e7f0386848ce755d1d7f4844b50eeb9202c12eb86358d`;
  the exact signed deployment-manifest SHA-256 is
  `5dbdd219831d22dc5b162da98320f765dd2af3016cc838d407aba0127346e017`.
- Before stopping anything, the controller re-proved the old fleet exact 6/6
  at h1220/root `8a534e5c…695b4d49cfbfe54ff9233665bfd0bd3986f` with empty
  mempools. Old data, logs, and units were preserved under rollback anchor
  `open-source-candidate-reset-20260717T061500Z`.
- All six candidate stores passed offline manifest, local-key, state and block
  checks as the real `postfiat` service user. The six validators then joined at
  the same new genesis/root with private-WireGuard transport, five peers each,
  loopback RPC, and exact binary identity.
- The one height-1 transparent activation transaction committed once. The
  driving CLI subsequently reported `batch already applied` because the
  resident validator had already applied the certified batch; no retry was
  made. Chain reconciliation proves receipt
  `7357eaeb…4a4411a` accepted/code `accepted` on 6/6, legacy/prepare/precommit
  vote counts 5/5/6, exact-six block `28381e24…aa5d`, root
  `4475e0cc…7979`, empty mempools, and native conservation
  999,999,978 + 22 burned = 1,000,000,000.
- `snapshot-export`, signed v6 export with `signer_material_included=false`,
  six independent signed imports, six `verify-state`, and six `verify-blocks`
  checks all pass and reproduce the exact h1 tip/root. Commands and raw outputs
  are sealed under
  `reports/open-source-public-candidate-20260717T052600Z/reset/open-source-candidate-reset-20260717T061500Z/`.
  `ACCEPTANCE.json` SHA-256 is
  `2269e611a93a2715c2746859c63e91eb577d79a8fbad5bedec029a6eb7083d73`.
- P0-STATE-01 is now fixed-candidate and the current-devnet cap-valid
  shadow/replay box is closed without an oracle bypass. P0 burn-down is
  `148/158` (`93.7%`); ten boxes remain, of which only the coherent issued
  customer flow is a substantive in-repository P0 implementation/proof gate.

## 2026-07-17T06:48:00Z — P0-ISSUED complete four-lane customer flow closed

- Added the missing composition regression at commit `aa35692a`. One issued
  asset is simultaneously held as 30 transparent + 20 FastLane + 25 encrypted
  AssetOrchard + 25 registered external atoms under `max_supply=100`.
- The exact-cap global oracle passes. A real signed one-atom issuer payment is
  deliberately accepted by the narrower execution dry-run that sees the 75
  non-private atoms, then the shipping node-global admission check adds the 25
  private atoms and rejects the 101-atom candidate before canonical mutation.
  Moving one atom through transparent→FastLane→private→external→transparent
  keeps the global total exactly 100 after every step.
- `cargo test --release -p postfiat-node --lib
  tests::issued_supply_complete_customer_custody_flow_counts_all_lanes_together
  --locked -- --exact --nocapture` — PASS `1/1`.
- The production transition components pass in release: FastLane reserve cap
  enforcement `1/1`; BFT-checkpoint external subscribe/export/refund `1/1`;
  real two-fresh-wallet encrypted K15 ingress, atomic swap, chain-only recovery
  and private egress `1/1` in 341.12 seconds with accepted receipt codes and
  conserved issued supply.
- `cargo check --workspace --all-targets --locked`, node strict Clippy,
  formatting and diff checks pass. Non-fork Foundry passes `103/103`. The
  official-mainnet fork tests fail closed because the provider-owned
  `ETHEREUM_MAINNET_RPC_URL` is absent; this is the already-open external
  credential gate, not a silent skip.
- The proxy unit regressions reached before its legacy live smoke pass. That
  smoke cannot reach its retired pre-reset RPC target and also hard-requires
  height >=470 while the cap-valid candidate chain is at h1; the prior exact
  candidate proxy suite remains `23/23`, and no proxy/runtime source changed in
  this test-only delta. This stale live-smoke binding remains recorded rather
  than misreported green.
- Evidence:
  `reports/open-source-p0-issued-customer-flow-20260717T064800Z/ACCEPTANCE.json`
  (SHA-256
  `993d961ada1d113a9bf91f74a17e430c87b8795439cf561ef310a23820b88d6a`).
  P0-ISSUED-SUPPLY-02 is fixed-candidate. P0 burn-down is now `149/158`
  (`94.3%`); the nine remaining P0 boxes are publication staging/history-scan
  and provider-owned credential evidence rather than live source defects.

## 2026-07-17T10:55:00Z — FastPay safety-correct WAN acceptance and roadmap correction

- Commits `07cb8cef`, `2bcea899`, and `1e9352c6` repaired the compact proxy
  response boundary and then restored the governing FastPay finality rule: a
  wallet reports success only after independently verifying the full owned
  certificate and five distinct signed durable apply acknowledgements. The
  interim approximately 1.1-second one-ack result was identified as unsafe
  premature success and is not acceptance evidence.
- Targeted proxy tests, all 18 current proxy security/routing/unit suites,
  Python wallet/latency 79/79, RPC SDK 60+3+2, workspace check, and workspace
  strict Clippy all pass at `1e9352c6`.
- One safety-correct live proof completed in 3,116.273 ms with five signed
  acknowledgements. The five-payment WAN battery then completed 5/5 with row
  times 2,777.700, 3,724.984, 2,489.978, 2,357.109 and 2,316.954 ms: p50
  2,489.978 ms and p95 3,724.984 ms. Every row carried five distinct signed
  acknowledgements. Evidence:
  `reports/open-source-p1-fastpay-wan-20260717T-quorum-ack-1e9352c6-proof/`
  and
  `reports/open-source-p1-fastpay-wan-20260717T-quorum-ack-1e9352c6-five-payment/`.
- The untimed post-battery audit found exact-six height 33, state root
  `45f179ed6c26da9f22d678a04cedd05d5dbe7d08e4180fddaf677bc3cdf90e58280ffe13ae955b4d6988f71e339cb21b`,
  tip
  `5d6a6ded59e90e7f4e8580a1fad5b73d85c13b6902319dd34a8d996bf423d88b860d75fbcaba69289ed73da7dc383749`,
  empty mempools, and identical destination holdings on every validator: 22
  one-atom objects, total 22 atoms.
- A prior manual height-14 recovery proved the consensus-v2 later-view safety
  path but exposed a product liveness gap: the shipping submit/finality RPC did
  not automatically escalate from the failed view-0 proposer; view 2 required
  explicit orchestration. The P0 consensus checkbox is therefore reopened and
  the corrected burn-down is `148/158` (`93.7%`), not `149/158`.
- Source inspection also confirms an open P1 FastPay idempotency gap:
  `FastpayCertificateOutbox.complete()` deletes the q signed acknowledgements
  after exact-six replication. An identical replay after a lost client response
  can receive only unsigned `UnknownInput` errors and cannot reconstruct the
  required product-finality proof. A bounded completed tombstone or equivalent
  replayable validator acknowledgement is required.
- During the WAN run, validator-4 alone had root-owned
  `certified-send-outbox/completed` left by the manual recovery. Ownership of
  the expected service data tree was restored to `postfiat:postfiat`; no node
  restart or chain mutation was required, and the built-in outbox resume found
  zero pending or quarantined jobs.

## 2026-07-17T12:20:11Z — final internal P0 and P1 implementation gaps closed

- `P0-CONSENSUS-01`: commit `09125687` wires timeout-vote collection and
  deterministic later-view routing into the normal shipping finality path. The
  exact failed-proposer product regression passed at `n=4` in 80.51 seconds and
  `n=6` in 122.55 seconds. The timeout-envelope unit, durable timeout-vote RPC,
  proposer-routing and public-auth regressions pass; `cargo fmt --all -- --check`,
  `cargo check -p postfiat-node --all-targets --locked`, and node strict Clippy
  all pass.
- `P1-FASTPAY-01`: the pre-fix unit failed because `markTerminal` did not exist;
  the pre-fix real proxy test failed because exact-six completion had no retained
  record. Commit `77e4a3c7` adds the bounded durable v2 completed record and exact
  response replay. The unit proves crash/restart, v1 migration, count/TTL
  compaction, operation conflict and terminal tamper failure. The six-validator
  proxy test proves replay after exact-six with unchanged apply counters.
- `P1-CI-01` follow-up: aggregate `npm test` first failed because legacy Gate 3/4/5
  security tests expected a separately running proxy and validator. Commit
  `90c3836a` provides one bounded loopback fixture used only by tests. The complete
  clean-checkout proxy suite now passes `24/24`; `npm audit --audit-level=moderate`
  reports zero vulnerabilities.
- No shared-devnet mutation, money movement, dependency addition, consensus
  threshold change, or capability disable occurred. The next step is evidence-only
  reconciliation, source freeze, and the immutable-candidate battery.

## 2026-07-17T12:25:47Z — RC1 invalidated by runtime-scanner self-fixture RED; RC2 fix green

- The first frozen source candidate (`f9177bd8`, tree `726442e1`) exposed a real
  clean-checkout CI defect: `scripts/public-runtime-default-scan` included its own
  implementation and negative-test fixture in the runtime product scope, so the
  scanner's deliberately embedded unsafe-TLS and retired-destination signatures
  caused the exact candidate to fail its own publication gate. RC1 is invalid and
  is not publication evidence.
- Commit `3b4b882f` excludes only the scanner implementation and its dedicated
  negative-test fixture; all product runtime paths remain fail-closed. The
  regression now runs the scanner against the actual repository so this failure
  mode cannot recur silently. It also corrects the release-plan invocation from
  unsupported `cargo audit --locked` to `cargo audit`.
- `scripts/test-public-runtime-default-scan` — PASS;
  `scripts/public-runtime-default-scan` — PASS; `cargo audit` — PASS with zero
  vulnerabilities; `cargo deny check` — PASS; `PYTHONPATH=python python3 -m
  pytest python/tests` — PASS `139/139`; `git diff --check` — PASS.
- This is a test/publication-boundary correction only: no protocol, consensus,
  wallet, custody, money, dependency or runtime behavior changed. A clean evidence
  commit and new exact RC2 freeze are required before the complete battery.

## 2026-07-17T12:31:25Z — RC2 invalidated by crypto-inventory suffix blind spot

- The RC2 publication lane passed source portability, runtime and secret scans,
  artifact/proof/RPC/whitepaper/closure inventories, determinism, publication-
  verifier regression, Halo2 provenance, public links, strict docs/redaction,
  deterministic SBOM, wallet `240/240`, proxy `24/24`, both production builds,
  both npm audits, Python `139/139`, and 103 non-fork Foundry tests.
- `scripts/test-crypto-callsite-policy` failed because it truncated each Rust file
  at its first inline `#[cfg(test)] mod`. `block_finality.rs` legitimately contains
  later production definitions, so the scanner saw one of three domain-separated
  deterministic signing calls. Direct source inspection confirmed that all three
  expected calls remain and use the existing block-certificate, timeout-vote and
  proposal signature contexts; no cryptographic call disappeared or changed.
- Commit `0d1115ae` makes the scanner remove only complete inline test modules,
  with Rust comment, nested-comment, normal/raw-string and character-literal
  masking for brace matching. Production source before and after a test module is
  now scanned. The exact 46-call policy passes in 4.2 seconds.
- RC2 (`52b96707`, tree `bacc7758`) is invalidated rather than patched in place.
  The earlier RC1 workspace run was interrupted after its diagnostic value was
  exhausted because neither RC1 nor RC2 can certify the corrected scanner tree.
  A clean evidence commit and RC3 full battery are required.

## 2026-07-17T12:35:52Z — RC3 invalidated by cross-checkout SBOM path leakage

- RC3 (`ea41512d`, tree `d5f3d35c`) passed the complete non-Rust candidate
  lanes: workspace check/strict Clippy, scanners and inventories, strict docs,
  wallet `240/240`, proxy `24/24`, Python `139/139`, Foundry non-fork `103/103`,
  npm/Rust dependency gates, and deterministic same-checkout SBOM generation.
- A fresh one-commit export and a second non-shallow clone reproduced the exact
  reviewed tree with 1,622 files and zero tracked-tree or reachable-history
  secret findings. The SBOMs nevertheless differed: all 20 local package
  `bom-ref` values and 22 dependency rows embedded each checkout's absolute
  `path+file://` Cargo package ID. This is a reproducible-release defect, so RC3
  is invalid rather than waived.
- Commit `09dc28d4` maps only source-less local packages to stable repository-
  relative component references and uses that mapping in dependency edges.
  Registry and Git package identities remain unchanged. The generator is v2;
  it still emits 307 locked components.
- New blocking regression `scripts/test-generate-cargo-sbom` copies the complete
  tracked tree to a different path, runs both generators, and requires byte
  identity. It passes; two same-tree outputs also match at SHA-256
  `cc126e94e31d189cd283b85c4327a736f19a221b0c48317626e54bde68078650`.
  Workflow YAML, source portability, runtime/secret scans, and `git diff --check`
  pass. This changes release metadata only, not protocol or runtime behavior.

## 2026-07-17T12:40:57Z — RC4 invalidated by clean-clone proxy WASM dependency

- RC4 (`04f3d89a`, tree `fafcf0c7`) reproduced as a 1,623-file one-commit
  public export and second clone. Both had only the intended refs, exact tree,
  zero tree/history secret findings, clean `git fsck`, and matching portable
  SBOM SHA-256 `cc126e94…8650`. The strict publication verifier reached its
  expected fail-closed missing-provider-record outcome.
- Fresh `npm ci` and wallet `240/240` passed in the second clone. Proxy tests
  then failed at 8/24 because six legacy test files loaded untracked
  `crates/wallet_wasm/pkg` output that existed only in the long-lived checkout.
  The product-security workflow would reproduce the same clean-checkout RED.
- Commit `5fd7045e` changes those tests to load the checked-in canonical wallet
  release artifact under `wallet-web/src/wasm`. That artifact is the shipping
  browser input, is byte-identical to the extension copy, and remains protected
  by the exact public-artifact hash policy. No generated output was committed and
  no production path changed.
- Full proxy regression now passes `24/24` without `crates/wallet_wasm/pkg`;
  the 14-artifact exact policy and `git diff --check` pass. RC4 is invalidated;
  a clean evidence commit and RC5 exact battery are required.

## 2026-07-17T13:48:49Z — RC5 inherited-lock RED fixed; final publication candidate sealed

### RC5 failure and reproduction

- RC5 was `942bc714d7467f7cbd778b059a16518fe367dc18`, tree
  `774374a97804b034438bee17bf9cc2091c005dca`. Its complete workspace run
  reached and passed the real two-wallet AssetOrchard boundary: node library
  `213/213`, zero failures, in 2,421.82 seconds.
- The subsequent node CLI target produced one RED among 177 ordinary tests:
  `fastswap_service::tests::six_validator_exit_is_durable_before_vote_and_restart_idempotent`
  could not reopen validator 0 after `drop(validators)` because the kernel
  reported `FastSwap store is already locked`. RC5 was invalidated; the
  isolated test passing was not treated as closure.
- The actual differentiator was parallel process creation. Unix `flock` is
  attached to an open file description, so a descriptor inherited during
  `fork` can briefly retain the lock after the logical `FastSwapStore` owner
  closes its descriptor. A new storage-level regression clones the descriptor
  to model that boundary. Before the fix it failed deterministically with
  `Conflict("FastSwap store is already locked")`.

### Bounded fix

- Commit `b49c85e4` adds `Drop` for the private `ProcessLock`. The logical owner
  performs `LOCK_UN` before closing, so an inherited duplicate cannot prolong
  the owner's lock. The existing concurrent-open test still proves a second
  store fails closed for the entire owner lifetime; the durable lock inode is
  retained and restart reacquires it.
- Post-fix evidence: storage `26/26`; inherited-descriptor regression PASS;
  FastSwap exit/restart test PASS under the exact workspace/all-targets feature
  context; formatting PASS; storage+node strict Clippy PASS.

### Explicit corrected-source gates

- Release AssetOrchard ignored suite: PASS `16/16` in 341.17 seconds. The
  separately selected writer passed `1/1`, emitted 2,097,220 bytes, and matched
  the committed artifact byte-for-byte at SHA-256
  `e1fb29749c7bd0870768044d5329b4e293cb2d44dae24db2554605427b19d0dd`.
- Release FastSwap six-process correctness: PASS, total 95 ms, prepare/decision/
  effects certificates each 5 votes, exact-six replication, conservation and
  restart true. Six-process 100-operation gate: cold 128 ms, p50 123 ms, p95
  134 ms, p99 138 ms, 101/101 accepted/exact-six/conserved. In-process 100-op:
  p50 91 ms, p95 96 ms, p99 99 ms.
- Release six-validator W6 atomic swap: PASS, accepted at height 20, deterministic
  proposer validator-2, exact-six catch-up and conserved state.
- Isolated governed-route deposit/withdrawal and mint-settlement Anvil gates:
  PASS `1/1` each. No shared fleet, external chain, customer money, key,
  threshold or deployed verifier was changed.

### Closure reconciliation

- All 35 closure rows now name real commits, reproduction, regression,
  integrated evidence, claim update and residual risk. Exactly one remains open:
  `P0-SECRET-01`, an external private provider-owner terminal-action record.
- Every remaining unchecked master-checklist line is explicitly classified as
  `SOURCE-PUBLICATION BLOCKER`, `REAL-VALUE LAUNCH`, or `FUTURE HARDENING / P2`.
  Independent specialist review, HSM custody, multi-region scale drills and
  signed independent builds are not misrepresented as public-source blockers.
- Strict docs and docs-site redaction PASS. An accidental raw NAVSwap scan of
  all `/tmp/navswap-*` correctly found eight secret-path references in an old
  private operator directory; nothing was allowlisted or copied. The scanner
  regression and the intended `--repo-only` candidate scan then PASS with zero
  findings, as does the broader tracked-tree secret scan.
- The final exact commit/tree, toolchains, command/log hashes, SBOM, refs, file
  count and two sanitized-clone locations are sealed outside the public tree in
  `open_source_publication_candidate_20260717/ACCEPTANCE.json`. The honest state
  is **CODE AND STAGING COMPLETE / PUBLICATION BLOCKED ONLY ON P0-SECRET-01
  PROVIDER RECORD**. The repository has not been published.

## 2026-07-17T14:22:39Z — RC6 clean-clone fixture RED corrected without duplication

- The second non-shallow publication clone failed the complete workspace test at
  `legacy_nav_profile_register_block_3_receipt_id_matches_committed`: the test
  read `asset.batch.json` from an ignored operator-only `reports/` directory.
  The long-lived checkout masked the dependency; the clean clone proved it.
- The exact batch was already committed as
  `crates/node/testdata/wan-devnet-catchup-block-3/batch.json`. Commit
  `929b0f40` makes the execution compatibility test consume that canonical
  tracked fixture. No duplicate payload, production code, protocol behavior,
  dependency, or public claim was added.
- Pre-fix clean-clone evidence: `cargo test --workspace --all-targets --locked`
  stopped with missing-file error after 155 execution tests passed. Post-fix:
  the exact receipt-ID regression passes `1/1`; execution all-targets passes
  `156/156`; affected check and strict Clippy pass; source portability, tracked
  secret scan, formatting and diff checks pass.
- RC6 is invalid. A new exact candidate and both clean-clone/full release gates
  are required; no prior partial run is promoted as closure evidence.

## 2026-07-17T14:30:00Z — RC7 invalidated by stale audit-state labels

- A final reconciliation search found seven finding statuses and four executive
  summary cells still describing immutable-candidate, integration, WAN, or
  hosted-review evidence as pending. The closure table itself was correct, but
  the narrative contradiction violated the closure plan's stale-label gate.
- The audit now records the already-proven exact-candidate evidence for
  consensus recovery, signed governance, governed bridge destinations,
  owned-asset safety, signed deposit, native supply, FastPay recovery, docs CI,
  validator authority, and atomic DvP. Hosted external review is explicitly not
  a public-source prerequisite; production custody/audit/scale gates remain
  classified separately as real-value work.
- RC7's short workspace runs were interrupted and are not acceptance evidence.
  This is evidence reconciliation only; no runtime, protocol, dependency, test,
  artifact, public API, money path, or safety claim changed.

## 2026-07-17T15:30:01Z — RC8 clean-clone fixture and parallel-suite timing REDs corrected

- RC8 is invalid. Its second clean clone completed the 2,518-second real
  issued-asset Orchard proof test, then failed
  `wan_devnet_legacy_receipt_replay_accepts_tx_id_drift_only` because that
  compatibility test read a signed round-6 reserve batch from the ignored
  operator-only `reports/` tree. Raw evidence is
  `/tmp/postfiat-rc8-clone2-workspace-test.log`.
- The 12,241-byte public historical batch is now a canonical tracked test vector
  at
  `crates/node/testdata/wan-devnet-legacy-round-6-reserve-submit/asset.batch.json`.
  Commit `d5740cdc` points the replay test there. It does not add runtime code,
  a dependency, a secret, or a second generated payload; it promotes the exact
  signed compatibility input the test already required into testdata.
- The source workspace independently failed the wall-clock-only
  `quorum_early_path_ignores_a_four_second_late_sixth_response` assertion at
  5.61 seconds while the exact isolated test passed at 1.38 seconds. The
  quorum collector itself returned after five votes; parallel suite work made
  the fixed 3.5-second unit threshold an invalid proxy for whether the sixth
  response was awaited. Raw evidence is `/tmp/postfiat-rc8-workspace-test.log`;
  isolated diagnosis is `/tmp/postfiat-rc8-quorum-early-isolated.log`.
- Commit `d5740cdc` replaces the sleep race with a deterministic response gate.
  The sixth validator executes and durably applies prepare, commit and apply,
  but all three responses remain blocked. The wallet must still return a valid
  quorum terminal result with validator-5 replication pending; the test then
  proves exactly three gated response workers exist, releases them, waits for
  clean termination and verifies all six validators applied. This strengthens
  the behavior assertion and removes no correctness or release-performance
  gate.
- Focused post-fix evidence: legacy replay `1/1` PASS in 0.05 seconds;
  deterministic quorum-early regression `1/1` PASS in 1.75 seconds; node
  all-target strict Clippy, formatting, diff, public tracked-tree secret scan,
  artifact policy and public-source portability PASS. A fresh immutable
  candidate, staging export, second clone and complete battery remain required.

## 2026-07-17T15:42:48Z — RC9 stopped before proof tail for final plan-state reconciliation

- RC9 source/export/second-clone tree identity, 1,624-file count, intended refs,
  strict bare-repository integrity, tracked and reachable-history secret scans,
  deterministic 307-component SBOM, static inventories, docs, wallet `240/240`,
  proxy `24/24`, Python `139/139`, Foundry non-fork `103/103`, official-fork
  fail-closed negative, dependency, check, strict Clippy and fuzz gates were green.
- The completion audit then found the controlling P0 plan still used unchecked
  boxes for already-proven internal replay, recovery, simulation, customer-flow,
  staging and history-scan work. Leaving those stale would contradict its own
  checkbox semantics and the final closure plan.
- Both just-started RC9 workspace runs were interrupted before the proof-heavy
  node tail; they are not acceptance evidence. The controlling plan now marks
  all internally completed work and leaves exactly the provider-owner terminal
  action plus its private evidence record open. This documentation-only
  reconciliation changes no runtime, protocol, dependency, test, key, money or
  deployment state. A new exact candidate and full battery are required.
