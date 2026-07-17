# PostFiat Whitepaper-to-Code Conformance Matrix

**Canonical candidate reviewed:** `docs/whitepaper.md`, Version 3, June 2026
**Discovery baseline:** `4b5af7bc6bb6e793ed8a60219d13d6d35be03058`
**Status:** STEP 1 baseline preserved; final P0/P1 dispositions are reconciled in
`OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md` and its closure table.

> The detailed tables below preserve what the audit found at the discovery
> baseline and the action each mismatch required. Phrases such as “pending” or
> “implement” in those tables are historical finding text, not the current
> blocker register. The authoritative final state is 34 fixed-candidate rows and
> one external source-publication blocker: `P0-SECRET-01`'s private
> provider-owner revocation/decommission record. Independent specialist review,
> HSM deployment and multi-region production drills are real-value launch gates,
> not prerequisites to make the source repository public.

The repository contains several older papers and business/research documents. For this review, only `docs/whitepaper.md` is treated as the candidate protocol whitepaper because it labels itself Version 3 and describes the current Rust system. That choice is not silent: all competing canonical-looking papers must receive an archived/superseded banner before publication.

Status vocabulary:

- **Conforms:** production code and tests implement the material claim.
- **Partial:** meaningful code exists, but the claim is broader than current implementation/evidence.
- **Contradicted:** reachable production code behaves differently.
- **Not implemented:** the named protocol artifact/path was not found.
- **Evidence missing:** the code may implement the claim, but the cited public evidence is absent or inadequate.

## 1. Foundational claims

| Whitepaper claim | Status | Code/evidence | Required publication action |
|---|---|---|---|
| Known validators, fixed native supply, fee burn, no validator rewards (§Abstract, §3) | Conforms for native PFT; external issued representation disabled | The exact native supply is genesis-hash bound, legacy replay enforces the same constant, the replay/checkpoint oracle proves live holdings plus cumulative burns, and no validator reward loop was found. EVM PFTL mint release remains undeployable without a concrete verifier/code-hash policy. | Retain the native invariant in release CI and keep the separate external issued representation disabled until its verifier is pinned. |
| Validator-list evolution is protocol state governed by authorized transitions (§Abstract, §1) | Implemented locally; integrated fault gates pending | Unsigned legacy governance rejects live. The candidate requires distinct ML-DSA-65 old-registry authorizations, delayed activation and committee-domain-separated consensus safety/QC state; real n=4/n=6 amendment and key-rotation transport/replay pass. Cobalt RBC/ABBA are signed research primitives, not the authoritative node admission path. | Complete concurrent/partition/crash/rollback gates and retain the exact implemented signed-batch boundary. |
| Shielded settlement is the baseline path (§Abstract, §1, §3) | Corrected / partial | Asset-Orchard is the supported private path; legacy cleartext Mint/Spend is rejected live and retained only for historical replay. Transparent settlement remains supported. | Describe Orchard as the supported privacy class and preserve the exact leakage statement. |
| Authorization is post-quantum from genesis (§Abstract, §1, §9) | Partial | ML-DSA-65 signs account and validator artifacts. Orchard note spend authorization and Ethereum bridge contracts retain classical components; the paper acknowledges some classical privacy assumptions. | Publish a complete primitive/key-purpose matrix and correct any blanket implication. |
| Fail closed on missing/stale/conflicting/oversized evidence (§1.1) | Conforms for remediated live boundaries, broader proof pending | Unsigned governance, unsigned owned wrapping, asserted external bridge transitions, oversized RPC/artifact/storage input, and rejected receipt paths now fail closed. | Keep integrated adversarial and replay gates as publication evidence. |
| Old rules validate new rules (§1.1) | Implemented locally; integration pending | Validator changes are authorized by signatures verified against the pre-transition registry and take effect only after the authorizing block. | Complete the adversarial reconfiguration and rollback campaign before candidate closure. |
| Hash-bound governance artifacts (§1.1) | Implemented locally | The complete amendment/registry payload and lifecycle domain are bound into each ML-DSA-65 authorization and rechecked at proposal, apply and replay. | Freeze encodings and publish conformance vectors on the immutable candidate. |
| Deleting model classification can only produce more holds (§1.1, §8) | Partial | Governance-agent work is largely evidence/dry-run tooling; no complete proof over every live selector input and fallback was found. | Add a no-model equivalence/property test or narrow the claim to current dry-run tooling. |

## 2. Threat model and ledger model

| Whitepaper claim | Status | Code/evidence | Required publication action |
|---|---|---|---|
| Safety tolerates `f=floor((n-1)/3)` Byzantine validators (§2) | Locally remediated; integration pending | Production now permits one durable vote per validator and height and rejects nonzero views, so conflicting direct-commit certificates require an honest validator to violate its persistent lock. | Complete integrated crash/replay/adversarial gates for `P0-CONSENSUS-01`. |
| Safety is asynchronous and timing-independent (§2) | Narrowed to safety-only view-zero direct commit | A durable one-vote-per-height lock plus distinct BFT certificate protects agreement; nonzero views halt. Asserted external bridge transitions are disabled live. | Do not claim liveness under asynchronous proposer failure; retain crash/replay/model gates. |
| Cryptographic assumptions cover the implemented hybrid system (§2) | Corrected / conforms to inventory | The candidate now names ML-DSA, SHA3, RedPallas, ChaCha20-Poly1305, Halo2 and SP1/Groth16 and explicitly labels the classical/PQ boundary. The complete code-derived inventory also covers Sinsemilla/Poseidon, ZIP-32 and secp256k1/Ethereum. | Keep `OPEN-SOURCE-CRYPTOGRAPHY-INVENTORY-20260716.md` synchronized and require specialist review before real value. |
| Replicated ledger/state domains are explicitly enumerated (§3) | Conforms to the corrected candidate | Section 3 now names accounts, issued assets/trustlines, escrow/NFT/offer, NAV/profiles, disabled external-handoff history, FastLane/FastSwap, Asset-Orchard, bridge, history and governance/registry domains, plus cache exclusion and state-v2 migration. | Keep the exhaustive state-root field inventory and migration tests synchronized with every new replicated field. |
| Native supply is fixed and fees only burn (§3) | Conforms locally for every enabled native custody lane | `P0-NATIVE-SUPPLY-01` binds the genesis constant for new and legacy replay; per-block replay and checkpoint-v2 prove account, escrow, offer, owned, FastLane and Orchard holdings plus cumulative burns equal genesis. EVM issued-asset release is separately disabled unless a bound verifier authorizes it. | Preserve the replay/checkpoint oracle; finish external issued/backed-supply proof only before enabling that separate route. |
| Fee classes separately price bytes, signatures, shielded actions, and registry work (§3) | Partial | Multiple fee calculators/resource policies exist, but registry/governance and every proof class are not shown to share the stated governed class model. | Map every family to an exact consensus fee formula and governance authority. |
| Every block uses exactly one active registry root (§3) | Conforms for the fixed-genesis live protocol | Headers, proposals, votes and certificates bind the one active root. Unsigned transitions are rejected live; signed registry evolution is explicitly future work. | Keep transition code unreachable until old-registry signed authorization and cross-boundary simulation exist. |

## 3. Consensus, ordering, and accountability

| Whitepaper claim | Status | Code/evidence | Required publication action |
|---|---|---|---|
| Leader is `validators[(height+view) mod n]` (§4.1) | Conforms for current helper | `ordering_fast::leader_for_view` is used by node finality. | Retain with cross-node conformance vectors. |
| Proposal binds parent, payload, state root, registry root, proposer (§4.1) | Conforms locally | The corrected claim matches the view-zero proposal schema; no justify-QC is claimed, and field/domain mutation plus finality regressions pass. | Preserve the integrated workspace and replay gates. |
| QC requires `floor(2n/3)+1` canonical distinct votes (§4.1) | Partial/conforms per certificate verifier | Threshold and distinct vote validation exist for block certs; complete certificate-type inventory is pending. | Cross-certificate quorum/dedup test matrix. |
| Timeout certificate safely advances views (§4.1) | Not claimed / disabled | Timeout artifacts remain diagnostic only; every nonzero-view production proposal fails closed. | Do not enable until verified QC ancestry and a safe lock rule are implemented. |
| Conflicting proposal/vote evidence is retained (§4.1) | Partial | Equivocation evidence helpers/tests exist; retention, governance consumption and operator evidence lifecycle are not end-to-end proven. | Add persistence/restart/reconfiguration and governance-consumption tests. |
| Verified view-zero certificate commits its proposal directly (§4.1) | Conforms locally | This is the implemented and documented production rule; cross-view progress is refused and the safety counterexample/regression suite passes. | Preserve the complete workspace and crash/replay evidence on the publication candidate. |
| Included transactions use fixed family and insertion order (§4.2) | Conforms | `create_mempool_batch` drains the documented family vectors in fixed order and preserves admitted insertion order. | Retain deterministic conformance tests; do not infer MEV resistance. |
| Threshold admission-receipt aggregates and omission evidence (§4.3) | Research-only, correctly labeled | Types exist in `ordering_fast`; no production node authority is claimed. | Implement before making censorship-attribution claims. |
| Automatic availability suspension (§4.4) | Research-only, correctly labeled | No production automatic Negative-UNL mechanism is claimed; registry change requires future signed governance. | Implement full protocol before enablement. |

## 4. Admission, launch, Cobalt, and governance

| Whitepaper claim | Status | Code/evidence | Required publication action |
|---|---|---|---|
| Executable validator admission selector with fail-closed fixtures (§5.3) | Partial | Extensive governance-agent schemas/fixtures/tests exist, but evidence collection and selector decisions are not themselves sufficient signed on-chain authority. | Separate “decision support/dry run” from live governance and prove the live authorization boundary. |
| Genesis `LaunchCertificate` with roots, ratifier set, signatures, >=7 ratifiers and control-group constraints (§5.4) | Not implemented as stated | Genesis bundle/operator manifests exist; exact `LaunchCertificate` type and seven-ratifier enforcement were not found. | Implement and migrate genesis artifact or label it a future launch requirement. |
| Genesis ratifiers have no override opcode (§6.1) | Partial | No explicit founder override opcode found in execution, but local direct-state/operator commands and unsigned governance break the stronger authority claim. | Remove direct live mutations and close governance authorization. |
| Transition packet binds parent/next roots, evidence, checker/profile, challenge, activation/expiry, governance cert (§6.2) | Partial | Many Cobalt structures/root checks exist; complete signed governance certificate is absent. | Add signed old-rule certificate and one canonical transition type. |
| Local Cobalt inequalities, linkedness, old/new intersection and bounded cover (§6.3–6.7) | Implemented in library/partial production | `consensus_cobalt` contains checks and tests; production authority/lifecycle composition is not proven, and RBC/ABBA signature verification is incomplete. | Signed message verification, adversarial transition model, production call-path proof. |
| Proposer cannot omit unfavorable cover rows (§6.6) | Partial | Derived cover checks exist in the library. | Fuzz/rooted-graph differential tests and proof production uses the exact extractor. |
| Emergency actions are old-registry quorum signed (§6.9) | Future protocol, not live | Unsigned emergency/governance actions are rejected live; no signed recovery mechanism is claimed as present. | Implement the signed old-registry recovery state machine before enablement. |

## 5. Shielded settlement

| Whitepaper claim | Status | Code/evidence | Required publication action |
|---|---|---|---|
| Asset/value/owner/memo hidden for supported private Asset-Orchard spends (§7.1) | Corrected / conforms at the action boundary | Private swaps hide note openings, raw assets/values/owners/recipients and price. Ingress v2 reveals the signed public burn asset/amount plus commitment/ciphertext; private egress reveals its public destination/asset/amount. Legacy cleartext Mint/Spend and ingress v1 are unreachable live and replay-only. | Complete wire/log/API leakage scans on the release candidate and preserve live-v1 no-mutation regressions. |
| Spend proves membership/nullifier/value/outputs/fee; consensus verifies rather than proves (§7.1–7.2) | Corrected / partial | Orchard/Halo2 verification and nullifier/anchor/accounting checks exist. The paper now discloses that prover tooling/service code shares the workspace and requires deployment separation instead of claiming its absence from node code. | Complete the circuit/public-input/VK/parameter audit and prove validator-service deployment excludes proving work. |
| Per-block action cap before verification (§7.2) | Partial/conforms | action/proof resource limits exist and CI no longer skips Orchard suites. | Retain adversarial cost and full privacy tests as required gates. |
| Asset-Orchard authorization binds the complete action without a claimed ML-DSA outer envelope (§7.3) | Corrected / conforms to current type boundary | RedPallas signatures cover a chain/genesis/protocol-bound action sighash; proof and binding hashes cover the public statement; `ShieldedActionBatch` contains only `batch_id` and `actions`; ML-DSA authenticates validator proposal/certificate inclusion. | Preserve the semantic regression and do not reintroduce registry/disclosure-envelope claims unless a signed type and verifier are implemented. |
| Turnstile bounds pool withdrawals by deposits (§7.4) | Partial/conforms | turnstile state and issued-asset ingress/egress accounting exist with tests. | Integrated per-asset conservation, counterfeit-proof negative control and replay oracle. |
| Registry rotation and shielded state commit atomically (§7.5) | Partial | Both are included in ordered block state/roots, but consensus/governance P0s prevent accepting the safety claim. | Close P0s and test boundary nullifier replay under rotation/rollback. |
| Disclosure is holder-controlled and public leakage is narrow (§7.6) | Partial | disclosure APIs and encrypted outputs exist; public RPC/log/evidence captures and thin-set/timing behavior need end-to-end validation. | Privacy threat-model tests and remove raw wallet evidence. |

## 6. Machine classification

| Whitepaper claim | Status | Code/evidence | Required publication action |
|---|---|---|---|
| No model runs inside consensus (§8.1) | Conforms by source inspection | Model invocation is operational/evidence tooling; consensus consumes committed records. | Keep an architectural dependency test preventing model/network crates in execution/consensus. |
| Closed schemas, citations, roots, deterministic selector (§8.2) | Partial | Schemas, validators, fixtures and dry-run records are extensive. | Inventory which artifacts are consensus-enforced versus research evidence; remove present-tense claims for non-live gates. |
| Replay certificates require independent signer quorum (§8.3) | Partial/unclear | Replay evidence and schemas exist; one canonical live cryptographically verified replay-certificate path was not established in this audit. | Code-path and signature/threshold/replay tests or claim correction. |
| Profile replacement is a Cobalt transition (§8.6) | Partial | Governance amendment kinds and dry-run records exist; unsigned governance prevents the stated authority. | P0-GOVERNANCE-01. |

## 7. Cryptography and recovery

| Whitepaper claim | Status | Code/evidence | Required publication action |
|---|---|---|---|
| ML-DSA-65 is default account/validator authorization (§9.1) | Conforms for enabled account/block paths | `crypto_provider` uses `fips204` ML-DSA-65 with purpose contexts; normal transactions and block votes verify it. A blocking call-site policy freezes all 46 default-context and deterministic-seed uses, and the canonical-encoding inventory maps every enabled transcript family. | Keep plaintext file signing behind the exact unsafe-devnet acknowledgement; remote signer/HSM remains a real-value launch requirement. |
| Certificates identify validators and bind registry roots (§9.1) | Partial/conforms | Block vote/certificate structures carry validator ID and registry root. | Prove for every certificate family and transition boundary. |
| Detached certificate sizes/performance (§9.2) | Evidence missing for publication | Arithmetic is plausible; referenced raw reports are not part of a clean reproducible public evidence package. | Re-run on release candidate and publish scripts, environment, signed report/hash. |
| SLH-DSA recovery commitments (§9.3) | Explicit future work | No SLH-DSA path is implemented and the candidate whitepaper no longer claims otherwise. | Implement and audit before adding a present-tense claim. |
| Emergency crypto recovery is precommitted (§9.3, §10) | Not claimed as current | The corrected recovery section distinguishes target mechanisms from implemented controls. | Add only with signed governance and recovery-key tests. |

## 8. Failure recovery and evidence register

| Whitepaper claim | Status | Code/evidence | Required publication action |
|---|---|---|---|
| Every failure has a precommitted state-machine fallback (§10) | Claim removed/narrowed | FastPay payments are available by default but bounded safe cancellation remains unresolved; asserted bridge transitions remain disabled; governance now has signed admission but still needs its complete crash/rollback campaign. | Maintain an exact per-feature recovery table rather than a blanket guarantee. |
| Availability collapse halts safely (§10) | Conforms for current direct-commit containment | Under-quorum certificates reject, nonzero-view progress rejects, and the node halts rather than signing around the durable height lock. | Integrated partition/restart tests remain required release evidence. |
| Evidence [E1]–[E8] backs only named empirical claims (Appendix A) | Evidence missing/inconsistent | Some reports referenced under `reports/` are absent from the tracked public candidate; raw evidence is instead duplicated under `docs/evidence`; reproduction manifests/hashes are not consistently linked. | Create a minimal redacted evidence manifest and external immutable archive; rerun release measurements. |
| Controlled evidence is not production proof (§11) | Text conforms; repository presentation inconsistent | The caveat exists, but SECURITY/README/docs make stronger current claims and raw devnet artifacts dominate the tree. | Add one maturity matrix/banner and remove contradictory claims. |

## 9. Additional implementation detail beyond the whitepaper's protocol-level treatment

The corrected candidate now names these systems in §3 and describes the
privacy and authorization boundaries in §§7 and 9, but it deliberately does
not turn every implementation detail into a protocol guarantee:

- 36 issued-asset/NAV/vault bridge/PFTL-Uniswap operations;
- W6 dual-auth atomic swaps;
- FastPay owned-object payments and signed unwrap;
- FastLane primary deposits/redeems/checkpoints/control;
- FastSwap certificate settlement;
- escrow, NFTs, offers and deterministic matching;
- SP1/Groth16 NAV proof verification;
- wallet/proxy/API/prover custody and trust boundaries;
- external Ethereum contracts and route operator authority.

The publication boundary is explicit: externally asserted bridge transitions,
unsigned registry evolution, FastPay owned mutations, debug proofs and
production file-key/JSON-storage modes are disabled or require an exact unsafe
development acknowledgement. The listed enabled transaction families remain
covered by the transaction, state commitment, fee, replay, privacy and
authorization invariants; specialized feature specifications remain supporting
documents rather than contradictory canonical papers.

## 10. Required whitepaper release gate

The whitepaper and public source candidate can ship only when:

1. every contradicted P0 claim is fixed in code first;
2. every P1 mismatch is implemented or rewritten honestly;
3. all transaction/state/certificate/crypto/recovery systems in the public binary appear in the protocol and threat model;
4. every retained empirical number is reproduced on the final revision and linked to a redacted hash-bound artifact;
5. each matrix row is self-contained and traceable to public code and tests; an
   independent review is encouraged but is not a source-publication gate;
6. superseded papers are labeled and cannot be mistaken for current protocol truth.

The final closure table and acceptance manifest prove these repository-controlled
conditions. Production HSM custody, independent specialist audits, multi-region
fault drills and independently signed builds remain mandatory before real value.
