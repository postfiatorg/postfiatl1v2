# First real Base Sepolia Lock released and bridge Phase 2 complete

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-06 UTC

## BLUF

The first real Lock of PFT on Base Sepolia went through the validators' four checkpoint commands and became 1,000,000 atoms of native PFT on a local six-validator fleet. The fleet refused it on resubmission, and the watcher confirmed it ([record, part 2](../listings/coinbase/pft-erc20-release-path-qualification-20261103.md#base-sepolia-rehearsal-2026-11-06-part-2-the-release-on-a-local-fleet)). The fleet was local and the lock's verifier a stand-in, so this is a rehearsal, not live authority. With it, Phase 2 of the [ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md) is complete except the outside audit of the lock contract (P1.4/P2.5). An [audit package](../listings/coinbase/pft-token-audit-package-20261106.md) for quotes now exists.

Also merged today:

- the client's signer double-sign guard (#192) and his local-EVM Lock rehearsal (#191), both with review fixes;
- his documentation refresh (#190);
- two outside contributions (#186, #189);
- the Phase 3 export accounting on PFTL behind a genesis switch;
- the two-user deployment shape of the validator signer;
- the two Phase 2 design-note corrections.

The private next-release pull request #26 and the real-host closeout #25 were checked and answered with comments. The server's pay-per-use inference credit ran out this morning (HTTP 402), so every Task Node task request failed and today's steps carry no Task Node ids.

## Current state

**Base Sepolia rehearsal, part 1** (06:56–07:07Z; [record section](../listings/coinbase/pft-erc20-release-path-qualification-20261103.md#base-sepolia-rehearsal-2026-11-06-part-1-the-deployment-and-the-lock), [`part-1.json`](../review/base-sepolia-lock-rehearsal-20261106/part-1.json)).

- A disposable token `0x90cE303356D80eCBE359FAF061C84130F40EacF0` (`PFT Rehearsal 20261106`, `PFTR`, 100 billion PFT to the holder).
- A new rehearsal lock `0xFb6CF3D7E759cB3130C094C1D7264619ecE74D73`: test mode, the stand-in verifier, minimum 10 atoms, cap 1,000,000 PFT.
- The Lock of 1,000,000 units in `0x599e15fb…5c859` at block 55,132,949.
- Deployment records in [`crates/pft-token-contracts/deployments/`](https://github.com/postfiatorg/postfiatl1v2/tree/65a889432ceb9135536dde0c64601bbad6118a4c/crates/pft-token-contracts/deployments); the old lock `0xE36e…dE0B` is archived.

**Part 2** (07:46–08:20Z; [record section](../listings/coinbase/pft-erc20-release-path-qualification-20261103.md#base-sepolia-rehearsal-2026-11-06-part-2-the-release-on-a-local-fleet), [`part-2.json`](../review/base-sepolia-lock-rehearsal-20261106/part-2.json)).

- Base Sepolia finalized at 55,140,481. The commands ran against the real chain: observe, sign ×5, assemble, lock-proof. Receipts root `0x9cdcc966…ebf4e946`, head 55,153,306.
- The release landed at certified block 2 on all six local validators: 1,000,000 atoms to `pfb2f06f…662eb6`, lane released 1,000,000, release history root `0xba248c76…7caf9b76`.
- Resubmission was refused with `external_lane_nonce_already_released`.
- The watcher found one `backed` release at its source block with its hash and zero alarms. Its repeat run resumed at height 3.
- Merged in `14bf1f8d7`, `76c278f22` and `1d886cb6d`. The milestone's P2.5 real-Lock sub-line is ticked, and P2.5 stays open only on the audit.

**The rehearsal contracts and the holder key.**

- Both rehearsal contracts were verified today on Sourcify (exact match) and Blockscout (`6cfee928a`).
- Follow-ups: the [contracts README](https://github.com/postfiatorg/postfiatl1v2/blob/65a889432ceb9135536dde0c64601bbad6118a4c/crates/pft-token-contracts/README.md)'s lock deploy order corrected (`93d30711a`), the audit package's verification table refreshed (`6463b9aa4`), and the release-plan row added (`65a889432`).
- The holder key `0x6E4E…11E17` stays active as the funded rehearsal holder for the coming Base Sepolia rehearsals, with 0.0019859 test ETH. When those rehearsals end, the key is retired and its leftover test ETH goes back to the funding address.

**The audit package** for the token and lock contracts ([`pft-token-audit-package-20261106.md`](../listings/coinbase/pft-token-audit-package-20261106.md)). It covers:

- the scope at commit `e5a508e7`;
- what the contracts do;
- the invariants, threats and system assumptions from the design notes;
- evidence: `forge test` 199 passed, 100 % coverage of both contracts, Slither 76 results with the 1 High and 9 Medium all in OpenZeppelin's `Math`;
- the deploy order, questions for the auditor and how to run everything.

Merged in `c2d8f08e5`, `a98607527` and `786d551d8`. One unticked sub-line sits under P1.4.

**Phase 3, the PFTL half, behind `exports_enabled`** (default off; every existing genesis unchanged; [lanes page, "Exports"](../architecture/genesis-external-lanes.md#exports)).

- What is built:
    - the export counters in the state commitment and the net-form remainder;
    - `apply_external_lane_export`, with checked arithmetic and the governed zero-cap halt;
    - the export packet with its digest and commitments, and pruning;
    - the paged read in RPC, the CLI and Python;
    - the conservation test.
- The public library entry `execute_asset_transaction` now refuses lane exports and verified releases unless governed caps and committees are supplied. No node path is affected, because every node path passes both.
- Merged in `214af76f2`, `f751185c7` and `05fe9c37a`. The P3.1 sub-line is ticked under its unticked parent, and the [release plan](../plans/active/next-validator-release-plan-20261115.md) says it needs its own qualification.
- The [Phase 2 design note](../specs/pft-erc20-base-bridge-phase2-design-20261026.md#corrections-2026-11-06) records its two corrections, the deployment order and the conditional release bound, in a dated "Corrections" section (`76716865e`).

**W-1 S7.1, the validator signer** ([W-1 milestone](../plans/active/user-proven-shielded-actions-milestone.md)).

- The client's durable double-sign guard (#192) merged with eight review fixes (`fcfdf595e`, `f86d501bb`, `5a69b481a`). Among them:
    - a floor seeded at initialization for keys that already signed (`--initialize-state --min-height`);
    - the guard bound to one chain domain;
    - the deployment checker refusing capabilities and a key copy under the data directory;
    - honest test counts.
- The two-user deployment shape (`7651635f6`, `dae568ce2`, `8daa7d4de`):
    - `deployment-validator-units-stage --signer-shape two-user` writes the signer, rpc and transport units, with the key only in the signer's directory;
    - `transport-validator-serve --finality-signer-socket`;
    - a process test that starts all three from the units' command lines, with readiness reporting `validator_signer_same_user` and no key copy;
    - the operator page's ["Two-user units"](../runbooks/public-rpc-operator-policy.md#two-user-units) and the [deploy sheet](../runbooks/validator-signer-two-user-deploy-sheet.md): stop both old units, search every readable key copy, move the key, seed the floor.
- This is a runtime change that needs its own qualification. S7.1 stays open until a two-user deployment on the devnet reports `ready`, which is a release-day step.

**Other public merges.**

- #191, the client's local-EVM Lock rehearsal script, merged with review fixes (`3c84580b7`, `d96dfe294`, `22db9507a`):
    - the evidence trimmed to records and hashes;
    - raising predicates and a watcher-summary check;
    - Foundry outputs and paths confined;
    - provenance including compiler and tool versions;
    - a bounded port retry.

    The code-hash difference between runs is Solidity metadata only.
- #190 merged with two stale claims corrected: the guard is merged, and the real Lock happened (`0d6b7a8d5`, `e5a508e73`).
- #186, the release-check table, completed with three rows (`620af6da6`, `7334b82ad`).
- #189, the Python client's release-limit bound (`3198674a8`).
- The Python client's export-limit bound (`6cfee928a`).

**Private side.** Comments went on the next-release pull request <https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/pull/26>:

- The rollout stays on the frozen candidate (docs `9a8628dc1`, runtime `c68b3b68f`). #26 is the next release, with its own qualification.
- One authorization rule changed on purpose, for the two lane amendment kinds only. It requires the registry threshold with the full active set declared, byte-identical to public main.
- Hosted CI is red on `de9277d48` in three jobs.
- The executable hash does not reproduce, because the binary embeds a library search path with a random temporary folder. The code sections are byte-identical.

Our fixes for the two deterministic red jobs are on our private branch `security/next-release-ci-fixes-20261106` (`1502dcd22`, two commits on `de9277d48`), with a [comment on #26](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/pull/26#issuecomment-6684430927):

- The signer process test now asserts the remote timeout-vote refusal `rpc_method_not_allowed` and signs locally through the signer.
- The Python CLI defines `--lane-id` once. 1,160 Python tests pass; the remaining two fail only on this host's long temp path.
- The third job (wallet-and-proxy) looks timing-related and was left alone.
- Nothing merged, and the client's branch is untouched.

A comment also went on the real-host closeout <https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/pull/25>. The live data-directory hashes change because rpc-serve opens and closes the redb store for every uncached status read. Nothing touches the ledger, votes or keys, and tip 1168 and the state root are identical on all six.

**The exchange guidance page.** Its 10 November update is re-pinned to today's main on the private repository postfiatorg/exchange-guidance-review, branch `update-20261110` (`28047d9`). Nothing is published. The score run, the generator patch and the publish are Monday's steps.

**Devnet and deployment.**

- The devnet was not touched: no deployment, signing or transaction.
- Read-only probes of the six hosts were made today. All six run executable `87cfe797…` at tip 1168 with state root `39771b74…`. The only idle write is redb's own header on each status read.
- That matches [Current State](../status/chain-state-current.md), last updated 2026-10-30T11:21Z: release `vote-records-20261030` on all six, build `ae70d66d`.
- Merged on main but undeployed: everything above, carried by the [next validator release plan](../plans/active/next-validator-release-plan-20261115.md) and the private next release.
- Repository: `origin/main` at `65a889432` before this handoff.

**Task Node.** No task today. Every request failed at generation with `inference_http_402`, the account's pay-per-use credit; the rehearsal records name the failed requests. The steps that would have carried a task are the ones above.

**Public CI at 12:14Z.**

- All three workflows are green on every push through `05fe9c37a`.
- On the later pushes (`8daa7d4de`, `786d551d8`, `76716865e`, and `65a889432`, which carried `93d30711a`, `6cfee928a` and `6463b9aa4`), docs-build is green. rust-ci and product-security-ci were still running at the time of writing.

## Next decision or action

Ours, first:

1. The Phase 3 deployment-order dry run on Base Sepolia, once the Base verifier route profile and asset id are decided. Predict both addresses, pin them in a genesis, and deploy the verifier and then the lock after the first finalized checkpoint ([Phase 3 note, section 5](../specs/pft-erc20-base-bridge-phase3-export-design-20261105.md#5-proof-on-base)).
2. Audit quotes from the package.
3. The exchange page's publish on Monday 10 November: the score, the generator patch, and a fresh commit on site-source.
4. The two-user signer deployment on the devnet with the next release, after the release's own qualification. The floor is seeded from the vote-lock index.
5. The rollout day under the runbook, once the keyed rehearsal on the client's hosts is done.
6. Read CI on today's last merges.

The client's, each with the date it was first asked:

- Pause minting on the Ethereum controller `0x9A02…` (2026-11-02). It was still on at 06:50Z today, the fifth working day.
- The rollout go, the keyed rehearsal on his hosts, the six migration signatures and the migration-trust descriptor (2026-11-02/03/04).
- The audit vendor and budget for the lock and token contracts, now with the package for quotes (2026-10-26 / 2026-11-06).
- The server's pay-per-use inference credit (2026-11-06). Task Node task creation fails with HTTP 402 until it is topped up.
- The wrapped-token path, A or B (2026-11-04).
- The Phase 3 decisions in the [export design note, section 9](../specs/pft-erc20-base-bridge-phase3-export-design-20261105.md#9-decisions), now also the verifier's route profile and asset id for the dry run (2026-11-05).
- The three red CI jobs on his next-release branch (2026-11-06; see the comment on #26 and our fix branch `security/next-release-ci-fixes-20261106`).
- The reachable host of the audit and operations machine (2026-11-04).
- The six cached checkpoint votes (2026-11-04).
- The old proving service and the faucet key (2026-11-02).
- The two issuer signatures (2026-09-25 / 2026-10-01).
- The reserve-proof successor registration (2026-09-24).
- The `ETHEREUM_MAINNET_RPC_URL` secret (2026-09-28).

## References

- [ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md), [the release path qualification record](../listings/coinbase/pft-erc20-release-path-qualification-20261103.md) and [the rehearsal evidence](../review/base-sepolia-lock-rehearsal-20261106/part-2.json).
- [Audit package](../listings/coinbase/pft-token-audit-package-20261106.md) and [The PFT token on Base](../architecture/pft-token-on-base.md).
- [Genesis external lanes](../architecture/genesis-external-lanes.md), the [Phase 2 design note](../specs/pft-erc20-base-bridge-phase2-design-20261026.md) and the [Phase 3 export design note](../specs/pft-erc20-base-bridge-phase3-export-design-20261105.md).
- [W-1 milestone](../plans/active/user-proven-shielded-actions-milestone.md), the [public RPC operator policy](../runbooks/public-rpc-operator-policy.md) and the [two-user deploy sheet](../runbooks/validator-signer-two-user-deploy-sheet.md).
- [Next validator release plan](../plans/active/next-validator-release-plan-20261115.md).
- The client's latest handoffs, both on private branches: `2026-11-05___codex__security_release_gate_engineering.md` (branch `security/consolidated-20261103`) and `2026-11-05___codex__priority_engineering_and_remaining_production_gates.md` (branch `security/priority-engineering-20261105`).
- Previous handoff: [Base checkpoint source merged and Phase 2 of the bridge complete](2026-11-05___dravlic__base_checkpoint_source_merged_and_phase_2_of_the_bridge_complete.md).
- [Current State](../status/chain-state-current.md), unchanged.

## End of session (13:20 UTC)

- The exchange page's 10 November draft (private review repository, branch `update-20261110`) scores 83.87 with the Text Improvement Harness against the 28 October baseline of 87.40. The baseline round is in no harness database on this host, so on Monday the currently published page is re-scored first with the same registration, and only then is the drop rule applied.
- Three readers approve the draft's text for Monday's publish, with seven wording fixes recorded in the review note beside the draft. Among them:
    - the "not yet verified" sentence goes, because the rehearsal lock's source is verified on Sourcify and Blockscout;
    - no "audit engaged": the audit is open, and a package for quotes exists;
    - the bridge items are "built", not "live", because the result is from a local six-validator test fleet;
    - future gates are attributed to the milestone;
    - the page carries no "next update" date.

    Nothing is published.
- The [next validator release plan](../plans/active/next-validator-release-plan-20261115.md) is complete after today's merges (`f05c2f7ea`, `1f400c4ca`):
    - the summary is corrected, and the missing rows for #186, #189 and #190 are added;
    - the signer process (`b610061fc`), the double-sign guard (`fcfdf595e`, `f86d501bb`) and the export operation skeleton (`7a968007e`) now need their own qualification, beside the two-user shape and the export accounting. Each changes either the process that holds the key or what two binaries do with a certified block;
    - the "Not before" row names every such merge.
- Correction to "W-1 S7.1" above: #192 is merged with nine review-fix commits, not eight.
- CI at 13:10 UTC is green on every merge point through `786d551d8`. The runs on `76716865e`, `65a889432`, `af592e98d`, `f05c2f7ea` and `1f400c4ca` are still in progress.
- The two helper sessions are closed and the main session is idle. Nothing on the devnet is touched.
