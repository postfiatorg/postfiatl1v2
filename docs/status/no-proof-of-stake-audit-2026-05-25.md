# No Proof-Of-Stake Audit - 2026-05-25

## Result

No proof-of-stake mechanism was found in executable consensus, state, RPC, or
wallet code.

The current implementation remains an authority-validator design:

- validator registry entries contain `node_id`, `algorithm_id`, `public_key_hex`,
  and `active`;
- Cobalt and HotStuff-style votes are keyed by validator identity, not token
  balance;
- quorum checks are count-based over validator ids;
- the canonical whitepaper monetary invariant states
  `native_issuance_per_block = 0` and `validator_reward_per_block = 0`;
- fees are burned and are not redirected to validators as staking yield.

## Guardrail Added

`scripts/no-proof-of-stake` now scans executable code under `crates`, `python`,
and `scripts` for proof-of-stake terminology that would indicate accidental
mechanism drift, including stake-weighted voting, validator stake fields,
staking rewards, voting power, validator power, and PoS-specific names.

The guard is wired into `scripts/check`.

## Cleanup

Two invalid-RPC test fixtures used the string `staking` only as a bad
`batch_kind` value. They were changed to `unsupported_batch_kind` so future
searches do not produce a false positive.

## Notes

Some docs and research artifacts compare PostFiat against proof of stake or
discuss slashing as a rejected or future-research concept. Those are not
implementation paths. The guard intentionally checks executable code and
whitepaper monetary invariants rather than banning comparative prose.
