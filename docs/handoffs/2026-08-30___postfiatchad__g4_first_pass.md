# G4 storage-scaling qualification: first PASS

- **Operator:** Post Fiat (`postfiatchad`)
- **Date:** 2026-08-30 UTC

## BLUF

The seventh one-run campaign **passed every G4 gate**: consensus height ratio
`1.0581` and wallet-to-finality `1.0623` against the 1.10 limits, all 15
windows complete, every correctness, convergence, bounded-work,
migration-position, coverage, comparison, and height-model gate green, and
`evidence_eligible = true` — the first passing storage-scaling campaign in the
project. The selected transactional path is ~2.9× faster than legacy at
height 50. G4 is closed **PASS**. G5 packet assembly remains blocked only on
the two standing external inputs: re-supplying the height-915 quarantine
archive and naming the height-924 custodian.

## Final identities

| Item | Value |
| --- | --- |
| Candidate source | `d0ae79f3342fc78cbf907dbf231a60de8bc40606` (`main`) |
| Candidate binary | SHA-256 `9e82d9286d79307b6246a773882e744ade1abad6b10498c3ed2d9c9e6b78c80c` |
| G1 / G2 manifests | `8df8f7a6…e7dff` / `689a96dc…609bc` (rollback PASS, tamper/crash PASS) |
| Runner/verifier | `ae6ec9cbc4781e9a0127ea2a8f95b41949e16589`; helper `bdff4bd8…f7f6` |
| Prepared input | `ec5a4715…98c3`; fleets unchanged from `ae658441` with provenance preserved |
| Campaign report | SHA-256 `e2cff9cde8c3c9e300393af924a63dc42b2451dacc90079df61dbdf66a6d999f` |
| Campaign checkpoint | SHA-256 `f05dca5346e002a05aaa58790841246fbc45a8ae16d56ac11667ac93d3dbcc3c` |
| Private output | `~/repos/postfiat-storage-g4-measurement-ae6ec9cb-d0ae79f3-v1` — never commit, publish, or delete |

## How the pass was reached (seven campaigns, every closure documented)

1. Certified-send eager index migration fixed the first contract mismatch.
2. Vote-lock eager marker fixed the same defect class in its sibling module.
3. Batched, fsync-lean completed-index mutations cut the at-cap resume from
   ~205 ms to ~56 ms (2.69 → ~1.10 ratio across campaigns).
4. Post-finality maintenance moved retention bookkeeping off the
   client-visible span (~1.05 ratio), with the per-pass sweep capped at five
   so the frozen work-gate arithmetic stays exact.
5. Two reviewed, operator-approved runner amendments: one isolated bounded
   (100–250 ms) residual round tolerated per window (KVM scheduler steal), and
   a 5 ms absolute materiality floor in the stage height model (the selected
   store's inherent ~1.7 ms O(log-height) commit drift). The 1.10 ratio
   thresholds were never touched.

Each of the six failed campaigns is closed in its own plan document with a
single diagnosis and no retries or relabeling; all private outputs are
preserved (reproducible fleet clones inside closed failed runs were deleted
after digest recording to recover disk).

## What remains before public testnet

| Item | Owner | State |
| --- | --- | --- |
| G3 height-915 replay on binary `9e82d928…8c80c` | Operator: re-supply the quarantine archive (not on this host) | Blocked externally |
| G3 height-924 exact replay | Operator: name custodian, authorize one read-only copy | Blocked externally |
| G5 redaction-safe packet + offline verifier | Local, mechanical once G3 exists | Waiting on G3 |
| G6 six-clone rehearsal, deployment decision, testnet gates | Later, separately authorized | Sequenced after G5 |

No devnet, deployment, Task Node, or live-fleet action occurred at any point.
