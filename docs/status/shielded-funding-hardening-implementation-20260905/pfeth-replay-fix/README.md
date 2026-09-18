# pfETH devnet replay repair — 2026-09-05

- [x] Reproduce the deployed v3 failure at block 962 from the original public snapshot.
- [x] Verify deployed v1 replays block 962, then exposes the omitted attestor-bond audit at block 963.
- [x] Add exact historical packet compatibility and complete outstanding bond custody accounting.
- [x] Preserve strict family-supply validation for new execution; mutation and custody tests pass.
- [x] Fully import and replay the unchanged 972-block snapshot, rebuilding transactional storage.
- [x] Python native snapshot/signing conformance and strict StakeHub documentation build pass.
- [x] Backed-up six-validator rollout: all twelve services run the exact repaired binary.
- [x] Full deployed validator-0 history audit passes through 972; all six served RPCs and wallet-configured status agree.
- [x] Update obsolete local/remote wallet runtime paths after preserving the prior configuration.

Fleet commit `1bcd0f0d` changed reserve validation from base-asset supply to the
whole source-series family. The older release accepted reserve packets at
heights 962 and 967 with zero base supply, while 5,000,000 pfETH source-series
units existed. Neither packet was finalized. New execution correctly uses
family supply; replay lacked the earlier rule.

The repair selects the old calculation only inside archive replay, bound to
the exact genesis, height, batch, block hash, state root, receipt and native
transaction ID of those two packets. It still executes the transaction and
checks signatures, backing, receipts and state commitments. No broad height
exception or supply-check bypass was added.

The native supply audit separately omitted NAV attestor registration bonds,
NAV challenge bonds and vault deposit challenge bonds. Block 963 locks one PFT
atom as an attestor bond. Counting outstanding bonded custody explains the
apparent deficit; payout clears the counted bond. Duplicate custody fails.

The original public snapshot now imports completely with the original tip:

- Height: `972`
- Block hash: `1ecacd536cba2ae2bc98eea11ab5e46293e094318900c19c5aff5de9aace8386ece25b52afbf281cce67415edb149ca5`
- State root: `3debf11e7ba94ee95cb68e8fdad181ec7b50c56a658addf8fcddcee2e86dca1803c96b79c82949805b98bd1ecb144455`
- Repaired binary SHA-256: `51f0895ca304298f5faef2af3a87f14336ebf7d8acdbf1fba80e62ea6a9991d2`

See `full-snapshot-import.json`, `diagnosis.json`, test logs and the source
manifest. The isolated tested release is based on `707e006f` at
`/home/postfiat/repos/postfiatl1v2-pfeth-replay-fix-20260905`; the patch and public
fixtures are included in `repair-source.tar.gz`. Changes were also applied to
the dirty canonical checkout without replacing its unrelated Arc work. The
existing fleet profile-resolution fix was carried into that checkout too.
The deployed release is an uncommitted patch build, identified by its binary
hash and signed manifest; its embedded base Git revision remains `707e006f`.

The earlier base-ID index is not a family-wide pfETH index. The source-series
ID `418c0dd528239758d2b684cd9fd06e066aefc5f4cc231a796a279e8cc0a31ad9e3bd474eb1c22847059300a2cce2bae2`
has one ingress and one exit, each 5,000,000 units, with zero remaining pool
balance. `source-series-index.json` records this; privacy coverage remains
incomplete. This repair does not complete the private Hyperliquid funding
adapter or authorize capital deployment.

The rollout is complete. `fleet-deployment.json` records same-host recovery
backups, signed-manifest verification and running process hashes for every
validator. `live-replay-0.json` verifies the deployed history directly.
`fleet-after.json` and `served-rpc-after.json` record exact six-way agreement.
The restart also cleared the previously cached height-971 status response.

The durable local binary is
`/home/postfiat/.local/lib/postfiat/releases/pfeth-replay-v4-20260905/postfiat-node`;
remote services use `/opt/postfiat/releases/pfeth-replay-v4-20260905/postfiat-node`.
Wallet configuration changes are limited to those two runtime paths.
No funds were moved, no historical receipts/balances were rewritten, and no
commits or pushes were made. The broader private funding task remains open.
