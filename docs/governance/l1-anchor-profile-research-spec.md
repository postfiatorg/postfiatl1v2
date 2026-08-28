# L1 Dynamic UNL Anchor Profile Research Specification

**Status:** Text Improvement Harness full gate passed on 2026-08-28 — average 88.67/100 (GPT 90.40, Fable 86.60, GLM 89.00; five runs per lane; run group `l1-anchor-profile-research-spec`); scored content SHA-256 `657195a8a209485d151fc672eb1255cdb4c20a4e0db31f73ac9779aea655b288`; Task Node lock pending the operator's decision

**Date:** 2026-08-28

**Author:** Domagoj Ravlić (`dravlic`)

**Decision owner:** Post Fiat

**Related:** [Dynamic UNL L1 Evidence-Source Decision Note](dynamic-unl-l1-evidence-source-note.md), [L1 Observer Service Research Specification](l1-observer-research-spec.md), [Dynamic UNL Proposal Source Research Specification](dynamic-unl-proposal-source-research-spec.md), and [Deferred Dynamic UNL Proposal Source Milestone](../deferred-plans/dynamic-unl-proposal-source-milestone.md)

## Plain-English directive

Keep `payment_v2` as the candidate transport for L1 Dynamic UNL evidence. Do not add a dedicated transaction merely to gain payload capacity. One compact record fits inside one existing memo.
Research Option B: add a versioned anchor profile, accepted-receipt metadata, and a bounded round index. Require the transaction's ML-DSA-65 signer to prove the profile role. Bind validator commits and reveals to the active registry key interval.
Let the L1 observer find a whole round without scanning every block or trusting a publisher database. Keep every result `SHADOW_ONLY`. This specification authorizes no transaction, devnet action, validator change, or authority transfer.

## Claims, evidence, and gaps

| Claim | Evidence now | Gap to close |
|---|---|---|
| The memo lane is bounded. | `PaymentMemo` permits lower-hex type, format, and data fields. `UnsignedPaymentV2` permits four memos and 512 total decoded bytes. Per-field caps are 64, 64, and 256 bytes. | No Dynamic UNL profile reserves those bytes or forbids record splitting. |
| The payment signs every memo byte. | `UnsignedPaymentV2::signing_bytes` binds chain, genesis, protocol, sender, recipient, amount, fee, sequence, memo count, lengths, and values. | The generic signature proves an account key, not a registry role by itself. |
| Accepted payments use ML-DSA-65. | `execute_payment_v2` verifies ML-DSA-65, derives `from` from the public key, checks the account key, sequence, balance, fee, and reserve. | A validator hot key is not currently classified as an anchor signer at the inclusion height. |
| Finality and success are distinct. | `tx_finality` links a receipt to a block. A receipt carries `accepted`, `code`, and fees. | `Receipt` carries no anchor kind, round, payload hash, registry root, or validator ID. |
| Account history is indexed. | `account_tx` indexes sender and recipient rows by height. A `payment_v2` row exposes transaction kind, memo hash, count, decoded byte count, acceptance, receipt code, batch ID, and transaction index. | It omits memo bytes, signer public key, profile kind, round, and registry attribution. |
| Full transactions remain retrievable. | Public `batch_archive` can return a bounded archived batch by batch kind and batch ID. | A verifier must join account history to archive payloads and locate the transaction manually. Archive retention and completeness are separate concerns. |
| A convention can avoid a full-chain scan. | Every record can pay one established profile sink. Its recipient history then lists candidate records. | The sink is a spam target. The generic index cannot prove that all accepted profile records for a round were returned. |
| Existing PFT Ledger records are compact facts. | The reference pipeline carries a round announcement, validator commit, validator reveal, final-bundle receipt, and convergence-report anchor. | Its JSON and secp256k1 payload signatures are not the L1 wire format. ML-DSA-65 signatures do not fit inside 512 memo bytes. |
| Outer-signature attribution can replace an inner signature. | The full `payment_v2` envelope already carries and verifies the ML-DSA-65 public key and signature outside the memo budget. | The profile must bind that key to the correct registry interval and must distinguish publisher and validator roles. |
| The L1 observer can consume finalized anchors. | The observer specification already names registry intervals, raw replay, gaps, and `SHADOW_ONLY` output. | It needs a complete, bounded anchor query and stable record names from this profile. |

The capacity question and the evidence-protocol question have different answers. Capacity is sufficient. Attribution and indexed completeness are not sufficient today.

## Decision question

Can the existing `payment_v2` lane, with a small recognized profile and index, carry and expose every L1 Dynamic UNL record safely enough for shadow research?

The answer is yes only if:

1. every record fits one memo without a split convention;
2. every accepted record has one canonical decoding;
3. validator authorship resolves to one active ML-DSA-65 registry entry;
4. publisher authorship resolves to one configured profile publisher;
5. a bounded RPC query returns the complete indexed round or an explicit gap;
6. finality plus an accepted receipt is checked; and
7. duplicates, equivocations, censorship, pruning, and fee failures remain visible.

## Scope

### In scope

- Announcement records with input package hash, CID, and commit/reveal height windows.
- Validator commit and reveal records.
- Final scoring report anchors.
- Sealed convergence report anchors.
- ML-DSA-65 publisher and validator attribution.
- Registry roots and key-rotation intervals.
- Canonical bytes, hashes, CIDs, salts, and output fingerprints.
- `payment_v2` fee, amount, sequence, account, and reserve behavior.
- Accepted receipts and finalized block linkage.
- Account history, batch archive, retained history, and a proposed round index.
- Duplicate, conflicting, late, oversized, split, replayed, and censored records.
- Offline snapshot replay.
- A local adversarial fleet.
- A Python CLI followed by a read-only user interface.
- A separately authorized controlled-devnet shadow announcement.

### Out of scope

- Sending any transaction under this specification.
- Running a live commit or reveal.
- Running a live convergence or report anchor.
- Giving Dynamic UNL, an observer, or a sidecar registry authority.
- Changing Cobalt or Consensus v2 roles.
- Changing the scoring model, formula, selector, or observer evidence contract.
- Moving model execution or artifact fetches into consensus.
- Publishing full model outputs in a memo.
- Mainnet or public-testnet activation.
- A claim that inclusion proves honest scoring or complete observation.

## Responsibility split

| Surface | Existing or proposed module | Responsibility |
|---|---|---|
| Payment and memo types | `crates/types/src/transactions_mempool_receipts.rs` and `crates/types/src/core_chain.rs` | Define the signed envelope and current memo bounds. |
| Payment execution | `crates/execution/src/entrypoints.rs` | Verify ML-DSA-65 account authorization, fees, sequence, balances, and acceptance. |
| Transaction ID | `crates/execution/src/tx_hashing.rs` | Bind the full signed payment to its transaction ID. |
| Receipt | `crates/types/src/transactions_mempool_receipts.rs` | Report generic acceptance today. |
| Finality and account index | `crates/node/src/block_finality.rs` and `crates/node/src/node_types.rs` | Link receipts to blocks and expose account-index rows. |
| Retained history | `crates/node/src/lifecycle_queries.rs` | Define retained-history and gap boundaries. |
| RPC | `crates/node/src/rpc_dispatch.rs`, `crates/rpc_sdk/src/lib.rs`, and `python/postfiat_rpc` | Expose bounded history, archives, finality, and client reads. |
| Registry history | `crates/types/src/shielded_bridge_governance.rs` and Cobalt history | Resolve validator ID, hot key, registry root, and active height interval. |
| Anchor record types | `new: crates/types/src/dynamic_unl_anchor_profile.rs` | Define canonical records, validation, role rules, domains, and receipt metadata. |
| Anchor round index | `new: crates/node/src/dynamic_unl_anchor_index.rs` | Index accepted records by profile, round, kind, signer role, and ledger position. |
| Anchor read RPC | `new: crates/node/src/dynamic_unl_anchor_rpc.rs` | Return bounded pages, cursor, indexed tip, gaps, and completeness. |
| Observer consumer | `new: crates/node/src/validator_evidence_observer.rs` | Consume finalized anchor pages under the observer specification. |
| Sidecar source | `new: validator-scoring-sidecar/src/validator_scoring_sidecar/sources/postfiat_l1.py` | Build, sign, persist, and replay L1 profile records. |
| Scoring publisher | `new: dynamic-unl-scoring/scoring_service/sources/postfiat_l1.py` | Build L1 announcements and artifact anchors outside consensus. |
| Human tooling | `new: python/postfiat_rpc/l1_anchor_profile.py` | Encode, size, verify, scan, replay, and explain records. |

Paths marked `new:` do not exist. They are ownership proposals, not implementation claims.

## Shared invariants

### I1. One record, one memo

The profile uses exactly one memo per `payment_v2`. It forbids continuation, chunk, and split records. Extra memos make the transaction profile-invalid even when the generic payment remains valid.

### I2. Signed outer domain

The authenticated object is the full accepted `SignedPaymentV2`, not detached memo bytes. Its chain ID, genesis hash, protocol version, sender, recipient, amount, fee, sequence, memo fields, public key, and signature remain part of verification. A copied memo on another payment is a different record.

### I3. Canonical profile bytes

The one memo has decoded `memo_type` `postfiat.dynamic-unl.v1` and decoded `memo_format` `application/pf-dunl1`. `memo_data` is the lower-hex form of the binary record below. Integers are unsigned big-endian. Lengths are unsigned bytes. Reserved bits must be zero. Unknown versions and kinds fail closed.

### I4. Fixed hash and CID rules

Input, commitment, output, and artifact hashes are raw 32-byte SHA-256 values. The registry root is the raw 48 bytes represented by the current 96-character lower-hex root. A CID is canonical ASCII CIDv0 base58btc or lowercase CIDv1 base32. Its length is 1 through 96 bytes. No Unicode normalization is permitted.

### I5. Role attribution

Announcement, convergence-anchor, and report-anchor payments must use the configured ML-DSA-65 profile publisher key. Commit and reveal payments must use the active ML-DSA-65 hot key for exactly one registry validator at the inclusion height. The encoded registry root must equal the active root used for that lookup. The transaction public key, not its sender label or sink account, supplies authorship.

### I6. Domain-separated commitment

The commitment hash is SHA-256 over the canonical binary preimage prefixed by `postfiat.l1.dynamic-unl.commitment.v1`. The preimage binds chain ID, genesis hash, protocol version, round number, validator ID, registry root, input package hash, three output hashes, and 32-byte salt. The reveal must reproduce the accepted first commit.

### I7. Finalized accepted records only

An anchor exists only when a finalized block links the transaction receipt and `receipt.accepted` is true. An RPC success, mempool admission, memo hash, or block inclusion with a rejected receipt is insufficient. Ledger order is block height then transaction index.

### I8. Height windows

The L1 profile uses half-open finalized-height windows. A commit is valid at `commit_open_height <= height < commit_close_height`. A reveal is valid at `reveal_open_height <= height < reveal_close_height`. Local time and observer arrival order never decide validity.

### I9. First valid and conflicts

The first valid commit per round and validator is accepted. An identical later record is a duplicate. A different later commit is an equivocation flag. The first valid reveal that opens the accepted commit is accepted. Every conflicting signed record remains queryable.

### I10. Complete indexed reads

The index keys accepted records by profile version, round, kind, validator ID or publisher role, height, transaction index, and transaction ID. Each page binds the chain tip and index tip. A pruned archive, stale index, missing registry view, or truncated page reports `INCOMPLETE`, not an empty round.

### I11. Spam and fee isolation

The profile changes no generic payment admission rule. Normal fees, account reserve, sequence, batch size, and rate limits still apply. Unknown senders to the sink remain ordinary payments and do not enter the recognized anchor index. Recognition must be bounded before registry or signature work.

### I12. Authority boundary

Anchors prove publication and attributed agreement facts. They do not prove that an artifact is correct, that all validators observed it, or that a score deserves authority. Every experiment and interface remains `SHADOW_ONLY`.

## Canonical encoding and size proof

The common header is 16 bytes:

| Field | Bytes | Rule |
|---|---:|---|
| Magic | 4 | ASCII `PFD1` |
| Profile version | 1 | `1` |
| Record kind | 1 | announcement `1`, commit `2`, reveal `3`, convergence anchor `4`, report anchor `5` |
| Flags | 2 | zero |
| Round number | 8 | nonzero |

Record bodies follow the common header in the listed order:

| Record | Body fields | Data bytes, worst case | Type + format + data | Headroom under 512 |
|---|---|---:|---:|---:|
| Announcement | input hash 32; CID length 1 + CID 96; four heights 32 | 177 | 220 | 292 |
| Commit | registry root 48; input hash 32; commitment hash 32 | 128 | 171 | 341 |
| Reveal | registry root 48; input hash 32; three output hashes 96; salt 32 | 224 | 267 | 245 |
| Convergence anchor | input hash 32; report hash 32; CID length 1 + CID 96 | 177 | 220 | 292 |
| Report anchor | final-bundle hash 32; CID length 1 + CID 96; VL sequence 8 | 153 | 196 | 316 |

The type is 23 decoded bytes. The format is 20 decoded bytes. The largest data field is 224 bytes, below the existing 256-byte `memo_data` cap. The largest complete memo is 267 decoded bytes, below the 512-byte aggregate cap. The profile leaves three generic memo slots unused and forbids using them for extensions.

The announcement carries the same frozen-input and window facts as the PFT Ledger announcement. The commit carries the same input and hidden commitment facts. The reveal carries the same three output hashes and salt. The report anchor carries the final bundle CID and VL sequence. The convergence anchor carries the sealed report CID. L1 signer roles replace the PFT Ledger's inner secp256k1 validator signature and relay-wallet distinction.

## Discovery and indexing

Option A uses one configured, established anchor sink address. Every profile payment sends the minimum allowed amount to that sink. An observer queries `account_tx_history` for the sink in bounded height windows. It keeps only accepted `payment_v2` rows. For each row, it fetches `batch_archive` by batch ID and selects the recorded transaction index. It then verifies the full payment, memo, finality, receipt, registry interval, role, and record bytes. This avoids a full block scan.

That convention is usable for E1 fixtures but weak as an operational contract. Today there is no query by profile, round, record kind, or validator. The account row does not return memo bytes or the signer key. The two-RPC join has no atomic completeness receipt. Sink spam consumes page space. Retained-history pruning can remove the archive needed to authenticate a row.

Option B adds `dynamic_unl_anchors` as a bounded read. It returns canonical bytes, transaction ID, block height, transaction index, accepted receipt code, signer-key hash, resolved role, validator ID when applicable, registry root, and payload hash. It also returns page cursors, indexed height range, block tip, index tip, gap ranges, truncation, and `COMPLETE` or `INCOMPLETE`. The observer rejects a round unless the requested height range is complete through its end height.

## Options and failure modes

| Option | Concrete modules | Build cost | Main failure modes | What it can claim | What it cannot claim |
|---|---|---|---|---|---|
| A. Memo lane as is | Existing payment, execution, receipt, account index, `tx`, and `batch_archive`; `new: python/postfiat_rpc/l1_anchor_profile.py` | Low; conventions and client work | Sink spam, page truncation, archive pruning, hot-key account funding, sequence conflicts, fee griefing, manual joins, equivocation seen late, publisher or validator censorship | Records fit; a verifier can authenticate records it finds | Complete round discovery, protocol-recognized attribution, or durable index completeness |
| B. Memo lane plus small additions | A plus `new: crates/types/src/dynamic_unl_anchor_profile.rs`, `new: crates/node/src/dynamic_unl_anchor_index.rs`, `new: crates/node/src/dynamic_unl_anchor_rpc.rs`; receipt and SDK fields in named existing modules | Medium; one versioned profile, index, receipt projection, RPC, migration, and tests | Index lag or corruption, registry-view loss, recognition bugs, fee or sequence failure, hot-key exposure, spam outside the recognized index, validator or publisher censorship | Canonical capacity, finalized attribution, bounded complete reads, duplicate and equivocation evidence | Honest scoring, uncensored inclusion, observer completeness outside retained history, or governance authority |
| C. Dedicated data transaction | `new: crates/types/src/dynamic_unl_anchor_transaction.rs`, execution handler, fee and replay state, mempool batch lane, receipt type, storage migration, RPC and SDK methods, activation fence | High; consensus, execution, storage, replay, wallet, and release work | New spam surface, underpriced data, state growth, activation forks, replay bugs, migration failure, fee griefing, censorship, oversize and split semantics | A first-class typed protocol lane with direct queries and dedicated authorization | Correct scoring, uncensored publication, or a reason to duplicate a capacity already proven sufficient |

All options face censorship. An index can prove what finalized, not what a proposer excluded. All options face fee griefing and missed windows when a signer account lacks funds or has a sequence conflict. All options must retain conflicting signed anchors. No option may accept oversize or split records as equivalent to one canonical record.

## Recommendation

Choose Option B for research and shadow qualification. Keep the existing `payment_v2` capacity and add only profile recognition, accepted-receipt metadata, and a complete bounded index. Do not add a bounded memo extension or a dedicated transaction unless E1 or E2 falsifies the size, attribution, or replay design.

The operator should record this exact decision:

> Select Option B as the L1 Dynamic UNL anchor profile: retain one-record `payment_v2` transport; add versioned canonical validation, ML-DSA-65 role attribution, accepted-receipt metadata, and a complete bounded round index; keep all use `SHADOW_ONLY`; and reconsider a dedicated transaction only after a recorded E1 or E2 failure proves the memo profile inadequate.

## Experiments

### E1. Canonical encoding and size proof

Build a standalone stdlib-only Python verifier. It encodes and decodes all five record kinds without importing node code. Freeze minimum, normal, and maximum valid vectors. Exercise 96-byte CIDs, maximum integers, wrong lengths, noncanonical CIDs, unknown flags, extra memos, split records, odd or upper hex, trailing bytes, and every one-field mutation. Compare a second independent encoder.

**Required result:** Both encoders emit byte-identical records and hashes. Every valid record uses one memo. `memo_data` is at most 224 decoded bytes and the full memo is at most 267. Every oversize, ambiguous, split, noncanonical, or mutated vector fails with a stable reason.

### E2. Attribution and indexed history replay

Use an offline finalized snapshot with blocks, accepted and rejected receipts, batch archives, registry history, and account history. Send nothing and call no live endpoint. Rebuild the proposed index from the snapshot. Resolve every validator signer against the active ML-DSA-65 registry interval. Resolve every publisher signer against the configured profile publisher. Compare the indexed round to a full offline history oracle. Repeat after restart, key rotation, rollback, index deletion, archive pruning, and an injected gap.

**Required result:** The complete snapshot yields the same ordered records, roles, validator IDs, roots, receipts, and index root in two clean replays. Wrong keys, roots, roles, chains, rejected receipts, and stale intervals never enter the accepted index. Missing archives or registry history yield an exact `INCOMPLETE` range, never an empty or complete round.

### E3. Adversarial cases on a local fleet

Use only a disposable local fleet. Do not use the controlled devnet. Exercise sink spam, unknown senders, fee starvation, sequence races, duplicate submissions, conflicting commits, conflicting reveals, copied payloads, wrong rounds, late records, publisher substitution, validator rotation, selective proposer censorship, index lag, restart, oversize data, four-memo splits, and malformed archives. Measure query cost, index growth, and bounded work per rejected candidate.

**Required result:** Valid finalized records converge on every node. Invalid or rejected payments never become accepted anchors. Equivocations remain visible and deterministically ordered. Spam cannot make recognized queries unbounded. Censorship produces a named missing-participation result and never a fabricated record. No local case mutates registry or governance state.

### E4. Controlled-devnet shadow announcement

Run only under separate written operational authorization. This specification authorizes nothing. If authorized, publish one shadow announcement only. Do not publish a commit, reveal, convergence anchor, report anchor, Cobalt proposal, or registry update. Require the exact E1 bytes and a pre-recorded fee, account, signer, sink, height-window, and rollback-free operation packet.

**Required result:** Every node finalizes the same accepted announcement receipt and returns the same indexed record and completeness state. The CLI and UI label it `SHADOW_ONLY`. No other profile transaction or authority action occurs. Without separate authorization, record E4 as `NOT_RUN`, not a pass.

## Gates

### ADOPT FOR SHADOW

Choose `ADOPT FOR SHADOW` only when E1, E2, and E3 meet every Required result. E4 must pass if separately authorized or remain explicitly `NOT_RUN`. Adoption selects Option B for shadow use only. It grants no live scoring, proposal, Cobalt, or registry authority.

### REMEDIATE

Choose `REMEDIATE` for a bounded encoding, attribution, index, receipt, retention, fee, or interface defect with a named owner and repeat test. Keep the same vectors and failure cases. Do not increase payload bounds or weaken completeness to obtain a pass.

### REJECT

Choose `REJECT` if one record cannot fit, attribution is ambiguous, replay is nondeterministic, a rejected receipt becomes an anchor, a gap appears complete, equivocation is hidden, spam makes reads unbounded, or safe operation requires validator-key handling that Post Fiat will not approve. Rejection triggers a new decision on Option C. It does not authorize Option C automatically.

## Required evidence packet

- Exact L1, `dynamic-unl-scoring`, and `validator-scoring-sidecar` revisions.
- Dirty-state reports for every source.
- Canonical schema and domain strings.
- Valid, boundary, and one-field mutation vectors.
- Independent encoder equality report.
- Per-record byte table and maximum proof.
- Offline snapshot manifest and hashes.
- Block, receipt, batch archive, account history, and registry-history roots.
- Full-history oracle and rebuilt-index comparison.
- Signer-to-role and signer-to-validator attribution report.
- Accepted, rejected, duplicate, and equivocation reports.
- Gap, pruning, restart, key-rotation, and rollback results.
- Local-fleet topology and adversarial case matrix.
- Spam, index size, query work, and fee measurements.
- CLI output and read-only UI captures.
- E4 authorization and receipt, or explicit `NOT_RUN`.
- A statement that no unauthorized transaction or authority action occurred.
- The final gate and the operator's recorded option decision.

Do not commit private keys, credentials, node data, raw private topology, or generated fleet state.

## Human interfaces

Build the Python CLI first. The proposed entry point is `python -m postfiat_rpc.l1_anchor_profile`. It supports `encode`, `decode`, `size`, `verify`, `scan`, `round`, `trace`, `gaps`, and `equivocations`. Offline files are the default. Network submission is absent. Every result shows chain, round, kind, height, finality, receipt code, signer role, validator ID when applicable, registry root, payload hash, and completeness.

After the CLI works, build a read-only interface from the same verified packet. Show the five record kinds as one round timeline. Show byte use, window validity, publisher identity, validator attribution, missing records, duplicates, equivocations, gaps, index tip, and block tip. Expose no transaction builder, signer, submit, commit, reveal, anchor, proposal, vote, or registry control.

## Required publication

Publish the canonical profile and vectors under `docs/governance`. Publish the index completeness contract and retention behavior. Publish the exact signer-role rules. Publish the PFT Ledger-to-L1 fact mapping. Publish E1 through E4 results, including `NOT_RUN`.
Update the observer specification with the accepted query name and gap semantics. Update RPC methods and the Python client runbook only after those surfaces exist. Publish `SHADOW_ONLY` and remaining censorship, fee, key-use, and retention limits as prominently as a pass.

## Decisions recorded

1. Existing memo capacity is sufficient for all five records.
2. One record uses one memo and may not be split.
3. The transaction signature supplies ML-DSA-65 authorship.
4. Validator authorship uses the registry interval active at inclusion height.
5. Publisher authorship is a separate configured role.
6. Finality and an accepted receipt are both required.
7. Finalized height, not local time, decides windows.
8. The sink-account convention is an Option A fallback, not the recommended operational index.
9. Option B is the research recommendation.
10. No memo-size extension is proposed.
11. A dedicated transaction is deferred unless E1 or E2 proves it necessary.
12. The anchor profile and observer remain `SHADOW_ONLY`.
13. This specification authorizes no chain transaction.
14. Task Node lock remains pending the operator's decision.

## Work sequence

- [ ] Score this exact research specification with the Text Improvement Harness full gate.
- [ ] Lock it immediately if the first compliant average is at least 86/100.
- [ ] If below 86/100, pass the harness output through one direct OpenRouter `openai/gpt-5.6-sol-pro` rewrite.
- [ ] Re-score only the rewritten content, with no more than two improvement loops.
- [ ] Record the final score and scored-content SHA-256 in the Status line.
- [ ] Await the operator's decision on the Task Node lock.
- [ ] Request one substantial milestone through Task Node only after that decision.
- [ ] Assign profile, index, observer, sidecar, CLI, UI, and key-safety owners.
- [ ] Build the CLI before the read-only UI.
- [ ] Run E1, E2, and E3 in order.
- [ ] Run E4 only under separate authorization.
- [ ] Publish the evidence packet and gate decision.
- [ ] If adopted, keep every surface `SHADOW_ONLY`.
- [ ] Retire the later milestone only after the CLI, UI, and concise documentation work.
