# Saved StakeHub safety repairs review — 2026-09-22

**Verdict: not fit for live funds.** The saved repairs close most original
source defects, including SH-10/11/12, but SH-03 and SH-08 are only partially
closed. Cleanup recovery and private-egress retry handling still have gaps;
route resume also introduces a reproducible failure on ordinary chain progress.
No StakeHub repair, deployment, wallet operation or capital use is authorized by
this review.

## Scope and provenance

Reviewed `~/repos/StakeHub-safety-20260907`, branch
`fix/pr8-safety-20260907`, against base
`8f6f27cf66896b48f9a645e995a1288fc5f7218d`. `git status --short --branch`
showed the saved dirty repair set; `git log --oneline 8f6f27cf..HEAD` was empty.
HEAD remains the base: the repairs are **local, uncommitted and unpublished**,
not a tested repair commit. Publication status was supplied by the operator;
no StakeHub remote was contacted. This repository's initial
`git pull --rebase origin main` reported up to date at `297c687c`.

The [September 10 review](stakehub-fix-branch-review-20260910.md) carries
forward the [nine original findings](stakehub-pr8-review-20260907.md) and adds
three follow-ups. This review covers all twelve, using the existing SH-01–SH-12
identifiers in the [inventory](defect-inventory-20260910.md): four P1, seven P2
and one P3. It does not silently substitute the nine originals for SH-10/11/12.
All StakeHub line references below identify the saved working files, including
the untracked `stakehub/operational_safety.py` and four untracked test files.

The repair record, complete tracked diff against the base, untracked helper and
four test files were inspected. Snapshot identifiers:

| Snapshot component | SHA-256 |
| --- | --- |
| `git diff --binary 8f6f27cf` | `c7996ba0c233a69424688d438587afee967a88bce3cd7162863b3a68d88d2708` |
| `stakehub/operational_safety.py` | `5c49aa66a2c2d30351b0cdc533e05487313253e7399acfb84f72bd302f86675b` |
| `docs/review/pr8-safety-repairs-20260907.md` | `1952b8a89657b8875050ea965693f17233abc29e1ea9ac96dd4192a8b31e336b` |

Task Node was skipped under the requested failure exception. Account scope was
preserved. An initial read-only helper preflight failed with `Read-only file
system`; after permitting the helper's own session-directory writes, **two
network-isolated `tasknode status --json` calls** returned `tasknode_helper_error`:
`Task Node request failed: error sending request`. Networking remained disabled
to honor the offline restriction. This does not establish a backend outage.
No task request, task ID, acceptance, evidence submission or reward exists for
this review; no self-authored task replaced a generated proposal.

## Finding → verdict

The causal runner-summary contract was checked against this repository's
`scripts/a666-remote-sync-batch-round.py:63–91` and
`crates/node/src/transport_runtime.rs:3746–3755`: successful local application
requires nonempty, wholly accepted receipts at the certified height. This is
source-contract evidence, not identification of the binaries installed on any
validator; no remote runner or certificate was queried.

“Yes” means the named original failure is closed in this saved source, including
closure by disabling a feature. It does not mean production qualification.
“Partially” means the finding remains open for the stated reason.

| Finding | Priority | Closed? | Disposition |
| --- | --- | --- | --- |
| SH-01 — policy-denial signer fallback | P1 | **Yes** | Agent denial/unavailability cannot select the direct signer. |
| SH-02 — live on bare invocation | P1 | **Yes** | Reviewed mains require explicit live intent; dangerous legacy entrypoints are disabled. |
| SH-03 — remote private-file cleanup | P1 | **Partially** | Ordinary-exit cleanup improved; failed cleanup is not durably enforced on resume, and remote-custody limits remain. |
| SH-10 — overwrite an active release | P1 | **Yes** | Fleet-copy mutation implementation removed; live invocation refuses. |
| SH-04 — false private-egress success | P2 | **Yes** | Exact runner result and asset/amount checks replace height-only success; retry gap R3 remains separate. |
| SH-05 — implicit Ethereum RPC | P2 | **Yes** | Explicit endpoint required and used for reads/submissions; identity observations retained. |
| SH-06 — irreversible referrer signer | P2 | **Yes** | Handler disabled before master-account retrieval or SDK action. |
| SH-07 — sixteen external archive paths | P2 | **Yes** | Explicit cross-repository dependency disposition satisfies the original alternative; no self-contained verification claimed. |
| SH-08 — contradictory archive/publication claim | P2 | **Partially** | Factual correction present; manager retention/publication decision still deferred. |
| SH-11 — unbound shielding success | P2 | **Yes** | Exact accepted runner summary required; uncertain/legacy journals stop for reconciliation. |
| SH-12 — height mistaken for route activation | P2 | **Yes** | Exact accepted runner summary and selected-profile binding required; new resume regression R2 remains. |
| SH-09 — venue float rounding | P3 | **Yes** | Unsafe amounts rejected against the installed SDK conversion paths. |

## Correctness assessment: P1

### SH-01 — closed

**Location:** `stakehub/cli.py:1499`, `:1536–1543`.
**Condition:** policy denial, locked/unavailable agent, or malformed response.
**Observed:** anything other than a dictionary with `ok is True` returns failure;
the passphrase/direct-signer fallback is removed. **Expected:** fail closed
without a second signer or automatic retry; satisfied. **Remaining failure
condition:** a transport loss after the agent submitted cannot prove that no
transfer occurred. **Suggested change:** none for the bypass; reconcile that
outcome before an operator retries. This is not an audit of every agent operation.

### SH-02 — closed for the reviewed entrypoints

**Location:** `stakehub/operational_safety.py:16–39`;
`scripts/shielded_exit_deploy.py:25–39`,
`scripts/shielded_exit_probe_arbitrum.py:97–115`,
`scripts/shielded_exit_probe_lighter.py:75–90`;
`tests/test_operational_safety.py:16–45` enumerates the reviewed mains.
**Condition:** invoke without `--live`, or fail the displayed digest confirmation.
**Observed:** bare mains stop before operational I/O; live execution requires
typed public intent. `pfeth_nav_par.py:108` remains disabled even with `--live`.
**Expected:** invocation alone cannot authorize mutation; satisfied.
**Remaining failure condition:** explicit `--live` enables preflight that may
read/create private material before confirmation, as documented; this is not a
safe sandbox for exploring live configuration. **Suggested change:** none for
the bare-invocation defect; preserve the narrow scope of the safety claim.

### SH-03 — partially closed; P1 remains open

**Location:** `stakehub/operational_safety.py:80–121`;
`scripts/pfeth_shield.py:90–137`, `:63–65`;
`scripts/pfeth_private_egress.py:154–193`;
`scripts/pfeth_bridge_out.py:69–145`.
**Condition:** upload, parse, proposer, runner or interrupt failure after private
material is copied. **Observed:** cleanup is registered before upload, attempts
all declared files and reports uncertainty. However, shield and private egress
persist successful rounds before context-manager cleanup; a later resume can
skip that cleanup. R1 reproduces the shield case. **Expected:** cleanup
uncertainty remains a blocking recovery state across invocations. **Remaining
failure conditions:** that resume gap, SIGKILL, partitions, snapshots,
copy-on-write storage and compromised hosts. **Suggested change:** persist and
reconcile cleanup state independently of transaction success; keep legacy remote
custody disabled for capital use. Erasure attempts cannot restore a local-key
custody boundary once spend authority has reached another host.

### SH-10 — closed by retirement

**Location:** `scripts/pftl_fleet_release_apply.py:13–20`.
**Condition:** supply the running release identifier or any other release with
`--live`. **Observed:** the entire copy/promotion implementation is gone; the
entrypoint exits before file reads, confirmation or subprocess execution.
**Expected:** no in-place release overwrite; satisfied. **Remaining failure
condition:** deployment through a historical copy or an unqualified replacement
is outside this containment. **Suggested change:** none in the retired script;
qualify the supported signed rollout separately before any deployment.

## Correctness assessment: P2

### SH-04 — original false-success condition closed

**Location:** `scripts/pfeth_private_egress.py:87–92`, `:180–202`;
`stakehub/operational_safety.py:42–60`.
**Condition:** unrelated height advance, nonzero runner exit, rejected or
mismatched report, or equal numeric balance of another asset. **Observed:** the
runner must exit zero and report accepted local application, the intended
height/proposer and a nonempty certificate; the final balance must match both
asset ID and amount. **Expected:** those original false positives must fail;
satisfied. **Remaining failure condition:** a timeout before result persistence,
or interruption between the two result saves, permits retry rather than
reconciliation (R3). **Suggested change:** retain these receipt/asset checks and
apply the shared verified-journal and pre-attempt guard to egress as well.

### SH-05 — closed within operator-selected RPC trust

**Location:** `stakehub/navcoin_deposit.py:175–184`, `:218–220`, `:286–287`,
`:348`, `:380`.
**Condition:** missing endpoint, remote HTTP, or embedded URL user credentials.
**Observed:** rejected before package/network reads; the selected endpoint feeds
preflight and agent submissions, chain ID 1 is required, and endpoint digest
plus observed genesis are retained. **Expected:** no implicit public provider;
satisfied. **Remaining failure condition:** an operator-selected dishonest RPC
can supply internally consistent false observations; genesis is recorded, not
independently authenticated. **Suggested change:** none for endpoint selection;
qualify the provider and source-finality trust model before live funds.

### SH-06 — closed by disabling

**Location:** `stakehub/agentd.py:1607–1608`;
`tests/test_review_safety_regressions.py:18–26`.
**Condition:** an unlocked agent receives `hl_register_referrer`.
**Observed:** deterministic refusal without obtaining the master account or
constructing a signing action. **Expected:** no unreviewed irreversible account
mutation; satisfied. **Remaining failure condition:** restoring the old handler
would restore the unbound signing surface. **Suggested change:** keep disabled
unless a separately reviewed, tested authorization ceremony replaces it.

### SH-07 — closed as an explicit dependency disposition

**Location:** `docs/handoffs/navcoin-recovery-20260907/README.md:11–13`.
**Condition:** try to verify all 50 root-manifest records from StakeHub alone.
**Observed:** README now identifies 34 local and 16 external records and pins
L1 PR #39 revision `f2e749a16b07be446cb11da1e1f523eb9cdabca7`; it explicitly
disclaims a self-contained bundle. **Expected:** include the missing evidence
or label the external dependency; the latter is satisfied. **Remaining failure
condition:** the exact external tree is unavailable or unverified. **Suggested
change:** obtain and verify that tree before asserting full archive integrity.
The historical 190-file inventory and secret scan were not rerun in this review.

### SH-08 — partially closed; P2 decision remains open

**Location:** `docs/handoffs/navcoin-recovery-20260907/README.md:9`.
**Condition:** rely on the earlier assertion that recovery artifacts are outside
Git, or treat a clean scan as permission to publish them. **Observed:** the
README correctly says the archive is committed and rejects that inference;
retention/publication is still explicitly deferred. **Expected:** factual
correction and a reconciled publication/retention decision. **Remaining failure
condition:** publication proceeds without the manager's recorded disposition.
**Suggested change:** record that decision and supersede the contradictory
handoff at its owner; no archive deletion or history rewrite is implied.

### SH-11 — original false-success condition closed

**Location:** `scripts/pfeth_shield.py:63–65`, `:126–138`;
`stakehub/operational_safety.py:42–77`.
**Condition:** flag-only, wrong height/proposer, rejected application, nonzero
exit, absent certificate, malformed output, timeout or a legacy journal.
**Observed:** exact runner-summary checks govern success and verified resume;
an attempt marker is saved before launching the runner. Uncertain cases block
further submission. **Expected:** a stored boolean cannot establish shielding;
satisfied. **Remaining failure condition:** a changed/untrusted remote runner
or journal can violate the assumed causal receipt contract; cleanup recovery
also remains R1. **Suggested change:** none for the original flag check; repair
R1 and pin/qualify the actual runner and release before operational use. These
ordinary-file saves are not a demonstrated power-loss durability guarantee.

### SH-12 — original false-success condition closed

**Location:** `scripts/pfeth_route_activation.py:75–87`, `:103–107`,
`:159–192`; `stakehub/operational_safety.py:42–77`.
**Condition:** another block advances the fleet while this governance attempt
fails, or the selected profile changes. **Observed:** runner return code,
accepted local application, height/proposer and certificate are checked; the
profile hash is checked after copying and on resume. An attempt marker precedes
the runner, and fleet heights cannot rescue an invalid result. **Expected:**
height alone never establishes activation; satisfied. **Remaining failure
condition:** runner/journal trust is violated, or normal chain progress causes
R2's false failure. **Suggested change:** repair R2 using exact activation
history/current-profile readback; qualify the runner contract rather than
presenting this Python check as an independent certificate verifier.

## Correctness assessment: P3

### SH-09 — closed for the installed SDK conversions

**Location:** `stakehub/shielded_exit_venues.py:262–274`, `:290`, `:334–340`;
`tests/test_review_safety_regressions.py:59–111`.
**Condition:** noninteger, out-of-range or non-round-tripping withdrawal amount.
**Observed:** rejected before key retrieval/signing. Hyperliquid decimal-string
conversion and Lighter's integer tick truncation are accounted for; the actual
installed Lighter `signer_client.py:1128–1130` uses that conversion.
**Expected:** signed units equal reviewed integer units; satisfied within this
SDK contract. **Remaining failure condition:** an SDK serialization/scale change
without requalification. **Suggested change:** preserve version-sensitive wire
regressions; conservative rejection is preferable to silently altered units.

## Additional issues and reproductions

These identifiers are local to this review; the historical inventory is not
rewritten. R2 is a new regression from the follow-up repairs. R1 is an incomplete
cleanup integration; R3 is a retained retry gap, not a newly introduced defect.

### R1 — P1 — cleanup failure does not survive a successful-round resume

**Location:** `scripts/pfeth_shield.py:63–65`, `:134–137`;
`stakehub/operational_safety.py:111–118`. The egress ordering at
`scripts/pfeth_private_egress.py:140–142`, `:186–193` has the same gap.
**Condition:** the certified round succeeds, then the remote cleanup SSH fails.
**Observed:** the first call raises `CLEANUP UNCONFIRMED`, but the saved journal
already passes `verified_round_in_journal`. A second shield invocation returns
zero without any SSH or cleanup call. **Expected:** accepted settlement and
confirmed cleanup are separate states; unresolved remote private files remain
blocking on every resume. **Suggested change:** durably record the cleanup host
and paths before upload, clear that pending state only after confirmed cleanup,
and require recovery before successful resume, without resubmitting settlement.

**Reproduction:** use `setup_script("shield", ..., VALID, 0)` from
`tests/test_certified_script_results.py`; make only the SSH cleanup containing
`shred -u --` raise. The first call raises, the journal has `round_ok=true`, and
a second call with SSH/subprocess forbidden returns **0**. This was reproduced
with synthetic files and no wallet, signer or remote connection.

### R2 — P2 — verified route resume fails after ordinary chain progress

**Location:** `scripts/pfeth_route_activation.py:55–62`, `:80–83`.
**Condition:** activation was verified at height 42 and all six validators later
reach height 43 with the same selected profile. **Observed:** every resumed
status is rejected because the helper demands equality to the historical
activation height; the command returns **1**. Unlike the old immediate-success
resume, the new resume can never recover as the chain keeps advancing.
**Expected:** distinguish historical accepted activation from current fleet
progress; later heights alone are neither activation proof nor failure.
**Suggested change:** retain the exact accepted receipt, verify its history and
current route/profile, and allow tips at or above its height after that binding.
Cover lagging, equal, advanced and wrong-profile cases without resubmission.

**Reproduction:** complete `setup_script("route_activation", ..., VALID, 0)`,
then resume with all six mocked status results at 43. Six reads occur, no runner
is invoked, and the result is **1** despite the valid height-42 journal.

### R3 — P2 — private egress still retries uncertain or already accepted attempts

**Location:** `scripts/pfeth_private_egress.py:87–92`, `:140–152`, `:180–189`.
**Condition:** the runner times out before `round_rc` is saved, or interruption
occurs after saving a valid result at line 187 but before saving `round_ok`.
**Observed:** there is no pre-run attempt marker. The timeout journal passes the
resume guards; a valid saved receipt also passes, but missing `round_ok` selects
the new-round branch. **Expected:** uncertainty blocks retry; an accepted stored
receipt determines completion without relying on a separately saved boolean.
**Suggested change:** reuse `verified_round_in_journal`, persist an attempt before
launch, and publish the complete result atomically with durability appropriate
to the claimed recovery guarantee. Preserve cleanup recovery separately.

**Reproduction:** start with synthetic `p_i`/sponsorship/shield state and mock all
SSH, subprocess, RPC and key-derivation surfaces. Inject (1) runner timeout and
(2) an exception immediately after persisting the accepted `round_report` but
before `round_ok`. In **both** cases, resume reaches `tip_and_proposer` in the
new submission branch instead of demanding reconciliation. The probe stops
there; it does not claim an actual second on-chain spend. The remote existing
work-directory check can stop a retry at the old height, but a later height
selects a fresh path and is not an outcome-reconciliation mechanism.

## Verification and three publication gates

The exact four-file test invocation from the repair record was rerun in the
StakeHub checkout:

```bash
PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m pytest -p no:cacheprovider -q \
  tests/test_cli_transfer_authority.py tests/test_operational_safety.py \
  tests/test_review_safety_regressions.py tests/test_certified_script_results.py
```

| Test file | Passed |
| --- | ---: |
| `test_cli_transfer_authority.py` | 46 |
| `test_operational_safety.py` | 39 |
| `test_review_safety_regressions.py` | 19 |
| `test_certified_script_results.py` | 34 |
| **Total** | **138** |

Result: **138 passed, 0 failed, 0 skipped, 2 dependency deprecation warnings**
in 1.73 seconds. Per-file counts were confirmed by collection without rerunning
the tests. Four additional synthetic fault probes reproduced R1, R2 and both R3
cases; these are defect reproductions, not extra passing regression tests.

StakeHub `PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m mkdocs build --strict
--site-dir /tmp/stakehub-safety-docs-20260922` and `git diff --check` also passed.
Tests, probes and documentation build ran under Bubblewrap with networking
unshared, the checkout/root mounted read-only, and disposable writable `/tmp`.
An initial sandbox launch could not resolve the existing interpreter's `/tmp`
symlink; exposing that existing runtime read-only fixed the harness setup.
No dependency installation or StakeHub file edit was needed.

Before/after comparison covers `git status --porcelain=v1`, the complete tracked
diff hash, HEAD, and hashes of the helper, all four tests and repair record.
All remained identical. No operator wallet/key/funds, live endpoint or fleet was
accessed. No StakeHub commit, fetch, push, rebase or remote write occurred.
No full Rust, Orchard, archive-history or live test suite was run; this review
changes only a document in the L1 repository.

All three required gates passed with networking disabled. The review document
was staged before the tracked-tree secret scan. This repository excludes
`docs/review/` from the built site; the link checker independently covered this
new document and its local references.

| Gate | Command | Result |
| --- | --- | --- |
| Strict documentation | `.venv-docs/bin/mkdocs build --strict --site-dir /tmp/stakehub-review-l1-docs-20260922` | **PASS**, exit 0 |
| Public document links | `scripts/public-doc-links` | **PASS**, 417 documents |
| Public secret scan | `scripts/public-secret-scan` | **PASS**, tracked-tree mode |

## Real-money disposition

**Do not put real money through this branch.** Ten named findings have scoped
source closure, while the P1 cleanup boundary and P2 archive decision remain
partial. The green regression set does not cover the four reproduced recovery
cases and does not establish custody, release or live-chain correctness.

Before live funds are considered:

1. Repair R1 and R3 and add failure/resume regressions, including durable pending
   cleanup and uncertain-attempt handling. Repair R2 without replacing exact
   activation evidence with a height-only rule.
2. Retire or replace the legacy validator-hosted wallet/note paths with a reviewed
   local-signing/local-proof custody boundary. Cleanup alone is insufficient;
   any exception requires a separate explicit custody decision and qualification.
3. Record SH-08's manager retention/publication decision and verify SH-07's exact
   external evidence before claiming a fully verified recovery bundle.
4. Have the StakeHub owner commit and publish the intended repair set, including
   its currently untracked helper/tests/record, then repeat the focused checks
   and review on that exact commit. This review publishes no StakeHub changes.
5. Separately qualify the intended release, runner, chain/genesis, active
   route/profile, RPC trust and recovery procedure; obtain operational approval
   for that exact configuration. The L1 README still classifies its
   controlled-devnet configuration as unsuitable for real-value keys or value.

This is a completed offline review with a Task Node skip, not an operational
qualification or a rewarded Task Node task. Implementation stops at the findings.
