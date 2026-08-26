# Cobalt E5: Live Authority Drills

## Result

**PASS.** This checksum-bound packet records the live six-validator controlled-devnet authority drills at heights 920 through 924 on `postfiat-wan-devnet-2`.

The final gate uses the corrected signed rollback at height 922 and the separately authorized return to Cobalt at height 923. All six validators accepted one history and Consensus v2 block finality continued without interruption. At height 924, Cobalt committed a legitimate validator-5 key rotation with five current-validator authorizations. An attempted rotation using the treated-as-stolen validator-5 key rejected without durable mutation.

All nine required negative cases rejected: early, stale, replayed, wrong-root, cross-chain, mixed-authority, self-authorized, replayed rollback, and stolen-key rotation. Cobalt remained active for validator-registry ratification after the drill. Consensus v2 remained responsible for blocks.

The first accepted rollback/return pair at heights 920 and 921 is retained in the packet. The height-921 return used a trust binding that did not match the protocol-native post-return graph. No conflicting root, fork, or finality interruption occurred. The issue was corrected by the signed height-922 rollback and height-923 return before the legitimate rotation. The packet does not erase that remediation history.

## Authority and operator boundary

Cobalt ratifies validator-registry and trust-graph changes. A separate layer decides which validators deserve trust. These drill proposals and authorizations originated from Foundation-administered validators. The result proves protocol capability, not operator decentralization.

The packet contains public signed protocol artifacts, checksummed fleet observations, and redacted operational receipts. It contains no validator signing key, seed, recovery material, or replacement-key file.

## Evidence map

- `authority-history.json`: accepted transition chronology and the designated final-gate pair.
- `h920-transition.json` through `h923-transition.json`: signed authority transitions.
- `h924-registry-update.json`: signed Cobalt validator-trust update and decision certificate.
- `h924-block-proposal.json` and `h924-block-certificate.json`: Consensus v2 commitment for the legitimate rotation.
- `finality-history.json`: all-six, height-920-through-924 accepted-history receipt.
- `fleet-after.json`: final all-six validator/RPC/shadow observation.
- `negative-cases.json`: nine live rejection results and before/after durable-state hashes.
- `rotation-operations.json`: public key-stage, stale-key, trust-view, and six-shadow lineage receipts.
- `source-pins.json`: source, binary, observation-time, and raw operational-artifact digests.
- `verifier.json`: expected semantic verification result.

## Verify

From the repository root:

```bash
python3 benchmarks/cobalt-adversarial-verification/e5/verify_packet.py
```

The verifier authenticates the checksum manifest, transition linkage and authorizations, finality continuity, the h924 update and block certificate, all nine rejections, durable-state preservation, key-stage result, six-shadow lineage, final fleet convergence, source pins, and the redaction boundary.
