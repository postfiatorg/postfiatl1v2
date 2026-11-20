# Next release replays genesis and Solana reviews

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-20 UTC

## BLUF

We continued the client's Solana handoff of 20 November.

- **Epoch change.** The daily Solana epoch-change step was reviewed and fixed in private PR #38 and Task Node PR #421.
- **Proofs.** The overnight proof changes were confirmed sound.
- **Phantom deposit card.** It now has an exact transaction check, built in three reviewed rounds (Task Node PR #422, stacked on #421; not merged).
- **Next chain version.** It joins the main line with release 6.4, the vote-phase fix, the stuck-height fix (heights 1701/1707, four reviewed rounds) and the three 1153 governance rules gated on new kinds. It replays the live chain from genesis to 1712 with the root of all six validators. It passed a disposable six-validator rehearsal and an offline rehearsal of the rollout itself: one validator at a time from 6.4, a mixed fleet, the way back, and a copy of live data.
- **Passwords.** The six validators' root passwords were changed; nothing was restarted.

Nothing else live was touched.

Primary references: [Current State](../status/chain-state-current.md), [the handoff of 19 November](2026-11-19___dravlic__solana_build_reviewed_freeze_fix_rehearsed_and_pilot_fixes.md).

## Current state

**Root passwords.** They were rotated on the six validator hosts at the client's request and access was checked afterwards. Nothing was restarted and the chain was unaffected. Where the new ones are kept, and the follow-ups, were shared privately, not here.

**Epoch change** (private PR #38, `sol-review/epoch-change-fixes-20261120`, head `75047a651`; Task Node PR #421, `fix/solana-send-readiness`, head `16c540d`)

- Deposits close about 12 minutes before the change, so a deposit still waiting in Phantom cannot land in the gap.
- The new validator set starts exactly at the change, so a deposit in between is credited.
- The re-anchor stops after two identical failures, with exit code 3.
- `/solana/send` refuses while deposits are closed (`solana_deposits_closed`).
- The other points are in the PR. Install both before the next change, about 14:45 UTC on 21 November.

Today's change, at about 12:52 UTC, ran the existing code:

- it crossed at about 12:53 UTC;
- about two minutes later the existing re-anchor moved the route to Solana epoch 1092 at block 1713 (route revision `706d8b86df45`, anchor slot 471744139);
- all six validators agree at 1713;
- deposits in that gap were not checked from our side (the relay host was not read).

**Last night's proof changes.** The new split and the later-signed-block path keep every check: no deposit is double counted, omitted or changed. Agave 4.4.1 records the replayed parent, and the FastLeaderHandover gate is not active on mainnet. The later-block path has no tests of its own yet.

**Phantom deposit check** (Task Node PR #422, `fix/solana-deposit-shape`, head `9255271`, stacked on #421)

- One exact-shape check runs in the browser before Phantom opens, and again on `/solana/send`.
- A server-set compute budget, a wallet-set budget within caps and Lighthouse assertion instructions are accepted only on the one-signer path.
- The vault program and mint are pinned in Fly's own settings.
- Expiry is by nonce value. Leases are released on decline and on errors.
- Re-send loops are bounded and stop when deposits close.
- An unfinished durable deposit is watched for up to 24 hours while a new one can start.
- Three independent reads. The last one approved with changes after round 4; the fifth round fixed both points, with failing tests first.
- Deploy Fly first, with `L1_SOLANA_VAULT_PROGRAM_ID` set on Fly (without it the card shows "awaiting configuration"), in a quiet window, because leases live in the relay process's memory.

**Deposit speed study.** Of about 100 s, the proof takes about 57, the chain about 15 and the relay about 28. Estimated totals:

- relay changes alone: about 77 s;
- dropping the Groth16 wrap: about 55 s. This needs a chain release, because of the 1 MiB proof batch limit in `mempool_dag` and the 128 KiB RPC limit;
- eight GPUs: about 50 s.

**Next chain version** (private `integration/main-with-b24-r64-20261120`, head `674dbb18a`, binary SHA-256 `4589dc8ff103cc18f17c5951f111e697a06642eece924f98af412475b5abf298`)

- **Contents:**
    - the main line, B24, release 6.1 and release 6.4, merged;
    - the vote-phase fix;
    - the stuck-height fix, rounds 1–4, also on `consensus/round-quorum-fix-r64-20261120` (head `0320fb653`). A proposer that already signed at view 0 reports "view spent" under its own RPC code. The next leader then finishes the height at view 1 with a timeout certificate that carries the prepare QC. The wallet proxy decides on codes;
    - the finality status-read cut;
    - three new governance kinds: `rejected_tx_fee_v2_activation_height`, `recipient_format_v2_activation_height` and `require_memo_v2_activation_height` (unset = off). The 1153 kinds stay stored but no longer gate execution, because releases since r2-1 never applied them and the live chain must replay as executed.
- **Replay.** A fresh copy of validator-3 replays from genesis: 1712 blocks, root equal to all six live roots, checkpoint verified.
- **Rehearsal** (disposable six-validator network): a transfer, a deposit, a stopped validator (five certify and the stopped one catches up), the 1453 pattern, and a stuck height recovered at view 1 in about a second.
- **Independent reads.** Rounds 1–4 approved; the join approved with the small changes now in.
- **Rollout rehearsal** (offline, fresh keys):
    - 6.4 to the new version one validator at a time, with six equal tips and roots after each step;
    - at three and three: transfers, refusals, stalls in both directions and a stuck height recovered at view 1;
    - rollback to 6.4 on current data;
    - the new binary on a copy of validator-3's live data wrote nothing 6.4 cannot read;
    - a long-running `rpc-serve` does not wait for late votes.
- **Rollback checks** use `status` and `verify-finalized-checkpoint`, because 6.4's `verify-state` cannot read live data (the 1153 amendment kind). `rpc-serve` ignores unknown flags, so check the unit text.
- The final head passed the same rehearsals.

**Relay speed, first slice** (private PR #39, `sol-review/relay-speed-20261120`, stacked on #38, head `a01309815`)

- No second public RPC lookup per deposit, and a rate-limit backoff instead of a crash.
- The host's native check runs during the proof and still gates the submit.
- The next deposit no longer waits for the previous one's six receipts; the checkpoint still advances only over settled rows.
- Estimated 1.5–4 s saved per single deposit (not measured), and up to 90 s for a deposit queued behind another.
- An independent read found two medium cases (a failed receipt wait did not stop the next submit; a restart could re-run a row already filed as missed) and two small ones. All four are fixed, with failing tests first: `tools/pfusdc-sol-relay` 13/13, `ops/sol-live` `test_reanchor_mainnet` 42/42.
- Later slices are in the PR text: the wallet API telling the relay about a signature at once, and the witness sent from the node straight to the prover.

**Ethereum successor** (started under the Ethereum-side goal). Private `navcoins/successor-node-receipts-20261119`, head `4aee5cf62`, adds a route setting (unset = today's contract, serialization unchanged). It selects the successor's decoders and builds the `consumeExport` calldata in the export preflight. Open: the RPC passing receipt files, consensus-side selection and the `PFTLNavFreshness` gate.

**Our older PRs.** Private #34 is merged into the instant pilot branch (both pilots stay off). #36 and #37 are closed with comments, overtaken by the launch and release 6.4. #33 is updated with what is still open.

**Fleet and deployment.**

- The fleet runs release 6.4. The next chain version is not deployed, and nothing from this session is merged into public `main`.
- Last fleet read: all six agree at block 1713 after today's epoch change.
- [Current State](../status/chain-state-current.md) still records release 6.1 at height 1513 (2026-11-19 05:56 UTC).

**Devnet.** Not touched apart from the root password change: reads and fresh read-only data copies only.

**Task Node.** Tasks were requested, accepted and submitted for the substantial steps, among them:

- `task_c8c85896db7ef2d43cbc6565bee9d260`
- `task_ae07dadaa76f95da329ce9b5a72755cc`
- `task_a00579a35de347b17af70127f78d1870`
- `task_ee5b74b004627816737a5fec4fa15f31`
- `task_3222e9fd45e12f802b7e0839d6bd162f`
- `task_06a1f20fe9c389e02ad5e8f6a2fb31f7`
- `task_a141b11170c17a37fe392c3e4a67a7ff`
- `task_932ea44f0ce87bfaa995adc29d0b6430`
- `task_132c548f1ba10957d2ad632fa66e3ca1`

Small steps had no task.

## Next decision or action

Ours:

1. [ ] Roll out the next chain version on a day without an epoch-change risk:
    - sign a manifest for the binary first;
    - the wallet proxy first, then the validators one at a time, with six equal tips and roots between hosts;
    - keep the rollback set ready (the 6.4 binary, unit and manifest).

    Ratify the new v2 kinds only after all six run the new version, and only if the client wants the rules back.
2. [ ] Install PR #38 and Task Node #421 (and #422 once merged) before the epoch change of about 14:45 UTC on 21 November. Install PR #39 after #38, on a quiet relay.
3. [ ] Successor node work: the RPC receipt files, consensus-side selection and the NAV freshness gate.

Only the client can do these (date first asked):

- the follow-ups of the password rotation, shared privately (2026-11-20);
- set `L1_SOLANA_VAULT_PROGRAM_ID` on Fly before #422 is deployed (2026-11-20);
- decide whether to switch the three 1153 rules back on by a governance vote after the rollout (2026-11-20);
- his own open decisions from his handoff: the Solana node's permanent home, SOL for the recorder and the StakeHub whitelisting, and dropping the Groth16 wrap with a validator release (2026-11-20);
- choose the audit firm and budget for the PFT token contracts (2026-10-26);
- give his reading of the 30- and 60-day goals (2026-11-10).

## References

- **Private archive:**
    - the client's handoff `docs/handoffs/2026-11-20___claude__solana_bridge_handoff_for_domagoj.md` (on `sol/integration`);
    - private PRs #38 and #39 and the branches named above.
- **Task Node:** PRs #421 and #422.
- **Status:** [Current State](../status/chain-state-current.md).
- **Previous handoff:** [2026-11-19: Solana build reviewed, freeze fix rehearsed and pilot fixes](2026-11-19___dravlic__solana_build_reviewed_freeze_fix_rehearsed_and_pilot_fixes.md).
