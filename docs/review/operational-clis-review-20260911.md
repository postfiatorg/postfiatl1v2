# Operational Python CLIs review — 2026-09-14

This A5 review covers, in the prescribed risk order, `wallet.py`, `client.py`,
`persistent_client.py`, `pftl_transfer.py`, `navcoin.py`,
`hyperliquid.py`, `cobalt.py`, `genesis_registry.py`, and
`storage_scaling.py` under `python/postfiat_rpc/`. The previously reviewed
`tasknode_unl` modules and bridge, proof, Orchard, and program source are
outside this review. No fleet or chain action was performed.

## Findings

### 1. P1 — transfer finality can be reported without an accepted receipt

**Source:** `python/postfiat_rpc/wallet.py:670-733`.

A finality-submit result whose block has a positive height but whose hot
finality receipts contain only another transaction, or a rejection of this
transaction, is returned as `finalized=True`. In polling mode, any positive
block height or the first nonempty receipt list also counts as success even if
the receipt is rejected or belongs to a different transaction. A caller can
therefore treat a failed payment as settled and release value.

The minimal repair is to require an accepted receipt bound to the submitted
transaction ID and a certified block for both paths; otherwise continue polling
and return pending on timeout.

### 2. P2 — incomplete account history results can be presented as complete

**Source:** `python/postfiat_rpc/client.py:1655-1790`.

A successful `account_tx` RPC result missing `rows` or `truncated` becomes
an empty, untruncated scan. `account_tx_history` then declares the history
complete. A truncated/malformed server response can silently hide transfers.
Require the rows array and a boolean truncation marker, and reject a response
whose address or requested window does not match the query.

### 3. P2 — the faucet CLI applies to an implicit validator data directory

**Source:** `python/postfiat_rpc/pftl_transfer.py:45-72,107-132,237-260`.

Invoking `faucet` without a data-directory argument selects a local node
directory or `.postfiat` and immediately runs the wallet helper that applies
the batch to validator state. An operator intending to inspect the command or
using an environment with an unexpected data directory can alter local chain
state. Require an explicit local-apply acknowledgement before calling the
faucet helper, including when the selected directory comes from defaults.

### 4. P2 — NAV operation bundles refer to a noncanonical asset identity

**Source:** `python/postfiat_rpc/navcoin.py:48-49,215-216`;
`crates/types/src/account_owned_asset_types.rs:641-659` (comparison only).

The NAV builder hashes an example string with SHA-384 without chain ID, while
`asset_create` derives a SHA3-384 domain-separated identity including chain
ID. The returned packet and subsequent native operations contain a plausible
96-character ID for an asset that `asset_create` never created. Derive the
canonical identity using an explicit chain ID and cover a Rust/Python vector.

### 5. P2 — missing venue fields become a valid zero-valued observation

**Source:** `python/postfiat_rpc/hyperliquid.py:59-110`.

If an info response is `null`, omits `marginSummary` or `balances`, or
drops a position amount, normalization replaces the missing data with zero or
an empty list and hashes it as an ordinary observation. Independent observers
can agree on a truncated response and attest a wrong account state. Validate
the required response structure and exact finite decimal strings before
creating an observation root.

### 6. P2 — a venue response has no byte cap before JSON decoding

**Source:** `python/postfiat_rpc/hyperliquid.py:35-43`.

`json.load(response)` reads an arbitrarily large HTTP response into memory.
A compromised endpoint or oversized error body can exhaust an observer even
though the request has a timeout. Enforce a finite response byte cap before
JSON decoding and reject malformed or oversized bodies.

### 7. P2 — shadow catch-up can mutate a remote service without a separate interlock

**Source:** `python/postfiat_rpc/cobalt.py:2160-2225`.

The `catch-up` branch sends a `catch_up` operation to the supplied target
endpoint as soon as the source returns a range. Unlike the status, probe, and
verification commands, this request changes a shadow service's history. A
mistargeted endpoint can therefore change a running service. Require an
explicit catch-up acknowledgement before either remote request.

### 8. P2 — mixed receipt deadlines are accepted into a proposed registry

**Source:** `python/postfiat_rpc/genesis_registry.py:598-607`.

Only chain and round are checked for every receipt. The builder takes the
deadline hash and ledger sequence from receipt zero but includes receipt
digests whose deadlines may differ. A mixed receipt set yields a plausible
registry that binds incompatible deadline evidence. Reject empty receipt sets
and require all receipt deadlines to match before building any entries.

## P3 observations

### 9. P3 — nonintegral NAV example valuation still emits native operations

**Source:** `python/postfiat_rpc/navcoin.py:183-202,235-247`.

For net assets of 10 micro-USD and supply 3, the helper floors NAV to 3 and
returns a true `>=` invariant, then emits a reserve-submit operation. The
native exact-equality check rejects that operation. The example documentation
already warns about this limitation; require exact divisibility before
presenting the operation as ready in a separate change.

### 10. P3 — large PFTL amounts lose precision in the CLI report

**Source:** `python/postfiat_rpc/pftl_transfer.py:94-104`.

`amount_pft` uses floating-point division. A sufficiently large valid atom
amount returns a JSON decimal that no longer represents the exact requested
amount, despite the exact integer `amount_atoms` alongside it. Keep the
human-readable value as an exact decimal string in a separate change.

### 11. P3 — packet tree enumeration precedes the file-count bound

**Source:** `python/postfiat_rpc/storage_scaling.py:312-340`.

A packet with millions of nested entries forces recursive filesystem
enumeration and a set allocation before the 4,096-file bound is checked.
The verifier is offline, and the resulting evidence is not accepted, but a
malicious local packet can waste resources. Bound enumeration while walking
in a separate change.

## Areas with no findings

- `persistent_client.py` applies socket timeouts, bounds newline-framed
  responses, closes on transport failures, and does not automatically retry a
  mutation.
- The wallet key-generation path passes master seeds through mode-0600
  temporary files instead of command-line arguments; generated quote files
  are private.
- The Cobalt live-status and evidence paths are read-only, pin publication
  bytes, and bound packet reads; the finding concerns only active catch-up.
- The genesis decoder rejects noncanonical CBOR, unknown labels, and
  malformed registry fields before hashing.
- The storage-scaling browser binds to loopback and serves only a report
  produced by offline verification, with no mutation route.

## Repair result

Pending the separate P1/P2 repair commit; P3 observations remain recorded.
