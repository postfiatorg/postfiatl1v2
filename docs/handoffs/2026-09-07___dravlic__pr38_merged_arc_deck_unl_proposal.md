# PR #38 merged; Arc proposal deck published; UNL proposal published; NAVCoin/Cobalt handoff branch (#39) reconciled

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-07 UTC
- **Status:** draft handoff, being assembled; sections marked *to add* are not yet written
- **Supersedes:** [2026-09-04 identity-only profiles replay](2026-09-04___dravlic__identity_only_profiles_replay.md) for the identity thread; the Arc thread continues [2026-09-02](2026-09-02___dravlic__identity_replay_paused_for_arc_grant.md)

## BLUF

Four threads moved. (1) PR #38, the Nitro-proven YOLO portfolio target receipts, is
merged to `main` at `1412b4dc` after a one-line clippy fix, the repo docs surface it
(`d8ff6f13`), and its public explainer is live on postfiat.org (being retitled "A Framework
for Trustless Single Stock Option Indices"). It is merged and **not deployed**; the feature defaults to disabled and no
testnet activation has occurred. (2) The pfUSDC-on-Arc grant material is live at
https://postfiat.org/research/pfusdc-on-arc-round-trip/ as a nine-slide deck on top of the
technical packet, with the named co-applicant and every grant dollar amount removed. (3)
The validator-identity work concluded that an institution-prestige rubric is the wrong
identity definition for Post Fiat; the replacement proposal, deriving the UNL from Task
Node identity and ratifying through Cobalt, is published at
https://postfiat.org/research/deterministic-unl-task-node-cobalt/ (TIH 88.8), and a
SHADOW_ONLY MVP of it is already on `main` (see below). (4) The other machine's
consolidation of the NAVCoin repairs, Cobalt/Task Node sources and storage fixes arrived
as draft PRs L1 #39 and StakeHub #8; #39 has been merged up to current `main` (one
mechanical conflict resolved, focused tests green) and now stacks cleanly. No fleet probe
was performed this session; the only fleet facts below are quoted from the other machine's
handoff with its capture time.

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
- **Public explainer published** (site PR #13, merged `1598b63`, 2026-09-07):
  https://postfiat.org/research/single-stock-options-trackers/ , ten responsive diagrams
  plus an interactive verification walkthrough (`layouts/partials/options-tee/`,
  `data/options_tee_diagrams.json`, `static/research/options-tee-indices/`). It is being
  retitled to **"A Framework for Trustless Single Stock Option Indices"** at
  `/blog/trustless-single-stock-option-indices/` with the research URL kept as an alias;
  that move was in progress on the other machine at handoff time and had not been pushed.
  What the article says, in one paragraph: a single-stock options tracker is a rulebook
  that maintains a rolling basket of calls on one company (the demo: five November 20
  calls each on Micron and Nvidia, 90 % premium budget in five equal sleeves, maturity
  closest to 60 days beyond a 30-day roll threshold, $100,000 hypothetical cash, zero
  holdings); the collector runs inside an AWS Nitro enclave that terminates the Schwab TLS
  session, Nitro attests the measured collector, an SP1 guest verifies the attestation and
  recomputes the target, and PFTL records the Groth16-verified result as a receipt bound
  to 408 public bytes. The "trustless" claim is scoped precisely: a verifier need not trust
  the operator's assertion that it applied the registered calculation; rulebook review,
  program/collector identity selection, data-source correctness, Nitro and SP1 assumptions,
  and any future custody/execution layer remain outside the proof. It states plainly that
  no trades were executed, no funded tracker exists, and the receipts were accepted only on
  an isolated local four-validator network.
- TIH on the article, first pass, five ratings per model: GPT-5.6-Sol 90.2, Claude Fable 5
  84.0, GLM 5.2 88.2, overall 87.47. Several judges scored the text export as missing
  figures; a second pass with the diagram format explained was running at handoff. The
  substantive criticism was "needs one concrete worked example, fewer repeated caveats."
- Relationship to the rest of this document: this is the NAVCoin "index layer" made
  concrete for one asset class. The Arc deck's market slide ("NAVCoins index tokenized
  stocks") and this primitive are the same product direction; the receipt is the
  verifiable strategy engine that a tokenized tracker, model portfolio or managed account
  would consume. It is not yet wired to any reserve, NAV or subscription/redemption path.

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
- **MVP already built on `main`** (09-04 session, [handoff](2026-09-04___dravlic__tasknode_unl_mvp_built_and_hardened.md),
  [plan](../plans/active/tasknode-unl-mvp-plan.md)): `python/postfiat_rpc/tasknode_unl*.py`
  implements the schema, accountability formula, exact-rational trust walk, offline binding
  CLI (prepare/finalize/verify/replay only, no submit), signed work-digest verification,
  vouch/co-work/funding edge extraction, the churn/overlap guard, and a shadow-derive CLI
  with golden fixtures. A frozen read-only testnet-ledger shadow round (49 transactions,
  three wallets, 48 `pf.ptr/v4` memos) produced an empty candidate set with named holds
  because no binding or digest emissions exist yet. A fresh-context adversarial review
  fixed 14 fail-closed defects. 103 tests + 34 subtests pass. Nothing has live authority.
  The published proposal remains authoritative for constants and rules; the MVP's Phase 0
  gap is the Task Node side (binding memos, signed digests, vouch memos, exclusion list).
- All Vast rentals destroyed; `vastai show instances` is empty as of this handoff.

### NAVCoin / Cobalt / storage consolidation from the other machine (draft PRs #39, StakeHub #8)

- Source of truth: [machine handoff](2026-09-07___codex__navcoin_cobalt_machine_handoff.md)
  on branch `handoff/navcoin-cobalt-local-20260907`, and
  [validation](../status/NAVCOIN-HANDOFF-VALIDATION-20260907.md). Companion StakeHub branch
  `handoff/navcoin-local-20260907` ([PR #8](https://github.com/postfiatorg/StakeHub/pull/8),
  targets `master`). Keep the two repos as siblings named `postfiatl1v2` and `StakeHub`;
  the StakeHub native inspector has a relative Cargo dependency on L1.
- What #39 carries: pfETH Ethereum ingress program, `WETHBridgeVaultL1.sol` and package
  scripts; exact historical pfETH reserve-replay repair; the transactional RPC-status cache
  fix (deployed 09-06); signed A666 source-series custody (`pftl_source_settlement.rs`,
  deployed 09-07); public regression fixtures; deployment evidence under
  `deployments/a666-source-route-20260907/`; the paused shielded-funding specs. It also
  includes the Task Node UNL MVP and Cobalt sources through its `main` merge.
- **Stacking, as of this handoff.** #39 targets `integrate/arc-tier4-current-v2-20260901`
  (i.e. it is stacked on PR #37) and originally merged `main` at `f2f09881`, which predates
  #38. I merged current `main` (`d8ff6f13`, includes #38) into it at `55330121`. One
  conflict, `crates/node/src/block_replay_wallet.rs`: the ledger destructure needed both the
  handoff's bound `nav_attestors` and #38's `yolo_target_registrations` /
  `yolo_target_receipts`. Resolved by keeping all three. `cargo build --locked -p
  postfiat-node` passes; `cargo test -p postfiat-node --lib -- pfeth_reserve
  source_settlement_commitment yolo_` 7 passed, 1 ignored (the supplied-proof test, as
  documented in #38). #39 now contains `main` and the integrate branch up to `b089a4b2`;
  it lacks only the integrate handoff commits (`817d93f7` and this one), which touch
  `docs/handoffs/` only. Merge order stays: **#37 → main, then retarget #39 to main**, or
  merge #39 into the integrate branch first if #37 is going to stay open as the review
  artifact.
- **Deployed fleet facts quoted from the other machine (not re-probed here):** last
  reconciled observation 2026-09-07, block 1005, six validators agreed, mempools empty,
  state root `6ed69ca9…65f9`; release `a666-source-route-20260907`, binary SHA-256
  `57b0f4d1…ec83`; deployed base preserved as branch `handoff/navcoin-deployed-base-20260907`
  at `707e006f` plus `source.patch` and `untracked-source/`. The consolidated branch is
  **not** the deployed source tree; do not label its build with the deployed hash.
- **Where the full NAVCoin round trip stopped** (Ethereum → pfUSDC → A666 → Uniswap → A666
  → pfUSDC → Ethereum): source custody shipped; the epoch-6 Ethereum verifier checkpoint 909
  predates the Cobalt committee rotations at 917/924 and no compatible proof was produced;
  an epoch-7 verifier/vault pair was deployed and then **rejected at block 1004** because
  route epochs are global per asset and Arc had already taken pfUSDC epoch 9 (the Arc
  binding was restored at 1005; the epoch-7 contracts must not be funded); the 881→917 A666
  witness executed (544.8M instructions, ~14.7 s) but the CPU prover died without a proof
  and no prover is running. No new USDC was deposited; protected balances at recovery were
  534.079891 Ethereum USDC and 103 wA666.
- Open P2 findings carried forward (09-06 review, `docs/review/storage-cobalt-tasknode-handoff-review-20260906.md`):
  Task Node shadow admission does not require a complete wallet/account mapping; candidate
  key identity is not joined to the authenticated binding key; the consolidated historical
  Cobalt packet verifier (`benchmarks/cobalt-adversarial-verification/packet/verify_packet.py`)
  still fails because it binds mutable publication documents, which is documented, not
  fixed; some storage-plan prose overstates concurrent-reader support.
- Credentials, wallet material, local `.postfiat` state and the private StakeHub recovery
  archive (`docs/handoffs/navcoin-recovery-20260907/`, with the frozen A666/pfUSDC guest
  ELFs and witness) are not in Git. A second host needs its own RPC/SSH access, endpoints,
  proof artifacts and unlocked StakeHub signer, and must re-query all six validators before
  signing anything.

## Next decision or action

1. **Deploy decision for PR #38.** Merged code is not on any validator. Decide whether the
   next validator release carries it (feature stays disabled until governance schedules
   `yolo_target_activation_height`), and who owns fixing the pre-existing CI failures on
   `main` so the merge queue is green again.
2. **PR #37, and therefore #39.** #39 cannot reach `main` until #37 does. Either close the
   gateway-owner finding and the historical-proof gap on the integrate branch and merge
   #37, then retarget and merge #39; or merge #39 into the integrate branch now and keep
   #37 as the single review artifact. StakeHub #8 has no such dependency and can be
   reviewed on its own.
3. **NAVCoin round trip.** Two blockers before any new deposit: a proof path compatible
   with the Cobalt committee rotations at 917/924 for the Ethereum epoch-6 verifier, and a
   next route epoch derived from governance (not 7). Then a full deployment-package
   preflight, then wire the traversal through StakeHub `wallet nav-roundtrip` and retain
   every receipt.
4. **UNL from Task Node, Phase 0.** The L1 side exists on `main`. What is missing is on the
   Task Node side: binding memos, signed work digests, vouch memos, the funding exclusion
   list. Then rerun the shadow derive against a live ledger view and diff against round 20.
5. **Association/collusion packet pipeline** (Corbanu per-entity association packets →
   deterministic join → pairwise research → pinned-model R0–R4) is designed, not built.

## To add before this handoff is final

- Archive-instance status for the Arc historical proof (the 09-02 handoff left it
  provisioned but not running a node; not verified this session).
- Current fleet observation with capture time, per the handoff standard.
- Whether the proposal v3 document should also drop its dollar amounts to match the deck.

## References

- PR #38: https://github.com/postfiatorg/postfiatl1v2/pull/38 · merge `1412b4dc` · docs `d8ff6f13`
- Options tracker explainer: https://postfiat.org/research/single-stock-options-trackers/ (moving to `/blog/trustless-single-stock-option-indices/`); site PR https://github.com/postfiatorg/postfiatorg.github.io/pull/13; demo record `static/research/options-tee-indices/demo-record.json`; article plan `docs/research/options-tee-trackers-article-plan.md` (site repo)
- PR #39: https://github.com/postfiatorg/postfiatl1v2/pull/39 · head `55330121` · StakeHub #8: https://github.com/postfiatorg/StakeHub/pull/8
- Machine handoff: `docs/handoffs/2026-09-07___codex__navcoin_cobalt_machine_handoff.md`; validation `docs/status/NAVCOIN-HANDOFF-VALIDATION-20260907.md`; deployment evidence `deployments/a666-source-route-20260907/`
- Task Node UNL MVP: `docs/handoffs/2026-09-04___dravlic__tasknode_unl_mvp_built_and_hardened.md`, `docs/plans/active/tasknode-unl-mvp-plan.md`, `python/postfiat_rpc/tasknode_unl*.py`, `docs/governance/tasknode-unl-shadow-run-20260904.md`
- `docs/yolo/target-receipt-v1.md`, `docs/navcoins/yolo-options-reserve-profile.md`, `docs/yolo/evidence/`
- PR #37: https://github.com/postfiatorg/postfiatl1v2/pull/37
- Arc packet: `docs/business/pfusdc-on-arc-round-trip-20260902.html`, `docs/business/arc-grant-claim-classification-20260902.md`, `docs/business/arc-audit-scope-20260902.md`, `docs/business/pfusdc-arc-grant-proposal-20260828-v3.md`
- Site: https://github.com/postfiatorg/postfiatorg.github.io — `layouts/page/pfusdc_arc_round_trip.html`, `static/research/pfusdc-arc-deck/`, `content/research/deterministic-unl-task-node-cobalt.md`
- Identity: `docs/governance/institution-reputation-packets-h200-results-20260904.md`, `benchmarks/ai-governance/validator-identity-packets-20260904/`, `benchmarks/ai-governance/institution-reputation-packets-20260904/`
- Governance grounding: `docs/governance/validator-registry.md`, `docs/governance/validator-evidence-field-registry.md`, `docs/governance/dynamic-unl-l1-evidence-source-note.md`, `docs/governance/cobalt-independent-operator-proposal-path-research-spec.md`
