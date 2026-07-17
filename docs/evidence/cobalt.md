# Cobalt Evidence

Cobalt evidence proves the controlled governance mechanics, not public operator
decentralization.

## Current Main Packet

`reports/testnet-cobalt-controlled-readiness-gate/amendment-replay-contract-clean-v0-20260519T145213Z/testnet-cobalt-controlled-readiness-gate.json`

## Supporting Evidence

- `reports/cobalt-safety-witness/20260526/cobalt-safety-witness-report.json`
- `reports/testnet-cobalt-gate-selection/amendment-replay-contract-clean-v0-20260519T145213Z/testnet-cobalt-gate-selection-self-test.json`
- `reports/testnet-cobalt-amendment-replay-bundle/cleanup-clean-v1-20260519T150324Z/testnet-cobalt-amendment-replay-bundle.json`
- `reports/testnet-cobalt-adversarial/`
- `reports/testnet-cobalt-controlled-launch-gate/strict-expected-fail-clean-head-v0-20260519T1438Z/testnet-cobalt-strict-launch-expected-fail.json`
- `reports/cobalt-cover-extractor-v1-report.json`
- `reports/cobalt-cover-sizing-v1-report.json`

## Interpretation

The Cobalt mechanics pass controlled readiness. The safety-witness checker also
has a local consensus-crate report showing accepted bounded rotation and
fail-closed rejection for stale roots, open challenges, oversized covers, and a
large unsafe old/new registry delta. The cover-extractor report shows that the
old/new cover is derived from the rooted trust graphs, not supplied by the
proposer, and that the witness path can reject omitted cover rows. The strict
public topology gate records the remaining public-placement deltas. The cover
sizing report checks 35- and 100-validator grouped trust graphs under the
current `max_cover_subsets=64` profile.
