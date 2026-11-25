# Addresses And Checksums

This page specifies the PFTL account address, its optional mixed-case
checksum, and how nodes validate destination addresses. Exchanges and wallets
should implement the parser below and test it against the published vectors.

## Canonical address

```text
address = "pf" + lowercase_hex(SHA3-384("postfiat.address.v1" || 0x00 || ml_dsa_65_public_key)[0..20])
```

- The address is always 42 characters: `pf` and 40 hex characters.
- The canonical form is all lowercase. It is the only form stored on chain,
  used as a ledger key, and accepted in a signed transaction.
- There is no BIP32-style public derivation. Each deposit address comes from
  its own ML-DSA-65 key.

Source: `address_from_public_key` in `crates/crypto_provider/src/lib.rs`.

## Checksum specification

The checksum is an optional mixed-case display encoding of the same 40 hex
characters. It follows EIP-55 but uses this chain's SHA3 family and its own
domain tag. It changes no bytes on chain.

```text
hex = the 40 lowercase hex characters after "pf"
h   = SHA3-256("postfiat.address.checksum.v1" || 0x00 || ascii(hex))
for i in 0..40:
    nibble = (i even) ? h[i/2] >> 4 : h[i/2] & 0x0f
    out[i] = (hex[i] in 'a'..'f' and nibble >= 8) ? uppercase(hex[i]) : hex[i]
checksummed = "pf" + out
```

- The prefix is never uppercased and is not part of the hashed input.
- An address with no letters is its own checksummed form.
- SHA3-256 is FIPS 202 SHA3, not Keccak-256.

Example: `pfb385030ad80e5f7a59ec93b71f11055e9ccb9230` is displayed as
`pfb385030Ad80e5F7a59EC93b71f11055E9cCb9230`.

## Parsing rules

A parser takes human input and returns the canonical lowercase address, or
rejects it with one reason. Rules are checked in this order:

| Rule | Reason |
| --- | --- |
| The first two characters must be exactly `pf`. `PF`, `Pf` and `pF` are rejected. | `uppercase_prefix`, otherwise `missing_prefix` |
| No non-ASCII character may follow the prefix. | `non_hex` |
| The address must be exactly 42 characters. | `wrong_length` |
| Every character after the prefix must be `0-9`, `a-f` or `A-F`. | `non_hex` |
| All-lowercase hex is accepted as the legacy, canonical form. | |
| Hex that contains any uppercase letter is accepted only if it equals the checksummed form exactly. All-uppercase hex is rejected. | `bad_checksum` |

The first four reasons mean the address is malformed. `bad_checksum` is a
separate error, so a wallet can say "this address has a typo" rather than
"this is not an address". Parsers do not trim whitespace; trim pasted text
before parsing.

Implementations:

- Rust: `parse_address`, `checksum_address`, `validate_canonical_address` and
  `display_address` in `crates/crypto_provider/src/address.rs`.
- JavaScript: `wallet-extension/lib/address.js`, with a self-contained
  SHA3-256.
- Python: `python/postfiat_rpc/address.py`.

## Test vectors

[pftl-address-vectors-v1.json](../fixtures/addresses/pftl-address-vectors-v1.json)
lists valid addresses in both forms (six derived from fixed ML-DSA-65 seeds),
wrong checksums, and malformed inputs with their expected reason. The Rust, JS
and Python test suites all read this file.

## Destination validation

A destination field is any field that credits an account: `transfer.to`,
`payment.to`, `issued_payment.to`, `nav_mint_at_nav.to`,
`vault_bridge_mint_from_receipts.to`, `vault_bridge_deposit_claim.recipient`,
`vault_bridge_nav_subscription_allocate.nav_recipient`,
`pftl_uniswap_primary_redeem.settlement_recipient`,
`pftl_uniswap_return_import.pftl_recipient`, `escrow_create.recipient`,
`nft_mint.owner` and `nft_transfer.to`. Atomic-swap leg recipients were
already validated as canonical addresses by the transaction envelope.

A destination field must be canonical. A correctly checksummed value is still
rejected in a signed transaction, because a mixed-case string is a different
ledger key. Wallets normalize with the parser before quoting and signing.

| Code | Meaning |
| --- | --- |
| `invalid_recipient_address` | The value is not `pf` + 40 hex characters. |
| `invalid_recipient_checksum` | The value uses uppercase hex that does not match its checksum. |
| `noncanonical_recipient_address` | The value is correctly checksummed, but a signed transaction must carry the lowercase form. |

The check runs in two places:

1. **Admission (local policy, active on upgrade).** The RPC server and mempool
   admission reject the transaction before it is stored. The RPC error `code`
   is one of the codes above, and the message names the field. Source:
   `rpc_serve_destination_address_rejection` in
   `crates/node/src/atomic_swap_rpc_server.rs` and
   `ensure_destination_addresses_admissible` in
   `crates/node/src/mempool_proposals.rs`.
2. **Execution (consensus, governed).** Block execution applies the same rule
   from the governance-scheduled `recipient_format_v2_activation_height`. Without
   that amendment the rule is inactive, so historical blocks replay unchanged.
   Source: `crates/execution/src/recipient_format.rs`.

From the activation height, a block that contains such a transaction records a
rejected receipt with the code above. Before the separately governed
`rejected_tx_fee_v2_activation_height`, it charges no fee and consumes no sequence.
After that gate, an authenticated, correctly sequenced transaction that can pay
its fee is charged and receives `tec_invalid_recipient_address`,
`tec_invalid_recipient_checksum` or `tec_noncanonical_recipient_address`. The
recipient receives nothing. RPC and mempool refusals always remain uncharged.

## Activation

The activation height is a governance amendment of kind
`recipient_format_v2_activation_height` whose value is the first enforced block
height.

!!! note "v2 kinds"

    Release 6.4 never applied the original `recipient_format_activation_height`,
    `rejected_tx_fee_activation_height` and `require_memo_activation_height`
    after the 2026-11-12 rollout, although governance set them to 1153. Execution
    now reads only the `*_v2_activation_height` kinds (ratify with
    `ratify-recipient-format-v2-activation-height`,
    `ratify-rejected-tx-fee-v2-activation-height` and
    `ratify-require-memo-v2-activation-height`). The original kinds are still
    accepted and stored but gate nothing. Ratify a v2 kind only after every
    validator runs a release that knows it.

- It must be scheduled strictly after the block that applies it.
- The earliest unpaused scheduled height wins, so a later amendment cannot
  postpone or disable the rule.
- Every validator must run a release that knows this kind before the
  amendment is submitted.

```bash
postfiat-node ratify-recipient-format-v2-activation-height --data-dir PATH \
  --validators CSV --height H --amendment-file recipient-format.unsigned.json
# each validator
postfiat-node governance-authorization-sign --data-dir PATH \
  --amendment-file recipient-format.unsigned.json --validator NODE_ID \
  --validator-key-file PATH --proposal-slot S --expires-at-height E \
  --authorization-file NODE_ID.authorization.json
postfiat-node governance-amendment-assemble --data-dir PATH \
  --amendment-file recipient-format.unsigned.json --authorization-files CSV \
  --proposal-slot S --output recipient-format.signed.json
postfiat-node governance-batch --data-dir PATH \
  --amendment-file recipient-format.signed.json --batch-file recipient-format.batch.json
# on the proposer of slot S (postfiat-node block-proposer --height S), as the postfiat user
postfiat-node transport-peer-certified-batch-round --data-dir PATH --topology PATH \
  --batch-kind governance --batch-file recipient-format.batch.json \
  --key-file PATH --artifact-dir PATH --require-local-proposer --height S \
  --quorum-early-full-propagation --local-apply-before-certified-send \
  --timeout-ms 60000 --send-retries 3 --retry-backoff-ms 2000
```

!!! warning "This activation cannot be undone"

    The recipient-format activation is the only one of the three exchange-readiness
    activations that cannot be reversed. `rejected_tx_fee_activation_height` and
    `require_memo_activation_height` take the latest amendment, so a later
    amendment with a far height postpones them. `recipient_format_activation_height`
    takes the earliest unpaused amendment, and a paused amendment is refused.

    Before submitting it, check every address in use against the parsing rules,
    and submit it last. On the devnet the original kind was set to 1153 at
    block 1146 on 2026-10-23; its record was on the branch
    `release/exchange-readiness-20261023`, which was archived and deleted on
    21 November with the other release branches
    ([Master State](../status/master-state.md#the-archive)).

When the round runs on a validator host against that validator's data
directory, pass `--local-apply-before-certified-send`. Without it, the round
also sends the certified block to its own validator service while holding the
store lease. That send is reset, the proposer does not apply the block, and the
round exits 1 after the peers have committed.

**If a round stalls after the votes are durable.** A same-view retry is refused
by the vote-safety rule. Move the height to the next view instead:

1. Each validator signs `block-timeout-vote --height H --view V` on its own host.
2. `block-timeout-certificate` assembles the votes; it needs a quorum of 5 of 6.
3. The leader of view V+1 (`block-proposer --height H --view V+1`) runs the same
   round with `--view V+1 --timeout-certificate-file PATH`.

The view-(V+1) proposal must carry the locked block, so the same batch commits.
The authorization's slot is the height, not the view. A validator that missed
the delivery takes the same block with `transport-batch-send --batch-file B
--certificate-file C` from a peer. The 2026-10-23 procedure, with code
references, was §8b of the release sheet on the branch
`release/exchange-readiness-20261023`, archived and deleted on 21 November
([Master State](../status/master-state.md#the-archive)).

## Display

Human-facing surfaces show the checksummed form. Machine fields stay canonical.

- The RPC `account` result keeps `address` and adds `address_checksummed`.
- `postfiat-rpc-sdk wallet-identity` and `postfiat-node wallet-keygen` and
  `wallet-restore` reports add `address_checksummed`.
- The Chrome extension shows the checksummed form. It copies, stores and signs
  the canonical form, and accepts either form in the send field.
- `python -m postfiat_rpc.pftl_transfer address ADDRESS` prints both forms.
  `send` and `faucet` accept either form for `--to` and sign the canonical form.
