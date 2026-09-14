# QA defect inventory — 2026-09-10

Status: burn 2 inventory reconciled; burn 3 findings appended. The burn 2
Text Improvement Harness result is in the [2026-09-10 campaign log](qa-campaign-20260910.md);
the burn 3 gate is in the [2026-09-11 campaign log](qa-campaign-20260911.md).

This inventory separates demonstrated failures from missing evidence, operating
assumptions, and capabilities that have not been built or activated. A
**reproduced defect** has an observed failing case. An **evidence gap** means the
available record cannot establish the claim. An **economic assumption** is a
load or incentive premise rather than a code result. A **proposed capability**
must not be described as shipped or authoritative.

## Campaign findings

| ID | Classification | Severity | Status | Source and reproduction |
| --- | --- | --- | --- | --- |
| ARC-01 | Reproduced defect | P1 | Fixed — `dbc73fea`; deployed controller still open | [Arc review](arc-facing-review-20260910.md#findings): a valid V2 source export could expire without a controller cancellation event matching the refund verifier. |
| ARC-02 | Reproduced defect | P1 | Fixed — `dbc73fea` | [Arc review](arc-facing-review-20260910.md#findings): invoking the shell entrypoint entered live setup without an explicit execution ceremony. |
| ARC-03 | Reproduced defect | P3 | Reproduced — P3 recorded only | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): the dedicated ingress pin remains 411648 while the shared path uses 411392. Frozen guest artifacts were not regenerated. |
| RPC-01 | Reproduced defect | P1 | Reproduced — fixed in source `15af691d`; fleet unchanged | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md) and [read-only validator-0 diagnosis](../status/chain-state-current.md#validator-0-rpc-diagnosis-20260910): generated and example RPC units now rotate after clean finite-budget exits. |
| CS-01 | Reproduced defect | P1 | Fixed — `f9f13ead` | [Consensus and storage review](consensus-storage-review-20260910.md#findings): a lower-view call could persist authorization, lose the safety guard to a higher-view call, and emit its signature only after the durable round floor advanced. Signing now completes under the guard. |
| CS-02 | Reproduced defect | P2 | Fixed — `f9f13ead` | [Consensus and storage review](consensus-storage-review-20260910.md#findings): distinct YOLO registrations appended to committed validator state without a count bound or state-expansion fee. Both row classes are now capped and charged. |
| WRS-01 | Reproduced defect | P1 | Fixed — `83488d91` | [Wallet/proxy/RPC SDK review](wallet-proxy-rpc-sdk-review-20260910.md#1-p1-transfer-quote-signing-trusts-an-rpc-selected-recipient-and-amount): the transfer signer sourced recipient and amount from an untrusted quote without binding the reviewed request. |
| WRS-02 | Reproduced defect | P1 | Fixed — `83488d91` | [Wallet/proxy/RPC SDK review](wallet-proxy-rpc-sdk-review-20260910.md#2-p1-the-loopback-session-token-endpoint-accepts-a-dns-rebinding-host): a same-origin rebinding Host could satisfy the loopback token endpoint and receive its bearer credential. |
| WRS-03 | Reproduced defect | P2 | Fixed — `83488d91` | [Wallet/proxy/RPC SDK review](wallet-proxy-rpc-sdk-review-20260910.md#3-p2-websocket-mutation-admission-releases-the-process-wide-slot-before-work-starts): WebSocket mutations released shared concurrency admission before routing or upstream I/O. |
| WRS-04 | Reproduced defect | P2 | Fixed for new vaults — `83488d91`; legacy vaults remain readable | [Wallet/proxy/RPC SDK review](wallet-proxy-rpc-sdk-review-20260910.md#4-p2-the-maintained-extension-permits-cheaply-brute-forced-new-vaults): four-character extension passphrases under the 100,000-iteration vault format permit cheap offline recovery after profile theft. |
| WRS-05 | Reproduced defect | P2 | Fixed — `83488d91` | [Wallet/proxy/RPC SDK review](wallet-proxy-rpc-sdk-review-20260910.md#5-p2-the-extension-popup-is-invalid-as-a-browser-module): an unmatched brace caused Chrome's module parser to reject the maintained extension popup while the Node syntax gate missed it. |
| UNL-01 | Reproduced defect | P1 | Fixed — `1c10f828` | [Task Node UNL review](tasknode-unl-review-20260910.md#1-p1-a-complete-score-window-passes-without-a-renewed-vouch-or-post-epoch-co-work): accounts with no bilateral records return `READY` after score-only continuity. |
| UNL-02 | Reproduced defect | P2 | Fixed — `1c10f828` | [Task Node UNL review](tasknode-unl-review-20260910.md#2-p2-a-stale-score-replay-can-suppress-fresh-score-evidence-by-input-order): stale-first and fresh-first orderings of the same digest produce different continuity decisions. |
| UNL-03 | Reproduced defect | P2 | Fixed — `1c10f828` | [Task Node UNL review](tasknode-unl-review-20260910.md#3-p2-valid-identifiers-can-inject-markdown-structure-into-the-operator-report): an accepted newline/backtick identifier creates an attacker-chosen heading in the root-valid human report. |
| UNL-04 | Reproduced defect | P3 | Reproduced — P3 recorded only | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): an in-memory report candidate replacement still bypasses a saturated-group hold in caller-owned hypothetical state. |

## Burn 3 campaign findings

These 28 findings are from the [burn 3 campaign](qa-campaign-20260911.md).
Consensus-affecting repairs remain source-only and were not activated or deployed.

| ID | Classification | Severity | Status | Source and reproduction |
| --- | --- | --- | --- | --- |
| STO-01 | Reproduced defect | P2 | Fixed — `69e1f1ce` | [Storage review](storage-snapshots-review-20260911.md): a truncated validator file left earlier restored validators at the final destination and prevented a corrected retry. |
| STO-02 | Reproduced defect | P2 | Fixed — `69e1f1ce` | [Storage review](storage-snapshots-review-20260911.md): FastSwap WAL append had no total growth fence; reopen and artifact reads allocated oversized files before enforcing bounds. |
| STO-03 | Reproduced defect | P3 | Recorded, not fixed | [Storage review](storage-snapshots-review-20260911.md): a crash after deleting the legacy ordered-history index and before renaming its replacement leaves no usable index generation. Comparison-only backend. |
| STO-04 | Reproduced defect | P3 | Recorded, not fixed | [Storage review](storage-snapshots-review-20260911.md): legacy receipt compaction performs the same atomic state write twice, doubling I/O without improving crash safety. Comparison-only backend. |
| STO-05 | Evidence gap | P2 | Needs live environment — source clarification `69e1f1ce`; fleet export receipt missing | [Storage review](storage-snapshots-review-20260911.md): the block-924 source repair predates the deployed base, but no post-repair signed fleet snapshot export establishes backup usability. |
| STO-06 | Reproduced defect | P1 | Fixed — `69e1f1ce` | [Storage review](storage-snapshots-review-20260911.md): a torn FastSwap WAL suffix remained after replay, so a subsequent synced vote record appended behind it could not be replayed after restart. |
| EXE-01 | Reproduced defect | P1 | Fixed — `e95efbdf`; consensus-affecting, source-only; not activated or deployed | [Execution review](execution-review-20260911.md): a repeated transaction generated duplicate receipt IDs eligible for certification but rejected at ordered commit, halting that height. |
| EXE-02 | Reproduced defect | P2 | Fixed — `e95efbdf`; consensus-affecting, source-only; not activated or deployed | [Execution review](execution-review-20260911.md): ordered `OwnedDeposit` bypassed the 100,000-object cap and could commit object 100,001. |
| EXE-03 | Reproduced defect | P2 | Fixed — `e95efbdf`; consensus-affecting, source-only; not activated or deployed | [Execution review](execution-review-20260911.md): validators could certify a state root whose serialized state file exceeded storage's 256 MiB limit and could not be persisted. |
| COB-01 | Reproduced defect | P1 | Fixed — `c9a61fcd`; consensus-affecting, source-only; not activated or deployed | [Cobalt review](cobalt-ratification-review-20260911.md): a five-of-seven single rotation passed a raw subset-overlap witness although valid old/new quorums could intersect in only two Byzantine validators. |
| COB-02 | Reproduced defect | P2 | Fixed — `c9a61fcd`; consensus-affecting, source-only; not activated or deployed | [Cobalt review](cobalt-ratification-review-20260911.md): signed DABC pending pairs could name a conflicting candidate at a ratified slot and still pass activation. |
| COB-03 | Reproduced defect | P3 | Recorded, not fixed | [Cobalt review](cobalt-ratification-review-20260911.md): the unused live-mode beacon coin accepts a caller-selected output bit without signer, signature, or round authentication. |
| COB-04 | Reproduced defect | P3 | Recorded, not fixed | [Cobalt review](cobalt-ratification-review-20260911.md): the first oracle counts unclassified available validators as responsive correct nodes; frozen scenarios classify all validators. |
| NET-01 | Reproduced defect | P1 | Fixed — `f2dea308` | [Network review](network-mempool-review-20260911.md): thousands of pre-authentication connections could each spawn a validator transport thread before exhausting the lifetime budget. |
| NET-02 | Reproduced defect | P1 | Fixed — `f2dea308` | [Network review](network-mempool-review-20260911.md): one unauthenticated persistent connection could send unlimited rejected frames and grow retained summaries and optional event logs. |
| NET-03 | Reproduced defect | P2 | Fixed — `f2dea308` | [Network review](network-mempool-review-20260911.md): standalone batch service rejections did not consume its termination budget and accumulated an unbounded report. |
| NET-04 | Reproduced defect | P3 | Recorded, not fixed | [Network review](network-mempool-review-20260911.md): a deserialized legacy validator set with a false quorum could certify one vote; no unauthenticated production path was found. |
| OPS-01 | Reproduced defect | P1 | Fixed — `c2724977` | [Operational CLI review](operational-clis-review-20260911.md): wallet transfer polling treated a positive block height or unrelated/rejected receipt as finalized payment. |
| OPS-02 | Reproduced defect | P2 | Fixed — `c2724977` | [Operational CLI review](operational-clis-review-20260911.md): malformed or missing `account_tx` history metadata became an empty complete scan and hid transfers. |
| OPS-03 | Reproduced defect | P2 | Fixed — `c2724977` | [Operational CLI review](operational-clis-review-20260911.md): a default faucet data directory could trigger local validator-state application without explicit acknowledgement. |
| OPS-04 | Reproduced defect | P2 | Fixed — `c2724977` | [Operational CLI review](operational-clis-review-20260911.md): NAV operation bundles used an unchained SHA-384 asset ID instead of the canonical chain-bound SHA3-384 identity. |
| OPS-05 | Reproduced defect | P2 | Fixed — `c2724977` | [Operational CLI review](operational-clis-review-20260911.md): missing venue balances or positions became zero-valued observations with plausible roots. |
| OPS-06 | Reproduced defect | P2 | Fixed — `c2724977` | [Operational CLI review](operational-clis-review-20260911.md): an unbounded venue HTTP body could exhaust observer memory during JSON decoding. |
| OPS-07 | Reproduced defect | P2 | Fixed — `c2724977` | [Operational CLI review](operational-clis-review-20260911.md): Cobalt shadow catch-up could mutate a remote service without its own operator acknowledgement. |
| OPS-08 | Reproduced defect | P2 | Fixed — `c2724977` | [Operational CLI review](operational-clis-review-20260911.md): a proposed genesis registry accepted receipts with incompatible deadlines but bound only the first deadline. |
| OPS-09 | Reproduced defect | P3 | Recorded, not fixed | [Operational CLI review](operational-clis-review-20260911.md): a nonintegral NAV example floors its value and emits a native operation rejected by the exact-equality check. |
| OPS-10 | Reproduced defect | P3 | Recorded, not fixed | [Operational CLI review](operational-clis-review-20260911.md): floating-point conversion rounds large valid PFTL amounts in CLI reports despite exact atom counts. |
| OPS-11 | Reproduced defect | P3 | Recorded, not fixed | [Operational CLI review](operational-clis-review-20260911.md): offline packet-tree verification enumerates and allocates millions of entries before checking its 4,096-file cap. |

## Storage, Cobalt, and Task Node review

The source is the historical [2026-09-06 review](https://github.com/postfiatorg/postfiatl1v2/blob/cfdeccd6897bfc8a80bfa2a95a6fb04f603c1cee/docs/review/storage-cobalt-tasknode-handoff-review-20260906.md).

| ID | Classification | Severity | Status | Source and reproduction |
| --- | --- | --- | --- | --- |
| SCT-01 | Reproduced defect | P1 | Fixed and deployed — 2026-09-06 RPC cache release | Transactional finality advanced every RPC to block 992 while cached status returned six different older heights. |
| SCT-02 | Reproduced defect | P2 | Fixed — `1267df6a` | Removing the candidate wallet mapping hid a known funding relation and changed the shadow result from reject to admit. |
| SCT-03 | Reproduced defect | P2 | Fixed — `1267df6a` | Replacing the candidate key hash preserved an admitted shadow result because the registry key was not joined to the authenticated binding key. |
| SCT-04 | Reproduced defect | P2 | Reproduced — fixed `acdbb1f2` | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): publication hashes now resolve against immutable source revision `41202067`; consolidated and independent verifiers pass. |
| SCT-05 | Reproduced defect | P2 | Reproduced — fixed `acdbb1f2` | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): a post-lock correction preserves the scored text while naming exclusive cross-process access and operation-scoped leases as the implemented mechanism. |
| SCT-06 | Economic assumption | P2 | Needs live environment | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): sustained overlapping public traffic and observable writer-fairness measurements are required. |
| SCT-07 | Evidence gap | P3 | Needs live environment | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): fleet disk-capacity and growth telemetry is not exposed by the permitted ledger/status endpoints. |
| SCT-08 | Evidence gap | P2 | Needs operator decision | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): a separately authorized evidence campaign is required; burn 2 permits no Task Node action. |
| SCT-09 | Proposed capability | P2 | Dispositioned by the locked V2 shadow successor; not promoted | The September 3 model-flag direction and Admission Policy V1 had different eligibility semantics and no selected versioned successor policy. The [V2 milestone](../plans/completed/tasknode-unl-amendment-v2-milestone.md) now implements that successor as `SHADOW_ONLY`; this is not live adoption. |
| SCT-10 | Proposed capability | P2 | Needs operator decision | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): independent operators and a separately authorized end-to-end rehearsal are prerequisites. |

## StakeHub PR #8 review

The source is the [StakeHub PR #8 review](stakehub-pr8-review-20260907.md).
Statuses describe the reviewed PR until the campaign's later read-only branch
review verifies the local fix branch.

| ID | Classification | Severity | Status | Source and reproduction |
| --- | --- | --- | --- | --- |
| SH-01 | Reproduced defect | P1 | Fixed in the local, uncommitted StakeHub repair set; not published | A daemon `policy_denied` response fell through to direct passphrase signing that did not apply the denied destination or cap policy. The [read-only branch review](stakehub-fix-branch-review-20260910.md) verified the fallback removal. |
| SH-02 | Reproduced defect | P1 | Fixed for the reviewed scripts in the local, uncommitted repair set; not published | New deploy, probe, funding, governance, finality, and fleet scripts entered live paths merely by invocation, without a live flag and intent-bound confirmation. |
| SH-03 | Reproduced defect | P1 | Dispositioned in the local, uncommitted repair set; retained custody limits; not published | Ordinary exceptions and proposer changes could skip cleanup after wallet keys or private note openings were copied to validators. Cleanup is now failure-visible, while SIGKILL, partitions, snapshots, and compromised hosts remain unresolved by design. |
| SH-04 | Reproduced defect | P2 | Fixed for private egress in the local, uncommitted repair set; not published | The private-egress runner accepted an unrelated height advance and any matching numeric asset balance as success. |
| SH-05 | Reproduced defect | P2 | Fixed in the local, uncommitted repair set; operator-selected RPC remains a trust boundary; not published | The NAVCoin deposit path hardcoded one public Ethereum RPC for preflight and submission, with no policy-pinned override. |
| SH-06 | Reproduced defect | P2 | Dispositioned by disabling the operation in the local, uncommitted repair set; not published | An untested agent operation could irreversibly register a Hyperliquid referral with the master EVM key and no dedicated authorization ceremony. |
| SH-07 | Evidence gap | P2 | Dispositioned as cross-repository evidence; publication remains open | The root evidence manifest depended on sixteen A666 lineage files absent from both reviewed target branches. |
| SH-08 | Reproduced defect | P2 | Fixed in the local, uncommitted documentation repair; not published | One handoff said the recovery archive was outside Git while PR #8 committed the archive, ELFs, witnesses, and historical scripts. |
| SH-09 | Reproduced defect | P3 | Fixed conservatively in the local, uncommitted repair set; not published | Exact integer withdrawal amounts crossed binary floating point before venue signing; tests covered only exactly representable small amounts. |
| SH-10 | Reproduced defect | P1 | Reproduced — external read-only lane | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): Git object `8f6f27cf` still permits release-ID reuse before aggregate verification. |
| SH-11 | Reproduced defect | P2 | Reproduced — external read-only lane | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): Git object `8f6f27cf` still persists unbound `round_ok` as shielding success. |
| SH-12 | Reproduced defect | P2 | Reproduced — external read-only lane | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): Git object `8f6f27cf` still substitutes unbound `round_ok` plus height for exact batch application. |

## Proof-input review

The source is the [proof inventory safety review](proof-input-review-20260907.md).
The eleven pin mismatches were individually traced before the repaired
inventory was committed in `80f2232b`.

| ID | Classification | Severity | Status | Source and reproduction |
| --- | --- | --- | --- | --- |
| PI-01 | Evidence gap | P2 | Fixed — `80f2232b` | The fuzz wrapper's source pin no longer matched its corpus-relocation change; targets and bounds were unchanged. |
| PI-02 | Evidence gap | P3 | Dispositioned — repinned | `manifest_builder.rs` changed only a test witness path. |
| PI-03 | Evidence gap | P2 | Dispositioned — repinned | `evm_adapter.rs` added a bounded, domain-separated disclosure command outside reserve verification or submission authority. |
| PI-04 | Evidence gap | P3 | Dispositioned — repinned | `hyperliquid_adapter.rs` changed only a test witness path. |
| PI-05 | Evidence gap | P3 | Dispositioned — repinned | `near_adapter.rs` changed only a test witness path. |
| PI-06 | Evidence gap | P2 | Dispositioned — repinned | `reserve-proof-types/src/lib.rs` exported YOLO modules without changing reserve execution dispatch or ABI. |
| PI-07 | Evidence gap | P3 | Dispositioned — repinned | `aave_v3.rs` changed only a test witness path. |
| PI-08 | Evidence gap | P3 | Dispositioned — repinned | `evm_spot.rs` changed only a test witness path. |
| PI-09 | Evidence gap | P3 | Dispositioned — repinned | `hyperliquid_receipt.rs` changed only a test witness path. |
| PI-10 | Evidence gap | P3 | Dispositioned — repinned | `near_receipt.rs` changed only a test witness path. |
| PI-11 | Evidence gap | P3 | Dispositioned — repinned | `solana_stake.rs` changed only a test witness path; relocated witnesses remained byte-identical. |
| PI-12 | Reproduced defect | P2 | Fixed — `80f2232b` | Nested fuzz `Cargo.lock` was stale and Cargo silently resolved 17 already-required packages; locked metadata and post-run immutability now gate it. |
| PI-13 | Evidence gap | P2 | Fixed — `80f2232b` | The new `yolo_broker.rs` dependency lacked a source pin and malformed-input regressions. |
| PI-14 | Reproduced defect | P2 | Fixed — `80f2232b` | The readiness checker referenced a deleted plan and could not establish the unchanged open gates. |
| PI-15 | Reproduced defect | P2 | Fixed — `80f2232b` | The source-qualification checker referenced a retired evidence tree; immutable full-packet fallback now rejects partial or corrupt working packets. |
| PI-16 | Evidence gap | P2 | Needs operator decision | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): an operator must assign an independent cryptography audit and define its YOLO scope. |
| PI-17 | Evidence gap | P2 | Dispositioned | Archive integrity checks establish retained bytes and bindings, not fresh cryptographic re-verification of the historical proofs. |
| PI-18 | Proposed capability | P1 | Needs live environment | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): the local readiness checker still reports `qualified=0/6 stakehub_deprecated=false`. |

## Consensus signing-fix qualification

The source is the [deploy decision](../governance/signing-fix-deploy-decision-20260909.md)
and its frozen [qualification receipt](https://github.com/postfiatorg/postfiatl1v2/blob/b6c13c9f675472a63b296994974042f41f102511/deployments/signing-fix-qualification-20260909/qualification-receipt.json).

| ID | Classification | Severity | Status | Source and reproduction |
| --- | --- | --- | --- | --- |
| SQ-01 | Reproduced defect | P1 | Needs operator decision | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): a fresh local release link still embeds a randomized Rust temporary `RUNPATH`; a governed release-normalization or toolchain contract is required. |
| SQ-02 | Evidence gap | P1 | Needs operator decision | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): deployed source remains outside main's ancestry and `pftl_source_settlement.rs` remains absent. |
| SQ-03 | Evidence gap | P1 | Needs live environment | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): a current simultaneous all-six snapshot is still required. |
| SQ-04 | Evidence gap | P1 | Needs live environment | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): a fresh local search found 0/50 matching node binaries; retrieval requires a separately authorized operational path. |
| SQ-05 | Evidence gap | P1 | Dispositioned for current state; see RPC-01 | Validator-0 now answers at the same height and root previously observed on validators 1–5 after an out-of-campaign restart. This does not repair the diagnosed recurrence condition or replace a current simultaneous all-six rehearsal. |
| SQ-06 | Proposed capability | P1 | Needs operator decision | [Burn 2 reproduction](burn2-open-row-reproduction-20260910.md): source contains `bbb291ce`, but the combined lineage, reproducibility, snapshot, rollback, and rollout decision remains unmade. |

## Final disposition

The inventory contains 89 unique rows, including 28 burn 3 findings (6 P1,
14 P2, 8 P3). Classification and severity describe what the cited evidence
establishes; status describes the bounded disposition in that row, not a
broader production claim.

| Measure | Count |
| --- | ---: |
| Reproduced defects | 62 |
| Evidence gaps | 22 |
| Economic assumptions | 1 |
| Proposed capabilities | 4 |
| P1 | 25 |
| P2 | 44 |
| P3 | 20 |
| Fixed | 48 |
| Dispositioned | 16 |
| Reproduced and retained (prior campaigns) | 5 |
| Recorded, not fixed (burn 3 P3) | 8 |
| Needs live environment | 6 |
| Needs operator decision | 6 |
| Bare open | 0 |

Burn 2 grounded all nineteen rows that entered it as open. Three received
in-repository source or documentation repairs; five remain directly
reproduced, comprising the two P3s and three findings in the read-only
StakeHub lane; five name the exact missing live environment; and six name the
operator decision required before more work is authorized. The bounded
commands and conditions are in the [burn 2 reproduction record](burn2-open-row-reproduction-20260910.md).

Burn 3 added 19 fixed source findings, one still-unproven fleet snapshot export,
and eight P3 findings recorded without repair. The five consensus-affecting
execution and Cobalt repairs are source-only, not activated or deployed. The
fleet export receipt remains missing even though the deployed source contains
the block-924 repair.

“Fixed” remains scoped by the row. In particular, the RPC supervisor repair is
not deployed, StakeHub fixes are local and unpublished, the Arc controller
migration remains operational work, V2 remains `SHADOW_ONLY`, and the fleet
still does not run the signing fix. “Dispositioned” means the cited concern
was classified or bounded; it does not imply that a proposed capability was
promoted. No row authorizes a deployment, live-chain action, Task Node action,
or StakeHub write.

## Completeness audit

| Source set | Rows |
| --- | ---: |
| 2026-09-10 campaign findings | 15 |
| 2026-09-06 storage, Cobalt, and Task Node review | 10 |
| StakeHub PR #8 and current fix-branch reviews | 12 |
| Proof-input review | 18 |
| Signing-fix qualification blockers | 6 |
| Burn 3 storage and snapshots (STO-) | 6 |
| Burn 3 execution (EXE-) | 3 |
| Burn 3 Cobalt ratification (COB-) | 4 |
| Burn 3 network and mempool admission (NET-) | 4 |
| Burn 3 operational Python CLIs (OPS-) | 11 |
| **Total** | **89** |

Campaign review and repair commits are recorded in the
[campaign log](qa-campaign-20260910.md). The inventory preserves open design,
evidence, and operational limits rather than converting them into code defects
or claims of authority.
