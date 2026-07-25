# Persistent PFTL handoff adapter

The public handoff is consumed as a digest-pinned release manifest. The
default dry check is read-only, does not connect to LND, does not load a signer,
and does not submit a transaction:

```bash
PYTHONDONTWRITEBYTECODE=1 \
python3 -m tools.lightning_navcoin_demo.real_value.pftl_handoff_check
```

It requires six distinct, converged validators; the exact chain, genesis,
build, binary, helper, asset, non-freezable controls, NAV epoch, reserve packet,
profile and coordinator pins; and an empty mempool. Its JSON preserves the
handoff's assurance boundary: the proof bytes are stored and hash-bound under
the `multi-fetch-quorum` profile, while consensus-native Groth16 verification
is `false`.

`PersistentHandoffPftlBackend.from_pinned_release()` is the concrete
coordinator backend. `plan_create()` is read-only and supports both coordinator
and user-owned escrow plans. A user-owned reverse-flow plan is never eligible
for the coordinator signer. Before it returns, the planner requires six
identical owner and recipient account/trustline views, sufficient owner
inventory, sufficient recipient headroom, and exact six-view create/finish fee
quotes whose post-fee balances remain above the protocol account reserve.
`submit_create()` remains coordinator-only.

The signer path is private runtime configuration, never part of the public
handoff, policy, dry-check output, or effect journal. The adapter checks only
its filesystem metadata and passes the path to the pinned `postfiat-node`; it
does not open or copy key contents. Signing/certification requires all of:

- `SignerHandle` for the exact handoff coordinator address;
- a private `PftlEffectStore` SQLite/WAL journal;
- a mode-0700 artifact directory;
- the exact `I_ACKNOWLEDGE_PINNED_PFTL_CHAIN_MUTATION` execution
  acknowledgement.

The journal binds every effect key to one request digest and signer sequence,
stores a hash of the protected signed artifact, and reconciles literal
six-of-six `accepted` receipts. After a crash in the certification uncertainty
window, an effect stays `SUBMITTING`; automatic duplicate submission is
refused until the existing transaction is reconciled. This is a deliberate
safety hold because the current pinned certification helper has no
protocol-defined resume operation.
