# Open-source secret-history classification — 2026-07-16

This is the value-redacted classification of the full-history Gitleaks report
used by the productionization review. It records rule, field/path class, and
disposition without reproducing or hashing a credential value.

## Scope and result

- Git commits scanned: `2288`
- Input bytes scanned: `258.68 MB`
- Findings: `719`
- Files: `233`
- Commits containing findings: `38`
- Rules fired: `generic-api-key` only
- Provider-specific, PEM/private-key rules: `0`

| Classification | Findings | Evidence-backed disposition |
|---|---:|---|
| Public EVM/token/pool identifiers | 660 | Public contract/token addresses, pool-key hashes, and token-in/out configuration. These are protocol/public-chain identifiers, not authentication secrets. Raw evidence files containing most repetitions are nevertheless excluded from the source tree. |
| Test/schema/fixture labels | 43 | Hypothesis-map `key` fields, explicit idempotency test keys, already-redacted negative fixtures, figure-map labels, and the canonical XRPL genesis benchmark secret. The XRPL value is the publicly documented genesis test account credential and existed only in removed comparison-benchmark scripts; it is not a PostFiat key. |
| Public verification/hash material | 13 | Randomized verification keys serialized as public proof inputs, SP1 verifying-key constants, and a genesis-hash source field. None grants proving, signing, wallet, validator, or provider authority. |
| Real credential | 3 | The same captured cloud-instance Jupyter token in three historical provider-response files. This is `P0-SECRET-01`: revoke/decommission with the provider and exclude all contaminated refs from public history. |

Total: `660 + 43 + 13 + 3 = 719`.

## Field-class trace

The 660 public-chain findings are dominated by `wrapped_navcoin_token` (364),
`token_address` (86), `uniswap_pool_key_hash` (38), legacy/output/USDC token
fields (109 combined), token-in/token-out (40), and named token-messenger or
environment constants (23). The 43 fixture findings are `key` (28),
`idempotency_key` (6), `MASTER_SECRET` (4), already-redacted negative fixtures
(4), and `figures` (1). Public verification material comprises
`randomized_verification_key` (9), SP1 verifying-key fields (3), and one
genesis-hash source field.

No broad value or directory allowlist follows from this classification. The
candidate uses narrow current-tree rules, excludes raw evidence, and must pass a
fresh full-ref scan after sanitized public history is constructed. The three
credential locations remain blocking regardless of whether the provider later
reports the token expired.

## First-party privacy/credential scanner reconciliation

Gitleaks' `generic-api-key` rule does not recognize Orchard note-opening field
names. After the publication scanner added the value-redacted
`private-note-opening` rule, `scripts/public-secret-scan --history` produced the
expected nonzero private-history baseline of 27 findings:

- three `jupyter-token` occurrences, matching the real-credential class above;
- 24 `private-note-opening` field occurrences across seven removed ingress
  evidence artifacts: six fields in one legacy ingress response and three
  fields in each of six private-swap batch/deferred-send captures.

The two reports are complementary rather than contradictory. The 719-row table
above remains the complete Gitleaks classification; the first-party scanner
adds a privacy-material class that Gitleaks did not detect. Neither class is in
the current candidate tree. Both must be absent from every ref in the sanitized
staging history, and neither count is an allowlist.
