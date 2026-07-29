# A666 Private Round-Trip Status and Handoff

**As of:** 2026-07-28 UTC  
**Status:** functional mainnet round trip complete; formal hands-off private
acceptance remains open; all execution is stopped

## Executive summary

The work was intended to prove that a user can acquire newly issued A666
without trading through a shallow Uniswap pool, move it between PFTL and
Ethereum, privately redeem it on PFTL, and receive Ethereum-mainnet USDC.

The economic flow is:

```text
Ethereum USDC
  -> reserve-backed PFTL pfUSDC
  -> newly issued A666 at 1.005 × NAV
  -> Ethereum wA666
  -> returned PFTL A666
  -> private A666 primary redemption at 0.9995 × NAV
  -> private pfUSDC
  -> Ethereum USDC
```

This is primary issuance and redemption. It does not require an OTC transfer
of existing A666, operator A666 inventory, a prefunded NAV redemption bucket,
or a Uniswap purchase. New A666 supply is created against the user's verified
reserve deposit and retired during redemption.

A real mainnet run completed this entire flow with exact conservation and
verified privacy proofs. That establishes a **functional pass**. It is not a
formal release pass because the run required operator repairs after funds
moved and missed the 25-minute issue and redemption targets.

## What is deployed and working

The following production components exist and have been exercised:

- the A666 v2 asset on PFTL;
- the buyer-funded A666 primary issue policy at `1.005 × NAV`;
- the A666 primary redemption policy at `0.9995 × NAV`;
- the PFTL-to-Ethereum A666 export route;
- proof-gated wA666 minting on Ethereum;
- proof-backed wA666 return import to PFTL;
- private-primary A666 issuance;
- private-primary A666 redemption;
- private pfUSDC egress;
- proof-native pfUSDC withdrawal to Ethereum-mainnet USDC;
- the wA666/USDC Uniswap v4 pool, which is deployed, seeded, and tradeable; and
- the replacement pfUSDC Epoch-5 Ethereum vault/verifier lane.

The wA666 controller is locked. The configured route and packet caps are risk
limits, not a permanent maximum on A666 supply. PFTL remains the canonical
A666 supply and NAV ledger.

## What was implemented during acceptance

The acceptance work added or repaired:

- deterministic replay across deployed PFTL history;
- a standard PFTL receipt-witness export path;
- production private-primary A666 issue and redeem actions;
- proof-backed V2 Ethereum-to-PFTL returns;
- binding between V2 export packets and their source commitments;
- primary-redemption custody release;
- a pfUSDC egress guest compatible with current PFTL consensus history;
- proof-backed destination-consume accounting after an Ethereum wA666 mint;
- receipt-local Ethereum log indexing;
- current verifier-checkpoint selection;
- current pfUSDC policy selection;
- six-validator rolling-release orchestration; and
- resumable operational scripts for return import, private redemption, pfUSDC
  proof egress, and the combined post-mint round trip.

Private note seeds, note openings, and spending keys were kept on validator 2
with mode `0600`. They were not copied into the evidence repository.

## Completed live result

The completed small-value private-redemption run was:

```text
1.005000 Ethereum USDC
  -> 1.005000 PFTL pfUSDC
  -> 1.000000 newly issued A666
  -> 1.000000 Ethereum wA666
  -> 1.000000 returned PFTL A666
  -> 1.000000 private A666 note
  -> 0.999500 private pfUSDC note
  -> 0.999500 transparent exit pfUSDC
  -> 0.999500 Ethereum USDC
```

The exact user cost was `0.005500 USDC`:

- `0.005000 USDC` issue spread; and
- `0.000500 USDC` redemption spread.

The vault gained the same `0.005500 USDC`. A666 supply returned to its
starting value, Joe's wA666 returned to its starting value, the wrapper supply
again matched finalized PFTL bridge claims, and Ethereum spendable bridge
supply returned to zero.

The completed run also proved:

- convergence of all six validators on one state root;
- exact A666 and pfUSDC conservation;
- verification of the private-primary redemption proof;
- verification of the private pfUSDC egress proof;
- verification of the SP1 Groth16 PFTL egress proof;
- rejection of an exact private-redemption replay;
- rejection of an exact return-import replay; and
- rejection of an exact Ethereum withdrawal replay.

Privacy covers the PFTL middle, not the entire route. Ethereum deposits,
wrapper mint/burn events, withdrawals, accounts, amounts, and timing remain
public. PFTL also exposes governed route economics, commitments, nullifiers,
assets, amounts, and action timing.

## Why the formal gate is still open

The completed private run required five operator interventions after the
deposit. Its measured latency was:

| Segment | Measured | Required |
|---|---:|---:|
| Deposit inclusion to spendable wA666 | `1,848s` (`30m48s`) | `<= 1,500s` |
| wA666 burn to Ethereum USDC release | `2,688s` (`44m48s`) | `<= 1,500s` |
| Entire deposit-to-withdrawal path | `4,668s` (`77m48s`) | Informational |

Therefore the Phase 8 machine verdict is correctly `FAIL` with
`functional_pass: true`. The result must not be described as a clean,
hands-off, 25-minute release pass.

The interventions corrected real orchestration and protocol-boundary defects:

1. an obsolete pfUSDC policy pin;
2. comparison against the PFTL tip instead of the verifier's prior finalized
   checkpoint;
3. missing proof-backed destination-consume accounting after Ethereum mint;
4. use of a block-global log index instead of a receipt-local index;
5. gaps in private-redemption and proof-egress orchestration.

Rejected attempts failed before mutation. No duplicated or lost A666,
pfUSDC, wA666, or USDC was observed.

## Current live state

All execution has been stopped. No acceptance runner or remote GPU capture is
active.

PFTL remains at the completed-run baseline:

| Item | Current value |
|---|---|
| PFTL height | `410` |
| Validator release revision | `90618294` |
| Mempool | Empty |
| State root | `12ea93322034fe5fcf092401ba0e71e2f7bb4edea9869ccb4752a30ad8c1bf3664b217240804ff1c7960cc9d80e5d48b` |
| A666 authorized supply | `31,489.197455` |
| A666 reserve principal | `103.000000` |
| Wrapped exposure | `31,489.197455` |
| Active issue reservations | `0` |
| Pending return imports | `0` |
| Ethereum spendable bridge supply | `0` |
| Supply invariant | Valid |

### Stopped Phase 9 deposit

A fresh hands-off verification was prepared and started. It was stopped at
the Ethereum-finality wait immediately after the deposit:

- amount: `1.005000 USDC`;
- vault: `0xaaa78FdA7062eFce769e95cd72Fc55e507BC8183`;
- transaction:
  `0x88f4c9ffc95568e1c44f422d8e7ba2162da70fb1fb753fd43b45458fd6cf4a48`;
- Ethereum block: `25,633,383`;
- deposit record: verified;
- PFTL mutation from this deposit: none;
- pfUSDC claim from this deposit: not created;
- A666 issue from this deposit: not started; and
- wA666 mint from this deposit: not started.

The vault currently holds `1.016000 USDC` with matching obligations. This
includes the prior `0.011000 USDC` spread balance and the stopped
`1.005000 USDC` deposit.

The deposit is recorded on Ethereum and has not been double-used. It is an
unfinished bridge transition, not a completed round trip. A future operator
must deliberately choose either to resume its exact proof/claim lineage or
use an audited recovery path. A second issue attempt must not be created
against the same deposit.

## What remains

### 1. Preserve the stopped state

- Commit the Phase 9 deposit evidence before any future mutation.
- Freeze its deposit ID, transaction, route binding, amount, and intended PFTL
  recipient.
- Verify again that all six validators remain at one unchanged state and that
  no Phase 9 reservation, claim, or export exists.

### 2. Resolve the Phase 9 deposit

Choose one explicit path:

- resume the exact deposit lineage through finalized ingress, primary issue,
  export, return, private redemption, and proof egress; or
- execute a separately reviewed recovery procedure.

Do not begin a replacement live-value run while this deposit is unresolved.

### 3. Run a clean transparent redemption verification

The transparent redemption path completed functionally, but its execution
crossed a recovery deployment and repair work. If a distinct formal A6 gate is
required, run it from a new proof-minted wrapper balance using one frozen
release and no post-funding changes.

### 4. Run a clean private redemption verification

The formal A8 run must:

- use one frozen release and committed manifest;
- require no command, configuration, or code change after deposit;
- use the production A666 and pfUSDC assets;
- keep note openings and keys out of evidence;
- prove exact issue, wrapper mint, return, private redemption, and USDC
  release;
- reject exact replays without state advancement;
- end with all six validators converged and empty mempools;
- preserve A666, wrapper, pfUSDC, vault, and spread conservation; and
- meet the `<= 25 minute` issue and `<= 25 minute` redemption targets.

### 5. Address latency as a release requirement

Ethereum finality is a real lower-bound component, but the prior measurements
also include avoidable serialized work and operator delay. Before another
live run:

- prewarm and health-check every prover;
- freeze prior verifier checkpoints before funding;
- validate all policy and manifest hashes before funding;
- ensure destination-consume and return-import proof builders are ready;
- remove duplicate or serial waits where safety permits;
- make every stage resumable without replaying a completed transition; and
- collect machine timestamps automatically rather than reconstructing them
  after the run.

If the chain-finality and proof architecture cannot reliably meet 25 minutes,
the release gate or architecture must be changed explicitly. A slow functional
pass must not be mislabeled as an SLO pass.

### 6. Package final evidence

The final packet must include:

- the frozen manifest and release hashes;
- Ethereum receipts and finalized checkpoint evidence;
- PFTL block certificates and state roots;
- proof reports and public values;
- balance and supply deltas;
- replay results;
- privacy/redaction results;
- latency measurements;
- an intervention ledger; and
- one machine-readable `PASS`, `FAIL`, or `RECOVERY_REQUIRED` verdict.

## Repository state

The pushed `main` head is
`0064bb32513f2298ccaf13f9a909637cd8d3685e`. Intended implementation work,
the completed Phase 8 evidence, the new orchestration scripts, and the Phase 9
pre-value intent are on the remote.

The Phase 9 deposit evidence was created after that commit and remains local.
The working tree also contains older untracked deployment and evidence
artifacts. Those artifacts must not be deleted, overwritten, or bulk-added
without a separate retention and redaction review.

## Primary references

- Acceptance specification:
  `docs/plans/A666-TRANSPARENT-PRIVATE-ISSUE-REDEEM-ACCEPTANCE-SPEC-20260728.md`
- A666 deployment status:
  `docs/status/A666-MAINNET-DEPLOYMENT-20260727.md`
- Completed private functional run:
  `docs/evidence/a666-acceptance-20260728/phase-8-private-redeem-verify/README.md`
- Phase 8 machine verdict:
  `docs/evidence/a666-acceptance-20260728/phase-8-private-redeem-verify/acceptance-summary.json`
- Stopped Phase 9 intent:
  `docs/evidence/a666-acceptance-20260728/phase-9-private-redeem-hands-off-verify/run-manifest.json`
- Stopped Phase 9 deposit:
  `docs/evidence/a666-acceptance-20260728/phase-9-private-redeem-hands-off-verify/deposit/deposit-result.json`

---

## Review comments (2026-07-28, independent codebase evaluation)

**Review verdict:** accurate, honest, well-evidenced handoff. No fund-risk
blockers. Safe to resume, with the sequencing below.

### Accuracy verification against the repository

Every verifiable claim in this document was cross-checked against the repo
and matched:

| Claim | Evidence checked | Result |
|---|---|---|
| Pushed `main` head `0064bb3...` | `git rev-parse HEAD` | Match |
| Phase 9 deposit evidence is local only | `git status` (untracked `deposit/`, `ingress/`, `pftl/`) | Match |
| `FAIL` with `functional_pass: true`, 5 interventions | `acceptance-summary.json` | Match |
| Latency `1848s` / `2688s` / `4668s` vs `1500s` SLO | `timing-summary.json` (block-timestamp derived) | Match |
| Five intervention causes | `defect-ledger.json`, 1:1 with regression guards | Match |
| Height `410`, state root, supply `31,489.197455`, spendable `0` | `acceptance-summary.json` `final` block | Match |
| Deposit tx `0x88f4...`, block `25,633,383`, vault `0xaaa7...` | `deposit/deposit-result.json` | Match |
| Economics: `1.005` in, `0.9995` out, `0.0055` cost | Arithmetic on issue/redeem spreads | Match |
| Keys and notes excluded from evidence | `redaction-scan.json` PASS (354 files) plus an independent secret-pattern scan of evidence and untracked `deployments/` | Match |

### No showstoppers

- `funds_at_risk: false`; conservation PASS; every rejected attempt failed
  before mutation; replay rejection proven on all three surfaces.
- The stopped deposit is recorded on Ethereum, un-double-used, and the vault
  balance matches obligations.
- All five defects were orchestration or preflight bugs, not protocol or
  accounting bugs, and each has a concrete regression guard in code
  (including one-line fixes `73e2f3c` and `3a5b970`).
- The frozen state is consistent: six validators converged, empty mempool,
  supply invariant valid.

### Real issues (manageable)

1. **The SLO will fail again as-is.** Redemption ran 79% over target; this
   was not a near-miss. Resuming without the Section 5 latency work (prover
   prewarm, pre-funding checkpoint freeze, parallelized waits) will produce
   another predictable `FAIL / functional_pass: true`. Either land the
   latency preparation first or explicitly change the gate.
2. **Fragile local state.** The Phase 9 deposit evidence is uncommitted and
   roughly 4.5 GB of deployment artifacts (46 entries, including full
   node-state snapshots) are untracked. A working-tree wipe would orphan the
   lineage of a live mainnet deposit. Commit the evidence and archive the
   artifacts out-of-band as part of this handoff, not as deferred work.
3. **Scripts are single-run tooling.** The orchestration scripts hardcode
   this run's amounts (`1000000` / `999500`), the holder address, asset IDs,
   the egress policy hash, and `log_index=1`. Bare `test` / `jq -e`
   assertions under `set -e` fail with exit codes only. Any resume with
   different constants will fail silently; a future operator must know the
   scripts are pinned to this exact run.
4. **Key-material hygiene is inconsistent with the stated policy.** Script
   defaults read mainnet holder and operator signing keys from
   `/home/postfiat/tmp/...` paths, the holder key is copied to validator 2
   per run, and `--note-seed-hex` passes seed material as a remote CLI
   argument (transiently visible in `ps`). Nothing leaks into evidence, but
   key locations and this exposure should be stated explicitly.

### Minor documentation gaps

- Section "Resolve the Phase 9 deposit" says to freeze the deposit ID but
  never states it; the identifier exists only in the uncommitted local JSON.
- `run-manifest.json` pins `orchestration_commit: 3a5b970`, one evidence-only
  commit behind the pushed head; correct, but worth stating to preempt a
  mismatch scare.
- The state table shows wrapped exposure equal to full authorized supply
  while spendable bridge supply is zero; one sentence on finalized-claims
  versus spendable supply would help the next operator.
- `defect-ledger.json` is high-value and belongs under Primary references:
  `docs/evidence/a666-acceptance-20260728/phase-8-private-redeem-verify/defect-ledger.json`

### Recommended resume order

1. Commit the Phase 9 deposit evidence and archive the untracked artifacts.
2. Complete the Section 5 latency preparation; freeze one release and
   manifest.
3. Resume the existing Phase 9 deposit lineage hands-off. Do not start a
   fresh deposit.
4. If the run fails on timing alone, decide the 25-minute gate question
   explicitly rather than rerunning unchanged.
