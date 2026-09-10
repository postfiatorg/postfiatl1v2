# QA defect inventory — 2026-09-10

Status: final reconciled inventory. Its exact-byte Text Improvement Harness
result is recorded in the [campaign log](qa-campaign-20260910.md).

This inventory separates demonstrated failures from missing evidence, operating
assumptions, and capabilities that have not been built or activated. A
**reproduced defect** has an observed failing case. An **evidence gap** means the
available record cannot establish the claim. An **economic assumption** is a
load or incentive premise rather than a code result. A **proposed capability**
must not be described as shipped or authoritative.

## Campaign findings

| ID | Classification | Severity | Status | Source and reproduction |
| --- | --- | --- | --- | --- |
| ARC-01 | Reproduced defect | P1 | Fixed — `dbc73fea`; deployed controller still open | [Arc review](arc-facing-review-20260910.md#1-p1--v2-source-debited-exports-have-no-realizable-refund-path): a valid V2 source export could expire without a controller cancellation event matching the refund verifier. |
| ARC-02 | Reproduced defect | P1 | Fixed — `dbc73fea` | [Arc review](arc-facing-review-20260910.md#2-p1--the-top-level-mainnet-round-trip-command-mutates-live-systems-without-an-execution-interlock): invoking the shell entrypoint entered live setup without an explicit execution ceremony. |
| ARC-03 | Reproduced defect | P3 | Open | [Arc review](arc-facing-review-20260910.md#3-p3--the-dedicated-ethereum-mainnet-ingress-guest-carries-a-stale-fulu-epoch-pin): the dedicated ingress pin is 411648 while the shared path and retained program identity use 411392. Frozen guest artifacts were not regenerated. |
| RPC-01 | Reproduced defect | P1 | Open; diagnosed, not repaired | [Read-only validator-0 diagnosis](../status/chain-state-current.md#validator-0-rpc-diagnosis-20260910): the deployed RPC reached its 10,000-connection ceiling with a keep-alive connection still active, stopped accepting, and remained systemd-active while new health reads timed out. |
| CS-01 | Reproduced defect | P1 | Fixed — `f9f13ead` | [Consensus and storage review](consensus-storage-review-20260910.md#1-p1--consensus-authorization-and-signature-emission-are-separated-by-an-unlocked-race-window): a lower-view call could persist authorization, lose the safety guard to a higher-view call, and emit its signature only after the durable round floor advanced. Signing now completes under the guard. |
| CS-02 | Reproduced defect | P2 | Fixed — `f9f13ead` | [Consensus and storage review](consensus-storage-review-20260910.md#2-p2--activated-yolo-registrations-can-grow-consensus-state-without-a-bound-or-state-expansion-charge): distinct YOLO registrations appended to committed validator state without a count bound or state-expansion fee. Both row classes are now capped and charged. |
| WRS-01 | Reproduced defect | P1 | Fixed — `83488d91` | [Wallet/proxy/RPC SDK review](wallet-proxy-rpc-sdk-review-20260910.md#1-p1--transfer-quote-signing-trusts-an-rpc-selected-recipient-and-amount): the transfer signer sourced recipient and amount from an untrusted quote without binding the reviewed request. |
| WRS-02 | Reproduced defect | P1 | Fixed — `83488d91` | [Wallet/proxy/RPC SDK review](wallet-proxy-rpc-sdk-review-20260910.md#2-p1--the-loopback-session-token-endpoint-accepts-a-dns-rebinding-host): a same-origin rebinding Host could satisfy the loopback token endpoint and receive its bearer credential. |
| WRS-03 | Reproduced defect | P2 | Fixed — `83488d91` | [Wallet/proxy/RPC SDK review](wallet-proxy-rpc-sdk-review-20260910.md#3-p2--websocket-mutation-admission-releases-the-process-wide-slot-before-work-starts): WebSocket mutations released shared concurrency admission before routing or upstream I/O. |
| WRS-04 | Reproduced defect | P2 | Fixed for new vaults — `83488d91`; legacy vaults remain readable | [Wallet/proxy/RPC SDK review](wallet-proxy-rpc-sdk-review-20260910.md#4-p2--the-maintained-extension-permits-cheaply-brute-forced-new-vaults): four-character extension passphrases under the 100,000-iteration vault format permit cheap offline recovery after profile theft. |
| WRS-05 | Reproduced defect | P2 | Fixed — `83488d91` | [Wallet/proxy/RPC SDK review](wallet-proxy-rpc-sdk-review-20260910.md#5-p2--the-extension-popup-is-invalid-as-a-browser-module): an unmatched brace caused Chrome's module parser to reject the maintained extension popup while the Node syntax gate missed it. |
| UNL-01 | Reproduced defect | P1 | Fixed — `1c10f828` | [Task Node UNL review](tasknode-unl-review-20260910.md#1-p1--a-complete-score-window-passes-without-a-renewed-vouch-or-post-epoch-co-work): accounts with no bilateral records return `READY` after score-only continuity. |
| UNL-02 | Reproduced defect | P2 | Fixed — `1c10f828` | [Task Node UNL review](tasknode-unl-review-20260910.md#2-p2--a-stale-score-replay-can-suppress-fresh-score-evidence-by-input-order): stale-first and fresh-first orderings of the same digest produce different continuity decisions. |
| UNL-03 | Reproduced defect | P2 | Fixed — `1c10f828` | [Task Node UNL review](tasknode-unl-review-20260910.md#3-p2--valid-identifiers-can-inject-markdown-structure-into-the-operator-report): an accepted newline/backtick identifier creates an attacker-chosen heading in the root-valid human report. |
| UNL-04 | Reproduced defect | P3 | Open; recorded, not fixed | [Task Node UNL review](tasknode-unl-review-20260910.md#4-p3--the-hypothetical-round-helper-accepts-a-report-that-is-not-bound-to-its-frozen-window): an in-memory report candidate replacement bypasses a saturated-group hold in caller-owned hypothetical state. |

## Storage, Cobalt, and Task Node review

The source is the historical [2026-09-06 review](https://github.com/postfiatorg/postfiatl1v2/blob/cfdeccd6897bfc8a80bfa2a95a6fb04f603c1cee/docs/review/storage-cobalt-tasknode-handoff-review-20260906.md).

| ID | Classification | Severity | Status | Source and reproduction |
| --- | --- | --- | --- | --- |
| SCT-01 | Reproduced defect | P1 | Fixed and deployed — 2026-09-06 RPC cache release | Transactional finality advanced every RPC to block 992 while cached status returned six different older heights. |
| SCT-02 | Reproduced defect | P2 | Fixed — `1267df6a` | Removing the candidate wallet mapping hid a known funding relation and changed the shadow result from reject to admit. |
| SCT-03 | Reproduced defect | P2 | Fixed — `1267df6a` | Replacing the candidate key hash preserved an admitted shadow result because the registry key was not joined to the authenticated binding key. |
| SCT-04 | Reproduced defect | P2 | Open | The committed consolidated Cobalt verifier still exits with `adversarial packet semantic verifier is missing, failed, or inconsistent` after routine publication-file changes. The independent E5 result is a separate pass. |
| SCT-05 | Reproduced defect | P2 | Open | The locked single-writer plan still says cross-process readers coexist with a writer and RPC never takes the write lock, while the deployed operation-scoped writable handle excludes sibling-process opens. |
| SCT-06 | Economic assumption | P2 | Open | The bounded lease qualification assumes serialized, operator-driven blocks; sustained overlapping public-network operations and writer fairness have not been qualified. |
| SCT-07 | Evidence gap | P3 | Open | The review measured the local build filesystem at 96% used but did not inventory validator-fleet disk capacity. |
| SCT-08 | Evidence gap | P2 | Open | The real-data Task Node replay lacks complete signed bindings, publisher-attested work digests, exclusions, remaining admission facts, and a native L1 registry-root binding. |
| SCT-09 | Proposed capability | P2 | Dispositioned by the locked V2 shadow successor; not promoted | The September 3 model-flag direction and Admission Policy V1 had different eligibility semantics and no selected versioned successor policy. The [V2 milestone](../plans/completed/tasknode-unl-amendment-v2-milestone.md) now implements that successor as `SHADOW_ONLY`; this is not live adoption. |
| SCT-10 | Proposed capability | P2 | Open | An independently operated end-to-end Task Node proposal and Cobalt ratification rehearsal remains unperformed; Foundation-controlled nodes do not establish independent operation. |

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
| SH-10 | Reproduced defect | P1 | Open in the StakeHub lane | [Read-only branch review](stakehub-fix-branch-review-20260910.md#1-p1--release-reuse-can-overwrite-an-active-release-before-promotion): release reuse can copy configuration directly into an active release directory before aggregate verification or promotion. |
| SH-11 | Reproduced defect | P2 | Open in the StakeHub lane | [Read-only branch review](stakehub-fix-branch-review-20260910.md#2-p2--shielding-records-success-from-an-unbound-runner-flag): shielding persists success from `round_ok` without the exact certified-batch contract used by private egress. |
| SH-12 | Reproduced defect | P2 | Open in the StakeHub lane | [Read-only branch review](stakehub-fix-branch-review-20260910.md#3-p2--route-activation-can-confuse-fleet-height-with-exact-batch-success): route activation combines an unbound runner flag with fleet height, neither of which proves application of the intended amendment. |

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
| PI-16 | Evidence gap | P2 | Open | The review was not an independent cryptography audit or a full review of the separate YOLO target-proof system. |
| PI-17 | Evidence gap | P2 | Dispositioned | Archive integrity checks establish retained bytes and bindings, not fresh cryptographic re-verification of the historical proofs. |
| PI-18 | Proposed capability | P1 | Open | The reviewed A666 reserve path remained 0/6 production-qualified; repinning source evidence did not qualify or activate it. |

## Consensus signing-fix qualification

The source is the [deploy decision](../governance/signing-fix-deploy-decision-20260909.md)
and its frozen [qualification receipt](https://github.com/postfiatorg/postfiatl1v2/blob/b6c13c9f675472a63b296994974042f41f102511/deployments/signing-fix-qualification-20260909/qualification-receipt.json).

| ID | Classification | Severity | Status | Source and reproduction |
| --- | --- | --- | --- | --- |
| SQ-01 | Reproduced defect | P1 | Open | Two locked builds from the same source differed in six ELF `RUNPATH` bytes, so exact release hash reproducibility failed. |
| SQ-02 | Evidence gap | P1 | Open | Current `main` is not a descendant of deployed source `707e006f` and omits live A666 source-route runtime behavior; a main-built candidate is not a signing-only successor. |
| SQ-03 | Evidence gap | P1 | Open | The newest authorized local all-six snapshot was height 931 while the fleet observation was height 1020, so no current-height deployment-exact rehearsal was possible. |
| SQ-04 | Evidence gap | P1 | Open | The exact deployed rollback binary was absent from the qualification server. |
| SQ-05 | Evidence gap | P1 | Dispositioned for current state; see RPC-01 | Validator-0 now answers at the same height and root previously observed on validators 1–5 after an out-of-campaign restart. This does not repair the diagnosed recurrence condition or replace a current simultaneous all-six rehearsal. |
| SQ-06 | Proposed capability | P1 | Open | Fix `bbb291ce` passes focused source tests but is absent from the fleet; a lineage-preserving, reproducible, current-state-qualified rollout remains unperformed and unauthorized. |

## Final disposition

The inventory contains 61 unique rows. Classification and severity describe
what the cited evidence establishes; status describes the bounded disposition
in that row, not a broader production claim.

| Measure | Count |
| --- | ---: |
| Reproduced defects | 35 |
| Evidence gaps | 21 |
| Economic assumptions | 1 |
| Proposed capabilities | 4 |
| P1 | 19 |
| P2 | 30 |
| P3 | 12 |
| Fixed | 26 |
| Dispositioned | 16 |
| Open | 19 |

The nineteen open rows comprise eight P1s, eight P2s, and three P3s. The open
P1 set is `RPC-01`, `SH-10`, `PI-18`, and `SQ-01` through `SQ-04` plus
`SQ-06`. It keeps validator-0's connection exhaustion, the unpublished
StakeHub release-reuse defect, A666 production qualification, and every named
signing-fix deployment blocker explicit. No row authorizes a deployment,
live-chain action, Task Node action, or StakeHub write.

“Fixed” remains scoped by the row. In particular, StakeHub fixes are local and
unpublished, the Arc controller migration remains operational work, V2 remains
`SHADOW_ONLY`, and the fleet still does not run the signing fix. “Dispositioned”
means the cited concern was classified or bounded; it does not imply that a
proposed capability was promoted.

## Completeness audit

| Source set | Rows |
| --- | ---: |
| 2026-09-10 campaign findings | 15 |
| 2026-09-06 storage, Cobalt, and Task Node review | 10 |
| StakeHub PR #8 and current fix-branch reviews | 12 |
| Proof-input review | 18 |
| Signing-fix qualification blockers | 6 |
| **Total** | **61** |

Campaign review and repair commits are recorded in the
[campaign log](qa-campaign-20260910.md). The inventory preserves open design,
evidence, and operational limits rather than converting them into code defects
or claims of authority.
