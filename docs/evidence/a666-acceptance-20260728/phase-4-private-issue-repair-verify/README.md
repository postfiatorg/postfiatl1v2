# A666 Phase 4 Private Issue Repair and Verification

Status: `IN PROGRESS`

The frozen live-value intent is:

```text
1.005000 Ethereum USDC
  -> 1.005000 transparent PFTL pfUSDC
  -> 1.005000 private pfUSDC
  -> 1.000000 newly issued private A666
```

The private-primary action uses one proof to consume the private pfUSDC note
and a second proof to establish that the encrypted output commitment contains
exactly the authorized A666 amount. No Uniswap trade or operator A666 inventory
is involved.

This first-generation output-validity construction publishes a proof-only
nullifier for the new A666 note. It does not reveal the note opening, but it is
linkable to that note's later spend. This residual leakage is explicit and is
not described as end-to-end privacy.
