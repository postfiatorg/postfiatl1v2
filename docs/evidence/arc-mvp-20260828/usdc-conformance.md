# Arc testnet USDC conformance

System contract: `0x3600000000000000000000000000000000000000` on chain 5,042,002.

## Confirmed read-only and public-chain behavior

- `name()` returns `USDC`; `symbol()` returns `USDC`; `decimals()` returns `6`.
- `approve(address,uint256)` returns ABI boolean `true` under `eth_call`.
- Live USDC transfers emit the standard `Transfer(address,address,uint256)` topic `0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef`, with indexed `from` and `to` and a 32-byte amount in log data. For example, transaction `0x2489140177b7019182f796b83c73c2e5d4481ca9c853585f72fc94114da496c2` in block 59,325,841 calls selector `0xa9059cbb` and emits that standard log.
- The `Approval(address,address,uint256)` topic expected by the vault flow is `0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925`.
- `ArcUsdcPullProbe` reproduces the vault assumptions: exactly six decimals, boolean-success `transferFrom`, and an exact balance delta (fee-on-transfer behavior rejects). Its local tests cover a successful `1_000_000` atom pull and failure without allowance.

## Funded vault-style pull

The operator `0xdB9b78C87F76054b204188109b35cE4614d03814` received
20,000,000 USDC atoms. `ArcUsdcPullProbe` was then deployed at
`0xDd3B2cd9143b392c32A488607966423FFFEC9292`; its runtime code hash is
`0x133e1c6cd30c0891ab9f9d448756a6eca480fb7c6db2e62401595e0dd672bd13`.

| Action | Transaction | Block | Block hash | Status | Gas used | Effective gas price |
|---|---|---:|---|---:|---:|---:|
| deploy pull probe | `0xb8b598657e7e7d64e576eaf1a5ff2a4192caf037d58db3db7e5e0896f6ffef3a` | 59,331,643 | `0x8c2782028c549f78445db28f9d04f02535947851e7d1c24bf97b52a0a02483b9` | 1 | 254,658 | 21,000,000,000 |
| approve 1,000,000 atoms | `0xb065338b9a25dbff718461a6dc447c2fdab8d0919c8069ab5c2ab059e0834eb9` | 59,331,731 | `0xacb49ba71b413be734d29ddb717557bfc2a3a36d4717529652d34cdfd9f2c2a9` | 1 | 55,438 | 21,000,000,000 |
| `transferFrom` pull 1,000,000 atoms | `0x2d91c7694bece003e9c5de5edd8d76f0ff84549bbd9e098e8ee417c5987c9566` | 59,331,737 | `0xb7373131b1cb2a8a0d92e64693bc7ceb2f75e0e9b69cd69468a586afea5ec1b6` | 1 | 59,788 | 21,000,000,000 |

The approve receipt contains the standard `Approval` log for owner, probe, and
1,000,000 atoms. The pull receipt contains the standard system-USDC `Transfer`
log from the operator to the probe for 1,000,000 atoms and the probe's `Pulled`
event for the same token, sender, and amount. Allowance is zero after the exact
pull.

| Observation | Before approve/pull | After pull |
|---|---:|---:|
| operator system-USDC balance | 19,984,538 atoms | 18,982,118 atoms |
| pull-probe system-USDC balance | 0 atoms | 1,000,000 atoms |
| operator-to-probe allowance | 0 atoms | 0 atoms |
| operator native balance | corresponding system balance less transaction gas | 18,982,118,717,350,000,000 units |
| pull-probe native balance | 0 units | 1,000,000,000,000,000,000 units |

Arc's system-USDC transfer emits two representations in the pull receipt: a
standard `Transfer` at `0x3600...0000` for 1,000,000 six-decimal atoms and a
standard `Transfer` at `0xfffffffffffffffffffffffffffffffffffffffe` for
1,000,000,000,000,000,000 native units. This mirrors one USDC across the ERC-20
and native views; it is not a second economic transfer. Gas debits the operator's
native/system balance, which accounts for the additional 2,420-atom difference
between the one-USDC pull and the before/after system balances.

The live sequence therefore confirms six decimals, boolean-success
`transferFrom`, exact vault-style balance delta, standard approval/transfer
logs, and Arc's gas-token accounting. The production vault deposit is a G1
deployment check, not a remaining G0 conformance dependency. G0.5 is accepted.
