# Cobalt benchmark diagnosis

## Verdict

The earlier 20-validator, 90%-UNL-overlap halt was caused by the benchmark adapter, not by a demonstrated Cobalt liveness failure.

The old adapter converted each validator's entire local UNL into exactly one Cobalt essential subset. Because a Cobalt subset identity includes the validator set, quorum, and Byzantine budget, two non-identical UNLs became two non-identical subset IDs. Production linkage correctly found no shared subset ID between those views and classified the pair as unlinked.

That conversion is not the Cobalt model. The paper treats essential subsets as primary declarations and says the loose analogue of an XRP UNL with local quorum `q_i` is a family of subsets of size at least `3(n_i-q_i)+1`, not one subset equal to the whole UNL. See `docs/references/cobalt-bft-governance-in-open-networks.md:62-76`. Linked and fully linked nodes must share an essential subset satisfying the relevant fault and correct-node bounds; see the same reference at lines 124-132.

## Code trace

The obsolete adapter is `crates/node/src/bin/postfiat_cobalt_benchmark.rs:257-285`:

1. It reads one `local_unls[validator]` row.
2. It derives one quorum and one clamped Byzantine budget.
3. It calls `build_essential_subset` once with the whole UNL.
4. It installs only that one subset in the validator's trust view.

The production implementation is internally consistent with the paper's declared-subset model:

- `crates/consensus_cobalt/src/trust_graph_governance.rs:1-63` derives the subset ID from the declared validators, `t`, and `q` and validates the subset.
- `crates/consensus_cobalt/src/internal_validation.rs:2132-2175` recognizes linkage only through an identical subset ID and applies the active-fault, correct-node, and `t <= n-q` checks.
- `crates/consensus_cobalt/src/internal_validation.rs:2177-2219` walks each trust view's transitive closure and requires every pair in the closure to be fully linked.

Therefore, exact subset-ID matching is not the defect. Inventing one whole-UNL subset per validator was the defect.

## Corrected contract

The replacement manifest, `scenario-manifest.json`, declares essential subsets independently of RippleD local UNLs. The decisive 20-validator cases use:

- a shared 18-validator core subset with `q=15`, `t=2`;
- a 19-validator supplemental subset with `q=16`, `t=2` for each edge view;
- the Cobalt inequalities `t < 2q-n` and `2t < q` on every subset;
- explicit proposal support at, above, and below the strong-support boundary.

The production adapter `crates/node/src/bin/postfiat_cobalt_decisive_benchmark.rs` now constructs every declared subset in every trust view and applies the production linkage report before attempting a signed RBC/ABBA/MVBA/DABC decision. It does not derive Cobalt trust from RippleD UNLs and does not call the oracle.

The RippleD adapter `rippled/DecisiveGovernanceBenchmark_test.cpp` uses the same proposals but evaluates each validator through its local UNL and local quorum. Native RippleD CSF ledger consensus is emitted as a separately labeled control; it is not presented as the validator-governance decision.

## Frozen result surface

The independent oracle crate has no dependency on `postfiat-consensus-cobalt`, `postfiat-node`, or any other production PostFiat protocol crate. Its frozen manifest contains 18 cases:

- 13 compatible cases;
- 5 incompatible cases;
- a per-node expected Cobalt and RippleD outcome for every correct validator;
- zero `characterize` or permissive expectations;
- one material safety-delta case in which RippleD local UNLs admit two conflicting governance roots while Cobalt's linkage gate halts.

Frozen canonical manifest ID (the SHA-256 with its own hash field blank): `78fc3f92d460f45a4941d40ef705af6c761e3782155a5b599dbd78c90396bde3`.

The decisive run has now executed this frozen contract. Production Cobalt passed all 18 cases with zero per-node mismatches and zero conflicts, including all three 90%-overlap support boundaries. The matched RippleD adapter reproduced the frozen material delta: divergent local UNL quorums admitted two registry roots, while Cobalt rejected the unsafe graph before commitment. The remaining activation gates concern independent live operators and the governed cutover.
