# Cobalt adversarial verification E6

This checksum-bound packet records the design-only E6 decision. The
independent-operator gate from the locked activation specification is
reinstated as its own mandatory follow-on milestone. Its source pins were
revalidated on 2026-08-26 after the E5 authority-lineage hardening and the
post-campaign evidence-review corrections.

The decision does not recruit operators, authorize a live migration, or claim
that the controlled testnet is independently operated. Cobalt currently
ratifies validator-registry and trust-graph changes on a
Foundation-administered controlled testnet. Consensus v2 remains block
finality.

The locked design specifies:

- a canonical proposal envelope signed by an admitted operator master identity
  and its active validator hot key;
- bounded authenticated submission with byte-preserving untrusted relays;
- deterministic proposer and view selection instead of the current implicit
  first-validator proposer;
- existing Cobalt RBC, ABBA, MVBA, and DABC ratification followed by scoped
  five-of-six current-registry authorizations;
- six independent operators with one validator each and at least three
  infrastructure domains; and
- explicit evidence gates for a later live migration.

With six validators and quorum five, one operator controls one vote and cannot
reach quorum. Removing one operator leaves five validators, so no single
operator can block quorum alone. The retained independent-operator onboarding
contract remains the admission boundary and must be refreshed to the live
release and roots before that later milestone.

Verify from the repository root:

    python3 benchmarks/cobalt-adversarial-verification/e6/verify_packet.py

Expected first line:

    e6-packet-ok
