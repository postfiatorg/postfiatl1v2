# pfUSDC Tier-4 Gate 2A conformance evidence

This is implementation/conformance evidence for the frozen ingress V3
statement. It is **not** Core Gate 2 acceptance evidence and contains no SP1
proof or credited deposit.

- source commit: `0b68a5be71c80d1cdc89d12e5c7cfe77b1eb831f`
- worktree during checks: dirty with exactly the changes committed above
- Nitro source commit: `a618155919315241665356fe60f3cd00d66d5e46`
- Nitro contracts source commit: `4341b132cfbdcc980ead03765ca5224ff6cb5d97`
- latest-confirmed storage slot: `0x74`
- controlled proof pair: Ethereum Sepolia `11155111` / Arbitrum Sepolia `421614`
- production allowlisted pair: Ethereum `1` / Arbitrum One `42161`

The fixed Nitro send-item, send-root, BoLD assertion-state, and assertion-hash
vectors are recorded in
`docs/specs/pfusdc-nitro-sendroot-conformance.md` and exercised by the ingress
library tests.

At the sampled finalized Ethereum blocks, `eth_getProof` for storage slot
`0x74` exactly matched the result of `latestConfirmed()` on both allowlisted
Rollup proxies. Both sampled proxies had runtime code hash
`0x5961c8c303762fe3bdc69d0df28db034e475e46ef4e3582c632eaaa51314ce29`.

No GitHub Action, workspace-wide test battery, SP1 build, or SP1 proof was run
for this evidence.
