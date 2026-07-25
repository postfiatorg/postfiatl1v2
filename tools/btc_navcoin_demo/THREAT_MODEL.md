# Threat model

## Enforced by the ledgers

- The Bitcoin claim path requires a 32-byte value hashing to `H` and the
  recipient signature.
- The Bitcoin refund path requires the owner's signature and absolute CLTV
  height.
- The PFTL claim requires its canonical PREIMAGE-SHA-256 fulfillment before its
  block-height deadline.
- Each ledger conserves principal independently; Bitcoin miner fees are
  explicit.

## Operational assumptions

- Both parties monitor both ledgers and can submit before their safety margin.
- Signet block production and fee inclusion remain live.
- PFTL finality and Bitcoin confirmations are observed before the second lock.
- The first timeout is long enough relative to the second timeout despite
  unrelated block clocks.

## Explicit non-claims

- This is not a mainnet, production, or real-value bridge.
- No protocol proves a conversion rate or a relationship between Bitcoin and
  PFTL block heights.
- Bitcoin's hash branch does not expire at CLTV. Once CLTV matures, claim and
  refund can race until one confirms.
- Conditional atomicity does not guarantee completion when either party aborts;
  it guarantees the available claim/refund outcomes under the stated timing and
  liveness assumptions.

## Mutation-free adversarial probes

Bitcoin wrong-preimage and early-refund candidates use Core's
`testmempoolaccept`, which performs validation without adding a transaction to
the mempool. The late claim is tested after a confirmed refund and is rejected
as a double spend. PFTL wrong-preimage, early-cancel, and late-finish candidates
are rejected before signing/submission. Confirmed duplicate Bitcoin broadcast
is rejected without another state transition.
