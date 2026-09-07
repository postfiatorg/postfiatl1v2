# PR #38 merged; Arc proposal deck published; UNL-from-Task-Node proposal published

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-07 UTC
- **Status:** draft handoff, being assembled; sections marked *to add* are not yet written
- **Supersedes:** [2026-09-04 identity-only profiles replay](2026-09-04___dravlic__identity_only_profiles_replay.md) for the identity thread; the Arc thread continues [2026-09-02](2026-09-02___dravlic__identity_replay_paused_for_arc_grant.md)

## BLUF

Three threads moved. (1) PR #38, the Nitro-proven YOLO portfolio target receipts, is
merged to `main` at `1412b4dc` after a one-line clippy fix, and the repo docs now surface
it (`d8ff6f13`). It is merged and **not deployed**; the feature defaults to disabled and no
testnet activation has occurred. (2) The pfUSDC-on-Arc grant material is live at
https://postfiat.org/research/pfusdc-on-arc-round-trip/ as a nine-slide deck on top of the
technical packet, with the named co-applicant and every grant dollar amount removed. (3)
The validator-identity work concluded that an institution-prestige rubric is the wrong
identity definition for Post Fiat; the replacement proposal, deriving the UNL from Task
Node identity and ratifying through Cobalt, is published at
https://postfiat.org/research/deterministic-unl-task-node-cobalt/ (TIH 88.8). No fleet
probe was performed this session; nothing below is a claim about running validators.

## Current state

### PR #38: YOLO target receipts (merged, undeployed)

- Merge commit `1412b4dc` on `main`, 2026-09-07. Head of the PR branch before merge
  `59598baf`, which adds only `fix(yolo): satisfy clippy nonminimal_bool` in
  `crates/execution/src/yolo_target_verifier.rs` (the activation check now uses
  `Option::is_none_or`). Verified locally: `cargo clippy --locked -p postfiat-execution
  --all-targets -- -D warnings` clean; `cargo test --locked -p postfiat-execution yolo_`
  6/6.
- What it adds: a governance-gated `yolo_target_register_v1` / `yolo_target_submit_v1`
  pair. Registration pins SP1 program hash and verification key, methodology and parameter
  commitments, collection manifest, expected prior state, replay identity, permitted
  submitter and a future activation height. Submission verifies the real Groth16 proof
  against the registered expectations and records the target with transaction finality.
  Proof bytes bounded to 4,096; exactly 408 public bytes; duplicates, substitutions, early
  submissions and malformed proofs reject. Receipts confer no trading, minting or reserve
  authority. Empty YOLO state preserves legacy state roots.
- Qualification recorded in the PR: real Nitro Groth16 proofs for the MU and NVDA liquid
  baskets, each passing independent verification, six tampering checks, exact 408-byte
  replay, five certified rounds with four votes each on an isolated local four-validator
  network, and Python CLI/HTML. Evidence under `docs/yolo/evidence/`.
- CI at merge: `build`, `evm-contracts`, `rust-supply-chain` pass. `check`, `test`,
  `python-sdk`, `wallet-and-proxy`, `open-reserve-proof-kit`, `public-tree-hygiene` fail on
  the PR **and on `main` before the merge** (main run `33877718544` / `33877718592`, head
  `f2f0988`). The only PR-introduced failure was the clippy lint, now fixed. The
  pre-existing failures are: A666 `regression-manifest.json` missing under
  `docs/evidence/a666-public-reserve-product-20260803/regressions/`; hygiene scan flags a
  global-IP runtime default in `scripts/pfusdc-eth-mainnet-epoch6-audit.py:185`; python-sdk
  primary-market row assertions; wallet static build. None were touched.
- Docs: `d8ff6f13` on `main` adds *YOLO Options Reserve Profile* and *YOLO Target Receipt
  v1* to the mkdocs nav, a "YOLO target receipts" row to the front-door table in
  `docs/index.md`, and a pointer from `docs/navcoins/index.md`. Strict mkdocs build passes.
- Release and rollback, per the PR: deploy a compatible validator release before
  scheduling governance activation; register reviewed program/run identities with a future
  activation height; retain the previous release as rollback candidate until receipt state
  exists; after that, do not assume a pre-feature binary can replay new state. Canary
  checks are matching state roots, receipt finality/replay, and rejection of early,
  malformed, substituted and duplicate submissions.

### Arc grant material (published; PR #37 still open)

- Live page: https://postfiat.org/research/pfusdc-on-arc-round-trip/ (site commit
  `076f697`). Deck slides: hero; the problem (committee bridges, nothing verifiable to
  hold, quantum risk); USDC on Arc status quo vs. what is missing; committee bridge / CCTP /
  pfUSDC comparison; the market (tokenized stocks ~$2.3B mid-July 2026, NAVCoins as the
  index layer); the round-trip loop; what's live now (four Arc testnet addresses, source,
  evidence, PR #37, claim classification, audit scope); CBDC expansion (pfUSDC↔pNOK atomic
  swap, 1 Aug 2026); upside for Arc. The written packet follows the deck.
- Removed on request: every Zellic reference and every grant, tranche, budget and
  reserve-pilot dollar amount, from the site page, `docs/business/pfusdc-on-arc-round-trip-20260902.html`,
  and the companion docs. Repo docs renamed on this branch: `zellic-review-packet-20260902.md`
  → `arc-grant-claim-classification-20260902.md`, `zellic-audit-scope-20260902.md` →
  `arc-audit-scope-20260902.md` (commits `27978009`, `80b6dfc8`, `b089a4b2`). The proposal
  v3 itself (`pfusdc-arc-grant-proposal-20260828-v3.md`) still states the ask and tranche
  amounts; it was not edited.
- Deck assets: `static/research/pfusdc-arc-deck/` on the site repo (five generated
  backgrounds, `deck.css`, `deck.js`). Backgrounds were generated through OpenRouter
  (Gemini 3 Pro image); there is no OpenAI API key on this host.
- PR #37 (`integrate/arc-tier4-current-v2-20260901` → `main`) remains open with
  `mergeStateStatus: UNKNOWN`. It still carries the gateway-owner finding (§04 of the
  packet) and the historical-proof gap described in the 09-02 handoff.
- Community post text (title "The Trustless USDC Bridge to Arc", benefits list) was
  drafted in-session and handed to the operator; it is not committed anywhere.

### Validator identity and UNL (research concluded; proposal published)

- Identity-only replay (`benchmarks/ai-governance/institution-reputation-packets-20260904`,
  commit `f9b1e5c5`): 192/192 byte-identical across two distinct-owner H200 hosts,
  aggregate `9d6935e2…`; 36 of 55 validators score zero; all 20 Post Fiat validators zero.
  Deterministic correlation finds one cluster (the three Foundation validators).
- Cross-model matrix (`outputs/cross-model.json`, commit `59f60713`): pinned Qwen vs
  GLM-5.3-flash 0.86, GPT-5.6-luna 0.93, Kimi-K3 0.91, Claude-fable-5 0.92,
  DeepSeek-v4-pro 0.85 Pearson.
- Operator conclusion: the prestige rubric measures what grant money buys (Ripple UBRI
  universities score high, community operators score zero) and "institutional validators"
  is not a coherent goal for Post Fiat. Identity in Post Fiat is Task Node identity: a
  wallet with proven control plus an AI-verified work history replayed on PFTL.
- Proposal published: https://postfiat.org/research/deterministic-unl-task-node-cobalt/
  (site `2e2f25e`). It grounds in Admission Policy V1, the evidence field registry,
  `DynamicUnlValidatorBindingV1` and the safe-churn limits, and proposes: validator-key ↔
  Task Node wallet binding; `accountability_score` from replayed task events with a stated
  35/25/20/10/10 formula over 180 days; `rho_score` from funding-graph and profile
  correlation; Admission Policy V1 unchanged in shape; a Task Node social graph
  (EigenTrust/SybilRank walk, seeds = ratified non-Foundation list, 20 steps, damping 0.85,
  conductance 0.1, two-seat cluster cap) for one-person-one-seat, with World ID as optional
  only; one change per round below 39 validators to hold 95 % overlap; SHADOW_ONLY until an
  independent ratifier signs. TIH 86.2 → 88.8 over three passes; remaining judge asks are
  clustering pseudocode, weight sensitivity and adversarial simulation.
- All Vast rentals destroyed; `vastai show instances` is empty as of this handoff.

## Next decision or action

1. **Deploy decision for PR #38.** Merged code is not on any validator. Decide whether the
   next validator release carries it (feature stays disabled until governance schedules
   `yolo_target_activation_height`), and who owns fixing the pre-existing CI failures on
   `main` so the merge queue is green again.
2. **PR #37.** Either close the gateway-owner finding and the historical-proof gap on the
   integrate branch and merge, or keep it open as the review artifact and say so on the PR.
3. **UNL from Task Node, Phase 0.** If the proposal stands: publish the scoring constants
   in `postfiat-consensus-cobalt`, implement the binding CLI and Task Node signed work
   digest, add `validator.identity.tasknode_binding.*` to the evidence field registry, and
   shadow-derive against the round-20 list.
4. **Association/collusion packet pipeline** (Corbanu per-entity association packets →
   deterministic join → pairwise research → pinned-model R0–R4) is designed, not built.

## To add before this handoff is final

- Archive-instance status for the Arc historical proof (the 09-02 handoff left it
  provisioned but not running a node; not verified this session).
- Current fleet observation with capture time, per the handoff standard.
- Whether the proposal v3 document should also drop its dollar amounts to match the deck.

## References

- PR #38: https://github.com/postfiatorg/postfiatl1v2/pull/38 · merge `1412b4dc` · docs `d8ff6f13`
- `docs/yolo/target-receipt-v1.md`, `docs/navcoins/yolo-options-reserve-profile.md`, `docs/yolo/evidence/`
- PR #37: https://github.com/postfiatorg/postfiatl1v2/pull/37
- Arc packet: `docs/business/pfusdc-on-arc-round-trip-20260902.html`, `docs/business/arc-grant-claim-classification-20260902.md`, `docs/business/arc-audit-scope-20260902.md`, `docs/business/pfusdc-arc-grant-proposal-20260828-v3.md`
- Site: https://github.com/postfiatorg/postfiatorg.github.io — `layouts/page/pfusdc_arc_round_trip.html`, `static/research/pfusdc-arc-deck/`, `content/research/deterministic-unl-task-node-cobalt.md`
- Identity: `docs/governance/institution-reputation-packets-h200-results-20260904.md`, `benchmarks/ai-governance/validator-identity-packets-20260904/`, `benchmarks/ai-governance/institution-reputation-packets-20260904/`
- Governance grounding: `docs/governance/validator-registry.md`, `docs/governance/validator-evidence-field-registry.md`, `docs/governance/dynamic-unl-l1-evidence-source-note.md`, `docs/governance/cobalt-independent-operator-proposal-path-research-spec.md`
