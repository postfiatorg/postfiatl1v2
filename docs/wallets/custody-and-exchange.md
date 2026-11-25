# Custody And Exchange

Custody and exchange support has two tracks:

1. transparent post-quantum account operations;
2. shielded Orchard workflows with scanning, disclosure, and policy review.

## Transparent Track

- account creation;
- fee quote;
- signed transfer;
- receipt/finality verification;
- account history and reconciliation.

## Addresses

- Signed transactions and on-chain state use only the canonical lowercase
  address, `pf` + 40 hex characters.
- Wallets may display and accept an optional mixed-case checksum.
- Nodes reject malformed destination addresses at RPC submission and mempool
  admission, and at execution from a governed activation height.

The format, checksum algorithm, error codes and test vectors are in
[Addresses And Checksums](address-format.md).

## Fees

Fees are burned, never paid to validators. A transaction declares its fee; the
minimum is 1 atom per started 512 bytes of signed transaction weight (at least
1), plus 10 atoms when a native payment creates its recipient account. Every
account keeps a 10-atom reserve. Use the `*_fee_quote` RPC methods to get the
exact minimum before signing.

### Rejected transactions pay their fee

From the governed `rejected_tx_fee_v2_activation_height`, a transaction included
in a certified block that is validly signed, uses the account's next sequence
and can pay its fee while keeping the reserve, but whose effect fails, is
charged like XRPL `tec` results:

- the receipt has `accepted: false` and code `tec_<reason>`, for example
  `tec_insufficient_funds` or `tec_destination_memo_required`;
- `fee_charged` and `fee_burned` equal the declared fee, which left the
  sender's balance and was burned;
- the sender's sequence advanced, and nothing else changed (no amount moved).

Never charged: malformed envelopes, a wrong chain, genesis, protocol, address
namespace or transaction kind, failed authentication (algorithm, key or
signature), a sequence other than the next one, a fee below the minimum, a fee
the sender cannot pay while keeping the reserve, an expired atomic swap, and a
transaction that uses a feature not active at that height. These keep their
plain code, change nothing, and do not consume the sequence.

Reconciliation: an `account_tx` row whose `receipt_code` starts with `tec_`
debited `fee` from `from` and consumed `sequence`; it did not credit `to`.
Before resubmitting a withdrawal that came back `tec_`, sign it again with
the next sequence.

## Deposit Attribution With Required Memos

An account can require a memo on incoming payments (XRPL `RequireDest`). The
owner sets or clears the flag with a signed `account_set` asset transaction:

```json
{"operation": "account_set", "account": "<address>", "set_flags": 1, "clear_flags": 0}
```

Flag `1` is `require_memo`; clear it with `"set_flags": 0, "clear_flags": 1`.
In Python, `set_require_memo(client, wallet=wallet, enabled=True)` quotes,
signs and submits it. The `account` RPC returns `"flags": 1` while the flag is
set and omits `flags` when it is clear.

While the flag is set, these payments to the account are rejected with
`destination_memo_required` unless they carry at least one memo:

| Payment kind | Can carry a memo | To a flagged account |
| --- | --- | --- |
| `payment_v2` (native PFT) | yes, up to 4 | needs at least 1 memo |
| `issued_payment_v2` (issued asset) | yes, up to 4 | needs at least 1 memo |
| `transparent_transfer` (native PFT) | no | always rejected |
| `issued_payment` (issued asset) | no | always rejected |

`issued_payment_v2` is `issued_payment` plus the `payment_v2` memo list; Python
`send_issued_asset(..., memo_data=...)` uses it. Not covered: escrow, atomic
swap, offer, NFT, bridge and FastPay credits, which carry no memo. Do not
advertise those as deposit routes to a memo-requiring omnibus address. A
rejected memo-less payment is fee-claimed once the rejected-fee gate is also
active, and honest mempools refuse it at admission without charging.

Put the customer tag in `memo_data` (lowercase hex) and attribute a deposit by
the memo in the finalized transaction; `account_tx` rows carry `memo_count`
and the same `memo_hash` for native and issued-asset payments, plus signed
`memos` with optional UTF-8 text in both `account_tx` and `tx`.

## Activation

Both rules are consensus changes behind governance activation heights. Before
the height, execution, receipts and state roots are unchanged, so existing
blocks replay identically.

| Governance kind | Enables | Ratify command |
| --- | --- | --- |
| `rejected_tx_fee_v2_activation_height` | fee claim on rejected transactions | `postfiat-node ratify-rejected-tx-fee-v2-activation-height` |
| `require_memo_v2_activation_height` | `account_set`, `issued_payment_v2`, memo enforcement | `postfiat-node ratify-require-memo-v2-activation-height` |

Execution reads only these v2 kinds (`crates/node/src/execution_actions.rs`).
The original kinds without `_v2` are still accepted and stored but gate
nothing; on the devnet they were set to 1153 in October and never applied
([Addresses And Checksums](address-format.md#activation)). Whether a v2 height
is set on the devnet is not recorded here.

Each height must be strictly after the block that commits its amendment. The
latest committed amendment of each kind sets the height, so governance can
postpone a height that has not been reached.

## pfUSDC From Ethereum And Solana

USDC locked in the Ethereum or the Solana vault is credited as pfUSDC on
`postfiat-wan-devnet-2`; each source mints its own pfUSDC series with its own
backing ([Glossary](../start/glossary.md#pfusdc)). Both routes are part of the
controlled devnet. Solana deposits and withdrawals run with small caps and
have not been announced publicly; the limits are on
[Network Status](../status/network-status.md#usdc-bridges).

- **Solana deposit.** A proof of the Solana vault deposit is applied with the
  `solana_bridge_proof_v1` operation and credits the Solana series
  (`crates/execution/src/solana_bridge.rs`).
- **Ethereum deposit.** A proven Ethereum deposit goes through
  `vault_bridge_deposit_propose`, `vault_bridge_deposit_finalize` and
  `vault_bridge_deposit_claim`
  (`crates/execution/src/nav_vault_asset_execution.rs`).
- **Withdrawal.** The holder signs `vault_bridge_burn_to_redeem` (owner, asset,
  bucket, amount, destination); a payout service pays on the source chain and
  the validators settle it. Since block 1890 each validator reviews a Solana
  settlement itself before it signs. One withdrawal spends one deposit batch
  (bucket).
- **Reads.** `vault_bridge_status` (buckets, deposits, redemptions) and
  `vault_bridge_route` ([RPC Method Coverage](../rpc/method-coverage.md)).

## Shielded Track

- receive/view key management;
- deposit address management;
- scan-only monitoring;
- selective disclosure;
- withdrawal review;
- policy and compliance workflows.

## Current Source

- `crates/execution/src/rejected_fee_claim.rs` (fee claim and memo rule)
- `docs/specs/wallet-exchange-custody-model.md`
- `docs/specs/account-key-rotation-boundary.md`
- `docs/specs/transparent-transaction-envelope.md`
