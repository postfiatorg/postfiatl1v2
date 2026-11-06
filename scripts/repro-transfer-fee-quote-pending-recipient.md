# Reproduction: `transfer_fee_quote` prices the transfer on the committed ledger

Issue: [postfiatorg/postfiatl1v2#193](https://github.com/postfiatorg/postfiatl1v2/issues/193).
Pinned to main `4b4da1b91493e2349f431b0c94652cc0bfa98d99`.

`transfer_fee_quote` takes the sequence of the quoted transfer from the mempool
([lifecycle_queries.rs#L1466](https://github.com/postfiatorg/postfiatl1v2/blob/4b4da1b91493e2349f431b0c94652cc0bfa98d99/crates/node/src/lifecycle_queries.rs#L1466))
but prices it against the committed ledger
([L1450](https://github.com/postfiatorg/postfiatl1v2/blob/4b4da1b91493e2349f431b0c94652cc0bfa98d99/crates/node/src/lifecycle_queries.rs#L1450),
[L1500](https://github.com/postfiatorg/postfiatl1v2/blob/4b4da1b91493e2349f431b0c94652cc0bfa98d99/crates/node/src/lifecycle_queries.rs#L1500),
[L1535-L1556](https://github.com/postfiatorg/postfiatl1v2/blob/4b4da1b91493e2349f431b0c94652cc0bfa98d99/crates/node/src/lifecycle_queries.rs#L1535-L1556)).
`mempool-submit-transfer` prices its fee the same way
([mempool_proposals.rs#L59](https://github.com/postfiatorg/postfiatl1v2/blob/4b4da1b91493e2349f431b0c94652cc0bfa98d99/crates/node/src/mempool_proposals.rs#L59),
[storage_commit.rs#L885](https://github.com/postfiatorg/postfiatl1v2/blob/4b4da1b91493e2349f431b0c94652cc0bfa98d99/crates/node/src/storage_commit.rs#L885)).
Admission ([mempool_proposals.rs#L230-L262](https://github.com/postfiatorg/postfiatl1v2/blob/4b4da1b91493e2349f431b0c94652cc0bfa98d99/crates/node/src/mempool_proposals.rs#L230-L262)),
block execution and the other quotes (`asset_fee_quote`, `escrow_fee_quote`,
`nft_fee_quote`, `offer_fee_quote`, for example
[L1750](https://github.com/postfiatorg/postfiatl1v2/blob/4b4da1b91493e2349f431b0c94652cc0bfa98d99/crates/node/src/lifecycle_queries.rs#L1750))
run the sender's pending entries first through `ledger_after_executable_mempool`
([L1306](https://github.com/postfiatorg/postfiatl1v2/blob/4b4da1b91493e2349f431b0c94652cc0bfa98d99/crates/node/src/mempool_proposals.rs#L1306)).

## What is in this reproduction

| File | Role |
| --- | --- |
| `scripts/repro-transfer-fee-quote-pending-recipient` | Runs the CLI commands below in a fresh `init` devnet. Exits 1 on main; `--control` (empty mempool) exits 0. |
| `crates/node/src/tests/transfer_fee_quote_pending_recipient.rs` | `transfer_fee_quote_matches_the_receipt_with_an_empty_mempool` is the control and passes. The two `#[ignore]` tests state the expected behaviour and fail on main. |

Nothing here changes node behaviour.

## Commands

```
cargo build -p postfiat-node --bin postfiat-node --locked
NODE_BIN=target/debug/postfiat-node scripts/repro-transfer-fee-quote-pending-recipient --control
NODE_BIN=target/debug/postfiat-node scripts/repro-transfer-fee-quote-pending-recipient
cargo test -p postfiat-node --lib --locked transfer_fee_quote_pending_recipient -- --include-ignored
```

Environment: Ubuntu 24.04.2 LTS x86_64, rustc 1.95.0 (59807616e 2026-04-14),
Python 3.12.3, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only`.

## Output on main 4b4da1b9

Control (empty mempool). The quote, the signed transfer and the receipt agree:

```
$ NODE_BIN=target/debug/postfiat-node scripts/repro-transfer-fee-quote-pending-recipient --control
# transfer_fee_quote faucet -> pf0000000000000000000000000000000000000002 amount 1000 (sequence 1)
{"sequence": 1, "mempool_pending_for_sender": 0, "sender_balance": 1000000000, "recipient_exists": false, "will_create_recipient_account": true, "base_transfer_fee": 22, "state_expansion_fee": 10, "minimum_fee": 32, "sender_balance_after_amount_and_fee": 999998968, "sender_meets_reserve_after_transfer": true}
# mempool-submit-transfer faucet -> pf0000000000000000000000000000000000000002 amount 1000 (sequence 1)
{"tx_id": "890fc2883eb3927c", "sequence": 1, "amount": 1000, "fee": 32}
# mempool-batch + apply-batch (1 transactions)
{"tx_id": "890fc2883eb3927c", "accepted": true, "code": "accepted", "fee_charged": 32, "minimum_fee": 32, "state_expansion_fee": 10}
# quote for sequence 1: state_expansion_fee=10 minimum_fee=32
# receipt for sequence 1: state_expansion_fee=10 minimum_fee=32 fee_charged=32 (declared 32)
OK: the quote and the receipt agree
```

Reproduction. A pending transfer creates the recipient; the quote for the next
sequence still prices the account creation, `mempool-submit-transfer` declares
that fee, and the receipt shows the node required 10 atoms less:

```
$ NODE_BIN=target/debug/postfiat-node scripts/repro-transfer-fee-quote-pending-recipient
# pending: faucet -> pf0000000000000000000000000000000000000002, sequence 1 (creates the recipient)
{"tx_id": "e04c0764fe977640", "sequence": 1, "amount": 1000, "fee": 32}
# transfer_fee_quote faucet -> pf0000000000000000000000000000000000000002 amount 1000 (sequence 2)
{"sequence": 2, "mempool_pending_for_sender": 1, "sender_balance": 1000000000, "recipient_exists": false, "will_create_recipient_account": true, "base_transfer_fee": 22, "state_expansion_fee": 10, "minimum_fee": 32, "sender_balance_after_amount_and_fee": 999998968, "sender_meets_reserve_after_transfer": true}
# mempool-submit-transfer faucet -> pf0000000000000000000000000000000000000002 amount 1000 (sequence 2)
{"tx_id": "d2c1be6d1a7002d1", "sequence": 2, "amount": 1000, "fee": 32}
# mempool-batch + apply-batch (2 transactions)
{"tx_id": "e04c0764fe977640", "accepted": true, "code": "accepted", "fee_charged": 32, "minimum_fee": 32, "state_expansion_fee": 10}
{"tx_id": "d2c1be6d1a7002d1", "accepted": true, "code": "accepted", "fee_charged": 32, "minimum_fee": 22, "state_expansion_fee": 0}
# quote for sequence 2: state_expansion_fee=10 minimum_fee=32
# receipt for sequence 2: state_expansion_fee=0 minimum_fee=22 fee_charged=32 (declared 32)
MISMATCH: the quote and the receipt disagree for the same sequence
```

Tests:

```
$ cargo test -p postfiat-node --lib --locked transfer_fee_quote_pending_recipient -- --include-ignored
test tests::transfer_fee_quote_pending_recipient::transfer_fee_quote_matches_the_receipt_with_an_empty_mempool ... ok
test tests::transfer_fee_quote_pending_recipient::transfer_fee_quote_counts_the_pending_debit_of_the_sender ... FAILED
test tests::transfer_fee_quote_pending_recipient::transfer_fee_quote_sees_the_recipient_a_pending_transfer_creates ... FAILED

---- tests::transfer_fee_quote_pending_recipient::transfer_fee_quote_counts_the_pending_debit_of_the_sender stdout ----
quote: sender_balance 2062 after Some(1030), meets reserve; admission: mempool admission rejected `insufficient_funds`: sender balance is too low for amount plus fee

---- tests::transfer_fee_quote_pending_recipient::transfer_fee_quote_sees_the_recipient_a_pending_transfer_creates stdout ----
assertion `left == right` failed: quote for sequence 2 vs its receipt
  left: (10, 32)
 right: (0, 22)

test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 630 filtered out
```

The second ignored test drains the faucet to 2062 atoms, submits one transfer
of 1000 (fee 32) and quotes a second one: the quote reports the committed
balance (`sender_balance: 2062`, `sender_balance_after_amount_and_fee: 1030`,
`sender_meets_reserve_after_transfer: true`) while admission refuses the same
transfer with `insufficient_funds`.

## Expected vs actual

| | Expected | Actual on main |
| --- | --- | --- |
| Quote for sequence 2, recipient created by pending sequence 1 | `recipient_exists: true`, `state_expansion_fee: 0`, `minimum_fee: 22`, as the receipt reports | `recipient_exists: false`, `state_expansion_fee: 10`, `minimum_fee: 32` |
| Fee `mempool-submit-transfer` declares for that transfer | 22 (the receipt's `minimum_fee`) | 32; the extra 10 atoms are burned |
| Quote with a pending debit that leaves 1030 before a 1032 transfer | `sender_meets_reserve_after_transfer: false`, as admission refuses | `true`; wallet-web only blocks on `false` ([tx-builder.js#L459](https://github.com/postfiatorg/postfiatl1v2/blob/4b4da1b91493e2349f431b0c94652cc0bfa98d99/wallet-web/src/lib/tx-builder.js#L459)) |

The expected column follows the other quotes, which dry-run the pending
entries, and `transfer_state_expansion_fee`, which charges only when the
recipient is absent from the ledger it is given
([fees_offer_planning.rs#L27](https://github.com/postfiatorg/postfiatl1v2/blob/4b4da1b91493e2349f431b0c94652cc0bfa98d99/crates/execution/src/fees_offer_planning.rs#L27)).
The Python wallet signs the quote's `minimum_fee`
([wallet.py#L539](https://github.com/postfiatorg/postfiatl1v2/blob/4b4da1b91493e2349f431b0c94652cc0bfa98d99/python/postfiat_rpc/wallet.py#L539)).
