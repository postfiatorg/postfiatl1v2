# Feedback — Pillar 4 — The NFT System

*Execution-hole audit · Round 4.*

> **Mean score:** 76/100 · Gemini 4 Neon 73 · Claude Opus 5.5 80
>
> **Method:** each model received (1) the founder's original prompt, (2) locked founder decisions & live state, (3) master `README.md`, and (4) [this pillar's spec](../../pillars/04-nft-system.md), once, scored across 5 explicit rubric dimensions (Execution Holes /30, Economics /20, Founder Alignment /20, Milestones & 4 Gates /15, Readability /15). Run 2026-11-26 23:35 UTC via OpenRouter.

## Gemini 4 Neon — 73/100

*Model `google/gemini-4-neon` · 17062 output tokens · `tech=15/30 econ=12/20 align=19/20 ship=13/15 read=14/15`*

#### Summary judgement

Pillar 4 is one of the cleanest-written specs in the plan: it is concise, readable, aligned with every locked founder decision, and properly scoped around Pillar 2's message envelope and Pillar 8's permissionless-by-default model. The high-level burn-first migration flow, one-hash envelope rule, and recognition of existing L1 gaps (linear `Vec` scans, unenforced `ISSUER_BURNABLE`, missing `wallet_wasm` NFT signing) give engineering a strong starting point.

However, as written, the spec contains **three critical and four major execution holes** that would cause theft/laundering of official collection NFTs, permanently stranded user burns, burn-then-remint replay, consensus splits on live `postfiatl1v2`, RPC DoS, and a broken Explorer origin badge the moment the old chain shuts down at Stage 4. Every one of these holes can be closed cleanly with small, concrete additions before M1 starts.

#### Rubric breakdown

- **Execution Holes & Technical Robustness (15/30):** Good architectural foundation, but missing cryptographic binding on `claim_cid` in `origin`, missing burn-tombstone/serial-uniqueness consensus specs, un-gated L1 state/rule upgrades, unspecified `pf.nft.provenance.v1` mechanics, and Explorer SSRF/XSS + post-Stage-4 breakage.
- **Economic & Incentive Coherence (12/20):** Zero issuer fee and Task Node-sponsored migration/pinning fit the product, but lacking an old-chain issuer allowlist and sponsorship caps lets anyone launder junk NFTs into the official collection and drain account-creation gas and IPFS storage; unsolicited mint spam still shifts burn gas and quota risk onto victims.
- **Founder Decision Alignment & Zero Open Questions (19/20):** Faithfully implements every locked founder decision with zero open questions; minor deduction because the Stage 4 "closes classical-key attack surface" claim is undermined while the same hot issuer key can still mint `origin`-bearing NFTs indefinitely.
- **Actionable Milestones & 4 Ship Gates (13/15):** Concrete M1–M5 checklists, real code links, and all four mandatory ship gates; loses points where M1 omits required consensus activation, state migration, secondary indexes, and serial tombstones that M3 and M5 depend on.
- **Plain-English Readability & Zero Jargon Bloat (14/15):** Crisp, clear plain English with a helpful flow diagram and zero "claudese" bloat; minor deduction only where pin-only provenance and verification rules are left too hand-wavy to implement unambiguously.

#### Execution holes

##### H1
- **Severity:** `critical`
- **Where:** Detailed specification §1 (`origin` fields), §2 (Steps 2–3), §3 (Explorer NFT page)
- **Hole:** `pf.nft.metadata.v1.origin` records only `claim_hash` — not the signed `pf.nft.claim.v1` blob/CID — and Step 2 has the user burn on the old chain *before* pre-validating eligibility and persisting the signed claim.
- **Why it breaks execution:**
  1. **Hijacked-burn forgery passes Explorer verification:** Because `claim_hash` is just a digest and `origin` omits the signed claim payload, a compromised migration service (or buggy relayer) can take Alice's valid old-chain burn, mint the new NFT to an attacker's address, copy Alice's `burn_tx` and `claim_hash` into `origin`, and the Explorer's old-chain RPC check will still display **`origin: checked`**.
  2. **Issuer-forced burn / failed `tec*` bypass:** On XRPL forks, failed `tec*` transactions are validated on-ledger, and issuers can burn user NFTs if `tfBurnable` was set. Checking only "burn is validated" without verifying `tesSUCCESS` and `tx.Account == actual holder` allows bogus or non-owner burns to mint.
  3. **Irreversibly stranded user NFTs:** If a user burns first and the migration service later rejects the NFT (ineligible issuer, bad metadata) or crashes/wipes its DB before minting (and the user's browser tab closed), the old NFT is permanently destroyed with no claim payload left to resume.
- **Minimal plain-English fix:** Require a pre-flight step that validates eligibility and pins the signed `pf.nft.claim.v1` envelope to IPFS **before** prompting `NFTokenBurn`. Add `claim_cid` to `origin`. Define Explorer's *checked* status to verify all four links: (1) `burn_tx` returned `tesSUCCESS` and removed `nftoken_id` from `tx.Account`'s own `NFTokenPage` (not an issuer-forced burn), (2) `burn_tx` memo equals `SHA3-384(claim_bytes)`, (3) `claim` carries a valid ML-DSA-65 signature matching `destination` and lists `(source_chain, nftoken_id)`, and (4) `destination` matches the initial mint recipient on `postfiatl1v2`.

##### H2
- **Severity:** `critical`
- **Where:** Locked-in design decisions (Collection issuer, Gas & pinning), Detailed specification §2 (Steps 1–3)
- **Hole:** No explicit old-chain issuer allowlist, ledger cutoff, or sponsorship caps gate who can migrate into the single official Task Node collection or consume Task Node-sponsored gas, account creation, and IPFS pinning.
- **Why it breaks execution:** Anyone can permissionlessly mint arbitrary junk NFTs on the old XRPL-fork chain right now. Without a hard allowlist of qualifying old-chain issuers/NFTs, an attacker can mint thousands of fake NFTs on the old chain, burn them with valid claims to thousands of fresh `postfiatl1v2` addresses, and simultaneously (a) launder fake NFTs into the **official verified Task Node collection**, (b) drain Task Node's sponsored account-creation/gas wallet, and (c) force Task Node to permanently pin arbitrary gigabytes of junk on IPFS.
- **Minimal plain-English fix:** Publish a strict allowlist of qualifying old-chain Task Node / PFTasks issuer addresses (plus any founder-approved historical inventory IDs) and a snapshot ledger cutoff for official-collection migration. Cap sponsored account creation to one destination address per verified old-chain account (or authenticated Task Node user), and enforce strict byte-size and MIME-type limits before pinning any migrated asset.

##### H3
- **Severity:** `critical`
- **Where:** Detailed specification §2 (Step 3), §4 (L1 scaling), M1
- **Hole:** Chain-enforced duplicate prevention via `serial = first_8_bytes(SHA3-384(...))` has a burn-then-remint replay flaw, omits canonical hashing/encoding rules, and is missing from M1's L1 consensus work.
- **Why it breaks execution:**
  1. **Burn-then-remint replay:** If L1 removes burned NFTs from its live keyed map on `nft_burn`, then after a holder intentionally burns their migrated NFT, re-running the migration service (or replaying the permanent old-chain burn tx) **re-mints the exact same NFT from the dead burn tx**, breaking scarcity and idempotency.
  2. **Unspecified L1 mechanism & namespace collision:** M1 never specifies adding caller-supplied `serial` support or `(issuer, taxon, serial)` uniqueness checks to `nft_mint`, never separates migrated serials from fresh profile mint serials under the same official Task Node issuer, and leaves byte encoding ambiguous (delimiting `source_chain` vs `nftoken_id`, raw vs hex `nftoken_id`, big-endian vs little-endian `u64`).
- **Minimal plain-English fix:** In M1, add a permanent minted-serial tombstone set keyed by `(issuer, taxon, serial)` that is never deleted on `nft_burn`; reserve a distinct `taxon` (e.g., `taxon = 1` migrated, `taxon = 2` native profile) under the official Task Node issuer; and specify canonical length-prefixed input encoding + big-endian `u64` extraction in `docs/specs/nft-v1.md` and L1 tests.

##### H4
- **Severity:** `major`
- **Where:** Locked-in design decisions (Collection issuer, Rollout & claim window), Detailed specification §4
- **Hole:** A single online ML-DSA-65 private key acts as the permanent issuer for both historical migrations and ongoing profile NFTs, with no key rotation, mint-authority revocation, or post-Stage-4 cutoff enforcement.
- **Why it breaks execution:** If the hot Task Node backend key is compromised, the attacker can mint unlimited official Task Node NFTs and squat unmigrated serials forever — with no way to revoke the compromised key without abandoning the official collection identity. Even without a breach, leaving migration mint authority indefinitely alive on that hot key contradicts the spec's claim that closing the window at Stage 4 closes the classical-key attack surface.
- **Minimal plain-English fix:** Isolate the issuer key in a dedicated signing service with strict per-hour mint caps; add an L1 **delegated/rotatable minter key** (or collection authority rotation) so a compromised hot minter can be revoked without splitting the official collection; and record a hard **Stage 4 migration cutoff block height** after which Explorer and clients reject any newly minted NFT claiming an old-chain `origin`.

##### H5
- **Severity:** `major`
- **Where:** Detailed specification §4 (L1 scaling), M1, Locked-in design decisions (Unsolicited spam NFTs)
- **Hole:** M1 changes live consensus rules and state layout without a protocol activation height or state migration, leaves `account_nfts`/`issuer_nfts` vulnerable to O(N) RPC DoS, and leaves a victim-griefing vector in permissionless minting.
- **Why it breaks execution:**
  1. **Consensus fork:** Replacing the `Vec` storage layout, enforcing `ISSUER_BURNABLE`, and rejecting unsolicited `BURN_LOCKED + TRANSFER_LOCKED` mints on live `postfiatl1v2` validators without a governance activation height and deterministic state migration will fork the network.
  2. **O(N) RPC DoS:** Swapping `Vec` for a single `nft_id -> NftRecord` map speeds up `nft_info`/`nft_transfer`/`nft_burn` but leaves `account_nfts` and `issuer_nfts` doing full-chain scans — letting anyone CPU-DoS Pillar 3's public RPCs by minting many NFTs and querying those endpoints.
  3. **Quota / reserve griefing:** Blocking only `BURN_LOCKED + TRANSFER_LOCKED` still lets an attacker permissionlessly mint thousands of normal junk NFTs directly into a victim's wallet — filling the victim's owned-object cap (Pillar 2) or locking recipient reserves (if any) and forcing the victim to pay gas to burn the spam.
- **Minimal plain-English fix:** Activate M1's L1 changes at a governance activation block with an atomic `Vec → keyed maps` migration; maintain secondary indexes `(owner, nft_id)` and `(issuer, nft_id)` with hard-capped cursor pagination on `account_nfts`/`issuer_nfts`; charge any NFT storage deposit/fee solely to the *minter* (never locking recipient PFT); and ensure unsolicited incoming NFTs cannot block a recipient's own owned-object cap.

##### H6
- **Severity:** `major`
- **Where:** Locked-in design decisions (Migration scope, Retirement mode), Detailed specification §2 (Optional pin-only mode), §3 (Dual-chain UI)
- **Hole:** Optional pin-only provenance (`pf.nft.provenance.v1`), including for external XRPL mainnet NFTs, specifies neither its on-chain anchor mechanism nor how ownership is proven without a burn.
- **Why it breaks execution:** Without a defined anchor transaction type, engineers cannot tell whether `pf.nft.provenance.v1` is an `nft_mint` (which would make it a transferable live NFT, contradicting "never displayed as a migrated live asset") or a Pillar 2 message transaction. Worse, without a burn transaction, there is no cryptographic proof that the claimant actually owns the unburned old-chain or XRPL mainnet NFT — allowing anyone to record provenance claims over someone else's high-value XRPL mainnet NFTs — and no disclosure when the original NFT is later transferred on its source chain.
- **Minimal plain-English fix:** Anchor `pf.nft.provenance.v1` strictly as a signed Pillar 2 message transaction (never an `nft_mint`). Require a signed classical-key ownership challenge from the source-chain holder binding `(source_chain, nftoken_id, snapshot_ledger, pftl_address)`, verify current ownership at that ledger before displaying *Recorded*, and label it in UI/Explorer as *"Unburned snapshot provenance (check source chain for current owner)"*.

##### H7
- **Severity:** `major`
- **Where:** Detailed specification §1 (one-hash rule), §3 (Explorer NFT page), Master README Stage 4
- **Hole:** Explorer's live origin verification depends permanently on old-chain history RPC (which dies at Stage 4), omits the `envelope.sender == nft.issuer` check, and lacks SSRF/XSS/size guards for fetching and rendering untrusted `metadata_uri` and `image` fields.
- **Why it breaks execution:**
  1. **Post-Stage-4 breakage:** Once the old XRPL-fork chain is frozen and its RPC nodes are shut down after Stage 4, live old-chain RPC queries fail and every migrated NFT loses its *checked* origin status.
  2. **Issuer spoofing:** Checking only `SHA3-384(envelope) == metadata_hash` and "valid envelope signature" without enforcing `envelope.sender == nft.issuer` and `encryption == none` allows an envelope signed by a different key to pass as verified.
  3. **SSRF, OOM, and stored XSS on Explorer:** Fetching arbitrary `metadata_uri` schemes (`http://169.254.169.254/...`, huge payloads exceeding IPFS single-block raw CIDv1 limits) or rendering untrusted SVG/HTML `image` CIDs directly on the Explorer origin enables server-side request forgery and stored XSS against Explorer users.
- **Minimal plain-English fix:** Pin a compact, self-contained old-chain burn proof (signed `burn_tx`, tx metadata, and ledger inclusion proof anchored to the published Stage 4 final ledger hash) alongside `claim_cid` so Explorer verifies origin offline forever. Require `envelope.sender == nft.issuer` and public encoding; restrict Explorer metadata/media fetches to `ipfs://<CIDv1>` via the configured IPFS gateway with a `<= 256 KiB` cap on raw metadata envelopes; and render artwork only from a sandboxed media origin restricted to safe raster MIME types (no inline SVG/HTML execution).

## Claude Opus 5.5 — 80/100

*Model `anthropic/claude-opus-5.5` · 7010 output tokens · `tech=20/30 econ=14/20 align=19/20 ship=14/15 read=13/15`*

#### Summary judgement

This is one of the tighter pillar specs. It honors every locked decision: v1 scope is old-chain NFTs only, migration is burn-first with optional pin-only, there is one official issuer with `ISSUER_BURNABLE` off, `TRANSFERABLE = true` and a 0 PFT fee, and Task Node pins all official metadata. It has no open questions. The one-hash rule (`metadata_hash` is the hash of the exact raw envelope bytes) is correct. The deterministic-serial idempotency idea is sound. Milestones and the four ship gates are concrete.

The holes are in the migration trust boundary and the state machine around it. Four gaps stand out:
- **Eligibility:** the spec never says which old-chain NFTs are eligible for the official collection.
- **Burn verification:** it never says which burn signer is accepted.
- **Burned-ID memory:** it never says whether the chain remembers burned IDs, so "the chain rejects duplicates" may not survive a holder burning a migrated NFT.
- **Single key:** the official issuer key is a single hot, unrotatable point of failure.

Unsolicited mint spam is also handled only at the UI layer, not in state or fees.

#### Rubric breakdown

- **Tech (20/30):** The idempotent serial, claim-plus-burn-memo binding and adversarial test list are good. Five things are missing: an eligibility allowlist, a check on who signed the burn, tombstones for burned IDs, separated serial ranges, and key rotation for the issuer.
- **Econ (14/20):** Gas sponsorship and pinning are sensible. However, sponsorship plus an unspecified eligibility rule is a drainable subsidy. Permissionless transferable spam mints still bloat recipients' storage and count against their owned-object cap.
- **Align (19/20):** Every locked decision is reflected faithfully. The extra operational defaults are reasonable and do not contradict anything.
- **Ship (14/15):** M1–M5 are concrete and the four gates are present and testable. "Zero duplicates on re-run" is a good acceptance test. It does not yet include the burn-then-mint resurrection test.
- **Read (13/15):** Plain English and well structured. A few field lists are dense, but there is no requirement-code soup.

#### Execution holes

**H1**
- **Severity:** critical
- **Where:** §2 Inventory & burn-first migration (Steps 2–3); Locked-in design decisions (gas sponsorship)
- **Hole:** The spec never restricts which old-chain issuers or taxons are eligible to be minted into the official Task Node collection. Any NFT that is validated as burned with a valid claim memo appears to qualify.
- **Why it breaks execution:**
  - Old-chain NFT minting is cheap and permissionless.
  - An attacker can mint thousands of junk NFTs on the old chain, then burn each with a claim memo.
  - Task Node would then mint them into the "verified" official collection on its own dime, paying gas and new-account creation for each.
  - The result is a polluted flagship collection, a drained sponsor, and the explorer badging junk as official.
- **Minimal plain-English fix:**
  - Hard-code an allowlist of the old-chain Task Node issuer accounts and taxons. Only those NFTs migrate into the official collection.
  - Anything else gets pin-only provenance at most.
  - Rate-limit sponsored account creation per claimant.
  - Add an "ineligible issuer" case to the adversarial suite.

**H2**
- **Severity:** major
- **Where:** §2 Step 3 ("once old-chain history RPC confirms the burn is validated")
- **Hole:** The spec only checks that a burn exists and carries `claim_hash` in its memo. It does not require that the burn transaction was signed by the NFT's owner at burn time.
- **Why it breaks execution:**
  - On XRPL-style chains, the issuer can burn NFTs that were minted with the burnable flag.
  - If old Task Node mints set that flag, then the old issuer key, or any attacker who steals it, can burn users' NFTs with its own claim hash.
  - That redirects every migrated copy to an attacker-controlled new-chain address.
- **Minimal plain-English fix:** Accept a burn only if all of the following are true:
  - The burn transaction's signer equals the NFT's owner in the ledger state just before the burn.
  - The burned NFTokenID is listed in the claim.
  - The claim's destination address matches the new-chain key that signed the claim.
  - Otherwise, reject and flag the burn for manual review.

**H3**
- **Severity:** major
- **Where:** §2 Step 3 ("the chain itself rejects any duplicate mint"); §4 keyed lookups
- **Hole:** Duplicate prevention relies on the NFT existing on chain. If a holder later burns a migrated NFT on `postfiatl1v2` and the record is deleted, which is especially likely once the `Vec` is replaced by keyed storage, the same serial becomes mintable again.
- **Why it breaks execution:**
  - Suppose the migration database is wiped, the service is re-run, or a compromised service replays old valid burn proofs.
  - The service then re-mints NFTs that holders deliberately burned, resurrecting them.
  - This breaks the "one live copy, ever" guarantee and the founder's zero-duplicates acceptance test.
- **Minimal plain-English fix:**
  - Keep a permanent tombstone keyed by (issuer, serial) when an NFT is burned.
  - Make `nft_mint` reject any serial that is live or tombstoned.
  - Add a "burn migrated NFT, then re-run migration" case to the M3 tests and to the Tested-by-user gate.

**H4**
- **Severity:** major
- **Where:** §2 Step 3 (deterministic serial); §3 automatic new-chain minting
- **Hole:** Migrated NFTs use hash-derived 64-bit serials, but the spec never says how fresh Task Node mints pick serials under the same issuer.
  - If the chain auto-assigns serials sequentially, the "map 0 → 1" rule suggests a shared range.
  - If the chain auto-assigns serials and does not accept caller-chosen ones at all, the idempotency scheme does not work as written.
- **Why it breaks execution:**
  - A fresh mint can occupy a serial that a not-yet-migrated old NFT hashes to.
  - That makes the legitimate migration permanently fail with "duplicate", destroying a user's burned NFT with no replacement.
  - Alternatively, the serial scheme simply cannot be implemented against the current `nft_mint`.
- **Minimal plain-English fix:**
  - Confirm in M1 that `nft_mint` accepts an issuer-chosen serial.
  - Reserve the top bit: migrated serials always set it, fresh mints never do.
  - Document this split in `nft-v1.md` test vectors.

**H5**
- **Severity:** major
- **Where:** Locked-in decisions (one official issuer); §4 ("if the migration key were ever compromised")
- **Hole:** A single issuer account signs every migration mint, every fresh profile mint and every metadata envelope. It must be online for both services. The spec names no protection, delegation or rotation story, only after-the-fact flagging.
- **Why it breaks execution:**
  - A single leak lets an attacker mint unlimited genuine-looking official NFTs and signed envelopes.
  - If account keys cannot be rotated on L1, the collection is permanently poisoned, because the collection is defined by issuer identity.
- **Minimal plain-English fix:**
  - Hold the issuer key in an HSM or KMS.
  - Mint through a rate-limited signer that refuses mints lacking a verified burn proof or a Task Node mint record.
  - Confirm in M1 that the issuer account supports key rotation, or add rotation.
  - Have the explorer verify envelope signatures against the issuer key that was valid at the mint height, so rotation does not break old NFTs.

**H6**
- **Severity:** major
- **Where:** §2 Step 2 (claim and burn ordering)
- **Hole:** The burn is irreversible, but the spec does not require the signed `pf.nft.claim.v1` document to be stored durably before the burn is allowed.
- **Why it breaks execution:**
  - Suppose the client crashes or the claim upload fails after the user signs `NFTokenBurn`.
  - The burn memo then holds a hash that points to nothing.
  - The service cannot learn the destination address, so the NFT is destroyed with no replacement and burn-first strands the user.
- **Minimal plain-English fix:**
  - The UI must submit the claim to the migration service and pin it to IPFS, getting an acknowledgment, before it lets the user sign the burn.
  - Add a recovery path: if the user re-presents a claim whose hash matches a validated burn memo, mint at any time before Stage 4.

**H7**
- **Severity:** major
- **Where:** Locked-in decisions (unsolicited spam NFTs); §4
- **Hole:** The only spam controls are hiding unfollowed issuers in the UI and blocking mints that are both burn-locked and transfer-locked. Spam that is transferable or burnable still lands in the victim's state.
- **Why it breaks execution:**
  - Each spam NFT counts against the victim's owned-object cap, which Pillar 2 raises but keeps.
  - It also inflates `account_nfts` responses and forces victims to pay fees to burn junk.
  - An attacker could fill a target's object quota and block that target's FastPay or NFT activity.
- **Minimal plain-English fix:** This keeps minting permissionless per the locked decision.
  - Apply Pillar 2's per-new-object fee to `nft_mint` when the recipient is not the issuer, charged to the minter.
  - Exclude unsolicited NFTs from the recipient's owned-object cap until the recipient accepts or transfers them.
  - Paginate `account_nfts` and support filtering by issuer.

**H8**
- **Severity:** minor
- **Where:** Locked-in decisions (claim window closes at Stage 4)
- **Hole:** Some burns may be validated just before the freeze but not yet minted when the window closes.
- **Why it breaks execution:** Those users lose their NFT at the cutover, with no defined way to finish the migration.
- **Minimal plain-English fix:** Any burn included in the published final-ledger-hash archive stays mintable after the freeze. Its proof is checked against the frozen archive. Only new claims or burns close at Stage 4.

**H9**
- **Severity:** minor
- **Where:** §2 optional pin-only mode; §3 explorer origin status
- **Hole:** Pin-only provenance records can reference external XRPL mainnet NFTs. Origin is only ever "checked" against old-chain history RPC, so nothing proves that the record's creator owns the referenced mainnet NFT.
- **Why it breaks execution:** Anyone can publish provenance records claiming famous XRPL NFTs, which invites spoofed-heritage scams.
- **Minimal plain-English fix:**
  - Require a signature from the current mainnet owner's key over the provenance record.
  - Show mainnet records as *not checked* unless that signature verifies against mainnet state.
