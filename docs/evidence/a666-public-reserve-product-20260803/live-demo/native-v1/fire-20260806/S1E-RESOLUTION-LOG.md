# S1e static-admission normalization resolution log

## R7 defect class and authority

R7 authorizes lowercase normalization only: it preserves the same 20-byte EVM
account and changes no money parameter, signer, route, policy, cap, deadline,
or receipt-chain rule. The first S1d leg2a attempt stopped before mutation
because the EIP-55 mixed-case `ethereum_recipient` failed the strict PFTL
admission rule: `crates/types/src/market_nav_asset_types.rs:1398-1411` and
`crates/types/src/transactions_mempool_receipts.rs:2756-2759`.

The static audit at `/tmp/ghash-s1e-bind/admission-audit.md` covers every
remaining PFTL operation and its executable/static fields. It confirms the
rule is present on the production lineage and orchard-fix; it was not added by
the orchard patch.

## Active S1e repairs

| Packet | Pointer | S1d form | S1e form | Admission citation |
|---|---|---|---|---|
| leg2a order reserve | `/ops_file_template/operations/0/operation/ethereum_recipient` | EIP-55 mixed case | `0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0` | `transactions_mempool_receipts.rs:2756-2759` |
| leg3a export debit | `/ops_file_template/operations/0/operation/ethereum_recipient` | EIP-55 mixed case | `0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0` | `transactions_mempool_receipts.rs:3136-3139` |

The packet-address rule is 42 characters, `0x`, lowercase hexadecimal, and
nonzero (`market_nav_asset_types.rs:1398-1411`). The full normalized packet
comparison is `S1D-VS-S1E-DIFF.txt`; no other active execution field changes.

## Deferred repairs recorded for later stage bindings

Held sources remain immutable. The following static source values MUST be
lowercased when their receipt-dependent stage is cut:

- **S3 leg4 return import:** `bridge_controller`, `wrapped_navcoin_token`, and
  `ethereum_sender` (`transactions_mempool_receipts.rs:3315-3331`).
- **S4 leg5b burn/withdraw/settle:** destination-ref recipient
  (`market_nav_asset_types.rs:1593-1607`), token address, and recipient once
  they materialize into the withdrawal packet
  (`account_owned_asset_types.rs:2231-2234,2250-2253`).

`--verifier` is not a format defect because the native withdrawal workflow
normalizes it (`vault_bridge_workflows.rs:2568-2574,1159-1161`). The EVM leaf
scripts do not impose PFTL lowercase admission on `--to`/`--recipient`; their
direct EVM RPC inputs remain outside this PFTL-format repair.

## R8 base-first overlay

R8 extends R1 for S1e only. Leg1 remains exactly three copied leaves. Leg3a
is exactly six leaves: the original five plus the lowercased
`/ops_file_template/operations/0/operation/ethereum_recipient`. The
exact-leaf proof is `/tmp/ghash-s1e-bind/overlay.txt`.

## R9 staged-field accounting and coverage

Per `staged_fields` array and per `staged_field_sources` array:

`83 active = 69 copied + 8 fresh-resolved drops + 6 vestigial-pre-D1 drops`.

R9's old-leaf to new-leaf proof is
`/tmp/ghash-s1e-bind/r9-drop-proof.txt`: all eight dropped entries no longer
contain their former PENDING token at the same pointer. The leg2a
`resolved_fields` insertion shifted source-lineage metadata. Its two remaining
PENDING source-value strings are explicitly covered as inert metadata because
their referenced executable fields are concrete; this is documented in
`/tmp/ghash-s1e-bind/r9-coverage-proof.txt`. All executable PENDING tokens
map to an exact staged-fields entry, uncovered count is zero, and the linter is
the final arbiter.

## Execution status and window

Leg0 and leg1 are S1c history. Leg2a and later require S1e. The carried
S1c/S1d deadline remains `1786137899`; the minimum safe GO-by time remains
20:49:59 UTC. Re-check freshness, replay, fleet state, and the full
STOP-no-retry gate immediately before GO.

## Evidence

- `/tmp/ghash-s1e-bind/admission-audit.md`
- `/tmp/ghash-s1e-bind/values-diff.txt`
- `/tmp/ghash-s1e-bind/overlay.txt`
- `/tmp/ghash-s1e-bind/r2-ledger.txt`
- `/tmp/ghash-s1e-bind/r9-drop-proof.txt`
- `/tmp/ghash-s1e-bind/r9-coverage-proof.txt`
- `S1D-VS-S1E-DIFF.txt`
