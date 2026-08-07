# S1b resolution log

**Status: BINDING MECHANICS VALID; EXECUTION WINDOW EXPIRED/UNSAFE. Do not fire from S1b.**

## Scope and immutable lineage

S1b resolves only the first ten active packets: legs 0, 1, 2a, 2b, 3a, 3b0, 3b, 3c, 3d, and 3e. Receipt-chained successor legs remain deferred by design. The S1 source packet set and `values-S1.json` remain immutable; S1 hash verification is 16/16 and `values-S1.json` remains SHA-256 `9d6c226217c0b72ff881ca5004060492649057098e957ec519c8b6aa88b18a33`.

## R1: base-first packet overlay

The resolver is pointer-set-only: it can replace existing leaves but cannot restore keys removed by prior post-resolution amendments. Therefore no re-render occurred after the initial verified eight-packet set.

- Leg1 was rebuilt from the S1 base, preserving the D1 67-leaf carry. Exactly three fresh leaves were overlaid from rendered S1b: `/resolution_stage`, `/fire_hash_confirmation`, and `/resolved_fields/2/resolved_value`.
- Leg3a was rebuilt from the S1 base, preserving the FIRE-10 13-leaf carry and the digest/schema omission. Exactly five fresh leaves were overlaid: `/resolution_stage`, `/fire_hash_confirmation`, `/ops_file_template/operations/0/operation/destination_deadline_seconds`, `/resolved_fields/0/resolved_value`, and `/resolved_fields/2/resolved_value`.
- The R1 binding rehash changed only packet entries `/packets/1/sha256` and `/packets/4/sha256`.

`S1-VS-S1B-DIFF.txt` records the full allowlisted canonical diff.

## R2/R3: staged-exemption metadata carry

The source S1 binding had 159 staged metadata entries. Eighty-three apply to the ten S1b-active packets. R2 carries an entry only when its packet leaf exists in both S1 and S1b and is canonically identical (`jq -S` equality); fields S1b freshly resolved are dropped.

R3 authorizes six additional **VESTIGIAL-PRE-D1** drops in each of `staged_fields` and `staged_field_sources`. The D1 leg1 amendment shifted executor command indexes; these metadata pointers exist in neither S1 nor S1b. The two prover-hash tokens no longer exist in the packet, and the surviving deposit-report references are separately covered by valid metadata entries.

Per metadata array, the complete accounting is **83 active = 71 copied + 6 fresh-resolved drops + 6 vestigial-pre-D1 drops**.

### Fresh-resolved drop ledger (six entries per array)

| Packet | JSON pointer | Resolved token class |
|---|---|---|
| `native-leg3c-approve-wa666-permit2.json` | `/executor/commands/0/17` | leg3c fee ceiling |
| `native-leg3c-approve-wa666-permit2.json` | `/resolved_fields/1/resolved_value/commands/0/17` | leg3c fee ceiling |
| `native-leg3d-permit2-wa666-router.json` | `/executor/commands/0/17` | leg3d fee ceiling |
| `native-leg3d-permit2-wa666-router.json` | `/resolved_fields/1/resolved_value/commands/0/17` | leg3d fee ceiling |
| `native-leg3e-swap-wa666-usdc.json` | `/executor/commands/0/17` | leg3e fee ceiling |
| `native-leg3e-swap-wa666-usdc.json` | `/resolved_fields/2/resolved_value/commands/0/17` | leg3e fee ceiling |

### Vestigial-pre-D1 drop ledger (six entries per array)

| Packet | Stale pointer | Token |
|---|---|---|
| `native-leg1-bridge-in.json` | `/executor/commands/2/12` | `leg1-evm-deposit-report` |
| `native-leg1-bridge-in.json` | `/executor/commands/2/28` | `leg1-prover-proof-hash` |
| `native-leg1-bridge-in.json` | `/executor/commands/2/30` | `leg1-prover-public-values-hash` |
| `native-leg1-bridge-in.json` | `/resolved_fields/1/resolved_value/commands/2/12` | `leg1-evm-deposit-report` |
| `native-leg1-bridge-in.json` | `/resolved_fields/1/resolved_value/commands/2/28` | `leg1-prover-proof-hash` |
| `native-leg1-bridge-in.json` | `/resolved_fields/1/resolved_value/commands/2/30` | `leg1-prover-public-values-hash` |

The binding carries the S1 staged-fields contract and digest-omission ruling verbatim. Detailed machine-readable accounting is `/tmp/ghash-s1b-bind/r2-ledger.txt`.

## Execution-window gate

**GATE VERDICT: FAIL. S1b is EXPIRED/UNSAFE for execution.**

- Hard leg3e deadline: unix `1786124483` = 2026-08-07 17:41:23 UTC.
- Conservative measured time to leg3e mined: 25 minutes, including EVM legs, PFTL build/sign/finality, and clone refreshes across the nine preceding stages.
- Required safety margin: 10 minutes.
- Required lead time: 25 + 10 = 35 minutes.
- Minimum safe GO-by: 17:06:23 UTC.
- Publication was approximately 17:20 UTC, leaving approximately 21 minutes, which is less than the required 35 minutes.

S1b hash and linter validation demonstrate correct binding mechanics only. The closed time window is a fail-closed condition. A future attempt requires a newly collected simulation, deadline, quote set, packet stage, and binding.
