# Glossary

Terms and codenames used across these docs, grouped by topic. Each entry gives
the meaning used in this repository and, where useful, the page that defines
it. Several labels (G-gates, E-labels, W6) are reused with different meanings
in different plans; those entries say so.

## Networks, names and units

### PFTL, PFT Ledger, and postfiatd

- **PFTL** is this repository's chain, Post Fiat L1 (also written "Post Fiat
  Ledger (PFTL)" in some specs, and "l1v2" in governance and architecture
  docs). It uses Consensus v2 finality and ML-DSA-65 authorization.
- **PFT Ledger** is a different, older network: the live rippled-derived (XRPL
  fork) testnet with community validators, secp256k1 identities and Dynamic UNL
  rounds. It is not this chain
  ([Testnet Genesis And Launch Specification](../architecture/l1v2-testnet-genesis-and-launch-spec.md)).
  Outside this repository, "PFTL" sometimes refers to the PFT Ledger; check the
  context.
- **postfiatd** is the validator daemon of the PFT Ledger fork. This
  repository's binary is `postfiat-node`.

### postfiat-wan-devnet-2

The chain ID of the only running Post Fiat L1 network: a controlled devnet run
by six Foundation-administered validators. See
[Network Status](../status/network-status.md).

### Controlled Testnet

A project-operated engineering network used to prove protocol and operator
behavior before public launch. `postfiat-wan-devnet-2` is the current one.

### PFT

The native token of Post Fiat L1. It has a fixed genesis supply, every native
fee burns, and there is no validator reward ([Whitepaper](../whitepaper.md)).

### Atom

The integer base unit for amounts. 1 PFT = 1,000,000 atoms
([Faucet runbook](../runbooks/faucet-testnet-transactions.md)). Issued assets
and NAVCoins also count in atoms of their own precision.

## Protocol components

### Account

A transparent ledger identity controlled by post-quantum account authorization.
See [Addresses, Keys And Signatures](../integrate/addresses-keys-signatures.md).

### Account History

Indexed transaction history for an account, exposed through `account_tx` and the
Python client. It covers transparent blocks only
([Account History](../rpc/account-history.md)).

### Asset-Orchard

The shielded settlement path: a typed-asset extension of Zcash Orchard with
Halo2 proofs, used for private transfers and swaps
([Settlement Lanes](../architecture/settlement-lanes.md)).

### Certificate

A quorum proof that validators accepted a block, batch, governance artifact, or
other protocol object.

### Cobalt

PostFiat's validator-trust governance lane, from XRPL research. It ratifies
validator-registry and trust-graph changes only. It does not order blocks,
finalize transactions or authorize unrelated governance. It has been active on
`postfiat-wan-devnet-2` since height 916 ([Cobalt](../governance/cobalt.md)).

### Consensus v2

The block-finality protocol: explicit prepare/precommit rounds with durable
locks, signed timeout certificates and deterministic proposer rotation. It
applies at and above a configured activation height. It is not chained HotStuff
([Finality](../architecture/finality.md)).

### DABC

Democratic atomic broadcast. In PostFiat docs this refers to the Cobalt
amendment path that orders governance transitions after the lower-level
agreement mechanics accept valid candidates.

### Essential Subset

A Cobalt trust-graph component with threshold parameters `t_S` and `q_S`.
PostFiat checks these before activating a graph.

### Fee-Claimed Receipt

A rejected receipt with code `tec_<reason>`: the transaction was validly
signed, correctly sequenced and could pay, so its fee was burned and its
sequence consumed although its effect failed. Governed by
`rejected_tx_fee_activation_height`.

### FastPay

A payment lane for prefunded single-owner PFT objects: an owner-signed order
plus a certificate from distinct validators. It is for payments and unwraps,
not swaps ([Settlement Lanes](../architecture/settlement-lanes.md)).

### FastSwap

A dual-owner delivery-versus-payment lane over prefunded objects. Both owners
sign one intent, and validators certify exactly one Confirm-or-Cancel outcome
([Settlement Lanes](../architecture/settlement-lanes.md)).

### ML-DSA

Module-Lattice Digital Signature Algorithm (FIPS 204). PostFiat uses ML-DSA-65
for account and validator authorization.

### Orchard

The Zcash shielded protocol family used by PostFiat's current Halo2 privacy
path.

### RBC, ABBA, MVBA

Reliable broadcast, asynchronous binary Byzantine agreement, and multi-valued
Byzantine agreement. PostFiat uses these mechanics in the Cobalt governance
implementation.

### Require-Memo Flag

Account flag `1`, set by the owner with `account_set`. Incoming covered
payments without a memo are rejected with `destination_memo_required`, the
PostFiat equivalent of XRPL `RequireDest`.

### Turnstile

The accounting boundary between transparent value and shielded value. Deposits
enter the shielded pool through a transparent funding envelope; withdraws exit
through a bound transparent recipient envelope.

### W6 atomic swap

One consensus transaction with two ML-DSA-65 owner authorizations that moves
both legs or neither ([Settlement Lanes](../architecture/settlement-lanes.md)).
Some pfUSDC bridge documents also use "W6 gate" for an unrelated bridge
verification gate.

### YOLO target receipt

A consensus-verified record of a proved options-portfolio calculation
(Nitro-attested, SP1-proven). It cannot execute orders, move capital, establish
reserves or authorize issuance. The feature is disabled by default
([YOLO Target Receipt v1](../yolo/target-receipt-v1.md)). The name is not
expanded in this repository.

## Assets

### NAVCoin

A floating-value issued asset whose supply, minting, redemption and halts are
tied to machine-checkable reserve packets. NAVCoins are not stablecoins
([NAVCoins](../navcoins/index.md)).

### A666 (a666, wA666)

A NAVCoin lineage on PFTL for large primary subscriptions and redemptions.
A666 v2 is the deployed lineage; `wA666` is its Ethereum representation. Public
general availability remains closed ([NAVCoins](../navcoins/index.md),
[Assets And Venues](../navcoins/assets-and-venues.md)).

### a651 and a652

`a651` is the first named NAVCoin instance (a six-leg reserve portfolio), now
legacy. `a652` was a second instance used to prove cross-NAVCoin swaps on the
devnet ([NAVCoins](../navcoins/index.md)).

### BMNRC

"BMNR Carry NAVCoin": a NAVCoin that holds BMNRon (an Ondo asset tracking
BitMine Immersion Technologies stock) long and shorts the same number of shares
on Hyperliquid's `xyz:BMNR` perpetual, earning funding. It was created on
`postfiat-wan-devnet-2` at heights 1089–1101 on 2026-10-02
([NAVCoin Create And Swap Specification](../navcoins/navcoin-create-and-swap-spec.md),
[Current State](../status/chain-state-current.md)).

### B23 and B24

Schwab-backed NAVCoins. A reading of the Schwab account's equity is taken in a
Nitro enclave and proven with SP1 (B24's active proof profile has source class
`b23-schwab-account-equity-nitro-sp1-v1`, read on 2026-11-25). B24 replaced the
earlier B23 line (`ops/b24-replacement/`) and has been created and cashed in
with real money on `postfiat-wan-devnet-2` since mid-November
([17 November handoff](../handoffs/2026-11-17___dravlic__navcoin_checklist_first_pass_wallet_round_trip_and_one_code_line.md),
[Current State](../status/chain-state-current.md)).

### pfUSDC

A PFTL-side receipt for USDC locked in a source-chain vault under a named route,
used as cash in NAVCoin settlement ([NAVCoins](../navcoins/index.md)). Ethereum
and Solana deposits mint separate pfUSDC series with separate backing; Solana
pfUSDC can be withdrawn to Solana since 2026-11-23; since 2026-11-26 deposits
and withdrawals are open only during a daily proving session
([Network Status](../status/network-status.md#usdc-bridges)).

### pNOK

A WNOK-backed (sandbox) Norwegian krone asset issued on PFTL, used in the
private fixed-quote currency swap demonstration (20 pfUSDC for 210 pNOK)
([Whitepaper](../whitepaper.md)).

## Plans, gates and campaigns

### Gate Zero: Z1, Z2, Z3

Operator preconditions in the public-testnet path milestone; no
community-facing step starts until all three close. Z1: Cobalt is live as the
sole registry-change path on the qualified storage lineage. Z2: the
AI-governance decision is made and live. Z3: NAVCoin round trips (deposit, mint,
swap, redemption) work end to end. All three are open
([Testnet Path](../status/testnet-path.md)).

### G0–G7 (storage gates)

The storage-scaling milestone's gates: G0 make runs resumable, G1 freeze the
release candidate, G2 safety and failure atomicity, G3 exact replay, G4
time-budgeted scaling qualification, G5 offline qualification packet, G6
pre-deployment six-clone gate, G7 next-milestone handoff
([Storage Scaling](../plans/completed/storage-scaling-milestone.md)). The Z3 plan
and the StakeHub decoupling plan reuse G0–G7 for their own, different gates.

### E1–E6

The six experiments of the Cobalt adversarial-verification campaign, which
closed `KEEP_ACTIVE` on 2026-08-26: E1 independent oracle, E2 Byzantine
schedules, E3 adversarial recovery, E4 Consensus v2 finality isolation, E5 live
authority transitions, E6 proposal source and operator independence
([Cobalt Adversarial Verification Results](../governance/cobalt-adversarial-verification-results.md)).
The testnet-path milestone also has unrelated "E" phase items.

### Dynamic UNL and UNL

UNL means Unique Node List, the validator list a node trusts. Dynamic UNL is the
PFT Ledger's validator-scoring pipeline (frozen inputs, deterministic scoring,
sidecar commit-reveal). It does not govern this chain's registry
([Dynamic UNL Proposal Source Research Specification](../governance/dynamic-unl-proposal-source-research-spec.md)).

### Task Node

An external Post Fiat task and reward service. Contributors' AI agents use it
to request and track work (`AGENTS.md`), and the Task Node UNL plans study
deriving a validator list from its identity records; every such output is
`SHADOW_ONLY` ([Task Node UNL MVP Plan](../plans/completed/tasknode-unl-mvp-plan.md)).

### StakeHub

An internal operator application that holds the reserve accounts used by A666.
The deferred StakeHub decoupling plan states that it must not be a protocol
dependency or proof authority for a NAVCoin.

## Handoff authors (lanes)

Handoff filenames carry a lowercase operator identifier
([Handoffs](../handoffs/index.md)):

| Identifier | Who |
| --- | --- |
| `dravlic` | Domagoj Ravlić |
| `postfiatchad` | Post Fiat Chad |
| `codex` | Codex agent sessions |
| `nazgul` | Claude ("Nazgûl") directing agents for the founder's lane; "the other lane" in `dravlic` handoffs |
