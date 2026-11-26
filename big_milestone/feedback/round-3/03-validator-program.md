# Feedback — Pillar 3 — Validator Rewards Program

*Review round 3.*

> **Mean score:** 70/100 · Gemini 4 Neon 64 · Claude Opus 5.5 76
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/03-validator-program.md), once, with the question below. Run 2026-11-26 22:31 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 64/100

*Model `google/gemini-4-neon` · 19558 output tokens*

#### Summary judgement

This sub-page is unusually well-grounded in the existing codebase: it answers the founder's concrete questions directly (confirming that RPC is `rpc-serve` inside `postfiat-node`), inventories real gaps honestly (7–10 s child-process reads, exclusive store locks, unsigned single-host uptime feed, unbuilt L3 verifier, key-file fleet, broken rotation rewards), and maps cross-pillar interfaces cleanly.

However, as a blueprint meant to output something **technically robust and economically coherent**, it falls short in three major ways:

1. **Concrete technical hazards and underspecified core mechanisms.** Forwarding public `private_action_submit` traffic to a consensus validator that holds an exclusive store lock creates a direct liveness DoS vector; the `k`-of-`n` observer design contradicts itself by letting a single dissenting observer invalidate a day; follower tip-sync and store-lock concurrency are left unspecified; upgrading a single binary containing both node and signer creates a guard-floor rollback hazard; and letting a non-root CLI talk to the consensus signer socket threatens the two-user security boundary.
2. **Economic incoherence.** The spec accurately diagnoses every incentive flaw (old-chain bindings permanently grant the same Finance Hub access for free; hosting costs are unmeasured and public RPC requires a second unpaid host; $20/month in non-cash credit or $0 testnet rewards cannot cover ops; no slashing or equivocation revocation exists), but stops at listing them as open questions instead of proposing a self-consistent economic model.
3. **Execution drag and decision paralysis.** Twelve multi-part open questions (~25 distinct decisions) are handed back to the founder without recommended defaults, while MVP onboarding deliverables are bundled on a flat plane with mainnet-grade infrastructure (reproducible second-builder releases, `k`-of-`n` observer federation, L3 correlation datasets, multi-proposer governance, webhook daemons). Built as written, this risks either months of delay before the first outside validator joins or a fragile launch.

---

#### Correspondence to the founder's intention

##### Where it aligns well
- **Direct answers to the founder's RPC questions:** Clearly resolves *"Ripple presumably has RPC software. Do we? Where is it? How does it get updated? If I want to run my own RPC, how do I do that?"* by positioning `postfiat-node` in follower mode as the single packaged binary and release channel.
- **Core deliverables covered:** Includes a single copy-paste docs page (`R11`), a structured software upgrade workflow (`R3`), one-command Task Node binding (`R13`), an SSH status dashboard (`R12`), Finance Hub / GoodAlexander content gating (`R15`), and staged migration from the old chain (`R0`).
- **Governance framing:** Connects validator admission and ratification records back to Pillar 1's deterministic, public, recomputable "post-human governance" narrative.

##### Where it drifts or falls short
- **Smooth staged onboarding vs. heavyweight waterfall:** The founder asked for *"smooth onboarding for Validators"* and *"a staged approach where we get some validators on the new chain"* as the first phase of migration. The sub-page instead stacks nearly all mainnet governance, release-engineering, and anti-Sybil machinery ahead of getting external operators on board, without splitting **MVP (get early validators + followers running)** from **Hardening (public mainnet launch)**.
- **Reward program without a coherent reward design:** Despite being titled *Validator Rewards Program*, the spec cannot say whether validators earn anything beyond research access, leaves old-chain access indefinitely open (removing any pull to migrate), and defers almost all economic choices to founder Q&A.
- **Wider performance context lightly coupled:** The founder linked chain migration readiness to FastPay low-latency payouts, NFTs, memos, and Cobalt understanding/simulations. While those belong to other pillars, `R10` only benchmarks `status`, `account`, and `tx` without verifying whether the RPC surface (`methods.md`) supports FastPay latency (e.g., streaming/push updates vs. polling) or missing wallet methods (such as `account_info`).
- **Decision burden pushed onto the founder:** Rather than proposing opinionated defaults and escalating only 3–5 genuinely strategic forks, the spec escalates 12 bundled questions that block `R0`, `R3`, `R6`, `R10`, `R13`, `R14`, `R15`, and `R17`.

---

#### Technical robustness

The spec shows strong instincts around operational safety (guard state never restored, no rollback after activation height `H`, `top` never opening the exclusive chain store, two-user signer isolation). Still, several architectural flaws will cause outages, security regressions, or build stalls if implemented verbatim:

- **Consensus-host DoS via private-action admission:** The security model promises *"Public reads only on follower hosts; no public port on validators"*, yet `R10` and the architecture diagram route public `private_action_submit` (which verifies ZK proofs!) and `private_action_receipt` traffic from the TLS edge straight into the validator's `rpc-serve`, which shares an exclusive store lock with block commits.
- **Broken observer fault tolerance (`R16` vs. Acceptance 7 vs. `R5b`):** `R16` specifies `k`-of-`n` agreement so one observer can fail, yet Acceptance 7 and the Security Model state that a single dissenting observer makes the day uncounted—turning `k`-of-`n` into `n`-of-`n` griefable unanimity. Furthermore, `R5b` uses the same agreement model for *subjective* 7-day follower probing (`uptime_window_bps`), where honest geographically distributed observers will naturally measure different values.
- **Unspecified follower replication and store-lock architecture:** `R10` defines follower mode as a requirement to build, but never specifies *how* a follower continuously tracks the tip (P2P push/gossip vs. RPC polling), how validators are shielded from follower sync load, or how moving reads in-process solves exclusive lock contention during block commits.
- **Upgrade/rollback hazard with a single binary and stateful guard:** Because `postfiat-node` contains both the node and `validator-signer-serve`, swapping the binary upgrades both without specifying signer restart/IPC compatibility. Worse, if an upgraded node signs one block (advancing the guard floor to `h+1`) and then fails health-check and rolls back the chain snapshot to `h`, chain state and guard state desynchronize.
- **Signer socket privilege creep:** Letting a non-root operator CLI (`tasknode-bind`) talk to the consensus signer socket (`R13`) without separate sockets, `SO_PEERCRED` UID checks, or strict domain separation collapses the two-user isolation boundary.
- **Brittle quorum math and missing L3 deliverable:** `n = 12`, quorum `10` tolerates only **2** offline validators (identical absolute fault tolerance to the current 5-of-6 devnet, across double the operators running manual upgrades). Meanwhile, the L3 independence verifier is flagged as unbuilt with no requirement assigned to build it.
- **Cross-pillar rotation mismatch:** `R15` keeps Task Node bindings alive across key rotations without re-binding, while Pillar 6's identity link goes stale on rotation—breaking per-operator rewards (Open Question 6) and directory linkage (`R18`) silently.

---

#### Economic coherence

The spec is candid about current gaps, which is helpful, but it does **not** assemble a workable economic system:

- **Negative expected ROI for honest operators:** An outside validator is expected to run a high-availability node (≥99.5% candidate uptime for 7 days; ≥95% ongoing participation), manage offline ML-DSA-65 keys and coordinated manual upgrades, potentially pay for a *second* host for a public RPC follower, and absorb ZK proof-verification load for private actions—in return for $0 on testnet (or $20/month in non-cash Task Node credit) plus Finance Hub / GoodAlexander research access.
- **Zero migration pull off the old chain:** Old-chain bindings unlock the exact same Finance Hub and research API keys today, and access never lapses. Without a sunset timeline or differentiated value on the new chain, rational operators have no reason to move.
- **No funding model for public infrastructure:** Pillar 5 depends on at least two independent public RPC + private-action endpoints, yet the spec leaves public RPC hosting completely uncompensated (`"nothing pays for the extra host"`) and offers no incentive to enable `--allow-private-action-admission`.
- **Unpriced Sybil and equivocation risk:** With no stake or bond, spinning up multiple masked identities across cheap VPS providers is inexpensive, and a 3-seat Sybil coalition holds a liveness veto in a 12-seat, quorum-10 set. Equally concerning, there is **no economic or access penalty specified for double-signing/equivocation** (e.g., immediate revocation of Finance Hub access, rewards, and permanent operator ban).
- **Unaddressed transition for excluded old-chain validators:** Old-chain rounds score 42–55 validators for a 20-seat UNL. Even if all 20 qualify for `G0`, 22–35 existing validators are left out of genesis with no transition plan or communication strategy.

---

#### Problems

##### P1 — Public private-action traffic proxies directly onto the consensus validator's exclusive store lock and CPU
- **Severity:** critical
- **Where:** *What exists today* (RPC surface and speed; Private-action admission), *Architecture* (Mermaid diagram), *Security model and failure modes* (Consensus host), *Technical requirements* (`R10 — Admission`).
- **What is wrong:** The spec asserts *"Public reads only on follower hosts; no public port on validators"* to protect consensus liveness from store-lock contention, yet simultaneously routes `private_action_submit` (which verifies user ZK proofs and accesses the live store) and `private_action_receipt` from the public TLS edge directly to the validator's `rpc-serve`.
- **Why it matters:** A flood of invalid or valid `private_action_submit` proofs from the public internet—even through a TLS edge—can saturate CPU and queue behind the validator's exclusive store lock, delaying block commits and causing missed precommits. At 5-of-6 or 10-of-12 quorum, stalling just 2–3 validators halts network finality.
- **How to investigate:** Inspect `crates/node/src/private_action_rpc.rs`, `crates/node/src/private_action_admission.rs`, and the store locking model in `crates/node`. Measure proof-verification CPU time on a 2-core host and check whether verification runs before or inside the store lock, and whether `private_action_receipt` actually needs the validator's live store instead of a follower.
- **Suggested direction for the fix:** Separate proof verification and receipt reads from the consensus validator: (1) serve `private_action_receipt` strictly from followers; (2) verify proofs and enforce strict rate-limits/PoW/auth/x402 or bounded admission queues *on the follower/edge* before forwarding verified, size-capped batches to the validator over an internal authenticated channel; (3) enforce strict resource budgets/circuit breakers so admission automatically sheds load whenever consensus lag rises.

##### P2 — Observer federation (`R16` / `R5b`) contradicts `k`-of-`n` fault tolerance and cannot validate subjective follower probes
- **Severity:** critical
- **Where:** *Security model and failure modes* (bullet *"Observers disagree on a day"*), *Technical requirements* (`R5b`, `R16`), *Acceptance criteria* (Criterion 7).
- **What is wrong:** Two separate flaws collide here:
  1. `R16` claims *"accepts a day when at least k of n feeds agree. With k below n, one observer can fail"*, yet Security Model and Acceptance 7 state *"A dissenting observer makes that day uncounted."* If one online dissenting observer invalidates a day, the protocol is actually `n`-of-`n` unanimity—any single buggy, out-of-sync, or malicious observer can permanently halt all rewards and uptime tracking.
  2. `R5b` uses the same observer feeds to measure candidate `uptime_window_bps` via 7-day external reachability/tip-lag probes. Unlike deterministic `precommit_qc.votes`, live network probes are inherently subjective across observers in different regions and will almost never match identically.
- **Why it matters:** As written, the feed will either constantly drop days due to minor observer drift/dissent (breaking rewards and admissions) or be trivially griefed by a single rogue observer. Additionally, introducing an observer federation merely to recount deterministic on-chain `precommit_qc.votes` adds governance and operational overhead (who approves pinned keys in Task Node, how keys rotate) for data Task Node could verify directly.
- **How to investigate:** Review `tools/tasknode-validator-binding/registry.py`, `server/l1-validator-uptime.js`, and how UTC day boundaries map to block heights/timestamps. Check whether Task Node can run an internal follower (or verify certified block QCs directly) for deterministic participation.
- **Suggested direction for the fix:** Split deterministic and subjective signals cleanly:
  1. **On-chain participation (`precommit_qc.votes`):** Define UTC day cutoffs strictly by finalized block height/consensus timestamp so recounts are 100% deterministic; accept any value supported by `≥ k` of `n` observers *ignoring* a minority dissenter (logging dissent for investigation rather than voiding the day)—or better, have Task Node derive participation directly from its own synced follower + a single independent cross-check observer.
  2. **Subjective candidate probing (`R5b`):** Aggregate across observers using a robust statistic (e.g., median or `k`-th highest `uptime_window_bps` across `≥ k` reporting observers) rather than exact agreement.

##### P3 — Economic design is incoherent: negative validator ROI, zero migration incentive, unpaid public RPCs, and no equivocation penalty
- **Severity:** critical
- **Where:** *Economics*, *Technical requirements* (`R15`, `R17`), *Open questions for the founder* (Questions 3, 4, 6, 7, 8).
- **What is wrong:** The spec exposes fundamental incentive contradictions without resolving them:
  1. Hosting 1–2 reliable servers + SRE effort costs far more than $0 (testnet) or $20/mo in non-cash Task Node credit.
  2. Old-chain bindings unlock the exact same Finance Hub + research API keys and never lapse, giving existing validators zero reason to incur the cost/risk of migrating.
  3. Public RPC followers (required by Pillar 5) and private-action admission impose extra hosting/DoS costs with zero compensation.
  4. There is no slashing, bond, or even explicit *access-revocation / permanent-ban* rule for proven equivocation (double-signing) or API-key abuse.
- **Why it matters:** Without a coherent incentive structure, either outside operators will refuse to join (or refuse to run public RPCs), or low-effort actors will farm Finance Hub access via bind-and-quit / hidden multi-seat Sybils—compromising both chain liveness and the exclusivity of GoodAlexander research.
- **How to investigate:** Audit current devnet hosting costs (cloud bills for the 6 Foundation validators), check how many unique accounts currently hold old-chain validator bindings vs. top-30 Task Node seats in `tasknode` DB, and verify the estimated market value of Finance Hub / GoodAlexander access with the founder and Pillar 9 owners.
- **Suggested direction for the fix:** Propose one concrete, default economic package for founder sign-off rather than leaving all switches blank:
  - Explicitly anchor the value proposition on **exclusive Finance Hub + research API access** (and PFT/credit only if budgeted to cover measured baseline hosting).
  - Set a clear **old-chain access sunset trigger** (e.g., 60 days after Step 4 genesis launch + readiness gate) so validators must migrate to keep access.
  - Default `R15` to **active new-chain membership + participation floor after a 14-day ramp-in** (Option 4d), with a 7-day *lapsed* grace window.
  - Add an explicit **equivocation rule**: cryptographic proof of double-signing triggers immediate `revoked` state in Task Node (terminating Finance Hub access and rewards) and an expedited removal proposal.
  - Decide explicitly who pays for Pillar 5's initial two public RPC/admission endpoints (e.g., Foundation-funded until x402 metering or an RPC bonus exists, rather than hoping unpaid volunteers provide them).

##### P4 — Follower continuous sync protocol and store-lock concurrency are unspecified
- **Severity:** major
- **Where:** *What exists today* (Node and RPC server; RPC surface and speed), *Technical requirements* (`R10`).
- **What is wrong:** Today `rpc-catch-up` pulls a bounded batch of 64 blocks and exits, and RPC reads contend on the store's exclusive lock with block commits (taking 7–10 s for child-process reads). `R10` simply states *"Follower mode (to build). A non-voting node that follows the tip and verifies each certified block"* and *"Move the remaining child-process reads in-process"*, without specifying:
  1. How a follower continuously receives new certified blocks (P2P gossip/transport stream vs. HTTP RPC polling, and which peers serve them without overloading validators).
  2. How in-process reads avoid blocking commits if the underlying store lock is exclusive.
- **Why it matters:** If followers poll validators over RPC, adding external followers degrades validator liveness. If moving reads in-process still acquires an exclusive lock on the same store, high public RPC concurrency will stall block application on followers (causing tip lag and failing `R5b` probes) and stall commits on validators.
- **How to investigate:** Inspect `crates/node` storage engine, `docs/specs/fast-validator-reads-20261124.md`, `rpc-catch-up-certified-delta`, and `transport-validator-serve` to determine whether the store supports concurrent readers (e.g., RocksDB snapshots / SQLite WAL / `RwLock`) and whether non-voting peers can subscribe to block broadcasts over the existing P2P transport.
- **Suggested direction for the fix:** Specify the replication topology and concurrency model in `R10`: (1) followers sync via a dedicated non-voting P2P block/QC subscription (or dedicated Foundation/relay seed followers, never polling voting validators' RPC); (2) move reads to a lock-free/snapshot-based or `RwLock` reader path (as sketched in the fast-reads note) and prove commit latency under sustained concurrent read load in Acceptance 5.

##### P5 — Single-binary `postfiat-node upgrade` creates a rollback/guard-floor split-brain and ignores signer lifecycle
- **Severity:** major
- **Where:** *User story* (item 4), *Security model and failure modes* (Failed upgrade; Lost guard state), *Technical requirements* (`R1`, `R3`).
- **What is wrong:** `postfiat-node` is a single binary serving both the validator node and `validator-signer-serve`. Two edge cases are unhandled in `R3`:
  1. If a pre-`H` upgrade restarts the node with the new binary, the node signs at least one message at height `h+1` (advancing the signer's persistent guard floor to `h+1`), and *then* fails a health check and rolls back both binary and chain snapshot to height `h`, the chain store is now behind the signer guard floor.
  2. Swapping the single `postfiat-node` binary on disk also replaces the signer executable, yet `R3` never specifies whether `validator-signer-serve` is restarted, how node-to-signer IPC versioning is kept backward-compatible, or how guard-file schema migrations work safely.
- **Why it matters:** A failed rolling upgrade could strand a validator in a state where its rolled-back node cannot sign or catch up cleanly, or an incompatible IPC change between node and signer could silently break signing across the fleet during an upgrade.
- **How to investigate:** Inspect `crates/node/src/validator_signer.rs`, `crates/node/src/validator_signer_guard.rs`, `docs/runbooks/safe-validator-rollout.md`, and the live-canary postmortem referenced in *What exists today*.
- **Suggested direction for the fix:** (1) Define a strict node-to-signer IPC versioning policy (signer binary is upgraded separately or stays protocol-compatible across N-1/N releases, with guard schema migrations strictly forward-only and tested before release); (2) in `R3`, run pre-voting dry-run/sync health checks *before* re-enabling signing, or document/test that after a rollback to snapshot `h` the node must catch up past the guard floor `h+1` as a non-voting sync before resuming votes.

##### P6 — Expanding the consensus signer socket to non-root application commands (`R13`) weakens two-user key isolation
- **Severity:** major
- **Where:** *What exists today* (Key custody), *Technical requirements* (`R9`, `R13`, `R5a`), *Open questions* (Question 12).
- **What is wrong:** Today the signer runs under a dedicated user and refuses non-consensus signing. `R13` proposes letting `postfiat-node tasknode-bind` (run by a non-root interactive SSH user) ask the signer over its socket to sign Task Node bindings and Pillar 6 links, while `R5a` adds ratification signing. The spec omits socket access controls, UID isolation (`SO_PEERCRED`), separate sockets per privilege tier, and explicit cryptographic domain separation tags (DSTs).
- **Why it matters:** If the interactive operator SSH user has permission to connect to the same Unix socket that signs consensus votes and governance ratifications, any compromise of the operator's SSH account (or a bug in the CLI/shell) gains access to consensus-key signing capabilities—defeating the entire point of the two-user signer layout.
- **How to investigate:** Inspect `crates/node/src/validator_signer.rs`, systemd unit definitions for the two-user layout, and ML-DSA-65 signing context/domain-separation code in `crates/types` and `crates/node`.
- **Suggested direction for the fix:** Either:
  - **Recommended (cleanest):** Default Open Question 12 to **(b) delegated operator key** (validator key signs a one-time delegation certificate at onboarding; Task Node bindings and Pillar 6 links never touch the consensus signer daemon at all), OR
  - If keeping **(a)**, require **two distinct Unix sockets** with separate OS group/UID permissions (`SO_PEERCRED` verified by the signer): one restricted strictly to the `postfiat` service user for consensus/transport/ratification, and a second socket restricted to `postfiat-ops` that *only* accepts strictly domain-separated, rate-limited `tasknode-bind` / `pf.identity.link.v1` contexts.

##### P7 — Launch topology (`n = 12`, quorum `10`) has fragile liveness, and the L3 independence verifier is unbuilt with no requirement
- **Severity:** major
- **Where:** *What exists today* (Admission and removal), *Security model and failure modes* (Small set), *Technical requirements* (`R5`, `R11`), *Open questions* (Question 2).
- **What is wrong:**
  1. Proposed launch is `n ≥ 12`, quorum `10`—meaning **only 2 validators can be offline** before finality stalls (3 offline halts the chain). That is the exact same absolute fault tolerance as the current 6-node devnet (quorum 5), while doubling the number of independent human operators running volunteer/low-reward infrastructure and manual upgrades.
  2. The spec notes *"The L3 independence verifier is not built"* and that correlation checks reject shared providers/ASNs/ownership, yet no requirement in `R1–R18` assigns building, testing, or maintaining the L3 verifier or correlation dataset—and `R11` does not publish live provider/ASN slot caps before candidates spend 7 days running an observed follower.
- **Why it matters:** At `n = 12, q = 10`, routine cloud maintenance plus two slow upgrades will halt the public testnet. Meanwhile, missing L3 tooling blocks Step 4 launch (`G0`), and hidden ASN/provider caps will cause frustrating 7-day rejections for well-intentioned applicants on Hetzner/AWS/DO.
- **How to investigate:** Check `docs/architecture/launch-topology-thresholds.md`, `docs/governance/validator-registry.md`, `docs/governance/cobalt.md`, and `python/postfiat_rpc/placement_preflight.py` to verify the exact Cobalt quorum formula (`ceil(0.8n)` vs. `2f+1`), what L3 checks actually require, and how many distinct ASN/provider groups exist in recent old-chain Dynamic UNL rounds.
- **Suggested direction for the fix:** (1) Explicitly analyze quorum resilience in the spec (e.g., if Cobalt requires 80% quorum, launching at `n ≥ 16`–`20` increases offline tolerance to 3–4 nodes; or document the exact liveness SLA required at `n = 12`); (2) add an explicit requirement + acceptance criterion for the L3 verifier and correlation dataset (or explicitly defer automated L3 enforcement to post-testnet with a transparent manual audit for `G0`); (3) expose current provider/ASN capacity in `R11` and in `R5`'s local pre-check *before* a candidate begins the 7-day observation window.

##### P8 — Decision overload and unphased scope risk stalling Phase 1 validator onboarding
- **Severity:** major
- **Where:** *Technical requirements* (`R0–R18`), *Open questions for the founder* (Questions 1–12).
- **What is wrong:** The sub-page bundles ~25 separate policy decisions across 12 open questions with **zero recommended defaults**, and lays out `R1–R18` as a flat requirement set mixing near-term onboarding essentials (follower RPC, single docs page, upgrade command, signer-compatible bind, basic `top`) with heavyweight mainnet systems (reproducible second-builder SBOM pipelines, `k`-of-`n` observer federation, webhook delivery daemon, public directory, macOS Apple Silicon cross-builds, multi-proposer E6).
- **Why it matters:** Engineers cannot implement `R0`, `R3`, `R6`, `R10`, `R13`, `R14`, `R15`, or `R17` until the founder answers dozens of tactical questions. Meanwhile, blocking the first outside validators on every hardening feature contradicts the founder's staged-approach goal.
- **How to investigate:** Cross-reference `docs/plans/active/l1v2-public-testnet-path-milestone.md` against `R1–R18` to identify which items strictly gate Step 2/3 (rehearsal & early followers), Step 4 (public testnet `G0`), and Mainnet Production.
- **Suggested direction for the fix:** (1) Split `R1–R18` into explicit **Phase A (Rehearsal & Invited Followers)**, **Phase B (Public Testnet `G0`)**, and **Phase C (Open Admission & Mainnet Hardening)** tables; (2) collapse the 12 open questions into **4–5 founder-only strategic decisions** (Gate Zero timing, Foundation seat cap, Reward/Access policy, Stake/Bond yes/no) and assign a concrete, opinionated **proposed default** to every remaining technical question (Q7–Q12) that ships unless the founder overrides it.

##### P9 — Key rotation (`R15`) silently breaks Pillar 6 identity links, contradicting per-operator rewards and directory attribution
- **Severity:** major
- **Where:** *Technical requirements* (`R15`, `R18`), *Interfaces to other pillars* (To Pillar 6), *Open questions* (Question 6).
- **What is wrong:** `R15` states that after a ratified key rotation, Task Node automatically updates the binding to the new key *"so grants continue without a re-bind."* Yet *Interfaces to Pillar 6* states: *"The link goes stale on rotation, as Pillar 6 specifies; the binding does not."* If Open Question 6 chooses **per-operator rewards through Pillar 6** (the primary anti-farming option) or `R18` displays Pillar 6 identity in the validator directory, a routine key rotation silently breaks identity linkage and potentially halts grants—directly contradicting `R15`.
- **Why it matters:** Operators rotating keys for routine security hygiene (or after a lost guard) will silently lose identity attribution or per-operator reward eligibility despite being told no re-bind is needed.
- **How to investigate:** Review Pillar 6 (`06-hive-identity.md`) on how `pf.identity.link.v1` handles key rotations and whether Pillar 6 can bind to a stable `validator_id` (or recognize ratified rotation continuity receipts) rather than a single mutable consensus public key.
- **Suggested direction for the fix:** Either bind Pillar 6 links to the immutable `validator_id` (or accept the on-chain rotation continuity receipt as extending the link), OR explicitly require `R6` rotation runbooks + `R12` `top` alerts to prompt a re-link within a defined grace window and document that per-operator rewards cannot rely on a stale link.

##### P10 — ML-DSA-65 benchmarking (`R8`) is sequenced too late, and RPC gaps for FastPay / wallets are unchecked
- **Severity:** major
- **Where:** *What exists today* (RPC surface and speed), *Technical requirements* (`R8`, `R10`, `R11`), *Acceptance criteria* (Criteria 5, 9).
- **What is wrong:**
  1. Each ML-DSA-65 vote signature is 3,309 bytes. At `n = 12`–`35`, quorum certificates can easily add tens to hundreds of KB per block—translating to major bandwidth and storage growth over months. Yet `R8` is treated as a parallel/late deliverable rather than an upstream prerequisite for `R11` (hardware sizing & monthly hosting cost) and `n`-cap selection.
  2. The spec notes *"There is no `account_info`"* and only validates `status`, `account`, and `tx` latency in Acceptance 5, without checking whether `methods.md` supports the methods and push/WebSocket subscriptions needed for FastPay's low-latency task payouts, memos, NFTs, or Local Task Node (Pillar 5).
- **Why it matters:** If post-quantum certificate storage/bandwidth makes 2-core cheap cloud instances unviable, both hardware requirements (`R11`) and hosting-cost economics change dramatically. And if the follower RPC lacks wallet/FastPay methods or push notifications, building `R10` will require an immediate second pass.
- **How to investigate:** Inspect `docs/quantum/signature-size-and-certificates.md`, `docs/rpc/methods.md`, and Pillars 2, 4, and 5 to list every RPC method and latency/subscription requirement needed by wallets, FastPay, block explorer, and Local Task Node. Measure actual bytes/block and disk growth on the 6-node devnet today.
- **Suggested direction for the fix:** Move `R8` and a baseline measurement of the current 6-node devnet (CPU, RAM, disk growth/day, bandwidth/day) into **Step 1** so `R11` and the economic model use real numbers; add an explicit RPC method-coverage check against Pillars 2, 4, and 5 in `R10`.

##### P11 — Post-activation (`> H`) upgrade failure lacks a recovery runbook, and rollback snapshot pruning has no trigger
- **Severity:** minor
- **Where:** *Security model and failure modes* (Failed upgrade), *Technical requirements* (`R3`, `R3a`).
- **What is wrong:** `R3` correctly prohibits rolling back after activation height `H` (*"halts and alerts after H"*), but does not specify what an operator (or the network) does next if a consensus-breaking upgrade fails at or after `H`. Additionally, `R3` says *"keeps one rollback pair until the release's observation window closes, then prunes it"*, without specifying what mechanism triggers pruning (a systemd timer? next upgrade?).
- **Why it matters:** If a release bug trips at `H` across ≥3 validators, operators need a documented recovery path (hotfix release, checkpoint resync, or emergency governance override). And if rollback snapshots aren't pruned automatically, small validator disks will fill up silently.
- **How to investigate:** Check `docs/runbooks/replicated-state-v2-activation.md` and `docs/runbooks/public-operator-runbook.md#7-upgrade-and-rollback` for existing post-`H` recovery steps and snapshot storage paths.
- **Suggested direction for the fix:** Add a concise post-`H` recovery procedure to `R3`/`R11` (wait for signed hotfix release -> verify -> apply without deleting guard -> resync from snapshot/peers) and specify a lightweight systemd timer (or `postfiat-node upgrade --prune`) for rollback cleanup, surfaced in `top`'s disk panel.

##### P12 — `postfiat-node top` (`R12`) and webhook alerting (`R12a`) lack diagnostic depth, egress resilience, and packaging details
- **Severity:** minor
- **Where:** *Technical requirements* (`R12`, `R12a`), *Acceptance criteria* (Criterion 8).
- **What is wrong:** `R12` renders status from loopback RPC, systemd, disk, and the remote signed feed, but does not specify: (a) asynchronous/cached feed fetching so slow observers or strict validator egress firewalls don't stall the TUI; (b) root-cause diagnostic counters (peer latency, NTP clock drift, last signed height, signer round-trip time, store-lock wait) to explain *why* precommits were missed; (c) a non-interactive `--json` mode for scripting and non-systemd/Docker compatibility; and (d) how `R12a`'s alert delivery service is packaged, configured, and authenticated (subcommand + systemd unit? webhook retry/backoff/secret storage?).
- **Why it matters:** The founder specifically highlighted a *"delightful UX I can log into if I ssh in to see its key uptime or other statistics."* A TUI that hangs on remote HTTP fetches or shows a red panel without actionable diagnostics won't meet that bar.
- **How to investigate:** Inspect `scripts/node-status`, `docs/runbooks/validator-doctor.md`, `scripts/testnet-monitor-snapshot`, and the `postfiat.monitor-alert.v1` envelope schema.
- **Suggested direction for the fix:** Specify `postfiat-node top` with non-blocking background feed polling (with local cache fallback), a diagnostics drawer/panel (peer count, clock skew, signer RTT, last signed vs. finalized height), `--json` output, and package `R12a` as a dedicated `postfiat-node alert-relay` subcommand + optional systemd unit configured via an environment/config file with `0600` permissions.

##### P13 — Missing transition plan for non-`G0` old-chain validators and missing trust model for the public RPC directory (`R18`)
- **Severity:** minor
- **Where:** *What exists today* (Current network), *Technical requirements* (`R0`, `R18`), *Interfaces to other pillars* (To Pillars 2 and 5).
- **What is wrong:**
  1. Recent old-chain rounds score 42–55 validators for a 20-seat UNL. Even if all 20 enter `G0`, 22–35 existing old-chain validators are excluded from genesis, yet `R0` has no onboarding path or communication plan for them (other than waiting for Step 5 open admission).
  2. `R18` publishes an optional RPC endpoint per validator in a public directory, and Pillar 5 (Local Task Node) needs public follower/admission endpoints—yet there is no endpoint health-check, abuse-reporting, or client-side verification guidance to prevent a malicious directory entrant from advertising a logging, censoring, or stale RPC endpoint.
- **Why it matters:** Alienating half the existing validator community during the transition undermines decentralization and goodwill, while treating an unvetted directory as a wallet/Local Task Node endpoint list creates a client-security trap.
- **How to investigate:** Check how many active old-chain validators currently hold Task Node bindings and review Pillar 5 (`05-local-task-node.md`) to see how Local Task Node selects and verifies RPC endpoints (e.g., light-client header/QC verification vs. trusting the RPC server).
- **Suggested direction for the fix:** (1) Explicitly invite the non-top-20 old-chain validators into **Step 3** as early R10 candidate followers so they immediately build their 7-day observation record for the first post-genesis admission wave; (2) clarify in `R18` whether listed endpoints are purely informational or health-checked by observers, and note that clients (Pillar 5) must verify certified proofs/receipts rather than trusting any directory-listed RPC endpoint.

## Claude Opus 5.5 — 76/100

*Model `anthropic/claude-opus-5.5` · 10071 output tokens*

#### Summary judgement

This is a strong sub-page. It takes the founder's Deliverable 3 story seriously and answers its direct questions:

- **"Do we have RPC software?"** Yes. It is `rpc-serve` in `postfiat-node`.
- **"How does it get updated?"** Through the same signed release.
- **"Where is the docs page?"** R11 defines it.
- **"How do I connect to Task Node easily?"** R13, a single command.
- **"Delightful SSH UX"** R12, `postfiat-node top`.

The page is grounded in real repository state, with file-level citations and candid "today vs. required" tables. Its security model, failure modes and acceptance criteria are concrete and mostly testable.

It falls short of a higher score for five reasons:

1. **Gating is heavy.** It wraps the founder's "first phase: get some validators on the new chain" in a large admission, ratification and observer apparatus. It also chains it behind gates that include NAVCoin swap stability (Z3). As a result, outside validators may arrive much later than the founder's "first phase" framing implies.
2. **Unexamined technical assumptions.** Several load-bearing ones have not been checked:
   - whether the precommit QC records all votes, which drives participation scoring;
   - circular bootstrapping of the "independent" observer feed;
   - rolling-upgrade liveness at small n;
   - whether G0 can actually be populated.
3. **The economics are honestly flagged but not resolved.** The incentive is research access plus $20 of credit. There is no stake, hosting cost is unmeasured, and old-chain bindings unlock everything. Nothing yet makes running a new-chain validator rational or farming-resistant.
4. **The founder has to answer 12 open questions**, many with sub-questions, before several steps can start. The page gives no recommended default for most of them.
5. **No sequencing by effort or time.** The founder's meta-question is "what activities maximise likelihood of success". The page does not say what can ship this week versus what waits for gates.

#### Correspondence to the founder's intention

**Strong matches:**

- **Direct Q&A.** The RPC questions get a plain answer, including the honest caveat that RPC is not yet a separate product. A stand-alone follower must be built.
- **Update path.** `upgrade --check`, rolling versus activation-height releases, snapshot and rollback. This matches "a clear way to update my validator software, which needs to exist".
- **Run my own RPC.** R10 covers private and public profiles with copy-paste examples.
- **Single docs page.** R11 has explicit sections and is placed next to the old-chain setup page.
- **Easy Task Node connect.** R13 is one command, uses the signer socket, needs no root and offers an offline mode.
- **SSH dashboard.** R12 specifies panels, keys, first-run guidance and degraded modes. It is concrete enough to build.
- **Wider brief.** Finance Hub and GoodAlexander access is treated as the core incentive. The old onboarding tool is explicitly re-pointed (R14, the `ValidatorSettings.jsx` default). The multi-phase migration is reflected in Steps 1–5.
- **Post-human governance framing.** Admission verdicts become public, recomputable records cited by Pillar 1. This ties nicely to the founder's central narrative.

**Divergences or risks to intent:**

- **The founder treats getting validators onto the new chain as the *first* phase.** The page puts outside validators at Step 4, behind Gate Zero (Z1–Z3) and C1, C4, G6 and D4. It even makes NAVCoin swap round-trips (Z3) a precondition. That may be the operator's recorded rule, but the page does not question whether it serves the founder's sequencing.
- **"Extremely easily" connect versus the founder burden.** Operator-side connect is easy. Joining is deliberately hard: follower observation for 7 days at ≥ 99.5%, an evidence packet, commit-reveal ratification. That is defensible, but it is a design choice the founder never asked for. It should be surfaced as a choice, not presented as a given.
- **No timeline or priority ordering.** The founder wants "activities to maximise likelihood of success". The page gives a dependency order but no effort estimates and no list of quick wins. Two near-term items could ship now for the Foundation fleet: `top` and the docs page.
- **Density.** The page is close to an engineering design document. A founder skimming for "what do I need to decide" must dig through about 12 multi-part questions with no recommended default for most.

#### Technical robustness

**Good:**

- Fail-closed catch-up.
- Signer under its own user, with a never-restore guard rule.
- "Never roll back after H".
- Public reads isolated on follower hosts, because the validator store lock is shared with commits.
- Explicit refusal paths in the upgrade command.
- Recognition that binary-only rollback failed in a postmortem.
- A k-of-n signed feed to replace the unsigned single-host feed.
- An R8 benchmark for ML-DSA-65 certificate growth.
- Rotation handled by validator id.

**Weak or unverified:**

- **Participation is measured from `precommit_qc.votes`.** If the QC carries only the first quorum of votes, honest but slower validators are systematically under-counted. With n=6 and quorum 5, one validator is excluded from every QC. That threatens a 95% threshold.
- **Observer bootstrap.** R16 requires at least one non-Foundation observer and k-of-n agreement before new bindings and grants work. At Steps 1–3 there are no outsiders, so the feed cannot meet its own definition.
- **Rolling upgrades at small n.** At 6 validators with quorum 5, two nodes upgrading at once halt finality. At 12 with quorum 10, three do. The upgrade flow has no coordination or staggering mechanism.
- **R3 sequencing.** R3 refuses to run when a key file sits in the data directory. The whole fleet uses key files today, so the upgrade tool is unusable until the Step 1 migration completes.
- **Consensus key signing application payloads.** R13 has the signer take on application contexts: binding, the Pillar 6 link and ratification. That widens the consensus key's signing surface. Strict domain separation is asserted but not specified.
- **Candidate uptime measures the follower, not the eventual validator host.** It can be gamed by running the follower on premium infrastructure.
- **The recompute script needs public blocks.** No public RPC exists yet.
- **RPC read latency of 7–10 s** is acknowledged. The fix is "move reads in-process". Followers will hit the same store-lock architecture unless the reader is redesigned, and the page does not show that follower mode avoids the exclusive lock.

#### Economic coherence

- **The incentive is research access plus an optional $20 per month of credit.** There is no stake, and hosting cost is unmeasured. The old chain's minimum is 2 cores and 4 GB RAM. A reliable validator plus a follower host likely costs more than $20 a month, so the economics rest almost entirely on how much operators value Finance Hub access. The page says so honestly, but does not quantify whether this attracts the more than 10 independent groups that L3 requires.
- **Nothing pulls validators across.** Old-chain bindings unlock everything, access never lapses, and credit is per validator id with multiple bindings per account. The page lists all of this as "founder choices" without recommending a coherent package.
- **Is G0 feasible?** G0 = Selected ∩ Receipted from a 20-seat UNL. The Foundation cap of 2 of 12 needs at least 10 independent receipting operators. With weak incentives, uptake is a real launch risk, and there is no contingency.
- **Unfunded duties.** Public RPC and admission endpoints are unpaid duties, yet Pillar 5 depends on them.
- **No PFT budget.** PFT over FastPay is offered as option 6c, but there is no emission budget, cap or treasury source.
- **No farming resistance yet.** Without stakes and with hidden common ownership undetectable, Sybil resistance rests on the admission gate and old-chain scoring. That is acknowledged, but bonding is deferred.

#### Problems

**P1 — Outside validators sequenced far later than the founder's "first phase"**
- **Severity:** major.
- **Where:** Gates table (Gate Zero Z1–Z3); R0 Steps 1–5; open question 1.
- **What is wrong:** Outside validators wait on Z1–Z3, C1, C4, G6 and D4, including NAVCoin swap stability (Z3). The founder framed getting some validators on the new chain as the *first* step of migration.
- **Why it matters:** It could stall Outcome B for months behind unrelated NAVCoin work, and it weakens the "decentralised new chain" story.
- **How to investigate:** Read `testnet-path.md` and the milestone doc for the rationale behind Z3. Estimate how long each gate will take to close.
- **Suggested fix:** Recommend a default, likely option 1(b) or 1(c). Then explain why Z3 must block validators rather than only the public testnet. Consider decoupling validator onboarding from Z3, with activation-height upgrades as the mitigation.

**P2 — Participation metric may be biased by QC composition**
- **Severity:** critical.
- **Where:** Participation metrics table; R16 content; Rewards row.
- **What is wrong:** Participation is "signed ÷ committed from `precommit_qc.votes`". If a QC contains only the first quorum of votes, an online validator that arrives late is not counted.
- **Why it matters:** The ≥ 95% reward threshold, warnings, removal and possibly access all rely on this number. With n=6 and quorum 5, at least one validator is missing from every QC, so participation averages about 83%.
- **How to investigate:**
  1. Inspect QC assembly in `crates/node` and the consensus crate.
  2. Check whether the QC holds all votes received before commit, or exactly the quorum.
  3. Compare the live `registry.json` counts per validator.
- **Suggested fix:** Measure participation from all received, signed precommits (vote logs gossiped to observers), or from a late-vote inclusion window. Alternatively, set thresholds relative to quorum composition.

**P3 — Observer-feed bootstrap is circular**
- **Severity:** major.
- **Where:** R16; security model "Participation feed"; failure mode "Feed stale or below k"; Step 1.
- **What is wrong:** R16 requires at least one non-Foundation observer and k-of-n agreement, and new bindings fail with a 503 below k. Steps 1–3 have no outsiders.
- **Why it matters:** Step 1 claims to "prove the signed feed", and acceptance criteria 6 and 7 rely on it. As specified, the feed cannot be legitimately satisfied before outsiders exist.
- **How to investigate:** Map which actors can run observers at each step.
- **Suggested fix:** Define a bootstrap mode, such as Foundation-only k-of-n across separate hosts and keys, explicitly labelled. The independence requirement then activates at a named step.

**P4 — Rolling upgrades can halt a small set**
- **Severity:** major.
- **Where:** R3, R3a; failure modes; "Small set".
- **What is wrong:** There is no staggering or coordination for rolling releases. At 6/5 two simultaneous restarts stop finality, and at 12/10 three do. Outsiders upgrading on announcement will cluster in time.
- **Why it matters:** Liveness incidents on the first public network damage credibility.
- **How to investigate:**
  1. Check `safe-validator-rollout.md` for Foundation-side staggering.
  2. Simulate upgrade arrival times.
- **Suggested fix:** Give each validator a deterministic upgrade slot derived from validator id and release. `upgrade` checks the live ratifier count before stopping and refuses if this node's downtime would breach quorum. `top` displays the slot.

**P5 — `upgrade` refuses the current fleet's key layout**
- **Severity:** minor.
- **Where:** R3, "refuses to run if a key file sits in the data directory"; Key custody row ("fleet uses key files").
- **What is wrong:** The upgrade tool is unusable on today's fleet until the Step 1 signer migration finishes. The page does not describe the transition.
- **Why it matters:** It blocks dogfooding of R3 during Step 1.
- **How to investigate:** Check where key files sit today.
- **Suggested fix:** Add a migration subcommand (`upgrade --migrate-signer`) or a documented override for the Foundation fleet during Step 1.

**P6 — Consensus key signs application contexts**
- **Severity:** major.
- **Where:** R13 signer mode; Keys table; open question 12.
- **What is wrong:** Binding, the Pillar 6 link and ratification payloads would all be signed by the consensus key. Domain separation is asserted ("no consensus slot") but not specified.
- **Why it matters:** A cross-protocol signature confusion could let an application payload be replayed as a consensus message, or the reverse. It also enlarges the attack surface on the signer socket.
- **How to investigate:**
  1. Review `validator_signer.rs` context tagging.
  2. Check whether ML-DSA context strings or domain prefixes are enforced.
  3. Audit who can reach the socket.
- **Suggested fix:** Recommend option 12(b), a delegated operator key, as the default. If 12(a) is chosen, specify the domain-separation tags, the per-context allow-list and the socket ACLs.

**P7 — Candidate uptime measures the wrong host**
- **Severity:** minor.
- **Where:** R5b; Participation metrics ("Candidate uptime").
- **What is wrong:** The follower is probed, but the validator may later run elsewhere. Probes measure reachability and lag, not signing capability.
- **Why it matters:** It is weak evidence for admission and can be gamed.
- **How to investigate:** Check whether Policy V1 binds the measured endpoint to the eventual validator host.
- **Suggested fix:** Require the candidate's follower to run on the same host and IP as the eventual validator, with an attested host key. Alternatively, add a short probationary voting window after ratification.

**P8 — Is G0 feasible to populate?**
- **Severity:** critical.
- **Where:** Step 4; open question 2; Economics.
- **What is wrong:** Launch needs n ≥ 12 with at most 2 seats per correlated group, so at least 10 independent groups must be both selected and receipting. Receipting requires effort for weak rewards. There is no estimate of uptake and no fallback if fewer receipt.
- **Why it matters:** The public testnet may never launch, or may launch with a waiver that undermines L3.
- **How to investigate:**
  1. Count distinct correlation groups in the latest frozen rounds.
  2. Survey old-chain validators' willingness to receipt.
  3. Estimate how many will run postfiatl1v2.
- **Suggested fix:** Do a pre-launch outreach and count step with a minimum go/no-go threshold. Define a contingency, such as a smaller n with a temporary cap, or a recruitment incentive.

**P9 — The incentive may not cover hosting**
- **Severity:** major.
- **Where:** Economics; open question 6; R17.
- **What is wrong:** $20 of credit, no stake, and unmeasured cost (R11 publishes cost later). The page relies on research access value without quantifying it.
- **Why it matters:** Rational operators may not join or stay. That feeds P8, and the risk is that only people farming Finance Hub show up.
- **How to investigate:**
  1. Measure fleet resource use and price a validator and a follower across providers.
  2. Estimate the market value of Finance Hub access.
- **Suggested fix:** Publish the cost now from the devnet hosts. Recommend a default package, for example access plus PFT scaled by participation with a per-operator cap. Tie it to a treasury budget.

**P10 — No migration pull and farmable access**
- **Severity:** major.
- **Where:** Research access row; Economics gaps; open question 4.
- **What is wrong:** Any unrevoked binding on either chain unlocks everything, access never lapses, and multiple bindings per account are allowed.
- **Why it matters:** Nothing pulls validators to the new chain, and the arrangement invites bind-and-quit.
- **How to investigate:** Check `validator-bindings.js` and `index-viewer-routes.js` behaviour. Count current bindings per account.
- **Suggested fix:** Recommend option 4(d) as the default: new-chain registry membership plus participation after a ramp-in. Add a dated sunset for old-chain bindings and one grant per operator.

**P11 — Rotation trusts the feed rather than the chain**
- **Severity:** minor.
- **Where:** R15 "Active".
- **What is wrong:** Task Node moves the binding using the feed's rotation record.
- **Why it matters:** It adds trust in observers for something verifiable on chain.
- **How to investigate:** Check whether ratified rotations are readable from registry state over RPC.
- **Suggested fix:** Read rotation from finalized registry state through a pinned checkpoint and follower, with the feed only as a convenience.

**P12 — Follower read path may inherit the store-lock problem**
- **Severity:** major.
- **Where:** RPC surface row (7–10 s reads, exclusive lock); R10 read path.
- **What is wrong:** The fix is to move reads in-process. On a follower, block import still takes the exclusive lock, so public reads may stall during sync.
- **Why it matters:** Public RPC, wallets, FastPay latency and Pillar 5 all depend on this.
- **How to investigate:**
  1. Review the store design and the fast-reads note's "resident reader".
  2. Benchmark a follower under import plus read load.
- **Suggested fix:** Specify a concurrent read architecture for the follower, such as snapshots/MVCC or a read replica, and make it part of R10 acceptance.

**P13 — The recompute script needs public data that doesn't exist**
- **Severity:** minor.
- **Where:** R16; acceptance criterion 7.
- **What is wrong:** "Anyone can recompute from public blocks", but there is no public RPC until Step 3+.
- **Why it matters:** Criterion 7 is untestable early.
- **How to investigate:** Check what public block archives exist.
- **Suggested fix:** Publish a block archive (for example, IPFS per Pillar 2) or a read-only public follower in Step 1.

**P14 — Too many open questions without recommended defaults**
- **Severity:** major.
- **Where:** Open questions 1–12.
- **What is wrong:** There are about 12 multi-option questions. Most present tradeoffs but no recommendation.
- **Why it matters:** It slows founder decisions, and several steps are blocked on them.
- **How to investigate:** Identify which questions block Steps 1–3 versus later steps.
- **Suggested fix:** Mark a recommended option for each. Split into "decide now" and "decide by Step 4".

**P15 — No effort, timeline or quick-win ordering**
- **Severity:** major.
- **Where:** Whole page; R0.
- **What is wrong:** The page never answers which activities to do first to maximise success.
- **Why it matters:** The founder's top-level question is about prioritisation.
- **How to investigate:** Estimate effort for each of R1–R18. Mark which items don't depend on any gate.
- **Suggested fix:** Add a "now / next / gated" table. Candidates for "now": R11 docs, R12 `top` on the Foundation fleet, R1 signed release, R13 signer mode, and R8 benchmark.

**P16 — The directory exposes validator infrastructure**
- **Severity:** minor.
- **Where:** R18; anti-farming (provider, ASN, country).
- **What is wrong:** Publishing state, roles, endpoints and correlation data can aid targeted DoS against a small set.
- **Why it matters:** A liveness risk at n of about 12.
- **How to investigate:** Decide which fields are public versus visible only to ratifiers.
- **Suggested fix:** Publish aggregate correlation groups, not raw ASN and country per validator. Make RPC endpoints opt-in, which is already planned.

**P17 — Stale or oddly dated evidence**
- **Severity:** minor.
- **Where:** Current network (Cobalt "last verified at height 924"); doc dates such as 20261124 and 20260901.
- **What is wrong:** The status claims may be outdated, and the dates look inconsistent.
- **Why it matters:** They drive the gate assessment.
- **How to investigate:** Re-check `network-status.md` and Cobalt at the current height.
- **Suggested fix:** Re-verify, and include a "verified as of" date.

**P18 — Offline tooling on macOS is vague**
- **Severity:** minor.
- **Where:** R1, "built for macOS … if they compile there".
- **What is wrong:** Support is conditional and undetermined. Many operators hold old-chain keys on laptops.
- **Why it matters:** It is friction for the receipt and offline-bind steps.
- **How to investigate:** Try building the receipt and keygen commands on Apple silicon.
- **Suggested fix:** Commit to support or name a supported offline environment now.

**P19 — Public endpoints for Pillar 5 are unfunded and unowned**
- **Severity:** major.
- **Where:** Interfaces to Pillars 2 and 5; open question 8.
- **What is wrong:** Pillar 5 (local shielded transactions) is blocked until Step 3. Endpoint operators are unpaid.
- **Why it matters:** The founder requires privacy on local MacBooks for production readiness, so this is a critical path.
- **How to investigate:** Trace Pillar 5's dependency on two independent endpoints.
- **Suggested fix:** Have the Foundation run two endpoints as an interim. Add a paid endpoint role, via x402 fees or a bonus, as a stated default.
