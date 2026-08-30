# L1v2 Public Testnet: Genesis Registry and Launch Specification

**Status:** Draft for review — not locked, not scored
**Date:** 2026-08-30
**Implements:** the design surfaces of the [public-testnet path plan](../plans/active/l1v2-public-testnet-path-milestone.md) (Task Node `task_510e7605cb2dff0dfd672b397d26f2a6`)
**Defers to:** [whitepaper](../whitepaper.md) §6 (Cobalt transitions), [storage-scaling-fix-spec](storage-scaling-fix-spec.md) (storage), `dynamic-unl-scoring/docs/DeterministicFinalScore.md` (scoring), [Dynamic UNL evidence-source note](../governance/dynamic-unl-l1-evidence-source-note.md)

## 1. Purpose and scope

The testnet-path plan orders five phases. Phases A (offline qualification)
and B (rehearsal/deployment) are fully specified by existing storage
documents and need no new design. This specification covers what is new:

1. how the fork's scored operator community becomes l1v2's genesis validator
   registry (plan C3);
2. how the validator sidecar becomes the l1v2 ratification client (plan C4);
3. the launch gates that make "public testnet" a checkable state (plan D);
4. the status CLI and interface (plan E).

Out of scope: mainnet, token migration/coexistence economics, Cobalt adoption
claims beyond the controlled-devnet KEEP_ACTIVE boundary, and any change to
Consensus v2 finality or storage behavior.

## 2. Actors and objects

| Term | Meaning |
| --- | --- |
| Fork / PFT Ledger | The live rippled-derived testnet: 51+ community validators, secp256k1 identities, Dynamic UNL rounds (frozen inputs, deterministic selection, commit-reveal sidecar verification) |
| Pipeline | The Dynamic UNL scoring pipeline; after the plan's C2 decision gate, its authoritative scores are deterministic functions of frozen evidence |
| l1v2 | This repository's chain: Consensus v2 finality, ML-DSA-65 authorization, qualified transactional storage, Cobalt registry/trust-graph machinery (`crates/consensus_cobalt/`) |
| Genesis manifest | Whitepaper §6 object: one hash root committing registry `G0`, trust graph `T0`, checker `χ0`, safety profile `π0`, witness schema, chain id, and the launch ratification certificate |
| Ratification client | Sidecar extension that replays a proposed transition's evidence deterministically and commit-reveals a signature; no discretionary input |

## 3. Genesis registry construction (plan C3)

### 3.1 Selection input

The candidate operator set is the output of a **named, frozen Dynamic UNL
round** on the fork (the "genesis round"), chosen by the operator and recorded
by round id, frozen-input CID, and selection output hash. Eligibility for the
genesis registry is exactly the round's published selection (score ≥ cutoff,
concentration caps applied). No operator may be added or removed except by
re-running selection on corrected frozen evidence; manual edits are forbidden.

### 3.2 Identity bridge (fork key → l1v2 key)

Each selected operator produces one **genesis identity receipt**:

1. generates a fresh ML-DSA-65 l1v2 validator key;
2. signs a statement `(fork_master_pubkey, l1v2_mldsa_pubkey, chain_id,
   genesis_round_id, expiry)` with the fork validator master key; and
3. counter-signs the same statement with the new ML-DSA key.

The receipt binds the scored fork identity to the l1v2 key in both
directions, mirroring the Cobalt key-continuity receipt design (both-key
signatures, old authority validates new identity). Receipts are hash-bound
into the genesis manifest. An operator without a valid receipt is dropped
from `G0` before ratification; the slot is not backfilled below the launch
minimum (§5).

### 3.3 Template trust graph

`T0` assigns every validator the same template trust view derived from `G0`:
one essential subset `S = G0` with

```text
q_S = ceil(4 * n_S / 5)
t_S = min( ceil(n_S / 5), floor((q_S - 1) / 2), 2*q_S - n_S - 1 )
```

The formula is a launch profile, not a protocol constant: the checker `χ0`
enforces the local-soundness inequalities (`t_S < 2q_S − n_S`, `2t_S < q_S`)
and linkage regardless of parameters, so a reviewed change to the profile
cannot bypass safety arithmetic. Uniform template views are expected at
launch; heterogeneous views become possible whenever an operator declares
one, and Cobalt's linkage check is the gate either way.

### 3.4 Genesis ratification

The launch ratification certificate is signed by the `G0` operators'
**l1v2 ML-DSA keys** (quorum `q_S`), each signature produced only after the
operator's ratification client has verified, from published inputs:

- the genesis round's frozen inputs replay to the published selection;
- every identity receipt validates;
- `T0` satisfies the checker inequalities and linkage;
- the genesis manifest root matches the locally recomputed root.

After finalization genesis ratifiers hold no override authority (whitepaper
§6). Changing the manifest is a fork, not governance.

## 4. Post-genesis registry operation (plan C4)

Registry changes on the testnet follow one loop:

```text
pipeline round (frozen, deterministic)      [proposes G_{t+1}, T_{t+1} delta]
  -> Cobalt transition check under G_t rules [cover extractor + old-new matrix]
  -> ratification-client replay + commit-reveal signatures [q of G_t]
  -> activation; receipts anchored on chain
```

- The **proposer** is initially the Foundation-operated pipeline; proposal
  plurality is the recorded E6 follow-on milestone and requires no redesign
  here, because the checker and ratification are proposer-independent.
- The **ratification client** extends `validator-scoring-sidecar`: fetch
  frozen round → deterministic replay (CPU-only once the plan's C2 decision
  demotes model authority; a GPU replay lane remains only if the model
  retains authority) → run the transition checker → commit-reveal an ML-DSA
  signature. It has no discretionary inputs; declining to sign happens only
  on verification failure, and every decline is an anchored, diagnosable
  artifact.
- **Fail closed:** missing, stale, conflicting, or oversized evidence holds
  the current registry. A failed round changes nothing.

## 5. Launch gates (plan D)

Public testnet is a checkable state, declared only when all hold:

| Gate | Requirement | Machinery |
| --- | --- | --- |
| L1 Qualification | Storage `OFFLINE QUALIFIED`; G6 six-clone rehearsal passed; deployed lineage fleet-receipted | Plan phases A–B artifacts |
| L2 Registry | Genesis per §3 with `n_S ≥ 12` valid identity receipts | Genesis manifest + receipts |
| L3 Independence | No single operator group can reach `q_S` or block it alone, measured by the fork's placement/concentration machinery on `G0` | Placement preflight + admission correlation caps |
| L4 Verification | ≥ `q_S` ratification clients live and committing on real rounds for two consecutive weekly rounds | Convergence reports |
| L5 Operations | Public join runbook, key-custody guidance, monitoring endpoints published | Plan D2 |
| L6 Decision | Explicit operator launch decision recorded | Outside this spec's authority |

## 6. Status CLI and interface (plan E)

A Python CLI (`tools/testnet_path/`, entry point `testnet-path`) reads a
small YAML gate registry (checked into `docs/status/`) mapping every plan
checkbox and §5 gate to its state and evidence reference, and renders:
`testnet-path status` (table, colors optional), `testnet-path blockers`
(open items with owners), `testnet-path gate <id>` (evidence pointers). The
user-facing interface is a generated docs/status page rendering the same
registry through MkDocs, so the CLI and page cannot disagree.

## 7. Invariants

1. **No manual registry membership, ever** — genesis and every later change
   trace to frozen evidence plus published deterministic rules.
2. **Old rules validate new rules** — including genesis→first-transition.
3. **Ratification is verification** — clients hold no discretionary inputs;
   agent-operated validators are first-class by construction.
4. **Verification must be commodity-hardware** unless the C2 decision
   explicitly retains model authority, in which case the GPU lane and its
   cost are documented as a launch-gate liability under L4.
5. **Fail closed everywhere**; a failed round or missing evidence never
   mutates the registry.
6. **No consensus-byte changes** to Consensus v2 or storage from anything in
   this specification.

## 8. Acceptance criteria

- §3: a dry-run genesis build from a real frozen fork round produces a
  reproducible manifest root on two independent machines; negative fixtures
  (edited selection, missing receipt, unsound `T0` parameters, wrong chain
  id) each fail with a named reason.
- §4: a rehearsal transition on the controlled devnet completes the full
  loop with ≥ `q_S` client signatures, and each negative case (stale round,
  tampered delta, replayed transition) holds the registry.
- §5: every gate has a machine-readable state in the gate registry with an
  evidence pointer.
- §6: the CLI runs from a clean checkout with `pip install`-able
  dependencies and the docs page renders under strict MkDocs.

## 9. Open questions (tracked, not blocking review)

1. Token/migration economics between fork and l1v2 — separate document.
2. Operator incentive disclosure for the no-reward model — belongs in the
   L5 runbook.
3. Model authority after the C2 shadow evaluation — this spec is written to
   be correct under either outcome.
