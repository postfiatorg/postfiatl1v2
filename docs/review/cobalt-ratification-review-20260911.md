# Cobalt ratification review — 2026-09-11

This review covers `crates/consensus_cobalt`,
`crates/cobalt_decision_oracle`, and
`crates/cobalt_adversarial_oracle`. It is the A3 surface of the
[burn 3 campaign](qa-campaign-20260911-burn3-brief.md). The pass traced
essential-subset arithmetic, old/new graph safety witnesses, signed RBC and
ABBA support, equivocation handling, deterministic MVBA selection, DABC
ratification and replay, and the two independent-oracle publication paths.
Frozen benchmark outputs and deployment evidence were read only.

The pre-repair focused baseline was green:

- `postfiat-consensus-cobalt`: 72 tests passed.
- `postfiat-consensus-cobalt` genesis registry checker: 9 tests passed.
- `postfiat-cobalt-decision-oracle`: 3 tests passed.
- `postfiat-cobalt-adversarial-oracle`: 3 tests passed.

## Findings

### 1. P1 — the transition witness checks subset overlap instead of quorum overlap

**Source:** `crates/consensus_cobalt/src/internal_validation.rs:1616-1633`.

`safety_witness_intersections` records the validators common to an old and a
new essential subset, then marks the transition safe whenever that raw
membership intersection is larger than the Byzantine budget. The safety
property requires the minimum intersection of any valid old quorum and any
valid new quorum. For old subset `S_o`, new subset `S_n`, and quorums
`q_o` and `q_n`, that lower bound is
`max(0, q_o + q_n - |S_o union S_n|)`, not `|S_o intersection S_n|`.

Concrete failure scenario: rotate one validator between two seven-member
subsets, with quorum five and Byzantine budget two. The raw subset
intersection has six validators, so the current witness reports the transition
safe. An old quorum and a new quorum can nevertheless be chosen with only two
validators in common; both may be Byzantine. The accepted witness therefore
does not prove an honest cross-transition quorum intersection and can authorize
conflicting old- and new-graph decisions.

The repair must derive the conservative quorum-intersection lower bound for
every old/new cover pair and require it to exceed the declared Byzantine
budget. This changes transition admissibility and is consensus-affecting.

### 2. P2 — DABC full-knowledge activation ignores the pending candidate identity

**Source:** `crates/consensus_cobalt/src/dabc_registry.rs:550-562`.

A signed full-knowledge check commits each pending item as
`(amendment_slot, output_candidate_id)`, but activation validation checks only
that the slot exists somewhere in the ratified chain. It never compares the
pending `output_candidate_id` with the ratification at that slot.

Concrete failure scenario: a quorum signs checks naming the real ratified slot
12 but a conflicting candidate ID. The checkpoint and every signature verify,
and activation succeeds because slot 12 exists. The resulting activation
evidence therefore claims full knowledge while its signed candidate identity
does not match the ratified history.

The repair must index the ratified chain by slot and reject a pending pair
whose candidate ID differs. This tightens DABC activation validation and is
consensus-affecting.

## P3 observations

### 3. P3 — the live-mode “signed beacon” common coin has no authentication input

**Source:** `crates/consensus_cobalt/src/core_types.rs:788-797` and
`crates/consensus_cobalt/src/rbc_abba_mvba.rs:1702-1740`.

`AbbaCommonRandomSource::SignedBeacon` carries only a beacon ID and output
hash. `abba_common_coin` validates their shape and uses the caller-selected
low bit; it cannot verify a beacon signature, signer, round binding, or
agreement binding.

Concrete failure scenario: a caller supplies either of two well-formed output
hashes and selects the desired live-mode coin bit without possessing any
beacon key. No production call site currently uses this abstraction—the live
authority transcript requires signed ABBA finish support directly—so this is
recorded as an incomplete, unreachable substrate rather than repaired during
the campaign.

### 4. P3 — the first decision oracle accepts incomplete validator classifications

**Source:** `crates/cobalt_decision_oracle/src/lib.rs:336-369,472-510`.

Scenario validation requires declared correct and Byzantine validators to be
known and disjoint, but does not require every validator to be classified.
The linkage calculation then counts every non-Byzantine, available subset
member as responsive correct, including a validator absent from
`correct_nodes`.

Concrete failure scenario: a scenario declares validators `a,b,c,d`, marks
only `a,b` correct, marks no node Byzantine, and uses a three-of-four shared
subset. The oracle can count `c,d` toward responsive liveness even though the
scenario made no correctness claim for them. Current frozen inputs classify
their validators completely and remain unchanged, so this input-contract gap
is recorded without rewriting or regenerating a frozen oracle artifact.

## Areas with no findings

- Signed RBC and ABBA support verifies every sender against the registered
  ML-DSA committee, deduplicates identities, and excludes same-round ABBA
  equivocators before threshold evaluation.
- Essential-subset validation enforces sorted unique identities and both
  `t_S < 2q_S - n_S` and `2t_S < q_S`.
- MVBA candidate order and output selection are canonical, bounded, and bound
  to the RBC proposal identity.
- DABC ratification and replay bind the domain, graph roots, sequence, parent,
  slot, candidate, checkpoint, and activation identities; duplicate slots,
  checkpoints, and activations fail closed.
- The adversarial oracle regenerates its deterministic corpus and checks both
  corpus and manifest hashes; the packet verifier separately pins the frozen
  corpus hash.
- The decisive-oracle packet verifier checks the canonical manifest hash,
  regenerated input bytes, oracle contract and source bytes, and both adapter
  hashes.

## Repair result

Repair `c9a61fcd` closes findings 1 and 2:

- old/new cover rows now derive the smallest possible intersection of their
  valid quorums from both quorum sizes and the union size, and require that
  lower bound—not raw membership overlap—to exceed the Byzantine budget;
- DABC activation now resolves every pending slot to its ratified candidate and
  rejects a signed pending pair that names a different candidate ID.

The safety regression proves that the prior five-of-seven single-rotation case
with budget two is rejected even though the subsets share six validators. The
positive witness fixture now uses six-of-seven quorums and budget one, whose
minimum cross-quorum intersection is four. The DABC regression builds a
structurally valid, quorum-signed checkpoint with the wrong candidate ID and
proves activation rejects it.

Both repairs tighten a consensus ratification or transition-admission rule and
are consensus-affecting. They are source-only and were not activated, deployed,
or presented as live behavior.

Post-repair verification:

- `postfiat-consensus-cobalt`: 73 tests passed.
- `postfiat-consensus-cobalt` genesis registry checker: 9 tests passed.
- Both oracle crates: 3 tests passed each.
- The Cobalt safety-witness example completed with all six checks passing.
- Node Cobalt authority: 1 focused test passed.
- Node Cobalt shadow and runtime: 15 focused tests passed.
- Strict clippy for the Cobalt crate and both oracles, plus the workspace
  formatting check, passed.

## Repair scope

Findings 1 and 2 were repaired in `c9a61fcd`. Findings 3 and 4 remain
recorded under the P3 rule. No frozen oracle output, benchmark receipt,
deployment evidence, authority state, or live system changed.
