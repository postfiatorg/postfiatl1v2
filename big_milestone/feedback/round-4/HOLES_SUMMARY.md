# Execution Holes Summary — Round 4

*Generated 2026-11-26 23:35 UTC. Lists all `critical` and `major` execution holes flagged across all 10 pillars.*

## [Pillar 1 — Hive Chat Governance Module](01-hive-governance.md) — 75/100

- **[CRITICAL]** (Claude Opus 5.5 H1) The verifier "checks every signature" but nothing says which public keys count as the founder
- **[CRITICAL]** (Gemini 4 Neon H1) No authority binding or signer filtering allows explorer spoofing and exposes Vast key custody
- **[CRITICAL]** (Gemini 4 Neon H2) Cross-pillar wire-format collision with Pillar 2 (`pf.ptr` v4 vs. canonical protobuf) and unspecified 256-byte memo packing
- **[CRITICAL]** (Gemini 4 Neon H3) Sybil spam and facilitator turn-exhaustion DoS in `all_members` mode
- **[MAJOR]** (Claude Opus 5.5 H2) The explorer groups `GOVERNANCE` (kind 12) memos by `thread_id`
- **[MAJOR]** (Claude Opus 5.5 H3) Two parts of the inclusion rule depend on Task Node's private server state:
- **[MAJOR]** (Claude Opus 5.5 H4) The session lifecycle is undefined after the session closes:
- **[MAJOR]** (Claude Opus 5.5 H5) - The only limit is 1 message per 15 seconds per pubkey, and Nostr keys are free.
- **[MAJOR]** (Claude Opus 5.5 H6) The anchor format mixes generations of the protocol:
- **[MAJOR]** (Gemini 4 Neon H4) Missing session state machine, idempotency mechanism, and revision/abort rules
- **[MAJOR]** (Gemini 4 Neon H5) Relay backfill conflicts with server-receipt rate limiting and ordering
- **[MAJOR]** (Gemini 4 Neon H6) Snapshot block-height drift and overstated offline verification in `verify-governance-session`

## [Pillar 2 — Message Protocol Spec](02-message-protocol.md) — 75/100

- **[CRITICAL]** (Claude Opus 5.5 H1) FastPay crash-retry equivocation locks payout objects
- **[CRITICAL]** (Gemini 4 Neon H1) Malleable `certificate_digest` breaks x402 replay protection and canonical `tx` indexing
- **[CRITICAL]** (Gemini 4 Neon H2) Zero-balance new users cannot publish an X-Wing public KEM key before their first encrypted payout
- **[CRITICAL]** (Gemini 4 Neon H3) Non-canonical protobuf re-serialization breaks cross-language `Envelope` signatures and CIDs
- **[MAJOR]** (Claude Opus 5.5 H2) `payment_v2` fallback can double-pay
- **[MAJOR]** (Claude Opus 5.5 H3) "Task reward" badge is spoofable
- **[MAJOR]** (Claude Opus 5.5 H4) No encryption target for recipients without a published message key
- **[MAJOR]** (Claude Opus 5.5 H5) Spam economics are unquantified, and a chain-wide cap is a global kill switch
- **[MAJOR]** (Gemini 4 Neon H4) Async IPFS pinning lacks a durable outbox and a browser/NAT upload path, risking permanent dead CIDs
- **[MAJOR]** (Gemini 4 Neon H5) Hot-wallet 20-object FastPay pool has no crash-safe idempotency, risking object equivocation or double payouts
- **[MAJOR]** (Gemini 4 Neon H6) Unspecified FastPay object fees, global cap griefing, and cheap 2,048-input merges create DoS vectors
- **[MAJOR]** (Gemini 4 Neon H7) x402 PFUSDC consensus fallback lacks finality/retry/sequence rules, and facilitator lacks DoS defenses

## [Pillar 3 — Validator Rewards Program](03-validator-program.md) — 72/100

- **[CRITICAL]** (Claude Opus 5.5 H1) Participation counts only the votes that made it into the finalized quorum certificate (QC)
- **[CRITICAL]** (Gemini 4 Neon H1) `precommit_qc.votes` uptime metric systematically evicts geographically diverse validators
- **[CRITICAL]** (Gemini 4 Neon H2) Rolling 25-of-30 math makes 14-day grace recovery impossible after a 6-day outage (and breaks Day-0 onboarding)
- **[CRITICAL]** (Gemini 4 Neon H3) Public RPC + `PFSH03` private-action admission on consensus validators creates a liveness DoS vector, and 7–10s read latency has no root cause or SLA
- **[MAJOR]** (Claude Opus 5.5 H2) The launch minimum is 12 validators with at most 2 seats per correlated group, but the Foundation runs 6 correlated nodes today
- **[MAJOR]** (Claude Opus 5.5 H3) "Activation at height H" upgrades are manual-only, yet there is:
- **[MAJOR]** (Claude Opus 5.5 H4) The manifest is "reproduced by a second builder", but `upgrade` only verifies the single publisher signature
- **[MAJOR]** (Claude Opus 5.5 H5) "One grant per operator" has no enforcement mechanism before Pillar 6 ships, and none is specified for how multiple validator keys map to one operator
- **[MAJOR]** (Claude Opus 5.5 H6) The transition from `pending` to `active` is undefined.
- **[MAJOR]** (Claude Opus 5.5 H7) The spec does not say:
- **[MAJOR]** (Gemini 4 Neon H4) `postfiat-node upgrade` data rollback can deadlock against the double-sign guard and exhaust disk space
- **[MAJOR]** (Gemini 4 Neon H5) Single pinned ML-DSA-65 release signing key is a fleet-wide supply-chain SPOF
- **[MAJOR]** (Gemini 4 Neon H6) Per-operator reward anti-farming, cross-chain de-duplication, and bearer API-key revocation have gaps
- **[MAJOR]** (Gemini 4 Neon H7) 2-observer signed uptime feed has no quorum tie-breaker and drifts across wall-clock boundaries
- **[MAJOR]** (Gemini 4 Neon H8) `tasknode-bind` key custody and replay fields are unspecified, and hot-key rotation is missing from build milestones

## [Pillar 4 — The NFT System](04-nft-system.md) — 76/100

- **[CRITICAL]** (Claude Opus 5.5 H1) The spec never restricts which old-chain issuers or taxons are eligible to be minted into the official Task Node collection
- **[CRITICAL]** (Gemini 4 Neon H1) `pf.nft.metadata.v1.origin` records only `claim_hash` — not the signed `pf.nft.claim.v1` blob/CID — and Step 2 has the user burn on the old chain *before* pre-v
- **[CRITICAL]** (Gemini 4 Neon H2) No explicit old-chain issuer allowlist, ledger cutoff, or sponsorship caps gate who can migrate into the single official Task Node collection or consume Task No
- **[CRITICAL]** (Gemini 4 Neon H3) Chain-enforced duplicate prevention via `serial = first_8_bytes(SHA3-384(...))` has a burn-then-remint replay flaw, omits canonical hashing/encoding rules, and 
- **[MAJOR]** (Claude Opus 5.5 H2) The spec only checks that a burn exists and carries `claim_hash` in its memo
- **[MAJOR]** (Claude Opus 5.5 H3) Duplicate prevention relies on the NFT existing on chain
- **[MAJOR]** (Claude Opus 5.5 H4) Migrated NFTs use hash-derived 64-bit serials, but the spec never says how fresh Task Node mints pick serials under the same issuer.
- **[MAJOR]** (Claude Opus 5.5 H5) A single issuer account signs every migration mint, every fresh profile mint and every metadata envelope
- **[MAJOR]** (Claude Opus 5.5 H6) The burn is irreversible, but the spec does not require the signed `pf.nft.claim.v1` document to be stored durably before the burn is allowed.
- **[MAJOR]** (Claude Opus 5.5 H7) The only spam controls are hiding unfollowed issuers in the UI and blocking mints that are both burn-locked and transfer-locked
- **[MAJOR]** (Gemini 4 Neon H4) A single online ML-DSA-65 private key acts as the permanent issuer for both historical migrations and ongoing profile NFTs, with no key rotation, mint-authority
- **[MAJOR]** (Gemini 4 Neon H5) M1 changes live consensus rules and state layout without a protocol activation height or state migration, leaves `account_nfts`/`issuer_nfts` vulnerable to O(N)
- **[MAJOR]** (Gemini 4 Neon H6) Optional pin-only provenance (`pf.nft.provenance.v1`), including for external XRPL mainnet NFTs, specifies neither its on-chain anchor mechanism nor how ownersh
- **[MAJOR]** (Gemini 4 Neon H7) Explorer's live origin verification depends permanently on old-chain history RPC (which dies at Stage 4), omits the `envelope.sender == nft.issuer` check, and l

## [Pillar 5 — The Task Node Local Experience](05-local-task-node.md) — 73/100

- **[CRITICAL]** (Gemini 4 Neon H1) No trusted validator-RPC discovery mechanism and no cryptographic light-client verification of "certified pages" or tree roots.
- **[CRITICAL]** (Gemini 4 Neon H2) Unauthenticated public validator RPC proof admission has no pre-verification DoS defense, and a flat governance-tunable fee with a hard 4-proofs-per-block cap h
- **[CRITICAL]** (Gemini 4 Neon H3) `pfUSDC` is required at launch and tested end-to-end in Ship Gate 2, but the spec never defines how `pfUSDC` Private Sends and Unshields pay transaction fees or
- **[MAJOR]** (Claude Opus 5.5 H1) The opt-in "IP-privacy" relay is `/api/l1/pq/*` from `tasknode#444`, deployed on Task Node's Fly.io infrastructure
- **[MAJOR]** (Claude Opus 5.5 H2) The spec requires "2+ independent" endpoints and cross-checks tree roots against a "second independent endpoint." Per Pillar 3, the Foundation runs the first tw
- **[MAJOR]** (Claude Opus 5.5 H3) The spec introduces a "governance pause" and a "time-bounded exit freeze," and says the beta cap is lifted "by governance vote." It never defines:
- **[MAJOR]** (Claude Opus 5.5 H4) - **No per-asset supply check.** Nothing ensures the pool cannot release more pfUSDC than was shielded into it.
- **[MAJOR]** (Claude Opus 5.5 H5) The hard cap of 4 private actions per block has no defined priority rule, mempool expiry, or per-submitter limit.
- **[MAJOR]** (Claude Opus 5.5 H6) Unifying the derivation changes key generation for existing Mac app users (`pq-seed/v1` hash) and `pftl_pq_walletd` users (random 48-byte seed)
- **[MAJOR]** (Gemini 4 Neon H4) The opt-in "IP-privacy relay" (`/api/l1/pq/*`, `tasknode#444`) is hosted on Task Node infrastructure while the spec claims *"Task Node server NEVER receives key
- **[MAJOR]** (Gemini 4 Neon H5) M1 unifies BIP-39 → `PFSH03` derivation between `tasknode-apple` and `pftl_pq_walletd`, but does not coordinate with Pillar 6's shared v2 seed KDF, does not spe
- **[MAJOR]** (Gemini 4 Neon H6) Two state-machine gaps: (a) FastPay payouts (Pillar 2 owned objects) vs consensus-layer `PFSH03` Shield inputs are not bridged for one-click *Make private*, and
- **[MAJOR]** (Gemini 4 Neon H7) The governance-capped "Shielded Beta" does not specify whether the cap is per-transaction, per-block, total pool TVL, or per-asset, nor whether unshields bypass

## [Pillar 6 — Hive Chat Identity on Chain](06-hive-identity.md) — 71/100

- **[CRITICAL]** (Claude Opus 5.5 H1) Uniqueness is enforced per account, `npub`, handle, `node_id` and old-chain account
- **[CRITICAL]** (Claude Opus 5.5 H2) One rotatable registrar key can immediately `handle_reassign` any handle
- **[CRITICAL]** (Gemini 4 Neon H1) Single hot registrar key can hijack any `@tasknode.postfiat.org` handle
- **[CRITICAL]** (Gemini 4 Neon H2) Global handle uniqueness allows permanent custom-domain squatting, and missing normalization enables spoofing
- **[CRITICAL]** (Gemini 4 Neon H3) `rotate_account` lacks destination co-signature and link re-authorization
- **[MAJOR]** (Claude Opus 5.5 H3) v2 Nostr keys and the ML-DSA account come from the same master seed
- **[MAJOR]** (Claude Opus 5.5 H4) Consensus cannot fetch DNS or `nostr.json`, so a custom-domain handle on chain is just a claim
- **[MAJOR]** (Claude Opus 5.5 H5) Old-chain links only cover secp256k1 *wallet* keys whose public key hashes to the address
- **[MAJOR]** (Claude Opus 5.5 H6) A Nostr event's `created_at` is set by whoever signs it
- **[MAJOR]** (Claude Opus 5.5 H7) The spec never says whether `identity_set` must go through ordered consensus
- **[MAJOR]** (Gemini 4 Neon H4) `handle_reserve` bound only to unsalted `hash(npub)` enables pre-mint handle theft and blocks safe v2 migration
- **[MAJOR]** (Gemini 4 Neon H5) Self-`unlink` by a linked key is front-runnable, and client-verified EVM/Solana links conflict with global uniqueness
- **[MAJOR]** (Gemini 4 Neon H6) Validator anti-farming claim has a Sybil and node-hopping loophole across Pillar 3 and Pillar 6
- **[MAJOR]** (Gemini 4 Neon H7) Undefined fee denomination (`"20 units"`) and missing inline field caps create state-bloat and onboarding risk
- **[MAJOR]** (Gemini 4 Neon H8) Activating the BIP-340 governance kill-switch can deadlock existing identities
- **[MAJOR]** (Gemini 4 Neon H9) First-party relay (`relay.postfiat.org`) has no write-admission gate or inbound mirroring filter

## [Pillar 7 — Optional Deterministic Fees for NAVCoins](07-navcoin-fees.md) — 74/100

- **[CRITICAL]** (Claude Opus 5.5 H1) The `floor(V × U / S)` check is enforced only "for any NAVCoin with an attached schedule"
- **[CRITICAL]** (Gemini 4 Neon H1) Unspecified initialization of high-water mark (`H`) and last accrual height (`h_last`)
- **[CRITICAL]** (Gemini 4 Neon H2) Notice-period, legacy-spread, and exit-capacity bypasses
- **[CRITICAL]** (Gemini 4 Neon H3) Snapshot-vs-finalization supply race and non-scheduled hidden-fee loophole
- **[CRITICAL]** (Gemini 4 Neon H4) L1 post-fee `N_after` vs Ethereum Uniswap NAV guard divergence
- **[MAJOR]** (Claude Opus 5.5 H2) The starting high-water mark `H` is undefined when a schedule is attached to a live asset
- **[MAJOR]** (Claude Opus 5.5 H3) The spec never says who triggers finalization or whether epochs can be skipped
- **[MAJOR]** (Claude Opus 5.5 H4) "Any change that could raise what holders pay" is not defined field by field.
- **[MAJOR]** (Claude Opus 5.5 H5) NAVCoins without a schedule keep today's instant spread updates through the Task Node routes.
- **[MAJOR]** (Claude Opus 5.5 H6) A pending rate change activates at the first finalization after its effective height, but `Δh` covers the whole span since the last accrual
- **[MAJOR]** (Claude Opus 5.5 H7) Fees dilute NAV and mint new units on L1
- **[MAJOR]** (Gemini 4 Neon H5) Unenforced "non-issuer attestor" rule allows sockpuppet HWM fee extraction
- **[MAJOR]** (Gemini 4 Neon H6) Ambiguous `nav_fee_claim` ownership, recipient rotation, `lock_schedule` trap, and Pillar 8 trust-line conflict
- **[MAJOR]** (Gemini 4 Neon H7) Atom-scale ambiguity, `u128` silent-zero overflow, and unrecorded `blocks_per_year`

## [Pillar 8 — On-chain Identity (DID, KYC & Optional Trust Lines)](08-onchain-identity.md) — 76/100

- **[CRITICAL]** (Gemini 4 Neon H1) `refuse_unsolicited_assets` and `holder_limit` can still jam Solana/Ethereum bridge batches
- **[CRITICAL]** (Gemini 4 Neon H2) Auto-creating gated lines risks a permanent `either`-mode bypass, and `asset_gate(..., direction, ...)` cannot express Return-only or escrow freeze rules
- **[CRITICAL]** (Gemini 4 Neon H3) Routing Pillar 7 atomic fee accrual through `asset_gate` can deadlock NAVCoin epoch finalization
- **[MAJOR]** (Claude Opus 5.5 H1) The one-way rule covers only `requires_authorization` and credential-policy tightening
- **[MAJOR]** (Claude Opus 5.5 H2) Holds are created by bridge credits that pay no L1 fee
- **[MAJOR]** (Claude Opus 5.5 H3) The spec does not fix the height or time at which the gate is evaluated, which creates two problems:
- **[MAJOR]** (Claude Opus 5.5 H4) Return-only promises an exit "at NAV", but nothing guarantees a NAV-redeem path exists for a lapsed holder
- **[MAJOR]** (Claude Opus 5.5 H5) The fix is behind a governed activation height, and the spec only says to "audit and list" lines the bug created with `authorized = true`
- **[MAJOR]** (Gemini 4 Neon H4) Claimable Holds are unreachable by clawback, lack direct NAV-exit mechanics, and allow unbounded state bloat
- **[MAJOR]** (Gemini 4 Neon H5) DEX skip-and-cancel of ineligible resting makers enables zero-fill order-book griefing
- **[MAJOR]** (Gemini 4 Neon H6) On-chain "verified domain" contradicts consensus determinism and creates a domain-spoofing or gatekeeper trap
- **[MAJOR]** (Gemini 4 Neon H7) 10-atom auto-line creation enables permanent un-deletable frozen-dust account pollution
- **[MAJOR]** (Gemini 4 Neon H8) Under-specified policy tightening rules, missing issuer-compromise escape hatch, and deposit/admin-key gaps

## [Pillar 9 — Finance Hub User Spec](09-finance-hub.md) — 75/100

- **[CRITICAL]** (Gemini 4 Neon H1) Same-origin `ga-models` viewer + hot Publish API key = full Task Node XSS
- **[CRITICAL]** (Gemini 4 Neon H2) Sandboxed Goodbrief iframe breaks authenticated loading/assets and allows network exfiltration
- **[CRITICAL]** (Gemini 4 Neon H3) Pillar 10 brief files (`agent_brief_members.json`, `agent_brief_public.json`, `contributor_insights.json`) break leak defense, non-member access, and MNPI takedowns
- **[MAJOR]** (Claude Opus 5.5 H1) An opaque origin stops the Goodbrief scripts from *reading* Task Node data
- **[MAJOR]** (Claude Opus 5.5 H2) "Signed rollback" contradicts "monotonic version" unless it is defined.
- **[MAJOR]** (Claude Opus 5.5 H3) - The locked Pillar 3 rule is "active in registry and ≥95% over 25 of 30 days, with 14-day grace"
- **[MAJOR]** (Claude Opus 5.5 H4) The Top-30 tier ships in M4, which the README places in Wave 2
- **[MAJOR]** (Claude Opus 5.5 H5) Finance Hub seats are now a valuable, leakable prize, and they are ranked over all non-blacklisted accounts
- **[MAJOR]** (Gemini 4 Neon H4) Top-30 hysteresis, grace, suspension, and validator-overlap state machine has an infinite-grace loophole and seat-count ambiguity
- **[MAJOR]** (Gemini 4 Neon H5) Signed Publish API contradicts monotonic versions with rollback, lacks key rotation, and exposes staging uploads to DoS
- **[MAJOR]** (Gemini 4 Neon H6) Dual-chain validator migration and Pillar 3 feed outage behavior are missing
- **[MAJOR]** (Gemini 4 Neon H7) Content-rights review is sequenced in M5 *after* M4 opens Finance Hub to Top-30 users (and Today-page Goodbrief coupling risks bypassing `allowed_bases`)

## [Pillar 10 — Information Hive Mind](10-information-hive-mind.md) — 70/100

- **[CRITICAL]** (Claude Opus 5.5 H1) Phase 2 shows existing pooled claims to later contributors
- **[CRITICAL]** (Claude Opus 5.5 H2) Nothing records what "consensus" or the model estimate was when the claim was made
- **[CRITICAL]** (Gemini 4 Neon H1) Non-Finance-Hub contributors receive `agent_brief_public.json`, but Phase 2 still prompts them with **open model questions** and **existing pooled claims** for 
- **[CRITICAL]** (Gemini 4 Neon H2) The spec states answers are screened "before storage" and discarded on refusal, but does not enforce where raw text travels before screening, how multi-turn MNP
- **[CRITICAL]** (Gemini 4 Neon H3) Two critical state transitions have no defined executor or data path: (1) fast payouts depend on citation in an applied model update, yet the Research Host brid
- **[MAJOR]** (Claude Opus 5.5 H3) "Never stored" is not true in practice.
- **[MAJOR]** (Claude Opus 5.5 H4) Only `agent_brief_public.json` is build-checked for gated content
- **[MAJOR]** (Claude Opus 5.5 H5) No timeouts are defined.
- **[MAJOR]** (Claude Opus 5.5 H6) Acceptance happens later on the server, when the contributor is offline, so they cannot sign then
- **[MAJOR]** (Claude Opus 5.5 H7) The base payout, the multiplier value, per-claim and per-month caps, and the total budget are unspecified
- **[MAJOR]** (Gemini 4 Neon H4) The reward mechanism has no concrete payout/multiplier bounds, duplicate-splitting rule, anti-hedging rule, or owner conflict-of-interest check—while falsified 
- **[MAJOR]** (Gemini 4 Neon H5) Requiring accepted claims to emit "encrypted Pillar 2 packets signed by the contributor's Pillar 6 identity or pseudonymous key" breaks on three fronts: (1) Pil
- **[MAJOR]** (Gemini 4 Neon H6) Untrusted contributor text from the claim store is fed into three downstream surfaces without sanitization or prompt-injection boundaries: the Reviewer Agent (s
