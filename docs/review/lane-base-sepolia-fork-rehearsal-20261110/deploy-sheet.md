# Lane deploy sheet (Base Sepolia, chain id 84532)

| Role | Address | Nonce | Predicted contract |
|---|---|---|---|
| verifier deployer | `0x2e115df82aaba1bc7bff4fbb1a090aefdf232876` | 0 | `0x500ab6c6c706cb12243b0b31e7267ac81e6f7cf9` |
| lock deployer | `0x9010af944809a2713c77bd6a441129cde43cc729` | 0 | `0xb902246b118f3989b3808499266cd9f9191c4cc0` |

| Key | Gas on the fork | Test ETH (x3 margin) |
|---|---|---|
| verifier deployer | 2630563 | 0.000047350 |
| lock deployer | 1134485 | 0.000020421 |
| holder | 263732 | 0.000004747 |
| relayer | n/a | release gas cannot be estimated without a real proof; fund it separately |

Program: ELF sha256 `e92f20046c20761bb7a26ec47540000a88fad28b51874453437c3dd390f448ab`, key `0x000cef5bfa4586cfb79e255a950ae8c1dbb15c4270a968ad5bfcbe32d0a81414`, source commit `439803aaa3d6f34dfe97a4a48eca5d6008cba43d`.

Gas price read: 6000000 wei. SP1 verifier: `0xb69f2584cbcff99a58c4e7002e8b89af54a6f4e2` (V6_1_0_SP1_VERIFIER_GROTH16, https://github.com/succinctlabs/sp1-contracts/blob/v6.1.0/contracts/deployments/84532.json).

## Verifier constructor

- `sp1Verifier`: `0xb69f2584cbcff99a58c4e7002e8b89af54a6f4e2`
- `programVKey`: `0x000cef5bfa4586cfb79e255a950ae8c1dbb15c4270a968ad5bfcbe32d0a81414`
- `pftlChainIdHash`: `0x4ea8ee52b27fd187a726848ac2e0a41d4cf2c511922f08404f79d25fdbef878d`
- `pftlGenesisHashCommitment`: `0x6ba299a832e43d41777e6a9bb68f857fff0afbc79ef2d15d8f0718a5aca0b93a`
- `pftlProtocolVersion`: `1`
- `routeProfileHashCommitment`: `0x4e8b4ea6c9337cc34f343a883cc3d6b535a93a8ed0d1b2ec321f9f2249e31433`
- `routeEpoch`: `1`
- `assetIdCommitment`: `0x423cf2a8e8781ae66ef4b6172d6031ed962cf2282b5ef1b77ed0385001ceb014`
- `arbitrumChainId`: `84532`
- `vaultRuntimeCodeHash`: `0x2492247d32d8dae6bfbee36c8b0c65330383fcb3c95c3df322546c6877d9a1e6`
- `token`: `0x90ce303356d80ecbe359faf061c84130f40eacf0`
- `tokenRuntimeCodeHash`: `0x486d2a13ce1250e62b52acd787d2d2ff04aaf2d623f5159cb62ee55b7f1cb76b`
- `maxProofBytes`: `4096`
- `maxPublicValuesBytes`: `16384`
- `initialCheckpointCommitment`: `0x8ef72db2c7002eff0db68368c5ffa91979e7ad27c80c2e91da97fd4d1b64cc13`
- `initialFinalizedHeight`: `1`
- `initialCommitteeRootCommitment`: `0x8a0aa5b410e9be50cddf559298c51e9882f90d40ff6ed414f8d7d7cab76c656a`
