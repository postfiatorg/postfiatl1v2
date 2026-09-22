# Overnight release and NAVCoin preparation

- **Operator:** Codex (`codex`), recording the user's instructions
- **Date:** 2026-09-22 UTC
- **Recipient:** Domagoj Ravlić and the next execution session

## BLUF

The user is going to bed and cannot set up or fund StakeHub on this machine
today. Continue useful release and NAVCoin work without depending on that
setup. The user wants the qualified release to proceed and StakeHub bugs fixed
before real money. The current publisher key has not been located; investigate
it for up to 30 minutes, then continue the independent work below if still
blocked. The whitepaper decision is settled: keep the higher-scoring version.
The unanswered AI-governance proposal does not block tonight's preparation.

This handoff records priorities and known results. It does not claim a
signature, deployment, funded wallet, or adoption of an unanswered policy.

## Current state

- On `hetnzerxxx`, L1 `main` is `2a3edc36`; the observed remote release
  branch is `048d23df`. The deployable candidate remains qualified tip
  `a5b1e757`, build revision `03e422a7`, executable SHA-256
  `051ad12ce22c33f2370388259d754a9c1016a8edc1b52db7d2852fe78f6fbc7d`.
  The later NAV-03 and SWX-01 fixes change execution and need their own
  qualification. Do not substitute the later branch tip for this executable.
- September 22 local preflight passed for
  `~/.cache/deploy-prep-20260921/validator-stage`. The manifest is absent.
  The trusted public-file SHA-256 is
  `66304dfca0b5893b156eb78e10d65a7b788c262043e85f26eadc82ea86a0314a`.
  The older public key at
  `~/.postfiat/deployments/cobalt-handoff-9c420ea2/keys/deployment.public.json`
  does not match. No private key was read or signature attempted.
- “Your machine” is unverified. The earlier integration map names
  `postfiatfoundationv2`, home `/home/postfiat`, as another work host.
  That is a lead, not an established key location. `/secure/` in SIGNING.md
  is a placeholder.
- The original StakeHub safety patch and the September 22 follow-up repairs
  are local and unpublished at
  `/home/postfiatchad/repos/StakeHub-safety-20260907`, branch
  `fix/pr8-safety-20260907`, base `8f6f27cf`.
  The original repairs remove the transfer-policy bypass, require explicit
  confirmation for live scripts, and attempt private-file cleanup on failures.
  Legacy remote custody still has documented limits; cleanup alone does not
  qualify those paths for real money.
  Follow-up repairs retire the unsafe fleet-copy script and require exact
  accepted runner results for shielding and route activation, including
  reconciliation after uncertain attempts. Four focused test files pass
  **138 tests**; strict MkDocs and `git diff --check` pass.
  See that checkout's `docs/review/pr8-safety-repairs-20260907.md`.
- No fleet contact, restart, deployment, wallet access or funds movement
  occurred in this session. The September 21 handoff cites the September 17
  all-six observation at height 1020, release `a666-source-route-20260907`,
  binary `57b0f4d1…634eec83`. That is historical, not tonight's fleet state.

## Next decision or action

Work in this order; an unavailable key must not leave the rest idle. Follow
the repository's Task Node requirements for substantial execution work.

1. **Trace the current publisher's custody.** Use existing release records,
   prior signing commands and already-authorized workstation access. Identify
   a concrete hostname/path and compare its public identity with the trusted
   release file before signing. Keep private keys on their trusted host.
   Do not generate a replacement key, change validator trust, or use the
   older mismatched key. If custody remains unknown, record exactly what
   was checked and move to step 2. The user cannot supply it from memory.

   If the correct key is available to the authorized operator, follow
   `deployments/combined-devnet-20260921/SIGNING.md` on the release branch.
   Any rollout still follows DEPLOY-SHEET.md: fresh fleet checks, verified
   signed canary backup, verified per-host rollback copies, then one machine
   at a time. Prepared rollback instructions do not prove those copies exist.

2. **Prepare the first NAVCoin cycle without funding StakeHub.** Continue the
   September 21 Z3 dry-run record: 39/39 commands resolve; 17 execution inputs
   and signer/prover bindings remain. Resolve what existing artifacts and
   authorized read-only observations can establish. Label every observation
   with its release/block and collect the remaining bindings and exact next
   commands. Values that require the new deployment must be read again after
   deployment. Do not manufacture missing signer files or treat historical
   observations as current. Tonight's work stops before a live value-moving
   cycle; the user has deferred setup and funding.

3. **Advance the next release's qualification if deployment is still held.**
   This is already requested in Domagoj's September 21 handoff. Qualify the
   candidate carrying NAV-03, SWX-01 and the other burn-6 repairs separately.
   Reuse the existing qualification procedure and retain exact candidate
   identities and results. Do not repeat the already-green old-build suite
   merely to occupy the night or alter its signed inputs.

4. **Review the saved StakeHub repairs offline.** Review SH-10/11/12 against
   the September 10 findings and preserve the original safety protections.
   The test command and limitations are in the local repair record.
   Funding and wallet setup are not prerequisites for this review.
   If the review host lacks the worktree, obtain a reviewable patch through
   existing authorized access; do not review the old Git head as though it
   contains the local changes. Commit, publication and integration status
   must remain explicit.

The user reaffirmed the L1 whitepaper score rule: a lower score signals that
the revision may be unsound. Keep the existing 87.13 version; do not promote
the 85.47 candidate or request an exception merely because some edits are
described as factual corrections. That candidate also changed the consensus
description. Inspect the actual reviewer objections and source support before
proposing further changes; apply the existing improvement requirement before
publication. This resolves the requested score-rule exception: no exception.
Whitepaper investigation does not block the overnight release work.

The AI-governance direction remains unadopted; leave its current behavior
unchanged. Missing height-915/924 artifacts and the September 11 restart actor
remain unresolved facts. Investigate from available records without inventing
an owner, authorization or result.

## Morning handoff

Report completed artifacts and evidence, not another undifferentiated blocker
list: signature/deployment status; exact key-custody findings if still blocked;
NAVCoin inputs resolved and still missing; next-release qualification results;
and StakeHub review findings. Ask the user only for a specific remaining action
that could not be resolved from accessible records.

## References

- [Domagoj's September 21 handoff](2026-09-21___dravlic__deploy_prepared_and_navcoin_audit.md)
- [Z3 dry-run and remaining inputs](../status/z3-dry-run-20260921.md)
- [StakeHub follow-up review](../review/stakehub-fix-branch-review-20260910.md)
- [Current fleet record and observation boundaries](../status/chain-state-current.md)
- Release branch: `deployments/combined-devnet-20260921/SIGNING.md` and
  `DEPLOY-SHEET.md`; local packet available at
  `/tmp/burn6-20260921/deployments/combined-devnet-20260921/`.
- Local signing check:
  `~/.cache/deploy-prep-20260921/signing-readiness-20260922.json`.
- Machine map: `/home/postfiatchad/INTEGRATION-START-HERE.md`.
