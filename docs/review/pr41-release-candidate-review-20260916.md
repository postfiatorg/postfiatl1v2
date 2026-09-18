# Combined release candidate correctness review — 2026-09-16

## Scope

Read-only code-quality review of `release/combined-devnet-20260915` at
`15126ac3408f3afe58cf4e4ed4c31d00efa04366` (`15126ac3`), using the supplied merge
base `faff0e53`. Qualified node source: `1c435f4f`. Fetched main was `3e56203a`;
candidate additions were distinguished from later main-only changes using the
supplied baseline.

Reading and Rust verification used the detached disposable worktree
`/tmp/pr41-review-20260916`. The candidate branch, its retained artifacts and
the other operator's checkout were unchanged. No Task Node or fleet action
occurred. This document records observations and suggested changes only.

The review read the production changes in `39688166`, `7603439d`, `5b1093fa`
and `1c435f4f`, the closing changes in `15126ac3`, and selected qualification
material from `67aa3ce6`. It traced FastPay verification, direct application,
acknowledgements, committee installation and V1/V2 commitments; withdrawal
release selection and reproduction workflow; Arc route authorization, finality
state, deposit admission and guest verification; and selected candidate changes
in source-specific settlement custody, supply accounting, archive execution,
state commitments and RPC status caching. The full generated-log diff in
`67aa3ce6` was not read line by line.

Completed local verification, with `CARGO_TARGET_DIR` inside the disposable
worktree, two build workers, incremental compilation disabled and test debug
information disabled:

| Command | Result |
| --- | --- |
| `cargo check --workspace --locked` | Passed |
| `cargo test -p postfiat-types fastpay_recovery_type_tests --locked` | 9 passed; 0 failed; 0 ignored |
| `cargo test -p postfiat-execution owned_transfer_recovery_tests --locked` | 8 passed; 0 failed; 0 ignored |
| `cargo test -p postfiat-node --lib v2_installation_binds_certificates_retained_under_v1 --locked` | 1 passed; 0 failed; 0 ignored |
| `cargo test -p postfiat-node --lib archive_bridge_supply --locked` | 1 passed; 0 failed; 0 ignored |
| `target/review-arc-probes review_arc_ --nocapture` | 2 observations reproduced |
| `target/review-fastpay-probe --nocapture` | 1 observation reproduced |

The three observation probes were compiled from standard input with
`rustc --edition=2021 --test`, linked against the candidate's freshly built
libraries, and reused existing fixture helpers. They confirmed the behaviours
below; they were not fixes or additional committed tests. No candidate source
file was edited. No full test suite was run.

## Findings

### 1. P1 — Arc bootstrap trust state is outside the signed route authorization

**File and line at `15126ac3`:**
[`crates/types/src/shielded_bridge_governance.rs:853`](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/types/src/shielded_bridge_governance.rs#L853).
Supporting paths: `crates/node/src/consensus_artifacts.rs:2761` and `:3070`,
`crates/node/src/execution_actions.rs:1618`.

**Condition:** a route activation retains its authorized profile and amendment,
but its `arc_finality_bootstrap.validator_set_commitment`, `latest_block_hash`
or `latest_block_height` differs from the state the authorizers intended.

**Observed behaviour:** activation validation compares the route identity,
epoch, addresses, code hashes and route binding, but does not bind these three
initial trust fields to the amendment. The amendment kind commits the profile;
the governance authorization signs that amendment. A focused probe changed the
validator-set commitment and block hash to different valid-length values and
the height to 1. Both activations validated with identical profiles and entire
amendments. Execution installs the supplied bootstrap, which subsequent Arc
proofs use as their trusted validator set. A proposer can therefore submit a
different initial trust state under the same route authorization. Consensus
ordering commits the selected batch but does not establish separate governance
authorization for those changed fields.

**Expected behaviour:** the signed activation authorizes the exact initial Arc
validator set and checkpoint as well as the route profile.

**Suggested minimal change:** bind a canonical digest of the complete bootstrap
into a versioned activation authorization and verify that binding before
installation. Preserve historical authorization encodings explicitly. Add
regressions that change each initial trust field while retaining the original
authorizations and require rejection without ledger mutation.

### 2. P1 — FastPay accepts certificates that the V2 commitment cannot encode

**File and line at `15126ac3`:**
[`crates/types/src/fastpay_recovery_types.rs:958`](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/types/src/fastpay_recovery_types.rs#L958).
Supporting paths: `crates/execution/src/owned_transfer_recovery.rs:174` and
`:394`, `crates/node/src/fastpay_recovery_node.rs:808`,
`crates/node/src/state_commitment.rs:1719`.

**Condition:** a certificate contains a valid owner authorization and validator
quorum, plus enough distinct unknown voter entries to exceed 128 total votes.

**Observed behaviour:** verification skips unknown voters and imposes no
matching vote-count limit. A focused probe appended 125 unknown voters to four
valid validator votes. Transfer application succeeded and retained the
129-vote certificate; its V1 fence commitment succeeded, while its V2 commitment
returned `FastPay retained certificate exceeds the validator bound`. The
node's direct-application path writes the resulting ledger before producing
its V1 acknowledgement. Consequently, accepted local state can become
unencodable under V2. A certificate retained under V1 can also prevent the
first V2 installation from computing its state root. The probe exercised
execution and commitment encoding; persistence was traced in code rather than
reproduced through a running node.

**Expected behaviour:** newly accepted certificates satisfy the commitment
bounds before any durable mutation or acknowledgement. Previously admissible
retained certificates remain covered by an explicit compatibility rule.

**Suggested minimal change:** share certificate count and shape validation
between verification and commitment encoding, applying it before direct
application, reveal retention and persistence. Define handling for previously
accepted over-bound certificates at the V2 transition without silently changing
historical bytes. Cover transfer and unwrap at 128 and 129 entries, including
unknown voters and installation over retained V1 state.

### 3. P2 — Arc finality advancement prevents independent deposits at one height

**File and line at `15126ac3`:**
[`crates/types/src/pfusdc_tier4_types.rs:309`](https://github.com/postfiatorg/postfiatl1v2/blob/15126ac3408f3afe58cf4e4ed4c31d00efa04366/crates/types/src/pfusdc_tier4_types.rs#L309).
Supporting paths: `crates/execution/src/nav_vault_asset_execution.rs:1404` and
`programs/pfusdc-arc-ingress/src/lib.rs:245`.

**Condition:** two distinct valid vault deposits occur in the same finalized
Arc block, or a newer deposit is submitted before an older unclaimed deposit.

**Observed behaviour:** accepting one deposit advances the route-wide height.
Every subsequent proof at that height or below is rejected, even when it
identifies a different deposit. The guest binds the receipt to the exact
source block, so the remaining deposit cannot simply be assigned a newer
height. A focused public-values probe accepted the retained corrected deposit,
then changed the deposit ID and nonce while retaining the same block and
validator set. The second call failed with the monotonic-finality error. This
probe did not generate a second SP1 proof. The existing deposit-admission path
already checks whether the deposit ID has been proposed.

**Expected behaviour:** finality advancement and per-deposit replay protection
allow each distinct deposit in an authenticated block to be processed once,
without making other deposits depend on submission order.

**Suggested minimal change:** separate authenticated checkpoint advancement
from deposit consumption. Permit additional receipts from an already
recognized checkpoint using its authenticated block hash and appropriate
validator-set history, while retaining deposit-ID uniqueness. Define bounded
handling for older finalized receipts. Merely replacing `<=` with `<` is
insufficient when validator sets change. Add same-block and out-of-order
deposit regressions, alongside duplicate-deposit and wrong-block rejection.

## Packet checks

Read-only checks of `deployments/release-repair-20260916/` found no mismatch in
the requested packet comparisons:

- `sha256sum --check SHA256SUMS`: **79 of 79 files match**.
- The raw workspace log contains 84 result groups totaling **1,433 passed,
  0 failed and 39 ignored**, matching the README and `qualification.json`.
- All six `history/post-v2-validator-*.stdout.json` reports have
  `verified: true` and a verified block log through **block 1021**. Their tip
  hashes and state roots match each other, `history/post-v2-run.json` and
  `qualification.json`; all six recorded exits are zero and stderr files empty.
- Qualified-source references in the qualification, software, history,
  rollback, measurement, node-build and proof-identity reports match
  **`1c435f4fb482ea7830bee5c7018d370a10a34bfd`**. The proof workflow separately
  identifies `5b1093fa`, consistently with its stated earlier CI scope.
- The six earlier-repair reports and old-binary checkpoint reports agree at
  block 1020; the final-candidate rollback report agrees with that same tip and
  state root. Four nested proof-report checksums also match after resolving
  their retained CI absolute paths to the packet's corresponding filenames.

These are packet consistency checks. The six saved-node replays and historical
proof builds were not rerun in this review.

## Areas read with no findings

- Apart from the bound mismatch above, the `1c435f4f` vote-order repair sorts a
  clone for the V2 commitment, preserves stored arrival order, keeps the wire
  encoder strict and rejects duplicate validators. Existing certificate
  digests already sort voters. V1 acknowledgements retain their historical
  encoding. Ordered V2 committee installation selects the new commitment for
  retained records immediately; its future admission height remains separate.
- `39688166` checks the selected release's ELF digest before egress/checkpoint
  execution and its derived verifying key before proving. Both paths compare
  guest output with native expected public values. The `7603439d`/`5b1093fa`
  reproduction changes apply the specified historical source reversal and
  retain byte comparisons against the release identities. Standalone prover
  tests and proof reproduction were not run.
- The archive execution change supplies Orchard balances to the same executor
  as live execution. The focused activation regression passed. The two older
  reserve-packet exceptions remain restricted by exact genesis, block,
  transaction, receipt and state-root identities.
- Selected source-specific settlement paths use checked arithmetic, separate
  source principal/spread/escrows and restrict pooled redemption to pooled
  reserves. The added custody fields enter state commitments. Comparison of
  the governance encoding extraction found no substantive change to the moved
  functions. This was not a complete settlement accounting review.
- The RPC status-cache change bounds the age of the report itself, including
  transactional storage updates, and publishes a refresh timestamp only after
  a successful read.

## Not reached

The review was closed early at the user's request. Remaining work includes a
complete read of the generated evidence/log diff in `67aa3ce6`; exhaustive
candidate-only node, types and execution review; full Arc guest and host-capture
review; pfETH ingress, Ethereum contracts, Arc conformance and the remaining
prover code; standalone egress-identity tests; independent proof reproduction;
and exact saved-chain replay. No full workspace test suite, deployment,
governance activation or fleet operation was performed.

## Repair result

Source repairs on `integrate/main-into-combined-20260918`, 2026-09-18:

- **Finding 1 (P1): repaired in
  [`6849d976`](https://github.com/postfiatorg/postfiatl1v2/commit/6849d97691039190b0ed646c86e79def65646eaf).**
  New Arc activations require V2 authorization: the signed amendment kind binds
  a domain-separated digest of the complete canonical bootstrap. Installation
  and retained route authorization check the binding. Explicit archive paths
  preserve historical V1 authorizations and record encodings. Regressions
  independently change the validator set, checkpoint hash and checkpoint
  height, require rejection without ledger/governance mutation, and show that
  recomputing the digest invalidates the original signatures. The existing Arc
  activation test now uses V2; the proof fixture initializes the new optional
  record field to `None`. Historical evidence files are unchanged.
- **Finding 2 (P1): repaired in
  [`397b4f53`](https://github.com/postfiatorg/postfiatl1v2/commit/397b4f53ac1f668efc24ba69feda5fc02f05f252).**
  Acceptance and encoding share certificate count/shape validation; acceptance
  also rejects unknown voters. Transfer and unwrap regressions accept 128
  voters and reject 129, unknown, empty and duplicate voters before direct
  application or reveal mutation. Arrival order remains supported. Historical
  V1 bytes remain unchanged; V2 installation over an old over-bound retained
  reveal or fence rejects atomically, without trimming or rewriting history.
  Existing FastPay fixtures are unchanged.

Verification used Rust 1.95.0, `CARGO_TARGET_DIR=/tmp/integrate-20260918-target`,
two build workers, incremental compilation disabled, and dev/test debug
information disabled. All listed commands passed; counts are per invocation.

| Command | Result |
| --- | --- |
| `cargo check --workspace --locked` | Passed |
| `cargo fmt --all -- --check` | Passed |
| `cargo test -p postfiat-types --lib arc_ --locked` | 5 passed |
| `cargo test -p postfiat-node --lib vault_bridge_governed_route --locked` | 15 passed; 2 existing Foundry/Anvil tests ignored |
| `cargo test -p postfiat-consensus-cobalt --lib governance_accepts_only_canonical_vault_bridge_route_authority_kinds --locked` | 1 passed |
| `cargo test -p postfiat-types --lib fastpay_recovery_type_tests --locked` | 9 passed |
| `cargo test -p postfiat-execution --lib owned_transfer_recovery_tests --locked` | 11 passed |
| `cargo test -p postfiat-node --lib fastpay --locked` | 19 passed |
| `cargo test -p postfiat-pfusdc-proofs --lib exact_finalized_egress_witness_accepts_and_binds_every_boundary --locked` | 1 passed |
| `cargo test -p postfiat-types --lib --locked` | 149 passed |
| `cargo test -p postfiat-execution --lib --locked` | 205 passed |
| `.venv-docs/bin/mkdocs build --strict` | Passed |
| `scripts/public-doc-links` (docs virtual environment on `PATH`) | Passed; 463 files |
| `scripts/public-secret-scan` | Passed; tracked-tree scan |

**Finding 3 (P2) remains open and unchanged.** These two repairs are
consensus-affecting and source-only. Nothing is deployed; the new combined tip
must be qualified before rollout. No Task Node or fleet action, full workspace
test suite, exact saved-chain qualification, or historical proof rebuild was
performed in this repair task. An old over-bound FastPay record requires a
separately authorized resolution before V2 installation can proceed.
