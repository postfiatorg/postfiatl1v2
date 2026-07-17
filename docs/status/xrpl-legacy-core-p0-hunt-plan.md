# XRPL Legacy-Core P0 Hunt Plan

Status: active investigation plan
Scope: live XRPL mainnet-enabled behavior only
Runtime target: one focused 8-hour pass
Primary goal: find old, live, unfixed XRPL state-transition bugs that materially strengthen the "do not inherit RippleD blindly" case.

## Urgency Overlay

This hunt is now part of the fundraising evidence spine, not an open-ended
security-research hobby. The goal is to produce compact proof that inheriting
`rippled` is strategically wrong for PostFiat.

High-value output:

- old-core, live, unfixed, reproducible defects;
- old-tag binary span that shows multi-year exposure;
- plain-English invariant break;
- packet-bound proof logs and hashes;
- clean negative inventory where a surface was seriously tested and did not
  reproduce.

Low-value output:

- already-fixed future-release issues unless they prove long-lived inherited
  risk;
- disputed product-semantics claims;
- disabled-surface findings;
- broad article edits;
- more raw candidate lists without minimized repros.

The next agent should bias for speed: package the `1.5.0` proof, then search
for sibling reserve/accounting bugs. If the work stops moving the fundraising
case, stop and write a concise negative inventory.

## Current Signal

The strongest current lead is `TRUSTLINE-POSITIVE-BALANCE-RESERVE-001`.

Observed behavior: an account can end with a positive IOU trustline balance
through offer crossing while `OwnerCount` remains `0` and the receiver reserve
flag remains unset.

Why this matters: this is old core reserve/accounting behavior, not a new
amendment-only surface. It has already reproduced on the current `3.1.3`
packet target, on `2.5.0`, on `2.0.0`, and was observed passing on `1.5.0`
inside an Ubuntu 20.04 Docker build. The `1.5.0` result still needs to be
saved, hashed, verifier-bound, committed, and the temporary worktree cleaned.

This lead should guide the next search. The next eight hours should hunt for
sibling defects in the same old ledger-effect region: trustline lifecycle,
offer crossing, reserve flags, owner counts, directories, freeze/auth receive
gates, and deterministic result-code hygiene.

## Thesis

The highest-value finding is not another policy-edge clone. The highest-value
finding is an old, boring, live, unfixed state-transition bug in core XRPL
behavior: reserve accounting, trustline lifecycle, authorization/freeze,
deterministic arithmetic, or object ownership.

If such a bug exists, it supports a stronger claim than "new amendments are
messy." It supports the claim that RippleD spreads ledger-effect policy and
accounting across too many transaction-family call sites, making inheritance
expensive and error-prone.

## Hard Constraints

Only promote a candidate if all of the following are true:

1. The surface is live on XRPL mainnet, verified through direct XRPL JSON-RPC
   or raw on-ledger amendment state.
2. The behavior reproduces in an upstream local `rippled` harness.
3. The candidate has a minimal proof marker or equivalent deterministic
   assertion.
4. The candidate is not already confirmed remediated in the checked
   beta/develop refs.
5. The candidate is materially stronger than another
   DisallowIncoming/DepositAuth semantic edge.
6. The candidate can be explained as state safety, accounting integrity,
   authorization integrity, or deterministic consensus hygiene.

Do not update the public article from this plan. Update packet, repros, and
triage only after verifier/repro checks pass.

## Eight-Hour Execution Plan

### Phase 0: Package The 1.5.0 Proof, Then Clean Up

Timebox: 45 minutes.

Tasks:

1. Preserve the `1.5.0` trustline repro patch and proof log.
2. Record the tag commit, tag date, Docker image, build flags, marker, and
   result count.
3. Update packet triage only if the proof artifact is complete.
4. Run packet verifier and the trustline repro wrapper.
5. Commit the packet slice.
6. Clean the temporary `rippled-1.5.0` worktree back to a non-misleading state.

Do not continue the hunt until this is either committed or explicitly
source-killed. A passing terminal result that is not packet-bound is not enough.

### Phase 1: Trustline/Offer Reserve Siblings

Timebox: 2 hours.

Target question: are there other old paths that move a trustline from
non-positive to positive, or from default to non-default, without the matching
reserve/owner-count transition?

Probe queue:

1. Path payment/rippling creates positive balance after the receiver has cleared
   its limit and reserve flags.
2. `CheckCash` auto-creates or resurrects a trustline into positive balance
   without the receiver-side reserve transition.
3. Offer crossing with transfer rate, quality-in/out, and partial crossing at
   reserve boundaries leaves positive balance or owner-count drift.
4. Trustline default-state deletion/resurrection clears accounting state, then
   an alternate settlement path restores economic balance without reserve.
5. Direct `TrustSet` and direct `Payment` controls reject or charge reserve
   while the alternate path succeeds.

Promote only if the post-state is impossible under the reserve model:
positive balance, owned object, directory entry, or non-default trustline state
without the expected owner/reserve accounting.

### Phase 2: Old Object Lifecycle And Directory State

Timebox: 90 minutes.

Target question: do old object create/delete paths leave partial objects,
owner-directory residue, or owner-count drift under normal failure conditions?

Probe queue:

1. `CreateCheck` directory-full and two-owner-directory failure paths.
2. `PayChan` close/claim/delete around recipient owner directory migration.
3. XRP `Escrow` cancel/finish/delete around account deletion and owner counts.
4. Ticket-paid create paths that might derive object keys from raw `sfSequence`
   instead of the sequence proxy.

Promote only if a live transaction can strand an object, leak a directory
entry, leave owner count wrong, collide object keys, or reach an internal
exception from normal input.

### Phase 3: Authorization/Freeze Receive-Path Siblings

Timebox: 90 minutes.

Target question: do old receive paths enforce `RequireAuth`, freeze, clawback,
and trustline flags consistently at the ledger-effect boundary?

Probe queue:

1. Direct payment control versus path payment, offer crossing, check cash, and
   escrow/check deferred settlement.
2. Local freeze and global freeze controls on IOU receive, send, and offer
   crossing.
3. RequireAuth trustline authorization on direct and indirect receive paths.
4. Clawback/deep-freeze interactions only where the surface is live by direct
   RPC.

Demote pure product-semantics disputes. Promote only if the same ledger effect
is rejected on a direct path but accepted through an indirect path and creates
durable value/state movement.

### Phase 4: Deterministic Exception And Arithmetic Sweep

Timebox: 75 minutes.

Target question: can normal transaction-shaped input on a live surface reach
`tefINTERNAL`, `tefEXCEPTION`, overflow, assertion, or invariant failure instead
of deterministic rejection?

Probe queue:

1. `STAmount`/quality/transfer-rate boundary values in old IOU and offer paths.
2. Path-payment amount extraction and issue mismatch paths.
3. Offer book crossing with tiny/huge qualities and partial crossing.
4. Owner-directory traversal and object deletion failure paths.

Promote only with a transaction-visible repro and a clean expected/actual result
pair.

### Phase 5: Source-Signal Clustering

Timebox: 45 minutes.

Target question: what other old areas are adjacent to known fix-looking signals?

Use commit history, branch names, and touched files as source signals, not as
evidence. Cluster around:

- `fix-positive-balance-trustline-pay-no-reserve`;
- owner count and reserve edits;
- `rippleCreditIOU`, `accountSend`, `trustCreate`, `trustDelete`, and
  `adjustOwnerCount`;
- directory insert/delete helpers;
- transaction result-code changes in `Payment`, `OfferCreate`, `SetTrust`,
  `CreateCheck`, `CashCheck`, `Escrow`, and `PayChan`.

If a source signal does not produce a repro quickly, write the source-kill note
and move on.

### Phase 6: Packet Hardening

Timebox: final 30 minutes.

For every promoted candidate:

1. Save the patch/log and SHA-256.
2. Add or update the repro wrapper.
3. Update `repro_manifest.json`, packet docs, run triage, and verifier rules.
4. Run packet verifier, JSON formatting check, `git diff --check`, and every
   changed repro wrapper.
5. Commit only after the checks pass.

If no new finding is promoted, commit a concise negative-inventory update only
if it is useful and does not touch the article.

## Triage Labels

- `core-accounting`: reserve, owner-count, directory, or impossible ledger
  state.
- `auth-freeze`: authorization, freeze, lock, clawback, or holder policy.
- `deterministic-exception`: exception, overflow, assertion, or invariant
  failure.
- `policy-semantics`: contested policy meaning, not enough by itself.
- `duplicate-pattern`: same pattern as an existing packet item.
- `not-live`: blocked by disabled amendment or non-mainnet surface.
- `fixed-upstream`: confirmed remediation in checked beta/develop refs.
- `reject`: not reproducible or not material.

## Promotion Bar

A promoted finding needs:

- finding id;
- one-paragraph plain-English summary;
- affected live surface;
- upstream baseline commit;
- local repro script or test target;
- proof marker;
- expected behavior;
- actual behavior;
- upstream remediation status;
- risk category;
- packet verifier update.

## Stop Conditions

Promote immediately if one of these is found:

- old core reserve/accounting drift;
- lock/freeze/auth bypass with durable state or value movement;
- deterministic exception in live consensus/state-transition path;
- deleted object leaves stale ownership, directory, or authorization state.

Stop and report negative inventory if:

- old core surfaces are clean after the planned passes;
- remaining candidates are only DisallowIncoming/DepositAuth semantic clones;
- all high-value candidates are already remediated upstream;
- no candidate can be made reproducible in upstream local harness.

## Expected Output

Best outcome:

- 1-5 new live, unfixed, old-core findings with minimized repros.

Acceptable outcome:

- the `1.5.0` proof is packet-bound and the next old-core surfaces have clean
  negative inventory.

Bad outcome:

- more policy-cluster clones without new state-safety value.
