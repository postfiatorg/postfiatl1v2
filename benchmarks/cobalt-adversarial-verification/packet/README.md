# Cobalt Adversarial Verification: Final Packet

Result: **KEEP_ACTIVE**

Cobalt ratifies validator-registry and trust-graph changes. A separate layer
decides which validators deserve trust. Current proposals and authorizations
originate from Foundation-administered validators.

This packet proves protocol capability on the controlled devnet. It does not
prove operator decentralization, authorize mainnet use, or give Cobalt control
of block finality. Consensus v2 remains responsible for blocks.

The packet authenticates all six experiment packet roots, the final height-924
live authority observation, the signed final-gate rollback and return at heights
922 and 923, the legitimate validator-5 rotation at 924, all nine live rejection
reasons, finality receipts, publication bindings, and the common CLI/browser
view.

The first accepted rollback/return pair at heights 920/921 is retained inside
the E5 packet as remediation history. The corrected h922/h923 pair is the final
gate.

## Contents

- adversarial-status.json: final decision and operator boundary.
- experiments.json: E1–E6 results and packet roots.
- live-authority.json: final all-six authority, transition, rotation, and
  finality receipts.
- rejected-cases.json: every named live rejection and reason.
- source-pins.json: exact experiment SHA256SUMS bindings and E5 packet root.
- publication.json: public article and repository-document bindings.
- cli-output.txt: authenticated human CLI rendering.
- browser-snapshot.json: authenticated read-only observatory snapshot.
- interfaces.json: CLI and browser behavior receipts.
- verifier.json: the complete semantic gate result.

## Verify

From repository root:

~~~bash
python3 benchmarks/cobalt-adversarial-verification/packet/verify_packet.py
PYTHONPATH=python python3 -m postfiat_rpc.cobalt adversarial
~~~

Any missing or changed required file, experiment packet, publication document,
authority field, rejection, interface receipt, or checksum causes verification
to fail.
