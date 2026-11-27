# Pillar 3 — Review brief

Written 2026-11-27 for a reviewer who was not part of the work. It says what to review, where it is, how to run it, and where the author is least sure. It is not a summary to agree with: read the code and try to break it.

Plan of record: [`03-validator-program.md`](03-validator-program.md) (numbered steps, decisions, status).

## 1. What to review

Five pull requests, none merged. Review them in this order; the later ones build on the earlier.

| # | PR | Branch → base | Head | Size | What it is |
|---|---|---|---|---|---|
| 1 | [postfiatl1v2#286](https://github.com/postfiatorg/postfiatl1v2/pull/286) | `bm/p0-p3-validator-spec` → `main` | `c15fa75a2` | +3,945, 11 files | The contract: two spec documents, two JSON schemas, a signed fixture, the reference verifier and its test |
| 2 | [postfiatl1v2#290](https://github.com/postfiatorg/postfiatl1v2/pull/290) | `bm/w1-p3-validator-ops` → `main` | `371b8b93c` | +15,790, 62 files | The node code, the observer, the packaged units |
| 3 | [postfiatl1v2#321](https://github.com/postfiatorg/postfiatl1v2/pull/321) | `bm/p3-release-pipeline` → `bm/w1-p3-validator-ops` | `7a3fb5267` | +659, 7 files | Release build workflow, assembly script, manifest creation, procedure |
| 4 | [tasknode#459](https://github.com/postfiatorg/tasknode/pull/459) | `bm/p3-validator-feed` → `main` | `1f1dae62d` | +1,070, 20 files | Task Node reads the feed and enforces it (off by default) |
| 5 | [postfiatorg.github.io#66](https://github.com/postfiatorg/postfiatorg.github.io/pull/66) | `docs/run-a-validator-status` → `main` | `ca161e1ae` | +16 | A notice on the live guide. Merging publishes it |

#290 includes the commits of #286 (the contract branch is merged into it). Much of #290's size is the fixture and the original PR; the author's own changes are the commits from `9e428b44a` on.

Local worktrees, all under `/home/postfiat/repos/_bm_worktrees/`: `p0-p3-spec`, `w1-p3-validator-ops`, `p3-release-pipeline`, `tn-p3-feed`, `site-p3-status`.

Earlier review to check the answers against: the second-reader comments on #286 (5 blocking points) and #290 (7 blocking points), both from 2026-11-27 09:00 UTC. The author's point-by-point answers are the next comments on each PR. **Check each answer against the code, not against the comment.**

Related issue: [postfiatl1v2#320](https://github.com/postfiatorg/postfiatl1v2/issues/320) (a new node cannot be seeded on devnet-2). Found, filed, not fixed.

## 2. Files that matter most

In `postfiatl1v2` (#290 unless noted):

| File | Lines | Why it matters |
|---|---|---|
| `crates/node/src/release_upgrade.rs` | 3,498 | Stops and restarts a validator, copies and restores its chain state. A mistake here can stop a validator or let it sign twice |
| `crates/node/src/validator_signer.rs`, `validator_signer_guard.rs` | +155 | The signer process gains a new signing context (`tasknode-bind`). Security-critical |
| `crates/node/src/rpc_read_cache.rs` | 1,071 | The replica read path, cache, method gate, lag enforcement |
| `crates/node/src/rpc_cli.rs`, `rpc_serve_runtime.rs`, `rpc_dispatch.rs` | +206 | Where the replica path is wired into the server every validator runs |
| `crates/node/src/rpc_http_edge.rs` | 761 | Hand-written HTTP/1.1 parser in front of the RPC, with a per-address rate limit |
| `crates/node/src/validator_uptime.rs` | 1,751 | The node's reader, signer and bundler of the uptime feed |
| `crates/node/src/validator_top.rs` | 1,525 | The SSH dashboard (mostly from the original PR) |
| `scripts/validator_uptime_contract.py` | 448 | The contract as code; three implementations must match it |
| `scripts/validator-uptime-observer.py` | 556 | The observer: polls, dates blocks, counts votes, applies the lifecycle |
| `docs/specs/validator-uptime-feed-v1.md`, `validator-rpc-profiles-v1.md` | #286 | The contract text |
| `ops/validator-rpc/`, `ops/validator-uptime-observer/` | | Packaged systemd units and guides |

In `tasknode` (#459): `server/l1-uptime-feed.js` (the JavaScript reader), `server/l1-validator-access.js` (enforcement), `server/db/migrations/171_l1_validator_access.sql`, and the small changes in `server/repositories/validator-bindings.js`, `research-gateway.js`, `l1-validator-rewards.js`, `validator-research-worker.js`, `validator-dashboard.js`.

## 3. Decisions the author took alone

The founder said "I'm not making calls, get this done". These were taken without sign-off. Say if any is wrong.

1. **A day is a UTC day, dated by the observer.** Block headers carry no time. Blocks between two reads on the same UTC day belong to that day; a head change across midnight or a gap over 900 s makes the day incomplete.
2. **Uptime counts every accepted vote**: in the precommit certificate, the prepare certificate, or the proposer's vote record. First taken as "precommit certificate only", reversed the same day after measuring (section 5).
3. **Releases are signed 2-of-3 from the first release.**
4. **Three observers, 2-of-3 agreement.** A served row is one observer's signed claim copied whole: the middle score of three, the lower of two.
5. **Too few observed days holds the credit and keeps access.** The original spec lapsed the validator.
6. **`operator_identity_id` and `public_rpc_url` are removed from the feed**, because an observer cannot know them from the chain.
7. **A replica caches every committed-state read by height**, on the argument that a replica's store changes only when a block is applied. Validators never use the cache.
8. **Research API keys are revoked** (destructively) when a validator is revoked, unless the account holds another binding with access.
9. **The Docker compose files were deleted** from #290 because no image is published.
10. **A validator's `server_info` path was put back to `main`'s version**, dropping a change the original PR made for all profiles.

## 4. How to run it

`postfiatl1v2`, from `_bm_worktrees/w1-p3-validator-ops` (or `p3-release-pipeline` for #321):

```bash
cargo clippy --locked -p postfiat-node -p postfiat-rpc-sdk --all-targets -- -D warnings
cargo test --locked -p postfiat-node --lib -- validator_uptime validator_top rpc_http_edge release_upgrade
cargo test --locked -p postfiat-node --bin postfiat-node -- rpc_read_cache_tests rpc_serve_request_tests child_dispatch_limit_tests
cargo test --locked -p postfiat-node --test replica_read_equivalence

# Signer tests need a temp directory no other user can write through, and --ignored.
mkdir -p /run/user/$(id -u)/p3tmp && chmod 700 /run/user/$(id -u)/p3tmp
TMPDIR=/run/user/$(id -u)/p3tmp cargo test --locked -p postfiat-node --bin postfiat-node -- validator_signer_socket_tests --ignored

# Python. The venv has dilithium-py, jsonschema and pytest.
P=/home/postfiat/var/p3/venv/bin/python
$P scripts/test-validator-uptime-schema.py
POSTFIAT_NODE_BIN=$PWD/target/debug/postfiat-node PYTHONPATH=python $P -m pytest \
  python/tests/test_validator_uptime_observer.py python/tests/test_release_flow.py -q
```

`tasknode`, from `_bm_worktrees/tn-p3-feed` (`npm ci --ignore-scripts` is already done there):

```bash
node --test scripts/l1-uptime-feed.test.mjs scripts/l1-validator-access.test.mjs
# Real Postgres:
docker run -d --name rv-pg -e POSTGRES_PASSWORD=p3 -e POSTGRES_DB=tasknode -p 127.0.0.1:55439:5432 pgvector/pgvector:pg16
DATABASE_URL=postgres://postgres:p3@127.0.0.1:55439/tasknode TASKNODE_DATABASE_ENABLED=true \
  node scripts/l1-validator-access-postgres-smoke.mjs
```

Running against the live devnet, read-only:

- Local tunnels `127.0.0.1:39650`–`39655` reach the RPC of validator-0 to validator-5. JSON, one request per line.
- A test replica runs in docker: `p3-replica` (TCP 27751), `p3-edge` (HTTP 8750), `p3-follow`. Store copy at `/mnt/HC_Volume_106212907/stakehub-hyperliquid/p3-replica-seed/`.
- A soak feed runs every 5 minutes: `~/var/p3/observer-soak/` (`soak.log`, `public/validator-uptime-feed.json`).
- Release build of #290 at `~/var/p3/bin/postfiat-node`.

**Do not write to the validators.** Everything the author did on the fleet was read-only (RPC reads, one `rsync` of validator-3's store without its key files, one `scp` of an old snapshot).

## 5. Claims to verify

Each is something the author states. Treat each as unproven until you have checked it.

**Measured**

- Over the last 400 blocks of devnet-2 with all six validators healthy, validator-5 is in the precommit certificate of 90.2% of blocks and in either certificate of 95.2%. In the 73 blocks that have a proposer vote record, it has an accepted vote in 97.3%. *Reproduce through the tunnels: `blocks` (limit 8) for certificates; the records are files under `block-vote-records/` on each validator, readable over SSH.* The 97.3% rests on 73 blocks only.
- Replica, warm, 1,200 mixed reads, 3 clients: p95 15 ms over TCP, 16 ms through the edge. Cold: `fee`, `ledger`, `account_tx` take 5–6 s after each block. *Rerun: `python3 ~/var/p3/bench.py`.* This was a 32-core host, not the 2 vCPU validators, and the chain produced no block during the run.
- `rpc-catch-up` without `--follow` takes 5 minutes when already up to date; with `--follow`, 1.5 s.

**Behaviour**

- Merging #290 changes no answer of a validator on the standard profile. *Check: `git diff public/main...HEAD -- crates/node/src/rpc_cli.rs crates/node/src/rpc_serve_runtime.rs crates/node/src/rpc_dispatch.rs` and read every hunk that is not behind `is_replica()`.*
- A rollback never restores a validator's vote record and never leaves a readable copy of its key.
- The signer signs, under the new binding context, only a current Task Node challenge.
- Three implementations (Python, Rust, JavaScript) accept and reject the same feeds.
- One wrong observer is dropped; fewer than two leaves the feed indeterminate; an indeterminate feed revokes nobody.

## 6. Where the author is least sure

Ordered by how much damage a mistake would do.

1. **`upgrade` rollback (`restore_pre_upgrade_backup`).** The excluded entries (`consensus-v2-safety/`, `validator_keys.json`, `runtime`, `readiness`) are carried over by `rename` from a stash directory. Questions: is `consensus-v2-safety/` the *only* place a validator records what it voted for? (`block_proposal_vote_locks/`, `transport_votes/`, `consensus-v2-qcs/` also exist in a live data directory and **are** backed up and rolled back.) Does rolling chain state back while the vote record stays ahead leave the old binary able to start and resync? Tested only with a mock of systemd and the node, never on a real validator.
2. **The exclusion list is by name.** A validator that keeps its key under another file name, or passes `--finality-key-file` pointing elsewhere inside the data directory, gets its key copied into the backup. The spec says files named by the unit's key flags are excluded; the code only excludes the default names plus `--exclude-path`.
3. **The signer change.** `TASKNODE_VALIDATOR_BINDING_CONTEXT` joins the allow-list and bypasses the double-sign guard, as `TRANSPORT_AUTH_CONTEXT` does. It signs with the *vote* key. Is the challenge check tight enough that no consensus message could also be a valid challenge? Is domain separation by ML-DSA context sufficient on its own?
4. **Decision 7, the replica cache.** The claim "nothing changes on a replica between blocks" was reasoned, not proven. Does anything else write a replica's store (FastPay effects, a mempool, `account_tx` index rebuilds, the receipt reader)? The cached height comes from a status report up to 1 s old.
5. **`--follow` has never applied a live block.** The chain produced none while the author watched. Check `…/p3-replica-seed/run/log/follow.applied.log`. Also: is the per-block check in `rpc_catch_up_with_audit` really a full certificate verification against the registry, or does the skipped `verify_state` carry something the follower now misses?
6. **The observer's dating rules.** `poll()` in `validator-uptime-observer.py`: the first day, gaps, midnight, the registry taken at read time for every block of a bracket, vote records fetched up to two days late. The three-week simulation (`test_validator_uptime_observer.py`) was written by the same author as the code.
7. **Trust in a proposer's vote record.** It is unsigned. The spec argues a proposer can only affect its own 1/n of blocks. With n = 6 and a 95% rule, one lying proposer moves everyone by up to 16.7 points. Is that acceptable?
8. **Lifecycle edge cases.** A validator that recovers inside its first 30 days is labelled `day0_ramp_in`, not `recovered_3day_streak`. Rows of validators removed long ago are never dropped from an observer's output. `grace_started_at` is the observer's evaluation day, so two honest observers can differ by a day.
9. **Task Node enforcement.** Suspension is by a new column; every gate that should respect it must read it. Find every query on `validator_bindings … revoked_at IS NULL` and check none was missed (the public Validator badge in `publicValidatorBindingsByAccount` was left alone on purpose: is that right?). Research keys are revoked for good. `refreshL1ValidatorAccess` takes `FOR UPDATE` on all L1 bindings every five minutes.
10. **The HTTP edge.** Hand-written request parsing. It trusts the last `X-Forwarded-For` entry for rate limiting and allows 50,000 buckets. It reads the head one byte at a time. Thread per connection, cap 64.
11. **Schema name collision.** Task Node already had a probe record called `postfiat-validator-uptime-v1` with another shape (`server/l1-validator-uptime.js`). Both now exist.
12. **Docs published ahead of code.** The live "Run a Validator" page still describes unreleased commands. #66 adds a notice rather than rewriting the page; its commands were not checked against the real flags.

## 7. Known not done or not tested

- No PR is merged. `main` needs approving reviews.
- CI on #290 and #321 had not finished when this was written. `build-lane-export-identity` fails on `main` as well and is not caused by these PRs.
- The `release-build` workflow has never run. Nothing shows that two clean builders produce the same binary hash.
- Nothing was deployed to a validator. `upgrade`, `top`, the edge and `tasknode-bind` have not run on a real validator host.
- The packaged systemd units were not started under systemd; their `ExecStart` lines were run by hand in docker.
- Snapshot seeding is broken on devnet-2 (#320). The test replica is a raw copy of a validator's store.
- The soak feed counts certificates only, because the validators' release has no `block_vote_records` method. It will show validator-5 below 95%.
- Not started: the three release keys, a first signed release, public RPC endpoints, three real observers, the join and ratification client, one credit per operator across chains, the validator directory, the settings UI.
- The full workspace test suite and Task Node's full `npm run check` were not run locally.

## 8. What a useful review returns

For each finding: the file and line, a concrete input or state that goes wrong, and what happens. Sort by severity. Say separately which of the twelve earlier review points you checked against the code and whether each holds. If something in sections 3 or 5 is wrong, say so plainly.
