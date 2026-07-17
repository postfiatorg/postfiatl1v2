# Cobalt Governance

Cobalt supplies trust-graph checks and agreement research for validator trust
evolution. It is not the current node's live governance authorization oracle.

In XRP-style systems, the hard question is not only how validators order a
payment. It is how the network changes who it trusts. PostFiat uses the Cobalt
research lineage to make those trust changes explicit, signed, linked, ratified,
and replayable.

## Plain English

Validators can hold local trust views. A trust graph is acceptable only if the
views overlap enough through essential subsets and thresholds. If a proposed
graph is unsafe, it fails before activation. The target Cobalt design moves
valid amendments through reliable broadcast, agreement, and democratic atomic
broadcast mechanics. The implemented live boundary is narrower: a governance
batch must carry distinct ML-DSA-65 authorizations from the active old-rule
registry and then enter ordinary consensus ordering.

## Why This Exists

An off-chain validator history service is not good enough as the long-term
coordination mechanism for a settlement network. Validator-set changes should
be part of the protocol evidence trail.

## What PostFiat Runs

PostFiat separates live authority from Cobalt research:

- ordinary transactions and signed governance batches use certified ordering;
- the active old-rule registry authorizes live amendments and validator updates;
- Cobalt mechanics validate and exercise a stronger future trust-evolution
  design without silently acquiring live authority.

## Current State

The controlled Cobalt mechanics are built:

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

The live signed-governance path separately covers complete action binding,
distinct old-registry authorization, delayed activation, registry rotation,
rollback, restart, and replay tests. It does not make the stronger Cobalt
ratification claim. Current controlled Cobalt evidence uses project-controlled
logical validators; strict public topology evidence is a public-launch
requirement, not a source-publication blocker.

## Read Next

- [Cobalt Implementation](cobalt-implementation.md)
- [Validator Registry](validator-registry.md)
