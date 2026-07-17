# Cobalt Governance

Cobalt is the governance layer for validator trust evolution.

In XRP-style systems, the hard question is not only how validators order a
payment. It is how the network changes who it trusts. PostFiat uses the Cobalt
research lineage to make those trust changes explicit, signed, linked, ratified,
and replayable.

## Plain English

Validators can hold local trust views. A trust graph is acceptable only if the
views overlap enough through essential subsets and thresholds. If a proposed
graph is unsafe, it fails before activation. Valid governance amendments move
through reliable broadcast, agreement, and democratic atomic broadcast mechanics
so the network has an ordered governance history.

## Why This Exists

An off-chain validator history service is not good enough as the long-term
coordination mechanism for a settlement network. Validator-set changes should
be part of the protocol evidence trail.

## What PostFiat Runs

PostFiat separates ordinary transaction ordering from Cobalt governance:

- ordinary transactions use the fast certified ordering path;
- Cobalt ratifies validator registry updates, trust graph transitions,
  amendments, and rollback/supersession records.

## Current State

The controlled-testnet Cobalt mechanics are built:

- non-identical trust views;
- essential subsets with `t_S` and `q_S`;
- linkedness checker;
- complete cover extractor for old/new safety witnesses;
- unsafe trust graph rejection;
- non-uniform governance certificates;
- RBC, ABBA, MVBA, and DABC amendment mechanics;
- validator registry and trust graph transitions;
- stale replay rejection;
- amendment replay bundles;
- adversarial packets;
- controlled-readiness gate.

Current controlled evidence uses seven logical validators under
project-controlled infrastructure. Strict public topology evidence is a public
launch requirement, not a controlled-testnet code blocker.

## Read Next

- [Cobalt Implementation](cobalt-implementation.md)
- Cobalt Adversarial Testing
- [Validator Registry](validator-registry.md)
- [Deterministic Governance Agent Plan](deterministic-governance-agent-plan.md)
- Governance Agent Burndown
