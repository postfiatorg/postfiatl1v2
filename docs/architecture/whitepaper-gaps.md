# Whitepaper alignment gaps

The [alignment inventory](whitepaper-alignment.md) preserves findings at `d351353e57b295368450a57866ace17b5e1ce6ad`. This backlog separates corrected documentation from remaining implementation and verification work. Priorities describe the impact of making an unsupported claim; they do not declare an exploitable runtime bug.

## Corrected in this documentation patch

- Replace research-only Cobalt wording with the implemented, activated validator-trust scope and active-registry signed protocol certificate requirements.
- Separate native fixed supply from issued-asset minting; narrow unified-fee and economic-flow claims.
- Describe supplied admission fields, missing exposure/attack gates, linkedness fault-model input and reject-over-hold precedence.
- Remove automatic turnstile-freeze and “nothing else” privacy claims; separate actual disclosure tooling from archived assurance fixtures.
- Identify actual V2 signature encoding, including public keys and multiple certificate stages; label compact-byte arithmetic and old timing summaries correctly.
- Mark the complete launch manifest and replay/profile-promotion pipeline as targets, and preserve the scope of unrelated Foundation governance.
- Reconcile the later retained storage deployment receipt with README, STATUS, security/storage summaries and the operational-state page; keep earlier failures as history.
- Synchronize the whitepaper download and publish a single inventory through the CLI and MkDocs.

## Remaining work

### G01: Live Cobalt composition

**Priority: P1 — authorization and safety claims.** The scoped handoff and protocol certificate are implemented; the complete §6 genesis/checker/profile/evidence tuple is broader. `verify_cobalt_validator_trust_update` and `verify_cobalt_safety_witness` are different consumers. Map every proposed transition field and safety premise to its exact live verifier, then independently review proof composition. Close when a versioned specification and adversarial integration tests demonstrate that no target field can be omitted or self-authorized. Do not widen current scope incidentally. Related: WP-01, WP-28–37.

### G02: Launch manifest and public topology

**Priority: P1 — launch authority.** `Genesis` lacks the exact §5.4 LaunchCertificate and seven-ratifier verifier. Implement or explicitly revise the target through a separate governed specification; test identity, control-group caps, signatures and genesis binding. Close with the exact artifact, verification path and approved topology evidence. A six-node controlled receipt cannot close this requirement. Related: WP-27, WP-29, WP-60.

### G03: Admission evidence and operator independence

**Priority: P1 — membership quality.** `evaluate_validator_admission` trusts supplied evidence flags and lacks separate exposure/attack gates from the displayed predicate. `analyze_trust_graph` also requires a fault model. Bind independently verified source packets, graph/fault-model computation and exact selector policy to any binding proposal path. Close with false/stale/conflicting-source, shared-control and clean-candidate tests at that consumer plus independent-operator evidence. The economic recruitment premise remains empirical. Related: WP-02, WP-23–26, WP-32.

### G04: Replay certificates and model promotion

**Priority: P1 — claimed model authority.** Parsing a replay root does not verify an independent replay-key quorum. Keep current model output advisory/shadow-only. Specify the signer set, profile/domain/expiry bindings, dissent handling, deterministic selector and applicable promotion authority. Close with the consuming verifier and wrong-root, duplicate-key, split-result, expired-profile and absent-model tests. Cobalt's validator-trust certificate cannot substitute for replay authentication. Related: WP-04, WP-46–52.

### G05: Privacy containment and action-boundary assurance

**Priority: P1 — confidentiality and value preservation.** Turnstiles cap public withdrawal but cannot alone detect counterfeit notes draining legitimate pool value. Specify incident triggers, which action classes each pause actually blocks, and recovery authority. Complete action-specific wire/log disclosure analysis, proof/VK/public-input review, and exact registry-rotation/nullifier/restart tests. Close each subclaim with its own executable evidence; do not convert an underflow rejection into a depositor-protection guarantee. Archived metadata and assurance fixtures need identified live consumers before being presented as controls. Related: WP-07, WP-38–45.

### G06: Fees and certificate costs

**Priority: P2 — economic and resource planning.** Map each transaction family to its exact fee formula, exemption, burn destination and limits. Measure actual V2 JSON/binary encoding, repeated keys, prepare/precommit QCs and timeout ancestry across committee sizes. Close with reproducible encoded-byte and verification-work reports; the compact 24/67-signer arithmetic is not sufficient. Related: WP-09–10, WP-54–56.

### G07: Original measurement provenance

**Priority: P2 — empirical reproducibility.** E1/E2/E3/E5/E6/E7 lack complete original measurement packets in the pinned public tree. Search scope included tracked `reports/`, `benchmarks/`, `docs/evidence/`, the old conformance matrix, cited paths and exact E5/E6 digests. Absence here does not prove the experiments never occurred. Recover a redacted immutable archive or rerun under a named profile. Close each entry with inputs, code/runtime/hardware identity, raw outputs, exact verification command and matching digest. Keep E5/E6/E7 experiments distinct; summary differences alone do not prove contradiction. Related: WP-05, WP-51, WP-56, WP-63–65, WP-67–69.

### G08: NAV and external-route claims

**Priority: P2 — reserve perimeter and deployment scope.** Reserve proofs bind the configured profile and disclosed evidence, not undisclosed liabilities or custody solvency. Keep source-specific routes, valuation units, attestations, finalization and external activation visible in NAV/bridge documents. Close route-specific claims with pinned public-input/verifier evidence, conservation/replay results and the exact deployment/activation packet. No route is enabled by this audit. Related: WP-13.

### G09: Censorship and unavailable validators

**Priority: P2 — liveness/accountability targets.** Production first-seen receipt aggregation, omission attribution and automatic Negative-UNL suspension are not established. Either retain these as non-claims or implement separately versioned receipt and suspension protocols with historical-threshold, expiration, replay and malicious-proposer tests. A timeout certificate recovers proposer progress only while a normal quorum remains available. Related: WP-19, WP-21–22, WP-59.

### G10: Cryptographic recovery and custody

**Priority: P1 before real value; otherwise P2 research.** Current transparent/validator authorization uses ML-DSA-65; no alternate SLH-DSA recovery family or post-quantum note KEM is implemented. Define commitments, activation, migration and failure procedures before promising independent crypto recovery. HSM/remote signing and external cryptography review remain release gates. Close only with the actual verifier/custody path, migration tests and review, not a reference to a standard. Related: WP-07, WP-45, WP-53, WP-57–59.

### G11: Operational freshness and gate reconciliation

**Priority: P2 — operator decisions.** The retained August 31 receipt records storage activation/continuation through height 931, superseding the older undeployed-candidate summaries. This audit did not query services or verify a new release. Reconcile the active storage/testnet gate journal against its exact receipts without inferring that every old gate passed; obtain a fresh authenticated fleet observation before any “running now” claim. Close with dated fleet/binary identities and explicit disposition of unresolved gates. The existing snapshot-export issue and pending public-testnet decisions stay open unless separately verified. Related: WP-72.
