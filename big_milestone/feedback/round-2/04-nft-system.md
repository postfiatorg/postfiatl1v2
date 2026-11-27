# Feedback — Pillar 4 — The NFT System

*Review round 2.*

> **Mean score:** 74/100 · Gemini 4 Neon 73 · Claude Opus 5.5 76
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/04-nft-system.md), once, with the question below. Run 2026-11-26 20:38 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 73/100

*Model `google/gemini-4-neon` · 19809 output tokens*

#### Summary judgement

This is a well-grounded, disciplined planning document that captures the founder's core intent for Deliverable 4 and demonstrates real code-level verification of both `postfiatl1v2` and `tasknode`. Round-1 revisions noticeably improved the spec: distinguishing the 79 CIDs from actual NFT counts, requiring an inventory before migration, switching to a single hashed `pf.msg.v1` byte stream, flagging the inert `ISSUER_BURNABLE` flag, and naming the classical-key claim window as a post-quantum weakness are all strong moves.

However, the spec is **not yet safe to build from**. Beneath its crisp prose sit four critical flaws that would cause security failures, permanent user asset loss, L1 denial-of-service, or engineering deadlock:

1. **The stolen migration-issuer defense does not work as specified** because `origin` records `claim_hash` without anchoring the signed `pf.nft.claim.v1` payload, letting a compromised issuer copy an honest user's burn and mint to an attacker while still passing explorer verification.
2. **Burn-first migration has an irreversible asset-loss race** (no mandatory pre-burn persistence/account check, plus non-deterministic re-signing risk) and an **XRPL `tfBurnable` theft vector** (an old issuer can burn a transferred NFT on XRPL unless the validator checks `Account == current_owner`).
3. **An unresolved issuer-model contradiction runs through the entire document**: R8, the diagram, the security model, and AC4 require a single Task Node migration issuer, while AC5 and R13 assume user-as-issuer for browser mints, and Open Questions leaves the choice wide open.
4. **Known `O(N)` `Vec` linear scans on a permissionless L1** are treated as a conditional benchmark ("If cost grows...") rather than a mandatory fix, paired with uncalibrated mint fees ("10 base units" with no denomination/USD grounding) and an unmitigated permanent unsolicited-NFT griefing vector (`BURN_LOCKED | TRANSFER_LOCKED`).

Fixing these issues—and turning the 13 raw open questions into opinionated recommendations with a clean dependency phase split—will bring this spec to build-ready quality.

#### Correspondence to the founder's intention

Overall alignment with the founder's prompt is **high (~85%)**:

- **Historical NFT migration and pinning:** Directly addressed via inventory-first discovery (R5), burn-first or snapshot migration (R7), batch execution (R8), and pin-only provenance records (R10).
- **Intelligible quantum-resistant spec and blog post:** Directly addressed via `docs/specs/nft-v1.md` and a companion blog post on `postfiatorg.github.io`, honestly framing ML-DSA-65 ownership, SHA3-384 commitments, and the classical-key migration window (R3, R9).
- **Native contiguity with Pillar 2 (Message Protocol Spec):** Directly addressed by modeling NFT metadata as `pf.nft.metadata.v1` inside a signed `pf.msg.v1` envelope with a shared `envelope_hash` (R1, R2).
- **Adversarial multi-model review and local signing:** Explicitly integrated via R13 (`wallet_wasm` shared with Pillar 5) and R18 (adversarial test suite plus multi-model review gate).

Where alignment weakens:

- **"Supports NFTs in a smart way" is slightly undersold and overclaimed at the same time:** The section *"What 'smart' means here"* opens with *"The chain already matches XRPL's NFT features"*, which contradicts the document's own tables (XRPL has native NFT offers, brokered sales, and percentage royalties; `postfiatl1v2` does not). Meanwhile, the spec does not examine how NFT metadata or gated media might interact with Pillar 2's Fastpay / X402 integration points mentioned in Deliverable 2, or with Pillar 6's *"I minted my Task Node Identity"* framing in Deliverable 6. Keeping v1 scope narrow is smart engineering, but the document should be honest about XRPL deltas and clearer on how v1 schemas connect to those adjacent deliverables.
- **Decision burden on the founder:** Presenting 13 unprioritized open questions—several of which have clear technical answers or contradict assumptions already embedded in R6–R15—asks the founder to resolve engineering footguns (e.g., dead chain flags, self-transfer fee traps, unsolicited locked spam) rather than approving concrete proposals.

#### Technical robustness

The code audit is a major asset: verifying exact field limits (`collection_id ≤ 64`, `metadata_hash 1–64`, `metadata_uri ≤ 256`), execution semantics (`nft_mint` straight to `owner`, inert `ISSUER_BURNABLE`, no memos on NFT transactions), and the naming hazard (`pftl-*` meaning old chain in Task Node code) prevents expensive surprises.

Still, several concrete protocol and systems flaws remain:

- **Cryptographic provenance gap (R1, R6, R15):** Because `burn_tx` only carries `claim_hash` and `origin` does not store a `claim_uri` pointing to the signed `pf.nft.claim.v1` bytes, neither the explorer nor any third party can verify that `claim_hash` actually authorized minting to the initial `mint_tx.owner`. A stolen migration-issuer key can replay any valid burn and redirect the mint to an attacker while displaying *origin: checked*.
- **Burn-first liveness & XRPL `NFTokenBurn` semantics (R6, R7, R8):** On XRPL, `Flags: 9` sets `tfBurnable`, letting the original issuer burn an NFT owned by someone else via the `Owner` field. Validating a burn without verifying the signer was the *holder* (not merely the issuer) enables theft of transferred NFTs. Additionally, burning before verifying `owner` account existence on the new chain and before durably pinning `pf.nft.claim.v1` risks permanent NFT destruction—especially since ML-DSA-65 signing is hedged (non-deterministic) by default, so re-signing after a crash produces a different `claim_hash` than the one etched into the burn memo.
- **Issuer-model contradiction (R4, R8, R12, R13, AC4, AC5):** Single migration issuer vs. per-user issuer changes who signs `nft_mint`, who signs `pf.msg.v1`, whether deterministic serials enforce global uniqueness, and whether migrated and newly minted profile NFTs share one collection. The spec currently mixes both models across requirements.
- **Consensus-layer scaling bug treated as optional (Chain Semantics, R16):** Confirming that NFTs live in a `Vec` with linear scans and then writing *"If cost grows with the number of NFTs..."* ignores basic complexity and adversarial reality. On a permissionless L1, linear-scan lookups are a deterministic DoS vector regardless of how small the initial honest inventory is.
- **Explorer durability and missing history API (R15, Dependencies, AC6):** Checking origin live against `pftl_history_rpc` means every migrated NFT's *checked* badge silently fails once the old chain is sunset. Separately, AC6 requires displaying NFT history, yet the documented new-chain RPCs only expose current-state queries (`nft_info`, `account_nfts`, `issuer_nfts`).
- **Unspecified canonical protobuf rules and collision handling (R2, R8):** Calling for "deterministic protobuf" across Rust/JS/Python without defining canonical constraints (and reconciling with R1's "unknown fields are preserved") invites cross-implementation hash mismatches. In R8, catching `duplicate_nft` on chain does not distinguish an idempotent retry (skip) from an actual 64-bit serial collision (failure) unless the service verifies the on-chain NFT's `origin.nftoken_id`.

#### Economic coherence

The Economics section accurately records the chain's fee mechanics (byte-based burned fees, 10-unit one-time mint fee, no ongoing reserve, no trust lines) and flags the right ownership questions (payer, pinning, fees, spam). However, it is **not economically coherent enough to ship on a permissionless L1**:

- **Uncalibrated state-expansion pricing:** A "10-unit state-expansion fee" with no ongoing reserve is meaningless without stating what one base unit is denominated in and what it costs in PFT/USD. If 10 base units is fractions of a cent, permissionless minting plus `Vec` storage and no consent requirement lets an attacker permanently bloat L1 state and flood user wallets for pennies. If it is high, batch-migrating dozens of historical NFTs becomes costly.
- **Off-chain verification/pinning DoS:** Permissionless minting lets any issuer point `metadata_uri` at an arbitrarily large IPFS payload. Because verifiers (explorer, Task Node, agents) must fetch the exact bytes to compute `SHA3-384`, an unbounded envelope size enables trivial memory/bandwidth DoS against gateways and explorers.
- **Self-transfer fee trap:** Leaving "issuer fee: zero or fixed PFT amount" neutrally open ignores that Chain Semantics already proved a fixed issuer fee is charged on *every* non-issuer transfer—including users rotating keys or moving between their own wallets. That is an economic tax on basic wallet hygiene and should be explicitly recommended as `0` for profile NFTs.
- **Long-term IPFS pinning incentives:** Today's 2-replica cluster + Pinata covers Task Node's small set of profile NFTs, but once minting is permissionless, the spec does not define a pinning boundary (e.g., Task Node pins only its own collections / migrated inventory up to size caps; third-party issuers self-pin).

#### Problems

##### P1 — Stolen migration-issuer defense is bypassable because `origin` omits the signed claim artifact
- **Severity:** critical
- **Where it occurs:** Technical requirements (R1, R6, R8, R15); Security model ("Theft of the migration-issuer key").
- **What is wrong:** `origin` stores `burn_tx` and `claim_hash`, and R15 says the explorer marks origin *checked* after reading the burn or snapshot from the old-chain history node. Neither `origin` nor R15 requires pinning or verifying the signed `pf.nft.claim.v1` payload itself, nor binding `claim.destination_account` to the initial `nft_mint` recipient (`mint_tx.owner`).
- **Why it matters:** If the migration-issuer key is compromised (or the migration service has a bug), an attacker can watch any honest user's old-chain `NFTokenBurn`, copy `burn_tx` and `claim_hash` from the memo, and mint the migrated NFT to the attacker's own ML-DSA account. An explorer that only checks `burn_tx.memo == origin.claim_hash` will mark the stolen NFT as *origin: checked*, completely defeating the stated defense in the Security model and R18.
- **How an agent should investigate it:** Trace the full verification graph from `nft_id` -> initial `nft_mint` transaction -> `metadata_uri` (`pf.nft.metadata.v1`) -> `origin.claim_uri` (`pf.nft.claim.v1`) -> old-chain `burn_tx` / snapshot state. Verify whether `nft_info` or chain history exposes the *initial* mint recipient (`mint_owner`) separately from the *current* `owner` (so verification remains valid after legitimate post-migration transfers).
- **Suggested direction for the fix:** Add `claim_uri` (IPFS CID of the signed `pf.nft.claim.v1` `pf.msg.v1` envelope) to `origin` in R1, require `claim_hash == SHA3-384(claim_bytes)`, and update R15/R18 so *checked* requires verifying all four links: (1) `SHA3-384(claim_bytes) == burn_tx.memo == origin.claim_hash`, (2) valid ML-DSA-65 signature on `claim_bytes` by `claim.destination_account`, (3) `claim.nftoken_id == origin.nftoken_id == burned_nftoken_id`, and (4) `claim.destination_account == initial_mint_owner`.

##### P2 — Burn-first migration has irreversible asset-loss traps and an XRPL `tfBurnable` theft vector
- **Severity:** critical
- **Where it occurs:** User story (steps 3–4); Technical requirements (R6, R7, R8, R18); Security model ("Claim by someone who isn't the owner").
- **What is wrong:** Three defects in the retirement/claim flow:
  1. Old Task Node NFTs have `Flags: 9` (`tfBurnable | tfTransferable`). On XRPL, `tfBurnable` allows the original *issuer* to submit `NFTokenBurn` with an `Owner` field targeting an NFT currently held by someone else. R7/R18 never specify rejecting burns where `tx.Account != burned_owner` (i.e., where `tx.Owner` is present and differs from `tx.Account`).
  2. Burn-first permanently destroys the old NFT before minting, yet R6–R8 do not mandate pre-burn invariants: confirming the destination account exists on the new chain *before* burning, and durably pinning/storing the exact signed `pf.nft.claim.v1` bytes *before* broadcasting `NFTokenBurn`. If the user's client crashes after burning and re-signs a claim, FIPS 204 ML-DSA-65's default hedged signing (or any timestamp/nonce) will generate a different `claim_hash`, permanently stranding the burned NFT.
  3. R6's alternative proof (`signWalletChallenge`) does not specify domain separation or binding to `claim_hash` (risking login-challenge replay) and only works for Task Node mnemonic wallets, leaving non-Task-Node wallets with no specified proof method for Snapshot or Pin-only modes.
- **Why it matters:** A single bug in burn validation enables theft of transferred NFTs, and a client crash or unfunded destination account during batch migration causes irreversible loss of historical founder/user NFTs.
- **How an agent should investigate it:** Inspect XRPL `NFTokenBurn` transaction/metadata schemas in `server/pftl-submit.js` and `pftl_history_rpc` (checking `tx.Account`, `tx.Owner`, and `meta.AffectedNodes` for deleted `NFTokenPage` ownership). Inspect `signWalletChallenge` in `tasknode/src/wallet-core.js` to see its exact signing domain and format, and check whether `crypto_provider` / `wallet_wasm` uses deterministic or hedged ML-DSA-65 signing.
- **Suggested direction for the fix:**
  - Require the burn verifier (and explorer) to confirm `tx.Account` was the actual pre-burn holder (reject issuer-initiated burns where `tx.Owner` is set and `tx.Owner != tx.Account`) and add this explicitly to R18.
  - Mandate a strict two-phase burn-first state machine: (a) preflight destination account existence + valid URI/CID, (b) create, sign, pin to IPFS, and durably persist `pf.nft.claim.v1` *before* submitting `NFTokenBurn` (using deterministic claim serialization and storing the signed blob server-side and locally).
  - Specify a domain-separated `pf.nft.claim_proof.v1` signature bound to `claim_hash` for Snapshot/Pin-only modes, plus an on-chain zero-value memo tx fallback for external wallets that cannot use `signWalletChallenge`.

##### P3 — Unresolved issuer-model contradiction across requirements, security model, and acceptance criteria
- **Severity:** critical
- **Where it occurs:** Technical requirements (R4, R8, R12, R13); Architecture diagram; Security model ("Double migration", "Theft of the migration-issuer key"); Acceptance criteria (4, 5); Open questions ("New-chain issuer").
- **What is wrong:** Open Questions asks the founder to choose between **one Task Node migration issuer** and **each user's own account** as the new-chain issuer. Yet the spec's own design contradicts the second option in half the document and contradicts the first option in the other half:
  - R8 ("The service mints straight to the claimant... The chain then rejects a second mint"), Security Model (deterministic serial prevents double migration; stolen migration-issuer key), and AC4 all **require a single Task Node migration issuer** (because `nft_id` is issuer-scoped and only the issuer can sign `nft_mint`).
  - Meanwhile, R12/R13/AC5 ("a user with a new-chain account mints a profile NFT on the new chain from the browser") imply **user-as-issuer** for new profile NFTs, which would put migrated and newly minted profile NFTs in completely different collections with different trust/display models—or else `wallet_wasm` isn't actually used to sign `nft_mint` for Task Node profile NFTs at all.
- **Why it matters:** Engineers cannot implement `nft_mint` flows, `wallet_wasm` UX, explorer collection verification, or duplicate prevention without knowing which issuer model governs migrated NFTs vs. new profile NFTs. Mixing them inadvertently fragments the official profile collection and invalidates AC4 or AC5.
- **How an agent should investigate it:** Check how Task Node currently models profile NFT ownership and issuer verification in `server/profile-routes.js`, `server/profile-nft-mint.js`, and `server/pftl-nfts.js`, and check how `explorer-spec.md` plans to identify official vs. user-created collections.
- **Suggested direction for the fix:** Pick a recommended default and align all sections to it—for example: **one dedicated Task Node issuer account** (with distinct collection IDs such as `tasknode.profile.migrated.v1` and `tasknode.profile.v1`, or one unified `tasknode.profile.v1`) for official profile NFTs (giving global chain-enforced duplicate prevention and a single canonical collection on the explorer), while still adding `nft_mint` / `nft_transfer` / `nft_burn` to `wallet_wasm` so users can self-issue arbitrary collections and manage transfers/burns locally. Rewrite AC5 and Open Questions so the implications are consistent.

##### P4 — L1 `Vec` linear scans and permanent unsolicited locked NFTs create critical DoS and griefing vectors
- **Severity:** critical
- **Where it occurs:** What exists today / Chain semantics ("Burn", "Network fees", "Scale"); Technical requirements (R16, R17); Economics; Open questions ("Unsolicited NFTs").
- **What is wrong:**
  1. Chain semantics confirms that NFTs live in a `Vec` with linear scans on every lookup, yet R16 says *"If cost grows with the number of NFTs, replace the linear scans with keyed lookups before Task Node switches over."* Linear scans *always* grow linearly (`O(N)`), and benchmarking at the current inventory (<79 NFTs) masks a consensus-level DoS attack on a permissionless L1 where anyone can cheaply mint tens of thousands of NFTs.
  2. Chain semantics confirms any issuer can mint into any existing account without consent, and a collection with `BURN_LOCKED | TRANSFER_LOCKED` can never be removed by its recipient. Leaving this to "wallets hide issuers the user doesn't follow" does not stop an attacker from flooding an account's on-chain `account_nfts` list (breaking RPC responses/pagination and explorer pages) with permanent, unburnable junk or hostile metadata.
- **Why it matters:** Pillar 4 is explicitly a readiness gate for Outcome B (*"a production-ready new chain"*). Shipping a permissionless L1 with `O(N)` consensus lookups and unburnable unsolicited mints violates basic production-readiness and adversarial resilience.
- **How an agent should investigate it:** Inspect `crates/types/src/market_nav_asset_types.rs` (`NftIndexes`, state layout), `crates/execution/src/nft_escrow_asset_execution.rs`, and `crates/node/src/rpc_dispatch.rs` (`account_nfts`, `issuer_nfts` pagination/limits) to quantify exact state-access complexity, serialization overhead, and RPC response limits.
- **Suggested direction for the fix:**
  - Make replacing `Vec` linear scans with indexed/keyed maps (by `nft_id`, `owner`, `issuer`) an **unconditional** requirement in R16 before mainnet/Stage-2 cutover, and add pagination to `account_nfts` / `issuer_nfts` plus an adversarial spam-mint stress test in R18.
  - Recommend an L1 protocol rule for R17 rather than pure wallet hiding: either require recipient opt-in/claim when `owner != issuer`, or forbid `BURN_LOCKED` when `owner != issuer` (so any holder can always burn an unsolicited NFT).

##### P5 — Explorer origin verification decays when the old chain retires, and NFT history has no confirmed data source
- **Severity:** major
- **Where it occurs:** Technical requirements (R15); Dependencies ("New-chain explorer", "Old-chain history RPC"); Acceptance criteria (6).
- **What is wrong:**
  1. R15 states origin is *checked* only after reading the burn or snapshot from an old-chain history node (`pftl_history_rpc`), and otherwise shows *not checked*. Once the old chain is decommissioned after migration completes, `pftl_history_rpc` will eventually be shut down (and may also lack browser CORS or full historical depth today), causing every migrated NFT's provenance status to permanently degrade to *not checked*.
  2. AC6 and R15 require the explorer to show NFT **history** (mint, transfers, burn), yet "What exists today" only lists state RPCs (`nft_info`, `account_nfts`, `issuer_nfts`)—no `nft_history` RPC or explorer indexer is identified.
- **Why it matters:** Migrated historical NFTs lose verifiable provenance right when the migration finishes (when the old chain is retired), and AC6 cannot be delivered without building an unbudgeted RPC or indexer.
- **How an agent should investigate it:** Check `postfiatorg.github.io/docs/explorer-spec.md` and explorer code to see whether the explorer is purely static client-side or has an indexer backend; check `pftl_history_rpc` CORS configuration and earliest available ledger index; check `rpc_dispatch.rs` for any transaction-history indexing by `nft_id` or account.
- **Suggested direction for the fix:**
  - Define an immutable archival proof bundle (e.g., signed old-chain ledger headers + XRPL transaction inclusion Merkle proofs for each `burn_tx`, or a signed final-snapshot manifest pinned to IPFS) so the explorer can verify provenance offline even after `pftl_history_rpc` is retired.
  - Explicitly add the missing data source for NFT history (`nft_history` RPC in `postfiatl1v2` or an explorer indexer table) to Technical requirements and Dependencies.

##### P6 — Pin-only provenance (`R10`) leaves two incompatible architectures and ignores ownership drift
- **Severity:** major
- **Where it occurs:** User story (step 5); Technical requirements (R7(c), R10, R15).
- **What is wrong:** R10 states: *"A pin-only item gets a `pf.nft.provenance.v1` message, or an NFT in a separately labelled collection. It is never shown as the live asset."* These are two completely different mechanisms (an off-chain/memo Pillar 2 message vs. an on-chain `nft_mint` object). Furthermore, neither addresses what happens when the original NFT on the old chain (or XRPL mainnet) is later transferred or burned: a static provenance record or NFT on the new chain immediately becomes stale and misleading.
- **Why it matters:** Implementers do not know whether to build a new `pf.nft.provenance.v1` schema + explorer view or a soulbound provenance collection, and users could trade or misinterpret a stale provenance NFT as a live wrapped asset.
- **How an agent should investigate it:** Check whether Pillar 2 supports standalone on-chain anchored messages (e.g., memo transactions on `postfiatl1v2`) and how the explorer plans to index/display non-NFT Pillar 2 messages vs. NFT collections.
- **Suggested direction for the fix:** Choose one concrete design for R10—prefer a **non-transferable (`!TRANSFERABLE`) provenance record** (or pure Pillar 2 anchored `pf.nft.provenance.v1` attestation, not a transferable NFT) explicitly stamped with *"Attested as of ledger X; live ownership remains on `<source_chain>`"*, with clear explorer display rules and optional re-attestation if ownership changes.

##### P7 — Deterministic serial collisions vs. idempotent retries and deterministic protobuf rules are underspecified
- **Severity:** major
- **Where it occurs:** Technical requirements (R1, R2, R8).
- **What is wrong:**
  1. R8 uses the first 8 bytes of SHA3-384 as the migration `serial` and states *"The chain then rejects a second mint... A collision is reported as a failure."* When `nft_mint` fails with `duplicate_nft`, the chain error alone cannot tell the batch runner whether the existing NFT is the *same* `NFTokenID` (an idempotent re-run that should be reported as *skipped*) or a *different* `NFTokenID` that collided on the 64-bit serial (a *failure*).
  2. R2 says *"Encoders emit deterministic protobuf, so the test vectors reproduce"*, while R1 says *"Unknown fields are preserved."* Standard protobuf does not guarantee canonical byte-for-byte encoding across languages (map ordering, packed vs. unpacked repeated fields, unknown-field interleaving, float NaN representations).
- **Why it matters:** Without explicit `duplicate_nft` disambiguation, re-running a batch after a DB wipe (AC4) could either falsely flag skips as failures or silently mask a collision. Without canonical protobuf rules, Rust (`wallet_wasm`), JS (Task Node), and Python (`wallet.py`) encoders will fail R2's test vectors.
- **How an agent should investigate it:** Check `NftMintOperation` in `crates/types/src/transactions_mempool_receipts.rs` for the exact type of `serial` (likely `u64`) and error codes in `nft_escrow_asset_execution.rs`. Check Pillar 2 (`02-message-protocol.md`) to see whether a canonical protobuf serialization profile already exists.
- **Suggested direction for the fix:**
  - In R8, specify that on `duplicate_nft`, the migration service queries `nft_info(nft_id)`, fetches and hash-verifies `metadata_uri`, and compares `origin.nftoken_id` + `origin.source_chain`: exact match -> *skipped (already migrated)*; mismatch -> *failed (serial collision)*.
  - In R2, reference (or define) strict canonical protobuf rules for `pf.nft.metadata.v1` (proto3, no `map<>` fields, canonical repeated-field ordering, no re-encoding during verification, and clarify that "unknown fields preserved" applies to forward-compatible decoding/display, never to mutating and re-hashing signed bytes).

##### P8 — Economics lacks base-unit calibration, verifier payload size caps, and permissionless pinning boundaries
- **Severity:** major
- **Where it occurs:** What exists today / Chain semantics ("Network fees"); Technical requirements (R2, R18); Economics; Open questions ("Who pays", "Who pins").
- **What is wrong:**
  1. "10-unit state-expansion fee" and `max(1, ceil(bytes/512))` base units are stated without defining the denomination or economic value of a base unit, making it impossible to evaluate whether minting is spam-resistant or affordable for batch migration.
  2. There is no maximum byte size specified for the off-chain `pf.msg.v1` metadata envelope fetched from IPFS during verification (R2). R18 tests *"oversized fields"* (on-chain), not oversized IPFS blobs.
  3. Economics notes Task Node's 2-replica IPFS cluster + Pinata today, without defining a pinning policy once permissionless minting opens on the new chain.
- **Why it matters:** Mispriced mint fees either enable cheap state bloat or price out users; unbounded IPFS metadata envelopes let any attacker crash or stall the explorer/verifiers via a multi-gigabyte CID; undefined pinning boundaries create an unbounded hosting liability for Task Node.
- **How an agent should investigate it:** Inspect `crates/execution/src/fees_offer_planning.rs` and tokenomics/chain config in `postfiatl1v2` to determine base-unit denomination (e.g., drops per PFT) and compare 10 base units against transfer fees and account-creation costs. Check `ipfs-infrastructure-rebuild.md` for current cluster storage/bandwidth limits and Pinata plan caps.
- **Suggested direction for the fix:**
  - Document exact base-unit denomination and PFT cost for mint/transfer/burn in Economics, and calibrate the state-expansion fee (or refundable per-NFT reserve) against spam-cost targets.
  - Add a hard spec limit in R2 (e.g., `pf.msg.v1` metadata envelope ≤ 64 KiB, enforced before full download/hashing) and add an oversized-IPFS-blob DoS test to R18.
  - Recommend a bounded pinning policy: Task Node pins its own collections and migrated inventory (with explicit media size limits); third-party issuers are responsible for pinning their own CIDs, and the explorer gracefully handles unpinned/oversized external CIDs.

##### P9 — False XRPL parity claim and 13 unrecommended open questions (including dead flag and self-transfer fee trap)
- **Severity:** major
- **Where it occurs:** What "smart" means here; Open questions for the founder.
- **What is wrong:**
  1. *"What 'smart' means here"* asserts *"The chain already matches XRPL's NFT features"*, which directly contradicts "What exists today" and "Chain semantics" (no NFT offers, no marketplace, no percentage royalties).
  2. Open Questions dumps 13 raw trade-offs on the founder with zero recommendations, including engineering defects that shouldn't be open product debates:
     - `ISSUER_BURNABLE` is currently accepted, stored, and reported by the chain while silently doing nothing in execution (a live security footgun).
     - A nonzero fixed `issuer_fee` taxes self-transfers between a user's own wallets.
     - Asking whether to include XRPL mainnet in scope without noting that Snapshot mode (R7(b)) is impossible on XRPL mainnet (Post Fiat cannot freeze XRPL mainnet).
- **Why it matters:** False parity claims mislead the founder and external blog readers; unrecommended open questions stall execution and risk choices that break invariants elsewhere in the spec.
- **How an agent should investigate it:** Review `docs/status/xrpl-feature-parity-burndown.md` to confirm exactly which XRPL NFT features are implemented vs. excluded, and check `nft_escrow_asset_execution.rs` for `ISSUER_BURNABLE` handling.
- **Suggested direction for the fix:**
  - Replace *"The chain already matches XRPL's NFT features"* with an accurate statement: *"The chain already covers core XRPL NFT lifecycle operations (mint, transfer, burn, collections, fixed fees); offers and percentage royalties are intentionally deferred."*
  - Split Open Questions into **(a) Recommended defaults for founder sign-off** (e.g., burn-first for old Post Fiat chain; single Task Node issuer for official profile NFTs; transferable; `issuer_fee = 0` due to self-transfer tax; reject `ISSUER_BURNABLE` at mint until explicitly implemented; disable `BURN_LOCKED` on unsolicited mints) and **(b) Genuine strategic choices** (XRPL mainnet pin-only scope, rollout order, credential/badge v2 roadmap, claim window close date).

##### P10 — Unhandled legacy-inventory edge cases, unverified chain IDs, and missing critical-path phasing
- **Severity:** minor
- **Where it occurs:** Technical requirements (R1, R5, R8); Dependencies.
- **What is wrong:**
  1. R5/R8 assume every old-chain NFT resolves cleanly to an image CID, without specifying behavior for non-Task-Node NFTs with HTTP/Arweave/hex/empty URIs, unreachable IPFS CIDs, or non-JSON metadata.
  2. R1 uses `xrpl:2025` as an example source chain ID without confirming the actual network ID of the old Post Fiat XRPL-fork chain, and without defining canonical `source_chain` identifiers (critical because `source_chain` is hashed into the deterministic migration serial in R8).
  3. Four major dependencies (Pillar 2, `wallet_wasm`, funded accounts policy, explorer NFT view) are unbuilt, yet Pillar 4 lacks an internal phase breakdown separating unblocked work from Pillar 2-blocked work.
- **Why it matters:** Edge-case URIs will crash or stall the founder's batch migration; changing `source_chain` strings later breaks deterministic serials; lacking a phase cut-line risks idling Pillar 4 engineers while waiting on Pillar 2.
- **How an agent should investigate it:** Check the old chain's `server_info` / network ID in `tasknode/docs/wiki/architecture/pftl.md` and `server/pftl-nfts.js`; check how `profile-nft-cid-repin.mjs` handles malformed URIs; cross-check Pillar 2's delivery sequence in `02-message-protocol.md`.
- **Suggested direction for the fix:**
  - Add fallback rules to R5/R8 for non-IPFS or broken-CID legacy NFTs (preserve raw `origin.original_uri`, flag in inventory report, route to pin-only or manual review rather than crashing the batch).
  - Pin canonical `source_chain` identifiers (and rename old-chain code references cleanly in spec/logs to avoid the "PFTL" naming collision).
  - Add a short 2-phase execution sequence: **Phase A (unblocked now):** run R5 inventory, fix L1 `Vec` indexing and `ISSUER_BURNABLE` in `postfiatl1v2`, confirm `pftl_history_rpc` depth/CORS, and draft `docs/specs/nft-v1.md` alongside Pillar 2's envelope freeze; **Phase B (after Pillar 2 + `wallet_wasm`):** ship migration service, explorer view, Task Node dual-read/mint, R18 tests, and blog post.

## Claude Opus 5.5 — 76/100

*Model `anthropic/claude-opus-5.5` · 8394 output tokens*

#### Summary judgement

This is a strong, code-grounded spec. It answers both halves of the founder's Deliverable 4:

- getting historical NFTs onto the new chain, or pinning them there;
- an intelligible quantum-resistant NFT spec, plus a blog post, that fits natively with the message protocol.

It also ties the work to the readiness gate "supports NFTs in a smart way". It checks real chain semantics in code, including the inert `ISSUER_BURNABLE`, issuer-scoped ids, the fact that mint goes straight to the owner, and the absence of memos. It also corrects an earlier overclaim: 79 is the number of CIDs, not of NFTs.

Its weaknesses are structural more than cosmetic:

- **Unresolved issuer model.** The core idempotency guarantee assumes a single migration issuer, yet the page leaves the issuer model open.
- **Provenance that expires.** Checking provenance depends on the old chain staying readable, which conflicts with retiring that chain.
- **Spam and state bloat.** The chain has cheap, reserve-free, permissionless minting, unsolicited NFTs that the holder can never remove, and linear-scan storage.
- **Narrow acceptance.** Acceptance is framed around the founder's own batch rather than all users' NFTs, which is what the readiness gate needs.
- **Blocked by Pillar 2.** The whole design hangs on Pillar 2, which is not built, and there is no interim path.

None of these is fatal, but several would cause failures in practice if implemented as written.

#### Correspondence to the founder's intention

**What the page gets right**

- **Migrate or pin.** Founder: "I need a way to get these new NFTs or pin them on the new chain." Page: the burn-first and snapshot modes migrate, and the pin-only mode keeps the original. Both options the founder named are covered.
- **Spec and blog.** Founder: "Intelligible quantum resistant NFT protocol spec … documentation and blog post." Page: R3 requires `docs/specs/nft-v1.md` with a post-quantum threat model, and a companion blog post. It is honest about the weak point, which is claims made with classical keys.
- **Fit with the message protocol.** Founder: "Natively contiguous with the MESSAGE PROTOCOL SPEC." Page: the metadata is a signed Pillar 2 envelope, and the on-chain hash covers exactly those bytes.
- **The readiness gate.** "Smart" is defined as four things: typed metadata, checkable provenance, policy fixed at mint, and prompts committed by digest.
- **The founder's process.** The page includes the multi-model adversarial review (R18) the founder asked for. The scope question about the "XRP" ambiguity (old Post Fiat chain versus XRPL mainnet) is raised appropriately.

**Gaps against intent**

- **Founder-centred acceptance.** The criteria cover the founder's batch. The readiness gate for migrating people requires every Task Node user's NFTs to move or be recorded, and no criterion covers that.
- **"Smart" is deferred.** Credentials, gated access to Finance Hub (Deliverable 9) and validator badges (Deliverable 3) are left as open questions. The founder's wider vision clearly links NFTs, identity (Deliverables 6 and 8) and validator access. A recommended default would align it better.
- **No sequencing.** There is no timeline against the chain-migration stages beyond scattered references to "README Stage 2/4". The founder's framing is "what activities maximise likelihood of success".
- **No Lean-proof component.** The founder wants formal and iterative studies for chain features. For NFTs, a small formal model of the retire-then-reissue invariant ("at most one live copy") would fit that ethos.

#### Technical robustness

**Strengths**

- The ordering "old copy retired and validated, then new copy minted" is correct.
- Hashing the exact fetched bytes avoids the canonicalisation traps of re-encoding protobuf.
- SHA3-384 fits within the 1–64 byte limit for `metadata_hash`.
- SHA-256 CIDs are used only as locators.
- The deterministic serial makes double-mints fail on chain.
- Unknown fields are preserved for forward compatibility.
- The "never overclaim" rule for showing origin in the explorer is sound.
- The adversarial test list is good.

**Weaknesses**

- The deterministic-serial idempotency claim (R8, criterion 4) is valid only when one issuer does all migration minting.
- Provenance cannot be checked once the old chain or its history node goes away.
- The burn-first mode has an unhandled failure state: the old NFT is burned but the new mint has not happened, because the account is missing, the memo is malformed or the service fails.
- `Vec` storage with linear scans, combined with permissionless low-fee minting, is a state-growth and denial-of-service risk.
- Collection flags are fixed irreversibly on first mint, with no required testnet rehearsal.
- There is a hard dependency on Pillar 2 and on `wallet_wasm` signing, both unbuilt.

#### Economic coherence

- **What is coherent.** Fees are byte-based and burned. NFTs need no trust lines, which matches Deliverable 8's "permissionless by default". The page notes that the fixed issuer fee is charged even on moves between one's own wallets.
- **No anti-spam pricing.** With no reserve and a 10-unit state-expansion fee, state bloat and unsolicited NFTs are close to free. XRPL uses owner reserves for exactly this reason. The page treats spam purely as a wallet display question.
- **Open economics with no defaults.** Payer, pinning, account funding and issuer-fee policy are all open questions with no recommended default. That leaves the migration's cost model undefined, though the costs are probably trivial at 79 CIDs.
- **No value story.** The page doesn't say why NFTs matter economically to Post Fiat, for example as access credentials or reputation. The "smart" section stays technical.

#### Problems

**P1 — Idempotency and uniqueness depend on an unresolved issuer model**
- **Severity:** critical
- **Where:** R8, Security-model row "Double migration", Acceptance criterion 4, and the Open question "New-chain issuer".
- **What is wrong:**
  - `nft_id` hashes the issuer. A deterministic serial therefore prevents a second mint only under the *same* issuer.
  - R8 says "the service mints", which implies one service issuer. The open question keeps "each user's own account" as an option.
  - Under per-user issuers:
    - the service cannot mint at all, because the issuer must sign;
    - the same NFTokenID could be re-issued under a different issuer account;
    - criterion 4 then can't be enforced on chain.
- **Why it matters:** The headline guarantee against double migration may not hold, depending on a decision the page has not made.
- **How to investigate:**
  - Confirm in `nft_id` and the `duplicate_nft` path that uniqueness is scoped per issuer.
  - Trace who signs `NftMintOperation`.
- **Fix direction:**
  - Recommend a single migration issuer with a dedicated collection, for example `tasknode.migrated.v1`, as the default.
  - If per-user issuance is kept, define a separate uniqueness mechanism and restate R8 and criterion 4 for each model. Options include a registry of claimed NFTokenIDs, or a chain-level `origin` index.

**P2 — Provenance stops being checkable once the old chain is retired**
- **Severity:** major
- **Where:** R7(b), R15, the dependency on the old-chain history RPC ("History depth not yet confirmed"), and the in-one-paragraph claim "provenance anyone can check".
- **What is wrong:**
  - The explorer marks an origin *checked* only by reading a history node live.
  - The founder's plan is to move away from the old chain. When it goes read-only or offline, every origin degrades to *not checked*, or the check can never be done.
- **Why it matters:** Provenance that only lasts while the old chain is alive defeats the point of migration.
- **How to investigate:**
  - Check the history depth of `pftl_history_rpc`.
  - Check whether the old-chain archive keeps full transaction metadata.
- **Fix direction:**
  - At migration time, archive the signed burn and mint transaction blobs, their metadata and the validated ledger headers, plus a validator-signed ledger hash. Pin them to IPFS and reference them in `origin`.
  - Define "checked" as verifiable against that archived evidence.
  - Optionally publish a final signed old-chain ledger hash.

**P3 — Burn-first has no recovery path for "burned but not re-issued"**
- **Severity:** major
- **Where:** R6, R7(a), R8 (the missing-owner "create account" step) and user-story step 3.
- **What is wrong:** In these cases the user's NFT is destroyed with no new copy and no defined state machine:
  - the memo is malformed, missing or doesn't match;
  - the destination account doesn't exist;
  - the service is down;
  - the claim message was never pinned.
- **Why it matters:** Each burn is irreversible. A single lost NFT, especially one of the founder's, breaks trust.
- **How to investigate:**
  - Walk through each failure case in R18 and look for the corresponding recovery behaviour.
- **Fix direction:**
  - Require the destination account to exist and the claim to be pinned *before* the burn is offered.
  - Accept any validated owner burn of the NFTokenID that comes with a matching ML-DSA claim, even if the memo is bad, using a `signWalletChallenge` fallback.
  - Define the states: pending, retired, minted and failed-recoverable.
  - Add these cases to R18.

**P4 — Cheap, reserve-free, permissionless minting enables spam, griefing and state bloat**
- **Severity:** major
- **Where:** the Chain semantics section (network fees, scale), R16, R17 and Economics.
- **What is wrong:**
  - A mint costs about 10 units plus a few bytes' worth of fee, and there is no reserve.
  - Any issuer can mint into any account.
  - A collection that is both `BURN_LOCKED` and `TRANSFER_LOCKED` can never be removed by its holder.
  - NFTs are stored in a `Vec` and every lookup is a linear scan.
  - Together these let anyone cheaply bloat state, degrade performance and permanently attach unwanted NFTs to accounts.
- **Why it matters:** The readiness gate requires the chain to perform adequately and to pass adversarial stress tests. Treating this only as a wallet-display question is insufficient.
- **How to investigate:**
  - Benchmark mint cost and lookup latency at 10⁵–10⁶ NFTs.
  - Compare with XRPL's owner reserve and NFT page design.
- **Fix direction:**
  - Recommend an owner reserve or a meaningful state-expansion fee.
  - Recommend an opt-in or consent rule for minting to a third party, or at least for minting into locked collections.
  - Make keyed storage a prerequisite rather than conditional on R16.

**P5 — The critical path runs through Pillar 2, which is not built, with no interim plan**
- **Severity:** major
- **Where:** R1, R2, R13, R14 and the Dependencies table.
- **What is wrong:**
  - The metadata envelope, schema registry, post-quantum suite and hash choice all depend on Pillar 2.
  - Wallet signing also depends on unbuilt `wallet_wasm` work.
  - No sequencing or minimal-viable path is given, such as freezing the `pf.msg.v1` envelope subset that NFTs need first.
- **Why it matters:** NFTs gate chain readiness. Waiting for the whole of Pillar 2 delays the chain migration.
- **How to investigate:** Check the Pillar 2 scope and timeline, and identify the minimal envelope fields NFTs require.
- **Fix direction:**
  - Define an "NFT-critical subset" of Pillar 2 to freeze first: public envelope, ML-DSA signature, SHA3-384 hash.
  - Make the migration service sign server-side so it doesn't wait for `wallet_wasm`, with user-device signing following later.

**P6 — Acceptance covers only the founder's batch, not all users**
- **Severity:** major
- **Where:** Acceptance criteria 2–3, and the Open question "Rollout".
- **What is wrong:**
  - The criteria cover only the founder's inventory.
  - There are no criteria for other users' migration, for unclaimed NFTs, or for what happens when the claim window closes.
- **Why it matters:** The founder's gate is about being able to migrate *people*.
- **How to investigate:** Cross-reference the README's Outcome B gate text.
- **Fix direction:**
  - Add criteria for:
    - a self-serve claim flow for any user;
    - a cohort-wide report;
    - the disposition of unclaimed NFTs at close, for example automatic provenance records.

**P7 — Collection policy becomes immutable at the first mint, with no rehearsal required**
- **Severity:** minor
- **Where:** the Chain semantics section ("Its first mint fixes the collection flags") and R4.
- **What is wrong:**
  - The first migrated NFT permanently fixes the flags of the official collection.
  - There is no step requiring a testnet or devnet dry run of the full batch.
- **Why it matters:** A mistake can't be undone, and would force a new collection id.
- **How to investigate:** Check whether a devnet with mirrored state exists.
- **Fix direction:** Add an acceptance step: a full dry run on devnet, plus the founder's sign-off on the flags before the first mainnet mint.

**P8 — Migrated and newly minted NFTs share a serial space in one collection**
- **Severity:** minor
- **Where:** R8 and R12.
- **What is wrong:**
  - Migration serials are hash-derived, while new mints presumably use some other serial scheme.
  - Collisions are improbable, but mixing the two schemes in one collection is fragile.
  - It is also unclear whether new mints use the same collection.
- **Why it matters:** It would cause confusing failures and blur the line between migrated and native assets.
- **How to investigate:** Check the serial width and how the generator assigns serials.
- **Fix direction:** Use separate collection ids for migrated and new profile NFTs, or reserve a serial range for each.

**P9 — The pin-only representation is ambiguous**
- **Severity:** minor
- **Where:** R10 and user-story step 5.
- **What is wrong:**
  - R10 allows "a message, or an NFT in a separately labelled collection".
  - A record that is itself an NFT can be traded or displayed as if it were the asset, which contradicts "never shown as the asset".
- **Fix direction:** Recommend a message-only provenance record, or a non-transferable record collection with explicit explorer labelling.

**P10 — The quantum-resistance story is incomplete for off-chain artefacts**
- **Severity:** minor
- **Where:** R3, R14 and the private prompt assets row in What exists today.
- **What is wrong:**
  - The prompt assets use classical encryption.
  - IPFS pinning infrastructure and gateway trust are classical.
  - The blog's "quantum-resistant" claim needs a crisp boundary statement, or readers will overgeneralise.
- **Fix direction:** Add a table to the spec listing each artefact and whether it is post-quantum, classical, or a commitment only.

**P11 — The "smart" uses are left without a recommended direction**
- **Severity:** minor
- **Where:** What "smart" means here, and the Open questions.
- **What is wrong:** Credential NFTs for validators and top-30 users connect directly to Deliverables 3 and 9, which give access to Finance Hub. They are left open, as is issuer burn, which revocable credentials need.
- **Fix direction:**
  - Recommend a default: credential NFTs are non-transferable and revocable.
  - Recommend implementing issuer burn behind a collection flag.
  - Cross-link to Pillars 3, 6 and 9.

**P12 — The page has no sequencing, timeline or effort framing**
- **Severity:** minor
- **Where:** the whole page.
- **What is wrong:** There is no phasing, for example: spec, then inventory, then service on devnet, then the founder's batch, then all users, then the window closes. Nor is there any mapping to the README stages.
- **Fix direction:** Add a phased plan with dependencies and rough effort estimates.

**P13 — The revision-notes table clutters a founder-facing page**
- **Severity:** minor
- **Where:** the final section.
- **What is wrong:** References to reviewer items ("Gemini P3", "Opus P14") mean nothing to the founder.
- **Fix direction:** Move the table to a changelog file.
