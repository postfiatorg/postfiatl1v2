# Glossary

## Account

A transparent ledger identity controlled by post-quantum account authorization.

## Account History

Indexed transaction history for an account, exposed through `account_tx` and the
Python client.

## Cobalt

The validator-governance lineage from XRPL research. In PostFiat it governs
trust graphs, validator registry transitions, amendments, and replayable
governance evidence.

## Certificate

A quorum proof that validators accepted a block, batch, governance artifact, or
other protocol object.

## Controlled Testnet

A project-operated engineering network used to prove protocol and operator
behavior before public launch.

## DABC

Democratic atomic broadcast. In PostFiat docs this refers to the Cobalt
amendment path that orders governance transitions after the lower-level
agreement mechanics accept valid candidates.

## Essential Subset

A Cobalt trust-graph component with threshold parameters `t_S` and `q_S`.
PostFiat checks these before activating a graph.

## ML-DSA

Module-Lattice Digital Signature Algorithm. PostFiat uses ML-DSA-style
authorization for account and validator paths.

## Orchard

The Zcash shielded protocol family used by PostFiat's current Halo2 privacy
path.

## RBC, ABBA, MVBA

Reliable broadcast, asynchronous binary Byzantine agreement, and multi-valued
Byzantine agreement. PostFiat uses these mechanics in the Cobalt governance
implementation.

## Turnstile

The accounting boundary between transparent value and shielded value. Deposits
enter the shielded pool through a transparent funding envelope; withdraws exit
through a bound transparent recipient envelope.

## UFW

Ubuntu Uncomplicated Firewall. The docs server does not edit UFW. The operator
opens the docs port when ready.
