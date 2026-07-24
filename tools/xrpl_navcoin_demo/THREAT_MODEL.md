# XRP ↔ NAVcoin Escrow Lane Threat Model

## Claim

This lane is **non-custodial, conditionally-atomic, coordinator-trusted
timing**. It is not a trustless cross-chain bridge.

XRPL and hardened PFTL verify the same PREIMAGE-SHA-256 relation independently.
Neither ledger verifies the other ledger's finality or clock. The coordinator
therefore gates the second lock using recent validated-ledger observations and
promises an operational PFTL height cadence. A malicious or failed coordinator
can deny service or violate the intended timing margin, but cannot claim either
hashlocked principal without the preimage or a timeout-authorized refund.

## XRP → NAVcoin

1. User generates `S`; `H = SHA256(S)`.
2. User locks XRP to coordinator on XRPL with the **long** `CancelAfter`.
3. Coordinator observes validated XRPL finality and checks remaining margin.
4. Coordinator locks NAVcoin to user on PFTL with the **short** cancel height.
5. User finishes the second/PFTL escrow first, revealing `S`.
6. Coordinator reads `S` from finalized PFTL history and finishes the first/XRPL
   escrow before its longer expiry.

## NAVcoin → XRP

1. User generates `S`; `H = SHA256(S)`.
2. User locks NAVcoin to coordinator on PFTL with the **long** cancel height.
3. Coordinator observes certified PFTL finality and checks remaining margin.
4. Coordinator locks XRP to user on XRPL with the **short** `CancelAfter`.
5. User finishes the second/XRPL escrow first. The validated EscrowFinish
   transaction publicly contains the fulfillment and therefore `S`.
6. Coordinator reads `S` from validated XRPL history and finishes the
   first/PFTL escrow before its longer expiry.

## Safety invariants

- Both escrows bind byte-identical crypto-condition data; only JSON hex casing
  differs: uppercase on XRPL and canonical lowercase on PFTL.
- User is always first locker and secret holder; coordinator is always second
  locker. The second mover claims first.
- No second lock is admitted from provisional XRPL results or uncertified PFTL
  state.
- A finish is never signed or submitted at or after its cancel boundary.
- An early cancel is never signed or submitted.
- Every signed XRPL transaction is persisted with transaction hash,
  `Sequence`, and `LastLedgerSequence` before submission.
- Duplicate request keys must return the original outcome or fail on payload
  conflict. A different transaction is never synthesized as a retry.
- NAVcoin conservation is exact in atoms. XRP conservation is exact in drops
  after separately accounting for XRPL transaction fees and faucet inflows.
- Rejected adversarial requests are stopped before signing/submission so their
  principal, escrows, account sequence, and fees are mutation-free.

## Failure and recovery

- If only the first lock exists, its owner refunds after its long timeout.
- If both locks exist but the user never claims the second lock, the
  coordinator refunds its short lock first; the user later refunds the long
  lock.
- If the second lock is claimed, `S` is public. The first-lock recipient can
  finish without cooperation from the secret holder, subject to the remaining
  coordinator-trusted timing margin.
- Database/process recovery reconciles by immutable XRPL transaction hash and
  PFTL transaction/escrow ID before submitting any new effect.

## Explicit non-claims

- No trustless mapping exists between XRPL close time and PFTL block height.
- This prototype does not prove censorship resistance, public-validator
  diversity, or mainnet readiness.
- Testnet XRP has no monetary value. The NAVcoin lane remains the dedicated
  hardened devnet and is not ce22.
