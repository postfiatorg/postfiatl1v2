# Feedback — Pillar 3 — Validator Rewards Program

*Review round 2.*

> **Mean score:** 73/100 · Gemini 4 Neon 74 · Claude Opus 5.5 72
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/03-validator-program.md), once, with the question below. Run 2026-11-26 20:38 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 74/100

*Model `google/gemini-4-neon` · 19193 output tokens*

#### Summary judgement

This is a well-grounded, unusually honest second-draft spec. It reflects real forensic inspection of both `postfiatl1v2` and `tasknode`, directly answers every explicit question in the founder's Deliverable 3 story, and cleanly surfaces existing code gaps (unsigned registry feed, no `account_info`, Cobalt only proven on key rotations, signer process built but not yet deployed, Task Node rewards switched off and denominated in $20 USD credit rather than PFT).

However, it is not yet ready to hand directly to implementation teams without significant rework or production risk. Two **critical** technical/security architecture problems remain unresolved:
1. The spec assures the founder "yes, we have RPC software" while admitting inside R10 that it is unverified whether a standalone non-voting RPC node can even follow the chain tip, while leaving 7–10s read latency with no SLO and forcing validators to expose public RPC + finality-key private-action admission on the same host/data directory as consensus signing.
2. Key custody across the offline master key, isolated hot-key signer process, Task Node binding, Pillar 6 identity linking, and routine hot-key rotation is contradictory—including a rule (R15) that appears to mark a validator *Lapsed* when it rotates its key, and a rollout (R0/R9) that defaults outsiders onto a signer process never deployed on the Foundation fleet.

On the governance and economic side, the admission path for post-genesis applicants (Step 4) has major holes (how a non-validator achieves 99.5% uptime before admission, how decentralized validators actually ratify new applicants, and an unexplained Z2 "AI model output" gate), and the economic design defers too much to 12 unbundled founder questions without proposing a coherent incentive package—especially around the severe free-rider problem on public RPC and Pillar 5's private-action admission.

#### Correspondence to the founder's intention

The spec aligns well (~85%) with the founder's explicit Deliverable 3 user story and broader brief:

- **Direct answers to founder questions:** It explicitly answers "Do we have RPC software? Where is it? How does it get updated? How do I run my own?" (same `postfiat-node` binary, same signed release, one copy-paste docs page). However, answering "yes, we have RPC software" is slightly overstated until standalone non-voting tip-following is actually confirmed to exist.
- **Core UX deliverables:** Single docs page (R11), signed update flow (R1/R3/R3a), one-command Task Node connect (R13/R14), Finance Hub & GoodAlexander content access (R15), and an SSH terminal dashboard (`postfiat-node top`, R12) are all clearly scoped.
- **Staged migration:** Respects the founder's multi-phased transition ("first being a staged approach where we get some validators on the new chain" while keeping old-chain bindings intact until chain readiness gates pass).
- **Cross-pillar alignment:** Properly wires interfaces to Pillar 1 (chain state), Pillar 2 (RPC/x402), Pillar 5 (local shielded private-action admission), Pillar 6 (validator key identity linking), and Pillar 9 (Finance Hub access tier).
- **Where it slightly misses the spirit:** The founder asked for a "delightful UX I can log into if I ssh in... such that my validator can be usefully used." `postfiat-node top` covers basic health metrics, but lacks RPC/serving telemetry, pending governance/ratification visibility, integrated doctor diagnostics, and proactive alerting—meaning operators still cannot easily *keep* a validator healthy at 99.5% uptime without babysitting SSH.

#### Technical robustness

The operational instincts are strong—double-sign guard protection, paired binary+data rollback, activation-height upgrades, ML-DSA-65 signature benchmarking (R8), and eliminating `curl | sudo bash` from Task Node connect are all the right calls. Yet several architectural cracks would fail under real-world operation:

- **Validator/RPC colocation & DoS exposure:** Today's topology shares one data directory between the validator and RPC, reads take 7–10 seconds, and `--allow-private-action-admission` requires the validator's *finality key*. Exposing public RPC on a consensus host under 7–10s read load is a direct consensus liveness hazard (especially at small `n`), and giving the RPC process access to the finality key breaks hot-key isolation unless routed through the signer process.
- **Unverified RPC follower mode & missing latency SLO:** R10 says "Confirm that a non-voting RPC node follows the tip on its own. If it doesn't, build that mode." That is a core product capability, not a minor checkbox. Meanwhile, AC 4 passes as long as latency is "published," even if it remains 7–10s (unusable for wallets, FastPay, or local Task Node).
- **Key hierarchy & rotation breakage:** The spec separates master key (offline) and hot key (in signer process), but never defines which key signs Task Node bindings (`tasknode-bind`), Pillar 6 links, or private-action receipts. Worse, R15 treats "rotated away" as *Lapsed*, implying routine key rotation revokes Finance Hub access.
- **Untested signer deployment order:** The signer process (`validator-signer-serve`) is built but unused on the 6-node Foundation fleet, yet R9 makes it the default for outsiders without requiring Foundation fleet migration and soak testing in R0 Step 1.
- **Missing validator-side ratification protocol:** R5 covers how an *applicant* submits evidence (to a centralized Foundation pipeline), and mentions a missing ratification command, but never specifies how decentralized validators receive, verify, and co-sign admission/rotation/removal transitions once the set moves beyond Foundation control.
- **Registry/uptime feed SPOF & signature mismatch (R16):** Requires "at least two independent observers" but has Task Node verify "a pinned key" (singular), leaving observer quorum verification and failover undefined.

#### Economic coherence

The spec correctly identifies the structural realities—no stake/slashing, $20/month Task Node USD credit (currently disabled), and Finance Hub + GoodAlexander research access as the primary real incentive—and properly flags anti-farming and bind-and-quit risks.

However, the economic design is still incomplete rather than coherent:

- **Hosting cost vs. incentive structure is unmodeled:** A 24/7 validator with TLS public RPC, backups, and bandwidth for 3,309-byte ML-DSA-65 certificates (at `n = 12..35`) likely costs $20–$60+/month plus non-trivial operator labor (manual activation upgrades, key rotations, Cobalt ratifications). $20/month of *non-fungible Task Node credit* does not pay hosting bills. If the program is actually "work-for-access to Finance Hub/GoodAlexander research," the spec should model and recommend that cleanly rather than leaving 12 independent questions open.
- **Acute RPC / private-action free-rider trap:** Pillar 5 requires >=2 independent public RPC endpoints with `--allow-private-action-admission` (which only validators can run). Running that endpoint strictly *increases* a validator's bandwidth cost, DoS risk, and risk of failing uptime thresholds—for $0 extra reward. Rational operators will leave it off unless incentivized or mandated.
- **Uptime threshold mismatch & impossible pre-admission floor:** Admission Policy V1 demands **99.5% uptime** (only 3.6 hours of downtime per month), while Task Node rewards require **25/30 days >= 95%**. Worse, there is no explanation of how a *new* Step 4 applicant proves 99.5% uptime before they are in the validator set, and no alerting tooling to help admitted validators stay above 99.5%.
- **Testnet access vs. monetary reward ambiguity:** The genesis spec calls the public testnet "no-reward," while Task Node credit code exists (disabled) and AC 5 expects Finance Hub access on bind. The distinction between *research access during public testnet* vs. *monetary rewards after production readiness* should be stated explicitly as a proposed default.

#### Problems

##### P1 — Standalone RPC follower mode is unverified, 7–10s read latency has no SLO, and validator/RPC colocation creates a DoS and key-isolation hazard
- **Severity:** critical
- **Where:** *In one paragraph*; *What exists today* (Node binary & RPC surface rows); *Technical requirements* (R10); *Acceptance criteria* (AC 4).
- **What is wrong:**
  1. The intro tells the founder "yes, we have RPC software," yet R10 admits it is unconfirmed whether a non-voting RPC node can continuously follow the chain tip on its own.
  2. Documented topology runs validator + RPC on a single data directory, and `--allow-private-action-admission` signs using the validator's *finality key*. Exposing public RPC on a consensus host (especially when reads take 7–10s) creates a severe DoS vector against consensus liveness and breaks hot-key isolation unless private-action signing goes through the signer process (or a dedicated admission key).
  3. R10 says "Cut the 7–10 s reads, and publish the measured figure" and AC 4 only requires publishing the figure—no latency SLO or root-cause investigation is required.
- **Why it matters:** If non-voting RPC nodes cannot follow the tip, outsiders cannot actually "run their own RPC" without being admitted as validators. If public RPC + private-action admission shares CPU/disk/keys with consensus validators under 7–10s reads, a trivial RPC flood can stall finality on a 6–12 validator network and block Pillar 5's local shielded UX.
- **How an agent should investigate:**
  - Inspect `crates/node` (`rpc-serve`, `rpc-catch-up`, `rpc-catch-up-certified-delta`, and `private_action_submit`) to verify: (a) whether continuous non-voting sync exists today; (b) why reads take 7–10s (DB locking against the validator, subprocess overhead, unindexed scans, or serialization); and (c) whether `--allow-private-action-admission` reads the raw finality key file directly or calls `validator-signer-serve`.
  - Review `docs/runbooks/public-rpc-operator-policy.md` and `systemd/postfiat-rpc.service.example` for isolation boundaries.
- **Suggested fix:**
  - Verify follower mode *now* and state honestly in "What exists today" whether standalone live RPC sync is built or must be built in R10.
  - Set concrete RPC latency SLOs in R10 and AC 4 (e.g., p95 < 200ms for `status`/`account`/`tx` under defined concurrency) and require testing continuous tip-following over live blocks.
  - Separate public RPC from validator consensus hosts (sentry/follower pattern) or enforce strict resource/rate isolation, and require private-action admission either to route through `validator-signer-serve` with a domain-separated key or to use a delegated/separate admission key so public RPC never touches raw finality key material.

##### P2 — Key-custody ambiguity across master key, hot key, signer process, Task Node binding, key rotation, and Pillar 6 linking; untested signer rolled out to outsiders first
- **Severity:** critical
- **Where:** *What exists today* (Key custody, Task Node binding); *Security model*; *Technical requirements* (R0, R6, R9, R13, R15); *Interfaces to other pillars* (Pillar 6); *Acceptance criteria* (AC 3, AC 5).
- **What is wrong:**
  1. R9 mandates an offline master key and an isolated hot-key signer process (`validator-signer-serve`), but the spec never specifies *which key* signs Task Node binding challenges (R13), Pillar 6 `pf.identity.link.v1` co-signatures, or `IdentityReceiptV1` (R4). If it's the offline master key, a root one-liner on the validator server cannot sign it without offline copy-paste; if it's the hot key, `tasknode-bind` must talk to `validator-signer-serve` rather than reading raw key files.
  2. R15 defines *Lapsed* as "removed, **rotated away** or below the participation rule." If binding ties to a hot key rather than a stable validator ID / master key, every legitimate hot-key rotation breaks Finance Hub access and Pillar 6 identity links.
  3. "What exists today" states the signer process "is not yet deployed: the fleet uses key files," yet R0 Step 1 does not require migrating and soak-testing the 6 Foundation validators on `validator-signer-serve` before making it the default for outsiders (R9).
- **Why it matters:** Ambiguous key boundaries cause either dangerous key exposure (putting master keys online or bypassing the signer process) or broken operational hygiene (operators avoiding key rotations because rotation revokes their Finance Hub access). Shipping an un-soak-tested signer directly to outside validators risks double-signing or consensus halts.
- **How an agent should investigate:**
  - Inspect `crates/node/src/main_parts/cli_dispatch_parts/`, `crates/node/src/validator_signer_guard.rs`, `crates/types/src/genesis_registry.rs`, and Task Node's `server/l1-validator-identity.js` + `public/l1-validator.sh` to confirm which key type (master vs. hot/consensus) is stored in the validator registry and verified by `tasknode-validator-binding/v1`.
  - Check whether `validator-signer-serve` supports domain-separated non-consensus signing (for binding/admission) or consensus votes only.
- **Suggested fix:**
  - Add an explicit **Key Matrix** table mapping each key (offline master key, hot consensus key in signer, optional admission key) to every operation (genesis receipt, consensus vote, key rotation, Task Node bind, Pillar 6 link, private-action receipt).
  - Bind Task Node and Pillar 6 to the validator's stable registry identity / master key (or automatically preserve binding across valid registry-ratified hot-key rotations) so routine rotation never causes *Lapsed* status; reserve *Lapsed* for removal or unratified key mismatch.
  - Add Foundation fleet migration to `validator-signer-serve` + a soak-test acceptance criterion to R0 Step 1 before Step 2/3.

##### P3 — Step 4 open admission has major gaps: impossible pre-admission 99.5% uptime floor, missing validator-side ratification workflow, and unexplained Z2 AI-authority gate
- **Severity:** major
- **Where:** *What exists today* (Admission and removal, Operator CLI pieces, Gates table Z2); *Technical requirements* (R5, R6).
- **What is wrong:**
  1. Admission Policy V1 requires "99.5% uptime, accountability of at least 70" to be admitted, yet the spec never explains *how* a new Step 4 applicant (who is not yet in the validator set and not in `G0`) builds a 99.5% uptime and accountability record prior to admission.
  2. R5 specifies the *applicant's* CLI and admits packets go to "the transition proposer (today the Foundation pipeline, a recorded dependency to remove)," while "What exists today" notes a "ratification command" is missing—yet there is **no requirement** specifying how *existing* outside validators discover pending admission/rotation/removal packets, independently verify Policy V1, and submit their ML-DSA-65 ratification signatures without a centralized Foundation operator.
  3. Gate Z2 states "Decision on whether AI model output stays authoritative | It fixes the admission evidence path," which appears to contradict the spec's framing of a "deterministic admission policy" and is never explained anywhere else on the page.
- **Why it matters:** Without a concrete pre-admission evidence source and a decentralized validator-side ratification CLI/protocol, Step 4 ("Open admission") is unbuildable and registry transitions remain permanently Foundation-centralized.
- **How an agent should investigate:**
  - Read `docs/governance/validator-registry.md`, `docs/architecture/l1v2-testnet-genesis-and-launch-spec.md`, `docs/status/testnet-path.md` (specifically Z2 and C4), and `python/postfiat_rpc/tasknode_unl_v2.py` to determine: (a) where uptime/accountability scores come from for non-validators (old-chain UNL, non-voting candidate probes, or Task Node); (b) what C4's ratification client protocol actually looks like; and (c) what AI model output Z2 refers to.
- **Suggested fix:**
  - Clarify the exact pre-admission evidence source for Step 4 applicants (e.g., candidate follower-node uptime probes, old-chain history, or Task Node standing) and align it with OQ 3.
  - Add a requirement (e.g., **R5a**) for the **existing-validator ratification workflow**: how validators fetch pending registry proposals, verify evidence locally via Policy V1, and sign/submit ML-DSA-65 ratification votes (plus visibility in `top`).
  - Briefly explain Z2 in the Gates table or Admission section so builders understand how AI-scored vs. deterministic evidence affects R5.

##### P4 — Economic design is fragmented across 12 unbundled open questions, lacks cost modeling, and leaves a severe free-rider trap on public RPC / private-action admission
- **Severity:** major
- **Where:** *Economics*; *Open questions for the founder* (OQ 1–12); *Technical requirements* (R10, R17).
- **What is wrong:**
  1. Instead of proposing a coherent recommended economic model with tradeoffs, the spec punts 12 coupled decisions to the founder as standalone questions (credit vs. PFT vs. access-only; per-validator vs. per-operator; lapse rules; bond vs. no bond; RPC bonuses; dual-chain rewards).
  2. There is a stark incentive mismatch on public RPC and private-action admission: Pillar 5 cannot function without >=2 independent public endpoints running `--allow-private-action-admission`, yet validators who enable it incur higher hosting costs, bandwidth, and DoS risk for zero additional benefit—creating a classic free-rider failure.
  3. The admission uptime floor (99.5%) and reward uptime floor (25/30 days >= 95%) are unexplainedly different, and the distinction between *testnet research access* vs. *mainnet monetary rewards* is muddled against the genesis spec's "no-reward" testnet rule.
- **Why it matters:** Even if all code ships cleanly, an incoherent incentive design will either fail to attract reliable outside operators, leave Pillar 5 with zero non-Foundation RPC endpoints, or allow cheap farming of Finance Hub and Task Node credits.
- **How an agent should investigate:**
  - Pull real host sizing/cost numbers from the 6-node devnet (`docs/status/network-status.md`, infra configs) and estimate bandwidth/storage costs at `n = 12` and `n = 35` given 3,309-byte ML-DSA-65 signatures.
  - Check Pillar 9 and Pillar 5 specs to confirm Finance Hub's intended value tier and Pillar 5's exact minimum RPC/admission requirements.
- **Suggested fix:**
  - Group the 12 open questions by **which R0 step they actually block** (e.g., Step 3 testnet blockers vs. Step 4/mainnet blockers) and provide a **recommended default package** for the founder to approve or override (for example: *research access + no monetary credit during Step 3 testnet; active-registry + grace-window access rule; per-operator cap via Pillar 6 for monetary grants; Foundation + mandated/compensated subset for Pillar 5 public admission endpoints*).
  - Explicitly explain and align the 99.5% admission floor vs. 95% reward floor.

##### P5 — R16 registry/uptime feed contradicts itself ("two independent observers" vs. "a pinned key") and leaves single-host failover undefined
- **Severity:** major
- **Where:** *What exists today* (Registry and participation feed); *Security model*; *Technical requirements* (R16); *Acceptance criteria* (AC 6).
- **What is wrong:** R16 mandates "at least two independent observers" and then says "the feed is signed with ML-DSA-65, and Task Node checks it against **a pinned key**" (singular). If one server aggregates two RPCs and signs with one key, it remains a single trust root and a single point of failure (`research.goodalexander.com:18443`). Furthermore, the spec does not define failover behavior if one observer disagrees or goes offline for >1 hour (which currently triggers 503s on binds/grants and breaks `top`'s eligibility view).
- **Why it matters:** A single aggregator signing key defeats the security goal of independent observation, and a single feed URL creates unnecessary operational fragility for onboarding, grants, and monitoring.
- **How an agent should investigate:**
  - Inspect `tools/tasknode-validator-binding/registry.py`, `server/l1-validator-identity.js`, `server/l1-validator-rewards.js`, and `docs/wiki/operations/airdrop-recovery-and-l1-uptime.md` to see the exact JSON schema, fetch/cache TTL logic, and how Task Node currently consumes `registry.json`.
- **Suggested fix:**
  - Specify true multi-observer verification in R16: each independent observer hosts and signs its own `postfiat-validator-uptime-v1` payload with its own pinned ML-DSA-65 key (or produces co-signatures), and Task Node verifies a quorum (e.g., 2-of-3) with explicit reconciliation rules (exact agreement on finalized `precommit_qc.votes` up to the common finalized height) and automatic failover when one observer is unreachable.
  - Update AC 6 to verify quorum signature checking and single-observer outage tolerance.

##### P6 — Upgrade and rollback design (R3) omits filesystem/user isolation for the double-sign guard, disk-space pre-checks, and cross-activation rollback safety
- **Severity:** major
- **Where:** *Security model*; *Technical requirements* (R3, R3a, R9); *Acceptance criteria* (AC 3).
- **What is wrong:**
  1. R3 requires `postfiat-node upgrade` to back up and restore the data directory while "never copy[ing], restor[ing] or reinitialis[ing] the signer guard," but never requires placing the signer guard and hot key in a **separate directory and Unix user** (e.g., `/var/lib/postfiat-signer` owned by `postfiat-signer`) from chain data (`/var/lib/postfiat` owned by `postfiat`). Relying solely on exclusion flags during backup/restore is brittle.
  2. Backing up the entire chain data directory on every upgrade requires >=2x chain-data free disk space—with no pre-flight disk check or snapshot strategy, upgrades on modest cloud hosts will fail mid-copy and risk corrupting state.
  3. R3 does not address what happens if a health-check fails and triggers automatic rollback *after* the network crosses consensus activation height `H` (rolling back binary+data strands the node on the pre-H fork) or after the new binary already signed rounds (requiring clear operator guidance).
- **Why it matters:** Double-sign guard accidents and disk-full upgrade failures are two of the most common ways real validators get slashed/removed or bricked during coordinated upgrades.
- **How an agent should investigate:**
  - Inspect `crates/node/src/validator_signer_guard.rs`, `docs/runbooks/safe-validator-rollout.md`, `docs/runbooks/public-operator-runbook.md` (especially §7 canary postmortem), and the systemd unit templates to see current paths, users, and DB layout.
- **Suggested fix:**
  - Mandate separate OS users and directories for node chain data vs. signer keys/guard state in R3 and R9 so chain-data backup/restore *structurally cannot* touch the guard.
  - Add a pre-flight free-disk check (and optional filesystem snapshot / hardlink support) to R3.
  - Specify explicit rollback policy around activation height `H` (refuse automatic downgrade past `H`; halt cleanly and surface an actionable alert instead).

##### P7 — `postfiat-node top` (R12) risks DB lock/permission failures, omits key operator telemetry, and provides no alerting to sustain 99.5% uptime
- **Severity:** major
- **Where:** *User story* (Step 7); *Technical requirements* (R12); *Acceptance criteria* (AC 7).
- **What is wrong:**
  1. R12 says `postfiat-node top` reads "over the local status endpoint **and store**." Opening the chain store directly from an SSH user session risks either permission failures (units run as user `postfiat` under `ProtectSystem=strict`) or embedded-DB lock contention against the running node.
  2. `top` omits telemetry operators actually need to be "usefully used": RPC health/latency/private-action admission counts (when RPC is enabled), Task Node binding & key-rotation status, pending Cobalt/registry ratification proposals awaiting signature, and integrated `validator-doctor` health checks.
  3. Maintaining 99.5% uptime (max ~3.6 hours/month downtime) is unrealistic with only a passive SSH TUI and a local alert spool—there is no push alerting or Prometheus/metrics endpoint specified.
- **Why it matters:** Operators will either have to `sudo` to view a read-only dashboard, risk interfering with node storage, or silently drop below uptime thresholds between SSH logins.
- **How an agent should investigate:**
  - Check `scripts/node-status`, `scripts/testnet-validator-doctor`, `scripts/testnet-rpc-doctor`, and `crates/node` status endpoints to see what is already exposed over local HTTP/Unix sockets and whether Prometheus metrics exist.
- **Suggested fix:**
  - Restrict `postfiat-node top` to read-only local HTTP/Unix-socket status APIs + OS stats (`statvfs`/systemd unit state) + gracefully degraded remote feed fetches (never opening the node DB directly, never requiring root, never hanging if the remote feed is down).
  - Add RPC stats, binding/key state, pending ratifications, and `validator-doctor` diagnostics to `top`.
  - Add a minimal operator alerting interface (e.g., local Prometheus `/metrics` endpoint and/or webhook hook for missed precommits, low disk, signer errors, and upcoming activation heights).

##### P8 — Connect command (R13) and release builds (R1) assume root/`unshare --net` and Linux-only binaries, creating friction for containers and macOS offline master keys
- **Severity:** minor
- **Where:** *User story* (Step 6); *Technical requirements* (R1, R13).
- **What is wrong:**
  1. Step 6 still says "I paste one command as root" and R13 relies on `unshare --net`, which requires `CAP_SYS_ADMIN`/root and fails on many hardened VPS kernels or unprivileged Docker containers. Once the signer ships inside the trusted signed release, requiring root just for `unshare --net` is unnecessary friction if signing can run via the signer socket or an explicit `--offline` flag.
  2. R1 ships binaries only for `linux-x86_64` and `linux-aarch64`, yet R9 tells operators to keep their master key offline (commonly on a personal macOS/laptop machine, consistent with the founder's MacBook-centric local workflow). No macOS CLI binary is shipped for offline `operator-onboarding-keygen`, `IdentityReceiptV1`, or offline `tasknode-bind`.
- **Why it matters:** Operators will either fail to run `tasknode-bind` in containerized/restricted environments or be forced to keep their master key on the Linux validator VPS, undermining R9.
- **How an agent should investigate:**
  - Check `public/l1-validator.sh` and `crates/node` Cargo/CI targets (`docs/release-process.md`, GitHub Actions workflows) to verify whether `postfiat-node` (or a lightweight offline key-tool binary) builds cleanly on `aarch64-linux` and `aarch64-apple-darwin` (macOS) given PQ crypto and DB dependencies.
- **Suggested fix:**
  - Support non-root execution for `postfiat-node tasknode-bind` (talking to `validator-signer-serve` or via offline challenge/response without requiring `unshare --net` when run in `--offline` mode).
  - Ship a lightweight macOS (`aarch64-apple-darwin`) offline key/receipt/bind CLI alongside the Linux server binaries in R1 (or document the exact supported offline signing environment).

##### P9 — Acceptance criteria contain a literal contradiction (AC 1) and leave critical requirements untested
- **Severity:** minor
- **Where:** *Acceptance criteria* (AC 1–8).
- **What is wrong:**
  1. AC 1 states: *"On a fresh, separate host, a Foundation-run validator follows only the docs page and is admitted through R5 and ratification. **Nobody SSHes into that host.**"* An operator following the docs page obviously must SSH into their own host to install, run `tasknode-bind`, and use `postfiat-node top`! The intent is clearly that no Foundation admin intervenes out-of-band or runs internal fleet tooling, but as written it contradicts the entire workflow.
  2. Critical requirements have no acceptance test: (a) `validator-signer-serve` running live in consensus on the devnet; (b) standalone RPC continuous tip-following, latency SLO, TLS edge, and private-action admission; (c) multi-validator decentralized ratification; (d) Finance Hub binding surviving routine hot-key rotation; (e) `top` working as non-root and degrading gracefully when the feed is down; and (f) multi-observer feed quorum and failover.
- **Why it matters:** Acceptance criteria define "done." Gaps here mean builders can check every box while leaving broken RPC follower mode, 9-second reads, untested signer custody, and broken key rotation in production.
- **How an agent should investigate:**
  - Cross-reference R0–R18 one by one against AC 1–8 to verify 1:1 test coverage of every risk-bearing requirement.
- **Suggested fix:**
  - Rewrite AC 1 to say *"Only the public docs page and signed release are used—no Foundation internal fleet scripts, pre-provisioned configs, or out-of-band admin intervention."*
  - Expand/amend AC 3–7 (or add AC 9–10) to explicitly cover: live signer soak test, RPC continuous sync + latency SLO + private-action admission, routine hot-key rotation preserving Task Node access, decentralized multi-validator ratification, and multi-observer feed failover.

##### P10 — Unnecessary hard dependency coupling to Pillar 6 and unclear testnet access policy
- **Severity:** minor
- **Where:** *Dependencies*; *Interfaces to other pillars* (Pillar 6); *Technical requirements* (R0, R13, R17).
- **What is wrong:**
  1. The Dependencies table lists `[Pillar 6 — Hive identity]` as a dependency, and Interfaces states "the connect command also emits the `pf.identity.link.v1` co-signature with `key_type=validator`." However, R0 Steps 1–3 (validator onboarding on the new chain) should not be blocked if Pillar 6 on-chain identity minting ships on a different timeline.
  2. The spec notes the tension between the genesis spec's "no-reward" testnet and Task Node's `$20` credit code, but doesn't clearly state in R0 Step 3 whether testnet validators immediately unlock Finance Hub/GoodAlexander research access even while monetary credit (`TASKNODE_VALIDATOR_REWARDS_ENABLED`) remains off.
- **Why it matters:** Hard-coupling validator onboarding to Pillar 6 creates an avoidable cross-pillar schedule deadlock; leaving testnet Finance Hub access ambiguous risks undermining validator recruitment for Step 3.
- **How an agent should investigate:**
  - Check `06-hive-identity.md` and `README.md` milestone ordering to confirm whether Pillar 6 ships before or after Pillar 3 Step 3.
  - Check `l1v2-testnet-genesis-and-launch-spec.md` for the exact wording of the "no-reward" constraint (whether it refers only to on-chain/monetary rewards vs. off-chain research access).
- **Suggested fix:**
  - Make Pillar 6 co-signature emission in `tasknode-bind` **optional / upgradeable** (emit when Pillar 6 identity is active; allow re-running a link command later) and mark Pillar 6 as a *soft / Step 4* dependency rather than blocking Steps 1–3.
  - Explicitly state in R0 Step 3 and Economics whether Finance Hub + GoodAlexander access is active on public testnet while monetary rewards remain off until production/mainnet.

## Claude Opus 5.5 — 72/100

*Model `anthropic/claude-opus-5.5` · 8914 output tokens*

#### Summary judgement

This is a strong, evidence-grounded spec. It answers almost every literal question in the founder's Deliverable 3:

- **RPC:** yes, RPC is the same binary in RPC mode.
- **Updates:** they come through the same release, via `upgrade`.
- **Running your own RPC:** private and public profiles.
- **One docs page:** R11.
- **Easy Task Node connect:** R13.
- **SSH dashboard:** `postfiat-node top`.

It inventories what exists and states the gates honestly. It also corrects a design error from round 1: joining was never meant to be self-service.

The weaknesses fall in three places:

1. **The time-to-first-outside-validator is long.** It is so heavily gated that it may contradict the founder's "first phase: get some validators on the new chain" framing.
2. **The economics are not yet coherent.** Old-chain bindings already unlock everything, access never lapses, the reward is token-sized, and RPC operators are unpaid. Yet Pillar 5 depends on public RPC.
3. **Several technical choices are risky and not flagged as such.** Validators serve public private-action admission with their finality key. Launch topology caps likely collide with a 6-node Foundation set. Reward and admission thresholds are inconsistent. Latency has no target.

None of these is fatal. Several, though, are the kind that silently stall the rollout.

#### Correspondence to the founder's intention

**Strong matches:**

- **"Do we have RPC software? Where is it? How does it get updated?"** These are answered explicitly and placed in the docs as a plain statement (R10).
- **"A clear way to update my validator software, which needs to exist."** R3 and R3a define `upgrade --check`, rolling vs activation-height releases, signature verification, and rollback.
- **"Go to a docs page and clearly see how all of this is done."** R11 specifies a single page with named sections, its location, and the MkDocs tab caveat.
- **"Extremely easily connect it to the Task Node."** R13 keeps connect to one command, moves the helper into the signed release, removes the hard-coded chain id, and adds aarch64 and offline modes.
- **"Delightful UX I can log into if I SSH in."** R12 `top` covers uptime, signing, lag, peers, upgrade-by height and eligibility.
- **"Validators should be able to access Finance Hub and GoodAlexander content."** This is tied in via R15 and the Pillar 9 interface.
- **"Existing onboarding tool not linked to the new chain."** This is identified precisely (`ValidatorSettings.jsx` `useState("ledger")`), with R14 to flip it.
- **"Multi-phased, first some validators on the new chain."** R0 lays out Steps 1–4.

**Partial or divergent matches:**

- **Speed of phase one.** The founder describes getting validators onto the new chain as the *first* phase of the migration, and implies it is near-term. The page makes the first real outsiders (Step 3) wait on Z1–Z3, C1, C4, G6 and D4.
  - Z3 means NAVCoin swaps must work end to end.
  - C1 means old-chain Dynamic UNL transparency.
  - The founder separately said people can't be *migrated* until the chain is performant. That is about users and the default flip, not necessarily about letting a handful of friendly validators onto a devnet or testnet.
  - The page conflates "outsiders validate" with "public testnet launched". It offers no "community devnet validator" step between Step 2 (a Foundation-run fake outsider) and Step 3 (a full genesis cohort).
- **"So that my validator can be usefully used."** The page reads this as a public directory plus an optional RPC endpoint. The founder may also have meant that the validator does useful work in the Task Node ecosystem, such as serving Task Node reads, private-action admission or information-hive tasks. The page touches this via Pillar 5 but never frames it as "usefulness".
- **"Delightful."** R12 is a functional list. Nothing addresses design quality: layout, colour, alerts, keybindings, or a first-run wizard.
- **Governance framing.** The founder's core thesis is deterministic, post-human governance. The admission policy, Cobalt ratification and Z2 ("does AI model output stay authoritative") are exactly that primitive applied to validator membership. The page never makes the connection. It only sends validator counts to Pillar 1's `chain_state`. An opportunity is missed to show admission verdicts as public, deterministic governance artefacts.

#### Technical robustness

**Good:**

- Fail-closed pinned catch-up.
- A signer double-sign guard that is never backed up or restored.
- Binary and data restored together, citing the postmortem.
- Activation heights for consensus changes.
- An offline signing sandbox.
- Signed feed recomputable from finalized `precommit_qc.votes`.
- Two observers.
- A rehearsal before any real admission (R7).
- A benchmark of ML-DSA-65 certificate size (R8).

**Concerns:**

- **Public endpoints on consensus hosts.** Private-action admission on public validator endpoints, signed with the finality key, mixes a consensus key with a public-facing service.
- **No sentry architecture.** The validator and RPC share a data directory on one host, and a public profile on that host exposes consensus to DoS.
- **Read latency.** It is 7–10 s, with no diagnosis or target.
- **Topology conflict.** Launch needs n ≥ 12 with at most 2 seats per correlated group. The six Foundation validators appear to be one correlated group, so the page may require 10 or more outsiders at launch. This is not reconciled.
- **Backup cost.** Backing up the data directory on each upgrade has disk and downtime costs that are not sized.
- **No quantitative acceptance thresholds** for latency, upgrade downtime or catch-up time.
- **Observer independence.** There is no plan for who runs the second observer.

#### Economic coherence

This is the weakest area. The page honestly lists the tensions but defers all of them to founder questions, without recommending a default. The resulting incentive picture is incoherent in five ways:

1. **No reason to migrate.** Old-chain bindings unlock the same Finance Hub and research access as new-chain ones.
2. **Bind-and-quit is free.** Access never lapses.
3. **The cash reward doesn't cover cost.** $20 of credit is likely below two-core hosting cost plus operator time. Hardware sizing is unknown.
4. **The infrastructure Pillar 5 needs is unpaid.** RPC operators earn nothing, yet Pillar 5 is blocked without two independent public endpoints that serve admission. These are bandwidth-heavy duties.
5. **There is no stake.** Anti-farming rests entirely on admission evidence, but the page does not say how "no shared control groups" is detected rather than self-declared.

Thresholds are also inconsistent: admission requires 99.5% uptime, while rewards need 95% on 25 of 30 days. There is an unresolved conflict between the "no-reward" public testnet in the genesis spec and Task Node's credit code. The page needs a recommended default package, not just a menu.

#### Problems

##### P1
- **Severity:** major
- **Where:** R0 Step 3; "Gates that block outsiders" table (Z3, C1, D4); Status block.
- **What is wrong:** The first real outside validators are gated on NAVCoin swaps (Z3), old-chain Dynamic UNL transparency (C1) and the public-testnet decision (D4). Nothing lets a few trusted community operators validate on the devnet or testnet earlier.
- **Why it matters:** The founder's first phase is "get some validators on the new chain". The founder's performance gates apply to *migrating people*, not to friendly validators. Over-gating could leave the validator program on the old chain indefinitely.
- **Investigate:** Read `testnet-path.md` and find why Z3 is a Gate Zero precondition. Check whether the genesis spec forbids non-genesis devnet validators. Ask whether a "Step 2.5: invited community devnet validators" is consistent with Cobalt (Z1) alone.
- **Fix direction:** Separate "outsiders validate on the devnet or testnet" (gated on Z1, C4 and R7) from "public testnet launch" (D4) and "user migration" (the chain readiness gates). Justify or remove Z3 as a validator gate.

##### P2
- **Severity:** critical
- **Where:** Admission row (`n ≥ 12`, at most 2 seats per correlated group); Current network (six Foundation validators); R0 Step 3.
- **What is wrong:** If the Foundation's six validators count as one correlated group, launch allows only 2 Foundation seats. That implies at least 10 qualifying outside validators at genesis. The page does not reconcile this, and does not say whether the Foundation is exempt.
- **Why it matters:** The cap may make launch impossible, or force the Foundation to give up 4 seats, which would significantly change liveness and control.
- **Investigate:** Read `launch-topology-thresholds.md` and genesis spec §5 for the correlated-group definition and any Foundation carve-out. Count how many old-chain validators would plausibly pass `G0 = Selected ∩ Receipted`.
- **Fix direction:**
  - State the Foundation's treatment explicitly.
  - Estimate the likely G0 size.
  - If it falls short, add an open question or a staged cap relaxation.

##### P3
- **Severity:** major
- **Where:** RPC surface row (private-action admission signs with the finality key); R10 "Private admission"; Pillar 5 interface.
- **What is wrong:** Validators are asked to expose a public profile with `--allow-private-action-admission`, and that path signs with the validator's consensus finality key. That combines key-domain reuse with putting the consensus host on the public internet.
- **Why it matters:**
  - A bug or DoS in the admission path can hit consensus liveness.
  - Signatures made with the finality key widen the attack surface for that key.
  - Pillar 5 privacy depends on this path.
- **Investigate:**
  - Check the admission code for domain separation of what the finality key signs.
  - Check whether admission can use a delegated sub-key registered in the manifest.
  - Check whether the RPC can run on a separate host from the validator (a sentry) while still being authorised to admit.
- **Fix direction:** Use a delegated admission key bound by the master key, and adopt a sentry topology: validator private, RPC/admission node public. Make that the documented default for public profiles.

##### P4
- **Severity:** major
- **Where:** Research access row; Economics, "Access never lapses"; open question 4.
- **What is wrong:** Any unrevoked old-chain *or* new-chain binding unlocks the full validator tier, and nothing ever lapses. That leaves no incentive to move to the new chain and no incentive to keep validating once bound.
- **Why it matters:** The founder's stated premise is that the validator program is the reward for useful validation, and migration is a goal. The current access rule undermines both.
- **Investigate:** Check binding counts and how many bound old-chain validators are still active. Look at the `validator-bindings.js` revocation paths.
- **Fix direction:** Recommend a default rather than only presenting options. For example:
  - access option (b) or (c) with a 14–30 day grace window;
  - old-chain bindings sunset at a published step;
  - optionally, new-chain-only perks (directory listing, RPC rewards).

##### P5
- **Severity:** major
- **Where:** Economics; open questions 1 and 10; R10; Pillar 5 interface.
- **What is wrong:** Public RPC and private-action admission are required for Pillar 5 (two independent endpoints), yet "RPC operators earn nothing". The validator credit is $20 against unknown hosting cost.
- **Why it matters:** Infrastructure that Pillar 5 critically needs has no funding model. There is a risk that only the Foundation serves it, which defeats the "independent endpoints" requirement.
- **Investigate:**
  - Estimate bandwidth and CPU for admission and reads at the expected load.
  - Price two-core hosts.
  - Check the readiness of the x402 metering in Pillar 2.
- **Fix direction:**
  - Commit the Foundation to two endpoints first.
  - Recommend a concrete reward for public endpoints, such as a PFT bonus over FastPay with a cap.
  - Compute the break-even against hosting.

##### P6
- **Severity:** minor-major
- **Where:** Admission floors (99.5% uptime) vs the Rewards row (95% on 25 of 30 days).
- **What is wrong:** Two different uptime standards apply, with no explanation. A validator could earn rewards while failing the admission bar. The page also does not say whether the admission floor keeps applying after admission.
- **Why it matters:** Operators get confusing signals, and it is unclear what removal (open question 7) would be measured against.
- **Investigate:** Read Admission Policy V1 and `l1-validator-uptime.js` for the measurement method. Is it the same metric (precommit votes vs probes)?
- **Fix direction:** Define one participation metric, from finalized votes, with explicit thresholds for admission, reward, warning and removal.

##### P7
- **Severity:** major
- **Where:** RPC row ("Most reads took 7–10 s"); R10 "Read latency"; acceptance 4.
- **What is wrong:** There is no root cause, no target, and the acceptance criterion only says "published". A 7–10 s read is unusable for a wallet, for Task Node, or for the founder's FastPay low-latency goal.
- **Why it matters:** RPC is a deliverable in its own right. Slow reads also block Pillars 2 and 5 and the FastPay payout default.
- **Investigate:** Profile `account` and `tx` on the devnet. Determine whether the cause is storage scans, missing indexes, or the two-read quorum used by the feed.
- **Fix direction:** Set a target, for example p95 < 500 ms for `status`, `account` and `tx`, and add it to the acceptance criteria. Add an indexing workstream.

##### P8
- **Severity:** major
- **Where:** Economics, "Anti-farming"; Admission floors ("no shared control groups").
- **What is wrong:** Anti-farming rests on admission, but the page doesn't explain how shared control or correlation is *detected*. Possible signals include ASN or IP, operator identity, manifests and attestations. Self-declaration is trivially gamed, and there is no stake.
- **Why it matters:** Research access is valuable, which makes it a target, and per-validator credit rewards Sybils.
- **Investigate:** Read the Admission Policy V1 evidence fields and the attestation contents. Check whether Pillar 6 identity or Task Node standing (the `tasknode_unl_v2.py` shadow scorer) feeds into it.
- **Fix direction:**
  - Document the detection signals.
  - Tie the reward to a per-operator identity (Pillar 6).
  - Consider a small bond for open admission (open question 8), with a recommendation.

##### P9
- **Severity:** minor
- **Where:** Small-set failure mode; R0 Steps 3 and 4.
- **What is wrong:** The page notes that early outsiders add liveness risk, but there is no mitigation plan. Options include admitting one at a time with a probation weight, automatic removal, or monitoring the Foundation's quorum share.
- **Why it matters:** At small n, one bad outsider plus one Foundation outage halts finality.
- **Investigate:** Check whether Cobalt supports non-voting or probationary members. Simulate quorum under a 12-validator mix.
- **Fix direction:** Add a probation stage (follower first, then voter), with criteria tied to the R16 feed.

##### P10
- **Severity:** minor
- **Where:** R3, "backs up the data directory".
- **What is wrong:** There is no sizing of backup disk or time, and no policy for pruning backups. A full backup before each upgrade doubles the disk requirement and extends downtime, which counts against uptime.
- **Investigate:** Measure data directory growth on the devnet. Check whether snapshot or copy-on-write (LVM or ZFS) or the R2 checkpoint can replace full copies.
- **Fix direction:** Specify backup strategy and retention, and count the space in the Requirements section. Consider not counting planned upgrade windows against participation when they fall inside the release window.

##### P11
- **Severity:** minor
- **Where:** R16; security table, "at least two independent observers".
- **What is wrong:** No one is named to run the second observer. The feed is currently on one research host owned by the founder's domain.
- **Why it matters:** If the Foundation runs both observers, "independent" is nominal.
- **Investigate:** Check whether observers could be community RPC operators, and whether anyone can recompute the figures from public blocks.
- **Fix direction:** Publish a recompute script and let any RPC operator act as an observer. Task Node accepts the feed when k of n signed observers agree.

##### P12
- **Severity:** minor
- **Where:** R12 `top`; user story step 7.
- **What is wrong:** The founder asked for a "delightful UX". The spec is a field list with no design, interaction or onboarding guidance, and no first-run experience such as "your validator is pending ratification".
- **Investigate:** Look at comparable tools (e.g. Solana `validator monitor`, `htop`-style TUIs, the Cosmos `cosmovisor` and `tenderduty` dashboards).
- **Fix direction:** Add a short UX spec: screens, colour states, alert banner, keybindings for logs and upgrade check, and a join-status panel.

##### P13
- **Severity:** minor
- **Where:** "Useful" story step 9; R18.
- **What is wrong:** "Usefully used" is read narrowly as a directory listing plus an RPC endpoint. The page doesn't consider validators serving Task Node reads or admission, or acting as hosts for information-hive agents.
- **Fix direction:** Add a short "what your validator does for the network" section that links to the Pillar 5 and 10 roles. Ask the founder whether more was intended.

##### P14
- **Severity:** minor
- **Where:** Interfaces to other pillars; overall framing.
- **What is wrong:** The page does not connect admission and ratification to the deterministic, post-human governance narrative (Pillar 1, Z2), which is the project's core story.
- **Fix direction:** Publish admission verdicts and ratification records as governance packets that are visible in Hive Chat and the block explorer.

##### P15
- **Severity:** minor
- **Where:** Acceptance criteria.
- **What is wrong:** Most criteria have no quantitative thresholds: catch-up time, upgrade downtime, latency, the fraction of docs followed without help, or the time it takes `top` to load.
- **Fix direction:** Add a measurable threshold to each criterion.

##### P16
- **Severity:** minor
- **Where:** R11 Location (MkDocs canonical plus a postfiat.org face).
- **What is wrong:** Two copies will drift. The old-chain page sits beside the new one, which risks confusion.
- **Fix direction:** Generate the postfiat.org page from the MkDocs source, or embed it. Add a clear chain selector and deprecation banners.

##### P17
- **Severity:** minor
- **Where:** Economics vs genesis spec ("no-reward" public testnet); open question 12.
- **What is wrong:** There is a direct contradiction between the governing spec and the switched-off code, and no recommendation is given for resolving it.
- **Fix direction:** Propose a default, for example "research access only on the testnet; credit or PFT at mainnet". Update either the genesis spec or the code to match.
