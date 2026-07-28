# A666 Phase 3 Private Issue Attempt

**Date:** 2026-07-28

**Gate:** A3 — private issue attempted

**Attempt verdict:** `UNSUPPORTED_PRIVATE_PRIMARY_ISSUE`

**Mutation:** none

The attempt used the production pfUSDC asset, A666 asset, primary policy, NAV
epoch, reserve packet, and route. It stopped at action construction and RPC
dispatch before moving user value.

The deployed node rejected:

- RPC method `asset_orchard_primary_subscribe` as unknown; and
- CLI command `asset-orchard-primary-subscribe-create` as unknown.

The consensus `ShieldedAction` surface contains Asset-Orchard ingress v1/v2,
the conservation swap wrapper, disclosed/private egress, and no
private-primary issue action. The current swap circuit explicitly constrains a
two-input/two-output pair conservation relation. It cannot authorize new A666
supply, reserve growth, issue-capacity use, or a primary export entitlement.

No a651/a652 pair, existing A666 inventory, owner mint, or transparent
subscription was substituted.

## No-mutation result

Before and after the rejected attempt:

- all six validators remained at height 373 with the same tip and state root;
- all mempools remained empty;
- authorized A666 supply remained `31,489.197455`;
- primary settlement reserve remained `103.000000 pfUSDC`;
- non-NAV spread remained `0.515000 pfUSDC`;
- active reservations and export entitlements remained zero; and
- the Asset-Orchard pool report was byte-for-byte unchanged.

No note, nullifier, proof, encrypted output, mempool entry, or consensus round
was created. The public evidence contains the intended production asset IDs
but no note opening, spending key, viewing key, note value, owner, recipient,
or wallet witness.

The machine-readable result is `summary.json`; the exact request is
`attempt-request.json`; source/action-surface evidence is `source-audit.txt`.

This satisfies the attempt gate A3 but is not a functional private-issue PASS.
Phase 4 must add versioned private-primary consensus and proof semantics, then
complete a fresh live run.
