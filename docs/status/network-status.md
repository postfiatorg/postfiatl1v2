# Network Status

**Rehearsal archive compatibility.** Rehearsal archives containing public-payer issued Internal or Exit actions require their original decoder. This includes rejected and mixed batches, historical commits and snapshot replay. Bare-schedule native payments and issued deposits retain base validation. The integration specification lists the affected relay and wallet records.

**Guidance reviewed 2026-11-25; latest fleet observation 2026-11-25.**
This page summarizes recorded observations and separately labels newer source
work. It is not a live query. The full, dated operating log is
[Current State](chain-state-current.md); when the two disagree, the log wins.

## In one paragraph

Post Fiat L1 runs one network today: `postfiat-wan-devnet-2`, a controlled
devnet (developer network) run by six Foundation-administered validators. The
source code is public, but outside validators cannot join and no public RPC
endpoint is published. It is not a production mainnet, no public testnet is
open, and the software has not been externally audited. Do not put real value
on it. See [Security](../security/index.md) for the audit and release-gate
position.

The PFSH02 branch allows activation only on its disposable rehearsal genesis. In this design deposits and exits expose the public account and amount. A payment exposes its asset, fee, inclusion time and tree. Its sender account stays off the wire. [Private payments](../quantum/private-payments.md).

## Network facts

| Item | Value | Source |
| --- | --- | --- |
| Chain ID | `postfiat-wan-devnet-2` | [Current State](chain-state-current.md) |
| Genesis hash | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` | [Current State](chain-state-current.md) |
| Validators | 6, all Foundation-administered; the devnet accepts no outside validators | [Cobalt](../governance/cobalt.md), [Public Operator Runbook](../runbooks/public-operator-runbook.md) |
| Block finality | Consensus v2 (prepare/precommit with a commit quorum certificate) | [Finality](../architecture/finality.md) |
| Quorum | 5 of 6 (`q = floor(2n/3) + 1`; tolerates `f = 1` faulty validator) | [Finality](../architecture/finality.md) |
| Validator-trust governance | Cobalt, active since height 916; it ratifies validator-registry and trust-graph changes only and does not order or finalize blocks | [Cobalt](../governance/cobalt.md) |
| Signatures | ML-DSA-65 (FIPS 204) for accounts and validators | [Addresses, Keys And Signatures](../integrate/addresses-keys-signatures.md) |
| Native asset | PFT; 1 PFT = 1,000,000 atoms; fees burn | [Exchange Integration Guide](../integrate/exchange-guide.md#fees) |

## Last recorded release and height

On 2026-11-25 a read-only `status` read of all six validators found the second
Solana exit release running, with one tip and one state root
([Current State](chain-state-current.md)). The 2026-10-30 deployment is still
the last one with a deployment receipt on that page. Since 21 November `main`
is the only long-lived branch and releases are cut from it
([Master State](master-state.md)).

| Item | Value |
| --- | --- |
| Running deployment | Second Solana exit release: build revision `04600868` on all six validators, rolled one host at a time on 2026-11-24, rules active from block 1890. `04600868` is `main` at `e5c4f6ceb` plus the release height and two small fixes, on the branch `release/sol-exit-r2`; it is on `main` since `26acd4639` (#234, 25 November 2026). `deployment_manifest_verified` reads true |
| Its node binary SHA-256 | `6dd2ee56d350854dc32f6542895fd51508d12901b068ac840be428d32f0f759b` (all six, 2026-11-25 read) |
| Earlier releases this week | `sol-exit-r1` (`71dbc9b3`, 2026-11-23, rules from block 1841) and `main-r64-20261121` (`674dbb18a`, 2026-11-21); both are on `main` |
| Last deployment receipt | `vote-records-20261030` (source `ae70d66d`), 2026-10-30, with `scripts/postfiat-safe-rollout apply-next` |
| Height | 1897 on all six validators (state root `53265992…`) |
| Latest fleet observation | 2026-11-25 10:16:21–10:16:36 UTC, read-only `status` on all six: one tip, one state root, empty mempools |

Changes merged to `main` after `e5c4f6ceb` are not evidence of deployment. A
`main` commit runs on the devnet only once the validators report a build that
contains it.

The chain makes blocks only when there are transactions to order, so height
does not advance with wall-clock time. A height that has not moved for days is
normal on this devnet.

## USDC bridges

Both routes are part of this controlled devnet. The Solana route runs with
small caps, and its withdrawals have not been announced publicly.

- **Solana USDC, both directions.** Deposits mint pfUSDC through the Task Node;
  withdrawals burn pfUSDC and pay USDC on Solana. Withdrawals have run since the
  first exit release (rules from block 1841, 2026-11-23); since the second
  (block 1890, 2026-11-24) every Solana deposit batch can be withdrawn, a burn
  that cannot be paid has a refund path, and each validator reviews a
  settlement itself before it signs. Real withdrawals of 0.30, 0.30 and 4.00
  USDC were paid and settled on 24 November.
  Limits today: one withdrawal spends one deposit batch; withdrawals below 0.30
  USDC are refused; the vault accepts 25 USDC more; a withdrawal takes about
  four minutes; cancel is refused while the route is paused. Not yet run on the
  live chain: a cancel and refund, and a settlement with one validator down. The
  services read Solana through free public endpoints.
- **Ethereum USDC.** The deposit bridge, wallet proxy and payout service were
  up at the 23 November handoff. Two deposits stuck since 19 November were
  credited at heights 1741–1745 after the 21 November move. No real Ethereum deposit or withdrawal is
  recorded since then.
- **Where the services run.** On 21 November every bridge service was moved to
  one checkout of `main` ([Master State](master-state.md#where-the-bridges-run)).
  At the 25 November handoff (state of 24 November, 22:30 UTC) the relay host
  ran local edits recorded on the branch `ops/relay-host-live-edits-20261125`
  and in #229 (not merged) and #230 (on `main` since #234), and the payout service ran a bundle of
  the second release plus a speed-up (`c033278b`, on `ops/sol-exit-r2-speed`).
  Moving both back to unmodified `main` is open.

Sources: the [25 November handoff](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/handoff-2026-11-25.md),
the [23 November handoff](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/handoff-2026-11-23.md),
[Master State](master-state.md) and the [Bridge Audit Map](bridge-audit-map.md).

## Other running components

- **Storage.** The transactional `redb` storage engine has been active on the
  devnet since height 930 (2026-08-31). The storage-scaling milestone's offline
  qualification packet (gate G5) passed on 2026-10-23 for source `86706699`,
  binary `eea96ba6…`. G6 remains open; this packet does not automatically
  qualify later binaries. See [State And Storage](../architecture/state-and-storage.md).
- **FastPay and FastSwap.** FastPay's epoch-2 recovery committee was installed
  at height 1123 (2026-10-12). The FastSwap service ran at committee epoch 2
  on all six validators from 2026-10-21; it was not re-read for this review.
  These are certificate lanes beside
  block consensus; see [Settlement Lanes](../architecture/settlement-lanes.md).
- **Validator keys.** The recorded fleet uses software key files. Main now
  implements a separate signer process with a Unix socket, peer-UID checks and
  a durable double-sign guard. Unit generation for the signer, RPC and transport
  services is implemented; its qualified deployment under two users remains open. HSM
  custody is not implemented. See the
  [signer policy](../runbooks/public-rpc-operator-policy.md#validator-signer-process).
  File-signer mode needs `--unsafe-devnet-file-signer`. Services on JSON/JSONL
  or not-yet-activated storage need `--unsafe-devnet-json-storage`; that flag
  bypasses the startup check and does not select JSON storage.
- **Private wallet path.** Public submission of shielded actions is off on
  every validator (23 November handoff). Source and local tests do not
  establish a live private-wallet service. See the
  [readiness milestone](../plans/active/user-proven-shielded-actions-milestone.md).

## Newer implementation on main

The [PFT token on Base](../architecture/pft-token-on-base.md) and
[genesis external lane](../architecture/genesis-external-lanes.md) have contract,
release, checkpoint-source and watcher implementations. The November 5 record
demonstrates checkpoint observation/signing on Base Sepolia. On 2026-11-06 the
first real Base Sepolia Lock was released into native PFT on a local
six-validator fleet, refused on resubmission, and the watcher reported it
backed ([record](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-erc20-release-path-qualification-20261103.md#base-sepolia-rehearsal-2026-11-06-part-2-the-release-on-a-local-fleet)). The
native half of the reverse route is implemented behind the genesis switch
`exports_enabled`, default off. The lane export proof program and its relay
were merged into `main` by 2026-11-11; the Base verifier deployment and any
deployment of the export prover and relay remain open. These are source/test results, not a Base mainnet
deployment or activation on `postfiat-wan-devnet-2`.

The [listing milestone](../plans/active/pft-erc20-base-listing-milestone.md)
leaves P2.5 open on the lock audit. That is the remaining Phase 2 qualification
item, not the only launch gate: token allocation and migration decisions, the
production Safe and contracts, the genesis plan and release qualification remain
separate. The [audit package](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-token-audit-package-20261106.md)
is ready for quotes; no external audit report is recorded.

## Access

This repository does not publish a public RPC endpoint for
`postfiat-wan-devnet-2`. Validator RPC listeners bind to loopback and are
reached through an operator-managed authenticated TLS edge
([Public RPC Operator Policy](../runbooks/public-rpc-operator-policy.md)).
To experiment without access, run a local devnet (see
[First Day](../start/first-day.md)).

The `rpc.testnet.postfiat.org` endpoint that appears in governance documents
serves the separate, rippled-derived PFT Ledger, not this chain (see the
[Task Node UNL shadow run](../governance/tasknode-unl-shadow-run-20260904.md)
and the [Glossary](../start/glossary.md#pftl-pft-ledger-and-postfiatd)).

## What is still open

- **Gate Zero** (Z1–Z3) is open: no community-facing step starts until all three
  close. See [Testnet Path](testnet-path.md).
- **Release gates:** the [Release Gate Inventory](release-gate-inventory.md)
  records 4 done, 16 open and 1 unknown **on 2026-09-01**. It is a historical
  inventory, not a recomputed total for the current source or candidate.
- **External audit:** none is recorded in this repository. See
  [Security](../security/index.md).

## How to keep this page current

Update this page when a release is deployed or a fleet observation changes the
facts above. Copy fleet values from [Current State](chain-state-current.md)
or a newer dated operator handoff. Retain the observation date separately from
the documentation review date. Cite source changes to their existing milestones and keep implementation,
qualification and deployment claims separate. A docs refresh is not a fleet probe.
