# Proof inventory safety review — 2026-09-07

Task Node: `task_bdb4280daf0d3699d7382b650b81e719`.
Reviewed checkout: `d351353e57b295368450a57866ace17b5e1ce6ad` plus the local
repairs below. No commit, push, deployment, signing, or live-chain action.

## Disposition of the eleven reported mismatches

Each reviewed baseline below was located by matching the inventory's old
SHA-256 to exact Git file bytes, not merely by choosing an earlier commit.
Paths under `tools/nav-reserve-proof/crates/` are shortened in the table.

| Source | Reviewed baseline | Disposition and reason |
| --- | --- | --- |
| `scripts/check-nav-reserve-proof-fuzz-smoke` | `f2aaa298e002` | Repin the corpus relocation, then strengthen lockfile discipline as described below. All ten targets and their bounds remain unchanged. |
| `reserve-proof-cli/src/manifest_builder.rs` | `b11549f57dcc` | Repin: test-only historical EVM witness path relocation; no manifest validation changes. |
| `reserve-proof-cli/src/evm_adapter.rs` | `b11549f57dcc` | Repin: adds an explicitly invoked YOLO disclosure-commitment command using bounded JSON input and validated, domain-separated commitments. It does not submit, sign, verify reserves, or upgrade attested evidence to cryptographic evidence. |
| `reserve-proof-cli/src/hyperliquid_adapter.rs` | `b11549f57dcc` | Repin: test-only historical witness path relocation. |
| `reserve-proof-cli/src/near_adapter.rs` | `b11549f57dcc` | Repin: test-only historical witness path relocation. |
| `reserve-proof-types/src/lib.rs` | `a3649dbd1cf4` | Repin: seven YOLO module exports; the reserve guest still calls the unchanged reserve execution function, not the separate YOLO target-proof pipeline. No reserve evidence dispatch, trust classification, or ABI changed. |
| `reserve-proof-types/src/aave_v3.rs` | `b11549f57dcc` | Repin: test-only historical witness path relocation. |
| `reserve-proof-types/src/evm_spot.rs` | `aef4ce80382d` | Repin: test-only historical witness path relocation. |
| `reserve-proof-types/src/hyperliquid_receipt.rs` | `b11549f57dcc` | Repin: test-only historical witness path relocation. |
| `reserve-proof-types/src/near_receipt.rs` | `b11549f57dcc` | Repin: test-only historical witness path relocation. |
| `reserve-proof-types/src/solana_stake.rs` | `89a4b76f964e` | Repin: test-only historical witness path relocation. |

All five relocated witnesses are byte-identical to their original files at
`b11549f57dcc`; their hashes also match
[the retained fixture manifest](../../benchmarks/nav-reserve-proof-historical/README.md).
No witness, assertion, fuzz target, or verification equation was substituted.

The reserve consumer was traced through
`crates/execution/src/nav_sp1_verifier.rs`: bounded Groth16 verification precedes
584-byte public-value decoding and the existing genesis, asset, profile,
policy, manifest, epoch, freshness, value, source-root, attestor-root, and
controlled-source checks. Those functions and the ABI are unchanged.

## Additional issues found while executing the gates

- Fuzzing exposed a stale nested `fuzz/Cargo.lock`: Cargo silently added the
  already-required YOLO certificate/crypto dependencies. The 17 added registry
  packages match the reserve workspace lockfile's exact versions and checksums;
  no existing package was removed or upgraded. The two local-package dependency
  lists now match their manifests. The reviewed lockfile is repinned. The fuzz
  wrapper now checks resolution with `cargo metadata --locked` and rejects any
  subsequent lockfile mutation; all original fuzz targets, budgets and bounds
  remain intact.
- Added a source pin for the reviewed `yolo_broker.rs` dependency of the new
  CLI command, plus negative regressions for bad schema, identity, time, digest,
  valuation scale and liability totals. Its end-to-end reserve test still
  classifies the value as attested, not cryptographically verified.
- The next two checks had been hidden by the inventory failure. The readiness
  checker referenced a deleted plan; a concise
  [current safety record](../plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md)
  restores the unchanged open gates and links the complete original requirements.
  No readiness-check assertion was changed.
- The source-qualification checker referenced the retired evidence tree. It now
  reads the complete packet from immutable commit
  `6edf57f84fc78bada1f108f8c12a4a59277ce34a` only when the whole working-tree
  packet is absent. A partial or corrupt current packet never falls back.
  Every existing cross-binding, size and SHA-256 check remains; duplicate and
  path-substituted proof artifacts are additionally rejected. Output explicitly
  labels `git-archive` evidence. No proof files are recreated or added to Git.
  Python CI now fetches full history for these archive regressions.

## Verification

- Before: `scripts/test-proof-public-input-inventory` exited 1; eleven mismatches.
- After: the same command passes: `systems=5 public_fields=70 source_hashes=86`.
- The complete `public-tree-hygiene` job's commands pass locally, including both
  downstream A666 checks, secret/runtime-default scans and redaction regressions.
- `cargo test --manifest-path tools/nav-reserve-proof/Cargo.toml --locked -p reserve-proof-types -p postfiat-reserve-proof`:
  **125 passed, 3 existing ignored SP1 tests**. No ignore was added.
- `python3 -m pytest -q python/tests/test_a666_public_source_qualification.py python/tests/test_a666_public_adapter_readiness.py`:
  **20 passed**, including eight packet-tampering cases and missing/partial archive cases.
- `scripts/check-nav-reserve-proof-fuzz-smoke`: all **10 targets** completed
  both before and after the lock discipline fix, at the unchanged default
  10-second per-target budget; final run did not mutate the lockfile.
  This host needed a local, ignored installation of the Ubuntu `jq` packages.
- Reserve-workspace formatting, strict MkDocs build and `git diff --check`: pass.

Generated logs and the reviewed before/after source hashes are under the ignored
`.tih/safety-proof*` and `.tih/proof-reviewed-pins.json` paths. The working-tree
diff is the patch evidence; no published commit is claimed.

## Limits

This is a scoped source/inventory and local regression review, not an independent
cryptography audit or full inventory of the separate YOLO target-proof system.
Repinning does not reproduce or authorize a new guest ELF/program key. Historical
qualification records are checked for integrity, not regenerated or freshly
cryptographically reverified by the archive checker. A666 remains **0/6
production-qualified** and StakeHub is **not deprecated**. Orchard and the full
L1 Rust workspace were not rerun because no Orchard or L1 consensus behavior was
changed. GitHub remains unmodified; green local checks are not a green remote run.
