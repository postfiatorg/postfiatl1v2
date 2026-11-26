# Feedback — Pillar 5 — The Task Node Local Experience

*Execution-hole audit · Round 4.*

> **Mean score:** 73/100 · Gemini 4 Neon 72 · Claude Opus 5.5 74
>
> **Method:** each model received (1) the founder's original prompt, (2) locked founder decisions & live state, (3) master `README.md`, and (4) [this pillar's spec](../../pillars/05-local-task-node.md), once, scored across 5 explicit rubric dimensions (Execution Holes /30, Economics /20, Founder Alignment /20, Milestones & 4 Gates /15, Readability /15). Run 2026-11-26 23:35 UTC via OpenRouter.

## Gemini 4 Neon — 72/100

*Model `google/gemini-4-neon` · 12712 output tokens · `tech=17/30 econ=11/20 align=18/20 ship=12/15 read=14/15`*

#### Summary judgement

Pillar 5 is one of the most grounded specs in the plan: it builds on real, live `PFSH03` devnet-2 execution (Blocks 1910–1914), keeps a crisp plain-English presentation with zero open questions, and honors every locked founder decision (native SwiftUI + Rust with no WebView, direct public validator RPC default, notarized macOS rollout, and a capped Shielded Beta during the external audit).

Before engineering ships M1–M3, however, several high-impact execution holes need explicit protocol and client fixes. The biggest gaps are: (1) no trusted RPC discovery or cryptographic light-client finality check, which allows a compromised Task Node server to silently MITM "direct validator" traffic; (2) unauthenticated public Binius proof submission with a flat fee and 4-proofs-per-block cap, creating both validator CPU DoS and trivial slot-stuffing censorship; (3) an unspecified fee-payment and asset-isolation model for `pfUSDC` shielded actions (which directly risks de-anonymization or breaking Ship Gate 2); and (4) routing the opt-in "IP-privacy relay" through `tasknode#444`, contradicting the hard boundary that Task Node never sees shielded proofs.

#### Rubric breakdown

- **Execution Holes & Technical Robustness (17/30):** Strong live foundation and clear custody intent, but missing light-client root verification/RPC discovery, public admission DoS defenses, `pfUSDC` shielded fee mechanics, FastPay-to-PFSH03 settlement atomicity, and anchor/intent expiry rules.
- **Economic & Incentive Coherence (11/20):** Sensible beta cap and anonymity-floor instincts, but a flat fee with a 4-proofs-per-block cap enables cheap censorship (including blocking exits), and undefined beta cap dimensions invite whale cap-squatting or uncapped unaudited TVL.
- **Founder Decision Alignment & Zero Open Questions (18/20):** Cleanly aligned with all Nov 26 locked decisions and zero deferred founder homework; minor friction where `tasknode#444` sits on Task Node infrastructure and M4 native Finance Hub lacks a non-WebView data/watermark contract with Pillar 9.
- **Actionable Milestones & 4 Ship Gates (12/15):** Clear M0–M5 progression, real code/PR links, and all 4 mandatory ship gates; loses points for an Orchard retirement ordering conflict in M1 and testing `pfUSDC` in Gate 2 without a dedicated milestone for `pfUSDC` shielded fee/circuit support.
- **Plain-English Readability & Zero Jargon Bloat (14/15):** Concise, scannable, free of alphabet-soup codes, with an honest privacy model and clear architecture diagram.

#### Execution holes

##### H1
- **Severity:** `critical`
- **Where:** *How local shielded transactions work*, *Detailed specification §1–2*, *M2*
- **Hole:** No trusted validator-RPC discovery mechanism and no cryptographic light-client verification of "certified pages" or tree roots.
- **Why it breaks execution:** First, if `TaskNode.app` fetches the list of public validator RPC endpoints from the Task Node server, a compromised Task Node server can silently substitute two colluding endpoints and capture proofs, IPs, and fake state — completely breaking the "zero data touches Task Node" guarantee. Second, naive root cross-checking between two endpoints breaks whenever Validator A is one block ahead of Validator B, causing false rejections on healthy networks.
- **Minimal plain-English fix:** Pin the genesis/checkpoint validator keyset inside the signed `TaskNode.app` binary, verify page roots and validator-set transitions via quorum finality signatures at an explicit finalized block height, and read public validator RPC endpoints from signed on-chain Pillar 3 registry state rather than Task Node APIs.

##### H2
- **Severity:** `critical`
- **Where:** *Locked-in design decisions (Fees & caps)*, *Detailed specification §2*, *M2*
- **Hole:** Unauthenticated public validator RPC proof admission has no pre-verification DoS defense, and a flat governance-tunable fee with a hard 4-proofs-per-block cap has no congestion pricing or exit reservation.
- **Why it breaks execution:** An attacker can flood public validator RPC endpoints with malformed Binius proofs to exhaust CPU and degrade node availability. Worse, an attacker can legally submit 4 dust shielded transactions every block at the flat minimum fee to fill 100% of shielded slots indefinitely, censoring all other users — including blocking unshields during an emergency or governance pause.
- **Minimal plain-English fix:** Run cheap admission pre-checks (schema bounds, active anchor, unseen nullifier, per-IP rate limit) on non-validating RPC/admission workers before running Binius verification; order competing proofs by priority fee (or escalate the fee when blocks are full); and reserve priority capacity for unshields whenever the pool is congested or paused.

##### H3
- **Severity:** `critical`
- **Where:** *Locked-in design decisions (Assets, payouts & memos)*, *Required ship gates (Gate 2)*
- **Hole:** `pfUSDC` is required at launch and tested end-to-end in Ship Gate 2, but the spec never defines how `pfUSDC` Private Sends and Unshields pay transaction fees or whether `PFT` and `pfUSDC` share a multi-asset circuit or run separate asset pools.
- **Why it breaks execution:** If a `pfUSDC` Private Send or Unshield pays its fee from a public `PFT` account, that public fee-payer immediately de-anonymizes the transaction. If it instead requires a shielded `PFT` fee note inside the same proof, the circuit must handle multi-asset inputs/value-balance (untested in M0) and users cannot move shielded `pfUSDC` without first holding shielded `PFT`.
- **Minimal plain-English fix:** Lock the `pfUSDC` fee rule before M2 — preferably deducting a governance-set `pfUSDC` fee directly from the shielded `pfUSDC` value balance (or explicitly shipping and benchmarking multi-asset fee-note proofs) — add a `pfUSDC` E2E check to M1/M2, and enforce the `16+` candidates / `128+` actions anonymity floor separately per asset.

##### H4
- **Severity:** `major`
- **Where:** *How local shielded transactions work*, *What exists today*, *Honest privacy model*
- **Hole:** The opt-in "IP-privacy relay" (`/api/l1/pq/*`, `tasknode#444`) is hosted on Task Node infrastructure while the spec claims *"Task Node server NEVER receives keys, notes, witnesses, or proofs."*
- **Why it breaks execution:** Enabling the IP-privacy toggle routes the user's proof, timing, and IP straight to Task Node's server. If shared hosting, logs, or Task Node auth tokens touch that endpoint, Task Node can correlate the user's authenticated account with their shielded transaction — violating the core privacy promise.
- **Minimal plain-English fix:** Host the opt-in relay as a separate, unauthenticated, zero-log service completely outside Task Node (with CI packet-capture tests confirming zero Task Node cookies/auth headers are sent), and disclose in the toggle copy that the relay operator sees submitter IP plus public proof.

##### H5
- **Severity:** `major`
- **Where:** *What exists today (Seed recovery compatibility)*, *Detailed specification §1*, *M1*
- **Hole:** M1 unifies BIP-39 → `PFSH03` derivation between `tasknode-apple` and `pftl_pq_walletd`, but does not coordinate with Pillar 6's shared v2 seed KDF, does not specify 24-word (256-bit) PQ entropy, and omits bare-seed recovery behavior when no birthday checkpoint file exists.
- **Why it breaks execution:** Separate KDFs between Pillar 5 and Pillar 6 force users to back up two seed phrases or break cross-client recovery; 12-word (128-bit) phrases weaken post-quantum key strength; and restoring from written BIP-39 words alone on a clean Mac (explicitly required in Ship Gate 2) lacks a defined fallback scan strategy from pool activation height.
- **Minimal plain-English fix:** Define one shared versioned HKDF tree across Pillars 5 and 6 using 24-word (256-bit) mnemonics for new wallets (deriving L1 ML-DSA, X-Wing, Nostr, and PFSH03 spend/view keys under distinct domain tags), and specify a bounded full-scan-from-activation fallback (with optional user-supplied approximate creation date) when restoring without a birthday checkpoint.

##### H6
- **Severity:** `major`
- **Where:** *Locked-in design decisions (Payouts)*, *Detailed specification §1*, *M3*
- **Hole:** Two state-machine gaps: (a) FastPay payouts (Pillar 2 owned objects) vs consensus-layer `PFSH03` Shield inputs are not bridged for one-click *Make private*, and (b) crash-safe intents have no defined L1 anchor validity window or client-side expiration height.
- **Why it breaks execution:** (a) If Shield requires settling a FastPay object into a consensus balance first, a crash mid-flow leaves funds publicly unshielded with ambiguous UI state. (b) Without a hard `expiry_height` and documented anchor window, a dropped submission can lock a user's local note in `submitted` forever — or a late retry can confirm hours later after the user already sent a replacement payment.
- **Minimal plain-English fix:** Specify whether `PFSH03` Shield consumes FastPay objects directly or runs a persisted two-step state machine (`fastpay_settled` → `shield_submitted`) that resumes automatically after a crash; require an explicit `expiry_height` on every shielded action and publish the L1 anchor history window (e.g., last 64 finalized roots) so stale intents cleanly expire and unlock notes.

##### H7
- **Severity:** `major`
- **Where:** *What this pillar is §5*, *Locked-in design decisions*, *M5*
- **Hole:** The governance-capped "Shielded Beta" does not specify whether the cap is per-transaction, per-block, total pool TVL, or per-asset, nor whether unshields bypass all caps.
- **Why it breaks execution:** A total-TVL-only cap lets one actor fill the pool in a single transaction, locking out beta testers and collapsing the anonymity set; a per-transaction-only cap allows unlimited unaudited value to enter via repeated shields; and any cap touching exits traps user funds.
- **Minimal plain-English fix:** Enforce per-asset governance caps (`PFT` and `pfUSDC` independently) combining a per-shield cap, a per-block shield inflow cap, and a total pool TVL cap, with unshields strictly exempt from all caps.

##### H8
- **Severity:** `minor`
- **Where:** *Locked-in design decisions (Fees & caps)*, *M1*
- **Hole:** M1 retires legacy Orchard and `pftl_swapd` immediately, while the design section promises a two-step exit → `PFSH03` shield migration for old Orchard notes.
- **Why it breaks execution:** Removing Orchard exit support in M1 strands any remaining Orchard balances; conversely, if no non-ephemeral devnet balances exist, building a migration flow wastes M1 engineering effort.
- **Minimal plain-English fix:** Audit whether any non-test Orchard balances exist; if zero, remove the migration promise and retire Orchard cleanly in M1; if non-zero, disable new Orchard shields in M1 and keep unshield-only active until a fixed sunset block height.

##### H9
- **Severity:** `minor`
- **Where:** *Detailed specification §3*, *M3*, *M4*
- **Hole:** Missing signed update + `PFSH03` protocol-version handshake for `TaskNode.app`, and M4 native SwiftUI Finance Hub lacks a structured-data/watermark contract with Pillar 9 (which uses sandboxed Goodbrief HTML).
- **Why it breaks execution:** Audit findings in M5 will almost certainly require client/verifier upgrades; without signed auto-update checks and an RPC protocol-version check, M3 beta installs will silently produce rejected proofs or miss security patches. Meanwhile, building M4's native Finance Hub with zero WebView will stall or weaken Pillar 9's Goodbrief isolation/watermarking unless Pillar 9 exposes sanitized structured JSON/Markdown feeds.
- **Minimal plain-English fix:** Add a signed app-update check (Sparkle with pinned signing key) plus an explicit `PFSH03` protocol-version handshake against validator RPCs in M3, and specify in M4 that native SwiftUI Finance Hub consumes Pillar 9's sanitized structured API and renders native user watermarks directly without HTML/WebViews.

## Claude Opus 5.5 — 74/100

*Model `anthropic/claude-opus-5.5` · 6764 output tokens · `tech=18/30 econ=13/20 align=18/20 ship=13/15 read=12/15`*

#### Summary judgement

This is one of the stronger pillars. It rests on a real, live devnet proof (Blocks 1912–1914). It honors every locked decision: native SwiftUI, direct validator RPC as the default, relay as opt-in only, Apple-silicon first, notarization, and a capped Shielded Beta until the audit closes. Its milestones are concrete and its privacy disclosures are unusually honest.

The weaknesses sit at the edges of the privacy boundary:

- **The opt-in "IP-privacy" relay runs on Task Node.** It breaks the spec's own "Task Node never receives proofs" promise and actually worsens privacy.
- **"Independent" endpoints are not independent at launch.** Pillar 3 has the Foundation running both first public RPCs, so the second-endpoint cross-check proves nothing.
- **The beta cap, pause, and "exit freeze" lack on-chain definitions.** Who controls them, and their limits, are unstated.
- **pfUSDC enters the pool with no per-asset turnstile and no bridge-hold interaction.**
- **Seed unification can strand existing devnet notes.**
- **The 4-proofs-per-block cap is an obvious griefing target.** No fee-priority rule is defined.

None of these require re-opening founder decisions. They are tight engineering fixes.

#### Rubric breakdown

- **Tech (18/30):** The local-proving architecture and crash-safe intents are solid. However, the relay contradicts the leak boundary, the "certified page" verification is undefined, the pause/freeze authority and limits are unspecified, the seed migration has no sweep path, and there is no app update/CDHash rotation path.
- **Econ (13/20):** The fixed 4-proof cap with no priority rule invites slot griefing. A per-account beta cap is trivially beaten with throwaway accounts. The anonymity floor can be met by one party filling the pool with its own transactions. "Make private" on publicly badged payouts invites amount-linking.
- **Align (18/20):** Faithful to all Pillar 5 locked decisions and the live state, with no open questions. Mentioning a "browser UI" for future Linux/Windows clients is harmless but slightly off-message.
- **Ship (13/15):** M0–M5 are concrete, and all 4 gates are present and testable. Gate 1 depends on "production" existing, without stating which chain or milestone that means.
- **Read (12/15):** Mostly plain English. Terms like CDHash, Seatbelt, RSS, and "certified pages" are used without a one-line explanation.

#### Execution holes

**H1**
- **Severity:** major
- **Where:** Locked-in design decisions → Submission path; diagram; §2 Hard custody boundary
- **Hole:** The opt-in "IP-privacy" relay is `/api/l1/pq/*` from `tasknode#444`, deployed on Task Node's Fly.io infrastructure. In relay mode, Task Node receives every proof. It also already knows the user's account, session, and IP.
- **Why it breaks execution:**
  - It contradicts the diagram's "Task Node NEVER receives proofs" line.
  - The M2 packet-capture leak proof will fail, or only pass with relay mode off.
  - Users who pick the "more private" toggle actually give one party their identity, IP, and proof timing together, which is worse than direct submission.
- **Minimal plain-English fix:**
  - State that the relay is a separate service with no Task Node login, logs, or account linkage, running on separate infrastructure. Alternatively, replace it with a generic proxy option such as Tor/SOCKS.
  - Run the leak-capture test in both modes.
  - Reword the UI toggle as "hide IP from validators (relay operator sees proof timing)."

**H2**
- **Severity:** major
- **Where:** §1 Verified page scanning; §2 Multi-endpoint failover
- **Hole:** The spec requires "2+ independent" endpoints and cross-checks tree roots against a "second independent endpoint." Per Pillar 3, the Foundation runs the first two public RPCs, and all 6 devnet validators are Foundation-run. "Certified pages" are never defined, so the client may simply trust whatever an endpoint returns.
- **Why it breaks execution:** A single compromised operator, or one buggy RPC build, can serve consistent but false pages or withhold outputs on both endpoints. The wallet then misses notes or marks the wrong notes as spent, and the cross-check passes anyway.
- **Minimal plain-English fix:**
  - Define a certified page as one carrying a validator quorum signature (for example, 4 of 6) over the tree root. The app verifies that signature locally against the on-chain validator set.
  - Treat endpoint count as a liveness feature only, not a trust feature.
  - Label endpoints by operator in the UI.

**H3**
- **Severity:** major
- **Where:** §2 Kill-switch readiness; §5 Honest privacy UX; M5
- **Hole:** The spec introduces a "governance pause" and a "time-bounded exit freeze," and says the beta cap is lifted "by governance vote." It never defines:
  - who holds that authority (validator on-chain governance vs. Pillar 1's founder-only sessions);
  - the maximum freeze length, or whether a freeze ends automatically;
  - whether the cap applies per transaction, per account, or to the whole pool.
- **Why it breaks execution:**
  - An exit freeze with no consensus-enforced maximum is a single point of failure for stranding funds.
  - A per-account cap is defeated by creating many free accounts.
  - The relationship between founder governance and chain governance is left ambiguous.
- **Minimal plain-English fix:** Specify all three as consensus parameters changed only by validator governance transactions (the same path as Block 1910):
  - a pool-wide total-value cap per asset, plus a per-transaction cap;
  - pause stops shield/send only and never blocks unshield;
  - any exit freeze has a hard on-chain maximum (for example, 72 hours) and expires automatically.

**H4**
- **Severity:** major
- **Where:** Assets at launch (pfUSDC); Fees & caps
- **Hole:**
  - **No per-asset supply check.** Nothing ensures the pool cannot release more pfUSDC than was shielded into it.
  - **Bridge holds can be bypassed.** pfUSDC is a bridged asset subject to Pillar 8 bridge holds, and nothing stops a balance under a hold from being shielded.
- **Why it breaks execution:** A circuit bug or a bridge exploit lets an attacker mint pfUSDC and shield it immediately. That makes the theft unfreezable and untraceable, or lets the attacker unshield more than the pool holds.
- **Minimal plain-English fix:**
  - Have consensus track total shielded-in minus total shielded-out per asset, and reject any unshield that would make it negative. Publish this figure on the Explorer.
  - Require that bridge holds clear before a balance can be shielded.
  - Include both checks in the M5 audit scope.

**H5**
- **Severity:** major
- **Where:** Fees & caps (4-proofs-per-block cap)
- **Hole:** The hard cap of 4 private actions per block has no defined priority rule, mempool expiry, or per-submitter limit.
- **Why it breaks execution:** An attacker with a few machines can keep all 4 slots full with cheap valid self-transfers at the minimum fee. Real users' "Make private" and unshield actions then stall or expire. Pause and exit flows starve too.
- **Minimal plain-English fix:**
  - Order private actions by fee, and give unshields a reserved slot.
  - Define how long a pending intent lives before it is marked `expired`, measured in blocks.
  - Make the governance-tunable fee rise automatically when slots stay full.

**H6**
- **Severity:** major
- **Where:** §1 Shared crate & unified BIP-39 derivation; M1
- **Hole:** Unifying the derivation changes key generation for existing Mac app users (`pq-seed/v1` hash) and `pftl_pq_walletd` users (random 48-byte seed). Notes held under the old keys become unrecoverable from the new seed. Separately, M1 retires Orchard and `pftl_swapd` without stating that the exit window stays open.
- **Why it breaks execution:** Devnet and early users restore from seed and see zero balance. Old Orchard notes are stranded if the exit path is removed in the same release.
- **Minimal plain-English fix:**
  - Keep the old derivations as read-only "legacy keys" in the app, and offer a one-click sweep into the new derivation.
  - Keep the Orchard exit open for a stated minimum period after retirement, with a final Explorer notice.

**H7**
- **Severity:** minor
- **Where:** Payouts → one-click *Make private*; Honest privacy model
- **Hole:** Task Node payouts are publicly labeled "Task reward" with known amounts. One-click shielding of the exact payout, followed by unshielding similar amounts, links the user's identity to private activity. This is the exact pattern the app warns against.
- **Why it breaks execution:** It is the default flow, so it becomes the most common way users leak their own privacy.
- **Minimal plain-English fix:** Have "Make private" shield whole payout amounts, but steer later unshields toward round or split amounts with a random delay. Show the user the specific linkage risk at unshield time.

**H8**
- **Severity:** minor
- **Where:** Honest privacy model (anonymity floor)
- **Hole:** The "16+ candidates / 128+ actions" floor that flips the label from Beta to Private can be met by one entity, including the Foundation, generating its own shield and send transactions.
- **Why it breaks execution:** The UI could claim "Private" while the effective anonymity set is tiny.
- **Minimal plain-English fix:** Count distinct funding accounts over a rolling window. Make the label change require the audit plus the floor holding for N consecutive days. Publish the measurement method in the docs.

**H9**
- **Severity:** minor
- **Where:** In-app disclosures (`pfpq1` lookup via "direct on-chain Pillar 6 RPC")
- **Hole:** The Pillar 6 `identity_set` links ML-DSA account, npub, handle, validator, old-chain, and agent keys. It never includes a `pfpq1` shielded address field. Publishing one would also permanently tie a public identity to a receive address.
- **Why it breaks execution:** This is a cross-pillar mismatch. The lookup path either doesn't exist, or it creates a privacy regression without disclosure.
- **Minimal plain-English fix:**
  - Either drop the Pillar 6 lookup and use paste/QR only, or add an optional, rotatable `pfpq1` field to `identity_set`.
  - In the latter case, show a warning that a published address is linkable, and add the field to Pillar 6's interface list.

**H10**
- **Severity:** minor
- **Where:** §3 Notarized distribution
- **Hole:** No update channel is specified, nor how the pinned `TaskNodeProver` CDHash rotates across app versions, nor how the vault and pending intents migrate between versions.
- **Why it breaks execution:**
  - An unsigned or ad-hoc update path is the easiest way to swap in a malicious prover.
  - An update during an in-flight intent could lose signed bytes.
- **Minimal plain-English fix:**
  - Ship updates via a signed updater (for example, Sparkle with EdDSA), with the new helper CDHash shipped inside the signed app bundle.
  - Block updates while intents are `submitted`.
  - Version the vault format with a tested migration step.

**H11**
- **Severity:** minor
- **Where:** §1 Crash-safe & multi-device intents
- **Hole:** The spec promises that "concurrent spends from two devices resolve cleanly." There is no device-to-device sync channel, so both devices can pick the same note.
- **Why it breaks execution:** One device always loses with `conflicted`, after the user has already waited about 10 seconds of proving and approved with Touch ID. The repeated failures look like bugs and cost fees.
- **Minimal plain-English fix:** Mark notes as "pending" locally the moment a spend is proved. Re-scan before proving on every device. State plainly that two-device use may produce a `conflicted` retry rather than claiming clean resolution.
