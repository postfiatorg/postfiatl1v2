# Pillar 3 — Review brief, second pass

Written 2026-11-28 for a reviewer who was not part of the work. The first brief is [`03-validator-program-review-brief.md`](03-validator-program-review-brief.md); its results are [`03-validator-program-review-results.md`](03-validator-program-review-results.md). This brief covers what changed since, and points at the places most likely to be wrong. Read the code and try to break it.

Plan of record: [`03-validator-program.md`](03-validator-program.md).

## 1. What is different from the first pass

- **Everything the first pass reviewed is now merged.** `postfiatl1v2` `main` is `62edebc6a` (`#286`, `#290` including `#321`). `tasknode` `main` has `#459` as `33a74ee57`. The site notice `#66` is live.
- **Most first-pass findings were merged unfixed.** One commit followed the review: `2b3e7111f` (never back up or roll back a signing record or key file). It answers the second P0 and part of one P1. Section 2 lists the rest.
- **A release is published.** [`v0.4.0-devnet2-p3`](https://github.com/postfiatorg/postfiatl1v2/releases/tag/v0.4.0-devnet2-p3), GitHub pre-release, built from `62edebc6a` by workflow run `46090050228`, signed 2 of 3, release sequence 1.
- **Two PRs are open.** [`#326`](https://github.com/postfiatorg/postfiatl1v2/pull/326) (`bm/p3-seed-new-node`, `32582ca43`, `history.rs` +185 −34) and [`#327`](https://github.com/postfiatorg/postfiatl1v2/pull/327) (`bm/p3-upgrade-follow-redirects`, `5f867ba60`, `release_upgrade.rs` +16 −1). Both show one failing check, `build-lane-export-identity`, which also fails on `main`.
- **Nothing is deployed.** The six validators run another branch (section 3.6). Task Node enforcement is off.
- **One person holds everything.** The founder is a solo developer. There is no second builder, no second signer, no reviewer. The author (an agent on the founder's machine) made all three release keys and merged its own PRs.

Line numbers below are for `crates/node/src/release_upgrade.rs` at `62edebc6a` unless another file is named.

## 2. First-pass findings: what is still open on `main`

The author has not re-verified the "open" rows beyond confirming that no commit touched the files. Confirm each against the code.

| First-pass finding | State on `main` | Where |
|---|---|---|
| P0 · A failed stop or failed restore can still start a voting service | **Open.** The stop result is discarded at L2573. The units are started whether or not the restore worked; the comment above `start_units` in `rollback_and_verify` says this is on purpose | `rollback_and_verify`, L2559–2620 |
| P0 · Rollback rewinds a persisted vote lock | **Changed, not proven.** `block_proposal_vote_locks`, `transport_votes`, `consensus-v2-qcs`, `owned_locks.*` and two signing-state directories joined the exclusion list. See 3.1 for what that may break | `DEFAULT_BACKUP_EXCLUDES`, L53 |
| P1 · A second `upgrade` can roll back one that is still running | **Open.** No lock is taken | `execute_upgrade`, L1820; `recover_interrupted_upgrade` |
| P1 · Key exclusion depends on convention; restore does not verify the backup | **Half changed.** A content scan now finds key files under any name (3.2). Restore still installs the saved binary and tree without checking the hashes the backup wrote | `key_bearing_files`, L519; `restore_pre_upgrade_backup`, L1694 |
| P1 · Downgrade protection lost if the installed-release record fails to write | **Open** | L2447 |
| P1 · Late-vote credit is not bound to its source | **Open** | `scripts/validator-uptime-observer.py` |
| P1 · Two observers can agree on a row without agreeing on the reward | **Open** | `scripts/validator_uptime_contract.py` |
| P1 · A cold-started observer grants a new ramp-in to old validators | **Open.** The soak feed shows it today | observer, contract |
| P1 · Task Node acts on an old quorum after the feed loses quorum | **Open.** `refreshL1ValidatorAccess` returns on `indeterminate` without recording it; `creditFromFeedRow` and `feedAllowsNewBindings` read rows up to an hour old | `tasknode/server/l1-validator-access.js` |
| P1 · Concurrent refreshes and grants commit stale decisions | **Open.** Observer watermarks are read before the transaction and overwritten without a monotonic check | same file |
| P1 · Research-key revocation depends on binding order | **Open.** The loop revokes keys inside the per-binding pass | same file |
| P1 · A follower reports ready on a divergent tip of equal height | **Open** | `rpc_cli.rs`, `rpc_read_cache.rs` |
| P1 · The guide runs the downloaded binary to verify itself | **Open.** `#66` added a notice only | `postfiatorg.github.io` `content/run-a-validator.md` |
| P2 · Manifest assembly does not check binary architecture | **Open.** `scripts/release-assemble` has no ELF check. The published release went through it | |
| P2 · Replica cache holds reads that can change at a fixed height | **Open** | `rpc_read_cache.rs` |
| P2 · Python reader does not compare the feed's keyset id | **Open** | `scripts/validator_uptime_contract.py` |
| P2 · Observer bracket attribution, removed ids kept for ever, HTTP edge limits | **Open** | |

The first results file set a release order: fix both rollback P0s and rehearse recovery before any release. The release was published without that. Say whether the published binary's `upgrade` command should be withdrawn or marked "do not run on a validator" until the first row is fixed.

## 3. Where a mistake is most likely

Ordered by damage.

### 3.1 Rollback now leaves voting state ahead of chain state

`2b3e7111f` widened the list of things a rollback never restores. After a rollback the node has old chain state and newer vote locks, QCs, transport votes and owned-object locks.

- Does the prior binary start in that state? Does it resync, or does it refuse blocks because a lock names a block it does not have?
- `consensus-v2-qcs` is excluded. Is a QC store ahead of the block store read safely on start?
- `owned_locks.json`, `.wal`, `.guard` are FastPay lock state. Is carrying them forward across a state rewind right, or can it strand an owned object?
- Is the list complete now? Search every writer under the data directory that records a signature the validator has given. The author found these by listing a live data directory, not by reading every signing path.
- The only test is `signing_records_and_key_files_under_any_name_are_never_backed_up_or_rolled_back` (L3256), with a mocked systemd and a mocked node. **No rollback has run against a real node on any machine.**

### 3.2 The key-file scan is a heuristic

`key_bearing_files` (L519) reads every regular file up to 1 MiB and looks for four quoted JSON names: `"private_key_hex"`, `"private_key"`, `"secret_key_hex"`, `"secret_key"`.

- **Misses:** a key in a file over 1 MiB, in PEM, raw hex, binary, or under another JSON name (`sk`, `seed`, `signing_key`, `mnemonic`). Grep the tree for every key-file writer and check each name.
- **False hits:** any state file under 1 MiB that happens to contain one of those strings is left out of the backup and is therefore not rolled back. Could that be a file the old binary needs at its old content?
- **Timing:** the scan runs at L1833, before the units are stopped at L2326. The backup is taken after the stop. A file written in between is not scanned.
- Symlinks are skipped by the scan and by the copy. A key directory reached through a symlink inside the data directory: confirm it is neither copied nor deleted on restore.

### 3.3 The published release

- **The 2-of-3 signature gives no separation.** All three private keys are plain files, mode 0600, in `~/var/p3/release/keys/` on one development machine, made by an agent. The release notes say the founder holds all three. An operator who installs `release-keyset.json` pins those keys as the trust root. Is that acceptable for a pre-release, and what is the rotation path to real custody? `apply_release_keyset_rotation` (L673) exists but has only unit tests.
- **Nobody rebuilt it.** `docs/release-process.md` step 2 requires a second clean build to match the hash. It was skipped. Rebuild `62edebc6a` and compare with `SHA256SUMS` (x86_64 `9c3d1fc8…`, aarch64 `191252ec…`).
- **The aarch64 binary has never been executed.**
- **Circuit metadata came from another build.** `swap.metadata.json` and `private-egress.metadata.json` were copied from validator-0, which runs `train-a-r3` (`#325`), not `main`. Their hashes are in the signed manifest. Check what consumes those hashes (`lifecycle_queries.rs` L1101–1115) and whether a `main` build with these files passes the pre-start check and proves and verifies correctly.
- **The checkpoint** (block 1941, committee epoch 3) was read from one replica the author runs. Check it against the six validators.
- **`protocol_version` is 1**; the documented example says 2. `release_sequence` is 1 with no earlier record. Are both right for devnet-2?
- **Known defect shipped:** `upgrade --channel-url` fails against GitHub (3.4). The notes give a workaround.

### 3.4 `#327`: following redirects

`fetch_https_bounded` (L1305) shells out to `curl`. The PR adds `--location --proto-redir =https --max-redirs 5`, and replaces `--max-time 30` with `--connect-timeout 30 --speed-limit 10240 --speed-time 30 --max-time 1800`.

- A redirect may now go to any https host. Integrity rests on the signed manifest and the hash check of the binary. Confirm nothing fetched this way is used before it is verified, and that `max_bytes` is still enforced on the redirected body.
- `--proto-redir` needs a recent `curl`. What does an old `curl` do: fail closed or ignore it?
- `curl` is found through `PATH` and `upgrade` runs as root. Is the path pinned?
- The fetches happen before the units stop (L1942, L2153), so a 30-minute download does not hold a validator down. Confirm no fetch sits after L2326.

### 3.5 `#326` and what it uncovered

Two transaction ids on devnet-2 are committed in more than one block: one at heights 64 and 66 with identical accepted receipts, one at 940, 941 and 945 with two distinct receipts.

- **Why does the chain commit one transaction id more than once?** If the transaction ran twice, that is a replay hole in the state machine, far more serious than the archive bug. The author fixed the archive code and did not investigate this. It is the first thing to look at.
- The fix (`receipts_in_commit_order`, `history.rs` L2020 on the PR branch) pairs the k-th block that names an id with the k-th stored receipt for that id. That assumes the receipt file is in commit order and holds exactly one receipt per commit. Three commits with two distinct receipts suggests one receipt may be missing or duplicated. What does the pairing do then?
- `history_prune_plan` now refuses a boundary that splits such an id (L811–826). On a chain with many repeats, is there any legal boundary?
- One test, written by the author of the fix.

### 3.6 A new node still cannot join

Issue [`#320`](https://github.com/postfiatorg/postfiatl1v2/issues/320) is open. This is the user story of the pillar, and it fails.

- The full snapshot's `blocks.json` is 369 MB; the import bound is 256 MiB.
- Pruning first is impossible: `build_history_checkpoint_state` replays from genesis and block 12 fails with `checkpoint state root mismatch` under current code.
- **Block 12 not replaying means the state transition changed without a version gate.** Find the change. Does any other path replay from genesis (audit, terminal full-state audit of the follower, archive verify)? Those are broken too.
- The author's proposed fix is block history in bounded segments streamed into the transactional store. Not started. Is that the right fix, or should the bound simply be raised for now?

### 3.7 Two release systems that do not meet

- `postfiat-node upgrade` swaps one binary at `/usr/local/bin/postfiat-node` and reads a v3 manifest.
- The six validators use a deployment-manifest layout (`/opt/postfiat/releases/<id>/`, `/etc/postfiat/releases/<id>/`, `python/postfiat_ops/safe_rollout`).
- So the command the guide tells outside operators to run has never run on any validator, and cannot be rehearsed on the fleet as it stands. Which system is the real one? Should outside operators and the fleet differ?
- The fleet runs `train-a-r3` from open PR `#325` (`b9b024327`), not `main`. Pillar 3 code is on no validator. Validators answer `block_vote_records` with `rpc_method_not_allowed`, so the soak feed still counts certificates only and shows validator-5 under 95%.

### 3.8 Task Node, before enforcement is switched on

All in `server/l1-validator-access.js` on `tasknode` `main`.

- `l1UptimeFeedConfig`: enforcement set to `true` with a missing URL or keyset returns `enforced: false` and says nothing.
- `refreshL1ValidatorAccess` locks every L1 binding `FOR UPDATE` on each run. Key revocation is permanent and happens inside the per-binding loop.
- `creditFromFeedRow` grants from a row up to an hour old and is not re-checked under the lock that writes the ledger entry.
- The smoke test `scripts/l1-validator-access-postgres-smoke.mjs` was added to `scripts/smoke-suites.json` only to make the `check` job pass. Confirm CI actually runs it against Postgres.

### 3.9 Process

- L1 `main` has no branch protection. `docs/release-process.md` ("Protected-branch requirements", and three signers on three machines) describes rules that do not exist. The document is wrong today.
- All PRs were merged by their author with no approving review. `#290` was merged with `build-lane-export-identity` red, and after a re-run of the flaky test `pq_active_pool_requires_atomic_inclusion_at_proposal_and_commit`. Is that test flaky, or is it an intermittent real fault?
- `build-lane-export-identity` fails because the committed SP1 program and verification key do not match a rebuild. The author left it alone. Someone should decide which is right.

## 4. How to run it

`postfiatl1v2`, from `/home/postfiat/repos/_bm_worktrees/w1-p3-validator-ops` (branch `bm/p3-upgrade-follow-redirects`; `public` is the remote for `postfiatorg/postfiatl1v2`):

```bash
git fetch public main bm/p3-seed-new-node bm/p3-upgrade-follow-redirects
cargo test --locked -p postfiat-node --lib -- release_upgrade history
git diff public/main...public/bm/p3-seed-new-node
git diff public/main...public/bm/p3-upgrade-follow-redirects
```

The release, as an outsider:

```bash
T=$(mktemp -d) && cd "$T"
gh release download v0.4.0-devnet2-p3 -R postfiatorg/postfiatl1v2
sha256sum -c SHA256SUMS
chmod +x postfiat-node-linux-x86_64
./postfiat-node-linux-x86_64 release-manifest-verify --manifest-file release-manifest.json \
  --keyset-file release-keyset.json --runtime-binary-file postfiat-node-linux-x86_64 --arch linux-x86_64
```

Seeding failure: store copy and logs at `/mnt/HC_Volume_106212907/stakehub-hyperliquid/p3-seed-test/` (`prune.log`, `seed.log`, `out/handoff.json`). The repeated ids are on devnet-2 at heights 64, 66 and 940, 941, 945; read them through the tunnels `127.0.0.1:39650`–`39655`.

Still running locally: replica `p3-replica` (TCP 27751), edge `p3-edge` (HTTP 8750), follower `p3-follow`, soak feed in `~/var/p3/observer-soak/`.

**Do not write to the validators.** The author's fleet access was read-only throughout.

## 5. Not exercised at all

- A full `upgrade` (stop, back up, swap, start, health check) on a real node.
- A rollback on a real node.
- A keyset rotation outside unit tests.
- The packaged systemd units under systemd.
- The aarch64 binary.
- A second, independent build of the release.
- Seeding a new node from the published release.
- Three observers on three machines. The soak runs three on one machine with keys the author made.
- Task Node with enforcement on, against a real feed.
- `tasknode-bind` on a real validator host.
- Not built: the join and ratification flow for an outside operator, public read servers, one credit per operator across chains.

## 6. What a useful review returns

For each finding: file and line, a concrete input or state that goes wrong, and what happens. Sort by severity. Answer these plainly:

1. Did the repeated transactions execute more than once?
2. What change makes block 12 fail to replay?
3. Is a rollback safe with voting state ahead of chain state?
4. Should the published `upgrade` be marked unsafe until the rollback P0 is fixed?
5. Which open rows of section 2 must be fixed before Task Node enforcement is switched on?
