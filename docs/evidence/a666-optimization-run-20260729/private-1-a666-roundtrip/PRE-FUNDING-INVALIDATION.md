# Pre-funding invalidation

**Invalidated:** 2026-07-29 UTC  
**Value moved:** none  
**Superseding lineage:** `../private-1-a666-roundtrip-nav-aware/`

This frozen input set was invalidated before its Ethereum deposit. Its issue
builder used `1.005000 USDC` for `1.000000 A666`, which omitted the
policy-pinned `$0.90103113` NAV from primary-issue pricing.

Consensus would have rejected the resulting subscription because the
canonical amount is:

```text
base_value_atoms = ceil(1,000,000 × 90,103,113 / 100,000,000)
                 = 901,032

issue_due_atoms  = ceil(901,032 × 10,050 / 10,000)
                 = 905,538
```

No deposit, PFTL transition, proof job, Ethereum mint, burn, or withdrawal
used this lineage. Its identifiers MUST NOT be submitted. It remains in the
evidence history solely to document the pre-funding fail-closed decision.

