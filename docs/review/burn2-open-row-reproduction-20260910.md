# Burn 2 open-row reproduction — 2026-09-10

Status: all 19 rows that entered burn 2 as open have a bounded outcome

This pass used local source, committed receipts, immutable Git objects, and the
read-only StakeHub branch. It made no Task Node request, fleet query, live-chain
write, deployment, service change, external purchase, or StakeHub mutation.
Historical and deployment evidence was read without modification.

## Bounded outcomes

| Row | Outcome | Bounded evidence and remaining condition |
| --- | --- | --- |
| `ARC-03` | Reproduced | Source still pins Fulu epoch `411648` in the dedicated guest/tool and `411392` in the shared guest/capture path. This is P3, so burn 2 records it without changing guest source or frozen artifacts. |
| `RPC-01` | Reproduced — fixed in source | The diagnosis remains valid. Commit `15af691d` makes generated and example RPC units use `Restart=always`, with a deployment-unit regression. The fleet was not changed; recurrence remains possible there until a separately authorized rollout. |
| `UNL-04` | Reproduced | `advance_shadow_round` still checks only frozen status and `PROPOSE_ADD`, then trusts the caller-supplied candidate. It is P3 and confined to caller-owned hypothetical state, so it remains recorded. |
| `SCT-04` | Reproduced — fixed | Before repair, the consolidated verifier exited with the recorded semantic-verifier error while E5 passed. Commit `acdbb1f2` binds publication hashes to immutable revision `41202067`; the 20-test Cobalt suite and both packet verifiers pass. |
| `SCT-05` | Reproduced — fixed | The Rust storage source states that a writer excludes sibling read-only opens and uses operation-scoped leases. Commit `acdbb1f2` adds a post-lock correction to the plan without rewriting its scored body. |
| `SCT-06` | Needs live environment | A sustained overlapping public workload with concurrent transport/RPC operations and measurable writer fairness is required. The serialized controlled-devnet gate cannot answer it. |
| `SCT-07` | Needs live environment | Current disk-capacity and growth telemetry from every validator host is required. The campaign's allowed ledger/status endpoints do not expose it. |
| `SCT-08` | Needs operator decision | A separately authorized evidence campaign must obtain complete signed bindings, publisher-attested work digests, exclusions, admission facts, and a native L1 registry-root binding. Burn 2 forbids Task Node action. |
| `SCT-10` | Needs operator decision | Independent operators and authorization for an end-to-end proposal/ratification rehearsal are prerequisites; Foundation-controlled local fixtures cannot substitute. |
| `SH-10` | Reproduced | Read-only Git object `8f6f27cf` copies configuration into a selected release directory before aggregate verification and does not reject reuse of the active release ID. StakeHub is outside this repository's writable scope. |
| `SH-11` | Reproduced | The same Git object persists success from `report.round_ok` without binding the process exit and exact certified-batch receipt. StakeHub remains read-only. |
| `SH-12` | Reproduced | The same Git object combines unbound `round_ok` with fleet height rather than proving application of the intended governance batch. StakeHub remains read-only. |
| `PI-16` | Needs operator decision | Independence cannot be self-asserted. An operator must assign an independent cryptography audit and define the separate YOLO target-proof scope. |
| `PI-18` | Needs live environment | The local readiness check still reports `qualified=0/6 stakehub_deprecated=false`. Six production-source qualification packets and their governed environment are required. |
| `SQ-01` | Needs operator decision | The retained receipt records two hashes differing in six RUNPATH bytes. A fresh local release link still embeds a randomized `rustc2PrtGo/raw-dylibs` path. An operator must approve a reproducible toolchain/link or normalization contract before requalification. |
| `SQ-02` | Needs operator decision | `git merge-base --is-ancestor 707e006f HEAD` returns 1 and `pftl_source_settlement.rs` remains absent, while `bbb291ce` is an ancestor. The release-lineage reconciliation choice remains explicit. |
| `SQ-03` | Needs live environment | The authorized receipt still has only the height-931 all-six snapshot versus the recorded height-1020 fleet. A current simultaneous all-six snapshot is required. |
| `SQ-04` | Needs live environment | A fresh bounded local search found 50 files named `postfiat-node` and zero with deployed SHA-256 `57b0f4d1…634eec83`. The exact rollback binary must be retrieved through a separately authorized operational path. |
| `SQ-06` | Needs operator decision | Source contains `bbb291ce`, but deployment remains unauthorized and the recorded fleet identity is pre-fix. The lineage, reproducibility, current-state, rollback, and rollout decisions must be made together. |

## Verification summary

- Cobalt unit tests: **20 passed**.
- Consolidated adversarial packet: **pass**, manifest
  `78e375ccfa09914dd1ea15c429ca1aff70137173bd84021720613b63fe6e2d63`.
- Independent E5 packet: **pass**, manifest
  `0695284a7b38ac0129c47e1242f4a2227ad25096147920e79569a924e5f3b3db`.
- A666 public-adapter readiness: **pass as a checker**, still
  **0/6 qualified** and `stakehub_deprecated=false`.
- Deployed rollback binary local search: **0/50 matches**.

“Needs live environment” and “needs operator decision” are bounded outcomes,
not completed qualifications or permissions. None of this evidence is live
authority.
