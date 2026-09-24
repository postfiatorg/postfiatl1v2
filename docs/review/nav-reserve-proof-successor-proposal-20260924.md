# Reserve-proof successor proposal, 2026-09-24

For the governance-registration lane. The code is in
[PR #49](https://github.com/postfiatorg/postfiatl1v2/pull/49)
(branch `reserve-proof/freshness-and-debt-20260924`). It is not merged. The
findings are in the
[reserve-proof mechanics review](nav-reserve-proof-mechanics-review-20260924.md).

## What changes

1. **NEAR stake must be fresh (N1).** A NEAR balance snapshot now counts only
   if it is recent enough relative to the certified head. The policy sets two
   limits: a maximum age in time and a maximum age in blocks. Today, a
   snapshot of any age is accepted.
2. **All Aave debt must be listed (A1).** The proof now reads the owner's Aave
   account bitmap. It fails if the owner is borrowing any asset that the
   policy does not list. Today, only the listed USDC debt is subtracted.
3. **One committee key, one vote (B1).** A committee that lists the same key
   under two names is rejected.
4. **One owner, one entry (M1).** A manifest that lists the same owner twice in
   the same chain is rejected, so the same reserve cannot be counted twice.

All four are in `tools/nav-reserve-proof`. L1 consensus code is unchanged.
Each has a regression test that fails on the old logic and passes after the
fix.

## What you must do to adopt it

1. **Set the new policy values** in `manifests/a666`, then regenerate the
   commitments, source manifest and profile registration. The proposed values
   are:
    - `max_receipt_age_ns = 1800000000000` (30 minutes);
    - `max_receipt_age_blocks = 3000`;
    - `user_config_mapping_slot_index = 53`, the Aave v3 Pool `_usersConfig`
      slot.

    In epochs 7 and 8, the receipts were 210 and 269 seconds (331 and 437
    blocks) below the head, so these bounds leave a wide margin.
2. **Build the successor identity.** Build it twice with the pinned SP1 6.3.1
   Docker image, using the commands in
   `qualifications/freshness-and-debt-20260924/candidate-identity.json`. That
   file is a candidate, not a registered identity. It was **not built here**
   because Docker and cargo-prove are unavailable on this host. Do not
   overwrite `elf/` or either existing identity file.
3. **Register the new profile through governance.**
4. **Re-qualify the successor epochs.** The Aave inputs must be collected
   again, because they now need the `_usersConfig` proof.
5. **Migrate A666 off the legacy `sp1-groth16` path** (G1). This proposal
   does not change that path, and the live NAV still uses it.

## Risk of not doing it

When A666 moves to the open-kit proof, the issuer could do either of these:

- count NEAR stake that has already been withdrawn, using an old snapshot;
- borrow an unlisted Aave asset against the proven collateral.

Either would overstate net assets while the proof reports them as fully
cryptographic. B1 and M1 have no current exposure: the A666 committee keys
and owners are distinct. Without the fixes, they depend on manifest review
alone.

## Estimate

About one working day before registration: 2 hours for the policy values and
regenerated manifests, 2 hours for two reproducible builds, and half a day to
re-qualify the epochs. Governance registration then takes its normal
proposal and voting time. The `sp1-groth16` migration is separate work.

## Answer

> **Adopt: yes / no**
