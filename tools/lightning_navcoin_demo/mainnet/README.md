# Mainnet Lightning coordinator deployment

This directory deploys one dedicated LND v0.20.1-beta coordinator node for the
real-value Lightning/NAVcoin demonstration. It does not create a wallet,
purchase liquidity, pay an invoice, or touch PFTL consensus.

The executable claim remains:

> non-custodial, conditionally-atomic, COORDINATOR-TRUSTED timing

PFTL height and Bitcoin height are not a shared consensus clock. The
coordinator is trusted to enforce the cross-ledger margin and remain live.

## Topology decision

Use **Phoenix as the user-side wallet, funded from Coinbase**. Phoenix is
self-custodial and exposes the successful BOLT11 payment preimage in payment
technical details, so the browser can verify `SHA256(preimage) == payment_hash`
and locally sign the PFTL escrow finish. A direct Coinbase Lightning payment
is useful only as a routing smoke test: Coinbase does not document a payer
preimage/first-hop-CLTV export adequate for the wallet-controlled claim.

The coordinator uses direct, TLS-pinned LND gRPC. No hosted payment API is in
the value path.

## Node lifecycle

The state root defaults to
`/home/postfiat/.pft/lightning-navcoin-mainnet`. The script refuses broad
filesystem roots. It publishes only gRPC on `127.0.0.1:11009`; REST and P2P are
not published. An LSP can still open a channel over a coordinator-initiated
persistent peer connection.

```bash
scripts/lightning-navcoin-mainnet-env prepare
scripts/lightning-navcoin-mainnet-env start
scripts/lightning-navcoin-mainnet-env export-grpc
scripts/lightning-navcoin-mainnet-env dry-status
```

Wallet creation is deliberately interactive so recovery words never enter an
automation log:

```bash
scripts/lightning-navcoin-mainnet-env init-wallet
```

At both wallet-password prompts, enter the exact contents of
`/home/postfiat/.pft/lightning-navcoin-mainnet/secrets/wallet-password` (or
the corresponding file under `PFTL_LN_MAINNET_STATE_DIR`). Do not choose a
different password: this pre-generated, mode-0600 file is the credential
mounted for LND auto-unlock. The command restarts LND after creation and fails
unless `getinfo` proves that the wallet auto-unlocked on the reviewed
`0.20.1-beta` version and commit. The check can be repeated without creating a
wallet or moving value:

```bash
scripts/lightning-navcoin-mainnet-env verify-auto-unlock
```

Record the aezeed offline. After LND is chain- and graph-synced, bake the
first-release receive-only coordinator macaroon:

```bash
scripts/lightning-navcoin-mainnet-env bake-macaroon
scripts/lightning-navcoin-mainnet-env verify-macaroon
scripts/lightning-navcoin-mainnet-env status
```

The macaroon has exactly `info:read`, `offchain:read`, `invoices:read`, and
`invoices:write`. It has neither `offchain:write` nor any on-chain spend
permission, so the first release can create and observe incoming invoices but
cannot initiate Lightning payments. Reverse remains HOLD.
`bake-macaroon` immediately decodes the saved credential with
`lncli printmacaroon` and fails unless that permission set is exact and the
macaroon has no caveats; `verify-macaroon` repeats the same check without
baking. The coordinator launcher runs this verification before every
`serve`. No wallet exists yet, so neither command is part of the checked-in
dry preflight.
The coordinator connection file separately pins the TLS certificate and
macaroon SHA-256 values and the exact LND identity public key. Composition
also requires `macaroon_path` to equal the canonical receive-only filename
under the configured mainnet state directory; an admin macaroon cannot be
substituted behind the receive-only profile label and hash.

The wallet release independently pins that same public identity in
`wallet-web/src/lib/lightning-navcoin-release.js`. Its checked-in value is
deliberately `null` before interactive wallet creation, which makes an
executable status and invoice presentation fail closed. After `status`
returns the founder-created identity, nazgul must review the compressed
public key, set that one release pin, commit the clean release, rebuild the
production UI manifest, and rerun `bootstrap`. Do not learn or replace this
pin from a coordinator response at runtime.

`export-grpc` copies only the digest-checked LND v0.20.1 generated Python
modules from the pinned local build image. `dry-status` works before wallet
creation and records the explicit `NON_EXISTING`/`HOLD` state without creating
a seed, invoice, channel, or address.

The coordinator uses an immutable `v2` Python environment with hash-pinned
wheels, including `cryptography==49.0.0`. Startup verifies the exact installed
versions before loading coordinator code.

`stop` is non-destructive:

```bash
scripts/lightning-navcoin-mainnet-env stop
```

## Inbound liquidity

The implemented discovery adapter performs only Magma's public BLIP-0051
`recommended/get_info` GET. It has no create-order method. A provider-funded
inbound channel avoids locking a large coordinator principal, but its lease
invoice is still real expenditure and counts against the same demo lifetime
budget.

Do not create or pay an LSP order until all of these are green:

1. the post-change full regtest E2E and crash/adversarial suite;
2. LND identity, mainnet, sync, active-channel, and liquidity checks;
3. the exact six-RPC persistent PFTL handoff;
4. a non-freezable/non-clawback NAVcoin with proven epoch/hash and inventory;
5. a fresh operator-reviewed BTC/USD observation;
6. a nazgul-signed, single-use `LIQUIDITY_SETUP` authorization whose maximum
   cost fits the remaining `<= $20` lifetime budget.

Reserve that permit before any manual external order with
`lightning-navcoin-mainnet-coordinator liquidity-reserve`. The fail-closed
operator command neither calls an LSP nor pays anything. Once separate public
evidence proves the external payment succeeded and a confirmed active channel
has positive inbound capacity, `liquidity-mark-spent` charges the permit's
full ceiling. There is no ambiguity-release command: an unresolved outcome
remains reserved. The exact artifact contract and commands are documented in
`COORDINATOR-CLI.md`.

Magma's order response must be reviewed before payment. The service cannot
compel an LSP to open a channel; channel funding outpoint, confirmations,
active state, and reserve-adjusted inbound capacity must be independently
observed from LND. No availability guarantee is claimed.

An active zero-conf channel contributes no executable liquidity until LND
reports a nonzero `zero_conf_confirmed_scid`; status reports those unconfirmed
active channels separately.

A provider-funded channel initially has no coordinator outbound balance. The
first BTC-to-NAVcoin receipt needs inbound only and creates outbound balance.
The reverse demonstration is capped by that actually received balance and is
re-preflighted immediately before `SendPaymentV2`.

## PFTL handoff gate

The coordinator will accept only an exact policy containing:

- six distinct `tcp://host:port` RPCs;
- chain ID and 48-byte genesis hash;
- 48-byte NAVcoin asset ID;
- finalized NAV epoch and SHA-384 reserve-packet hash;
- coordinator PFTL address and positive inventory;
- `freeze_enabled=false`, `clawback_enabled=false`, and
  `requires_authorization=false`;
- six distinct active validator identities on one height/tip/root;
- an empty mempool and the hardened binary revision.

Executable status also carries a browser-verified six-ledger USD-e8 valuation
binding. Its assurance boundary stays explicit: one attestor under
`multi-fetch-quorum`, proof bytes stored and hash-bound, and no claim that
Groth16 is verified natively by consensus.

The coordinator signing backend remains a separate injected boundary. Browser
wallet keys never enter it. An incomplete, provisional, zero-NAV, freezable,
or non-running handoff leaves the public interface in `HOLD` and withholds the
payable invoice.

## Value gates

Policy parsing hard-caps each run at `$5` and the durable lifetime ceiling at
`$20`. Every swap needs a fresh nazgul Ed25519 authorization bound to the
signed quote hash, swap ID, direction, principal, fee ceiling, policy ID, and
expiry. Price age and quote lifetime each have an immutable five-minute
maximum, and an on-ramp permit must cover the full payable BOLT11 lifetime.
SQLite runs with WAL and `synchronous=FULL`; a spent ceiling is never released.
Uncertain outgoing payments are reconciled by payment hash through
`TrackPaymentV2` before any retry.

The coordinator cannot observe a Phoenix payer-side routing/LSP fee. The
interface therefore requires both an initial fee budget and a second
acknowledgement against Phoenix's final payment screen. Both must fit the
reserved `max_fee_msat` and the all-in `$5` run cap. This is explicit
operator/wallet evidence, not a protocol-observed guarantee.

No real BTC should be sent until nazgul has independently verified the final
regtest evidence, mainnet dry-check report, PFTL handoff, LND liquidity, and
exact dust authorization.
