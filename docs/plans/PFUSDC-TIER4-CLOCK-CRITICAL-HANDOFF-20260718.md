# pfUSDC Tier-4 Clock-Critical Closure Plan and Agent Handoff

**Date:** 2026-07-18
**Priority:** P0 / clock critical
**Execution mode:** continuous implementation to the four-gate Tier-4 protocol
acceptance boundary; controlled-testnet product hardening follows
**Canonical architecture reference:** `docs/plans/PFUSDC-TIER4-IMPLEMENTATION-PLAN-20260717.md`

## 1. Mission

Finish the pfUSDC Tier-4 protocol on the existing PFTL and Arbitrum path, then
carry the frozen protocol artifacts into the existing CLI and StakeHub product
path.

Tier 4 means both bridge directions are authorized by cryptographic finality
proofs, not by an observer or withdrawal-signing committee:

1. **USDC -> pfUSDC:** PFTL verifies a proof that a canonical vault deposit
   completed on Arbitrum and is contained in an Arbitrum assertion confirmed
   under finalized Ethereum state.
2. **pfUSDC -> USDC:** the Arbitrum vault verifies a proof that an exact burn
   and withdrawal packet were accepted in a finalized PFTL consensus-v2 block.
3. Relayers transport proofs but have no authority to fabricate, redirect, or
   duplicate value.
4. The Tier-4 route cannot fall back to observer or threshold authorization.

The original public tier ladder defines Tier 4 by the evidence accepted in both
bridge directions. It does not require frontend polish, private swaps, two
fresh-wallet demonstrations, or a broad production battery. This plan therefore
separates:

- **Tier-4 protocol implementation:** Gates 1-4. Both real proof directions work
  on an activated controlled target with no observer, threshold signer, mock
  verifier, or downgrade path.
- **Controlled-testnet product launch:** Gates 5-7. CLI/StakeHub integration,
  broad recovery batteries, rolling deployment, and demonstration evidence.

No percentage completes either boundary. Report core gates passed out of four,
then launch gates passed out of three. Gates 5-7 must not delay work on Gates
1-4.

## 2. Non-negotiable execution rules

### 2.1 Stay on the Tier-4 critical path

- Do not work on repository publication, FastSwap, FastPay, Cobalt, frontend
  redesign, a651 epoch work, generalized cleanup, or unrelated documentation.
- Do not start, poll, wait for, or inspect GitHub Actions on the Tier-4 critical
  path. In particular, do not run `gh run view`, create a background CI monitor,
  or treat public-repo CI as a protocol or launch gate. Local commands and
  captured local evidence are authoritative for this closure.
- Do not start an external or independent review gate. External review is not a
  prerequisite for completing and proving the devnet implementation.
- Keep architecture-plan edits limited to correcting stale protocol facts. This
  file is the live closure checklist.
- Do not report completion percentages. Report `gates passed / gates total`,
  the current failing command, and the next bounded action.

### 2.2 No random 45-minute validation loops

- Use targeted tests during implementation. A test must correspond to the
  component just changed and should normally return within five minutes on the
  existing warm build tree.
- Do not run `cargo test --workspace` after every edit or commit.
- Run the full local workspace gate exactly once for an immutable
  controlled-testnet launch candidate, after the four core gates and all
  targeted checks are green. It is launch hardening, not a reason to delay the
  Tier-4 protocol result.
- Run a second full workspace gate only if Rust source changes after that first
  release-candidate run. Documentation, evidence, manifests, or deployment
  records do not justify repeating it.
- Generate exactly one required ingress proof and one required egress proof from
  the frozen release-candidate guests. Do not generate proofs for benchmarking,
  progress theater, repeated validation, or deployment. Regenerate an ELF/proof
  only when that guest's Rust source changes, then record why the old artifact
  was invalidated.
- A long operation is allowed only when it directly produces a required
  artifact: an SP1 build/proof, the one release-candidate workspace battery, an
  Arbitrum fork battery, or a live deployment gate. Record its purpose before
  starting it.
- Do not run open-ended fuzzers or soak tests on the critical path. Property,
  fuzz, and invariant commands must have an explicit case or time bound and
  target the decoder/state transition just changed.
- If a command produces no useful progress for ten minutes, diagnose the
  process instead of blindly waiting. Do not repeatedly poll it from the
  conversational loop.

### 2.3 Safety boundaries

- Never accept block inclusion or six-node convergence as success without the
  literal accepted receipt code.
- Never replace proof verification with a trusted RPC assertion, operator
  signature, mock verifier, hash-only placeholder, or observer attestation.
- Never loosen consensus quorum, proof bounds, route bindings, finality rules,
  replay protection, or conservation checks to make a test pass.
- Never add an automatic fallback from the Tier-4 route to the observer or
  threshold route.
- A devnet reset is permitted only if the activated block encoding cannot be
  introduced by a versioned future-height transition. Finish local and fork
  gates first and batch all reset-requiring changes into one reset.
- Preserve the existing uncommitted Tier-4 work. Do not reset, checkout, clean,
  or overwrite it.

## 3. Exact handoff state

### 3.1 Worktree and branch

```text
worktree: /home/postfiat/repos/postfiatl1v2-public-main-verification-20260717
branch:   pfusdc-tier4-20260717
last verified code commit: 69a056f5 (host witness capture/audit)
V3 guest freeze commit:    0b68a5be71c80d1cdc89d12e5c7cfe77b1eb831f
base:     cc23185
remote:   https://github.com/postfiatorg/postfiatl1v2.git
public main observed 2026-07-18: 66de35034c46dabe46302e2abbeead23a438d3d0
```

There are 22 Tier-4 commits through the host capture commit. The corrected Nitro-output ingress is
committed at `ce511818eab246d59d3aed66e4628c5f9045d802`; the storage integration
fixture repair is committed at `887d98280bf9ff755966c322e156aaa1aee8794e`.
Public `main` is two CI-only commits ahead of the branch base
(`b4dab41f2de9`, `66de35034c46`). They do not change Tier-4 Rust or Solidity.
No dirty ingress correction remains. Integrate those CI-only commits only at a
normal clean-tree boundary, and do not run their GitHub workflows.

### 3.2 Implemented and committed

- `BridgeExitLeafV1`, canonical encodings, Merkle roots, and proof public values.
- Consensus-v2 binding of accepted bridge exits through `bridge_exit_root`.
- Block/replay/history/state/RPC plumbing for the exit commitment.
- Proof-native ingress operation fields and PFTL execution verification path.
- PFTL finality witness export and pure egress proof verification.
- Bounded checkpoint advancement and PFTL committee-rotation proof handling.
- Dedicated ingress and egress SP1 guest workspaces and generated ELF artifacts.
- Tier-4 proof builder/prover CLI foundations.
- `PFTLFinalityVerifierV1.sol` and `ERC20BridgeVaultV2.sol`.
- Solidity replay, wrong-binding, wrong-version, committee-progression, rejected
  receipt, atomic deposit-send, and exact-payment tests.

### 3.3 Corrected-ingress work — committed and preserved

```text
crates/execution/src/nav_vault_asset_execution.rs
crates/node/src/vault_bridge_workflows.rs
crates/types/src/pfusdc_tier4_types.rs
crates/types/src/tests.rs
programs/pfusdc-ingress/elf/pfusdc-ingress-program
programs/pfusdc-ingress/src/lib.rs
```

Commit `ce511818eab246d59d3aed66e4628c5f9045d802` replaces the initial ingress V1
receipt attachment with ingress V2:

- bind the confirmed Nitro assertion `sendRoot`;
- prove the exact output leaf and Merkle path;
- bind sender, destination, value, calldata, L2/L1 block numbers, timestamp, and
  output index;
- decode the exact canonical `recordDepositV1(...)` payload;
- bind deposit ID, depositor, recipient, amount, nonce, route, vault, and token;
- update canonical public values and the regenerated ingress ELF.

Do not revert this correction merely to recover an earlier green test. Its
Gate-1 ingress V2 ELF SHA-256 was
`04856ece0239146e6f3ce9ca191ef5ff6ce6c1fac42a0dfef719423539876fd7`.
Gate-2 code-proof closure changes the guest statement to V3, so that V2 ELF is
now explicitly invalidated and must not be proved or activated.

### 3.4 Gate-2 V3 closure — committed and frozen

Commit `0b68a5be71c80d1cdc89d12e5c7cfe77b1eb831f` supersedes the
pre-release ingress public values V2 with V3 before any Tier-4 activation:

- Nitro leaf and accumulator rules are pinned to Offchain Labs Nitro commit
  `a618155919315241665356fe60f3cd00d66d5e46`, with fixed item-hash/sendRoot
  vectors and the canonical final `index == 0` rule.
- Zero-sibling single-leaf proofs are accepted; indexes deeper than their proof
  path reject.
- The confirmed assertion block hash authenticates a canonically decoded L2
  header; its state root verifies exact vault and token account/code proofs.
- The Ethereum-finalized state root verifies both Rollup `latestConfirmed` and
  the parent-chain ingress-anchor account/code proof.
- The guest allowlists only the canonical Ethereum/Arbitrum mainnet pair or the
  canonical Sepolia/Arbitrum-Sepolia pair, including genesis root, fork
  schedule, Rollup proxy, and storage slot. Cross-network mixtures reject.
- Arbitrum Sepolia is the clock-critical controlled target. It provides real
  Ethereum/Nitro finality in minutes; a local fork cannot provide a valid
  Ethereum-finality witness, and a new Arbitrum One assertion normally cannot
  close inside this work window.
- `PfUsdcIngressPublicValuesV3` commits the Rollup code hash/slot, asserted L2
  state root, output sender, and ingress-anchor runtime code hash in addition to
  all prior deposit bindings.
- `PfUsdcIngressAnchorV1` is the production parent-chain destination and checks
  active Outbox, proof-derived L2 sender, route fields, recipient hash, and
  deposit replay if the Nitro message is executed.
- The anchor route binding is constructor-set, non-settable storage rather than
  an immutable. Making it an immutable creates a deployment hash cycle
  (`route profile -> verifier policy -> anchor runtime hash -> route binding ->
  route profile`). The bridge, vault, token, and chain ID remain bytecode-level
  immutables. The deployment manifest must record and read back the stored route
  binding; PFTL mint authorization independently verifies that same binding in
  the proof policy and public values.

Conformance record:
`docs/specs/pfusdc-nitro-sendroot-conformance.md`.
Local evidence:
`docs/evidence/pfusdc-tier4-gate2a-20260718T022015Z/`.

Host commit `69a056f5` adds two bounded commands without changing guest code:

- `ingress-capture` decodes the real deposit/ArbSys receipt, bootstraps a
  Helios light-client store from standard Beacon REST data, fetches the
  finalized Ethereum Rollup/anchor proofs, recovers the confirmed BoLD
  assertion, fetches asserted-L2 vault/token proofs, obtains the canonical
  NodeInterface outbox path, and writes a witness only after native V3
  verification succeeds.
- `ingress-audit` runs 21 named security-field mutations against that captured
  witness and requires every mutation to reject. SP1 proof-byte mutation and
  PFTL deposit replay remain execution-level cases after the one proof exists.

### 3.5 Current verified results

These targeted results were current through the frozen V3 source commit on
2026-07-18:

- Tier-4 type tests: **3 passed, 0 failed**.
- Ingress V2 public-value/finality-state type tests: **3 passed, 0 failed**.
- Pure egress proof tests: **4 passed, 0 failed**.
- Corrected ingress V3 library tests: **5 passed, 0 failed** when the library
  target is selected.
- Proof-native ingress execution test: **1 passed, 0 failed**.
- Targeted bridge-exit activation and egress-export node tests: **2 passed, 0
  failed**.
- Tier-4 Foundry tests: **11 passed, 0 failed**, including the production-anchor
  runtime-hash cycle regression test.
- Ingress capture/ABI/fork/mutation helper tests: **3 passed, 0 failed**.
- `cargo fmt --all -- --check`: **passed**.
- The earlier Gate-1 `cargo check --workspace --all-targets --locked` remains
  passed; V3 was checked only through its affected packages because the full
  workspace battery is reserved for the immutable launch candidate.
- Running the SP1 guest binary as a host unit test is invalid because
  `sp1_zkvm::io::read` only runs on the zkVM target. Use the library test target
  for unit tests and the SP1 toolchain for the guest execution/proof gate.

### 3.6 Gate-1 compile closure

The former compile failure was:

```text
crates/storage/src/lib.rs:1393
missing field `bridge_exit_root` in initializer of `BlockHeader`
```

Commit `887d98280bf9ff755966c322e156aaa1aee8794e` adds
`bridge_exit_root: None` to that legacy storage fixture. The complete workspace
all-target check now passes. There is no remaining Gate-1 compile blocker.

### 3.7 Live state

- The Tier-4 branch is not merged into current public `main`.
- The Tier-4 binary and contracts are not deployed.
- The active pfUSDC route is not Tier 4.
- Live ingress still depends on the old observer route.
- Live egress still depends on the old threshold-authorized route.
- No live Tier-4 fresh-wallet round trip has been completed.

## 4. Mandatory architecture closure before declaring ingress safe

The corrected `sendRoot` direction is promising but must close every item below.
Mock-only evidence is insufficient.

- [x] Prove against canonical Arbitrum Nitro encoding or official vectors that
      the output leaf hash, tree path, and `sendRoot` calculation are byte exact.
- [x] Prove that the output sender is populated by ArbOS from the executing L2
      contract and cannot be selected arbitrarily by calldata.
- [x] Bind the asserted L2 block hash to the confirmed rollup assertion under an
      Ethereum-finalized Rollup contract state proof.
- [ ] Bind the deployed Tier-4 vault and token runtime code at the asserted L2
      state through the canonical L2 header and account proofs. Bind the
      production ingress-anchor runtime code through an account proof at the
      Ethereum-finalized parent-chain state; it is not an L2 account. A route
      field that merely states an expected code hash is not a proof.
- [ ] Pin the production ingress-anchor address and its runtime code hash in the
      route profile. A test-only mock anchor is not sufficient.
- [x] Prove that `depositV2` measures the exact USDC balance delta, records the
      deposit, and emits/sends the canonical output atomically. A reverted token
      transfer or failed ArbSys send must create neither a valid output nor a
      mintable deposit.
- [x] Bind Arbitrum chain ID, Rollup address/code hash, confirmation slot,
      assertion hash, route epoch, vault, token, and all relevant runtime code
      hashes into the proof policy and public-values commitment.
- [x] Reject malformed and oversized output paths/calldata before expensive
      proof work or state mutation.
- [ ] Generate one real SP1 proof from a captured finalized Arbitrum witness and
      verify the exact proof in PFTL execution. Native/mock guest execution alone
      does not satisfy this item.

If any one of these bindings cannot be proven, ingress remains NO-GO and Tier 4
is not complete.

## 5. Binary acceptance gates

There are four Tier-4 protocol gates and three controlled-testnet launch gates.

- **Tier-4 protocol implemented:** Gates 1-4 are green (**4/4 core**).
- **Controlled-testnet product launch complete:** Gates 5-7 are also green
  (**3/3 launch**).

Do not describe 0/3 launch gates as blocking the Tier-4 protocol implementation.
Do not describe Tier 4 as live while its route is not activated.

### Core Gate 1 — integrated release candidate compiles

- [x] Preserve and finish the ingress V2 patch.
- [x] All direct initializers, codecs, RPCs, snapshots, replay, and history code
      understand the versioned `bridge_exit_root` and ingress V2 public values.
- [x] Formatting passes.
- [x] Targeted type, execution, node, proof, guest-library, and contract tests
      pass.
- [x] The six-file ingress correction is committed as one reviewable commit.
- [x] Confirm after the worktree is clean whether public `main` contains any
      Rust/Solidity changes that require integration. The two currently known
      CI-only commits require no code integration and do not justify running
      GitHub workflows.

**Gate evidence:** commit SHA, changed-file list, targeted command log, and zero
compile errors.

### Core Gate 2 — proof-verified ingress, no observer

- [ ] A real dust test-USDC deposit enters `ERC20BridgeVaultV2` on Arbitrum
      Sepolia and creates the canonical Nitro output. A fork-only receipt does
      not satisfy this gate because its modified state is not Ethereum-finalized.
- [ ] Ethereum finality, confirmed Rollup assertion, exact output path, exact
      vault code, exact token, exact route, exact recipient, amount, nonce, and
      deposit ID are verified by the ingress SP1 guest.
- [ ] PFTL execution verifies the real proof bytes with the route-pinned program
      vkey and exact public values.
- [ ] The deposit finalizes and credits exact pfUSDC with `code=accepted`.
- [ ] No observer attestation or observer signature is supplied or consulted.
- [ ] Wrong chain, checkpoint, assertion, output path, sender, destination,
      calldata, code hash, token, vault, recipient, amount, nonce, route, proof,
      and replay each reject before balance mutation.
- [ ] Restart/replay does not reopen the deposit ID.

**Gate evidence:** proof hash, public-values hash, program vkey, ELF hash,
accepted receipt, before/after balances, mutation-negative matrix, and replay
result.

### Core Gate 3 — proof-verified egress, no threshold signer

- [ ] An accepted `vault_bridge_burn_to_redeem` creates exactly one
      `BridgeExitLeafV1`; a rejected transaction creates none.
- [ ] The ordered exit root is bound into proposal, prepare, and precommit bytes
      for the activated block encoding.
- [ ] The exported witness proves a valid consensus-v2 precommit commit QC,
      distinct validators, correct committee root, parent/view ancestry,
      accepted receipt code, and exact Merkle path.
- [ ] Committee rotation is accepted only through a proof-verified finalized
      governance transition.
- [ ] A real egress SP1 proof is verified by the deployed
      `PFTLFinalityVerifierV1`, using the immutable program vkey.
- [ ] `ERC20BridgeVaultV2` pays exact USDC to the proof-bound recipient and
      consumes the withdrawal ID and proof nullifier atomically.
- [ ] No ECDSA threshold withdrawal bundle is created, requested, or accepted.
- [ ] Four-of-six, duplicate validator, wrong committee, bad ML-DSA context,
      proposal-only, prepare-only, rejected receipt, wrong leaf/path, altered
      packet, stale checkpoint, and replay all fail without value movement.

**Gate evidence:** finalized PFTL block/commit artifact, exit path, proof/public
values/vkey hashes, accepted EVM transaction, exact balance delta, nullifiers,
and negative matrix.

### Core Gate 4 — immutable route and contracts

- [ ] Deploy production, not mock, verifier/vault/anchor components on the pinned
      Sepolia/Arbitrum-Sepolia controlled target.
- [ ] Read back the anchor's constructor-set `governedRouteBinding` and require
      it to equal the manifest/route-profile binding. Its runtime hash is
      deliberately independent of this stored value to avoid the deployment
      hash cycle described in Section 3.4.
- [ ] Deployment manifest records compiler, optimizer, constructor arguments,
      creation/runtime bytecode hashes, program vkeys, initial checkpoints,
      chain IDs, vault, token, Rollup, anchor, and route epoch.
- [ ] Route profile pins both verifier kinds, proof bounds, program versions,
      vkeys, runtime code hashes, checkpoints, and activation height.
- [ ] Activate that exact route and block encoding on the controlled target;
      verify the already-generated ingress and egress artifacts bind to it.
      Do not regenerate either proof for activation.
- [ ] New Tier-4 work cannot downgrade to observer/threshold verification.
- [ ] Pause stops new work but cannot rewrite, redirect, duplicate, or fabricate
      an existing claim.
- [ ] Actual USDC balance deltas reject fee-on-transfer/rebasing behavior.
- [ ] The pinned Arbitrum fork suite and contract fuzz/invariant suite pass.

**Gate evidence:** deterministic manifests, deployed code-hash checks, route
profile hash, fork receipts, fuzz summary, and invariant summary.

### Launch Gate 5 — canonical CLI and StakeHub integration

- [ ] CLI bridge-in builds, verifies, submits, and resumes the ingress proof
      without an observer ceremony.
- [ ] CLI bridge-out exports the finality witness, proves, submits, claims, and
      resumes without a threshold-signature bundle.
- [ ] Commands are parameterized by wallet, route, amount, recipient, and RPC;
      no test wallet or address is hardcoded.
- [ ] StakeHub shows `Tier 4 - proof verified` only after live verifier kinds,
      vkeys, route hash, and runtime code hashes match.
- [ ] StakeHub shows source-finality age, proof generation time, settlement
      time, route epoch, accepted/rejected/unknown receipt state, and exact value
      movement.
- [ ] Rejected or unknown receipts never render as success.
- [ ] Existing NAV freshness, transparent swap, private swap, and private-egress
      surfaces continue to work.
- [ ] Retrying proof transport is idempotent and never repeats a deposit, burn,
      withdrawal, or claim.

**Gate evidence:** backend/API tests, browser screenshots, receipt-code proof,
and one GUI/API-driven proof operation.

### Launch Gate 6 — adversarial, restart, and conservation battery

- [ ] Complete the ingress and egress mutation matrices from Gates 2 and 3.
- [ ] Crash/restart prover, relayer, node, and StakeHub after every durable step;
      resume without repeating a money transition.
- [ ] Restart all six validators and replay to the identical state root.
- [ ] Partition/delay validators while proving PFTL finality; accept only a valid
      precommit commit QC.
- [ ] Complete both directions from a second relayer using only public artifacts.
- [ ] Rotate the PFTL committee and prove the first withdrawal after rotation.
- [ ] Rotate the route/program vkey through a new epoch without reopening replay.
- [ ] Reconcile `V = S + D + B - R` after each accepted operation and at terminal
      state.

**Gate evidence:** machine-readable battery matrix, restart roots, receipts,
proof/nullifier sets, balance snapshots, and conservation calculation.

### Launch Gate 7 — live six-validator Tier-4 demonstration

- [ ] Stage rollback artifacts.
- [ ] Perform one rolling validator deployment; after each node require six-node
      rejoin and identical tip/root before proceeding.
- [ ] Activate the exit-root encoding and Tier-4 route at explicit future heights.
- [ ] Confirm all six nodes report identical release, protocol version, route
      profile, vkeys, and activation heights.
- [ ] Complete two fresh-wallet round trips with different amounts/recipients:
      USDC -> proof ingress -> pfUSDC -> pfUSDC/a651 swap path -> pfUSDC -> proof
      egress -> exact USDC.
- [ ] Include one transparent swap and one private/warm-prover swap path.
- [ ] Every money step has `code=accepted`; all six validators agree in the
      untimed audit; exact balance deltas and conservation hold.
- [ ] No observer attestation and no threshold withdrawal signature appear in
      either run.
- [ ] Old-route creation is rejected after activation; explicitly pinned drain
      artifacts remain readable but cannot create new old-route work.

**Gate evidence:** two `ACCEPTANCE.json` files, receipts from both chains,
six-node status/root audit, proof and deployment hashes, before/after balances,
conservation, screenshots, and terminal gate record.

## 6. Clock-critical execution order

Timeboxes are escalation points, not permission to weaken a gate.

### Block A — stabilize current tree — COMPLETE

1. Add `bridge_exit_root: None` to the failing storage fixture.
2. Continue targeted compile checks and repair every mechanical initializer or
   codec mismatch caused by the versioned field/public-values change.
3. Run the already-green targeted Tier-4 tests and ingress library tests.
4. Run the Tier-4 Foundry test file.
5. Commit the complete ingress V2 correction. Do not include unrelated files.

**Exit:** Core Gate 1 is locally green on targeted checks. Commits:
`ce511818eab246d59d3aed66e4628c5f9045d802` and
`887d98280bf9ff755966c322e156aaa1aee8794e`.

### Block B — close real ingress proof (target: next 90 minutes)

1. Official Nitro/BoLD vectors and the asserted runtime-code proof statement are
   frozen at `0b68a5b`; do not reopen them without a concrete failing witness.
2. Freeze the deterministic Sepolia deployment manifest and route profile,
   including the constructor-set anchor route binding and all read-back hashes.
3. Fund the approved deployment wallet with testnet-only Ethereum Sepolia and
   Arbitrum Sepolia ETH, deploy the production parent-chain anchor and asserted-
   L2 verifier/vault, and submit one Circle test-USDC dust deposit.
4. Capture the finalized witness and run the bounded 21-case native mutation
   audit once.
5. Build the frozen ingress guest once, record ELF/vkey hashes, generate exactly
   one real proof, and verify it in PFTL execution with deposit replay rejection.

**Exit:** Gate 2 passes; otherwise record the exact cryptographic binding that
remains open. Do not substitute a mock or hash-only assertion.

### Block C — close real egress proof and contracts (target: next 90 minutes)

1. Generate a real PFTL finality proof from a finalized accepted burn.
2. Verify it through the production SP1 verifier contract, not the mock.
3. Prove exact payment, nullifier/withdrawal replay protection, committee
   progression, and every consensus negative case.
4. Freeze deterministic deployment manifests and route profile.

**Exit:** Core Gates 3 and 4 pass. At this point the Tier-4 protocol
implementation is 4/4 core gates green. Record that result before starting
product-launch work.

### Block D — controlled-testnet product wiring (post-core; target: next 60 minutes)

1. Replace old observer/signature steps in the canonical CLI for Tier-4 profiles.
2. Wire the existing StakeHub backend/API/dashboard to those CLI/backend paths.
3. Prove receipt semantics, proof-age/status display, idempotent transport, and
   no regression to NAV/swap/private surfaces.

**Exit:** Launch Gate 5 passes. This work does not block the 4/4 protocol result.

### Block E — one launch-candidate validation (post-core; target: next 60 minutes)

Run, once, against the immutable launch candidate. Do not invoke these commands
as debugging loops:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
/home/postfiat/.foundry/bin/forge test --root crates/ethereum-contracts -vv
```

Run SP1 guest/proof, fork, CLI, StakeHub, replay, and adversarial gates separately
with their exact evidence paths. Do not invoke GitHub Actions as a substitute.

**Exit:** immutable binary, ELFs, vkeys, contracts, route profile, and manifests.

### Block F — one controlled-testnet deployment event (post-core; remaining time)

1. Prefer a versioned future-height activation and rolling deployment.
2. If a reset is technically unavoidable, archive the old chain and perform one
   reset containing every required change.
3. Run Gate 6 recovery/conservation cases that require the final binary.
4. Run both Gate 7 fresh-wallet demonstrations.
5. Write the controlled-testnet launch record only after 3/3 launch gates are
   green in addition to the already-recorded 4/4 core result.

## 7. Efficient test matrix

During Blocks A-D, select only the smallest relevant target:

| Change | Immediate gate |
|---|---|
| Canonical types/encodings | Tier-4 filtered `postfiat-types` tests |
| Pure PFTL finality proof | `postfiat-pfusdc-proofs` tests |
| Ingress guest logic | ingress program **library** tests, then one SP1 execution |
| Egress guest logic | egress guest/SP1 execution for the affected witness |
| Execution verifier | filtered vault-bridge/Tier-4 execution tests |
| Block root/replay/history | filtered node/storage/consensus Tier-4 tests |
| Solidity verifier/vault | `PFUSDCTier4.t.sol`, then fork/fuzz at Gate 4 |
| CLI | affected CLI parser/workflow tests |
| StakeHub | affected backend/API/browser tests |

Do not use `cargo test --workspace` as a debugging command. It is the Block E
release-candidate gate.

## 8. Evidence contract

Write append-only evidence under:

```text
docs/evidence/pfusdc-tier4-<gate>-<UTC>/
  ACCEPTANCE.json
  commands.log
  hashes.json
  receipts/
  proofs/
  public-values/
  balances-before.json
  balances-after.json
  deployment-manifests/
  screenshots/
```

Every `ACCEPTANCE.json` must include:

- gate number and boolean result;
- exact commit SHA and dirty-worktree boolean;
- binary, contract, ELF, vkey, route-profile, and manifest hashes;
- chain IDs, genesis hash, route epoch, activation height, and contract addresses;
- proof/public-values/nullifier/withdrawal/deposit identifiers;
- accepted receipt codes, not just inclusion;
- before/after balances and conservation equation;
- exact test commands, exit codes, and durations;
- unresolved findings, with no percentage-based completion claim.

## 9. Current next actions

Core Gate 1 is complete. Continue without a repository-wide review:

1. Freeze the deterministic two-chain deployment manifest and route profile.
   The approved deployment wallet is
   `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0`; it currently has zero test ETH
   on both target chains, which is the only external deployment blocker.
2. Fund that wallet with testnet-only Ethereum Sepolia and Arbitrum Sepolia ETH,
   deploy/pin the production anchor on Ethereum Sepolia and verifier/vault on
   Arbitrum Sepolia, verify every constructor/read-back/code hash, obtain Circle
   test USDC, and submit one dust deposit.
3. Capture the finalized target witness using the V3 layout: Ethereum proofs for
   Rollup plus parent-chain anchor, and asserted-L2 proofs for vault plus token.
   Use `pfusdc-tier4-prover ingress-capture`; it refuses to write a witness that
   fails native verification.
4. Run `pfusdc-tier4-prover ingress-audit` once on that witness and retain its
   21-case JSON rejection report.
5. Pin the deployed addresses/code hashes and complete the proof-policy V2 and
   governed finality-state V2 bootstrap/route profile.
6. Build the frozen ingress guest once, invalidate the Gate-1 V2 ELF, and
   generate/verify the one required ingress SP1 proof.
   If the proof exposes a guest defect, fix it in a new commit and explicitly
   invalidate the prior ELF/proof.
7. Record Core Gate 2 evidence, then proceed directly to the one required egress
   proof for Core Gate 3.
8. Report status only as: current core gate, core gates passed out of four,
   exact blocker, last evidence path, and next bounded action. After 4/4 core,
   report launch gates separately out of three.

## 10. Completion statements

The valid protocol completion statement is:

> pfUSDC Tier 4 protocol implementation is 4/4 core gates green. A real
> finalized Arbitrum proof credited pfUSDC without an observer, a real finalized
> PFTL proof released exact USDC without a threshold signer, the route activated
> on the controlled target had no downgrade path, and the
> proof/vkey/ELF/contract/route hashes are bound
> in the core acceptance evidence.

The stronger controlled-testnet product-launch statement is:

> pfUSDC Tier 4 is 4/4 core and 3/3 launch gates green. Two fresh-wallet live round trips used real
> proof-verified ingress and egress, every money receipt was accepted, exact
> conservation held, six validators agreed, no observer authorized ingress, no
> threshold signer authorized egress, no downgrade path was available, and the
> terminal evidence manifests are hash-bound to the deployed release.

Do not withhold the 4/4 protocol result merely because product polish or the
broader launch demonstration remains open. Do not claim a controlled-testnet
product launch until the separate 3/3 launch gates are green.
