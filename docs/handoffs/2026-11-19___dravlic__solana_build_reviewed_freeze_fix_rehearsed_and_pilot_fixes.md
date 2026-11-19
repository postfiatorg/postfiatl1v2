# Solana build reviewed, freeze fix rehearsed and pilot fixes

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-19 UTC

## BLUF

Three independent reviews read the client's overnight trustless Solana route. Four follow-ups are open as private pull requests:

- #33: the review document;
- #35: the vault-account fix;
- #36: the launch order on real Solana;
- #37: deposits from recent past epochs, with the cheap checks run before the proof.

The fix for the 15-minute stall at height 1453 went through four rounds, each read independently. It is merged into a release candidate. That candidate replays the live chain from genesis to 1577 and reaches the same state root as all six validators, and it passed a disposable six-validator rehearsal.

The instant pilot fixes are private PR #34, approved by an independent read.

Since then the fleet moved to release 6.2 and the route was connected to real Solana. The candidate must now be rebuilt on 6.2.

Primary references: [Current State](../status/chain-state-current.md), [the handoff of 18 November](2026-11-18___dravlic__instant_usdc_in_and_out_built_explorer_checked_and_speed_fix_reviewed.md).

## Current state

**Solana reviews.** These were read-only reviews; the reports are on the private archive.

- **The deposit proof.** The reviews found no way to credit an unfinalized, duplicate, wrong-amount or wrong-recipient deposit. 16 of the design's 18 refusal cases are implemented and tested. The other 2 belong to the vault.
- **The release 6 chain code.** It is deterministic and cannot halt the chain.
- **The vault and recorder programs.** Once the upgrade authority is removed, funds leave only through a proven withdrawal.
- **Before real money.** The points to settle and their suggested changes are in PR #33 (`review/solana-independent-read-20261119` → `sol/integration`).

**PR #35** (→ `sol/integration`, head `5cfd49a9c`)

- Zero-data accounts owned by the vault are now skipped instead of the whole block being refused. Tests pass.
- It changes the guest program, so three things must follow:
    - rebuild the deposit and aggregate ELFs, and their verifying keys, with the pinned SP1 Docker image;
    - update the route's verifier key once, by governance.
- The PR also carries 34 `sol/live` commits with worker scratch files (`PROMPT.md`, `CONTINUE.md`, `WORKER-STATUS.md`, `last.txt`). Those should not be merged with it.

**PR #36** (→ `sol/live`, head `b0f296938`)

- It sets the launch order on real Solana:
    1. preflight;
    2. route governance;
    3. deploy;
    4. verifier id check;
    5. withdrawals live;
    6. recorder caught up;
    7. only then open deposits;
    8. a 1-atom deposit and a 1-atom withdrawal;
    9. freeze last.
- Each refusal is tested in `ops/sol-live/test_mainnet_go.py`.
- The launch that ran at about 12:40 UTC used `mainnet-go.py` from `sol/live`, which still has the earlier order.

**PR #37** (→ `sol/release6-fix`, head `9a1b42e2b`)

- The route keeps up to three earlier validator sets. A deposit therefore stays creditable for at least about six days after its epoch.
- The cheap checks run before the Groth16 proof.
- All of it sits behind a new governed activation height. While that height is unset, release 6 behaviour applies and finalized blocks replay identically.
- Execution Solana tests: 15 passed, 0 failed, 6 of them new.

**Stall fix** (height 1453, 15 min 31 s). The fix is on private branches `consensus/round-quorum-fix-20261119` and `consensus/round-quorum-fix-r6fix-20261119`.

- The precommit phase ends at quorum.
- Each vote phase waits at most the value of a new `rpc-serve` flag, `--finality-vote-phase-deadline-ms`. The default is 20000, and the value must be between 1 and `--finality-timeout-ms`.
- Refusals are retried until the phase deadline.
- Precommits are requested only from validators whose prepare vote was accepted.
- Only verified replies count.
- A certified shielded batch reports success.
- Vote thresholds are unchanged.
- **Correction to the stall write-up:** the 15 minutes come from the round's `--finality-timeout-ms 30000` with 16 retries. They do not come from `transport-validator-serve --timeout-ms 900000`, which is the connection hold time.

**Release candidate** (private `release/joined-r6-candidate-20261119`, head `cff41bfb0`, binary SHA-256 `a362045b05aa7f2e201190220c2e03413a1c0c992b528b20930d8dda2a987825`)

- It combines the joined line (B24, `main`, the finality fix and release prep), release 6.1 and the stall fix.
- It replays a fresh copy of validator-3 from genesis to 1577 and reaches the root all six validators reported.
- Tests (passed/failed/ignored):
    - transport and consensus v2: 82/0/1;
    - b23 facility: 6/0;
    - `rpc_serve_request_tests`: 45/0.
- Rehearsal as a disposable six-validator network without network access:
    - the transfer passed (2.11 s);
    - the deposit passed (8.99 s);
    - with the live finality flags, a round with a stopped validator and a round with a refusing validator both certified with five;
    - strict all-votes mode still needs all six, by design.

**Fleet**

- Release 6.2 went onto all six validators at about 10:24 UTC. It is the client's private `sol/release6-2`, the certified-delivery fix for lost apply acknowledgements.
- 6.2 does not contain the stall fix, so the candidate above is now one release behind the fleet.
- [Current State](../status/chain-state-current.md) last records release 6.1 at height 1513 (05:56 UTC).
- We made no live probe in this session.

**Instant pilots.** Private PR #34 (`instant/pilot-fixes-20261119` → `instant/pilot-integration-20261118`, head `6e3b23a47`). Its changes:

- every Solana refusal is recorded with a final operator refund;
- the directory is fsynced after every create and rename;
- the recipient is checked before signing;
- an operator void path exists only when there is proof that the payment's sequence was taken;
- Ethereum caps are required when enabled;
- each service has its own float account, fixed by a binding file.

Three independent reads; the last one approves.

Tests: `ops/sol-advance` 30/30, safety 32/32, wallet-proxy 58/58 files.

Both pilots now need separate float accounts.

**Ethereum successor.** This work started under the Ethereum-side goal, on private `navcoins/successor-node-receipts-20261119` (`f49c49a23`, `0483798c7`).

- Done, each pinned to the contract's output:
    - decoding `ExportDelivered` and `ReturnLocked`;
    - deriving the return burn id and the export key;
    - building `consumeExport`.
- Open: the send path, the route selection and the `PFTLNavFreshness` gate.

**Wallet.** Task Node pull request 413 is brought up to date with that repository's `main`. It is mergeable but not merged.

**Devnet.** We did not touch it: reads and fresh data copies only. Key folders are now excluded from the copy scripts.

**Task Node.** Tasks were requested, accepted and rewarded for the substantial steps, among them:

- `task_1ca13ae383c6e674e94b5b26bc243fc5`
- `task_a19db6307a9a00483906360065f45390`
- `task_bf2d291ad91e40780c1fcf44bc0412e5`
- `task_d48a258bb708cdec50cd3c08afc4c3b5`

Small steps had no task.

## Next decision or action

Ours:

1. [ ] Rebuild the release candidate on release 6.2 (the joined line plus the stall fix), replay from genesis and rehearse. Then roll out one validator at a time, with a test transfer after each, in our window and with the operator's go.
2. [ ] Before more real money goes over the Solana route:
    - take PR #36 (launch order) and the points of PR #33;
    - then take #35, with the ELF and key rebuild;
    - then take #37, with its activation height scheduled before the next epoch handoff.

    Keep real deposits small until then.
3. [ ] Merge PR #34 into the pilot branch, and set up a second float account before both pilots run.
4. [ ] Successor node: the `consumeExport` send path, route selection and the NAV freshness gate.

Only the client can do these (date first asked):

- move the test Solana routes to a separate test asset instead of the real PFUSDC (2026-11-19);
- choose the audit firm and budget for the PFT token contracts (2026-10-26);
- give his reading of the 30- and 60-day goals, and say which goal moves now that Solana comes first (2026-11-10).

## References

- **Private archive:**
    - the client's handoff `docs/handoffs/2026-11-19___claude__trustless_solana_bridge_handoff_and_overnight_plan.md` and his 09:15 status;
    - private PRs #33–#37 and the branches named above.
- **Status:** [Current State](../status/chain-state-current.md).
- **Previous handoff:** [2026-11-18: Instant USDC in and out built, explorer checked and speed fix reviewed](2026-11-18___dravlic__instant_usdc_in_and_out_built_explorer_checked_and_speed_fix_reviewed.md).
