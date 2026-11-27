# Feedback — Pillar 4 — The NFT System

*Review round 1.*

> **Mean score:** 65/100 · Gemini 4 Neon 62 · Claude Opus 5.5 68
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/04-nft-system.md), once, with the question below. Run 2026-11-26 20:13 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 62/100

*Model `google/gemini-4-neon` · 16384 output tokens*

#### Summary judgement

This sub-page is a well-structured, codebase-grounded first draft that captures the surface deliverables of Pillar 4 (an NFT spec and blog post, a link to Pillar 2's message protocol, a batch migration tool, Task Node minting on the new chain, and an explorer view). Its biggest strength is that it maps requirements directly onto real crates, RPCs, and Task Node scripts rather than inventing greenfield abstractions.

However, it is **not ready to build from as written**. Under scrutiny, the core migration mechanism has critical ownership and double-spend flaws, the cryptographic commitment linking on-chain NFTs to Pillar 2 messages is underspecified in a way that will break interoperability, the protocol mechanics of the existing `postfiatl1v2` NFT module are taken on faith without reconciling issuer/collection/flag constraints, and the economic model is largely absent or self-contradictory. It also narrows the founder's broader "supports NFTs in a smart way" and "historical NFTs" requirements to just the 79 `pft-ledger` profile NFTs without verifying whether XRPL mainnet or non-profile NFTs are in scope.

#### Correspondence to the founder's intention

The founder's prompt sets two layers of intent for NFTs:
1. **Chain readiness gate:** *"Supports NFTs in a smart way"* as a prerequisite for migrating users from the old chain to the new chain.
2. **Deliverable 4:** Bring the founder's *"large number of historical NFTs"* (mentioning *"right now XRP has NFTs"* and *"I have minted on the task node"*) onto the new chain (or pin them there), publish an *"intelligible quantum resistant NFT protocol spec"* in docs and a blog post, and make the NFT system *"natively contiguous with the MESSAGE PROTOCOL SPEC."*

**Where the sub-page aligns well:**
- Directly schedules the spec (`docs/specs/nft-v1.md`) and blog post (`postfiatorg.github.io`) with founder review as Acceptance Criterion 1.
- Models NFT metadata (`pf.nft.metadata.v1`), claims (`pf.nft.claim.v1`), and retirement pointers (`pf.ptr`) as Pillar 2 message types, and explicitly moves private NFT prompt assets to Pillar 2's post-quantum encryption suite (R8).
- Includes batch migration with an audit report (R6) so the founder does not have to click through dozens or hundreds of NFTs one by one.
- Preserves existing image CIDs rather than re-uploading art.

**Where the sub-page diverges or falls short:**
- **Premature scope narrowing:** It assumes all historical NFTs are the 79 profile NFTs on `pft-ledger`. The founder's wording (*"right now XRP has NFTs, and I have a large number of historical NFTs"*) may also include NFTs on XRPL mainnet or non-profile historical Task Node NFTs. This isn't even raised as an open question.
- **No thesis on "smart" NFTs:** The sub-page treats `postfiatl1v2`'s existing `nft_mint` / `nft_transfer` / `nft_burn` primitives as done and layers a basic profile-picture metadata schema on top. It never evaluates whether those primitives support Post Fiat's broader machine-native / post-human vision (e.g., encrypted/gated content NFTs, verifiable credentials, agent-consumable metadata, or seamless local signing per Deliverable 5).
- **Missing local Task Node alignment:** Deliverable 5 emphasizes that the local Task Node should handle nearly everything the hosted Task Node does, with local key custody. This spec centralizes migration and implies hosted-server orchestration without addressing local ML-DSA-65 signing or wallet custody.

#### Technical robustness

Technically, this is the weakest dimension alongside economics. Several parts of the design would fail or produce security vulnerabilities if implemented literally:

1. **Migration authority and ordering are broken.** The sub-page notes that on the old chain *"The user's wallet signs"* `NFTokenMint`, which means the user—not Task Node—is the old-chain issuer/owner. Yet R5 and the architecture diagram have the *migration issuer* burn or tag the old NFT *after* minting on PFTL. The migration issuer lacks burn authority, post-mint retirement creates a failure window where two live NFTs exist, checking ownership at a "recorded ledger index" invites transfer front-running, and a `pf.ptr` memo on the old chain does nothing to stop the old NFT from being transferred or sold.
2. **Cryptographic commitment ambiguity with Pillar 2.** R1 requires both a `.proto` definition and a canonical JSON form, and says `metadata_hash` is the hash of the canonical encoding while `uri` is the metadata CID. It never defines which format is pinned to IPFS, which byte stream is hashed (standard protobuf wire format is not canonical), what hash function is used, or whether the CID points to a Pillar 2 envelope or a bare payload.
3. **Unchecked `postfiatl1v2` protocol mechanics.** The spec does not verify whether `NftMintOperation` supports minting directly to a recipient account or requires mint-then-transfer (which breaks if profile NFTs are soulbound / `TRANSFER_LOCKED`), whether `collection_id` is issuer-scoped or spoofable globally, or who acts as issuer for newly minted vs. migrated NFTs.
4. **Superficial quantum-resistance treatment.** Naming ML-DSA-65 and a hash commitment is not a complete quantum-resistant protocol spec. It omits hash algorithm security margins, post-quantum key/re-encryption mechanics for private NFT/prompt assets, and a **classical-key migration cutoff** (without a sunset, any unmigrated old-chain NFT can eventually be stolen via quantum forgery of the old classical key).
5. **Operational and transition gaps.** Switching Task Node inventory to read from the new chain (R7) hides unmigrated old-chain NFTs during staged migration; `account_nfts` does not return `mint_tx` (needed for `origin`); marking the old copy "migrated" requires changes to the *old-chain* explorer (unscoped); and the entire dependency on PFTL ML-DSA-65 wallet/signing UX in Task Node is omitted from `Dependencies`.

#### Economic coherence

Economic design is almost entirely absent and contains an internal contradiction:

1. **Contradiction on fees/royalties and undecided asset nature.** The spec states `issuer_transfer_fee` is built on-chain while declaring marketplaces and royalties out of scope, then asks the founder whether NFTs should carry royalties or issuer transfer fees. Without a sale/offer primitive, how is an issuer transfer fee denominated and enforced on `nft_transfer` (flat PFT fee on every wallet move? percentage of what)? More fundamentally, the spec never decides whether Task Node profile NFTs are **soulbound identity badges** or **tradable assets**—a decision that dictates whether transfer fees, royalties, and dual-chain trading arbitrage even apply.
2. **No spam, reserve, or migration-subsidy economics.** There is no analysis of PFTL transaction fees or state reserves for `NftDefinition` / `NftIndexes` (unlike XRPL's owner reserve), who pays gas during migration and new mints, or how an open subsidized migration prevents attackers from mass-minting junk NFTs on the old chain to grief the migration issuer.
3. **No IPFS pinning permanence model.** Today's 79 profile NFTs survive because Task Node pins them. If NFTs are a chain-level primitive, the spec provides no economic model for who pins metadata and media over time—especially for non-Task-Node NFTs or after a user leaves Task Node.

#### Problems

##### P1 — Migration retirement, burn authority, and double-spend controls are broken
- **Severity:** Critical
- **Where it occurs:** `User story` (Augmented story steps 2–4), `Architecture` (Mermaid diagram), `Technical requirements` (`R3`, `R4`, `R5`)
- **What is wrong:**
  1. The sub-page states that on the old chain *"The user's wallet signs"* `NFTokenMint`, yet `R5` and the Mermaid diagram assume the *migration issuer* can burn the old NFT or post a `pf.ptr` memo "from the issuer." If the user is the old-chain issuer/owner, the migration issuer cannot burn it.
  2. Retirement happens *after* PFTL minting in the diagram and prose, so any failure on the retirement leg leaves two live copies.
  3. `R4` checks ownership at a "recorded ledger index" without requiring the old NFT to be burned or locked *first*, allowing an owner to sign a claim and transfer the old NFT to a buyer before or after migration.
  4. `R5` claims a `pf.ptr` memo ensures *"the old copy and the new one can't both pass as the live NFT,"* which is false—a memo on an XRPL-shaped chain does not freeze transfers or sales on the old chain.
  5. Idempotency is left to an off-chain "migration issuer" process with no on-chain uniqueness enforcement and no verifiable claim proof linked to the minted PFTL NFT.
- **Why it matters:** Enables asset duplication/split-brain fraud on transferable NFTs, breaks automation when the migration issuer lacks burn permissions, and forces users to blindly trust a centralized migration server instead of cryptographically verifying migrations.
- **How an agent should investigate:**
  1. Inspect `server/pftl-submit.js` and `server/profile-nft-mint.js` in `postfiatorg/tasknode` to determine who is the `Account` and `Issuer` on historical `NFTokenMint` transactions and what `Flags` (`tfBurnable`, `tfTransferable`) were set on all 79+ existing NFTs.
  2. Inspect `crates/execution/src/nft_escrow_asset_execution.rs` and `crates/types/src/core_chain.rs` in `postfiatorg/postfiatl1v2` to check how `postfiat.nft_id.v1` is derived (can it incorporate `origin` or an idempotency salt so the chain itself rejects duplicate migrations?).
- **Suggested direction for the fix:**
  - Use a **burn-first (or lock-first) two-phase protocol** for transferable NFTs: the *owner* signs an `NFTokenBurn` (or transfer to a provably unspendable burn address) on the old chain with a memo committing to their destination PFTL ML-DSA address (or `pf.nft.claim.v1` hash). Once finalized on the old chain, the migration service verifies the burn transaction and mints on PFTL, embedding the burn tx hash and `claim_cid` in `origin`.
  - If old NFTs are non-transferable (soulbound) or the founder explicitly wants read-only preservation on the old chain, state clearly that `pf.ptr` is an *advisory deprecation marker* only, not a transfer lock, and freeze eligibility at a published old-chain snapshot ledger index.
  - Enforce idempotency deterministically on-chain (e.g., via deterministic NFT ID derivation or a deterministic `metadata_hash` / registry check) so a DB reset cannot cause double-minting.

##### P2 — Ambiguous cryptographic commitment between Protobuf, canonical JSON, IPFS CID, and `metadata_hash`
- **Severity:** Critical
- **Where it occurs:** `User story` (step 1), `Technical requirements` (`R1`, `R2`)
- **What is wrong:** `R1` specifies both a `.proto` schema and a canonical JSON form, and states *"On PFTL, `metadata_hash` is the hash of the canonical encoding, and `uri` is the metadata CID."* It never specifies: (a) whether the bytes pinned at `uri` are protobuf, canonical JSON, or a Pillar 2 envelope; (b) which byte representation is hashed for `metadata_hash` (note that protobuf wire format is not canonical by default); (c) which hash algorithm and digest size are mandated (the chain allows `≤64 bytes`); and (d) how an explorer distinguishes a Pillar 2 NFT metadata object from an arbitrary URL or encrypted envelope.
- **Why it matters:** Two compliant implementations (Task Node, CLI, explorer, third-party wallets) will compute different `metadata_hash` values or fail to decode `uri`, breaking the core promise that NFTs are *"natively contiguous with the MESSAGE PROTOCOL SPEC."*
- **How an agent should investigate:**
  1. Read the Pillar 2 sub-page (`02-message-protocol.md`) and any existing protobuf/IPFS envelope specs in `postfiatl1v2` and `tasknode` to see how Pillar 2 defines canonical serialization, envelopes, content addressing, and hashing.
  2. Check `crates/types/src/core_chain.rs` and `crates/execution/src/nft_escrow_asset_execution.rs` to see what validation (if any) the chain enforces on `metadata_hash` length/format and `uri`.
- **Suggested direction for the fix:**
  - Pick **one** canonical on-IPFS representation aligned with Pillar 2 (e.g., a deterministic Pillar 2 protobuf envelope pinned as a raw CIDv1, with JSON defined strictly as a derived display/RPC projection—not a parallel hashed format).
  - Explicitly define the exact hash function (e.g., SHA-256 or SHA-512/SHA3-256 over the exact pinned envelope bytes, plus deterministic protobuf encoding rules if protobuf is hashed directly), and specify how `uri` (`ipfs://<cidv1>`) and `metadata_hash` are cross-verified by the explorer.

##### P3 — Unverified `postfiatl1v2` minting, collection scoping, and flag semantics
- **Severity:** Major
- **Where it occurs:** `What exists today`, `User story` (steps 3 & 5), `Technical requirements` (`R4`, `R7`)
- **What is wrong:**
  1. `R4` has the migration issuer mint NFTs on PFTL for users, while `R7` says new profile NFTs mint on PFTL without specifying whether the *user* or a *Task Node issuer* submits `nft_mint`.
  2. It is never checked whether `NftMintOperation` supports minting directly to a different `owner` address, or if the issuer must mint to itself and then call `nft_transfer`—which fails if the NFT is minted non-transferable (`TRANSFERABLE` unset or `TRANSFER_LOCKED` set).
  3. It is never checked whether `collection_id` (`tasknode.profile.v1`) is scoped by `(issuer, collection_id)` or globally. If anyone can pass `collection_id = "tasknode.profile.v1"`, attackers can spoof official Task Node profile NFTs.
  4. The spec never defines which flags (`TRANSFERABLE`, `ISSUER_BURNABLE`, `TRANSFER_LOCKED`, `BURN_LOCKED`) apply to profile NFTs.
- **Why it matters:** Directly determines whether the proposed migration and Task Node minting flows are even executable on the current `postfiatl1v2` state machine, and whether official collections can be spoofed on the explorer.
- **How an agent should investigate:**
  1. Read `NftMintOperation`, `NftTransferOperation`, `NftBurnOperation`, `NftDefinition`, and `NftIndexes` in `crates/types/src/transactions_mempool_receipts.rs` and `crates/types/src/market_nav_asset_types.rs`.
  2. Read `crates/execution/src/nft_escrow_asset_execution.rs` to verify: (a) who becomes initial `owner` on `nft_mint`; (b) exact semantics of all four flags; (c) how `collection_id` is indexed and validated; and (d) whether `issuer_nfts` / `account_nfts` distinguish collections by issuer.
- **Suggested direction for the fix:**
  - Document the exact `postfiatl1v2` execution semantics in the spec. If `NftMintOperation` lacks an initial `recipient`/`owner` field and profile NFTs are intended to be non-transferable, add a small chain requirement (or authorized-minter flow) rather than assuming mint-then-transfer works.
  - Define canonical collection identity as `(issuer_account, collection_id)` in `nft-v1.md` and the explorer, and explicitly specify the issuer model and flag defaults for both migrated and newly minted profile NFTs.

##### P4 — Incomplete quantum-resistance specification
- **Severity:** Major
- **Where it occurs:** `User story` (step 1), `Technical requirements` (`R2`, `R8`)
- **What is wrong:** The spec reduces quantum resistance to *"ML-DSA-65 ownership and a hash commitment to the metadata."* It omits:
  1. **Classical-key migration sunset:** `pf.nft.claim.v1` accepts classical signatures from the old chain. Without a hard cutoff date/block height for claims, a future quantum adversary who breaks old-chain Ed25519/secp256k1 keys could claim any unmigrated historical NFTs onto PFTL.
  2. **Hash security margins:** Does not state whether `metadata_hash`, `postfiat.nft_id.v1`, and `content_hashes` use 256-bit or 384/512-bit hashes, or how CIDv1 SHA-256 hashes interact with post-quantum collision bounds.
  3. **Post-quantum encrypted NFT lifecycle (`R8`):** Moving private prompt assets to Pillar 2's PQ encryption is mentioned in one bullet, with no specification of how encrypted metadata or gated assets behave on ownership transfer (key rotation / re-encryption / buyer access).
- **Why it matters:** The founder explicitly wants an *"intelligible quantum resistant NFT protocol spec"* published as a flagship blog post. Publishing a shallow claim that misses obvious attack surfaces (like open-ended classical migration claims) will invite technical criticism.
- **How an agent should investigate:**
  1. Check `crates/crypto_provider/src/lib.rs` and `crates/types/src/core_chain.rs` to see which hash algorithms and PQ primitives (ML-DSA-65, ML-KEM, SHA-256/SHA-512/BLAKE3) are implemented in `postfiatl1v2`.
  2. Check how `pf.asset.nft_prompt_series.v1` and `pf.profile.nft_generation_run.v1` are currently encrypted and accessed in `postfiatorg/tasknode`.
- **Suggested direction for the fix:**
  - Add a dedicated **Threat Model & Post-Quantum Guarantees** section to `docs/specs/nft-v1.md` covering: ML-DSA-65 signature security, hash algorithm choices and PQ collision/preimage bounds, an explicit **migration sunset window** after which classical-key claims are disabled, and the encryption/key-wrapping model for private NFT assets (including whether encrypted NFTs are non-transferable or require owner-mediated re-encryption on transfer).

##### P5 — Economic incoherence: contradictory transfer-fee/royalty model and missing spam, reserve, and pinning economics
- **Severity:** Major
- **Where it occurs:** `What exists today`, `Technical requirements` (`R2`), `Open questions for the founder`
- **What is wrong:**
  1. The sub-page says `Issuer transfer fee supported` is built, then says `Marketplace and royalties out of scope`, then asks *"Should NFTs carry royalties or issuer transfer fees on the new chain?"* without explaining what `issuer_transfer_fee` actually does on `nft_transfer` when there is no sale price or marketplace.
  2. It never specifies whether profile NFTs are soulbound or tradable, leaving the economic rationale of transfer fees and dual-chain arbitrage unaddressed.
  3. It omits on-chain minting fees and state reserves (spam protection for `NftIndexes`), who pays fees for migration and new mints, anti-griefing bounds on open subsidized migration (no snapshot height or collection allowlist), and any long-term IPFS pinning incentive/responsibility model beyond Task Node's single gateway.
- **Why it matters:** Without clear fee/reserve rules, migration allowlists, and pinning accountability, the chain is exposed to state-bloat spam, subsidized migration abuse, broken transfer UX (e.g., penalizing self-transfers), and broken/unpinned NFT links once third-party minting begins.
- **How an agent should investigate:**
  1. Inspect `crates/execution/src/nft_escrow_asset_execution.rs` to see how `issuer_transfer_fee` is actually calculated and deducted during `nft_transfer`, and whether `nft_mint` charges a fee or locks an account reserve.
  2. Review `docs/status/xrpl-feature-parity-burndown.md` (`NFT-001` to `NFT-008`) to understand the intended economic constraints of the existing implementation.
  3. Check Task Node's IPFS pinning setup (`docs/wiki/architecture/ipfs-infrastructure-rebuild.md`) for capacity, cost, and persistence guarantees.
- **Suggested direction for the fix:**
  - Reconcile and document the exact mechanics of `issuer_transfer_fee` (and clarify whether it is a flat fee, disabled by default, or reserved for future offer/settlement integration).
  - Add an **Economics & Resource Bounds** section defining: (a) mint fee + state reserve/rent (or explicit spam mitigation), (b) migration eligibility bounded by a **pre-announced snapshot ledger index** and **approved issuer/collection allowlist** so subsidized migration cannot be griefed, (c) soulbound vs. tradable decision for `tasknode.profile.v1`, and (d) explicit IPFS pinning responsibilities (Task Node pins its own collections; third-party minters/pinning services pin theirs, with `content_hashes` enabling decentralized re-pinning).

##### P6 — Unverified scope narrowing: assumes all historical NFTs are the 79 `pft-ledger` profile NFTs and overlooks "smart" NFT capabilities
- **Severity:** Major
- **Where it occurs:** `In one paragraph`, `What exists today`, `Technical requirements` (`R1`, `R4`, `R7`), `Acceptance criteria` (2)
- **What is wrong:** The founder wrote *"right now XRP has NFTs, and I have a large number of historical NFTs. I have minted on the task node"* and listed *"Supports NFTs in a smart way"* as a chain readiness gate. The sub-page assumes every historical NFT is one of the 79 profile NFTs on `pft-ledger` (`tasknode.profile.v1`), never checks whether the founder holds historical NFTs on **XRPL mainnet** or in other collections, and narrows the metadata schema (`R1`) to static profile images rather than general/machine-native NFTs (e.g., multi-media, encrypted payloads, agent/governance artifacts).
- **Why it matters:** If the founder runs Acceptance Criterion 2 and half their historical NFTs (e.g., on XRPL mainnet or non-profile collections) aren't discovered, the deliverable fails the founder's primary user story. And if `nft-v1.md` only fits static profile pics, it will need a breaking v2 as soon as Post Fiat uses NFTs for richer primitives.
- **How an agent should investigate:**
  1. Check `scripts/import-pftasks-profile-nfts.mjs`, `server/pftl-nfts.js`, and Task Node docs (`docs/wiki/architecture/pftl.md`) to see which chains (`xrpl` mainnet vs `pft-ledger`) and which NFT types/collections have ever been minted for Task Node and the founder.
  2. Add an explicit scoping question for the founder confirming which wallet addresses and chains (XRPL mainnet vs. `pft-ledger`) hold the historical NFTs to migrate.
- **Suggested direction for the fix:**
  - Make `origin.chain` and the migration reader support any XRPL-compatible RPC endpoint (`pft-ledger` AND XRPL mainnet, since both share `NFTokenID` / `account_nfts` / `NFTokenMint` / `NFTokenBurn` semantics) and support arbitrary `collection` mapping rather than hardcoding `tasknode.profile.v1`.
  - Keep `pf.nft.metadata.v1` cleanly extensible (typed media attachments, optional encrypted payload reference per Pillar 2, optional structured claims/traits) so the spec genuinely backs *"supports NFTs in a smart way."*

##### P7 — Broken staged transition and missing ML-DSA wallet / Pillar 5 dependency
- **Severity:** Major
- **Where it occurs:** `Technical requirements` (`R3`, `R7`), `Dependencies`, `User story` (steps 2 & 5)
- **What is wrong:**
  1. `R7` states *"inventory reads `account_nfts` from the new chain."* During a staged migration (as required by the founder's prompt), switching inventory reads exclusively to PFTL will make all unmigrated old-chain NFTs vanish from Task Node—directly contradicting User Story step 2 where users see their old-chain NFTs with a **Bring to Post Fiat L1** button.
  2. `R3` and `R7` require users to sign with a **new-chain ML-DSA-65 wallet**, yet `Dependencies` does not list any PFTL wallet/key-management deliverable or Pillar 5 (Task Node Local Experience), nor does the spec explain how browser or local Task Node users create, fund, and sign with ML-DSA-65 keys.
- **Why it matters:** Guarantees a broken rollout UX where existing users lose visibility of their NFTs and cannot complete the claim flow because they have no PFTL ML-DSA signer in Task Node.
- **How an agent should investigate:**
  1. Check the other planning sub-pages (especially Pillar 5 — Task Node Local Experience and Pillar 6 — Hive Identity) to see where ML-DSA-65 wallet creation, key storage, and signing UX are owned.
  2. Inspect `server/profile-routes.js` in `postfiatorg/tasknode` to see how `/api/profile/nfts` and `/select` currently resolve wallets and selected profile NFTs.
- **Suggested direction for the fix:**
  - Specify a **dual-read inventory mode** during the transition window: Task Node merges PFTL `account_nfts` (live) with old-chain `account_nfts` (badged *Unmigrated* with a claim CTA, or hidden/badged *Migrated* once matched by `origin.nftoken_id`).
  - Add **PFTL ML-DSA-65 wallet/signer integration** (and Pillar 5 local-mode signing compatibility) as an explicit hard dependency, including how new users obtain gas/fees for PFTL minting (e.g., sponsored mint or onboarding faucet).

##### P8 — Data-sourcing and explorer-scope mismatches (`mint_tx` and old-chain explorer)
- **Severity:** Minor
- **Where it occurs:** `User story` (step 3), `Technical requirements` (`R4`, `R5`, `R9`)
- **What is wrong:**
  1. `R4` checks ownership via `account_nfts` and populates `origin: {chain, nftoken_id, mint_tx}`. On XRPL-shaped chains, `account_nfts` returns `NFTokenID`, `URI`, `Issuer`, `Flags`, etc., but does **not** return the `mint_tx` hash. Retrieving `mint_tx` requires full historical transaction indexing (`account_tx` or an NFT indexer), which may not be available on pruned RPC nodes.
  2. `R5` states *"The explorer marks the old copy migrated"*, which requires modifying the **old-chain** explorer to parse `pf.ptr` memos—yet `R9` and `Dependencies` only scope the **new-chain** explorer.
- **Why it matters:** Small oversights like these create unexpected implementation stalls (missing full-history old-chain RPC nodes or unowned work on a legacy explorer codebase).
- **How an agent should investigate:**
  1. Check what old-chain RPC node and history depth are currently available for `pft-ledger` (and XRPL mainnet if in scope), and whether `scripts/import-pftasks-profile-nfts.mjs` or Task Node's DB already stores historical `mint_tx` hashes.
  2. Check which explorer codebase serves the old chain vs. the new chain and whether the old-chain explorer is even still being actively modified.
- **Suggested direction for the fix:**
  - Make `origin.mint_tx` optional (populated when retrievable from Task Node DB or full-history RPC, omitted or replaced with `snapshot_ledger_index` / `burn_tx` when full history is unavailable), and explicitly document the history-node requirement if `mint_tx` is mandatory.
  - Clarify in `R5`/`R9` whether old-chain "migrated" badging is handled in Task Node UI + legacy explorer (with legacy explorer explicitly scoped) or via burn-only retirement where the old NFT simply disappears from the old chain.

## Claude Opus 5.5 — 68/100

*Model `anthropic/claude-opus-5.5` · 8957 output tokens*

#### Summary judgement

This is a strong sub-page, grounded in the codebase. It reads the founder's Deliverable 4 correctly. It covers bringing historical NFTs onto the new chain, a reviewable quantum-resistant NFT spec with a blog post, and native fit with the message protocol. It separates what exists (native `nft_mint`/`nft_transfer`/`nft_burn`, ML-DSA-65, RPC, parity items NFT-001 to NFT-008) from what is missing (metadata standard, migration, Task Node minting on the new chain, explorer view). It turns the gap into concrete, testable requirements.

The weak point is the migration design, which is the hardest part. It has a probable authority error about who can burn or annotate old NFTs. It overclaims that old and new copies "can't both pass as the live NFT". It has an unaddressed race between the ownership snapshot and the mint. Its claim-signing mechanism is hand-waved. It also leaves these unaddressed:

- whether "a large number of historical NFTs" means more than the 79 Task Node profile NFTs;
- what "supports NFTs in a smart way" means beyond XRPL parity;
- how the migration's classical-to-post-quantum bridge holds up against the quantum-safety story;
- the economics of re-minting and pinning;
- whether Task Node users will have new-chain wallets when R7 switches minting over.

None of these is fatal, but several would produce a migration that is unsafe or fails to retire old copies as promised.

#### Correspondence to the founder's intention

**What matches well:**
- **"I need a way to get these new NFTs or pin them on the new chain."** Covered by the claim → re-issue → retire flow. The image CID is reused, so the art is never re-uploaded. This respects "pin" as well as "get".
- **"Intelligible quantum resistant NFT protocol spec that I can review in the documentation and the blog post."** Covered by R2 (`docs/specs/nft-v1.md` plus a blog post on postfiatorg.github.io) and acceptance criterion 1, which requires founder review.
- **"Natively contiguous with the MESSAGE PROTOCOL SPEC."** Covered by R1 (metadata as a Pillar 2 message type with a `.proto` file), R3 (the claim as a message type) and R5 (the `pf.ptr` successor). This is a real integration, not a gesture.
- **The readiness gate ("Supports NFTs in a smart way").** Correctly linked to "a production-ready new chain".
- **The founder's own inventory.** Treated as the first-class case: batch mode (R6) and acceptance criterion 2.

**Where it drifts or under-reads:**
- **"XRP has NFTs."** The founder may mean NFTs on XRPL mainnet, not only on the XRPL-shaped Post Fiat old chain. The page assumes `pft-ledger` only and never asks.
- **"A large number of historical NFTs."** The page cites 79 profile NFT CIDs in total, across all users. If the founder holds "many of them", that is not a large number, so either the inventory is incomplete or the scale assumption is off.
- **"In a smart way."** Never defined. The page delivers parity plus migration and puts marketplace and royalties out of scope. It does not explain what makes PFTL NFTs smart for Post Fiat specifically: for example, NFTs as bearers of message-protocol payloads, identity anchors (Pillar 6), validator or Task Node credentials, or gates to Finance Hub. The founder appears to want a distinctive primitive, not just XRPL parity on a post-quantum chain.
- **Quantum resistance.** Framed narrowly (an ML-DSA signature plus a hash commitment). The migration path itself is not addressed from a quantum-threat standpoint.
- **The founder's broader production-readiness bar.** The founder asks for adversarial stress-testing across multiple models. The NFT and migration flows have no adversarial-testing requirement.

#### Technical robustness

**Strong points:**
- Each component in the inventory has a file reference, size limits and flags.
- Migration is keyed by NFTokenID so it can be re-run safely (idempotent), and there is an acceptance test that a re-run mints nothing.
- Each re-issued NFT records an `origin` link to the original.
- Reusing the image CID avoids a second copy of the art.
- Batch reporting (done / skipped / failed).

**Weak points:**
- **Authority to retire old NFTs is wrong.** Old NFTs were minted by the user's wallet, so the user is the issuer. A "migration issuer" cannot burn them or write issuer memos about them.
- **The memo overclaims.** A successor memo does not prevent the old NFT from being transferred or sold, so the "can't both pass as live" claim is false.
- **The ownership snapshot races the mint.** Ownership is checked at a recorded ledger index. Without lock or burn before mint, an NFT sold after the snapshot can end up live on both chains.
- **The claim signature is hand-waved.** It is unclear how old-chain wallets sign an arbitrary `pf.nft.claim.v1` message. They may not have arbitrary-message signing, may be derived or custodial, or may only sign transactions.
- **Canonical encoding is unspecified.** It is not stated which encoding is hashed (proto or JSON), or whether deterministic protobuf is canonical across languages.
- **Classical keys still matter.** The ownership proof on the old chain is classical. The migration window is an attack surface the quantum story doesn't address.
- **Issuer powers change.** The migration issuer becomes the PFTL issuer, which changes who holds ISSUER_BURNABLE and fee powers over user NFTs.
- **Sequencing conflicts with the chain-migration plan.** R7 switches all new mints to PFTL before users are on the new chain.
- **Dependencies are hidden.** The page relies on the explorer, on Pillar 2's post-quantum encryption suite, on local or new-chain wallets (Pillar 5), and on long-term pinning, none of which it states as risks.

#### Economic coherence

The page is thin here:
- **Migration costs.** It does not say who pays transaction fees and any per-object reserves for re-minting.
- **Pinning.** It does not say who funds pinning of image and metadata CIDs long-term. Is it the Task Node operator, validators, or users?
- **Fees.** Issuer transfer fees are left as an open question. That is reasonable, but the page doesn't connect it to NAVCoin fee transparency (Deliverable 7) or to the economics of the chain as a whole.
- **Spam.** If migration opens to all users or minting becomes free, there is no limit on cheap mints. Collection and minting rules are unspecified.
- **Issuer value capture.** Flags and fees are set by a platform-controlled migration issuer. This may conflict with permissionless norms (Deliverable 8 says trust lines should be "an option not a default") and with user ownership.

None of these is incoherent, but they are absent rather than designed.

#### Problems

**P1 — Retirement authority is likely wrong (burn and memo by "the issuer")**
- **Severity:** critical.
- **Where:** Augmented story step 4; R5; open question 1; the architecture edge `M -- pf.ptr successor memo / burn --> O1`.
- **What's wrong:** "What exists today" says that for old-chain minting, "The user's wallet signs". So the user is the issuer of the old NFT and the migration service is not. It cannot burn the old NFT via ISSUER_BURNABLE semantics or post an issuer successor memo. A memo from a Post Fiat account is only an off-ledger-style assertion.
- **Why it matters:** The central safety claim, that both copies can't pass as live, depends on this.
- **How to investigate:** Read `server/pftl-submit.js` and `profile-nft-mint.js`:
  - Who is the `Issuer` account in `NFTokenMint`?
  - Which flags are set (`tfBurnable`, `tfTransferable`)?
  - Is there an `Issuer`/`NFTokenMinter` delegation?
- **Suggested fix:** Make the claim flow require the owner to burn the old NFT on the old chain, which an owner can always do on XRPL-style ledgers. Alternatively, transfer it to a published burn or escrow account, or post an owner-signed memo transaction. Mint on PFTL only after the burn or transfer is final, and record its transaction hash in `origin`. Keep an issuer memo only as a fallback, with honest wording.

**P2 — The "can't both pass as live" guarantee is overclaimed**
- **Severity:** major.
- **Where:** Step 4; R5.
- **What's wrong:** A successor memo does not stop old-chain transfers or sales. The explorer marking an NFT "migrated" only works on Post Fiat's own explorer, not on third-party XRPL-style tools.
- **Why it matters:** Buyers could be misled, and provenance becomes ambiguous.
- **How to investigate:** List the places old NFTs are displayed or traded: old-chain explorers, wallets, any marketplaces. Check whether the old chain will be frozen or halted after cutover.
- **Suggested fix:** Either require a burn or lock as in P1, or state explicitly that the old chain's NFT state is frozen at a published cutover ledger and that the new chain is canonical after that.

**P3 — Race between the ownership snapshot and the mint**
- **Severity:** major.
- **Where:** R4 ("`account_nfts` at a recorded ledger index").
- **What's wrong:** After the snapshot, the owner can transfer the old NFT. The buyer then holds the old copy while the seller holds the new one. Making the migration safe to re-run by keying it on NFTokenID only prevents duplicate mints. It does not prevent the wrong person receiving the mint.
- **Why it matters:** This is a double-spend of provenance.
- **How to investigate:** Model the claim timeline. Check whether the old chain supports locking an NFT, or whether burn-first is feasible within the user experience.
- **Suggested fix:** Use a burn-or-escrow-first ordering. Alternatively, run a single global snapshot at a fixed cutover ledger, where only the holder at that ledger can claim and later transfers are ignored, and publish that rule.

**P4 — The old-chain claim-signing mechanism is unspecified**
- **Severity:** major.
- **Where:** Step 2; R3.
- **What's wrong:** It assumes old-chain wallets can sign an arbitrary `pf.nft.claim.v1` message. Task Node wallet custody, key derivation and the available signing APIs are unexamined.
- **How to investigate:** Check how Task Node holds old-chain keys (browser, derived, server-assisted), whether it supports signing arbitrary messages, and which XRPL-style signing format is used.
- **Suggested fix:** Define the claim as an old-chain transaction carrying a memo with the hash of the claim and the destination PFTL account. This makes it verifiable on the ledger. Combine it with the burn from P1, and add the new-chain ML-DSA countersignature.

**P5 — Inventory scope is unclear (XRPL mainnet, and "a large number")**
- **Severity:** major.
- **Where:** In one paragraph; What exists today ("all 79 profile NFT CIDs"); R6.
- **What's wrong:** The founder says "XRP has NFTs" and "a large number of historical NFTs". The page assumes only profile NFTs on the Post Fiat old chain, and 79 total doesn't sound like a large founder inventory.
- **How to investigate:**
  - Enumerate the founder's accounts on both the `pft-ledger` chain and XRPL mainnet.
  - Count NFTs by issuer and taxon.
  - Check for non-profile NFT types, such as task or achievement NFTs.
- **Suggested fix:** Add an inventory step and an open question: "Does migration include XRPL mainnet NFTs and NFTs from third-party issuers?" Make `origin.chain` a standard chain identifier that supports multiple source chains. Define how third-party issuer provenance is represented.

**P6 — Issuer identity and powers change on re-issue**
- **Severity:** major.
- **Where:** Step 3; R4; the flags in the inventory.
- **What's wrong:** On PFTL, the migration issuer becomes the issuer of every migrated NFT. Whether ISSUER_BURNABLE is set and who controls the issuer fee are unstated. The platform could gain burn or fee power over user-owned NFTs.
- **How to investigate:** Read `NftMintOperation` to see whether a mint can name an issuer other than the signer, or delegate minting.
- **Suggested fix:** Specify the flags for migrated NFTs: no ISSUER_BURNABLE and zero issuer fee by default. Record the original issuer in `origin`. Alternatively, have the user's ML-DSA account mint with a migration attestation from the service.

**P7 — The quantum-safety argument ignores the migration bridge**
- **Severity:** minor/major.
- **Where:** R2.
- **What's wrong:** Ownership on PFTL is post-quantum, but entitlement is derived from classical old-chain signatures, and the hash function behind `metadata_hash` isn't named.
- **Why it matters:** The founder wants an intelligible quantum-resistant story. A reviewer will ask about the classical-to-post-quantum transition.
- **How to investigate:** Determine the old-chain key types and whether owners' public keys are exposed. Determine the hash used for `metadata_hash`.
- **Suggested fix:** In the spec, name the hash (for example SHA-256 or SHA3/BLAKE3), discuss quantum risk during the migration window, and set a cutover deadline after which old-chain claims close.

**P8 — The canonical encoding for `metadata_hash` is undefined**
- **Severity:** minor/major.
- **Where:** R1.
- **What's wrong:** The page specifies both a `.proto` and a canonical JSON form. It doesn't say which is hashed, whether deterministic protobuf is relied on (it isn't canonical across implementations), or how `metadata_hash` relates to the metadata CID.
- **How to investigate:** Check what canonicalisation Pillar 2 specifies, if any.
- **Suggested fix:** Define one canonical byte encoding and the hash algorithm, and provide test vectors. Either state that `metadata_hash` equals the CID's multihash digest, or explain why the two differ.

**P9 — "Supports NFTs in a smart way" is not defined**
- **Severity:** major (intent).
- **Where:** Bubbles-up line; overall scope.
- **What's wrong:** The page delivers parity plus migration. It doesn't articulate what makes NFTs smart in Post Fiat's design.
- **How to investigate:** Look for NFT roles in other pillars: identity (P6), validator linkage (P3), Finance Hub access (P9), message-protocol attachments (P2).
- **Suggested fix:** Add a short "smart NFTs" section that defines one or two native uses, for example NFTs as an identity or credential anchor linked to Nostr and validator keys, or NFTs that carry encrypted message-protocol payloads. Ask the founder to confirm.

**P10 — R7 sequencing conflicts with the chain-migration plan**
- **Severity:** major.
- **Where:** R7; step 5; acceptance criterion 3.
- **What's wrong:** "From now on" new mints go to PFTL. But the founder says users can't easily be migrated until the chain performs adequately, so most users won't have PFTL wallets yet.
- **How to investigate:** Check the migration-phase plan and the dependency on Pillar 5's local wallet and new-chain key provisioning in Task Node.
- **Suggested fix:**
  - Gate R7 behind a feature flag per user, triggered when the user has a PFTL account.
  - In the meantime, mint on the old chain with the new metadata schema, so those NFTs are already prepared for migration.
  - List Pillar 5 and wallet provisioning as dependencies.

**P11 — Economics of re-minting and pinning are absent**
- **Severity:** major.
- **Where:** Dependencies; open questions.
- **What's wrong:** The page doesn't say:
  - who pays mint fees and any reserves for migrated NFTs;
  - who funds long-term IPFS pinning of image and metadata CIDs;
  - whether "pin on the new chain" implies validator-hosted pinning;
  - how mint spam is prevented.
- **How to investigate:** Check the PFTL fee and reserve model for NFT objects, and the costs in the IPFS infrastructure rebuild.
- **Suggested fix:** Add an economics subsection covering:
  - a migration-issuer budget for fees;
  - a pinning-responsibility policy (and the redundancy that implies);
  - mint fees or rate limits for new mints;
  - how the issuer-fee decision fits with Deliverable 7's fee transparency.

**P12 — No adversarial or stress testing in the acceptance criteria**
- **Severity:** major.
- **Where:** Acceptance criteria.
- **What's wrong:** The criteria only cover the happy path. The founder's readiness bar includes multi-model adversarial stress tests.
- **Suggested fix:** Add tests for:
  - a forged claim;
  - a claim for an NFT the claimant doesn't own;
  - a transfer after the snapshot;
  - concurrent claims for the same NFT;
  - a metadata hash mismatch;
  - oversized URI or collection fields;
  - gateway outage.

  Add a test of migration at batch scale, and an adversarial review of the migration design by multiple models.

**P13 — Hidden dependencies and existence assumptions**
- **Severity:** minor.
- **Where:** Dependencies; R8; R9.
- **What's wrong:** Three things the page relies on may not exist:
  - "New-chain explorer" may not exist yet.
  - Pillar 2's post-quantum encryption suite may be unbuilt.
  - Marking old copies "migrated" requires an explorer that also indexes the old chain.

  R8, encrypting prompt assets, is mild scope creep.
- **How to investigate:** Confirm the explorer repository and its status, and confirm Pillar 2's encryption status.
- **Suggested fix:** Mark each dependency as built or unbuilt with an owner. Make R8 conditional, or move it to Pillar 2 or 5.

**P14 — Code references should be verified**
- **Severity:** minor.
- **Where:** What exists today.
- **What's wrong:** Some references are odd and should be checked, for example `NftDefinition` living in `market_nav_asset_types.rs`, and the exact RPC names. Parity items NFT-001 to NFT-008 being complete should also be confirmed against `xrpl-feature-parity-burndown.md`.
- **Suggested fix:** Spot-check each linked file and symbol, and correct any paths.

**P15 — Open questions omit key decisions**
- **Severity:** minor.
- **Where:** Open questions.
- **What's wrong:** The list is missing:
  - whether to include XRPL mainnet and third-party NFTs (P5);
  - a cutover deadline for claims (P7);
  - who is the PFTL issuer of migrated NFTs (P6);
  - who funds pinning (P11);
  - what "smart" means (P9).
- **Suggested fix:** Add these so the founder decides them explicitly.
