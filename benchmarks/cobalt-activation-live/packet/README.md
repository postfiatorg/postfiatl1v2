# Cobalt controlled-testnet activation packet

This packet records the terminal result: **ACTIVATE / ACTIVATED**.

Cobalt became the live authority for validator-registry and trust-graph changes at height 916. A Cobalt-authorized validator-5 ML-DSA key rotation committed at height 917. Consensus v2 continued to own block finality, recovered an abandoned height through timeout certificates, and converged all six validators at height 919.

The activation addresses the capability tested in the frozen RippleD comparison: a RippleD-style node proves quorum under its local UNL, while Cobalt makes trust-view compatibility part of the registry decision each validator verifies. In the decisive divergent-local-quorum case, the RippleD adapter admitted two registry roots and Cobalt rejected the incompatible trust graph before commitment.

The prior evidence dependencies remain in their original checksum-bound packets. `prior-evidence.json` pins the decisive oracle/corpus, isolated-validator liveness simulation, and release qualification. This packet adds the compact live authority, registry change, view-change recovery, six-node convergence, full state replay, CLI, and read-only UI receipts. It contains no private key material or raw ML-DSA signatures.

Verify from the repository root:

```bash
python3 benchmarks/cobalt-activation-live/packet/verify_packet.py
```

Expected result: `packet-ok` followed by the packet checksum root.
