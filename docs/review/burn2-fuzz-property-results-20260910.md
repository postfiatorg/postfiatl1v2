# Burn 2 fuzz and property results — 2026-09-10

This record covers Unit 3 of the [Burn 2 campaign](qa-campaign-20260910.md#burn-2).
The work expands deterministic, bounded coverage around four repaired surfaces;
it is not release, deployment, or live-authority evidence.

## New coverage

- `bridge-proof-parser` constructs a valid single-leaf Ethereum receipt proof,
  mutates the receipt bytes while recomputing the proof root, catches panics,
  checks the accepted seed exactly, and asserts the receipt-size and proof-node
  bounds reject.
- `consensus-round-monotonicity` exercises production timeout-vote
  authorization over 4,096 deterministic combinations of prior prepare,
  precommit, and timeout rounds. Acceptance must match the cross-phase floor
  and strict same-phase monotonicity rule; accepted transitions must preserve
  the other phase marks.
- The RPC serve-loop property exhaustively checks every accept-budget boundary
  from zero through 1,024 against the production loop predicate.
- The UNL V2 evidence property subjects the committed evidence fixture to 512
  deterministic byte mutations. Parsed cases are resealed and verified twice;
  results must be byte-identical and every failure or record rejection must
  name its field. Malformed JSON is rejected before verification.

## Bounded runs

Each new Rust target ran twice with `--iterations 4096`, under a 600-second
process ceiling. Both reruns were byte-identical.

| Target | Corpus | Accepted/parsed | Rejected | Invariant failures | Elapsed | Output SHA-256 |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `bridge-proof-parser` | 4,099 | 1,313 | 2,786 | 0 | 1.12 s | `48f7278c4776dd4a00964685f58d15ae6d8385b27b8f5069384e3d19c6d99b05` |
| `consensus-round-monotonicity` | 4,096 | 1,920 | 2,176 | 0 | 0.38 s | `ef154da73239c31ff473777060076c4fdfc77213735ae401bceb40bb8b4f0d60` |

The UNL mutation property evaluated 513 inputs and required more than 100
parsed and more than 100 malformed cases. No target crashed or violated a
property, so Unit 3 produced no new finding.

## Focused regression results

- Bridge library: 38 passed.
- Consensus v2 node-library selection: 9 passed.
- RPC serve-request selection: 25 passed.
- UNL V2 evidence file: 20 passed, including 10 subtests.
- Offline fuzz-harness dependency check: passed.

No network, fleet, chain, deployment, Task Node, StakeHub, frozen-artifact, or
whitepaper action occurred.
