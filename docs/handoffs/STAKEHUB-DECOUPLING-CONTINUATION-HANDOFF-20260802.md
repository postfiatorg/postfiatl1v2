# StakeHub Decoupling Continuation Handoff

**As of:** 2026-08-02 UTC  
**Repository:** `/home/postfiat/repos/a666-eth-fast-lane-combined-20260724`  
**Branch:** `feature/pnok-private-fix`  
**HEAD and upstream:** `a65792d4af4f441629eb0747a76549b34596cb93`  
**Primary plan:**
`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md`  
**Work status:** stopped at a safe local checkpoint; no live route, validator,
Ethereum, wallet, or customer-fund mutation occurred in this continuation.

## 1. Executive status

Phases 0 through 3 of the decoupling plan were already substantially complete
at the starting commit. The browser wallet, public proxy, node runtime, generic
signer, provider-neutral public-values ABI, open proof kit, and controlled qNAV
lifecycle were previously qualified.

This continuation audited the remaining A666-specific path and found a concrete
gap that prevents the public proof kit from constructing the same reserve
packet shape that live A666 needs:

- consensus already supports adding the PFTL-accounted pfUSDC subscription
  overlay to proof-verified external reserves;
- the public `postfiat-reserve-proof packet build` CLI could only construct a
  proof-only packet; and
- an external reserve-proof operator therefore could not construct an A666
  packet whose source root and verified assets include that overlay.

A narrow uncommitted patch now adds that capability, centralizes the immutable
composite-root formula in `postfiat-types`, makes consensus use the shared
helper, and adds tests. Those tests pass. The patch has not been committed or
pushed.

The overall plan is **not complete**. A new A666 successor profile has not been
registered, governed, activated, or exercised live. The current production
A666 route remains unchanged on its historical proof lineage.

## 2. Exact repository and process state

At handoff:

- branch and upstream both point to
  `a65792d4af4f441629eb0747a76549b34596cb93`;
- the latest exact-tip remote CI was green before this local patch:
  <https://github.com/postfiatorg/postfiatl1v2/actions/runs/30724759974>;
- there are three modified tracked code files listed in section 3;
- this handoff is one new untracked documentation file;
- `git status --porcelain` reports 344 other untracked paths, primarily old
  deployment and evidence output;
- no Cargo test, check, formatter, reserve-proof, or SP1 proving process was
  left running by this continuation; and
- no local commit was created.

Do **not** run `git add .`, bulk-clean the worktree, delete untracked evidence,
or assume the untracked deployment trees belong to this task. Stage only
explicitly reviewed files.

The frozen Monday-demo checkout is separate:

```text
/home/postfiat/tmp/a666-pfusdc-monday-demo-2246d257
```

Do not modify, rebase, clean, or use that checkout as a migration sandbox.

## 3. Uncommitted code patch

### 3.1 `crates/types/src/nav_reserve_public_values.rs`

Added:

```rust
nav_reserve_subscription_composite_source_root_v1(...)
```

This helper owns the exact consensus construction for a reserve proof plus a
PFTL-accounted NAV-subscription overlay. It:

- validates the canonical reserve public values;
- requires a 48-byte lowercase-hex overlay source root;
- rejects a zero overlay value;
- uses checked addition for total verified net assets;
- hashes the canonical public-values encoding; and
- derives the versioned composite source root under
  `postfiat.nav_reserve_subscription_composite_source_root.v1`.

The stable test vector currently asserts:

```text
overlay root: 0b repeated 48 bytes
overlay value: 75
expected root: de231528d015f5f4b7290b59837e343d13fc4b392dcd0beab8d87ba2959470c2173a413ae94b2c53dc2a16af21063ab4
```

The test also covers zero value, malformed root, and `u64` overflow.

### 3.2 `crates/execution/src/nav_sp1_verifier.rs`

Removed the duplicate inline composite-root implementation and delegated to
the shared types helper. The error remains mapped into deterministic verifier
failure; there is no new dependency or unbounded input path.

This refactor is consensus-sensitive because the resulting root is part of the
accepted reserve packet. Preserve the hard-coded vector and add equivalence or
context tests before changing the formula.

### 3.3 `tools/nav-reserve-proof/crates/reserve-proof-cli/src/main.rs`

Extended `PacketTemplateV1` with backward-compatible optional fields:

```json
{
  "subscription_overlay_source_root": null,
  "subscription_overlay_value": 0
}
```

When an overlay root is supplied, `packet build` now:

1. requires canonical 48-byte lowercase hex;
2. requires a nonzero overlay value;
3. requires the decoded public values to round-trip to the exact supplied
   canonical bytes;
4. derives the composite root with the shared consensus helper;
5. checks that the packet template supplied that exact root; and
6. sets `verified_net_assets` to proof assets plus the checked overlay value.

When the fields are absent/defaulted, the original proof-only behavior is
preserved. Tests cover both shapes and a composite-root mismatch.

### 3.4 Verification already run against this patch

The following passed on 2026-08-02:

```text
cargo test -p postfiat-types nav_reserve_subscription_composite_root_has_stable_vector_and_bounds
  1 passed

cd tools/nav-reserve-proof
cargo test --locked -p postfiat-reserve-proof
  6 passed

git diff --check
  passed
```

An earlier run in the same continuation also passed the same six CLI tests.
Do not claim full root-workspace or full execution regression coverage for the
current uncommitted patch; that has not been established at this checkpoint.

## 4. Important audit findings

### 4.1 The previously committed A666 shadow is retired

The existing directory is historical and explicitly retired:

```text
tools/nav-reserve-proof/qualifications/a666-shadow-20260730
```

It was built against the old verification key and must not be registered or
activated as the successor.

Old shadow vkey:

```text
0x007e3267...
```

Current canonical Docker-built reserve-proof vkey:

```text
0x000c7271e0711abce0c61d293222fd4a144599a779db8cadadc4df35e31a4100
```

The successor manifest, profile registration, profile ID, observations,
proofs, and packets must be regenerated against the current canonical program
identity.

### 4.2 Required A666 authorities still exist

Owner-only issuer and reserve-operator key files were located. Their contents
were not printed or copied.

Issuer address `pffcb93d...`:

```text
/home/postfiat/.pft/a666-uniswap-bridge-build-20260723/runtime/validator-0/faucet_key.json
/home/postfiat/.pft/venue/orchard-mirror/faucet_key.json
```

Reserve operator address `pfd0c86d...`:

```text
/home/postfiat/.pft/a666-uniswap-bridge-build-20260723/runtime/validator-0/navswap-fast-live/orchard-a651-h78/reserve-key.json
/home/postfiat/.pft/venue/orchard-mirror/navswap-fast-live/orchard-a651-h78/reserve-key.json
```

The `a651` path segment is historical naming. A651 no longer exists; do not
infer that A666 should be replaced or that the key authorizes a new asset.

Never place these files or their contents in the repository, command output,
evidence, or a new temporary directory.

### 4.3 New reserve-attestor key

A new encrypted Ed25519 reserve-attestor key was created outside the repo:

```text
/home/postfiat/.pft/a666-reserve-proof-v1
```

Directory mode is `0700`; files are `0600`:

```text
attestor.pass
attestor.pkcs8.pem
attestor.public.pem
attestor.public.hex
```

Public-key SHA-256:

```text
df7246a59e3868ee4a04607bda1c18001cb8013e51b3ee15ca85677c71b0dc5a
```

Verifier commitment:

```text
22b5a0cee988940c2569a933496bbbfb3ce3d684b45ed5949f0cd6ad65b2dda5957d17215bdf78bd08fcef21e12613d6
```

The old shadow attestor private key was not retained. Before any live
activation, arrange an explicit secure off-host backup of the new encrypted
private key and passphrase. Do not claim production-safe key custody while the
only known copy is on this host.

### 4.4 Two real historical reserve-observation epochs are available

These can drive provider-neutral multi-epoch shadow reproduction. They are
genuine distinct source observations, but they do **not** replace a fresh
current observation for a live packet.

#### 2026-07-28 / historical NAV epoch 2

Primary inputs:

```text
docs/evidence/a666-variable-size-nav-roundtrip-20260728/stakehub-nav-mark/stable-policy-preview/aggregate-witness-report.json
docs/evidence/a666-variable-size-nav-roundtrip-20260728/stakehub-nav-mark/nav-epoch-2/live-nav-mark-manifest.json
```

Reconciliation values:

```text
policy hash: 076c... (read the full value from the manifest)
base proof net assets: 2,825,975,143,580
pfUSDC overlay:           20,400,000,000
total net assets:      2,846,375,143,580
circulating supply:       31,590,197,455
NAV per unit:                 90,103,113
observation interval: heights 424-427; observed height 426
```

Source gross/liability values:

```text
aave:          57,251,478,898 / 20,084,880,217
evm spot:      70,555,612,972 / 0
hyperliquid: 1,811,301,046,401 / 0
near:         800,717,712,526 / 0
solana:       106,234,173,000 / 0
xmr:                        0 / 0
```

#### 2026-07-30 / historical NAV epoch 3

Primary inputs:

```text
docs/evidence/a666-pfusdc-reserve-demo-20260730/live-run-01/por-preissue/aggregate-witness-report.json
docs/evidence/a666-pfusdc-reserve-demo-20260730/live-run-01/por-preissue/nav-epoch-3/live-nav-mark-manifest.json
```

Reconciliation values:

```text
base proof net assets: 2,826,373,076,806
pfUSDC overlay:           11,299,585,500
total net assets:      2,837,672,662,306
circulating supply:       31,489,197,455
NAV per unit:                 90,115,750
observation interval: heights 546-548; observed height 547
```

Source gross/liability values:

```text
aave:          57,542,112,446 / 20,088,300,169
evm spot:      70,781,930,347 / 0
hyperliquid: 1,789,640,753,296 / 0
near:         820,450,032,886 / 0
solana:       108,046,548,000 / 0
xmr:                        0 / 0
```

Historical epoch 4 reuses the same base reserve proof with a changed pfUSDC
overlay. Do not count it as a third independent source-observation epoch.

The historical directory and schema names may retain StakeHub for evidence
integrity. New manifests, domains, executable tools, and operator instructions
must be provider-neutral.

## 5. What remains, in the required order

### Step 1 — finish and commit the narrow packet-builder patch

1. Review the three-file diff adversarially for consensus equivalence.
2. Document the two optional overlay fields in
   `tools/nav-reserve-proof/README.md`.
3. Add or update a public packet-template fixture that demonstrates the overlay
   without making A666 identity a generic CLI constant.
4. Run formatting checks, the full `postfiat-types` suite, relevant execution
   context tests, all proof-kit tests, and `git diff --check`.
5. Stage only the three code files, explicit documentation/fixture changes,
   and this handoff; commit and push a reviewable checkpoint.
6. Confirm remote CI at the new exact commit.

Do not combine a live governance transaction with this code checkpoint.

### Step 2 — build a repeatable provider-neutral A666 shadow generator

Create a reviewed script under the open proof kit or `scripts/` that accepts
explicit inputs and produces, without importing or calling StakeHub:

- the canonical current program identity;
- the provider-neutral A666 source manifest;
- valuation policy and valuation unit;
- the new attestor public-key commitment;
- immutable profile registration and derived profile ID;
- per-epoch canonical source observations and disclosure commitments;
- witness/context files;
- signed canonical attestor statements;
- executed public values;
- Groth16 proof/calldata; and
- proof-only and pfUSDC-overlay packet operations.

Use the open CLI wherever it already owns a canonical encoding or hash domain.
Do not reimplement domains in shell. Sign canonical statement bytes using the
encrypted key outside the proof-kit process, for example with OpenSSL
`pkeyutl -sign -rawin`; never echo the passphrase or private key.

The generator must be deterministic except where the proof system is
intentionally randomized, must refuse overwrites, and must emit a provenance
manifest containing input hashes and tool/commit identities.

### Step 3 — complete multi-epoch shadow qualification

1. Translate the two historical observation sets in section 4.4 into the new
   provider-neutral format.
2. Execute each witness on CPU and verify exact reconciliations.
3. Produce CPU Groth16 proofs for both epochs and independently verify them.
4. Build overlay-aware packet operations with the new CLI.
5. Confirm tamper rejection for asset, chain genesis, profile, vkey, policy,
   manifest, overlay source root/value, time interval, totals, and proof bytes.
6. Record the proof/profile/public-value hashes and reconciliation report.

This satisfies historical multi-epoch shadow evidence. A current live packet
still requires current source observations, current supply, and current
pfUSDC subscription state.

Accelerated proving is an operational acceleration gate. This host has no CUDA
device and no authenticated network-prover credential. Per the controlled
pre-testnet mandate, CPU proof success is sufficient for correctness and
reproducibility; label acceleration separately rather than blocking controlled
engineering.

### Step 4 — controlled six-validator migration rehearsal

On an isolated project-controlled six-validator environment:

1. register the immutable successor profile;
2. submit and finalize an overlay-aware reserve packet;
3. govern the A666-shaped route to the successor profile and packet;
4. verify all validators have identical state roots;
5. execute transparent issue and redeem;
6. execute private-middle issue and redeem;
7. export the wrapped asset through the generic constrained signer;
8. return it and restore the native asset;
9. test replay, restart, snapshot restore, stale packet, wrong profile, and
   overlay mismatch rejection;
10. verify conservation and historical profile/packet queryability; and
11. rehearse pause/rollback using governed operations, never state edits.

The currently running pNOK controlled environment was observed to use an older
commit/profile and is not proof of this successor migration. Rebuild or start a
clean environment pinned to the patch commit.

### Step 5 — prepare live A666 activation

Before any live mutation, require all of the following:

- green exact-tip CI and release artifacts;
- successful multi-epoch shadow reconciliation;
- successful six-validator migration and rollback rehearsal;
- secure off-host attestor-key backup;
- fresh current reserve observations;
- current finalized A666 profile/packet/route/supply/pfUSDC-overlay state read
  independently from every validator;
- a new packet and profile whose hashes are reproduced independently;
- explicit governance operation bundle with preconditions;
- staged pause and rollback bundle;
- generic signer readiness and correct Ethereum chain/contract/selector
  policy; and
- an evidence directory created before submission.

Then, and only then:

1. register the new immutable profile;
2. submit and finalize the fresh reserve packet;
3. govern the existing A666 route to the successor;
4. verify fleet convergence;
5. run small transparent issue/redeem;
6. run small private issue/redeem;
7. run Ethereum export and return;
8. verify balances, supply, reserve accounting, packet freshness, state roots,
   replay rejection, and historical records; and
9. leave the route active only if every gate passes. Otherwise pause affected
   operations and execute the rehearsed governed recovery.

Do not create a new NAVCoin merely because the proof profile changes. The plan
requires an immutable successor **proof profile for A666**, not A667 or a
replacement asset.

### Step 6 — public/operator reproduction

For controlled-testnet readiness, perform the full workflow from a clean
public checkout using project-controlled machines and an asset manifest
different from A666. This is normal engineering qualification and must not be
blocked on recruiting an outside institution.

The plan's unaffiliated-operator requirement is a later public-credibility
gate. Keep it visibly open until someone without internal filesystem/API/tool
access reproduces the lifecycle. Do not describe that external participation
as a blocker to the code, controlled network, or A666 migration rehearsal.

## 6. First commands for the next agent

Start read-only:

```bash
cd /home/postfiat/repos/a666-eth-fast-lane-combined-20260724
git status --short --branch
git rev-parse HEAD
git diff --check
git diff -- \
  crates/types/src/nav_reserve_public_values.rs \
  crates/execution/src/nav_sp1_verifier.rs \
  tools/nav-reserve-proof/crates/reserve-proof-cli/src/main.rs
```

Read before editing:

```text
docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md
tools/nav-reserve-proof/README.md
tools/nav-reserve-proof/qualifications/a666-shadow-20260730/README.md
crates/types/src/nav_reserve_public_values.rs
crates/execution/src/nav_sp1_verifier.rs
tools/nav-reserve-proof/crates/reserve-proof-cli/src/main.rs
```

Minimum immediate verification:

```bash
cargo fmt --all -- --check
cargo test -p postfiat-types
cargo test -p postfiat-execution nav_reserve_public_values

cd tools/nav-reserve-proof
cargo fmt --all -- --check
cargo test --locked
cargo check --locked -p postfiat-reserve-proof --features sp1
```

Search for exact execution-context test names before deciding whether the
filtered execution command is sufficient. Run the full execution suite before
merging or producing a release.

## 7. Safety and accuracy rules

- Do not call the historical shadow "current" or "activatable."
- Do not call historical epoch 4 a fresh independent source observation.
- Do not claim the current uncommitted patch has passed full CI.
- Do not touch the frozen Monday-demo worktree.
- Do not mutate live A666 while building or testing the successor.
- Do not rotate, print, copy, or commit issuer/operator/attestor secrets.
- Do not silently reinterpret the deployed proof profile; register a versioned
  immutable successor.
- Do not edit validator state to recover a failed migration.
- Do not delete historical profiles, packets, evidence, or provider-named hash
  preimages.
- Do not reintroduce StakeHub into the public wallet, proxy, node runtime,
  relays, proof kit, or current operator runbooks.
- Do not confuse provider-neutral execution with cryptographic truth about all
  external data. Trust classes must remain explicit.
- Do not let the later unaffiliated-operator credibility gate block controlled
  testnet engineering.

## 8. Honest completion boundary

At this handoff, the codebase has a tested local fix for the missing A666
overlay packet-construction capability. It does **not** yet have a regenerated,
multi-epoch-qualified, governed, live A666 successor profile.

The decoupling objective is complete only after the successor is qualified and
activated without rewriting history, all transparent/private and Ethereum
round trips pass against it, the public workflow is reproducible from a clean
checkout, and the remaining public-credibility gate is accurately classified.
