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

## Evidence

- `reports/testnet-ml-dsa-performance/`
- `docs/status/controlled-testnet-burndown.md`
- `docs/runbooks/validator-history-retention.md`
