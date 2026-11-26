# Feedback — Pillar 4 — The NFT System

*Review round 3.*

> **Mean score:** 68/100 · Gemini 4 Neon 64 · Claude Opus 5.5 72
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/04-nft-system.md), once, with the question below. Run 2026-11-26 22:31 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 64/100

*Model `google/gemini-4-neon` · 23824 output tokens*

#### Summary judgement

This sub-page is unusually well grounded in the actual Post Fiat repositories (`postfiatl1v2`, `tasknode`, `postfiatorg.github.io`) and captures the founder's headline beats for Deliverable 4: migrating or pinning historical NFTs, publishing an intelligible post-quantum NFT spec and blog post, integrating with the Pillar 2 message protocol, fixing real chain scaling bottlenecks, and testing adversarially.

However, it is **not ready to build as written**. Beneath its polished structure lie several severe cryptographic and protocol defects—most notably a temporal impossibility in the old-chain evidence archive (R10), a missing claim-window enforcement check in explorer verification (R17) that breaks the post-quantum security argument, a direct contradiction between burn recovery (R8) and explorer verification (R17), and incomplete chain-scaling fixes (R18) that leave $O(N)$ bottlenecks intact. Economically, the spec is barely sketched: it leaves 14 foundational decisions as open questions with no recommended defaults, fails to close a trivial permanent-state-bloat attack (and proposes a refundable-reserve option in Q13 that would actually worsen it), and leaves unsolicited unburnable NFT griefing and IPFS pinning costs unresolved.

With targeted engineering revisions to the provenance/verification protocol, chain state model, and economic defaults, this can become a strong execution spec. As it stands, building it literally would produce broken verification, unworkable archiving, and a spam-vulnerable ledger.

---

#### Correspondence to the founder's intention

The sub-page aligns well with the *explicit checklist* in Deliverable 4 and the broader prompt:
- **Historical NFT migration & pinning:** Directly addresses the founder's Task Node NFTs on the old XRPL-fork chain (and appropriately asks in Q1 about XRPL mainnet / third-party issuers), providing both full migration and a "pin instead" record option.
- **Post-quantum spec & blog post:** Commits to `docs/specs/nft-v1.md`, test vectors, a blog post, and an honest post-quantum boundary table that calls out classical-key claim windows as the weak link.
- **Message protocol contiguity:** Models metadata, claims, and provenance records as `pf.msg.v1` Pillar 2 protobuf envelopes committed via SHA3-384.
- **Adversarial & staged approach:** Includes R20 multi-model adversarial review and state-machine model checking, plus a staged rollout starting with a read-only inventory and devnet dry run.

Where it falls short of the founder's intention:
1. **"Supports NFTs in a smart way" is thin and largely deferred.** The founder made smart NFT support the #1 readiness gate for moving to the new chain, and emphasized machine-native communication, X402 integration, and local Task Node parity across deliverables. This spec treats NFTs mostly as static migrated profile pictures, relegates credentials/gated-content to Open Question 10, leaves `media[]` encrypted-payload semantics undefined, and never explains how agents consume NFTs programmatically or how Task Node's active profile avatar (`/select`) stays unbroken during migration.
2. **Decision overload instead of opinionated leadership.** Handing the founder 14 unranked open questions—many of them low-level protocol hygiene issues like `ISSUER_BURNABLE` (Q11), unsolicited locked spam (Q12), and reserve mechanics (Q13)—with **no recommended defaults** forces the spec to carry bifurcated designs everywhere (`Task Node issuer` vs. `User as issuer`; `Burn-first` vs. `Snapshot` vs. `Pin-only`) and hides schedule conflicts (e.g., Snapshot mode cannot execute until Stage 4's old-chain freeze, conflicting with using NFTs as an earlier migration gate).
3. **Local Task Node (Pillar 5) implications are under-examined.** Deliverable 5 states the local Task Node should do "virtually everything the task node can do." Under the central `Task Node issuer` option, local Task Node users remain dependent on a central server to mint profile NFTs; that architectural tradeoff is never surfaced in the issuer comparison table.

---

#### Technical robustness

The sub-page deserves real credit for inspecting the code and catching genuine pre-existing chain and wallet bugs: `Vec` linear scans with full-ledger `validate_nft_state` re-checks on every transaction, `ISSUER_BURNABLE` being stored but ignored in execution, `wallet_wasm` lacking NFT signing, randomised ML-DSA-65 signing requiring claim persistence before hashing, and no memos on NFT transactions. Binding `pf.nft.claim.v1` to an exact target `nft_id` (preventing cross-issuer front-running) is also well designed.

Nevertheless, the protocol has **five serious technical defects**:
1. **The offline-verifiable evidence archive (R10) cannot work as specified.** At retirement time, the final old-chain freeze ledger hash does not yet exist, so it is impossible to pin a skip-list link to it in `evidence_cid` before minting. The alternative—pinning XRPL validator validations—fails because XRPL RPCs do not archive ephemeral P2P validation messages, verifying them requires historical UNLs, and classical validator signatures become forgeable post-quantum without a trust anchor on PFTL. Furthermore, XRPL skip-lists do not span arbitrary ledger distances in a single hop, and allowing full ledger transaction sets can violate R2's 1 MiB cap.
2. **Explorer origin verification (R17) undermines the post-quantum claim window (R11).** R17 never checks that the mint occurred before the claim window closed. In snapshot mode (or recovery using off-chain method (ii)), once a quantum attacker later breaks an old classical key, they can forge an off-chain claim for any unclaimed old NFT, mint under a user issuer, and pass all five R17 checks as `checked`.
3. **Burn recovery (R8) contradicts explorer verification (R17).** If a user burns with a wrong memo or loses the claim, R8 recovers via a fresh claim proved by method (ii) or (iii). Yet R10 does not archive method (ii)/(iii) proofs, and R17 demands a burn transaction *bound to the claim*, which permanently flags every recovered NFT as `not checked`.
4. **R18 chain-scaling fixes are incomplete.** Replacing `Vec` with maps by `nft_id` and `(issuer, collection)` still leaves: (a) the full-ledger clone per NFT transaction mentioned in `Chain semantics`, (b) `account_nfts` as an $O(N)$ scan because there is no `owner` index, (c) no requirement for deterministic `BTreeMap` ordering across validators, and (d) no coordination plan for a consensus-breaking state format change relative to Pillar 3 validator onboarding.
5. **Cross-issuer earliest-wins (R17) creates an explorer DoS surface and centralizes verification.** Requiring the explorer to fetch and verify IPFS envelopes and evidence bundles for *every* NFT minted by *any* issuer on chain makes the indexer trivially DoS-able for pennies, while `NftDefinition` omits `initial_owner` and `mint_height`, forcing wallets and local nodes to depend on the central explorer indexer rather than RPC state.

Additional inconsistencies exist around R17's image-CID check (breaks hand-mapped *Needs review* items and depends on unarchived original metadata JSON), R12 provenance records (contradict R7's retirement prerequisite, lack display/media schema, and permanently block re-recording on ownership change due to `duplicate_nft`), and R2 conflating deterministic protobuf body encoding with randomised ML-DSA-65 envelope bytes.

---

#### Economic coherence

This is the weakest section of the document. The `Economics` section is four sentences long and delegates nearly every economic parameter to Open Questions 4–7 and 13 without analysis or recommendations.

1. **Catastrophic permanent-state-bloat pricing (and a broken reserve proposal).** At ~33 atoms per mint (`1 PFT = 1,000,000 atoms`), **1 PFT buys ~30,300 NFTs** (~15 MB of permanent, unprunable state on every validator because burned NFTs stay in state forever). Worse, Open Question 13 floats "a per-NFT reserve refunded on burn"—which, combined with permanent burned tombstones, would allow **infinite free state bloat** (mint → burn → reclaim reserve → repeat).
2. **Unsolicited unburnable NFT griefing.** Anyone can mint a `BURN_LOCKED | TRANSFER_LOCKED` NFT into any user's or validator's account for 0.000033 PFT, permanently polluting their ledger state and `account_nfts` output. Leaving "wallet filtering only, leaving locked junk on chain" as an open option (Q12) is economically and operationally unsafe.
3. **Unresolved IPFS pinning economics and abuse limits.** The security model assumes Task Node's two-replica cluster plus Pinata, while Q7 asks who pins long-term. There is no cost model, quota/rate-limit, or incentive mechanism for pinning user-issued NFTs, historical evidence bundles, or third-party collections—nor protection against attackers using Task Node/explorer pinning endpoints as free storage.
4. **Fixed-PFT issuer fee fragility.** Because `postfiatl1v2` only supports a fixed native PFT transfer fee charged on *every* transfer (including self-transfers), a non-zero fee punishes wallet rotation/cold storage and becomes economically broken if PFT appreciates sharply (effectively freezing transfers). The spec should explicitly warn against this and recommend `0` for profile/migrated NFTs.
5. **Migration funding gaps on both chains.** New-chain minting requires funded destination accounts (`missing_owner`), and old-chain burns / `AccountSet` proofs require spendable old-chain fees. Neither is solved—`Funded new-chain accounts` is simply listed as "Funding policy open"—even though sponsoring the entire user cohort's migration fees would cost negligible PFT.

---

#### Problems

##### P1 — R10 Evidence Archive is temporally impossible, relies on unarchived XRPL validations, and lacks a trust anchor
- **Severity:** critical
- **Where:** `Technical requirements → R10. Evidence archive`; `Security model`; `Acceptance criteria #9`
- **What is wrong:** R10 requires pinning `evidence_cid` *at retirement* (so the mint commits to it) containing either "the old validators' validations" or "a skip-list hash link to the final ledger hash published at the freeze." First, the final freeze ledger hash does not exist at retirement time (Stages 2–3 vs. Stage 4), making a skip-list link to it impossible to include at mint. Second, XRPL's `LedgerHashes` skip-list only spans bounded hops (256 recent + 256-step flag ledgers up to ~65,536 ledgers), not arbitrary distances. Third, XRPL archive RPCs do not store ephemeral P2P validator validation messages, and verifying historical validations requires a historical UNL and relies on classical signatures that fail post-quantum. Fourth, allowing "the ledger's transaction set" can easily exceed R2's 1 MiB cap, and R10 omits how `NFTokenPage` owner deletion is proven from XRPL metadata.
- **Why it matters:** Offline provenance verification after the old chain is retired (User Story step 7, AC #9) is completely unimplementable as written, and without a PFTL-anchored trust root, post-quantum attackers or rogue issuers can fabricate old-chain ledger headers.
- **How an agent should investigate:** Inspect `pftl-history-rpc-and-archive-wss.md` and test the live old-chain RPC (`ledger`, `tx`, `ledger_entry`, `AccountObjects`, SHAMap proof availability / `proof` RPC support). Check `postfiatl1v2` for any existing checkpoint/oracle/governance anchoring mechanism.
- **Suggested direction for fix:** Anchor validated old-chain ledger hashes on PFTL (e.g., periodic checkpoint messages/records signed by governance/validators before minting, or a final freeze checkpoint anchored on PFTL before verification closes). Mandate compact **SHAMap Merkle inclusion proofs** (tx+meta in `tx_hash`, or `NFTokenPage` in `account_hash`) with a strict byte cap, never full transaction sets, and explicitly specify how the verifier checks `tesSUCCESS`, absence of `Owner` override, and the upper 160 bits of the deleted/modified `NFTokenPage` index.

##### P2 — R17 Explorer origin verification never checks the claim window close, breaking post-quantum security
- **Severity:** critical
- **Where:** `Technical requirements → R11. Claim window`, `R17. Explorer NFT view`; `Security model`
- **What is wrong:** R11 closes the claim window so classical-key compromises cannot mint valid claims later. However, R17's five-point `checked` algorithm never checks that the new-chain mint occurred *before* the claim window closed (`mint_block_height <= claim_window_close_height`).
- **Why it matters:** Under snapshot mode (R7b) or recovery via off-chain Pillar 6 signatures (R6(ii)), a future quantum attacker who recovers an old classical private key years after the window closes can forge a valid off-chain signature for any unclaimed old NFT, mint it on PFTL (under their own user issuer, or a compromised issuer), and R17 will mark it ***checked***. This invalidates the core post-quantum claim in R3 and the Security Model table.
- **How an agent should investigate:** Trace all five checks in R17 against an adversary with a quantum computer operating at `T > claim_window_close_height` for both `Task Node issuer` and `User as issuer` settings, across burn-first, snapshot, and recovery flows.
- **Suggested direction for fix:** Add a mandatory check to R17: the PFTL mint transaction must be included at `block_height <= claim_window_close_height` (a published, immutable constant/checkpoint on PFTL), AND the old-chain evidence must anchor to a ledger hash checkpointed on PFTL before window close. Also record `mint_block_height` on-chain or require the indexer to enforce it.

##### P3 — R8 Burn recovery contradicts R10 and R17, leaving all recovered NFTs marked `not checked`
- **Severity:** critical
- **Where:** `Technical requirements → R6`, `R8. Burn-first state machine`, `R10`, `R17`; `Acceptance criteria #6`
- **What is wrong:** When a burn has a wrong memo or lost claim, R8 recovers by having the holder sign a fresh `pf.nft.claim.v1` and prove old-wallet control with method (ii) (off-chain Pillar 6 link) or method (iii) (`AccountSet` tx with new `claim_hash`). Yet R10 only archives the burn tx (which still has the wrong/missing memo), and R17 requires the evidence bundle to show "a holder-signed retirement bound to the claim."
- **Why it matters:** Every NFT rescued via R8 recovery will fail R17 and permanently display as ***not checked***, directly violating Acceptance Criterion #6 and #9.
- **How an agent should investigate:** Walk through the exact artifact set produced in each R8 recovery branch (wrong memo, lost claim, post-burn outage) and test each against R10's schema and R17's verification predicates.
- **Suggested direction for fix:** Extend `pf.nft.evidence.v1` (R10) to include an optional `recovery_proof` containing either the verified method (ii) classical signature + public key (matching the burner's XRPL account ID) or the method (iii) `AccountSet` tx + SHAMap inclusion proof. Update R17 so `checked` accepts either direct burn-memo binding OR (holder-signed burn + valid `recovery_proof` minted before window close).

##### P4 — Permanent state bloat for ~0.000033 PFT and an infinite-bloat bug in Open Question 13
- **Severity:** critical
- **Where:** `Chain semantics`; `Economics`; `Technical requirements → R19`; `Open questions → #13`
- **What is wrong:** Minting an NFT costs ~33 atoms (0.000033 PFT), yet burned NFTs stay in ledger state forever (`duplicate_nft` tombstones). For 1 PFT, an attacker can permanently write ~30,300 NFTs (~15 MB) into every validator's consensus state. Worse, Open Question 13 proposes "a per-NFT reserve refunded on burn" without changing tombstone retention—which would let an attacker mint, burn, reclaim the reserve, and repeat indefinitely to create unbounded permanent state for near-zero cost.
- **Why it matters:** Permissionless NFT launch under these fee parameters creates a trivial, low-cost chain-halting state bloat vector, failing the "production-ready new chain" gate.
- **How an agent should investigate:** Inspect `crates/execution/src/fees_offer_planning.rs`, `crates/execution/src/entrypoints.rs`, and `crates/types/src/market_nav_asset_types.rs` to measure exact serialized byte size of `NftDefinition` and tombstones, current fee constants, and account reserve mechanics.
- **Suggested direction for fix:** Make a firm recommendation instead of an open question: (a) charge a **non-refundable state-expansion burn fee** scaled to stored bytes (covering permanent tombstone cost), OR (b) if using a refundable per-NFT reserve, **delete burned NFTs from state** and prevent ID reuse via a per-(issuer, collection) monotonic counter rather than permanent tombstones. Quantify the bytes-per-PFT spam cost explicitly in `Economics`.

##### P5 — R18 scaling fixes omit full-ledger cloning, `owner` indexing, deterministic map ordering, and upgrade coordination
- **Severity:** critical
- **Where:** `Chain semantics`; `Technical requirements → R18. Scale`; `Dependencies → Build order`
- **What is wrong:** `Chain semantics` notes that "Each NFT transaction copies the ledger and runs `validate_nft_state`." R18 fixes the `validate_nft_state` scan and adds maps by `nft_id` and `(issuer, collection)`, but: (1) never requires eliminating the per-transaction full-ledger clone (which keeps every NFT tx $O(N)$ even with maps); (2) omits an `(owner, nft_id)` index, leaving `account_nfts` an $O(N)$ full-chain scan; (3) does not specify deterministic ordered maps (`BTreeMap`) required for consensus state hashing; and (4) does not flag that changing ledger NFT state structures is a consensus-breaking upgrade relative to Pillar 3 validator rollout.
- **Why it matters:** Shipping R18 as written will fail stress benchmarks under realistic spam, leave `account_nfts` RPCs trivially DoS-able, and risk consensus divergence or uncoordinated forks across validators.
- **How an agent should investigate:** Read `crates/execution/src/nft_escrow_asset_execution.rs`, `crates/execution/src/entrypoints.rs`, `crates/execution/src/issued_asset_ledger_helpers.rs`, and `crates/node/src/rpc_dispatch.rs` to confirm how ledger state is cloned/mutated, serialized, hashed, and queried for `account_nfts` / `issuer_nfts`.
- **Suggested direction for fix:** Expand R18 to mandate: (1) transactional delta/copy-on-write or in-place mutation with rollback instead of full-ledger cloning; (2) `BTreeMap` / `BTreeSet` indices by `nft_id`, `(issuer, collection, serial)`, and `(owner, nft_id)`; (3) cursor pagination backed by those ordered indices; (4) explicit scheduling as a pre-mainnet / coordinated protocol upgrade synced with Pillar 3.

##### P6 — 14 open questions with no recommended defaults paralyze architecture and hide schedule conflicts
- **Severity:** major
- **Where:** `Technical requirements → Issuer settings`; `Open questions for the founder` (#1–#14); `Dependencies → Build order`
- **What is wrong:** The spec defers 14 decisions—including issuer model (Q3), retirement mode (Q2), claim window (Q8), unsolicited NFTs (Q12), spam pricing (Q13), and `ISSUER_BURNABLE` (Q11)—without recommending a default for any of them. This forces dual-path requirements throughout the document and obscures concrete conflicts: e.g., choosing Snapshot mode in Q2 blocks migration until Stage 4 (old-chain freeze), contradicting Steps 4–5 and the readiness gate; choosing User-as-issuer weakens chain-level duplicate prevention and increases explorer DoS risk.
- **Why it matters:** Engineers cannot begin Steps 2–3 cleanly, the founder is asked to adjudicate low-level protocol hygiene alongside product choices, and mutually incompatible answers can easily be selected.
- **How an agent should investigate:** Map dependencies and trade-offs across Q1–Q14, separating genuine founder product choices (scope, soulbound vs. transferable, rollout order) from engineering defaults (spam pricing, `ISSUER_BURNABLE`, unsolicited `BURN_LOCKED` rules).
- **Suggested direction for fix:** Reduce founder open questions to 5–6 true product/policy calls, supply a clear **Recommended Default** with one-line rationale for every question (e.g., burn-first for active migration + pin-only opt-out; `Task Node issuer` for official migrated/profile collections with permissionless user collections supported; `0` issuer fee; transferable; reject `ISSUER_BURNABLE` until implemented), and explicitly flag the Stage 4 schedule conflict on Snapshot mode.

##### P7 — Unsolicited `BURN_LOCKED | TRANSFER_LOCKED` NFT griefing is left as an optional choice
- **Severity:** major
- **Where:** `Technical requirements → R19. Unsolicited NFTs`; `Open questions → #12`
- **What is wrong:** Today any account can mint a `BURN_LOCKED | TRANSFER_LOCKED` NFT into any other account for ~0.000033 PFT, and the recipient can never burn or transfer it away. Q12 treats "wallet filtering only, leaving locked junk on chain" as a viable option.
- **Why it matters:** Attackers can permanently poison high-profile accounts (the founder, validators, Governance) with illicit/phishing metadata or thousands of unremovable entries that degrade `account_nfts` queries and cannot be cleaned up even if the victim wants to pay gas.
- **How an agent should investigate:** Check `apply_nft_operation` in `crates/execution/src/nft_escrow_asset_execution.rs` for how `owner != issuer` and `BURN_LOCKED` are validated on `nft_mint` and `nft_burn`.
- **Suggested direction for fix:** Make it a firm R19 chain rule (not an open question) that **holders can always burn NFTs minted to them by another account**—either disallow `BURN_LOCKED` when `owner != issuer`, or require an explicit recipient accept transaction when `owner != issuer` (keeping self-minting permissionless). Keep wallet issuer-filtering as a UI defense-in-depth layer, not the primary mitigation.

##### P8 — Explorer indexer DoS vector in R17 cross-issuer earliest-wins and missing on-chain mint provenance fields
- **Severity:** major
- **Where:** `Chain semantics`; `Technical requirements → R17. Explorer NFT view`; `R20`
- **What is wrong:** Under User-as-issuer (and for detecting rival claims under arbitrary issuers), R17's "no earlier mint holds a checked origin for this NFTokenID" forces the explorer indexer to fetch, parse, and verify up to 1 MiB IPFS metadata envelopes, claim envelopes, and evidence bundles for *every* NFT minted by *any* issuer on chain. Furthermore, because `NftDefinition` (`nft_info`) does not store `initial_owner` or `mint_block_height`, verifying the first owner requires scanning historical `account_tx` rows or trusting a centralized explorer indexer.
- **Why it matters:** An attacker can spend <1 PFT to mint thousands of spam NFTs referencing slow/dead/1 MiB CIDs and stall the explorer's origin indexer. Simultaneously, local Task Node (Pillar 5) and autonomous agents cannot verify NFT provenance directly from an RPC node without trusting the central explorer.
- **How an agent should investigate:** Check `explorer-spec.md` and `NftDefinition` in `crates/types/src/market_nav_asset_types.rs` to see how the indexer fetches off-chain IPFS content and whether `initial_owner` / `mint_block_height` can be added to `NftDefinition` during the R18 state upgrade.
- **Suggested direction for fix:** (1) Add `initial_owner` and `mint_block_height` (or `mint_tx_hash`) to `NftDefinition` in R18 so any RPC client can verify first-owner and window-close rules directly. (2) Scope automatic origin indexing to configured migration collections (or enforce strict rate-limits, bounded queues, and lazy on-demand verification with caching for arbitrary issuers), and prefer `Task Node issuer` for official migrations so chain-level `duplicate_nft` eliminates cross-issuer conflict indexing entirely.

##### P9 — Economic model omits IPFS pinning sustainability, fixed-PFT fee volatility, and two-chain migration funding
- **Severity:** major
- **Where:** `Economics`; `Dependencies`; `Open questions → #5, #6, #7`
- **What is wrong:** (1) The pinning plan relies on Task Node's centralized cluster + Pinata with no quota, rate limit, cost model, or long-term retention guarantee for non-Task-Node issuers or `evidence_cid` archives. (2) The spec does not warn that `postfiatl1v2`'s fixed native PFT issuer fee creates a severe economic hazard (punishes self-transfers/cold-storage moves, and can accidentally freeze NFTs if PFT appreciates). (3) Migration requires funded accounts on the new chain (`missing_owner`) *and* spendable fee balances on the old chain (for burns / `AccountSet` proofs), yet new-chain funding is "open" and old-chain fee funding isn't mentioned at all.
- **Why it matters:** Users with historical NFTs in zero-spendable-balance wallets or unfunded new-chain accounts will be blocked from migrating; unrestricted pinning exposes Task Node to storage abuse; and setting any non-zero fixed PFT fee on profile NFTs risks breaking future usability.
- **How an agent should investigate:** Check current Task Node IPFS pinning endpoints/quotas (`docs/wiki/architecture/ipfs-infrastructure-rebuild.md`, `ipfs.md`), old-chain wallet reserve/fee balances for existing profile NFT holders, and Pillar 2 open questions 3–4.
- **Suggested direction for fix:** Expand `Economics` with: (a) a concrete recommendation of `issuer_fee = 0` for profile/migrated NFTs (deferring percentage royalties until a native offer/sale primitive exists); (b) explicit pinning policy (Task Node pins its own collections + migration evidence bundles with strict per-user/per-item size caps; third-party issuers self-pin or use paid pinning); and (c) a concrete migration sponsorship budget/faucet flow covering both new-chain account activation/mint atoms and any old-chain burn drops needed for inventoried holders.

##### P10 — Under-delivers on "Supports NFTs in a smart way" and Deliverable 2/5/6/9 contiguity
- **Severity:** major
- **Where:** `What "smart" means here`; `Technical requirements → R1, R13–R16`; `Open questions → #10`
- **What is wrong:** (1) R1 includes `media[] (typed, optionally encrypted Pillar 2 payloads)` without defining key management, recipient key binding, or transfer behavior (contradicting Q10's note that encrypted holder content needs re-encryption on transfer). (2) Task Node's active profile avatar route (`/select`, cited in `Code references`) has no specified dual-chain continuity behavior when a user's selected old-chain NFT is burned and re-issued on the new chain. (3) The spec does not specify how agents/machines discover or consume NFTs via Pillar 2 / X402 (Deliverable 2) or how Pillar 6 (Hive identity) references a user's canonical profile NFT. (4) It never notes that central `Task Node issuer` breaks Pillar 5 local-only minting independence.
- **Why it matters:** Users whose active profile avatar gets burned during migration could end up with broken avatars; undefined fields in v1 protobufs invite incompatible implementations; and "smart NFTs" feels like a relabeling of standard static NFTs rather than a machine-native primitive.
- **How an agent should investigate:** Inspect `tasknode/server/profile-routes.js` (`/select` and avatar resolution), Pillar 2 (`02-message-protocol.md`), Pillar 5 (`05-local-task-node.md`), and Pillar 6 (`06-hive-identity.md`) to align schemas and cross-pillar interfaces.
- **Suggested direction for fix:** Either define concrete v1 semantics for `media[]` encryption (e.g., encrypted to a specific content-access key or Task Node key, explicitly *not* auto-transferred to buyers) or remove encryption from `media[]` in v1 and reserve a field number for v1.1. Add an explicit requirement in R13/R14 for atomic `/select` active-avatar re-pointing upon migration, document the canonical Pillar 6 avatar-reference format, and clarify how agents/X402 endpoints resolve `pf.nft.metadata.v1`.

##### P11 — R17 image-CID verification breaks for hand-mapped *Needs review* NFTs and unarchived original metadata
- **Severity:** major
- **Where:** `Technical requirements → R5, R10, R17`
- **What is wrong:** R17 bullet 4 requires that "the image CID matches the original metadata's." However: (1) R10 does not archive the original metadata JSON (or verify its hash against the old-chain `NFToken.URI`), so if the old metadata JSON is later unpinned or was an HTTP URL, R17 fails offline; (2) R5 allows non-Task-Node or non-IPFS *Needs review* NFTs to be "mapped by hand" into migration batches, which will fail automated image-CID matching against original metadata and permanently show ***not checked***.
- **Why it matters:** Validly migrated historical NFTs—especially any non-standard NFTs the founder holds—will fail explorer verification or break once old metadata JSONs disappear.
- **How an agent should investigate:** Run the Step 1 inventory (R5) across the founder's wallets and inspect all 79 Task Node CIDs plus any external NFTs to catalog actual URI/metadata formats in the wild.
- **Suggested direction for fix:** Archive the raw original metadata blob (and its SHA3-384 hash) inside `evidence_cid` (R10) whenever fetchable, verify `origin.original_uri` directly against the `NFToken.URI` proven in the burn/snapshot state proof, and define an explicit R17 verification rule for hand-mapped/non-IPFS items (e.g., matching against the archived original metadata blob or displaying a distinct verified-with-remap state rather than `not checked`).

##### P12 — R12 Provenance Records ("Pin-only") contradict R7, lack display schema, and break on ownership transfer
- **Severity:** major
- **Where:** `Technical requirements → R7, R9, R12, R13`
- **What is wrong:** (1) R7 opens with "A mint follows only a retirement read from a validated old-chain ledger," contradicting R7(c) / R12 where no retirement occurs. (2) R12 uses body `pf.nft.provenance.v1` without specifying whether it includes `name`, `image`, or `content_hashes`, leaving explorer rendering and IPFS image pinning unspecified. (3) R12 doesn't define how a `Recorded` item later upgrades to `Migrated` (e.g., auto-burning/hiding the old record). (4) Because R9 derives serials solely from `(source_chain, NFTokenID)` and burned NFTs remain in state forever (`duplicate_nft`), if an XRPL mainnet NFT is transferred to a new owner, the new owner can **never** mint an updated provenance record under `tasknode.profile.record.v1`—even if the prior owner burns theirs.
- **Why it matters:** "Pin instead" is an explicit founder requirement ("get these new NFTs or pin them on the new chain"); as written, records may fail to render images, leave duplicate UI entries on later migration, and permanently lock out subsequent owners.
- **How an agent should investigate:** Review R7, R9, R12, and R17 together to trace the full lifecycle of a pin-only item (create → render in explorer/Task Node → original transfers on source chain → new owner pins OR original owner later burns & migrates).
- **Suggested direction for fix:** Fix R7 wording to distinguish *retirement proofs* (burn/snapshot) from *ownership-at-ledger proofs* (pin-only via method ii/iii); include display/image fields (or embed `provenance` inside `pf.nft.metadata.v1` with a `record_only` flag) so explorer/pinning work uniformly; derive record serials from `(source_chain, NFTokenID, ledger_index, owner)` (or per-user issuer) so ownership updates aren't permanently blocked by `duplicate_nft`; and specify the `Recorded → Migrated` UI/burn transition.

##### P13 — R2 conflates deterministic body encoding with randomised ML-DSA-65 envelopes; `ISSUER_BURNABLE` footgun left live
- **Severity:** minor
- **Where:** `Technical requirements → R2, R4, R17`; `Open questions → #11`
- **What is wrong:** (1) R2 says "Producers use Pillar 2's deterministic encoding ... so vectors reproduce in Rust, JS and Python," while `What exists today` notes ML-DSA-65 signing is randomised (`try_sign`). Deterministic encoding ensures the *inner body / signing preimage* reproduces across languages, NOT the signed envelope bytes—meaning R2 needs the same "persist signed envelope bytes before hashing/minting" rule that R6 states for `claim_hash`, and test vectors must fix a concrete pre-signed envelope. (2) `ISSUER_BURNABLE` is a live bug in `postfiatl1v2` (accepted and stored on-chain, silently ignored on burn), yet R18 doesn't reject it by default and leaves it to Q11.
- **Why it matters:** Implementers could assume re-signing metadata on retry produces the same `metadata_hash` (causing orphan pins or false hash mismatches), and third-party issuers on a permissionless chain can mint NFTs with `ISSUER_BURNABLE` expecting revocation power that doesn't exist.
- **How an agent should investigate:** Inspect `crates/crypto_provider/src/lib.rs` (check if deterministic ML-DSA-65 signing is supported or if `try_sign` is always randomised) and `crates/execution/src/nft_escrow_asset_execution.rs` for flag validation on `nft_mint`.
- **Suggested direction for fix:** Clarify in R2 that inner `pf.nft.metadata.v1` protobuf encoding is deterministic across languages while signed `pf.msg.v1` envelopes must be persisted as raw bytes before computing `metadata_hash` and submitting `nft_mint`. Reject `ISSUER_BURNABLE` at `nft_mint` in R18 as a mandatory bugfix unless/until issuer-burn execution is explicitly implemented.

##### P14 — Operational UX and signing gaps: N-signature batch burns, `signWalletChallenge` blind-signing, and unconfirmed history depth
- **Severity:** minor
- **Where:** `Technical requirements → R5, R6(ii), R8, R15`; `Dependencies`
- **What is wrong:** (1) A multi-item claim lets a user sign one ML-DSA claim for 50 NFTs, but XRPL has no batch `NFTokenBurn`, so browser-wallet users still face signing 50 individual burn transactions unless batch/local signing UX is designed. (2) `signWalletChallenge` signs arbitrary bytes with no domain tag; while verifiers reject login challenges as claims, any XSS/phishing prompt could trick a user into blind-signing a valid R6(ii) claim digest because the wallet cannot distinguish it. (3) `pftl_history_rpc` depth is unconfirmed, with no fallback plan specified if early ledgers are pruned or if the node prunes ledgers mid-migration.
- **Why it matters:** Poor batch-burn UX creates abandonment/error risk during migration; blind signing weakens wallet security; and a pruned old-chain history node would halt Step 3–5 mid-flight.
- **How an agent should investigate:** Check `tasknode/src/wallet-core.js` (`signWalletChallenge` and `signPreparedPftlTransaction`) for batch signing and UI confirmation prompts, and query `server_info` (`complete_ledgers`) on the live `pftl_history_rpc` endpoint.
- **Suggested direction for fix:** Specify explicit human-readable claim confirmation UI (and a distinct domain-prefixed signing method for R6(ii) rather than raw `signWalletChallenge`), document batch-burn pacing/resume UX for browser wallets vs. Python CLI, and add an operational prerequisite in Step 1 to stand up or verify a non-pruning archive snapshot of the old chain before opening migration.

## Claude Opus 5.5 — 72/100

*Model `anthropic/claude-opus-5.5` · 13457 output tokens*

#### Summary judgement

This is a strong, carefully reasoned sub-page. It addresses all three parts of the founder's Deliverable 4:

- bring the historical NFTs over, or "pin" them;
- publish an intelligible quantum-resistant NFT spec, in the docs and as a blog post;
- make the NFT system natively contiguous with the message protocol.

It also honours the readiness-gate framing ("supports NFTs in a smart way") and the founder's demand for multi-model adversarial testing. The design shows real engineering judgment:

- It audits the code and finds real defects: `ISSUER_BURNABLE` is stored but never enforced, `validate_nft_state` re-checks every NFT on every transaction, there is no pagination cursor, and anyone can mint an unremovable NFT into any account.
- The migration is burn-first and claim-first, with a state machine that cannot strand a burned NFT.
- The post-quantum boundary is stated honestly, including the weak point: claims rest on classical old-chain keys.

The score is held back by four things:

1. **Overengineering for the actual inventory.** There are fewer than 79 CIDs, probably a few dozen NFTs, mostly user-issued profile NFTs. Against that, the design has 20 requirements, 14 open questions, ledger-proof evidence bundles, model checking, and a five-test explorer verifier.
2. **Critical-path dependence on unbuilt pillars.** Pillar 2's envelope and Pillar 6's old-chain link are both unbuilt, which stalls a readiness gate.
3. **An unspecified trust anchor for the evidence archive.** The design promises evidence that "verifies without the old chain" but never says what it verifies against. It also assumes inclusion and state proofs that rippled-style RPCs do not readily give.
4. **Economics deferred rather than resolved.** Spam pricing, state growth and pinning cost are all left to the founder with no recommended default.

The page would likely produce something that works, but slowly, and only after the founder answers a long list of questions that the page itself could narrow.

#### Correspondence to the founder's intention

**Strong matches**

- **"I need a way to get these new NFTs or pin them on the new chain."** Covered by the inventory (R5), burn-first migration (R7, R8) and pin-only provenance records (R12). The founder-first rollout with a single batch report (acceptance criterion 4) matches "I am goodalexander and I have a large number of NFTs."
- **"Intelligible quantum resistant NFT protocol spec… documentation and the blog post."** R3 commits to `docs/specs/nft-v1.md` with test vectors, a threat model and a post-quantum boundary table, plus a blog post. The table is honest rather than marketing: it says classical old-chain keys remain trusted until the window closes.
- **"Natively contiguous with the MESSAGE PROTOCOL SPEC."** Metadata is a signed Pillar 2 `pf.msg.v1` envelope. `metadata_hash` is SHA3-384 over exactly the pinned bytes. The claim and provenance records are Pillar 2 types too. This is a genuine "native" integration, not a bolt-on.
- **"Supports NFTs in a smart way" as a readiness criterion.** The page links back to Outcome B and defines "smart" as typed, agent-readable metadata, checkable provenance, policy fixed at mint, and private prompts committed by digest.
- **"Adversarially stress tested… across multiple models."** R20 plus a model check, and the multi-model review is a gate before the founder's batch.
- **Deliverable 8's "permissionless by default, trust lines optional."** Noted correctly: NFTs need no trust lines.

**Partial or weak matches**

- **"Smart" is interpreted narrowly.** The founder's wider vision, across Deliverables 6, 9 and 10, ties NFTs to identity, as in "I minted my Task Node Identity." It also links them to validator keys and to gated access to Finance Hub for validators and top-30 users. The page pushes credentials and identity to open question 10 and mentions Pillar 6 only as an avatar dependency. A founder reading "smart" may have expected the NFT to be the identity or credential primitive, or at least a committed recommendation.
- **"XRP has NFTs" is ambiguous.** It could mean XRPL mainnet or the old Post Fiat XRPL fork. The page handles this correctly as open question 1, but the inventory never establishes basic facts: how many NFTs exist, and on which chains. It says "fewer than 79, not counted," and a read-only count is trivial.
- **FastPay and the "public, post-human" narrative.** The founder wants FastPay as the default for speed and wants the message spec integrated with FastPay. The page never says whether NFT operations can or should route through FastPay or consensus. It also does not connect NFT migration events to the Hive Chat governance story, the public "information packet" framing.
- **Decision burden.** The founder asked "what set of activities maximize the likelihood of success." Fourteen open questions with balanced options but few recommendations push design work back onto the founder.

#### Technical robustness

**Sound**

- **Burn-first ordering.** The claim is persisted (pinned and on the device) before the burn. The state machine has `failed_recoverable`, recovery via an alternate proof of control, and preflight checks. This directly handles the dangerous "burned but never re-issued" case.
- **Deterministic idempotency.** The serial is derived from (domain, source chain, NFTokenID), with re-runs safe after a database wipe (acceptance criterion 5).
- **Snapshot restricted to a frozen chain and banned for XRPL mainnet.** Correct: snapshot is only safe when the source can no longer change.
- **Issuer burns excluded.** They are correctly excluded as proof of holder retirement. Old Task Node NFTs carry `Flags: 9`, which is burnable plus transferable.
- **Scale and DoS defects identified, with a fix scheduled before switch-over.** O(n) validation per NFT transaction, combined with a near-zero fee, is a quadratic DoS vector. R18 is correctly placed in Step 1.
- **Unsolicited-mint problem identified.** A `BURN_LOCKED` + `TRANSFER_LOCKED` item minted into a victim's account is permanent junk.
- **Commitment discipline.** Readers hash exactly the fetched bytes, never re-encode, and stop at a size cap. NFT schemas exclude maps and floats so test vectors reproduce across Rust, JS and Python.

**Weak or unverified**

- **The evidence-archive trust anchor (R10) is under-specified, and possibly infeasible as written.**
  - It relies on "validators' validations or a skip-list hash link to the final ledger hash published at the freeze."
  - It does not say who publishes the final hash, or with what post-quantum signature.
  - It does not say how items migrated before the freeze are verified (only by classical validator signatures, against which UNL?).
  - It does not say whether the old chain's RPC can actually serve SHAMap inclusion proofs or NFToken-page state proofs. Stock rippled does not expose state proofs; you would need full ledger state or a custom tool.
- **The skip-on-duplicate logic in R9 is too permissive.** It skips if the existing NFT's metadata names the same source chain and NFTokenID. It does not check owner, first owner, envelope signer or claim binding. With a stolen Task Node issuer key, an attacker could pre-mint fake "origin" NFTs at the deterministic serials, and the tool would skip the legitimate re-issue, so the user never gets the item.
- **The cost of explorer verification in R17 is unaddressed.** Each NFT page must fetch the claim, metadata and evidence from IPFS and verify ML-DSA signatures and old-chain ledger proofs. It must also check "earliest-wins" across all mints, under all issuers, that claim the same NFTokenID. The page does not say where this runs (indexer or browser) or what caching it uses.
- **Code-level claims are asserted but not reproduced.** Examples: `owner` can be any existing account; `ISSUER_BURNABLE` is unenforced; burned NFTs stay in state; the fee formula. They are plausible, but downstream agents should verify them. Several design decisions hinge on them, including R9, R17 and R19.

#### Economic coherence

- **Fees are negligible**, about 20 atoms, or 0.00002 PFT, per mint. Migration is therefore essentially free, but so is spam into permanent chain state, with no per-NFT reserve and a 10-atom account reserve. The page identifies this (R18, R19, open question 13) but makes no recommendation. That is a chain-wide economic flaw, and it affects whether "production ready" is credible.
- **The fixed issuer fee is economically odd.** It is charged on every transfer, including moves between a holder's own wallets, and there is no sale or royalty primitive. For profile NFTs the right default is almost certainly zero; for anything market-like the primitive is inadequate. The page notes this but does not tie it to the NAVCoin fee work in Pillar 7 beyond display.
- **Pinning has real cost, but it is not quantified.** The page names a Pinata plus cluster setup and puts long-term responsibility in open question 7. Media size limits are unspecified.
- **Custody and economic risk of the Task Node issuer** (a single hot key) is mentioned but not costed against the user-as-issuer option. The "who pays account creation" policy, a dependency, is open.

Overall, the economics are coherent where they exist, and minting is cheap. The hard economic questions are deferred rather than answered.

#### Problems

**P1. Evidence-archive trust anchor undefined; proof sources may not exist**
- **Severity:** critical
- **Where:** R10; R17, the third origin test; acceptance criterion 9; the post-quantum table row "evidence fixed at mint."
- **What is wrong:**
  - The bundle must verify "without the old chain" using validations or a "skip-list hash link to the final ledger hash published at the freeze." No publisher, signature scheme or on-chain anchor for that final hash is specified.
  - Items migrated before the freeze cannot link to a hash that does not yet exist.
  - Validations are classical signatures from a UNL whose keys must themselves be archived and trusted.
  - rippled-style RPCs do not provide NFToken-page state proofs, and SHAMap inclusion proofs need full transaction sets or custom tooling.
- **Why it matters:** "Checked" origins after the old chain is retired are a headline guarantee. If the anchor is vague, the explorer either overclaims or never marks anything checked.
- **How to investigate:**
  - Check what `pftl_history_rpc` actually returns: `ledger` with `transactions: true, expand: true`, `ledger_data`, `validations`.
  - Confirm whether validator public keys and the UNL for NetworkID 2025 are recorded anywhere.
  - Prototype building one evidence bundle for a real old NFT.
- **Suggested direction:**
  - Define a concrete anchor: a post-quantum-signed `pf.chain.freeze.v1` message on the new chain that commits to the old chain's final ledger hash and the UNL key set.
  - Until the freeze, mark origins "checked (pre-freeze, validator-attested)" as a distinct state.
  - Accept full transaction-set inclusion in place of Merkle paths if paths are unavailable.
  - Drop state proofs for snapshot mode, or require a full `ledger_data` dump hash.

**P2. R9 skip condition lets a stolen issuer key permanently block legitimate re-issues**
- **Severity:** major
- **Where:** R9 (`duplicate_nft` handling); the security-model rows on double migration and stolen issuer keys.
- **What is wrong:** An existing NFT at the target id is treated as done if its metadata names the same source chain and NFTokenID. Serials are public and deterministic, so an attacker holding the issuer key can pre-mint decoys with copied origin fields. The tool then skips the real item.
- **Why it matters:** This stops a duplicate but strands a burned NFT, violating the "never strands a burned NFT" promise.
- **How to investigate:** Trace R9 against the threat "stolen issuer key." Write the R20 test "attacker mints decoy at deterministic serial before user's mint."
- **Suggested direction:**
  - Skip only if the existing NFT passes all R17 origin tests for this claim: claimant is first owner, claim hash matches, issuer envelope signature is valid.
  - Otherwise mark it "failed (squatted)" and route it to a fallback serial or collection, with operator alert.

**P3. Readiness gate blocked on two unbuilt pillars; no minimal path**
- **Severity:** major
- **Where:** Dependencies; build order, Steps 2–3; R6(ii); snapshot mode.
- **What is wrong:**
  - Migration needs the Pillar 2 public-envelope subset frozen.
  - Snapshot and recovery need the Pillar 6 R14 old-chain link.
  - Both are "Not built."
  - Snapshot also needs old-chain freeze (README Stage 4), which plausibly comes *after* migration. That is a circular ordering.
- **Why it matters:** NFT support gates Outcome B, and this design makes the gate wait on the slowest sibling pillars.
- **How to investigate:** Map the README stage ordering against Pillar 2 and Pillar 6 schedules. Determine whether freeze can precede migration.
- **Suggested direction:**
  - Specify a minimal viable path: burn-first only, with method (i) or (iii) proofs that need no Pillar 6.
  - Use a frozen "envelope v0" containing only the fields R2 needs, decoupled from the full Pillar 2 spec.
  - Explicitly drop snapshot from the founder batch unless freeze comes first.

**P4. Overengineering relative to the inventory; inventory not yet counted**
- **Severity:** major
- **Where:** In one paragraph ("fewer than 79… not counted"); R5–R20 as a whole.
- **What is wrong:** For perhaps a few dozen profile NFTs, mostly user-issued, the design carries:
  - claim envelopes;
  - three proof-of-control methods;
  - evidence bundles with ledger proofs;
  - model checking;
  - a five-test explorer verifier;
  - per-user self-serve flows.

  The basic count, a trivial read-only call, is not done.
- **Why it matters:** Cost and time are out of proportion. The founder can't judge proportionality without the numbers.
- **How to investigate:** Run `account_nfts` across known Task Node wallets now. Report the count per chain, issuer and taxon, and how many are founder-owned.
- **Suggested direction:**
  - Put the real counts in the status.
  - Tier the design: a simple path for the founder batch (burn-first, CLI, Task Node issuer), and the full self-serve machinery only if user counts justify it.

**P5. Fourteen open questions with few recommended defaults**
- **Severity:** major
- **Where:** Open questions 1–14; the issuer settings table.
- **What is wrong:** The options are balanced, but the page rarely says which one it recommends. Many questions have obvious defaults for profile NFTs:
  - zero issuer fee;
  - burn-first;
  - a Task Node issuer for migration;
  - burn allowed for unsolicited NFTs;
  - a per-NFT reserve.
- **Why it matters:** This raises founder decision load and delays Step 3. The founder asked for the plan that "maximizes likelihood of success," which implies recommendations.
- **How to investigate:** For each question, assess reversibility and the cost of waiting.
- **Suggested direction:** Add a "Recommended default" column, and mark which questions are blocking and which can be deferred.

**P6. "Smart NFT" scope omits identity and credential linkage expected across deliverables**
- **Severity:** major
- **Where:** "What 'smart' means here"; open question 10; Dependencies (Pillar 6 as avatar only).
- **What is wrong:** The founder links NFTs to:
  - minted Task Node identity (Deliverable 6);
  - validator keys (Deliverable 6);
  - Finance Hub gating for validators and top-30 users (Deliverables 3 and 9).

  The page defers all of these and gives no interface sketch.
- **Why it matters:** This risks a v1 schema that later needs a breaking v2. It also leaves "smart" feeling like hygiene rather than a new primitive.
- **How to investigate:** Read Pillars 6, 8 and 9 for their expectations of NFTs.
- **Suggested direction:**
  - Reserve schema fields or a type registry for credential and identity NFTs now.
  - State a recommended split: Pillar 8 credentials for gating, NFTs as holdable identity artefacts linked via Pillar 6.
  - Include one worked example, such as a "Task Node identity NFT."

**P7. Spam and state-growth economics left unresolved for a production gate**
- **Severity:** major
- **Where:** Chain semantics (fees, reserve); Economics; R18; R19; open questions 12 and 13.
- **What is wrong:** At about 20 atoms per mint with no per-NFT reserve, anyone can permanently grow state and drop locked junk into any account. The page benchmarks the problem but does not recommend a price or rule.
- **Why it matters:** Unbounded cheap state growth undermines both performance and the "production ready" claim. This is chain-wide, not NFT-local.
- **How to investigate:** Model state growth cost per NFT on validators. Compare XRPL's owner reserve (now 0.2 XRP per object) against the PFT valuation.
- **Suggested direction:**
  - Recommend a refundable per-NFT reserve plus a ban on minting `BURN_LOCKED` items to other accounts.
  - Ship both with R18 in Step 1.

**P8. Explorer verification cost and location unspecified**
- **Severity:** minor (borderline major)
- **Where:** R17; acceptance criterion 9; explorer dependency.
- **What is wrong:** Each origin check needs IPFS fetches, ML-DSA verification and old-chain proof verification, plus a global "earliest-wins" scan across issuers. The page does not say whether the indexer precomputes and caches this or the browser does it.
- **Why it matters:** It affects latency, gateway dependence and whether "checked" results are reproducible by third parties.
- **How to investigate:** Review the current explorer architecture, a static site plus indexer, and estimate verification cost per page.
- **Suggested direction:**
  - The indexer computes origin state and publishes signed or derivable results.
  - The browser offers "re-verify" on demand.
  - Keep an index of NFTokenID → earliest checked mint.

**P9. Recovery and claim window may strand real users**
- **Severity:** minor
- **Where:** R8 recovery; R11; open question 8.
- **What is wrong:**
  - Recovery requires the classical old key while the window is open.
  - Users who have lost keys, or who are inactive, are stranded.
  - The quantum threat to secp256k1 and ed25519 is years away, so an early close trades real users for a theoretical risk.
- **Why it matters:** This affects UX and trust in the migration narrative.
- **How to investigate:** Estimate the dormant-wallet fraction from Task Node activity data.
- **Suggested direction:** Recommend a long window tied to old-chain read-only status, and give a recorded unclaimed item to the old owner by default.

**P10. `signWalletChallenge` signs arbitrary bytes, creating phishing exposure for method (ii)**
- **Severity:** minor
- **Where:** Signing row; R6(ii).
- **What is wrong:** Verifiers accept only a domain-separated digest. But the wallet still signs any bytes, so a malicious page could solicit a valid claim-link signature.
- **Why it matters:** A non-holder could obtain a valid claim proof.
- **How to investigate:** Review the `wallet-core.js` signing UI and whether it shows what is being signed.
- **Suggested direction:** Add a dedicated wallet method that builds and displays the claim digest, refusing raw-bytes signing for that domain.

**P11. FastPay relationship unaddressed**
- **Severity:** minor
- **Where:** Absent from the whole page.
- **What is wrong:** The founder wants FastPay as the default and the message spec integrated with FastPay. The page doesn't say whether NFT transfers or mints use FastPay or consensus, or how they are represented on the explorer.
- **Why it matters:** This is a coherence gap across Deliverables 2 and 3 and the performance criteria.
- **How to investigate:** Check whether FastPay supports non-payment object operations in `postfiatl1v2`.
- **Suggested direction:** State explicitly: consensus-only for NFTs in v1, with a rationale, or a FastPay path for transfers.

**P12. Post-quantum table slightly overstates the classical weakness of CIDs**
- **Severity:** minor
- **Where:** Post-quantum boundary table, the "IPFS CIDs" row.
- **What is wrong:** SHA-256 is not materially broken by quantum computers; Grover leaves about 128-bit preimage security. The reason to commit to SHA3-384 is consistency, not quantum weakness.
- **Why it matters:** The blog and spec should be precise for a technical audience.
- **How to investigate:** Review the NIST post-quantum guidance on hash functions.
- **Suggested direction:** Reword: CIDs are locators, and SHA3-384 is the commitment for uniformity and margin.

**P13. Issuer semantics change under the Task Node issuer**
- **Severity:** minor
- **Where:** Issuer settings table; open question 3.
- **What is wrong:** Old NFTs are user-issued. Migrating under a Task Node issuer changes provenance semantics and creates custodial-looking central control. The page names this as "central key risk" but does not discuss the user-facing meaning of issuer.
- **Why it matters:** It may conflict with the "post-human, permissionless" story.
- **How to investigate:** Ask how the explorer will label original issuer versus new issuer.
- **Suggested direction:** Always display `origin.original_issuer` prominently, and recommend user-as-issuer for the founder's own NFTs if browser signing is not on the critical path, since the CLI suffices.

**P14. Code-level facts need independent verification**
- **Severity:** minor
- **Where:** "Chain semantics this design relies on (checked in code)."
- **What is wrong:** Key design choices depend on these assertions:
  - `owner` can be any account;
  - `ISSUER_BURNABLE` is unenforced;
  - burned ids are retained;
  - fees are charged on self-transfers;
  - the fee formula and the "about 23 atoms" maximum.
- **Why it matters:** If any one is wrong, R9, R17 or R19 change.
- **How to investigate:** Read `nft_escrow_asset_execution.rs`, `entrypoints.rs` and `market_nav_asset_types.rs` and write unit tests asserting each claim.
- **Suggested direction:** Link each semantic claim to a test name in the repo.

**P15. No connection to the public governance and information-packet narrative**
- **Severity:** minor
- **Where:** Absent from the whole page.
- **What is wrong:** The founder's overarching story is public, post-human governance via information packets in Hive Chat. The migration (batch reports, freeze, window close) is a natural governance event but is not tied to Deliverable 1.
- **Why it matters:** This is a missed narrative coherence. Window-close and flag decisions could be governance outputs.
- **How to investigate:** Cross-read Pillar 1.
- **Suggested direction:** Make the open-question answers and the freeze anchor (P1) outputs of a Hive Chat governance session, published as Pillar 2 messages.

**P16. Collection flags are fixed forever by the first mint; devnet sign-off may not catch mainnet differences**
- **Severity:** minor
- **Where:** R4; acceptance criterion 3.
- **What is wrong:** The page has no provision for a mistaken flag choice after mainnet mint, beyond creating a new collection id. `nft_id` includes the chain id, so the devnet run does not exercise mainnet ids.
- **Why it matters:** An irreversible error would affect the founder's showcase collection.
- **How to investigate:** Confirm in code that a collection's flags cannot change after the first mint.
- **Suggested direction:** Document a versioned-collection escape hatch, such as `.v2` plus a migration via burn-and-remint, in the spec.
