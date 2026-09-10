# StakeHub safety-fix branch review — 2026-09-10

Status: read-only review; no StakeHub change or publication authorized

Reviewed branch: `fix/pr8-safety-20260907` at committed head `8f6f27cf`, plus
its pre-existing uncommitted safety repair set. The checkout was not changed,
committed, fetched, or pushed. Findings are recorded in this repository only.

## Scope and method

This fresh-eyes pass checked the proposed dispositions of the nine findings in
the [2026-09-07 PR #8 review](stakehub-pr8-review-20260907.md), then traced the
new live-intent boundary, remote-private-file cleanup, certified-batch result
checks, release application, policy-enforced transfer commands, external RPC
selection, and venue amount serialization. It reviewed the actual dirty diff,
not a published branch revision.

## Findings

### 1. P1 — Release reuse can overwrite an active release before promotion

**Location:** StakeHub `scripts/pftl_fleet_release_apply.py:48-74` in the local
repair set.

The applicator validates the release identifier and binds every source hash to
typed intent, but it does not require the remote release directory to be new or
different from the running release. Configuration files are copied directly to
their final paths before the aggregate remote-hash check; only the binary and
unit files use `.incoming` staging.

**Concrete failure scenario:** an operator accidentally supplies the identifier
of the currently running release with a newly built stage directory. The script
copies a prefix of the new configuration into the live release directory. A
later copy or hash check fails, so promotion and restart do not occur, but a
subsequent service restart consumes the partially replaced configuration. The
typed digest proves what was selected; it does not prevent in-place reuse or
restore the old files.

### 2. P2 — Shielding records success from an unbound runner flag

**Location:** StakeHub `scripts/pfeth_shield.py:123-137` in the local repair set.

The shielding path stores `round_ok` from the runner's JSON without requiring a
zero process exit, accepted `local_apply_verified`, the expected height and
proposer, or a nonempty certificate identifier. Its resume path returns success
immediately whenever that single stored boolean is true. The same repair set
introduces `certified_batch_succeeded` and applies it to private egress, showing
that the stronger causal receipt is available but unused here.

**Concrete failure scenario:** the runner emits `{"round_ok": true}` but exits
nonzero after failing local application, or emits that flag for a report whose
height/proposer differs from the intended batch. The script records success;
future invocations return zero before checking public balance or the exact
certificate. A locally copied note opening can then be treated as the product
of a shield operation that was never established.

### 3. P2 — Route activation can confuse fleet height with exact batch success

**Location:** StakeHub `scripts/pfeth_route_activation.py:148-179` in the local
repair set.

The governance route path likewise accepts only `report.round_ok`, ignores the
runner return code and exact certified-batch fields, and then checks that all
validators reached the target height. Height agreement establishes neither the
contents of that block nor the application of this route amendment. A stored
`round_ok` also short-circuits every later invocation.

**Concrete failure scenario:** this runner fails or returns a report not bound
to the intended governance batch while another valid round fills the same next
height. All six height reads equal the target, so the script exits zero and its
journal permanently says activated without reading back the exact route/profile
commitment. The later block is healthy but is not evidence for this amendment.

## Verified dispositions and retained limits

- The direct-signer fallback is removed: malformed, unavailable, locked, and
  policy-denied agent responses fail closed.
- Bare invocation of the reviewed mutation scripts exits before configuration,
  key, file, or network access. The unsafe attestation downgrade script remains
  disabled even with `--live`.
- The three remote-key/note paths register cleanup before upload and make
  cleanup uncertainty visible. SIGKILL, partitions, snapshots, and compromised
  hosts remain explicit custody limits, not solved properties.
- Private egress uses the exact certified-batch check and exact asset identity;
  the two other batch paths identified above do not.
- NAVCoin deposit requires an explicit credential-free RPC URL, enforces HTTPS
  away from loopback, checks chain ID, and records endpoint and genesis
  identity. That remains operator-selected RPC evidence, not independent
  Ethereum consensus proof.
- Irreversible Hyperliquid referrer registration is disabled. Venue withdrawals
  reject amounts that do not round-trip through the installed SDK path.
- The recovery archive remains cross-repository and publication remains a
  manager decision; this review neither changes nor republishes it.

## Read-only verification

`PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m pytest -p no:cacheprovider -q
tests/test_cli_transfer_authority.py tests/test_operational_safety.py
tests/test_review_safety_regressions.py` passed **105 tests** with two dependency
deprecation warnings. `git status --porcelain=v1` was identical before and after
the run. The green set exercises the proposed fixes but has no regression for
the three findings above. No live endpoint, wallet, validator, or chain was
used.

## Disposition

The local repair set materially improves PR #8 but is not ready to publish as a
complete safety closeout. Findings 1–3 remain open in the read-only branch.
They require repair and focused regressions in the StakeHub lane before any
merge or operational qualification; this campaign will not make those changes.
