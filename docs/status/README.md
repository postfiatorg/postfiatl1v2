# Status Records

> **Internal engineering record.** This directory holds the operating log and
> dated status records written while the work happened. Most files are
> point-in-time snapshots and many are superseded. For the network as it is
> now, read [Network Status](network-status.md).

## Current pages

| Page | What it is |
| --- | --- |
| [Network Status](network-status.md) | One-page summary of the running network, dated |
| [Master State](master-state.md) | The one line of code, what the validators run, and where archived work is |
| [Current State](chain-state-current.md) | The full, dated operating log: releases, rollouts, fleet observations, drills |
| [Testnet Path](testnet-path.md) | Generated status of the public-testnet path milestone (Gate Zero and phases) |
| [Release Gate Inventory](release-gate-inventory.md) | Non-storage gates that must pass before a public testnet |
| [Handoff of 25 November 2026](handoff-2026-11-25.md) | The latest dated handoff in this directory: the second Solana exit release, its limits and open work (earlier: [23 November](handoff-2026-11-23.md), [24 November](handoff-2026-11-24.md)) |

## Everything else

The other files here are dated burndowns, audits, inventories, experiment
reports and machine-readable manifests (for example the `OPEN-SOURCE-*-20260716`
productionization inventories, the `A666-*` NAVCoin records and the `z3-*` round-trip
records, such as the [frozen G2 read of 2026-11-11](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/z3-g2-selected-state-20261111.json)). Most are not
published on the docs site; browse them in the
[repository](https://github.com/postfiatorg/postfiatl1v2/tree/main/docs/status).
Their file paths are referenced by scripts and tests, so do not move or rename
them.
