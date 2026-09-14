# QA campaign log — 2026-09-11

This is the canonical progress record for the
[burn 3 campaign](qa-campaign-20260911-burn3-brief.md). The campaign began at
2026-09-11T10:43:34Z from clean `main` commit
`c25b3389d5c221ff1c23e700d53460003cc50b2a`. It is a review-and-repair
campaign, not a release, deployment, or live-authority action.

**Status:** Closed. A1 through A5 and B are done; the inventory extension
passed its first full Text Improvement Harness gate at 88.87/100. No release,
deployment, or live activation occurred.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Storage and snapshots | done | 1 | 3 | 2 | [Review](storage-snapshots-review-20260911.md); findings `8533f5d7`, `0d015227`; repairs `69e1f1ce` |
| A2 | Execution | done | 1 | 2 | 0 | [Review](execution-review-20260911.md); findings `0449de41`; repairs `e95efbdf` |
| A3 | Cobalt ratification | done | 1 | 1 | 2 | [Review](cobalt-ratification-review-20260911.md); findings `b5c16c2c`; repairs `c9a61fcd` |
| A4 | Network and mempool admission | done (repaired) | 2 | 1 | 1 | [Review](network-mempool-review-20260911.md); findings `13a23d91`; repairs `f2dea308` |
| A5 | Operational Python CLIs | done (repaired) | 1 | 7 | 3 | [Review](operational-clis-review-20260911.md); findings `b25e3bec`; repairs `c2724977` |
| B | Defect inventory and TIH gate | done | — | — | — | [Inventory](defect-inventory-20260910.md) extended to 89 rows in `2eec1ece`; first full gate 88.87/100; run group `qa-defect-inventory-burn3-20260914`; scored SHA-256 `2e4adbc6f78686af8316291dca516827a2d7eea988848cb5705bf5ee9cf7311f` |

Current finding totals: **6 P1, 14 P2, 8 P3**.

## Operational Python CLIs review result

All nine A5 files were reviewed in the required order: `wallet.py`,
`client.py`, `persistent_client.py`, `pftl_transfer.py`, `navcoin.py`,
`hyperliquid.py`, `cobalt.py`, `genesis_registry.py`, and
`storage_scaling.py`. No A5 file was left unreviewed. The prior
`tasknode_unl` modules were outside A5. The
[operational CLI review](operational-clis-review-20260911.md) records one P1:
the wallet could mark an unsuccessful transfer finalized from a block height
without a matching accepted receipt. Seven P2s concern incomplete account
history being presented as complete, implicit faucet state application,
incorrect NAV asset identity, missing venue fields and unbounded venue response
bodies, remote shadow catch-up without a separate interlock, and inconsistent
genesis receipt deadlines. Three P3s remain recorded: nonintegral example
NAV valuations, rounded PFTL report amounts, and unbounded packet-tree
enumeration.

Repair `c2724977` requires a matching accepted certified receipt for transfer
success, validates account-history metadata, adds explicit faucet and shadow
catch-up interlocks, derives the NAV asset ID from the chain-bound canonical
algorithm, rejects missing or oversized venue responses, and checks receipt
deadlines together. It adds focused regressions for every P1/P2 finding and
updates the NAV example to require an explicit chain ID. No A5 repair is
consensus-affecting, deployed, or activated.

Verification:

- `PYTHONPATH=python python3 -m pytest -q python/tests/test_wallet.py python/tests/test_persistent_client.py python/tests/test_pftl_transfer.py python/tests/test_navcoin.py python/tests/test_cobalt.py python/tests/test_genesis_registry.py python/tests/test_storage_scaling.py python/tests/test_constrained_signer.py`:
  183 passed, 3 skipped, 44 subtests passed.
- `PYTHONPATH=python python3 docs/examples/navcoin_mint_and_nav.py --chain-id postfiat-navcoin-devnet`:
  completed without error.
- `python3 -m compileall -q python/postfiat_rpc/{wallet,client,pftl_transfer,navcoin,hyperliquid,cobalt,genesis_registry}.py`
  and `git diff --check`: passed.

## Network and mempool admission review result

The crate review and node reachability trace found two P1 defects in the
long-running validator transport. The service could spawn one operating-system
thread for every pre-authentication TCP connection up to its lifetime budget,
and one unauthenticated persistent connection could submit unlimited rejected
frames while every status-bearing rejection was retained in memory and appended
to the optional event log. One P2 affected the standalone batch service: only
successful batches consumed its termination budget, so unauthenticated
rejections could grow its report without bound. The [network and mempool review](network-mempool-review-20260911.md)
also records one unfixed P3: the unreachable legacy ordering API can
deserialize a validator set with a caller-selected false quorum.

Repair `f2dea308` limits the validator service to 16 simultaneous connection
workers independently of its lifetime connection budget, caps each connection
at 4,096 requests, closes a connection after a rejection, and retains at most
1,024 response or rejection summaries while maintaining exact saturating
counters. The standalone batch service now derives a bounded rejection budget
from `max_batches` and fails closed when it is exhausted. No A4 repair is
consensus-affecting.

Verification:

- `cargo check -p postfiat-node --locked`: passed.
- Focused validator accept-loop and resource-bound tests: 5 passed, including
  the new bounded-resource regression.
- Pre-repair crate baseline: 9 network, 15 mempool DAG, and 32 ordering tests
  passed.

## Cobalt ratification review result

The Cobalt pass found one P1: old/new safety witnesses compare raw subset
membership overlap rather than the minimum possible overlap of valid quorums.
A five-of-seven single rotation can therefore pass with an old/new quorum
intersection no larger than the Byzantine budget. One P2 is recorded: signed
DABC pending pairs bind a candidate ID, but activation checks only that their
slot is ratified. Both repairs tighten ratification or transition admission and
will be labeled consensus-affecting. The [Cobalt review](cobalt-ratification-review-20260911.md)
also records two unfixed P3s: the unused live-mode beacon abstraction has no
authentication material, and the frozen first-oracle input validator permits
incomplete validator classifications.

Pre-repair verification:

- `cargo test -p postfiat-consensus-cobalt -p postfiat-cobalt-decision-oracle -p postfiat-cobalt-adversarial-oracle --locked`:
  72 Cobalt tests, 9 genesis registry checker tests, and 3 tests in each oracle
  passed.

Repair `c9a61fcd` now evaluates the minimum possible old/new quorum
intersection and binds every signed DABC pending candidate ID to the ratified
candidate at that slot. Both changes are consensus-affecting and remain
source-only.

Post-repair verification:

- Cobalt: 73 tests; genesis registry checker: 9 tests; both oracles: 3 tests
  each.
- The focused safety-witness example passed all six checks.
- Node Cobalt authority: 1 test; Cobalt shadow/runtime: 15 tests.
- Strict clippy for the Cobalt crate and both oracles, plus workspace
  formatting, passed.

## Execution review result

The full execution crate and its node-side state-transition, archive-replay,
proposal, commit, and validator-registry entry points were reviewed. The pass
found one P1: a repeated transaction produces duplicate receipt IDs that
proposal construction and validation accept, although ordered commit rejects
them after certification. Two P2s are recorded: the ordered OwnedDeposit path
bypasses the declared 100,000-object cap, and proposal construction can certify
a replicated state value above the storage layer's 256 MiB serialization
limit. All three repairs affect consensus admission or state-transition results
and will be labeled consensus-affecting. The findings are detailed in the
[execution review](execution-review-20260911.md).

Pre-repair verification:

- `cargo test -p postfiat-execution --locked`: 196 passed.

Repair `e95efbdf` rejects duplicate receipt IDs at proposal construction and
supplied-proposal validation, applies the declared object cap to every owned
value path, and performs a write-free exact state-file-size check before a
proposal can expose an unpersistable state root. The duplicate and size checks
cover validator reconstruction as well as local proposal creation. No on-disk
format changed.

Post-repair verification:

- `cargo test -p postfiat-execution --locked`: 198 passed.
- `cargo test -p postfiat-storage --locked`: 90 passed, 2 ignored; process-crash
  integration: 1 passed.
- Duplicate-receipt proposal regressions: 2 passed; serialized state-size
  regression: 1 passed.
- Node block-proposal tests: 3 passed; validator-registry continuation tests: 6
  passed.
- Focused transparent asset, replay, NFT, offer, and atomic-swap ordering tests:
  5 passed.
- Strict clippy for `postfiat-execution`, `postfiat-storage`, and
  `postfiat-node`, and the workspace formatting check, passed.

All three A2 repairs are consensus-affecting. They are source-only and were not
activated or deployed.

## Storage review result

The full `crates/storage` surface and the node snapshot, restore, checkpoint,
migration, commit-recovery, and writer-lease paths were reviewed. The pass found
one P1 source defect: a torn FastSwap WAL suffix remains in place, so a later
synced record can be appended behind it and become unreplayable. It also found
two P2 source defects: failed snapshot imports can publish partial destination
state, and FastSwap WAL reads can allocate an unbounded whole file while total
WAL growth has no fence. Two comparison-only P3 defects are recorded without
repair: destructive ordered-index replacement and a duplicate legacy receipt
materialization write. The block-924 snapshot source repair is present in the
deployed source ancestry, but no post-repair fleet export receipt exists; the
remaining P2 evidence gap cannot be closed without a prohibited host write.

Repair `69e1f1ce` now truncates torn FastSwap WAL tails before later appends,
fences total WAL and bounded-file reads before allocation, and publishes a
snapshot or complete lifecycle validator root only after every verification
passes. Transactional generation pointers are rebound before the atomic move,
so active-storage restores remain usable at the final path. The mutable chain
state now separates deployed source repair ancestry from the missing fleet
export receipt. No A1 repair is consensus-affecting.

Pre-repair verification:

- `cargo test -p postfiat-storage --locked`: 88 passed, 2 ignored; process
  crash integration: 1 passed.
- `cargo test -p postfiat-node snapshot --lib --locked`: 22 passed.
- `cargo test -p postfiat-node lifecycle_checkpoint --lib --locked`: 7 passed.

Post-repair verification:

- `cargo test -p postfiat-storage --locked`: 89 passed, 2 ignored; process
  crash integration: 1 passed.
- `cargo test -p postfiat-node snapshot --lib --locked`: 24 passed.
- `cargo test -p postfiat-node lifecycle_checkpoint --lib --locked`: 7 passed.
- `cargo test -p postfiat-node validator_registry_continuation --lib --locked`:
  6 passed.
- Focused transactional migration regressions: 2 passed.
- Strict clippy for `postfiat-storage` and `postfiat-node`, and the workspace
  formatting check, passed.

## Skips and boundary decisions

- No Task Node action occurred.
- No fleet, chain, deployment, restart, configuration, or remote-host mutation
  occurred.
- A post-repair fleet snapshot export was skipped because it would write to a
  validator host. Source ancestry and local regressions do not substitute for
  that operational evidence.
- Ordered-history index publication and the duplicate legacy receipt write are
  P3 findings, so they are recorded but not repaired.
- No frozen artifact or out-of-scope bridge, proof, program, or Orchard/privacy
  source was modified.
- No broad workspace or Orchard/Halo2 suite was started; no burn 3 repair
  crossed an Orchard boundary.
- Three genesis-registry tests requiring the out-of-tree archived round
  fixture were skipped; the local golden vectors and synthetic receipt
  deadline regression passed.
- A5 reviewed all nine listed files; no A5 file was skipped.
- No inventory wording rewrite or rescore was needed: the first compliant
  15-review gate averaged 88.87/100, above the 86/100 stop condition.

## Verification

The mandatory strict documentation build, `scripts/public-doc-links`, and
`scripts/public-secret-scan` passed before both B commits and each earlier
campaign commit. A5 ran
`PYTHONPATH=python python3 -m pytest -q python/tests/test_wallet.py python/tests/test_persistent_client.py python/tests/test_pftl_transfer.py python/tests/test_navcoin.py python/tests/test_cobalt.py python/tests/test_genesis_registry.py python/tests/test_storage_scaling.py python/tests/test_constrained_signer.py`:
183 passed, 3 skipped, and 44 subtests passed. B changed only documentation:
all 61 earlier inventory rows were verified unchanged, the 28 new IDs and 89
unique rows were counted, and the scored file hash was checked against the
15 fresh SQLite score records. No Rust, Python, workspace, or Orchard suite
was rerun for B. Pushed repair and inventory commit IDs are recorded in the
campaign state table.

## Scores

The exact extended inventory bytes, SHA-256
`2e4adbc6f78686af8316291dca516827a2d7eea988848cb5705bf5ee9cf7311f`,
received fifteen fresh OpenRouter reviews at temperature 0 with an 8,000-token
response limit. The prompt was
`Rate this document on a scale of 1-100. Output the score and your reasoning.`
The credential came from vault label `openroutertih` and was passed to the
harness in memory.

| Judge | Scores | Average |
| --- | --- | ---: |
| `openai/gpt-6-astra-pro` | 90, 90, 90, 89, 90 | 89.80 |
| `anthropic/claude-fable-5.1` | 88, 84, 89, 86, 87 | 86.80 |
| `z-ai/glm-5.3` | 90, 88, 90, 92, 90 | 90.00 |
| **All fifteen** | — | **88.87** |

Run group: `qa-defect-inventory-burn3-20260914`. The first compliant full
score exceeded the 86/100 gate; the inventory was not rewritten or rescored.
The external score log and SQLite record are under
`/home/postfiatchad/pastedocs/.qa-campaign-defect-inventory-burn3-20260914/`;
their SHA-256 values are respectively
`8e0806297b1b96afd1c3f3a1e7ffd52404b3889e2a09053b057e1e06d9beaf60` and
`2aac91acfc73b27c0052e3484aecbe98b07ad84453485e566fc3a347b96e4fe9`.

## Final summary

| Surface | P1 | P2 | P3 | Repair commits |
| --- | ---: | ---: | ---: | --- |
| A1 — Storage and snapshots | 1 | 3 | 2 | `69e1f1ce` |
| A2 — Execution | 1 | 2 | 0 | `e95efbdf` |
| A3 — Cobalt ratification | 1 | 1 | 2 | `c9a61fcd` |
| A4 — Network and mempool admission | 2 | 1 | 1 | `f2dea308` |
| A5 — Operational Python CLIs | 1 | 7 | 3 | `c2724977` |
| **A1–A5 total** | **6** | **14** | **8** | — |

B added all 28 findings to the consolidated inventory: 6 STO-, 3 EXE-, 4
COB-, 4 NET-, and 11 OPS- rows. The 89-row inventory contains 25 P1, 44 P2,
and 20 P3 rows: 48 fixed, 16 dispositioned, five previously retained, eight
burn 3 P3 recorded without repair, six needing a live environment, and six
needing an operator decision. It scored **88.87/100** on the first full Text
Improvement Harness gate, run group
`qa-defect-inventory-burn3-20260914`, scored file SHA-256
`2e4adbc6f78686af8316291dca516827a2d7eea988848cb5705bf5ee9cf7311f`.
No rescore was needed.

Consensus-affecting repairs, all source-only and not activated or deployed:

- Duplicate receipt-ID rejection during local proposal construction and
  supplied-proposal validation — `e95efbdf`.
- The declared owned-object cap applied to every owned value path, including
  `OwnedDeposit` — `e95efbdf`.
- Exact serialized state-size admission before an unpersistable state root can
  be proposed or accepted — `e95efbdf`.
- Minimum possible old/new quorum intersection required by Cobalt transition
  safety witnesses — `c9a61fcd`.
- Signed DABC pending candidate IDs bound to the ratified candidate at each
  slot — `c9a61fcd`.

Remaining risks:

- A1 still lacks a post-repair fleet snapshot export receipt, and retains the
  P3 destructive ordered-index replacement and duplicate legacy receipt write.
- A3 retains the P3 unauthenticated live-mode beacon abstraction and incomplete
  first-oracle validator classifications.
- A4 retains the P3 deserializable legacy validator set with a false quorum;
  no unauthenticated production reachability was found.
- A5 retains three P3 observations: the nonintegral NAV example operation,
  rounded PFTL display amounts, and unbounded packet-tree enumeration.
- All burn 3 repairs remain source-only; no deployment or live activation was
  performed.

A1 through A5 and B are closed. The inventory extension and closeout are
pushed to origin `main`.
