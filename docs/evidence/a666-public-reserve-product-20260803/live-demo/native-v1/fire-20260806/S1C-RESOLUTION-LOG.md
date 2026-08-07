# S1c resolution log

**Status: READY FOR THE GO-TIME EXECUTION-WINDOW RE-CHECK. This document is a binding record, not an execution command.**

## Scope and immutable lineage

S1c resolves only legs 0, 1, 2a, 2b, 3a, 3b0, 3b, 3c, 3d, and 3e. Receipt-chained successor legs remain S2-deferred. The HELD sources, S1, and S1b are immutable. Hash verification recorded S1c 10/10, S1 16/16, and S1b 10/10; `values-S1.json` and `values-S1b.json` retain their prior hashes.

## Fresh S1b → S1c values

| Binding field | S1b | S1c | Evidence |
|---|---:|---:|---|
| Quorum block | 25704345 | 25704563 | `/tmp/snaga-s1c/quorum.txt` |
| Quorum block hash | `0x43642b3e…` | `0x5dbc88c2e31a4c79287da28a6b6434ada0c8957bbcdad90e6fbfaf5e52b442e3` | `/tmp/snaga-s1c/quorum.txt` |
| Base fee (wei) | 159284909 | 182349614 | `/tmp/snaga-s1c/quorum.txt` |
| Fork block / timestamp | 25704348 / 1786120883 | 25704564 / 1786123499 | `/tmp/snaga-s1c/NUMBERS.json` |
| leg3a binding clock / deadline | 1786121356 / 1786128556 | 1786123962 / 1786131162 | bound clock + 7200 seconds |
| leg3e deadline | 1786124483 | 1786137899 | fork timestamp + 14400 seconds |
| leg3e fill / min-out / input | 8057858 / 7816122 / 11027135 | unchanged | `/tmp/snaga-s1c/NUMBERS.json` |
| EVM owner nonce plan | base 304 | unchanged | two-RPC quorum read |
| leg3d calldata | S1b rebuilt calldata | `/tmp/snaga-s1c/leg3d-calldata.txt` | fresh Permit2 expiry `1786137899` |
| leg3e calldata | S1b rebuilt calldata | `/tmp/snaga-s1c/leg3e-calldata.txt` | fresh deadline `1786137899`, input 11027135, min-out 7816122 |
| S2 advisory leg3h | 11013374 / 10682972 | unchanged; advisory deadline 1786137967 | `/tmp/snaga-s1c/leg3h-receipt.json`; never resolved into a leg3h packet |

### Gas ceilings and cap arithmetic

The cap uses the runbook section 14 form, not the S1b shorthand:

```text
511.024845 + 0.550231982449388 = 511.575076982449388 <= 530.000000
headroom = 18.424923017550612
```

Fresh per-leg gas ceilings (USDC): 3b0 `0.011966693418750`, 3c `0.031341339906250`, 3d `0.017095276312500`, 3e `0.077424506419312`, 3f `0.031341339906250`, 3g `0.017095276312500`, 3h `0.079046278298825`, 4 `0.113968508750000`, and 5b `0.170952763125000`. The active executor fee ceilings are re-derived from base fee `182349614` wei and the same buffered gas limits. Evidence: `/tmp/snaga-s1c/gas.txt` and `NUMBERS.json`.

## Pre-authorized R1/R2/R3 mechanics

### R1 — base-first overlay

The resolver is pointer-set-only and cannot restore keys deleted by prior amendments. Therefore leg1 was rebuilt from S1 and exactly three rendered S1c leaves were overlaid: `/resolution_stage`, `/fire_hash_confirmation`, and `/resolved_fields/2/resolved_value`. This preserves the D1 67-leaf carry.

Leg3a was rebuilt from S1 and exactly five rendered S1c leaves were overlaid: `/resolution_stage`, `/fire_hash_confirmation`, `/ops_file_template/operations/0/operation/destination_deadline_seconds`, `/resolved_fields/0/resolved_value`, and `/resolved_fields/2/resolved_value`. This preserves the FIRE-10 13-leaf carry and the optional digest/schema omission. The R1 overlay proof is `/tmp/ghash-s1c-bind/r1-overlay.txt`.

### R2 — staged-exemption carry

Of 159 source staged metadata entries, 83 apply to the ten S1c-active packets. An entry is copied only where its S1 and S1c leaves both exist and are canonically identical (`jq -S` equality). The staged-fields contract and digest-omission ruling are carried verbatim.

### R3 — vestigial-pre-D1 metadata drops

Per metadata array, accounting is exactly **83 active = 71 copied + 6 fresh-resolved drops + 6 vestigial-pre-D1 drops**.

The six fresh-resolved drops are the two metadata pointers for each of leg3c, leg3d, and leg3e current fee ceilings. The six vestigial drops are leg1’s obsolete `/executor/commands/2/{12,28,30}` and `/resolved_fields/1/resolved_value/commands/2/{12,28,30}` pointers. D1 shifted those command indexes; neither old pointer exists in S1 or S1c. The surviving leg1 deposit-report references remain covered by valid staged entries. Full machine-readable ledger: `/tmp/ghash-s1c-bind/r2-r3-ledger.txt`.

## Diff and immutable-field verdict

`S1B-VS-S1C-DIFF.txt` is allowlist-clean. Differences are limited to stage markers, fresh quorum/fork/deadline/base-fee/gas/fee-ceiling/binding-context values, and fresh leg3d/leg3e calldata expiry/deadline bytes. Fill 8057858, min-out 7816122, input 11027135, reservation ID, subscription nonce, and export nonce are identical to S1b.

`S1-VS-S1C-DIFF.txt` is the same S1-prefire allowlist check. No S2 receipt-chained field was materialized.

## Execution-window pre-check

- leg3e hard deadline: unix `1786137899` = **2026-08-07 21:24:59 UTC**.
- Conservative time to leg3e mined: 25 minutes.
- Required safety margin: 10 minutes.
- Required lead time: 35 minutes.
- Latest safe GO-by: unix `1786135799` = **2026-08-07 20:49:59 UTC**.

This is a pre-check only. At GO time, the executor must re-verify quorum/fork freshness, deadline remaining time, gas cap arithmetic, fleet state, and every receipt gate. Any failed or stale gate is STOP-no-retry.
