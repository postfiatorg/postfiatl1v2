# Bridge workflows correctness review — 2026-09-21

A4 of the [burn 6 campaign](qa-campaign-20260921-burn6-brief.md), at
`acfda116` after the required release-branch pull on clean `burn6-work` in
`/tmp/burn6-20260921`. The 25-minute window began at approximately 11:03 UTC.
All 8,624 lines of the four permitted files were read: node
`vault_bridge_workflows.rs` (3,918), `vault_bridge_conservation.rs` (1,755),
`ethereum_checkpoint_signing.rs` (2,325), and `pfusdc_tier4.rs` (626), including
their in-file tests. Line references identify this pre-repair revision.

## Findings

### 1. BRW-01 — P2 — conservation verification trusts cached success fields

**Source:** `crates/node/src/vault_bridge_conservation.rs:90-108` and `:900-935`.

**Condition:** a report retains `conserved=true` and a zero cached delta while
its source balance or claim components differ.
**Observed:** `verify()` accepts the inconsistent report.
**Expected:** recompute the checked component sums and `V = S + D + B - R`,
and reject cached fields inconsistent with those values. This is report consistency, not proof
that a supplied report's observations are authentic.

**Suggested minimal change:** recompute the summary during verification;
retain a non-conserved report for inspection.
Add regressions for altered cached fields, overflow, impossible releases,
and valid non-wrapped allocations. **Consensus-affecting: no**; local audit
output only, with unchanged ledger, encoding and protocol rules.

### 2. BRW-02 — P2 — deposit receipts can contradict their selected log

**Source:** `crates/node/src/vault_bridge_workflows.rs:1384-1441`, `:1944-1962`,
`:2353-2433`, and `:2436-2533`.

**Condition:** a supplied receipt declares failure, an RPC receipt omits its
status, a selected log names a different transaction/block than its enclosing
receipt, or a log is marked removed. **Observed:** the file path never checks
receipt success; the RPC success helper accepts missing status; selection
fills absent log coordinates but does not compare present coordinates; the
parser ignores `removed`. Such input can become a success attestation/relay
plan using facts from different events. No consensus acceptance or source-chain
release is established.

**Expected:** RPC receipts require explicit success; supplied receipt failures,
contradictory coordinates and removed/malformed removal flags fail before a
plan is returned. **Suggested minimal change:** enforce these checks in the
existing success helper, receipt selector and log parser. Preserve legacy
manually supplied receipts without a status and raw-log input, without claiming
they establish finality. Regress each invalid class and a valid inherited-hash
receipt. **Consensus-affecting: no**; unsigned local workflow validation only.

### 3. BRW-03 — P2 — conservation observations mix source blocks

**Source:** `crates/node/src/vault_bridge_conservation.rs:478-569`, `:605-617`,
and `:669-681`.

**Condition:** source state advances between sequential code, balance, deposit
and withdrawal reads. For example, funds move from one governed vault to
another between their balance reads. **Observed:** every read defaults to
latest; the same funds can be counted at both locations and hide a shortfall,
or a later withdrawal flag can be combined with an earlier balance.
**Expected:** all observations for one source chain use one fixed block.

**Suggested minimal change:** resolve one finalized height and hash per chain,
pin every state read to that height, and reject a changed block hash before
returning the report. Fail closed when finalized/history reads are unavailable.
Use a local cast stub that requires the pinned height and simulates hash drift.
**Consensus-affecting: no**; local source observation policy only. This does
not authenticate a dishonest RPC or make separate chains/PFTL one atomic snapshot.

### 4. BRW-04 — P3 — resource limits apply after unbounded collection

**Source:** `crates/node/src/vault_bridge_conservation.rs:309-313`, `:377-378`,
`:1029-1063`; `crates/node/src/vault_bridge_workflows.rs:900-921`,
`:1767-1800`, `:1817-1850`, and `:1865-1891`.

**Condition:** a local proof file grows after metadata inspection, or a cast
child/source response emits excessive output or stalls. **Observed:** unbounded
file reads and `Command::output()` collect bytes before the advertised limit;
lineage files have no byte cap. **Expected:** enforce limits while reading and
bound subprocess duration/output, including stderr. **Suggested minimal change:**
bounded file handles and bounded child pipes with a deadline and cleanup.
Operator-tool resource exposure. **Repaired 2026-09-23 in `0b9c6715`:** bounded
file handles and a bounded, deadline-limited cast runner.
Repairs land after the qualified tip and ship in the release after next.

### 5. BRW-05 — P3 — malformed HTTP lengths are treated as absent

**Source:** `crates/node/src/ethereum_checkpoint_signing.rs:728-757`.

**Condition:** a JSON-RPC response has an invalid Content-Length or duplicate
conflicting lengths. **Observed:** invalid lengths are discarded by `.ok()`;
only the first parsable length is checked, so otherwise valid JSON can pass.
**Expected:** reject malformed or ambiguous framing. **Suggested minimal change:**
parse each length strictly and reject duplicate/conflicting framing, while
retaining the response byte cap. No wrong checkpoint signature is demonstrated.
**Repaired 2026-09-23 in `0b9c6715`:** each length parses strictly; duplicates are
rejected.
Repairs land after the qualified tip and ship in the release after next.

### 6. BRW-06 — P3 — V2 recipient offset uses the V1 head bound

**Source:** `crates/node/src/vault_bridge_workflows.rs:2457-2474` and
`:2718-2730`.

**Condition:** a V2 event points its dynamic recipient to offset 192, inside
its seven-word (224-byte) head; a small token-address word can act as the
string length. **Observed:** the shared decoder checks only the six-word V1
minimum. **Expected:** use the selected event version's head size before
decoding its recipient. **Suggested minimal change:** pass the computed head
length or require the canonical recipient offset in the caller. Later identity
checks still apply; no receipt-proof bypass is established. **Repaired
2026-09-23 in `0b9c6715`:** the decoder receives the selected version's head length.
Repairs land after the qualified tip and ship in the release after next.

## Areas with no findings

- Workflow deposit IDs, recipient hashes, indexed-address padding, bounded
  arithmetic and proof-file commitments are checked. Proof-native routes compare
  their exposed identities and handle source-specific evidence coordinates.
  No additional finding beyond the receipt/decoder issues above.
- Withdrawal planning validates the chain-bound redemption and pending state;
  ABI digest construction uses distinct domains and binds the EVM chain,
  verifier, packet digest, withdrawal commitment and finalized height. Bucket
  selection refuses ambiguous sources. Actual withdrawal consumption/replay
  prevention belongs to excluded execution/contracts, not these plan builders.
- Conservation deduplicates chain/vault/token balances across profile history,
  checks governed code hashes and interface lineage, rejects missing deposits
  and impossible settled-without-claimed withdrawals, and reconciles redemption
  queues with checked arithmetic. The findings above limit its audit result.
- Checkpoint signing matches the governed chain, route, committee and runtime
  code policy; checks RPC identity before durable intent; signs through the
  canonical vote bytes with the ML-DSA context; verifies cached votes and calls
  the certificate verifier. No additional signing-byte finding in these call
  sites. Physical crash durability and delegated crypto were not qualified.
- Tier-4 exporters select the burn receipt and require literal acceptance,
  verify exit Merkle membership, bind the commit exit root, reconstruct
  historical committees in stable update order, and include governance changes
  in ancestry. PFTL receipt export checks the route-state boundary and mint
  digest and calls its witness verifier. No additional finding in this file.

## Review limits and skips

Only the four named implementations received a correctness review. Module/test
registration and test names supplied selection context; repository guidance,
existing reviews and NAV documentation supplied invariants and report format.
Delegated type validation/canonical encodings, global supply calculation,
execution/rollback, storage, historical registry replay, finality verification,
RPC dispatch and all proof/contract implementations were not reviewed.
Specifically excluded: `crates/privacy_orchard`, `crates/privacy`,
`crates/bridge`, `crates/ethereum-contracts`, `crates/pfusdc_proofs`,
`crates/pftl_uniswap_proofs`, `crates/proofs`, and `programs/`.

No other surface or B/inventory/scoring work is included. No Task Node, fleet,
live RPC, spend, installation, deployment, qualification, frozen-artifact edit,
or protected-release-checkout access. The brief limits network use to git and
TIH; checkpoint tests that bind sockets and Anvil/fork tests are therefore
skipped. Local temporary stores and fake cast processes are permitted; no
generated command bundle is executed against a chain. No full workspace or
Orchard suite, archived-chain replay, physical crash experiment or CI query.
Full-suite verdict belongs to CI on the pushed branch. P3 findings stay unfixed.

## Repair result

Findings were separately gated and pushed as `66a68624` before repairs.
Only `vault_bridge_conservation.rs` and `vault_bridge_workflows.rs` change as
source, with three added in-file regression tests.

- **BRW-01:** verification recomputes checked claim/deposit sums and the vault
  identity, rejects inconsistent cached totals and impossible released amounts,
  and preserves valid non-wrapped allocations. This validates summary arithmetic;
  it does not authenticate row observations or reconcile delegated family/private
  issued-supply semantics. **Consensus-affecting: no.**
- **BRW-02:** RPC receipts require a success status; the supplied-receipt path
  rejects explicit failure; selected log coordinates must agree with enclosing
  receipt coordinates and any aliases; raw and selected logs reject removed or
  malformed removal flags. Legacy manually supplied receipts without status
  remain accepted as unproven input. **Consensus-affecting: no.**
- **BRW-03:** one finalized height/hash is selected per source chain, every
  code/balance/deposit/withdrawal read uses that height, and the hash is rechecked
  before returning a report. An unavailable finalized/history query fails closed.
  Local cast stubs enforce pinned arguments and reject simulated hash drift;
  no live RPC behavior or cross-chain/PFTL atomic snapshot is claimed.
  **Consensus-affecting: no.**
- **BRW-04/05/06:** repaired 2026-09-23 in `0b9c6715`, after the qualified tip;
  they ship in the release after next. **Consensus-affecting: no.**

Before repair, `cargo test -p postfiat-node burn6_ --lib --locked` produced
**3 passed, 3 failed, 0 ignored, 380 filtered**: the three new A4 tests failed
on an accepted altered source balance, an accepted contradictory block hash,
and an unpinned source query. That filter also matched three existing A2 tests;
they passed and received no additional review or changes.

Final focused verification, all with
`env CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/integrate-20260918-target`:

| Command | Result |
| --- | --- |
| `cargo test -p postfiat-node vault_bridge_workflows::tests --lib --locked` | 4 passed, 0 failed, 0 ignored, 382 filtered |
| `cargo test -p postfiat-node vault_bridge_conservation::tests --lib --locked` | 8 passed, 0 failed, 0 ignored, 378 filtered |
| `cargo test -p postfiat-node pfusdc_tier4::tests --lib --locked` | 1 passed, 0 failed, 0 ignored, 385 filtered |
| `cargo check --workspace --locked` | Passed |

**13 A4 tests passed, none failed or ignored.** The two checkpoint-signing
module tests were not invoked because they bind sockets; they are skipped,
not counted as ignored/passed. No release-tip re-qualification is triggered by
these local repairs; existing earlier-surface requirements remain in force.
No full-suite or CI pass is claimed.
