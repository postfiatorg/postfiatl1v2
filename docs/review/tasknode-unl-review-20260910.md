# Task Node UNL V1/V2 review — 2026-09-10

Scope: the shipped V1 and V2 schemas, binding and evidence paths, graph and
admission policy, CLIs, renderers, tests, and committed fixtures. This was a
fresh-eyes adversarial review of evidence freshness, directionality, duplicate
credit, canonical ordering, commitment binding, fail-closed behavior, and the
shadow-only authority boundary. It is not a promotion or live-authority claim.

## Findings

### 1. P1 — A complete score window passes without a renewed vouch or post-epoch co-work

**Location:** `python/postfiat_rpc/tasknode_unl_v2_evidence.py:1275`.

`assess_fresh_window` counts active vouches and co-work but never requires
either count to be nonzero. It also counts a directed vouch when the account is
the voucher, although acceptance preserves source-to-target direction. The
locked amendment requires additions to have post-epoch score evidence, renewed
directed vouches, and post-epoch co-work.

**Concrete failure scenario:** remove every bilateral record from the committed
evidence fixture, reseal the snapshot, and verify it. Alice, Bob, and Carol all
return `READY` with `renewed_vouches=0`, `post_epoch_cowork=0`, and no reason.
An addition can therefore pass continuity using score records alone. An
outgoing vouch also satisfies the current counter for its source account even
though that is not an endorsement of the source.

### 2. P2 — A stale score replay can suppress fresh score evidence by input order

**Location:** `python/postfiat_rpc/tasknode_unl_v2_evidence.py:1442`.

Score duplicate suppression keys only on account, evidence kind, and evidence
digest, and claims that key before deciding whether a row belongs to the
current control epoch. A stale row can therefore consume the duplicate key for
the valid fresh row that follows it.

**Concrete failure scenario:** add one stale old-epoch `work` row with the same
evidence digest as Alice's fresh row. With the stale row first, Alice returns
`HOLD_CONTINUITY`, three fresh records, and
`post_epoch_score_evidence_missing:work`. Reverse only those two rows and Alice
returns `READY` with four fresh records. The facts are identical; only input
order changes the admission result.

### 3. P2 — Valid identifiers can inject Markdown structure into the operator report

**Location:** `python/postfiat_rpc/tasknode_unl_v2.py:405` and
`python/postfiat_rpc/tasknode_unl_v2.py:526`.

The bounded identifier contract rejects leading and trailing whitespace but
permits embedded newlines and backticks. The renderer interpolates those values
inside a one-backtick span without escaping control characters or selecting a
safe fence.

**Concrete failure scenario:** use the valid candidate identifier
`validator-carol` followed by a backtick, newline, and
`# FORGED AUTHORITY`, update the root-bound V1 reference, and derive normally.
The derived Markdown contains a real `# FORGED AUTHORITY` heading between the
side-by-side verdict and `HOLD_CONTINUITY`. The JSON commitment remains valid,
so an operator reading only the rendered surface can be shown attacker-chosen
document structure.

### 4. P3 — The hypothetical round helper accepts a report that is not bound to its frozen window

**Location:** `python/postfiat_rpc/tasknode_unl_v2_policy.py:1488`.

`advance_shadow_round` checks only that the supplied frozen window is frozen
and that the report says `PROPOSE_ADD`. It does not recompute or validate the
report root, nor establish that the decision came from that frozen window and
registry state.

**Concrete failure scenario:** evaluate `candidate-clear` to obtain a proposal,
replace only its in-memory decision candidate with `candidate-control`, and
pass the modified report to `advance_shadow_round`. The real decision for
`candidate-control` is `HOLD` because its declared control group is saturated,
but the helper adds it to the hypothetical registry state. This helper has no
live authority and the campaign rule leaves P3 findings recorded rather than
fixed.

## Areas reviewed without P1/P2 findings

- V1 binding records retain dual-custody signature checks, binding/key joins,
  bounded replay, and explicit hold semantics.
- V1 public vouch, co-work, and funding evidence remains directionally and
  cryptographically checked before graph construction.
- V2 schemas remain closed and versioned, commitments are domain separated,
  funding has no positive mass or unilateral veto, and malformed declarations
  remain isolated to their records and named accounts.
- V2 admission retains exact rational arithmetic, boundary-frozen seeds and
  graph state, current-seat recounting, one-round root overlap, preserved
  incumbent breaches, and explicit no-proposal states.
- Both CLIs remain local and shadow-only; neither exposes submission,
  ratification, funding, or registry mutation commands.

## Pre-repair verification

- `PYTHONPATH=python python3 -m pytest -q python/tests/test_tasknode_unl*.py`:
  **167 passed, 43 subtests passed**.

That green baseline does not exercise the four adversarial cases above. The
repair unit will add focused regressions for findings 1–3. Frozen V1 attack
simulation and V2 gate outputs will remain byte-unchanged.
