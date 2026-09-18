# Arc precompile conformance and gas

Probe date: 2026-08-28 UTC. RPC: `https://rpc.testnet.arc.network`. The
receipt set was rechecked at block 59,331,927, after 227--289 confirmations.

## Read-only live calls

These calls establish that all four required precompiles exist and return the expected EVM vectors. Gas here is `eth_estimateGas` for a direct EOA call and includes intrinsic transaction gas; it is not the final deployed verifier cost.

| Precompile | Address | Vector | Expected/observed result | Direct estimate |
|---|---|---|---|---:|
| SHA-256 | `0x02` | ASCII `abc` | `0xba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad` | 21,286 |
| BN254 add | `0x06` | `G + G` | standard `2G` coordinates | 22,735 |
| BN254 mul | `0x07` | `G * 2` | same standard `2G` coordinates | 27,765 |
| BN254 pairing | `0x08` | empty product | 32-byte integer `1` | 66,845 |

The observed `2G` coordinates are:

```text
x = 030644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd3
y = 15ed738c0e0a7c92e7845f96b2ae9c0a68a6a449e3538fc7ff3ebf7a5a18a2c4
```

## Contract probe

`crates/ethereum-contracts/src/ArcConformanceProbe.sol` exercises the same vectors through Solidity and rejects malformed pairing input. Local Foundry execution passes all four tests. The local function gas reported by Foundry was 7,175 for SHA-256, 17,483 for the combined add/mul test, and 51,582 for empty pairing; those are test-harness measurements, not Arc transaction receipts.

The probe was deployed at `0x1766039cD6EeE04EF1ff2B3e882d222D78dbf793`.
Its runtime code hash is
`0x38804c6f4b84987ae55333704ce191cacd963b3796438e279fe6f2e4143656db`.

| Action | Transaction | Block | Block hash | Status | Gas used | Effective gas price | Gas cost |
|---|---|---:|---|---:|---:|---:|---:|
| deploy | `0x7353c0108ed26999a03bc3e39c615fc9b775acc13381c90876f0d0b2ce5f2c36` | 59,331,639 | `0x4c80142d887c3b098e9564213e7e54b5384b9ec81d58e6724df9cfb87c873120` | 1 | 333,518 | 21,000,000,000 | 7,003,878,000,000,000 |
| SHA-256 `abc` | `0x7b1bac1b24bd1a7df7d062a60c1decd88744f40ae39e2260e741291d24ebac98` | 59,331,686 | `0x37545b8da66e2eef739f38e94fa0cdd7800609d0efc7d74cbe2c999b738b5288` | 1 | 22,516 | 21,000,000,000 | 472,836,000,000,000 |
| BN254 add `G + G` | `0x0fc514409d03f8d68457fd2be1810173adfce6edd435830058fef98749bb444d` | 59,331,691 | `0x1eb1089859cbb553b9c0dace78d3397c01bd011411053f4988f0711ab452bb5d` | 1 | 23,565 | 21,000,000,000 | 494,865,000,000,000 |
| BN254 multiply `G * 2` | `0x0de99730caa16ffee90906d3493bdb9b257d1c8efe3f322f474a1244cc84f00f` | 59,331,697 | `0x3438fe70be2657a1af29d93527009dd54ebcba7889dd3a1b072ae832051d52d4` | 1 | 29,313 | 21,000,000,000 | 615,573,000,000,000 |
| BN254 empty pairing | `0x3bf85ae51593714f4dfb62647791a16ef066196b4240f4d8f3c244fcd0132245` | 59,331,701 | `0xaa6cc6388858a101a8b6f892589ac2d8acd9e368a4b673ac7bf0baed41a899a1` | 1 | 67,622 | 22,575,000,000 | 1,526,566,650,000,000 |

Read-only calls to the deployed contract returned the same expected vectors.
The contract's `gasleft()` deltas isolate the internal call path from transaction
intrinsic and ABI-dispatch costs:

| Probe | Returned value | Internal gas delta |
|---|---|---:|
| SHA-256 | `0xba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad` | 465 |
| BN254 add | standard `2G` coordinates above | 294 |
| BN254 multiply | standard `2G` coordinates above | 6,144 |
| BN254 empty pairing | `true` | 45,144 |

Every deployment and call receipt has status `1`. G0.4 is accepted.
