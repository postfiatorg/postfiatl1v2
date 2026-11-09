# Regression check: `transfer_fee_quote` prices the transfer on the ledger after the mempool

Issue: [postfiatorg/postfiatl1v2#193](https://github.com/postfiatorg/postfiatl1v2/issues/193).
Fixed on main: `transfer_fee_quote` and `mempool-submit-transfer` take the
sequence from the mempool and price the transfer on the ledger after the
executable mempool (`ledger_after_executable_mempool`), like admission, block
execution and the other quotes. The script and the tests below were the
reproduction; they now guard the fix.

## What is in this check

| File | Role |
| --- | --- |
| `scripts/repro-transfer-fee-quote-pending-recipient` | Runs the CLI commands below in a fresh `init` devnet. Exits 0 when the quote, the declared fee and the receipt agree; 1 on a regression. `--control` (empty mempool) exits 0. |
| `crates/node/src/tests/transfer_fee_quote_pending_recipient.rs` | `transfer_fee_quote_matches_the_receipt_with_an_empty_mempool` (control), `transfer_fee_quote_sees_the_recipient_a_pending_transfer_creates` and `transfer_fee_quote_counts_the_pending_debit_of_the_sender`. All three run by default. |

## Commands

```
cargo build -p postfiat-node --bin postfiat-node --locked
NODE_BIN=target/debug/postfiat-node scripts/repro-transfer-fee-quote-pending-recipient --control
NODE_BIN=target/debug/postfiat-node scripts/repro-transfer-fee-quote-pending-recipient
cargo test -p postfiat-node --lib --locked transfer_fee_quote_pending_recipient
```

## Output after the fix

A pending transfer creates the recipient; the quote for the next sequence, the
fee `mempool-submit-transfer` declares and the receipt agree on 22 atoms:

```
$ NODE_BIN=target/debug/postfiat-node scripts/repro-transfer-fee-quote-pending-recipient
# pending: faucet -> pf0000000000000000000000000000000000000002, sequence 1 (creates the recipient)
{"tx_id": "b7ed68c08726b87b", "sequence": 1, "amount": 1000, "fee": 32}
# transfer_fee_quote faucet -> pf0000000000000000000000000000000000000002 amount 1000 (sequence 2)
{"sequence": 2, "mempool_pending_for_sender": 1, "sender_balance": 999998968, "recipient_exists": true, "will_create_recipient_account": false, "base_transfer_fee": 22, "state_expansion_fee": 0, "minimum_fee": 22, "sender_balance_after_amount_and_fee": 999997946, "sender_meets_reserve_after_transfer": true}
# mempool-submit-transfer faucet -> pf0000000000000000000000000000000000000002 amount 1000 (sequence 2)
{"tx_id": "f5ae542b500a04ca", "sequence": 2, "amount": 1000, "fee": 22}
# mempool-batch + apply-batch (2 transactions)
{"tx_id": "b7ed68c08726b87b", "accepted": true, "code": "accepted", "fee_charged": 32, "minimum_fee": 32, "state_expansion_fee": 10}
{"tx_id": "f5ae542b500a04ca", "accepted": true, "code": "accepted", "fee_charged": 22, "minimum_fee": 22, "state_expansion_fee": 0}
# quote for sequence 2: state_expansion_fee=0 minimum_fee=22
# receipt for sequence 2: state_expansion_fee=0 minimum_fee=22 fee_charged=22 (declared 22)
OK: the quote, the declared fee and the receipt agree
```

`--control` (empty mempool) quotes, declares and charges 32 for sequence 1, as
before the fix.

## Before the fix (main 4b4da1b9)

| | After the fix | Before the fix |
| --- | --- | --- |
| Quote for sequence 2, recipient created by pending sequence 1 | `recipient_exists: true`, `state_expansion_fee: 0`, `minimum_fee: 22`, as the receipt reports | `recipient_exists: false`, `state_expansion_fee: 10`, `minimum_fee: 32` |
| Fee `mempool-submit-transfer` declares for that transfer | 22 (the receipt's `minimum_fee`) | 32; the extra 10 atoms were burned |
| Quote with a pending debit that leaves 1030 before the next transfer | `sender_balance: 1030`, `sender_meets_reserve_after_transfer: false`; admission refuses the 1022 debit with `below_account_reserve` | `sender_balance: 2062`, `true`; admission refused the 1032 debit with `insufficient_funds` |

On 4b4da1b9 the script exited 1 (`MISMATCH`, quote 10/32 against receipt 0/22)
and the two tests, then `#[ignore]`, failed with `left: (10, 32) right: (0, 22)`
and `sender_balance 2062 after Some(1030), meets reserve`.
