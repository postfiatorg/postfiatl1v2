# YOLO target receipts deploy decision

**Authority boundary:** Decision support only. This sheet deploys nothing,
schedules nothing, registers nothing, submits nothing on-chain, and contacts no
validator or fleet.

**Question:** Does the next validator release carry the merged, default-disabled PR #38 code?

**Recommended one-line answer:** `A — carry PR #38 in the next routine validator release; leave YOLO target activation unscheduled.`

## What is true today

- PR #38 is merged into `main` at `1412b4dc`. Its YOLO target-receipt code
  is running nowhere and has not been activated on a testnet or validator
  fleet.
- The feature defaults to disabled. It can activate only after a future
  governance amendment schedules `yolo_target_activation_height` strictly
  after the amendment block.
- A later registration must separately pin reviewed program and run
  identities, the permitted submitter, expected prior state, replay identity,
  and its own future activation height.
- A target receipt records a proved portfolio calculation. It cannot trade,
  move capital, establish reserves, authorize minting or issuance, or approve
  a provider or portfolio.
- Two remediation commits, `4a961eeb` and `343a2fe9`, targeted the six
  jobs that had been failing on both PR #38 and pre-merge `main`. The
  verification run on `343a2fe9` was still executing when this sheet was
  requested. CI green is therefore a precondition to check, not a fact
  established here.

## Options

### A — Carry it in the next routine release; do not schedule activation

- Validators receive the compatible code at the earliest routine release, but
  the feature remains dormant because no activation height is scheduled.
- This decouples binary rollout and rollback readiness from the later,
  separately reviewed activation and registration decisions.
- Release surface grows by default-disabled code; the CI, rollback, and canary
  preconditions below still apply in full.

### B — Hold it out until the first activation decision

- The next release has a smaller feature surface, even though `main` already
  contains PR #38 and release composition must deliberately exclude it.
- The first activation decision must later be coupled to a compatible validator
  release, its rollout, and the activation amendment.
- This delays dormant-code readiness and concentrates more coordination and
  rollback judgment into the activation window.

### C — Carry it and schedule a far-future activation height immediately

- Validators receive the code as in A, while governance also commits now to a
  future activation height.
- This provides no material readiness benefit over A: the feature is already
  dormant until scheduled, and program/run registration remains a separate
  gate.
- It adds an on-chain scheduling decision before one is needed. This option is
  listed for completeness and is not the recommendation; this sheet schedules
  nothing.

## Lean

**Lean A.** PR #38 is dormant by design, its release and rollback guidance is
already written, and carrying it without scheduling activation separates a
routine software-compatibility decision from the higher-risk governance,
program-identity, and on-chain activation decisions. A does not authorize a
release; it is the recommended answer if every precondition below is met.

## Preconditions before any release

These conditions apply whichever option the operator selects:

1. **Current CI is green on `main`.** Check the latest completed run at the
   release candidate commit. Require all required jobs, including the six
   formerly failing jobs—`check`, `test`, `python-sdk`,
   `wallet-and-proxy`, `open-reserve-proof-kit`, and
   `public-tree-hygiene`—to pass. Do not treat `4a961eeb` or
   `343a2fe9` alone as green-run evidence.
2. **Release precedes activation.** A compatible validator release must be
   deployed before any governance scheduling of
   `yolo_target_activation_height`. Selecting A, B, or C in this sheet does
   not perform either action.
3. **Rollback boundary is explicit.** Retain the previous release as the
   rollback candidate until YOLO target-receipt state exists. After receipt
   state exists, a pre-feature binary must not be assumed replay-compatible
   with the populated state.
4. **The canary passes the PR checklist.** Verify “matching state roots,
   receipt finality/replay, and rejection of early, malformed, substituted and
   duplicate submissions.” Record those results against the exact release
   candidate before broader rollout.

A CI pass or canary pass is evidence for its stated boundary only. Neither
schedules activation, approves a program identity, or grants economic
authority to a receipt.

## What this sheet does not decide

This sheet does not decide or authorize:

- an activation height or any activation scheduling;
- any program, verification-key, registrant, submitter, methodology, parameter,
  collection-manifest, or replay identity;
- any registration or submission transaction;
- any asset registration, issuance, reserve treatment, mint, redemption,
  capital allocation, or trade;
- a release date, rollout owner, deployment command, canary execution, rollback
  execution, or other fleet action; or
- any other on-chain action.

## Operator answer

Reply with one line:

`A — carry it in the next routine validator release; leave activation unscheduled.`

The operator may instead answer `B` or `C` with the option wording above.
No answer by itself performs the selected action.

## Sources

- PR #38 merge: `1412b4dc`.
- PR #38 release, rollback, and canary guidance:
  `docs/handoffs/2026-09-07___dravlic__pr38_merged_arc_deck_unl_proposal.md`
  as recorded in historical commit `817d93f7`.
- Feature and authority boundary: `docs/yolo/target-receipt-v1.md`.
- Reserve-profile activation limits:
  `docs/navcoins/yolo-options-reserve-profile.md`.
- CI-remediation commits: `4a961eeb` and `343a2fe9`; live CI status must
  be checked at decision time.
