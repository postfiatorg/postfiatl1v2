# S1d reservation-id repair resolution log

## Scope and ruling

S1d is S1c with one authoring repair under Troll ruling R6.  The former
leg2a reservation id was a 64-lowercase-hex (32-byte) literal.  It could never
pass `pftl_uniswap_order_reserve` admission, which requires 96 lowercase hex
characters: `crates/types/src/transactions_mempool_receipts.rs:2697-2705` and
`crates/types/src/core_chain.rs:131`.  The rejected batch-only operation made
no ledger mutation; the fleet was at h779.

R6 treats this field as a replay-guard nonce, not a money parameter.  Amounts,
route, policy, identities, signers, expiry, deadlines, fresh quorum/fork pins,
gas ceilings, and the 530.000000 USDC cap are carried unchanged from S1c.

## Fresh identifier and replay check

- Generation command: `python3 -c "import secrets; print(secrets.token_hex(48))"`
- Generated at: 2026-08-07T18:27:49Z
- New reservation id: `a2137f00f173cfff7a56696cd9afa7b940ea5a7e1302331fdc32c07df47ec7ff4a41f09ddbccd6708bb4378c5a656647`
- Format: 96 lowercase hexadecimal characters (48 bytes).
- Replay evidence: `/tmp/ghash-s1d-bind/replay-check.txt`; zero matches in the
  h779 post-claim clone `/tmp/krimp-val/validator-1-clone`.

## Exact replacement set

| Artifact | Pointer | S1c | S1d |
|---|---|---|---|
| `values-S1c.json` -> `values-S1d.json` | `/stage` | `S1c` | `S1d` |
| same | `/binding_context/leg2a_reservation_id` | 64-hex invalid id | new 96-hex id above |
| same | `/resolved_values/native-leg2a-order-reserve.json/~1ops_file_template~1operations~10~1operation~1reservation_id` | 64-hex invalid id | new 96-hex id above |
| `packets-S1d/native-leg2a-order-reserve.json` | `/ops_file_template/operations/0/operation/reservation_id` | 64-hex invalid id | new 96-hex id above |
| same | `/resolved_fields/3/resolved_value` | 64-hex invalid id | new 96-hex id above |
| `binding-S1d.json` | `/stage_resolution_context/leg2a_reservation_id` | 64-hex invalid id | new 96-hex id above |

The complete canonical comparison is `S1C-VS-S1D-DIFF.txt`: only stage
lineage, the two leg2a packet leaves, the S1d binding context, and packet
hash-chain values differ.  `values-diff.txt` independently proves the values
file differs only at its stage and the two specified input pointers.

## R1/R2/R3 carry mechanics

- **R1 base-first overlay:** S1d leg1 is S1 base plus exactly three fresh
  leaves; S1d leg3a is S1 base plus exactly five fresh leaves.  The exact-set
  proof is `/tmp/ghash-s1d-bind/overlay.txt`.
- **R2 staged-exemption carry:** for each active-packet S1 staged entry, copy
  only byte-identical leaves.  Per metadata array: 83 active = 71 copied + 6
  freshly resolved fee-ceiling drops + 6 inactive vestigial drops.
- **R3 vestigial drops:** the six pre-D1 leg1 relay metadata entries are absent
  from the post-D1 packet command shape and were not copied.  The full two-array
  ledger is `/tmp/ghash-s1d-bind/r2-ledger.txt`.

`staged_fields_contract` and `digest_omission_ruling` are copied verbatim from
S1.  S1, S1b, and S1c remain immutable lineage evidence.

## Execution status and window

Leg0 and leg1 executed under S1c binding
`d8f1ccdb12d055560bb7805cbb7573b023ad35fa1ae6a163c61f088b009d38ab`.
Leg2a and later require S1d.  S1d does not authorize execution outside its
freshness and receipt gates.

The carried leg3e deadline is unix `1786137899` (2026-08-07 21:24:59 UTC).
With the established 25-minute measured path plus 10-minute margin, the
minimum safe GO-by time remains 20:49:59 UTC.  Re-check the execution window,
state pins, replay result, and all STOP-no-retry gates immediately before any
GO.

## Evidence

- `/tmp/ghash-leg2a-rc/rootcause.md`
- `/tmp/ghash-s1d-bind/new-id.txt`
- `/tmp/ghash-s1d-bind/replay-check.txt`
- `/tmp/ghash-s1d-bind/values-diff.txt`
- `/tmp/ghash-s1d-bind/overlay.txt`
- `/tmp/ghash-s1d-bind/r2-ledger.txt`
- `S1C-VS-S1D-DIFF.txt`
