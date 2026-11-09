# Exchange Integration Guide

This guide is for exchange and custody engineers evaluating native PFT
deposits and withdrawals on Post Fiat L1. It covers transparent (non-shielded)
accounts only. It describes what the code does today and lists the gaps that
must close before a real-value integration.

!!! warning "Devnet only"

    The only running network is the controlled devnet `postfiat-wan-devnet-2`,
    run by six Foundation-administered validators. It is not a production
    mainnet, no public testnet is open, and the software has not been
    externally audited. Do not hold customer funds on it. Current facts are on
    [Network Status](../status/network-status.md).

For the full integration plan written for Coinbase, with roadmap, audit plan and
comparisons, see the
[Coinbase listing package](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/integration-plan.md).

## At a glance

| Topic | Rule today | Details |
| --- | --- | --- |
| Account model | Account-based, XRP-style: balance plus strict sequence | [below](#withdrawals) |
| Signatures | ML-DSA-65 only; 1,952-byte keys, 3,309-byte signatures | [Addresses, Keys And Signatures](addresses-keys-signatures.md) |
| Address | `pf` + 40 lowercase hex characters; optional display checksum in source | [Addresses, Keys And Signatures](addresses-keys-signatures.md#address-format) |
| Recipient check | Canonical address checks at admission in the new binary; execution checks require governance activation | [Addresses, Keys And Signatures](addresses-keys-signatures.md#recipient-validation) |
| Unit | 1 PFT = 1,000,000 atoms; amounts are integer atoms | [Fees](#fees) |
| Fee | At least 1 atom per started 512 bytes of transaction weight, plus 10 atoms when the transfer creates the recipient account; the whole fee burns | [Fees](#fees) |
| Reserve | 10 atoms must remain in every account | [Fees](#fees) |
| Confirmation | One committed block (commit quorum certificate) plus an accepted receipt | [Confirmation](#confirmation-finality) |
| Memo / tag | Not in v1 transfers; up to 4 memos in `payment_v2` | [Deposits](#deposits) |
| Account history | `account_tx`, transparent blocks only | [Reconciliation](#reconciliation) |
| Key rotation | Not available for accounts | [Addresses, Keys And Signatures](addresses-keys-signatures.md#key-binding-at-first-spend) |

## Running a node

An exchange would normally run its own node and never trust a third-party RPC
for crediting. Today that path is limited:

- **No public network to join.** There is no published public genesis, chain ID
  or peer set. The controlled devnet accepts no outside validators
  ([Public Operator Runbook](../runbooks/public-operator-runbook.md), section 3).
- **No public RPC endpoint** for `postfiat-wan-devnet-2` is published in this
  repository. Validator RPC binds to loopback and is exposed only through an
  operator-managed authenticated TLS edge
  ([Public RPC Operator Policy](../runbooks/public-rpc-operator-policy.md)).
- **Independent archive or indexer onboarding is an open decision**, and
  validators keep partial history by policy
  ([History Retention](../runbooks/validator-history-retention.md)).

What you can do today:

1. Build from source and run a local multi-validator devnet to exercise every
   flow below (`scripts/devnet-up`, see [First Day](../start/first-day.md)).
2. Read the node join mechanics: a node can catch up from an existing RPC
   endpoint with `postfiat-node rpc-catch-up`, or with the pinned
   `rpc-catch-up-certified-delta`, which fails unless the chain ends at the
   expected height, block hash and state root
   ([Public Operator Runbook](../runbooks/public-operator-runbook.md)).
3. Check that a node follows the chain: all nodes must agree on chain ID,
   genesis hash, registry root, tip and state root (`scripts/testnet-rpc-doctor`).

## RPC basics

- **Transport.** Newline-delimited JSON over plain TCP. A request is one JSON
  object followed by `\n`:
  `{"version":"postfiat-local-rpc-v1","id":"…","method":"…","params":{…}}`.
  A response carries the same `version` and `id`, `ok`, and either `result` or
  `error` (`python/postfiat_rpc/client.py`). The persistent client
  (`python/postfiat_rpc/persistent_client.py`) reuses one connection; a
  WebSocket client exists for wallet-facing proxies.
- **Port.** Validator RPC listens on `27650 + validator index` on loopback by
  default ([Public RPC Operator Policy](../runbooks/public-rpc-operator-policy.md)).
- **Writes are opt-in.** Submission methods are disabled unless the operator
  starts `rpc-serve` with the matching flag
  ([Write Policy](../rpc/write-policy.md)).
- **Clients.** Use the Python client (`python/postfiat_rpc`) or the Rust SDK
  (`crates/rpc_sdk`) rather than hand-building signatures.

Methods an exchange needs ([RPC Methods](../rpc/methods.md)):

| Method | Use |
| --- | --- |
| `status`, `server_info` | Height, health, chain identity |
| `account` | Balance, sequence and bound public key for an address |
| `account_tx`, `account_tx_index_status` | Bounded transaction history for an address; index readiness |
| `tx` | Finality and receipt for one transaction ID |
| `receipts`, `blocks` | Receipt and block lookup |
| `fee`, `transfer_fee_quote` | Fee policy; exact quote for a transfer (and for memos) |
| `mempool_submit_signed_transfer` | Submit a signed v1 transfer (when enabled) |
| `mempool_submit_signed_payment_v2` | Submit a signed payment with memos (when enabled) |
| `mempool_status` | Pending transaction count |

The release generator's RPC service unit starts `rpc-serve` with
`--allow-mempool-submit-finality`, the finality-returning submit path
(`crates/node/src/batch_snapshot.rs`). The full code-derived authorization
list is the [RPC Method Inventory](../runbooks/rpc-method-inventory.md).

## Confirmation (finality)

Block finality is deterministic. Under Consensus v2 a block is committed when
`q` distinct validators sign precommit votes for it, forming a commit quorum
certificate; with six validators `q = 5`. The protocol tolerates
`f = floor((n - 1) / 3)` faulty validators, which is 1 of 6
([Finality](../architecture/finality.md)). There is no probabilistic
confirmation depth: waiting for more blocks does not add safety. Note that all
six validators are administered by one organization today, so the fault
assumption rests on that organization.

A committed block can contain a **rejected** transaction. Credit a deposit or
mark a withdrawal complete only when both hold
([Finality](../architecture/finality.md#client-finality),
[RPC Overview](../rpc/overview.md)):

1. `tx` returns a finality report with `confirmed: true`; the report also
   carries `proof_id`, the `block` record with its certificate, `tip_hash` and
   `tip_state_root` (`TxFinalityReport` in `crates/node/src/node_types.rs`);
   and
2. the report's `receipt` has `accepted: true` and an accepted `code`.

Store the evidence with the credit: finality proof ID, block hash, certificate
ID, registry root, receipt code and message
([Wallet Exchange Custody Model](../specs/wallet-exchange-custody-model.md)).
Consider querying more than one validator RPC and comparing the results.

If a finality poll times out, the transaction may still finalize. Poll again by
transaction ID; do not sign a replacement (`finality_warning` in
`python/postfiat_rpc/pftl_transfer.py`).

## Deposits

Deposit attribution has two options. The repository's custody model recommends
the first.

**1. One address per customer (recommended).** Generate a unique address for
each customer or deposit intent inside your custody boundary, back it up, and
record the mapping before showing the address
([Wallet Exchange Custody Model](../specs/wallet-exchange-custody-model.md)).
There is no extended public key, so a watch-only system cannot derive new
deposit addresses. Then:

1. Watch each address with `account` (balance) and `account_tx` (history).
2. For each inbound transaction, check finality and the accepted receipt with
   `tx` as described above before crediting.
3. Reconcile every balance change against the transactions you have credited
   (see [Reconciliation](#reconciliation)).

**2. Shared address with memos (requires deployment and activation).**
`payment_v2` and `issued_payment_v2` can carry up to four memos (512 bytes total).
The source implementation returns signed memo contents through `tx` and
`account_tx`, alongside their hash, count and byte length. After deployment,
operators must rebuild the account history index to backfill v2 rows.

After governance enables `require_memo_activation_height`, an exchange can set
its account's require-memo flag with `account_set`. Covered payments without a
memo then fail. The flag does not cover shielded withdrawals, escrow, atomic
swap, offer, NFT, bridge or FastPay credits: an exchange must handle those
separately and validate that a supplied memo identifies a customer. See
[Custody And Exchange](../wallets/custody-and-exchange.md).

**Sweeping.** To consolidate, send ordinary transfers from deposit addresses to
a hot wallet. Each sweep pays its own fee, and each deposit address keeps its
10-atom reserve. The first spend from a deposit address binds its public key.

## Withdrawals

The SDK's quote-bound flow
([Wallet Exchange Custody Model](../specs/wallet-exchange-custody-model.md)):

1. Validate the destination (`^pf[0-9a-f]{40}$` at minimum; the chain does not).
2. Request `transfer_fee_quote` with `from` set to your hot wallet address.
3. Bind the quote to the exact request; the SDK rejects a quote whose sender,
   recipient, amount or sequence differs.
4. Sign with `wallet_sign_transfer_from_quote` (Rust SDK,
   `crates/rpc_sdk/src/wallet_sdk.rs`) or the Python helpers `send_pft` and
   `send_pft_and_poll_finality` (`python/postfiat_rpc/wallet.py`;
   [Python Wallet Functions](../python/wallet-functions.md)).
5. Submit through a write-enabled RPC, then poll `tx` by transaction ID.
6. Store the finality evidence listed above.

Sequence and mempool rules (`crates/execution/src/entrypoints.rs`,
`crates/node/src/mempool_proposals.rs`, `crates/node/src/lifecycle_queries.rs`):

- **Strict sequence.** A transfer's `sequence` must equal the account's stored
  sequence plus one, or it is rejected with `bad_sequence`.
- **Rejected transfers change nothing.** A rejected transfer moves no balance,
  burns no fee (`fee_charged` is 0) and does not use up its sequence.
- **Pipelining.** Mempool admission (`admit_signed_transfer_to_mempool`)
  dry-runs the new transfer after the transfers already pending and admits it
  only if it would be accepted. It refuses a duplicate
  transaction ID or a second pending transaction with the same sender
  sequence. At most 64 transactions per sender and 1,024 in total may be
  pending. `transfer_fee_quote` prices the next transfer on the ledger after
  the pending plain transfers, the ones admission runs before it. A quote
  taken while earlier withdrawals are pending therefore counts their debits
  and the accounts they create. An account that only a pending memo payment,
  asset, escrow, NFT or offer entry creates still costs the creation fee.
- **No expiry.** A signed v1 transfer stays valid until its sequence is used.
  To cancel a signed but unexecuted withdrawal, use up that sequence with
  another transaction.

## Fees

| Rule | Value | Source |
| --- | --- | --- |
| Unit | 1 PFT = 1,000,000 atoms | `python/postfiat_rpc/pftl_transfer.py`; [Faucet runbook](../runbooks/faucet-testnet-transactions.md) |
| Byte fee | `max(1, ceil(weight / 512))` atoms | `minimum_transfer_fee` in `crates/execution/src/fees_offer_planning.rs` |
| Weight | Length of signing bytes + algorithm ID + public-key hex + signature hex | `transfer_weight_bytes`, same file |
| New-account fee | +10 atoms if the recipient account does not exist yet | `TRANSFER_ACCOUNT_CREATION_FEE` in `crates/execution/src/entrypoints.rs` |
| Burn | The whole `fee` field burns; nothing goes to validators | `execute_transfer`, same file; [Whitepaper](../whitepaper.md) |
| Reserve | Sender and recipient must each keep at least 10 atoms after the transfer | `ACCOUNT_RESERVE`, same file |

An ML-DSA-65 public key and signature in hex add 10,522 bytes to the weight, so
a typical native transfer costs 22 atoms, or 32 atoms when it creates the
recipient account. The canonical test vector shows `fee = 32` for a transfer to
a new account
([Transparent Transaction Envelope](../specs/transparent-transaction-envelope.md)).
`payment_v2` uses the same formula, with memos counted in its weight. Always
use `transfer_fee_quote` rather than computing the fee yourself; a fee below
the minimum is rejected with `fee_too_low`.

Because of the reserve, the first deposit to a new address must be at least 10
atoms, and the last 10 atoms of an account cannot be withdrawn.

## Reconciliation

- **Balances.** `account` returns `address`, `balance`, `sequence` and
  `public_key_hex` (`Account` in
  `crates/types/src/account_owned_asset_types.rs`). An address that has never
  been funded returns balance 0 and sequence 0. The response does not include a
  height, so record `status` before and after a balance sweep.
- **History.** `account_tx` returns at most `limit` rows in ascending height
  order. `truncated: true` means rows may be missing; page with `from_height`
  and `to_height` or use `account_tx_history`
  ([Account History](../rpc/account-history.md)).
- **What history misses.** The index is built only from blocks whose batch kind
  is `transparent` (`append_account_tx_index_block` and
  `build_account_tx_index` in `crates/node/src/block_finality.rs`). Value that
  reaches a transparent account another way, such as a shielded (Orchard)
  withdrawal, changes the balance but does not appear in `account_tx`. Balance
  reconciliation must allow for this. FastPay and FastSwap objects have their
  own read methods (`owned_objects`, `fastswap_objects`).
- **Supply.** Native PFT has a fixed genesis supply, every fee burns, and there
  is no validator reward ([Whitepaper](../whitepaper.md)).

## Known gaps before exchange integration

These are recorded limits, not hidden ones. Each must be closed or accepted
before real customer funds are involved.

1. **No production network.** Only a controlled devnet exists, run by one
   organization's six validators. Gate Zero (Z1–Z3) is open
   ([Testnet Path](../status/testnet-path.md)).
2. **No external audit.** 16 of 21 release gates are open, including validator
   key custody, specialist review and launch authority
   ([Security](../security/index.md),
   [Release Gate Inventory](../status/release-gate-inventory.md)).
3. **No self-service node or public RPC.** No public genesis, peer set or
   endpoint; independent archive and indexer onboarding is undecided.
4. **No published release artifacts.** No signed download, checksum, SBOM or
   second-builder reproduction exists for outsiders to verify (release gate
   rows 6 and 7).
5. **Address-safety deployment and activation remain pending.** Source includes
   recipient validation and an optional checksum; the execution rule is governed.
6. **No account key rotation**, no hardware-wallet or external-signer
   interface for ML-DSA, and no formal backup-encryption standard
   ([Wallet Exchange Custody Model](../specs/wallet-exchange-custody-model.md),
   "Remaining Production Work").
7. **No watch-only address derivation**; every deposit address needs custody-side
   secret material.
8. **History deployment and backfill remain pending.** Source indexes transparent
   transactions and shielded withdrawals; other balance-changing lanes still
   need separate reconciliation.
9. **Required-memo activation remains pending.** Source exposes signed memo
   contents and provides a governed account flag for covered payments.
10. **No transaction expiry** in the v1 envelope.
11. **Controlled write path**: transaction submission is off by default, and a
    persistent public write edge needs explicit abuse controls and operator
    acceptance ([Write Policy](../rpc/write-policy.md)).
12. **Plaintext validator keys** and unsafe-devnet acknowledgement flags on the
    running services ([Network Status](../status/network-status.md)).

## Related pages

- [Addresses, Keys And Signatures](addresses-keys-signatures.md)
- [RPC Overview](../rpc/overview.md) and [Methods](../rpc/methods.md)
- [Python Client Quickstart](../python/quickstart.md)
- [Custody And Exchange](../wallets/custody-and-exchange.md)
- [Account Key Rotation Boundary](../specs/account-key-rotation-boundary.md)
