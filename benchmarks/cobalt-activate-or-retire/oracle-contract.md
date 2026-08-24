# Cobalt Decisive Oracle Contract v1

This contract classifies validator-governance scenarios before either production adapter runs. The oracle implementation is `crates/cobalt_decision_oracle`; that crate depends only on serialization and SHA-256 libraries. It must not import, call, copy, or link `consensus_cobalt`, `node`, RippleD, or either benchmark adapter.

## Inputs

Each scenario fixes:

- validator identities;
- correct, unavailable, and actively Byzantine nodes;
- every validator's explicit essential subsets `(S, q_S, t_S)`;
- every validator's local UNL and RippleD-style local quorum;
- candidate registry roots and their signed supporters;
- the event schedule; and
- the validator-registry transition kind.

An essential subset is valid only when:

- `0 < q_S <= n_S`;
- `t_S < 2q_S - n_S`; and
- `2t_S < q_S`.

Subset identity is the SHA-256 of the canonical tuple `(sorted validators, q_S, t_S)`. A shared subset must therefore have identical membership and parameters in both trust views. Human labels do not create linkage.

## Cobalt classification

Two responsive correct nodes are fully linked when their views contain at least one identical essential subset for which:

- actively Byzantine members are at most `t_S`;
- responsive non-Byzantine members are at least `q_S`; and
- `t_S <= n_S - q_S`.

A node's known closure is the transitive union of validators in its essential subsets. The closure is strongly connected when every pair of responsive correct nodes inside it is fully linked.

A node sees strong support for a candidate root when at least `q_S` supporters belong to every essential subset in its view.

The scenario is **compatible** only when:

1. every responsive correct-node pair is fully linked;
2. every responsive correct node has a strongly connected known closure; and
3. exactly one candidate root has strong support at every responsive correct node.

Every responsive correct node is predicted to decide that root in a compatible scenario. Every responsive correct node is predicted to halt in an incompatible scenario. A node that remains unavailable is reported as unavailable, not as a halt. If the event schedule restores it before the observation boundary, it is evaluated normally.

This is a deliberately conservative activation contract. It does not claim to mechanize every theorem in the Cobalt paper. It defines the exact behavior Post Fiat requires before granting validator-trust authority.

## RippleD local-UNL admission

For each node and candidate root, count supporters that belong to the node's local UNL. The node predicts:

- **decide root** when exactly one root reaches its local quorum;
- **halt** when no root reaches local quorum; and
- **halt ambiguous** when multiple roots reach local quorum.

This models the local admission rule used for the governance comparison. RippleD's native CSF ledger-consensus run remains a separately labeled control and is never presented as the same protocol.

## Material safety delta

A scenario demonstrates a material safety distinction only when Cobalt predicts no conflicting decided roots while the RippleD local-UNL admission model predicts at least two different decided roots among correct nodes.

## Freeze rule

The final manifest records hashes of:

- the unscored input scenarios;
- this contract;
- the oracle source;
- the production Cobalt and pinned RippleD adapter sources; and
- the canonical manifest with its own hash field blank.

Once Section 1 is accepted, later decision runs may not change the contract, oracle, inputs, expected per-node results, or adapter source pins. An oracle defect discovered after freeze invalidates the campaign; it cannot be reclassified as a Cobalt implementation failure.
