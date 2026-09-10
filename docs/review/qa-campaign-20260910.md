# QA campaign log — 2026-09-10

This is the canonical progress record for the [2026-09-10 QA campaign](qa-campaign-20260910-brief.md). The campaign began at 2026-09-10T09:31:59Z from `main` commit `3ec59c0bea6383445d1465d9a006317a33973003`. It is a review-and-repair campaign, not a release or deployment authorization.

**Status:** Completed 2026-09-10. All ordered surfaces were reviewed, every
in-scope P1/P2 repair was pushed, and the final inventory passed its first
compliant full Text Improvement Harness gate at 89.00/100. Open and skipped
items remain explicit below.

## Current state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fix commits |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Arc-facing code | done | 2 | 0 | 1 | [Findings](arc-facing-review-20260910.md) `1c67ec07`; repair `dbc73fea` |
| D | Bounded whitepaper corrections | done | — | — | — | Gate `9124b6de`; candidate `6fbcee8e…`; 85.47; not promoted |
| B1 | Initial defect inventory | done | — | — | — | `634e3625`; [inventory](defect-inventory-20260910.md), 46 initial rows |
| C | Validator-0 RPC diagnosis | done | 1 | 0 | 0 | `314952a9`; [chain-state diagnosis](../status/chain-state-current.md#validator-0-rpc-diagnosis-20260910) |
| A2 | Consensus and storage | done | 1 | 1 | 0 | Findings `cfa96bb0`; repair `f9f13ead` |
| A3 | Wallet, proxy, and RPC SDK | done | 2 | 3 | 0 | Findings `7967fd10`; repair `83488d91` |
| A4 | StakeHub `fix/pr8-safety-20260907` | done | 1 | 2 | 0 | Read-only review `07d0b3b4` |
| A5 | Task Node UNL V1 and V2 modules | done | 1 | 2 | 1 | Findings `8fc9b02c`; repair `1c10f828` |
| B2 | Final defect inventory and TIH gate | done | — | — | — | 61 rows; first full gate 89.00/100 |

Three Arc-facing findings are recorded. Both P1 source defects are repaired and
their focused gates pass; the deployed immutable V2 controller was not changed,
so route migration remains an open operational requirement. The P3 fork-schedule
inconsistency is recorded without fixture or golden regeneration. The bounded
whitepaper candidate scored 85.47 and was not promoted.

## Completed units

- Arc-facing review completed at `b8560de9`: two P1 findings and one P3 finding.
- Arc-facing repair completed: V2 cancellation and refund commitments now share
  the consume fence, and the live round-trip entrypoint requires two explicit
  execution acknowledgements. Verification: contracts 9/9 focused and 148/148
  local non-fork, Rust 38/38 + 195/195 + 5/5, interlock 2/2, strict Clippy pass.
- Bounded whitepaper correction completed. Candidate
  `/home/postfiatchad/pastedocs/.qa-campaign-whitepaper-20260910/candidate.md`
  changes only the abstract's question count and the source-backed consensus
  signing status. SHA-256: `6fbcee8ebeed6eb255247fecd23836fd4bb564d4ea560d9d5c561b3da14aef8f`.
  Full TIH run group `qa-bounded-whitepaper-6fbcee8e-20260910` used five reviews
  each from `openai/gpt-6-astra-pro`, `anthropic/claude-fable-5.1`, and
  `z-ai/glm-5.3`, temperature 0, 8,000 maximum response tokens, and the prompt
  `Rate this document on a scale of 1-100. Output the score and your reasoning.`
  GPT averaged 87.20 (87–88), Fable 85.60 (84–87), and GLM 83.60 (81–88), for
  85.47 across 15 fresh reviews. Because 85.47 does not strictly exceed 87.13,
  neither the published paper nor its Markdown download was changed; both
  remain SHA-256 `28f004a6a0e08a87561e62d14ace6a87659718c4845feba21eb468b3fe314202`.
- Initial defect inventory completed with 46 rows from the campaign and the
  named 2026-09-06, StakeHub, proof-input, and signing-qualification sources:
  20 reproduced defects, 21 evidence gaps, one economic assumption, and four
  proposed capabilities. The final campaign pass appended later findings,
  reconciled statuses, recalculated counts, and ran the required TIH gate.
- Validator-0 diagnosis completed read-only. The affected RPC exhausted its
  configured 10,000 accepted connections while a keep-alive connection
  remained active; the serve loop stopped accepting and waited for the active
  connection, so systemd stayed `active` while all three health requests timed
  out. Repeated wallet bridge-readiness traffic makes validator-0's connection
  rate materially different from its peers. A separate session restarted the
  service at `2026-09-09T20:19:37Z`; this campaign made no repair. The current
  process answered at height 1020 with root `587c6526…d39bead6` and an empty
  mempool, but it retains the same recurrence condition.
- Consensus and storage review completed against validator-runtime changes
  since 2026-09-05. It found one P1 race between durable Consensus v2
  authorization and signature emission, plus one P2 unbounded, uncharged YOLO
  consensus-state growth path. Pre-repair focused baselines remain green at
  20 Consensus v2 library, 3 Consensus v2 binary, and 6 YOLO execution tests;
  the missing adversarial cases accompany the repairs. Authorization and
  signature construction now share one per-height critical section. YOLO
  registrations and receipts each have an exact 4,096-row cap and a 10-PFT
  state-expansion fee. Post-repair gates pass at 20 Consensus v2 library, 3
  Consensus v2 binary, 7 YOLO execution, and 4 YOLO node tests, with the one
  documented external-proof opt-in case ignored; strict focused Clippy passes.
- Wallet, proxy, and RPC SDK review completed. It found two P1 failures: an
  RPC-selected transfer recipient/amount could replace reviewed intent before
  signing, and the loopback local-session endpoint accepted a DNS-rebinding
  Host. It also found three P2 failures: WebSocket mutations released the
  shared concurrency slot before work began, the maintained extension permitted
  four-character passphrases under its older 100,000-iteration vault format,
  and an unmatched brace made the extension popup invalid as a browser module.
  Pre-repair baselines passed at wallet proxy 35/35, web wallet 259/259, and
  Rust RPC SDK/WASM 69/69. The repair binds signing to reviewed intent, binds
  session issuance to request authority, holds shared admission through work,
  strengthens new extension vaults, and repairs plus correctly gates the popup
  module. Post-repair results: proxy 36/36, web wallet 260/260, extension 2/2,
  Python wallet/latency 79/79, Rust RPC SDK/WASM 69/69, node integration compile
  pass, and strict focused Clippy pass.
- StakeHub `fix/pr8-safety-20260907` review completed without changing,
  committing, fetching, or pushing its pre-existing dirty checkout. The 105
  focused local safety tests pass and leave its status unchanged. The repair set
  closes or explicitly dispositions the original nine PR findings, but this
  fresh pass found one P1 release-reuse hazard and two P2 non-causal batch
  success checks. All three remain open for the StakeHub lane; this campaign
  made no fix there.
- Task Node UNL V1/V2 review completed. The V1 paths retain their binding,
  evidence, graph, and hold controls. The V2 review found one P1 fresh-window
  bypass, two P2 input/report integrity failures, and one shadow-only P3 helper
  binding gap. Findings 1–3 proceeded to a separate repair; the P3 remains
  recorded under the campaign rule. The 167-test plus 43-subtest pre-repair
  selection passed.
- Task Node UNL repair completed. Fresh continuity now requires an incoming
  renewed vouch and post-epoch co-work; historical score rows cannot suppress
  fresh credit by ordering; and the human renderer makes untrusted fields
  structurally inert. The evidence golden's signed input and snapshot remain
  unchanged while its corrected expected result records two holds. The V1,
  V2, and combined selections pass at 107 plus 36 subtests, 64 plus 10
  subtests, and 171 plus 46 subtests, respectively. The shadow-helper P3 stays
  open and all frozen experiment outputs remain untouched.
- Final defect inventory completed with 61 unique rows: 35 reproduced defects,
  21 evidence gaps, one economic assumption, and four proposed capabilities.
  Statuses are 26 fixed, 16 dispositioned, and 19 open. The open set contains
  eight P1s, eight P2s, and three P3s; its exact priorities and production
  caveats are in the [inventory](defect-inventory-20260910.md).

## Final Text Improvement Harness gate

The exact final inventory bytes, SHA-256
`06eef17d7f0acf933e3b2bf104955c5baf33f27d1e2b8d0f39c4008a13940046`,
received fifteen fresh OpenRouter reviews at temperature 0 and an 8,000-token
response limit. The prompt was
`Rate this document on a scale of 1-100. Output the score and your reasoning.`

| Judge | Scores | Average |
| --- | --- | ---: |
| `openai/gpt-6-astra-pro` | 91, 90, 91, 91, 91 | 90.80 |
| `anthropic/claude-fable-5.1` | 82, 88, 89, 88, 84 | 86.20 |
| `z-ai/glm-5.3` | 90, 92, 88, 90, 90 | 90.00 |
| **All fifteen** | — | **89.00** |

Run group: `qa-defect-inventory-20260910`. The first compliant full score
exceeded the 86/100 gate, so no rewrite or rescore was performed. The external
SQLite record is
`/home/postfiatchad/pastedocs/.qa-campaign-defect-inventory-20260910/scores.sqlite3`,
SHA-256 `dc58dbab5b3d703064ec7f29a4a44d217bfb27deff23006f94f5df68ba0f7abf`.

## Skips and boundary decisions

- No Task Node action occurred.
- No fleet mutation, deployment, restart, configuration change, host write, or live-chain write occurred.
- The validator-0 diagnosis used only documented read-only RPC, service status,
  logs, sockets, process metadata, and host telemetry. The prior process had
  already been restarted outside this campaign, and no repair was attempted.
- Frozen simulations, V2 gate outputs, deployment evidence,
  `docs/whitepaper_legacy.md`, the locked amendment, and lock records were not
  modified.
- `docs/whitepaper.md` remained unchanged because the bounded candidate did
  not strictly exceed the recorded 87.13 score.
- The dedicated mainnet ingress Fulu-epoch inconsistency is P3, so this campaign
  records but does not alter its guest source, ELF, program key, or frozen
  deployment evidence.
- StakeHub review was read-only. Its branch, uncommitted repair set, archive,
  working files, remotes, and original checkout were not changed. Focused tests
  disabled bytecode and pytest cache writes and produced identical pre/post
  porcelain status.

## Verification

Every campaign commit passed the strict documentation build. Each findings
document records its pre- and post-repair focused gates. The final boundary
audit found no campaign diff in deployment evidence, either frozen simulation,
the locked amendment or lock records, `docs/whitepaper.md`,
`docs/whitepaper_legacy.md`, or a live-fleet configuration. No Task Node action,
deployment, restart, chain write, rental, signup, or StakeHub write occurred.
