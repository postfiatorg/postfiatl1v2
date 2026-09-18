# Arc test wallet and Z3 offline gates

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-17 UTC

The [September 18 handoff](2026-09-18___dravlic__combined_release_merged_repaired_and_qualified.md)
supersedes the candidate/P1 status and Z3 lineage, pair, cap, and latency decisions
below; the dated observations remain historical.

## BLUF

The [other lane's handoff](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/handoffs/2026-09-16___codex__combined_release_check_kickoff_and_private_funding.md)
named funded USDC on this server and end-to-end NAVCoin swaps as priorities;
both were started today. The server wallet holds **20.000000 Arc testnet USDC**.
The [Z3 plan](../plans/active/z3-navcoin-roundtrip-plan.md) advanced through
G2 route compatibility (4/5 boxes), G3 tooling (6/6), and the offline G5
failure/recovery rehearsal (4/5). The [read-only preflight](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/docs/status/z3-preflight-20260917.md)
records fleet, pair, and wallet inputs for the first cycle. Pair/lineage selection,
G1/G4 authorization, and confirmation of proposed latency bounds remain open.
Z3 work submitted no transaction or signature and made no fleet or candidate change.

## Current state

- **Repository and release:** `main` at `81a5a38c` before this handoff;
  `git pull --rebase origin main` was already up to date. Candidate
  `release/combined-devnet-20260915` remains untouched at `15126ac3`.
  The [September 16 review's two P1s](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/docs/review/pr41-release-candidate-review-20260916.md)
  remain open. September 16 main-only fixes remain outside PR #41 and
  undeployed; today's Z3 tooling is also undeployed.

- **Wallet:** Public address `0xC75Bf05Ce82d6f4b6139dd9446D6De5F5994a4CB`;
  Arc testnet chain `5042002`, RPC `https://rpc.testnet.arc.network`, USDC
  system contract `0x3600000000000000000000000000000000000000` (six decimals;
  USDC is also the gas token). The key was generated on this server with
  `eth_account`; the encrypted keystore is compatible with Foundry and
  `eth_account`, with a verified decrypt round-trip. Keystore
  `~/.foundry/keystores/arc-testnet-server` and password file
  `~/.postfiat/arc-testnet-server.password` are both mode `600`, outside
  the repository; the password is also in the operators' shared password
  manager. No key material is in repository files. Neither secret file was
  opened for this handoff.
  The operator funded it through `faucet.circle.com`: transaction
  `0x96ef5060b64f2811b531a0b3cb0f8cedab59caee7cd76fd0a49ecd9c8ed95c75`,
  status `1`, block `62549207`. At `2026-09-17T09:58Z`, the balance was
  **20.000000 USDC / 20,000,000 six-decimal atoms**; native and ERC-20 views
  agreed. The later preflight agrees and records nonce `0`. Faucet rate
  limits remain unmeasured.

- **G2 — route compatibility:** [Review](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/docs/review/z3-g2-route-compatibility-20260917.md)
  records G2.1/3/4/5 PASS and 13 focused tests passed. Candidate `15126ac3`
  contains Arc ingress/egress/source code. Existing governance/source
  selectors support Arc source-labelled pfUSDC in
  `pftl_uniswap_primary_subscribe_v2` and `pftl_uniswap_primary_redeem`;
  the same-cycle reserve is counted once and the settlement asset cannot
  be substituted. No new transaction kind, consensus code, generic NRRS
  facility, bridge contract, or settlement-price format is required.
  Both pairs' full identities were read back; **G2.2 remains open** until
  the operator selects the pair and qualified lineage.

- **G3 — driver:** The [reserve driver](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/scripts/a666-pfusdc-reserve-demo.py)
  requires explicit route, asset, settlement family, source series/bucket/profile,
  chain, and account identities; production defaults are removed. It emits
  the signed `settlement_source_asset_id`, accepts current NAV builder schemas,
  rejects unknown schemas, caps redemption by both same-cycle reserve and
  selected-source principal, and preserves fail-on-overwrite.

- **G3 — cycle tooling:** [z3_cycle.py](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/python/postfiat_rpc/z3_cycle.py)
  supplies a redaction-safe, fail-on-overwrite manifest covering the plan's
  ten packet sections. Its offline verifier checks artifact hashes,
  accepted receipts/finality, identity/proof bindings, exact deltas, reserve
  counted once, zero entitlement/pending state, replay rejection, and
  six-validator convergence. The wrapper composes 39 commands around the
  driver, stopping before every Arc/PFTL submission: each explicit confirmation
  executes at most one step; attempt markers prevent automatic retries.
  Dry-run prints commands and writes an unverifiable skeleton. Execution
  fails closed when Arc tooling is absent: **main has no Arc commands; use
  the qualified lineage**. The [dry-run sheet](../runbooks/z3-cycle-dry-run.md)
  supplies ordered commands, operator placeholders, read-backs, and hard stops.
  Recorded tests: cycle/composition 32 passed; full Python suite 567 passed,
  3 skipped, 103 subtests; driver 25 passed. The operator reran the Z3 and
  driver tests. All six G3 boxes are ticked.

- **Read-only preflight:** [Fact sheet](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/docs/status/z3-preflight-20260917.md),
  linked from G4, records fleet observations at
  `2026-09-17T11:16:49.908219Z–11:32:07.540468Z`: six validators agreed at
  height `1020`, with the recorded tip/state root and empty mempools;
  validator-5 answered on retry. All 12 validator/RPC processes ran release
  `a666-source-route-20260907`, binary `57b0f4d1…634eec83`, reporting build
  revision `707e006f`; this deployed identity is separate from main and the
  candidate. See [Current State](../status/chain-state-current.md) for the
  deployment baseline. Arc reads at block `62561207` found both the epoch-7
  and current-v2 (route epoch 9) pairs on chain, each vault holding exactly
  `1.000000 USDC`. Full addresses and capture times are in the fact sheet.
  That preflight ticked no box beyond G3. These were earlier read-only live
  probes; no fresh live probe was performed while writing this handoff.

- **G5 — offline rehearsal:** [Review](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/docs/review/z3-g5-failure-rehearsal-20260917.md)
  records nine synthetic failures: stale NAV/proof, wrong route/asset,
  duplicate deposit/subscription nonce, active entitlement, duplicate burn,
  and duplicate Arc release. Each stopped before builder/submission dispatch,
  produced an unclean manifest verifying FAIL, and asserted preservation of
  its scenario's state. Resuming preserves exact request identity and returns
  the original terminal result or rejects replay without resubmitting.
  A pre-submission environmental interruption preserves the consecutive count;
  a post-submission interruption requires pause/reset. **Boxes 1, 2, 3, 5
  are ticked; box 4 remains open.** Latency bounds are **PROPOSED**, with no
  stage timings in the September 2 evidence: 300 s for preflight, subscription,
  redemption, and final convergence; 180 s for deposit and entitlement release;
  7,200 s for ingress proof/claim, NAV/route epoch, and burn/egress. Exceeding
  a bound means pause. Focused runs passed 77, 88, and 101 tests, each with
  67 subtests; the operator reran the Z3 test files. No consensus change
  was needed. This is offline failure/recovery evidence, not a live cycle
  or live authority.

- **Operational boundary:** No devnet mutation, deployment, restart, or
  Task Node action. The operator's faucet funding is the recorded wallet
  transaction; no integrated Z3 cycle has run. This handoff changes only
  documentation/navigation and does not rerun the recorded operational tests.
  Documentation checks passed: `mkdocs build --strict`,
  `scripts/public-doc-links`, and `scripts/public-secret-scan`.

## Next decision or action

1. Supply the [G1/G4 inputs](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/docs/status/z3-preflight-20260917.md#operator-inputs-still-missing-for-g1g4):
   explicit go naming qualified lineage (`15126ac3` or another), epoch-7 or
   current-v2 pair with full addresses, A666 primary-route confirmation,
   per-cycle cap in USDC atoms, exact seven-day UTC window, testnet-only
   confirmation, and signer custody (server keystore above or operator's own,
   plus required PFTL signers). The wallet's `20,000,000` atoms are a balance,
   not an authorized cap; leave room for USDC gas. Confirm or amend the
   proposed G5 latency bounds. Use the dry-run sheet's hard stops on a
   qualified-lineage checkout for one authorized G4 cycle. G5 tooling supports
   repetition after a clean cycle; sustained repetition still needs G6 authorization.
2. PR #41: resolve the two open P1 review items and decide which main-only
   fixes to merge; these decisions are unchanged.
3. Campaign line remains as in the [previous handoff](2026-09-16___dravlic__flaky_tests_p3_sweep_candidate_review_and_burn_five.md):
   burn-five P3s, SMG-07 after the merge, then a sixth burn after the candidate lands.

## References

- [Z3 plan](../plans/active/z3-navcoin-roundtrip-plan.md),
  [Arc MVP specification](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/docs/specs/pfusdc-arc-mvp-testnet-spec-20260828.md),
  [dry-run command sheet](../runbooks/z3-cycle-dry-run.md).
- Tests: [reserve driver](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/scripts/test-a666-pfusdc-reserve-demo.py),
  [cycle verifier](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/python/tests/test_z3_cycle.py),
  [composition](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/python/tests/test_z3_composition.py),
  [failure/recovery rehearsal](https://github.com/postfiatorg/postfiatl1v2/blob/81a5a38c/python/tests/test_z3_failure_rehearsal.py).
- G2: [a88deda9](https://github.com/postfiatorg/postfiatl1v2/commit/a88deda9).
  G3: [90fbc5da](https://github.com/postfiatorg/postfiatl1v2/commit/90fbc5da),
  [bc4ede7e](https://github.com/postfiatorg/postfiatl1v2/commit/bc4ede7e),
  [f680d78a](https://github.com/postfiatorg/postfiatl1v2/commit/f680d78a),
  [f23f77b9](https://github.com/postfiatorg/postfiatl1v2/commit/f23f77b9),
  [95304ce1](https://github.com/postfiatorg/postfiatl1v2/commit/95304ce1).
  Preflight: [b8e54ff4](https://github.com/postfiatorg/postfiatl1v2/commit/b8e54ff4).
  G5: [b150b14e](https://github.com/postfiatorg/postfiatl1v2/commit/b150b14e),
  [9269278b](https://github.com/postfiatorg/postfiatl1v2/commit/9269278b),
  [81a5a38c](https://github.com/postfiatorg/postfiatl1v2/commit/81a5a38c).
