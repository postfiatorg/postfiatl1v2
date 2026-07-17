# Write Policy

The controlled network does not expose unrestricted public write RPC by default.

## Why

Write exposure changes:

- abuse surface;
- spam pressure;
- validator CPU load;
- verifier load;
- rate-limit policy;
- support burden;
- public operator risk.

## Current Policy

- Read RPC is the default public-facing posture.
- Controlled writes are used for live evidence and wallet finality.
- Temporary bounded write edges are acceptable for controlled tests.
- Persistent public write exposure requires explicit policy, rate limits,
  abuse controls, and operator acceptance.
- Privacy writes are even more sensitive because proof verification is
  expensive.

## Source

- `docs/runbooks/controlled-write-edge-policy.md`
- `docs/runbooks/public-rpc-operator-policy.md`
- `scripts/testnet-controlled-write-edge-policy-audit`
- `reports/testnet-controlled-write-edge-policy/`
