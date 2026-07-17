# PostFiat L1 Open-Source Productionization Audit

**Status:** STEP 1/STEP 2 complete; sanitized source published; real-value launch gates remain open
**Audit date:** 2026-07-16
**Audited repository:** `postfiatl1v2`
**Audited branch:** `open-source-productionization-20260716`
**Audited baseline:** `4b5af7bc6bb6e793ed8a60219d13d6d35be03058`
**Controlling checklist:** `docs/status/OPEN-SOURCE-PRODUCTIONIZATION-REVIEW-CHECKLIST-20260716.md`
**Execution lab book:** `docs/status/OPEN-SOURCE-PRODUCTIONIZATION-LAB-BOOK-20260716.md`

## 1. Executive verdict

The audited private baseline must not be published. The remediated candidate is
published only as history rooted in a new one-commit sanitized export after the
complete closure battery, provider-owner destruction confirmation, and clean GitHub-clone
publication verification. The contaminated development history remains private.

The audit confirmed fourteen P0 classes. The published candidate fixes every
class listed below; the descriptions preserve the original findings rather
than pretending they never existed:

1. the production block-finality path does not implement the whitepaper's stated chained HotStuff lock/high-QC/two-chain commit rule;
2. a web-wallet signing path can send a wallet backup containing the master seed to the wallet proxy;
3. a legacy path described as shielded persists owner, asset, amount, and memo in cleartext and remains reachable through ordered shielded batches;
4. the reference wallet uses a publicly bound vulnerable Vite development server as a live mutation proxy;
5. the private Git history contains a captured cloud-instance Jupyter token that must never enter public history;
6. governance “votes” are unsigned validator-name assertions, so policy authorization is not cryptographically proven.
7. the legacy owned-wrap path could label native-backed objects as issued assets;
8. the public RPC allowlist exposes an unsigned `wrap_owned` mutation that can debit any account named by the caller and mint a FastPay object for an attacker-controlled key;
9. the PFTL↔Ethereum route accepts operator assertions of Ethereum consumption/burn/finality without verifying Ethereum headers, receipts, logs, or non-consumption, permitting cross-chain double representation under operator compromise;
10. the EVM `MintController` releases escrowed issued assets against a beneficiary-supplied “settlement proof” that has no signature or verifier and can represent fictitious proceeds/liquidity;
11. the wallet proxy binds all interfaces, allows all origins, enables its native custody signer, and dispatches HTTP/WebSocket mutations without authentication by default;
12. the replicated state root omitted every FastLane/FastSwap ledger field, allowing economically different reserve, authorization, committee, checkpoint, and activation state to share one root.
13. the height-zero replay base allowed an operator to rewrite both the faucet account and ledger to a different native supply while `verify-blocks` still returned `verified=true` under an otherwise unchanged genesis identity.
14. issued-asset cap checks summed transparent trustlines, escrows, and offers but omitted FastLane reserves and live AssetOrchard balances, allowing custody migration followed by otherwise authorized reminting above `max_supply`.

The audit also confirmed P1 blockers in storage scaling, deterministic monetary
arithmetic, validator-key custody, public RPC hardening, FastPay
cancellation/liveness, dependency health, CI, documentation truthfulness, and
public-history hygiene. All have candidate fixes. The provider owner confirmed
destruction of the historical credential/instance, and publication uses history
rooted in the sanitized export. The complete workspace/release-scale safety
battery, snapshot/replay, six-node deployment gates and customer flow are
recorded below.

The correct sequence is:

- finish this audit and its blocker register;
- reproduce each P0/P1 with a failing test or formal counterexample;
- implement complete fixes or remove the affected surface;
- run the integrated adversarial and release batteries;
- construct a sanitized public-release candidate with zero open P0/P1 findings;
- publish only that candidate, not the current repository history by default.

## 2. Scope, baseline, and evidence rules

### 2.1 Baseline

The audited baseline consolidates the complete local FastSwap work and the FastPay payment-safety branch. It also contains the already-tested issued-asset owned-wrap guard. The original `main` branch is not an adequate audit target because it omits later production-relevant code.

At audit start:

- branch: `open-source-productionization-20260716`;
- HEAD: `4b5af7bc6bb6e793ed8a60219d13d6d35be03058`;
- upstream: none; branch is local-only;
- remote: `https://github.com/postfiatorg/postfiatl1v2.git`;
- only dirty file: the controlling checklist wording added during this audit;
- host toolchain: `rustc 1.95.0`, `cargo 1.95.0`, Node `20.20.2`, npm `10.8.2`, Python `3.12.3`;
- repository toolchain declaration at baseline: floating `stable`; the candidate
  now pins Rust `1.95.0` with `rustfmt` and `clippy`.

### 2.2 Classification standard

- **Confirmed:** the code or a tool result directly demonstrates the behavior.
- **Reproduction required:** source inspection establishes a credible invariant violation, but a targeted failing test must be added before remediation.
- **Clean:** a specifically named surface has been examined with sufficient evidence; it does not mean adjacent surfaces are clean.
- **Unknown:** the required evidence is absent or incomplete. Unknown is not a pass.

Line references in this document refer to baseline `4b5af7bc`; fixes must update the closure register with their new commit and test locations.

The code-derived transaction/action/RPC inventory is maintained in `docs/status/OPEN-SOURCE-PRODUCTIONIZATION-SURFACE-INVENTORY-20260716.md`. It records all eight normal transaction-batch families, all 36 asset/NAV/bridge variants and their signed source roles, the ten shielded actions, bridge/governance/FastLane/FastSwap families, 81 no-flag remote RPC names, protocol mutation endpoints, and opt-in submission gates.

The controlling claim-by-claim review is maintained in
`docs/status/OPEN-SOURCE-WHITEPAPER-CONFORMANCE-MATRIX-20260716.md`. It maps the
candidate Version 3 paper across protocol foundations, state, consensus,
ordering, governance, privacy, cryptography, recovery, evidence, and major live
features omitted from the paper. The short matrix in section 8 below is only an
executive summary; the separate matrix is the STEP 1 evidence artifact.

The primitive, proof-system, circuit, domain, key-purpose, randomness, parameter,
and custody review is maintained in
`docs/status/OPEN-SOURCE-CRYPTOGRAPHY-INVENTORY-20260716.md`. It records the
actual hybrid/PQ/classical boundary and the concrete release gates, including
the absence of SLH-DSA recovery. `scripts/test-crypto-callsite-policy` freezes
all 46 generic-context and deterministic-seed/key production call sites so a
new purpose cannot silently reuse an audited cryptographic boundary.

The persisted-domain, crash-journal, state-root, complexity, migration, and
determinism review is maintained in
`docs/status/OPEN-SOURCE-STORAGE-STATE-DETERMINISM-INVENTORY-20260716.md`. It
records the useful existing WAL/canonical-root behavior, the complete enabled
canonical-encoding family map, and the nondeterministic-API classification.
`scripts/test-consensus-determinism-surface` now fails closed on clocks,
randomness, environment input, directory enumeration, unordered collections or
floating point entering 23 execution/order/batch/root source files. The bounded
JSON/JSONL design remains feature-contained and is not represented as a
production storage engine.

The bridge/Ethereum/Solidity trust-boundary review is maintained in
`docs/status/OPEN-SOURCE-BRIDGE-EVM-CONTRACT-AUDIT-20260716.md`. It separates the
generic bridge, vault bridge, Uniswap handoff, market-operations, and withdrawal
federation designs and records both asserted-finality P0-BRIDGE-01 and the newly
confirmed unauthenticated mint-release P0-SUPPLY-01.

## 3. Repository inventory and bloat

### 3.1 Measured composition

| Measure | Baseline result |
|---|---:|
| Tracked files | 2,787 |
| Tracked bytes | 80,492,610 |
| Git object packs | 55.73 MiB |
| Loose Git objects | 19.11 MiB |
| Working checkout excluding `.git`, `target`, `node_modules`, and `site` | 29 GiB (local virtual environments/evidence/build caches dominate; not a source archive) |
| Rust LOC | 302,640 |
| `docs/` | 2,167 files / 52,180,062 bytes |
| `crates/` | 283 files / 19,078,159 bytes |
| `wallet-web/` | 83 files / 4,050,776 bytes |
| `wallet-extension/` | 15 files / 1,420,280 bytes |
| `wallet-proxy/` | 31 files / 1,153,656 bytes |
| `scripts/` | 79 files / 858,195 bytes |
| `third_party/` | 74 files / 788,434 bytes |

The checkout itself is much larger because it contains untracked build outputs, virtual environments, and JavaScript dependencies. Those are not a public-Git bloat finding but must be excluded from release archives.

Largest tracked files at the audited baseline are the 2.10 MB Asset-Orchard
parameter artifact; two 2.00 MB latency reports; two 1.96–1.99 MB browser
captures; a 1.54 MB research PDF; the 1.45 MB web-wallet WASM; four 1.31–1.38 MB
active/replay verifier-key or extension-WASM artifacts; and additional raw
evidence captures below 1 MB. Largest first-party source modules are
`crates/rpc_sdk/src/protocol_requests.rs` (5,182 lines),
`crates/types/src/transactions_mempool_receipts.rs` (5,002), and
`crates/node/src/node_types.rs` (4,996). The exact command and top-30 inventory
are recorded in the lab book; generated dependencies under `node_modules` are
excluded from source-module ranking.

### 3.2 Bloat classes and required disposition

| Class | Evidence | Disposition before publication |
|---|---|---|
| Raw live/test evidence | `docs/evidence/` dominated the 52 MB baseline documentation tree and contained multi-megabyte browser/RPC captures | CLOSED-LOCAL: complete raw tree moved to a restricted hash-manifested archive; only ten curated summaries plus publication policy/manifest remain |
| Screenshots and browser captures | Repeated full browser captures up to ~2 MB each | CLOSED-LOCAL: 19 root/wallet screenshots plus all raw-evidence screenshots are in restricted hash-manifested archives; only public extension icons remain |
| Research PDFs | downloaded VeriLLM PDF and the Cobalt source reference | CLOSED-LOCAL: VeriLLM download removed in favor of canonical DOI; 500,545-byte Cobalt PDF retained under its existing hash-pinned reference policy and Markdown extraction |
| Orchard proving parameters | `crates/privacy_orchard/artifacts/asset_orchard_k15_params.v1.bin` (2.1 MB) | CLOSED-LOCAL / KEEP: active exact parameter bytes remain in source so proving and verification are offline/reproducible; runtime metadata and the artifact policy hash-bind them |
| Verifying keys and replay keys | Four 1.31–1.38 MB binary artifacts, including two explicitly historical replay variants | CLOSED-LOCAL / KEEP: active keys are required for live verification; old keys remain only under `artifacts/replay` for authenticated byte-identical history and cannot be selected live; all are hash-pinned |
| Generated wallet WASM | web and extension bundles previously embedded the builder's home path and had no canonical regeneration command | CLOSED-LOCAL / KEEP: one release script remaps builder paths and reproduced SHA-256 `395576c1efa2fc5115e94df17645f1fb0f5584fd5ce4f7677e6e3539258ea5a2` twice; the two package paths are byte-identical and one Git blob, retained for offline web/extension builds |
| Status diaries and burndowns | Hundreds of `docs/status/` and governance-progress documents | Archive engineering history outside supported product documentation; keep final design/security records only |
| Giant modules | 13 Rust modules exceed 4,000 LOC; largest is 5,182 LOC | Split by protocol responsibility during remediation where it improves review boundaries; do not perform risk-free-looking mechanical churn inside active P0 fixes |
| Duplicate wallet surfaces | Web wallet, extension, proxy, CLI/API/demo paths | Declare one supported custody architecture and one supported backend contract; archive or mark others experimental |
| One-off operational scripts | Large script surface mixes evidence generation, deployment, demo, and supported operations | Classify and relocate to `scripts/release`, `scripts/devnet`, `scripts/audit`, and archived evidence tooling; test supported scripts |

The first executed archive pass removes 1,293 raw-evidence/media files and about
39.6 MB of working-tree content. Remaining generated artifacts/status diaries
are handled by their separate keep/move decisions; exact candidate size is
recomputed at the clean-checkout gate.

The supported public boundary is: root policy/build metadata; `crates/` source
plus the exact hash-bound cryptographic parameters and live/replay keys;
`wallet-web/` and `wallet-proxy/` for the single supported self-custody wallet
path; `scripts/{release,devnet,audit}` after classification; `systemd/` safe
examples; `docs/` for canonical product/protocol/operator/security material; and
`third_party/` only for hash-verified licensed source. Raw evidence, status
diaries and superseded wallet/extension/demo implementations move to a
hash-manifested archive. The sole retained research PDF and deterministic wallet
WASM have explicit source-tree dispositions. `scripts/test-public-artifact-policy`
pins the exact 14 binary/media paths (13 blobs), rejects additions or byte drift,
and records the detailed rationale in
`OPEN-SOURCE-PUBLIC-ARTIFACT-DISPOSITION-20260716.md`.

### 3.3 Structural reviewability

The following modules are too large to treat as single review units: `rpc_sdk/protocol_requests.rs`, `types/transactions_mempool_receipts.rs`, `node/node_types.rs`, `execution/nav_vault_asset_execution.rs`, `rpc_sdk/response_validation.rs`, `privacy_orchard/asset_orchard.rs`, `node/fastswap_service.rs`, `node/rpc_cli.rs`, `node/block_finality.rs`, and `privacy_orchard/asset_orchard_circuit.rs`. Their size is P2 debt, but P0/P1 fixes must first establish ownership boundaries and test seams rather than combining safety fixes with wholesale refactors.

## 4. Confirmed P0 blocker register

### P0-CONSENSUS-01 — production commit rule and cross-view locks do not match the claimed protocol

**Status:** fixed-candidate; automatic shipping recovery and the exact-candidate
four-/six-node and complete workspace gates pass at and after `09125687`.
**Invariant:** two correct nodes must never commit different blocks at the same height.
**Affected code:**

- `crates/node/src/block_finality.rs:2374-2388` derives a vote-lock file from `(height, view, validator)`, allowing an honest validator to reserve a different proposal in a later view at the same height;
- `crates/node/src/block_finality.rs:2855-2865` selects `high_qc_id` as the lexicographic maximum supplied string rather than the highest verified QC by protocol order;
- `crates/node/src/block_finality.rs:3805-3855` proves only that a timeout certificate is for the same height and immediately previous view; it does not prove the referenced high QC exists or that the new proposal extends the locked/high-QC branch;
- `crates/node/src/consensus_artifacts.rs:1191-1220` verifies timeout vote signatures and threshold over opaque QC identifiers but does not resolve those identifiers to verified QCs;
- `crates/node/src/batch_snapshot.rs:191-305` accepts one verified block certificate and proceeds directly into execution/commit preparation;
- `crates/ordering_fast/src/lib.rs` contains a separate HotStuff proposal/QC model and two-chain candidate logic, but production `crates/node` does not use those types for its block commit path.

**Claim conflict:** the historical whitepaper claimed chained HotStuff, while the
live contained path is single-view direct-certificate consensus. The selected v2
replacement is now an explicit two-phase prepare/precommit protocol: prepare QCs
establish durable locks and only a non-nil precommit QC commits.

**Failure scenario to reproduce:** form a quorum certificate for proposal A in view `v`; advance a quorum using a timeout certificate carrying arbitrary or inconsistent high-QC identifiers; have the same correct validators sign conflicting proposal B in view `v+1` because their durable locks are view-scoped; deliver the two certificates to stores with the same parent and demonstrate divergent commits/roots at one height.

**Required complete fix:** either integrate one production consensus state machine with a verified QC graph, durable highest-voted/locked QC state, safe voting predicate, parent/justify binding, view monotonicity, and an explicit commit rule, or remove the unsafe view-change feature and every chained-HotStuff claim. Merely changing the lock filename to height-wide while continuing to accept later views is not complete because it can preserve safety by permanently destroying liveness after a failed proposal.

**Closure evidence:** a pre-fix failing counterexample; model/property proof for quorum intersection and lock monotonicity; deterministic simulation under delay/drop/reorder/partition/restart; crash tests around vote-lock persistence; replay compatibility/migration tests; no lone prepare/direct legacy certificate commit path reachable after v2 activation; whitepaper updated to the exact implemented rule.

**Local remediation evidence:** the pre-fix
`exploit_cross_view_vote_lock_accepts_conflicting_proposals_with_unresolved_high_qc`
test passed on the vulnerable implementation: validator 0 signed proposal A at
height 1/view 0, a three-of-four timeout certificate named the nonexistent
`fabricated-unresolved-qc`, and the same validator then signed conflicting
proposal B at height 1/view 2. Pre-activation proposal validation continues to
reject every nonzero view. After the explicit v2 activation height, later views
are admitted only with a signed timeout certificate whose typed high-QC ancestry
verifies against the exact committee/domain and authorizes the immediately next
view. Legacy heterogeneous opaque high-QC IDs fail closed.

The v2 foundation now defines canonical signed proposal, prepare/precommit vote,
QC, timeout-vote, and timeout-certificate artifacts bound to chain, genesis,
protocol version, committee epoch/root, height/view, parent, payload, state root,
validator, and phase. Timeout votes carry typed QC references; every reference
must resolve to a verified QC, and ranking is numeric rather than lexical. The
legacy v1 timeout aggregator now fails closed on heterogeneous opaque QC IDs.
An exhaustive quorum-subset model passes for `n=4` and `n=6`; signed failed-
proposer simulations advance from view 0 to view 1 and commit only on a
precommit QC. A node-side atomic per-height safety store persists prepare round,
precommit round, locked QC, high QC, and the last vote digest before signature
emission; its restart regression rejects duplicate prepare and precommit votes.
The production TCP regressions commit height 1/view 0 on four and six replicas, advance
past a failed height 2/view 0 proposer, commits height 2/view 1, converges one
tip/root, and replays every node. Snapshot v6 preserves durable safety/QC state,
and activated commits are self-contained in block history. Commit `09125687`
closes the product boundary: the wallet proxy verifies one exact-parent quorum,
collects distinct durable timeout votes, sends a bounded compressed envelope to
the deterministic later-view proposer, and the node aggregates/verifies the
timeout certificate before voting or committing. The normal shipping transfer
finality RPC passes the failed-view-0 scenario at `n=4` in 80.51 seconds and at
`n=6` in 122.55 seconds. Timeout-vote durability, truncated-envelope rejection,
proxy routing, browser exclusion, formatting, affected check and strict Clippy
also pass. The final exact-candidate workspace, replay, four-/six-node simulation, failed-
proposer recovery, snapshot and clean-clone gates now pass; only the separately
classified real-value multi-region campaign remains.

### P0-CUSTODY-01 — browser wallet can export the master seed to the wallet proxy

**Status:** fixed and rerun on candidate `00747667` with real Chromium boundary capture.
**Invariant:** a self-custody browser wallet must never transmit seed or signing authority to the backend.
**Affected code:**

- `wallet-web/src/lib/tx-builder.js:134-149` prefers `rpc.walletSignOwnedTransfer` / `rpc.walletSignOwnedUnwrap` when the methods exist and only falls back to local WASM signing otherwise;
- `wallet-web/src/lib/rpc-client.js:385-389` sends `backup_json` to the proxy;
- the wallet backup contains `master_seed_hex`;
- `wallet-proxy/rpc-routing.js:1222-1339` writes the backup to a temporary file and invokes native wallet signing;
- `wallet-proxy/server.js:235` enables native wallet signing unless explicitly set to `false`.

**Failure scenario:** any compromised, misconfigured, logged, or remotely exposed proxy receiving the request gains the wallet master seed and can sign arbitrary future transactions.

**Required complete fix:** make all self-custody signing local in browser/WASM; remove the backup-bearing RPC contract from the public wallet and public proxy; make any intentionally custodial signer a separate build/service with explicit mode labeling, authentication, encrypted custody, audit controls, and no shared endpoint; prevent sensitive temporary files and subprocess arguments.

**Closure evidence:** an end-to-end browser network-capture regression proving no seed/private key/backup crosses the process boundary for every transaction type; negative RPC tests for removed methods; browser signing tests for transfer, unwrap, swap, bridge, and shielded actions; log and crash-artifact scans.

**Local remediation evidence:** `wallet-web/src/lib/tx-builder.js` now invokes the
WASM owner signers unconditionally; `wallet-web/src/lib/rpc-client.js` no longer
defines either backup-bearing signer RPC; and the public proxy native signer,
temporary wallet-backup file, and subprocess dispatch were removed. Wallet tests
prove a trap proxy signer is never called for owned transfer or unwrap. The real
proxy regression proves both legacy methods return `proxy_method_removed`.
`wallet-web/src/lib/custody-boundary.js` now enforces a shared fail-closed
runtime boundary at WebSocket RPC, swap/private-flow HTTP, bridge-relay and
FastSwap-demo egress. It rejects recursively named private fields, private
fields embedded in JSON strings, and the active in-memory seed or backup even
when hidden under an innocuous key; public keys, signatures and signed
envelopes remain permitted. The registry is populated only from unlocked vault
memory and cleared on lock.

The dedicated headless-Chromium capture exercises 10 WebSocket mutation
classes (Account, PaymentV2, issued asset, escrow, offer, fastlane, FastPay
sign/apply and unwrap sign/apply), 10 HTTP money routes (transparent/private/
shielded/bridge/FastSwap demo), and both MetaMask transaction classes. The
active random seed and backup are absent from every captured ingress body,
browser local/session storage, console output, Node and Chromium process
arguments, the persistent Chromium profile and crash artifacts. A public
signature marker is present, proving the test did not obtain a false pass by
blocking signed traffic. Evidence:
`reports/open-source-p0-browser-custody-20260716T225000Z/ACCEPTANCE.json`,
SHA-256
`538d08989bd5f5f8584a5ee1f021e54dc8a4eaca7801d51adbf7fbb08c203ac0`.
On exact candidate `00747667`, `npm run test:custody-browser` and the public
browser suite pass `1/1` each, wallet tests pass `240/240`, the production build
passes, the wallet-proxy regression suite passes `23/23`, and both audits report
zero vulnerabilities.

### P0-CUSTODY-02 — redacted wallet test-vector report echoes both input seeds

**Status:** fixed and shipping-subprocess regression rerun on candidate `00747667`.
**Invariant:** a command that claims `private_key_material_redacted=true` must
not serialize caller-supplied master or signature seeds.
**Real boundary reproduction:** invoking the shipping `postfiat-node
wallet-test-vector` CLI with explicit `--master-seed-hex` and
`--signature-seed-hex` emitted both exact values as `master_seed_hex` and
`signature_seed_hex` in stdout JSON while simultaneously setting
`private_key_material_redacted=true`. A user treating the command as a safe
public-vector/report generator could therefore copy a live wallet seed into
logs, CI artifacts, or bug reports.

**Fix and closure evidence:** `WalletTestVectorReport` no longer has either
secret field and its schema is versioned to
`postfiat-wallet-test-vector-v2`. The deterministic address, public key,
signing bytes/hash, signed transfer, transaction ID, fee and verification
result remain. The unit regression rejects both field names and both supplied
values. A real shipping-binary subprocess test now exercises success and a
post-secret-ingress failure, scans stdout, stderr, argv, working-directory
artifacts, and crash/panic artifact names, and proves the v2 schema and absence
of both names/values. `cargo test -p postfiat-node --test
wallet_test_vector_redaction --locked -- --nocapture` passes 1/1 on the exact
candidate.

### P0-PUBLIC-EVIDENCE-01 — public source tree contains private note openings

**Status:** fixed locally in the publication tree; public-history sanitation is
still covered by `P0-SECRET-01`.
**Invariant:** a public source repository must not distribute raw wallet/prover
evidence containing shielded note openings or operator-local artifacts.

The tracked `docs/evidence/` tree contained 1,283 files (36,326,352 source
bytes). Real certified legacy-ingress batches serialized the complete note,
including 64-hex `rho`, `psi`, and `rcm` values, recipient data, amount, and the
old plaintext `encrypted_output`. It also contained browser captures, validator
responses, absolute machine paths, topology, and wallet identifiers. This was
both a privacy publication defect and the largest removable documentation-bloat
class.

A real scanner regression first failed because the existing secret scanner did
not recognize a shielded note opening. The scanner now has a
`private-note-opening` rule for note-opening/spend-authority fields and reports
only rule/path/line metadata. The pre-fix tracked-tree run found 21 concrete
opening values across seven raw ingress artifacts without printing any value.

The entire raw evidence directory was preserved as a deterministic restricted
tar archive before removal. Its manifest records 1,283 files, archive size
38,144,000 bytes, and SHA-256
`ac6911368cb199e475dce8fce2309ffd18811ab9c6ca5048aae9a85084cb5eea`.
A full listing count and byte-for-byte extraction hash of a known ingress batch
matched the source. The publication tree now retains ten curated redaction-safe
evidence summaries plus `RAW-EVIDENCE-POLICY.md` and the archive manifest.
`scripts/test-public-secret-scan` passes, and the tracked-tree scan is green.
No claim is made that contaminated historical commits are safe to publish; the
sanitized-history requirement remains fail-closed under `P0-SECRET-01`. A local
1,561-file one-commit export rehearsal passed the exact-tree/ref/current/history
publication gate with zero findings; final reviewed-revision and staging-remote
execution remain required.

### P0-GOVERNANCE-01 — governance support is represented by unsigned validator names

**Status:** fixed-candidate; signed authorization, adversarial, replay, rollback,
and integrated exact-candidate gates pass.
**Invariant:** no governance amendment, validator-registry transition, pause, activation, crypto policy, or FastSwap bootstrap may become valid without cryptographic authorization by the required old-rule authority set.
**Baseline affected code (superseded by the local remediation below):**

- `crates/types/src/shielded_bridge_governance.rs:865-870` defines `GovernanceVote` as only `vote_id`, `validator`, and `accept`; there is no public key, algorithm, registry-root binding, or signature;
- `crates/consensus_cobalt/src/internal_validation.rs:447-465` constructs votes directly from a caller-supplied support list;
- `crates/consensus_cobalt/src/internal_validation.rs:1346-1481` verifies membership, sorted support, quorum length, and recomputed hashes, but verifies no signature;
- `crates/node/src/governance.rs:1-43` “ratifies” amendments from validator/support names without validator keys;
- `crates/node/src/consensus_artifacts.rs:1457-1515` accepts a governance batch after the unsigned evidence and batch hash validate;
- `crates/node/src/execution_actions.rs:291-352` applies amendments and records validator updates once that structural validation has passed;
- the RBC/ABBA message validators in `crates/consensus_cobalt/src/rbc_abba_mvba.rs:101-197` validate signature text shape and message IDs, not the signature against the sender's registered key. Signing-payload helpers exist, but production verification call sites were not found outside tests/examples.

**Why the block certificate is insufficient:** a normal validator signs a block because every included transition passes deterministic validity rules; that vote is not an operator policy decision. If a proposer can synthesize the purported governance support and the deterministic validity function accepts it, honest validators will certify the resulting block as valid. Treating that automatic block vote as the missing governance authorization collapses governance to proposer initiation plus ordinary block validity and contradicts the whitepaper's old-rules-authorize-new-rules premise.

**Failure scenario to reproduce:** without any validator private key, construct an amendment or registry update naming a quorum of active validators, recompute its vote/certificate/amendment IDs with the public builders, include it in a proposal, and demonstrate that proposal validation accepts the governance state transition.

**Required complete fix:** define signed, domain-separated governance votes bound to chain/genesis/protocol, amendment slot/instance, complete proposal payload, old active registry root, validator ID, key algorithm, and activation lifecycle. Verify distinct signatures against the old active registry at construction, proposal validation, execution, replay, and state verification. Make RBC/ABBA signature verification real if those message paths are production-authoritative; otherwise label/remove them from the production claim. Preserve historical replay through an explicit activation height/versioned decoder, never through a permissive live fallback.

**Closure evidence:** pre-fix no-key forgery test; post-fix forged/missing/duplicate/wrong-domain/wrong-registry/stale-key votes fail without mutation; old/new registry transition and rollback simulations; crash/replay/migration tests; whitepaper updated to the exact signed authorization path.

**Local remediation evidence:** the no-key regression constructs an amendment
using only four validator names and proves the vulnerable legacy builder needed
no private key. Live proposal and apply now require the v2 signed authorization
envelope. Its ML-DSA-65 transcript binds chain/genesis/protocol, the complete
action, old registry root, committee epoch, exact proposal slot, expiry,
validator and algorithm; distinct signatures are checked against the old active
registry at proposal, apply, archived replay and full state verification.
Unsigned legacy artifacts remain authenticated-history-only. Isolated-key CLI
signing/assembly exists for amendments, registry updates and FastSwap bootstrap.
Wrong-chain, wrong-epoch, wrong-registry, wrong-slot, expired, stale-key,
altered-payload, missing and duplicate evidence all reject without mutation.
Real n=4 and n=6 TCP tests commit a signed amendment, old-rule-authorized delayed
key rotation, the activation block and a first block under the replacement key,
then converge and replay every replica. Consensus-v2 QC/safety artifacts are
committee-domain namespaced so historical epoch evidence cannot poison the live
QC graph. Concurrent-amendment, partition, crash and rollback campaigns remain.

### P0-PRIVACY-01 — legacy cleartext note path remains reachable under a shielded name

**Status:** fixed candidate; historical decoder remains replay-only.
**Invariant:** an operation represented as shielded must not publish owner, asset, amount, or memo in ledger state.
**Affected code:**

- `crates/types/src/shielded_bridge_governance.rs:55-65` defines legacy `ShieldedNote` with cleartext `owner`, `asset`, `value`, and `memo`;
- `crates/node/src/shielded_batch_actions.rs:49-93` creates legacy mint, spend, and migrate action batches;
- the shielded ordered-batch execution path still handles legacy mint/spend actions alongside Orchard actions.

**Claim conflict:** `docs/whitepaper.md:11`, `:57`, and the privacy sections present note-based settlement as the baseline privacy path, with narrow metadata leakage rather than cleartext ownership/value.

**Required complete fix:** reject creation/admission of new legacy cleartext mint/spend actions at the consensus boundary; retain only an explicitly versioned historical replay/migration decoder if existing chain history requires it; rename legacy types and receipts so no API labels them private; route supported privacy operations exclusively through Asset-Orchard; remove or correct claims that exceed the proven leakage model.

**Closure evidence:** pre-fix test demonstrates a cleartext legacy note can be admitted; post-fix consensus/RPC tests reject new actions without mutation; historical replay still reproduces old roots if required; wire/RPC/state scans show supported shielded actions expose only the documented public fields.

**Local remediation evidence:** direct legacy `shield_spend` and the legacy mint
and spend batch constructors now return `PermissionDenied`. The live proposal
builder rejects any manually supplied batch containing `ShieldedAction::Mint`
or `Spend`; execution also returns
`legacy_cleartext_shielded_action_disabled` without mutation whenever
`archive_replay=false`. The versioned decoder and execution remain available
only when the caller is the archive-replay path. The regression
`legacy_cleartext_shielded_actions_are_historical_replay_only` proves creator
rejection, proposal-admission rejection, live no-mutation rejection, and
successful historical replay of the same fixture. `Migrate` remains available
as an explicit turnstile operation for historical note retirement; it must not
be presented as confidential transfer.

The public file-based shielded apply boundary now has a concrete legacy-action
injection regression: a manually serialized, correctly identified legacy mint
batch produces only `accepted=false,
code=legacy_cleartext_shielded_action_disabled` and leaves shielded state
byte-identical. Review of authenticated catch-up found and reproduced a second
real P0: historical apply previously substituted the archived header state root
without comparing it to the state actually produced locally. A valid historical
certificate therefore committed successfully over a ledger whose balance had
been changed by one atom, producing a header/state divergence. The same path
also did not require the archived parent to equal the current local tip.

`prepare_ordered_commit_timed` now fails before certificate acceptance or WAL
write unless the archived parent equals the current tip, the recomputed receipt
IDs are exact, and the resulting state matches either the current root schema
or one of the narrowly retained legacy root schemas under its existing exact
chain/height/batch predicates. The positive retained-history catch-up fixture
still passes. Both a one-atom pre-state divergence and a wrong-parent tip reject
without changing ledger, tip, or block history. Targeted replay tests pass
`3/3`; the public legacy-action boundary passes `1/1`; affected node check and
strict Clippy pass.

The exhaustive public creation-surface pass also found that the direct
`shield_mint` function remained live even though `shield_spend` and the two
legacy batch builders were already fail-closed. With direct-state mode enabled,
the shipping CLI/RPC wrappers could therefore still create a cleartext note and
append an accepted receipt outside Asset-Orchard. The new real-boundary
regression failed by returning a complete `ShieldedNote`; `shield_mint` now
returns `PermissionDenied` before opening the store, matching the other three
legacy creation APIs. The Orchard migration capability remains tested by
seeding an authenticated historical fixture internally, not by reopening a
public cleartext mint.

A repo-wide production call-site scan now finds legacy `Mint`, `Spend`, and
`AssetOrchardIngressV1` variants only in the live rejection gate, the
historical-replay executor and archive lookup. Neither wallet nor proxy contains
a v1 ingress schema; both bind `postfiat-asset-orchard-ingress-file-v2`.
The current issued-asset ingress/egress round trip asserts the emitted batch is
`AssetOrchardIngressV2`; wallet shielded/swap tests pass `34/34`, the proxy
adapter passes, and the public direct/ordered legacy regression passes without
mutation. This closes the public creation-surface version gate without removing
Asset-Orchard deposit, transfer, swap, or egress functionality.

The complete local privacy flow is now proven at the ordered-store boundary.
Two issued assets enter through encrypted ingress-v2 envelopes, a generic
private transfer commits with exact receipt/finality linkage, a real K15
Asset-Orchard atomic swap nullifies both inputs and creates recoverable encrypted
outputs, and a private egress nullifies its input while crediting the exact
public issued-asset delta. Every applied money receipt is `accepted=true,
code=accepted`; global issued supply is unchanged. The flow scans 13 public
envelope, batch, archive, block, receipt, ledger and shielded-state artifacts
for both private field names and the exact note-opening/spend-authority values.
The exact swap/egress regression passes `1/1` in 2,524.27 seconds and the generic
private-transfer regression passes `1/1` in 372.91 seconds. Evidence:
`reports/open-source-p0-privacy-complete-flow-20260716T235019Z/ACCEPTANCE.json`
(`sha256:b830ce023d078aeed4acc832679591072519827f60af72d778b144dc5d5672ec`).
The immutable-candidate proving/replay rerun is also green: ordinary Orchard
and replay tests passed `83/83`; all 16 release-scale proving/verifying,
tamper, authority, anchor/path, conservation, egress and key-metadata tests
passed; and the separate parameter writer reproduced the committed
2,097,220-byte K15 artifact byte-for-byte (SHA-256
`e1fb2974a4a0a87f8ac0dbaaa4c7ea3c4e9f293a560585f7ca6233b78f42d0dd`).

### P0-PRIVACY-02 — live Asset-Orchard ingress archives the recipient note opening

**Status:** fixed candidate; live v2 encrypted boundary, v1 historical replay only.
**Invariant:** a shielded ingress may expose the public burn, asset/amount,
commitment and authenticated ciphertext, but must not publish the recipient note
opening or its randomness to validators, RPC relays, or block history.
**Affected real boundary:** `AssetOrchardIngressActionPayload` contains
`AssetOrchardIngressNote`, whose fields include `value`, `rho`, `psi`, `rcm`,
diversifier and recipient key material. `create_asset_orchard_ingress_batch`
places that payload in `ShieldedAction::AssetOrchardIngressV1`; the ordered
shielded batch is certified and archived. The browser builder likewise copies
the full local-vault note into the remotely submitted payload and, absent an
override, labels a deterministic plaintext string as `encrypted_output`.

**Impact:** the public burn already reveals ingress asset and amount, but the
current payload also reveals and permanently links the newly created note
opening. It directly contradicts the whitepaper statement that validators do
not learn the recipient note and defeats the intended ciphertext boundary.

**Required complete fix:** add a versioned ingress payload containing only the
signed burn, pool/asset/amount, output commitment and the existing
`PFAOENC1` authenticated note ciphertext. The burn signer may commit an opaque
output: a malformed commitment can only make the signer's burned funds
unspendable and cannot inflate supply. Disable live v1 before accepting the new
path; retain v1 only behind authenticated archive replay. Update the local
prover/browser handoff so private note material remains in the loopback vault
and only the ciphertext crosses the relay.

**Closure evidence:** pre-fix serialized batch/archive contains the exact note
opening; post-fix live v1 rejects before burn with no mutation; v2 JSON and wire
capture contain none of the opening values/field names; ciphertext has the
versioned magic, rejects mutation/wrong chain/wrong recipient, and decrypts for
the intended note seed; a real ingress-to-private-swap-to-private-egress flow
conserves assets and archive replay reproduces historical v1 receipts/roots.

**Local remediation evidence:** `ShieldedAction::AssetOrchardIngressV2` and the
v2 ingress-file schema omit the note opening and accept only lowercase bounded
`PFAOENC1` ciphertext. The local prover returns a genuine randomized encrypted
output separately from the wallet-local note; the React and scripted clients
must supply it and no longer have a plaintext fallback. Proposal admission and
live execution reject v1, while an explicit archive-replay execution of the
same valid fixture remains accepted. The issued-asset round-trip regression
checks the serialized v2 batch for the absence of `note`, `value`, `rho`,
`psi`, and `rcm`, decrypts for the intended seed, and completes conserved
ingress/egress. Targeted node tests, all 21 local-prover tests, wallet 219/219,
wallet production build/audit, proxy 22/22, workspace check, formatting, and
strict Clippy pass.

The encrypted-v2 adversarial matrix is now explicit. A wrong recipient and a
tampered authenticated ciphertext return no note; malformed length/magic and a
caller-supplied plaintext label reject; an already accepted v2 ingress batch
returns `AlreadyExists`; live v1 downgrade rejects before state change; and a
mixed batch containing one valid v2 action cannot mask an embedded v1 action.
The updated issued-asset ingress/egress round trip passes with these replay and
mixed-version assertions, and the focused encryption/length tests pass `2/2`.

The public capture boundary is now covered across the actual headless-Chromium
HTTP/WebSocket client, proxy ingress, and the ordered node store's receipts,
block/archive log, ledger and shielded state. The browser capture reports no
private marker, custody seed or backup in the wire, browser storage, console,
process arguments, Chromium profile or crash artifacts; its acceptance hash is
`e07bacd757b646022a13bdf28a9730a30a158920141eaf73677ee92f1597df01`.
An adversarial follow-up found that the recursive JSON scanner previously
skipped strings above 1 MiB even though the HTTP body boundary permits a larger
request. Wallet and proxy regressions failed before remediation; JSON-looking
serialized fields above the inspection budget now fail closed before transport
or custody dispatch. Wallet `232/232`, proxy `23/23`, both production npm audits,
the wallet build, workspace check and strict Clippy are green. Historical v1
artifacts remain reproduction evidence and are not represented as clean v2
capture evidence.

The final two-fresh-wallet candidate gate is green at commit `d1e68ee8`.
Two distinct previously absent accounts are funded with separate accepted
receipts, establish independent trustlines/issued holdings, and create separate
encrypted-v2 private inputs. The real release-profile K15 swap accepts, records
both nullifiers exactly once, supports chain-only output recovery, and completes
private egress with `code=accepted`, an exact positive public delta, conserved
global issued supply, and no private material in 13 public artifacts. The same
run retains the plaintext, mixed-version, invalid-proof, stale-epoch, wrong-
packet, off-band and replay negative controls. Evidence:
`reports/open-source-p0-privacy-two-wallet-20260717T052157Z/ACCEPTANCE.json`
(SHA-256
`ffcd818b23f60cc81eaa660d2b0f01bb0fddc9e6a593e9758bce842fa2686978`).

### P0-WALLET-02 — publicly bound vulnerable Vite server proxies wallet mutations

**Status:** fixed and rerun on candidate `00747667`; static hardened serving replaces public development serving.
**Invariant:** public wallet serving must not expose a development server or implicit mutation proxy.
**Affected code:**

- `wallet-web/vite.config.js:45-70` binds the development server to `0.0.0.0` and proxies RPC, NAV swap, shielded swap, and FastSwap demo mutations;
- CSP is deliberately absent in development mode;
- `wallet-web/package.json` uses Vite `^5.4.2`;
- `npm audit --json` reports a direct high-severity Vite path traversal/server-fs bypass class plus an esbuild cross-origin dev-server disclosure.

**Required complete fix:** bind development to loopback by default; never document or package Vite as the production server; update the dependency chain; produce a static immutable build served through the hardened same-origin service/Caddy with CSP, origin enforcement, authentication for mutations, and cache controls.

**Closure evidence:** clean npm audit or explicit non-reachable advisory proof; tests that default dev/preview cannot bind publicly; browser security-header test; origin/CSRF/clickjacking tests; production deployment artifact contains no Vite dev middleware.

**Local remediation evidence:** `wallet-web/vite.config.js` now binds both the
development and preview servers to `127.0.0.1` with strict ports, and its CSP
does not grant arbitrary `ws:`/`wss:` origins. Vite was upgraded from 5.4 to
8.1.4 with the matching React plugin; `npm audit` reports zero vulnerabilities.
The production wallet builds to static files, and the authenticated same-origin
wallet proxy serves only those files with CSP, anti-clickjacking, MIME-sniffing,
referrer, permissions, and explicit HTML/asset cache policies. A follow-up
runtime reproduction found two remaining defects: every file under `assets/`
was cached immutable even without a content hash, and unknown Vite/source-map
paths fell through to the generic HTTP `200` response. The test failed before
the fix on the un-hashed cache assertion. The server now grants immutable
caching only to content-hashed build assets, rejects dotfiles, source maps,
Vite, source and `node_modules` paths even if present, confines canonical file
paths to the build root, exposes an explicit `/healthz`, and returns `404` for
all other unknown paths. The Docker image contains no Vite dependency or
command, both supported Compose profiles mount only `/wallet/dist` read-only,
and their health checks use `/healthz`.

Real headless Chromium then exercised the actual production build through the
shipping proxy. CSP blocked inline code, a foreign base, foreign connections
and foreign framing; a same-origin unauthenticated mutation returned `401`, a
credentialed foreign-origin mutation was blocked, HTML remained `no-store`,
hashed assets were immutable, and Vite/source/source-map paths returned `404`.
Evidence:
`reports/open-source-p0-wallet-browser-20260716T225500Z/ACCEPTANCE.json`
(SHA-256
`465041cdf6e0cedd81f36da2ccad72de74159559c8fe0125811325bf8684e5ea`).
On exact candidate `00747667`, the hardened static boundary, both Chromium
suites, wallet `240/240`, proxy `23/23`, production build, and both npm audits
(zero vulnerabilities) pass.

### P0-WALLET-BRIDGE-DEST-01 — public wallet targeted a retired bridge vault

**Status:** fixed-candidate; governed route authority, wallet binding, rotation
handling, source-backed conservation, verifier-neutral promotion, state
migration/rollback, controlled round trip, and exact-candidate gates pass.
**Invariant:** a public wallet must never acquire a money destination from an implicit source default or mutable per-user setting; an absent reviewed deployment binding must disable the operation.

**Affected code and reproduction:** `wallet-web/src/lib/utils.js` exported
`BRIDGE_VAULT_CONTRACT` with a source fallback of
`0x1A15e6103D6Af4e88924F748e13B829D3948DEa9`, while
`docs/status/navswap-morning-handoff-2026-06-29.md` identifies that address as
the drained old vault. `wallet-web/src/components/More.jsx` also exposed a
per-wallet bridge-vault setting. The same retired address was the implicit
`VAULT_BRIDGE_VAULT_ADDRESS` backend-relay target in
`wallet-proxy/server.js` and the transaction allowlisted `VAULT` in
`scripts/stakehub-wallet-bridge-ux-live.mjs`. The boundary regression
`public wallet has no implicit bridge vault money destination` was written
first and failed because the retired address was exported instead of the empty
fail-closed value.

**Required complete fix:** bridge money destinations must come only from a
complete versioned route profile authenticated by signed governance and
replicated chain state. The profile must bind source chain, vault and token
addresses and runtime-code hashes, activation/expiry, evidence tier, verifier,
and confirmation/challenge policy. New ingress must use only the current route;
in-flight deposits and redemptions must finish against their immutable pinned
route. Wallet and relay must independently verify the source network and live
code before signing. The complete bridge state must expose and fail closed on
the exact `V = S + D + B - R` conservation identity across every governed route
epoch. No environment variable, proxy, per-user setting, or bundled manifest
may substitute the authenticated money destination.

**Local remediation evidence:** the original empty-default and retired-address
guards remain. `VaultBridgeRouteProfileV1` now commits every authority field,
and signed governance stores the complete canonical profile plus deterministic
active selection in replicated state. `vault_bridge_route(asset_id)` returns
the complete chain-authenticated active profile, governance authorization,
route binding, height/freshness, verifier, and evidence tier. The wallet and
proxy accept only that response; neither may select or override an address,
token, code hash, tier, or epoch. The browser verifies both deployed runtime
hashes before approval or deposit, signs `depositV2` with the exact profile-hash
and epoch binding, and displays the concrete observer-quorum or receipt-proof
trust dependency. Legacy/unbound deposits fail before token movement.

Execution and node admission now distinguish new ingress from pinned lifecycle
state. Rotation rejects stale-route new deposits without mutation, while an
already admitted deposit and pending redemption continue using their exact
historical profile; removing that profile fails closed before mutation. Egress
withdrawal packets bind the pinned chain, vault, token, bucket, and policy.
`vault-bridge-conservation-audit` reads every governed old/current source vault
directly, verifies source chain and runtime bytecode, checks each known deposit
and withdrawal against the source contract, and proves `V = S + D + B - R`.
The real process boundary covers balances split across two route epochs and
rejects wrong chain, runtime drift, a source-absent deposit, PFTL settlement
without source claim, and a one-atom unexplained delta.

The route authority now commits the complete verifier contract as well: the
proof policy, program verifying key, encoding, and proof/public-value limits.
An SP1 profile keeps its 32-byte public-input policy distinct from the
SHA3-384 governed route hash, and the registered NAV profile binds both. Node
activation rejects any mismatch between those commitments. The unchanged
`vault_bridge_route(asset_id)` API discovers both observer-quorum and
receipt-proven routes without changing bridge accounting. The browser
recomputes the complete route preimage itself, rejects substituted timing,
vault, or verifier fields even when a proxy repeats the expected digest, and
requires the complete receipt-proof contract before presenting the route.

The route-state matrix commits asset/profile setup at height 1, activates the
signed route-authority boundary at height 2, and commits the signed exact route
at height 3. It verifies history at both boundaries, exports both snapshots,
restores the pre-route snapshot to the byte-identical height-2 root, reapplies
the exact certified height-3 batch without validator private keys, and reaches
the byte-identical height-3 root and tip. A separate post-route restore retains
the complete active profile and passes full block replay. Every route field and
the deterministic active selection are state-root-sensitive, while empty
legacy route bindings preserve their original JSON and profile IDs.

The final real-boundary gate deploys the production `PFTLWithdrawalVerifier`
and `ERC20BridgeVault` plus a test ERC20 on isolated Anvil. One profile binds
the observed chain, deployed runtime hashes, route epoch and evidence policy.
The production receipt relay admits the actual `depositV2` receipt, and the
same pinned profile governs PFTL propose/attest/finalize/claim, burn-to-redeem,
source proof/finalize/claim, observed redemption settlement, and conservation.
All 11 PFTL receipts are `accepted=true, code=accepted`; all eight EVM lifecycle
receipts have status `0x1`. Exact `V = S + D + B - R` audits pass after claim,
burn, source release and terminal settlement, ending with zero vault balance,
zero PFTL claim/redemption balance and the original 1,000,000 atoms returned.
The rerunnable gate and redacted report are
`reports/open-source-p0-governed-bridge-roundtrip-20260716T221559Z/`.

Current gates: execution `146/146`; node governed-route `6/6` plus the ignored
real-boundary gate `1/1`; conservation `3/3`; wallet `226/226`, static build,
and zero-vulnerability audit; proxy `23/23`; ERC20 vault `13/13`; workspace
check, node strict Clippy, formatting and diff checks pass. All 15
bridge-destination-specific boxes and the immutable exact-candidate gates are
green. Production external-route activation remains a real-value launch gate.

### P0-PROXY-AUTH-01 — public wallet proxy has no authentication boundary

**Status:** fixed and exhaustive route inventory rerun on candidate `00747667`.
**Invariant:** a public wallet/backend process must reject every state-changing or custody operation before route dispatch unless the request is bound to an authenticated user/session and an allowed origin; public binding must never occur by default.

**Affected code:**

- `wallet-proxy/server.js:31-48` defaults `LISTEN_HOST` to `0.0.0.0`;
- `wallet-proxy/server.js:35` makes the origin allowlist empty by default, while `server.js:339-347` and `navswap-persistence-http.js:1469-1472` accept every origin when the list is empty and also accept requests with no `Origin` header;
- `wallet-proxy/server.js:235` enables native wallet signing unless explicitly disabled;
- `wallet-proxy/server.js:330-347` places the NAVSwap HTTP dispatcher and WebSocket dispatcher on the same listener without an authentication check;
- `wallet-proxy/navswap-persistence-http.js:1500-1708` exposes bridge relay, shielded ingress/swap/egress, devnet funding, NAVSwap run, and atomic-template mutation routes without user authentication;
- `wallet-proxy/rpc-routing.js:1222-1339` dispatches originless/tokenless WebSocket requests into a native process that receives a wallet backup and signs owned operations.

`wallet-proxy/test_public_auth_boundary_regression.js` starts the real exported server and proves the current behavior without moving money or supplying a secret: the configured default is public; an originless, tokenless HTTP bridge mutation reaches route-specific handling rather than an authorization rejection; and an originless, tokenless WebSocket call reaches the enabled custody signer and fails only because `backup_json` was omitted. The reproduction passes on the vulnerable baseline.

**Required complete fix:** default to loopback and refuse non-loopback startup without an explicit production exposure profile; require a cryptographically random, non-logged, non-URL bearer/session token or stronger authenticated gateway for every mutation; bind idempotency and authorization to user, chain, method and payload; require an exact nonempty browser origin allowlist; split public reads from wallet mutations and operator/devnet funding/bridge/prover controls; remove native seed-bearing signing from the public process under P0-CUSTODY-01. CORS/origin checks alone are not authentication and requests without `Origin` must not bypass the policy.

**Closure evidence:** invert the dynamic reproduction so default public bind fails closed, tokenless/wrong-token/missing-origin/foreign-origin mutation requests return authorization errors before any route work, public WebSocket method dispatch contains no custody signer, cross-user idempotency replay fails, and authenticated same-origin money flows retain receipt-code and conservation verification. Add CSRF, WebSocket cross-site hijacking, token leakage/logging, rate-limit, restart/session, and reverse-proxy tests.

**Local remediation evidence:** the dynamic reproduction is now inverted and
green. The proxy defaults to loopback, refuses an incomplete non-loopback
exposure profile, requires a constant-time bearer for the enumerated money
mutations, exact-allowlists browser origins, deletes the bearer before upstream
forwarding, and removes public custody signing. The wallet stores the configured
mutation bearer only in browser `sessionStorage` and attaches it to HTTP/WS
requests; it is deliberately excluded from persistent wallet settings. Targeted
proxy and browser suites and the production wallet build pass. Every HTTP POST
route is now source-inventoried: seven deliberately side-effect-free
preparation/read routes are explicit exceptions, while every other and every
future unknown POST fails closed behind authentication. Hostile-origin and
cross-user idempotency regressions pass.

The supported `docker-compose.wallet-public.yml` deployment has also passed an
isolated real-edge proof. Only pinned Caddy publishes HTTPS; the proxy is
internal-only. Both processes run non-root with read-only root filesystems,
least capabilities and bounded writable state. Principal tokens come from a
private file; durable idempotency is scoped by authenticated principal, method,
path and key. Real HTTPS probes prove 401 without authentication, 403 for a
foreign browser origin, 413 at the body ceiling, 429 for both principal rate
and process concurrency limits, and authenticated route dispatch without money
execution. Valid WSS opens and a hostile-origin handshake receives 403 before
upgrade. Container logs contain neither the test bearer nor an Authorization
field. Evidence and exact artifact hashes are in
`reports/open-source-p0-proxy-auth-edge-20260716T192900Z/README.md`. Proxy tests
pass 23/23; wallet tests pass 240/240; both npm audits report zero
vulnerabilities. The exact-candidate RPC inventory additionally classifies all
143 observed methods with zero unknown or unpartitioned public mutations.

### P0-SECRET-01 — historical cloud-instance token would be disclosed by publishing existing history

**Status:** closed. The provider owner confirmed destruction; the private
terminal-action record and exact sanitized GitHub clone pass the fail-closed gate.
**Invariant:** no credential or bearer token may exist in any publicly reachable Git object/ref.
**Evidence:** the full-history Gitleaks report identifies `jupyter_token` in three historical Vast instance captures under `reports/gov-inference-provider/dga-gate-3_5-validator-evidence-lineage-v1/` at commit `fcd54bfedb5468a153c5b41abd3a53855759e6be`. Local non-disclosing inspection confirms all three records contain the same 64-character value. The scanner report redacts the value; this audit records neither the token nor a reusable derivative.

**Required complete fix:** prove the associated instance/token is revoked or destroyed; treat it as compromised regardless of believed expiry; remove the captures and any other real secrets from every published ref/object; publish from a new sanitized repository or a verified rewritten history; run full-history secret scans against exactly the refs to be made public; document the publication procedure so private backup refs are not accidentally exposed.

**Closure evidence:** provider-side revocation/decommission record, sanitized-ref manifest, zero untriaged full-history findings, clean clone scan from the exact public candidate, and a test push to a private staging remote proving no excluded refs are transferred.

The publication verifier now requires the private provider record as an input;
there is no bypassing optional mode. It accepts only a bounded versioned schema,
requires a terminal action and owner/verifier UTC evidence, rejects group/other
permissions, symlinks, files inside the public candidate, missing/extra fields,
and any missing record. The record contains incident/evidence references, never
the credential. Regression coverage preserves exact-tree/ref and clean-history
checks while proving the provider-record boundary fails closed. The provider
owner's terminal-action confirmation and the final sanitized GitHub clone now
close the operational side of this finding.

**Local prevention evidence:** `scripts/verify-publication-candidate` now accepts
only a clean, complete, non-shallow staging worktree with an exact ref allowlist,
an exact reviewed Git tree object ID, and zero findings from both the tracked-tree
and all-reachable-history scans. Its regression proves a clean one-ref export
passes while an unexpected tag, unreviewed tree drift, and a credential that was
deleted from the current tree but remains reachable in history each fail. The
gate does not waive the provider-side revocation/decommission requirement and
cannot be run successfully against the contaminated private history. A local
one-commit clean-export rehearsal did pass the complete gate for all 1,561
current candidate files (tree
`11710623dee0182e1bb525a4f6caa5c6c9dbad62`), proving new untracked audit files
are scanned once staged. That temporary rehearsal is not the final reviewed
revision or private staging-remote fetch and therefore is not closure evidence
for the external credential incident.

The repository scanner's direct full-history run now establishes the complete
value-redacted expected failure: 27 findings, comprising the three credential
copies plus 24 `rho`/`psi`/`rcm` note-opening occurrences in seven removed raw
ingress artifacts. Gitleaks classified its own 719 generic findings separately
because it does not recognize note-opening field names. Sanitization must reduce
both scanner classes to zero; the private-history counts are not allowlisted.

### P0-ASSET-01 — issued assets could be mislabeled as native-backed owned objects

**Status:** fixed-candidate; the immutable owned-asset/FastPay integration,
workspace, replay, migration, and fuzz batteries pass.
**Prior invariant violations:** legacy `wrap_owned` accepted a non-PFT label while debiting native PFT; a reused object ID debited again and created duplicate live state; both unwrap paths could convert an issued owned object into native `Account.balance`; and overflow could consume an input before returning an error.
**Local remediation:** every live constructor is inventoried in `docs/status/OPEN-SOURCE-OWNED-OBJECT-CREATION-INVENTORY-20260716.md`. Native wrap/deposit require exact `PFT` and bind object value to the same native debit; owned transfers preserve the exact input asset and checked conservation; only exact `PFT` may unwrap into native account balance. Duplicate/zero outputs, live output-ID collisions, and destination overflow reject before mutation. The FastPay demo now enters through the production native-backed wrapper rather than directly pushing value. The isolated prototype rejects malformed/duplicate genesis fixtures, duplicate inputs/output IDs, overflow, zero outputs, and duplicate validator IDs.
**Evidence:** execution 144/144; node FastPay safety 11/11 including a correctly dual-signed issued-asset unwrap and eight simultaneous unsigned-wrap rejections at the real store boundary; prototype 9/9 plus its native-backed executable flow; 256-iteration/2,816-case owned-object fuzz with zero invariant failures; affected strict Clippy and formatting PASS.
**Remaining closure:** rerun the full workspace, replay, FastPay payment, and migration compatibility batteries on the immutable candidate and record results in the closure table.

### P0-RPC-01 — public `wrap_owned` can debit an arbitrary account without owner authorization

**Status:** fixed-candidate; the signed replacement and integrated public-candidate
authorization, deterministic execution, replay, and caller-migration gates pass.
**Invariant:** no public request may debit an account or create spendable value without authenticated authorization from that account through a consensus-ordered transition.
**Affected code:**

- `crates/node/src/rpc_cli.rs:3290-3385` unconditionally includes `wrap_owned` in `rpc_serve_method_allowed`, independently of the opt-in mempool mutation flags;
- `crates/node/src/rpc_cli.rs:1533-1579` accepts caller-provided `from_address`, `owner_pubkey_hex`, `amount`, `asset`, and optional object ID, with no signature, sequence, transaction, certificate, administrator authentication, or capability check;
- `crates/node/src/consensus_artifacts.rs:2694-2775` directly reads the node ledger, debits `from_address`, creates an owned object controlled by `owner_pubkey_hex`, and rewrites ledger state;
- the generated object ID uses local `SystemTime` when the caller omits it (`:2731-2756`), so invoking the same mutation independently can also diverge validator state;
- wallet and proxy code intentionally broadcast this unsigned operation to validators, so this is not unreachable dead code.

The existing issued-asset label guard closes P0-ASSET-01 but does not authorize native-PFT debits. A caller can select a victim account as `from_address`, its own FastPay public key as `owner_pubkey_hex`, and transfer the victim's native PFT into an attacker-controlled owned object. Public-bind examples make the handler network-reachable. Even when the client broadcasts to every validator, the mutation bypasses ordered consensus, sequence control, and account authentication.

**Required complete fix:** immediately remove the unsigned method from the public server surface, then replace the account-to-owned transition with a signed, domain-separated, sequence-bound transaction admitted and committed through the canonical consensus lane. The created object ID and debit must be deterministic consensus outputs, not caller/local-clock authority. Historical replay may retain a versioned decoder, but no live compatibility fallback may call the direct mutation. If a local test/admin helper remains, it must be compile-time or explicit local-test-only, loopback-restricted, separately authorized, and impossible to enable in a production build.

**Closure evidence:** a pre-fix real-store regression demonstrating that an unsigned remote request can debit another account; post-fix tests proving the public method is denied by default and no unauthorized request mutates balances or owned objects; signed deposit tests for wrong key/domain/sequence/replay/object ID/amount; deterministic six-node execution and crash/replay tests; wallet/proxy migration to the signed transaction; removal of the unsigned RPC from all public API and FastPay claims.

**Local remediation evidence:** `wrap_owned` now unconditionally returns
`PermissionDenied` before reading or writing the store; both unsigned owned-lane
methods were removed from the remote RPC allowlist and dispatch; the proxy no
longer classifies or normalizes `wrap_owned` as a broadcast mutation; and the
browser RPC client exposes neither unsigned method. The legacy funding controls
were replaced by `OwnedDepositV1`, a canonical, domain-separated operation that
binds chain/genesis/protocol, source address and public key, sequence, fee,
destination owner key, asset, amount, expiry, and nonce before any mutation. It
is signed locally by the wallet, ordered through `FastLanePrimaryOperationV1`,
and succeeds only on the exact `owned_deposit_applied` accepted receipt code.
The transition is clone/apply atomic, derives the object ID deterministically,
burns the declared fee, and rejects wrong key/domain/sequence/asset, replay,
collision, expiry, zero, insufficient funds, and overflow without mutation.
The restored Account-to-FastPay controls refresh both account and owned balances
after finality; no backend receives wallet signing authority. The real transport
tests fund a previously absent wallet, certify its locally signed deposit after
failed-proposer recovery and validator rotation, then prove acceptance, exact
account-plus-owned conservation, deterministic state, and replay rejection on
all four and all six replicas. The node real-store regression separately proves
that no server flag re-enables the removed unsigned method.
The stale Python caller was also removed: its public client no longer exposes
`wrap_owned`/`unwrap_owned`, and its FastPay/WAN setup now validates the live
domain/account, signs `OwnedDepositV1` locally through the SDK, submits the
FastLane transaction, requires `accepted=true` plus exact code
`owned_deposit_applied`, and resolves the exact new object from bounded state
diffs. Evidence:
`reports/open-source-p0-python-fastpay-signed-deposit-20260717T034903Z/ACCEPTANCE.json`,
SHA-256
`ac370c7dc094b49b5c6c9606b81fc4f6cf065acde5f0d3ffd14bd47320bf91d1`.

### P0-BRIDGE-01 — PFTL↔Ethereum imports and refunds trust unverified operator assertions

**Status:** implemented locally; governed checkpoint, production receipt-proof
construction, bidirectional proof, crash-persistence, global-supply, adversarial
matrix, and the accepted-receipt/conservation round trip are green. Immutable-
candidate and deployment gates remain open.
**Invariant:** PFTL value may be credited after a return only if the exact canonical Ethereum representation was irreversibly burned on the configured chain/contract, and source value may be refunded only if no destination mint/consume can ever become valid.
**Affected code:**

- `PftlUniswapDestinationConsumeOperation` at `crates/types/src/transactions_mempool_receipts.rs:2229-2270` carries only an operator, route/packet hash, asserted Ethereum transaction hash, and asserted consumed/finalized heights;
- `PftlUniswapReturnImportOperation` at `:2307-2388` carries asserted burn fields and heights but no Ethereum header, receipt, log inclusion proof, consensus/finality proof, or attestation certificate;
- `apply_pftl_uniswap_destination_consume` at `crates/execution/src/nav_vault_asset_execution.rs:3136-3226` checks operator policy, packet status, and arithmetic over the two caller-provided heights, but never verifies the referenced Ethereum transaction or event;
- `apply_pftl_uniswap_return_import` at `:3229-3355` recomputes a hash from caller-supplied burn fields and compares `finalized_height >= burn_height + configured_blocks`; it never proves that the burn occurred on Ethereum;
- `apply_pftl_uniswap_refund_source` at `:3027-3133` treats a locally recomputable hash of route, packet, and refund height as a “non-consumption proof.” It does not prove absence of a destination mint/consume, so a delayed or withheld destination observation can race a refund.

**Failure scenario to reproduce:** export a real amount to Ethereum and leave the wrapped representation spendable; submit an operator-signed `PftlUniswapReturnImport` containing a syntactically correct fictitious burn event and sufficiently separated asserted heights; observe the PFTL credit and ledger-side Ethereum supply decrement without an Ethereum burn. Separately, withhold destination-consume observation until the refund height, refund the PFTL source, then deliver or exercise the already-authorized Ethereum mint/consume path.

The route's internal accounting remains arithmetically consistent while external reality diverges. That is not conservation: both representations can remain spendable. A controlled devnet operator trust assumption does not make this acceptable for public production claims.

**Required complete fix:** disable all live external consume/import/refund transitions in the public candidate until they verify a chain-ID/contract/code-hash-bound Ethereum header and receipt/log inclusion proof under a governed finality checkpoint or a clearly specified Byzantine attestation committee. Bind event topic, emitter, route, packet/burn nonce, token, amount, sender/recipient, and finalized canonical header. Design refunds as a mutually exclusive state machine with a destination-side expiry/cancel proof that makes any later mint impossible; a mere elapsed PFTL height is insufficient. If a trusted federation remains the selected design, represent it explicitly as a threshold certificate with distinct keys, slashing/rotation/recovery, and an honest custody disclosure; never call a single operator assertion trustless verification.

**Closure evidence:** pre-fix fictitious-burn and consume/refund-race tests; post-fix invalid/forked/wrong-chain/wrong-contract/wrong-topic/wrong-amount/noncanonical/under-finality/replayed proofs reject without mutation; proof of mutual exclusion between destination mint and source refund; cross-chain supply oracle against a local Ethereum fork; reorg, partition, delayed-relay, restart, replay, and migration tests; public claims state the exact trust model.

**Local remediation evidence:** historical records without the new optional
policy/proof fields retain their original signing and state encodings and remain
available only through explicit archive replay. Live route creation now requires
an exact governed Ethereum verification policy resolving to an on-ledger BFT
committee. `EthereumFinalizedCheckpointV1` binds the PFTL domain, route/config,
Ethereum chain, finalized block/hash/receipt root, confirmation depth, authority
epoch/root, controller/token addresses, and both runtime code hashes. Sorted,
distinct ML-DSA-65 votes must meet the committee's exact BFT quorum.

The bounded Ethereum verifier implements canonical RLP, Merkle-Patricia receipt
inclusion, successful-receipt decoding, exact log index, ABI/topic/emitter
binding, and typed `PacketConsumed`, `PacketCancelled`, and `ReturnBurned`
events. Source export now signs and persists the exact EVM packet digest; live
consume/cancel evidence must match that digest and the committed source packet.
The signed export and persisted route record also carry the exact external
packet schema version; live evidence accepts only v1, matching the Ethereum
`postfiat.pftl_uniswap.mint_and_swap_packet.v1` digest domain. The linked state
machine has one pending state and two mutually exclusive terminal states:
destination-consumed or source-refunded.
The EVM replay registry atomically shares source-packet/source-receipt keys
between consume and post-deadline cancellation. PFTL refunds now require a
threshold-certified successful `PacketCancelled` receipt in addition to the
legacy height audit commitment. The real asset-transaction boundary accepts a
valid 3-of-4 checkpoint/receipt consume and cancellation, rejects a separately
certified wrong-amount event with exact pre/post ledger equality, and rejects
consume after a certified refund. Bridge 35/35, execution 139/139, types 83/83,
and affected strict Clippy pass. The reverse direction is implemented as
an explicitly named `BFT_CHECKPOINT` trust class, not a trustless claim:
`ThresholdPFTLReceiptVerifier` immutably binds the PFTL domain, bridge committee
epoch/root and exact BFT threshold, requires sorted distinct ECDSA bridge keys,
and certifies only the exact `accepted` receipt code. The production handoff
controller consumes that durable certificate through `IPFTLReceiptVerifier`;
the integration test proves the certificate is required before mint and a valid
3-of-4 certificate drives the mint exactly once.

Crash recovery is now exercised through the real ordered-commit delta journal.
For every persist prefix from journal-only through ledger, receipt, ordered
batch, archive, block, and tip, restart recovery produces the exact terminal
packet, replay receipt, accepted operation receipt, and tip; removes the journal;
and remains byte-identical on a second restart. The execution boundary separately
serializes and reloads both terminal branches and proves duplicate or late
relayers cannot mutate them.

The global issued-supply oracle now counts public/FastLane custody plus every
route-matched off-ledger bridge bucket: outstanding export claims, pending return
imports, Ethereum-spendable representation, and other registered venues. Every
addition is checked. A real root regression rejects 60 public plus 40 external
under a cap of 99 and accepts the exact cap of 100; primary mint rejects without
mutation when 40 external atoms already exhaust a cap of 40. The live signed
flow proves subscription, export, certified consume, certified return, and the
alternate certified refund branch preserve the same global total, with terminal
replay and cross-terminal attempts rejected after serialization restart.

Each validator checkpoint signer now independently re-reads the exact chain,
head, block/hash, receipts root, and historical controller/token runtime code
from its own bounded Ethereum RPC before any signing intent is persisted. The
per-validator route/epoch/height intent is created durably and without overwrite
before ML-DSA signing; exact replay is idempotent, while a conflicting root is
rejected across restart and in a simultaneous two-candidate race. A candidate
that does not match the validator's own Ethereum view creates neither signing
state nor a vote artifact.

The production receipt-proof builder fetches the canonical transaction receipt,
block, and bounded full block receipt set, reconstructs the Ethereum receipt
trie locally, requires its Keccak root to equal the governed block
`receiptsRoot`, and emits the exact inclusion path. Its independent Anvil 1.7.1
fixture reconstructs the captured three-EIP-1559-receipt root
`25e6b7af647c519a27cc13276a1e6abc46154b51414d174b072698df1f6c19df`.
The integrated production-artifact regression drives signed PFTL subscription
and two exports, constructs each consume/return/cancel proof through that
builder, observes and independently signs the matching governed checkpoint with
three isolated validator keys, assembles exact-quorum certificates, then applies
consume, return, and the mutually exclusive refund alternate. Every money
receipt is `accepted=true`, `code=accepted`; the global and per-route total stays
exactly 50 atoms at every step.

Current-tree verification is green: bridge 36/36, execution 139/139, types
83/83, production checkpoint/signing round trip 2/2, receipt builder 3/3, the
focused node crash-prefix and external-supply regressions, non-fork Foundry
95/95, and strict Clippy for types/bridge/execution/node. The adversarial evidence now covers minority
partition and unvoted reorg rejection; wrong chain/address/code/topic/token/
amount/recipient/nonce and malformed receipt proofs; terminal replay; both
consume-before-refund and refund-before-delayed-consume orderings; concurrent
checkpoint-signing races; and restart recovery. The official mainnet-fork test
remains deliberately fail-closed without its RPC URL. This closes the
P0-BRIDGE-01 implementation checklist locally; immutable-candidate, pinned-fork,
test-environment deployment, and complete global release gates still prevent a
production/publication closure claim.

### P0-SUPPLY-01 — EVM mint release trusts an unauthenticated beneficiary assertion

**Status:** P0-specific remediation complete locally; production BFT verifier,
controlled PFTL-to-Anvil release and continuous mint/return/failure
conservation are proven. Immutable-candidate signer custody and pinned-fork
release gates remain open.
**Invariant:** issued supply must not become transferable unless the backing or
settlement value claimed for that release is cryptographically or directly
on-chain proven under the accepted policy.

`crates/ethereum-contracts/src/MintController.sol:31-38` defines
`SettlementProof` as caller-supplied recipient, two amounts, two booleans and a
`bytes32 proof_hash`. `releaseMint` at `:189-243` is permissionless. It checks
that the recipient equals the escrow beneficiary, that the hash is nonzero and
unused, and that at least one boolean is true. `_proofValueUsdE8` at `:270-280`
simply sums whichever caller-supplied amount has a true boolean. No contract,
signature, Merkle proof, venue receipt, token balance delta, bridge certificate,
authorized attestor, or verifier authenticates those fields.

The backing inequality at `:282-296` therefore operates on attacker-controlled
“settled value.” It is arithmetic over an untrusted premise, not proof of
backing. The accepted market envelope and mint cap limit maximum issuance per
envelope but do not establish that any post-mint value was received or locked.

**Failure scenario to reproduce:** under a currently executable accepted
envelope with a positive mint cap, the beneficiary calls `requestMint`, then
calls `releaseMint` with itself as recipient, `proceeds_settled=true`, an
arbitrarily large `settled_proceeds_usd_e8`, and any new nonzero `proof_hash`.
The escrowed token becomes transferable although no proceeds were settled and
no liquidity was locked. Repeating with unique hashes up to the envelope cap
creates unbacked issued supply.

**Required complete fix:** replace the data-only struct with one exact settlement
authority. Prefer a direct on-chain settlement contract whose immutable address,
asset/token, venue, envelope/pending ID, escrow ID, payer, beneficiary, amounts,
transaction nonce and chain ID are bound into a one-time record that
`MintController` queries and consumes atomically. If external evidence is
unavoidable, require a separately specified threshold certificate or proof
verifier with signer/committee epoch, distinct-signer threshold, domain,
expiry/replay protection, rotation and emergency behavior. A single owner or
beneficiary assertion must never satisfy the proof. `proof_hash` must be derived
by the verifier from canonical evidence, not selected by the caller.

If that protocol cannot be completed before publication, remove/disable
`requestMint` and `releaseMint` in the public candidate and withdraw every claim
that above-NAV issuance is backed by verified settlement.

**Closure evidence:** pre-fix Forge exploit demonstrates transfer of escrowed
tokens without token/value movement; post-fix forged boolean/amount/hash,
wrong-chain, wrong-controller, wrong-token, wrong-envelope, wrong-escrow,
wrong-beneficiary, replay, duplicate, expired, partial and front-run attempts all
reject without mutation; a real settlement releases exactly once; fuzzed
cross-contract conservation proves issued/released supply never exceeds
cryptographically accepted backing under rounding and maximum values.

**Local remediation evidence:** `MintController` now requires an exact-code-hash,
nonzero `IMintSettlementVerifier`. Release rechecks the verifier runtime hash and
queries it with the exact
pending ID, escrow ID, beneficiary, amount, and proof hash; zero/unrecorded
evidence fails, caller amounts/booleans must byte-for-byte match verifier output,
and the proof hash remains one-use.

`ThresholdMintSettlementVerifier` is the concrete production-candidate
implementation. Its exact BFT quorum of sorted distinct low-`s` secp256k1
signatures binds EVM chain/verifier, PFTL chain/genesis/protocol/authority epoch,
committee root, controller/token, pending and escrow IDs, recipient, amount,
settled/locked value, finalized height/state root, accepted receipt hash,
route-config digest and exact `accepted` receipt code. The verifier derives the
one-use settlement ID; the beneficiary cannot select it. Cross-verifier,
cross-controller, wrong-amount, under-quorum, duplicate-signer, rejected-receipt
and tampered-certificate paths fail without releasing the escrow.

Verifier rotation requires an exact expected runtime code hash, a fixed two-day
on-chain delay, and zero unresolved mint escrows both when scheduled and when
activated. A new mint during the delay blocks activation until it resolves. The
one-time initializer cannot replace the live verifier, and the active code hash
is checked on every release. The explicit federated trust and classical ECDSA
boundary are documented in `docs/security/mint-settlement-verifier.md`; no
trustless-finality claim is made.

The pre-fix beneficiary fabrication test was inverted. Current focused evidence
is MintController 11/11, threshold verifier 5/5 including a 256-run wrong-amount
fuzz boundary, a 256-run/128,000-call stateful invariant, MarketOps adversarial
10/10, and complete non-fork Foundry 102/102. A pinned-mainnet-fork settlement
certificate test is compiled into the fail-closed official fork suite. Isolated
production signer custody and the secret-backed pinned fork remain open.

The controlled deployment gate now runs against two actual processes rather
than an in-memory verifier mock. An isolated PFTL node commits an accepted
110-atom backing transfer at height 1 and records its real state root and
canonical receipt hash. Isolated Anvil deploys the production `MintController`
and production `ThresholdMintSettlementVerifier`, pins their runtime code
hashes, and accepts exactly three signatures from a sorted four-member test
committee over that height/root/receipt and exact escrow/recipient/amount. The
escrow releases once; certified backing, released supply and beneficiary
balance are each 110 atoms, with zero controller escrow and zero unresolved
obligations. Release before certification, certificate replay and release
replay all reject. Evidence SHA-256 is
`b9cf666416126a81c3ecf18bd9686e485e84c7db055fce48c834f7a0311f66fa`.
The aggregate gate is independently closed by one continuous governed-route
test over the same asset, isolated PFTL state and live Anvil vault. A
wrong-amount claim first rejects with
`vault_bridge_deposit_amount_mismatch`; the conservation oracle remains green
with the full source amount classified as an uncredited deposit. The valid
claim then creates exactly 1,000,000 PFTL atoms against 1,000,000 vault atoms;
return burn moves that amount into the burned-unsettled bucket; source release
moves it into released-unsettled; and final PFTL settlement clears every bucket.
The exact `V = S + D + B - R` equation has zero unexplained delta at all five
checkpoints. All 11 PFTL receipts are accepted and all eight EVM receipts have
status `0x1`. Evidence SHA-256 is
`0a18b1a9ab808fe74c2f70df0c4de1e2866a70990758af0a94b41096e400f8e9`.
This completes the P0-specific burn-down; frozen-candidate, signer-custody and
secret-backed pinned-fork gates remain part of the global release closure.

### P0-NATIVE-SUPPLY-01 — height-zero native supply was not genesis-bound

**Status:** fixed-candidate; immutable genesis-to-tip, conservation, prune,
snapshot, restore, replay, and adversarial supply gates pass.
**Invariant:** the native supply selected at genesis is immutable under the same
chain identity, and every later decrease is an explicit fee burn rather than an
untracked rewrite or issuance.

The persisted replay base uses `faucet_account.json` to reconstruct height-zero
ledger state. Its validator required only a nonempty address, nonzero balance,
sequence zero, and a valid public key. `Genesis` did not contain the native
supply. An operator able to rewrite local state could therefore decrement the
faucet balance in both `faucet_account.json` and `ledger.json`; because replay
started from the rewritten faucet file, `verify-blocks` accepted the
coordinated rewrite at height zero and returned `verified=true`. The genesis
hash did not change.

The real node regression performs exactly that coordinated rewrite and captured
the false success before remediation. New genesis files now contain
`native_supply_atoms=1000000000`, so the value is committed by the canonical
genesis hash. Genesis validation rejects any other explicit value. Historical
genesis JSON without the field remains parseable and retains its byte-identical
old hash, but replay constrains its faucet record to the same protocol constant;
there is no legacy arbitrary-supply escape. The faucet replay-base validator
also requires the exact amount rather than merely a nonzero balance.

The exploit regression and the existing one-file tamper regression pass.
Canonical vectors pin the new supply-bound genesis hash
`16843cd2…27d889f`, the prior state-v2 hash `f340b4b1…6026b83f`, and the older
legacy hash `97982d73…1aa04a`.

The post-genesis oracle now runs inside canonical block replay and history
checkpoint construction. It sums each live PFT custody lane exactly once:
account balances, open native escrows, open-offer native sell balances, offer
reserves, owned native objects, FastLane native reserves, and live Orchard
turnstile value. For every replayed block it requires
`live_before - sum(receipt.fee_burned) == live_after`; overflow, unreported
destruction, or issuance fails verification. Real native escrow, offer,
Orchard-deposit, registry-replay, and history-prune paths pass this check.

That trace exposed a second concrete defect: an accepted native FastLane
deposit debited `fee_pft` but its canonical receipt reported zero charged and
burned fee. The regression failed `left: 0, right: 2` before remediation. The
shipping primary executor now propagates the exact deposit burn, and checkpoint
receipts propagate any native pending-fee burn applied to the reserve. Execution
passes `135/135`.

History checkpoints are now schema v2, hash-domain separated, and commit the
cumulative native fee-burn total. Validation requires checkpoint live custody
plus that explicit burn total to equal the genesis supply. Legacy v1
checkpoints, which cannot prove cumulative burns, fail closed and must be
rebuilt rather than silently trusted. The shipping offline
`history-checkpoint-rebuild-from-archive` operation ignores all v1 economic
state, verifies contiguous imported archive windows, replays from genesis,
verifies the prefix and retained suffix in isolated shadow stores, writes an
exact backup, and only then atomically installs v2. A tampered archive leaves
the v1 checkpoint byte-identical; a deliberately inflated v1 balance is ignored
and reconstructed correctly. The exhaustive custody/transition inventory is
`docs/status/OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md`.

The native-supply adversarial harness passes 256 iterations / 2,304 cases over
all custody lanes, maximum arithmetic, every duplicate key class, unknown
issued-lane exclusion, and impossible Orchard accounting. Real prune,
post-prune append, archive rebuild, and snapshot round-trip regressions pass;
the snapshot test compares the exact all-lane live total before and after
restore and reruns block verification. On candidate `012d3a8b`, the real oracle,
coordinated genesis rewrite rejection, both prune/recovery paths, and signed
snapshot restore/replay were rerun successfully. Affected strict Clippy and
formatting pass. The immutable-candidate native-supply item is closed.

### P0-ISSUED-SUPPLY-02 — issued-asset caps omitted private and FastLane custody

**Status:** fixed candidate; compiler inventory, adversarial fuzz, concurrency,
replay, and complete four-lane customer-flow boundaries green.
**Invariant:** moving issued assets between supported public, FastLane, and
AssetOrchard custody cannot create mint headroom or allow aggregate live supply
to exceed the asset definition's `max_supply`.

`issued_asset_supply` counted trustline balances, open issued escrows, and open
offer sell balances. It did not count `LedgerState.fast_lane_reserves` or
`OrchardPoolState.asset_orchard_balances[].live_total`. Both ingress paths debit
the source trustline before crediting their custody lane. An issuer could
therefore issue to the cap, move value into FastLane or AssetOrchard, and issue
again because the mint validator observed only the reduced transparent total.

The real replicated-state regression constructs an asset with `max_supply=10`,
ten atoms on a public trustline, and one live AssetOrchard atom. Before
remediation `replicated_state_root` returned a valid root
`abde05b9…c6cf9ee`; the test required fail-closed rejection and failed. The
execution-boundary companion places all ten atoms in a FastLane reserve and
attempts one further issuer payment.

`issued_asset_supply` now includes matching issued FastLane reserves and all
registered external-route custody with checked arithmetic. The global node
oracle adds live AssetOrchard balances and uses compile-exhaustive
`LedgerState`/`ShieldedState` destructures. It rejects duplicate definitions or
custody keys, unknown asset references, issued assets in native-only owned
objects, per-lane overflow, and aggregate overflow. The same oracle gates state
commitment, mint admission, status reporting, and the v2 reserve replay bundle.
The bundle now serializes FastLane, PFTL-Uniswap route, and AssetOrchard rows;
tampering FastLane or Orchard custody fails replay rather than under-reporting
supply. A finalized NAV circulating-supply value remains an additional ceiling.

The complete custody and operation map is
`OPEN-SOURCE-ISSUED-SUPPLY-INVENTORY-20260716.md`. It covers issuer payments,
holder burns, clawback, NAV mint/redeem, vault bridge receipt mint/redemption,
every supply-neutral custody move, and registered PFTL/Ethereum representation.

**Evidence:** pre-fix state-root regression FAIL `1/1`; post-fix PASS `1/1`;
FastLane reserve mint-cap and no-mutation regression PASS `1/1`; real issued
AssetOrchard ingress/disclosed-egress/snapshot/replay round trip PASS `1/1`.
The production-boundary fuzz harness passes 256 iterations / 4,352 cases with
zero invariant failures. Targeted mint, burn, clawback, escrow, offer, private
custody, vault replay, and strict-Clippy gates pass.

The immutable-candidate customer-flow gate is closed by a composed production-
boundary regression with one issued asset simultaneously split 30/20/25/25
across transparent, FastLane, AssetOrchard, and registered external custody at
a 100-atom cap. A signed one-atom mint is accepted by the narrower execution
dry-run that sees 75 atoms, then rejected by node global admission when the 25
private atoms are included; canonical state remains byte-identical. Supply
stays exactly 100 through each neutral lane shift. The real release FastLane
deposit/cap, BFT-checkpoint subscribe/export/refund, and two-fresh-wallet
encrypted K15 ingress/swap/private-egress tests also pass. Evidence:
`reports/open-source-p0-issued-customer-flow-20260717T064800Z/ACCEPTANCE.json`
(SHA-256
`993d961ada1d113a9bf91f74a17e430c87b8795439cf561ef310a23820b88d6a`).

### P0-COMMIT-ATOMICITY-01 — ordered commits admitted concurrent double application

**Status:** fixed candidate across all ordered batch kinds; full crash matrix
and workspace battery pass.
**Invariant:** one ordered parent/state transition can be consumed by at most
one local commit writer, and no caller may report a stale concurrent mutation
as accepted.

The existing `NodeStore::lock_ordered_commit` protected only journal write and
file replacement. Transparent, shielded, bridge, and governance apply paths
read their parent and state, executed, and prepared a commit before acquiring
that lock. A barrier-synchronized real-store regression submitted the same
AssetOrchard disclosed-egress batch from eight threads. Before remediation all
eight calls returned `accepted` for the same nullifier.

Each ordered apply path now acquires the cross-process mutation lock before
journal recovery and any consensus-state read, and holds it through execution,
commit construction, journal persistence, and final file replacement. Locked
recovery and writer helpers require a borrowed `StorageMutationLock`, making
the critical-section obligation visible at compile time. The generic unlocked
writer entry points were removed rather than retained as a bypass.

**Evidence:** the unchanged race stimulus now yields exactly one accepted
egress and seven `AlreadyExists` idempotency failures. Global issued supply is
40 before ingress, after ingress, after the winning egress, and after snapshot
restore; block replay passes. The affected node all-target check, formatting,
and strict-Clippy gates pass. The immutable candidate's complete node/workspace
run also passed the transparent atomic-swap 0..10 prefix matrix, replicated-
activation 0..9 prefix matrices, bridge 0..6 prefix matrix, shielded concurrent
egress, generic ordered-journal recovery, FastPay rollback 0..9 prefix matrix,
snapshot, replay and supply gates.

### P0-STATE-01 — replicated state root omitted FastLane/FastSwap ledger state

**Status:** fixed candidate; isolated six-node rollout/activation/rollback and
the cap-valid current-devnet reset plus signed exact-six shadow replay pass.

`LedgerState` contains ten FastLane/FastSwap domains after `owned_objects`:
reserves, deposit receipts, redeemed exit claims, asset rules, holder permits,
policy snapshots, committees, prepare fences, checkpoint anchors, and the
activation height. The production `append_ledger_state` commitment stopped at
`owned_objects`. Consequently, changing any of those economically or
authorization-relevant fields produced the same replicated root.

**Real-boundary reproduction:**
`replicated_state_root_commits_every_fastlane_ledger_field` created otherwise
identical states and added one reserve balance. Before remediation both roots
were exactly
`0f0e8e7e3ac76f5e08f805cc56b8a46c10f02be5702b0a306db9bc09c969304f9ae13769aec672a85a77ee153fa90ee2`;
the assertion failed with `state root omitted reserves`. The completed
regression repeats the inequality check independently for all ten fields.

**Local remediation:** when any FastLane field is present, the state root now
commits a presence marker, a count and a SHA3-384 commitment over an explicit
length-delimited canonical binary encoding for every sorted record in all nine
collections, plus exact optional
activation-height presence/value. Reversing reserve storage order preserves the
root; changing an amount changes it. Serialization failure is returned, never
collapsed to an empty/default commitment. An exhaustive `LedgerState`
destructure makes any future field addition fail compilation until the
commitment inventory is updated. Equivalent exhaustive destructures guard
`Genesis`, `GovernanceState`, `ShieldedState`, and `BridgeState`; every
top-level replicated-state encoder calls its boundary, so adding a field to any
committed domain is a compile error until its disposition is explicit.

The remediation is versioned rather than retroactive. New genesis files carry
`replicated_state_v2_activation_height: 0`. Legacy genesis JSON omits the field
and preserves its original hash and pre-v2 roots. An existing chain can commit
an irreversible future-height `replicated_state_v2_activation_height`
amendment; the earliest committed value wins, and same-block/backdated values
fail before ordering. Old and new binaries therefore agree before the
transition and complete FastLane commitment begins exactly at the scheduled
ordered-batch height. The runbook now includes an old/candidate binary by
pre/post-activation compatibility matrix, fail-closed mixed-version behavior,
signed v6 snapshot migration, a coordinated pre-activation rollback procedure,
and forward-only post-activation recovery. It forbids an uncoordinated restart
and a post-activation rollback to an old binary.

**Evidence:**

- pre-fix targeted test: FAIL with identical reserve roots;
- post-fix targeted test: PASS, including all ten fields, permutation
  invariance, amount sensitivity, and a frozen canonical reserve-root vector;
- legacy genesis JSON that omits the activation field retains its exact
  historical genesis hash, while newly generated genesis intentionally has a
  new domain hash because it activates v2 at height zero; both frozen vectors
  and `replicated_state_root_commits_to_chain_domain` pass;
- the real governance-bootstrap path commits nonempty FastSwap committee,
  asset-rule and activation state, then `verify_state` and `verify_blocks`
  both pass against the persisted block/receipt history;
- historical registry rotation, catch-up certificate, and atomic-swap archive
  replay regressions remain green. No permissive legacy FastLane-root fallback
  was added: empty historical FastLane state retains its exact old encoding,
  while nonempty state is committed at and after the explicit activation;
- new-genesis height-zero, legacy-genesis omission, governed future-height,
  exact-boundary, irreversibility and backdated-rejection regressions pass;
- `replicated_state_v2_activation_journal_recovers_every_persist_prefix`
  constructs the real governed scheduling commit and first v2 block through
  the production ordered-commit journal. It crashes after every ordered write
  prefix for both commits, recovers idempotently, removes the WAL, matches exact
  ledger/governance/receipt/order/archive/block/tip state, and passes both
  `verify_state` and `verify_blocks` across the activation boundary;
- `cargo clippy -p postfiat-node --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` pass; the new focused test module is 361 lines
  and the repository remains below the 5,000-line Rust-file ceiling;
- an isolated six-node chain passed a one-node-at-a-time compatible rolling
  upgrade, a six-signature governance schedule at height 1, exact activation
  at height 2, exact-six roots and accepted receipts, pre-activation snapshot
  rollback into fresh directories, and exact forward recovery by replaying the
  same activation batch. `verify-state` and `verify-blocks` passed on all six
  live, all six rollback, and all six recovery stores. Evidence:
  `reports/open-source-p0-state-six-node-20260716T184400Z/README.md`;
- the key-free height-1220 current-devnet copy reproduced the old root but the
  complete issued-supply oracle correctly refused it: 291,978,179 transparent
  pfUSDC + 9 FastLane + 798,070,376 AssetOrchard = 1,090,048,564, exceeding
  `max_supply=1,000,000,000` by 90,048,564. No oracle bypass was added. The
  founder-authorized batched reset replaced that invalid legacy state with the
  reproducible candidate genesis rather than rewriting balances in place;
- release `open-source-candidate-249149bc` was independently clean-built twice
  to byte-identical binary SHA-256
  `c379bfca23d4ed43097e7f0386848ce755d1d7f4844b50eeb9202c12eb86358d`
  and rolling-staged behind exact service-user manifest/key/state/block
  preflights. All six validators started the same signed release over the
  private WireGuard mesh with loopback-only RPC;
- the first ordered block crossed consensus-v2 activation at height 1 with
  five legacy votes, a five-vote prepare QC, six-vote precommit QC, and one
  accepted receipt on all six. Every node converged on block
  `28381e2483…aa5d` and root `4475e0cc…7979`; native conservation is exact at
  999,999,978 live + 22 burned = 1,000,000,000 and issued supply is zero;
- a signed v6 snapshot of that live state contains no signer material and was
  imported into six independent shadow stores. `verify-state` and
  `verify-blocks` pass 6/6, reproducing the exact height-1 tip and root.
  Evidence:
  `reports/open-source-public-candidate-20260717T052600Z/reset/open-source-candidate-reset-20260717T061500Z/ACCEPTANCE.json`
  (SHA-256
  `2269e611a93a2715c2746859c63e91eb577d79a8fbad5bedec029a6eb7083d73`).
  The current-devnet supply-migration/shadow gate is closed without weakening
  the cap oracle.

## 5. Confirmed P1 blocker register

### P1-CERT-DOMAIN-01 — live certificate verification accepted an empty legacy registry root

**Status:** fixed locally.

The block certificate verifier cryptographically checked every vote against the
current fixed registry, so the empty root did not reduce the distinct-validator
threshold. However, live external proposal certificates, preverified
certificates, and timeout certificates all called
`certificate_registry_root_or_legacy`; a certificate could therefore select the
legacy signing preimage with no registry-root binding. That contradicted the
public certificate-domain contract and would become unsafe if registry
reconfiguration were later re-enabled without simultaneously closing this path.

The candidate now requires the exact nonempty current registry root on every
live external/preverified/timeout boundary. Only verification of an already
committed historical `BlockRecord` retains the legacy helper. The real
`apply_batch` regression strips the root from an otherwise valid external
proposal certificate, requires the typed `registry root is required` rejection,
and proves height remains zero. The valid rooted certificate then commits and
the timeout reconstruction regression remains green.

### P1-ARITH-01 — exhausted non-swap account sequences panic or wrap

**Status:** fixed locally; cross-domain arithmetic and rounding inventory complete.

Every normal signed transaction entrypoint other than the already-hardened
atomic swap calculated `account.sequence + 1` with unchecked `u64` arithmetic.
At an exhausted persisted sequence this panicked in checked/debug builds and
wrapped to zero in optimized builds, making receipt behavior build-profile
dependent. The node's local signed-transfer builder had the same panic before
the transaction reached execution. A real execution-boundary regression was
written first and captured `attempt to add with overflow`; a node builder
regression independently captured the same panic.

Transfer, PaymentV2, asset, escrow, NFT, and offer execution now use
`checked_add(1)`, return the deterministic `sequence_overflow` rejected receipt,
and assign the already-validated expected sequence during mutation. The local
builder returns typed `InvalidData` before signing. Height-window additions in
the NAV/vault path now use deterministic saturating deadlines instead of
debug-panic/release-wrap arithmetic. Continued compiler-assisted review then
found two more externally supplied height boundaries: SDK validation of an
adjacent `blocks` response computed `previous_height + 1`, and the FastSwap
governance-bootstrap generator computed `tip.height + 1`. Separate regressions
captured both debug-build panics at `u64::MAX`; both now use `checked_add` and
return typed, fail-closed errors. The RPC SDK suite passes `55/55`, the
bootstrap boundary regression passes, the single-threaded execution suite
passes `136/136`, and workspace check plus strict Clippy pass. The broader
Phase 6 pass is complete in
`OPEN-SOURCE-ARITHMETIC-ROUNDING-INVENTORY-20260716.md`. It traces every
production diagnostic to checked arithmetic, an exact dominating guard, a
bounded index/size domain, or intentional finite-field arithmetic. That pass
also found and closed `P0-ISSUED-SUPPLY-02`; it was not treated as a harmless
lint warning.

### P1-MEMPOOL-01 — mixed-family admission simulated a different execution order

**Status:** fixed locally.

Normal mempool admission replayed every existing transaction family and only
then executed the candidate. Proposal construction and block execution instead
use the canonical family order: transfer, PaymentV2, asset, atomic swap,
FastLane primary, escrow, NFT, and offer. Several admission copies also omitted
atomic/FastLane state entirely. A real node regression first admitted an asset
transaction at sender sequence 1, then demonstrated that a transfer at sequence
2 was reported admitted even though canonical proposal order executes the
transfer first and rejects it. The mempool mutated on that false success.

Admission now inserts each candidate at its canonical family boundary and
executes the same earlier/later family prefix used by batch construction.
Atomic and FastLane pending state is included where active. FastLane and NFT use
bounded canonical prefixes rather than replaying later families before the
candidate. This preserves the liveness invariant that an atomic swap which is
independently stale against committed state is skipped during admission and
evicted during proposal construction rather than wedging unrelated traffic.
The simulator first replays the existing non-atomic canonical prefix without
the candidate: it skips an atomic/FastLane entry only when that entry already
fails there, while an entry valid without the candidate still rejects a
candidate that creates the conflict. The pre-fix false-admission regression is
green with no mempool mutation, and the later integrated stale-atomic
regression is green without weakening the valid pending-atomic conflict rule.
All 15 atomic-swap consensus tests pass, including both canonical orderings,
stale eviction, paused atomic, fee, batch/replay, and exact-parent boundaries.

### P1-STORAGE-01 — JSON/JSONL store has unbounded whole-state and whole-history work

**Status:** feature-contained locally for public source; production engine remains open.

`crates/storage/src/lib.rs:17-33` stores consensus domains across individual JSON/JSONL files. Ledger/governance/shielded/bridge/mempool reads deserialize entire files. `append_receipt` at `:153-156` reads and rewrites all receipts. JSONL readers merge append history into full vectors. Similar behavior exists for blocks, archives, and ordered batches. This makes latency, memory, startup, recovery, and denial-of-service cost grow with history/state size.

**Required closure:** introduce a transactional indexed storage engine or a rigorously bounded segment/snapshot design; define schema/version migration and rollback; preserve deterministic state roots independent of storage ordering; add crash injection, corruption repair, backup/restore, pruning, and production-scale growth tests.

**Local containment evidence:** every primary JSON read now checks a 256 MiB
metadata cap before allocation. JSONL is streamed through `BufReader` with a
512 MiB file cap, 16 MiB record cap, and one-million-record cap; appends reject
oversized records and projected files before mutation. Receipt and ordered-batch
appends no longer read and rewrite the full history. Storage regressions prove an
oversized sparse file and oversized append fail before allocation/mutation;
`cargo test -p postfiat-storage` passes 24/24. A real concurrent-write
regression also reproduced a storage-integrity defect: 24 simultaneous
`append_mempool_entry` calls all returned success while only one entry remained.
All whole-file mempool writers and every family-specific read-modify-write
append now share a blocking cross-process `flock` held from read through synced
atomic replacement; the same test preserves 24/24 successful writes. These
The ordered-commit writer and startup recovery path also acquire a separate
cross-process lock before touching the singleton journal or any committed
domain; an independent-handle regression proves the second actor cannot enter
until the first releases it. These changes bound OOM/work
amplification. The production ordered-commit delta journal regression now
injects restart at all 11 durable prefixes across ledger, governance, shielded,
bridge, receipt, ordered-batch, archive, block, chain-tip and validator-registry
writes, then requires exact state, no duplicate history, journal removal and a
green `verify_state`. This closes the known ordered-commit crash-boundary gap,
but does not turn the JSON store into the indexed transactional production
engine specified above. Core long-running mutation surfaces (`rpc-serve`, both
validator listeners, the node run loop, and certified batch/private-egress
loops) now fail before binding or processing unless the operator supplies the
exact `--unsafe-devnet-json-storage` acknowledgement. Generated devnet units
and controlled smoke callers carry it explicitly; README and SECURITY name the
limitation. This makes accidental production operation fail closed and closes
the public-source P1 by feature containment, while real-value production stays
unsupported until the engine and acceptance gates above exist.

### P1-ORDERING-01 — production inclusion and censorship accountability do not implement the whitepaper

**Status:** fixed locally by claim correction; stronger protocol remains research-only.

`docs/whitepaper.md:87-101` claims canonical fee-class/admission-bucket/transaction-hash inclusion, threshold admission-receipt aggregates, omission evidence, and capped quorum-certified availability suspension. The relevant structures and algorithms exist in `crates/ordering_fast`, but production `crates/node` imports only quorum, leader, timeout, and reference helpers from that crate. `create_mempool_batch` at `crates/node/src/mempool_proposals.rs:2086-2350` drains separate family vectors in fixed family/insertion order and does not sort included transactions by the claimed tuple. Production call sites for `AdmissionReceiptAggregate` and omission evidence were not found. Registry `suspend` exists as a governance update, but the claimed missed-round suspension certificate/auto-expiry/cap mechanism was not found in the production path.

**Required closure:** either integrate the exact ordering, signed-receipt, omission, and suspension protocol with full domain/threshold/replay/state-root/fault tests, or rewrite the whitepaper and security policy to the actual proposer/inclusion and registry-suspension semantics. Deterministic family order is not evidence for the stronger MEV/censorship claim.

**Local remediation evidence:** `docs/whitepaper.md` now specifies the actual
fixed transaction-family order and per-family insertion order. Admission
aggregate, omission attribution, automatic availability suspension, and
Negative-UNL recovery are explicitly labeled research rather than production
behavior. `SECURITY.md` describes only the current direct-certificate path.

### P1-KEYS-01 — validator signing keys are plaintext software files

**Status:** contained locally by making production operation explicitly unsupported.

Validator private keys are stored in local JSON files protected mainly by filesystem permissions. This is acceptable for a controlled devnet but not for a public production-security claim.

**Required closure:** remote-signer/HSM or encrypted-keystore interface, key-purpose separation, non-exportability policy, rotation and recovery, audit logging, operator ceremony, and tests for unavailable/refusing/wrong-domain signers. If not implemented, production operation must be explicitly unsupported and the repository labeled research/devnet.

**Local remediation evidence:** both long-running validator transport services
now refuse the plaintext file signer unless the operator supplies the exact
`--unsafe-devnet-file-signer` acknowledgement. Generated controlled-devnet and
example units state that acknowledgement; README and SECURITY label the model
controlled pre-testnet only. The regression
`validator_service_requires_explicit_plaintext_file_signer_acknowledgement`
passes. This is containment, not an HSM implementation.

### P1-NET-01 — production examples expose plaintext unauthenticated RPC/transport

**Status:** fixed locally.

Systemd examples opt into public transport binding and `0.0.0.0`, with extremely high connection/request limits. The runtime contains an explicit public-bind guard, but the shipped examples bypass it and provide no TLS/mTLS/auth boundary.

**Required closure:** safe loopback defaults; separate validator/private/admin/public listeners; TLS/mTLS or an explicitly required authenticated reverse proxy; bounded rate/concurrency/body/proof limits; no billion-request examples; DDoS and backpressure tests.

**Local remediation evidence:** public/wildcard plaintext transport is rejected
even when the removed legacy override environment variable is present. Generated
RPC units bind loopback; validator transport binds only the topology's validated
loopback/private IP; both use 10,000-request/connection ceilings rather than
billion-scale examples. Deployment staging rejects non-private topology hosts.
The runtime bind-guard and generated-unit regressions pass, and the operator
policy requires an authenticated TLS edge outside the validator process.

### P1-RPC-ERROR-01 — remote RPC errors disclose operator filesystem paths

**Status:** fixed locally.

The remote response helper returned internal storage and worker error strings
verbatim. A real response-boundary regression demonstrated a public
`rpc_internal` response containing
`/home/operator/private-validator/ledger.json`. The same path existed for
status/cache failures, child-worker failures and FastSwap initialization, and
path-bearing errors from other method handlers could reach the generic response
helper.

**Required closure:** keep precise typed transaction/market/finality rejection
codes and safe messages, but replace internal errors with stable public text and
fail closed on any path-bearing message. Operator-only diagnostics may retain
detail in protected local logs; remote responses may not expose host paths,
data layout or private filenames.

**Local remediation evidence:** the response boundary now maps internal,
worker, timeout, read, status, mempool-status and FastSwap-unavailable classes to
stable public messages and replaces any other absolute-path-bearing message with
`request failed`. Typed protocol messages remain unchanged. The pre-fix
`rpc_serve_internal_failures_do_not_expose_operator_paths` regression failed on
the exact path; it now passes, the RPC request suite passes 18/18, and workspace
check plus strict Clippy pass.

### P1-FASTPAY-01 — abandoned owned-object locks have no complete cancellation protocol

**Status:** fixed-candidate; recovery, WAN quorum-finality, completed-response
replay, crash/reconfiguration, and immutable-candidate gates pass.

The current `owned_safe_unlock` is intentionally fail-closed to prevent the late-certificate-after-unlock double-spend race. That preserves safety but lets an abandoned or griefed lock remain indefinitely, creating a permanent liveness/availability failure for the owned object.

**Required closure:** a durable cancellation/drain protocol that prevents any old vote/certificate becoming applicable after unlock, including partitions and delayed messages; model it before implementation; test late certificates, partial lock sets, Byzantine withholding, restart, reconfiguration, and replay. This must be implemented without replacing the core FastPay capability with permanent default-disablement.

**Local remediation evidence:** normal `rpc-serve` exposes signed `owned_sign`,
`owned_apply`, `owned_unwrap_sign`, and `owned_unwrap_apply`; the exact
`--disable-owned-lane` option is retained for emergency operation. The real-process
`fastpay_default_rpc` regression proves default availability and explicit disable.
FastPay owner authorizations and validator votes now use a v2 domain binding the
chain ID, genesis hash, protocol version, and active validator-registry ID. Execution
rejects a foreign domain before state mutation, the wallet refuses to sign for a
foreign wallet chain, and browser clients fail closed when the server omits the
domain. Evidence: execution 137/137, RPC SDK 56/56, wallet-web 220/220, seven node
FastPay safety tests, the process-boundary default RPC test, and strict Clippy for
all affected crates pass. The bounded recovery protocol is now specified and
modeled. The first `q-f` partial-vote design produced an `n=4` Byzantine-only
intersection counterexample and was rejected. The selected model confirms only
from a complete normal `n-f` certificate, requires `n-f` durable apply
acknowledgements for wallet finality, and otherwise orders cancel plus atomic
object-version advance. Twelve adversarial model tests cover `n=4`/`n=6`,
partial locks, delayed messages, withheld brokers, restart, certificate
retrieval, reconfiguration, replay, expiry and a persist-before-apply crash
boundary. Evidence:
`reports/open-source-p1-fastpay-recovery-model-20260717T001735Z/ACCEPTANCE.json`.
Production v3 owner/vote/certificate types, ordered reveal/cancel decisions,
atomic effect/fence persistence, signed apply acknowledgements, certificate
retrieval, wallet recovery UX and six-node catch-up are now implemented. Direct
effects are bound into the next certified block. A real six-validator regression
also proves that a sub-quorum effect omitted by the other five is rolled back
without stranding the minority validator, while its full certificate remains in
the bounded durable recovery journal. Snapshot v6 preserves both the lock state
and recovery journal; legacy snapshots containing activated FastPay state
without them fail closed. The same real six-validator omission boundary now
uses an unwrap certificate, proving the speculative account credit is reversed,
the exact owned input is restored, and the certificate remains recoverable.
Focused evidence is green: model 12/12, execution recovery 7/7, node FastPay
14/14, snapshot 16/16, wallet-web 240/240,
wallet-proxy 23/23, affected-crate check and strict Clippy. The legacy
`owned_safe_unlock` deletion endpoint remains fail-closed because the supported
replacement is the ordered v3 recovery workflow, not unsafe local deletion.
Production recovery evidence:
`reports/open-source-p1-fastpay-production-recovery-20260717T031617Z/ACCEPTANCE.json`,
SHA-256
`50bffc346fa91c728d140de7031e9d7a2de138110ca1a6d6a0a07fbce4e58195`.
The signed governance carrier now stages a future next committee without
deleting the historical epoch: exact epoch/window continuity and unchanged
policy/domain are enforced, overlap rejects atomically, and old-epoch ordered
recovery remains live after rotation. Evidence:
`reports/open-source-p1-fastpay-committee-rotation-20260717T033036Z/ACCEPTANCE.json`,
SHA-256
`93f7b7bbabfc62119891a3b04b060678ff133b31cc2968ee37a2a2fbef965ff5`.

The real six-node WAN run at client/proxy commit `1e9352c6` now satisfies the
governing product-finality rule rather than the unsafe one-ack shortcut. Five of
five payments returned five distinct signed durable apply acknowledgements,
with p50 2,489.978 ms and p95 3,724.984 ms; a subsequent untimed audit found all
six replicas identical at height 33 with empty mempools and the exact same
destination object set. Evidence:
`reports/open-source-p1-fastpay-wan-20260717T-quorum-ack-1e9352c6-five-payment/`.
The approximately 1.1-second one-ack measurement is rejected as premature
success and is not closure evidence.

Commit `77e4a3c7` closes the remaining response-loss boundary without adding a
second store. The existing outbox migrates from v1 to v2 and moves completed
certificates into a record bounded by 1,024 entries, seven days and 16 MiB. The
record binds the exact method/certificate operation digest, terminal digest and
signed apply acknowledgements; the proxy persists the terminal result before
returning it and replays that exact result without another validator call.
Pre-terminal crash refusal, restart, deterministic count/TTL compaction, v1
migration, conflicting replay, and tamper rejection pass. The real six-validator
proxy regression loses/replays the result after exact-six completion and proves
the per-validator apply counters do not change. Commit `90c3836a` also replaces
legacy external-process assumptions with one bounded loopback fixture, making
the full clean-checkout proxy suite pass `24/24`; npm audit reports zero findings.
The persistence matrix now covers lock WAL restart/torn tail, inverse-journal
before ledger, effect/fence before acknowledgement, every ordered rollback
write prefix, snapshot, and replay. The complete adversarial matrix is recorded
at
`reports/open-source-p1-fastpay-crash-matrix-20260717T033709Z/ACCEPTANCE.json`,
SHA-256
`0602315444ae44fa0516be3b65ad9be978766b17f2207127248971a8bfc671e8`.
The WAN Python runner's remaining v2 send/unwrap path has also been migrated:
it reads an uncached governed recovery capability for each operation, signs the
exact v3 lock locally, collects distinct votes, and verifies a governed quorum
of signed durable-apply acknowledgements against the certificate digest. The
Rust verifier rejects wrong domain/epoch/lock/certificate/terminal state,
duplicate validator IDs, bad signatures, and under-quorum results. Python flow
tests pass `76/76`; RPC SDK tests pass `64/64`, including a real ML-DSA quorum
and tampered-ack negative control. Evidence:
`reports/open-source-p1-fastpay-python-v3-wan-client-20260717T040732Z/ACCEPTANCE.json`,
SHA-256
`2ee2a07b9c2c2a65d58ec169827440e4ed545932a09b8eef218f676acd8ddb80`.
Remaining finding-specific closure is the six-node WAN correctness/latency gate
on the immutable candidate.

### P1-SUPPLYCHAIN-01 — Rust advisory, unmaintained/yanked crates, and no dependency policy

**Status:** fixed locally with bounded, expiring upstream exceptions.

`cargo audit` against the 2026-07-13 advisory database reports:

- vulnerable `crossbeam-epoch 0.9.18`, RUSTSEC-2026-0204, fixed in `>=0.9.20`;
- unmaintained `ansi_term 0.12.1`, `bincode 1.3.3`, and `proc-macro-error2 2.0.1`;
- yanked `spin 0.9.8`.

There is no root `deny.toml`, cargo-vet/supply-chain policy, SBOM gate, or immutable action pinning. The in-tree upstream `third_party/halo2_proofs` snapshot records SHA `f6200adaa6ca064d8d2eaa6fcc5e2671232d7249`, but at this audit baseline its local compatibility diff/provenance was not documented as a reviewed patch set.

**Required closure:** upgrade/remove/replace each item or document a bounded non-reachable exception with expiry; add cargo-deny/vet and license/source policies; document and test the Halo2 patch; generate signed SBOM/provenance.

**Local remediation evidence:** `crossbeam-epoch` is 0.9.20 and `spin` is 0.9.9;
`cargo audit` reports zero vulnerabilities. The three remaining unmaintained
transitives are scoped to SP1 logging/serialization or Orchard compile-time
code, documented in `deny.toml`, and expire 2026-08-31. `cargo deny check`
passes advisories, bans, licenses, and sources. SP1 is 6.3.1. The deterministic
CycloneDX generator emits 307 components and byte-identical output for the same
lockfile. Python documentation and test dependencies are separately
hash-locked, and both npm surfaces use committed lockfiles plus clean `npm ci`
and zero-vulnerability audits. Actions are SHA-pinned and locked dependency
resolution is required.

### P1-CI-01 — current CI omits critical tests and the docs job references missing paths

**Status:** fixed-candidate; clean-clone reproduction is part of the publication gate.

`.github/workflows/rust-ci.yml` floats mutable `stable`, omits `--locked`, and runs `cargo test --workspace -- --skip orchard`, excluding the most security-sensitive privacy tests. `.github/workflows/docs-build.yml` installs `docs_site/requirements.txt`, but `docs_site/` is absent; the repository instead has `requirements-docs.txt`. Redaction/docs gates therefore do not work from a clean checkout.

**Required closure:** exact toolchain; locked builds; full privacy suites split by runtime rather than skipped; dependency/license/secret/docs gates; clean-checkout reproduction; fuzz/property/DST jobs; immutable action versions or pinned SHAs; branch protection requirements documented.

**Local remediation evidence:** Rust CI installs 1.95.0, runs locked all-target
check, strict Clippy, and the complete workspace tests without Orchard skips.
Python 3.12, Node 20.20.2 and Foundry 1.7.1 are exact in every job that invokes
them; the local `scripts/check` wrapper mirrors locked all-target check and
strict Clippy rather than a weaker unlocked subset.
Docs CI uses `requirements-docs.txt`, regenerates the evidence index, runs
redaction, verifies all local file/media/anchor targets across 256 Markdown
documents, and builds strict. Product-security CI runs current-tree secret
scanning, cargo-deny, pinned upstream Halo2 source-and-patch verification, deterministic SBOM,
wallet/proxy audits and tests, offline Forge, a negative missing-RPC fork test,
and a separately secret-backed real fork test. It also installs the Python test
toolchain from a hash-locked requirements file and runs all 139 SDK/operations
tests with the repository package root explicit. This gate caught and fixed the
latency runner's stale import of removed operator-local wallet defaults; both
wallet descriptors are now required CLI inputs. Every action is commit-SHA
pinned; all workflow YAML parses locally.

`docs/release-process.md` names the exact required branch-protection check
contexts, forbids force-push/deletion and administrator bypass, requires
CODEOWNERS/security review, and keeps the secret-backed mainnet-fork test as an
exact-revision promotion gate rather than pretending an offline PR run proved
it.

### P1-HISTORY-01 — remaining full-history high-entropy findings are not fully classified

**Status:** classified locally; residual real finding is P0-SECRET-01.

A local full-history Gitleaks scan covered 2,288 commits and 258.68 MB and
reported 719 generic-api-key matches across 233 files/38 commits.
`OPEN-SOURCE-SECRET-HISTORY-CLASSIFICATION-20260716.md` classifies every row:
660 public EVM/token/pool identifiers, 43 test/schema/fixture labels, 13 public
verification/hash values, and three copies of one real Jupyter credential.
Provider-specific and PEM rules did not fire. The four `MASTER_SECRET` rows are
the canonical public XRPL genesis benchmark credential in removed comparison
scripts, not a PostFiat or operator secret. The three real credential rows are
separately elevated to `P0-SECRET-01`; no blanket allowlist was added.

A separate scan of a clean tracked-files archive at the audited HEAD reported
666 generic matches and no provider-specific credential/private-key rule. Raw
operator and wallet evidence has since been removed under
`P0-PUBLIC-EVIDENCE-01`, and the purpose-built tracked-tree scanner is green.
The historical Jupyter token is absent from the candidate tree but remains
reachable through existing Git history.

**Required closure:** classify every unique secret candidate; rotate any real credential before publication; remove wallet/operator-sensitive raw evidence; add blocking current-tree/history scans. The default publication strategy should be a new sanitized public repository or a verified rewritten history, because making the existing remote public exposes all reachable refs, not only the cleaned working tree.

**Local remediation evidence:** the fail-closed redacting scanner and its own
regression pass on the tracked tree. Narrow path/marker exceptions cover only
published deterministic vectors and explicit negative fixtures. A direct
full-history run reports exactly three locations of the same historical
Jupyter-token class plus 24 note-opening field occurrences in removed raw
evidence. The prior generic address/hash/proof/test-vector noise is classified,
not globally suppressed. Publication procedure requires provider revocation and
a clean sanitized repository/ref scan with zero findings; that external P0
remains open.

### P1-DOCS-01 — canonical whitepaper includes unimplemented or contradictory claims

**Status:** fixed locally; canonical claim matrix reconciled.

Confirmed baseline and current-sweep examples:

- `docs/whitepaper.md:81-85` claims a production chained/two-chain HotStuff rule not used by the block commit path;
- `docs/whitepaper.md:143-159` defines a `LaunchCertificate` and public minimum of seven ratifiers, but no exact launch-certificate type/path was found and the controlled devnet has six validators;
- `docs/whitepaper.md:420-428` says every validator registration commits to an SLH-DSA recovery key and describes governed activation, but no FIPS 205/SLH-DSA implementation or activation type was found;
- organization/business papers describe the architecture as “XRPL-derived” and contain roadmap/placeholder language that conflicts with the repository whitepaper's stronger L1 claims.
- the current reconciliation found a later authorization overclaim: §7 said an
  account-level ML-DSA outer envelope bound shielded fee, registry root and
  disclosure policy, while the real `ShieldedActionBatch` contains only
  `batch_id` and `actions`; Asset-Orchard uses RedPallas action signatures and
  ML-DSA authenticates the validator block certificate.

**Required closure:** designate one versioned canonical protocol specification; map every claim to code/tests; implement safety-critical promises or rewrite them as future work; label devnet measurements and controlled authority honestly; archive superseded papers with an explicit status banner rather than leaving multiple canonical-looking accounts.

**Local remediation evidence:** `docs/whitepaper.md` is designated the candidate
protocol document and now describes view-zero direct certificates, fixed-family
ordering, fixed-genesis live governance, no implemented SLH-DSA recovery, and no
automatic availability suspension. `docs/business/whitepaper.md` is explicitly
non-normative commercial material. README and SECURITY identify controlled
pre-testnet maturity and current unsupported production boundaries.
The shielded-authorization and cryptographic-assumption text now matches the
hybrid RedPallas/Halo2/ML-DSA/SP1 boundary and discloses the actual prover-code
separation requirement. `scripts/test-whitepaper-implementation-boundaries`
guards that exact type/claim boundary in docs CI, and the conformance matrix row
links the corrected claim to the real types and verification path.
The final local matrix sweep also reconciles the expanded §3 state model, the
native holdings-plus-cumulative-burn replay/checkpoint oracle, fixed-genesis
one-root registry behavior, view-zero consensus containment, ML-DSA call-site
policy, and the explicit enabled/disabled status of FastLane/FastSwap,
Asset-Orchard, bridges, owned objects and debug proof paths. The strict docs
build, redaction check and implementation-boundary check pass after that sweep.

### P1-METADATA-01 — package/release metadata is not public-release quality

**Status:** fixed locally.

The workspace repository URL still points to `agticorp/postfiatl1v2`, not the actual `postfiatorg` remote. The Rust toolchain floats `stable`; crate publish intent is inconsistent; CODEOWNERS and dependency policy are absent; generated/release artifacts lack a single reproducible signed-manifest flow.

**Required closure:** correct metadata, mark internal crates `publish = false`, pin toolchains, complete ownership/security/release docs, and prove reproducible signed builds from clean checkouts.

**Local remediation evidence:** all 19 packages now have descriptions, the
`postfiatorg` repository URL, `MIT OR Apache-2.0`, and `publish=false` as verified
through locked Cargo metadata. Rust 1.95.0 is pinned. CODEOWNERS, issue/PR
templates, CONTRIBUTING, SECURITY, and the release process are present. SBOM
generation is deterministic; signed artifact reproduction remains a release
ceremony gate rather than an absent policy.

### P1-LICENSE-01 — in-tree upstream Halo2 source lacked a self-contained license/provenance package

**Status:** fixed locally.

The workspace overrides crates.io with `third_party/halo2_proofs` through the root
`[patch.crates-io]` section. This is an in-tree snapshot of upstream Zcash
`halo2_proofs 0.3.2` with a bounded compatibility patch, not a PostFiat
reimplementation of Halo2. At the audit baseline its package metadata declared
`MIT OR Apache-2.0` and recorded upstream commit
`f6200adaa6ca064d8d2eaa6fcc5e2671232d7249`, but the directory contained no
`LICENSE-MIT`, `LICENSE-APACHE`, `LICENSE`, or `NOTICE` file. The root PostFiat
license could not substitute for upstream copyright/license materials, and the
baseline had no checked-in patch manifest showing exactly how the retained tree
differed from upstream.

**Required closure:** add the exact upstream license and attribution files; record
the upstream repository and commit; produce a reproducible normalized diff and
reviewed local-patch manifest; add a CI license/source policy that fails if the
vendored provenance or allowed diff changes; include the component in the SBOM
and source offer. Review every other redistributed binary, WASM, font, image,
proving parameter, verifying key, PDF, and generated artifact under the same rule.

**Local remediation evidence:** the exact upstream Apache-2.0, MIT, and COPYING
files are vendored with pinned hashes. `PROVENANCE.md` fixes upstream commit
`f6200ada...`, enumerates the intentional local patch, and records normalized
patch SHA-256 `d51e2e6e...`. `scripts/verify-vendored-halo2` clones or accepts an
exact upstream checkout, fails on inventory/license/patch drift, and passes.
Cargo-deny and the SBOM include the vendored component.

The retained upstream implementation is not a new proof-system construction.
The local compatibility patch is 361 normalized lines and does not intentionally
change the proving algorithm, verifier equations, transcript, fields, curves, or
proof encoding. PostFiat circuits, public-input bindings, and that compatibility
boundary remain subject to specialist review before real-value use.

### P1-OPS-01 — production operations lack alerting, drills, and independent control planes

**Status:** fixed locally for source publication by production-feature containment; real-value launch remains blocked.

The controlled-pretestnet source has signed manifests/snapshots, canary-first
rollout, local/RPC doctors and recovery runbooks. The candidate now has an
initial numeric height/RPC/mempool/recent-receipt/disk monitor policy and a
tested 14-day/100-MiB log-rotation policy and a machine-enforced controlled
pre-testnet SLO/page/incident policy, but no maintained production alert
delivery stack, multi-region disk/network/control-plane fault drill, or proof
that operational authority is independent of a founder account. The cleanup
baseline also deleted the doctor and monitor executables while leaving their
runbook, and rollout did not check the mutable live committee roster that
certificate verification depends on.

**Required closure:** retain tested operator tools; enforce byte-identical
release and exact committee-roster preflight; sandbox and resource-bound every
service; define and exercise height/root, certificate participation,
rejected-receipt, mempool, disk, proof-latency, RPC-saturation and clock-skew
alerts; run backup/restore, replacement, rollback and multi-region fault drills
under independent credentials; publish incident severity and communication
policy.

**Local remediation evidence:** the bounded validator/RPC doctors, monitor
snapshot, account-history client and code-derived RPC inventory are restored;
the latter was repaired for modular dispatch and reports 132 union methods.
Systemd examples and generated validator units now block privilege escalation,
drop capabilities, protect kernel/control-group state, disable core dumps and
bound descriptors/tasks. Safe rollout now checks all six exact committee
rosters at preflight and immediately before every mutation; its one-member-node
regression and complete-six acceptance pass. See
`OPEN-SOURCE-OPERATIONS-READINESS-INVENTORY-20260716.md`. The monitor now reads
ordering/execution/storage metrics from their real response sections, reports
accepted/rejected/unknown recent receipt semantics, and fails/warns on ordered
height, cross-validator clock-skew, RPC-latency and mempool thresholds. Node
metrics now provide bounded recent certificate-vote and local-participation
counters plus checked total/available filesystem capacity. The monitor warns
below the reviewed participation floor or at/below 15% disk available and goes
critical at/below 5% or when capacity is unavailable. Its
unit regressions and four-node real-RPC smoke pass. Supported AssetOrchard
private-money proof verification now persists the exact non-gating Halo2
verification duration and observation time; the monitor warns above 5 s,
goes critical above 15 s, and rejects a stale prior sample after 5 minutes.
The release-profile real-proof boundary test verifies one accepted and one
rejected timing record and prevents collector double counting. The monitor can
atomically spool private, idempotent warning/critical envelopes for a separate
delivery agent and rejects symlink destinations. The RPC listener adds direct
active/limit/peak/accepted connection telemetry to `metrics`; saturation warns
at 75%, is critical at 95%, and missing telemetry is critical.
`systemd/postfiat-logrotate.example` and
`scripts/test-postfiat-logrotate` add a validated 14-day compressed retention
policy for flat and per-validator logs. External alert delivery/dashboard
health, production custody, and independent fault drills remain open. The
numeric SEV-1/SEV-2 acknowledgement, escalation, public-update and closure
policy is defined in `docs/runbooks/incident-response.md` and embedded into
each alert envelope; this is not evidence that a page was delivered.

## 6. P2/P3 findings and cleanup backlog

- **P2-BLOAT-01 (fixed locally):** 1,283 raw evidence files plus screenshots/PDFs are preserved in two restricted hash-bound archives and removed from the publication tree; the exact 14 retained binary/media paths are hash-manifested and CI-gated.
- **P2-MODULES-01:** protocol-critical modules of 3,000–5,000 LOC impede ownership and focused review.
- **P2-WALLETS-01:** multiple wallet/proxy/extension/demo paths lack a single support/maturity matrix.
- **P2-SCRIPTS-01:** one-off research/evidence/deployment tools are mixed with supported operator tools.
- **P2-API-01 (fixed locally):** the first restored RPC inventory incorrectly called default-enabled FastSwap lock/vote/apply methods “read-only” and recognized only two of fourteen signed-submit methods. A real-boundary regression now requires exact postures for representative read, protocol-mutation, signed-submit, Orchard, owned-lane, and local methods. The v2 generator explicitly partitions all 135 observed methods into 63 reads, 12 default-public cryptographically authorized protocol mutations, 14 flag-gated signed submissions, four flag-gated Orchard methods, four flag-gated owned-lane methods, and 38 operator/local methods, with zero unknowns. Any new default method must be explicitly classified or generation/CI fails. Evidence: `OPEN-SOURCE-RPC-AUTHORIZATION-INVENTORY-20260716.{json,md}` and `scripts/test-rpc-method-inventory`.
- **P2-UNSAFE-01 (reviewed):** production first-party `unsafe` is limited to three Unix FFI boundaries in storage: checked `statvfs`, blocking mutation `flock`, and nonblocking FastSwap process `flock`. Each block has a local safety argument; capacity and lock/concurrency regressions pass. A crate-level deny/allow policy and non-Linux CI remain follow-up hardening.
- **P3-HYGIENE-01:** documentation link rot, stale local paths, outdated branch names, and internal IPs/operators need systematic removal after authoritative docs are selected.

## 7. Unsafe and outdated inventory

| Surface | Finding | Severity/disposition |
|---|---|---|
| Rust unsafe | Three narrow Unix storage FFI blocks: checked `statvfs`, mutation `flock`, FastSwap process `flock`; each has a safety comment and regression | P2 reviewed: add explicit crate policy and non-Linux CI |
| Rust toolchain | Exact `1.95.0` pinned | Fixed locally; CI and metadata gate verify it |
| Rust dependencies | zero known vulnerabilities; three expiring unmaintained and one yanked transitive exceptions | P1 fixed locally with deny policy and 2026-08-31 expiry |
| Wallet build | Static hardened build; updated Vite/esbuild; audits zero | P0 fixed locally; development serving is not a release path |
| Wallet custody | Browser signs locally; seed-bearing proxy contract removed | P0 fixed locally; no public custodial mode supplied |
| Validator custody | plaintext JSON software keys | P1: implement production signer boundary or label production unsupported |
| Storage | JSON/JSONL full reads/rewrites | P1: replace/bound and migration-test |
| Public transport | Loopback/private binds, bounded limits, authenticated TLS edge required; public plaintext bind rejects | P1 fixed locally for source publication; edge deployment is a real-value gate |
| Unsigned public mutation | Legacy remote `wrap_owned` removed; FastPay mutations require signed domain-bound envelopes and verified certificates | P0 fixed locally |
| Ethereum bridge evidence | Asserted transitions reject live; historical replay only; controller requires a bound verifier not supplied by default | P0 feature-contained; no external route may be enabled |
| Legacy shielded path | Cleartext actions and ingress-v1 reject live; encrypted Asset-Orchard ingress-v2 supported | P0 fixed locally; historical decode only |
| FastPay unlock | Ordered v3 consume-or-cancel recovery advances versions; legacy local deletion stays fail-closed | P1 fixed-candidate; crash/reconfiguration, WAN and exact-candidate gates pass |
| Docs CI | Strict site build/redaction paths restored and passing | P1 fixed-candidate; exact local and clean-clone gates pass; hosted external review is not a source-publication prerequisite |
| CI coverage | Complete workspace including Orchard plus real-proof release test | P1 fixed locally |

## 8. Whitepaper conformance summary

The full claim-by-claim matrix is
`docs/status/OPEN-SOURCE-WHITEPAPER-CONFORMANCE-MATRIX-20260716.md`. STEP 1
resolved the canonical paper and reconciled every row against the supported
public-source feature set. The matrix preserves the discovery baseline; this is
the final-candidate executive summary.

| Claim | Final-candidate status | Evidence/action |
|---|---|---|
| Known-validator authority ledger | Fixed-candidate and integrated | Validator registry and certificate paths exist; registry changes require signed old-rule authorization and delayed activation; n=4/n=6 replay and rollback gates pass |
| Quorum `floor(2n/3)+1` | Fixed-candidate | Every live certificate family deduplicates validator IDs and enforces its exact committee-domain quorum; cross-family inventory/regressions pass |
| Proposal justify and cross-view locks | Fixed-candidate | Consensus v2 verifies typed QC ancestry, persists high/locked QC and votes before signing, and safely advances after a failed proposer |
| Two-phase commit | Fixed-candidate | Only a non-nil precommit QC commits; exhaustive n=4/n=6 models and real TCP failed-proposer recovery pass |
| Deterministic inclusion order | Fixed-candidate | Every live batch family is mapped to canonical prefix/proposal order; stale-vs-conflicting mempool regressions pass |
| Cobalt-governed registry | Fixed-candidate | Unsigned live mutations reject; signed old-rule amendments and key rotation converge, replay and roll back at n=4/n=6; longer multi-region drills are a real-value gate |
| LaunchCertificate with >=7 ratifiers | Explicit future public-launch requirement | Exact artifact is not implemented and is no longer described as current state |
| Fixed native supply and fee burn | Implemented and replay-proven | Genesis-bound supply; per-block all-lane custody/burn oracle; checkpoint-v2 cumulative-burn equality |
| Atomic dual-auth DvP | Fixed-candidate and devnet-proven | Integrated property, fault, replay, crash-prefix, exact-six and conservation coverage passes |
| Issued asset/NAV accounting | Fixed-candidate | Complete cap, custody-lane, redemption, rounding, stale-packet, bridge and two-wallet private-flow conservation matrices pass |
| Shielded settlement hides owner/value | Action-specific and implemented for private Asset-Orchard spends | Private swap hides openings/assets/values/parties; ingress/egress deliberately reveal their public boundary fields; legacy Mint/Spend and ingress v1 are historical replay only |
| Honest privacy leakage statement | Fixed-candidate at supported boundaries | Circuit public inputs, RPC/log/evidence redaction, browser custody and private ingress/swap/egress scans pass; timing/thin-set limitations remain disclosed |
| ML-DSA authorization | Fixed-candidate | Complete domain/key/canonical-encoding inventory and 46-call blocking policy pass |
| SLH-DSA recovery commitments | Explicit future work | Present-tense claim removed; no implementation implied |
| Models outside consensus authority | Claim-corrected and source-verified | Consensus consumes committed deterministic records; model tooling remains decision support and has no signing or state-transition authority |
| Canonical Ethereum/Uniswap representation | Devnet/demo only | Live-production implication is removed; governed verifier/code-hash activation and independent contract/bridge assurance remain real-value gates |
| No production guarantee from controlled measurements | Corrected in primary entry points | README, SECURITY, canonical paper and release process label controlled pre-testnet scope |

## 9. Executed STEP 2 implementation order

The dependency order below records the completed remediation sequence; final
dispositions and exact evidence are in the closure table.

1. **Immediately contain unsigned mutation P0:** reproduce P0-RPC-01, remove the public direct mutation, implement the signed consensus deposit, and migrate every caller before any public runtime is shipped.
2. **Contain external bridge P0:** reproduce fictitious-burn and refund/consume races, disable the route by default, then implement and prove the selected Ethereum finality/proof and mutual-exclusion design before re-enabling.
3. **Contain EVM supply-release P0:** reproduce P0-SUPPLY-01, disable unauthenticated mint release, then bind release to a one-time verified on-chain settlement or certificate before re-enabling.
4. **State commitment P0:** reproduce P0-STATE-01, commit every consensus field exactly once, and prove historical replay plus coordinated upgrade/rollback behavior.
5. **Consensus agreement P0:** reproduce P0-CONSENSUS-01, select and implement the complete production commit/lock/high-QC rule, migrate/replay, and run adversarial simulation.
6. **Governance authorization P0:** reproduce the no-key forgery, require old-registry signed votes at every validation boundary, and migration/replay-test activation.
7. **Custody P0:** remove remote seed-bearing signing from the self-custody wallet and lock the proxy/API boundary.
8. **Privacy P0:** disable new legacy cleartext shielded actions and prove Orchard-only supported privacy semantics.
9. **Wallet serving P0:** update dependencies and replace public Vite development serving with a hardened static production path.
10. **Monetary integration:** re-run the issued-wrap fix and all native/issued/NAV/bridge/swap conservation oracles.
11. **Storage P1:** implement transactional bounded storage plus migrations, recovery, and scale tests before calling the node production-ready.
12. **Ordering/network/key custody P1:** implement or correct ordering/accountability claims, separate listeners, add authentication/TLS boundary, and implement production signing custody.
13. **FastPay liveness P1:** model and implement safe cancellation/drain or disable the owned lane and narrow claims.
14. **Supply-chain/licensing/CI P1:** close advisories, restore vendored provenance and license materials, pin toolchains/actions, repair docs/privacy gates, add deny/vet/SBOM/provenance.
15. **History/docs P1:** sanitize publication history/evidence and make the canonical whitepaper exactly match the final implementation.
16. **Integrated release proof:** clean checkout, all unit/integration/property/fuzz/DST/replay/migration/rollback/soak tests, reproducible builds, signed manifests, and a zero-open-blocker closure table.

## 10. Closure table schema

Every P0/P1 must eventually have one row with no blank evidence fields:

All closed rows below inherit the complete exact-tree battery and
sanitized-clone reproduction recorded in the release-owner
`open_source_publication_candidate_20260717/ACCEPTANCE.json` manifest outside
the public tree. Row-level evidence is the finding-specific proof in addition
to that shared battery. No P0/P1 row remains open.

| ID | Status | Reproduction | Fix commit | Regression test | Integrated evidence | Claim update | Residual risk |
|---|---|---|---|---|---|---|---|
| P0-CONSENSUS-01 | FIXED-CANDIDATE | pre-fix cross-view counterexample passed; lexical opaque-QC regression failed before fix | `4a2d4131`, `09125687` | v2 canonical artifacts, typed QC graph, durable prepare/precommit/timeout/QC restart, self-contained commit/replay, signer-safe snapshot-v6 continuity, byte-preserving legacy boundary, adversarial n4/n6 model, and automatic shipping timeout-certified recovery PASS | shipping failed-proposer recovery n4 80.51s and n6 122.55s; timeout RPC/envelope/proxy/auth regressions, exact-candidate workspace, affected check and strict Clippy PASS | whitepaper and architecture docs state the implemented two-phase rule, signed timeout ancestry, bounded automatic recovery, and reset/new-genesis migration boundary | independent consensus review and multi-region operations remain real-value launch gates, not source-publication blockers |
| P0-CUSTODY-01 | FIXED-CANDIDATE | pre-fix source/dynamic boundary confirmed | `9c5b29c9`; verified in `00747667` | local-WASM, removed-RPC, runtime custody guard and real Chromium full-egress capture PASS | exact candidate: browser capture 1/1; public-browser 1/1; wallet 240/240; proxy 23/23; build PASS; both npm audits zero | self-custody boundary corrected without blocking public signatures or signed envelopes | intentionally custodial service is not supplied |
| P0-CUSTODY-02 | FIXED-CANDIDATE | real shipping CLI stdout contained exact caller master/signature seeds while claiming redaction | `4a2d4131`; verified in `00747667` | report v2 unit rejects field names and supplied values; shipping binary success/failure stdout, stderr, argv, work-artifact, crash/panic-name scan PASS | exact-candidate shipping subprocess test 1/1 PASS | public vector retains derivations but never echoes secret inputs | deterministic fixture inputs remain documented separately as intentionally public vectors |
| P0-GOVERNANCE-01 | FIXED-CANDIDATE | pre-fix no-key amendment entered proposal; unsigned live proposal/apply still reject without mutation | `4a2d4131` | v2 signed authorization binds complete action and chain/registry/epoch/slot/expiry; wrong-chain/epoch/registry/slot/key/payload and missing/duplicate authorization reject; signed operation-kind/FastSwap matrix, concurrent same-slot conflict, rollback, restart, old-rule delayed rotation and historical replay PASS | governance-targeted node 40/40 and Cobalt 66/66; real TCP n4 50.91s and n6 90.79s commit signed amendment, old-key-authorized rotation, activation, replacement-key epoch, convergence and replay on every replica; exact-candidate workspace and strict Clippy PASS | signed batch is the live governance authority; Cobalt RBC/ABBA are signed research primitives outside the node production governance call graph | launch ceremony and independent ratification remain real-value launch gates |
| P0-PRIVACY-01 | FIXED-CANDIDATE / HISTORICAL-REPLAY-ONLY | pre-fix local cleartext mint acceptance source/test confirmed | `4a2d4131` | creator + admission + live no-mutation + archive replay regression; encrypted v2 ingress, generic private transfer, real K15 private atomic swap, chain-only recovery and private egress all PASS with exact accepted receipts, nullifiers, deltas and conservation | complete-flow 1/1 in 2,524.27s; generic transfer 1/1 in 372.91s; 13 public artifacts clean; ordinary privacy `83/83`; release-scale `17/17` including byte-identical K15 artifact; wallet 232/232; proxy 23/23; evidence SHA-256 `b830ce023d078aeed4acc832679591072519827f60af72d778b144dc5d5672ec` | supported privacy narrowed to Asset-Orchard without removing deposit/transfer/swap/egress | legacy decoder remains replay-only under exact authenticated history predicates |
| P0-PRIVACY-02 | FIXED-CANDIDATE / HISTORICAL-REPLAY-ONLY V1 | pre-fix serialized live ingress contained the exact note opening and browser fallback was plaintext; oversized serialized JSON bypassed recursive inspection | `d1e68ee8` | v1 admission/live no-mutation reject + archive replay accept; v2 serialization, PFAOENC1 decrypt and two-fresh-wallet K15 swap/private-egress PASS; oversized JSON-looking transport fields fail closed | two-wallet release proof 1/1 in 340.41s; 13 public artifacts clean; browser SHA-256 `e07bacd757b646022a13bdf28a9730a30a158920141eaf73677ee92f1597df01`; candidate evidence SHA-256 `ffcd818b23f60cc81eaa660d2b0f01bb0fddc9e6a593e9758bce842fa2686978`; strict Clippy/fmt PASS | §7.1 and matrix state action-specific leakage | historical v1 decoder remains archive-only under authenticated replay predicates |
| P0-PUBLIC-EVIDENCE-01 | FIXED-CANDIDATE | pre-fix tracked-tree scanner missed real `rho`/`psi`/`rcm`; new rule found 21 current-tree values before archive removal and 24 field occurrences in reachable history | `c5472708`, `00747667`, `012d3a8b` | scanner regression fails on a note opening without echoing it; raw evidence archived and removed; tracked-tree scan PASS | deterministic 1,283-file archive verified; final one-commit staging and initial GitHub clone have exact tree/ref/file-count equality and zero tracked-tree or reachable-history findings | raw evidence is explicitly excluded from public source | historical credential disposition is closed under `P0-SECRET-01` |
| P0-WALLET-02 | FIXED-CANDIDATE | pre-fix loopback/CSP regression failed; npm audit found Vite/esbuild advisories; un-hashed asset cache regression failed before follow-up fix | `9c5b29c9`; verified in `00747667` | loopback/CSP + canonical-root static serving + real Chromium CSP/origin/navigation/cache/disclosure boundary PASS | exact candidate: wallet 240/240; build PASS; Chromium public/custody 1/1 each; both npm audits zero; proxy 23/23 | Vite documented and enforced development-only; production image has no Vite runtime | production deployment still requires the documented authenticated edge profile |
| P0-WALLET-BRIDGE-DEST-01 | FIXED-CANDIDATE | pre-fix browser boundary returned the drained old vault; configuration remained destination authority; follow-up browser test exposed trust in a reported digest without recomputing every profile field | `4a2d4131`, `9c5b29c9` | signed complete route/verifier profile, replicated discovery, separate SP1-public-input and governed-route commitments, route-bound ingress/egress, current-vs-pinned rotation, byte-identical pre/post snapshots and certified rollback/reapply, source chain/code/deposit/withdrawal audit, exact `V=S+D+B-R`, stale/substitution/downgrade/verifier-mismatch/one-atom rejection PASS | controlled Anvil+PFTL round trip: 11/11 PFTL accepted-code receipts, 8/8 EVM status-1 receipts, four lifecycle conservation audits; node governed-route 6/6 plus real gate 1/1; conservation 3/3; vault 13/13; execution 146/146; types 87/87; wallet 226/226; proxy 23/23; exact-candidate workspace/check/strict Clippy PASS | concrete evidence tier/trust dependency, verifier contract, historical route lifecycle, state migration and controlled round-trip procedure are explicit | production bridge deployment and specialist audit remain real-value launch gates |
| P0-PROXY-AUTH-01 | FIXED-CANDIDATE | pre-fix real public HTTP/WS boundary reproduction; multi-principal regression failed before fix | `9c5b29c9`; verified in `00747667` | inverted HTTP/WS boundary, exhaustive mutation classification, principal-scoped replay, rate/body/concurrency and WSS regressions PASS | exact candidate: proxy 23/23 and audit zero; wallet 240/240/build; RPC inventory explicitly classifies all 143 observed methods with zero unknowns | authenticated edge/operator contract documented | production deployment still requires the documented authenticated TLS edge and operator secrets |
| P0-SECRET-01 | CLOSED-PUBLISHED | history/source-confirmed | `012d3a8b` (fail-closed gate) plus provider-owner terminal-action record outside Git | exact-ref/tree + deleted-but-reachable-history + mandatory private provider-record regressions PASS | provider owner confirmed destruction; mode-0600 record passes; public `main` descends only from the sanitized root and all current public refs pass reachable-history scanning | publication runbook and executable gate remain fail-closed | contaminated development history remains private; only sanitized history is public |
| P0-ASSET-01 | FIXED-CANDIDATE | pre-fix wrong-label mint plus failing duplicate-ID, issued-to-native unwrap and overflow-atomicity regressions | `4a2d4131`; verified `8696cad5` | complete constructor inventory; execution wrong-label/zero/overflow/collision/replay/conservation tests; real node-store signed issued-unwrap and concurrent unsigned-wrap rejection; prototype duplicate fixture/input/vote tests PASS | candidate execution 156/156; node FastPay safety 14/14; prototype 21/21; owned-object fuzz 256 iterations/2,816 cases; exact-candidate workspace and strict Clippy PASS | exact native/issued lane binding and non-production fixture boundary documented | future object constructors remain compile-inventoried and fail closed until classified |
| P0-RPC-01 | FIXED-CANDIDATE | source-confirmed arbitrary-debit path | `4a2d4131`, `9c5b29c9` | removed unsigned real-store/allowlist boundary; signed field-binding, rejection, atomicity, receipt-code, wallet-local-signing and replay tests PASS | real n=4 (73.31s) and n=6 (123.93s) TCP flows fund a fresh wallet through consensus and prove exact conservation on every replica; exact-candidate wallet/proxy/build/audit and workspace gates PASS | signed Account-to-FastPay funding and exact accepted-code UX restored without backend custody | authenticated production edge remains a deployment gate, not a public-source defect |
| P0-BRIDGE-01 | FIXED-CANDIDATE | pre-fix consensus test accepted asserted refund/burn; source race confirmed | `4a2d4131` | production receipt builder + governed checkpoint observer + isolated durable exact-quorum signer drive signed subscribe/export, consume, return and refund alternate; every money receipt accepted; exact 50-atom conservation; wrong amount zero-mutation; consume/refund mutual exclusion in both orderings; minority partition/unvoted reorg reject; every ordered-commit crash prefix recovers idempotently; external inventory is included in the checked global cap | bridge 36/36; execution 139/139; types 83/83; production checkpoint/signing 2/2; receipt builder 3/3; focused node crash/supply regressions; exact-candidate Anvil/non-fork Foundry/workspace/strict Clippy PASS | explicit BFT_CHECKPOINT trust model and packet-digest binding documented | official-mainnet fork execution and contract audit remain real-value activation gates |
| P0-SUPPLY-01 | FIXED-CANDIDATE | pre-fix beneficiary fabrication passed | `4a2d4131` | exact-code-hash BFT settlement verifier; exact domain/value/finality binding; wrong amount/controller/verifier, under-quorum, duplicate, rejected receipt, replay and timelocked drain-safe rotation PASS | controlled local-PFTL + Anvil production-verifier release has exact 110-atom backing/release and three negative boundaries, SHA-256 `b9cf666416126a81c3ecf18bd9686e485e84c7db055fce48c834f7a0311f66fa`; continuous same-asset mint/return/wrong-amount-failure oracle has zero unexplained delta at five checkpoints, SHA-256 `0a18b1a9ab808fe74c2f70df0c4de1e2866a70990758af0a94b41096e400f8e9`; exact-candidate Anvil/non-fork Foundry/workspace PASS | explicit federated trust model and rotation/runbook boundary documented | production signer custody and official-mainnet fork remain real-value activation gates |
| P0-NATIVE-SUPPLY-01 | FIXED-CANDIDATE | coordinated replay-base faucet/ledger rewrite returned `verified=true`; native FastLane deposit burned 2 atoms while reporting `fee_burned=0` | `4a2d4131`; verified `012d3a8b` | compile-exhaustive custody inventory; per-lane total and mismatch oracle; exact FastLane burn; checkpoint v2 cumulative-burn proof; v1 refusal plus offline archive rebuild that discards legacy state and verifies prefix/suffix before atomic replace | native fuzz 256 iterations/2,304 cases; candidate custody oracle 1/1, coordinated rewrite rejection 1/1, prune/recovery 2/2, signed snapshot restore/replay 1/1, exact-candidate workspace/strict Clippy/fmt PASS | fixed genesis supply and replay/checkpoint conservation are explicit | no known residual source-publication P0 |
| P0-ISSUED-SUPPLY-02 | FIXED-CANDIDATE | state root accepted public 10 + private 1 under max_supply 10; mint cap omitted FastLane reserve; status/replay omitted FastLane, external, and Orchard custody | `aa35692a` | compile-exhaustive global oracle; duplicate/unknown/unsupported rejection; status/replay v2 bind every lane; four-lane exact-cap composition rejects signed false-headroom mint without canonical mutation | issued fuzz 256 iterations/4,352 cases; real release FastLane, BFT-checkpoint external, two-fresh-wallet K15 private flow, snapshot/replay and strict Clippy PASS; acceptance SHA-256 `993d961ada1d113a9bf91f74a17e430c87b8795439cf561ef310a23820b88d6a` | every supported custody lane and supply-changing transition classified | no known residual P0; future custody fields are compile-gated until classified |
| P0-COMMIT-ATOMICITY-01 | FIXED-CANDIDATE | eight concurrent same-nullifier egress calls all returned accepted | `4a2d4131` | cross-process ordered-commit lock spans recovery/read/execute/persist for transparent, shielded, bridge, governance; typed lock witness required | post-fix race: 1 accepted, 7 AlreadyExists; transparent 0..10, activation/FastPay 0..9, bridge 0..6 crash prefixes plus shielded, generic journal, supply/snapshot/replay and exact-candidate workspace PASS | no stale concurrent state transition may report accepted | production filesystem must preserve documented Unix `flock` semantics |
| P0-STATE-01 | FIXED-CANDIDATE | pre-fix real root collision and old h1220 over-cap current-devnet state reproduced | `4a2d4131`; reset artifact permissions at `249149bc` | all ten fields, compile-exhaustive five-domain inventory, order/amount sensitivity, versioned activation, backdated rejection and every activation-journal write prefix PASS | isolated exact-six upgrade/rollback drill plus cap-valid live reset, accepted h1 consensus-v2 block, signed v6 snapshot and six independent exact-tip/root `verify-state`/`verify-blocks` replays PASS; acceptance SHA-256 `2269e611a93a2715c2746859c63e91eb577d79a8fbad5bedec029a6eb7083d73`; exact-candidate workspace PASS | explicit compatibility matrix, v6 snapshot migration, preactivation rollback and postactivation forward-recovery contract | old invalid h1220 stores remain rollback evidence only and are not active state |
| P1-CERT-DOMAIN-01 | FIXED-CANDIDATE | live helper accepted empty legacy root | `4a2d4131` | real `apply_batch` missing-root rejection PASS | rooted proposal, timeout and exact-candidate workspace regressions PASS | live/historical boundary made explicit | historical committed blocks retain versioned legacy verification only |
| P1-ARITH-01 | FIXED-CANDIDATE / INVENTORY COMPLETE | real transfer execution, node builder, untrusted block-response adjacency, and FastSwap bootstrap tip calculations each panic at `u64::MAX` | `4a2d4131` | all four boundaries reject without mutation/signing; compiler-assisted monetary classification complete | execution 136/136 single-threaded; RPC SDK 55/55; bootstrap/state-root regressions, exact-candidate workspace/check/strict Clippy PASS | deterministic rejection and rounding contracts recorded | circuit field arithmetic remains intentionally modular and separately constrained |
| P1-MEMPOOL-01 | FIXED-CANDIDATE | real mixed-family node admission accepted transfer sequence 2 behind pending asset sequence 1 although proposal order rejects it; integrated suite then reproduced an independently stale atomic entry wedging unrelated admission | `4a2d4131` | canonical-prefix admission rejects false success without mutation; independently stale atomic/FastLane entries are skipped, but valid pending entries still reject candidate-induced conflicts | atomic-swap consensus 15/15 plus exact-candidate workspace and six-process atomic-swap PASS | canonical family order and stale-vs-conflicting distinction documented in finding | future transaction families must be added to both prefix and proposal inventories |
| P1-STORAGE-01 | FIXED-CANDIDATE / SOURCE-PUBLICATION FEATURE-CONTAINED; REAL-VALUE LAUNCH GATE | pre-fix whole-history/allocation surfaces; concurrent 24-writer test retained only 1 successful mempool append; daemons started without storage acknowledgement | `4a2d4131`, `b49c85e4` | caps; concurrent append 24/24; commit/recovery lock; inherited-descriptor owner-drop; all core daemon dispatches reject missing exact acknowledgement | storage 26/26; service/generated-unit/restart regressions; exact-candidate workspace/check/strict Clippy PASS | README/SECURITY explicitly controlled-devnet | indexed transactional production engine and production-scale tests required before real value |
| P1-ORDERING-01 | FIXED-CANDIDATE / CLAIM-CORRECTED | source/claim mismatch confirmed | `c5472708` | production order tests remain PASS | exact-candidate strict docs and workspace builds PASS | §4.2–4.4 corrected | stronger accountability protocol remains research |
| P1-KEYS-01 | FIXED-CANDIDATE / REAL-VALUE FEATURE-CONTAINED | source-confirmed plaintext signer | `c5472708` | explicit unsafe-devnet acknowledgement regression PASS | exact-candidate runtime-default, strict Clippy and check gates PASS | production custody explicitly unsupported | HSM/remote signer required before real value, not before source publication |
| P1-NET-01 | FIXED-CANDIDATE | public-bind examples and override reproduced | `4a2d4131`, `c5472708` | bind guard and generated-unit regressions PASS | exact-candidate node/runtime-default/check gates PASS | authenticated TLS edge policy | edge deployment testing remains a real-value launch gate |
| P1-RPC-ERROR-01 | FIXED-CANDIDATE | real remote response returned an operator data path and filename | `4a2d4131` | internal/path-bearing errors redacted; typed protocol error preserved | RPC request suite 18/18 plus exact-candidate workspace/check/strict Clippy PASS | stable public error contract | protected local diagnostic sink remains operational follow-up |
| P1-FASTPAY-01 | FIXED-CANDIDATE | late-certificate hazard, unsafe `q-f` counterexample, sub-quorum apply/ordered-omission divergence, missing committee rotation, stale Python v2 WAN caller, and completed-outbox response-loss gap reproduced | `1e9352c6`, `77e4a3c7`, `90c3836a` | full recovery/crash/rotation matrix plus bounded durable completed response, restart, compaction, migration, tamper/conflict and zero-reapply replay PASS | WAN battery 5/5 p50 2.49s/p95 3.72s with five signed acks and exact-six audit; exact-candidate proxy 24/24, npm audit zero and workspace PASS | recovery v1 spec and q-ack finality match; unsafe one-ack result excluded; completed results replay from bounded durable evidence | external WAN operations remain a real-value deployment gate |
| P1-SUPPLYCHAIN-01 | FIXED-CANDIDATE | advisory/yank baseline and cross-checkout SBOM absolute-path drift captured | `4a2d4131`, `09dc28d4` | cargo audit 0 vulnerabilities; cargo-deny and Halo2 PASS; SBOM v2 is byte-identical after complete tracked-tree relocation | deterministic 307-component SBOM; npm audits zero; exact candidate and second clone reproduce byte-identical SBOM | local component identities are repository-relative; exceptions expire 2026-08-31 | three unmaintained transitives remain time-bounded and explicitly tracked |
| P1-CI-01 | FIXED-CANDIDATE | missing docs path, skipped Orchard, externally coupled/locally generated proxy fixtures, runtime/crypto scanner blind spots, nonportable SBOM, inherited FastSwap lock race, ignored operator-report fixture dependencies, and a load-sensitive quorum-early sleep threshold confirmed | `90c3836a`, `3b4b882f`, `0d1115ae`, `09dc28d4`, `5fd7045e`, `b49c85e4`, `929b0f40`, `d5740cdc` | YAML/local equivalents; self-contained proxy 24/24 loading hash-pinned tracked WASM; self-verifying runtime/crypto scanners; SBOM relocation; inherited-descriptor lock regression; both legacy WAN compatibility vectors use canonical tracked testdata; quorum-early behavior uses a deterministic gated-sixth-response proof | exact candidate and clean clone execute complete workspace/release Orchard/FastSwap/atomic/Foundry/Anvil and publication gates | release workflow and clean-checkout test inputs are explicit and reproducible | no known residual source-publication P1 |
| P1-HISTORY-01 | CLOSED INTO P0-SECRET-01 / P0-PUBLIC-EVIDENCE-01 | 719 generic baseline findings classified | `c5472708`, `00747667`, `012d3a8b` | scanner regression + final tracked tree and sanitized history PASS | contaminated private history fails on 3 credential plus 24 private-note-opening occurrences; public GitHub history has zero | sanitized-history procedure and exact-ref/tree gate added | contaminated development refs remain private and are not publication sources |
| P1-DOCS-01 | FIXED-CANDIDATE | code/claim matrix confirmed consensus, governance, recovery, privacy-envelope and state-model mismatches | `c5472708` | strict docs build, redaction and implementation-boundary checks PASS | exact-candidate and clean-clone docs strict/redaction/boundary suite PASS | canonical paper, business-paper status, README/SECURITY and complete matrix corrected | empirical evidence remains controlled and supporting feature specs remain non-canonical |
| P1-METADATA-01 | FIXED-CANDIDATE | metadata baseline captured | `c5472708` | locked metadata validates all 19 crates | exact-candidate strict check/docs/SBOM PASS | release/contribution/ownership docs added | signed release ceremony remains a real-value launch gate |
| P1-LICENSE-01 | FIXED-CANDIDATE | missing license/provenance baseline confirmed | `c5472708` | normalized upstream verifier PASS | exact-candidate cargo-deny/SBOM/Halo2 and artifact policy PASS | upstream commit and patch hash documented | specialist crypto review remains a real-value launch gate |
| P1-OPS-01 | FIXED-CANDIDATE / PRODUCTION FEATURE-CONTAINED | missing tooling, proof timing, alert emission and incomplete roster preflight reproduced | `c5472708` | roster, runtime-default, unit hardening, proof timing, alert spool, incident-policy and RPC-inventory regressions PASS | exact-candidate real proof, monitor, workspace check and strict Clippy PASS | controlled-pretestnet boundary explicit; production units refuse file signer without unsafe acknowledgement | external delivery, production custody and independent drills remain real-value launch gates, not source-publication claims |

## 11. Commands captured for this audit

The following command classes were run locally against the baseline and must be rerun against the release candidate:

```text
git status --porcelain=v2 --branch
git rev-parse HEAD
git branch --format=...
git count-objects -vH
git ls-files / stat / wc inventory
rustc --version; cargo --version; node --version; npm --version; python3 --version
cargo audit --json
npm audit --json                      # wallet-web and wallet-proxy independently
gitleaks git --report-format json ... # all local history/refs
rg/nl source and claim traces
```

Final source-candidate result (exact identifiers and log hashes are in the
external acceptance manifest):

- `cargo fmt --all -- --check`: PASS;
- `cargo check --workspace --all-targets --locked`: PASS;
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS;
- `cargo test --workspace --all-targets --locked`: PASS, including the complete
  model, replay, recovery and ordinary Orchard suites;
- release Orchard: PASS 16/16, with the parameter writer separately reproducing
  the exact 2,097,220-byte committed artifact;
- six-process FastSwap correctness/restart and 100-operation performance,
  six-validator W6 atomic swap, and both isolated Anvil gates: PASS;
- Forge non-fork suite: PASS 103/103; the official-mainnet fork gate fails
  closed, rather than silently skipping, because its provider-owned RPC value is
  absent and is classified as a real-value activation gate;
- wallet: PASS 240/240, build PASS, audit 0; proxy: PASS 24/24, audit 0;
- strict docs/redaction, current-tree secret scan, cargo-deny, RustSec, Halo2
  provenance and deterministic SBOM: PASS.

Raw reports containing suspected secret material are not committed to the public
tree. The external acceptance manifest contains hashes, tool versions, redacted
summaries and storage locations.

### 2026-07-16 P0-RPC-01 signed FastPay funding evidence

- `npm test` in `wallet-web`: PASS 222/222; `npm run build`: PASS;
  `npm audit --audit-level=high`: PASS with zero vulnerabilities. The regressions
  require local WASM signing, exact field binding, canonical submission, exact
  `owned_deposit_applied` acceptance, and fail-closed rejected/unknown handling.
- `cargo test -p postfiat-types -p postfiat-execution -p postfiat-rpc-sdk
  -p postfiat-wallet-wasm -p postfiat-fastpay-prototype --all-targets --locked`:
  PASS (types 83, execution 139, RPC SDK 57 plus binaries, wallet WASM 1,
  FastPay prototype 6).
- `cargo test -p postfiat-node --lib
  main_parts::tests::transport_batch_payload_tests::peer_certified_transport_recovers_from_failed_proposer_and_rotates_validator_keys -- --exact --nocapture`:
  PASS at `n=4` in 73.31s and `n=6` in 123.93s. Both runs start with an absent
  wallet, certify its funding transfer, certify its locally signed deposit,
  observe the exact accepted receipt, prove fee plus owned-value conservation
  and byte-identical state on every replica, and reject replay.
- `cargo check --workspace --all-targets --locked`: PASS. Strict Clippy on every
  affected Rust crate (`types`, `execution`, `rpc-sdk`, `wallet-wasm`, `node`,
  and `fastpay-prototype`) passes with `-D warnings`.
- `./scripts/build-wallet-wasm-release`: PASS; all shipped web/extension WASM
  artifacts have SHA-256
  `6cb4c8a51d61296b129893cef220d64e3a61a1e83ad322c27621955325855955`.
- The complete node/all-targets and immutable-candidate workspace/CI reruns are
  included in the shared exact-candidate evidence described above.

## 12. Closure state after STEP 1 and STEP 2

The repository-scale STEP 1 discovery and classification pass is complete: the
surface, arithmetic, supply, cryptography, bridge/EVM, storage/determinism,
operations, RPC authorization, whitepaper, history and artifact inventories now
cover the supported candidate and explicitly classify contained or disabled
surfaces. Every confirmed P0/P1 has a failure argument, disposition, regression
boundary, owner class and residual-risk entry in this audit. Unchecked launch
criteria in the master checklist are not discovery placeholders; they are
documented real-value assurance work that the source candidate explicitly does
not claim to satisfy.

All internally actionable STEP 2 P0/P1 work is complete. Automatic shipping view
recovery is committed at `09125687`; bounded FastPay completed-response replay is
committed at `77e4a3c7`; the exact candidate and clean staging clone pass the
integrated publication battery. RC5 additionally exposed a restart race in which
a descriptor inherited by a concurrently forked child retained the FastSwap
kernel lock after the logical store owner dropped. Commit `b49c85e4` explicitly
releases the owner lock before close, preserves fail-closed concurrent-open
exclusion, and adds a deterministic inherited-descriptor regression.
RC6 then exposed a clean-checkout-only test dependency on an ignored legacy NAV
operator report. Commit `929b0f40` reuses the already tracked canonical block-3
catch-up batch, so the receipt-ID compatibility check is identical in a fresh
clone without adding a duplicate fixture or changing runtime behavior.

Source publication is closed. For `P0-SECRET-01`, the provider owner confirmed
destruction and the required private mode-0600 record passes. The published
GitHub repository is rooted in the one-commit sanitized export; its initial
fresh clone passed exact tree/ref/file-count checks, and current public refs,
including automated dependency-update branches, pass reachable-history scanning
with zero findings. The contaminated development repository remains private.

The proof-system review is no longer a discovery placeholder. The
machine-readable `OPEN-SOURCE-PROOF-PUBLIC-INPUT-INVENTORY-20260716.json` maps
all 41 AssetOrchard live public fields, private witnesses, host-only checks,
live/replay VK policy, the SP1 decoded ABI subset, and debug-proof reachability.
Its CI gate freezes five proof-source hashes and exact public-index coverage.
SP1 guest/program reproduction and specialist circuit review remain real-value
activation gates, not unreported public-source claims.

The privacy evidence gate is also fail-closed. A pre-fix regression proved the
NAVSwap redaction reporter echoed a detected seed value into its own JSON and
did not recognize `rho`/`psi`/`rcm` note openings. Report schema v2 emits no
matched sample, detects note openings and spend-authorization fields, and
passes alongside the broader tracked-tree scanner. Deployed wire/browser/log
capture review remains a real-value evidence gate, not a claim inferred from a
source scan.

Real-value launch additionally requires production signer custody,
transactional indexed storage, external bridge verifier deployment, specialist
consensus/circuit/bridge review, independent multi-region drills, reproducible
signed releases, and launch authority. Those are deliberately not treated as
source-publication P0/P1s because their corresponding production features and
claims are disabled or contained.
