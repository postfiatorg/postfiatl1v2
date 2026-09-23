# Combined devnet deployment preparation — 2026-09-23

**Unsigned preparation only. Deploy candidate; supersedes the plan to deploy
[combined-devnet-20260921](../combined-devnet-20260921/README.md) first, which
is retained unchanged as the fallback release.** Nothing was deployed, copied
to a host, restarted, signed, or changed on any validator.

Release: `combined-devnet-20260923`. Qualified source tip (qualification packet):
`d75356f6cf1a1686cd129805ae9e3b1414341000`.
Executable build source: `1a0989ad6c35b7ea3eb6418f864958563233945a`
(runtime build revision `1a0989ad`).
Executable SHA-256:
`e7bb1afa17b4c6322ac8eadabdab570595ad778a5173a5516c09ba966ed9e4b1`.
Previous (rollback) release: `a666-source-route-20260907`, executable
`57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83`, RPC build `707e006f`.

## Qualification and CI

The [September 22 qualification](../release-repair-20260922/README.md) passed
after its repair follow-up; two clean builds produced the identical executable
([build records](../release-repair-20260922/node-builds.json)). It is accepted
without re-running. Release-branch CI is green at `d75356f6` and `7f64c12a`
(rust-ci, product-security-ci, docs-build, arc-proof-identities). That closes
the packet's `DEFERRED_TO_CI` / `LEFT TO CI` rows: node-fastpay and the full
workspace test suite.

The staged executable was **verified, not rebuilt**: `binaries/candidate-1`,
`binaries/candidate-2`, `target-1/release/postfiat-node` and
`target-2/release/postfiat-node` under `~/.cache/qualify-fix-20260922` all
hash to `e7bb1afa…` on 2026-09-23.

## Contents

- [RELEASE-ID.txt](RELEASE-ID.txt): release, executable, source, rollback and fallback identities.
- `rootfs/etc/`: 12 canonical units, 12 environment files, six runtime bindings,
  topology and both circuit metadata files.
- `stage-report.template.json`, `validator-bindings.signing.template.json`:
  generated files with the local stage prefix replaced by `@STAGE@`.
- [manifest-input.unsigned.json](manifest-input.unsigned.json): reviewed hashes and
  signing parameters; not a deployment manifest.
- `operator/`: verifier-only operator reference unit/environment (not installed).
- [prepare-stage.py](prepare-stage.py), [local-preflight.py](local-preflight.py)
  ([result](local-preflight.json)), [observe-fleet.py](observe-fleet.py),
  [sign-manifest.py](sign-manifest.py): thin wrappers that execute the reviewed
  20260921 scripts unchanged with this packet's identities (see below).
- [rollback-one.sh](rollback-one.sh): conditional pre-activation recovery.
- [SIGNING.md](SIGNING.md): both signing paths, exact commands and outputs.
- [PREFLIGHT.md](PREFLIGHT.md): what ran locally and what is blocked.
- [DEPLOY-SHEET.md](DEPLOY-SHEET.md): deployment-day commands and recovery.
- [publication-gates.json](publication-gates.json): gate commands and exits.
- `observed/`: the previous-release identity used by the observer (byte copy of
  the 20260921 file) and today's validator-0 re-check.

## What changed from 20260921

Only the executable and the release ID. Generating with the new executable and
the same topology/circuit metadata produced 33 configuration files that equal
the 20260921 inputs byte-for-byte after substituting the release ID. Operator
reference files differ the same way. Topology, both circuit metadata files,
the inventory, and the trusted publisher file hash
(`66304dfca0b5893b156eb78e10d65a7b788c262043e85f26eadc82ea86a0314a`) are unchanged.
The manifest input changes only release, source, revision, executable hash,
and the release-ID-dependent unit/environment hashes.

The Python helpers are wrappers, not forks: each executes the file of the same
name in `../combined-devnet-20260921/` with `__file__` pointing here, so all
paths and identities come from this packet. `prepare-stage.py` overrides only
the release ID and executable hash, read from the manifest input.
`local-preflight.py` uses `~/.cache/deploy-prep-20260923/tmp` as scratch.
`rollback-one.sh` runs over `ssh bash -s`, so it is a copy; it differs only in
its rollback-anchor paths (`/var/lib/postfiat/rollback/combined-devnet-20260923/`).
Keep the 20260921 directory while this packet is in use.

## Fleet notes

Layout, units, auxiliary services and redaction follow the
[20260921 README](../combined-devnet-20260921/README.md#layout-and-redaction).
A read-only re-check of validator-0 on 2026-09-23T09:46:47Z matched the recorded
observation exactly ([record](observed/validator-0-recheck.json)).

The 2026-09-11 fleet-wide service restart was Ubuntu's unattended security
upgrade (glibc/python) plus needrestart, recorded in
`docs/status/chain-state-current.md` on main (`b8b5130c`). The
`apt-daily-upgrade` timer runs on each host daily at a random time between about
06:00 and 07:00Z. Do not start the rollout inside that window; the deploy sheet
checks the timer on every host first.

## Redaction

No key material or secret values are committed. The deployment public file is
pinned by hash and kept in `~/.cache/deploy-prep-20260921/observed/`. Key and
API-credential paths, signing times and `@STAGE@` are operator placeholders.
The local stage is `~/.cache/deploy-prep-20260923/validator-stage`; a second
regeneration is under `~/.cache/deploy-prep-20260923/regeneration-check/`.
