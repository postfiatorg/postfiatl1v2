# Network Status

**Guidance reviewed 2026-11-06; latest fleet observation 2026-11-13.**
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

The 2026-10-30 deployment is the last one with a deployment receipt in
[Current State](chain-state-current.md); the 2026-11-13 read found a later
deployment running, for which no receipt is recorded yet.
The [November 6 handoff](../handoffs/2026-11-06___dravlic__first_real_base_sepolia_lock_released_and_bridge_phase_2_complete.md)
reports read-only probes of all six hosts with the same executable, height, tip
and state root. This documentation refresh made no new fleet probe; the handoff
does not re-certify every October deployment check.

| Item | Value |
| --- | --- |
| Running deployment | Build revision `abc228ed` and deployment manifest SHA-256 `19e74906…` on all six validators; no deployment receipt recorded yet |
| Previous release (last deployment receipt) | `vote-records-20261030` (source `ae70d66d`, branch `release/vote-records-20261030`) |
| Its node executable SHA-256 | `87cfe797af8c8c1c98e08b4794473bd25acb58ce0b7f57f0ea1b1f0fe98023c7` |
| Its rollout | One validator at a time with `scripts/postfiat-safe-rollout apply-next`, 2026-10-30 10:31–11:14 UTC; no rollback needed |
| Height | 1271 on all six validators (state root `900fedd4…`) |
| Latest fleet observation | 2026-11-13 10:51 UTC, read-only by the operator on all six: status running, mempools empty, `deployment_manifest_verified` false. Since 1209: 62 transactions, all accepted — transfers, asset transactions, offers (two fills at 1270–1271) and the governance activation of the vault bridge route `ethereum-mainnet-usdc-v1` at route epoch 11 (block 1256) |

Changes merged after that release are not evidence of deployment. The
[next-release plan](../plans/active/next-validator-release-plan-20261115.md)
separates source changes from the candidate cut and its qualification.

The chain makes blocks only when there are transactions to order, so height
does not advance with wall-clock time. A height that has not moved for days is
normal on this devnet.

## Other running components

- **Storage.** The transactional `redb` storage engine has been active on the
  devnet since height 930 (2026-08-31). The storage-scaling milestone's offline
  qualification packet (gate G5) passed on 2026-10-23 for source `86706699`,
  binary `eea96ba6…`. G6 remains open; this packet does not automatically
  qualify later binaries. See [State And Storage](../architecture/state-and-storage.md).
- **FastPay and FastSwap.** FastPay's epoch-2 recovery committee was installed
  at height 1123 (2026-10-12). The FastSwap service runs at committee epoch 2
  on all six validators since 2026-10-21. These are certificate lanes beside
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
- **Private wallet path.** Present in the recorded release, but admission was
  off and pricing/egress governance incomplete at the last observation. Source
  and local tests do not establish a live private-wallet service. See the
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
`exports_enabled`, default off. The Base verifier deployment, export prover
and relay remain open. These are source/test results, not a Base mainnet
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
