# Threat model

## What is enforced

- Both ledgers bind release to the same 32-byte SHA-256 preimage.
- EVM principal is held by deployed contract code, not the coordinator.
- PFTL principal is held by the hardened escrow state machine.
- Each leg has an independently enforced refund boundary.
- Terminal state prevents claim/refund replay.
- Redeemed events expose authenticated preimages publicly.
- Exact ERC-20 and NAVcoin atoms are conserved, including open escrows.

## What is not enforced

- Neither ledger proves a relationship between EVM wall time and PFTL height.
- The prototype does not guarantee transaction inclusion, fee liveness, or
  monitoring availability.
- The mock token is intentionally not real USDC and Anvil is not a public
  adversarial network.
- The coordinator chooses timing margins; conditional atomicity depends on
  those margins remaining adequate.

## Fail-closed boundaries

Wrong preimages, early refunds, EVM redeems at or after the deadline, late PFTL
finishes, and duplicates are rejected without state mutation. The evidence
bundle records pre/post-state checks and the independent verifier re-reads
terminal state from both ledgers.
