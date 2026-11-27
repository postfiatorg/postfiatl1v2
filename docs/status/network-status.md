# Network Status

**Rehearsal archive compatibility.** Rehearsal archives containing public-payer issued Internal or Exit actions require their original decoder. This includes rejected and mixed batches, historical commits and snapshot replay. Bare-schedule native payments and issued deposits retain base validation. The integration specification lists the affected relay and wallet records.

**Guidance reviewed 2026-11-27; latest fleet observation 2026-11-27.**
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

PFSH03 post-quantum private payments run on this devnet: by the handoff of 27 November they were activated at block 1910 and used end to end from the signed Mac app, and blocks 1912 to 1914 hold one shielded deposit, one private payment and one exit (read on 27 November). Deposits and exits expose the public account and amount. A payment exposes its asset, fee, inclusion time and tree. Its sender account stays off the wire. [Private payments](../quantum/private-payments.md).

## Network facts

| Item | Value | Source |
| --- | --- | --- |
| Chain ID | `postfiat-wan-devnet-2` | [Current State](chain-state-current.md) |
| Genesis hash | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` | [Current State](chain-state-current.md) |
| Validators | 6, all Foundation-administered; the devnet accepts no outside validators | [Cobalt](../governance/cobalt.md), [Public Operator Runbook](../runbooks/public-operator-runbook.md) |
| Block finality | Consensus v2 (prepare/precommit with a commit quorum certificate) | [Finality](../architecture/finality.md) |
| Quorum | 5 of 6 (`q = floor(2n/3) + 1`; tolerates `f = 1` faulty validator) | [Finality](../architecture/finality.md) |
| Validator-trust governance | Cobalt last verified at height 924; it ratifies validator-registry and trust-graph changes only and does not order or finalize blocks | [Cobalt](../governance/cobalt.md) |
| Signatures | ML-DSA-65 (FIPS 204) for accounts and validators | [Addresses, Keys And Signatures](../integrate/addresses-keys-signatures.md) |
| Native asset | PFT; 1 PFT = 1,000,000 atoms; fees burn | [Exchange Integration Guide](../integrate/exchange-guide.md#fees) |

## Last recorded release and height

<!-- component-observations -->

| Component and evidence | Chain / height | Last observed | Result |
| --- | --- | --- | --- |
| [Validator release](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/chain-state-current.md) | postfiat-wan-devnet-2 / 1915 | 2026-11-27 09:01 UTC | Six validators on pq-r3, one state root |
| [Validator-trust authority](https://github.com/postfiatorg/postfiatl1v2/blob/003af00011c6af086f14f4d4017e450a11eaf45f/docs/governance/cobalt.md#current-controlled-devnet-state) | postfiat-wan-devnet-2 / 924 | 2026-08-26 06:35:50 UTC | Cobalt last verified active for validator trust |
| [Lane-export proof identity](https://github.com/postfiatorg/postfiatl1v2/actions/runs/45938002025) | Source build / n/a | 2026-11-27T04:22:33Z | Unverified (workflow failure, comparison failure) |

[Observation records, source revisions, artifacts and observers](component-observations.json)

<!-- /component-observations -->



On 2026-11-27 a read-only `status` read of all six validators found the
release `pq-r3` running, with one tip and one state root
([Current State](chain-state-current.md)). The 2026-10-30 deployment is still
the last one with a deployment receipt on that page. Since 21 November `main`
is the only long-lived branch; the running build `9dcb689e` is not a commit of
`main` ([Master State](master-state.md)).

| Item | Value |
| --- | --- |
| Running deployment | `pq-r3`: build revision `9dcb689e` on all six validators since the evening of 2026-11-26. It carries PFSH03 post-quantum private payments, activated at block 1910 (27 November handoff). `9dcb689e47a4` is a commit of #280's branch, not of `main`; `main` holds #280 as the squashed, newer `b121df606` |
| Its node binary SHA-256 | `c563a4dcf48ad4359ab0f37fb7a626efed2b2b6808fc1ebd57e2963900326fb6` (all six, 2026-11-27 read) |
| Earlier releases | `sol-exit-r3` (`2356caf1`, 2026-11-25), the second Solana exit release (`04600868`, 2026-11-24, rules from block 1890), `sol-exit-r1` (`71dbc9b3`, 2026-11-23, rules from block 1841) and `main-r64-20261121` (`674dbb18a`, 2026-11-21); all are on `main` |
| Last deployment receipt | `vote-records-20261030` (source `ae70d66d`), 2026-10-30, with `scripts/postfiat-safe-rollout apply-next` |
| Latest verified height | 1915 on all six validators (state root `ef5e41baae1b…`, tip `ef4b9c66d62e…`), 2026-11-27 09:01 UTC |
| Latest fleet observation | 2026-11-27 09:01:27–09:01:34 UTC, read-only `status` on all six through local forwards: one tip, one state root, one deployment manifest (`25edebe2ba40…`, verified on all six), empty mempools |

Changes merged to `main` are not evidence of deployment. Nothing merged to
`main` after the running build was cut is deployed: #257, #274, #281, #285,
`b121df606` itself and every later commit. A `main` commit runs on the devnet
only once the validators report a build that contains it.

`main`'s lane-export identity check (`build-lane-export-identity`) fails from
`b121df606` on, because #280 changed the export program's inputs; the 27
November handoff calls this expected and leaves it to its own step.

The chain makes blocks only when there are transactions to order, so height
does not advance with wall-clock time. A height that has not moved for days is
normal on this devnet.

## USDC bridges

Both routes are part of this controlled devnet. The Solana route runs with
small caps, and its withdrawals have not been announced publicly.

**When you can deposit and withdraw (since 26 November).** Deposits from
Solana and withdrawals to Solana or Ethereum need a proof, and proofs are now
made only while a proving session runs. The plan is one session a day, open
from 17:00 to 19:00 UTC. Outside it, Solana deposits and both withdrawal paths
are closed on purpose, to cut cost: a deposit is not credited and a
withdrawal is not paid until the next session. On 26 November at 17:13 UTC the
window was not open: the Task Node showed Solana deposits and withdrawals
closed. The window's launcher (#257) was merged into `main` on 27 November at
00:41 UTC, with two changes of the evening of 26 November (ask for a GPU two
hours ahead and alert when deposits stay closed; rent a Vast 5090 for deposits
when Colab has no G4); by the 27 November handoff it is not deployed. A day can
pass without a session. The Task Node's opening-hours screens (tasknode #442
and #445) are merged in the Task Node's `main` and not deployed, and the
setting that switches them on, `L1_PROVING_WINDOW_UTC`, stays unset until
follow-up work on those screens is done, so the Task Node does not say when
a route reopens
([26 November handoff](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/handoff-2026-11-26.md),
[27 November handoff](../handoffs/2026-11-27___antigravity__pfsh03_merged_big_milestone_specs_and_wave1_to_dravlic.md)).

- **Solana USDC, both directions.** Deposits mint pfUSDC through the Task Node;
  withdrawals burn pfUSDC and pay USDC on Solana. Withdrawals have run since the
  first exit release (rules from block 1841, 2026-11-23); since the second
  (block 1890, 2026-11-24) every Solana deposit batch can be withdrawn, a burn
  that cannot be paid has a refund path, and each validator reviews a
  settlement itself before it signs. Since `sol-exit-r3` (2026-11-25) a
  payout or refund settles with five of six validators. Real withdrawals of
  0.30, 0.30 and 4.00 USDC were paid and settled on 24 November. During the roll of
  `sol-exit-r3`, from 21:03 to 21:32 UTC on 25 November, withdrawals were
  closed.
  Limits today: one withdrawal spends one deposit batch; withdrawals below 0.30
  USDC are refused; the vault accepts 25 USDC more; a withdrawal takes about
  four minutes once a proving session is running; cancel is refused while the
  route is paused. Not yet run on the live chain: a cancel and refund, a
  settlement with one validator down, and a withdrawal proved in the daily
  session (26 November handoff). The services read Solana through free public
  endpoints.
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
  Moving both back to unmodified `main` is open. The proving machines that ran
  all day were stopped on 26 November; proofs come from the daily session above.

Sources: the [26 November handoff](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/handoff-2026-11-26.md),
the [25 November handoff](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/handoff-2026-11-25.md),
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
  Validator 5 holds a FastPay certified payment without signing since its key
  rotation and answers with an unsigned held-effect receipt; #239, merged on
  25 November, makes the wallet proxy end the delivery on that receipt
  (26 November handoff).
- **Validator keys.** The recorded fleet uses software key files. Main now
  implements a separate signer process with a Unix socket, peer-UID checks and
  a durable double-sign guard. Unit generation for the signer, RPC and transport
  services is implemented; its qualified deployment under two users remains open. HSM
  custody is not implemented. See the
  [signer policy](../runbooks/public-rpc-operator-policy.md#validator-signer-process).
  File-signer mode needs `--unsafe-devnet-file-signer`. Services on JSON/JSONL
  or not-yet-activated storage need `--unsafe-devnet-json-storage`; that flag
  bypasses the startup check and does not select JSON storage.
- **Private wallet path.** PFSH03 payments were made end to end from the
  signed Mac app at blocks 1912 to 1914 (27 November handoff; the blocks were
  read on 27 November). The Task Node's private relay for them (tasknode #444)
  is merged and, by that handoff, not deployed. For the Orchard pool, public
  submission of shielded actions was off on every validator at the 23 November
  handoff. See [Private payments](../quantum/private-payments.md) and the
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
