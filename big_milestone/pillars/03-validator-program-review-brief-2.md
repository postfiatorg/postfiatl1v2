# Pillar 3 — Review brief, second pass

Updated 2026-11-28, after the fixes. Written for a reviewer who was not part of the work. The first brief is [`03-validator-program-review-brief.md`](03-validator-program-review-brief.md); its results are [`03-validator-program-review-results.md`](03-validator-program-review-results.md). Plan of record: [`03-validator-program.md`](03-validator-program.md).

Read the code and try to break it. Section 3 says where a mistake is most likely.

## 1. Where things stand

- The five original PRs were merged on 28 November with most first-review findings open. A release was published from that code. Both were mistakes. The release is withdrawn (a draft, not downloadable).
- Every first-review finding now has a fix with tests. Section 2 lists them.
- Two rehearsals ran on real nodes under systemd. They found seven more faults that the mocked tests had hidden. All are fixed. Section 4 lists what was run.
- Everything is merged: [`postfiatl1v2#329`](https://github.com/postfiatorg/postfiatl1v2/pull/329) (`7d80aef48`, upgrade and release), [`#330`](https://github.com/postfiatorg/postfiatl1v2/pull/330), [`#331`](https://github.com/postfiatorg/postfiatl1v2/pull/331), [`#334`](https://github.com/postfiatorg/postfiatl1v2/pull/334) (seeding), [`#326`](https://github.com/postfiatorg/postfiatl1v2/pull/326), [`tasknode#461`](https://github.com/postfiatorg/tasknode/pull/461), [`site#84`](https://github.com/postfiatorg/postfiatorg.github.io/pull/84) (guide, live).
- A release is published as a pre-release: [`v0.4.1-devnet2-p3`](https://github.com/postfiatorg/postfiatl1v2/releases/tag/v0.4.1-devnet2-p3), from `main` `7d80aef48`, sequence 2. Two clean workflow builds gave identical hashes. It was installed by `upgrade --channel-url` straight from GitHub on the replica rig, and the guide's install steps were followed in a clean container.
- Nothing is deployed. The six validators run another branch (`train-a-r7`). Task Node `main` is not deployed with `#461`, and enforcement is off.
- A new node can join devnet-2 from a full snapshot (`#334`: imported at 1961, followed the chain, matched validator-0's tip and state root at 2016). A pruned snapshot still does not work ([`#320`](https://github.com/postfiatorg/postfiatl1v2/issues/320)).
- One person holds everything. The founder is a solo developer. There is no second builder, signer or reviewer. An agent on the founder's machine made the release keys and merged its own PRs. `docs/release-process.md` now says this.

## 2. First-review findings and their fixes

Confirm each against the code. "Rehearsed" means it ran on a real node, not only in a mocked test.

| First-review finding | Fix | Where | Rehearsed |
|---|---|---|---|
| P0 · A failed stop or failed restore can still start a voting service | Rollback refuses if any unit is still active. After a failed restore the units stay stopped, exit 6, journal kept | `rollback_and_verify`, `#329` | Yes: damaged backup left the units stopped, then recovered |
| P0 · Rollback rewinds a persisted vote lock | Every signing record is left out of backup and restore and only carried forward | `DEFAULT_BACKUP_EXCLUDES`, `#329` | Yes: a rolled-back validator refused to sign a second block for a slot it had signed |
| P1 · A second `upgrade` can roll back one that is running | Exclusive lock on `<backup-dir>/upgrade.lock` | `acquire_upgrade_lock`, `#329` | Yes |
| P1 · Restore does not verify the backup; key exclusion by convention | Backup checked file by file (size, SHA-256, no unnamed file) before anything is replaced. Key files found by content | `verify_pre_upgrade_backup`, `key_bearing_files`, `#329` | Yes (damaged backup) |
| P1 · Downgrade protection lost if the installed record fails to write | Journal step `record_pending`; status `upgraded_record_pending`, exit 4; next run writes the record | `#329` | Test only |
| P1 · Late-vote credit not bound to its source | A vote record counts only from the proposer's own source (`--source validator-id=endpoint`) | `validator-uptime-observer.py`, `#330` | Test only |
| P1 · Observers agree on a row without agreeing on the reward | `reward_eligible` and `grace_started_at` are agreement fields in Python, Rust and JS | `#330`, `tasknode#461` | Test only |
| P1 · Cold start grants a new ramp-in | Cold start is `bootstrap`; activation dates come from `--activation-file` | `#330` | Test only |
| P1 · Task Node acts on an old quorum | Every refresh outcome is recorded (`l1_uptime_feed_state`); grants need a current decision | `tasknode#461` | Postgres smoke |
| P1 · Concurrent refreshes commit stale decisions | Advisory lock; state re-read inside the transaction; `superseded` | `tasknode#461` | Postgres smoke |
| P1 · Key revocation depends on binding order | Keys revoked once per account after all bindings are processed | `tasknode#461` | Postgres smoke |
| P1 · Follower ready on a divergent tip of equal height | The source's tip must be a block of the follower's own chain | `rpc_catch_up_source_not_ahead`, `#331` | Test only |
| P1 · Guide runs the downloaded binary to verify itself | `scripts/verify-release`: an independent Python check of signatures, size, hash and ELF | `#329`, `site#84` | Yes: agrees with the Rust check on a real release |
| P2 · No architecture check at assembly | ELF machine check in `scripts/release-assemble` | `#329` | Yes |
| P2 · Cache holds reads that change at a fixed height | `block_vote_records` and `account_tx_index_status` are never cached | `#331` | Test only |
| P2 · Python reader ignores the keyset id | Compared | `#330` | Test only |
| P2 · Observer and edge limits | `removed_since` retention, per-block committee, whole-request time budget on the HTTP edge | `#330`, `#331` | Test only |

## 3. Where a mistake is most likely

Ordered by damage.

### 3.1 Rollback on a voting validator

Rehearsed on four voting validators (section 4). What it showed and what it did not:

- A rolled-back validator has old chain state and newer vote records. It refused to sign a conflicting block. Only the proposal lock was provoked. Try the vote path: make it vote for a different block at a height it already voted on.
- **New fault found and fixed:** the rolled-back node stayed behind by the blocks made during the attempt and never caught up, and the chain waited when its turn to propose came. `upgrade --catch-up-source HOST:PORT` now fetches them before the restart; without it the report sets `catch_up_needed`. Check `run_catch_up_as_data_owner`: it runs the restored binary as the data directory's owner, with retries. Is running `rpc-catch-up` on a validator's data directory sound in every storage mode? An older note in `scripts/wan-devnet-state-sync` avoids it for historic payloads.
- The catch-up does a full state verification on each run. On devnet-2 that takes minutes, with the validator down.
- Not tried: a vote record the old binary cannot read; two validators down at once; the redb store (the fleet and the rig use the JSON store).
- The health gate took a full minute to fail on a wrong revision. In that minute the new binary voted on 17 blocks. Should a wrong revision fail at the first answer?
- **The chain waits whenever the proposer for a height is down.** Seen on the rig; nothing takes over. That is existing behaviour, not part of this pillar, and it sets how a rolling upgrade feels.

### 3.2 The key-file scan is a heuristic

`key_bearing_files` reads every regular file up to 1 MiB for four quoted JSON names (`"private_key_hex"`, `"private_key"`, `"secret_key_hex"`, `"secret_key"`).

- Misses: a key in a larger file, in PEM, raw hex, binary, or under another name. Grep every key-file writer.
- False hits: a state file that contains one of those strings is left out of the backup and not rolled back.
- The scan runs before the units stop; the backup is taken after.

### 3.3 Releases

- 2 of 3 signatures give no separation while one machine holds all three keys.
- The release was built twice by the same workflow with identical hashes. Nobody has rebuilt it on other hardware or by another route.
- Circuit metadata now comes from the files committed in `main` (`deployments/combined-fastpay-20260928`); the artifacts they name match `main` by size and SHA-256. The manifest's `committee_epoch` (3) is not checked by the node against the chain.
- `upgrade --channel-url` follows redirects to any https host (`#327`, inside `#329`). Integrity rests on the signed manifest and the binary hash. Confirm nothing fetched is used before it is verified.

### 3.4 Replica audit as its own job

The full state audit left the follower and became `rpc-replica-audit` with its own timer (`#331`). While it runs the replica refuses reads. Check `RPC_REPLICA_AUDIT_MAX_RUNNING_SECS` (4 h): what happens when an audit dies without clearing its record, and when it outlives the limit.

### 3.5 Seeding a new node (`#320`)

Three separate faults:

1. The import bound on the block history file was 256 MiB; devnet-2's is 370 MB. Fixed in `#334`: 4 GiB for that one file. `rpc-catch-up` also asked for more blocks than one reply can hold; it now shrinks its request (`fetch_in_shrinking_windows`). One block over 4 MiB still cannot be fetched. At about 190 KB a block the new bound lasts to roughly height 22,000. That is a stopgap.
2. `build_history_checkpoint_state` replays from genesis and block 12 fails under current code. So no pruned checkpoint can be built. **Something in the state transition changed without a version gate.** Find it.
3. `validate_history_checkpoint_state` uses the v1 state root, so a checkpoint taken after the activation cannot validate.

### 3.6 Repeated transaction ids on devnet-2

Ids repeat at heights 64/66 and 940/941/945. The author's answer: these are governance route-profile actions that were refused and resubmitted, and the code allows it under `WAN_DEVNET2_DEDUPLICATED_GOVERNANCE_RECEIPT`; nothing ran twice. Check that independently.

### 3.7 Two release systems that do not meet

`postfiat-node upgrade` swaps `/usr/local/bin/postfiat-node` under a v3 manifest. The six validators use a deployment-manifest layout (`/opt/postfiat/releases/<id>/`). The command the guide gives outside operators has never run on a fleet validator. Which system is the real one?

### 3.8 Task Node, before enforcement is switched on

`server/l1-validator-access.js` and `server/l1-validator-rewards.js` after `#461`. Enforcement with a missing URL or keyset now throws at start. Look for a path that grants credit or a research key without a current feed decision.

### 3.9 Process

- L1 `main` has no branch protection. PRs are merged by their author.
- `build-lane-export-identity` fails on `main` and every PR: the committed SP1 program and key do not match a rebuild. Nobody has decided which is right.
- A consensus timing test (`a_prepare_answered_8s_late_does_not_delay_an_early_quorum_round`) failed once on `#329` at 8.17 s and passed on re-run. Flaky, or a real intermittent delay?
- A source on the JSON store can refuse a read while it writes a block ([`#333`](https://github.com/postfiatorg/postfiatl1v2/issues/333)).

## 4. What was rehearsed

**Replica of devnet-2 under systemd** (container `p3-rehearse`, real 2,000-block store): upgrade (59 s), rollback on a failed health check, rollback with a damaged backup (units left stopped, exit 6, then recovered), second upgrader refused by the lock, `kill -9` mid-upgrade and recovery, the audit job (786 s; reads refused in about 100 ms each while it ran). Faults found: the manifest was not installed with the binary; one probe after rollback gave a false failure; the audit inside the follower blocked the store.

**Four voting validators under systemd** (containers `p3-v0`..`p3-v3`, test chain, loopback, units and flags copied from the fleet; rig in `~/var/p3/vote/`): upgrade of one validator under traffic (4 s, every transfer certified), forced rollback, attempt to make the rolled-back validator sign twice (refused), rollback with catch-up (rejoined at the tip, proposed again). Faults found: the node left behind after rollback; one refused read on the source.

## 5. How to run it

```bash
cd /home/postfiat/repos/_bm_worktrees/w1-p3-validator-ops   # remote `public` = postfiatorg/postfiatl1v2
git fetch public main
git diff 62edebc6a..public/main -- crates scripts ops docs
cargo test --locked -p postfiat-node --lib release_upgrade
python3 -m unittest python/tests/test_release_flow.py
```

Voting rig: `. ~/var/p3/vote/lib.sh`, then `python3 -I ~/var/p3/vote/st.py 6` (heights and signers), `python3 -I ~/var/p3/vote/send.py 27850` (one transfer), `vupgrade <release>`.

**Do not write to the six validators.** The author's fleet access was read-only throughout.

## 6. Not exercised at all

- Any of this on a fleet validator.
- A keyset rotation outside unit tests.
- The aarch64 binary beyond one run of `release-build-info` on its build runner.
- A build of the release by another route than the workflow.
- Seeding from a pruned snapshot.
- Three observers on three machines.
- Task Node with enforcement on, against a real feed.
- `tasknode-bind` on a real validator host.
- Not built: the join and ratification flow for an outside operator, public read servers, one credit per operator across chains.

## 7. What a useful review returns

For each finding: file and line, a concrete input or state that goes wrong, and what happens. Sort by severity. Answer these plainly:

1. Can a rolled-back validator be made to sign two different votes for one slot?
2. Is `rpc-catch-up` on a validator's data directory sound?
3. What change makes block 12 fail to replay?
4. Is there a path in Task Node that grants without a current feed decision?
5. Which of the two release systems should the fleet use?
