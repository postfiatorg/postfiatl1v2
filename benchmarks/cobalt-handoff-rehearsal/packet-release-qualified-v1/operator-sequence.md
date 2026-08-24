# Disposable Cobalt authority rehearsal

1. Resolve the six current provider identities, capture live service/registry/authority facts, and read the public genesis, tip, and validator registry from validator-0.
2. Build clone manifest `clone-manifest.json` at source commit `8694b99de22e`, anchored to height 915 and future activation height 1015.
3. Request five current-registry ML-DSA-65 transition approvals on the validators; keys never leave the validators.
4. Verify the valid transition, then discard it and record `pre-activation-abort.json` with no clone mutation.
5. Verify and apply the transition to the disposable clone, then run all six negative cases against the signed transition.
6. Build a validator-5 key-rotation update, clone each live Cobalt signer state into a disposable directory, and run a six-validator signed RBC -> ABBA -> MVBA -> DABC decision over the exact update payload.
7. Attach and verify that protocol decision before requesting five scoped validator authorizations; apply the certified update and reject an unrelated crypto-policy amendment.
8. Build a rollback that binds the update lock and new trust root, request five approvals under the updated registry, verify it, and apply it as a second forward transition.
9. Capture live facts again and require validator process/binary/registry/trust/authority fields to be unchanged.

The live fleet is never restarted or written. Consensus v2 remains the only block-finality protocol; this packet does not authorize activation.
