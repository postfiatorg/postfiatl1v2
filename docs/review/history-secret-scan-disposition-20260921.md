# Reachable-history secret scan disposition — 2026-09-21

The seven `secret-field` findings recorded in
`deployments/combined-release-20260915/logs/candidate-history-secret-scan.log`
are **test-only passphrase fixtures**. No real credential was identified among
these seven findings. This disposition addresses the reachable-history blocker
in the combined-release assessment.

## Classification

Reviewed current source, passphrase changes with `git log -S`, and source at each
introducing commit with `git show`. Historical literals were redacted before
inspection output; no literal values or value digests are reproduced here.
Line numbers below are the historical finding locations.

| Path and historical line | Literal purpose | Introducing commit | Disposition |
| --- | --- | --- | --- |
| `wallet-extension/lib/security-regression.test.mjs:45` | Keystore regression passphrase used to encrypt/decrypt a synthetic seed and test legacy vault compatibility. | `83488d9154f3` | test-only |
| `wallet-web/scripts/live-pnok-private-fix-recovery-faults.mjs:13` | Passphrase for a fresh browser test wallet during pNOK FIX recovery and restart faults. | `c6610c4ec43c` | test-only |
| `wallet-web/scripts/live-pnok-private-fix-campaign.mjs:15` | Passphrase for a fresh browser test wallet during the repeated pNOK FIX qualification campaign. | `64be0cd352aa` | test-only |
| `wallet-web/scripts/live-pnok-private-fix-ux.mjs:10` | Passphrase for a fresh browser test wallet during pNOK FIX UX and reload recovery checks. | `64be0cd352aa` | test-only |
| `wallet-web/scripts/smoke-navcoin-market-registry-ux.mjs:15` | Passphrase for a fresh browser test wallet during the NAVCoin market registry smoke test. | `1db7a8273063` | test-only |
| `wallet-web/scripts/live-a666-metamask-export-ux.mjs:103` | Temporary browser vault encryption passphrase after importing the externally supplied test-wallet seed for export UX checks. | `335bf1c4e69e` | test-only |
| `wallet-web/scripts/verify-live-a666-metamask-balance-ux.mjs:99` | Temporary browser vault encryption passphrase after importing the externally supplied test-wallet seed for balance UX checks. | `335bf1c4e69e` | test-only |

The extension now uses a minimum-length placeholder, introduced by `6a858064`.
The six browser scripts now generate passphrases at runtime. They create fresh,
nonpersistent Playwright browser contexts. In the A666 scripts, the external
keystore password and wallet seed are read separately from operator-supplied
files; the flagged literals only set the temporary browser vault passphrase.

## Allowlist and regression coverage

Added these seven exact `(secret-field, path)` pairs to `TEST_ONLY_ALLOWLIST` in
[scripts/public-secret-scan](../../scripts/public-secret-scan), each with a
one-line reason. Both modes already call `scan_line`, which applies this
allowlist; no history-mode logic change was needed.

[scripts/test-public-secret-scan](../../scripts/test-public-secret-scan) now
commits and deletes synthetic fixtures in an isolated temporary repository. It
checks that history accepts an allowlisted passphrase, rejects the same literal
at a sibling path, and rejects another secret rule at the allowlisted path.
The latter two findings remain detectable after deletion from the tracked tree,
and scanner output must omit their values.

## Validation

Main validation is based on `01eb47a0` with this change applied. The invocations
below select the packet's `tracked-tree` and `history` modes; `--history` scans
all reachable refs, including the fetched release branch.

| Command | Result |
| --- | --- |
| `scripts/public-secret-scan` | PASS, exit 0: `public secret scan passed mode=tracked-tree`; zero findings. |
| `scripts/public-secret-scan --history` | PASS, exit 0: `public secret scan passed mode=history`; zero findings. |
| `scripts/test-public-secret-scan` | PASS, exit 0: `public secret scan regression passed`. |
| `.venv-docs/bin/mkdocs build --strict` | PASS, exit 0. |
| `scripts/public-doc-links` | PASS, exit 0: `public_documentation_links=ok files=412`. |
