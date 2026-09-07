# Signature Size And Certificates

Post-quantum signatures are larger than classical elliptic-curve signatures.
That affects:

- transaction size;
- block and certificate size;
- validator bandwidth;
- storage and archive growth;
- wallet and custodian UX;
- RPC payload limits.

PostFiat accepts this cost because the design values post-quantum authorization
from genesis. The mitigation is not to pretend signatures are small. The
mitigation is bounded certificates, fee/resource pricing, partial-history roles,
and explicit performance evidence.

## Actual V2 encoding versus illustrative arithmetic

`ConsensusV2Signature` in `crates/types/src/consensus_v2_types.rs` contains an
algorithm identifier, signer, public key and signature, with key/signature
material represented as hex. A V2 commit embeds proposal and prepare/precommit
evidence and can include timeout ancestry. The whitepaper's
`24 × (3309 + 32) = 80,184` and `67 × (3309 + 32) = 223,847` examples account for
one simplified signature set; they omit public keys, framing and multiple
stages. They are not measured V2 wire sizes or proof of detached transport.

The provider's ML-DSA-65 constants are 1,952-byte public keys and 3,309-byte
signatures. Actual release budgeting must measure the serialized artifacts and
all verifier work at the intended committee size. The historical throughput
report below is absent from the audited public checkout. See the
[alignment audit](../architecture/whitepaper-alignment.md#9) and
[measurement backlog](../architecture/whitepaper-gaps.md#g07-original-measurement-provenance).

## Historical evidence references

- `reports/testnet-ml-dsa-performance/`
- `docs/status/controlled-testnet-burndown.md`
- `docs/runbooks/validator-history-retention.md`
